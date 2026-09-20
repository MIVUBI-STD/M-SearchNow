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
