use super::error::CommandError;
use searchnow_core::{app_runtime::SearchNowBackendRuntime, LocalBackendSnapshot};
use tauri::State;

#[tauri::command]
pub async fn scan_local_library(
    state: State<'_, SearchNowBackendRuntime>,
) -> Result<LocalBackendSnapshot, CommandError> {
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.scan_local_library())
        .await
        .map_err(|error| {
            CommandError::new(
                "library_scan_task_failed",
                format!("Local library scan task failed: {error}"),
            )
        })?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn remove_local_content(
    state: State<'_, SearchNowBackendRuntime>,
    item_id: String,
) -> Result<LocalBackendSnapshot, CommandError> {
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.remove_local_content(&item_id))
        .await
        .map_err(|error| {
            CommandError::new(
                "library_remove_task_failed",
                format!("Local content removal task failed: {error}"),
            )
        })?
        .map_err(CommandError::from)
}
