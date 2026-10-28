//! Optional Node registry, isolated runtime preparation, and explicitly owned runs.
//! PHP service snapshots and global Node activation stay independent.
use crate::config::Layout;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, sync::Mutex};

mod environment;
mod execution;
mod run_logs;
mod runner;
pub use execution::{
    LogRequest, RestartRequest, RunState, RunStatus, RunsSnapshot, StartRequest, StopRequest,
};
pub use run_logs::{LogBatch, LogRecord, LogStream};
pub use runner::entrypoint as runner_entrypoint;
mod isolation;
mod manifest;
pub use environment::ChildEnvironment;
pub use isolation::{NpmRuntime, PreparedRuntime, RuntimePreparation};
#[cfg(target_os = "macos")]
pub(crate) fn inspect_generated_manifest(root: &str) -> Result<Inspection> {
    manifest::inspect(root)
}
mod runtime;
pub(crate) use isolation::prepare_creation_runtime;
pub(crate) use runtime::{discover_planning, Inventory};
mod store;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    UnsupportedPlatform,
    Unavailable,
    InvalidProject,
    InvalidManifest,
    RuntimeUnavailable,
    RuntimeChanged,
    NpmUnavailable,
    EnvironmentUnavailable,
    RequirementConflict,
    UnsupportedPackageManager,
    RegistryUnavailable,
    Conflict,
    NotFound,
    SaveFailed,
    ConfirmationRequired,
    ProjectBusy,
    RunNotFound,
    StaleRun,
    ExecutionFailed,
    LogUnavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

