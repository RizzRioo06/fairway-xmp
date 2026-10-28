//! Read-only catalog/planning IPC. There is intentionally no creation/start command.
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
