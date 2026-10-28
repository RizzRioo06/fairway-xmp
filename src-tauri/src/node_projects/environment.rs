//! Build a new environment; never mutate the app's or the user's shell settings.
use super::{Error, ErrorCode, Result};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

pub type ChildEnvironment = BTreeMap<OsString, OsString>;

pub(super) fn checked_path(path: &Path) -> Result<PathBuf> {
    let invalid = || {
        Error::new(ErrorCode::EnvironmentUnavailable, "Runtime preparation needs absolute UTF-8 paths without control characters or PATH delimiters.")
    };
    if !path.is_absolute() {
        return Err(invalid());
    }
    let text = path.to_str().ok_or_else(invalid)?;
    if text.len() > 8192 || text.chars().any(char::is_control) || text.contains(':') {
        return Err(invalid());
    }
    std::fs::canonicalize(path).map_err(|_| {
        Error::new(
            ErrorCode::EnvironmentUnavailable,
            "A runtime, project, or home path is missing.",
        )
    })
}

pub(super) fn build(
    home: &Path,
    root: &Path,
    node: &Path,
    npm_cli: &Path,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<ChildEnvironment> {
    let home = checked_path(home)?;
    let root = checked_path(root)?;
    let node = checked_path(node)?;
    if !home.is_dir() || !root.is_dir() {
        return Err(Error::new(
            ErrorCode::EnvironmentUnavailable,
            "The home and project paths must be folders.",
        ));
    }
    let bin = node.parent().ok_or_else(|| {
        Error::new(
            ErrorCode::EnvironmentUnavailable,
            "The Node executable has no bin directory.",
        )
    })?;
    // Absolute system tools only. Do not inherit a manager shim, relative PATH
    // component, project directory, NODE_OPTIONS, npm config, or shell hook.
    let mut paths = vec![bin.to_path_buf()];
    for directory in ["/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
        let directory = PathBuf::from(directory);
        if !paths.contains(&directory) {
            paths.push(directory);
        }
    }
    let mut env = ChildEnvironment::new();
    for (key, value) in inherited {
        let name = key.to_str().unwrap_or("");
        if matches!(
            name,
            "USER"
                | "LOGNAME"
                | "LANG"
                | "LC_ALL"
                | "LC_CTYPE"
                | "LC_COLLATE"
                | "LC_MESSAGES"
                | "LC_MONETARY"
                | "LC_NUMERIC"
                | "LC_TIME"
                | "TZ"
        ) && value
            .to_str()
            .is_some_and(|value| value.len() <= 1024 && !value.chars().any(char::is_control))
        {
            env.insert(key, value);
        }
    }
    env.insert(
        "PATH".into(),
        std::env::join_paths(paths).map_err(|_| {
            Error::new(
                ErrorCode::EnvironmentUnavailable,
                "Could not construct an isolated PATH.",
            )
        })?,
    );
    env.insert("HOME".into(), home.into_os_string());
    env.insert("PWD".into(), root.into_os_string());
    env.insert("TMPDIR".into(), "/tmp".into());
    env.insert("SHELL".into(), "/bin/sh".into());
    env.insert("NODE".into(), node.as_os_str().into());
    env.insert("npm_node_execpath".into(), node.into_os_string());
    env.insert("npm_execpath".into(), npm_cli.as_os_str().into());
    // Project .npmrc is still project data, but personal/global configuration
    // and inherited npm settings cannot override this planned tool selection.
    for (key, value) in [
        ("npm_config_userconfig", "/dev/null"),
        // npm rejects loading the exact same config path twice. A child
        // of the null device is also unreadable, cannot contain settings,
        // and has a distinct name; no config files need to be created.
        ("npm_config_globalconfig", "/dev/null/fairway-global.npmrc"),
        ("npm_config_node_options", ""),
        ("npm_config_script_shell", "/bin/sh"),
        ("npm_config_update_notifier", "false"),
        ("npm_config_audit", "false"),
        ("npm_config_fund", "false"),
    ] {
        env.insert(key.into(), value.into());
    }
    Ok(env)
}

pub(super) fn command(node: &Path, cwd: &Path, env: &ChildEnvironment) -> Command {
    let mut command = crate::process::command(node);
    command
        .env_clear()
        .envs(env)
        .current_dir(cwd)
        .env("PWD", cwd);
    command
}
