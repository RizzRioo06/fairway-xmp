//! Windows paths and reversible shell startup integration.
use super::windows_scripts::{cmd_init, cmd_path, powershell_init, profile, remove_autorun};
use super::{
    activation_directory as store, command, enabled, executable, output, probe, run, valid_version,
    NodeBinary, NodeInstallation,
};
use crate::config::{atomic_write, Layout, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use winreg::{enums::*, RegKey, RegValue};

fn path_entries() -> Vec<PathBuf> {
    crate::shell_environment::windows_environment()
        .into_iter()
        .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("PATH"))
        .map(|(_, path)| std::env::split_paths(&path).collect())
        .unwrap_or_default()
}
pub(super) fn detected(_: &Layout) -> Result<Option<NodeBinary>> {
    let start = Instant::now();
    for dir in path_entries() {
        if start.elapsed() > Duration::from_secs(8) {
            return Err("Node PATH detection timed out. Refresh to retry.".into());
        }
        if !dir.is_absolute() {
            continue;
        }
        if let Some(node) = probe(dir.join("node.exe")) {
            return Ok(Some(node));
        }
    }
    Ok(None)
}
fn source(directory: &std::path::Path, path: &std::path::Path) -> &'static str {
    let path = path
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .replace('/', "\\")
        .to_lowercase();
    let directory = fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf());
    let directory = directory
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .replace('/', "\\")
        .to_lowercase();
    if std::path::Path::new(&path).starts_with(directory) {
        "fnm"
    } else if std::env::var_os("NVM_HOME")
        .is_some_and(|home| path.starts_with(&home.to_string_lossy().to_lowercase()))
        || path.contains("\\nvm\\")
    {
        "nvm-windows"
    } else if path.contains("\\volta\\") {
        "Volta"
    } else if path.contains("\\scoop\\") {
        "Scoop"
    } else {
        "External"
    }
}
pub(super) fn inventory(
    layout: &Layout,
    detected: Option<&NodeBinary>,
    directory: &std::path::Path,
) -> Vec<NodeInstallation> {
    let mut candidates = Vec::new();
    if let Some(node) = detected {
        candidates.push(PathBuf::from(&node.path));
    }
    candidates.extend(
        path_entries()
            .iter()
            .filter(|dir| dir.is_absolute())
            .map(|dir| dir.join("node.exe")),
    );
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(
            std::env::split_paths(&path)
                .filter(|dir| dir.is_absolute())
                .map(|dir| dir.join("node.exe")),
        );
    }
    for key in ["ProgramFiles", "ProgramFiles(x86)", "NVM_SYMLINK"] {
        if let Some(path) = std::env::var_os(key) {
            candidates.push(if key == "NVM_SYMLINK" {
                PathBuf::from(path).join("node.exe")
            } else {
                PathBuf::from(path).join("nodejs/node.exe")
            });
        }
    }
    candidates.extend([
        layout.home.join("scoop/apps/nodejs/current/node.exe"),
        layout.home.join("scoop/apps/nodejs-lts/current/node.exe"),
        layout.home.join("AppData/Local/Volta/bin/node.exe"),
    ]);
    for (root, managed) in [
        (directory.join("node-versions"), true),
        (
            std::env::var_os("NVM_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| layout.home.join("AppData/Roaming/nvm")),
            false,
        ),
    ] {
        for entry in fs::read_dir(root)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .take(2048)
        {
            if valid_version(&entry.file_name().to_string_lossy()) {
                candidates.push(entry.path().join(if managed {
                    "installation/node.exe"
                } else {
                    "node.exe"
                }));
            }
        }
    }
    let (mut result, mut seen) = (Vec::new(), HashSet::new());
    let start = Instant::now();
    for path in candidates {
        if start.elapsed() > Duration::from_secs(8) || result.len() >= 64 {
            break;
        }
        let Ok(path) = fs::canonicalize(path) else {
            continue;
        };
        if !seen.insert(path.to_string_lossy().to_lowercase()) {
            continue;
        }
        if let Some(node) = probe(path) {
            result.push(NodeInstallation {
                source: source(directory, std::path::Path::new(&node.path)).into(),
                node,
            });
        }
    }
    result
}

pub(super) fn write_launcher(layout: &Layout) -> Result<()> {
    let context = crate::fnm_setup::inspect(layout);
    if context.external.is_some() && !enabled(layout) {
        return crate::fnm_setup::remove_launcher(layout);
    }
    if context.error.is_some() {
        return Ok(());
    }
    fs::create_dir_all(&layout.bin).map_err(|e| e.to_string())?;
    let bytes = fs::read(executable(layout)).map_err(|e| e.to_string())?;
    let path = layout.bin.join("fnm.exe");
    // Skip identical files, including when the existing CLI is running. This is
    // an unchanged executable copy, not a Fairway command wrapper or symlink.
    if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
        atomic_write(&path, &bytes)?;
    }
    crate::fnm_setup::record_launcher(layout)
}

