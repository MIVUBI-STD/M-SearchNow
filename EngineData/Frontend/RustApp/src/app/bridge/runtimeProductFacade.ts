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
import { publishApplicationEvent, type ApplicationEvent } from "../state/applicationEvents";
import { toProductError } from "../shared/productErrors";
import { runtimeApi } from "./runtimeApi";

const DOWNLOADS_CHANGED_EVENT = "searchnow://downloads-changed";

async function productRequest<T>(
  operation: () => Promise<T>,
  fallbackMessage: string,
): Promise<ProductResult<T>> {
  try {
    return { ok: true, data: await operation() };
  } catch (error) {
    return { ok: false, error: toProductError(error, fallbackMessage) };
  }
}

function productQueryCall<T>(
  operation: () => Promise<T>,
  fallbackMessage: string,
): Promise<ProductResult<T>> {
  return productRequest(operation, fallbackMessage);
}

function productActionCall<T>(
  operation: () => Promise<T>,
  fallbackMessage: string,
): Promise<ProductResult<T>> {
  return productRequest(operation, fallbackMessage);
}

async function productCommandCall<T>(
  operation: () => Promise<T>,
  fallbackMessage: string,
  event: ApplicationEvent | ((data: T) => ApplicationEvent),
): Promise<ProductResult<T>> {
  const result = await productRequest(operation, fallbackMessage);
  if (result.ok) {
    publishApplicationEvent(typeof event === "function" ? event(result.data) : event);
  }
  return result;
}

