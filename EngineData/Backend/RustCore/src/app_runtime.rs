use crate::{
    build_local_backend_snapshot,
    catalog::{CatalogDownloadRef, CatalogError, CatalogPage, CatalogRequest},
    diagnostics::{
        BackendDiagnosticsSnapshot, DiagnosticComponent, DiagnosticSeverity, DiagnosticsBuffer,
    },
    download::{
        default_download_paths, DownloadExecutionRuntime, DownloadJob, DownloadJobState,
        DownloadManagerSnapshot, DownloadPolicy, DownloadRequest, DownloadStore,
        DownloadTransportRegistry, HttpTransport, HttpTransportPolicy, ProviderResolvedTransport,
    },
    error::{BackendError, BackendResult},
    library::{scan_library, valid_local_content_id, LocalContentType},
    library_export::export_directory,
    minecraft::{discover_minecraft_storage, MinecraftDiscoverySnapshot},
    package::{
        import_archive, inspect_package, replace_single_pack, PackageImportRequest,
        PackageImportResult, PackageInspection, PackageReplaceRequest,
    },
    platform::PlatformContext,
    provider_adapter::{IntegratedProvider, ProviderAdapterRuntime, ProviderRuntimeStatus},
    runtime::{runtime_status, RuntimeStatus},
    settings::{AppSettings, SettingsStore},
    LocalBackendSnapshot,
};
use serde::{Deserialize, Serialize};
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
pub struct BackendRuntimeSnapshot {
    pub runtime: RuntimeStatus,
    pub minecraft: MinecraftDiscoverySnapshot,
    pub providers: Vec<ProviderRuntimeStatus>,
    pub downloads: DownloadManagerSnapshot,
    pub diagnostics: BackendDiagnosticsSnapshot,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueueCatalogDownloadRequest {
    pub download: CatalogDownloadRef,
    pub display_name: String,
    pub destination_file_name: String,
    pub destination_directory: Option<PathBuf>,
    pub expected_bytes: Option<u64>,
}

fn parse_numeric_version(value: &str) -> Option<Vec<u32>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .split('.')
        .map(|part| part.parse::<u32>().ok())
        .collect()
}

