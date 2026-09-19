use crate::{
    catalog::{
        CatalogContentType, CatalogDownloadMetadata, CatalogDownloadRef, CatalogProvider,
        CatalogProviderFailure, CatalogProviderItem, CatalogProviderPage, CatalogQuery,
        CatalogSort,
    },
    download::{ProviderResolveFailure, ResolvedResource, ResourceResolver},
    error::{BackendError, BackendResult},
    provider_adapter::IntegratedProvider,
    provider_session::{
        ProviderSessionFailure, ProviderSessionLease, ProviderSessionManager,
        ProviderSessionMaterial, ProviderSessionSource,
    },
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use url::Url;

const PROVIDER_KEY: &str = "curseforge";
const API_BASE: &str = "https://api.curseforge.com";
const BEDROCK_GAME_SLUG: &str = "minecraft-bedrock";
const MAX_API_KEY_BYTES: usize = 4096;
const API_PAGE_SIZE: u32 = 50;
const MAX_API_INDEX: u32 = 10_000;

#[derive(Clone)]
struct CurseForgeSession {
    api_key: String,
}

struct CurseForgeSessionSource {
    api_key: String,
}

impl ProviderSessionSource for CurseForgeSessionSource {
    fn provider_key(&self) -> &str {
        PROVIDER_KEY
    }

    fn acquire(&self) -> Result<ProviderSessionMaterial, ProviderSessionFailure> {
        Ok(ProviderSessionMaterial::new(
            CurseForgeSession {
                api_key: self.api_key.clone(),
            },
            None,
        ))
    }

    fn refresh(
        &self,
        _current: &ProviderSessionLease,
    ) -> Result<ProviderSessionMaterial, ProviderSessionFailure> {
        self.acquire()
    }
}

struct CurseForgeClient {
    base_url: String,
    bedrock_game_id: Mutex<Option<u32>>,
}

impl CurseForgeClient {
    fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            bedrock_game_id: Mutex::new(None),
        }
    }

    fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        session: &CurseForgeSession,
    ) -> Result<T, ProviderHttpFailure> {
        let url = format!("{}{}", self.base_url, path);
        let response = ureq::get(&url)
            .set("Accept", "application/json")
            .set("x-api-key", &session.api_key)
            .call()
            .map_err(map_http_error)?;
        serde_json::from_reader(response.into_reader()).map_err(|_| ProviderHttpFailure {
            code: "curseforge_response_invalid",
            retryable: false,
        })
    }

    fn bedrock_game_id(&self, session: &CurseForgeSession) -> Result<u32, ProviderHttpFailure> {
        if let Some(cached) = *self
            .bedrock_game_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
        {
            return Ok(cached);
        }

        let mut index = 0_u32;
        loop {
            let page: ApiResponse<Vec<ApiGame>> = self.get_json(
                &format!("/v1/games?index={index}&pageSize={API_PAGE_SIZE}"),
                session,
            )?;
            if let Some(game) = page
                .data
                .into_iter()
                .find(|game| game.slug == BEDROCK_GAME_SLUG)
            {
                *self
                    .bedrock_game_id
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(game.id);
                return Ok(game.id);
            }
            let pagination = page.pagination.unwrap_or_default();
            if pagination.result_count == 0
                || index.saturating_add(pagination.result_count) >= pagination.total_count
                || index.saturating_add(API_PAGE_SIZE) >= MAX_API_INDEX
            {
                return Err(ProviderHttpFailure {
                    code: "curseforge_bedrock_game_unavailable",
                    retryable: false,
                });
            }
            index = index.saturating_add(API_PAGE_SIZE);
        }
    }

    fn search(
        &self,
        query: &CatalogQuery,
        session: &CurseForgeSession,
    ) -> Result<ApiSearchResponse, ProviderHttpFailure> {
        let game_id = self.bedrock_game_id(session)?;
        let index = query
            .page
            .cursor
            .as_deref()
            .map(parse_cursor)
            .transpose()?
            .unwrap_or(0);
        let mut url = Url::parse(&format!("{}/v1/mods/search", self.base_url)).map_err(|_| {
            ProviderHttpFailure {
                code: "curseforge_request_invalid",
                retryable: false,
            }
        })?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("gameId", &game_id.to_string());
            pairs.append_pair("index", &index.to_string());
            pairs.append_pair("pageSize", &API_PAGE_SIZE.to_string());
            if let Some(text) = query.text.as_deref() {
                pairs.append_pair("searchFilter", text);
            }
            match query.sort {
                CatalogSort::Relevance => {}
                CatalogSort::Newest => {
                    pairs.append_pair("sortField", "3");
                    pairs.append_pair("sortOrder", "desc");
                }
                CatalogSort::Oldest => {
                    pairs.append_pair("sortField", "11");
                    pairs.append_pair("sortOrder", "asc");
                }
                CatalogSort::NameAsc => {
                    pairs.append_pair("sortField", "4");
                    pairs.append_pair("sortOrder", "asc");
                }
                CatalogSort::NameDesc => {
                    pairs.append_pair("sortField", "4");
                    pairs.append_pair("sortOrder", "desc");
                }
            }
        }
        let path = format!(
            "{}{}",
            url.path(),
            url.query()
                .map(|query| format!("?{query}"))
                .unwrap_or_default()
        );
        self.get_json(&path, session)
    }

    fn download_url(
        &self,
        mod_id: u64,
        file_id: u64,
        session: &CurseForgeSession,
    ) -> Result<String, ProviderHttpFailure> {
        let response: ApiResponse<String> = self.get_json(
            &format!("/v1/mods/{mod_id}/files/{file_id}/download-url"),
            session,
        )?;
        validate_download_url(&response.data)?;
        Ok(response.data)
    }
}

