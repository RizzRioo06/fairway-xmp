//! One local PTY, independent of the service manager. Output is drained in bounded
//! batches: a hidden/throttled WebView applies backpressure rather than growing
//! an unbounded IPC event queue. No terminal contents are persisted to disk.
use crate::config::{Layout, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
#[cfg(unix)]
use std::time::Duration;
use std::{
    collections::VecDeque,
    io::{Read, Write},
    sync::{Arc, Condvar, Mutex},
};

const BUFFER_LIMIT: usize = 256 * 1024;
const READ_LIMIT: usize = 32 * 1024;
const INPUT_LIMIT: usize = 4096;

#[derive(Default)]
pub struct Terminals(Mutex<Option<Arc<Session>>>);

struct Session {
    id: String,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Option<Box<dyn Child + Send + Sync>>>,
    output: Arc<Output>,
    #[cfg(windows)]
    job: Job,
}

#[derive(Default)]
struct Output {
    state: Mutex<OutputState>,
    drained: Condvar,
}

#[derive(Default)]
struct OutputState {
    bytes: VecDeque<u8>,
    eof: bool,
    stopped: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInfo {
    shell: &'static str,
    directory: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutput {
    data: Vec<u8>,
    exited: bool,
    exit_code: Option<u32>,
    error: Option<String>,
}

fn size(cols: u16, rows: u16) -> Result<PtySize> {
    if !(2..=500).contains(&cols) || !(1..=300).contains(&rows) {
        return Err("Terminal size is out of range.".into());
    }
    Ok(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })
}

/// Separate launchers work even when persistent PATH integration is disabled.
/// Config regeneration updates these too, so a PHP switch applies to the next command.
pub fn prepare_tools(layout: &Layout) -> Result<std::path::PathBuf> {
    let mut tools = layout.clone();
    tools.bin = layout.data.join("terminal-bin");
    std::fs::create_dir_all(&tools.bin).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    crate::integration::write_unix_launchers(&tools)?;
    #[cfg(windows)]
    crate::integration::write_windows_launchers(&tools)?;
    Ok(tools.bin)
}

fn shell(layout: &Layout) -> Result<CommandBuilder> {
    if !layout.php_exe("php")?.is_file() || !layout.runtime.join("composer.phar").is_file() {
        return Err(
            "Runtime files are missing. Reinstall Fairway XMP to restore PHP and Composer.".into(),
        );
    }
    let bin = prepare_tools(layout)?;
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = CommandBuilder::new("/bin/zsh");
        c.args(["-f", "-i"]);
        c
    };
    #[cfg(windows)]
    let mut command = {
        let executable = std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "C:\\Windows".into())
            .join("System32/cmd.exe");
        let mut c = CommandBuilder::new(executable);
        c.args(["/D", "/Q"]);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut c = CommandBuilder::new("/bin/sh");
        c.arg("-i");
        c
    };
    let mut paths = vec![bin];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    #[cfg(unix)]
    paths.extend(["/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"].map(Into::into));
    command.env(
        "PATH",
        std::env::join_paths(paths).map_err(|e| e.to_string())?,
    );
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("TERM_PROGRAM", "Fairway XMP");
    // A clean interactive shell preserves the app's tool ordering. User startup
    // scripts must not replace PATH or run unrelated commands on terminal open.
    #[cfg(unix)]
    command.env_remove("ENV");
    command.cwd(&layout.www);
    Ok(command)
}

