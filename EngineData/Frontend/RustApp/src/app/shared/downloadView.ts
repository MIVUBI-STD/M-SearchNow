import type { DownloadJob, DownloadManagerSnapshot } from "./types";

export type DownloadFilter = "all" | "active" | "completed" | "issues";

type TransferSample = {
  bytes: number;
  sampledAtMs: number;
  bytesPerSecond: number | null;
};

const SPEED_EMA_ALPHA = 0.35;
const MAX_REASONABLE_ETA_SECONDS = 7 * 24 * 60 * 60;

export function displayRank(job: DownloadJob): number {
  if (["preparing", "transferring", "pauseRequested", "finalizing", "cancelRequested"].includes(job.state)) return 0;
  if (job.state === "queued") return 1;
  if (["paused", "interrupted"].includes(job.state)) return 2;
  return 3;
}

export function queueIndex(job: DownloadJob, queuedJobs: DownloadJob[]): number {
  return queuedJobs.findIndex((candidate) => candidate.id === job.id);
}

export function canMoveEarlier(job: DownloadJob, queuedJobs: DownloadJob[]): boolean {
  return job.state === "queued" && queueIndex(job, queuedJobs) > 0;
}

export function canMoveLater(job: DownloadJob, queuedJobs: DownloadJob[]): boolean {
  const index = queueIndex(job, queuedJobs);
  return job.state === "queued" && index >= 0 && index < queuedJobs.length - 1;
}

export function matchesDownloadFilter(
  job: DownloadJob,
  filter: DownloadFilter,
  query: string,
): boolean {
  if (
    filter === "active" &&
    !["queued", "preparing", "transferring", "pauseRequested", "paused", "finalizing", "cancelRequested"].includes(job.state)
  ) return false;
  if (filter === "completed" && job.state !== "completed") return false;
  if (filter === "issues" && !["failed", "interrupted", "cancelled"].includes(job.state)) return false;

  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return `${job.displayName} ${job.destinationFileName} ${job.destinationDirectory ?? ""} ${job.state}`
    .toLowerCase()
    .includes(needle);
}

export function canPause(job: DownloadJob): boolean {
  return ["queued", "preparing", "transferring"].includes(job.state);
}

export function canResume(job: DownloadJob): boolean {
  return job.state === "paused" || job.state === "interrupted";
}

export function canCancel(job: DownloadJob): boolean {
  return ["queued", "preparing", "transferring", "pauseRequested", "paused"].includes(job.state);
}

export function canRetry(job: DownloadJob): boolean {
  if (job.state === "failed") return job.lastError?.retryable ?? false;
  return job.state === "cancelled";
}

export function canRemove(job: DownloadJob): boolean {
  return job.state === "completed" || job.state === "failed" || job.state === "cancelled";
}

export function downloadStageDetail(job: DownloadJob): string {
  switch (job.state) {
    case "queued": return "Waiting for an available download slot.";
    case "preparing": return "Preparing the destination and validating the download request.";
    case "transferring": return "Downloading content.";
    case "pauseRequested": return "Finishing the current transfer step before pausing.";
    case "paused": return "Download is paused. Resume when you are ready.";
    case "finalizing": return "Verifying and saving the completed file safely.";
    case "cancelRequested": return "Stopping the current operation safely.";
    case "completed": return "Download finished and the file was saved successfully.";
    case "failed": return job.lastError?.retryable
      ? "The download stopped with a recoverable error."
      : "The download stopped and needs attention before it can continue.";
    case "cancelled": return "Download was cancelled. You can retry it from the beginning.";
    case "interrupted": return "SearchNow recovered this unfinished download after an interruption.";
  }
}

export function downloadRecoveryHint(job: DownloadJob): string | null {
  if (job.state === "interrupted") return "Resume to continue this recovered download.";
  if (job.state === "cancelled") return "Retry to start this download again.";
  if (job.state !== "failed") return null;
  if (job.lastError?.retryable) return "Retry the download. If it fails again, check the destination folder and your connection.";
  return "Review the error details and destination folder before trying again.";
}

export class DownloadTransferEstimator {
  readonly #samples = new Map<string, TransferSample>();

  update(next: DownloadManagerSnapshot): void {
    const sampledAtMs = performance.now();
    const activeIds = new Set<string>();

    for (const job of next.jobs) {
      if (job.state !== "transferring") continue;
      activeIds.add(job.id);
      const previous = this.#samples.get(job.id);
      let bytesPerSecond: number | null = previous?.bytesPerSecond ?? null;

      if (previous && job.progress.downloadedBytes >= previous.bytes) {
        const elapsedSeconds = (sampledAtMs - previous.sampledAtMs) / 1000;
        const deltaBytes = job.progress.downloadedBytes - previous.bytes;
        if (elapsedSeconds > 0 && deltaBytes > 0) {
          const instantRate = deltaBytes / elapsedSeconds;
          bytesPerSecond =
            previous.bytesPerSecond === null
              ? instantRate
              : previous.bytesPerSecond * (1 - SPEED_EMA_ALPHA) + instantRate * SPEED_EMA_ALPHA;
        }
      }

      this.#samples.set(job.id, {
        bytes: job.progress.downloadedBytes,
        sampledAtMs,
        bytesPerSecond,
      });
    }

    for (const jobId of this.#samples.keys()) {
      if (!activeIds.has(jobId)) this.#samples.delete(jobId);
    }
  }

  rate(job: DownloadJob): number | null {
    return job.state === "transferring" ? (this.#samples.get(job.id)?.bytesPerSecond ?? null) : null;
  }

  etaSeconds(job: DownloadJob): number | null {
    const rate = this.rate(job);
    const total = job.progress.totalBytes;
    if (rate === null || rate <= 0 || total === null || total <= job.progress.downloadedBytes) return null;
    const eta = (total - job.progress.downloadedBytes) / rate;
    return Number.isFinite(eta) && eta <= MAX_REASONABLE_ETA_SECONDS ? eta : null;
  }
}