impl Error {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub(crate) fn revision(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntime {
    pub node_path: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeChoice {
    pub id: String,
    #[serde(flatten)]
    pub runtime: ProjectRuntime,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectConfig {
    pub id: String,
    pub name: String,
    pub root: String,
    pub script: String,
    pub runtime: ProjectRuntime,
    pub open_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub config: ProjectConfig,
    pub root_available: bool,
    pub manifest_available: bool,
    /// File availability only; execution-time version validation is future work.
    pub runtime_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub supported: bool,
    pub revision: Option<String>,
    pub registry_error: Option<Error>,
    pub projects: Vec<ProjectView>,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionHints {
    pub node_version: Option<String>,
    pub nvmrc: Option<String>,
    pub engines_node: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManagerHint {
    pub name: String,
    pub declaration: Option<String>,
    pub lockfiles: Vec<String>,
    pub npm_supported: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub root: String,
    pub name: String,
    pub scripts: BTreeMap<String, String>,
    pub package_manager: PackageManagerHint,
    pub version_hints: VersionHints,
    pub manifest_revision: String,
    pub warnings: Vec<String>,
    pub runtimes: Vec<RuntimeChoice>,
    pub suggested_runtime_id: Option<String>,
    pub runtime_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectRequest {
    pub root: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveConfig {
    pub id: Option<String>,
    pub name: String,
    pub root: String,
    pub script: String,
    pub runtime_id: String,
    pub open_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveRequest {
    pub config: SaveConfig,
    pub expected_revision: String,
    pub manifest_revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveRequest {
    pub id: String,
    pub expected_revision: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResponse {
    pub revision: String,
    pub project: ProjectConfig,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryChanged {
    pub revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareRuntimeRequest {
    pub id: String,
    pub expected_revision: String,
    pub manifest_revision: String,
}

type Discover = fn(&Layout) -> runtime::Inventory;

/// Independently managed Tauri state: a corrupt Node registry cannot fail PHP startup.
pub struct NodeProjects {
    layout: Option<Layout>,
    supported: bool,
    mutation: Mutex<()>,
    discover: Discover,
    executions: execution::Executions,
}

impl NodeProjects {
    pub fn new(layout: Option<Layout>) -> Self {
        let executions =
            execution::Executions::new(layout.as_ref().map(|layout| layout.data.clone()));
        Self {
            layout,
            executions,
            supported: cfg!(target_os = "macos"),
            mutation: Mutex::new(()),
            discover: runtime::discover,
        }
    }

    fn layout(&self) -> Result<&Layout> {
        if !self.supported {
            return Err(Error::new(
                ErrorCode::UnsupportedPlatform,
                "Node Projects is currently available on macOS only.",
            ));
        }
        self.layout.as_ref().ok_or_else(|| {
            Error::new(
                ErrorCode::Unavailable,
                "The application data directory is unavailable.",
            )
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot = Snapshot {
            supported: self.supported,
            revision: None,
            registry_error: None,
            projects: Vec::new(),
        };
        if !self.supported {
            return snapshot;
        }
        let loaded = (|| {
            let layout = self.layout()?;
            let _guard = self.mutation.lock().map_err(|_| {
                Error::new(ErrorCode::Unavailable, "The Node registry is unavailable.")
            })?;
            store::load(&layout.data)
        })();
        match loaded {
            Ok((document, revision)) => {
                snapshot.revision = Some(revision);
                snapshot.projects = document
                    .projects
                    .into_iter()
                    .map(|config| ProjectView {
                        root_available: Path::new(&config.root).is_dir(),
                        manifest_available: Path::new(&config.root).join("package.json").is_file(),
                        runtime_available: runtime::available(&config.runtime),
                        config,
                    })
                    .collect();
            }
            Err(error) => snapshot.registry_error = Some(error),
        }
        snapshot
    }

    pub fn inspect(&self, request: InspectRequest) -> Result<Inspection> {
        let layout = self.layout()?;
        let mut inspection = manifest::inspect(&request.root)?;
        let inventory = (self.discover)(layout);
        inspection.suggested_runtime_id =
            runtime::suggestion(&inspection.version_hints, &inventory);
        inspection.runtimes = inventory.choices;
        inspection.runtime_error = inventory.error;
        Ok(inspection)
    }

    pub fn save(&self, request: SaveRequest) -> Result<SaveResponse> {
        let layout = self.layout()?;
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| Error::new(ErrorCode::Unavailable, "The Node registry is unavailable."))?;
        let (mut document, current) = store::load(&layout.data)?;
        store::check_revision(&request.expected_revision, &current)?;
        let config = request.config;
        if let Some(id) = &config.id {
            self.executions.ensure_stopped(id)?;
        }
        validate_text(&config.name, 80, "Enter a project name of 1–80 characters.")?;
        validate_text(
            &config.script,
            128,
            "Choose a package script of 1–128 characters.",
        )?;
        validate_url(config.open_url.as_deref())?;
        let inspection = manifest::inspect(&config.root)?;
        store::check_revision(&request.manifest_revision, &inspection.manifest_revision)?;
        if !inspection.scripts.contains_key(&config.script) {
            return Err(Error::new(
                ErrorCode::InvalidProject,
                "This script is no longer present in package.json. Inspect the project again.",
            ));
        }
        let inventory = (self.discover)(layout);
        let selected = inventory.choices.into_iter().find(|choice| choice.id == config.runtime_id)
            .filter(|choice| runtime::available(&choice.runtime))
            .ok_or_else(|| Error::new(ErrorCode::RuntimeUnavailable, inventory.error.unwrap_or_else(|| "The selected Node runtime is unavailable. Refresh runtime detection and choose an installed version.".into())))?;
        let id = match config.id {
            Some(id) if document.projects.iter().any(|project| project.id == id) => id,
            Some(_) => {
                return Err(Error::new(
                    ErrorCode::NotFound,
                    "This Node project is no longer registered.",
                ))
            }
            None => format!("{:032x}", rand::random::<u128>()),
        };
        if document
            .projects
            .iter()
            .any(|project| project.id != id && manifest::same_root(&project.root, &inspection.root))
        {
            return Err(Error::new(
                ErrorCode::InvalidProject,
                "This project folder is already registered.",
            ));
        }
        let project = ProjectConfig {
            id,
            name: config.name,
            root: inspection.root,
            script: config.script,
            runtime: selected.runtime,
            open_url: config.open_url,
        };
        if let Some(previous) = document
            .projects
            .iter_mut()
            .find(|previous| previous.id == project.id)
        {
            *previous = project.clone();
        } else {
            document.projects.push(project.clone());
        }
        // Discovery can execute user shell startup files and take time. Do not
        // save an inspection that became stale while discovering runtimes.
        let latest = manifest::inspect(&project.root)?;
        store::check_revision(&request.manifest_revision, &latest.manifest_revision)?;
        let revision = store::save(&layout.data, &request.expected_revision, &document)?;
        Ok(SaveResponse { revision, project })
    }

    /// Validate a saved selection and prepare an in-memory child environment.
    /// This does not execute an imported package script or persist any settings.
    pub fn prepare_runtime(&self, request: PrepareRuntimeRequest) -> Result<PreparedRuntime> {
        let layout = self.layout()?;
        let project = {
            let _guard = self.mutation.lock().map_err(|_| {
                Error::new(ErrorCode::Unavailable, "The Node registry is unavailable.")
            })?;
            let (document, current) = store::load(&layout.data)?;
            store::check_revision(&request.expected_revision, &current)?;
            document
                .projects
                .into_iter()
                .find(|project| project.id == request.id)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::NotFound,
                        "This Node project is no longer registered.",
                    )
                })?
        };
        let inspect = || -> Result<Inspection> {
            let inspection = manifest::inspect(&project.root)?;
            store::check_revision(&request.manifest_revision, &inspection.manifest_revision)?;
            if !inspection.scripts.contains_key(&project.script) {
                return Err(Error::new(
                    ErrorCode::InvalidProject,
                    "The selected script is missing. Inspect the project again.",
                ));
            }
            Ok(inspection)
        };
        let inspection = inspect()?;
        let prepared = isolation::prepare(
            layout,
            &project,
            &inspection,
            request.expected_revision.clone(),
        )?;
        inspect()?;
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| Error::new(ErrorCode::Unavailable, "The Node registry is unavailable."))?;
        store::check_revision(&request.expected_revision, &store::load(&layout.data)?.1)?;
        Ok(prepared)
    }

    pub fn start(&self, request: StartRequest) -> Result<RunStatus> {
        let layout = self.layout()?;
        if !request.confirmed {
            return Err(Error::new(
                ErrorCode::ConfirmationRequired,
                "Starting imported package scripts requires an explicit user action.",
            ));
        }
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| Error::new(ErrorCode::Unavailable, "The registry is unavailable."))?;
        self.executions.ensure_stopped(&request.id)?;
        let (document, revision) = store::load(&layout.data)?;
        store::check_revision(&request.expected_revision, &revision)?;
        let project = document
            .projects
            .into_iter()
            .find(|project| project.id == request.id)
            .ok_or_else(|| {
                Error::new(ErrorCode::NotFound, "This project is no longer registered.")
            })?;
        let inspection = manifest::inspect(&project.root)?;
        store::check_revision(&request.manifest_revision, &inspection.manifest_revision)?;
        self.executions.start(
            runner::Launch {
                home: layout.home.clone(),
                data: layout.data.clone(),
                runtime: layout.runtime.clone(),
                id: project.id,
                run_id: format!("{:032x}", rand::random::<u128>()),
                expected_revision: request.expected_revision,
                manifest_revision: request.manifest_revision,
            },
            project.runtime,
        )
    }
    pub fn stop(&self, request: StopRequest) -> Result<RunStatus> {
        self.layout()?;
        self.executions.stop(request)
    }
    pub fn restart(&self, request: RestartRequest) -> Result<RunStatus> {
        self.layout()?;
        if !request.confirmed {
            return Err(Error::new(
                ErrorCode::ConfirmationRequired,
                "Restart requires an explicit user action.",
            ));
        }
        self.stop(StopRequest {
            id: request.id.clone(),
            run_id: request.run_id,
        })?;
        self.start(StartRequest {
            id: request.id,
            expected_revision: request.expected_revision,
            manifest_revision: request.manifest_revision,
            confirmed: true,
        })
    }
    pub fn runs(&self) -> RunsSnapshot {
        RunsSnapshot {
            supported: self.supported,
            runs: self.executions.statuses(),
        }
    }
    pub fn logs(&self, request: LogRequest) -> Result<LogBatch> {
        self.layout()?;
        self.executions.logs(request)
    }
    pub fn stop_all_runs(&self) -> Result<()> {
        self.executions.stop_all()
    }
    pub fn disconnect_runs(&self) {
        self.executions.disconnect_all();
    }
    pub fn ensure_runtime_unused(&self, node: &Path) -> Result<()> {
        self.executions.ensure_runtime_unused(node)
    }
    pub fn observe_runs(&self, observer: impl Fn(RunStatus) + Send + Sync + 'static) {
        self.executions.set_observer(observer);
    }
    #[cfg(target_os = "macos")]
    fn execution_script(
        &self,
        id: &str,
        expected: &str,
        manifest_revision: &str,
    ) -> Result<String> {
        let (document, revision) = store::load(&self.layout()?.data)?;
        store::check_revision(expected, &revision)?;
        let project = document
            .projects
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| {
                Error::new(ErrorCode::NotFound, "This project is no longer registered.")
            })?;
        let inspection = manifest::inspect(&project.root)?;
        store::check_revision(manifest_revision, &inspection.manifest_revision)?;
        if !inspection.scripts.contains_key(&project.script) {
            return Err(Error::new(
                ErrorCode::InvalidProject,
                "The selected script disappeared.",
            ));
        }
        Ok(project.script)
    }

    /// Internal handoff from an explicitly confirmed local generation job.
    /// Uses the existing registry serializer/atomic writer, without shell discovery.
    #[cfg(target_os = "macos")]
    pub(crate) fn register_generated(
        &self,
        config: ProjectConfig,
        expected: Option<&str>,
        manifest_revision: &str,
    ) -> Result<SaveResponse> {
        let layout = self.layout()?;
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| Error::new(ErrorCode::Unavailable, "The Node registry is unavailable."))?;
        let (mut document, current) = store::load(&layout.data)?;
        if let Some(existing) = document
            .projects
            .iter()
            .find(|project| project.id == config.id)
        {
            if serde_json::to_vec(existing).ok() != serde_json::to_vec(&config).ok() {
                return Err(Error::new(
                    ErrorCode::Conflict,
                    "The generated project ID belongs to another registration.",
                ));
            }
            return Ok(SaveResponse {
                revision: current,
                project: existing.clone(),
            });
        }
        let expected = expected.ok_or_else(|| {
            Error::new(
                ErrorCode::RegistryUnavailable,
                "Reload the registry and retry registration; the published project was kept.",
            )
        })?;
        store::check_revision(expected, &current)?;
        if document
            .projects
            .iter()
            .any(|project| manifest::same_root(&project.root, &config.root))
        {
            return Err(Error::new(
                ErrorCode::Conflict,
                "This root is already registered. Its existing settings were kept.",
            ));
        }
        self.executions.ensure_stopped(&config.id)?;
        let inspection = manifest::inspect(&config.root)?;
        store::check_revision(manifest_revision, &inspection.manifest_revision)?;
        if inspection.root != config.root
            || !inspection.scripts.contains_key(&config.script)
            || !runtime::available(&config.runtime)
        {
            return Err(Error::new(
                ErrorCode::InvalidProject,
                "The generated root, script or selected runtime changed.",
            ));
        }
        document.projects.push(config.clone());
        let revision = store::save(&layout.data, expected, &document)?;
        Ok(SaveResponse {
            revision,
            project: config,
        })
    }

    pub fn remove(&self, request: RemoveRequest) -> Result<RegistryChanged> {
        let layout = self.layout()?;
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| Error::new(ErrorCode::Unavailable, "The Node registry is unavailable."))?;
        let (mut document, current) = store::load(&layout.data)?;
        store::check_revision(&request.expected_revision, &current)?;
        self.executions.ensure_stopped(&request.id)?;
        let count = document.projects.len();
        document.projects.retain(|project| project.id != request.id);
        if document.projects.len() == count {
            return Err(Error::new(
                ErrorCode::NotFound,
                "This Node project is no longer registered.",
            ));
        }
        let revision = store::save(&layout.data, &request.expected_revision, &document)?;
        self.executions.forget(&request.id);
        Ok(RegistryChanged { revision })
    }
}

fn validate_text(value: &str, limit: usize, message: &str) -> Result<()> {
    if value.is_empty()
        || value.trim() != value
        || value.chars().count() > limit
        || value.chars().any(char::is_control)
    {
        return Err(Error::new(ErrorCode::InvalidProject, message));
    }
    Ok(())
}

fn validate_url(value: Option<&str>) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let invalid = || {
        Error::new(
            ErrorCode::InvalidProject,
            "Use an HTTP/HTTPS URL on localhost, 127.0.0.1, or [::1], without credentials.",
        )
    };
    if value.len() > 2048
        || value.chars().any(|c| c.is_control() || c.is_whitespace())
        || value.contains('\\')
    {
        return Err(invalid());
    }
    let rest = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .ok_or_else(invalid)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let port = ["localhost", "127.0.0.1", "[::1]"]
        .iter()
        .find_map(|host| authority.strip_prefix(host))
        .ok_or_else(invalid)?;
    if !port.is_empty()
        && !port.strip_prefix(':').is_some_and(|p| {
            !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|p| p > 0)
        })
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "node_projects/execution_tests.rs"]
mod execution_tests;
