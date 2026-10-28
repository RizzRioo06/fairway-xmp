use super::{revision, Error, ErrorCode, Inspection, PackageManagerHint, Result, VersionHints};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Read,
    path::Path,
};

pub(super) const MANIFEST_LIMIT: u64 = 1024 * 1024;
const VERSION_LIMIT: u64 = 4096;

/// Open once, without following symlinks or blocking on Unix special files.
/// Read limits also cover a file growing after metadata was inspected.
pub(super) fn read_regular(path: &Path, limit: u64) -> std::io::Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() {
        return Err(std::io::Error::other(
            "Choose a regular file, not a link or special file.",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(std::io::Error::other(format!(
            "File must be regular and at most {limit} bytes."
        )));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::other(format!(
            "File exceeds {limit} bytes."
        )));
    }
    Ok(Some(bytes))
}

fn optional_string(value: Option<&Value>, field: &str) -> Result<Option<String>> {
    value
        .map(|value| {
            value.as_str().map(String::from).ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidManifest,
                    format!("package.json {field} must be a string."),
                )
            })
        })
        .transpose()
}

fn version_file(root: &Path, name: &str, hash_input: &mut Vec<u8>) -> Result<Option<String>> {
    let bytes = read_regular(&root.join(name), VERSION_LIMIT).map_err(|error| {
        Error::new(
            ErrorCode::InvalidManifest,
            format!("Could not inspect {name}: {error}"),
        )
    })?;
    hash_input.extend_from_slice(name.as_bytes());
    let Some(bytes) = bytes else {
        hash_input.extend_from_slice(&0u64.to_le_bytes());
        return Ok(None);
    };
    // Include presence as well as contents so an absent and empty file differ.
    hash_input.extend_from_slice(&(bytes.len() as u64 + 1).to_le_bytes());
    hash_input.extend_from_slice(&bytes);
    let value = String::from_utf8(bytes).map_err(|_| {
        Error::new(
            ErrorCode::InvalidManifest,
            format!("{name} must contain UTF-8 text."),
        )
    })?;
    if value.chars().any(|c| c.is_control() && !c.is_whitespace()) {
        return Err(Error::new(
            ErrorCode::InvalidManifest,
            format!("{name} contains control characters."),
        ));
    }
    Ok(Some(value.trim().to_owned()))
}