export async function loadProductRuntimeSnapshot(): Promise<ProductRuntimeSnapshot> {
  const backendResult = await productQueryCall(
    () => runtimeApi.getBackendSnapshot(),
    "SearchNow could not read the desktop runtime snapshot.",
  );

  if (backendResult.ok) {
    const { runtime, health } = backendResult.data;
    const degraded = health.state === "degraded";
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

  const runtimeResult = await productQueryCall(
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
    return productQueryCall(
      () => runtimeApi.scanLocalLibrary(),
      "SearchNow could not scan your Minecraft content.",
    );
  },

  removeLocalContent(itemId: string): Promise<ProductResult<LocalBackendSnapshot>> {
    return productCommandCall(
      () => runtimeApi.removeLocalContent(itemId),
      "SearchNow could not remove this Minecraft content.",
      { kind: "contentRemoved" },
    );
  },

  exportLocalContent(itemId: string): Promise<ProductResult<string | null>> {
    return productActionCall(
      () => runtimeApi.exportLocalContent(itemId),
      "SearchNow could not export this Minecraft content.",
    );
  },

  exportLocalContentBatch(itemIds: string[]): Promise<ProductResult<string[] | null>> {
    return productActionCall(
      () => runtimeApi.exportLocalContentBatch(itemIds),
      "SearchNow could not export the selected Minecraft content.",
    );
  },

  chooseAndInspectPackage(): Promise<ProductResult<PackageInspection | null>> {
    return productActionCall(
      () => runtimeApi.chooseAndInspectPackage(),
      "SearchNow could not inspect this Minecraft package.",
    );
  },

  chooseAndInspectPackageFolder(): Promise<ProductResult<PackageInspection | null>> {
    return productActionCall(
      () => runtimeApi.chooseAndInspectPackageFolder(),
      "SearchNow could not inspect this Minecraft package folder.",
    );
  },

  inspectPackagePath(path: string): Promise<ProductResult<PackageInspection>> {
    return productQueryCall(
      () => runtimeApi.inspectPackagePath(path),
      "SearchNow could not inspect the dropped Minecraft package.",
    );
  },

  subscribeDesktopDrops(
    handler: (event: DesktopDropEvent) => void,
  ): Promise<ProductResult<() => void>> {
    return productActionCall(
      () => runtimeApi.subscribeDesktopDrops(handler),
      "SearchNow could not listen for dropped Minecraft packages.",
    );
  },

  importPackage(request: PackageImportRequest): Promise<ProductResult<PackageImportResult>> {
    return productCommandCall(
      () => runtimeApi.importPackage(request),
      "SearchNow could not import this Minecraft package.",
      { kind: "packageImported" },
    );
  },

  replacePackage(request: PackageReplaceRequest): Promise<ProductResult<PackageImportResult>> {
    return productCommandCall(
      () => runtimeApi.replacePackage(request),
      "SearchNow could not update this installed Minecraft pack.",
      { kind: "packageReplaced" },
    );
  },

  replacePackageBundle(request: PackageBundleUpdateRequest): Promise<ProductResult<PackageImportResult>> {
    return productCommandCall(
      () => runtimeApi.replacePackageBundle(request),
      "SearchNow could not update this Minecraft add-on bundle.",
      { kind: "packageReplaced" },
    );
  },

  discoverMinecraft(): Promise<ProductResult<MinecraftDiscoverySnapshot>> {
    return productQueryCall(
      () => runtimeApi.discoverMinecraft(),
      "SearchNow could not detect Minecraft Bedrock storage.",
    );
  },

  loadSettings(): Promise<ProductResult<AppSettings>> {
    return productQueryCall(
      () => runtimeApi.loadAppSettings(),
      "SearchNow could not load your settings.",
    );
  },

  saveSettings(
    settings: AppSettings,
    previous: AppSettings | null = null,
  ): Promise<ProductResult<AppSettings>> {
    return productCommandCall(
      () => runtimeApi.saveAppSettings(settings),
      "SearchNow could not save your settings.",
      (saved) => ({ kind: "settingsChanged", previous, next: saved }),
    );
  },

  getStartupRouteOverride(): Promise<ProductResult<string | null>> {
    return productQueryCall(
      () => runtimeApi.getStartupRouteOverride(),
      "SearchNow could not read the startup route override.",
    );
  },

  chooseDownloadDirectory(): Promise<ProductResult<string | null>> {
    return productActionCall(
      () => runtimeApi.chooseDownloadDirectory(),
      "SearchNow could not open the folder picker.",
    );
  },

  chooseMinecraftDirectory(): Promise<ProductResult<string | null>> {
    return productActionCall(
      () => runtimeApi.chooseMinecraftDirectory(),
      "SearchNow could not choose a Minecraft data folder.",
    );
  },

  chooseExportDirectory(): Promise<ProductResult<string | null>> {
    return productActionCall(
      () => runtimeApi.chooseExportDirectory(),
      "SearchNow could not choose an export folder.",
    );
  },

  openLocalContentDirectory(itemId: string): Promise<ProductResult<void>> {
    return productActionCall(
      () => runtimeApi.openLocalContentDirectory(itemId),
      "SearchNow could not open this content folder.",
    );
  },

  openDownloadDirectory(jobId: string): Promise<ProductResult<void>> {
    return productActionCall(
      () => runtimeApi.openDownloadDirectory(jobId),
      "SearchNow could not open this folder.",
    );
  },

  loadDownloads(): Promise<ProductResult<DownloadManagerSnapshot>> {
    return productQueryCall(
      () => runtimeApi.getDownloadSnapshot(),
      "SearchNow could not read your downloads.",
    );
  },

  subscribeDownloadChanges(onChange: () => void): Promise<ProductResult<() => void>> {
    return productActionCall(
      () => listen(DOWNLOADS_CHANGED_EVENT, () => onChange()),
      "SearchNow could not subscribe to download updates.",
    );
  },

  queueCatalogDownload(request: QueueCatalogDownloadRequest): Promise<ProductResult<DownloadJob>> {
    return productCommandCall(
      () => runtimeApi.queueCatalogDownload(request),
      "SearchNow could not start this download.",
      { kind: "downloadChanged" },
    );
  },

  moveDownloadInQueue(
    jobId: string,
    direction: "earlier" | "later",
  ): Promise<ProductResult<DownloadJob>> {
    return productCommandCall(
      () => runtimeApi.moveDownloadInQueue(jobId, direction),
      "SearchNow could not change this download's queue position.",
      { kind: "downloadChanged" },
    );
  },

  pauseDownload(jobId: string): Promise<ProductResult<DownloadJob>> {
    return productCommandCall(
      () => runtimeApi.pauseDownload(jobId),
      "SearchNow could not pause this download.",
      { kind: "downloadChanged" },
    );
  },

  resumeDownload(jobId: string): Promise<ProductResult<DownloadJob>> {
    return productCommandCall(
      () => runtimeApi.resumeDownload(jobId),
      "SearchNow could not resume this download.",
      { kind: "downloadChanged" },
    );
  },

  cancelDownload(jobId: string): Promise<ProductResult<DownloadJob>> {
    return productCommandCall(
      () => runtimeApi.cancelDownload(jobId),
      "SearchNow could not cancel this download.",
      { kind: "downloadChanged" },
    );
  },

  retryDownload(jobId: string): Promise<ProductResult<DownloadJob>> {
    return productCommandCall(
      () => runtimeApi.retryDownload(jobId),
      "SearchNow could not retry this download.",
      { kind: "downloadChanged" },
    );
  },

  removeDownload(jobId: string): Promise<ProductResult<DownloadManagerSnapshot>> {
    return productCommandCall(
      () => runtimeApi.removeDownload(jobId),
      "SearchNow could not remove this download from the list.",
      { kind: "downloadChanged" },
    );
  },
  clearCompletedDownloads(): Promise<ProductResult<DownloadManagerSnapshot>> {
    return productCommandCall(
      () => runtimeApi.clearCompletedDownloads(),
      "SearchNow could not clear completed downloads.",
      { kind: "downloadChanged" },
    );
  },

  loadDiagnostics(): Promise<ProductResult<BackendDiagnosticsSnapshot>> {
    return productQueryCall(
      () => runtimeApi.getBackendDiagnostics(),
      "SearchNow could not read diagnostics.",
    );
  },

  exportDiagnosticsReport(): Promise<ProductResult<string | null>> {
    return productActionCall(
      () => runtimeApi.exportDiagnosticsReport(),
      "SearchNow could not export the diagnostics report.",
    );
  },

  queryCatalog(request: CatalogRequest): Promise<ProductResult<CatalogPage>> {
    return productQueryCall(
      () => runtimeApi.queryCatalog(request),
      "SearchNow could not search this content source.",
    );
  },
};
