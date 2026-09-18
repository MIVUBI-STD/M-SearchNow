use super::http::RuntimeHttpHeader;
use super::{
    DownloadSourceRef, DownloadTransport, DownloadTransportFailure, DownloadTransportStream,
    HttpTransport,
};
use crate::{
    error::{BackendError, BackendResult},
    identity::{valid_provider_key, valid_stable_resource_id},
};
use std::{
    collections::HashMap,
    io::{self, Read},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub const RESOLVED_PROVIDER_TRANSPORT_KEY: &str = "provider-resolved";
const MAX_REFRESH_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResourceRef {
    pub provider: String,
    pub resource_id: String,
}

impl ProviderResourceRef {
    pub fn new(provider: impl Into<String>, resource_id: impl Into<String>) -> BackendResult<Self> {
        let reference = Self {
            provider: provider.into(),
            resource_id: resource_id.into(),
        };
        validate_provider_reference(&reference)
            .map_err(|message| BackendError::new("download_provider_reference_invalid", message))?;
        Ok(reference)
    }

    pub fn encode(&self) -> String {
        format!("{}:{}", self.provider, self.resource_id)
    }

    pub fn to_download_source(&self) -> DownloadSourceRef {
        DownloadSourceRef {
            transport: RESOLVED_PROVIDER_TRANSPORT_KEY.to_string(),
            resource_id: self.encode(),
        }
    }

    fn decode(value: &str) -> Result<Self, DownloadTransportFailure> {
        let Some((provider, resource_id)) = value.split_once(':') else {
            return Err(reference_failure());
        };
        let reference = Self {
            provider: provider.to_string(),
            resource_id: resource_id.to_string(),
        };
        validate_provider_reference(&reference).map_err(|_| reference_failure())?;
        Ok(reference)
    }
}

pub fn provider_download_source(
    provider: impl Into<String>,
    resource_id: impl Into<String>,
) -> BackendResult<DownloadSourceRef> {
    Ok(ProviderResourceRef::new(provider, resource_id)?.to_download_source())
}

pub struct ResolvedResource {
    url: String,
    headers: Vec<ResolvedHeader>,
    expires_at_ms: Option<u64>,
}

impl ResolvedResource {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: Vec::new(),
            expires_at_ms: None,
        }
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(ResolvedHeader {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    pub fn with_expiry_ms(mut self, expires_at_ms: u64) -> Self {
        self.expires_at_ms = Some(expires_at_ms);
        self
    }
}

struct ResolvedHeader {
    name: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResolveFailure {
    pub code: String,
    pub retryable: bool,
}

impl ProviderResolveFailure {
    pub fn new(code: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            retryable,
        }
    }
}

pub trait ResourceResolver: Send + Sync {
    fn provider_key(&self) -> &str;

    fn resolve(&self, resource_id: &str) -> Result<ResolvedResource, ProviderResolveFailure>;
}

#[derive(Default)]
pub struct ResourceResolverRegistry {
    resolvers: HashMap<String, Arc<dyn ResourceResolver>>,
}

impl ResourceResolverRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, resolver: Arc<dyn ResourceResolver>) -> BackendResult<()> {
        let key = resolver.provider_key().trim();
        if !valid_provider_key(key) {
            return Err(BackendError::new(
                "download_provider_key_invalid",
                "Provider resolver key is empty or unsupported.",
            ));
        }
        if self.resolvers.contains_key(key) {
            return Err(BackendError::new(
                "download_provider_duplicate",
                "A provider resolver with the same key is already registered.",
            ));
        }
        self.resolvers.insert(key.to_string(), resolver);
        Ok(())
    }

    fn get(&self, key: &str) -> Option<&Arc<dyn ResourceResolver>> {
        self.resolvers.get(key)
    }
}

pub struct ProviderResolvedTransport {
    resolvers: Arc<ResourceResolverRegistry>,
    http: HttpTransport,
}

