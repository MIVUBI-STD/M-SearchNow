use super::error::CommandError;
use searchnow_core::{
    app_runtime::{QueueCatalogDownloadRequest, SearchNowBackendRuntime},
    download::{DownloadJob, DownloadManagerSnapshot, DownloadQueueMove},
};
use tauri::State;

#[tauri::command]
pub fn get_download_snapshot(
    state: State<'_, SearchNowBackendRuntime>,
) -> Result<DownloadManagerSnapshot, CommandError> {
    state.download_snapshot().map_err(CommandError::from)
}

#[tauri::command]
pub fn queue_catalog_download(
    state: State<'_, SearchNowBackendRuntime>,
    request: QueueCatalogDownloadRequest,
) -> Result<DownloadJob, CommandError> {
    state
        .queue_catalog_download(request)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn move_download_in_queue(
    state: State<'_, SearchNowBackendRuntime>,
    job_id: String,
    direction: DownloadQueueMove,
) -> Result<DownloadJob, CommandError> {
    state
        .move_download_in_queue(&job_id, direction)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn pause_download(
    state: State<'_, SearchNowBackendRuntime>,
    job_id: String,
) -> Result<DownloadJob, CommandError> {
    state.pause_download(&job_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn resume_download(
    state: State<'_, SearchNowBackendRuntime>,
    job_id: String,
) -> Result<DownloadJob, CommandError> {
    state.resume_download(&job_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn cancel_download(
    state: State<'_, SearchNowBackendRuntime>,
    job_id: String,
) -> Result<DownloadJob, CommandError> {
    state.cancel_download(&job_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn retry_download(
    state: State<'_, SearchNowBackendRuntime>,
    job_id: String,
) -> Result<DownloadJob, CommandError> {
    state.retry_download(&job_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn remove_download(
    state: State<'_, SearchNowBackendRuntime>,
    job_id: String,
) -> Result<DownloadManagerSnapshot, CommandError> {
    state.remove_download(&job_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn clear_completed_downloads(
    state: State<'_, SearchNowBackendRuntime>,
) -> Result<DownloadManagerSnapshot, CommandError> {
    state.clear_completed_downloads().map_err(CommandError::from)
}
