//! Read-only, bounded snapshots of Fairway-owned logs. No caller-supplied paths.
use crate::config::Result;
use serde::Serialize;
use std::hash::{Hash, Hasher};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::Path,
};

#[cfg(any(feature = "desktop", test))]
use std::path::PathBuf;

const TAIL_BYTES: u64 = 64 * 1024;
const ENTRIES_PER_FILE: usize = 120;
const SOURCES: &[(&str, &str)] = &[
    ("php.log", "php"),
    ("php-fpm.log", "php"),
    ("php-process.log", "php"),
    ("nginx.log", "nginx"),
    ("nginx-process.log", "nginx"),
    ("access.log", "requests"),
    ("mysql.log", "mysql"),
    ("mysql-process.log", "mysql"),
    ("mail-process.log", "mail"),
];

#[cfg(any(feature = "desktop", test))]
pub struct LogDirectory {
    path: PathBuf,
    reading: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(any(feature = "desktop", test))]
pub(crate) struct ReadPermit(std::sync::Arc<std::sync::atomic::AtomicBool>);
#[cfg(any(feature = "desktop", test))]
impl Drop for ReadPermit {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}
#[cfg(any(feature = "desktop", test))]
impl LogDirectory {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            reading: Default::default(),
        }
    }
    pub(crate) fn begin(&self) -> Result<(PathBuf, ReadPermit)> {
        self.reading
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            )
            .map_err(|_| "Logs are already being refreshed. Try again shortly.".to_string())?;
        Ok((self.path.clone(), ReadPermit(self.reading.clone())))
    }
}

#[derive(Serialize, Debug, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    id: String,
    source: String,
    file: String,
    severity: String,
    time: Option<String>,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(skip)]
    truncated: bool,
}

#[derive(Serialize, Default)]
pub struct Snapshot {
    revision: String,
    entries: Vec<Entry>,
    unavailable: Vec<String>,
    limited: bool,
}

fn severity(text: &str) -> &'static str {
    let lower = text.to_lowercase();
    let body = lower
        .split_once("] ")
        .map_or(lower.as_str(), |(_, value)| value);
    // Match native severity markers, not arbitrary request paths or SQL text.
    if lower.contains("level=error")
        || lower.contains("level=fatal")
        || ["error:", "alert:", "emergency:", "fatal:"]
            .iter()
            .any(|marker| body.starts_with(marker))
        || [
            "[error]",
            "[crit]",
            "[alert]",
            "[emerg]",
            "[fatal]",
            "php fatal error:",
            "php parse error:",
            "php recoverable fatal error:",
        ]
        .iter()
        .any(|s| lower.contains(s))
    {
        "error"
    } else if lower.contains("level=warn")
        || body.starts_with("warning:")
        || [
            "[warn]",
            "[warning]",
            "php warning:",
            "php deprecated:",
            "php notice:",
        ]
        .iter()
        .any(|s| lower.contains(s))
    {
        "warning"
    } else if body.starts_with("notice:")
        || [
            "[note]",
            "[notice]",
            "[info]",
            "[system]",
            "level=info",
            "level=debug",
        ]
        .iter()
        .any(|s| lower.contains(s))
    {
        "info"
    } else {
        "unknown"
    }
}

fn timestamp(line: &str) -> Option<String> {
    // Preserve offsets on ISO timestamps; Nginx's unzoned timestamps are local.
    let token = line.split_whitespace().next().unwrap_or("");
    if token.as_bytes().get(4) == Some(&b'-') && token.contains('T') {
        return Some(token.trim_start_matches('[').trim_end_matches(']').into());
    }
    if line.as_bytes().get(4) == Some(&b'/') && line.as_bytes().get(10) == Some(&b' ') {
        return line
            .get(..19)
            .map(|value| value.replace('/', "-").replace(' ', "T"));
    }
    if let Some(stamp) = line
        .strip_prefix('[')
        .and_then(|v| v.split_once(']').map(|v| v.0))
    {
        let parts: Vec<_> = stamp.split_whitespace().collect();
        if parts.len() >= 2 {
            let date: Vec<_> = parts[0].split('-').collect();
            if date.len() == 3 {
                let months = [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ];
                if let Some(month) = months.iter().position(|m| *m == date[1]) {
                    return Some(format!(
                        "{}-{:02}-{:0>2}T{}{}",
                        date[2],
                        month + 1,
                        date[0],
                        parts[1],
                        if parts.get(2) == Some(&"UTC") {
                            "Z"
                        } else {
                            ""
                        }
                    ));
                }
            }
        }
    }
    None
}

