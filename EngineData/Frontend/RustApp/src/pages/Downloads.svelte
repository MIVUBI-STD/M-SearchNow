<script lang="ts">
  import { ArrowDown, ArrowUp, FolderOpen, Pause, Play, RefreshCw, RotateCcw, Search, Trash2, X } from "@lucide/svelte";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import {
    downloadStateLabel,
    formatBytes,
    formatDateTime,
    formatDuration,
    formatTransferRate,
    progressPercent,
  } from "../app/shared/format";
  import {
    canCancel,
    canMoveEarlier,
    canMoveLater,
    canPause,
    canRemove,
    canResume,
    canRetry,
    displayRank,
    downloadRecoveryHint,
    downloadStageDetail,
    DownloadTransferEstimator,
    matchesDownloadFilter,
    queueIndex,
    type DownloadFilter,
  } from "../app/shared/downloadView";
  import type { DownloadJob, DownloadManagerSnapshot } from "../app/shared/types";
  import Notice from "../components/ui/Notice.svelte";
  import PageState from "../components/ui/PageState.svelte";
  import ResultsBar from "../components/ui/ResultsBar.svelte";
  import StatePill from "../components/ui/StatePill.svelte";
  import TechnicalDetails from "../components/ui/TechnicalDetails.svelte";

  type DownloadFeedback = {
    tone: "error";
    title: string;
    message: string;
    actionLabel?: string;
    action?: () => void;
  };
  let { runtimeReady, active }: { runtimeReady: boolean; active: boolean } = $props();
  let snapshot = $state<DownloadManagerSnapshot | null>(null);
  let loading = $state(false);
  let feedback = $state<DownloadFeedback | null>(null);
  let actionJobId = $state<string | null>(null);
  let clearingCompleted = $state(false);
  let query = $state("");
  let filter = $state<DownloadFilter>("all");
  let refreshSequence = 0;
  const transferEstimator = new DownloadTransferEstimator();

  let queuedJobs = $derived((snapshot?.jobs ?? []).filter((job) => job.state === "queued"));
  let jobs = $derived(
    (snapshot?.jobs ?? []).slice().sort((a, b) => {
      const aRank = displayRank(a);
      const bRank = displayRank(b);
      if (aRank !== bRank) return aRank - bRank;
      if (a.state === "queued" && b.state === "queued") {
        return queueIndex(a, queuedJobs) - queueIndex(b, queuedJobs);
      }
      return b.updatedAtMs - a.updatedAtMs;
    }),
  );
  let visibleJobs = $derived(jobs.filter((job) => matchesDownloadFilter(job, filter, query)));
  let completedJobs = $derived(jobs.filter((job) => job.state === "completed").length);
  let controlsChanged = $derived(query.trim().length > 0 || filter !== "all");

  function resetControls(): void {
    query = "";
    filter = "all";
  }

  async function refresh(showBusy = true, reportFailure = true): Promise<boolean> {
    if (!runtimeReady || (showBusy && loading)) return false;
    const sequence = ++refreshSequence;
    if (showBusy) loading = true;
    if (reportFailure) feedback = null;

    const result = await runtimeProductFacade.loadDownloads();
    if (sequence !== refreshSequence) return false;

    if (result.ok) {
      transferEstimator.update(result.data);
      snapshot = result.data;
      if (reportFailure) feedback = null;
    } else if (reportFailure) {
      feedback = {
        tone: "error",
        title: "Downloads could not be refreshed",
        message: result.error.message,
        actionLabel: "Refresh",
        action: () => void refresh(),
      };
    }

    loading = false;
    return result.ok;
  }

  async function moveInQueue(job: DownloadJob, direction: "earlier" | "later"): Promise<void> {
    actionJobId = job.id;
    feedback = null;
    const result = await runtimeProductFacade.moveDownloadInQueue(job.id, direction);
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Queue order could not be changed",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void moveInQueue(job, direction),
      };
    }
    await refresh(false, false);
    actionJobId = null;
  }

  async function pause(job: DownloadJob): Promise<void> {
    actionJobId = job.id;
    feedback = null;
    const result = await runtimeProductFacade.pauseDownload(job.id);
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Download could not be paused",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void pause(job),
      };
    }
    await refresh(false, false);
    actionJobId = null;
  }

  async function resume(job: DownloadJob): Promise<void> {
    actionJobId = job.id;
    feedback = null;
    const result = await runtimeProductFacade.resumeDownload(job.id);
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Download could not be resumed",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void resume(job),
      };
    }
    await refresh(false, false);
    actionJobId = null;
  }

  async function cancel(job: DownloadJob): Promise<void> {
    actionJobId = job.id;
    feedback = null;
    const result = await runtimeProductFacade.cancelDownload(job.id);
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Download could not be cancelled",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void cancel(job),
      };
    }
    await refresh(false, false);
    actionJobId = null;
  }

  async function retry(job: DownloadJob): Promise<void> {
    actionJobId = job.id;
    feedback = null;
    const result = await runtimeProductFacade.retryDownload(job.id);
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Download could not be retried",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void retry(job),
      };
    }
    await refresh(false, false);
    actionJobId = null;
  }

  async function remove(job: DownloadJob): Promise<void> {
    actionJobId = job.id;
    feedback = null;
    const result = await runtimeProductFacade.removeDownload(job.id);
    if (result.ok) {
      snapshot = result.data;
    } else {
      feedback = {
        tone: "error",
        title: "Download could not be removed",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void remove(job),
      };
    }
    actionJobId = null;
  }

  async function clearCompleted(): Promise<void> {
    if (clearingCompleted || completedJobs === 0) return;
    clearingCompleted = true;
    feedback = null;
    const result = await runtimeProductFacade.clearCompletedDownloads();
    if (result.ok) {
      snapshot = result.data;
    } else {
      feedback = {
        tone: "error",
        title: "Completed downloads could not be cleared",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void clearCompleted(),
      };
    }
    clearingCompleted = false;
  }

  async function openFolder(job: DownloadJob): Promise<void> {
    if (job.state !== "completed" || !job.destinationDirectory) return;
    actionJobId = job.id;
    feedback = null;
    const result = await runtimeProductFacade.openDownloadDirectory(job.id);
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Download folder could not be opened",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void openFolder(job),
      };
    }
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
        await refresh(false, false);
      }
      eventRefreshRunning = false;
    };

    const startFallbackPolling = (): void => {
      const poll = async (): Promise<void> => {
        await refresh(false, false);
        if (!disposed && active) fallbackTimer = setTimeout(poll, 2000);
      };
      fallbackTimer = setTimeout(poll, 2000);
    };

    void refresh(false, false);
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
    <div class="action-row action-row--heading">
      {#if completedJobs > 0}
        <button class="button button--ghost" type="button" onclick={clearCompleted} disabled={!runtimeReady || clearingCompleted || actionJobId !== null}>
          <Trash2 size={15} aria-hidden="true" />
          {clearingCompleted ? "Clearing" : "Clear completed"}
        </button>
      {/if}
      <button class="button button--secondary" type="button" onclick={() => refresh()} disabled={!runtimeReady || loading || clearingCompleted}>
        <RefreshCw size={15} class={loading ? "spin" : ""} aria-hidden="true" />
        Refresh
      </button>
    </div>
  </div>

  {#if snapshot && jobs.length > 0}
    <div class="downloads-controls">
      <label class="search-field search-field--downloads">
        <Search size={15} aria-hidden="true" />
        <input bind:value={query} type="search" placeholder="Search downloads" aria-label="Search downloads" />
      </label>
      <div class="downloads-tabs" role="group" aria-label="Download status">
        <button class:downloads-tab--active={filter === "all"} class="downloads-tab" type="button" onclick={() => (filter = "all")}>All</button>
        <button class:downloads-tab--active={filter === "active"} class="downloads-tab" type="button" onclick={() => (filter = "active")}>Active</button>
        <button class:downloads-tab--active={filter === "completed"} class="downloads-tab" type="button" onclick={() => (filter = "completed")}>Downloaded</button>
        <button class:downloads-tab--active={filter === "issues"} class="downloads-tab" type="button" onclick={() => (filter = "issues")}>Needs attention</button>
      </div>
      <div class="downloads-summary">
        <span>{snapshot.activeJobs} active</span>
        <span>{snapshot.queuedJobs} queued</span>
        <span>{completedJobs} complete</span>
      </div>
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
    <Notice
      tone="warning"
      title="Downloads paused"
      message="SearchNow could not continue the download queue automatically."
      actionLabel="Refresh"
      actionDisabled={loading}
      onAction={() => void refresh()}
    />
  {/if}

  {#if feedback}
    <Notice
      tone={feedback.tone}
      title={feedback.title}
      message={feedback.message}
      actionLabel={feedback.actionLabel ?? null}
      actionDisabled={loading || actionJobId !== null || clearingCompleted}
      onAction={feedback.action ?? null}
    />
  {/if}

  {#if !runtimeReady}
    <PageState marker="03" title="Downloads unavailable" message="SearchNow cannot manage downloads right now." />
  {:else if !snapshot && !feedback}
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
    <div class="download-queue" aria-live="polite">
      {#each visibleJobs as job (job.id)}
        {@const percent = progressPercent(job.progress.downloadedBytes, job.progress.totalBytes)}
        {@const rate = transferEstimator.rate(job)}
        {@const eta = transferEstimator.etaSeconds(job)}
        <article class="download-row" aria-busy={actionJobId === job.id}>
          <div class="download-row__main">
            <div class="download-row__topline">
              <div class="download-row__identity">
                <h2>{job.displayName}</h2>
                <StatePill state={job.state} label={downloadStateLabel(job.state)} />
              </div>
              <span class="download-row__time">{formatDateTime(job.updatedAtMs)}</span>
            </div>

            {#if ["preparing", "finalizing", "pauseRequested", "cancelRequested", "interrupted", "failed"].includes(job.state)}
              <div class="download-row__stage">
                <span>{downloadStageDetail(job)}</span>
              </div>
            {/if}

            <div
              class:progress-track--indeterminate={percent === null && ["preparing", "transferring", "pauseRequested", "finalizing"].includes(job.state)}
              class="progress-track download-row__progress"
              role="progressbar"
              aria-label={`${job.displayName} progress`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={percent ?? undefined}
              aria-valuetext={percent === null ? downloadStateLabel(job.state) : `${percent}%`}
            >
              {#if percent !== null}<span style={`width:${percent}%`}></span>{:else}<span></span>{/if}
            </div>

            <div class="download-row__meta">
              <span>{formatBytes(job.progress.downloadedBytes)}{job.progress.totalBytes !== null ? ` / ${formatBytes(job.progress.totalBytes)}` : ""}</span>
              <span>
                {#if job.state === "transferring"}
                  {formatTransferRate(rate)}{eta !== null ? ` · ${formatDuration(eta)} left` : ""}
                {:else if percent !== null}
                  {percent}%
                {/if}
              </span>
              {#if job.destinationDirectory}
                <span class="download-row__destination" title={job.destinationDirectory}>{job.destinationDirectory}</span>
              {/if}
            </div>

            {#if job.lastError}
              <div class="download-row__error">
                <strong>{job.lastError.retryable ? "Download can be retried" : "Download needs attention"}</strong>
                <span>{job.lastError.message}</span>
                {#if downloadRecoveryHint(job)}<small>{downloadRecoveryHint(job)}</small>{/if}
              </div>
            {:else if downloadRecoveryHint(job)}
              <div class="download-row__recovery">{downloadRecoveryHint(job)}</div>
            {/if}

            <TechnicalDetails
              items={[
                { label: "File", value: job.destinationFileName },
                { label: "Job", value: job.id },
                { label: "Transport", value: job.source.transport },
                { label: "Integrity", value: job.expectedSha256 ? "SHA-256 required" : "Size/structure checks" },
              ]}
            />
          </div>

          <div class="download-row__actions">
            {#if job.state === "completed" && job.destinationDirectory}
              <button class="icon-button" type="button" title="Open folder" aria-label={`Open folder for ${job.displayName}`} onclick={() => openFolder(job)} disabled={actionJobId === job.id}>
                <FolderOpen size={16} aria-hidden="true" />
              </button>
            {/if}
            {#if job.state === "queued"}
              <button class="icon-button" type="button" title="Move earlier" aria-label={`Move ${job.displayName} earlier in queue`} onclick={() => moveInQueue(job, "earlier")} disabled={actionJobId === job.id || !canMoveEarlier(job, queuedJobs)}>
                <ArrowUp size={16} aria-hidden="true" />
              </button>
              <button class="icon-button" type="button" title="Move later" aria-label={`Move ${job.displayName} later in queue`} onclick={() => moveInQueue(job, "later")} disabled={actionJobId === job.id || !canMoveLater(job, queuedJobs)}>
                <ArrowDown size={16} aria-hidden="true" />
              </button>
            {/if}
            {#if canPause(job)}
              <button class="icon-button" type="button" title="Pause" aria-label={`Pause ${job.displayName}`} onclick={() => pause(job)} disabled={actionJobId === job.id}>
                <Pause size={16} aria-hidden="true" />
              </button>
            {/if}
            {#if canResume(job)}
              <button class="icon-button" type="button" title="Resume" aria-label={`Resume ${job.displayName}`} onclick={() => resume(job)} disabled={actionJobId === job.id}>
                <Play size={16} aria-hidden="true" />
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
    </div>  {/if}
</section>
