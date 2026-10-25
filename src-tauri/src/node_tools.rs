//! Desktop fnm adapter. The bundled executable remains the ordinary upstream CLI.
use crate::{
    command_output::output,
    config::{atomic_write, Layout, Result},
};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::time::{Duration, Instant};
use std::{fs, path::PathBuf, process::Command};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeBinary {
    pub version: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct NodeInstallation {
    #[serde(flatten)]
    pub node: NodeBinary,
    pub source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    pub fnm_path: String,
    pub fnm_version: Option<String>,
    pub fnm_error: Option<String>,
    pub directory: String,
    pub detected: Option<NodeBinary>,
    pub installations: Vec<NodeInstallation>,
    pub detection_error: Option<String>,
    pub managed: Option<NodeBinary>,
    pub installed: Vec<String>,
    pub default_version: Option<String>,
    pub enabled: bool,
}

#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
mod windows_scripts;

pub fn executable(layout: &Layout) -> PathBuf {
    layout.runtime.join(if cfg!(windows) {
        "fnm/bin/fnm.exe"
    } else {
        "fnm/bin/fnm"
    })
}

fn node_relative() -> &'static str {
    if cfg!(windows) {
        "node.exe"
    } else {
        "bin/node"
    }
}

pub fn directory(layout: &Layout) -> PathBuf {
    if let Some(path) = std::env::var_os("FNM_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return path;
    }
    #[cfg(windows)]
    return windows::directory(layout);
    #[cfg(unix)]
    {
        // Match upstream 1.39's XDG default and its legacy macOS fallbacks.
        let modern = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| layout.home.join(".local/share"))
            .join("fnm");
        if modern.exists() {
            return modern;
        }
        for legacy in [
            layout.home.join(".fnm"),
            layout.home.join("Library/Application Support/fnm"),
        ] {
            if legacy.exists() {
                return legacy;
            }
        }
        modern
    }
}

pub fn enabled(layout: &Layout) -> bool {
    layout.data.join("fnm-enabled").is_file()
}

fn command(layout: &Layout) -> Command {
    let mut cmd = crate::process::command(executable(layout));
    cmd.env("HOME", &layout.home)
        .env("FNM_DIR", directory(layout))
        .env("NO_COLOR", "1")
        .env_remove("FNM_MULTISHELL_PATH")
        .current_dir(&layout.home);
    cmd
}

fn run(layout: &Layout, args: &[&str]) -> Result<String> {
    let mut cmd = command(layout);
    cmd.args(args);
    output(cmd, 30, &|| false)
}

pub fn valid_version(value: &str) -> bool {
    let value = value.strip_prefix('v').unwrap_or(value);
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 5 && part.bytes().all(|b| b.is_ascii_digit())
        })
}

fn versions(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let version = line
                .trim()
                .trim_start_matches('*')
                .split_whitespace()
                .next()?;
            valid_version(version).then(|| version.to_string())
        })
        .collect()
}

fn probe(path: PathBuf) -> Option<NodeBinary> {
    if !path.is_file() {
        return None;
    }
    let mut cmd = crate::process::command(path);
    cmd.args([
        "-p",
        "JSON.stringify({version:process.version,path:process.execPath})",
    ])
    .env_remove("NODE_OPTIONS")
    .env_remove("NODE_PATH");
    serde_json::from_str(&output(cmd, 5, &|| false).ok()?).ok()
}

#[cfg(unix)]
pub(crate) fn configure_shell(cmd: &mut Command, layout: &Layout) {
    let zdot = std::env::var_os("ZDOTDIR").filter(|path| {
        dirs::home_dir().as_ref() == Some(&layout.home)
            && !std::path::Path::new(path).starts_with(layout.data.join("terminal-shell"))
    });
    match zdot {
        Some(path) => {
            cmd.env("ZDOTDIR", path);
        }
        None => {
            cmd.env_remove("ZDOTDIR");
        }
    }
}

#[cfg(unix)]
fn detected(layout: &Layout) -> Result<Option<NodeBinary>> {
    // Finder does not inherit the user's shell PATH. Ask their normal login shell,
    // with a bounded timeout, and ignore any unrelated profile output.
    let mut cmd = crate::process::command("/bin/zsh");
    cmd.args(["-l", "-i", "-c", "unset HISTFILE; SAVEHIST=0; if command -v node >/dev/null 2>&1; then node -p '\"FAIRWAY_NODE=\"+JSON.stringify({version:process.version,path:process.execPath})'; fi"])
        .env("HOME", &layout.home)
        .env_remove("NODE_OPTIONS").env_remove("NODE_PATH")
        .current_dir(&layout.home);
    configure_shell(&mut cmd, layout);
    let text = output(cmd, 8, &|| false)?;
    Ok(parse_detection(&text))
}

