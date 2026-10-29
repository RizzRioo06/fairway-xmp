//! Finite, explicitly authorized npm jobs; separate from project run state.
use super::{Error, ErrorCode, Result};
use crate::node_projects;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InstallState {
    Starting,
    Installing,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InstallProgress {
    Preparing,
    Installing,
    Verifying,
    Publishing,
    Registering,
    Complete,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallStatus {
    pub job_id: String,
    pub creation_id: String,
    pub run_id: String,
    pub revision: u64,
    pub state: InstallState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<InstallProgress>,
    pub runtime: node_projects::ProjectRuntime,
    pub lockfile_sha256: String,
    pub registry: String,
    pub ci_completed: bool,
    pub cancel_requested: bool,
    pub exit_code: Option<i32>,
    pub error: Option<Error>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallRequest {
    pub plan_id: String,
    pub digest: String,
    pub confirmed: bool,
    pub install_confirmed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallReference {
    pub job_id: String,
    pub expected_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallQuery {
    pub job_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallLogRequest {
    pub job_id: String,
    pub after_cursor: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallRecoveryAction {
    Finalize,
    DiscardStaging,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallRecoveryRequest {
    pub job_id: String,
    pub expected_revision: u64,
    pub action: InstallRecoveryAction,
    pub confirmed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DependencyReceipt {
    pub dev: u64,
    pub ino: u64,
    pub sha256: String,
    pub entries: u64,
    pub bytes: u64,
}
struct Slot {
    status: InstallStatus,
    cancel: Arc<AtomicBool>,
    active: bool,
    publishing: bool,
}
pub(super) struct Installations {
    pub executions: node_projects::Executions,
    slots: Mutex<BTreeMap<String, Slot>>,
    finished: Condvar,
    accepting: AtomicBool,
    #[cfg(all(debug_assertions, feature = "node-install-fixtures"))]
    pub fixture_registry: Mutex<Option<String>>,
}
impl Installations {
    pub fn new(data: Option<PathBuf>) -> Self {
        Self {
            executions: node_projects::Executions::new(data),
            slots: Mutex::new(BTreeMap::new()),
            finished: Condvar::new(),
            accepting: AtomicBool::new(true),
            #[cfg(all(debug_assertions, feature = "node-install-fixtures"))]
            fixture_registry: Mutex::new(None),
        }
    }
    pub fn creation_status(&self, id: &str) -> Option<(InstallStatus, bool)> {
        self.slots
            .lock()
            .ok()?
            .values()
            .find(|slot| slot.status.creation_id == id)
            .map(|slot| (slot.status.clone(), slot.active))
    }
    pub fn ensure_creation_idle(&self, id: &str) -> Result<()> {
        if self
            .slots
            .lock()
            .map_err(|_| unavailable())?
            .values()
            .any(|slot| slot.active && slot.status.creation_id == id)
        {
            return Err(Error::new(
                ErrorCode::InstallBusy,
                "An owned installer is still using this creation workspace.",
            ));
        }
        Ok(())
    }
    pub fn ensure_runtime_unused(&self, node: &Path) -> Result<()> {
        let node = std::fs::canonicalize(node).unwrap_or_else(|_| node.into());
        if self
            .slots
            .lock()
            .map_err(|_| unavailable())?
            .values()
            .any(|slot| slot.active && Path::new(&slot.status.runtime.node_path) == node)
        {
            return Err(Error::new(ErrorCode::InstallBusy,"An installation owns this runtime. Cancel/wait for it before removing the runtime."));
        }
        self.executions
            .ensure_runtime_unused(&node)
            .map_err(Into::into)
    }
    fn update(&self, status: &InstallStatus, active: bool) {
        let mut slots = self.slots.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(slot) = slots.get_mut(&status.job_id) {
            slot.status = status.clone();
            slot.active = active;
        }
        self.finished.notify_all();
    }
    pub fn cancel_all(&self) {
        for slot in self
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
        {
            if slot.active {
                slot.cancel.store(true, Ordering::SeqCst);
            }
        }
    }
    pub fn stop_all(&self) -> Result<()> {
        self.cancel_all();
        self.executions.stop_all().map_err(Error::from)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut slots = self.slots.lock().map_err(|_| unavailable())?;
        while slots.values().any(|slot| slot.active) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Error::new(
                    ErrorCode::InstallationFailed,
                    "Installation cleanup is incomplete; ownership was retained.",
                ));
            }
            slots = self
                .finished
                .wait_timeout(slots, remaining)
                .map_err(|_| unavailable())?
                .0;
        }
        Ok(())
    }
    pub fn disconnect(&self) {
        self.accepting.store(false, Ordering::SeqCst);
        self.cancel_all();
        self.executions.disconnect_all();
    }
}
fn unavailable() -> Error {
    Error::new(ErrorCode::Unavailable, "Installation state is unavailable.")
}
#[cfg(target_os = "macos")]
#[path = "installation_native.rs"]
mod native;
#[cfg(target_os = "macos")]
pub(super) use native::verify_project;
#[cfg(target_os = "macos")]
pub(crate) use native::{runner_command, runner_lock};
#[cfg(not(target_os = "macos"))]
impl super::NodeProjectCreation {
    pub fn start_installation(
        self: &Arc<Self>,
        _nodes: Arc<node_projects::NodeProjects>,
        _request: InstallRequest,
    ) -> Result<InstallStatus> {
        Err(Error::new(
            ErrorCode::UnsupportedPlatform,
            "Dependency installation is macOS-only.",
        ))
    }
    pub fn installation_status(&self, _request: InstallQuery) -> Result<InstallStatus> {
        Err(Error::new(
            ErrorCode::UnsupportedPlatform,
            "Dependency installation is macOS-only.",
        ))
    }
    pub fn installation_logs(
        &self,
        _request: InstallLogRequest,
    ) -> Result<node_projects::LogBatch> {
        Err(Error::new(
            ErrorCode::UnsupportedPlatform,
            "Dependency installation is macOS-only.",
        ))
    }
    pub fn cancel_installation(&self, _request: InstallReference) -> Result<InstallStatus> {
        Err(Error::new(
            ErrorCode::UnsupportedPlatform,
            "Dependency installation is macOS-only.",
        ))
    }
    pub fn recover_installation(
        &self,
        _nodes: &node_projects::NodeProjects,
        _request: InstallRecoveryRequest,
    ) -> Result<InstallStatus> {
        Err(Error::new(
            ErrorCode::UnsupportedPlatform,
            "Dependency installation is macOS-only.",
        ))
    }
}
impl super::NodeProjectCreation {
    pub fn ensure_install_runtime_unused(&self, node: &Path) -> Result<()> {
        self.installation.ensure_runtime_unused(node)?;
        #[cfg(target_os = "macos")]
        if self.layout.is_some() {
            native::ensure_recovered_runtime_unused(self, node)?;
        }
        Ok(())
    }
    pub fn stop_installations(&self) -> Result<()> {
        self.installation.stop_all()
    }
    pub fn cancel_installations_for_shutdown(&self) {
        self.installation.cancel_all();
        #[cfg(target_os = "macos")]
        if let Some(active) = self
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            active.cancelled.store(true, Ordering::SeqCst);
        }
    }
    pub fn disconnect_installations(&self) {
        self.installation.disconnect();
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "installation_tests.rs"]
mod tests;
