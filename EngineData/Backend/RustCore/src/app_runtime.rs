use crate::{
    application::{ApplicationCapabilities, ApplicationLifecycleState, ApplicationState},
    application_content::ContentApplicationService,
    application_download::DownloadApplicationService,
    build_local_backend_snapshot,
    catalog::{CatalogError, CatalogPage, CatalogRequest},
    diagnostics::{
        BackendDiagnosticsSnapshot, DiagnosticComponent, DiagnosticSeverity, DiagnosticsBuffer,
    },
    download::{
        default_download_paths, DownloadExecutionRuntime, DownloadJob, DownloadManagerSnapshot,
        DownloadPolicy, DownloadQueueMove, DownloadStore, DownloadTransportRegistry, HttpTransport,
        HttpTransportPolicy, ProviderResolvedTransport,
    },
    error::{BackendError, BackendResult},
    minecraft::{discover_minecraft_storage, MinecraftDiscoverySnapshot},
    package::{
        PackageBundleUpdateRequest, PackageImportRequest, PackageImportResult, PackageInspection,
        PackageReplaceRequest,
    },
    platform::PlatformContext,
    provider_adapter::{IntegratedProvider, ProviderAdapterRuntime, ProviderRuntimeStatus},
    runtime::{runtime_status, RuntimeStatus},
    settings::{AppSettings, ExportDuplicatePolicy, SettingsStore},
    LocalBackendSnapshot,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

#[derive(Debug, Clone)]
pub struct SearchNowBackendPaths {
    pub settings_path: PathBuf,
    pub download_state_path: PathBuf,
    pub download_workspace_root: PathBuf,
    pub download_destination_root: PathBuf,
}

impl SearchNowBackendPaths {
    pub fn from_roots(config_root: impl AsRef<Path>, data_root: impl AsRef<Path>) -> Self {
        let (download_state_path, download_workspace_root, download_destination_root) =
            default_download_paths(data_root.as_ref());
        Self {
            settings_path: config_root.as_ref().join("settings.json"),
            download_state_path,
            download_workspace_root,
            download_destination_root,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSupportReport {
    pub schema_version: u32,
    pub runtime: RuntimeStatus,
    pub diagnostics: BackendDiagnosticsSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendRuntimeSnapshot {
    pub runtime: RuntimeStatus,
    pub lifecycle: ApplicationLifecycleState,
    pub capabilities: ApplicationCapabilities,
    pub minecraft: MinecraftDiscoverySnapshot,
    pub providers: Vec<ProviderRuntimeStatus>,
    pub downloads: DownloadManagerSnapshot,
    pub diagnostics: BackendDiagnosticsSnapshot,
}

pub use crate::application_download::QueueCatalogDownloadRequest;

#[derive(Clone)]
pub struct SearchNowBackendRuntime {
    settings: SettingsStore,
    platform: PlatformContext,
    providers: Arc<ProviderAdapterRuntime>,
    downloads: DownloadApplicationService,
    content: ContentApplicationService,
    diagnostics: DiagnosticsBuffer,
}

impl SearchNowBackendRuntime {
    pub fn new(
        paths: SearchNowBackendPaths,
        platform: PlatformContext,
        integrated_providers: Vec<Arc<dyn IntegratedProvider>>,
    ) -> BackendResult<Self> {
        let http = HttpTransport::new(HttpTransportPolicy::default())?;
        Self::compose(paths, platform, integrated_providers, http)
    }

    pub(crate) fn compose(
        paths: SearchNowBackendPaths,
        platform: PlatformContext,
        integrated_providers: Vec<Arc<dyn IntegratedProvider>>,
        http: HttpTransport,
    ) -> BackendResult<Self> {
        let diagnostics = DiagnosticsBuffer::default();
        let startup_started = Instant::now();
        diagnostics.record(
            DiagnosticComponent::Runtime,
            DiagnosticSeverity::Info,
            "backend_runtime_starting",
            "SearchNow backend runtime is starting.",
            None,
        );

        let provider_started = Instant::now();
        let providers = Arc::new(ProviderAdapterRuntime::compose(integrated_providers)?);
        diagnostics.record(
            DiagnosticComponent::Provider,
            DiagnosticSeverity::Info,
            "provider_runtime_ready",
            "Provider runtime composition is ready.",
            Some(provider_started.elapsed()),
        );

        let mut transports = DownloadTransportRegistry::new();
        transports.register(Arc::new(http.clone()))?;
        transports.register(Arc::new(ProviderResolvedTransport::new(
            providers.resolvers(),
            http,
        )))?;

        let settings = SettingsStore::new(&paths.settings_path);
        let initial_bandwidth_limit = settings
            .load()
            .ok()
            .and_then(|settings| settings.download.bandwidth_limit_bytes_per_second);

        let download_started = Instant::now();
        let downloads = DownloadExecutionRuntime::new(
            DownloadPolicy::default(),
            DownloadStore::new(&paths.download_state_path),
            &paths.download_workspace_root,
            &paths.download_destination_root,
            transports,
        )?;
        downloads.set_bandwidth_limit(initial_bandwidth_limit);
        diagnostics.record(
            DiagnosticComponent::Download,
            DiagnosticSeverity::Info,
            "download_runtime_ready",
            "Download runtime is ready.",
            Some(download_started.elapsed()),
        );

        let downloads = DownloadApplicationService::new(downloads, diagnostics.clone());
        let content = ContentApplicationService::new(
            settings.clone(),
            platform.clone(),
            diagnostics.clone(),
        );

        diagnostics.mark_ready();
        diagnostics.record(
            DiagnosticComponent::Runtime,
            DiagnosticSeverity::Info,
            "backend_runtime_ready",
            "SearchNow backend runtime is ready.",
            Some(startup_started.elapsed()),
        );

        Ok(Self {
            settings,
            platform,
            providers,
            downloads,
            content,
            diagnostics,
        })
    }

    pub fn load_settings(&self) -> BackendResult<AppSettings> {
        let started = Instant::now();
        let result = self.settings.load();
        self.diagnostics.record_outcome(
            DiagnosticComponent::Settings,
            started,
            result.is_ok(),
            "settings_load_ok",
            "Settings loaded successfully.",
            "settings_load_failed",
            "Settings could not be loaded.",
            DiagnosticSeverity::Error,
        );
        result
    }

    pub fn save_settings(&self, settings: &AppSettings) -> BackendResult<AppSettings> {
        let started = Instant::now();
        let result = self.settings.save(settings).map(|()| {
            self.downloads
                .set_bandwidth_limit(settings.download.bandwidth_limit_bytes_per_second);
            settings.clone()
        });
        self.diagnostics.record_outcome(
            DiagnosticComponent::Settings,
            started,
            result.is_ok(),
            "settings_save_ok",
            "Settings saved successfully.",
            "settings_save_failed",
            "Settings could not be saved.",
            DiagnosticSeverity::Error,
        );
        result
    }

    pub fn discover_minecraft(&self) -> BackendResult<MinecraftDiscoverySnapshot> {
        let started = Instant::now();
        let result = self.discover_minecraft_raw();
        self.diagnostics.record_outcome(
            DiagnosticComponent::Minecraft,
            started,
            result.is_ok(),
            "minecraft_discovery_ok",
            "Minecraft storage discovery completed.",
            "minecraft_discovery_failed",
            "Minecraft storage discovery could not complete.",
            DiagnosticSeverity::Error,
        );
        result
    }

    pub fn scan_local_library(&self) -> BackendResult<LocalBackendSnapshot> {
        let started = Instant::now();
        let result = self.scan_local_library_raw();
        self.diagnostics.record_outcome(
            DiagnosticComponent::Library,
            started,
            result.is_ok(),
            "library_scan_ok",
            "Local library scan completed.",
            "library_scan_failed",
            "Local library scan could not complete.",
            DiagnosticSeverity::Error,
        );
        result
    }

    pub fn query_catalog(&self, request: &CatalogRequest) -> Result<CatalogPage, CatalogError> {
        let started = Instant::now();
        let result = self.providers.catalog().query(request);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Catalog,
            started,
            result.is_ok(),
            "catalog_query_ok",
            "Catalog query completed.",
            "catalog_query_failed",
            "Catalog query could not complete.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub fn provider_status(&self) -> Vec<ProviderRuntimeStatus> {
        self.providers.status()
    }

    pub fn runtime_status(&self) -> RuntimeStatus {
        runtime_status()
    }

    pub fn diagnostics_snapshot(&self) -> BackendDiagnosticsSnapshot {
        self.diagnostics.snapshot()
    }

    pub fn diagnostics_support_report_json(&self) -> BackendResult<Vec<u8>> {
        let report = DiagnosticsSupportReport {
            schema_version: 1,
            runtime: self.runtime_status(),
            diagnostics: self.diagnostics_snapshot(),
        };
        serde_json::to_vec_pretty(&report).map_err(|error| {
            BackendError::new(
                "diagnostics_report_serialize_failed",
                format!("SearchNow could not serialize diagnostics report: {error}"),
            )
        })
    }

    pub fn snapshot(&self) -> BackendResult<BackendRuntimeSnapshot> {
        let started = Instant::now();
        let result = (|| {
            let providers = self.provider_status();
            let diagnostics = self.diagnostics_snapshot();
            let application = ApplicationState::from_runtime(&diagnostics, &providers);
            Ok(BackendRuntimeSnapshot {
                runtime: self.runtime_status(),
                lifecycle: application.lifecycle,
                capabilities: application.capabilities,
                minecraft: self.discover_minecraft_raw()?,
                providers,
                downloads: self.downloads.download_snapshot()?,
                diagnostics,
            })
        })();
        self.diagnostics.record_outcome(
            DiagnosticComponent::Runtime,
            started,
            result.is_ok(),
            "backend_snapshot_ok",
            "Backend runtime snapshot completed.",
            "backend_snapshot_failed",
            "Backend runtime snapshot could not complete.",
            DiagnosticSeverity::Error,
        );
        result
    }

    pub fn download_snapshot(&self) -> BackendResult<DownloadManagerSnapshot> {
        self.downloads.snapshot()
    }

    pub fn set_download_change_notifier<F>(&self, notifier: F)
    where
        F: Fn() + Send + Sync + 's    pub fn download_snapshot(&self) -> BackendResult<DownloadManagerSnapshot> {
        self.downloads.download_snapshot()
    }

    pub fn set_download_change_notifier<F>(&self, notifier: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.downloads.set_download_change_notifier(notifier);
    }

    pub fn queue_catalog_download(
        &self,
        request: QueueCatalogDownloadRequest,
    ) -> BackendResult<DownloadJob> {
        self.downloads.queue_catalog_download(request)
    }

kendResult<PackageImportResult> {
        self.content.import_package(request)
    }

    pub fn local_content_export_file_name(&self, item_id: &str) -> BackendResult<String> {
        self.content.local_content_export_file_name(item_id)
    }

    pub fn export_local_content(&self, item_id: &str, destination: &Path) -> BackendResult<()> {
        self.content.export_local_content(item_id, destination)
    }

    pub fn export_local_content_to_directory(
        &self,
        item_id: &str,
        destination_directory: &Path,
        duplicate_policy: ExportDuplicatePolicy,
    ) -> BackendResult<String> {
        self.content
            .export_local_content_to_directory(item_id, destination_directory, duplicate_policy)
    }

    pub fn replace_package(
        &self,
        request: PackageReplaceRequest,
    ) -> BackendResult<PackageImportResult> {
        self.content.replace_package(request)
    }

    pub fn export_local_content_batch(
        &self,
        item_ids: &[String],
        destination_directory: &Path,
    ) -> BackendResult<Vec<String>> {
        self.content
            .export_local_content_batch(item_ids, destination_directory)
    }

    pub fn export_local_content_batch_with_policy(
        &self,
        item_ids: &[String],
        destination_directory: &Path,
        duplicate_policy: ExportDuplicatePolicy,
    ) -> BackendResult<Vec<String>> {
        self.content.export_local_content_batch_with_policy(
            item_ids,
            destination_directory,
            duplicate_policy,
        )
    }

    pub fn replace_package_bundle(
        &self,
        request: PackageBundleUpdateRequest,
    ) -> BackendResult<PackageImportResult> {
        self.content.replace_package_bundle(request)
    }

    pub fn remove_local_content(&self, item_id: &str) -> BackendResult<LocalBackendSnapshot> {
        self.content.remove_local_content(item_id)
    }

    pub fn local_content_directory(&self, item_id: &str) -> BackendResult<PathBuf> {
        self.content.local_content_directory(item_id)
    }

    pub fn completed_download_directory(&self, job_id: &str) -> BackendResult<PathBuf> {
        self.downloads.completed_download_directory(job_id)
    }

    pub fn move_download_in_queue(
        &self,
        job_id: &str,
        direction: DownloadQueueMove,
    ) -> BackendResult<DownloadJob> {
        self.downloads.move_download_in_queue(job_id, direction)
    }

    pub fn pause_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        self.downloads.pause_download(job_id)
    }

    pub fn resume_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        self.downloads.resume_download(job_id)
    }

    pub fn cancel_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        self.downloads.cancel_download(job_id)
    }

    pub fn retry_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        self.downloads.retry_download(job_id)
    }

    pub fn remove_download(&self, job_id: &str) -> BackendResult<DownloadManagerSnapshot> {
        self.downloads.remove_download(job_id)
    }

    pub fn clear_completed_downloads(&self) -> BackendResult<DownloadManagerSnapshot> {
        self.downloads.clear_completed_downloads()
    }

    fn discover_minecraft_raw(&self) -> BackendResult<MinecraftDiscoverySnapshot> {
        let settings = self.settings.load()?;
        Ok(discover_minecraft_storage(
            &settings.minecraft,
            &self.platform,
        ))
    }

    fn scan_local_library_raw(&self) -> BackendResult<LocalBackendSnapshot> {
        let settings = self.settings.load()?;
        Ok(build_local_backend_snapshot(&settings, &self.platform))
    }
}

