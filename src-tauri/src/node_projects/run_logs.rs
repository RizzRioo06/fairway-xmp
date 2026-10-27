//! App-owned bounded logs. Imported project directories are never written here.
use super::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

const MEMORY_BYTES: usize = 256 * 1024;
const SEGMENT_BYTES: u64 = 5 * 1024 * 1024;
const RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LogStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogRecord {
    pub run_id: String,
    pub cursor: u64,
    pub time: u64,
    pub stream: LogStream,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogBatch {
    pub records: Vec<LogRecord>,
    pub next_cursor: u64,
    pub truncated: bool,
    pub eof: bool,
    pub error: Option<String>,
}

pub(super) fn valid_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn error(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::LogUnavailable, message)
}

pub(super) fn directory(data: &Path, project: &str) -> Result<PathBuf> {
    if !valid_id(project) {
        return Err(error("Invalid project log identity."));
    }
    let root = data.join("node-project-logs");
    let directory = root.join(project);
    for path in [&root, &directory] {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(path)
            .map_err(|e| error(format!("Could not create app-owned logs: {e}")))?;
        let metadata = fs::symlink_metadata(path).map_err(|e| error(e.to_string()))?;
        if !metadata.is_dir() {
            return Err(error(
                "The app-owned log directory must not be a link or special file.",
            ));
        }
    }
    Ok(directory)
}

fn open_log(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("Log is not a regular file"));
    }
    Ok(file)
}

