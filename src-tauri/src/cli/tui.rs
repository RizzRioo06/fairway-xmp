use super::{supervisor, Request};
use crate::{
    config::{Layout, Ports, Result},
    manager::Snapshot,
};
use std::{
    io::{self, Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

static EXIT: AtomicBool = AtomicBool::new(false);
extern "C" fn signal_handler(_: libc::c_int) {
    EXIT.store(true, Ordering::Relaxed);
}

struct Terminal {
    original: libc::termios,
}
impl Terminal {
    fn enter() -> Result<Self> {
        let mut original = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(0, &mut original) } != 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        let terminal = Self { original };
        terminal.resume()?;
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = signal_handler as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
            libc::sigaction(libc::SIGHUP, &action, std::ptr::null_mut());
        }
        Ok(terminal)
    }
    fn resume(&self) -> Result<()> {
        let mut raw = self.original;
        unsafe {
            libc::cfmakeraw(&mut raw);
        }
        if unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        print!("\x1b[?1049h\x1b[?25l");
        io::stdout().flush().map_err(|e| e.to_string())
    }
    fn pause(&self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.original);
        }
        print!("\x1b[?25h\x1b[?1049l");
        let _ = io::stdout().flush();
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        self.pause();
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

pub(super) fn run(layout: &Layout) -> Result<()> {
    if std::env::var("TERM").is_ok_and(|s| s == "dumb") {
        return Err("Use fairway status in a terminal without ANSI support.".into());
    }
    let terminal = Terminal::enter()?;
    let (sender, receiver) = mpsc::channel::<Result<serde_json::Value>>();
    let mut snapshot: Option<Snapshot> = None;
    let mut selected = 0;
    let mut message =
        "q closes this dashboard and leaves services running. Q shuts everything down.".to_string();
    let mut log: Option<String> = None;
    let mut busy = false;
    let mut shutting_down = false;
    let mut refresh = Instant::now() - Duration::from_secs(3);
    while !EXIT.load(Ordering::Relaxed) {
        if let Ok(result) = receiver.try_recv() {
            busy = false;
            match result {
                Ok(value) => {
                    if shutting_down {
                        return Ok(());
                    }
                    if let Some(text) = value.as_str() {
                        message = clean(text);
                    } else {
                        message = "Done.".into();
                    }
                }
                Err(error) => {
                    message = clean(&error);
                    shutting_down = false;
                }
            }
            refresh = Instant::now() - Duration::from_secs(3);
        }
        if !busy && refresh.elapsed() >= Duration::from_secs(2) {
            match supervisor::call(layout, &Request::Status)
                .and_then(|data| serde_json::from_value(data).map_err(|e| e.to_string()))
            {
                Ok(value) => snapshot = Some(value),
                Err(error) => message = clean(&error),
            }
            refresh = Instant::now();
        }
        draw(snapshot.as_ref(), selected, busy, &message, log.as_deref())?;
        let mut descriptor = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut descriptor, 1, 200) } <= 0 {
            continue;
        }
        let mut input = [0_u8; 32];
        let count = io::stdin().read(&mut input).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        let key = &input[..count];
        if matches!(key, b"q" | b"\x03") {
            break;
        }
        if matches!(key, b"\x1b[A" | b"k") {
            selected = selected.saturating_sub(1);
            continue;
        }
        if matches!(key, b"\x1b[B" | b"j") {
            selected = (selected + 1).min(1);
            continue;
        }
        if busy {
            continue;
        }
        let service = if selected == 0 { "nginx" } else { "mysql" }.to_string();
        let request = match key {
            b"s" => Request::Start { service },
            b"x" => Request::Stop { service },
            b"r" => Request::Restart { service },
            b"a" => Request::Start {
                service: "all".into(),
            },
            b"z" => Request::Stop {
                service: "all".into(),
            },
            b"b" => Request::Backup,
            b"Q" => {
                shutting_down = true;
                Request::Shutdown
            }
            b"l" | b"f" => {
                let id = if key == b"f" { "php" } else { &service };
                log = Some(
                    match supervisor::call(layout, &Request::Log { service: id.into() }) {
                        Ok(value) => clean(value.as_str().unwrap_or("")),
                        Err(error) => error,
                    },
                );
                continue;
            }
            b"\x1b" => {
                log = None;
                continue;
            }
            b"p" => {
                terminal.pause();
                println!("Enter four ports: website phpMyAdmin MySQL PHP-FPM (blank cancels).");
                let mut line = String::new();
                let result = io::stdin().read_line(&mut line);
                terminal.resume()?;
                result.map_err(|e| e.to_string())?;
                if line.trim().is_empty() {
                    continue;
                }
                let values = line
                    .split_whitespace()
                    .map(str::parse::<u16>)
                    .collect::<std::result::Result<Vec<_>, _>>();
                match values {
                    Ok(values) if values.len() == 4 => Request::Ports {
                        ports: Ports {
                            website: values[0],
                            phpmyadmin: values[1],
                            mysql: values[2],
                            php: values[3],
                        },
                    },
                    _ => {
                        message = "Enter four valid port numbers.".into();
                        continue;
                    }
                }
            }
            _ => continue,
        };
        let layout = layout.clone();
        let sender = sender.clone();
        thread::spawn(move || {
            let _ = sender.send(supervisor::call(&layout, &request));
        });
        busy = true;
        message = "Working… You can close the dashboard with q; the operation will finish in the background.".into();
    }
    Ok(())
}

