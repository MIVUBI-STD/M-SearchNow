use super::model::DownloadSourceRef;
use crate::error::{BackendError, BackendResult};
use std::{collections::HashMap, io::Read, sync::Arc};
#[cfg(test)]
use std::{
    fs::File,
    io::{Seek, SeekFrom},
    path::Path,
};

pub struct DownloadTransportStream {
    pub reader: Box<dyn Read + Send>,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadTransportFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl DownloadTransportFailure {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

pub trait DownloadTransport: Send + Sync {
    fn key(&self) -> &str;

    fn open(
        &self,
        source: &DownloadSourceRef,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure>;

    fn open_from(
        &self,
        source: &DownloadSourceRef,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        if offset == 0 {
            self.open(source)
        } else {
            Err(DownloadTransportFailure::new(
                "download_resume_unsupported",
                "This download source does not support resuming a partial transfer.",
                false,
            ))
        }
    }
}

#[derive(Default)]
pub struct DownloadTransportRegistry {
    transports: HashMap<String, Arc<dyn DownloadTransport>>,
}

impl DownloadTransportRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn with_local_file() -> BackendResult<Self> {
        let mut registry = Self::new();
        registry.register(Arc::new(LocalFileTransport))?;
        Ok(registry)
    }

    pub fn register(&mut self, transport: Arc<dyn DownloadTransport>) -> BackendResult<()> {
        let key = transport.key().trim();
        if key.is_empty()
            || key.len() > 64
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(BackendError::new(
                "download_transport_key_invalid",
                "Download transport key is empty or unsupported.",
            ));
        }

        if self.transports.contains_key(key) {
            return Err(BackendError::new(
                "download_transport_duplicate",
                "A download transport with the same key is already registered.",
            ));
        }
        self.transports.insert(key.to_string(), transport);
        Ok(())
    }

    pub fn open_from(
        &self,
        source: &DownloadSourceRef,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        let Some(transport) = self.transports.get(&source.transport) else {
            return Err(DownloadTransportFailure::new(
                "download_transport_unavailable",
                format!(
                    "Download transport '{}' is not available in this SearchNow build.",
                    source.transport
                ),
                false,
            ));
        };
        transport.open_from(source, offset)
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalFileTransport;

#[cfg(test)]
impl DownloadTransport for LocalFileTransport {
    fn key(&self) -> &str {
        "local-file"
    }

    fn open(
        &self,
        source: &DownloadSourceRef,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        self.open_from(source, 0)
    }

    fn open_from(
        &self,
        source: &DownloadSourceRef,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        let path = Path::new(&source.resource_id);
        let metadata = std::fs::symlink_metadata(path).map_err(|error| {
            DownloadTransportFailure::new(
                "download_local_source_metadata_failed",
                format!("SearchNow could not inspect the local download source: {error}"),
                true,
            )
        })?;

        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(DownloadTransportFailure::new(
                "download_local_source_invalid",
                "Local download source must be a regular non-symlink file.",
                false,
            ));
        }

        if offset > metadata.len() {
            return Err(DownloadTransportFailure::new(
                "download_resume_offset_invalid",
                "Partial download offset exceeds the source size.",
                false,
            ));
        }

        let mut reader = File::open(path).map_err(|error| {
            DownloadTransportFailure::new(
                "download_local_source_open_failed",
                format!("SearchNow could not open the local download source: {error}"),
                true,
            )
        })?;

        reader.seek(SeekFrom::Start(offset)).map_err(|error| {
            DownloadTransportFailure::new(
                "download_local_source_seek_failed",
                format!("SearchNow could not seek the local download source: {error}"),
                true,
            )
        })?;

        Ok(DownloadTransportStream {
            reader: Box::new(reader),
            total_bytes: Some(metadata.len()),
        })
    }
}
