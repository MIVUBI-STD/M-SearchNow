<script lang="ts">
  import { FolderOpen, RefreshCw, RotateCcw, Search, Trash2, X } from "@lucide/svelte";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import {
    downloadStateLabel,
    formatBytes,
    formatDateTime,
    formatDuration,
    formatTransferRate,
    progressPercent,
  } from "../app/shared/format";
  import type { DownloadJob, DownloadManagerSnapshot } from "../app/shared/types";
  import MetricCard from "../components/ui/MetricCard.svelte";
  import Notice from "../components/ui/Notice.svelte";
  import PageState from "../components/ui/PageState.svelte";
  import ResultsBar from "../components/ui/ResultsBar.svelte";
  import StatePill from "../components/ui/StatePill.svelte";
  import TechnicalDetails from "../components/ui/TechnicalDetails.svelte";

  type DownloadFilter = "all" | "active" | "completed" | "issues";
  type TransferSample = {
    bytes: number;
    sampledAtMs: number;
    bytesPerSecond: number | null;
  };

  const SPEED_EMA_ALPHA = 0.35;
  const MAX_REASONABLE_ETA_SECONDS = 7 * 24 * 60 * 60;

  let { runtimeReady, active }: { runtimeReady: boolean; active: boolean } = $props();
  let snapshot = $state<DownloadManagerSnapshot | null>(null);
  let loading = $state(false);
  let error = $state("");
  let actionJobId = $state<string | null>(null);
  let query = $state("");
  let filter = $state<DownloadFilter>("all");
  let refreshSequence = 0;
  const transferSamples = new Map<string, TransferSample>();

  let jobs = $derived((snapshot?.jobs ?? []).slice().sort((a, b) => b.updatedAtMs - a.updatedAtMs));
  let visibleJobs = $derived(jobs.filter((job) => matchesFilter(job)));
  let completedJobs = $derived(jobs.filter((job) => job.state === "completed").length);
  let controlsChanged = $derived(query.trim().length > 0 || filter !== "all");

  function matchesFilter(job: DownloadJob): boolean {
    if (filter === "active" && !["queued", "preparing", "transferring", "finalizing", "cancelRequested"].includes(job.state)) return false;
    if (filter === "completed" && job.state !== "completed") return false;
    if (filter === "issues" && !["failed", "interrupted", "cancelled"].includes(job.state)) return false;
    const needle = query.trim().toLowerCase();
    if (!needle) return true;
    return `${job.displayName} ${job.destinationFileName} ${job.destinationDirectory ?? ""} ${job.state}`.toLowerCase().includes(needle);
  }

  function resetControls(): void {
    query = "";
    filter = "all";
  }

  function canCancel(job: DownloadJob): boolean {
    return ["queued", "preparing", "transferring"].includes(job.state);
  }

  function canRetry(job: DownloadJob): boolean {
    if (job.state === "failed") return job.lastError?.retryable ?? false;
    return job.state === "cancelled" || job.state === "interrupted";
  }

  function canRemove(job: DownloadJob): boolean {
    return job.state === "completed" || job.state === "failed" || job.state === "cancelled";
  }

  function updateTransferSamples(next: DownloadManagerSnapshot): void {
    const sampledAtMs = performance.now();
    const activeIds = new Set<string>();

    for (const job of next.jobs) {
      if (job.state !== "transferring") continue;
      activeIds.add(job.id);
      const previous = transferSamples.get(job.id);
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

      transferSamples.set(job.id, {
        bytes: job.progress.downloadedBytes,
        sampledAtMs,
        bytesPerSecond,
      });
    }

    for (const jobId of transferSamples.keys()) {
      if (!activeIds.has(jobId)) transferSamples.delete(jobId);
    }
  }

  function transferRate(job: DownloadJob): number | null {
    return job.state === "transferring" ? (transferSamples.get(job.id)?.bytesPerSecond ?? null) : null;
  }

  function etaSeconds(job: DownloadJob): number | null {
    const rate = transferRate(job);
    const total = job.progress.totalBytes;
    if (rate === null || rate <= 0 || total === null || total <= job.progress.downloadedBytes) return null;
    const eta = (total - job.progress.downloadedBytes) / rate;
    return Number.isFinite(eta) && eta <= MAX_REASONABLE_ETA_SECONDS ? eta : null;
  }

  async function refresh(showBusy = true): Promise<void> {
    if (!runtimeReady || (showBusy && loading)) return;
    const sequence = ++refreshSequence;
    if (showBusy) loading = true;
    const result = await runtimeProductFacade.loadDownloads();
    if (sequence !== refreshSequence) return;
    if (result.ok) {
      updateTransferSamples(result.data);
      snapshot = result.data;
      error = "";
    } else {
      error = result.error.message;
    }
    loading = false;
  }

  async function cancel(job: DownloadJob): Promise<void> {
    actionJobId = job.id;
    const result = await runtimeProductFacade.cancelDownload(job.id);
    if (!result.ok) error = result.error.message;
    await refresh(false);
    actionJobId = null;
  }

  async function retry(job: DownloadJob): Promise<void> {
    actionJobId = job.id;
    const result = await runtimeProductFacade.retryDownload(job.id);
    if (!result.ok) error = result.error.message;
    await refresh(false);
    actionJobId = null;
  }

  async function remove(job: DownloadJob): Promise<void> {
    actionJobId = job.id;
    const result = await runtimeProductFacade.removeDownload(job.id);
    if (result.ok) {
      snapshot = result.data;
      error = "";
    } else {
      error = result.error.message;
    }
    actionJobId = null;
  }

  async function openFolder(job: DownloadJob): Promise<void> {
    if (job.state !== "completed" || !job.destinationDirectory) return;
    actionJobId = job.id;
    const result = await runtimeProductFacade.openDownloadDirectory(job.id);
    error = result.ok ? "" : result.error.message;
    actionJobId = null;
  }

  $effect(() => {
    if (runtimeReady) return;
    refreshSequence += 1;
    loading = false;
  });

  $effect(() => {
    if (!active || !runtimeReady) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let fallbackTimer: ReturnType<typeof setTimeout> | undefined;
    let eventRefreshRunning = false;
    let eventRefreshQueued = false;

    const refreshFromEvent = async (): Promise<void> => {
      eventRefreshQueued = true;
      if (eventRefreshRunning) return;
      eventRefreshRunning = true;
      while (eventRefreshQueued && !disposed) {
        eventRefreshQueued = false;
        await refresh(false);
      }
      eventRefreshRunning = false;
    };

    const startFallbackPolling = (): void => {
      const poll = async (): Promise<void> => {
        await refresh(false);
        if (!disposed && active) fallbackTimer = setTimeout(poll, 2000);
      };
      fallbackTimer = setTimeout(poll, 2000);
    };

    void refresh(false);
    void runtimeProductFacade
      .subscribeDownloadChanges(() => {
        void refreshFromEvent();
      })
      .then((result) => {
        if (disposed) {
          if (result.ok) result.data();
          return;
        }
        if (result.ok) unlisten = result.data;
        else startFallbackPolling();
      });

    return () => {
      disposed = true;
      unlisten?.();
      if (fallbackTimer) clearTimeout(fallbackTimer);
    };
  });