pub struct CurseForgeProvider {
    session_source: Arc<CurseForgeSessionSource>,
    client: Arc<CurseForgeClient>,
}

impl CurseForgeProvider {
    pub fn new(api_key: impl Into<String>) -> BackendResult<Self> {
        Self::new_with_base_url(api_key, API_BASE)
    }

    fn new_with_base_url(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> BackendResult<Self> {
        let api_key = api_key.into();
        if api_key.is_empty()
            || api_key.len() > MAX_API_KEY_BYTES
            || api_key.chars().any(char::is_control)
        {
            return Err(BackendError::new(
                "curseforge_api_key_invalid",
                "CurseForge API key is empty or unsupported.",
            ));
        }
        let base_url = base_url.into();
        let parsed = Url::parse(&base_url).map_err(|_| {
            BackendError::new(
                "curseforge_api_base_invalid",
                "CurseForge API base URL is invalid.",
            )
        })?;
        if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
            return Err(BackendError::new(
                "curseforge_api_base_invalid",
                "CurseForge API base URL is invalid.",
            ));
        }

        Ok(Self {
            session_source: Arc::new(CurseForgeSessionSource { api_key }),
            client: Arc::new(CurseForgeClient::new(base_url)),
        })
    }
}

impl IntegratedProvider for CurseForgeProvider {
    fn provider_key(&self) -> &str {
        PROVIDER_KEY
    }

    fn session_source(&self) -> Option<Arc<dyn ProviderSessionSource>> {
        Some(self.session_source.clone())
    }

    fn catalog_provider(
        &self,
        sessions: Arc<ProviderSessionManager>,
    ) -> BackendResult<Option<Arc<dyn CatalogProvider>>> {
        Ok(Some(Arc::new(CurseForgeCatalog {
            sessions,
            client: self.client.clone(),
            metadata: Mutex::new(HashMap::new()),
        })))
    }

    fn resource_resolver(
        &self,
        sessions: Arc<ProviderSessionManager>,
    ) -> BackendResult<Option<Arc<dyn ResourceResolver>>> {
        Ok(Some(Arc::new(CurseForgeResolver {
            sessions,
            client: self.client.clone(),
        })))
    }
}

struct CurseForgeCatalog {
    sessions: Arc<ProviderSessionManager>,
    client: Arc<CurseForgeClient>,
    metadata: Mutex<HashMap<String, CatalogDownloadMetadata>>,
}

impl CatalogProvider for CurseForgeCatalog {
    fn key(&self) -> &str {
        PROVIDER_KEY
    }

