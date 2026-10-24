//! Check bundled client extensions and Unix workers through generated PHP configuration.
use fairway_core::{
    config::{Layout, Ports},
    manager::Manager,
    process,
};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    time::Duration,
};

fn check(body: &str, version: &Value, cli: bool) -> Result<(), String> {
    let result: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let worker = if cfg!(windows) {
        "unavailable"
    } else if cli {
        "ok"
    } else {
        "extensions"
    };
    if result["phpExtensions"] != "ok" || &result["php"] != version || result["worker"] != worker {
        return Err(format!("PHP extension check failed: {result}"));
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let root = current_dir.join(args.next().ok_or("Pass a fresh test directory.")?);
    let runtime = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime"));
    // Keep ordinary Windows paths: canonicalize adds a verbatim prefix that
    // native PHP configuration paths do not consistently accept.
    let runtime = current_dir.join(runtime);
    if root.exists() {
        return Err("Use a fresh test directory.".into());
    }
    let layout = Layout::at(
        root.join("student home"),
        root.join("student data"),
        runtime,
    );
    let mut manager = Manager::new(layout)?;
    let listeners = (0..6)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let ports: Vec<_> = listeners
        .iter()
        .map(|listener| listener.local_addr().unwrap().port())
        .collect();
    manager.save_ports(Ports {
        website: ports[0],
        phpmyadmin: ports[1],
        mysql: ports[2],
        php: ports[3],
        mail: ports[4],
        smtp: ports[5],
    })?;
    drop(listeners);
    let script = manager.layout.www.join("php-extensions.php");
    fs::write(
        &script,
        include_str!("../../tests/fixtures/php_extensions.php"),
    )
    .map_err(|e| e.to_string())?;
    for (minor, version) in manager.layout.php_versions()? {
        manager.save_php_version(minor.clone())?;
        let mut command = process::command(manager.layout.php_exe("php")?);
        command
            .arg("-c")
            .arg(fairway_core::config::windows_ansi_path(
                &manager.layout.data.join("config/php.ini"),
            ))
            .arg(&script)
            .env("PHP_INI_SCAN_DIR", "")
            .env("MIBS", "");
        let output = command.output().map_err(|e| e.to_string())?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(format!(
                "PHP {minor} CLI client check failed: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        check(&String::from_utf8_lossy(&output.stdout), &version, true)?;
        // Only Nginx/PHP start; the fixture supplies its own local sockets.
        let web = (|| -> Result<(), String> {
            manager.start("nginx")?;
            let mut stream = TcpStream::connect(("127.0.0.1", manager.ports.website))
                .map_err(|e| e.to_string())?;
            stream
                .set_read_timeout(Some(Duration::from_secs(20)))
                .map_err(|e| e.to_string())?;
            stream
                .write_all(b"GET /php-extensions.php HTTP/1.0\r\nHost: localhost\r\n\r\n")
                .map_err(|e| e.to_string())?;
            let mut response = String::new();
            stream
                .read_to_string(&mut response)
                .map_err(|e| e.to_string())?;
            if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
                return Err(format!("PHP {minor} web client check failed: {response}"));
            }
            check(
                response
                    .split_once("\r\n\r\n")
                    .ok_or("Missing HTTP body")?
                    .1,
                &version,
                false,
            )
        })();
        let stopped = manager.stop_all();
        stopped?;
        web?;
        println!("PASS: PHP {minor} sockets, SOAP and LDAP in CLI/web; Unix CLI worker signals and child status where supported.");
    }
    Ok(())
}