fn request_severity(status: u16) -> &'static str {
    match status {
        500..=599 => "error",
        400..=499 => "warning",
        100..=399 => "info",
        _ => "unknown",
    }
}

const ENTRY_BYTES: usize = 4 * 1024;
const SHORTENED: &str = "\n… Entry shortened. Open the logs folder for the full message.";

fn bound_text(text: &mut String) -> bool {
    if text.len() <= ENTRY_BYTES {
        return false;
    }
    let mut end = ENTRY_BYTES - SHORTENED.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(SHORTENED);
    true
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn decode_legacy(value: &str) -> Vec<u8> {
    let input = value.as_bytes();
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input.get(index..index + 2) == Some(b"\\x") {
            if let (Some(a), Some(b)) = (
                input.get(index + 2).and_then(|b| hex(*b)),
                input.get(index + 3).and_then(|b| hex(*b)),
            ) {
                output.push(a * 16 + b);
                index += 4;
                continue;
            }
        }
        output.push(input[index]);
        index += 1;
    }
    output
}

fn readable_path(value: &str) -> String {
    // Decode valid UTF-8 URLs for reading, leaving malformed encodings intact.
    let input = value.split('?').next().unwrap_or(value).as_bytes();
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            if let (Some(a), Some(b)) = (
                input.get(index + 1).and_then(|b| hex(*b)),
                input.get(index + 2).and_then(|b| hex(*b)),
            ) {
                let byte = a * 16 + b;
                if byte >= 0x80 {
                    output.push(byte);
                } else {
                    output.extend_from_slice(&input[index..index + 3]);
                }
                index += 3;
                continue;
            }
        }
        output.push(input[index]);
        index += 1;
    }
    let decoded =
        String::from_utf8(output).unwrap_or_else(|_| String::from_utf8_lossy(input).into_owned());
    let mut readable = String::with_capacity(decoded.len());
    for character in decoded.chars() {
        if character.is_control() {
            readable.extend(character.escape_default());
        } else {
            readable.push(character);
        }
    }
    readable
}

fn tls_request(value: &[u8]) -> bool {
    value.starts_with(&[0x16, 0x03])
}

fn bad_request(tls: bool, status: &str) -> String {
    if tls {
        format!("HTTPS traffic reached an HTTP port → {status}. Use http:// for this port, or open the site's HTTPS address.")
    } else {
        format!(
            "Invalid HTTP request → {status}. Check the address and protocol used for this port."
        )
    }
}

fn legacy_time(line: &str) -> Option<String> {
    let stamp = line.split_once('[')?.1.split_once(']')?.0;
    let (date, rest) = stamp.split_once(':')?;
    let parts: Vec<_> = date.split('/').collect();
    let (clock, offset) = rest.split_once(' ')?;
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    if parts.len() != 3 || offset.len() != 5 || !matches!(offset.as_bytes()[0], b'+' | b'-') {
        return None;
    }
    let month = months.iter().position(|month| *month == parts[1])? + 1;
    Some(format!(
        "{}-{month:02}-{:0>2}T{clock}{}:{}",
        parts[2],
        parts[0],
        offset.get(..3)?,
        offset.get(3..)?
    ))
}