impl Terminals {
    pub fn start(&self, layout: &Layout, id: String, cols: u16, rows: u16) -> Result<TerminalInfo> {
        if id.is_empty()
            || id.len() > 80
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("Invalid terminal session.".into());
        }
        let dimensions = size(cols, rows)?;
        let mut slot = self.0.lock().map_err(|_| "Terminal is unavailable.")?;
        // A reload replaces the old session. Stale close/read/write requests
        // carry their old id and cannot affect the replacement.
        if let Some(previous) = slot.take() {
            previous.stop();
        }
        let command = shell(layout)?;
        let pair = native_pty_system()
            .openpty(dimensions)
            .map_err(|e| e.to_string())?;
        let output = Arc::new(Output::default());
        #[cfg(unix)]
        let reader: Box<dyn Read + Send> = {
            use std::os::fd::BorrowedFd;
            let fd = pair.master.as_raw_fd().ok_or("Cannot read the terminal.")?;
            let owned = unsafe { BorrowedFd::borrow_raw(fd) }
                .try_clone_to_owned()
                .map_err(|e| e.to_string())?;
            Box::new(StoppableReader {
                file: owned.into(),
                output: output.clone(),
            })
        };
        #[cfg(windows)]
        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|e| format!("Could not start terminal: {e}"))?;
        drop(pair.slave);
        #[cfg(windows)]
        let job = match Job::new(child.as_raw_handle()) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        #[cfg(not(windows))]
        let _ = &mut child;
        let session = Arc::new(Session {
            id,
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            child: Mutex::new(Some(child)),
            output: output.clone(),
            #[cfg(windows)]
            job,
        });
        if let Err(error) = std::thread::Builder::new()
            .name("fairway-terminal-output".into())
            .spawn(move || read_output(reader, output))
        {
            session.stop();
            return Err(error.to_string());
        }
        *slot = Some(session);
        Ok(TerminalInfo {
            shell: if cfg!(windows) {
                "Command Prompt"
            } else if cfg!(target_os = "macos") {
                "zsh"
            } else {
                "sh"
            },
            directory: layout.www.to_string_lossy().into(),
        })
    }

    fn session(&self, id: &str) -> Result<Arc<Session>> {
        self.0
            .lock()
            .map_err(|_| "Terminal is unavailable.")?
            .as_ref()
            .filter(|s| s.id == id)
            .cloned()
            .ok_or_else(|| "Terminal session has ended. Start a new session.".into())
    }

    pub fn read(&self, id: &str) -> Result<TerminalOutput> {
        let session = self.session(id)?;
        let mut output = session
            .output
            .state
            .lock()
            .map_err(|_| "Terminal output is unavailable.")?;
        let count = output.bytes.len().min(READ_LIMIT);
        let data = output.bytes.drain(..count).collect();
        session.output.drained.notify_all();
        let status = session
            .child
            .lock()
            .map_err(|_| "Terminal process is unavailable.")?
            .as_mut()
            .ok_or("Terminal process has ended.")?
            .try_wait()
            .map_err(|e| e.to_string())?;
        Ok(TerminalOutput {
            data,
            exited: output.eof && output.bytes.is_empty() && status.is_some(),
            exit_code: status.map(|s| s.exit_code()),
            error: output.error.clone(),
        })
    }

    pub fn write(&self, id: &str, data: &str) -> Result<()> {
        if data.len() > INPUT_LIMIT {
            return Err("Terminal input is too large.".into());
        }
        let session = self.session(id)?;
        let mut writer = session
            .writer
            .lock()
            .map_err(|_| "Terminal input is unavailable.")?;
        writer
            .write_all(data.as_bytes())
            .and_then(|_| writer.flush())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let dimensions = size(cols, rows)?;
        self.session(id)?
            .master
            .lock()
            .map_err(|_| "Terminal is unavailable.")?
            .resize(dimensions)
            .map_err(|e| e.to_string())
    }

    pub fn close(&self, id: &str) {
        if let Ok(mut slot) = self.0.lock() {
            if slot.as_ref().is_some_and(|s| s.id == id) {
                if let Some(session) = slot.take() {
                    session.stop();
                }
            }
        }
    }

    pub fn close_all(&self) {
        if let Ok(mut slot) = self.0.lock() {
            if let Some(session) = slot.take() {
                session.stop();
            }
        }
    }
}

impl Drop for Terminals {
    fn drop(&mut self) {
        self.close_all();
    }
}

