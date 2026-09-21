use super::error::CommandError;
use searchnow_core::{
    app_runtime::{BackendRuntimeSnapshot, SearchNowBackendRuntime},
    diagnostics::BackendDiagnosticsSnapshot,
    runtime::RuntimeStatus,
};
use std::fs;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn get_runtime_status(state: State<'_, SearchNowBackendRuntime>) -> RuntimeStatus {
    state.runtime_status()
}

#[tauri::command]
pub fn get_backend_snapshot(state: State<'_, SearchNowBackendRuntime>) -> BackendRuntimeSnapshot {
    state.snapshot()
}

#[tauri::command]
pub fn get_backend_diagnostics(
    state: State<'_, SearchNowBackendRuntime>,
) -> BackendDiagnosticsSnapshot {
    state.diagnostics_snapshot()
}

#[tauri::command]
pub async fn export_diagnostics_report<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SearchNowBackendRuntime>,
) -> Result<Option<String>, CommandError> {
    let selected = app
        .dialog()
        .file()
        .set_title("Export SearchNow diagnostics")
        .set_file_name("searchnow-diagnostics.json")
        .add_filter("JSON report", &["json"])
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(None);
    };

    let path = selected.into_path().map_err(|_| {
        CommandError::new(
            "diagnostics_report_destination_invalid",
            "The selected diagnostics report destination could not be used.",
        )
    })?;

    let bytes = state
        .diagnostics_support_report_json()
        .map_err(CommandError::from)?;

    fs::write(&path, bytes).map_err(|error| {
        CommandError::new(
            "diagnostics_report_write_failed",
            format!("SearchNow could not write the diagnostics report: {error}"),
        )
    })?;

    Ok(path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| Some("searchnow-diagnostics.json".into())))
}
