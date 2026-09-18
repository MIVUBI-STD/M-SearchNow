use super::*;
use crate::download::{PUBLIC_HTTPS_TRANSPORT_KEY, RESOLVED_PROVIDER_TRANSPORT_KEY};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

struct FakeCatalogProvider {
    calls: AtomicUsize,
    items: Vec<CatalogProviderItem>,
}

impl FakeCatalogProvider {
    fn new(items: Vec<CatalogProviderItem>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            items,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl CatalogProvider for FakeCatalogProvider {
    fn key(&self) -> &str {
        "fake"
    }

    fn query(&self, query: &CatalogQuery) -> Result<CatalogProviderPage, CatalogProviderFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut items = self
            .items
            .iter()
            .filter(|item| matches_query(item, query))
            .cloned()
            .collect::<Vec<_>>();
        apply_sort(&mut items, query.sort);

        let start = query
            .page
            .cursor
            .as_deref()
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|_| CatalogProviderFailure::new("fake_cursor_invalid", false))?;
        let end = (start + query.page.limit as usize).min(items.len());
        let page_items = if start < items.len() {
            items[start..end].to_vec()
        } else {
            Vec::new()
        };
        let next_cursor = (end < items.len()).then(|| end.to_string());
        Ok(CatalogProviderPage {
            items: page_items,
            next_cursor,
        })
    }
}

fn matches_query(item: &CatalogProviderItem, query: &CatalogQuery) -> bool {
    if let Some(text) = &query.text {
        let needle = text.to_ascii_lowercase();
        let title_match = item.title.to_ascii_lowercase().contains(&needle);
        let creator_match = item
            .creator_name
            .as_ref()
            .is_some_and(|value| value.to_ascii_lowercase().contains(&needle));
        let description_match = item
            .description
            .as_ref()
            .is_some_and(|value| value.to_ascii_lowercase().contains(&needle));
        if !title_match && !creator_match && !description_match {
            return false;
        }
    }

    if !query.filters.content_types.is_empty()
        && !query.filters.content_types.contains(&item.content_type)
    {
        return false;
    }
    let item_tags = item.tags.iter().collect::<HashSet<_>>();
    query.filters.tags.iter().all(|required| {
        item_tags
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case(required))
    })
}

fn apply_sort(items: &mut [CatalogProviderItem], sort: CatalogSort) {
    match sort {
        CatalogSort::Relevance => {}
        CatalogSort::Newest => items.sort_by_key(|item| std::cmp::Reverse(item.published_at_ms)),
        CatalogSort::Oldest => items.sort_by_key(|item| item.published_at_ms),
        CatalogSort::NameAsc => {
            items.sort_by_key(|item| item.title.to_ascii_lowercase());
        }
        CatalogSort::NameDesc => {
            items.sort_by_key(|item| std::cmp::Reverse(item.title.to_ascii_lowercase()));
        }
    }
}

fn item(
    id: &str,
    title: &str,
    content_type: CatalogContentType,
    tags: &[&str],
    published_at_ms: u64,
) -> CatalogProviderItem {
    CatalogProviderItem {
        item_id: id.to_string(),
        title: title.to_string(),
        creator_name: Some("Example Creator".to_string()),
        thumbnail_url: Some(format!("https://cdn.example.com/{id}.webp")),
        description: Some(format!("{title} description")),
        content_type,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        published_at_ms: Some(published_at_ms),
        updated_at_ms: Some(published_at_ms),
        download: Some(CatalogDownloadRef::ProviderResolved {
            provider: "fake".to_string(),
            resource_id: format!("resource-{id}"),
        }),
    }
}

fn service(provider: Arc<dyn CatalogProvider>) -> CatalogService {
    let mut registry = CatalogProviderRegistry::new();
    registry.register(provider).expect("register provider");
    CatalogService::new(Arc::new(registry))
}

