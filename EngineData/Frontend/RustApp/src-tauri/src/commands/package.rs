use super::error::CommandError;
use searchnow_core::{
    app_runtime::SearchNowBackendRuntime,
    package::{PackageImportRequest, PackageImportResult, PackageInspection},
};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn choose_and_inspect_package<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SearchNowBackendRuntime>,
) -> Result<Option<PackageInspection>, CommandError> {
    let selected = app
        .dialog()
        .file()
        .add_filter("Minecraft package", &["mcpack", "mcaddon"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|_| {
        CommandError::new(
            "package_path_invalid",
            "The selected package path could not be used.",
        )
    })?;
    state
        .inspect_package(&path)
        .map(Some)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn import_package(
    state: State<'_, SearchNowBackendRuntime>,
    request: PackageImportRequest,
) -> Result<PackageImportResult, CommandError> {
    state.import_package(request).map_err(CommandError::from)
}
