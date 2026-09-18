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
    fs,
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
    let destination = temp.path().join("chosen-downloads");
    std::fs::create_dir_all(&destination).expect("chosen destination");
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
            destination_directory: Some(destination.clone()),
            expected_bytes: Some(payload.len() as u64),
        })
        .expect("queue");

    let snapshot = wait_for_terminal(&runtime);
    assert_eq!(snapshot.jobs[0].state, DownloadJobState::Completed);
    assert_eq!(
        std::fs::read(destination.join("runtime.mcpack")).expect("final file"),
        payload
    );
    assert_eq!(
        runtime
            .completed_download_directory(&snapshot.jobs[0].id)
            .expect("completed directory"),
        destination
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

#[test]
fn local_content_directory_is_resolved_from_current_library_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config = temp.path().join("config");
    let data = temp.path().join("data");
    let root = temp.path().join("minecraft-root");
    let pack = root.join("resource_packs/example");
    fs::create_dir_all(&pack).expect("pack");
    fs::write(
        pack.join("manifest.json"),
        r#"{"header":{"name":"Example Pack","version":[1,0,0]}}"#,
    )
    .expect("manifest");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(config, data),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");

    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root);
    runtime.save_settings(&settings).expect("save settings");

    let snapshot = runtime.scan_local_library().expect("library");
    let item = snapshot.library.items.first().expect("library item");
    assert_eq!(
        runtime
            .local_content_directory(&item.id)
            .expect("content directory"),
        pack
    );
}

#[test]
fn local_content_directory_rejects_stale_item_ids_after_content_is_removed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("minecraft-root");
    let pack = root.join("resource_packs/example");
    fs::create_dir_all(&pack).expect("pack");
    fs::write(
        pack.join("manifest.json"),
        r#"{"header":{"name":"Example Pack","version":[1,0,0]}}"#,
    )
    .expect("manifest");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(
            temp.path().join("config"),
            temp.path().join("data"),
        ),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");

    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root);
    runtime.save_settings(&settings).expect("save settings");

    let snapshot = runtime.scan_local_library().expect("library");
    let item_id = snapshot.library.items[0].id.clone();
    fs::remove_dir_all(&pack).expect("remove pack");

    let error = runtime
        .local_content_directory(&item_id)
        .expect_err("stale item id must fail closed");
    assert_eq!(error.code(), "library_item_not_found");
}

#[test]
fn completed_download_directory_rejects_unknown_job_ids() {
    let temp = tempfile::tempdir().expect("tempdir");
    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(
            temp.path().join("config"),
            temp.path().join("data"),
        ),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");

    let error = runtime
        .completed_download_directory("download-missing")
        .expect_err("unknown download id must fail closed");
    assert_eq!(error.code(), "download_job_not_found");
}
