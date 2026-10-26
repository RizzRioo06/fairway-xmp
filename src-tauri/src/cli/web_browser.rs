//! The standalone browser is a foreground process, independent of the PHP supervisor.
use crate::config::{Layout, Result};
use serde::Deserialize;
use std::{
    fs::{self, OpenOptions},
    os::{
        fd::AsRawFd, unix::fs::DirBuilderExt, unix::fs::OpenOptionsExt, unix::process::CommandExt,
    },
    process::Command,
};

const HELP: &str = "fairway data-browser [--port PORT] [--prepare]\n\nBrowse PostgreSQL, MySQL and SQLite on loopback port 8803.\nOpen the printed access URL; its temporary token grants this browser session.\nRequires Node 22 or newer. First use downloads locked dependencies with npm;\nlater runs work offline. Keep this command running; Ctrl+C stops the browser.\n--port PORT    Use another loopback port (1024–65535)\n--prepare      Set up dependencies and exit, for later offline use\nFor a remote Linux host: ssh -L 8803:127.0.0.1:8803 user@host";

#[derive(Debug, PartialEq)]
struct Options {
    port: u16,
    prepare: bool,
}

fn options(args: &[String]) -> Result<Options> {
    let mut options = Options {
        port: 8803,
        prepare: false,
    };
    let mut arguments = args.iter();
    let mut port_seen = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--prepare" if !options.prepare => options.prepare = true,
            "--port" if !port_seen => {
                options.port = arguments
                    .next()
                    .ok_or("--port needs a number.")?
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port >= 1024)
                    .ok_or("Choose a data browser port between 1024 and 65535.")?;
                port_seen = true;
            }
            _ => return Err(format!("Unknown data browser option: {argument}\n{HELP}")),
        }
    }
    Ok(options)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Build {
    id: String,
    minimum_node_major: u32,
}

#[derive(Deserialize)]
struct Node {
    version: String,
    modules: String,
    platform: String,
    arch: String,
    executable: std::path::PathBuf,
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn node_guidance(layout: &Layout) -> String {
    let context = crate::fnm_setup::inspect(layout);
    let executable = context
        .external
        .unwrap_or_else(|| layout.runtime.join("fnm/bin/fnm"));
    let fnm = crate::integration::shell_quote(&executable.to_string_lossy());
    format!("Install Node using fnm, then retry:\n  {fnm} install --lts\n  eval \"$({fnm} env --shell bash)\"\n  {fnm} use lts-latest\nUse --shell zsh if that is your shell. For Fish, run: {fnm} env --shell fish | source\nNative fnm is also on PATH after install-cli.")
}

fn detect(layout: &Layout, minimum: u32) -> Result<Node> {
    let mut command = Command::new("node");
    command.args(["-p", "JSON.stringify({version:process.versions.node,modules:process.versions.modules,platform:process.platform,arch:process.arch,executable:process.execPath})"]);
    let output = crate::command_output::output(command, 10, &|| false).map_err(|_| {
        format!(
            "Node was not detected on your current PATH. {}",
            node_guidance(layout)
        )
    })?;
    let node: Node = serde_json::from_str(output.trim())
        .map_err(|_| "Node did not report a usable runtime identity.")?;
    let major = node
        .version
        .split('.')
        .next()
        .and_then(|value| value.parse::<u32>().ok());
    if major.is_none_or(|major| major < minimum) {
        return Err(format!(
            "Data browser requires Node {minimum} or newer; found v{}. {}",
            node.version,
            node_guidance(layout)
        ));
    }
    if ![&node.modules, &node.platform, &node.arch]
        .into_iter()
        .all(|value| safe_component(value))
        || !node.executable.is_absolute()
        || !node.executable.is_file()
    {
        return Err("Node reported an invalid runtime identity.".into());
    }
    Ok(node)
}

fn runtime_lock(layout: &Layout, exclusive: bool) -> Result<std::fs::File> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&layout.data)
        .map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(layout.data.join("data-browser.lock"))
        .map_err(|e| e.to_string())?;
    let kind = if exclusive {
        libc::LOCK_EX
    } else {
        libc::LOCK_SH
    };
    if unsafe { libc::flock(lock.as_raw_fd(), kind | libc::LOCK_NB) } != 0 {
        return Err(if exclusive {
            "Stop every fairway data-browser command with Ctrl+C before updating or uninstalling."
        } else {
            "Fairway is being updated or uninstalled. Retry afterward."
        }
        .into());
    }
    Ok(lock)
}

