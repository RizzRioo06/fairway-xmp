use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    task::{Poll, Waker},
};

/// Both stacks must get a cleanup attempt even when either is unavailable.
/// Keep this separate from PHP Start All/Stop All, which never controls Node.
pub(crate) fn stop_components(
    node: impl FnOnce() -> Result<(), String>,
    php: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let errors: Vec<_> = [node(), php()]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

#[derive(Default)]
struct State {
    working: bool,
    quitting: bool,
    exit_allowed: bool,
    installing: bool,
    download: Option<Arc<DownloadState>>,
}

/// Coordinates panel/tray operations and explicit quit requests. Quit reserves
/// shutdown immediately, then waits for the current operation to finish.
#[derive(Clone, Default)]
pub(crate) struct Lifecycle(Arc<Inner>);

#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    completed: Condvar,
}

impl Lifecycle {
    pub fn begin(&self) -> Result<Operation, String> {
        let mut state = self.0.state.lock().unwrap();
        if state.quitting || state.exit_allowed || state.installing {
            return Err("Fairway XMP is quitting. Wait for shutdown to finish.".into());
        }
        if state.working {
            return Err("An operation is already in progress. Please wait.".into());
        }
        state.working = true;
        Ok(Operation(self.clone()))
    }

    pub fn request_quit(&self) -> bool {
        let mut state = self.0.state.lock().unwrap();
        if state.quitting || state.exit_allowed || state.installing {
            return false;
        }
        state.quitting = true;
        let download = state.download.clone();
        drop(state);
        if let Some(download) = download {
            download.cancel();
        }
        true
    }

    pub fn wait_for_operation(&self) {
        let _state = self
            .0
            .completed
            .wait_while(self.0.state.lock().unwrap(), |state| state.working)
            .unwrap();
    }

    pub fn cancel_quit(&self) {
        self.0.state.lock().unwrap().quitting = false;
    }

    pub fn allow_exit(&self) {
        self.0.state.lock().unwrap().exit_allowed = true;
    }

    /// Reserve bundle replacement atomically against an explicit Quit. The
    /// caller already owns the normal operation guard. A failed install drops
    /// this reservation so services and Quit can be used again.
    pub fn begin_install(&self) -> Result<Installation, String> {
        let mut state = self.0.state.lock().unwrap();
        if state.quitting || state.exit_allowed || state.installing || !state.working {
            return Err("Fairway XMP is shutting down. The update was not installed.".into());
        }
        state.installing = true;
        Ok(Installation(self.clone()))
    }

    pub fn exit_allowed(&self) -> bool {
        self.0.state.lock().unwrap().exit_allowed
    }

    pub fn quitting(&self) -> bool {
        let state = self.0.state.lock().unwrap();
        state.quitting || state.exit_allowed || state.installing
    }

    pub fn busy(&self) -> bool {
        let state = self.0.state.lock().unwrap();
        state.working || state.quitting || state.exit_allowed
    }

    pub fn begin_download(&self) -> Result<Download, String> {
        let mut state = self.0.state.lock().unwrap();
        if state.quitting || state.exit_allowed || state.installing {
            return Err("Fairway XMP is quitting. The update was not downloaded.".into());
        }
        if state.download.is_some() {
            return Err("An update is already downloading.".into());
        }
        let signal = Arc::new(DownloadState::default());
        state.download = Some(signal.clone());
        Ok(Download {
            lifecycle: self.clone(),
            signal,
        })
    }
}

#[derive(Default)]
struct DownloadState {
    cancelled: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl DownloadState {
    fn wake(&self) {
        let waker = self.waker.lock().unwrap().take();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.wake();
    }
}

pub(crate) struct Download {
    lifecycle: Lifecycle,
    signal: Arc<DownloadState>,
}

impl Download {
    pub async fn run<T>(
        &self,
        future: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let mut future = std::pin::pin!(future);
        std::future::poll_fn(|context| {
            *self.signal.waker.lock().unwrap() = Some(context.waker().clone());
            if self.signal.cancelled.load(Ordering::SeqCst) {
                return Poll::Ready(Err(
                    "The update was cancelled because Fairway XMP is quitting.".into(),
                ));
            }
            future.as_mut().poll(context)
        })
        .await
    }

