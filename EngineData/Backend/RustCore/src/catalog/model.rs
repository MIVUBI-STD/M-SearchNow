use crate::{
    download::{provider_download_source, public_https_download_source, DownloadSourceRef},
    error::BackendResult,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum CatalogContentType {
    World,
    Addon,
    ResourcePack,
    Skin,
    Persona,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum CatalogSort {
    #[default]
    Relevance,
    Newest,
    Oldest,
    NameAsc,
    NameDesc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogFilters {
    pub content_types: Vec<CatalogContentType>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogPageRequest {
    pub limit: u16,
    pub cursor: Option<String>,
}

impl Default for CatalogPageRequest {
    fn default() -> Self {
        Self {
            limit: 30,
            cursor: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogQuery {
    pub text: Option<String>,
    pub filters: CatalogFilters,
    pub sort: CatalogSort,
    pub page: CatalogPageRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRequest {
    pub provider: String,
    pub query: CatalogQuery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum CatalogDownloadRef {
    PublicHttps {
        url: String,
    },
    ProviderResolved {
        provider: String,
        resource_id: String,
    },
}

impl CatalogDownloadRef {
    pub fn to_download_source(&self) -> BackendResult<DownloadSourceRef> {
        match self {
            Self::PublicHttps { url } => public_https_download_source(url.clone()),
            Self::ProviderResolved {
                provider,
                resource_id,
            } => provider_download_source(provider.clone(), resource_id.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub provider: String,
    pub item_id: String,
    pub title: String,
    pub creator_name: Option<String>,
    pub thumbnail_url: Option<String>,
    pub description: Option<String>,
    pub content_type: CatalogContentType,
    pub tags: Vec<String>,
    pub published_at_ms: Option<u64>,
    pub updated_at_ms: Option<u64>,
    pub file_name: Option<String>,
    pub expected_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
    pub download: Option<CatalogDownloadRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    pub provider: String,
    pub items: Vec<CatalogItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl CatalogError {
    pub(crate) fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDownloadMetadata {
    pub file_name: String,
    pub expected_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
}

impl CatalogDownloadMetadata {
    pub fn new(file_name: impl Into<String>, expected_bytes: Option<u64>) -> Self {
        Self {
            file_name: file_name.into(),
            expected_bytes,
            expected_sha256: None,
        }
    }

    pub fn with_sha256(mut self, expected_sha256: impl Into<String>) -> Self {
        self.expected_sha256 = Some(expected_sha256.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProviderItem {
    pub item_id: String,
    pub title: String,
    pub creator_name: Option<String>,
    pub thumbnail_url: Option<String>,
    pub description: Option<String>,
    pub content_type: CatalogContentType,
    pub tags: Vec<String>,
    pub published_at_ms: Option<u64>,
    pub updated_at_ms: Option<u64>,
    pub download: Option<CatalogDownloadRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProviderPage {
    pub items: Vec<CatalogProviderItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProviderFailure {
    pub code: String,
    pub retryable: bool,
}

impl CatalogProviderFailure {
    pub fn new(code: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            retryable,
        }
    }
}
