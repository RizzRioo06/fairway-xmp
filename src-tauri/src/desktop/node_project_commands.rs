//! Node registry, runtime and owned execution IPC, independent of the PHP manager.
use crate::node_projects::{self, Error, ErrorCode, NodeProjects};
use std::sync::Arc;
use tauri::{Emitter, State};

fn task_error(error: tauri::Error) -> Error {
    Error {
        code: ErrorCode::Unavailable,
        message: error.to_string(),
    }
}

#[tauri::command]
pub(super) async fn node_projects_snapshot(
    state: State<'_, Arc<NodeProjects>>,
) -> node_projects::Result<node_projects::Snapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.snapshot())
        .await
        .map_err(task_error)
}

#[tauri::command]
pub(super) async fn node_project_inspect(
    state: State<'_, Arc<NodeProjects>>,
    request: node_projects::InspectRequest,
) -> node_projects::Result<node_projects::Inspection> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.inspect(request))
        .await
        .map_err(task_error)?
}

#[tauri::command]
pub(super) async fn node_project_save(
    app: tauri::AppHandle,
    state: State<'_, Arc<NodeProjects>>,
    request: node_projects::SaveRequest,
) -> node_projects::Result<node_projects::SaveResponse> {
    let _operation =
        super::begin_operation(&app, "Saving Node project…").map_err(|message| Error {
            code: ErrorCode::Unavailable,
            message,
        })?;
    let state = state.inner().clone();
    let saved = tauri::async_runtime::spawn_blocking(move || state.save(request))
        .await
        .map_err(task_error)??;
    let _ = app.emit(
        "node-projects-changed",
        node_projects::RegistryChanged {
            revision: saved.revision.clone(),
        },
    );
    Ok(saved)
}

#[tauri::command]
pub(super) async fn node_project_remove(
    app: tauri::AppHandle,
    state: State<'_, Arc<NodeProjects>>,
    request: node_projects::RemoveRequest,
) -> node_projects::Result<node_projects::RegistryChanged> {
    let _operation =
        super::begin_operation(&app, "Removing Node project…").map_err(|message| Error {
            code: ErrorCode::Unavailable,
            message,
        })?;
    let state = state.inner().clone();
    let changed = tauri::async_runtime::spawn_blocking(move || state.remove(request))
        .await
        .map_err(task_error)??;
    let _ = app.emit("node-projects-changed", &changed);
    Ok(changed)
}

#[tauri::command]
pub(super) async fn node_project_prepare_runtime(
    app: tauri::AppHandle,
    state: State<'_, Arc<NodeProjects>>,
    request: node_projects::PrepareRuntimeRequest,
) -> node_projects::Result<node_projects::RuntimePreparation> {
    let _operation =
        super::begin_operation(&app, "Preparing Node runtime…").map_err(|message| Error {
            code: ErrorCode::Unavailable,
            message,
        })?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .prepare_runtime(request)
            .map(|prepared| prepared.summary().clone())
    })
    .await
    .map_err(task_error)?
}

#[tauri::command]
pub(super) async fn node_project_start(
    app: tauri::AppHandle,
    state: State<'_, Arc<NodeProjects>>,
    request: node_projects::StartRequest,
) -> node_projects::Result<node_projects::RunStatus> {
    let _operation =
        super::begin_operation(&app, "Starting Node project…").map_err(|message| Error {
            code: ErrorCode::Unavailable,
            message,
        })?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.start(request))
        .await
        .map_err(task_error)?
}
#[tauri::command]
pub(super) async fn node_project_stop(
    app: tauri::AppHandle,
    state: State<'_, Arc<NodeProjects>>,
    request: node_projects::StopRequest,
) -> node_projects::Result<node_projects::RunStatus> {
    let _operation =
        super::begin_operation(&app, "Stopping Node project…").map_err(|message| Error {
            code: ErrorCode::Unavailable,
            message,
        })?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.stop(request))
        .await
        .map_err(task_error)?
}
#[tauri::command]
pub(super) async fn node_project_restart(
    app: tauri::AppHandle,
    state: State<'_, Arc<NodeProjects>>,
    request: node_projects::RestartRequest,
) -> node_projects::Result<node_projects::RunStatus> {
    let _operation =
        super::begin_operation(&app, "Restarting Node project…").map_err(|message| Error {
            code: ErrorCode::Unavailable,
            message,
        })?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.restart(request))
        .await
        .map_err(task_error)?
}
#[tauri::command]
pub(super) async fn node_project_runs(
    state: State<'_, Arc<NodeProjects>>,
) -> node_projects::Result<node_projects::RunsSnapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.runs())
        .await
        .map_err(task_error)
}
#[tauri::command]
pub(super) async fn node_project_logs(
    state: State<'_, Arc<NodeProjects>>,
    request: node_projects::LogRequest,
) -> node_projects::Result<node_projects::LogBatch> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.logs(request))
        .await
        .map_err(task_error)?
}