    pub async fn begin_operation(&self) -> Result<Operation, String> {
        std::future::poll_fn(|context| {
            let mut state = self.lifecycle.0.state.lock().unwrap();
            if state.quitting
                || state.exit_allowed
                || state.installing
                || self.signal.cancelled.load(Ordering::SeqCst)
            {
                return Poll::Ready(Err(
                    "Fairway XMP is shutting down. The update was not installed.".into(),
                ));
            }
            if state.working {
                *self.signal.waker.lock().unwrap() = Some(context.waker().clone());
                return Poll::Pending;
            }
            state.working = true;
            Poll::Ready(Ok(Operation(self.lifecycle.clone())))
        })
        .await
    }
}

impl Drop for Download {
    fn drop(&mut self) {
        let mut state = self.lifecycle.0.state.lock().unwrap();
        if state
            .download
            .as_ref()
            .is_some_and(|signal| Arc::ptr_eq(signal, &self.signal))
        {
            state.download = None;
        }
    }
}

pub(crate) struct Operation(Lifecycle);

pub(crate) struct Installation(Lifecycle);

impl Drop for Installation {
    fn drop(&mut self) {
        self.0 .0.state.lock().unwrap().installing = false;
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        let mut state = self.0 .0.state.lock().unwrap();
        state.working = false;
        let download = state.download.clone();
        drop(state);
        if let Some(download) = download {
            download.wake();
        }
        self.0 .0.completed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct WakeCounter(std::sync::atomic::AtomicUsize);
    impl std::task::Wake for WakeCounter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn downloading_does_not_block_services_and_quit_drops_a_stalled_request() {
        struct Request(Arc<AtomicBool>);
        impl Future for Request {
            type Output = Result<(), String>;
            fn poll(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> Poll<Self::Output> {
                Poll::Pending
            }
        }
        impl Drop for Request {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let lifecycle = Lifecycle::default();
        let download = lifecycle.begin_download().unwrap();
        assert!(!lifecycle.busy());
        assert!(lifecycle.begin_download().is_err());
        drop(lifecycle.begin().unwrap());
        let dropped = Arc::new(AtomicBool::new(false));
        let counter = Arc::new(WakeCounter(std::sync::atomic::AtomicUsize::new(0)));
        let waker = std::task::Waker::from(counter.clone());
        let mut context = std::task::Context::from_waker(&waker);
        let mut pending = Box::pin(download.run(Request(dropped.clone())));
        assert!(pending.as_mut().poll(&mut context).is_pending());
        assert!(lifecycle.request_quit());
        lifecycle.wait_for_operation();
        assert!(counter.0.load(Ordering::SeqCst) > 0);
        assert!(matches!(
            pending.as_mut().poll(&mut context),
            Poll::Ready(Err(_))
        ));
        assert!(dropped.load(Ordering::SeqCst));
        assert!(lifecycle.begin_install().is_err());
        drop(pending);
        drop(download);
        lifecycle.cancel_quit();
        drop(lifecycle.begin_download().unwrap());
        drop(lifecycle.begin().unwrap());
    }

    #[test]
    fn verified_download_waits_for_active_work_then_reserves_installation() {
        let lifecycle = Lifecycle::default();
        let download = lifecycle.begin_download().unwrap();
        let operation = lifecycle.begin().unwrap();
        let waker = std::task::Waker::from(Arc::new(WakeCounter(
            std::sync::atomic::AtomicUsize::new(0),
        )));
        let mut context = std::task::Context::from_waker(&waker);
        let mut acquire = Box::pin(download.begin_operation());
        assert!(acquire.as_mut().poll(&mut context).is_pending());
        drop(operation);
        let Poll::Ready(Ok(operation)) = acquire.as_mut().poll(&mut context) else {
            panic!("verified update did not acquire operation");
        };
        let installation = lifecycle.begin_install().unwrap();
        assert!(!lifecycle.request_quit());
        assert!(lifecycle.begin().is_err());
        drop(installation);
        drop(operation);
        drop(acquire);
        drop(download);
        drop(lifecycle.begin().unwrap());
    }

    #[test]
    fn quit_prevents_installation_while_verified_update_waits_for_work() {
        let lifecycle = Lifecycle::default();
        let download = lifecycle.begin_download().unwrap();
        let operation = lifecycle.begin().unwrap();
        let waker = std::task::Waker::from(Arc::new(WakeCounter(
            std::sync::atomic::AtomicUsize::new(0),
        )));
        let mut context = std::task::Context::from_waker(&waker);
        let mut acquire = Box::pin(download.begin_operation());
        assert!(acquire.as_mut().poll(&mut context).is_pending());
        assert!(lifecycle.request_quit());
        drop(operation);
        assert!(matches!(
            acquire.as_mut().poll(&mut context),
            Poll::Ready(Err(_))
        ));
        assert!(lifecycle.begin_install().is_err());
    }

    #[test]
    fn installation_and_quit_cannot_race_and_failure_allows_retry() {
        let lifecycle = Lifecycle::default();
        assert!(lifecycle.begin_install().is_err());
        let operation = lifecycle.begin().unwrap();
        let installing = lifecycle.begin_install().unwrap();
        assert!(!lifecycle.request_quit());
        assert!(lifecycle.begin().is_err());
        drop(installing);
        assert!(!lifecycle.quitting());
        drop(operation);
        let operation = lifecycle.begin().unwrap();
        assert!(lifecycle.request_quit());
        assert!(lifecycle.begin_install().is_err());
        drop(operation);
    }

    #[test]
    fn quit_waits_for_work_and_rejects_new_operations() {
        let lifecycle = Lifecycle::default();
        let operation = lifecycle.begin().unwrap();
        assert!(lifecycle.begin().is_err());
        assert!(lifecycle.request_quit());
        assert!(!lifecycle.request_quit());
        assert!(!lifecycle.exit_allowed());
        assert!(lifecycle.begin().is_err());

        let waiting = lifecycle.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            waiting.wait_for_operation();
            send.send(()).unwrap();
        });
        assert!(receive.try_recv().is_err());
        drop(operation);
        receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        thread.join().unwrap();
        assert!(lifecycle.busy());
        assert!(lifecycle.quitting());
        assert!(!lifecycle.exit_allowed());
        lifecycle.allow_exit();
        assert!(lifecycle.exit_allowed());
        assert!(lifecycle.begin().is_err());
    }

    #[test]
    fn shutdown_attempts_both_stacks_and_preserves_both_errors() {
        use std::cell::Cell;
        for node_fails in [false, true] {
            for php_fails in [false, true] {
                let calls = Cell::new(0);
                let result = stop_components(
                    || {
                        calls.set(calls.get() + 1);
                        if node_fails {
                            Err("node".into())
                        } else {
                            Ok(())
                        }
                    },
                    || {
                        calls.set(calls.get() + 1);
                        if php_fails {
                            Err("php".into())
                        } else {
                            Ok(())
                        }
                    },
                );
                assert_eq!(calls.get(), 2);
                match (node_fails, php_fails) {
                    (false, false) => assert!(result.is_ok()),
                    (true, false) => assert_eq!(result.unwrap_err(), "node"),
                    (false, true) => assert_eq!(result.unwrap_err(), "php"),
                    (true, true) => assert_eq!(result.unwrap_err(), "node\nphp"),
                }
            }
        }
    }

    #[test]
    fn failed_shutdown_can_be_retried() {
        let lifecycle = Lifecycle::default();
        assert!(lifecycle.request_quit());
        lifecycle.wait_for_operation();
        lifecycle.cancel_quit();
        assert!(!lifecycle.busy());
        assert!(!lifecycle.exit_allowed());
        drop(lifecycle.begin().unwrap());
        assert!(lifecycle.request_quit());
    }
}