fn draw(
    snapshot: Option<&Snapshot>,
    selected: usize,
    busy: bool,
    message: &str,
    log: Option<&str>,
) -> Result<()> {
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    unsafe {
        libc::ioctl(1, libc::TIOCGWINSZ, &mut size);
    }
    let rows = if size.ws_row == 0 {
        24
    } else {
        size.ws_row as usize
    };
    let columns = if size.ws_col == 0 {
        80
    } else {
        size.ws_col as usize
    };
    let mut lines = vec![
        format!(
            "\x1b[1mFairway XMP {}\x1b[0m  •  Linux terminal edition",
            env!("CARGO_PKG_VERSION")
        ),
        String::new(),
    ];
    if let Some(snapshot) = snapshot {
        for (index, id) in ["nginx", "mysql"].iter().enumerate() {
            if let Some(service) = snapshot.services.iter().find(|s| s.id == *id) {
                let (color, label) = if service.ready {
                    (32, "Ready")
                } else if service.running {
                    (33, "Starting / not ready")
                } else {
                    (31, "Stopped")
                };
                lines.push(format!(
                    "{} \x1b[{color}m●\x1b[0m {:<7} {:<22} port {}",
                    if index == selected { ">" } else { " " },
                    id,
                    label,
                    service.port
                ));
            }
        }
        lines.push("  PHP-FPM follows Nginx; PHP CLI is always available.".into());
        lines.push(format!(
            "  Website:    http://127.0.0.1:{}/",
            snapshot.ports.website
        ));
        lines.push(format!(
            "  phpMyAdmin: http://127.0.0.1:{}/",
            snapshot.ports.phpmyadmin
        ));
        lines.push(format!("  Projects:   {}", clean(&snapshot.document_root)));
        lines.push(format!("  Data:       {}", clean(&snapshot.data_dir)));
    }
    lines.push(String::new());
    lines.push("↑/↓ Select  s Start  x Stop  r Restart  a Start all  z Stop all".into());
    lines.push("l Logs  f PHP logs  p Ports  b Backup  q Close  Q Shutdown".into());
    lines.push(String::new());
    for line in message.lines().take(3) {
        lines.push(line.chars().take(columns).collect());
    }
    if let Some(log) = log {
        lines.push("── Recent log entries (Esc hides) ──".into());
        let room = rows.saturating_sub(lines.len() + 1);
        let entries: Vec<_> = log.lines().collect();
        for line in entries.iter().skip(entries.len().saturating_sub(room)) {
            lines.push(line.chars().take(columns).collect());
        }
    }
    print!("\x1b[H\x1b[2J");
    for line in lines.into_iter().take(rows.saturating_sub(1)) {
        print!("{line}\r\n");
    }
    if busy {
        print!("\x1b[33mOperation in progress\x1b[0m");
    }
    io::stdout().flush().map_err(|e| e.to_string())
}
