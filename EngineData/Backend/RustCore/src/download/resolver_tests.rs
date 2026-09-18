use super::*;
use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

struct SequenceResolver {
    key: String,
    responses: Mutex<VecDeque<Result<ResolvedResource, ProviderResolveFailure>>>,
    calls: AtomicUsize,
}

impl SequenceResolver {
    fn new(key: &str, responses: Vec<Result<ResolvedResource, ProviderResolveFailure>>) -> Self {
        Self {
            key: key.to_string(),
            responses: Mutex::new(responses.into()),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ResourceResolver for SequenceResolver {
    fn provider_key(&self) -> &str {
        &self.key
    }

    fn resolve(&self, _resource_id: &str) -> Result<ResolvedResource, ProviderResolveFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.responses
            .lock()
            .expect("resolver responses")
            .pop_front()
            .unwrap_or_else(|| Err(ProviderResolveFailure::new("provider_fixture_empty", false)))
    }
}

fn test_http_policy() -> HttpTransportPolicy {
    HttpTransportPolicy {
        connect_timeout: Duration::from_secs(1),
        read_timeout: Duration::from_secs(1),
        overall_timeout: Duration::from_secs(5),
        max_redirects: 2,
        max_response_bytes: 2 * 1024 * 1024,
    }
}

fn provider_runtime(
    root: &std::path::Path,
    resolver: Option<Arc<SequenceResolver>>,
) -> DownloadExecutionRuntime {
    let mut resolvers = ResourceResolverRegistry::new();
    if let Some(resolver) = resolver {
        resolvers.register(resolver).expect("register resolver");
    }
    let provider_transport = ProviderResolvedTransport::new(
        Arc::new(resolvers),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    );
    let mut transports = DownloadTransportRegistry::new();
    transports
        .register(Arc::new(provider_transport))
        .expect("register provider transport");

    DownloadExecutionRuntime::new(
        DownloadPolicy::default(),
        DownloadStore::new(root.join("state.json")),
        root.join("workspace"),
        root.join("files"),
        transports,
    )
    .expect("provider runtime")
}

fn request(provider: &str, resource_id: &str, name: &str, bytes: usize) -> DownloadRequest {
    DownloadRequest {
        source: provider_download_source(provider, resource_id).expect("provider source"),
        display_name: name.to_string(),
        destination_file_name: format!("{name}.mcpack"),
        expected_bytes: Some(bytes as u64),
        expected_sha256: None,
    }
}

fn wait_for(
    runtime: &DownloadExecutionRuntime,
    predicate: impl Fn(&DownloadManagerSnapshot) -> bool,
) -> DownloadManagerSnapshot {
    let started = Instant::now();
    loop {
        let snapshot = runtime.snapshot().expect("snapshot");
        if predicate(&snapshot) {
            return snapshot;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "provider fixture timed out: {snapshot:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn spawn_server(payload: Vec<u8>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("provider fixture address");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let request = read_request(&mut stream).unwrap_or_default();
        let _ = sender.send(request);
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        let _ = stream.write_all(&payload);
        let _ = stream.flush();
    });
    (format!("http://{address}"), receiver)
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<String> {
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    while bytes.len() < 16 * 1024 {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[test]
fn provider_reference_rejects_runtime_url_material() {
    assert!(ProviderResourceRef::new("fake", "https://example.test/file").is_err());
    assert!(ProviderResourceRef::new("fake", "item?token=secret").is_err());
    let reference = ProviderResourceRef::new("fake", "catalog-item-42").expect("reference");
    assert_eq!(reference.encode(), "fake:catalog-item-42");
}

#[test]
fn missing_provider_fails_without_retry_loop() {
    let directory = tempfile::tempdir().expect("tempdir");
    let runtime = provider_runtime(directory.path(), None);
    let job = runtime
        .queue(request("missing", "item-1", "missing-provider", 1))
        .expect("queue");

    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot
            .jobs
            .iter()
            .any(|candidate| candidate.id == job.id && candidate.state == DownloadJobState::Failed)
    });
    let failed = snapshot
        .jobs
        .iter()
        .find(|candidate| candidate.id == job.id)
        .expect("failed job");
    let failure = failed.last_error.as_ref().expect("failure");
    assert_eq!(failure.code, "download_provider_unavailable");
    assert!(!failure.retryable);
}

#[test]
fn expired_material_is_resolved_again_before_transfer() {
    let payload = b"fresh-provider-payload".repeat(256);
    let (base, _) = spawn_server(payload.clone());
    let resolver = Arc::new(SequenceResolver::new(
        "fake",
        vec![
            Ok(
                ResolvedResource::new("http://127.0.0.1:1/expired?sig=old-ephemeral-secret")
                    .with_expiry_ms(0),
            ),
            Ok(ResolvedResource::new(format!(
                "{base}/fresh?sig=fresh-secret"
            ))),
        ],
    ));
    let directory = tempfile::tempdir().expect("tempdir");
    let runtime = provider_runtime(directory.path(), Some(resolver.clone()));
    let job = runtime
        .queue(request("fake", "item-refresh", "refreshed", payload.len()))
        .expect("queue");

    wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id && candidate.state == DownloadJobState::Completed
        })
    });
    assert_eq!(resolver.calls(), 2);
    let persisted = std::fs::read_to_string(directory.path().join("state.json")).expect("state");
    assert!(!persisted.contains("old-ephemeral-secret"));
    assert!(!persisted.contains("fresh-secret"));
}

