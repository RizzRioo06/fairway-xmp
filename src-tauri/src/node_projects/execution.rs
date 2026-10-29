#[cfg(target_os = "macos")]
use super::run_logs::LogStream;
#[cfg(target_os = "macos")]
use super::runner::{self, Control, Event};
use super::{
    run_logs::{self, LogBatch, RunLog},
    runner::Launch,
    Error, ErrorCode, ProjectRuntime, Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Exited,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunStatus {
    pub project_id: String,
    pub run_id: String,
    pub revision: u64,
    pub state: RunState,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub error: Option<Error>,
    pub forced_cleanup: bool,
    pub runtime: ProjectRuntime,
    /// Current run's reported loopback address; no HTTP readiness check is implied.
    #[serde(default)]
    pub reported_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRequest {
    pub id: String,
    pub expected_revision: String,
    pub manifest_revision: String,
    pub confirmed: bool,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StopRequest {
    pub id: String,
    pub run_id: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestartRequest {
    pub id: String,
    pub run_id: String,
    pub expected_revision: String,
    pub manifest_revision: String,
    pub confirmed: bool,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogRequest {
    pub id: String,
    pub run_id: String,
    pub after_cursor: u64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunsSnapshot {
    pub supported: bool,
    pub runs: Vec<RunStatus>,
}

type Observer = Arc<dyn Fn(RunStatus) + Send + Sync>;
pub(super) struct Run {
    status: Mutex<RunStatus>,
    active: AtomicBool,
    stopping: AtomicBool,
    finished: Condvar,
    log: Mutex<RunLog>,
    observer: Option<Observer>,
    metadata: PathBuf,
    #[cfg(target_os = "macos")]
    control: Mutex<Option<std::os::unix::net::UnixStream>>,
}
impl Run {
    fn status(&self) -> RunStatus {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn change(&self, update: impl FnOnce(&mut RunStatus)) {
        let status = {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            update(&mut status);
            status.revision += 1;
            self.persist(&status);
            status.clone()
        };
        if let Some(observer) = &self.observer {
            observer(status);
        }
    }
    fn persist(&self, status: &RunStatus) {
        if let Ok(bytes) = serde_json::to_vec(status) {
            let _ = run_logs::persist_metadata(&self.metadata, &bytes);
        }
    }
    fn fail(&self, error: Error) {
        self.change(|s| {
            s.error = Some(error);
            s.state = RunState::Failed;
        });
    }
    fn stop(&self) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }
        let status = {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            if !self.active.load(Ordering::SeqCst) {
                return;
            }
            self.stopping.store(true, Ordering::SeqCst);
            status.state = RunState::Stopping;
            status.revision += 1;
            self.persist(&status);
            status.clone()
        };
        if let Some(observer) = &self.observer {
            observer(status);
        }
        #[cfg(target_os = "macos")]
        if let Ok(mut control) = self.control.lock() {
            if let Some(socket) = control.as_mut() {
                let _ = runner::send(socket, &Control::Stop);
            }
        }
    }
    fn disconnect(&self) {
        #[cfg(target_os = "macos")]
        if let Ok(mut control) = self.control.lock() {
            if let Some(socket) = control.take() {
                let _ = socket.shutdown(std::net::Shutdown::Both);
            }
        }
    }
    fn wait(&self, deadline: Instant) -> Result<RunStatus> {
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        while self.active.load(Ordering::SeqCst) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Error::new(
                    ErrorCode::ExecutionFailed,
                    "Node cleanup has not completed. Runtime ownership is retained; retry Stop.",
                ));
            }
            status = self
                .finished
                .wait_timeout(status, remaining)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        Ok(status.clone())
    }
    fn complete(&self) {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).finish();
        let status = {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            status.state = if self.stopping.load(Ordering::SeqCst) {
                RunState::Stopped
            } else if status.error.is_some() || status.exit_code != Some(0) {
                RunState::Failed
            } else {
                RunState::Exited
            };
            status.ended_at = Some(now());
            status.revision += 1;
            self.persist(&status);
            self.active.store(false, Ordering::SeqCst);
            status.clone()
        };
        self.finished.notify_all();
        if let Some(observer) = &self.observer {
            observer(status);
        }
    }
}

pub(crate) struct Executions {
    data: Option<PathBuf>,
    runs: Mutex<BTreeMap<String, VecDeque<Arc<Run>>>>,
    observer: Mutex<Option<Observer>>,
    accepting: AtomicBool,
    #[cfg(target_os = "macos")]
    executable: Mutex<Option<PathBuf>>,
}

impl Executions {
    pub fn new(data: Option<PathBuf>) -> Self {
        Self {
            data,
            runs: Mutex::new(BTreeMap::new()),
            observer: Mutex::new(None),
            accepting: AtomicBool::new(true),
            #[cfg(target_os = "macos")]
            executable: Mutex::new(None),
        }
    }
    pub fn set_observer(&self, observer: impl Fn(RunStatus) + Send + Sync + 'static) {
        *self.observer.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(observer));
    }
    pub fn ensure_stopped(&self, id: &str) -> Result<()> {
        if self
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .is_some_and(|runs| runs.iter().any(|run| run.active.load(Ordering::SeqCst)))
        {
            return Err(Error::new(
                ErrorCode::ProjectBusy,
                "Stop this Node project before editing, removing, or starting it again.",
            ));
        }
        Ok(())
    }
    pub fn forget(&self, id: &str) {
        // Called only after successful removal while the registry lock is held.
        self.runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
    }
    pub fn ensure_runtime_unused(&self, node: &Path) -> Result<()> {
        let canonical = std::fs::canonicalize(node).unwrap_or_else(|_| node.into());
        let busy = self
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .flatten()
            .any(|run| {
                let status = run.status();
                run.active.load(Ordering::SeqCst)
                    && (Path::new(&status.runtime.node_path) == node
                        || Path::new(&status.runtime.node_path) == canonical)
            });
        if busy {
            return Err(Error::new(ErrorCode::ProjectBusy, "This Node runtime is used by a starting, running, or stopping project. Stop its projects first."));
        }
        Ok(())
    }
    pub fn statuses(&self) -> Vec<RunStatus> {
        let mut statuses: BTreeMap<_, _> = self
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter_map(|(id, runs)| runs.back().map(|run| (id.clone(), run.status())))
            .collect();
        if let Some(data) = &self.data {
            for entry in std::fs::read_dir(data.join("node-project-logs"))
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .take(64)
            {
                let id = entry.file_name().to_string_lossy().into_owned();
                if !run_logs::valid_id(&id)
                    || statuses.contains_key(&id)
                    || !entry.file_type().is_ok_and(|t| t.is_dir())
                {
                    continue;
                }
                if let Ok(Some(bytes)) =
                    super::manifest::read_regular(&entry.path().join("last-run.json"), 65536)
                {
                    if let Ok(mut status) = serde_json::from_slice::<RunStatus>(&bytes) {
                        if status.project_id != id {
                            continue;
                        }
                        if matches!(
                            status.state,
                            RunState::Starting | RunState::Running | RunState::Stopping
                        ) {
                            status.state = RunState::Failed;
                            status.error = Some(Error::new(ErrorCode::ExecutionFailed, "The previous session ended. No process was adopted; retry Start after cleanup."));
                        }
                        statuses.insert(id, status);
                    }
                }
            }
        }
        statuses.into_values().collect()
    }
    fn current(&self, id: &str, run_id: &str) -> Result<Arc<Run>> {
        let runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
        let run = runs.get(id).and_then(|runs| runs.back()).ok_or_else(|| {
            Error::new(
                ErrorCode::RunNotFound,
                "This managed Node run is unavailable.",
            )
        })?;
        if run.status().run_id != run_id {
            return Err(Error::new(
                ErrorCode::StaleRun,
                "This command refers to an older run. Refresh its status.",
            ));
        }
        Ok(run.clone())
    }
    // A MainExit event precedes the helper's final descendant cleanup. Finite
    // installers must wait for reaping, without changing Exited into Stopped.
    pub fn wait_for_exit(&self, request: StopRequest) -> Result<RunStatus> {
        self.current(&request.id, &request.run_id)?
            .wait(Instant::now() + Duration::from_secs(22))
    }
    pub fn stop(&self, request: StopRequest) -> Result<RunStatus> {
        let run = self.current(&request.id, &request.run_id)?;
        run.stop();
        run.wait(Instant::now() + Duration::from_secs(22))
    }
    pub fn stop_all(&self) -> Result<()> {
        let runs: Vec<_> = self
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .flatten()
            .filter(|run| run.active.load(Ordering::SeqCst))
            .cloned()
            .collect();
        for run in &runs {
            run.stop();
        }
        let deadline = Instant::now() + Duration::from_secs(22);
        let errors: Vec<_> = runs
            .iter()
            .filter_map(|run| run.wait(deadline).err().map(|e| e.message))
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::new(ErrorCode::ExecutionFailed, errors.join("\n")))
        }
    }
    pub fn disconnect_all(&self) {
        self.accepting.store(false, Ordering::SeqCst);
        for run in self
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .flatten()
        {
            run.stopping.store(true, Ordering::SeqCst);
            run.disconnect();
        }
    }
    pub fn logs(&self, request: LogRequest) -> Result<LogBatch> {
        if !run_logs::valid_id(&request.id) || !run_logs::valid_id(&request.run_id) {
            return Err(Error::new(
                ErrorCode::LogUnavailable,
                "Invalid log identity.",
            ));
        }
        let found = self
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&request.id)
            .and_then(|runs| {
                runs.iter()
                    .find(|run| run.status().run_id == request.run_id)
            })
            .cloned();
        if let Some(run) = found {
            return Ok(run
                .log
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .read(request.after_cursor, !run.active.load(Ordering::SeqCst)));
        }
        run_logs::read_disk(
            self.data.as_deref().ok_or_else(|| {
                Error::new(
                    ErrorCode::LogUnavailable,
                    "Application data is unavailable.",
                )
            })?,
            &request.id,
            &request.run_id,
            request.after_cursor,
        )
    }
    pub fn start(&self, launch: Launch, runtime: ProjectRuntime) -> Result<RunStatus> {
        if !self.accepting.load(Ordering::SeqCst) {
            return Err(Error::new(
                ErrorCode::ExecutionFailed,
                "Node execution is shutting down.",
            ));
        }
        let mut runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
        // Recheck under the same lock used by disconnect_all: an exit that won
        // the race must not miss a newly inserted run.
        if !self.accepting.load(Ordering::SeqCst) {
            return Err(Error::new(
                ErrorCode::ExecutionFailed,
                "Node execution is shutting down.",
            ));
        }
        if runs
            .get(&launch.id)
            .is_some_and(|runs| runs.iter().any(|run| run.active.load(Ordering::SeqCst)))
        {
            return Err(Error::new(
                ErrorCode::ProjectBusy,
                "This project already has a managed run.",
            ));
        }
        if runs
            .values()
            .flatten()
            .filter(|run| run.active.load(Ordering::SeqCst))
            .count()
            >= 8
        {
            return Err(Error::new(
                ErrorCode::ProjectBusy,
                "At most eight Node projects may run concurrently.",
            ));
        }
        let status = RunStatus {
            project_id: launch.id.clone(),
            run_id: launch.run_id.clone(),
            revision: 1,
            state: RunState::Starting,
            created_at: now(),
            started_at: None,
            ended_at: None,
            exit_code: None,
            signal: None,
            error: None,
            forced_cleanup: false,
            runtime,
            reported_url: None,
        };
        let log = RunLog::new(&launch.data, &launch.id, &launch.run_id)?;
        let metadata = run_logs::directory(&launch.data, &launch.id)?.join("last-run.json");
        let run = Arc::new(Run {
            status: Mutex::new(status.clone()),
            active: AtomicBool::new(true),
            stopping: AtomicBool::new(false),
            finished: Condvar::new(),
            log: Mutex::new(log),
            metadata,
            observer: self
                .observer
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            #[cfg(target_os = "macos")]
            control: Mutex::new(None),
        });
        // Publish initial metadata before another thread can find and stop the run.
        run.persist(&status);
        let history = runs.entry(launch.id.clone()).or_default();
        history.push_back(run.clone());
        while history.len() > 2 {
            history.pop_front();
        }
        drop(runs);
        if let Some(observer) = &run.observer {
            observer(status.clone());
        }
        #[cfg(target_os = "macos")]
        {
            let executable = match self
                .executable
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .map(Ok)
                .unwrap_or_else(std::env::current_exe)
            {
                Ok(executable) => executable,
                Err(error) => {
                    let error = Error::new(ErrorCode::ExecutionFailed, error.to_string());
                    run.fail(error.clone());
                    run.complete();
                    return Err(error);
                }
            };
            let worker = run.clone();
            if let Err(error) = std::thread::Builder::new()
                .name("fairway-node-run".into())
                .spawn(move || native_worker(worker, executable, launch))
            {
                run.fail(Error::new(ErrorCode::ExecutionFailed, error.to_string()));
                run.complete();
                return Err(Error::new(
                    ErrorCode::ExecutionFailed,
                    "Could not create the Node runner worker.",
                ));
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = launch;
            run.fail(Error::new(
                ErrorCode::UnsupportedPlatform,
                "Node execution is currently available on macOS only.",
            ));
            run.complete();
        }
        Ok(status)
    }
    #[cfg(all(test, target_os = "macos"))]
    pub fn test_runner(&self, executable: PathBuf) {
        *self.executable.lock().unwrap() = Some(executable);
    }
}
impl Drop for Executions {
    fn drop(&mut self) {
        self.disconnect_all();
    }
}