    fn query(&self, query: &CatalogQuery) -> Result<CatalogProviderPage, CatalogProviderFailure> {
        let session = acquire_session(&self.sessions).map_err(to_catalog_failure)?;
        let response = self
            .client
            .search(query, &session)
            .map_err(to_catalog_http_failure)?;

        let mut metadata = self
            .metadata
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        metadata.clear();

        let mut items = Vec::with_capacity(query.page.limit as usize);
        for project in response.data {
            if !project.is_available {
                continue;
            }
            let content_type = project_content_type(&project);
            if !query.filters.content_types.is_empty()
                && !query.filters.content_types.contains(&content_type)
            {
                continue;
            }
            let tags = project_tags(&project);
            if !query.filters.tags.is_empty()
                && !query.filters.tags.iter().all(|required| {
                    tags.iter()
                        .any(|tag| tag.eq_ignore_ascii_case(required.as_str()))
                })
            {
                continue;
            }

            let package = downloadable_file(&project);
            let download = package
                .as_ref()
                .map(|file| CatalogDownloadRef::ProviderResolved {
                    provider: PROVIDER_KEY.to_string(),
                    resource_id: encode_resource_id(project.id, file.id),
                });
            if let Some(file) = package {
                metadata.insert(
                    project.id.to_string(),
                    CatalogDownloadMetadata::new(file.file_name.clone(), Some(file.file_length)),
                );
            }

            items.push(CatalogProviderItem {
                item_id: project.id.to_string(),
                title: project.name,
                creator_name: project.authors.first().map(|author| author.name.clone()),
                thumbnail_url: project.logo.and_then(|logo| logo.thumbnail_url),
                description: nonempty(project.summary),
                content_type,
                tags,
                published_at_ms: None,
                updated_at_ms: None,
                download,
            });
            if items.len() >= query.page.limit as usize {
                break;
            }
        }

        let next_cursor = response.pagination.and_then(|page| {
            let next = page.index.saturating_add(page.result_count);
            (page.result_count > 0 && next < page.total_count && next < MAX_API_INDEX)
                .then(|| next.to_string())
        });

        Ok(CatalogProviderPage { items, next_cursor })
    }

    fn download_metadata(&self, item_id: &str) -> Option<CatalogDownloadMetadata> {
        self.metadata
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(item_id)
            .cloned()
    }
}

struct CurseForgeResolver {
    sessions: Arc<ProviderSessionManager>,
    client: Arc<CurseForgeClient>,
}

impl ResourceResolver for CurseForgeResolver {
    fn provider_key(&self) -> &str {
        PROVIDER_KEY
    }

    fn resolve(&self, resource_id: &str) -> Result<ResolvedResource, ProviderResolveFailure> {
        let (mod_id, file_id) = decode_resource_id(resource_id)?;
        let session = acquire_session(&self.sessions).map_err(to_resolver_failure)?;
        let url = self
            .client
            .download_url(mod_id, file_id, &session)
            .map_err(to_resolver_http_failure)?;
        Ok(ResolvedResource::new(url))
    }
}

fn acquire_session(
    sessions: &ProviderSessionManager,
) -> Result<Arc<CurseForgeSession>, crate::provider_session::ProviderSessionError> {
    sessions
        .acquire(PROVIDER_KEY)?
        .downcast::<CurseForgeSession>()
}

fn project_content_type(project: &ApiProject) -> CatalogContentType {
    let class_slug = project
        .categories
        .iter()
        .find(|category| category.is_class)
        .map(|category| category.slug.as_str())
        .unwrap_or_default();
    match class_slug {
        "addons" | "scripts" => CatalogContentType::Addon,
        "maps" => CatalogContentType::World,
        "texture-packs" | "resource-packs" => CatalogContentType::ResourcePack,
        "skins" => CatalogContentType::Skin,
        _ => CatalogContentType::Other,
    }
}

fn project_tags(project: &ApiProject) -> Vec<String> {
    let mut tags = Vec::new();
    for category in &project.categories {
        if category.is_class {
            continue;
        }
        let tag = if category.slug.is_empty() {
            category.name.clone()
        } else {
            category.slug.clone()
        };
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
        if tags.len() >= 32 {
            break;
        }
    }
    tags
}

fn downloadable_file(project: &ApiProject) -> Option<&ApiFile> {
    if project.allow_mod_distribution == Some(false) {
        return None;
    }
    project
        .latest_files
        .iter()
        .filter(|file| file.is_available && file.is_early_access_content != Some(true))
        .filter(|file| valid_bedrock_package_name(&file.file_name))
        .max_by_key(|file| (file.id == project.main_file_id, file.id))
}

fn valid_bedrock_package_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".mcpack") || lower.ends_with(".mcaddon") || lower.ends_with(".mcworld")
}

fn encode_resource_id(mod_id: u64, file_id: u64) -> String {
    format!("{mod_id}-{file_id}")
}

fn decode_resource_id(resource_id: &str) -> Result<(u64, u64), ProviderResolveFailure> {
    let Some((mod_id, file_id)) = resource_id.split_once('-') else {
        return Err(ProviderResolveFailure::new(
            "curseforge_resource_invalid",
            false,
        ));
    };
    let mod_id = mod_id
        .parse::<u64>()
        .map_err(|_| ProviderResolveFailure::new("curseforge_resource_invalid", false))?;
    let file_id = file_id
        .parse::<u64>()
        .map_err(|_| ProviderResolveFailure::new("curseforge_resource_invalid", false))?;
    Ok((mod_id, file_id))
}

