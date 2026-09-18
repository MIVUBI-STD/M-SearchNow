use super::error::CommandError;
use searchnow_core::{app_runtime::SearchNowBackendRuntime, LocalBackendSnapshot};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

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

#[tauri::command]
pub async fn export_local_content<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SearchNowBackendRuntime>,
    item_id: String,
) -> Result<Option<String>, CommandError> {
    let suggested = state
        .local_content_export_file_name(&item_id)
        .map_err(CommandError::from)?;
    let extension = if suggested
        .to_ascii_lowercase()
        .ends_with(".mcworld")
    {
        "mcworld"
    } else {
        "mcpack"
    };
    let selected = app
        .dialog()
        .file()
        .set_title("Export Minecraft backup")
        .set_file_name(&suggested)
        .add_filter("Minecraft backup", &[extension])
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|_| {
        CommandError::new(
            "library_export_destination_invalid",
            "The selected backup destination could not be used.",
        )
    })?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| suggested.clone());
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.export_local_content(&item_id, &path))
        .await
        .map_err(|error| {
            CommandError::new(
                "library_export_task_failed",
                format!("Local content export task failed: {error}"),
            )
        })?
        .map_err(CommandError::from)?;
    Ok(Some(file_name))
}
