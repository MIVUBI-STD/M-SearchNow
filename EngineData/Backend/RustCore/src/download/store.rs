use super::model::{PersistedDownloadState, DOWNLOAD_SCHEMA_VERSION};
use crate::{
    error::{BackendError, BackendResult},
    storage::AtomicFileStore,
};
use std::path::PathBuf;

const MAX_DOWNLOAD_STATE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct DownloadStore {
    file: AtomicFileStore,
}

impl DownloadStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            file: AtomicFileStore::new(path, ".downloads.backup", ".downloads"),
        }
    }

    pub fn load(&self) -> BackendResult<PersistedDownloadState> {
        let Some(text) = self.file.read_to_string(MAX_DOWNLOAD_STATE_BYTES)? else {
            return Ok(PersistedDownloadState::default());
        };
        let state: PersistedDownloadState = serde_json::from_str(&text).map_err(|error| {
            BackendError::new(
                "download_state_invalid_json",
                format!("SearchNow download state is invalid: {error}"),
            )
        })?;
        if state.schema_version != DOWNLOAD_SCHEMA_VERSION {
            return Err(BackendError::new(
                "download_state_schema_unsupported",
                format!(
                    "Download state schema {} is not supported by this SearchNow build.",
                    state.schema_version
                ),
            ));
        }
        Ok(state)
    }

    pub fn save(&self, state: &PersistedDownloadState) -> BackendResult<()> {
        if state.schema_version != DOWNLOAD_SCHEMA_VERSION {
            return Err(BackendError::new(
                "download_state_schema_unsupported",
                "SearchNow refused to save an unsupported download state schema.",
            ));
        }
        let bytes = serde_json::to_vec_pretty(state).map_err(|error| {
            BackendError::new(
                "download_state_serialize_failed",
                format!("SearchNow could not serialize download state: {error}"),
            )
        })?;
        self.file.replace(&bytes, MAX_DOWNLOAD_STATE_BYTES)
    }
}