fn parse_cursor(cursor: &str) -> Result<u32, ProviderHttpFailure> {
    let value = cursor.parse::<u32>().map_err(|_| ProviderHttpFailure {
        code: "curseforge_cursor_invalid",
        retryable: false,
    })?;
    if value >= MAX_API_INDEX {
        return Err(ProviderHttpFailure {
            code: "curseforge_cursor_invalid",
            retryable: false,
        });
    }
    Ok(value)
}

fn validate_download_url(value: &str) -> Result<(), ProviderHttpFailure> {
    let url = Url::parse(value).map_err(|_| ProviderHttpFailure {
        code: "curseforge_download_url_invalid",
        retryable: false,
    })?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ProviderHttpFailure {
            code: "curseforge_download_url_invalid",
            retryable: false,
        });
    }
    Ok(())
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn map_http_error(error: ureq::Error) -> ProviderHttpFailure {
    match error {
        ureq::Error::Status(status, _) => ProviderHttpFailure {
            code: match status {
                401 | 403 => "curseforge_auth_failed",
                404 => "curseforge_not_found",
                429 => "curseforge_rate_limited",
                _ if status >= 500 => "curseforge_service_unavailable",
                _ => "curseforge_request_failed",
            },
            retryable: status == 429 || status >= 500,
        },
        ureq::Error::Transport(_) => ProviderHttpFailure {
            code: "curseforge_transport_failed",
            retryable: true,
        },
    }
}

fn to_catalog_failure(
    error: crate::provider_session::ProviderSessionError,
) -> CatalogProviderFailure {
    CatalogProviderFailure::new(error.code, error.retryable)
}

fn to_catalog_http_failure(error: ProviderHttpFailure) -> CatalogProviderFailure {
    CatalogProviderFailure::new(error.code, error.retryable)
}

fn to_resolver_failure(
    error: crate::provider_session::ProviderSessionError,
) -> ProviderResolveFailure {
    ProviderResolveFailure::new(error.code, error.retryable)
}

fn to_resolver_http_failure(error: ProviderHttpFailure) -> ProviderResolveFailure {
    ProviderResolveFailure::new(error.code, error.retryable)
}

