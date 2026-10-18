//! Read the bundled Mailpit inbox through its local REST API.
use crate::config::Result;
use serde::Deserialize;
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    List { query: String },
    Message { id: String },
    Delete { ids: Vec<String> },
    Clear,
}

const LIMIT: u32 = 200;
const PREVIEW_LIMIT: u64 = 8 * 1024 * 1024;
const ATTACHMENT_LIMIT: u64 = 64 * 1024 * 1024;

pub fn execute(port: u16, request: Request) -> Result<serde_json::Value> {
    match request {
        Request::List { query } if query.trim().is_empty() => {
            call(port, "GET", &format!("/api/v1/messages?limit={LIMIT}"), "")
        }
        Request::List { query } => call(
            port,
            "GET",
            &format!(
                "/api/v1/search?query={}&limit={LIMIT}",
                encode(query.trim())
            ),
            "",
        ),
        // Mailpit marks a message read when it is opened.
        Request::Message { id } => call(
            port,
            "GET",
            &format!("/api/v1/message/{}", checked(&id)?),
            "",
        ),
        Request::Delete { ids } => {
            if ids.is_empty() {
                return Err("Choose an email to delete.".into());
            }
            for id in &ids {
                checked(id)?;
            }
            let body = serde_json::json!({ "IDs": ids }).to_string();
            call(port, "DELETE", "/api/v1/messages", &body)
        }
        // An empty DELETE removes every message.
        Request::Clear => call(port, "DELETE", "/api/v1/messages", ""),
    }
}

/// Mailpit IDs are URL-safe; reject anything else before building a path.
fn checked(id: &str) -> Result<&str> {
    if !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Ok(id)
    } else {
        Err("This email is no longer available.".into())
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Raw bytes of one attachment, for saving to disk.
pub fn attachment(port: u16, id: &str, part: &str) -> Result<Vec<u8>> {
    // Part IDs are dotted numbers such as "2" or "1.2".
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return Err("This attachment is no longer available.".into());
    }
    fetch(
        port,
        "GET",
        &format!("/api/v1/message/{}/part/{part}", checked(id)?),
        "",
        ATTACHMENT_LIMIT,
    )
}

fn call(port: u16, method: &str, path: &str, body: &str) -> Result<serde_json::Value> {
    let payload = fetch(port, method, path, body, PREVIEW_LIMIT)?;
    if payload.trim_ascii() == b"ok" {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_slice(&payload)
        .map_err(|e| format!("Mail returned an unexpected response: {e}"))
}

// ponytail: HTTP/1.0 makes Mailpit close the connection after a plain body, so
// no chunked decoding or client library is needed for this loopback-only API.
fn fetch(port: u16, method: &str, path: &str, body: &str, limit: u64) -> Result<Vec<u8>> {
    let mut stream =
        TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2))
            .map_err(|_| "Mail is not running. Start mail to see caught emails.".to_string())?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    write!(
        stream,
        "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    stream
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    parse_response(bytes, limit)
}

fn parse_response(mut bytes: Vec<u8>, limit: u64) -> Result<Vec<u8>> {
    if bytes.len() as u64 > limit {
        return Err("This email or attachment is too large for the built-in viewer. Open the mail inbox in your browser.".into());
    }
    let split = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("Mail returned an unexpected response.")?;
    if split > 64 * 1024 {
        return Err("Mail returned oversized response headers.".into());
    }
    let head = std::str::from_utf8(&bytes[..split])
        .map_err(|_| "Mail returned invalid response headers.")?;
    let status = head.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(if status.contains(" 404") {
            "This email is no longer available.".into()
        } else {
            format!("Mail could not complete the request ({status}).")
        });
    }
    let mut length = None;
    for line in head.lines().skip(1) {
        let (name, value) = line
            .split_once(':')
            .ok_or("Mail returned invalid response headers.")?;
        if name.eq_ignore_ascii_case("transfer-encoding")
            && !value.trim().eq_ignore_ascii_case("identity")
        {
            return Err("Mail returned an unsupported response encoding.".into());
        }
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err("Mail returned conflicting response lengths.".into());
            }
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "Mail returned an invalid response length.")?,
            );
        }
    }
    if length.is_some_and(|length| length != bytes.len() - split - 4) {
        return Err("The mail download was incomplete. Retry before saving the attachment.".into());
    }
    Ok(bytes.split_off(split + 4))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn responses_reject_truncation_oversize_and_unsupported_framing() {
        let parse = |value: &str| parse_response(value.as_bytes().to_vec(), 1024);
        assert_eq!(
            parse("HTTP/1.0 200 OK\r\nContent-Length: 3\r\n\r\nabc").unwrap(),
            b"abc"
        );
        assert!(parse("HTTP/1.0 200 OK\r\nContent-Length: 4\r\n\r\nabc")
            .unwrap_err()
            .contains("incomplete"));
        assert!(parse("HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nabc").is_err());
        assert!(
            parse("HTTP/1.0 200 OK\r\nContent-Length: 3\r\ncontent-length: 3\r\n\r\nabc").is_err()
        );
        assert!(parse(
            "HTTP/1.0 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n"
        )
        .is_err());
        assert!(parse_response(vec![b'x'; 1025], 1024)
            .unwrap_err()
            .contains("too large"));
    }

    #[test]
    fn requests_are_encoded_validated_and_parsed() {
        assert_eq!(encode("a b&c/ü"), "a%20b%26c%2F%C3%BC");
        assert!(checked("Pn6Z7wnHjTs4XKqyTv4vTQ").is_ok());
        assert!(checked("../x").is_err() && checked("").is_err());
        assert!(execute(1, Request::Delete { ids: vec![] }).is_err());
        assert!(attachment(1, "abc", "../1").is_err() && attachment(1, "abc", "").is_err());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut request = Vec::new();
            for reply in [
                "HTTP/1.0 200 OK\r\n\r\n{\"total\":1}",
                "HTTP/1.0 404 Not Found\r\n\r\n",
                "HTTP/1.0 200 OK\r\n\r\n\x00\x7f\r\n\r\n",
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                let mut received = Vec::new();
                let mut buffer = [0; 1024];
                // Read the whole request so closing the socket does not reset it.
                while !received.ends_with(b"\r\n\r\n") {
                    let read = socket.read(&mut buffer).unwrap();
                    received.extend_from_slice(&buffer[..read]);
                }
                request.push(String::from_utf8_lossy(&received).into_owned());
                socket.write_all(reply.as_bytes()).unwrap();
            }
            request
        });
        let found = execute(
            port,
            Request::List {
                query: "to:a b".into(),
            },
        )
        .unwrap();
        assert_eq!(found["total"], 1);
        assert_eq!(
            execute(port, Request::Message { id: "gone".into() }).unwrap_err(),
            "This email is no longer available."
        );
        assert_eq!(attachment(port, "abc", "1.2").unwrap(), b"\x00\x7f\r\n\r\n");
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /api/v1/search?query=to%3Aa%20b&limit=200 HTTP/1.0"));
        assert!(requests[1].starts_with("GET /api/v1/message/gone HTTP/1.0"));
        assert!(requests[2].starts_with("GET /api/v1/message/abc/part/1.2 HTTP/1.0"));
    }
}
