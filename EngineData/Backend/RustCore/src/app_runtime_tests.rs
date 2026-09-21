use crate::{
    app_runtime::{QueueCatalogDownloadRequest, SearchNowBackendPaths, SearchNowBackendRuntime},
    application::ApplicationLifecycleState,
    catalog::CatalogDownloadRef,
    diagnostics::BackendStartupPhase,
    download::{
        DownloadJobState, DownloadManagerSnapshot, HttpTransport, HttpTransportPolicy,
        ProviderResolveFailure, ResolvedResource, ResourceResolver,
    },
    error::BackendResult,
    package::PackageImportRequest,
    platform::PlatformContext,
    provider_adapter::IntegratedProvider,
    settings::ExportDuplicatePolicy,
};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

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
    assert_eq!(snapshot.lifecycle, ApplicationLifecycleState::Ready);
    assert!(snapshot.capabilities.library);
    assert!(snapshot.capabilities.downloads);
    assert!(snapshot.capabilities.settings);
    assert!(!snapshot.capabilities.discover);
    assert!(snapshot.providers.is_empty());
    assert_eq!(
        snapshot.diagnostics.health.startup_phase,
        BackendStartupPhase::Ready
    );

    let serialized = serde_json::to_value(&snapshot).expect("serialize snapshot");
    assert!(serialized.get("minecraft").is_none());
    assert!(serialized.get("downloads").is_none());
}

#[test]
fn diagnostics_support_report_is_local_safe_and_serializable() {
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

    let bytes = runtime
        .diagnostics_support_report_json()
        .expect("diagnostics report");
    let text = String::from_utf8(bytes).expect("utf8");

    assert!(text.contains("\"schemaVersion\": 1"));
    assert!(text.contains("\"runtime\""));
    assert!(text.contains("\"diagnostics\""));
    assert!(!text.contains("Authorization"));
    assert!(!text.contains("Bearer "));
    assert!(!text.contains("token="));
    assert!(!text.contains(temp.path().to_string_lossy().as_ref()));
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
            expected_sha256: None,
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
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
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
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
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

#[test]
fn package_import_rejects_installed_manifest_uuid_conflict() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("minecraft-root");
    let installed = root.join("resource_packs/installed");
    fs::create_dir_all(&installed).expect("installed pack");
    let uuid = "11111111-1111-4111-8111-111111111111";
    let manifest = format!(
        r#"{{
  "format_version": 2,
  "header": {{"name":"Installed","uuid":"{uuid}","version":[1,0,0]}},
  "modules": [{{"type":"resources","uuid":"44444444-4444-4444-8444-444444444444","version":[1,0,0]}}]
}}"#
    );
    fs::write(installed.join("manifest.json"), &manifest).expect("installed manifest");

    let source = temp.path().join("duplicate.mcpack");
    let file = fs::File::create(&source).expect("archive");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer
        .start_file("manifest.json", options)
        .expect("archive manifest");
    writer
        .write_all(manifest.as_bytes())
        .expect("manifest bytes");
    writer.finish().expect("finish archive");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");

    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root.clone());
    runtime.save_settings(&settings).expect("save settings");
    let discovery = runtime.discover_minecraft().expect("discovery");
    let root_id = discovery.roots.first().expect("custom root").id.clone();

    let error = runtime
        .import_package(PackageImportRequest {
            source_path: source,
            root_id,
        })
        .expect_err("duplicate UUID must fail closed");
    assert_eq!(error.code(), "package_import_conflict");
    assert_eq!(
        fs::read_dir(root.join("resource_packs"))
            .expect("resource packs")
            .count(),
        1
    );
}

#[test]
fn remove_local_content_deletes_only_current_library_item() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("minecraft-root");
    let pack = root.join("resource_packs/remove-me");
    let survivor = root.join("resource_packs/keep-me");
    fs::create_dir_all(&pack).expect("pack");
    fs::create_dir_all(&survivor).expect("survivor");
    fs::write(
        pack.join("manifest.json"),
        r#"{"header":{"name":"Remove Me","version":[1,0,0]}}"#,
    )
    .expect("manifest");
    fs::write(
        survivor.join("manifest.json"),
        r#"{"header":{"name":"Keep Me","version":[1,0,0]}}"#,
    )
    .expect("survivor manifest");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");
    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root);
    runtime.save_settings(&settings).expect("save settings");

    let before = runtime.scan_local_library().expect("library");
    let item = before
        .library
        .items
        .iter()
        .find(|item| item.title == "Remove Me")
        .expect("remove item");
    let after = runtime
        .remove_local_content(&item.id)
        .expect("remove local content");

    assert!(!pack.exists());
    assert!(survivor.is_dir());
    assert_eq!(after.library.summary.total, 1);
    assert_eq!(after.library.items[0].title, "Keep Me");
}

