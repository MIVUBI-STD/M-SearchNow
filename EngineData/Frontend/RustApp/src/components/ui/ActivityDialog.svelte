<script lang="ts">
  import { Activity, RefreshCw, X } from "@lucide/svelte";
  import { runtimeProductFacade } from "../../app/bridge/runtimeProductFacade";
  import { activityLabel, recentActivity } from "../../app/shared/activityView";
  import { focusFirstInDialog, trapDialogFocus } from "../../app/shared/dialogFocus";
  import { formatDateTime } from "../../app/shared/format";
  import type { BackendDiagnosticsSnapshot } from "../../app/shared/types";
  import Notice from "./Notice.svelte";

  let {
    open,
    runtimeReady,
    onClose,
  }: {
    open: boolean;
    runtimeReady: boolean;
    onClose: () => void;
  } = $props();

  let diagnostics = $state<BackendDiagnosticsSnapshot | null>(null);
  let loading = $state(false);
  let error = $state("");
  let dialogElement: HTMLElement | null = $state(null);
  let activity = $derived(recentActivity(diagnostics?.events ?? []));

  async function refresh(): Promise<void> {
    if (!open || !runtimeReady || loading) return;
    loading = true;
    error = "";
    const result = await runtimeProductFacade.loadDiagnostics();
    if (result.ok) diagnostics = result.data;
    else error = result.error.message;
    loading = false;
  }

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === event.currentTarget && !loading) onClose();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (!open) return;
    if (event.key === "Escape" && !loading) {
      event.preventDefault();
      onClose();
      return;
    }
    trapDialogFocus(event, dialogElement);
  }

  $effect(() => {
    if (!open) return;
    const focusToRestore = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    queueMicrotask(() => focusFirstInDialog(dialogElement));
    void refresh();
    return () => queueMicrotask(() => focusToRestore?.focus());
  });
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div class="catalog-modal__backdrop" role="presentation" onclick={handleBackdrop}>
    <div bind:this={dialogElement} class="catalog-modal activity-dialog" role="dialog" aria-modal="true" aria-labelledby="activity-dialog-title" aria-busy={loading} tabindex="-1">
      <button class="catalog-modal__close" type="button" aria-label="Close activity" onclick={onClose} disabled={loading}>
        <X size={18} aria-hidden="true" />
      </button>

      <div class="activity-dialog__header">
        <div>
          <span>Session history</span>
          <h2 id="activity-dialog-title">Recent activity</h2>
          <p>Installs, updates, backups, removals, and download actions from this SearchNow session.</p>
        </div>
        <button class="button button--secondary button--compact" type="button" onclick={refresh} disabled={!runtimeReady || loading}>
          <RefreshCw size={14} class={loading ? "spin" : ""} aria-hidden="true" />
          Refresh
        </button>
      </div>

      {#if error}
        <Notice tone="error" title="Activity unavailable" message={error} />
      {:else if !runtimeReady}
        <div class="diagnostic-empty"><Activity size={15} aria-hidden="true" /><span>Activity requires the desktop runtime.</span></div>
      {:else if loading && !diagnostics}
        <div class="diagnostic-empty"><RefreshCw size={15} class="spin" aria-hidden="true" /><span>Reading recent activity.</span></div>
      {:else if activity.length === 0}
        <div class="diagnostic-empty"><Activity size={15} aria-hidden="true" /><span>No install, update, backup, remove, or download actions yet.</span></div>
      {:else}
        <div class="activity-list">
          {#each activity as event (`${event.timestampMs}:${event.code}`)}
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

      <p class="activity-dialog__footnote">Activity is session-only and resets when SearchNow restarts.</p>
    </div>
  </div>
{/if}