#[cfg(unix)]
fn parse_detection(text: &str) -> Option<NodeBinary> {
    // Shell integrations can prepend OSC/control sequences without a newline.
    // Decode the marked JSON object instead of assuming a clean output line.
    text.rmatch_indices("FAIRWAY_NODE=")
        .find_map(|(index, marker)| {
            serde_json::Deserializer::from_str(&text[index + marker.len()..])
                .into_iter::<NodeBinary>()
                .next()?
                .ok()
                .filter(|node| {
                    valid_version(&node.version) && std::path::Path::new(&node.path).is_absolute()
                })
        })
}

#[cfg(unix)]
fn installation_source(layout: &Layout, path: &str) -> &'static str {
    if std::path::Path::new(path).starts_with(directory(layout)) {
        "fnm"
    } else if path.contains("/.nvm/")
        || std::env::var_os("NVM_DIR")
            .is_some_and(|root| std::path::Path::new(path).starts_with(root))
    {
        "nvm"
    } else if path.contains("/Cellar/") || path.starts_with("/opt/homebrew/") {
        "Homebrew"
    } else {
        "External"
    }
}

#[cfg(unix)]
fn inventory(layout: &Layout, detected: Option<&NodeBinary>) -> Vec<NodeInstallation> {
    fn children(path: PathBuf) -> Vec<PathBuf> {
        fs::read_dir(path)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .take(2048)
            .collect()
    }
    let mut candidates = Vec::new();
    if let Some(node) = detected {
        candidates.push((
            PathBuf::from(&node.path),
            installation_source(layout, &node.path),
        ));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(
            std::env::split_paths(&path)
                .filter(|dir| dir.is_absolute())
                .map(|dir| (dir.join("node"), "External")),
        );
    }
    let nvm = std::env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| layout.home.join(".nvm"));
    candidates.extend(
        children(nvm.join("versions/node"))
            .into_iter()
            .map(|path| (path.join("bin/node"), "nvm")),
    );
    candidates.extend(
        children(directory(layout).join("node-versions"))
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| valid_version(&name.to_string_lossy()))
            })
            .map(|path| (path.join("installation/bin/node"), "fnm")),
    );
    let mut prefixes = vec![PathBuf::from("/opt/homebrew"), PathBuf::from("/usr/local")];
    if let Some(prefix) = std::env::var_os("HOMEBREW_PREFIX")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        prefixes.push(prefix);
    }
    for prefix in prefixes {
        candidates.push((prefix.join("bin/node"), "External"));
        for formula in children(prefix.join("opt")).into_iter().filter(|path| {
            path.file_name()
                .is_some_and(|name| name == "node" || name.to_string_lossy().starts_with("node@"))
        }) {
            candidates.push((formula.join("bin/node"), "Homebrew"));
        }
        for formula in children(prefix.join("Cellar")).into_iter().filter(|path| {
            path.file_name()
                .is_some_and(|name| name == "node" || name.to_string_lossy().starts_with("node@"))
        }) {
            candidates.extend(
                children(formula)
                    .into_iter()
                    .map(|path| (path.join("bin/node"), "Homebrew")),
            );
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    let start = Instant::now();
    for (path, source) in candidates {
        if start.elapsed() > Duration::from_secs(8) {
            break;
        }
        let Ok(canonical) = fs::canonicalize(path) else {
            continue;
        };
        if !seen.insert(canonical.clone()) {
            continue;
        }
        if let Some(node) = probe(canonical) {
            let source = if source == "External" {
                installation_source(layout, &node.path)
            } else {
                source
            };
            result.push(NodeInstallation {
                node,
                source: source.into(),
            });
        }
        if result.len() >= 64 {
            break;
        }
    }
    result
}

pub fn is_enabled_default(layout: &Layout, version: &str) -> bool {
    enabled(layout)
        && valid_version(version)
        && run(layout, &["default"])
            .ok()
            .is_some_and(|default| same_version(&default, version))
}

fn same_version(left: &str, right: &str) -> bool {
    left.trim_start_matches('v') == right.trim_start_matches('v')
}

pub fn status(layout: &Layout) -> Result<NodeStatus> {
    #[cfg(unix)]
    let detected = detected(layout);
    #[cfg(windows)]
    let detected = windows::detected(layout);
    #[cfg(windows)]
    use windows::inventory;
    let installations = inventory(
        layout,
        detected.as_ref().ok().and_then(|node| node.as_ref()),
    );
    let fnm = if executable(layout).is_file() {
        run(layout, &["--version"])
    } else {
        Err("The bundled fnm is missing. Reinstall the complete app.".into())
    };
    let available = fnm.is_ok();
    let mut fnm_error = fnm.as_ref().err().cloned();
    let installed = if available {
        match run(layout, &["list"]) {
            Ok(text) => versions(&text),
            Err(error) => {
                fnm_error = Some(error);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let default_version = if available {
        run(layout, &["default"]).ok().filter(|v| valid_version(v))
    } else {
        None
    };
    Ok(NodeStatus {
        fnm_path: executable(layout).to_string_lossy().into(),
        fnm_version: fnm.ok(),
        fnm_error,
        directory: directory(layout).to_string_lossy().into(),
        detection_error: detected.as_ref().err().cloned(),
        detected: detected.unwrap_or(None),
        installations,
        managed: probe(
            directory(layout)
                .join("aliases/default")
                .join(node_relative()),
        ),
        installed,
        default_version,
        enabled: enabled(layout),
    })
}

pub fn remote(layout: &Layout) -> Result<Vec<String>> {
    Ok(latest_per_major(&versions(&run(
        layout,
        &["list-remote", "--sort", "desc"],
    )?)))
}

fn latest_per_major(versions: &[String]) -> Vec<String> {
    let mut latest = std::collections::BTreeMap::new();
    for version in versions {
        if !valid_version(version) {
            continue;
        }
        let numbers: Vec<u32> = version
            .trim_start_matches('v')
            .split('.')
            .filter_map(|part| part.parse().ok())
            .collect();
        let key = (numbers[0], numbers[1], numbers[2]);
        if numbers[0] < 20 {
            continue;
        }
        let entry = latest.entry(numbers[0]).or_insert((key, version.clone()));
        if key > entry.0 {
            *entry = (key, version.clone());
        }
    }
    latest
        .into_values()
        .rev()
        .map(|(_, version)| version)
        .collect()
}

pub fn activation(layout: &Layout, action: &str) -> Result<()> {
    let previously_enabled = layout.data.join("fnm-enabled").is_file();
    change(layout, action, "", &|| false)?;
    if crate::integration::installed(layout) {
        if let Err(error) = crate::integration::install(layout) {
            let restore = if previously_enabled {
                "enable"
            } else {
                "disable"
            };
            let _ = change(layout, restore, "", &|| false);
            let rollback = crate::integration::install(layout);
            return Err(match rollback {
                Ok(()) => error,
                Err(rollback) => {
                    format!("{error} Previous shell setup could not be fully restored: {rollback}")
                }
            });
        }
    }
    Ok(())
}

pub fn prepare_uninstall(layout: &Layout, version: &str) -> Result<()> {
    if !valid_version(version) {
        return Err("Choose an installed Node version.".into());
    }
    if is_enabled_default(layout, version) {
        activation(layout, "disable")?;
    }
    Ok(())
}

pub fn change(
    layout: &Layout,
    action: &str,
    version: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    if action == "enable" || action == "disable" {
        if action == "enable" {
            if probe(
                directory(layout)
                    .join("aliases/default")
                    .join(node_relative()),
            )
            .is_none()
            {
                return Err("Install Node and choose an installed default first.".into());
            }
            atomic_write(&layout.data.join("fnm-enabled"), b"1")?;
        } else {
            match fs::remove_file(layout.data.join("fnm-enabled")) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        return Ok(());
    }
    if !valid_version(version) {
        return Err("Choose an exact Node version from the list.".into());
    }
    if !["install", "default", "uninstall"].contains(&action) {
        return Err("Unsupported Node operation.".into());
    }
    if action == "uninstall"
        && enabled(layout)
        && run(layout, &["default"])
            .ok()
            .is_some_and(|default| same_version(&default, version))
    {
        return Err("Choose another default or switch to your existing Node setup before removing this version.".into());
    }
    let installed_binary = directory(layout)
        .join("node-versions")
        .join(format!("v{}", version.trim_start_matches('v')))
        .join("installation")
        .join(node_relative());
    if action == "default" && probe(installed_binary.clone()).is_none() {
        return Err("Install this Node version before choosing it as the default.".into());
    }
    let mut cmd = command(layout);
    cmd.args([action, version]);
    if action == "install" {
        cmd.args(["--progress", "never"]);
    }
    output(cmd, 600, cancelled)?;
    if action == "install" && probe(installed_binary).is_none() {
        return Err("Node installation did not produce a usable executable. Remove the incomplete version and retry.".into());
    }
    Ok(())
}

/// No directory hooks or project version resolution. Native fnm remains unrestricted.
#[cfg(unix)]
pub fn shell_init(layout: &Layout, shell: &str) -> String {
    if !enabled(layout) || !executable(layout).is_file() {
        return String::new();
    }
    let quote = crate::integration::shell_quote;
    let fnm = quote(&executable(layout).to_string_lossy());
    format!("\n# Node managed with fnm; default applies to new shells.\nexport FNM_DIR={}\neval \"$({fnm} env --shell {shell})\"\nif _fairway_node_default=$({fnm} default 2>/dev/null); then\n  {fnm} use --silent-if-unchanged \"$_fairway_node_default\" >/dev/null\nfi\nunset _fairway_node_default\n", quote(&directory(layout).to_string_lossy()))
}

pub fn write_launcher(layout: &Layout) -> Result<()> {
    if !executable(layout).is_file() {
        return Ok(());
    }
    #[cfg(windows)]
    return windows::write_launcher(layout);
    #[cfg(unix)]
    {
        crate::integration::write_fnm_launcher(layout)
    }
}

#[cfg(windows)]
pub use windows::{
    cmd_startup, install_shells, powershell_startup, remove_shells, terminal_environment,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_comparison_accepts_versions_with_or_without_v() {
        assert!(same_version("v24.14.1", "24.14.1"));
        assert!(!same_version("v24.14.1", "24.14.0"));
    }
    #[cfg(unix)]
    #[test]
    fn broken_fnm_is_reported_without_failing_node_status() {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("fairway-broken-fnm-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(layout.runtime.join("fnm/bin")).unwrap();
        fs::create_dir_all(&layout.home).unwrap();
        fs::write(
            executable(&layout),
            "#!/bin/sh\necho broken-fnm >&2\nexit 1\n",
        )
        .unwrap();
        fs::set_permissions(executable(&layout), fs::Permissions::from_mode(0o755)).unwrap();
        let view = status(&layout).unwrap();
        assert!(view.fnm_version.is_none());
        assert!(view.fnm_error.unwrap().contains("broken-fnm"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(unix)]
    fn detection_accepts_terminal_sequences_and_extra_output() {
        let text = "\x1b]1337;RemoteHost=@host\x07\x1b]1337;CurrentDir=/Users/test\x07FAIRWAY_NODE={\"version\":\"v24.22.1\",\"path\":\"/Users/test/.nvm/versions/node/v24.22.1/bin/node\"}\n\x1b]0;done\x07";
        assert_eq!(parse_detection(text).unwrap().version, "v24.22.1");
        assert!(
            parse_detection("FAIRWAY_NODE={\"version\":\"bad\",\"path\":\"relative\"}").is_none()
        );
    }
    #[test]
    fn picker_returns_latest_numeric_release_for_each_major() {
        let releases = [
            "v18.20.8",
            "v19.9.0",
            "v20.19.0",
            "v21.7.3",
            "v22.1.0",
            "v23.11.0",
            "v25.0.0",
            "v24.9.9",
            "v24.10.0",
            "v22.20.1",
            "v24.10.2",
            "v24.10.2-rc.1",
        ]
        .map(String::from);
        assert_eq!(
            latest_per_major(&releases),
            ["v25.0.0", "v24.10.2", "v23.11.0", "v22.20.1", "v21.7.3", "v20.19.0"]
        );
        assert!(latest_per_major(&[]).is_empty());
        assert!(latest_per_major(&["v18.20.8".into(), "v19.9.0".into()]).is_empty());
    }
    #[test]
    fn cli_output_accepts_only_exact_versions_and_ignores_system() {
        assert_eq!(
            versions("* system\n* v24.14.1 default\nv22.1.0 (Jod)\n"),
            ["v24.14.1", "v22.1.0"]
        );
        for bad in [
            "--lts",
            "../../node",
            "24",
            "24.1",
            "24.1.0;touch /tmp/a",
            "v1.2.3\n",
        ] {
            assert!(!valid_version(bad));
        }
    }
    #[test]
    #[cfg(unix)]
    fn command_timeout_is_bounded() {
        let mut cmd = crate::process::command("/bin/sh");
        cmd.args(["-c", "sleep 10"]);
        let start = Instant::now();
        assert!(output(cmd, 0, &|| false).unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
