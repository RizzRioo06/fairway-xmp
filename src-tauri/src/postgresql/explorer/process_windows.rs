//! A bounded Windows query helper. The owned Job Object never contains a server.
use super::{
    super::explorer_contract::*,
    wire::{Envelope, PrivateBytes},
};
use std::{
    io::{Read, Write},
    os::windows::io::AsRawHandle,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::*,
};
struct Job(HANDLE);
impl Job {
    fn attach(child: &std::process::Child) -> Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(fail(Code::Unavailable, "Private worker job unavailable"));
        }
        let job = Self(handle);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        limits.ProcessMemoryLimit = 512 * 1024 * 1024;
        if unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        } == 0
            || unsafe { AssignProcessToJobObject(handle, child.as_raw_handle() as _) } == 0
        {
            return Err(fail(
                Code::Unavailable,
                "Private worker containment unavailable",
            ));
        }
        Ok(job)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
pub fn run(e: Envelope, cancel: Arc<AtomicBool>) -> Result<Output> {
    let writing = matches!(
        e.operation,
        Operation::Query {
            mode: QueryMode::ReadWrite,
            ..
        }
    );
    let mut bytes = PrivateBytes(
        serde_json::to_vec(&e)
            .map_err(|_| fail(Code::InvalidRequest, "Private request encoding failed"))?,
    );
    if bytes.0.len() >= 1024 * 1024 {
        return Err(fail(Code::InvalidRequest, "Private request limit exceeded"));
    }
    bytes.0.push(b'\n');
    let executable =
        std::env::current_exe().map_err(|_| fail(Code::Unavailable, "Query worker unavailable"))?;
    #[cfg(test)]
    let executable = std::env::var_os("FAIRWAY_TEST_POSTGRES_WORKER")
        .map(std::path::PathBuf::from)
        .unwrap_or(executable);
    let system = std::env::var_os("SystemRoot")
        .ok_or_else(|| fail(Code::Unavailable, "Windows system directory unavailable"))?;
    let mut command = crate::process::command(executable);
    command
        .arg("--fairway-postgres-query-worker")
        .env_clear()
        .env("SystemRoot", &system)
        .env("PATH", std::path::Path::new(&system).join("System32"))
        .current_dir(&e.runtime)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // The trusted worker waits on stdin. No credentials are sent until its job
    // assignment succeeds; failure kills/reaps only this directly owned child.
    let mut child = command
        .spawn()
        .map_err(|_| fail(Code::Unavailable, "Query worker unavailable"))?;
    let job = match Job::attach(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| fail(Code::Unavailable, "Private worker input unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| fail(Code::Unavailable, "Private worker output unavailable"))?;
    drop(command);
    drop(e);
    let done = Arc::new(AtomicBool::new(false));
    let finish = done.clone();
    let flag = cancel.clone();
    let writer = std::thread::spawn(move || {
        let mut stdin = stdin;
        let result = stdin.write_all(&bytes.0);
        drop(bytes);
        if result.is_err() {
            return;
        }
        while !finish.load(Ordering::SeqCst) {
            if flag.load(Ordering::SeqCst) {
                let _ = stdin.write_all(b"!");
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Closing stdin also delivers owner loss/cancellation to the worker.
    });
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stdout.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes);
        (read, bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => break None,
        }
    };
    done.store(true, Ordering::SeqCst);
    drop(job);
    if status.is_none() {
        let _ = child.wait();
    }
    let _ = writer.join();
    let failure = || {
        let mut e = fail(
            if cancel.load(Ordering::SeqCst) {
                Code::Cancelled
            } else {
                Code::TimedOut
            },
            "Query worker ended without a verified outcome",
        );
        if writing {
            e.outcome = Outcome::Unknown;
        }
        e
    };
    let (read, bytes) = reader.join().map_err(|_| failure())?;
    if status.is_none_or(|s| !s.success()) || read.is_err() || bytes.len() > 2 * 1024 * 1024 {
        return Err(failure());
    }
    serde_json::from_slice::<Result<Output>>(&bytes).map_err(|_| failure())?
}