#[test]
fn retry_re_resolves_instead_of_reusing_failed_material() {
    let payload = b"retry-provider-payload".repeat(128);
    let (base, _) = spawn_server(payload.clone());
    let resolver = Arc::new(SequenceResolver::new(
        "fake",
        vec![
            Err(ProviderResolveFailure::new(
                "provider_session_expired",
                true,
            )),
            Ok(ResolvedResource::new(format!(
                "{base}/retry?sig=retry-secret"
            ))),
        ],
    ));
    let directory = tempfile::tempdir().expect("tempdir");
    let runtime = provider_runtime(directory.path(), Some(resolver.clone()));
    let job = runtime
        .queue(request(
            "fake",
            "item-retry",
            "retry-resolve",
            payload.len(),
        ))
        .expect("queue");

    let failed = wait_for(&runtime, |snapshot| {
        snapshot
            .jobs
            .iter()
            .any(|candidate| candidate.id == job.id && candidate.state == DownloadJobState::Failed)
    });
    assert!(failed
        .jobs
        .iter()
        .find(|candidate| candidate.id == job.id)
        .and_then(|candidate| candidate.last_error.as_ref())
        .is_some_and(|failure| failure.retryable));

    runtime.retry(&job.id).expect("retry");
    wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id && candidate.state == DownloadJobState::Completed
        })
    });
    assert_eq!(resolver.calls(), 2);
}

#[test]
fn ephemeral_query_and_auth_header_are_used_but_never_persisted() {
    let payload = b"credential-safe-provider-payload".repeat(128);
    let (base, captured_request) = spawn_server(payload.clone());
    let resolver = Arc::new(SequenceResolver::new(
        "fake",
        vec![Ok(ResolvedResource::new(format!(
            "{base}/file?sig=ephemeral-query-secret"
        ))
        .with_header("Authorization", "Bearer ephemeral-header-secret"))],
    ));
    let directory = tempfile::tempdir().expect("tempdir");
    let runtime = provider_runtime(directory.path(), Some(resolver));
    let job = runtime
        .queue(request(
            "fake",
            "catalog-item-42",
            "secret-safe",
            payload.len(),
        ))
        .expect("queue");

    wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id && candidate.state == DownloadJobState::Completed
        })
    });

    let request_text = captured_request
        .recv_timeout(Duration::from_secs(1))
        .expect("captured provider request");
    let request_lower = request_text.to_ascii_lowercase();
    assert!(request_lower.contains("/file?sig=ephemeral-query-secret"));
    assert!(request_lower.contains("authorization: bearer ephemeral-header-secret"));

    let persisted = std::fs::read_to_string(directory.path().join("state.json")).expect("state");
    assert!(persisted.contains("fake:catalog-item-42"));
    assert!(!persisted.contains("ephemeral-query-secret"));
    assert!(!persisted.contains("ephemeral-header-secret"));
    assert!(!persisted.to_ascii_lowercase().contains("authorization"));
}