</script>

<section class="page" hidden={!active}>
  <div class="page-heading page-heading--actions">
    <div>
      <h1>Downloads</h1>
      <p>See current downloads and recently saved files.</p>
    </div>
    <button class="button button--secondary" type="button" onclick={() => refresh()} disabled={!runtimeReady || loading}>
      <RefreshCw size={15} class={loading ? "spin" : ""} aria-hidden="true" />
      Refresh
    </button>
  </div>

  <div class="metric-grid">
    <MetricCard label="Active" value={snapshot?.activeJobs ?? "—"} detail="Downloading now" />
    <MetricCard label="Queued" value={snapshot?.queuedJobs ?? "—"} detail="Waiting to start" />
    <MetricCard label="Downloaded" value={snapshot ? completedJobs : "—"} detail="Saved successfully" />
  </div>

  {#if snapshot && jobs.length > 0}
    <div class="toolbar">
      <label class="search-field">
        <Search size={15} aria-hidden="true" />
        <input bind:value={query} type="search" placeholder="Search downloads" aria-label="Search downloads" />
      </label>
      <select class="select-field" bind:value={filter} aria-label="Filter downloads">
        <option value="all">All downloads</option>
        <option value="active">Active</option>
        <option value="completed">Downloaded</option>
        <option value="issues">Needs attention</option>
      </select>
    </div>
    <ResultsBar
      label={`${visibleJobs.length} of ${jobs.length} download${jobs.length === 1 ? "" : "s"}`}
      detail={null}
      showReset={controlsChanged}
      resetLabel="Reset"
      onReset={resetControls}
    />
  {/if}

  {#if snapshot?.schedulerError}
    <Notice tone="warning" title="Downloads paused" message="SearchNow could not continue the download queue automatically. Refresh to try again." />
  {/if}

  {#if error}
    <Notice
      tone="error"
      title="Something went wrong"
      message={error}
      actionLabel="Refresh"
      actionDisabled={loading}
      onAction={() => void refresh()}
    />
  {/if}

  {#if !runtimeReady}
    <PageState marker="03" title="Downloads unavailable" message="SearchNow cannot manage downloads right now." />
  {:else if !snapshot && !error}
    <PageState kind="loading" title="Loading downloads" message="Reading your current downloads." />
  {:else if snapshot && jobs.length === 0}
    <PageState marker="03" title="No downloads yet" message="Downloads started from Discover will appear here." />
  {:else if snapshot && visibleJobs.length === 0}
    <PageState
      marker="03"
      title="No matching downloads"
      message="Change the search or filter to see other downloads."
      actionLabel={controlsChanged ? "Reset" : null}
      onAction={controlsChanged ? resetControls : null}
    />
  {:else if snapshot}
    <div class="download-list" aria-live="polite">
      {#each visibleJobs as job (job.id)}
        {@const percent = progressPercent(job.progress.downloadedBytes, job.progress.totalBytes)}
        {@const rate = transferRate(job)}
        {@const eta = etaSeconds(job)}
        <article class="download-card" aria-busy={actionJobId === job.id}>
          <div class="download-card__main">
            <div class="download-card__heading">
              <div>
                <StatePill state={job.state} label={downloadStateLabel(job.state)} />
                <h2>{job.displayName}</h2>
              </div>
              <span class="download-card__time">{formatDateTime(job.updatedAtMs)}</span>
            </div>

            <div
              class:progress-track--indeterminate={percent === null && ["preparing", "transferring", "finalizing"].includes(job.state)}
              class="progress-track"
              role="progressbar"
              aria-label={`${job.displayName} progress`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={percent ?? undefined}
              aria-valuetext={percent === null ? downloadStateLabel(job.state) : `${percent}%`}
            >
              {#if percent !== null}<span style={`width:${percent}%`}></span>{:else}<span></span>{/if}
            </div>

            <div class="download-card__meta">
              <span>{formatBytes(job.progress.downloadedBytes)}{job.progress.totalBytes !== null ? ` / ${formatBytes(job.progress.totalBytes)}` : ""}</span>
              <span>
                {#if job.state === "transferring"}
                  {formatTransferRate(rate)}{eta !== null ? ` · ${formatDuration(eta)} left` : ""}
                {:else if percent !== null}
                  {percent}%
                {/if}
              </span>
            </div>

            {#if job.destinationDirectory}
              <div class="download-card__destination">
                <span>{job.state === "completed" ? "Saved to" : "Save to"}</span>
                <strong>{job.destinationDirectory}</strong>
              </div>
            {/if}

            {#if job.lastError}
              <Notice tone="warning" title="Download failed" message={job.lastError.message} />
            {/if}

            <TechnicalDetails
              items={[
                { label: "File", value: job.destinationFileName },
                { label: "Job", value: job.id },
                { label: "Transport", value: job.source.transport },
              ]}
            />
          </div>

          <div class="download-card__actions">
            {#if job.state === "completed" && job.destinationDirectory}
              <button class="button button--secondary button--compact" type="button" onclick={() => openFolder(job)} disabled={actionJobId === job.id}>
                <FolderOpen size={15} aria-hidden="true" />
                Open folder
              </button>
            {/if}
            {#if canCancel(job)}
              <button class="icon-button" type="button" title="Cancel" aria-label={`Cancel ${job.displayName}`} onclick={() => cancel(job)} disabled={actionJobId === job.id}>
                <X size={16} aria-hidden="true" />
              </button>
            {/if}
            {#if canRetry(job)}
              <button class="icon-button" type="button" title="Retry" aria-label={`Retry ${job.displayName}`} onclick={() => retry(job)} disabled={actionJobId === job.id}>
                <RotateCcw size={16} aria-hidden="true" />
              </button>
            {/if}
            {#if canRemove(job)}
              <button class="icon-button" type="button" title="Remove from list" aria-label={`Remove ${job.displayName} from list`} onclick={() => remove(job)} disabled={actionJobId === job.id}>
                <Trash2 size={16} aria-hidden="true" />
              </button>
            {/if}
          </div>
        </article>
      {/each}
    </div>
  {/if}
</section>
