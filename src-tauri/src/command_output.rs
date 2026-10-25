//! Bounded subprocess capture shared by Node operations and shell environment discovery.
use crate::config::Result;
#[cfg(unix)]
use std::io::Read;
use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Drain both pipes without blocking the child or retaining unbounded output.
/// Quit/cancel and timeout terminate the process group, including download children.
pub(crate) fn output(
    mut cmd: Command,
    seconds: u64,
    cancelled: &dyn Fn() -> bool,
) -> Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(unix)]
    use std::os::fd::AsRawFd;
    #[cfg(unix)]
    fn nonblocking(pipe: &impl AsRawFd) -> Result<()> {
        let flags = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(unix)]
    fn drain(pipe: &mut impl Read, saved: &mut Vec<u8>) {
        let mut buf = [0; 8192];
        for _ in 0..64 {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let take = n.min((2 * 1024 * 1024usize).saturating_sub(saved.len()));
                    saved.extend_from_slice(&buf[..take]);
                }
            }
        }
    }
    #[cfg(windows)]
    use windows::drain;
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(windows)]
    let _job = match windows::Job::new(&child) {
        Ok(job) => Some(job),
        // Fast commands may finish between spawn and job assignment. Their
        // cached exit status and pipes can still be consumed normally.
        Err(_) if child.try_wait().is_ok_and(|status| status.is_some()) => None,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let mut stdout = child.stdout.take().ok_or("Missing command output")?;
    let mut stderr = child.stderr.take().ok_or("Missing command errors")?;
    #[cfg(unix)]
    if let Err(error) = nonblocking(&stdout).and_then(|_| nonblocking(&stderr)) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let start = Instant::now();
    let status = loop {
        drain(&mut stdout, &mut out);
        drain(&mut stderr, &mut err);
        let cancel = cancelled();
        if cancel || start.elapsed() > Duration::from_secs(seconds) {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(if cancel {
                "Command cancelled."
            } else {
                "Command timed out."
            }
            .into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                drain(&mut stdout, &mut out);
                drain(&mut stderr, &mut err);
                break status;
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
        }
    };
    let stdout = String::from_utf8_lossy(&out).trim().to_string();
    let stderr = String::from_utf8_lossy(&err).trim().to_string();
    if !status.success() {
        return Err(format!("{stdout}\n{stderr}")
            .trim()
            .chars()
            .take(4000)
            .collect());
    }
    Ok(stdout)
}

#[cfg(windows)]
mod windows {
    use crate::config::Result;
    use std::{
        io::Read,
        os::windows::io::{AsRawHandle, RawHandle},
        process::Child,
    };
    pub(super) fn drain(pipe: &mut (impl Read + AsRawHandle), saved: &mut Vec<u8>) {
        use windows_sys::Win32::System::Pipes::PeekNamedPipe;
        let mut buf = [0; 8192];
        for _ in 0..64 {
            let mut available = 0;
            let ok = unsafe {
                PeekNamedPipe(
                    pipe.as_raw_handle() as _,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || available == 0 {
                break;
            }
            let count = buf.len().min(available as usize);
            match pipe.read(&mut buf[..count]) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let count = n.min((2 * 1024 * 1024usize).saturating_sub(saved.len()));
                    saved.extend_from_slice(&buf[..count]);
                }
            }
        }
    }

    pub(super) struct Job(RawHandle);
    impl Job {
        pub(super) fn new(child: &Child) -> Result<Self> {
            use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
            unsafe {
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
                    || AssignProcessToJobObject(handle, child.as_raw_handle() as _) == 0
                {
                    let error = std::io::Error::last_os_error().to_string();
                    CloseHandle(handle);
                    return Err(error);
                }
                Ok(Self(handle as _))
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.0 as _);
            }
        }
    }
}
