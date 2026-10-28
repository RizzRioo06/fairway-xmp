//! Stage 2 prepares tools and an environment, but owns no running project.
use super::{
    environment, manifest, revision, Error, ErrorCode, Inspection, ProjectConfig, ProjectRuntime,
    Result,
};
use crate::config::Layout;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

const PROBE: &str = include_str!("runtime_probe.js");
const TOOL_LIMIT: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpmRuntime {
    pub cli_path: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePreparation {
    pub project_id: String,
    pub registry_revision: String,
    pub manifest_revision: String,
    pub runtime: ProjectRuntime,
    pub npm: NpmRuntime,
    /// A receipt for this validation, not authority to execute at a later time.
    pub runtime_revision: String,
}

/// Environment values stay native and in memory; they are never IPC payloads.
pub struct PreparedRuntime {
    summary: RuntimePreparation,
    root: PathBuf,
    env: environment::ChildEnvironment,
}

impl PreparedRuntime {
    pub fn summary(&self) -> &RuntimePreparation {
        &self.summary
    }
    pub fn environment(&self) -> &environment::ChildEnvironment {
        &self.env
    }
    /// Construct an unspawned Node command, with no project script arguments.
    /// Stage 3 must prepare again immediately before creating a managed run.
    pub fn node_command(&self) -> Command {
        environment::command(
            Path::new(&self.summary.runtime.node_path),
            &self.root,
            &self.env,
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    path: PathBuf,
    hash: String,
    identity: String,
}

fn fingerprint(path: &Path) -> Result<Fingerprint> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path).map_err(|_| {
        Error::new(
            ErrorCode::RuntimeChanged,
            "A selected runtime or npm file disappeared.",
        )
    })?;
    let before = file
        .metadata()
        .map_err(|e| Error::new(ErrorCode::RuntimeChanged, e.to_string()))?;
    if !before.is_file() || before.len() > TOOL_LIMIT {
        return Err(Error::new(
            ErrorCode::RuntimeChanged,
            "A runtime tool must be a bounded regular file (at most 256 MiB).",
        ));
    }
    let identity = |metadata: &fs::Metadata| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            format!(
                "{}:{}:{}:{}:{}:{}",
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.mode()
            )
        }
        #[cfg(not(unix))]
        {
            format!("{}:{:?}", metadata.len(), metadata.modified())
        }
    };
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut total = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        let size = file
            .read(&mut buffer)
            .map_err(|e| Error::new(ErrorCode::RuntimeChanged, e.to_string()))?;
        if size == 0 {
            break;
        }
        total += size as u64;
        if total > TOOL_LIMIT {
            return Err(Error::new(
                ErrorCode::RuntimeChanged,
                "The runtime tool grew beyond its size limit.",
            ));
        }
        context.update(&buffer[..size]);
    }
    let after = file
        .metadata()
        .map_err(|e| Error::new(ErrorCode::RuntimeChanged, e.to_string()))?;
    if identity(&before) != identity(&after) || total != after.len() {
        return Err(Error::new(
            ErrorCode::RuntimeChanged,
            "A runtime tool changed while being inspected.",
        ));
    }
    Ok(Fingerprint {
        path: path.into(),
        hash: context
            .finish()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        identity: identity(&after),
    })
}

#[derive(PartialEq, Eq)]
struct NpmPair {
    root: PathBuf,
    cli: PathBuf,
    metadata: PathBuf,
    version: String,
    engine: String,
}

fn npm_error(message: &str) -> Error {
    Error::new(ErrorCode::NpmUnavailable, message)
}