#[test]
fn export_local_content_creates_reimportable_archive() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("minecraft-root");
    let pack = root.join("resource_packs/export-me");
    fs::create_dir_all(pack.join("textures")).expect("pack");
    fs::write(
        pack.join("manifest.json"),
        r#"{"header":{"name":"Export Me","uuid":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","version":[1,0,0]},"modules":[{"type":"resources","uuid":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","version":[1,0,0]}]}"#,
    )
    .expect("manifest");
    fs::write(pack.join("textures/example.txt"), "texture").expect("texture");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");
    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root);
    runtime.save_settings(&settings).expect("save settings");

    let snapshot = runtime.scan_local_library().expect("library");
    let item = snapshot
        .library
        .items
        .iter()
        .find(|item| item.title == "Export Me")
        .expect("export item");
    assert_eq!(
        runtime
            .local_content_export_file_name(&item.id)
            .expect("export name"),
        "Export Me.mcpack"
    );

    let destination = temp.path().join("Export Me.mcpack");
    runtime
        .export_local_content(&item.id, &destination)
        .expect("export");
    let inspection = runtime
        .inspect_package(&destination)
        .expect("inspect export");
    assert_eq!(
        inspection.status,
        crate::package::PackageInspectionStatus::Ready
    );
    assert_eq!(inspection.packs.len(), 1);
    assert_eq!(inspection.packs[0].name, "Export Me");
}

#[test]
fn configured_export_policy_keeps_both_or_stops_on_conflict() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("minecraft-root");
    let pack = root.join("resource_packs/export-policy");
    fs::create_dir_all(&pack).expect("pack");
    fs::write(
        pack.join("manifest.json"),
        r#"{"header":{"name":"Export Policy","uuid":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaab","version":[1,0,0]},"modules":[{"type":"resources","uuid":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbc","version":[1,0,0]}]}"#,
    )
    .expect("manifest");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");
    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root);
    runtime.save_settings(&settings).expect("save settings");

    let snapshot = runtime.scan_local_library().expect("library");
    let item = snapshot
        .library
        .items
        .iter()
        .find(|item| item.title == "Export Policy")
        .expect("export item");

    let exports = temp.path().join("exports");
    fs::create_dir_all(&exports).expect("exports");

    let first = runtime
        .export_local_content_to_directory(&item.id, &exports, ExportDuplicatePolicy::KeepBoth)
        .expect("first export");
    let second = runtime
        .export_local_content_to_directory(&item.id, &exports, ExportDuplicatePolicy::KeepBoth)
        .expect("second export");

    assert_eq!(first, "Export Policy.mcpack");
    assert_eq!(second, "Export Policy (2).mcpack");

    let error = runtime
        .export_local_content_to_directory(
            &item.id,
            &exports,
            ExportDuplicatePolicy::StopOnConflict,
        )
        .expect_err("conflict must stop");
    assert_eq!(error.code(), "library_export_destination_exists");
}

#[test]
fn replace_package_updates_single_installed_pack_in_place() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("minecraft-root");
    let installed = root.join("resource_packs/existing");
    fs::create_dir_all(&installed).expect("installed");
    let uuid = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
    fs::write(
        installed.join("manifest.json"),
        format!(
            r#"{{"format_version":2,"header":{{"name":"Example","uuid":"{uuid}","version":[1,0,0]}},"modules":[{{"type":"resources","uuid":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","version":[1,0,0]}}]}}"#
        ),
    )
    .expect("old manifest");
    fs::write(installed.join("old.txt"), "old").expect("old file");

    let source = temp.path().join("update.mcpack");
    let file = fs::File::create(&source).expect("archive");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer
        .start_file("manifest.json", options)
        .expect("manifest entry");
    writer
        .write_all(
            format!(
                r#"{{"format_version":2,"header":{{"name":"Example","uuid":"{uuid}","version":[2,0,0]}},"modules":[{{"type":"resources","uuid":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee","version":[2,0,0]}}]}}"#
            )
            .as_bytes(),
        )
        .expect("manifest bytes");
    writer.start_file("new.txt", options).expect("new entry");
    writer.write_all(b"new").expect("new bytes");
    writer.finish().expect("finish archive");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");
    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root);
    runtime.save_settings(&settings).expect("save settings");
    let root_id = runtime.discover_minecraft().expect("discovery").roots[0]
        .id
        .clone();

    let result = runtime
        .replace_package(crate::package::PackageReplaceRequest {
            source_path: source,
            root_id,
        })
        .expect("replace");

    assert_eq!(result.imported.len(), 1);
    assert_eq!(result.imported[0].destination_path, installed);
    assert!(!installed.join("old.txt").exists());
    assert_eq!(
        fs::read_to_string(installed.join("new.txt")).expect("new file"),
        "new"
    );
    let snapshot = runtime.scan_local_library().expect("library");
    assert_eq!(snapshot.library.items[0].version, vec![2, 0, 0]);
}

