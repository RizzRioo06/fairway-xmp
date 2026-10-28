//! Local generation/recovery DTOs. No dependency installer or project runner.
use super::Error;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateRequest {
    pub plan_id: String,
    pub digest: String,
    pub confirmed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelRequest {
    pub job_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryAction {
    DiscardStaging,
    RetryRegistration,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoverRequest {
    pub job_id: String,
    pub expected_revision: u64,
    pub action: RecoveryAction,
    pub confirmed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Prepared,
    Staging,
    Ready,
    Publishing,
    NeedsRegistration,
    Completed,
    Cancelled,
    Preserved,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobStatus {
    pub job_id: String,
    pub revision: u64,
    pub phase: Phase,
    pub destination: String,
    pub staging_path: String,
    pub project_id: Option<String>,
    pub error: Option<Error>,
    pub active: bool,
    pub recovery_required: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobsSnapshot {
    pub supported: bool,
    pub jobs: Vec<JobStatus>,
    pub errors: Vec<String>,
}
#[cfg(target_os = "macos")]
pub(super) struct Control {
    pub id: String,
    pub cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub publishing: bool,
}
#[cfg(target_os = "macos")]
#[path = "generation_native.rs"]
mod native;
#[cfg(not(target_os = "macos"))]
impl super::NodeProjectCreation {
    pub fn generate(
        &self,
        _nodes: &crate::node_projects::NodeProjects,
        _request: GenerateRequest,
    ) -> super::Result<JobStatus> {
        Err(super::Error::new(
            super::ErrorCode::UnsupportedPlatform,
            "Local generation is currently macOS-only.",
        ))
    }
    pub fn recover(
        &self,
        _nodes: &crate::node_projects::NodeProjects,
        _request: RecoverRequest,
    ) -> super::Result<JobStatus> {
        Err(super::Error::new(
            super::ErrorCode::UnsupportedPlatform,
            "Local recovery is currently macOS-only.",
        ))
    }
    pub fn cancel_generation(&self, _request: CancelRequest) -> super::Result<()> {
        Err(super::Error::new(
            super::ErrorCode::UnsupportedPlatform,
            "Local generation is currently macOS-only.",
        ))
    }
    pub fn creation_jobs(&self) -> JobsSnapshot {
        JobsSnapshot {
            supported: false,
            jobs: Vec::new(),
            errors: Vec::new(),
        }
    }
}
