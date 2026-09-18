use super::*;
use crate::{
    catalog::{
        CatalogContentType, CatalogDownloadRef, CatalogProvider, CatalogProviderFailure,
        CatalogProviderItem, CatalogProviderPage, CatalogQuery, CatalogRequest,
    },
    download::{
        DownloadExecutionRuntime, DownloadJobState, DownloadPolicy, DownloadRequest, DownloadStore,
        DownloadTransportRegistry, HttpTransport, HttpTransportPolicy, ProviderResolveFailure,
        ProviderResolvedTransport, ResolvedResource, ResourceResolver,
    },
    error::BackendResult,
    provider_session::{
        ProviderSessionFailure, ProviderSessionLease, ProviderSessionManager,
        ProviderSessionMaterial, ProviderSessionSource,
    },
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const PROVIDER_KEY: &str = "fake-integrated";
const SESSION_SECRET: &str = "Bearer provider-adapter-runtime-secret";

struct SecretSession {
    authorization: String,
}

struct FakeSessionSource {
    acquire_calls: AtomicUsize,
}

impl FakeSessionSource {
    fn new() -> Self {
        Self {
            acquire_calls: AtomicUsize::new(0),
        }
    }
}

impl ProviderSessionSource for FakeSessionSource {
    fn provider_key(&self) -> &str {
        PROVIDER_KEY
    }

    fn acquire(&self) -> Result<ProviderSessionMaterial, ProviderSessionFailure> {
        self.acquire_calls.fetch_add(1, Ordering::SeqCst);
        Ok(ProviderSessionMaterial::new(
            SecretSession {
                authorization: SESSION_SECRET.to_string(),
            },
            Some(u64::MAX),
        ))
    }

    fn refresh(
        &self,
        _current: &ProviderSessionLease,
    ) -> Result<ProviderSessionMaterial, ProviderSessionFailure> {
        self.acquire()
    }
}

struct FakeIntegratedProvider {
    sessions: Arc<FakeSessionSource>,
    base_url: String,
}

impl IntegratedProvider for FakeIntegratedProvider {
    fn provider_key(&self) -> &str {
        PROVIDER_KEY
    }

    fn session_source(&self) -> Option<Arc<dyn ProviderSessionSource>> {
        Some(self.sessions.clone())
    }

    fn catalog_provider(
        &self,
        sessions: Arc<ProviderSessionManager>,
    ) -> BackendResult<Option<Arc<dyn CatalogProvider>>> {
        Ok(Some(Arc::new(FakeCatalogProvider { sessions })))
    }

    fn resource_resolver(
        &self,
        sessions: Arc<ProviderSessionManager>,
    ) -> BackendResult<Option<Arc<dyn ResourceResolver>>> {
        Ok(Some(Arc::new(FakeResolver {
            sessions,
            base_url: self.base_url.clone(),
        })))
    }
}

struct FakeCatalogProvider {
    sessions: Arc<ProviderSessionManager>,
}

impl CatalogProvider for FakeCatalogProvider {
    fn key(&self) -> &str {
        PROVIDER_KEY
    }

    fn query(&self, _query: &CatalogQuery) -> Result<CatalogProviderPage, CatalogProviderFailure> {
        let lease = self
            .sessions
            .acquire(PROVIDER_KEY)
            .map_err(|error| CatalogProviderFailure::new(error.code, error.retryable))?;
        let secret = lease
            .downcast::<SecretSession>()
            .map_err(|error| CatalogProviderFailure::new(error.code, error.retryable))?;
        assert_eq!(secret.authorization, SESSION_SECRET);

        Ok(CatalogProviderPage {
            items: vec![CatalogProviderItem {
                item_id: "demo-addon".into(),
                title: "Demo Add-On".into(),
                creator_name: Some("Demo Creator".into()),
                thumbnail_url: Some("https://cdn.example.com/demo-addon.webp".into()),
                description: Some("Integrated provider fixture".into()),
                content_type: CatalogContentType::Addon,
                tags: vec!["fixture".into()],
                published_at_ms: Some(1),
                updated_at_ms: Some(1),
                download: Some(CatalogDownloadRef::ProviderResolved {
                    provider: PROVIDER_KEY.into(),
                    resource_id: "asset-1".into(),
                }),
            }],
            next_cursor: None,
        })
    }
}

struct FakeResolver {
    sessions: Arc<ProviderSessionManager>,
    base_url: String,
}

impl ResourceResolver for FakeResolver {
    fn provider_key(&self) -> &str {
        PROVIDER_KEY
    }

    fn resolve(&self, resource_id: &str) -> Result<ResolvedResource, ProviderResolveFailure> {
        assert_eq!(resource_id, "asset-1");
        let lease = self
            .sessions
            .acquire(PROVIDER_KEY)
            .map_err(|error| ProviderResolveFailure::new(error.code, error.retryable))?;
        let secret = lease
            .downcast::<SecretSession>()
            .map_err(|error| ProviderResolveFailure::new(error.code, error.retryable))?;
        Ok(ResolvedResource::new(format!("{}/asset", self.base_url))
            .with_header("Authorization", secret.authorization.clone()))
    }
}

struct MismatchedCatalog;

impl CatalogProvider for MismatchedCatalog {
    fn key(&self) -> &str {
        "wrong-key"
    }

    fn query(&self, _query: &CatalogQuery) -> Result<CatalogProviderPage, CatalogProviderFailure> {
        Ok(CatalogProviderPage {
            items: Vec::new(),
            next_cursor: None,
        })
    }
}

struct MismatchedProvider;

impl IntegratedProvider for MismatchedProvider {
    fn provider_key(&self) -> &str {
        "expected-key"
    }

    fn catalog_provider(
        &self,
        _sessions: Arc<ProviderSessionManager>,
    ) -> BackendResult<Option<Arc<dyn CatalogProvider>>> {
        Ok(Some(Arc::new(MismatchedCatalog)))
    }
}

#[test]
fn composition_rejects_mismatched_component_keys() {
    let result = ProviderAdapterRuntime::compose(vec![Arc::new(MismatchedProvider)]);
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("mismatched provider component must fail"),
    };
    assert_eq!(error.code(), "provider_adapter_component_key_mismatch");
}

