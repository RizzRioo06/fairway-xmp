//! Desktop fnm adapter. The bundled executable remains the ordinary upstream CLI.
use crate::{
    command_output::output,
    config::{atomic_write, Layout, Result},
};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::time::{Duration, Instant};
use std::{fs, path::PathBuf, process::Command};

mod jobs;
pub use jobs::{Installation, Installations};

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
    pub installation: Option<Installation>,
    pub external_fnm: Option<String>,
    pub read_only_reason: Option<String>,
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
    crate::fnm_setup::inspect(layout).directory
}

/// Keep a previously selected Fairway store stable until explicit handover.
fn activation_directory(layout: &Layout) -> PathBuf {
    let directory = fs::read(layout.data.join("fnm-store.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PathBuf>(&bytes).ok())
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| directory(layout));
    #[cfg(windows)]
    let directory = PathBuf::from(windows_scripts::fnm_directory(&directory.to_string_lossy()));
    directory
}

pub fn enabled(layout: &Layout) -> bool {
    layout.data.join("fnm-enabled").is_file()
}

fn command_in(layout: &Layout, directory: &std::path::Path) -> Command {
    let mut cmd = crate::process::command(executable(layout));
    #[cfg(windows)]
    let directory = windows_scripts::fnm_directory(&directory.to_string_lossy());
    cmd.env("HOME", &layout.home)
        .env("FNM_DIR", directory)
        .env("NO_COLOR", "1")
        .env_remove("FNM_MULTISHELL_PATH")
        .current_dir(&layout.home);
    cmd
}

#[cfg(windows)]
fn command(layout: &Layout) -> Command {
    command_in(layout, &activation_directory(layout))
}

fn run_in(layout: &Layout, directory: &std::path::Path, args: &[&str]) -> Result<String> {
    let mut cmd = command_in(layout, directory);
    cmd.args(args);
    output(cmd, 30, &|| false)
}

#[cfg(windows)]
fn run(layout: &Layout, args: &[&str]) -> Result<String> {
    run_in(layout, &activation_directory(layout), args)
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
fn installation_source(store: &std::path::Path, path: &str) -> &'static str {
    let store = fs::canonicalize(store).unwrap_or_else(|_| store.to_path_buf());
    if std::path::Path::new(path).starts_with(store) {
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
fn inventory(
    layout: &Layout,
    detected: Option<&NodeBinary>,
    store: &std::path::Path,
) -> Vec<NodeInstallation> {
    inventory_using(layout, detected, store, probe)
}

/// Reuse the inventory scanner, without a login shell or fnm invocation.
#[cfg(target_os = "macos")]
pub(crate) fn planning_inventory(layout: &Layout) -> Vec<NodeInstallation> {
    let context = crate::fnm_setup::inspect_process_environment(layout);
    inventory_using(layout, None, &context.directory, |path| {
        let mut command = crate::process::command(path);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HOME", &layout.home)
            .current_dir(&layout.home)
            .args([
                "-p",
                "JSON.stringify({version:process.version,path:process.execPath})",
            ]);
        serde_json::from_str(&output(command, 5, &|| false).ok()?).ok()
    })
}

#[cfg(unix)]
fn inventory_using(
    layout: &Layout,
    detected: Option<&NodeBinary>,
    store: &std::path::Path,
    mut probe: impl FnMut(PathBuf) -> Option<NodeBinary>,
) -> Vec<NodeInstallation> {
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
            installation_source(store, &node.path),
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
        children(store.join("node-versions"))
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
                installation_source(store, &node.path)
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
    let context = crate::fnm_setup::inspect(layout);
    enabled(layout)
        && valid_version(version)
        && run_in(layout, &context.directory, &["default"])
            .ok()
            .is_some_and(|default| same_version(&default, version))
}

fn same_version(left: &str, right: &str) -> bool {
    left.trim_start_matches('v') == right.trim_start_matches('v')
}

pub fn status(layout: &Layout) -> Result<NodeStatus> {
    let context = crate::fnm_setup::inspect(layout);
    #[cfg(unix)]
    let detected = detected(layout);
    #[cfg(windows)]
    let detected = windows::detected(layout);
    status_in(layout, context, detected)
}

fn status_in(
    layout: &Layout,
    context: crate::fnm_setup::Context,
    detected: Result<Option<NodeBinary>>,
) -> Result<NodeStatus> {
    let read_only_reason = crate::fnm_setup::read_only_reason(layout, &context);
    #[cfg(windows)]
    use windows::inventory;
    let installations = inventory(
        layout,
        detected.as_ref().ok().and_then(|node| node.as_ref()),
        &context.directory,
    );
    let fnm = if context.external.is_some() {
        Ok(String::new())
    } else if !executable(layout).is_file() {
        Err("The bundled fnm is missing. Reinstall the complete app.".into())
    } else if read_only_reason.is_some() {
        Ok(String::new())
    } else {
        run_in(layout, &context.directory, &["--version"])
    };
    let fnm_error = fnm.as_ref().err().cloned();
    // Inspect files directly. Even `fnm list` can initialize a previously empty
    // store, which must not make a read-only status request claim ownership.
    let mut installed: Vec<String> = fs::read_dir(context.directory.join("node-versions"))
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let version = entry.file_name().to_string_lossy().into_owned();
            (valid_version(&version)
                && entry
                    .path()
                    .join("installation")
                    .join(node_relative())
                    .is_file())
            .then_some(version)
        })
        .collect();
    installed.sort_by_key(|version| {
        std::cmp::Reverse(
            version
                .trim_start_matches('v')
                .split('.')
                .filter_map(|part| part.parse::<u32>().ok())
                .collect::<Vec<_>>(),
        )
    });
    let managed_store = if enabled(layout) {
        activation_directory(layout)
    } else {
        context.directory.clone()
    };
    let managed = probe(managed_store.join("aliases/default").join(node_relative()));
    let default_version = managed.as_ref().map(|node| node.version.clone());
    Ok(NodeStatus {
        installation: None,
        external_fnm: context.external.map(|path| path.to_string_lossy().into()),
        read_only_reason,
        fnm_path: executable(layout).to_string_lossy().into(),
        fnm_version: fnm.ok().filter(|version| !version.is_empty()),
        fnm_error,
        directory: context.directory.to_string_lossy().into(),
        detection_error: detected.as_ref().err().cloned(),
        detected: detected.unwrap_or(None),
        installations,
        managed,
        installed,
        default_version,
        enabled: enabled(layout),
    })
}

pub fn remote(layout: &Layout) -> Result<Vec<String>> {
    let context = crate::fnm_setup::inspect(layout);
    crate::fnm_setup::require_management(layout, &context)?;
    Ok(latest_per_major(&versions(&run_in(
        layout,
        &layout.data.join("fnm-release-query"),
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
            // Restoring our prior marker is not a new activation request. In
            // particular, handover rollback must work after external fnm appears.
            let rollback = if previously_enabled {
                atomic_write(&layout.data.join("fnm-enabled"), b"1")
            } else {
                change(layout, "disable", "", &|| false)
            }
            .and_then(|()| crate::integration::install(layout));
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
    crate::fnm_setup::require_management(layout, &crate::fnm_setup::inspect(layout))?;
    if !valid_version(version) {
        return Err("Choose an installed Node version.".into());
    }
    if is_enabled_default(layout, version) {
        activation(layout, "disable")?;
    }
    Ok(())
}

/// Select and activate under one desktop operation. A failed activation must
/// restore the shared fnm default as well as the activation preference.
pub fn use_version(layout: &Layout, version: &str, cancelled: &dyn Fn() -> bool) -> Result<()> {
    select_in(
        layout,
        &crate::fnm_setup::inspect(layout),
        version,
        cancelled,
        &|| activation(layout, "enable"),
    )
}

fn select_in(
    layout: &Layout,
    context: &crate::fnm_setup::Context,
    version: &str,
    cancelled: &dyn Fn() -> bool,
    activate: &dyn Fn() -> Result<()>,
) -> Result<()> {
    crate::fnm_setup::require_management(layout, context)?;
    if cancelled() {
        return Err("Command cancelled.".into());
    }
    let alias = context.directory.join("aliases/default");
    let previous = match fs::symlink_metadata(&alias) {
        Ok(_) => Some(run_in(layout, &context.directory, &["default"])?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    if previous
        .as_ref()
        .is_some_and(|value| !valid_version(value) && value != "system")
    {
        return Err("The existing fnm default could not be preserved. Repair it with fnm before choosing Use.".into());
    }
    let result = change_in(layout, context, "default", version, cancelled).and_then(|()| {
        if cancelled() {
            Err("Command cancelled.".into())
        } else {
            activate()
        }
    });
    if let Err(error) = result {
        // Rollback intentionally ignores cancellation/quit so a cancelled
        // selection cannot leave a different default behind.
        let rollback = if let Some(previous) = previous {
            run_in(layout, &context.directory, &["default", &previous]).map(|_| ())
        } else {
            #[cfg(windows)]
            let removed = fs::remove_dir(&alias);
            #[cfg(unix)]
            let removed = fs::remove_file(&alias);
            match removed {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            }
        };
        return Err(match rollback {
            Ok(()) => error,
            Err(rollback) => {
                format!("{error} The previous fnm default could not be restored: {rollback}")
            }
        });
    }
    Ok(())
}

pub fn change(
    layout: &Layout,
    action: &str,
    version: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    change_in(
        layout,
        &crate::fnm_setup::inspect(layout),
        action,
        version,
        cancelled,
    )
}

fn change_in(
    layout: &Layout,
    context: &crate::fnm_setup::Context,
    action: &str,
    version: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    if action != "disable" {
        crate::fnm_setup::require_management(layout, context)?;
    }
    if action == "enable" || action == "disable" {
        if action == "enable" {
            if probe(
                context
                    .directory
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
        && run_in(layout, &context.directory, &["default"])
            .ok()
            .is_some_and(|default| same_version(&default, version))
    {
        return Err("Choose another default or switch to your existing Node setup before removing this version.".into());
    }
    let installed_binary = context
        .directory
        .join("node-versions")
        .join(format!("v{}", version.trim_start_matches('v')))
        .join("installation")
        .join(node_relative());
    if action == "default" && probe(installed_binary.clone()).is_none() {
        return Err("Install this Node version before choosing it as the default.".into());
    }
    if action == "install" {
        crate::fnm_setup::claim_store(layout, context)?;
    }
    let mut cmd = command_in(layout, &context.directory);
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
    format!("\n# Node managed with fnm; default applies to new shells.\nexport FNM_DIR={}\neval \"$({fnm} env --shell {shell})\"\nif _fairway_node_default=$({fnm} default 2>/dev/null); then\n  {fnm} use --silent-if-unchanged \"$_fairway_node_default\" >/dev/null\nfi\nunset _fairway_node_default\n", quote(&activation_directory(layout).to_string_lossy()))
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
    #[cfg(unix)]
    struct SelectionFixture {
        root: PathBuf,
        layout: Layout,
        context: crate::fnm_setup::Context,
    }
    #[cfg(unix)]
    impl SelectionFixture {
        fn new(previous: bool) -> Self {
            use std::os::unix::fs::{symlink, PermissionsExt};
            let root = std::env::temp_dir()
                .join(format!("fairway-node-selection-{}", rand::random::<u64>()));
            let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
            fs::create_dir_all(&layout.home).unwrap();
            let context = crate::fnm_setup::Context {
                directory: root.join("store"),
                external: None,
                error: None,
            };
            crate::fnm_setup::claim_store(&layout, &context).unwrap();
            fs::create_dir_all(context.directory.join("aliases")).unwrap();
            for version in ["v22.1.0", "v24.1.0"] {
                let path = context
                    .directory
                    .join(format!("node-versions/{version}/installation/bin/node"));
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                let value = serde_json::json!({"version": version, "path": path.to_string_lossy()});
                fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", value)).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            }
            if previous {
                symlink(
                    context.directory.join("node-versions/v22.1.0/installation"),
                    context.directory.join("aliases/default"),
                )
                .unwrap();
            }
            let fnm = executable(&layout);
            fs::create_dir_all(fnm.parent().unwrap()).unwrap();
            fs::write(
                &fnm,
                r#"#!/bin/sh
if [ "$1" = default ]; then
  if [ -z "$2" ]; then
    target=$(/usr/bin/readlink "$FNM_DIR/aliases/default")
    target=${target%/installation}
    printf '%s\n' "${target##*/}"
  else
    /bin/rm -f "$FNM_DIR/aliases/default"
    /bin/ln -s "$FNM_DIR/node-versions/$2/installation" "$FNM_DIR/aliases/default"
    if [ "$2" = v24.1.0 ] && [ -f "$FNM_DIR/fail-default" ]; then exit 1; fi
  fi
fi
"#,
            )
            .unwrap();
            fs::set_permissions(fnm, fs::Permissions::from_mode(0o755)).unwrap();
            Self {
                root,
                layout,
                context,
            }
        }
    }
    #[cfg(unix)]
    impl Drop for SelectionFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    #[test]
    fn failed_use_restores_existing_default_or_absent_alias() {
        for previous in [true, false] {
            let fixture = SelectionFixture::new(previous);
            let alias = fixture.context.directory.join("aliases/default");
            let old = fs::read_link(&alias).ok();
            let error = select_in(
                &fixture.layout,
                &fixture.context,
                "v24.1.0",
                &|| false,
                &|| Err("Shell setup failed.".into()),
            )
            .unwrap_err();
            assert_eq!(error, "Shell setup failed.");
            assert_eq!(fs::read_link(&alias).ok(), old);
            assert!(fixture
                .context
                .directory
                .join("node-versions/v24.1.0/installation/bin/node")
                .exists());
            select_in(
                &fixture.layout,
                &fixture.context,
                "v24.1.0",
                &|| false,
                &|| Ok(()),
            )
            .unwrap();
            assert!(fs::read_link(alias)
                .unwrap()
                .ends_with("node-versions/v24.1.0/installation"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn failed_default_command_and_cancelled_selection_keep_prior_default() {
        let fixture = SelectionFixture::new(true);
        let alias = fixture.context.directory.join("aliases/default");
        let old = fs::read_link(&alias).unwrap();
        fs::write(fixture.context.directory.join("fail-default"), "1").unwrap();
        assert!(select_in(
            &fixture.layout,
            &fixture.context,
            "v24.1.0",
            &|| false,
            &|| panic!("activation must not run")
        )
        .is_err());
        assert_eq!(fs::read_link(&alias).unwrap(), old);
        let error = select_in(
            &fixture.layout,
            &fixture.context,
            "v24.1.0",
            &|| true,
            &|| panic!("activation must not run"),
        )
        .unwrap_err();
        assert!(error.contains("cancelled"));
        assert_eq!(fs::read_link(alias).unwrap(), old);
    }

    #[cfg(unix)]
    #[test]
    fn cancelling_download_kills_children_before_they_can_finish_writing() {
        let fixture = SelectionFixture::new(true);
        let marker = fixture.root.join("download-finished");
        let mut cmd = crate::process::command("/bin/sh");
        cmd.args([
            "-c",
            &format!(
                "(sleep 0.4; touch {}) & wait",
                crate::integration::shell_quote(&marker.to_string_lossy())
            ),
        ]);
        let start = Instant::now();
        assert!(
            output(cmd, 5, &|| start.elapsed() > Duration::from_millis(50))
                .unwrap_err()
                .contains("cancelled")
        );
        assert!(start.elapsed() < Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(500));
        assert!(!marker.exists());
    }
    #[cfg(unix)]
    #[test]
    fn readonly_status_and_actions_do_not_invoke_fnm_or_change_store() {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("fairway-readonly-fnm-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(layout.runtime.join("fnm/bin")).unwrap();
        fs::create_dir_all(&layout.home).unwrap();
        fs::create_dir_all(&layout.data).unwrap();
        let store = root.join("existing-store");
        fs::create_dir_all(store.join("node-versions/v24.0.0/installation/bin")).unwrap();
        fs::write(
            store.join("node-versions/v24.0.0/installation/bin/node"),
            b"not executable",
        )
        .unwrap();
        let invoked = root.join("fnm-was-invoked");
        fs::write(
            executable(&layout),
            format!(
                "#!/bin/sh\ntouch {}\n",
                crate::integration::shell_quote(&invoked.to_string_lossy())
            ),
        )
        .unwrap();
        fs::set_permissions(executable(&layout), fs::Permissions::from_mode(0o755)).unwrap();
        let context = crate::fnm_setup::Context {
            directory: store,
            external: None,
            error: None,
        };
        let view = status_in(
            &layout,
            crate::fnm_setup::Context {
                directory: context.directory.clone(),
                external: None,
                error: None,
            },
            Ok(None),
        )
        .unwrap();
        assert!(view.read_only_reason.is_some());
        assert_eq!(view.installed, ["v24.0.0"]);
        for action in ["install", "default", "uninstall", "enable"] {
            assert!(change_in(&layout, &context, action, "v24.0.0", &|| false).is_err());
        }
        fs::write(layout.data.join("fnm-enabled"), b"1").unwrap();
        change_in(&layout, &context, "disable", "", &|| false).unwrap();
        assert!(!enabled(&layout));
        assert!(!invoked.exists());
        assert!(context
            .directory
            .join("node-versions/v24.0.0/installation/bin/node")
            .exists());
        assert!(!layout.data.join("fnm-store.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn existing_activation_keeps_its_owned_store_until_explicit_handover() {
        let root =
            std::env::temp_dir().join(format!("fairway-fnm-handover-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        let original = crate::fnm_setup::Context {
            directory: root.join("fairway-store"),
            external: None,
            error: None,
        };
        crate::fnm_setup::claim_store(&layout, &original).unwrap();
        fs::write(layout.data.join("fnm-enabled"), b"1").unwrap();
        let new = crate::fnm_setup::Context {
            directory: root.join("other-store"),
            external: Some(root.join("external-fnm")),
            error: None,
        };
        // Activation may use a CLI-compatible spelling (without Windows' \\?\
        // prefix), but it must still point to the original owned directory.
        assert_eq!(
            fs::canonicalize(activation_directory(&layout)).unwrap(),
            fs::canonicalize(&original.directory).unwrap()
        );
        assert!(change_in(&layout, &new, "enable", "", &|| false).is_err());
        change_in(&layout, &new, "disable", "", &|| false).unwrap();
        assert!(!enabled(&layout));
        assert!(original.directory.is_dir());
        assert!(!new.directory.exists());
        fs::remove_dir_all(root).unwrap();
    }
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
        let view = status_in(
            &layout,
            crate::fnm_setup::Context {
                directory: root.join("store"),
                external: None,
                error: None,
            },
            Ok(None),
        )
        .unwrap();
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