#[test]
fn replace_package_rejects_downgrade() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("minecraft-root");
    let installed = root.join("resource_packs/existing");
    fs::create_dir_all(&installed).expect("installed");
    let uuid = "ffffffff-ffff-4fff-8fff-ffffffffffff";
    fs::write(
        installed.join("manifest.json"),
        format!(
            r#"{{"format_version":2,"header":{{"name":"Example","uuid":"{uuid}","version":[2,0,0]}},"modules":[{{"type":"resources","uuid":"12121212-1212-4212-8212-121212121212","version":[2,0,0]}}]}}"#
        ),
    )
    .expect("old manifest");

    let source = temp.path().join("older.mcpack");
    let file = fs::File::create(&source).expect("archive");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer
        .start_file("manifest.json", options)
        .expect("manifest entry");
    writer
        .write_all(
            format!(
                r#"{{"format_version":2,"header":{{"name":"Example","uuid":"{uuid}","version":[1,9,0]}},"modules":[{{"type":"resources","uuid":"34343434-3434-4434-8434-343434343434","version":[1,9,0]}}]}}"#
            )
            .as_bytes(),
        )
        .expect("manifest bytes");
    writer.finish().expect("finish archive");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");
    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root);
    runtime.save_settings(&settings).expect("save settings");
    let root_id = runtime.discover_minecraft().expect("discovery").roots[0]
        .id
        .clone();

    let error = runtime
        .replace_package(crate::package::PackageReplaceRequest {
            source_path: source,
            root_id,
        })
        .expect_err("downgrade must fail closed");

    assert_eq!(error.code(), "package_replace_older_version");
    assert_eq!(
        runtime.scan_local_library().expect("library").library.items[0].version,
        vec![2, 0, 0]
    );
}

#[test]
fn replace_package_bundle_updates_and_imports_transactionally() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("minecraft-root");
    let behavior = root.join("behavior_packs/existing-behavior");
    fs::create_dir_all(&behavior).expect("behavior");
    let behavior_uuid = "10101010-1010-4010-8010-101010101010";
    let resource_uuid = "20202020-2020-4020-8020-202020202020";
    fs::write(
        behavior.join("manifest.json"),
        format!(
            r#"{{"format_version":2,"header":{{"name":"Behavior","uuid":"{behavior_uuid}","version":[1,0,0]}},"modules":[{{"type":"data","uuid":"30303030-3030-4030-8030-303030303030","version":[1,0,0]}}],"dependencies":[{{"uuid":"{resource_uuid}","version":[2,0,0]}}]}}"#
        ),
    )
    .expect("old behavior");
    fs::write(behavior.join("old.txt"), "old").expect("old file");

    let source = temp.path().join("bundle.mcaddon");
    let file = fs::File::create(&source).expect("archive");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    writer
        .start_file("behavior/manifest.json", options)
        .expect("behavior manifest");
    writer
        .write_all(
            format!(
                r#"{{"format_version":2,"header":{{"name":"Behavior","uuid":"{behavior_uuid}","version":[2,0,0]}},"modules":[{{"type":"data","uuid":"40404040-4040-4040-8040-404040404040","version":[2,0,0]}}],"dependencies":[{{"uuid":"{resource_uuid}","version":[2,0,0]}}]}}"#
            )
            .as_bytes(),
        )
        .expect("behavior bytes");
    writer
        .start_file("behavior/new.txt", options)
        .expect("behavior file");
    writer.write_all(b"new behavior").expect("behavior data");

    writer
        .start_file("resource/manifest.json", options)
        .expect("resource manifest");
    writer
        .write_all(
            format!(
                r#"{{"format_version":2,"header":{{"name":"Resource","uuid":"{resource_uuid}","version":[2,0,0]}},"modules":[{{"type":"resources","uuid":"50505050-5050-4050-8050-505050505050","version":[2,0,0]}}]}}"#
            )
            .as_bytes(),
        )
        .expect("resource bytes");
    writer
        .start_file("resource/new.txt", options)
        .expect("resource file");
    writer.write_all(b"new resource").expect("resource data");
    writer.finish().expect("finish archive");

    let runtime = SearchNowBackendRuntime::compose(
        SearchNowBackendPaths::from_roots(temp.path().join("config"), temp.path().join("data")),
        PlatformContext::windows(temp.path().join("roaming"), temp.path().join("local")),
        Vec::new(),
        HttpTransport::new_test_http(test_http_policy()).expect("test HTTP"),
    )
    .expect("runtime");
    let mut settings = runtime.load_settings().expect("settings");
    settings.minecraft.root_override = Some(root.clone());
    runtime.save_settings(&settings).expect("save settings");
    let root_id = runtime.discover_minecraft().expect("discovery").roots[0]
        .id
        .clone();

    let result = runtime
        .replace_package_bundle(crate::package::PackageBundleUpdateRequest {
            source_path: source,
            root_id,
        })
        .expect("bundle replace");

    assert_eq!(result.imported.len(), 2);
    assert!(!behavior.join("old.txt").exists());
    assert_eq!(
        fs::read_to_string(behavior.join("new.txt")).expect("new behavior"),
        "new behavior"
    );
    let snapshot = runtime.scan_local_library().expect("library");
    assert_eq!(snapshot.library.summary.total, 2);
    assert!(snapshot
        .library
        .items
        .iter()
        .any(|item| item.manifest_uuid.as_deref() == Some(resource_uuid)));
}