#[test]
fn integrated_provider_catalog_to_download_reuses_one_secret_session() {
    let payload = b"searchnow-integrated-provider-fixture".repeat(1024);
    let base_url = spawn_authorized_server(payload.clone(), SESSION_SECRET.to_string());
    let session_source = Arc::new(FakeSessionSource::new());
    let providers = ProviderAdapterRuntime::compose(vec![Arc::new(FakeIntegratedProvider {
        sessions: session_source.clone(),
        base_url,
    })])
    .expect("compose integrated provider");

    let initial_status = providers.status();
    assert_eq!(initial_status.len(), 1);
    assert!(initial_status[0].capabilities.session);
    assert!(initial_status[0].capabilities.catalog);
    assert!(initial_status[0].capabilities.resolved_download);

    let page = providers
        .catalog()
        .query(&CatalogRequest {
            provider: PROVIDER_KEY.into(),
            query: CatalogQuery::default(),
        })
        .expect("catalog query");
    assert_eq!(page.items[0].creator_name.as_deref(), Some("Demo Creator"));
    assert_eq!(
        page.items[0].thumbnail_url.as_deref(),
        Some("https://cdn.example.com/demo-addon.webp")
    );
    let source = page.items[0]
        .download
        .as_ref()
        .expect("download ref")
        .to_download_source()
        .expect("download source");

    let http = HttpTransport::new_test_http(test_policy()).expect("test HTTP transport");
    let mut transports = DownloadTransportRegistry::new();
    transports
        .register(Arc::new(ProviderResolvedTransport::new(
            providers.resolvers(),
            http,
        )))
        .expect("register provider transport");

    let temp = tempfile::tempdir().expect("tempdir");
    let state_path = temp.path().join("state.json");
    let files_root = temp.path().join("files");
    let downloads = DownloadExecutionRuntime::new(
        DownloadPolicy::default(),
        DownloadStore::new(&state_path),
        temp.path().join("workspace"),
        &files_root,
        transports,
    )
    .expect("download runtime");

    downloads
        .queue(DownloadRequest {
            source,
            display_name: "Demo Add-On".into(),
            destination_file_name: "demo.mcaddon".into(),
            expected_bytes: Some(payload.len() as u64),
            expected_sha256: None,
        })
        .expect("queue provider download");

    let snapshot = wait_for_completed(&downloads);
    assert_eq!(snapshot.jobs[0].state, DownloadJobState::Completed);
    assert_eq!(
        std::fs::read(files_root.join("demo.mcaddon")).expect("final file"),
        payload
    );
    assert_eq!(session_source.acquire_calls.load(Ordering::SeqCst), 1);

    let catalog_json = serde_json::to_string(&page).expect("catalog json");
    let status_json = serde_json::to_string(&providers.status()).expect("provider status json");
    let state_json = std::fs::read_to_string(state_path).expect("download state");
    for text in [&catalog_json, &status_json, &state_json] {
        assert!(!text.contains("provider-adapter-runtime-secret"));
        assert!(!text.contains(SESSION_SECRET));
    }
}

fn spawn_authorized_server(payload: Vec<u8>, expected_authorization: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("fixture address");
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let authorized =
            read_authorization(&mut stream).is_some_and(|value| value == expected_authorization);
        let (status, body) = if authorized {
            ("200 OK", payload)
        } else {
            ("401 Unauthorized", Vec::new())
        };
        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(&body);
        let _ = stream.flush();
    });
    format!("http://{address}")
}

fn read_authorization(stream: &mut TcpStream) -> Option<String> {
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).ok()?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("authorization")
            .then(|| value.trim().to_string())
    })
}

fn test_policy() -> HttpTransportPolicy {
    HttpTransportPolicy {
        connect_timeout: Duration::from_secs(1),
        read_timeout: Duration::from_secs(1),
        overall_timeout: Duration::from_secs(5),
        max_redirects: 2,
        max_response_bytes: 2 * 1024 * 1024,
    }
}

fn wait_for_completed(
    runtime: &DownloadExecutionRuntime,
) -> crate::download::DownloadManagerSnapshot {
    let started = Instant::now();
    loop {
        let snapshot = runtime.snapshot().expect("download snapshot");
        if snapshot
            .jobs
            .first()
            .is_some_and(|job| job.state == DownloadJobState::Completed)
        {
            return snapshot;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "integrated provider download timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
