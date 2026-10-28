//! Catalog/planning and explicitly confirmed local generation/recovery IPC. No installer.
use crate::node_project_creation::{self as creation, NodeProjectCreation};
use std::sync::Arc;
use tauri::State;
fn task_error(error: tauri::Error) -> creation::Error {
    creation::Error {
        code: creation::ErrorCode::Unavailable,
        message: error.to_string(),
    }
}
#[tauri::command]
pub(super) async fn node_project_templates(
    state: State<'_, Arc<NodeProjectCreation>>,
) -> creation::Result<creation::CatalogSnapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.catalog())
        .await
        .map_err(task_error)
}
#[tauri::command]
pub(super) async fn node_project_creation_plan(
    state: State<'_, Arc<NodeProjectCreation>>,
    request: creation::PlanRequest,
) -> creation::Result<creation::CreationPlan> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.plan(request))
        .await
        .map_err(task_error)?
}
#[tauri::command]
pub(super) async fn node_project_creation_validate_plan(
    state: State<'_, Arc<NodeProjectCreation>>,
    request: creation::PlanReference,
) -> creation::Result<creation::CreationPlan> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.validate_plan(request))
        .await
        .map_err(task_error)?
}

#[tauri::command]
pub(super) async fn node_project_creation_generate(
    app: tauri::AppHandle,
    state: State<'_, Arc<NodeProjectCreation>>,
    nodes: State<'_, Arc<crate::node_projects::NodeProjects>>,
    request: creation::GenerateRequest,
) -> creation::Result<creation::JobStatus> {
    use tauri::Emitter;
    let _operation =
        super::begin_operation(&app, "Generating local Node project…").map_err(|message| {
            creation::Error {
                code: creation::ErrorCode::Unavailable,
                message,
            }
        })?;
    let state = state.inner().clone();
    let nodes = nodes.inner().clone();
    let (result, revision) = tauri::async_runtime::spawn_blocking(move || {
        let result = state.generate(&nodes, request);
        (result, nodes.snapshot().revision)
    })
    .await
    .map_err(task_error)?;
    if let Some(revision) = revision {
        let _ = app.emit(
            "node-projects-changed",
            crate::node_projects::RegistryChanged { revision },
        );
    }
    result
}
#[tauri::command]
pub(super) async fn node_project_creation_recover(
    app: tauri::AppHandle,
    state: State<'_, Arc<NodeProjectCreation>>,
    nodes: State<'_, Arc<crate::node_projects::NodeProjects>>,
    request: creation::RecoverRequest,
) -> creation::Result<creation::JobStatus> {
    use tauri::Emitter;
    let _operation =
        super::begin_operation(&app, "Recovering local Node project…").map_err(|message| {
            creation::Error {
                code: creation::ErrorCode::Unavailable,
                message,
            }
        })?;
    let state = state.inner().clone();
    let nodes = nodes.inner().clone();
    let (result, revision) = tauri::async_runtime::spawn_blocking(move || {
        let result = state.recover(&nodes, request);
        (result, nodes.snapshot().revision)
    })
    .await
    .map_err(task_error)?;
    if let Some(revision) = revision {
        let _ = app.emit(
            "node-projects-changed",
            crate::node_projects::RegistryChanged { revision },
        );
    }
    result
}
#[tauri::command]
pub(super) async fn node_project_creation_jobs(
    state: State<'_, Arc<NodeProjectCreation>>,
) -> creation::Result<creation::JobsSnapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.creation_jobs())
        .await
        .map_err(task_error)
}
// Cancellation must bypass the normal operation guard held by Generate.
#[tauri::command]
pub(super) fn node_project_creation_cancel(
    state: State<'_, Arc<NodeProjectCreation>>,
    request: creation::CancelRequest,
) -> creation::Result<()> {
    state.cancel_generation(request)
}
