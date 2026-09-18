use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DOWNLOAD_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadSourceRef {
    pub transport: String,
    pub resource_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub source: DownloadSourceRef,
    pub display_name: String,
    pub destination_file_name: String,
    pub expected_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadJobState {
    Queued,
    Preparing,
    Transferring,
    PauseRequested,
    Paused,
    Finalizing,
    CancelRequested,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl DownloadJobState {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Preparing
                | Self::Transferring
                | Self::PauseRequested
                | Self::Finalizing
                | Self::CancelRequested
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled | Self::Interrupted)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadJob {
    pub id: String,
    pub source: DownloadSourceRef,
    pub display_name: String,
    pub destination_file_name: String,
    #[serde(default)]
    pub destination_directory: Option<PathBuf>,
    pub state: DownloadJobState,
    pub progress: DownloadProgress,
    #[serde(default)]
    pub expected_sha256: Option<String>,
    pub attempt: u32,
    pub last_error: Option<DownloadFailure>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPolicy {
    pub max_active: usize,
    pub max_jobs: usize,
}

impl Default for DownloadPolicy {
    fn default() -> Self {
        Self {
            max_active: 3,
            max_jobs: 1_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadManagerSnapshot {
    pub policy: DownloadPolicy,
    pub jobs: Vec<DownloadJob>,
    pub active_jobs: usize,
    pub queued_jobs: usize,
    #[serde(default)]
    pub scheduler_error: Option<DownloadFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedDownloadState {
    pub schema_version: u32,
    pub next_sequence: u64,
    pub jobs: Vec<DownloadJob>,
}

impl Default for PersistedDownloadState {
    fn default() -> Self {
        Self {
            schema_version: DOWNLOAD_SCHEMA_VERSION,
            next_sequence: 1,
            jobs: Vec::new(),
        }
    }
}