pub(super) fn inspect(root: &str) -> Result<Inspection> {
    if root.len() > 8192 || root.chars().any(char::is_control) || !Path::new(root).is_absolute() {
        return Err(Error::new(
            ErrorCode::InvalidProject,
            "Choose the project folder using its full path.",
        ));
    }
    let root = fs::canonicalize(root).map_err(|error| {
        Error::new(
            ErrorCode::InvalidProject,
            format!("The project folder is unavailable: {error}"),
        )
    })?;
    if !root.is_dir() {
        return Err(Error::new(
            ErrorCode::InvalidProject,
            "Choose a project folder.",
        ));
    }
    let canonical = root
        .to_str()
        .ok_or_else(|| {
            Error::new(
                ErrorCode::InvalidProject,
                "The project path must be valid UTF-8.",
            )
        })?
        .to_owned();
    let bytes = read_regular(&root.join("package.json"), MANIFEST_LIMIT)
        .map_err(|error| {
            Error::new(
                ErrorCode::InvalidManifest,
                format!("Could not inspect package.json: {error}"),
            )
        })?
        .ok_or_else(|| {
            Error::new(
                ErrorCode::InvalidManifest,
                "This folder does not contain package.json.",
            )
        })?;
    let json: Value = serde_json::from_slice(&bytes).map_err(|error| {
        Error::new(
            ErrorCode::InvalidManifest,
            format!("package.json is invalid: {error}"),
        )
    })?;
    let object = json.as_object().ok_or_else(|| {
        Error::new(
            ErrorCode::InvalidManifest,
            "package.json must contain a JSON object.",
        )
    })?;
    let name = optional_string(object.get("name"), "name")?
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            root.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    let mut scripts = BTreeMap::new();
    if let Some(value) = object.get("scripts") {
        let object = value.as_object().ok_or_else(|| {
            Error::new(
                ErrorCode::InvalidManifest,
                "package.json scripts must be an object.",
            )
        })?;
        if object.len() > 256 {
            return Err(Error::new(
                ErrorCode::InvalidManifest,
                "Inspect at most 256 package scripts.",
            ));
        }
        for (key, value) in object {
            let value = value.as_str().ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidManifest,
                    "Every package script must be a string.",
                )
            })?;
            if key.is_empty()
                || key.chars().count() > 128
                || key.chars().any(char::is_control)
                || value.len() > 16384
                || value.contains('\0')
            {
                return Err(Error::new(
                    ErrorCode::InvalidManifest,
                    "A package script name or command exceeds the inspection limits.",
                ));
            }
            scripts.insert(key.clone(), value.to_owned());
        }
    }
    let engines_node = match object.get("engines") {
        Some(value) => optional_string(
            value
                .as_object()
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::InvalidManifest,
                        "package.json engines must be an object.",
                    )
                })?
                .get("node"),
            "engines.node",
        )?,
        None => None,
    };
    let mut hash_input = Vec::new();
    hash_input.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    hash_input.extend_from_slice(&bytes);
    let version_hints = VersionHints {
        node_version: version_file(&root, ".node-version", &mut hash_input)?,
        nvmrc: version_file(&root, ".nvmrc", &mut hash_input)?,
        engines_node,
    };
    let declaration = optional_string(object.get("packageManager"), "packageManager")?;
    let mut lockfiles = Vec::new();
    let mut managers = BTreeSet::new();
    for (file, manager) in [
        ("package-lock.json", "npm"),
        ("npm-shrinkwrap.json", "npm"),
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("bun.lock", "bun"),
        ("bun.lockb", "bun"),
    ] {
        if root.join(file).is_file() {
            lockfiles.push(file.to_owned());
            managers.insert(manager);
        }
    }
    hash_input
        .extend_from_slice(&serde_json::to_vec(&lockfiles).expect("serialize lockfile names"));
    let mut warnings = Vec::new();
    if scripts.is_empty() {
        warnings.push("No package scripts were found. Add a script in your editor before registering this project.".into());
    }
    let declared = declaration
        .as_deref()
        .map(|value| value.split('@').next().unwrap_or(""));
    let manager = declared.unwrap_or_else(|| {
        if managers.len() > 1 {
            "unknown"
        } else {
            managers.iter().next().copied().unwrap_or("npm")
        }
    });
    if managers.len() > 1
        || declared.is_some_and(|name| managers.iter().any(|manager| *manager != name))
    {
        warnings.push("Package manager declarations and lockfiles disagree. Verify the intended package manager.".into());
    }
    let npm_supported = manager == "npm" && managers.iter().all(|manager| *manager == "npm");
    if !npm_supported {
        warnings.push("Only npm scripts are planned for the first launcher. This project can be inspected and registered, but its package manager is not yet supported for execution.".into());
    }
    if [&version_hints.node_version, &version_hints.nvmrc]
        .iter()
        .filter_map(|value| value.as_ref())
        .any(|value| super::runtime::exact_version(value).is_none())
    {
        warnings.push("A Node version file contains a range, alias, or non-exact version. Choose an installed runtime explicitly.".into());
    }
    if let (Some(a), Some(b)) = (&version_hints.node_version, &version_hints.nvmrc) {
        if super::runtime::exact_version(a)
            .zip(super::runtime::exact_version(b))
            .is_some_and(|(a, b)| a != b)
        {
            warnings.push(".node-version and .nvmrc request different Node versions.".into());
        }
    }
    Ok(Inspection {
        root: canonical,
        name,
        scripts,
        package_manager: PackageManagerHint {
            name: manager.into(),
            declaration,
            lockfiles,
            npm_supported,
        },
        version_hints,
        manifest_revision: revision(&hash_input),
        warnings,
        runtimes: Vec::new(),
        suggested_runtime_id: None,
        runtime_error: None,
    })
}

pub(super) fn same_root(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (fs::metadata(a), fs::metadata(b)) {
            return a.dev() == b.dev() && a.ino() == b.ino();
        }
    }
    #[cfg(windows)]
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    false
}
