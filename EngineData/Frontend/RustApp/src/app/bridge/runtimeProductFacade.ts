import { listen } from "@tauri-apps/api/event";
import type {
  AppSettings,
  BackendDiagnosticsSnapshot,
  CatalogPage,
  CatalogRequest,
  DesktopDropEvent,
  DownloadJob,
  DownloadManagerSnapshot,
  LocalBackendSnapshot,
  MinecraftDiscoverySnapshot,
  ProductResult,
  ProductRuntimeSnapshot,
  QueueCatalogDownloadRequest,
  PackageInspection,
  PackageImportRequest,
  PackageImportResult,
  PackageReplaceRequest,
  PackageBundleUpdateRequest,
} from "../shared/types";
import { publishApplicationChanges, settingsChangeSet, type ApplicationChangeSet } from "../state/applicationChanges";
import { toProductError } from "../shared/productErrors";
import { runtimeApi } from "./runtimeApi";

const DOWNLOADS_CHANGED_EVENT = "searchnow://downloads-changed";

async function productCall<T>(operation: () => Promise<T>, fallbackMessage: string): Promise<ProductResult<T>> {
  try {
    return { ok: true, data: await operation() };
  } catch (error) {
    return { ok: false, error: toProductError(error, fallbackMessage) };
  }
}

async function productMutationCall<T>(
  operation: () => Promise<T>,
  fallbackMessage: string,
  changes: ApplicationChangeSet | ((data: T) => ApplicationChangeSet),
): Promise<ProductResult<T>> {
  const result = await productCall(operation, fallbackMessage);
  if (result.ok) {
    publishApplicationChanges(typeof changes === "function" ? changes(result.data) : changes);
  }
  return result;
}

export async function loadProductRuntimeSnapshot(): Promise<ProductRuntimeSnapshot> {
  const backendResult = await productCall(
    () => runtimeApi.getBackendSnapshot(),
    "SearchNow could not read the desktop runtime snapshot.",
  );

  if (backendResult.ok) {
    const { runtime, diagnostics } = backendResult.data;
    const degraded = diagnostics.health.state === "degraded";
    return {
      ready: runtime.appReady,
      summary: !runtime.appReady
        ? "Desktop runtime unavailable"
        : degraded
          ? "Desktop runtime ready with diagnostics"
          : "Desktop runtime ready",
      runtime,
      backend: backendResult.data,
      error: null,
    };
  }

  const runtimeResult = await productCall(
    () => runtimeApi.getRuntimeStatus(),
    "SearchNow could not connect to the desktop runtime.",
  );
  if (runtimeResult.ok) {
    return {
      ready: runtimeResult.data.appReady,
      summary: runtimeResult.data.appReady ? "Desktop runtime ready" : "Desktop runtime unavailable",
      runtime: runtimeResult.data,
      backend: null,
      error: backendResult.error,
    };
  }

  return {
    ready: false,
    summary: "Desktop runtime unavailable",
    runtime: null,
    backend: null,
    error: runtimeResult.error,
  };
}