fn entry(file: &str, source: &str, offset: u64, line: &str) -> Entry {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    line.hash(&mut hash);
    let mut item = Entry {
        // Including content distinguishes replacement/truncation at the same offset.
        id: format!("{file}:{offset}:{:x}", hash.finish()),
        source: source.into(),
        file: file.into(),
        severity: severity(line).into(),
        time: timestamp(line),
        text: line.into(),
        host: None,
        truncated: false,
    };
    if source == "requests" {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            if let (Some(method), Some(uri), Some(status), Some(host)) = (
                value["method"].as_str(),
                value["uri"].as_str(),
                value["status"].as_str(),
                value["host"].as_str(),
            ) {
                // Only structured request metadata establishes host scope. Never
                // infer a site from PHP paths, message text or legacy access lines.
                let host_key = host.trim_end_matches('.').to_ascii_lowercase();
                if !host_key.is_empty() && host_key != "-" && host_key.len() <= 255 {
                    item.host = Some(host_key);
                }
                let status_code = status.parse::<u16>().unwrap_or(0);
                item.severity = request_severity(status_code).into();
                item.time = value["time"].as_str().map(String::from);
                let uri = readable_path(uri);
                item.text = if tls_request(method.as_bytes())
                    || method == "-"
                    || method.is_empty()
                    || !method.is_ascii()
                    || method
                        .bytes()
                        .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
                {
                    bad_request(tls_request(method.as_bytes()), status)
                } else {
                    format!(
                        "{method} {}://{host}{} {uri} → {status} · {} s",
                        value["scheme"].as_str().unwrap_or("http"),
                        value["port"]
                            .as_str()
                            .map(|p| format!(":{p}"))
                            .unwrap_or_default(),
                        value["duration"].as_str().unwrap_or("?")
                    )
                };
            }
        } else {
            // Pre-upgrade Nginx logs used the standard combined format. Keep
            // their original text, but do not show successful requests as problems.
            let mut fields = line.split('"');
            let _prefix = fields.next();
            let request = fields.next().unwrap_or_default();
            let status = fields
                .next()
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse::<u16>().ok());
            if let Some(status) = status {
                item.severity = request_severity(status).into();
                item.time = legacy_time(line);
                let bytes = decode_legacy(request);
                item.text = if tls_request(&bytes) {
                    bad_request(true, &status.to_string())
                } else {
                    let request = String::from_utf8_lossy(&bytes);
                    let mut words = request.split_whitespace();
                    match (words.next(), words.next(), words.next()) {
                        (Some(method), Some(path), Some(version))
                            if version.starts_with("HTTP/") =>
                        {
                            format!("{method} {} → {status}", readable_path(path))
                        }
                        _ => bad_request(false, &status.to_string()),
                    }
                };
            }
        }
    }
    item.truncated = bound_text(&mut item.text);
    item
}

fn read_file(directory: &Path, file: &str, source: &str) -> Result<(Vec<Entry>, bool)> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut input = match options.open(directory.join(file)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((vec![], false)),
        Err(error) => return Err(error.to_string()),
    };
    let metadata = input.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Expected a regular log file.".into());
    }
    let size = metadata.len();
    let offset = size.saturating_sub(TAIL_BYTES);
    input
        .seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    // A continuously writing service cannot enlarge a single read without bound.
    input
        .take(size - offset)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let mut start = 0;
    if offset != 0 {
        // Never render the beginning of a cut line as a complete event.
        start = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |p| p + 1);
    }
    if offset != 0 && start == bytes.len() {
        let notice = entry(file, source, offset, "[warning] An oversized or incomplete entry exceeds the 64 KiB read window. Open the logs folder for the full message.");
        return Ok((vec![notice], true));
    }
    let mut entries: Vec<Entry> = Vec::new();
    for line in bytes[start..].split_inclusive(|b| *b == b'\n') {
        // Leave an incomplete write for the next snapshot.
        if !line.ends_with(b"\n") {
            break;
        }
        let text = String::from_utf8_lossy(line).trim_end().to_string();
        if !text.is_empty() {
            let continuation = text.starts_with(char::is_whitespace)
                || text.starts_with("Stack trace:")
                || text.starts_with('#')
                || text.starts_with("thrown in ");
            if continuation && !entries.is_empty() && source != "requests" {
                let previous = entries.last_mut().unwrap();
                if !previous.truncated {
                    previous.text.push('\n');
                    previous.text.push_str(&text);
                    previous.truncated = bound_text(&mut previous.text);
                }
            } else {
                entries.push(entry(file, source, offset + start as u64, &text));
            }
        }
        start += line.len();
    }
    let limited = offset != 0 || entries.len() > ENTRIES_PER_FILE;
    if entries.len() > ENTRIES_PER_FILE {
        entries.drain(..entries.len() - ENTRIES_PER_FILE);
    }
    Ok((entries, limited))
}

