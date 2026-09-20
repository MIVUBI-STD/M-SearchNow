import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type {
  AppSettings,
  BackendDiagnosticsSnapshot,
  BackendRuntimeSnapshot,
  CatalogPage,
  CatalogRequest,
  DesktopDropEvent,
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

  exportDiagnosticsReport(): Promise<string | null> {
    return invoke<string | null>("export_diagnostics_report");
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

  inspectPackagePath(path: string): Promise<PackageInspection> {
    return invoke<PackageInspection>("inspect_package_path", { path });
  },

  subscribeDesktopDrops(handler: (event: DesktopDropEvent) => void): Promise<() => void> {
    return getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "drop") {
        handler({ type: "drop", paths: payload.paths });
      } else if (payload.type === "over") {
        handler({ type: "over" });
      } else {
        handler({ type: "cancel" });
      }
    });
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

  getStartupRouteOverride(): Promise<string | null> {
    return invoke<string | null>("get_startup_route_override");
  },

  chooseDownloadDirectory(): Promise<string | null> {
    return invoke<string | null>("choose_download_directory");
  },

  chooseMinecraftDirectory(): Promise<string | null> {
    return invoke<string | null>("choose_minecraft_directory");
  },

  chooseExportDirectory(): Promise<string | null> {
    return invoke<string | null>("choose_export_directory");
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
  clearCompletedDownloads(): Promise<DownloadManagerSnapshot> {
    return invoke<DownloadManagerSnapshot>("clear_completed_downloads");
  },

  queryCatalog(request: CatalogRequest): Promise<CatalogPage> {
    return invoke<CatalogPage>("query_catalog", { request });
  },
};
