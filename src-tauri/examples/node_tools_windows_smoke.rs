//! Non-interactive Windows fnm smoke test. No real profiles or user registry keys
//! are edited; runtimes, markers and CLI copies stay in a fresh scratch folder.
#[cfg(windows)]
fn main() -> Result<(), String> {
    use fairway_core::{config::Layout, node_tools, process};
    use std::{fs, path::PathBuf};
    let root = std::env::current_dir().map_err(|e| e.to_string())?.join(
        std::env::args()
            .nth(1)
            .ok_or("Pass a fresh scratch directory")?,
    );
    if root.exists() {
        return Err("Use a fresh scratch directory.".into());
    }
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let root = root.join("日本 with spaces & tools");
    let layout = Layout::at(
        root.join("home"),
        root.join("data"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime"),
    );
    fs::create_dir_all(&layout.home).map_err(|e| e.to_string())?;
    fs::create_dir_all(&layout.bin).map_err(|e| e.to_string())?;
    std::env::set_var("FNM_DIR", root.join("node store"));
    std::env::set_var("XDG_STATE_HOME", root.join("state"));
    node_tools::write_launcher(&layout)?;
    let mut cli = process::command(layout.bin.join("fnm.exe"));
    cli.arg("--version");
    let version = cli.output().map_err(|e| e.to_string())?;
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("fnm 1.39.0"));
    for version in ["v24.14.1", "v22.14.0"] {
        node_tools::change(&layout, "install", version, &|| false)?;
    }
    node_tools::use_version(&layout, "v24.14.1", &|| false)?;
    // Exercise the actual external Command Prompt startup text, without AutoRun
    // registry edits. It must work with spaces, Unicode and ampersands in paths.
    let startup = layout.data.join("scratch-startup.cmd");
    fs::write(
        &startup,
        format!(
            "{}\r\nnode --version\r\ncall npm --version\r\ncall npx --version\r\n",
            node_tools::cmd_startup(&layout)?
        ),
    )
    .map_err(|e| e.to_string())?;
    let cmd_path = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into())
        .join("System32/cmd.exe");
    let mut shell = process::command(cmd_path);
    use std::os::windows::process::CommandExt;
    shell
        .args(["/D", "/V:OFF", "/S", "/C"])
        .raw_arg(format!("\"\"{}\"\"", startup.display()));
    let startup_output = shell.output().map_err(|e| e.to_string())?;
    assert!(
        startup_output.status.success(),
        "{}",
        String::from_utf8_lossy(&startup_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&startup_output.stdout).contains("v24.14.1"),
        "{}",
        String::from_utf8_lossy(&startup_output.stdout)
    );
    // Run the same managed PowerShell body explicitly, without loading or
    // editing a real profile and without changing the execution policy.
    use base64::Engine;
    let ps_script = format!(
        "{}\r\nnode --version\r\nnpm.cmd --version\r\nnpx.cmd --version\r\n",
        node_tools::powershell_startup(&layout)
    );
    let encoded = base64::engine::general_purpose::STANDARD.encode(
        ps_script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let powershell = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut ps = process::command(powershell);
    let ps_output = ps
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encoded,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    assert!(
        ps_output.status.success(),
        "{}",
        String::from_utf8_lossy(&ps_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&ps_output.stdout).contains("v24.14.1"),
        "{}",
        String::from_utf8_lossy(&ps_output.stdout)
    );
    let first = node_tools::terminal_environment(&layout)?;
    let check = |env: &[(String, String)], expected: &str| -> Result<(), String> {
        let bin = env
            .iter()
            .find(|(key, _)| key == "FAIRWAY_NODE_BIN")
            .ok_or("No Node PATH")?
            .1
            .clone();
        let mut node = process::command(PathBuf::from(&bin).join("node.exe"));
        let output = node
            .args(["--version"])
            .envs(
                env.iter()
                    .filter(|(key, _)| key != "FAIRWAY_NODE_BIN")
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            )
            .output()
            .map_err(|e| e.to_string())?;
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
        assert!(PathBuf::from(&bin).join("npm.cmd").is_file());
        assert!(PathBuf::from(&bin).join("npx.cmd").is_file());
        Ok(())
    };
    check(&first, "v24.14.1")?;
    node_tools::change(&layout, "default", "v22.14.0", &|| false)?;
    let second = node_tools::terminal_environment(&layout)?;
    check(&second, "v22.14.0")?;
    check(&first, "v24.14.1")?;
    assert!(node_tools::change(&layout, "uninstall", "v22.14.0", &|| false).is_err());
    node_tools::prepare_uninstall(&layout, "v22.14.0")?;
    assert!(!node_tools::enabled(&layout));
    assert!(node_tools::terminal_environment(&layout)?.is_empty());
    node_tools::change(&layout, "uninstall", "v22.14.0", &|| false)?;
    node_tools::change(&layout, "uninstall", "v24.14.1", &|| false)?;
    assert!(node_tools::directory(&layout).is_dir());
    assert!(
        !layout.data.join("node-shells.json").exists(),
        "No real shell setup was touched"
    );
    println!("Windows fnm CLI, downloads, Node/npm/npx, per-terminal defaults and uninstall passed in scratch.");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Run this smoke test on Windows.");
}