fn npm_pair(node: &Path, project: &Path) -> Result<NpmPair> {
    let bin = node
        .parent()
        .filter(|bin| bin.file_name().is_some_and(|name| name == "bin"))
        .ok_or_else(|| {
            npm_error(
                "Expected an installed Node at bin/node; no paired npm layout could be identified.",
            )
        })?;
    let prefix = bin
        .parent()
        .ok_or_else(|| npm_error("The Node installation prefix is unavailable."))?;
    let anchor = bin.join("npm");
    let mut roots = Vec::new();
    if fs::symlink_metadata(&anchor).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        let cli = fs::canonicalize(&anchor)
            .map_err(|_| npm_error("The selected installation has a broken npm link."))?;
        if cli.file_name().is_none_or(|name| name != "npm-cli.js")
            || cli
                .parent()
                .and_then(Path::file_name)
                .is_none_or(|name| name != "bin")
        {
            return Err(npm_error(
                "The npm link does not identify a paired npm CLI.",
            ));
        }
        roots.push(cli.parent().and_then(Path::parent).unwrap().to_path_buf());
    } else {
        for candidate in [
            prefix.join("lib/node_modules/npm"),
            prefix.join("libexec/lib/node_modules/npm"),
        ] {
            if candidate.exists() {
                roots.push(
                    fs::canonicalize(candidate)
                        .map_err(|_| npm_error("The npm package path is unavailable."))?,
                );
            }
        }
    }
    roots.sort();
    roots.dedup();
    if roots.len() != 1 {
        return Err(npm_error("A unique npm installation paired with this Node could not be found. npm on PATH is never used as a fallback."));
    }
    let root = roots.remove(0);
    if root.starts_with(project) {
        return Err(npm_error(
            "Runtime tools must be installed outside the imported project.",
        ));
    }
    let metadata = root.join("package.json");
    let bytes = manifest::read_regular(&metadata, manifest::MANIFEST_LIMIT)
        .map_err(|_| npm_error("The npm manifest must be a bounded regular file."))?
        .ok_or_else(|| npm_error("The npm manifest is missing."))?;
    let package: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| npm_error("The npm manifest is malformed."))?;
    let version = package["version"]
        .as_str()
        .filter(|v| v.len() < 128)
        .ok_or_else(|| npm_error("The npm version is invalid."))?
        .to_owned();
    if package["name"] != "npm" || package["bin"]["npm"] != "bin/npm-cli.js" {
        return Err(npm_error(
            "The paired package does not declare the standard npm CLI.",
        ));
    }
    let engine = package["engines"]["node"]
        .as_str()
        .filter(|value| value.len() <= 4096)
        .ok_or_else(|| {
            npm_error("The paired npm package does not declare a valid Node requirement.")
        })?
        .to_owned();
    let cli = root.join("bin/npm-cli.js");
    manifest::read_regular(&cli, manifest::MANIFEST_LIMIT)
        .map_err(|_| npm_error("The npm CLI must be a bounded regular file."))?
        .ok_or_else(|| npm_error("The npm CLI is missing."))?;
    Ok(NpmPair {
        root,
        cli,
        metadata,
        version,
        engine,
    })
}

