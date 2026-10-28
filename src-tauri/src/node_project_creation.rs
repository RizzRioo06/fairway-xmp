//! Metadata-only Node project creation planning. No filesystem mutation or setup runner.
use crate::{
    config::Layout,
    node_projects::{self, ProjectRuntime, RuntimeChoice, RuntimePreparation},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod catalog;
mod destination;
pub use catalog::{Catalog, Dependency, PackageManager, Provenance, Template};

const PLAN_TTL: Duration = Duration::from_secs(600);
const PLAN_LIMIT: usize = 64;
type Discover = fn(&Layout) -> node_projects::Inventory;
type Prepare = fn(
    &Layout,
    ProjectRuntime,
    &Path,
    &Path,
    &str,
    &str,
) -> node_projects::Result<RuntimePreparation>;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    UnsupportedPlatform,
    Unavailable,
    InvalidName,
    InvalidDestination,
    DestinationExists,
    InvalidTemplate,
    UnsupportedPackageManager,
    RuntimeUnavailable,
    RuntimeChanged,
    NpmUnavailable,
    RequirementConflict,
    EnvironmentUnavailable,
    RuntimeValidationFailed,
    CatalogChanged,
    InvalidRequest,
    DuplicateRequest,
    PlanUnavailable,
    PlanTampered,
    PlanExpired,
    PlanStale,
    PlanLimit,
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
impl From<node_projects::Error> for Error {
    fn from(value: node_projects::Error) -> Self {
        use node_projects::ErrorCode as Node;
        let code = match value.code {
            Node::RuntimeUnavailable => ErrorCode::RuntimeUnavailable,
            Node::RuntimeChanged => ErrorCode::RuntimeChanged,
            Node::NpmUnavailable => ErrorCode::NpmUnavailable,
            Node::RequirementConflict => ErrorCode::RequirementConflict,
            Node::EnvironmentUnavailable => ErrorCode::EnvironmentUnavailable,
            _ => ErrorCode::RuntimeValidationFailed,
        };
        Self::new(code, value.message)
    }
}
fn digest(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshot {
    pub supported: bool,
    pub catalog_revision: String,
    pub catalog: Catalog,
    pub runtimes: Vec<RuntimeChoice>,
    pub runtime_error: Option<String>,
    pub discovery_note: &'static str,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanRequest {
    pub request_id: String,
    pub catalog_revision: String,
    pub template_id: String,
    pub project_name: String,
    pub parent: String,
    pub runtime_id: String,
    pub package_manager: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanReference {
    pub plan_id: String,
    pub digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidatedTools {
    pub runtime_id: String,
    pub node_path: String,
    pub node_version: String,
    pub npm_cli_path: String,
    pub npm_version: String,
    pub revision: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallationPolicy {
    pub package_manager: String,
    pub intended_command: String,
    pub ignore_scripts: bool,
    pub registry: String,
    pub cache_scope: String,
    pub dependency_lock: String,
    pub installation_available: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreationPlan {
    pub schema_version: u32,
    pub id: String,
    pub digest: String,
    pub catalog_revision: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub project_name: String,
    pub parent: String,
    pub destination: String,
    pub template: Template,
    pub tools: ValidatedTools,
    pub installation: InstallationPolicy,
    pub warnings: Vec<String>,
}
struct StoredPlan {
    plan: CreationPlan,
    request: PlanRequest,
    request_digest: String,
    location: destination::Destination,
    deadline: Instant,
}
/// Independent optional Tauri state; it does not read/write the project registry.
pub struct NodeProjectCreation {
    layout: Option<Layout>,
    supported: bool,
    discover: Discover,
    prepare: Prepare,
    plans: Mutex<BTreeMap<String, StoredPlan>>,
}
impl NodeProjectCreation {
    pub fn new(layout: Option<Layout>) -> Self {
        Self {
            layout,
            supported: cfg!(target_os = "macos"),
            discover: node_projects::discover_planning,
            prepare: node_projects::prepare_creation_runtime,
            plans: Mutex::new(BTreeMap::new()),
        }
    }
    fn layout(&self) -> Result<&Layout> {
        if !self.supported {
            return Err(Error::new(
                ErrorCode::UnsupportedPlatform,
                "Node project creation planning is available on macOS first.",
            ));
        }
        self.layout.as_ref().ok_or_else(|| {
            Error::new(
                ErrorCode::Unavailable,
                "The application layout is unavailable.",
            )
        })
    }
    pub fn catalog(&self) -> CatalogSnapshot {
        let inventory = if self.supported {
            self.layout.as_ref().map(|layout| (self.discover)(layout))
        } else {
            None
        };
        let (runtimes, runtime_error) = inventory.map(|i| (i.choices, i.error)).unwrap_or_default();
        CatalogSnapshot { supported: self.supported && self.layout.is_some(), catalog_revision: catalog::revision(), catalog: catalog::catalog(), runtimes, runtime_error,
            discovery_note: "Read-only inventory uses process environment and standard installation folders. Login-shell-only custom stores may not appear; no shell profiles or fnm commands are executed." }
    }
    fn tools(
        &self,
        request: &PlanRequest,
        template: &Template,
        location: &destination::Destination,
    ) -> Result<ValidatedTools> {
        let layout = self.layout()?;
        let choice = (self.discover)(layout).choices.into_iter().find(|choice| choice.id == request.runtime_id)
            .ok_or_else(|| Error::new(ErrorCode::RuntimeUnavailable, "Select a currently discovered installed Node runtime; executable paths are not accepted."))?;
        let prepared = (self.prepare)(
            layout,
            choice.runtime,
            &location.parent,
            &location.root,
            &template.node_range,
            &template.package_manager.version_range,
        )?;
        Ok(ValidatedTools {
            runtime_id: choice.id,
            node_path: prepared.runtime.node_path,
            node_version: prepared.runtime.version,
            npm_cli_path: prepared.npm.cli_path,
            npm_version: prepared.npm.version,
            revision: prepared.runtime_revision,
        })
    }
    pub fn plan(&self, request: PlanRequest) -> Result<CreationPlan> {
        self.plan_at(request, Instant::now(), time_ms())
    }
    fn plan_at(&self, request: PlanRequest, now: Instant, unix_ms: u64) -> Result<CreationPlan> {
        self.layout()?;
        if !hex(&request.request_id, 32) || !hex(&request.runtime_id, 64) {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Use a 32-hex request ID and a discovered runtime ID.",
            ));
        }
        if request.catalog_revision != catalog::revision() {
            return Err(Error::new(
                ErrorCode::CatalogChanged,
                "Reload the template catalog before planning.",
            ));
        }
        if request.package_manager != "npm" {
            return Err(Error::new(
                ErrorCode::UnsupportedPackageManager,
                "Only paired npm is supported.",
            ));
        }
        let template = catalog::catalog()
            .templates
            .into_iter()
            .find(|template| template.id == request.template_id)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidTemplate,
                    "Choose a built-in template from the current catalog.",
                )
            })?;
        let location = destination::validate(&request.parent, &request.project_name)?;
        let request_digest = digest(&serde_json::to_vec(&request).expect("serializable request"));
        // A bounded, independent lock serializes duplicate submissions. Neither
        // the PHP manager nor the Node registry lock is held during probes.
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| Error::new(ErrorCode::Unavailable, "Plan storage is unavailable."))?;
        if let Some(stored) = plans.get(&request.request_id) {
            if stored.request_digest != request_digest {
                return Err(Error::new(
                    ErrorCode::DuplicateRequest,
                    "This request ID belongs to different inputs. Submit a new request ID.",
                ));
            }
            return self.validate_stored(stored, now);
        }
        plans.retain(|_, stored| stored.deadline > now);
        if plans.len() >= PLAN_LIMIT {
            return Err(Error::new(ErrorCode::PlanLimit, "At most 64 live plans are retained. Wait for expiration before planning more projects."));
        }
        let tools = self.tools(&request, &template, &location)?;
        if destination::validate(&request.parent, &request.project_name)? != location {
            return Err(Error::new(
                ErrorCode::PlanStale,
                "The parent folder changed during runtime validation.",
            ));
        }
        // Expiration starts before bounded probes: slow validation cannot extend consent.
        if now.elapsed() >= PLAN_TTL {
            return Err(Error::new(
                ErrorCode::PlanExpired,
                "Runtime validation took too long; plan again.",
            ));
        }
        let mut plan = CreationPlan {
            schema_version: 1, id: format!("{:032x}", rand::random::<u128>()), digest: String::new(), catalog_revision: catalog::revision(),
            created_at: unix_ms, expires_at: unix_ms.saturating_add(PLAN_TTL.as_millis() as u64), project_name: request.project_name.clone(),
            parent: location.parent.to_str().expect("validated UTF-8").into(), destination: location.root.to_str().expect("validated UTF-8").into(),
            template, tools,
            installation: InstallationPolicy { package_manager: "npm".into(), intended_command: "ci".into(), ignore_scripts: true,
                registry: "https://registry.npmjs.org/".into(), cache_scope: "appOwned".into(), dependency_lock: "notPrepared".into(), installation_available: false },
            warnings: vec!["Dry run only: no project files, dependencies or registrations were created; no setup or project scripts were executed.".into(),
                "Template assets and full dependency lockfiles are not prepared or install-qualified in Stage 1.".into(),
                "A plan neither reserves a destination nor grants execution permission. Later stages must revalidate and request explicit approval.".into()],
        };
        plan.digest =
            digest(&serde_json::to_vec(&(&plan, &location.identity)).expect("serializable plan"));
        plans.insert(
            request.request_id.clone(),
            StoredPlan {
                plan: plan.clone(),
                request,
                request_digest,
                location,
                deadline: now + PLAN_TTL,
            },
        );
        Ok(plan)
    }
    pub fn validate_plan(&self, reference: PlanReference) -> Result<CreationPlan> {
        self.validate_at(reference, Instant::now())
    }
    fn validate_at(&self, reference: PlanReference, now: Instant) -> Result<CreationPlan> {
        self.layout()?;
        if !hex(&reference.plan_id, 32) || !hex(&reference.digest, 64) {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Invalid plan reference.",
            ));
        }
        let plans = self
            .plans
            .lock()
            .map_err(|_| Error::new(ErrorCode::Unavailable, "Plan storage is unavailable."))?;
        let stored = plans.values().find(|stored| stored.plan.id == reference.plan_id)
            .ok_or_else(|| Error::new(ErrorCode::PlanUnavailable, "This plan is unavailable after expiration, eviction or app restart. Plan again."))?;
        if stored.plan.digest != reference.digest {
            return Err(Error::new(
                ErrorCode::PlanTampered,
                "The plan digest does not match the native review plan.",
            ));
        }
        self.validate_stored(stored, now)
    }
    fn validate_stored(&self, stored: &StoredPlan, now: Instant) -> Result<CreationPlan> {
        if now >= stored.deadline {
            return Err(Error::new(
                ErrorCode::PlanExpired,
                "This plan expired. Plan again before reviewing creation.",
            ));
        }
        if stored.plan.catalog_revision != catalog::revision() {
            return Err(Error::new(
                ErrorCode::PlanStale,
                "The template catalog changed.",
            ));
        }
        if destination::validate(&stored.request.parent, &stored.request.project_name)?
            != stored.location
        {
            return Err(Error::new(
                ErrorCode::PlanStale,
                "The destination parent changed. Plan again.",
            ));
        }
        if self.tools(&stored.request, &stored.plan.template, &stored.location)?
            != stored.plan.tools
        {
            return Err(Error::new(
                ErrorCode::PlanStale,
                "The selected runtime/npm installation changed. Plan again.",
            ));
        }
        if Instant::now() >= stored.deadline {
            return Err(Error::new(
                ErrorCode::PlanExpired,
                "The plan expired during runtime validation. Plan again.",
            ));
        }
        if destination::validate(&stored.request.parent, &stored.request.project_name)?
            != stored.location
        {
            return Err(Error::new(
                ErrorCode::PlanStale,
                "The destination changed during validation. Plan again.",
            ));
        }
        Ok(stored.plan.clone())
    }
}
#[cfg(all(test, unix))]
#[path = "node_project_creation/tests.rs"]
mod tests;