pub(super) struct RunLog {
    directory: PathBuf,
    run: String,
    records: VecDeque<LogRecord>,
    bytes: usize,
    cursor: u64,
    file: Option<File>,
    file_bytes: u64,
    segment_bytes: u64,
    pub error: Option<String>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl RunLog {
    pub fn new(data: &Path, project: &str, run: &str) -> Result<Self> {
        if !valid_id(run) {
            return Err(error("Invalid run log identity."));
        }
        let directory = directory(data, project)?;
        let file = open_log(&directory.join("events.jsonl")).map_err(|e| error(e.to_string()))?;
        let file_bytes = file.metadata().map_err(|e| error(e.to_string()))?.len();
        Ok(Self {
            directory,
            run: run.into(),
            records: VecDeque::new(),
            bytes: 0,
            cursor: 0,
            file: Some(file),
            file_bytes,
            segment_bytes: SEGMENT_BYTES,
            error: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
    #[cfg(any(target_os = "macos", test))]
    pub fn output(&mut self, stream: LogStream, data: &[u8]) {
        let pending = if stream == LogStream::Stdout {
            &mut self.stdout
        } else {
            &mut self.stderr
        };
        pending.extend_from_slice(data);
        let mut text = String::new();
        loop {
            match std::str::from_utf8(pending) {
                Ok(value) => {
                    text.push_str(value);
                    pending.clear();
                    break;
                }
                Err(error) => {
                    let good = error.valid_up_to();
                    text.push_str(std::str::from_utf8(&pending[..good]).unwrap_or(""));
                    pending.drain(..good);
                    if let Some(length) = error.error_len() {
                        text.push('\u{fffd}');
                        pending.drain(..length);
                    } else {
                        break;
                    }
                }
            }
        }
        if !text.is_empty() {
            self.append(stream, text);
        }
    }
    pub fn finish(&mut self) {
        for stream in [LogStream::Stdout, LogStream::Stderr] {
            let pending = if stream == LogStream::Stdout {
                &mut self.stdout
            } else {
                &mut self.stderr
            };
            if !pending.is_empty() {
                pending.clear();
                self.append(stream, "\u{fffd}".into());
            }
        }
        if let Some(file) = &self.file {
            let _ = file.sync_all();
        }
    }
    pub fn append(&mut self, stream: LogStream, text: String) {
        // Every source passes bounded chunks. Keep this true for system messages too.
        let text: String = text.chars().take(16384).collect();
        self.cursor += 1;
        let record = LogRecord {
            run_id: self.run.clone(),
            cursor: self.cursor,
            time: super::execution::now(),
            stream,
            text,
        };
        self.bytes += record.text.len();
        self.records.push_back(record.clone());
        while self.bytes > MEMORY_BYTES || self.records.len() > 1024 {
            if let Some(old) = self.records.pop_front() {
                self.bytes -= old.text.len();
            } else {
                break;
            }
        }
        if self.file.is_some() {
            let result = (|| -> std::io::Result<()> {
                let mut line = serde_json::to_vec(&record)?;
                line.push(b'\n');
                if self.file_bytes + line.len() as u64 > self.segment_bytes {
                    self.file.take();
                    let previous = self.directory.join("events.1.jsonl");
                    if let Ok(metadata) = fs::symlink_metadata(&previous) {
                        if !metadata.is_file() {
                            return Err(std::io::Error::other("Invalid rotated log target"));
                        }
                    }
                    // Windows replacement is irrelevant to execution, which is macOS-only.
                    #[cfg(windows)]
                    if previous.exists() {
                        fs::remove_file(&previous)?;
                    }
                    fs::rename(self.directory.join("events.jsonl"), previous)?;
                    self.file = Some(open_log(&self.directory.join("events.jsonl"))?);
                    self.file_bytes = 0;
                }
                self.file
                    .as_mut()
                    .ok_or_else(|| std::io::Error::other("Log file unavailable"))?
                    .write_all(&line)?;
                self.file_bytes += line.len() as u64;
                Ok(())
            })();
            if let Err(error) = result {
                self.error = Some(format!("Disk logging stopped: {error}"));
                self.file.take();
            }
        }
    }
    pub fn read(&self, after: u64, finished: bool) -> LogBatch {
        batch(
            self.records
                .iter()
                .filter(|record| record.cursor > after)
                .cloned(),
            after,
            self.cursor,
            finished,
            self.error.clone(),
        )
    }
}

fn batch(
    records: impl Iterator<Item = LogRecord>,
    after: u64,
    latest: u64,
    finished: bool,
    error: Option<String>,
) -> LogBatch {
    let mut saved = Vec::new();
    let mut bytes = 0;
    let mut next = after;
    let mut gap = false;
    for record in records {
        if saved.is_empty() {
            gap = record.cursor > after.saturating_add(1);
        }
        if saved.len() >= 256 || bytes + record.text.len() > RESPONSE_BYTES {
            break;
        }
        bytes += record.text.len();
        next = record.cursor;
        saved.push(record);
    }
    LogBatch {
        records: saved,
        next_cursor: next,
        truncated: gap,
        eof: finished && next >= latest,
        error,
    }
}

pub(super) fn read_disk(data: &Path, project: &str, run: &str, after: u64) -> Result<LogBatch> {
    if !valid_id(project) || !valid_id(run) {
        return Err(error("Invalid log identity."));
    }
    let directory = data.join("node-project-logs").join(project);
    for path in [data.join("node-project-logs"), directory.clone()] {
        if !fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(error("Retained log directories are unavailable or linked."));
        }
    }
    let mut records = VecDeque::new();
    let mut bytes = 0;
    let mut latest = 0;
    for name in ["events.1.jsonl", "events.jsonl"] {
        let path = directory.join(name);
        let Some(content) = super::manifest::read_regular(&path, SEGMENT_BYTES + 65536)
            .map_err(|e| error(e.to_string()))?
        else {
            continue;
        };
        for line in BufReader::new(content.as_slice()).split(b'\n') {
            if let Ok(record) =
                serde_json::from_slice::<LogRecord>(&line.map_err(|e| error(e.to_string()))?)
            {
                if record.run_id == run {
                    latest = latest.max(record.cursor);
                    if record.cursor > after {
                        bytes += record.text.len();
                        records.push_back(record);
                    }
                }
                while bytes > MEMORY_BYTES || records.len() > 1024 {
                    if let Some(old) = records.pop_front() {
                        bytes -= old.text.len();
                    }
                }
            }
        }
    }
    if latest == 0 {
        return Err(error("This run's retained logs are unavailable."));
    }
    Ok(batch(records.into_iter(), after, latest, true, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logs_bound_memory_rotate_disk_preserve_streams_and_unicode() {
        let data = std::env::temp_dir().join(format!("fw-log-{:032x}", rand::random::<u128>()));
        let project = "a".repeat(32);
        let run = "b".repeat(32);
        let mut log = RunLog::new(&data, &project, &run).unwrap();
        log.segment_bytes = 4096;
        log.output(LogStream::Stdout, &[0xe2, 0x82]);
        log.output(LogStream::Stdout, &[0xac, b'\n']);
        log.output(LogStream::Stderr, b"one\ntwo\n");
        let initial = log.read(0, false);
        assert_eq!(initial.records[0].text, "€\n");
        assert_eq!(initial.records[1].stream, LogStream::Stderr);
        for _ in 0..400 {
            log.output(LogStream::Stdout, &[b'x'; 4096]);
        }
        let batch = log.read(0, false);
        assert!(batch.truncated);
        assert!(!batch.eof);
        assert!(log.bytes <= MEMORY_BYTES);
        assert!(
            fs::metadata(log.directory.join("events.jsonl"))
                .unwrap()
                .len()
                <= 8192
        );
        assert!(
            fs::metadata(log.directory.join("events.1.jsonl"))
                .unwrap()
                .len()
                <= 8192
        );
        log.finish();
        drop(log);
        let batch = read_disk(&data, &project, &run, 0).unwrap();
        assert!(batch.truncated);
        fs::remove_dir_all(data).unwrap();
    }
    #[test]
    fn malformed_utf8_and_path_traversal_are_bounded() {
        let data = std::env::temp_dir().join(format!("fw-log-{:032x}", rand::random::<u128>()));
        assert!(RunLog::new(&data, "../escape", &"a".repeat(32)).is_err());
        let mut log = RunLog::new(&data, &"a".repeat(32), &"b".repeat(32)).unwrap();
        log.output(LogStream::Stderr, &[0xff, 0xe2]);
        log.finish();
        assert_eq!(
            log.read(0, true)
                .records
                .iter()
                .map(|r| r.text.clone())
                .collect::<String>(),
            "��"
        );
        drop(log);
        fs::remove_dir_all(data).unwrap();
    }
}