#[test]
fn query_validation_happens_before_provider_call() {
    let provider = Arc::new(FakeCatalogProvider::new(Vec::new()));
    let catalog = service(provider.clone());
    let error = catalog
        .query(&CatalogRequest {
            provider: "fake".into(),
            query: CatalogQuery {
                page: CatalogPageRequest {
                    limit: 101,
                    cursor: None,
                },
                ..CatalogQuery::default()
            },
        })
        .expect_err("oversized page must fail");
    assert_eq!(error.code, "catalog_query_invalid");
    assert_eq!(provider.calls(), 0);
}

#[test]
fn fake_provider_supports_filter_sort_and_cursor_pagination() {
    let provider = Arc::new(FakeCatalogProvider::new(vec![
        item(
            "gamma",
            "Gamma Addon",
            CatalogContentType::Addon,
            &["tools"],
            30,
        ),
        item(
            "alpha",
            "Alpha Addon",
            CatalogContentType::Addon,
            &["tools"],
            10,
        ),
        item(
            "beta",
            "Beta Resource",
            CatalogContentType::ResourcePack,
            &["visual"],
            20,
        ),
    ]));
    let catalog = service(provider);
    let base_query = CatalogQuery {
        text: Some("addon".into()),
        filters: CatalogFilters {
            content_types: vec![CatalogContentType::Addon],
            tags: vec!["tools".into()],
        },
        sort: CatalogSort::NameAsc,
        page: CatalogPageRequest {
            limit: 1,
            cursor: None,
        },
    };

    let first = catalog
        .query(&CatalogRequest {
            provider: "fake".into(),
            query: base_query.clone(),
        })
        .expect("first page");
    assert_eq!(first.items[0].item_id, "alpha");
    assert_eq!(
        first.items[0].creator_name.as_deref(),
        Some("Example Creator")
    );
    assert_eq!(
        first.items[0].thumbnail_url.as_deref(),
        Some("https://cdn.example.com/alpha.webp")
    );
    assert_eq!(first.next_cursor.as_deref(), Some("1"));

    let second = catalog
        .query(&CatalogRequest {
            provider: "fake".into(),
            query: CatalogQuery {
                page: CatalogPageRequest {
                    limit: 1,
                    cursor: first.next_cursor,
                },
                ..base_query
            },
        })
        .expect("second page");
    assert_eq!(second.items[0].item_id, "gamma");
    assert!(second.next_cursor.is_none());
}

#[test]
fn creator_text_participates_in_search() {
    let provider = Arc::new(FakeCatalogProvider::new(vec![item(
        "creator-item",
        "Unrelated title",
        CatalogContentType::Addon,
        &[],
        10,
    )]));
    let catalog = service(provider);
    let page = catalog
        .query(&CatalogRequest {
            provider: "fake".into(),
            query: CatalogQuery {
                text: Some("example creator".into()),
                ..CatalogQuery::default()
            },
        })
        .expect("creator search");
    assert_eq!(page.items.len(), 1);
}

#[test]
fn missing_provider_is_explicit_and_non_retryable() {
    let catalog = CatalogService::new(Arc::new(CatalogProviderRegistry::new()));
    let error = catalog
        .query(&CatalogRequest {
            provider: "missing".into(),
            query: CatalogQuery::default(),
        })
        .expect_err("missing provider");
    assert_eq!(error.code, "catalog_provider_unavailable");
    assert!(!error.retryable);
}

struct MalformedProvider;

impl CatalogProvider for MalformedProvider {
    fn key(&self) -> &str {
        "bad"
    }

    fn query(&self, _query: &CatalogQuery) -> Result<CatalogProviderPage, CatalogProviderFailure> {
        Ok(CatalogProviderPage {
            items: vec![CatalogProviderItem {
                item_id: "bad-item".into(),
                title: "x".repeat(300),
                creator_name: None,
                thumbnail_url: None,
                description: None,
                content_type: CatalogContentType::Other,
                tags: Vec::new(),
                published_at_ms: None,
                updated_at_ms: None,
                download: None,
            }],
            next_cursor: None,
        })
    }
}

