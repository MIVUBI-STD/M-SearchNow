<script lang="ts">
  import { Activity, FileDown, RefreshCw } from "@lucide/svelte";
  import { runtimeProductFacade } from "../../app/bridge/runtimeProductFacade";
  import { formatDateTime } from "../../app/shared/format";
  import type { BackendDiagnosticsSnapshot, DiagnosticEvent } from "../../app/shared/types";
  import Notice from "../ui/Notice.svelte";

  let { runtimeReady, active }: { runtimeReady: boolean; active: boolean } = $props();
  let diagnostics = $state<BackendDiagnosticsSnapshot | null>(null);
  let loading = $state(false);
  let loaded = $state(false);
  let error = $state("");
  let exportBusy = $state(false);
  let exportMessage = $state("");

  const ACTIVITY_CODES = new Set([
    "package_import_ok",
    "package_import_failed",
    "package_replace_ok",
    "package_replace_failed",
    "package_bundle_replace_ok",
    "package_bundle_replace_failed",
    "library_export_ok",
    "library_export_failed",
    "library_remove_ok",
    "library_remove_failed",
    "download_queue_ok",
    "download_queue_failed",
    "download_pause_ok",
    "download_pause_failed",
    "download_resume_ok",
    "download_resume_failed",
    "download_cancel_ok",
    "download_cancel_failed",
    "download_retry_ok",
    "download_retry_failed",
  ]);

  let recentEvents = $derived((diagnostics?.events ?? []).slice(-8).reverse());
  let recentActivity = $derived(
    (diagnostics?.events ?? [])
      .filter((event) => ACTIVITY_CODES.has(event.code))
      .slice(-10)
      .reverse(),
  );

  function activityLabel(event: DiagnosticEvent): string {
    switch (event.code) {
      case "package_import_ok": return "Content installed";
      case "package_import_failed": return "Install failed";
      case "package_replace_ok": return "Pack updated";
      case "package_replace_failed": return "Pack update failed";
      case "package_bundle_replace_ok": return "Add-on bundle updated";
      case "package_bundle_replace_failed": return "Add-on bundle update failed";
      case "library_export_ok": return "Backup exported";
      case "library_export_failed": return "Backup export failed";
      case "library_remove_ok": return "Content removed";
      case "library_remove_failed": return "Content removal failed";
      case "download_queue_ok": return "Download queued";
      case "download_queue_failed": return "Download could not start";
      case "download_pause_ok": return "Download pause requested";
      case "download_pause_failed": return "Download could not pause";
      case "download_resume_ok": return "Download resumed";
      case "download_resume_failed": return "Download could not resume";
      case "download_cancel_ok": return "Download cancellation requested";
      case "download_cancel_failed": return "Download could not cancel";
      case "download_retry_ok": return "Download retry queued";
      case "download_retry_failed": return "Download retry failed";
      default: return event.message;
    }
  }

  async function refresh(): Promise<void> {
    if (!runtimeReady || !active || loading) return;
    loading = true;
    error = "";
    const result = await runtimeProductFacade.loadDiagnostics();
    if (result.ok) diagnostics = result.data;
    else error = result.error.message;
    loaded = true;
    loading = false;
  }

  async function exportReport(): Promise<void> {
    if (!runtimeReady || !active || exportBusy) return;
    exportBusy = true;
    exportMessage = "";
    error = "";
    const result = await runtimeProductFacade.exportDiagnosticsReport();
    if (result.ok) {
      if (result.data) exportMessage = `${result.data} exported successfully.`;
    } else {
      error = result.error.message;
    }
    exportBusy = false;
  }

  $effect(() => {
    if (runtimeReady) return;
    loaded = false;
    error = "";
    exportMessage = "";
    exportBusy = false;
  });

  $effect(() => {
    if (!active || !runtimeReady) return;
    if (!loaded && !loading) void refresh();
  });
</script>

<div class="diagnostics-panel">
  <div class="diagnostics-panel__heading">
    <div><strong>Runtime diagnostics</strong><span>Recent runtime events and health counters.</span></div>
    <div class="diagnostics-panel__actions">
      <button class="button button--secondary button--compact" type="button" onclick={exportReport} disabled={!active || !runtimeReady || exportBusy}>
        <FileDown size={14} aria-hidden="true" />{exportBusy ? "Exporting" : "Export report"}
      </button>
      <button class="button button--secondary button--compact" type="button" onclick={refresh} disabled={!active || !runtimeReady || loading}>
        <RefreshCw size={14} class={loading ? "spin" : ""} aria-hidden="true" />Refresh
      </button>
    </div>
  </div>

  <p class="diagnostics-panel__copy">Credentials, provider payloads, and sensitive paths are excluded.</p>

  {#if error}
    <Notice tone="error" title="Diagnostics unavailable." message={error} />
  {:else if exportMessage}
    <Notice tone="success" title="Diagnostics exported" message={exportMessage} />
  {:else if !runtimeReady}
    <div class="diagnostic-empty"><Activity size={15} /><span>Diagnostics require the desktop runtime.</span></div>
  {:else if !diagnostics}
    <div class="diagnostic-empty" aria-live="polite"><RefreshCw size={15} class="spin" /><span>Reading runtime diagnostics.</span></div>
  {:else}
    <div class="activity-panel">
      <div class="activity-panel__heading">
        <div>
          <strong>Recent activity</strong>
          <span>Product operations from this SearchNow session. This list resets when the app restarts.</span>
        </div>
      </div>
      {#if recentActivity.length === 0}
        <div class="diagnostic-empty"><Activity size={15} /><span>No install, update, export, remove, or download actions yet.</span></div>
      {:else}
        <div class="activity-list">
          {#each recentActivity as event (`${event.timestampMs}:${event.code}`)}
            <div class:activity-row--failed={event.severity !== "info"} class="activity-row">
              <div>
                <strong>{activityLabel(event)}</strong>
                <span>{event.message}</span>
              </div>
              <time datetime={new Date(event.timestampMs).toISOString()}>{formatDateTime(event.timestampMs)}</time>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="diagnostic-summary">
      <div><span>Health</span><strong>{diagnostics.health.state}</strong></div>
      <div><span>Warnings</span><strong>{diagnostics.health.warningEvents}</strong></div>
      <div><span>Errors</span><strong>{diagnostics.health.errorEvents}</strong></div>
      <div><span>Retained</span><strong>{diagnostics.health.retainedEvents}</strong></div>
    </div>

    <details class="diagnostics-details">
      <summary>Recent events</summary>
      {#if recentEvents.length === 0}
        <div class="diagnostic-empty"><Activity size={15} /><span>No diagnostic events are retained.</span></div>
      {:else}
        <div class="diagnostic-list">
          {#each recentEvents as event (`${event.timestampMs}:${event.code}`)}
            <div class={`diagnostic-row diagnostic-row--${event.severity}`}>
              <div>
                <strong>{event.message}</strong>
                <span>{event.component} · {event.code}</span>
              </div>
              <time datetime={new Date(event.timestampMs).toISOString()}>{formatDateTime(event.timestampMs)}</time>
            </div>
          {/each}
        </div>
      {/if}
    </details>
  {/if}
</div>
