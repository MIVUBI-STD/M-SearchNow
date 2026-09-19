use super::error::CommandError;
use searchnow_core::{
    app_runtime::SearchNowBackendRuntime,
    package::{
        PackageBundleUpdateRequest, PackageImportRequest, PackageImportResult, PackageInspection,
        PackageReplaceRequest,
    },
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
        .add_filter("Minecraft package", &["mcpack", "mcaddon", "mcworld"])
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
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.inspect_package(&path))
        .await
        .map_err(|error| {
            CommandError::new(
                "package_inspection_task_failed",
                format!("Package inspection task failed: {error}"),
            )
        })?
        .map(Some)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn import_package(
    state: State<'_, SearchNowBackendRuntime>,
    request: PackageImportRequest,
) -> Result<PackageImportResult, CommandError> {
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.import_package(request))
        .await
        .map_err(|error| {
            CommandError::new(
                "package_import_task_failed",
                format!("Package import task failed: {error}"),
            )
        })?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn choose_and_inspect_package_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SearchNowBackendRuntime>,
) -> Result<Option<PackageInspection>, CommandError> {
    let selected = app.dialog().file().blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|_| {
        CommandError::new(
            "package_path_invalid",
            "The selected package folder path could not be used.",
        )
    })?;
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.inspect_package(&path))
        .await
        .map_err(|error| {
            CommandError::new(
                "package_inspection_task_failed",
                format!("Package folder inspection task failed: {error}"),
            )
        })?
        .map(Some)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn replace_package(
    state: State<'_, SearchNowBackendRuntime>,
    request: PackageReplaceRequest,
) -> Result<PackageImportResult, CommandError> {
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.replace_package(request))
        .await
        .map_err(|error| {
            CommandError::new(
                "package_replace_task_failed",
                format!("Package update task failed: {error}"),
            )
        })?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn replace_package_bundle(
    state: State<'_, SearchNowBackendRuntime>,
    request: PackageBundleUpdateRequest,
) -> Result<PackageImportResult, CommandError> {
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.replace_package_bundle(request))
        .await
        .map_err(|error| {
            CommandError::new(
                "package_bundle_replace_task_failed",
                format!("Package bundle update task failed: {error}"),
            )
        })?
        .map_err(CommandError::from)
}