pub fn read(directory: &Path) -> Snapshot {
    let mut snapshot = Snapshot::default();
    for (file, source) in SOURCES {
        match read_file(directory, file, source) {
            Ok((entries, limited)) => {
                snapshot.entries.extend(entries);
                snapshot.limited |= limited;
            }
            Err(error) => snapshot.unavailable.push(format!("{file}: {error}")),
        }
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    snapshot.entries.hash(&mut hash);
    snapshot.unavailable.hash(&mut hash);
    snapshot.limited.hash(&mut hash);
    snapshot.revision = format!("{:x}", hash.finish());
    snapshot
}

/// Trim only before starting the managed owning service, when its log handles
/// are closed. Avoid live copy/truncate races, especially with Windows runtimes.
/// Long-running sessions may exceed this threshold until the next start.
pub fn trim_stopped(directory: &Path, service: &str) -> Result<()> {
    for (file, source) in SOURCES {
        if *source != service && !(service == "nginx" && *source == "requests") {
            continue;
        }
        let path = directory.join(file);
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if metadata.len() <= 2 * 1024 * 1024 {
            continue;
        }
        let mut input = File::open(&path).map_err(|e| e.to_string())?;
        input
            .seek(SeekFrom::Start(metadata.len() - 256 * 1024))
            .map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        input
            .take(256 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let start = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |p| p + 1);
        // The file is stopped; replacing it preserves the current user's permissions.
        crate::config::atomic_write(&path, &bytes[start..])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!("fairway-logs-{}", rand::random::<u64>()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn snapshot_preserves_multiline_and_unknown_output_and_waits_for_partial_writes() {
        let path = directory();
        fs::write(path.join("php.log"), "[18-Oct-2026 12:00:00 UTC] PHP Fatal error: broken\nStack trace:\n#0 index.php\n  thrown in index.php\ncustom diagnostic\npartial").unwrap();
        let result = read(&path);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].severity, "error");
        assert!(result.entries[0].text.contains("#0 index.php"));
        assert_eq!(result.entries[1].severity, "unknown");
        assert!(!result.entries.iter().any(|e| e.text.contains("partial")));
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn fpm_severities_and_native_timestamps_are_understood() {
        let notice = "[18-Oct-2026 12:00:00] NOTICE: ready to handle connections";
        assert_eq!(severity(notice), "info");
        assert_eq!(
            severity("level=error msg=\"upstream returned [INFO]\""),
            "error"
        );
        assert_eq!(
            severity("level=warn msg=\"upstream returned [INFO]\""),
            "warning"
        );
        assert_eq!(severity(&notice.replace("NOTICE:", "WARNING:")), "warning");
        assert_eq!(severity(&notice.replace("NOTICE:", "ERROR:")), "error");
        assert_eq!(timestamp(notice).as_deref(), Some("2026-10-18T12:00:00"));
        assert_eq!(
            timestamp("[18-Oct-2026 12:00:00 UTC] PHP Warning: bad").as_deref(),
            Some("2026-10-18T12:00:00Z")
        );
        assert_eq!(
            timestamp("2026/10/18 12:00:00 [error] bad").as_deref(),
            Some("2026-10-18T12:00:00")
        );
        assert_eq!(
            timestamp("2026-10-18T12:00:00.000Z 0 [ERROR] bad").as_deref(),
            Some("2026-10-18T12:00:00.000Z")
        );
    }
    #[test]
    fn request_status_is_classified_and_json_is_never_html() {
        let line = r#"{"method":"GET","uri":"/<script>","status":"503","host":"shop.test","time":"2026-10-18T12:00:00+00:00","scheme":"https","port":"8843","duration":"0.010"}"#;
        let result = entry("access.log", "requests", 0, line);
        assert_eq!(result.severity, "error");
        assert!(result.text.contains("https://shop.test:8843"));
        assert!(result.text.contains("/<script>"));
        let legacy =
            r#"127.0.0.1 - - [18/Oct/2026:12:00:00 +0000] "GET /old HTTP/1.1" 200 10 "-" "-""#;
        assert_eq!(entry("access.log", "requests", 0, legacy).severity, "info");
        assert_eq!(
            entry(
                "access.log",
                "requests",
                0,
                &legacy.replace("200 10", "404 10")
            )
            .severity,
            "warning"
        );
        assert_eq!(
            entry("access.log", "requests", 0, &line.replace("503", "404")).severity,
            "warning"
        );
        assert_eq!(
            entry("access.log", "requests", 0, &line.replace("503", "200")).severity,
            "info"
        );
    }
    #[test]
    fn reads_are_bounded_and_replacement_and_truncation_are_fresh() {
        let path = directory();
        fs::write(
            path.join("mysql.log"),
            "[ERROR] old failure\n".repeat(20000),
        )
        .unwrap();
        let result = read(&path);
        assert!(result.limited);
        assert_eq!(result.entries.len(), ENTRIES_PER_FILE);
        fs::write(path.join("mysql.log"), "[System] restarted\n").unwrap();
        let result = read(&path);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].severity, "info");
        fs::remove_file(path.join("mysql.log")).unwrap();
        fs::create_dir(path.join("php.log")).unwrap();
        assert_eq!(read(&path).unavailable.len(), 1);
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn site_scope_uses_only_recorded_request_hosts() {
        let request = r#"{"method":"GET","uri":"/index.php","status":"500","host":"Shop.TEST.","scheme":"https"}"#;
        assert_eq!(
            entry("access.log", "requests", 0, request).host.as_deref(),
            Some("shop.test")
        );
        let php = entry(
            "php.log",
            "php",
            0,
            "PHP Fatal error: /projects/shop.test/index.php",
        );
        assert!(php.host.is_none());
        let legacy = r#"127.0.0.1 - - [18/Oct/2026:12:00:00 +0000] "GET /shop.test/index.php HTTP/1.1" 500 10 "-" "-""#;
        assert!(entry("access.log", "requests", 0, legacy).host.is_none());
        assert!(entry(
            "access.log",
            "requests",
            0,
            &request.replace("Shop.TEST.", "-")
        )
        .host
        .is_none());
    }
    #[test]
    fn legacy_tls_and_unicode_urls_are_readable() {
        let tls = r#"127.0.0.1 - - [18/Oct/2026:09:35:34 +0630] "\x16\x03\x01\x05\xF7garbage" 400 157 "-" "-""#;
        let result = entry("access.log", "requests", 0, tls);
        assert_eq!(result.severity, "warning");
        assert!(result.text.contains("HTTPS traffic reached an HTTP port"));
        assert!(!result.text.contains("garbage"));
        assert_eq!(result.time.as_deref(), Some("2026-10-18T09:35:34+06:30"));
        let unicode = r#"127.0.0.1 - - [18/Oct/2026:09:35:34 +0630] "GET /\xE6\x97\xA5\xE6\x9C\xAC HTTP/1.1" 404 157 "-" "-""#;
        assert!(entry("access.log", "requests", 0, unicode)
            .text
            .contains("/日本"));
        let json = r#"{"method":"GET","uri":"/%E1%80%99%E1%80%BC%E1%80%94%E1%80%BA%E1%80%99%E1%80%AC?private=secret","status":"404","host":"shop.test"}"#;
        let result = entry("access.log", "requests", 0, json);
        assert!(result.text.contains("/မြန်မာ"));
        assert!(!result.text.contains("secret"));
        assert_eq!(readable_path("/a%0Ab"), "/a%0Ab");
        assert_eq!(readable_path("/%2F%3F"), "/%2F%3F");
        assert_eq!(readable_path("/%FF"), "/%FF");
    }
    #[test]
    fn oversized_unicode_messages_and_multiline_traces_have_a_hard_size_bound() {
        let path = directory();
        fs::write(
            path.join("php.log"),
            format!(
                "PHP Fatal error: {}\n{}",
                "မြန်မာ".repeat(1000),
                "  trace frame 日本\n".repeat(1000)
            ),
        )
        .unwrap();
        let result = read(&path);
        assert_eq!(result.entries.len(), 1);
        let item = &result.entries[0];
        assert!(item.text.len() <= ENTRY_BYTES);
        assert!(item.text.ends_with(SHORTENED));
        assert!(!item.text.contains('\u{fffd}'));
        fs::write(path.join("mysql.log"), "x".repeat(128 * 1024) + "\n").unwrap();
        let result = read(&path);
        let notice = result
            .entries
            .iter()
            .find(|entry| entry.source == "mysql")
            .unwrap();
        assert_eq!(notice.severity, "warning");
        assert!(notice.text.contains("exceeds the 64 KiB read window"));
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn giant_and_continuously_growing_logs_do_not_expand_snapshots() {
        use std::{
            io::Write,
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            },
        };
        let path = directory();
        let file = File::create(path.join("access.log")).unwrap();
        file.set_len(if cfg!(windows) {
            16 * 1024 * 1024
        } else {
            1024 * 1024 * 1024
        })
        .unwrap(); // Sparse on Unix; keep Windows allocation small.
        let stop = Arc::new(AtomicBool::new(false));
        let writing = stop.clone();
        let destination = path.join("php.log");
        let writer = std::thread::spawn(move || {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(destination)
                .unwrap();
            while !writing.load(Ordering::Relaxed) {
                file.write_all(b"PHP Warning: repeated diagnostic\n")
                    .unwrap();
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        });
        for _ in 0..30 {
            let result = read(&path);
            assert!(result.limited);
            assert!(result.entries.len() <= SOURCES.len() * ENTRIES_PER_FILE);
            assert!(serde_json::to_vec(&result).unwrap().len() < 2 * 1024 * 1024);
        }
        stop.store(true, Ordering::Relaxed);
        writer.join().unwrap();
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn read_permit_rejects_overlapping_work_and_recovers_after_drop() {
        let reader = LogDirectory::new(PathBuf::from("unused"));
        let (_, permit) = reader.begin().unwrap();
        assert!(reader.begin().is_err());
        drop(permit);
        assert!(reader.begin().is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn special_files_cannot_block_log_reading() {
        use std::os::unix::ffi::OsStrExt;
        let path = directory();
        let fifo = std::ffi::CString::new(path.join("php.log").as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert_eq!(read(&path).unavailable.len(), 1);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn stopped_retention_keeps_recent_output_and_other_services_intact() {
        let path = directory();
        fs::write(
            path.join("access.log"),
            "old request\n".repeat(200000) + "latest request\n",
        )
        .unwrap();
        fs::write(path.join("php.log"), "php error\n".repeat(250000)).unwrap();
        trim_stopped(&path, "nginx").unwrap();
        assert!(fs::metadata(path.join("access.log")).unwrap().len() <= 256 * 1024);
        assert!(fs::read_to_string(path.join("access.log"))
            .unwrap()
            .ends_with("latest request\n"));
        assert!(fs::metadata(path.join("php.log")).unwrap().len() > 2 * 1024 * 1024);
        fs::remove_dir_all(path).unwrap();
    }
}
