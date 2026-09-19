import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  BackendDiagnosticsSnapshot,
  BackendRuntimeSnapshot,
  CatalogPage,
  CatalogRequest,
  DownloadJob,
  DownloadManagerSnapshot,
  LocalBackendSnapshot,
  MinecraftDiscoverySnapshot,
  QueueCatalogDownloadRequest,
  RuntimeStatus,
  PackageInspection,
  PackageImportRequest,
  PackageImportResult,
  PackageReplaceRequest,
  PackageBundleUpdateRequest,
} from "../shared/types";

export const runtimeApi = {
  getRuntimeStatus(): Promise<RuntimeStatus> {
    return invoke<RuntimeStatus>("get_runtime_status");
  },

  getBackendSnapshot(): Promise<BackendRuntimeSnapshot> {
    return invoke<BackendRuntimeSnapshot>("get_backend_snapshot");
  },

  getBackendDiagnostics(): Promise<BackendDiagnosticsSnapshot> {
    return invoke<BackendDiagnosticsSnapshot>("get_backend_diagnostics");
  },

  loadAppSettings(): Promise<AppSettings> {
    return invoke<AppSettings>("load_app_settings");
  },

  saveAppSettings(settings: AppSettings): Promise<AppSettings> {
    return invoke<AppSettings>("save_app_settings", { settings });
  },

  discoverMinecraft(): Promise<MinecraftDiscoverySnapshot> {
    return invoke<MinecraftDiscoverySnapshot>("discover_minecraft_storage_command");
  },

  scanLocalLibrary(): Promise<LocalBackendSnapshot> {
    return invoke<LocalBackendSnapshot>("scan_local_library");
  },

  removeLocalContent(itemId: string): Promise<LocalBackendSnapshot> {
    return invoke<LocalBackendSnapshot>("remove_local_content", { itemId });
  },

  exportLocalContent(itemId: string): Promise<string | null> {
    return invoke<string | null>("export_local_content", { itemId });
  },

  exportLocalContentBatch(itemIds: string[]): Promise<string[] | null> {
    return invoke<string[] | null>("export_local_content_batch", { itemIds });
  },

  chooseAndInspectPackage(): Promise<PackageInspection | null> {
    return invoke<PackageInspection | null>("choose_and_inspect_package");
  },

  chooseAndInspectPackageFolder(): Promise<PackageInspection | null> {
    return invoke<PackageInspection | null>("choose_and_inspect_package_folder");
  },

  importPackage(request: PackageImportRequest): Promise<PackageImportResult> {
    return invoke<PackageImportResult>("import_package", { request });
  },

  replacePackage(request: PackageReplaceRequest): Promise<PackageImportResult> {
    return invoke<PackageImportResult>("replace_package", { request });
  },

  replacePackageBundle(request: PackageBundleUpdateRequest): Promise<PackageImportResult> {
    return invoke<PackageImportResult>("replace_package_bundle", { request });
  },

  chooseDownloadDirectory(): Promise<string | null> {
    return invoke<string | null>("choose_download_directory");
  },

  openLocalContentDirectory(itemId: string): Promise<void> {
    return invoke<void>("open_local_content_directory", { itemId });
  },

  openDownloadDirectory(jobId: string): Promise<void> {
    return invoke<void>("open_download_directory", { jobId });
  },

  getDownloadSnapshot(): Promise<DownloadManagerSnapshot> {
    return invoke<DownloadManagerSnapshot>("get_download_snapshot");
  },

  queueCatalogDownload(request: QueueCatalogDownloadRequest): Promise<DownloadJob> {
    return invoke<DownloadJob>("queue_catalog_download", { request });
  },

  moveDownloadInQueue(jobId: string, direction: "earlier" | "later"): Promise<DownloadJob> {
    return invoke<DownloadJob>("move_download_in_queue", { jobId, direction });
  },

  pauseDownload(jobId: string): Promise<DownloadJob> {
    return invoke<DownloadJob>("pause_download", { jobId });
  },

  resumeDownload(jobId: string): Promise<DownloadJob> {
    return invoke<DownloadJob>("resume_download", { jobId });
  },

  cancelDownload(jobId: string): Promise<DownloadJob> {
    return invoke<DownloadJob>("cancel_download", { jobId });
  },

  retryDownload(jobId: string): Promise<DownloadJob> {
    return invoke<DownloadJob>("retry_download", { jobId });
  },

  removeDownload(jobId: string): Promise<DownloadManagerSnapshot> {
    return invoke<DownloadManagerSnapshot>("remove_download", { jobId });
  },

  queryCatalog(request: CatalogRequest): Promise<CatalogPage> {
    return invoke<CatalogPage>("query_catalog", { request });
  },
};
