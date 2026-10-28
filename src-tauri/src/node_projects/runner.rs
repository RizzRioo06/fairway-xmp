//! Private, inherited-socket protocol. No public listener or persisted PID authority.
#[cfg(any(target_os = "macos", test))]
use super::run_logs::LogStream;
#[cfg(target_os = "macos")]
use super::PrepareRuntimeRequest;
use super::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "macos", test))]
use std::io::{BufRead, Read, Write};

pub(super) const FLAG: &str = "--node-project-runner";
#[cfg(any(target_os = "macos", test))]
const FRAME_LIMIT: u64 = 128 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Launch {
    pub home: std::path::PathBuf,
    pub data: std::path::PathBuf,
    pub runtime: std::path::PathBuf,
    pub id: String,
    pub run_id: String,
    pub expected_revision: String,
    pub manifest_revision: String,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum Control {
    Launch(Launch),
    Stop,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum Event {
    Started,
    Output {
        stream: LogStream,
        data: Vec<u8>,
    },
    MainExit {
        code: Option<i32>,
        signal: Option<i32>,
    },
    Failure {
        error: Error,
    },
    ForcedCleanup,
    Finished,
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn send<T: Serialize>(writer: &mut impl Write, value: &T) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 >= FRAME_LIMIT {
        return Err(std::io::Error::other("Runner frame too large"));
    }
    bytes.push(b'\n');
    writer.write_all(&bytes)
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn receive<T: serde::de::DeserializeOwned>(
    reader: &mut impl BufRead,
) -> std::io::Result<Option<T>> {
    let mut bytes = Vec::new();
    let count = reader.take(FRAME_LIMIT + 1).read_until(b'\n', &mut bytes)?;
    if count == 0 {
        return Ok(None);
    }
    if count as u64 > FRAME_LIMIT || !bytes.ends_with(b"\n") {
        return Err(std::io::Error::other("Invalid runner frame"));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(std::io::Error::other)
}

pub fn entrypoint() -> Result<bool> {
    if std::env::args_os().nth(1).is_none_or(|arg| arg != FLAG) {
        return Ok(false);
    }
    #[cfg(target_os = "macos")]
    {
        native::run().map(|()| true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(Error::new(
            ErrorCode::UnsupportedPlatform,
            "The Node runner is currently available on macOS only.",
        ))
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use crate::{config::Layout, node_projects::NodeProjects};
    use std::{
        fs,
        io::BufReader,
        os::{
            fd::FromRawFd,
            unix::{fs::OpenOptionsExt, net::UnixStream, process::ExitStatusExt},
        },
        process::Stdio,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    static SIGNAL: AtomicBool = AtomicBool::new(false);
    extern "C" fn signal(_: i32) {
        SIGNAL.store(true, Ordering::SeqCst);
    }

    struct Group {
        id: i32,
        armed: bool,
    }
    impl Drop for Group {
        fn drop(&mut self) {
            if self.armed {
                unsafe {
                    libc::kill(-self.id, libc::SIGKILL);
                }
            }
        }
    }

    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_listpids(kind: u32, info: u32, buffer: *mut libc::c_void, size: i32) -> i32;
    }
    fn others(group: i32) -> bool {
        let mut pids = [0i32; 4096];
        let bytes = unsafe {
            proc_listpids(
                2,
                group as u32,
                pids.as_mut_ptr().cast(),
                std::mem::size_of_val(&pids) as i32,
            )
        };
        if bytes <= 0 {
            return true;
        } // Failure cannot establish cleanup.
        pids[..(bytes as usize / 4).min(pids.len())]
            .iter()
            .any(|pid| {
                if *pid <= 0 || *pid == group || unsafe { libc::getpgid(*pid) } != group {
                    return false;
                }
                let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
                let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
                let count = unsafe {
                    libc::proc_pidinfo(
                        *pid,
                        libc::PROC_PIDTBSDINFO,
                        0,
                        info.as_mut_ptr().cast(),
                        size,
                    )
                };
                count != size || unsafe { info.assume_init() }.pbi_status != 5 // Ignore zombies.
            })
    }

    type Socket = Arc<Mutex<UnixStream>>;
    fn emit(socket: &Socket, event: &Event) -> bool {
        socket
            .lock()
            .ok()
            .is_some_and(|mut stream| send(&mut *stream, event).is_ok())
    }

    pub(super) fn run() -> Result<()> {
        let id = unsafe { libc::getpid() };
        if id <= 1 || unsafe { libc::getpgrp() } != id {
            return Err(Error::new(
                ErrorCode::ExecutionFailed,
                "Runner requires its own process group.",
            ));
        }
        // This process remains the group leader until every signal is sent.
        // Caught handlers reset to default on exec; descendants do not inherit SIG_IGN.
        unsafe {
            libc::signal(libc::SIGTERM, signal as *const () as usize);
            libc::signal(libc::SIGHUP, signal as *const () as usize);
            libc::signal(libc::SIGINT, signal as *const () as usize);
        }
        let mut group = Group { id, armed: true };
        let socket = unsafe { UnixStream::from_raw_fd(0) };
        let mut reader = BufReader::new(socket.try_clone().map_err(io_error)?);
        socket
            .set_write_timeout(Some(Duration::from_millis(500)))
            .map_err(io_error)?;
        let socket = Arc::new(Mutex::new(socket));
        let Some(Control::Launch(launch)) = receive(&mut reader).map_err(io_error)? else {
            return Err(Error::new(
                ErrorCode::ExecutionFailed,
                "Missing runner launch request.",
            ));
        };
        let stopped = Arc::new(AtomicBool::new(false));
        let control = stopped.clone();
        thread::spawn(move || {
            // Any stop request, malformed frame, or liveness EOF ends this run.
            let _ = receive::<Control>(&mut reader);
            control.store(true, Ordering::SeqCst);
        });
        let result = (|| -> Result<()> {
            if !super::super::run_logs::valid_id(&launch.id)
                || !super::super::run_logs::valid_id(&launch.run_id)
            {
                return Err(Error::new(
                    ErrorCode::ExecutionFailed,
                    "Invalid runner identity.",
                ));
            }
            let directory = super::super::run_logs::directory(&launch.data, &launch.id)?;
            let lock = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(directory.join("run.lock"))
                .map_err(io_error)?;
            use std::os::fd::AsRawFd;
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(Error::new(
                    ErrorCode::ProjectBusy,
                    "An earlier runner for this project is still cleaning up. Retry shortly.",
                ));
            }
            let layout = Layout::at(launch.home, launch.data, launch.runtime);
            let projects = NodeProjects::new(Some(layout));
            let request = PrepareRuntimeRequest {
                id: launch.id.clone(),
                expected_revision: launch.expected_revision,
                manifest_revision: launch.manifest_revision,
            };
            let plan = projects.prepare_runtime(request)?;
            let script = projects.execution_script(
                &launch.id,
                &plan.summary().registry_revision,
                &plan.summary().manifest_revision,
            )?;
            if script.starts_with('-') {
                return Err(Error::new(
                    ErrorCode::InvalidProject,
                    "Script names starting with '-' cannot be executed.",
                ));
            }
            if stopped.load(Ordering::SeqCst) || SIGNAL.load(Ordering::SeqCst) {
                emit(&socket, &Event::Finished);
                return Ok(());
            }
            let mut command = plan.node_command();
            use std::os::unix::process::CommandExt;
            command
                .process_group(id)
                .arg(&plan.summary().npm.cli_path)
                .args(["--silent", "run-script", &script]);
            let mut child = command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(io_error)?;
            emit(&socket, &Event::Started);
            let mut readers = Vec::new();
            let drained = Arc::new(AtomicBool::new(false));

            for fd in [
                child.stdout.as_ref().unwrap().as_raw_fd(),
                child.stderr.as_ref().unwrap().as_raw_fd(),
            ] {
                if unsafe {
                    libc::fcntl(
                        fd,
                        libc::F_SETFL,
                        libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK,
                    )
                } < 0
                {
                    return Err(io_error(std::io::Error::last_os_error()));
                }
            }
            for (stream, mut pipe) in [
                (
                    LogStream::Stdout,
                    Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
                ),
                (
                    LogStream::Stderr,
                    Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
                ),
            ] {
                let socket = socket.clone();
                let stopped = stopped.clone();
                let drained = drained.clone();
                readers.push(thread::spawn(move || {
                    let mut bytes = [0; 4096];
                    let mut final_bytes = 0;
                    loop {
                        match pipe.read(&mut bytes) {
                            Ok(0) => break,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                if drained.load(Ordering::SeqCst) {
                                    break;
                                }
                                thread::sleep(Duration::from_millis(5));
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                                continue
                            }
                            Err(_) => break,
                            Ok(size) => {
                                if !emit(
                                    &socket,
                                    &Event::Output {
                                        stream,
                                        data: bytes[..size].to_vec(),
                                    },
                                ) {
                                    stopped.store(true, Ordering::SeqCst);
                                    break;
                                }
                                if drained.load(Ordering::SeqCst) {
                                    final_bytes += size;
                                    if final_bytes >= 256 * 1024 {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }));
            }
            let mut exit = None;
            loop {
                if let Some(status) = child.try_wait().map_err(io_error)? {
                    emit(
                        &socket,
                        &Event::MainExit {
                            code: status.code(),
                            signal: status.signal(),
                        },
                    );
                    exit = Some(status);
                    break;
                }
                if stopped.load(Ordering::SeqCst) || SIGNAL.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            // The anchor is still alive, so this PGID cannot have been recycled.
            unsafe {
                libc::kill(-id, libc::SIGTERM);
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            while others(id) {
                if exit.is_none() {
                    if let Some(status) = child.try_wait().map_err(io_error)? {
                        emit(
                            &socket,
                            &Event::MainExit {
                                code: status.code(),
                                signal: status.signal(),
                            },
                        );
                        exit = Some(status);
                    }
                }
                if Instant::now() >= deadline {
                    // SIGKILL deliberately includes this anchor. The parent sees
                    // the private channel close and reaps its direct child.
                    emit(&socket, &Event::ForcedCleanup);
                    unsafe {
                        libc::kill(-id, libc::SIGKILL);
                    }
                    return Err(Error::new(
                        ErrorCode::ExecutionFailed,
                        "Forced group cleanup failed.",
                    ));
                }
                thread::sleep(Duration::from_millis(20));
            }
            if exit.is_none() {
                let status = child.wait().map_err(io_error)?;
                emit(
                    &socket,
                    &Event::MainExit {
                        code: status.code(),
                        signal: status.signal(),
                    },
                );
            }
            // Group membership is empty; ordinary inherited pipes now reach EOF.
            // Detached descendants are outside the managed foreground contract.
            drained.store(true, Ordering::SeqCst);
            for reader in readers {
                let _ = reader.join();
            }
            emit(&socket, &Event::Finished);
            drop(lock);
            Ok(())
        })();
        if let Err(error) = &result {
            emit(
                &socket,
                &Event::Failure {
                    error: error.clone(),
                },
            );
        }
        if !others(id) {
            group.armed = false;
        }
        result
    }
    fn io_error(error: std::io::Error) -> Error {
        Error::new(ErrorCode::ExecutionFailed, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_protocol_rejects_unbounded_partial_and_unknown_frames() {
        for bytes in [
            vec![b'x'; FRAME_LIMIT as usize + 1],
            b"{\"kind\":\"stop\"}".to_vec(),
            b"{\"kind\":\"unknown\"}\n".to_vec(),
        ] {
            assert!(receive::<Control>(&mut bytes.as_slice()).is_err());
        }
        let mut bytes = Vec::new();
        send(&mut bytes, &Control::Stop).unwrap();
        assert!(matches!(
            receive::<Control>(&mut bytes.as_slice()).unwrap(),
            Some(Control::Stop)
        ));
        assert!(receive::<Event>(&mut b"".as_slice()).unwrap().is_none());
    }
}