/// npm prepends these directories itself, independently of inherited PATH.
fn reject_project_shims(root: &Path, node: &Path, npm: &Path) -> Result<()> {
    for parent in root.ancestors() {
        for (name, expected) in [("node", node), ("npm", npm)] {
            let path = parent.join("node_modules/.bin").join(name);
            if fs::symlink_metadata(&path).is_ok()
                && fs::canonicalize(&path).ok().as_deref() != Some(expected)
            {
                return Err(Error::new(ErrorCode::EnvironmentUnavailable, "A project or ancestor node_modules/.bin contains a Node/npm shim that would override the selected tools."));
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Requirement {
    source: &'static str,
    kind: &'static str,
    range: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeInput<'a> {
    node_path: &'a str,
    version: &'a str,
    npm_version: &'a str,
    root: &'a str,
    requirements: Vec<Requirement>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProbeResult {
    node_path: String,
    version: String,
    npm_version: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ProbeResponse {
    Success(ProbeResult),
    Failure { error: Error },
}

fn capture(command: Command) -> Result<String> {
    #[cfg(any(target_os = "macos", windows))]
    {
        crate::command_output::output(command, 8, &|| false)
            .map_err(|_| npm_error("The bounded runtime/npm probe failed or timed out."))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = command;
        Err(Error::new(
            ErrorCode::UnsupportedPlatform,
            "Node runtime preparation is currently available on macOS only.",
        ))
    }
}

pub(super) fn prepare(
    layout: &Layout,
    project: &ProjectConfig,
    inspection: &Inspection,
    registry_revision: String,
) -> Result<PreparedRuntime> {
    prepare_with(
        layout,
        project,
        inspection,
        registry_revision,
        std::env::vars_os(),
        capture,
    )
}

fn prepare_with(
    layout: &Layout,
    project: &ProjectConfig,
    inspection: &Inspection,
    registry_revision: String,
    inherited: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
    probe: impl FnMut(Command) -> Result<String>,
) -> Result<PreparedRuntime> {
    prepare_at_with(
        layout,
        project,
        inspection,
        (registry_revision, None),
        inherited,
        probe,
    )
}

/// An uncreated destination has no manifest or working directory. Validate the
/// same tools against catalog constraints using its existing parent as cwd.
pub(crate) fn prepare_creation_runtime(
    layout: &Layout,
    runtime: ProjectRuntime,
    parent: &Path,
    destination: &Path,
    node_range: &str,
    npm_range: &str,
) -> Result<RuntimePreparation> {
    let root = parent
        .to_str()
        .ok_or_else(|| npm_error("The parent path must be UTF-8."))?
        .to_owned();
    let project = ProjectConfig {
        id: String::new(),
        name: String::new(),
        root: root.clone(),
        script: String::new(),
        runtime,
        open_url: None,
    };
    let inspection = Inspection {
        root,
        name: String::new(),
        scripts: Default::default(),
        package_manager: super::PackageManagerHint {
            name: "npm".into(),
            declaration: Some(format!("npm@{npm_range}")),
            lockfiles: Vec::new(),
            npm_supported: true,
        },
        version_hints: super::VersionHints {
            engines_node: Some(node_range.into()),
            ..Default::default()
        },
        manifest_revision: String::new(),
        warnings: Vec::new(),
        runtimes: Vec::new(),
        suggested_runtime_id: None,
        runtime_error: None,
    };
    prepare_at_with(
        layout,
        &project,
        &inspection,
        (String::new(), Some(destination)),
        std::env::vars_os(),
        capture,
    )
    .map(|prepared| prepared.summary)
}

fn prepare_at_with(
    layout: &Layout,
    project: &ProjectConfig,
    inspection: &Inspection,
    context: (String, Option<&Path>),
    inherited: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
    mut probe: impl FnMut(Command) -> Result<String>,
) -> Result<PreparedRuntime> {
    let (registry_revision, uncreated_root) = context;
    if !inspection.package_manager.npm_supported {
        return Err(Error::new(
            ErrorCode::UnsupportedPackageManager,
            "Only npm projects can be prepared in this version.",
        ));
    }
    let root = environment::checked_path(Path::new(&project.root))?;
    if root.to_str() != Some(&project.root) || inspection.root != project.root {
        return Err(Error::new(
            ErrorCode::InvalidProject,
            "The saved project root changed. Inspect and save its new location.",
        ));
    }
    if !super::runtime::available(&project.runtime) {
        return Err(Error::new(
            ErrorCode::RuntimeUnavailable,
            "The saved Node executable is missing, invalid, or not executable.",
        ));
    }
    let node = environment::checked_path(Path::new(&project.runtime.node_path))?;
    if node.to_str() != Some(&project.runtime.node_path) {
        return Err(Error::new(
            ErrorCode::RuntimeChanged,
            "The saved Node path now resolves to a different executable.",
        ));
    }
    let tool_boundary = uncreated_root.unwrap_or(&root);
    if node.starts_with(tool_boundary) {
        return Err(Error::new(
            ErrorCode::RuntimeUnavailable,
            "Select a runtime installed outside the imported project.",
        ));
    }
    let node_before = fingerprint(&node)?;
    let npm = npm_pair(&node, tool_boundary)?;
    let files_before = [fingerprint(&npm.cli)?, fingerprint(&npm.metadata)?];
    reject_project_shims(tool_boundary, &node, &npm.cli)?;
    let env = environment::build(&layout.home, &root, &node, &npm.cli, inherited)?;
    let mut requirements = vec![Requirement {
        source: "npm engines.node",
        kind: "node",
        range: npm.engine.clone(),
    }];
    for (source, value) in [
        (".node-version", &inspection.version_hints.node_version),
        (".nvmrc", &inspection.version_hints.nvmrc),
        (
            "package.json engines.node",
            &inspection.version_hints.engines_node,
        ),
    ] {
        if let Some(range) = value {
            requirements.push(Requirement {
                source,
                kind: "node",
                range: range.clone(),
            });
        }
    }
    if let Some(declaration) = &inspection.package_manager.declaration {
        let range = declaration.strip_prefix("npm@").ok_or_else(|| {
            Error::new(
                ErrorCode::RequirementConflict,
                "packageManager must specify npm with a supported version range.",
            )
        })?;
        requirements.push(Requirement {
            source: "package.json packageManager",
            kind: "npm",
            range: range.into(),
        });
    }
    if requirements.iter().any(|item| item.range.len() > 4096) {
        return Err(Error::new(
            ErrorCode::RequirementConflict,
            "Version requirements must be at most 4 KiB each.",
        ));
    }
    let input = ProbeInput {
        node_path: &project.runtime.node_path,
        version: &project.runtime.version,
        npm_version: &npm.version,
        root: tool_boundary
            .to_str()
            .ok_or_else(|| npm_error("The project boundary must be UTF-8."))?,
        requirements,
    };
    let mut command = environment::command(&node, &npm.root, &env);
    command.args(["-e", PROBE, "--"]).arg(&npm.root).arg(
        serde_json::to_string(&input)
            .map_err(|_| npm_error("Could not prepare the runtime probe."))?,
    );
    let output = probe(command)?;
    if output.len() > 32768 {
        return Err(npm_error("The runtime probe returned too much metadata."));
    }
    let response: ProbeResponse = serde_json::from_str(&output)
        .map_err(|_| npm_error("The runtime probe returned invalid metadata."))?;
    let result = match response {
        ProbeResponse::Success(result) => result,
        ProbeResponse::Failure { error } => return Err(error),
    };
    if result.node_path != project.runtime.node_path || result.version != project.runtime.version {
        return Err(Error::new(
            ErrorCode::RuntimeChanged,
            "The selected Node executable or exact version changed.",
        ));
    }
    if result.npm_version != npm.version {
        return Err(npm_error("The paired npm version changed."));
    }
    let mut command = environment::command(&node, &npm.root, &env);
    command.arg(&npm.cli).arg("--version");
    if probe(command)?.trim() != npm.version {
        return Err(npm_error(
            "The npm CLI does not report the paired package's version.",
        ));
    }
    if node_before != fingerprint(&node)? {
        return Err(Error::new(
            ErrorCode::RuntimeChanged,
            "The Node executable changed during preparation.",
        ));
    }
    if files_before != [fingerprint(&npm.cli)?, fingerprint(&npm.metadata)?] {
        return Err(npm_error(
            "The npm installation changed during preparation.",
        ));
    }
    if npm_pair(&node, tool_boundary)? != npm {
        return Err(npm_error(
            "The paired npm link or metadata changed during preparation.",
        ));
    }
    reject_project_shims(tool_boundary, &node, &npm.cli)?;
    let receipt = format!("{node_before:?}{files_before:?}");
    Ok(PreparedRuntime {
        summary: RuntimePreparation {
            project_id: project.id.clone(),
            registry_revision,
            manifest_revision: inspection.manifest_revision.clone(),
            runtime: project.runtime.clone(),
            npm: NpmRuntime {
                cli_path: npm.cli.to_string_lossy().into(),
                version: npm.version,
            },
            runtime_revision: revision(receipt.as_bytes()),
        },
        root,
        env,
    })
}

#[cfg(all(test, unix))]
#[path = "isolation_tests.rs"]
mod tests;
