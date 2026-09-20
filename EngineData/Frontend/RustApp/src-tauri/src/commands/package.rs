use super::error::CommandError;
use searchnow_core::{
    app_runtime::SearchNowBackendRuntime,
    package::{
        PackageBundleUpdateRequest, PackageImportRequest, PackageImportResult, PackageInspection,
        PackageReplaceRequest,
    },
};
use std::path::PathBuf;
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
pub async fn inspect_package_path(
    state: State<'_, SearchNowBackendRuntime>,
    path: String,
) -> Result<PackageInspection, CommandError> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(CommandError::new(
            "package_path_invalid",
            "Dropped package path must be absolute.",
        ));
    }

    let metadata = std::fs::symlink_metadata(&path).map_err(|_| {
        CommandError::new(
            "package_path_invalid",
            "The dropped package or folder is no longer available.",
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(CommandError::new(
            "package_path_invalid",
            "Symbolic-link package drops are not supported.",
        ));
    }
    if metadata.is_file() {
        let supported = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "mcpack" | "mcaddon" | "mcworld"
                )
            });
        if !supported {
            return Err(CommandError::new(
                "package_path_invalid",
                "Drop a .mcpack, .mcaddon, .mcworld, or an unpacked Minecraft content folder.",
            ));
        }
    } else if !metadata.is_dir() {
        return Err(CommandError::new(
            "package_path_invalid",
            "The dropped path is not a supported file or folder.",
        ));
    }

    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.inspect_package(&path))
        .await
        .map_err(|error| {
            CommandError::new(
                "package_inspection_task_failed",
                format!("Dropped package inspection task failed: {error}"),
            )
        })?
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
