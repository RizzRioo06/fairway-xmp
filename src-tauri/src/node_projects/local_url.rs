//! Advisory addresses reported by the current child, never readiness or process authority.
use super::validate_url;

#[derive(Default)]
pub(super) struct ReportedUrl {
    line: Vec<u8>,
    overflow: bool,
    escape: u8,
}
impl ReportedUrl {
    pub fn output(&mut self, bytes: &[u8]) -> Option<String> {
        let mut found = None;
        for &byte in bytes {
            // Strip bounded ANSI CSI sequences, including sequences split across frames.
            if self.escape != 0 {
                self.escape = match (self.escape, byte) {
                    (1, b'[') => 2,
                    (2, 0x40..=0x7e) => 0,
                    (2, _) => 2,
                    _ => 0,
                };
                continue;
            }
            if byte == 0x1b {
                self.escape = 1;
            } else if byte == b'\n' || byte == b'\r' {
                if !self.overflow {
                    if let Ok(line) = std::str::from_utf8(&self.line) {
                        if let Some(url) = address(line) {
                            found = Some(url.to_owned());
                        }
                    }
                }
                self.line.clear();
                self.overflow = false;
            } else if self.line.len() < 4096 && !self.overflow {
                self.line.push(byte);
            } else {
                self.overflow = true;
            }
        }
        found
    }
}
fn address(line: &str) -> Option<&str> {
    let line = line.trim();
    let url = [
        "➜  Local:",
        "Local:",
        "Node.js listening at ",
        "Express listening at ",
        "Hono listening at ",
    ]
    .iter()
    .find_map(|prefix| line.strip_prefix(prefix))?
    .trim();
    // Do not turn arbitrary output, external links, credentials or query secrets into actions.
    if validate_url(Some(url)).is_err() || url.contains(['?', '#']) {
        return None;
    }
    let authority = url.split_once("://")?.1;
    if authority.trim_end_matches('/').contains('/') {
        return None;
    }
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reported_addresses_are_bounded_loopback_and_frame_independent() {
        let mut parser = ReportedUrl::default();
        assert_eq!(parser.output(b"\x1b[3"), None);
        assert_eq!(
            parser.output("2m  ➜  Local:   http://127.0.0.1:51".as_bytes()),
            None
        );
        assert_eq!(
            parser.output(b"74/\x1b[0m\n"),
            Some("http://127.0.0.1:5174/".into())
        );
        assert_eq!(
            parser.output(b"Hono listening at http://[::1]:3010\n"),
            Some("http://[::1]:3010".into())
        );
        for line in [
            "Error: http://localhost:5173/",
            "Local: http://evil.test:5173/",
            "Local: http://localhost:0/",
            "Local: http://localhost:65536/",
            "Local: http://localhost:30/?token=secret",
            "Local: http://user@localhost:30/",
            "Local: http://localhost.evil:30/",
            "Local: http://localhost:30/extra",
        ] {
            assert_eq!(
                parser.output(format!("{line}\n").as_bytes()),
                None,
                "{line}"
            );
        }
        parser.output(&vec![b'x'; 8192]);
        assert_eq!(parser.output(b"Local: http://localhost:3000/\n"), None);
        assert_eq!(
            parser.output(b"Node.js listening at http://127.0.0.1:3001\n"),
            Some("http://127.0.0.1:3001".into())
        );
    }
}
