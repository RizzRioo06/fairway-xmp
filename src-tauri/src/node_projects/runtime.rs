#[cfg(any(target_os = "macos", test))]
use super::revision;
use super::{ProjectRuntime, RuntimeChoice, VersionHints};
use crate::config::Layout;
use std::path::Path;

#[derive(Default)]
pub(crate) struct Inventory {
    pub choices: Vec<RuntimeChoice>,
    pub preferred_id: Option<String>,
    pub error: Option<String>,
}

pub(super) fn exact_version(value: &str) -> Option<String> {
    let value = value.trim().strip_prefix('v').unwrap_or(value.trim());
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty() || part.len() > 5 || !part.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return None;
    }
    Some(format!(
        "v{}",
        parts
            .iter()
            .map(|part| part.parse::<u32>().map(|number| number.to_string()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .ok()?
            .join(".")
    ))
}

pub(super) fn available(runtime: &ProjectRuntime) -> bool {
    let path = Path::new(&runtime.node_path);
    if !path.is_absolute() || exact_version(&runtime.version).as_deref() != Some(&runtime.version) {
        return false;
    }
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return false;
        }
    }
    true
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn choice(node_path: String, version: String, source: String) -> Option<RuntimeChoice> {
    let path = std::fs::canonicalize(node_path).ok()?;
    let runtime = ProjectRuntime {
        node_path: path.to_str()?.into(),
        version: exact_version(&version)?,
    };
    if !available(&runtime) {
        return None;
    }
    let id = revision(&serde_json::to_vec(&runtime).ok()?);
    Some(RuntimeChoice {
        id,
        runtime,
        source,
    })
}

#[cfg(target_os = "macos")]
pub(super) fn discover(layout: &Layout) -> Inventory {
    // Use the existing fnm/nvm/Homebrew inventory. Never call fnm use/default,
    // activation, install, or shell_init for a project registration.
    let status = match crate::node_tools::status(layout) {
        Ok(status) => status,
        Err(error) => {
            return Inventory {
                error: Some(error),
                ..Default::default()
            }
        }
    };
    let preferred = if status.enabled {
        status.managed.as_ref().or(status.detected.as_ref())
    } else {
        status.detected.as_ref()
    };
    let preferred = preferred
        .and_then(|node| choice(node.path.clone(), node.version.clone(), String::new()))
        .map(|node| node.id);
    let mut choices: Vec<_> = status
        .installations
        .into_iter()
        .filter_map(|node| choice(node.node.path, node.node.version, node.source))
        .collect();
    choices.sort_by(|a, b| a.id.cmp(&b.id));
    choices.dedup_by(|a, b| a.id == b.id);
    Inventory {
        choices,
        preferred_id: preferred,
        error: status.detection_error.or(status.fnm_error),
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn discover(_: &Layout) -> Inventory {
    Inventory::default()
}

pub(crate) fn discover_planning(layout: &Layout) -> Inventory {
    #[cfg(target_os = "macos")]
    {
        let mut choices: Vec<_> = crate::node_tools::planning_inventory(layout)
            .into_iter()
            .filter_map(|node| choice(node.node.path, node.node.version, node.source))
            .collect();
        choices.sort_by(|a, b| a.id.cmp(&b.id));
        choices.dedup_by(|a, b| a.id == b.id);
        Inventory {
            choices,
            ..Default::default()
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = layout;
        Inventory::default()
    }
}

pub(super) fn suggestion(hints: &VersionHints, inventory: &Inventory) -> Option<String> {
    let values: Vec<_> = [&hints.node_version, &hints.nvmrc]
        .into_iter()
        .filter_map(|value| value.as_ref())
        .collect();
    if values.is_empty() {
        return inventory
            .preferred_id
            .clone()
            .filter(|id| inventory.choices.iter().any(|choice| &choice.id == id));
    }
    let versions = values
        .iter()
        .map(|value| exact_version(value))
        .collect::<Option<Vec<_>>>()?;
    if versions.iter().any(|version| version != &versions[0]) {
        return None;
    }
    inventory
        .choices
        .iter()
        .find(|choice| choice.runtime.version == versions[0])
        .map(|choice| choice.id.clone())
}
