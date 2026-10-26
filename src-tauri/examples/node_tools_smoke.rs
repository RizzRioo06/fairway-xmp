//! Non-interactive fnm integration check; all downloads/profiles stay in scratch.
#[cfg(target_os = "macos")]
fn main() -> Result<(), String> {
    use fairway_core::{config::Layout, integration, node_tools};
    use std::{fs, path::PathBuf, process::Command};
    if std::env::args().nth(1).as_deref() == Some("--inspect") {
        let runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime");
        println!(
            "{}",
            serde_json::to_string_pretty(&node_tools::status(&Layout::new(runtime)?)?)
                .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let root = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(PathBuf::from(
            std::env::args()
                .nth(1)
                .ok_or("Pass a fresh scratch directory")?,
        ));
    if root.exists() {
        return Err("Use a fresh scratch directory.".into());
    }
    let home = root.join("home 日本 with spaces");
    fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    // Never inherit a personal fnm launcher from the caller’s PATH.
    std::env::set_var("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    // Never inherit a personal custom Zsh profile directory into this fixture.
    std::env::set_var("ZDOTDIR", &home);
    // A single-threaded example owns its environment, never the actual user's store.
    std::env::set_var("FNM_DIR", root.join("node versions"));
    std::env::set_var("XDG_STATE_HOME", root.join("state"));
    let runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime");
    let layout = Layout {
        home,
        data: root.join("data"),
        runtime,
        www: root.join("www"),
        bin: root.join("bin"),
    };
    layout.initialize()?;
    fs::write(layout.home.join(".zshrc"), "# keep my settings\n").map_err(|e| e.to_string())?;
    let initial = node_tools::status(&layout)?;
    assert!(initial.fnm_version.as_deref() == Some("fnm 1.39.0"));
    assert!(!initial.enabled && initial.installed.is_empty());
    let version = "v24.14.1";
    node_tools::change(&layout, "install", version, &|| false)?;
    node_tools::change(&layout, "default", version, &|| false)?;
    let installed = node_tools::status(&layout)?;
    // Finder-style detection must honor the same custom Zsh config as terminals.
    let custom = layout.home.join("custom-zsh");
    fs::create_dir_all(&custom).map_err(|e| e.to_string())?;
    let node_bin = node_tools::directory(&layout)
        .join("node-versions")
        .join(version)
        .join("installation/bin");
    let history = custom.join("history");
    fs::write(
        custom.join(".zshrc"),
        format!(
            "export PATH={}:\"$PATH\"\nHISTFILE={}\nSAVEHIST=100\n",
            integration::shell_quote(&node_bin.to_string_lossy()),
            integration::shell_quote(&history.to_string_lossy())
        ),
    )
    .map_err(|e| e.to_string())?;
    let previous_home = std::env::var_os("HOME");
    std::env::set_var("HOME", &layout.home);
    std::env::set_var("ZDOTDIR", &custom);
    let custom_status = node_tools::status(&layout)?;
    assert_eq!(
        custom_status
            .detected
            .as_ref()
            .map(|node| node.version.as_str()),
        Some(version)
    );
    assert_eq!(
        fs::canonicalize(&custom_status.detected.as_ref().unwrap().path).unwrap(),
        fs::canonicalize(node_bin.join("node")).unwrap()
    );
    assert!(!history.exists(), "Detection wrote personal shell history");
    if let Some(previous) = previous_home {
        std::env::set_var("HOME", previous);
    } else {
        std::env::remove_var("HOME");
    }
    std::env::set_var("ZDOTDIR", &layout.home);
    assert_eq!(installed.default_version.as_deref(), Some(version));
    assert_eq!(
        installed.managed.as_ref().map(|n| n.version.as_str()),
        Some(version)
    );
    assert!(node_tools::change(&layout, "default", "--lts", &|| false).is_err());
    node_tools::change(&layout, "enable", "", &|| false)?;
    assert!(node_tools::change(&layout, "uninstall", version, &|| false).is_err());
    integration::install(&layout)?;
    assert!(fs::read_link(layout.bin.join("fnm")).is_ok());
    let profile = fs::read_to_string(layout.home.join(".zshrc")).map_err(|e| e.to_string())?;
    assert!(profile.contains("# keep my settings"));
    assert!(profile.contains("env --shell zsh") && !profile.contains("--use-on-cd"));
    // Prove ordinary fnm commands work, npm/npx are on PATH, project pins are ignored,
    // and selecting a version in one shell does not change another shell's default.
    fs::write(layout.home.join(".node-version"), "v0.0.1\n").map_err(|e| e.to_string())?;
    let script = format!("export PATH={}:\"$PATH\"\n{}\nnode --version\nnpm --version\nnpx --version\nfnm use {}\nnode --version\n", integration::shell_quote(&layout.bin.to_string_lossy()), node_tools::shell_init(&layout, "zsh"), version);
    let result = Command::new("/bin/zsh")
        .args(["-f", "-c", &script])
        .env("HOME", &layout.home)
        .current_dir(&layout.home)
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).into());
    }
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .matches(version)
            .count()
            >= 2
    );
    let other = "v22.14.0";
    node_tools::change(&layout, "install", other, &|| false)?;
    let script = format!(
        "{}\nnode --version\n{} default {}\nnode --version\n",
        node_tools::shell_init(&layout, "zsh"),
        integration::shell_quote(&node_tools::executable(&layout).to_string_lossy()),
        other
    );
    let existing = Command::new("/bin/zsh")
        .args(["-f", "-c", &script])
        .env("HOME", &layout.home)
        .current_dir(&layout.home)
        .output()
        .map_err(|e| e.to_string())?;
    assert!(existing.status.success());
    assert_eq!(
        String::from_utf8_lossy(&existing.stdout)
            .lines()
            .filter(|line| *line == version)
            .count(),
        2
    );
    let script = format!(
        "{}\nnode --version\n",
        node_tools::shell_init(&layout, "zsh")
    );
    let fresh = Command::new("/bin/zsh")
        .args(["-f", "-c", &script])
        .env("HOME", &layout.home)
        .current_dir(&layout.home)
        .output()
        .map_err(|e| e.to_string())?;
    assert!(fresh.status.success());
    assert_eq!(String::from_utf8_lossy(&fresh.stdout).trim(), other);
    node_tools::change(&layout, "default", version, &|| false)?;
    // Both public spellings must disable activation before removing the default.
    assert!(node_tools::change(
        &layout,
        "uninstall",
        version.trim_start_matches('v'),
        &|| false
    )
    .is_err());
    node_tools::prepare_uninstall(&layout, version.trim_start_matches('v'))?;
    assert!(!node_tools::enabled(&layout));
    node_tools::change(&layout, "uninstall", version, &|| false)?;
    assert!(!fs::read_to_string(layout.home.join(".zshrc"))
        .unwrap()
        .contains("env --shell"));
    integration::remove(&layout)?;
    assert_eq!(
        fs::read_to_string(layout.home.join(".zshrc")).unwrap(),
        "# keep my settings\n"
    );
    assert!(node_tools::directory(&layout)
        .join("node-versions")
        .exists());
    node_tools::change(&layout, "uninstall", other, &|| false)?;
    assert!(node_tools::status(&layout)?.installed.is_empty());
    println!("fnm install/default/remove, native CLI, npm/npx, profiles, spaces/Unicode and project-pin isolation passed. Scratch: {}", root.display());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("macOS-only fnm smoke check");
}
