import type {
  CatalogItem,
  DownloadJob,
  ProductResult,
  QueueCatalogDownloadRequest,
} from "../shared/types";
import { runtimeProductFacade } from "../bridge/runtimeProductFacade";

export type CatalogDownloadWorkflowResult =
  | {
      kind: "queued";
      job: DownloadJob;
      preferenceWarning: string | null;
    }
  | {
      kind: "cancelled";
    };

function unavailable(): ProductResult<CatalogDownloadWorkflowResult> {
  return {
    ok: false,
    error: {
      code: "catalog_download_unavailable",
      message: "This catalog item does not expose a downloadable file.",
    },
  };
}

export async function startCatalogDownloadWorkflow(
  item: CatalogItem,
): Promise<ProductResult<CatalogDownloadWorkflowResult>> {
  if (!item.download || !item.fileName) return unavailable();

  const settings = await runtimeProductFacade.loadSettings();
  let destinationDirectory = settings.ok
    ? settings.data.download.defaultDirectory
    : null;

  if (!destinationDirectory) {
    const picker = await runtimeProductFacade.chooseDownloadDirectory();
    if (!picker.ok) return { ok: false, error: picker.error };
    if (!picker.data) return { ok: true, data: { kind: "cancelled" } };
    destinationDirectory = picker.data;
  }

  const request: QueueCatalogDownloadRequest = {
    download: item.download,
    displayName: item.title,
    destinationFileName: item.fileName,
    destinationDirectory,
    expectedBytes: item.expectedBytes,
    expectedSha256: item.expectedSha256,
  };

  let queued = await runtimeProductFacade.queueCatalogDownload(request);
  let recoveredDefaultDirectory: string | null = null;

  const staleDefaultDirectory =
    settings.ok &&
    settings.data.download.defaultDirectory !== null &&
    !queued.ok &&
    [
      "download_destination_directory_invalid",
      "download_destination_create_failed",
    ].includes(queued.error.code);

  if (staleDefaultDirectory) {
    const picker = await runtimeProductFacade.chooseDownloadDirectory();
    if (!picker.ok) return { ok: false, error: picker.error };
    if (!picker.data) return { ok: true, data: { kind: "cancelled" } };

    recoveredDefaultDirectory = picker.data;
    queued = await runtimeProductFacade.queueCatalogDownload({
      ...request,
      destinationDirectory: recoveredDefaultDirectory,
    });
  }

  if (!queued.ok) return { ok: false, error: queued.error };

  let preferenceWarning: string | null = null;
  if (recoveredDefaultDirectory && settings.ok) {
    const save = await runtimeProductFacade.saveSettings({
      ...settings.data,
      download: {
        ...settings.data.download,
        defaultDirectory: recoveredDefaultDirectory,
      },
    });
    if (!save.ok) preferenceWarning = save.error.message;
  }

  return {
    ok: true,
    data: {
      kind: "queued",
      job: queued.data,
      preferenceWarning,
    },
  };
}
