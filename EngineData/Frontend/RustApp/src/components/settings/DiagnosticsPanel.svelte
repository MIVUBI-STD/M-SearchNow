<script lang="ts">
  import { Activity, RefreshCw } from "@lucide/svelte";
  import { runtimeProductFacade } from "../../app/bridge/runtimeProductFacade";
  import { formatDateTime } from "../../app/shared/format";
  import type { BackendDiagnosticsSnapshot } from "../../app/shared/types";
  import Notice from "../ui/Notice.svelte";

  let { runtimeReady, active }: { runtimeReady: boolean; active: boolean } = $props();
  let diagnostics = $state<BackendDiagnosticsSnapshot | null>(null);
  let loading = $state(false);
  let loaded = $state(false);
  let error = $state("");

  let recentEvents = $derived((diagnostics?.events ?? []).slice(-8).reverse());

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

  $effect(() => {
    if (runtimeReady) return;
    loaded = false;
    error = "";
  });

  $effect(() => {
    if (!active || !runtimeReady) return;
    if (!loaded && !loading) void refresh();
  });
</script>

<div class="diagnostics-panel">
  <div class="diagnostics-panel__heading">
    <div><strong>Runtime diagnostics</strong><span>Recent runtime events and health counters.</span></div>
    <button class="button button--secondary button--compact" type="button" onclick={refresh} disabled={!active || !runtimeReady || loading}>
      <RefreshCw size={14} class={loading ? "spin" : ""} />Refresh
    </button>
  </div>

  <p class="diagnostics-panel__copy">Credentials, provider payloads, and sensitive paths are excluded.</p>

  {#if error}
    <Notice tone="error" title="Diagnostics unavailable." message={error} />
  {:else if !runtimeReady}
    <div class="diagnostic-empty"><Activity size={15} /><span>Diagnostics require the desktop runtime.</span></div>
  {:else if !diagnostics}
    <div class="diagnostic-empty" aria-live="polite"><RefreshCw size={15} class="spin" /><span>Reading runtime diagnostics.</span></div>
  {:else}
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