pub(super) fn lock_installation(layout: &Layout) -> Result<std::fs::File> {
    runtime_lock(layout, true)
}

pub(super) fn run(layout: &Layout, args: &[String]) -> Result<()> {
    if args == ["--help"] || args == ["-h"] {
        println!("{HELP}");
        return Ok(());
    }
    let options = options(args)?;
    let payload = layout.runtime.join("data-browser");
    let build: Build = fs::read(payload.join("build.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or("The data browser payload is missing. Reinstall the complete Linux package.")?;
    if build.id.len() != 64 || !build.id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("The data browser build identity is invalid.".into());
    }
    let node = detect(layout, build.minimum_node_major)?;
    let running = runtime_lock(layout, false)?;
    let cache = layout
        .data
        .join("data-browser")
        .join(build.id)
        .join(format!("{}-{}-{}", node.platform, node.arch, node.modules));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&cache)
        .map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(cache.join(".setup.lock"))
        .map_err(|e| e.to_string())?;
    // The kernel releases this lock if setup is interrupted or its parent exits.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        println!("Waiting for another data browser dependency setup…");
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    let command = || {
        let mut command = Command::new(&node.executable);
        command
            .arg(payload.join("launch.mjs"))
            .env("FAIRWAY_DATA_BROWSER_CACHE", &cache)
            .env("FAIRWAY_DATA_BROWSER_STATIC", payload.join("dist"))
            .env("FAIRWAY_DATA_BROWSER_PORT", options.port.to_string());
        command
    };
    let mut setup = command();
    setup.arg("--prepare");
    let lock_fd = lock.as_raw_fd();
    // Keep the lock alive in the setup process too if the CLI is force-killed.
    unsafe {
        setup.pre_exec(move || {
            if libc::fcntl(lock_fd, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let status = setup.status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Data browser dependencies are not ready. Setup can be retried without changing your databases.".into());
    }
    drop(lock);
    if options.prepare {
        println!(
            "Data browser dependencies are ready for Node v{}.",
            node.version
        );
        return Ok(());
    }
    // Replace the CLI so signals, exit status and the terminal belong to Node.
    let mut server = command();
    let running_fd = running.as_raw_fd();
    unsafe {
        server.pre_exec(move || {
            if libc::fcntl(running_fd, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Err(server.exec().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_browser_blocks_removal_and_removal_blocks_new_browsers() {
        let root =
            std::env::temp_dir().join(format!("fairway-browser-lock-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        let browser = runtime_lock(&layout, false).unwrap();
        assert!(lock_installation(&layout).unwrap_err().contains("Ctrl+C"));
        drop(browser);
        let removal = lock_installation(&layout).unwrap();
        assert!(runtime_lock(&layout, false).is_err());
        drop(removal);
        assert!(runtime_lock(&layout, false).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }
    #[test]
    fn browser_options_reject_public_hosts_privileged_ports_and_ambiguous_arguments() {
        assert_eq!(
            options(&[]).unwrap(),
            Options {
                port: 8803,
                prepare: false
            }
        );
        assert_eq!(
            options(&args(&["--port", "4567", "--prepare"])).unwrap(),
            Options {
                port: 4567,
                prepare: true
            }
        );
        for values in [
            vec!["--host", "0.0.0.0"],
            vec!["--port"],
            vec!["--port", "80"],
            vec!["--port", "65536"],
            vec!["--port", "8803", "--port", "8804"],
            vec!["--prepare", "--prepare"],
        ] {
            assert!(options(&args(&values)).is_err());
        }
        for value in ["../escape", "/tmp", "", "a:b"] {
            assert!(!safe_component(value));
        }
    }
}
