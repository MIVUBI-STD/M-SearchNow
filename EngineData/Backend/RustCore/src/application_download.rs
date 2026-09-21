use crate::{
    catalog::CatalogDownloadRef,
    diagnostics::{DiagnosticComponent, DiagnosticSeverity, DiagnosticsBuffer},
    download::{
        DownloadExecutionRuntime, DownloadJob, DownloadJobState, DownloadManagerSnapshot,
        DownloadQueueMove, DownloadRequest,
    },
    error::{BackendError, BackendResult},
};
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc, time::Instant};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueueCatalogDownloadRequest {
    pub download: CatalogDownloadRef,
    pub display_name: String,
    pub destination_file_name: String,
    pub destination_directory: Option<PathBuf>,
    pub expected_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
}

#[derive(Clone)]
pub(crate) struct DownloadApplicationService {
    runtime: DownloadExecutionRuntime,
    diagnostics: DiagnosticsBuffer,
}

impl DownloadApplicationService {
    pub(crate) fn new(runtime: DownloadExecutionRuntime, diagnostics: DiagnosticsBuffer) -> Self {
        Self {
            runtime,
            diagnostics,
        }
    }

    pub(crate) fn set_bandwidth_limit(&self, limit: Option<u64>) {
        self.runtime.set_bandwidth_limit(limit);
    }

    pub(crate) fn download_snapshot(&self) -> BackendResult<DownloadManagerSnapshot> {
        self.runtime.snapshot()
    }

    pub(crate) fn set_download_change_notifier<F>(&self, notifier: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.runtime.set_change_notifier(Arc::new(notifier));
    }

    pub(crate) fn queue_catalog_download(
        &self,
        request: QueueCatalogDownloadRequest,
    ) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = (|| {
            let source = request.download.to_download_source()?;
            self.runtime.queue_to(
                DownloadRequest {
                    source,
                    display_name: request.display_name,
                    destination_file_name: request.destination_file_name,
                    expected_bytes: request.expected_bytes,
                    expected_sha256: request.expected_sha256,
                },
                request.destination_directory,
            )
        })();
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_queue_ok",
            "Catalog download job queued.",
            "download_queue_failed",
            "Catalog download job could not be queued.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub(crate) fn completed_download_directory(&self, job_id: &str) -> BackendResult<PathBuf> {
        let snapshot = self.runtime.snapshot()?;
        let job = snapshot
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .ok_or_else(|| {
                BackendError::new("download_job_not_found", "Download job was not found.")
            })?;
        if job.state != DownloadJobState::Completed {
            return Err(BackendError::new(
                "download_directory_not_ready",
                "The download folder is available only after the download completes.",
            ));
        }
        job.destination_directory.clone().ok_or_else(|| {
            BackendError::new(
                "download_directory_unavailable",
                "This download does not have a user-selected destination folder.",
            )
        })
    }

    pub(crate) fn move_download_in_queue(
        &self,
        job_id: &str,
        direction: DownloadQueueMove,
    ) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = self.runtime.move_queued(job_id, direction);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_queue_move_ok",
            "Download queue order updated.",
            "download_queue_move_failed",
            "Download queue order could not be updated.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub(crate) fn pause_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = self.runtime.pause(job_id);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_pause_ok",
            "Download pause requested.",
            "download_pause_failed",
            "Download could not be paused.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub(crate) fn resume_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = self.runtime.resume(job_id);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_resume_ok",
            "Download resume queued.",
            "download_resume_failed",
            "Download could not be resumed.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub(crate) fn cancel_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = self.runtime.cancel(job_id);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_cancel_ok",
            "Download cancellation requested.",
            "download_cancel_failed",
            "Download cancellation could not be requested.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub(crate) fn retry_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = self.runtime.retry(job_id);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_retry_ok",
            "Download retry queued.",
            "download_retry_failed",
            "Download retry could not be queued.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub(crate) fn remove_download(&self, job_id: &str) -> BackendResult<DownloadManagerSnapshot> {
        let started = Instant::now();
        let result = self.runtime.remove_terminal(job_id);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_remove_ok",
            "Terminal download job removed.",
            "download_remove_failed",
            "Terminal download job could not be removed.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub(crate) fn clear_completed_downloads(&self) -> BackendResult<DownloadManagerSnapshot> {
        let started = Instant::now();
        let result = self.runtime.remove_completed();
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_clear_completed_ok",
            "Completed download jobs cleared.",
            "download_clear_completed_failed",
            "Completed download jobs could not be cleared.",
            DiagnosticSeverity::Warning,
        );
        result
    }
}