impl Session {
    fn stop(&self) {
        if let Ok(mut output) = self.output.state.lock() {
            output.stopped = true;
        }
        self.output.drained.notify_all();
        // Signal only groups attached to this PTY. This also interrupts the
        // foreground command when the shell has put it in its own job group.
        #[cfg(unix)]
        {
            let shell_pid = self
                .child
                .lock()
                .ok()
                .and_then(|child| child.as_ref().and_then(|child| child.process_id()));
            if let Ok(master) = self.master.lock() {
                if let Some(group) = master.process_group_leader() {
                    unsafe {
                        libc::kill(-group, libc::SIGHUP);
                    }
                    if Some(group as u32) != shell_pid {
                        // Keep the shell/session leader alive until its attached
                        // foreground job is gone. Once the leader exits, the OS
                        // can detach the PTY before we can identify that group.
                        std::thread::sleep(Duration::from_millis(50));
                        if let Some(group) = master.process_group_leader() {
                            if Some(group as u32) != shell_pid {
                                unsafe {
                                    libc::kill(-group, libc::SIGKILL);
                                }
                            }
                        }
                    }
                }
            }
        }
        #[cfg(windows)]
        self.job.terminate();
        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.as_mut() {
                if child.try_wait().ok().flatten().is_none() {
                    let _ = child.kill();
                    #[cfg(unix)]
                    {
                        for _ in 0..5 {
                            std::thread::sleep(Duration::from_millis(20));
                            if child.try_wait().ok().flatten().is_some() {
                                break;
                            }
                        }
                        // The unreaped child still owns this PID.
                        if child.try_wait().ok().flatten().is_none() {
                            if let Some(pid) = child.process_id() {
                                unsafe {
                                    libc::kill(pid as i32, libc::SIGKILL);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Reap on a separate thread; neither app shutdown nor a restart waits
        // indefinitely for an uncooperative shell. Dropping the master after
        // this method closes the controlling terminal and hangs up its jobs.
    }
}

// Poll an owned descriptor so ending the session also stops its reader even
// when a deliberately detached descendant retains a slave descriptor.
#[cfg(unix)]
struct StoppableReader {
    file: std::fs::File,
    output: Arc<Output>,
}
#[cfg(unix)]
impl Read for StoppableReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        use std::os::fd::AsRawFd;
        loop {
            if self.output.state.lock().unwrap().stopped {
                return Ok(0);
            }
            let mut descriptor = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut descriptor, 1, 100) };
            if ready > 0 {
                return self.file.read(buffer);
            }
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Normal shells are already dead after kill; wait in the background to
        // avoid zombies without retaining the PTY handles.
        if let Ok(child) = self.child.get_mut() {
            if let Some(mut child) = child.take() {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
        }
    }
}

fn read_output(mut reader: Box<dyn Read + Send>, output: Arc<Output>) {
    let mut bytes = [0u8; 8192];
    loop {
        match reader.read(&mut bytes) {
            Ok(0) => break,
            Ok(count) => {
                let state = output.state.lock().unwrap();
                let mut state = output
                    .drained
                    .wait_while(state, |s| {
                        !s.stopped && s.bytes.len() + count > BUFFER_LIMIT
                    })
                    .unwrap();
                if state.stopped {
                    break;
                }
                state.bytes.extend(&bytes[..count]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            #[cfg(unix)]
            Err(e) if e.raw_os_error() == Some(libc::EIO) => break, // PTY slave closed.
            Err(e) => {
                output.state.lock().unwrap().error = Some(e.to_string());
                break;
            }
        }
    }
    output.state.lock().unwrap().eof = true;
}

#[cfg(windows)]
struct Job(isize);
#[cfg(windows)]
impl Job {
    fn new(process: Option<std::os::windows::io::RawHandle>) -> Result<Self> {
        use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
        unsafe {
            let process = process.ok_or("Cannot track the terminal process.")?;
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, process as _) == 0
            {
                let error = std::io::Error::last_os_error().to_string();
                CloseHandle(handle);
                return Err(error);
            }
            Ok(Self(handle as isize))
        }
    }
    fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0 as _, 1);
        }
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0 as _);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::Instant};

    struct Fixture {
        root: std::path::PathBuf,
        layout: Layout,
        terminals: Terminals,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "fairway-terminal-{}-{} with ' spaces 日本",
                std::process::id(),
                rand::random::<u64>()
            ));
            let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
            fs::create_dir_all(&layout.runtime).unwrap();
            fs::write(
                layout.runtime.join("manifest.json"),
                br#"{"php":"8.5.11","phpVersions":{"8.4":"8.4.26","8.5":"8.5.11"}}"#,
            )
            .unwrap();
            layout.initialize().unwrap();
            layout
                .write_configs(&crate::config::Ports::default())
                .unwrap();
            let php = layout.php_exe("php").unwrap();
            fs::create_dir_all(php.parent().unwrap()).unwrap();
            fs::write(php, b"").unwrap();
            fs::write(layout.runtime.join("composer.phar"), b"").unwrap();
            Self {
                root,
                layout,
                terminals: Terminals::default(),
            }
        }
        fn start(&self, id: &str) {
            self.terminals
                .start(&self.layout, id.into(), 80, 24)
                .unwrap();
        }
        fn until(&self, id: &str, expected: &str) -> String {
            let deadline = Instant::now() + std::time::Duration::from_secs(8);
            let mut bytes = Vec::new();
            while Instant::now() < deadline {
                bytes.extend(self.terminals.read(id).unwrap().data);
                let text = String::from_utf8_lossy(&bytes);
                if text.contains(expected) {
                    return text.into_owned();
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!(
                "Missing {expected:?} in {}",
                String::from_utf8_lossy(&bytes)
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.terminals.close_all();
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn validates_session_and_dimensions_before_replacing_a_shell() {
        let fixture = Fixture::new();
        assert!(fixture
            .terminals
            .start(&fixture.layout, "bad/id".into(), 80, 24)
            .is_err());
        assert!(fixture
            .terminals
            .start(&fixture.layout, "valid".into(), 0, 24)
            .is_err());
        fixture.start("first");
        assert!(fixture
            .terminals
            .write("first", &"x".repeat(INPUT_LIMIT + 1))
            .is_err());
        assert!(fixture.terminals.resize("first", 80, 0).is_err());
        fixture.start("second");
        fixture.terminals.close("first");
        assert!(fixture.terminals.read("first").is_err());
        assert!(fixture.terminals.write("first", "exit\r").is_err());
        assert!(fixture.terminals.read("second").is_ok());
        fixture.terminals.close_all();
        assert!(fixture.terminals.read("second").is_err());
        fs::remove_file(fixture.layout.runtime.join("composer.phar")).unwrap();
        assert!(fixture
            .terminals
            .start(&fixture.layout, "missing".into(), 80, 24)
            .unwrap_err()
            .contains("Runtime files are missing"));
    }

    #[cfg(unix)]
    #[test]
    fn native_shell_has_a_tty_coursework_directory_resize_unicode_interrupt_and_exit() {
        let fixture = Fixture::new();
        fixture.start("native");
        fixture.terminals.write("native", "pwd\r").unwrap();
        fixture.until("native", &fixture.layout.www.to_string_lossy());
        fixture.terminals.resize("native", 100, 32).unwrap();
        fixture.terminals.write("native", "stty size\r").unwrap();
        fixture.until("native", "32 100");
        fixture
            .terminals
            .write(
                "native",
                "printf '\\116\\101\\124\\111\\126\\105\\137\\117\\113 日本\\n'\r",
            )
            .unwrap();
        fixture.until("native", "NATIVE_OK 日本");
        fixture.terminals.write("native", "sleep 30\r").unwrap();
        std::thread::sleep(Duration::from_millis(100));
        fixture.terminals.write("native", "\x03").unwrap();
        fixture
            .terminals
            .write(
                "native",
                "printf '\\111\\116\\124\\105\\122\\122\\125\\120\\124\\105\\104\\n'\r",
            )
            .unwrap();
        fixture.until("native", "INTERRUPTED");
        fixture.terminals.write("native", "exit 7\r").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let output = fixture.terminals.read("native").unwrap();
            if output.exited {
                assert_eq!(output.exit_code, Some(7));
                break;
            }
            assert!(Instant::now() < deadline, "Shell did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    #[test]
    fn session_tools_follow_selected_php_without_installing_path_integration() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new();
        for version in ["8.4", "8.5"] {
            let bin = fixture
                .layout
                .runtime
                .join(if version == "8.5" {
                    "php".into()
                } else {
                    format!("php-{version}")
                })
                .join("bin");
            fs::create_dir_all(&bin).unwrap();
            fs::write(
                bin.join("php"),
                format!("#!/bin/sh\nprintf 'FAIRWAY_PHP_{version}\\n'\n"),
            )
            .unwrap();
            fs::set_permissions(bin.join("php"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        fixture.start("tools");
        assert!(!crate::integration::installed(&fixture.layout));
        fixture.terminals.write("tools", "php --version\r").unwrap();
        fixture.until("tools", "FAIRWAY_PHP_8.5");
        fs::write(fixture.layout.data.join("php-version"), "8.4").unwrap();
        fixture
            .layout
            .write_configs(&crate::config::Ports::default())
            .unwrap();
        fixture.terminals.write("tools", "php --version\r").unwrap();
        fixture.until("tools", "FAIRWAY_PHP_8.4");
        assert!(!fixture.layout.data.join("cli-installed").exists());
        assert!(!fixture.layout.home.join(".zshrc").exists());
    }

    #[cfg(unix)]
    #[test]
    fn closing_interrupts_hangup_resistant_commands_and_releases_the_reader() {
        let fixture = Fixture::new();
        fixture.start("cleanup");
        fixture
            .terminals
            .write(
                "cleanup",
                "trap '' HUP; printf '\\122\\105\\101\\104\\131\\n'; sleep 30\r",
            )
            .unwrap();
        fixture.until("cleanup", "READY");
        let session = fixture.terminals.session("cleanup").unwrap();
        let started = Instant::now();
        fixture.terminals.close("cleanup");
        assert!(started.elapsed() < Duration::from_secs(2));
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let exited = session
                .child
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .try_wait()
                .unwrap()
                .is_some();
            if exited && session.output.state.lock().unwrap().eof {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Terminal processes/reader outlived close"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_windows_shell_accepts_input_resizes_and_exits() {
        let fixture = Fixture::new();
        fixture.start("windows");
        fixture.terminals.resize("windows", 100, 32).unwrap();
        fixture
            .terminals
            .write("windows", "echo %TERM_PROGRAM%\r")
            .unwrap();
        fixture.until("windows", "Fairway XMP");
        fixture.terminals.write("windows", "exit 7\r").unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let output = fixture.terminals.read("windows").unwrap();
            if output.exited {
                assert_eq!(output.exit_code, Some(7));
                break;
            }
            assert!(Instant::now() < deadline, "Windows shell did not exit");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn output_backpressure_is_bounded_and_stopping_wakes_the_reader() {
        let output = Arc::new(Output::default());
        let reader_output = output.clone();
        let reader = std::thread::spawn(move || {
            read_output(
                Box::new(std::io::Cursor::new(vec![b'x'; BUFFER_LIMIT * 2])),
                reader_output,
            )
        });
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let mut state = output.state.lock().unwrap();
            assert!(state.bytes.len() <= BUFFER_LIMIT);
            if state.bytes.len() == BUFFER_LIMIT {
                assert!(!state.eof);
                state.stopped = true;
                output.drained.notify_all();
                break;
            }
            assert!(Instant::now() < deadline, "Output never filled its buffer");
            drop(state);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        reader.join().unwrap();
        assert!(output.state.lock().unwrap().eof);
    }
}