#[test]
fn malformed_provider_data_fails_closed() {
    let catalog = service(Arc::new(MalformedProvider));
    let error = catalog
        .query(&CatalogRequest {
            provider: "bad".into(),
            query: CatalogQuery::default(),
        })
        .expect_err("malformed item");
    assert_eq!(error.code, "catalog_provider_data_invalid");
    assert!(!error.retryable);
}

struct UnsafeThumbnailProvider;

impl CatalogProvider for UnsafeThumbnailProvider {
    fn key(&self) -> &str {
        "unsafe-thumbnail"
    }

    fn query(&self, _query: &CatalogQuery) -> Result<CatalogProviderPage, CatalogProviderFailure> {
        Ok(CatalogProviderPage {
            items: vec![CatalogProviderItem {
                item_id: "unsafe".into(),
                title: "Unsafe thumbnail".into(),
                creator_name: Some("Example Creator".into()),
                thumbnail_url: Some("http://example.com/image.png".into()),
                description: None,
                content_type: CatalogContentType::Other,
                tags: Vec::new(),
                published_at_ms: None,
                updated_at_ms: None,
                download: None,
            }],
            next_cursor: None,
        })
    }
}

#[test]
fn thumbnail_must_be_public_https() {
    let catalog = service(Arc::new(UnsafeThumbnailProvider));
    let error = catalog
        .query(&CatalogRequest {
            provider: "unsafe-thumbnail".into(),
            query: CatalogQuery::default(),
        })
        .expect_err("unsafe thumbnail must fail");
    assert_eq!(error.code, "catalog_provider_data_invalid");
}

struct SecretFailureProvider;

impl CatalogProvider for SecretFailureProvider {
    fn key(&self) -> &str {
        "secret-failure"
    }

    fn query(&self, _query: &CatalogQuery) -> Result<CatalogProviderPage, CatalogProviderFailure> {
        Err(CatalogProviderFailure::new(
            "token=runtime-secret-must-not-leak",
            true,
        ))
    }
}

#[test]
fn provider_failure_is_normalized_without_secret_text() {
    let catalog = service(Arc::new(SecretFailureProvider));
    let error = catalog
        .query(&CatalogRequest {
            provider: "secret-failure".into(),
            query: CatalogQuery::default(),
        })
        .expect_err("provider failure");
    assert_eq!(error.code, "catalog_provider_query_failed");
    assert!(error.retryable);
    assert!(!error.message.contains("runtime-secret"));
}

#[test]
fn catalog_download_refs_map_only_to_existing_download_boundaries() {
    let provider_source = CatalogDownloadRef::ProviderResolved {
        provider: "fake".into(),
        resource_id: "catalog-item-42".into(),
    }
    .to_download_source()
    .expect("provider download source");
    assert_eq!(provider_source.transport, RESOLVED_PROVIDER_TRANSPORT_KEY);
    assert_eq!(provider_source.resource_id, "fake:catalog-item-42");

    let public_source = CatalogDownloadRef::PublicHttps {
        url: "https://cdn.example.com/public.mcpack".into(),
    }
    .to_download_source()
    .expect("public download source");
    assert_eq!(public_source.transport, PUBLIC_HTTPS_TRANSPORT_KEY);

    let signed_public = CatalogDownloadRef::PublicHttps {
        url: "https://cdn.example.com/public.mcpack?token=secret".into(),
    };
    assert!(signed_public.to_download_source().is_err());
}

#[test]
fn unknown_catalog_request_fields_fail_closed() {
    let json = r#"{
        "provider": "fake",
        "query": {
            "text": null,
            "filters": {
                "contentTypes": [],
                "tags": [],
                "unexpectedFilter": true
            },
            "sort": "relevance",
            "page": {
                "limit": 30,
                "cursor": null
            }
        }
    }"#;
    assert!(serde_json::from_str::<CatalogRequest>(json).is_err());
}