fn compare_numeric_versions(left: &[u32], right: &[u32]) -> std::cmp::Ordering {
    let length = left.len().max(right.len());
    for index in 0..length {
        let left_part = left.get(index).copied().unwrap_or(0);
        let right_part = right.get(index).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    std::cmp::Ordering::Equal
}

#[derive(Clone)]
pub struct SearchNowBackendRuntime {
    settings: SettingsStore,
    platform: PlatformContext,
    providers: Arc<ProviderAdapterRuntime>,
    downloads: DownloadExecutionRuntime,
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

        let download_started = Instant::now();
        let downloads = DownloadExecutionRuntime::new(
            DownloadPolicy::default(),
            DownloadStore::new(&paths.download_state_path),
            &paths.download_workspace_root,
            &paths.download_destination_root,
            transports,
        )?;
        diagnostics.record(
            DiagnosticComponent::Download,
            DiagnosticSeverity::Info,
            "download_runtime_ready",
            "Download runtime is ready.",
            Some(download_started.elapsed()),
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
            settings: SettingsStore::new(paths.settings_path),
            platform,
            providers,
            downloads,
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
        let result = self.settings.save(settings).map(|()| settings.clone());
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

    pub fn snapshot(&self) -> BackendResult<BackendRuntimeSnapshot> {
        let started = Instant::now();
        let result = (|| {
            Ok(BackendRuntimeSnapshot {
                runtime: self.runtime_status(),
                minecraft: self.discover_minecraft_raw()?,
                providers: self.provider_status(),
                downloads: self.downloads.snapshot()?,
                diagnostics: self.diagnostics_snapshot(),
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

    pub fn queue_catalog_download(
        &self,
        request: QueueCatalogDownloadRequest,
    ) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = (|| {
            let source = request.download.to_download_source()?;
            self.downloads.queue_to(
                DownloadRequest {
                    source,
                    display_name: request.display_name,
                    destination_file_name: request.destination_file_name,
                    expected_bytes: request.expected_bytes,
                },
                request.destination_directory,
            )
        })();
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_queue_ok",
            "Catalog download job queued.",
            "download_queue_failed",
            "Catalog download job could not be queued.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub fn inspect_package(&self, path: &Path) -> BackendResult<PackageInspection> {
        inspect_package(path)
    }

    pub fn import_package(
        &self,
        request: PackageImportRequest,
    ) -> BackendResult<PackageImportResult> {
        let settings = self.settings.load()?;
        let discovery = discover_minecraft_storage(&settings.minecraft, &self.platform);
        let root = discovery
            .roots
            .iter()
            .find(|root| root.id == request.root_id)
            .ok_or_else(|| {
                BackendError::new(
                    "package_import_root_unavailable",
                    "The selected Minecraft storage root is no longer available.",
                )
            })?;
        let inspection = inspect_package(&request.source_path)?;
        let target_library = scan_library(
            std::slice::from_ref(root),
            settings.minecraft.include_development_content,
        );
        let installed_uuids = target_library
            .items
            .iter()
            .filter_map(|item| item.manifest_uuid.as_deref())
            .map(str::to_ascii_lowercase)
            .collect::<std::collections::HashSet<_>>();
        let conflict = inspection
            .packs
            .iter()
            .filter_map(|pack| pack.uuid.as_deref())
            .map(str::to_ascii_lowercase)
            .any(|uuid| installed_uuids.contains(&uuid));
        if conflict {
            return Err(BackendError::new(
                "package_import_conflict",
                "A pack with the same manifest UUID is already installed in the selected Minecraft storage.",
            ));
        }
        import_archive(&request.source_path, &root.root, &root.id)
    }

    pub fn local_content_export_file_name(&self, item_id: &str) -> BackendResult<String> {
        let item = self.resolve_local_content(item_id)?;
        let extension = match item.content_type {
            LocalContentType::World => "mcworld",
            LocalContentType::BehaviorPack
            | LocalContentType::ResourcePack
            | LocalContentType::SkinPack => "mcpack",
        };
        let mut base = item
            .title
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.') {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        base = base.trim_matches([' ', '.']).trim().to_string();
        if base.is_empty() {
            base = "Minecraft backup".into();
        }
        Ok(format!("{base}.{extension}"))
    }

    pub fn export_local_content(&self, item_id: &str, destination: &Path) -> BackendResult<()> {
        let item = self.resolve_local_content(item_id)?;
        let expected_extension = match item.content_type {
            LocalContentType::World => "mcworld",
            LocalContentType::BehaviorPack
            | LocalContentType::ResourcePack
            | LocalContentType::SkinPack => "mcpack",
        };
        let valid_extension = destination
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(expected_extension));
        if !valid_extension {
            return Err(BackendError::new(
                "library_export_extension_invalid",
                format!("This content must be exported as .{expected_extension}."),
            ));
        }
        export_directory(&item.path, destination)
    }

    pub fn replace_package(
        &self,
        request: PackageReplaceRequest,
    ) -> BackendResult<PackageImportResult> {
        let settings = self.settings.load()?;
        let discovery = discover_minecraft_storage(&settings.minecraft, &self.platform);
        let root = discovery
            .roots
            .iter()
            .find(|root| root.id == request.root_id)
            .ok_or_else(|| {
                BackendError::new(
                    "package_import_root_unavailable",
                    "The selected Minecraft storage root is no longer available.",
                )
            })?;

        let inspection = inspect_package(&request.source_path)?;
        if inspection.input_kind != crate::package::PackageInputKind::McPack
            || inspection.packs.len() != 1
            || inspection.status != crate::package::PackageInspectionStatus::Ready
            || inspection.safety != crate::package::PackageSafety::Safe
        {
            return Err(BackendError::new(
                "package_replace_not_supported",
                "Automatic update supports one inspected .mcpack at a time.",
            ));
        }
        let incoming = inspection.packs.first().ok_or_else(|| {
            BackendError::new(
                "package_replace_not_supported",
                "Automatic update requires exactly one inspected pack.",
            )
        })?;
        let incoming_uuid = incoming.uuid.as_deref().ok_or_else(|| {
            BackendError::new(
                "package_replace_uuid_missing",
                "This pack cannot be updated automatically because it has no manifest UUID.",
            )
        })?;
        let library = scan_library(
            std::slice::from_ref(root),
            settings.minecraft.include_development_content,
        );
        let mut conflicts = library.items.iter().filter(|item| {
            item.manifest_uuid
                .as_deref()
                .is_some_and(|uuid| uuid.eq_ignore_ascii_case(incoming_uuid))
        });
        let existing = conflicts.next().ok_or_else(|| {
            BackendError::new(
                "package_replace_target_missing",
                "No installed pack with this manifest UUID was found.",
            )
        })?;
        if conflicts.next().is_some() {
            return Err(BackendError::new(
                "package_replace_target_ambiguous",
                "More than one installed pack uses this manifest UUID.",
            ));
        }
        let expected_type = match incoming.kind {
            crate::package::PackKind::BehaviorPack => LocalContentType::BehaviorPack,
            crate::package::PackKind::ResourcePack => LocalContentType::ResourcePack,
            crate::package::PackKind::SkinPack => LocalContentType::SkinPack,
            _ => {
                return Err(BackendError::new(
                    "package_replace_kind_unsupported",
                    "This pack type is not supported by automatic update.",
                ))
            }
        };
        if existing.content_type != expected_type {
            return Err(BackendError::new(
                "package_replace_type_mismatch",
                "The installed content type does not match the incoming pack.",
            ));
        }
        let incoming_version = incoming
            .version
            .as_deref()
            .and_then(parse_numeric_version)
            .ok_or_else(|| {
                BackendError::new(
                    "package_replace_version_invalid",
                    "The incoming pack version is missing or not numeric.",
                )
            })?;
        if existing.version.is_empty() {
            return Err(BackendError::new(
                "package_replace_installed_version_missing",
                "The installed pack version is unavailable, so SearchNow will not replace it automatically.",
            ));
        }
        match compare_numeric_versions(&incoming_version, &existing.version) {
            std::cmp::Ordering::Equal => {
                return Err(BackendError::new(
                    "package_replace_same_version",
                    "This exact pack version is already installed.",
                ));
            }
            std::cmp::Ordering::Less => {
                return Err(BackendError::new(
                    "package_replace_older_version",
                    "The incoming pack is older than the installed version.",
                ));
            }
            std::cmp::Ordering::Greater => {}
        }
        if !existing.path.starts_with(&root.root) || existing.path == root.root {
            return Err(BackendError::new(
                "package_replace_path_rejected",
                "SearchNow refused to update content outside its detected Minecraft storage.",
            ));
        }

        replace_single_pack(&request.source_path, &root.root, &root.id, &existing.path)
    }

    pub fn export_local_content_batch(
        &self,
        item_ids: &[String],
        destination_directory: &Path,
    ) -> BackendResult<Vec<String>> {
        if item_ids.is_empty() {
            return Err(BackendError::new(
                "library_batch_export_empty",
                "Select at least one Minecraft content item to export.",
            ));
        }
        if item_ids.len() > 100 {
            return Err(BackendError::new(
                "library_batch_export_too_large",
                "SearchNow exports up to 100 Library items at a time.",
            ));
        }
        if !destination_directory.is_dir() {
            return Err(BackendError::new(
                "library_export_destination_invalid",
                "The selected export folder is not available.",
            ));
        }

        let mut destinations = Vec::with_capacity(item_ids.len());
        let mut reserved = std::collections::HashSet::new();
        for item_id in item_ids {
            let item = self.resolve_local_content(item_id)?;
            let suggested = self.local_content_export_file_name(item_id)?;
            let destination = next_batch_export_destination(
                destination_directory,
                &suggested,
                &mut reserved,
            )?;
            destinations.push((item.id, destination));
        }

        let mut created = Vec::new();
        let result = (|| {
            for (item_id, destination) in &destinations {
                self.export_local_content(item_id, destination)?;
                created.push(destination.clone());
            }
            Ok(created
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect())
        })();

        if result.is_err() {
            for path in created.iter().rev() {
                let _ = std::fs::remove_file(path);
            }
        }
        result
    }

    pub fn remove_local_content(&self, item_id: &str) -> BackendResult<LocalBackendSnapshot> {
        if !valid_local_content_id(item_id) {
            return Err(BackendError::new(
                "library_item_not_found",
                "The selected Minecraft content is no longer available.",
            ));
        }

        let settings = self.settings.load()?;
        let discovery = discover_minecraft_storage(&settings.minecraft, &self.platform);
        let library = scan_library(
            &discovery.roots,
            settings.minecraft.include_development_content,
        );
        let item = library
            .items
            .iter()
            .find(|item| item.id == item_id)
            .ok_or_else(|| {
                BackendError::new(
                    "library_item_not_found",
                    "The selected Minecraft content is no longer available.",
                )
            })?;
        let root = discovery
            .roots
            .iter()
            .find(|root| root.id == item.root_id)
            .ok_or_else(|| {
                BackendError::new(
                    "library_root_unavailable",
                    "The Minecraft storage root for this content is no longer available.",
                )
            })?;

        if !item.path.starts_with(&root.root) || item.path == root.root {
            return Err(BackendError::new(
                "library_remove_path_rejected",
                "SearchNow refused to remove content outside its detected Minecraft storage.",
            ));
        }

        let metadata = std::fs::symlink_metadata(&item.path).map_err(|error| {
            BackendError::from_io(
                "library_remove_metadata_failed",
                "SearchNow could not verify the selected Minecraft content.",
                error,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(BackendError::new(
                "library_remove_path_rejected",
                "SearchNow only removes detected Minecraft content directories.",
            ));
        }

        std::fs::remove_dir_all(&item.path).map_err(|error| {
            BackendError::from_io(
                "library_remove_failed",
                "SearchNow could not remove the selected Minecraft content.",
                error,
            )
        })?;
        self.scan_local_library_raw()
    }

    pub fn local_content_directory(&self, item_id: &str) -> BackendResult<PathBuf> {
        Ok(self.resolve_local_content(item_id)?.path)
    }

    fn resolve_local_content(
        &self,
        item_id: &str,
    ) -> BackendResult<crate::library::LocalContentItem> {
        if !valid_local_content_id(item_id) {
            return Err(BackendError::new(
                "library_item_not_found",
                "The selected Minecraft content is no longer available.",
            ));
        }
        let snapshot = self.scan_local_library_raw()?;
        snapshot
            .library
            .items
            .into_iter()
            .find(|item| item.id == item_id)
            .ok_or_else(|| {
                BackendError::new(
                    "library_item_not_found",
                    "The selected Minecraft content is no longer available.",
                )
            })
    }

    pub fn completed_download_directory(&self, job_id: &str) -> BackendResult<PathBuf> {
        let snapshot = self.downloads.snapshot()?;
        let job = snapshot
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .ok_or_else(|| {
                BackendError::new("download_job_not_found", "Download job was not found.")
            })?;
        if job.state != DownloadJobState::Completed {
            return Err(BackendError::new(
                "download_directory_not_ready",
                "The download folder is available only after the download completes.",
            ));
        }
        job.destination_directory.clone().ok_or_else(|| {
            BackendError::new(
                "download_directory_unavailable",
                "This download does not have a user-selected destination folder.",
            )
        })
    }

    pub fn cancel_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = self.downloads.cancel(job_id);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_cancel_ok",
            "Download cancellation requested.",
            "download_cancel_failed",
            "Download cancellation could not be requested.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub fn retry_download(&self, job_id: &str) -> BackendResult<DownloadJob> {
        let started = Instant::now();
        let result = self.downloads.retry(job_id);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_retry_ok",
            "Download retry queued.",
            "download_retry_failed",
            "Download retry could not be queued.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub fn remove_download(&self, job_id: &str) -> BackendResult<DownloadManagerSnapshot> {
        let started = Instant::now();
        let result = self.downloads.remove_terminal(job_id);
        self.diagnostics.record_outcome(
            DiagnosticComponent::Download,
            started,
            result.is_ok(),
            "download_remove_ok",
            "Terminal download job removed.",
            "download_remove_failed",
            "Terminal download job could not be removed.",
            DiagnosticSeverity::Warning,
        );
        result
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

fn next_batch_export_destination(
    directory: &Path,
    suggested: &str,
    reserved: &mut std::collections::HashSet<PathBuf>,
) -> BackendResult<PathBuf> {
    let suggested_path = Path::new(suggested);
    let extension = suggested_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let stem = suggested_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Minecraft backup");

    for sequence in 1_u32..=u32::MAX {
        let file_name = if sequence == 1 {
            suggested.to_string()
        } else if extension.is_empty() {
            format!("{stem} ({sequence})")
        } else {
            format!("{stem} ({sequence}).{extension}")
        };
        let candidate = directory.join(file_name);
        if !candidate.exists() && reserved.insert(candidate.clone()) {
            return Ok(candidate);
        }
    }

    Err(BackendError::new(
        "library_batch_export_destination_exhausted",
        "SearchNow could not allocate a unique backup filename.",
    ))
}