export const runtimeProductFacade = {
  loadProductRuntimeSnapshot,

  loadLibrary(): Promise<ProductResult<LocalBackendSnapshot>> {
    return productCall(
      () => runtimeApi.scanLocalLibrary(),
      "SearchNow could not scan your Minecraft content.",
    );
  },

  removeLocalContent(itemId: string): Promise<ProductResult<LocalBackendSnapshot>> {
    return productMutationCall(
      () => runtimeApi.removeLocalContent(itemId),
      "SearchNow could not remove this Minecraft content.",
      { library: true, diagnostics: true },
    );
  },

  exportLocalContent(itemId: string): Promise<ProductResult<string | null>> {
    return productCall(
      () => runtimeApi.exportLocalContent(itemId),
      "SearchNow could not export this Minecraft content.",
    );
  },

  exportLocalContentBatch(itemIds: string[]): Promise<ProductResult<string[] | null>> {
    return productCall(
      () => runtimeApi.exportLocalContentBatch(itemIds),
      "SearchNow could not export the selected Minecraft content.",
    );
  },

  chooseAndInspectPackage(): Promise<ProductResult<PackageInspection | null>> {
    return productCall(
      () => runtimeApi.chooseAndInspectPackage(),
      "SearchNow could not inspect this Minecraft package.",
    );
  },

  chooseAndInspectPackageFolder(): Promise<ProductResult<PackageInspection | null>> {
    return productCall(
      () => runtimeApi.chooseAndInspectPackageFolder(),
      "SearchNow could not inspect this Minecraft package folder.",
    );
  },

  inspectPackagePath(path: string): Promise<ProductResult<PackageInspection>> {
    return productCall(
      () => runtimeApi.inspectPackagePath(path),
      "SearchNow could not inspect the dropped Minecraft package.",
    );
  },

  subscribeDesktopDrops(
    handler: (event: DesktopDropEvent) => void,
  ): Promise<ProductResult<() => void>> {
    return productCall(
      () => runtimeApi.subscribeDesktopDrops(handler),
      "SearchNow could not listen for dropped Minecraft packages.",
    );
  },

  importPackage(request: PackageImportRequest): Promise<ProductResult<PackageImportResult>> {
    return productMutationCall(
      () => runtimeApi.importPackage(request),
      "SearchNow could not import this Minecraft package.",
      { library: true, diagnostics: true },
    );
  },

  replacePackage(request: PackageReplaceRequest): Promise<ProductResult<PackageImportResult>> {
    return productMutationCall(
      () => runtimeApi.replacePackage(request),
      "SearchNow could not update this installed Minecraft pack.",
      { library: true, diagnostics: true },
    );
  },

  replacePackageBundle(request: PackageBundleUpdateRequest): Promise<ProductResult<PackageImportResult>> {
    return productMutationCall(
      () => runtimeApi.replacePackageBundle(request),
      "SearchNow could not update this Minecraft add-on bundle.",
      { library: true, diagnostics: true },
    );
  },

  discoverMinecraft(): Promise<ProductResult<MinecraftDiscoverySnapshot>> {
    return productCall(
      () => runtimeApi.discoverMinecraft(),
      "SearchNow could not detect Minecraft Bedrock storage.",
    );
  },

  loadSettings(): Promise<ProductResult<AppSettings>> {
    return productCall(
      () => runtimeApi.loadAppSettings(),
      "SearchNow could not load your settings.",
    );
  },

  saveSettings(
    settings: AppSettings,
    previous: AppSettings | null = null,
  ): Promise<ProductResult<AppSettings>> {
    return productMutationCall(
      () => runtimeApi.saveAppSettings(settings),
      "SearchNow could not save your settings.",
      (saved) => settingsChangeSet(previous, saved),
    );
  },

  getStartupRouteOverride(): Promise<ProductResult<string | null>> {
    return productCall(
      () => runtimeApi.getStartupRouteOverride(),
      "SearchNow could not read the startup route override.",
    );
  },

  chooseDownloadDirectory(): Promise<ProductResult<string | null>> {
    return productCall(
      () => runtimeApi.chooseDownloadDirectory(),
      "SearchNow could not open the folder picker.",
    );
  },

  chooseMinecraftDirectory(): Promise<ProductResult<string | null>> {
    return productCall(
      () => runtimeApi.chooseMinecraftDirectory(),
      "SearchNow could not choose a Minecraft data folder.",
    );
  },

  chooseExportDirectory(): Promise<ProductResult<string | null>> {
    return productCall(
      () => runtimeApi.chooseExportDirectory(),
      "SearchNow could not choose an export folder.",
    );
  },

  openLocalContentDirectory(itemId: string): Promise<ProductResult<void>> {
    return productCall(
      () => runtimeApi.openLocalContentDirectory(itemId),
      "SearchNow could not open this content folder.",
    );
  },

  openDownloadDirectory(jobId: string): Promise<ProductResult<void>> {
    return productCall(
      () => runtimeApi.openDownloadDirectory(jobId),
      "SearchNow could not open this folder.",
    );
  },

  loadDownloads(): Promise<ProductResult<DownloadManagerSnapshot>> {
    return productCall(
      () => runtimeApi.getDownloadSnapshot(),
      "SearchNow could not read your downloads.",
    );
  },

  subscribeDownloadChanges(onChange: () => void): Promise<ProductResult<() => void>> {
    return productCall(
      () => listen(DOWNLOADS_CHANGED_EVENT, () => onChange()),
      "SearchNow could not subscribe to download updates.",
    );
  },

  queueCatalogDownload(request: QueueCatalogDownloadRequest): Promise<ProductResult<DownloadJob>> {
    return productMutationCall(
      () => runtimeApi.queueCatalogDownload(request),
      "SearchNow could not start this download.",
      { downloads: true, diagnostics: true },
    );
  },

  moveDownloadInQueue(
    jobId: string,
    direction: "earlier" | "later",
  ): Promise<ProductResult<DownloadJob>> {
    return productMutationCall(
      () => runtimeApi.moveDownloadInQueue(jobId, direction),
      "SearchNow could not change this download's queue position.",
      { downloads: true, diagnostics: true },
    );
  },

  pauseDownload(jobId: string): Promise<ProductResult<DownloadJob>> {
    return productMutationCall(
      () => runtimeApi.pauseDownload(jobId),
      "SearchNow could not pause this download.",
      { downloads: true, diagnostics: true },
    );
  },

  resumeDownload(jobId: string): Promise<ProductResult<DownloadJob>> {
    return productMutationCall(
      () => runtimeApi.resumeDownload(jobId),
      "SearchNow could not resume this download.",
      { downloads: true, diagnostics: true },
    );
  },

  cancelDownload(jobId: string): Promise<ProductResult<DownloadJob>> {
    return productMutationCall(
      () => runtimeApi.cancelDownload(jobId),
      "SearchNow could not cancel this download.",
      { downloads: true, diagnostics: true },
    );
  },

  retryDownload(jobId: string): Promise<ProductResult<DownloadJob>> {
    return productMutationCall(
      () => runtimeApi.retryDownload(jobId),
      "SearchNow could not retry this download.",
      { downloads: true, diagnostics: true },
    );
  },

  removeDownload(jobId: string): Promise<ProductResult<DownloadManagerSnapshot>> {
    return productMutationCall(
      () => runtimeApi.removeDownload(jobId),
      "SearchNow could not remove this download from the list.",
      { downloads: true, diagnostics: true },
    );
  },
  clearCompletedDownloads(): Promise<ProductResult<DownloadManagerSnapshot>> {
    return productMutationCall(
      () => runtimeApi.clearCompletedDownloads(),
      "SearchNow could not clear completed downloads.",
      { downloads: true, diagnostics: true },
    );
  },

  loadDiagnostics(): Promise<ProductResult<BackendDiagnosticsSnapshot>> {
    return productCall(
      () => runtimeApi.getBackendDiagnostics(),
      "SearchNow could not read diagnostics.",
    );
  },

  exportDiagnosticsReport(): Promise<ProductResult<string | null>> {
    return productCall(
      () => runtimeApi.exportDiagnosticsReport(),
      "SearchNow could not export the diagnostics report.",
    );
  },

  queryCatalog(request: CatalogRequest): Promise<ProductResult<CatalogPage>> {
    return productCall(
      () => runtimeApi.queryCatalog(request),
      "SearchNow could not search this content source.",
    );
  },
};