/// Initialize native fnm without evaluating generated batch text in the PTY.
/// A fresh multishell is pinned to the exact default, so old terminals keep theirs.
pub fn terminal_environment(layout: &Layout) -> Result<Vec<(String, String)>> {
    if !enabled(layout) || !executable(layout).is_file() {
        return Ok(Vec::new());
    }
    // Native CLI users may remove the selected version or its default alias.
    // Keep the terminal available so they can repair that setup with fnm.
    let Some(default) = run(layout, &["default"])
        .ok()
        .filter(|value| valid_version(value) || value == "system")
    else {
        return Ok(Vec::new());
    };
    let env: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&run(layout, &["env", "--json"])?).map_err(|e| e.to_string())?;
    let multishell = env
        .get("FNM_MULTISHELL_PATH")
        .ok_or("fnm did not provide a terminal environment.")?
        .clone();
    let mut cmd = command(layout);
    cmd.envs(&env)
        .args(["use", "--silent-if-unchanged", &default]);
    output(cmd, 30, &|| false)?;
    let mut values: Vec<_> = env.into_iter().collect();
    // PATH is assembled by terminal::shell with this entry at the front.
    values.push(("FAIRWAY_NODE_BIN".into(), multishell));
    Ok(values)
}

#[derive(Default, Serialize, Deserialize)]
struct Record {
    profiles: Vec<Profile>,
    autorun_hook: String,
    #[serde(default)]
    autorun_created: bool,
}
#[derive(Serialize, Deserialize)]
struct Profile {
    path: PathBuf,
    created: bool,
}
struct Edit {
    path: PathBuf,
    previous: Option<Vec<u8>>,
    next: Option<Vec<u8>>,
}
fn read(path: &std::path::Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Cannot read {}: {e}", path.display())),
    }
}
fn write(path: &std::path::Path, bytes: Option<&[u8]>) -> Result<()> {
    if let Some(bytes) = bytes {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        atomic_write(path, bytes)
    } else {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
fn save_autorun(key: &RegKey, value: Option<&RegValue>) -> Result<()> {
    if let Some(value) = value {
        key.set_raw_value("AutoRun", value)
            .map_err(|e| e.to_string())
    } else {
        match key.delete_value("AutoRun") {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub fn powershell_startup(layout: &Layout) -> String {
    powershell_init(
        &layout.bin.join("fnm.exe").to_string_lossy(),
        &store(layout).to_string_lossy(),
        &layout.data.join("fnm-enabled").to_string_lossy(),
    )
}

pub fn cmd_startup(layout: &Layout) -> Result<String> {
    cmd_init(
        &layout.bin.join("fnm.exe").to_string_lossy(),
        &store(layout).to_string_lossy(),
        &layout.data.join("fnm-enabled").to_string_lossy(),
    )
}

pub fn install_shells(layout: &Layout) -> Result<()> {
    update_shells(layout, enabled(layout))
}
pub fn remove_shells(layout: &Layout) -> Result<()> {
    update_shells(layout, false)
}
fn update_shells(layout: &Layout, active: bool) -> Result<()> {
    if !active && !layout.data.join("node-shells.json").exists() {
        return Ok(());
    }
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey("Software\\Microsoft\\Command Processor")
        .map_err(|e| e.to_string())?;
    let documents = dirs::document_dir().unwrap_or_else(|| layout.home.join("Documents"));
    update_shells_at(layout, active, &key, &documents)
}
fn update_shells_at(
    layout: &Layout,
    active: bool,
    key: &RegKey,
    documents: &std::path::Path,
) -> Result<()> {
    let record_path = layout.data.join("node-shells.json");
    let previous_record = read(&record_path)?;
    let old: Record = previous_record
        .as_ref()
        .map(|bytes| serde_json::from_slice(bytes).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default();
    if !active && previous_record.is_none() {
        return Ok(());
    }
    let previous_autorun = match key.get_raw_value("AutoRun") {
        Ok(value) => Some(value),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    if previous_autorun
        .as_ref()
        .is_some_and(|v| ![REG_SZ, REG_EXPAND_SZ].contains(&v.vtype))
    {
        return Err(
            "Command Prompt AutoRun has an unsupported registry type; it was not changed.".into(),
        );
    }
    let old_autorun = previous_autorun
        .as_ref()
        .map(|v| {
            String::from_utf16(
                &v.bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| u16::from_le_bytes([p[0], p[1]]))
                    .take_while(|w| *w != 0)
                    .collect::<Vec<_>>(),
            )
            .map_err(|e| e.to_string())
        })
        .transpose()?
        .unwrap_or_default();
    let mut autorun = remove_autorun(&old_autorun, &old.autorun_hook);
    let startup = layout.data.join("node-init.cmd");
    let mut edits = Vec::new();
    let mut record = Record {
        autorun_created: previous_autorun.is_none() || old.autorun_created,
        ..Record::default()
    };
    let mut paths: Vec<PathBuf> = old.profiles.iter().map(|p| p.path.clone()).collect();
    if active {
        paths.extend([
            documents.join("WindowsPowerShell/Microsoft.PowerShell_profile.ps1"),
            documents.join("PowerShell/Microsoft.PowerShell_profile.ps1"),
        ]);
    }
    // Dotfile managers may symlink a profile file to a separate checkout.
    // Edit its target rather than replacing that link with a regular file.
    paths = paths
        .into_iter()
        .map(|path| {
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
                fs::canonicalize(path).map_err(|e| e.to_string())
            } else {
                Ok(path)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    paths.sort();
    paths.dedup();
    let init = powershell_startup(layout);
    for path in paths {
        let previous = read(&path)?;
        let created = old
            .profiles
            .iter()
            .find(|p| p.path == path)
            .map_or(previous.is_none(), |p| p.created);
        let next = profile(
            previous.as_deref().unwrap_or_default(),
            active.then_some(init.as_str()),
        )?;
        let empty = next.is_empty() || next == [0xef, 0xbb, 0xbf];
        edits.push(Edit {
            path: path.clone(),
            previous,
            next: if !active && created && empty {
                None
            } else {
                Some(next)
            },
        });
        record.profiles.push(Profile { path, created });
    }
    if active {
        let script = cmd_startup(layout)?;
        edits.push(Edit {
            path: startup.clone(),
            previous: read(&startup)?,
            next: Some(script.into_bytes()),
        });
        record.autorun_hook = format!(
            "{}call {}",
            if autorun.trim().is_empty() { "" } else { " & " },
            cmd_path(&startup.to_string_lossy())?
        );
        autorun.push_str(&record.autorun_hook);
    } else {
        edits.push(Edit {
            path: startup.clone(),
            previous: read(&startup)?,
            next: None,
        });
    }
    edits.push(Edit {
        path: record_path,
        previous: previous_record,
        next: if active {
            Some(serde_json::to_vec(&record).map_err(|e| e.to_string())?)
        } else {
            None
        },
    });
    let next_autorun = if autorun.is_empty() && (previous_autorun.is_none() || old.autorun_created)
    {
        None
    } else {
        Some(RegValue {
            bytes: autorun
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect(),
            vtype: previous_autorun
                .as_ref()
                .map_or(REG_SZ, |v| v.vtype.clone()),
        })
    };
    // Validate every profile before writing; roll back both files and registry
    // if any write fails, rather than leaving half-enabled startup hooks.
    let operation = (|| {
        for edit in &edits {
            write(&edit.path, edit.next.as_deref())?;
        }
        save_autorun(key, next_autorun.as_ref())
    })();
    if let Err(error) = operation {
        let mut failures = Vec::new();
        for edit in edits.iter().rev() {
            if let Err(error) = write(&edit.path, edit.previous.as_deref()) {
                failures.push(error);
            }
        }
        if let Err(error) = save_autorun(key, previous_autorun.as_ref()) {
            failures.push(error);
        }
        return Err(if failures.is_empty() {
            error
        } else {
            format!(
                "{error} Could not fully restore the previous startup setup: {}",
                failures.join("; ")
            )
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        layout: Layout,
        key: RegKey,
        registry_path: String,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let name = format!("fnm-{}-{}", std::process::id(), rand::random::<u64>());
            let root = std::env::temp_dir().join(format!("{name} 日本 & tools"));
            let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
            fs::create_dir_all(&layout.data).unwrap();
            let registry_path = format!("Software\\FairwayXMPTests\\{name}");
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey(&registry_path)
                .unwrap();
            Self {
                layout,
                key,
                registry_path,
                root,
            }
        }
        fn documents(&self) -> PathBuf {
            self.root.join("Documents 日本")
        }
        fn update(&self, active: bool) -> Result<()> {
            update_shells_at(&self.layout, active, &self.key, &self.documents())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.registry_path);
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn native_startup_transaction_preserves_existing_setup_and_later_edits() {
        let fixture = Fixture::new();
        fixture
            .key
            .set_value("AutoRun", &"echo user startup")
            .unwrap();
        let path = fixture
            .documents()
            .join("PowerShell/Microsoft.PowerShell_profile.ps1");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"# existing profile\r\n").unwrap();
        fixture.update(true).unwrap();
        let installed = fs::read(&path).unwrap();
        fixture.update(true).unwrap();
        assert_eq!(fs::read(&path).unwrap(), installed);
        let mut edited = installed;
        edited.extend_from_slice(b"# added after Fairway\r\n");
        fs::write(&path, edited).unwrap();
        let autorun: String = fixture.key.get_value("AutoRun").unwrap();
        assert!(autorun.starts_with("echo user startup & call "));
        fixture
            .key
            .set_value("AutoRun", &format!("{autorun} & echo later"))
            .unwrap();
        fixture.update(false).unwrap();
        assert_eq!(
            fixture.key.get_value::<String, _>("AutoRun").unwrap(),
            "echo user startup & echo later"
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            b"# existing profile\r\n# added after Fairway\r\n"
        );
        assert!(!fixture
            .documents()
            .join("WindowsPowerShell/Microsoft.PowerShell_profile.ps1")
            .exists());
        assert!(!fixture.layout.data.join("node-shells.json").exists());
        assert!(!fixture.layout.data.join("node-init.cmd").exists());
    }
    #[test]
    fn invalid_profile_aborts_before_mutation_and_new_autorun_is_removed() {
        let fixture = Fixture::new();
        let path = fixture
            .documents()
            .join("WindowsPowerShell/Microsoft.PowerShell_profile.ps1");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, [0x80]).unwrap();
        assert!(fixture.update(true).is_err());
        assert!(fixture.key.get_value::<String, _>("AutoRun").is_err());
        assert!(!fixture.layout.data.join("node-init.cmd").exists());
        fs::remove_file(path).unwrap();
        fixture.update(true).unwrap();
        fixture.update(false).unwrap();
        assert!(fixture.key.get_raw_value("AutoRun").is_err());
    }
    #[test]
    fn canonical_store_records_are_converted_before_cli_and_shell_activation() {
        let fixture = Fixture::new();
        let canonical = fs::canonicalize(&fixture.root).unwrap();
        assert!(canonical.to_string_lossy().starts_with(r"\\?\"));
        fs::write(
            fixture.layout.data.join("fnm-store.json"),
            serde_json::to_vec(&canonical).unwrap(),
        )
        .unwrap();
        let directory = store(&fixture.layout);
        assert!(!directory.to_string_lossy().starts_with(r"\\?\"));
        assert_eq!(fs::canonicalize(&directory).unwrap(), canonical);
        let command = crate::node_tools::command_in(&fixture.layout, &canonical);
        let value = command
            .get_envs()
            .find(|(name, _)| *name == "FNM_DIR")
            .unwrap()
            .1
            .unwrap();
        assert_eq!(std::path::Path::new(value), directory);
        assert!(cmd_startup(&fixture.layout)
            .unwrap()
            .contains(&format!("set \"FNM_DIR={}\"", directory.display())));
        assert_eq!(
            powershell_startup(&fixture.layout),
            powershell_init(
                &fixture.layout.bin.join("fnm.exe").to_string_lossy(),
                &directory.to_string_lossy(),
                &fixture.layout.data.join("fnm-enabled").to_string_lossy(),
            )
        );
    }
    #[test]
    fn cmd_startup_reads_unicode_marker_before_running_native_tools() {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE;
        let fixture = Fixture::new();
        let windows = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| "C:\\Windows".into());
        let marker = fixture.layout.data.join("fnm-enabled");
        fs::write(&marker, "1").unwrap();
        // WHERE is an ordinary EXE that rejects fnm's arguments, taking the
        // script's error/cleanup path without requiring a staged fnm or Node.
        // Only an existing, correctly decoded marker can reach the final echo.
        let script = cmd_init(
            &windows.join("System32").join("where.exe").to_string_lossy(),
            &fixture.root.join("node store").to_string_lossy(),
            &marker.to_string_lossy(),
        )
        .unwrap();
        fs::write(
            fixture.layout.data.join("probe.cmd"),
            format!("{script}\r\necho FAIRWAY_MARKER_READ\r\nexit /b 0\r\n"),
        )
        .unwrap();
        let launch = || {
            // CMD parses its own executable name before processing /C. Unlike
            // CreateProcessW, it rejects forward slashes in that name.
            let mut command = crate::process::command(windows.join("System32").join("cmd.exe"));
            command
                .creation_flags(CREATE_NEW_CONSOLE)
                .current_dir(&fixture.layout.data)
                .args(["/D", "/V:OFF", "/S", "/C", "probe.cmd"]);
            output(command, 10, &|| false).unwrap()
        };
        assert!(launch().contains("FAIRWAY_MARKER_READ"));
        fs::remove_file(marker).unwrap();
        assert!(!launch().contains("FAIRWAY_MARKER_READ"));
    }
    #[test]
    fn command_timeout_kills_a_windows_process_without_pipe_hangs() {
        let root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| "C:\\Windows".into());
        let mut cmd = crate::process::command(root.join("System32").join("cmd.exe"));
        cmd.args(["/D", "/C", "ping -n 10 127.0.0.1 >nul"]);
        let start = Instant::now();
        assert!(output(cmd, 0, &|| false).is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
