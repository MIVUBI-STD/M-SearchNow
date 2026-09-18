use crate::{
    app_runtime::{QueueCatalogDownloadRequest, SearchNowBackendPaths, SearchNowBackendRuntime},
    catalog::CatalogDownloadRef,
    diagnostics::BackendStartupPhase,
    download::{
        DownloadJobState, DownloadManagerSnapshot, HttpTransport, HttpTransportPolicy,
        ProviderResolveFailure, ResolvedResource, ResourceResolver,
    },
    error::BackendResult,
    platform::PlatformContext,
    provider_adapter::IntegratedProvider,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

struct InvalidProvider;

impl IntegratedProvider for InvalidProvider {
    fn provider_key(&self) -> &str {
        "invalid provider key"
    }
}

struct ResolverProvider {
    base_url: String,
}

impl IntegratedProvider for ResolverProvider {
    fn provider_key(&self) -> &str {
        "runtime-fixture"
    }

    fn resource_resolver(
        &self,
        _sessions: Arc<crate::provider_session::ProviderSessionManager>,
    ) -> BackendResult<Option<Arc<dyn ResourceResolver>>> {
        Ok(Some(Arc::new(LoopbackResolver {
            base_url: self.base_url.clone(),
        })))
    }
}

struct LoopbackResolver {
    base_url: String,
}

impl ResourceResolver for LoopbackResolver {
    fn provider_key(&self) -> &str {
        "runtime-fixture"
    }

    fn resolve(&self, resource_id: &str) -> Result<ResolvedResource, ProviderResolveFailure> {
        if resource_id != "asset" {
            return Err(ProviderResolveFailure::new(
                "fixture_resource_missing",
                false,
            ));
        }
        Ok(ResolvedResource::new(format!("{}/asset", self.base_url)))
    }
}

#[test]
fn runtime_snapshot_is_safe_and_consistent_without_providers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let paths =
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data"));
    let runtime = SearchNowBackendRuntime::compose(
        paths,
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");

    let snapshot = runtime.snapshot().expect("snapshot");
    assert!(snapshot.runtime.app_ready);
    assert!(snapshot.providers.is_empty());
    assert_eq!(snapshot.downloads.active_jobs, 0);
    assert_eq!(snapshot.downloads.queued_jobs, 0);
    assert!(snapshot.downloads.scheduler_error.is_none());
    assert_eq!(
        snapshot.diagnostics.health.startup_phase,
        BackendStartupPhase::Ready
    );
}

#[test]
fn composed_provider_resolver_is_used_by_application_download_runtime() {
    let payload = b"searchnow-backend-runtime".repeat(1024);
    let base_url = spawn_server(payload.clone());
    let temp = tempfile::tempdir().expect("tempdir");
    let paths =
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data"));
    let destination = paths.download_destination_root.clone();
    let runtime = SearchNowBackendRuntime::compose(
        paths,
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        vec![Arc::new(ResolverProvider { base_url })],
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");

    let status = runtime.provider_status();
    assert_eq!(status.len(), 1);
    assert!(status[0].capabilities.resolved_download);

    runtime
        .queue_catalog_download(QueueCatalogDownloadRequest {
            download: CatalogDownloadRef::ProviderResolved {
                provider: "runtime-fixture".into(),
                resource_id: "asset".into(),
            },
            display_name: "Runtime Fixture".into(),
            destination_file_name: "runtime.mcpack".into(),
            expected_bytes: Some(payload.len() as u64),
        })
        .expect("queue");

    let snapshot = wait_for_terminal(&runtime);
    assert_eq!(snapshot.jobs[0].state, DownloadJobState::Completed);
    assert_eq!(
        std::fs::read(destination.join("runtime.mcpack")).expect("final file"),
        payload
    );
}

#[test]
fn invalid_provider_prevents_application_runtime_construction() {
    let temp = tempfile::tempdir().expect("tempdir");
    let paths =
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data"));
    let result = SearchNowBackendRuntime::compose(
        paths,
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        vec![Arc::new(InvalidProvider)],
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    );
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("invalid provider must prevent runtime construction"),
    };
    assert_eq!(error.code(), "provider_adapter_key_invalid");
}

fn spawn_server(payload: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture");
    let address = listener.local_addr().expect("fixture address");
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let _ = read_request(&mut stream);
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        let _ = stream.write_all(&payload);
        let _ = stream.flush();
    });
    format!("http://{address}")
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut buffer = [0_u8; 4096];
    let _ = stream.read(&mut buffer)?;
    Ok(())
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

fn wait_for_terminal(runtime: &SearchNowBackendRuntime) -> DownloadManagerSnapshot {
    let started = Instant::now();
    loop {
        let snapshot = runtime.download_snapshot().expect("download snapshot");
        if snapshot
            .jobs
            .first()
            .is_some_and(|job| job.state.is_terminal())
        {
            return snapshot;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "application runtime download timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