struct ProviderHttpFailure {
    code: &'static str,
    retryable: bool,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
    #[serde(default)]
    pagination: Option<ApiPagination>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ApiPagination {
    index: u32,
    result_count: u32,
    total_count: u32,
}

type ApiSearchResponse = ApiResponse<Vec<ApiProject>>;

#[derive(Deserialize)]
struct ApiGame {
    id: u32,
    slug: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiProject {
    id: u64,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    authors: Vec<ApiAuthor>,
    logo: Option<ApiAsset>,
    #[serde(default)]
    categories: Vec<ApiCategory>,
    main_file_id: u64,
    #[serde(default)]
    latest_files: Vec<ApiFile>,
    #[serde(default)]
    allow_mod_distribution: Option<bool>,
    #[serde(default)]
    is_available: bool,
}

#[derive(Deserialize)]
struct ApiAuthor {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiAsset {
    thumbnail_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiCategory {
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    is_class: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiFile {
    id: u64,
    #[serde(default)]
    is_available: bool,
    file_name: String,
    file_length: u64,
    #[serde(default)]
    is_early_access_content: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    #[test]
    fn resource_identity_round_trips() {
        let encoded = encode_resource_id(123, 456);
        assert_eq!(decode_resource_id(&encoded).expect("decode"), (123, 456));
    }

    #[test]
    fn only_bedrock_package_extensions_are_downloadable() {
        assert!(valid_bedrock_package_name("demo.mcpack"));
        assert!(valid_bedrock_package_name("demo.MCADDON"));
        assert!(valid_bedrock_package_name("world.mcworld"));
        assert!(!valid_bedrock_package_name("server.jar"));
        assert!(!valid_bedrock_package_name("archive.zip"));
    }

    #[test]
    fn distribution_and_early_access_restrictions_are_enforced() {
        let allowed = ApiProject {
            id: 1,
            name: "Allowed".into(),
            summary: String::new(),
            authors: Vec::new(),
            logo: None,
            categories: Vec::new(),
            main_file_id: 2,
            latest_files: vec![ApiFile {
                id: 2,
                is_available: true,
                file_name: "allowed.mcaddon".into(),
                file_length: 10,
                is_early_access_content: Some(false),
            }],
            allow_mod_distribution: Some(true),
            is_available: true,
        };
        assert!(downloadable_file(&allowed).is_some());

        let mut blocked_distribution = allowed_for_test();
        blocked_distribution.allow_mod_distribution = Some(false);
        assert!(downloadable_file(&blocked_distribution).is_none());

        let mut early_access = allowed_for_test();
        early_access.latest_files[0].is_early_access_content = Some(true);
        assert!(downloadable_file(&early_access).is_none());
    }

    #[test]
    fn integrated_catalog_and_resolver_use_real_curseforge_contract_shape() {
        use crate::{
            catalog::{CatalogPageRequest, CatalogRequest},
            provider_adapter::ProviderAdapterRuntime,
        };
        const API_KEY: &str = "fixture-curseforge-key";
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = thread::spawn(move || {
            for expected_path in [
                "/v1/games?index=0&pageSize=50",
                "/v1/mods/search?gameId=777&index=0&pageSize=50&searchFilter=demo",
                "/v1/mods/123/files/456/download-url",
            ] {
                let (mut stream, _) = listener.accept().expect("accept");
                let request = read_request(&mut stream);
                assert!(request.starts_with(&format!("GET {expected_path} HTTP/1.1")));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("x-api-key: fixture-curseforge-key"));

                let body = if expected_path.starts_with("/v1/games") {
                    r#"{"data":[{"id":777,"slug":"minecraft-bedrock"}],"pagination":{"index":0,"pageSize":50,"resultCount":1,"totalCount":1}}"#
                } else if expected_path.starts_with("/v1/mods/search") {
                    r#"{"data":[{"id":123,"name":"Demo Add-On","summary":"Bedrock fixture","authors":[{"name":"Creator"}],"logo":{"thumbnailUrl":"https://cdn.example.com/icon.png"},"categories":[{"name":"Addons","slug":"addons","isClass":true},{"name":"Utility","slug":"utility","isClass":false}],"mainFileId":456,"latestFiles":[{"id":456,"isAvailable":true,"fileName":"demo.mcaddon","fileLength":9876,"isEarlyAccessContent":false}],"allowModDistribution":true,"isAvailable":true}],"pagination":{"index":0,"pageSize":50,"resultCount":1,"totalCount":1}}"#
                } else {
                    r#"{"data":"https://cdn.example.com/demo.mcaddon"}"#
                };
                write_json_response(&mut stream, body);
            }
        });

        let provider = Arc::new(
            CurseForgeProvider::new_with_base_url(API_KEY, format!("http://{address}"))
                .expect("provider"),
        );
        let runtime = ProviderAdapterRuntime::compose(vec![provider.clone()]).expect("runtime");
        let page = runtime
            .catalog()
            .query(&CatalogRequest {
                provider: PROVIDER_KEY.into(),
                query: CatalogQuery {
                    text: Some("demo".into()),
                    filters: Default::default(),
                    sort: CatalogSort::Relevance,
                    page: CatalogPageRequest {
                        limit: 30,
                        cursor: None,
                    },
                },
            })
            .expect("catalog");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].content_type, CatalogContentType::Addon);
        assert_eq!(page.items[0].file_name.as_deref(), Some("demo.mcaddon"));
        assert_eq!(page.items[0].expected_bytes, Some(9876));
        assert_eq!(page.items[0].tags, vec!["utility".to_string()]);
        match page.items[0].download.as_ref().expect("download ref") {
            CatalogDownloadRef::ProviderResolved {
                provider,
                resource_id,
            } => {
                assert_eq!(provider, PROVIDER_KEY);
                assert_eq!(resource_id, "123-456");
                assert!(!resource_id.contains(API_KEY));
            }
            other => panic!("unexpected download ref: {other:?}"),
        }

        let resolver = provider
            .resource_resolver(runtime.sessions())
            .expect("resolver build")
            .expect("resolver");
        resolver.resolve("123-456").expect("resolved download");
        server.join().expect("server");
    }

    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8(bytes).expect("utf8")
    }

    fn write_json_response(stream: &mut std::net::TcpStream, body: &str) {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("write response");
        stream.flush().expect("flush");
    }

    fn allowed_for_test() -> ApiProject {
        ApiProject {
            id: 1,
            name: "Allowed".into(),
            summary: String::new(),
            authors: Vec::new(),
            logo: None,
            categories: Vec::new(),
            main_file_id: 2,
            latest_files: vec![ApiFile {
                id: 2,
                is_available: true,
                file_name: "allowed.mcaddon".into(),
                file_length: 10,
                is_early_access_content: Some(false),
            }],
            allow_mod_distribution: Some(true),
            is_available: true,
        }
    }
}
