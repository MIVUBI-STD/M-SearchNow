use super::error::CommandError;
use searchnow_core::app_runtime::SearchNowBackendRuntime;
use std::process::Command;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

fn pick_directory<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, CommandError> {
    let selected = app.dialog().file().blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|_| {
        CommandError::new(
            "dialog_path_invalid",
            "The selected folder could not be used.",
        )
    })?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn choose_download_directory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, CommandError> {
    pick_directory(&app)
}

#[tauri::command]
pub async fn choose_minecraft_directory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, CommandError> {
    pick_directory(&app)
}

#[tauri::command]
pub async fn choose_export_directory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, CommandError> {
    pick_directory(&app)
}

#[tauri::command]
pub fn open_local_content_directory(
    state: State<'_, SearchNowBackendRuntime>,
    item_id: String,
) -> Result<(), CommandError> {
    let path = state
        .local_content_directory(&item_id)
        .map_err(CommandError::from)?;
    open_directory(path)
}

#[tauri::command]
pub fn open_download_directory(
    state: State<'_, SearchNowBackendRuntime>,
    job_id: String,
) -> Result<(), CommandError> {
    let path = state
        .completed_download_directory(&job_id)
        .map_err(CommandError::from)?;
    open_directory(path)
}

fn open_directory(path: std::path::PathBuf) -> Result<(), CommandError> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(CommandError::new(
            "directory_unavailable",
            "The folder is no longer available.",
        ));
    }

    let mut command = platform_open_command(&path);
    command.spawn().map_err(|_| {
        CommandError::new("directory_open_failed", "The folder could not be opened.")
    })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn platform_open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("explorer.exe");
    command.arg(path);
    command
}

#[cfg(target_os = "macos")]
fn platform_open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}
