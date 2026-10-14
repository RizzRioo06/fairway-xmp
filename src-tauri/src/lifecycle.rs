use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct State {
    working: bool,
    quitting: bool,
    exit_allowed: bool,
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
        if state.quitting || state.exit_allowed {
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
        if state.quitting || state.exit_allowed {
            return false;
        }
        state.quitting = true;
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

    pub fn exit_allowed(&self) -> bool {
        self.0.state.lock().unwrap().exit_allowed
    }

    pub fn quitting(&self) -> bool {
        let state = self.0.state.lock().unwrap();
        state.quitting || state.exit_allowed
    }

    pub fn busy(&self) -> bool {
        let state = self.0.state.lock().unwrap();
        state.working || state.quitting || state.exit_allowed
    }
}

pub(crate) struct Operation(Lifecycle);

impl Drop for Operation {
    fn drop(&mut self) {
        self.0 .0.state.lock().unwrap().working = false;
        self.0 .0.completed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