#[cfg(target_os = "macos")]
fn native_worker(run: Arc<Run>, executable: PathBuf, launch: Launch) {
    use std::{io::BufReader, os::unix::net::UnixStream, process::Stdio};
    let result = (|| -> Result<()> {
        let (mut socket, child_socket) = UnixStream::pair().map_err(exec_error)?;
        socket
            .set_write_timeout(Some(Duration::from_millis(500)))
            .map_err(exec_error)?;
        let mut command = crate::process::command(executable);
        command
            .arg(runner::FLAG)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HOME", &launch.home)
            .env("TMPDIR", "/tmp");
        for key in ["USER", "LOGNAME", "LANG", "LC_ALL", "LC_CTYPE", "TZ"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        let cleanup_socket = socket.try_clone().map_err(exec_error)?;
        let control_socket = socket.try_clone().map_err(exec_error)?;
        let child = command
            .stdin(Stdio::from(std::os::fd::OwnedFd::from(child_socket)))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(exec_error)?;
        // Command retains configured Stdio handles after spawn. Drop the
        // parent's copy of the peer endpoint or normal runner exit cannot EOF.
        drop(command);
        let mut owned = HelperChild {
            child,
            socket: cleanup_socket,
            armed: true,
        };
        *run.control.lock().unwrap_or_else(|e| e.into_inner()) = Some(control_socket);
        runner::send(&mut socket, &Control::Launch(Box::new(launch))).map_err(exec_error)?;
        if run.stopping.load(Ordering::SeqCst) {
            run.stop();
        }
        let mut reader = BufReader::new(socket);
        let mut finished = false;
        let mut forced = false;
        let mut reported = super::local_url::ReportedUrl::default();
        loop {
            let event = runner::receive::<Event>(&mut reader).map_err(exec_error)?;
            let Some(event) = event else {
                break;
            };
            match event {
                Event::Started => run.change(|s| {
                    s.started_at = Some(now());
                    if !run.stopping.load(Ordering::SeqCst) {
                        s.state = RunState::Running;
                    }
                }),
                Event::Output { stream, data } => {
                    if data.len() > 4096 || stream == LogStream::System {
                        return Err(Error::new(
                            ErrorCode::ExecutionFailed,
                            "Invalid runner output frame.",
                        ));
                    }
                    if stream == LogStream::Stdout {
                        if let Some(url) = reported.output(&data) {
                            if run.status().reported_url.as_ref() != Some(&url) {
                                run.change(|s| s.reported_url = Some(url));
                            }
                        }
                    }
                    run.log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .output(stream, &data);
                }
                Event::MainExit { code, signal } => run.change(|s| {
                    s.exit_code = code;
                    s.signal = signal;
                    s.state = RunState::Stopping;
                }),
                Event::Failure { error } => run.fail(error),
                Event::ForcedCleanup => {
                    forced = true;
                    run.change(|s| s.forced_cleanup = true);
                }
                Event::Finished => {
                    finished = true;
                }
            }
        }
        let status = owned.child.wait().map_err(exec_error)?;
        owned.armed = false;
        run.control.lock().unwrap_or_else(|e| e.into_inner()).take();
        if !finished && !forced && run.status().error.is_none() {
            return Err(Error::new(
                ErrorCode::ExecutionFailed,
                format!("The Node runner disconnected unexpectedly ({status})."),
            ));
        }
        Ok(())
    })();
    if let Err(error) = result {
        run.fail(error);
        run.disconnect();
    }
    run.log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .append(LogStream::System, "Managed run ended.\n".into());
    run.complete();
}
#[cfg(target_os = "macos")]
fn exec_error(error: std::io::Error) -> Error {
    Error::new(ErrorCode::ExecutionFailed, error.to_string())
}

#[cfg(target_os = "macos")]
struct HelperChild {
    child: std::process::Child,
    socket: std::os::unix::net::UnixStream,
    armed: bool,
}
#[cfg(target_os = "macos")]
impl Drop for HelperChild {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.socket.shutdown(std::net::Shutdown::Both);
            let _ = self.child.wait();
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod finite_completion_tests {
    use super::*;
    #[test]
    fn exit_or_failure_status_is_not_reaping_and_timeout_keeps_runtime_owned() {
        for state in [RunState::Exited, RunState::Failed] {
            let root =
                std::env::temp_dir().join(format!("fw-reap-wait-{:032x}", rand::random::<u128>()));
            std::fs::create_dir(&root).unwrap();
            let executions = Arc::new(Executions::new(Some(root.clone())));
            let runtime = ProjectRuntime {
                node_path: root.join("selected-node").to_str().unwrap().into(),
                version: "v22.23.4".into(),
            };
            let id = "a".repeat(32);
            let run_id = "b".repeat(32);
            let run = Arc::new(Run {
                status: Mutex::new(RunStatus {
                    project_id: id.clone(),
                    run_id: run_id.clone(),
                    revision: 1,
                    state,
                    created_at: now(),
                    started_at: Some(now()),
                    ended_at: None,
                    exit_code: Some(0),
                    signal: None,
                    error: if state == RunState::Failed {
                        Some(Error::new(
                            ErrorCode::ExecutionFailed,
                            "Controlled early failure event",
                        ))
                    } else {
                        None
                    },
                    forced_cleanup: false,
                    runtime: runtime.clone(),
                    reported_url: None,
                }),
                active: AtomicBool::new(true),
                stopping: AtomicBool::new(false),
                finished: Condvar::new(),
                log: Mutex::new(RunLog::new(&root, &id, &run_id).unwrap()),
                observer: None,
                metadata: root.join("status.json"),
                control: Mutex::new(None),
            });
            executions
                .runs
                .lock()
                .unwrap()
                .insert(id.clone(), VecDeque::from([run.clone()]));
            let before = Instant::now();
            assert!(run.wait(before + Duration::from_millis(30)).is_err());
            assert!(before.elapsed() < Duration::from_secs(2));
            assert!(run.active.load(Ordering::SeqCst));
            assert!(executions
                .ensure_runtime_unused(Path::new(&runtime.node_path))
                .is_err());
            let returned = Arc::new(AtomicBool::new(false));
            let observed = returned.clone();
            let waiting = executions.clone();
            let waiter = std::thread::spawn(move || {
                let status = waiting.wait_for_exit(StopRequest { id, run_id }).unwrap();
                observed.store(true, Ordering::SeqCst);
                status
            });
            std::thread::sleep(Duration::from_millis(50));
            assert!(!returned.load(Ordering::SeqCst));
            // Model the native worker's child.wait() followed by complete().
            run.complete();
            let status = waiter.join().unwrap();
            assert_eq!(status.state, state);
            assert!(executions
                .ensure_runtime_unused(Path::new(&runtime.node_path))
                .is_ok());
            drop(executions);
            drop(run);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