impl ProviderResolvedTransport {
    pub fn new(resolvers: Arc<ResourceResolverRegistry>, http: HttpTransport) -> Self {
        Self { resolvers, http }
    }

    fn resolve_and_open(
        &self,
        source: &DownloadSourceRef,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        if source.transport != RESOLVED_PROVIDER_TRANSPORT_KEY {
            return Err(DownloadTransportFailure::new(
                "download_provider_transport_key_invalid",
                "Provider transport received a job for a different transport key.",
                false,
            ));
        }

        let reference = ProviderResourceRef::decode(&source.resource_id)?;
        let Some(resolver) = self.resolvers.get(&reference.provider) else {
            return Err(DownloadTransportFailure::new(
                "download_provider_unavailable",
                "No runtime resolver is registered for this provider resource.",
                false,
            ));
        };

        for refresh_attempt in 0..MAX_REFRESH_ATTEMPTS {
            let resolved = resolver
                .resolve(&reference.resource_id)
                .map_err(map_resolver_failure)?;

            if resolved
                .expires_at_ms
                .is_some_and(|expires_at_ms| expires_at_ms <= now_ms())
            {
                if refresh_attempt + 1 < MAX_REFRESH_ATTEMPTS {
                    continue;
                }
                return Err(DownloadTransportFailure::new(
                    "download_provider_resource_expired",
                    "Provider resource resolution returned expired transfer material.",
                    true,
                ));
            }

            let headers = resolved
                .headers
                .into_iter()
                .map(|header| RuntimeHttpHeader::new(header.name, header.value))
                .collect::<Result<Vec<_>, _>>()?;
            let stream = self
                .http
                .open_runtime_request_from(&resolved.url, &headers, offset)?;
            return Ok(sanitize_stream_errors(stream));
        }

        Err(DownloadTransportFailure::new(
            "download_provider_resource_expired",
            "Provider resource resolution returned expired transfer material.",
            true,
        ))
    }
}

impl DownloadTransport for ProviderResolvedTransport {
    fn key(&self) -> &str {
        RESOLVED_PROVIDER_TRANSPORT_KEY
    }

    fn open(
        &self,
        source: &DownloadSourceRef,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        self.resolve_and_open(source, 0)
    }

    fn open_from(
        &self,
        source: &DownloadSourceRef,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        self.resolve_and_open(source, offset)
    }
}

fn validate_provider_reference(reference: &ProviderResourceRef) -> Result<(), &'static str> {
    if !valid_provider_key(&reference.provider) {
        return Err("Provider resource reference contains an invalid provider key.");
    }
    if !valid_stable_resource_id(&reference.resource_id) {
        return Err(
            "Provider resource id must be a stable non-secret opaque identity, not runtime URL material.",
        );
    }
    Ok(())
}

fn reference_failure() -> DownloadTransportFailure {
    DownloadTransportFailure::new(
        "download_provider_reference_invalid",
        "Persisted provider resource reference is invalid.",
        false,
    )
}

fn map_resolver_failure(failure: ProviderResolveFailure) -> DownloadTransportFailure {
    let safe_code = if valid_provider_key(&failure.code) {
        failure.code
    } else {
        "download_provider_resolve_failed".to_string()
    };
    DownloadTransportFailure::new(
        safe_code,
        "Provider could not resolve the requested resource at runtime.",
        failure.retryable,
    )
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn sanitize_stream_errors(stream: DownloadTransportStream) -> DownloadTransportStream {
    DownloadTransportStream {
        reader: Box::new(SanitizedReadErrorReader {
            inner: stream.reader,
        }),
        total_bytes: stream.total_bytes,
    }
}

struct SanitizedReadErrorReader {
    inner: Box<dyn Read + Send>,
}

impl Read for SanitizedReadErrorReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buffer).map_err(|error| {
            io::Error::new(
                error.kind(),
                "Resolved provider transfer read failed without exposing runtime request material.",
            )
        })
    }
}
