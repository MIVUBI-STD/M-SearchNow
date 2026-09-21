use crate::{
    application_library::LibraryApplicationService,
    application_package::PackageApplicationService,
    diagnostics::DiagnosticsBuffer,
    error::BackendResult,
    package::{
        PackageBundleUpdateRequest, PackageImportRequest, PackageImportResult, PackageInspection,
        PackageReplaceRequest,
    },
    platform::PlatformContext,
    settings::{ExportDuplicatePolicy, SettingsStore},
    LocalBackendSnapshot,
};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub(crate) struct ContentApplicationService {
    library: LibraryApplicationService,
    package: PackageApplicationService,
}

impl ContentApplicationService {
    pub(crate) fn new(
        settings: SettingsStore,
        platform: PlatformContext,
        diagnostics: DiagnosticsBuffer,
    ) -> Self {
        Self {
            library: LibraryApplicationService::new(
                settings.clone(),
                platform.clone(),
                diagnostics.clone(),
            ),
            package: PackageApplicationService::new(settings, platform, diagnostics),
        }
    }

    pub(crate) fn inspect_package(&self, path: &Path) -> BackendResult<PackageInspection> {
        self.package.inspect_package(path)
    }

    pub(crate) fn import_package(
        &self,
        request: PackageImportRequest,
    ) -> BackendResult<PackageImportResult> {
        self.package.import_package(request)
    }

    pub(crate) fn local_content_export_file_name(&self, item_id: &str) -> BackendResult<String> {
        self.library.local_content_export_file_name(item_id)
    }

    pub(crate) fn export_local_content(
        &self,
        item_id: &str,
        destination: &Path,
    ) -> BackendResult<()> {
        self.library.export_local_content(item_id, destination)
    }

    pub(crate) fn export_local_content_to_directory(
        &self,
        item_id: &str,
        destination_directory: &Path,
        duplicate_policy: ExportDuplicatePolicy,
    ) -> BackendResult<String> {
        self.library.export_local_content_to_directory(
            item_id,
            destination_directory,
            duplicate_policy,
        )
    }

    pub(crate) fn replace_package(
        &self,
        request: PackageReplaceRequest,
    ) -> BackendResult<PackageImportResult> {
        self.package.replace_package(request)
    }

    pub(crate) fn export_local_content_batch(
        &self,
        item_ids: &[String],
        destination_directory: &Path,
    ) -> BackendResult<Vec<String>> {
        self.library
            .export_local_content_batch(item_ids, destination_directory)
    }

    pub(crate) fn export_local_content_batch_with_policy(
        &self,
        item_ids: &[String],
        destination_directory: &Path,
        duplicate_policy: ExportDuplicatePolicy,
    ) -> BackendResult<Vec<String>> {
        self.library.export_local_content_batch_with_policy(
            item_ids,
            destination_directory,
            duplicate_policy,
        )
    }

    pub(crate) fn replace_package_bundle(
        &self,
        request: PackageBundleUpdateRequest,
    ) -> BackendResult<PackageImportResult> {
        self.package.replace_package_bundle(request)
    }

    pub(crate) fn remove_local_content(
        &self,
        item_id: &str,
    ) -> BackendResult<LocalBackendSnapshot> {
        self.library.remove_local_content(item_id)
    }

    pub(crate) fn local_content_directory(&self, item_id: &str) -> BackendResult<PathBuf> {
        self.library.local_content_directory(item_id)
    }
}
