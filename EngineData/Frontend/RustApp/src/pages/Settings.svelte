<script lang="ts">
  import { Check, RefreshCw, Save } from "@lucide/svelte";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import { minecraftChannelLabel, minecraftStorageKindLabel } from "../app/shared/format";
  import type { AppSettings, MinecraftDiscoverySnapshot, ProductRuntimeSnapshot } from "../app/shared/types";
  import DiagnosticsPanel from "../components/settings/DiagnosticsPanel.svelte";
  import Notice from "../components/ui/Notice.svelte";
  import StatePill from "../components/ui/StatePill.svelte";

  let { snapshot, active }: { snapshot: ProductRuntimeSnapshot | null; active: boolean } = $props();
  let schemaVersion = $state(1);
  let rootOverride = $state("");
  let includePreview = $state(false);
  let includeLegacyUwp = $state(true);
  let includeDevelopmentContent = $state(false);
  let bandwidthLimitMib = $state("");
  let discovery = $state<MinecraftDiscoverySnapshot | null>(null);
  let baselineSettings = $state<AppSettings | null>(null);
  let loading = $state(false);
  let loaded = $state(false);
  let saving = $state(false);
  let scanning = $state(false);
  let error = $state("");
  let saved = $state(false);

  let dirty = $derived(
    baselineSettings !== null &&
      (rootOverride.trim() !== (baselineSettings.minecraft.rootOverride ?? "") ||
        includePreview !== baselineSettings.minecraft.includePreview ||
        includeLegacyUwp !== baselineSettings.minecraft.includeLegacyUwp ||
        includeDevelopmentContent !== baselineSettings.minecraft.includeDevelopmentContent ||
        bandwidthLimitMib !== formatBandwidthLimit(baselineSettings.download.bandwidthLimitBytesPerSecond)),
  );
  let bandwidthLimitInvalid = $derived.by(() => {
    const value = bandwidthLimitMib.trim();
    if (!value) return false;
    const mib = Number(value);
    return !Number.isFinite(mib) || mib < 0.0625 || mib > 1024;
  });
  let discoveryLabel = $derived(
    discovery?.state === "found" ? "Found" : discovery?.state === "unsupportedPlatform" ? "Unsupported" : "Not found",
  );
  let discoveryTone = $derived(discovery?.state === "found" ? "completed" : "interrupted");

  function formatBandwidthLimit(bytesPerSecond: number | null): string {
    if (bytesPerSecond === null) return "";
    return (bytesPerSecond / (1024 * 1024)).toFixed(2).replace(/\.00$/, "").replace(/(\.\d)0$/, "$1");
  }

  function parsedBandwidthLimit(): number | null {
    const value = bandwidthLimitMib.trim();
    if (!value) return null;
    const mib = Number(value);
    if (!Number.isFinite(mib) || mib <= 0) return null;
    return Math.round(mib * 1024 * 1024);
  }

  function applySettings(settings: AppSettings): void {
    schemaVersion = settings.schemaVersion;
    rootOverride = settings.minecraft.rootOverride ?? "";
    includePreview = settings.minecraft.includePreview;
    includeLegacyUwp = settings.minecraft.includeLegacyUwp;
    includeDevelopmentContent = settings.minecraft.includeDevelopmentContent;
    bandwidthLimitMib = formatBandwidthLimit(settings.download.bandwidthLimitBytesPerSecond);
    baselineSettings = settings;
  }

  function revertChanges(): void {
    if (!baselineSettings || saving || scanning) return;
    applySettings(baselineSettings);
    error = "";
    saved = false;
  }

  async function load(): Promise<void> {
    if (!active || !snapshot?.ready || loading) return;
    loading = true;
    error = "";
    const result = await runtimeProductFacade.loadSettings();
    if (result.ok) applySettings(result.data);
    else error = result.error.message;
    loaded = true;
    loading = false;
  }

  async function save(): Promise<void> {
    if (!active || !snapshot?.ready || saving || scanning || !dirty || bandwidthLimitInvalid) return;
    saving = true;
    saved = false;
    error = "";
    const result = await runtimeProductFacade.saveSettings({
      schemaVersion,
      minecraft: {
        rootOverride: rootOverride.trim() || null,
        includePreview,
        includeLegacyUwp,
        includeDevelopmentContent,
      },
      download: {
        bandwidthLimitBytesPerSecond: parsedBandwidthLimit(),
      },
    });
    if (result.ok) {
      applySettings(result.data);
      saved = true;
    } else {
      error = result.error.message;
    }
    saving = false;
  }

  async function rescan(): Promise<void> {
    if (!active || !snapshot?.ready || scanning || saving || dirty) return;
    scanning = true;
    error = "";
    const result = await runtimeProductFacade.discoverMinecraft();
    if (result.ok) discovery = result.data;
    else error = result.error.message;
    scanning = false;
  }

  $effect(() => {
    if (snapshot?.ready) return;
    error = "";
    saved = false;
    if (!dirty) {
      loaded = false;
      baselineSettings = null;
      discovery = null;
    }
  });

  $effect(() => {
    if (!discovery && snapshot?.backend?.minecraft) discovery = snapshot.backend.minecraft;
  });

  $effect(() => {
    if (!active || !snapshot?.ready) return;
    if (!loaded && !loading) void load();
  });

  $effect(() => {
    if (dirty) saved = false;
  });
</script>

<section class="page" hidden={!active}>
  <div class="page-heading page-heading--actions">
    <div>
      <span class="eyebrow">Application</span>
      <h1>Settings</h1>
      <p>Choose where SearchNow looks for Minecraft content.</p>
    </div>
    <button class="button button--primary" type="button" onclick={save} disabled={!active || !snapshot?.ready || loading || saving || scanning || !dirty || bandwidthLimitInvalid}>
      {#if saved && !saving}<Check size={15} aria-hidden="true" />{:else}<Save size={15} aria-hidden="true" />{/if}
      {saving ? "Saving" : saved ? "Saved" : "Save changes"}
    </button>
  </div>

  {#if error}
    <Notice tone="error" title="Could not update settings." message={error} />
  {:else if bandwidthLimitInvalid}
    <Notice tone="warning" title="Invalid bandwidth limit" message="Enter a value from 0.0625 to 1024 MiB/s, or leave it empty for unlimited speed." />
  {:else if dirty}
    <Notice
      tone="info"
      title="Unsaved changes"
      message="Save or revert your changes before scanning again."
      actionLabel="Revert"
      actionDisabled={saving || scanning}
      onAction={revertChanges}
    />
  {:else if saved}
    <Notice tone="success" title="Changes saved." message="Scan again to refresh detected Minecraft locations." />
  {/if}

  <div class="settings-layout">
    <div class="settings-stack">
      <article class="settings-section">
        <div class="settings-section__heading">
          <div><span class="eyebrow">Minecraft</span><h2>Minecraft locations</h2></div>
          <button
            class="button button--secondary button--compact"
            type="button"
            title={dirty ? "Save changes before scanning again" : "Scan Minecraft locations again"}
            onclick={rescan}
            disabled={!active || !snapshot?.ready || scanning || saving || dirty}
          >
            <RefreshCw size={14} class={scanning ? "spin" : ""} aria-hidden="true" />{scanning ? "Scanning" : "Scan again"}
          </button>
        </div>

        <label class="field">
          <span>Minecraft data folder (optional)</span>
          <input bind:value={rootOverride} type="text" placeholder="Leave empty for automatic detection" disabled={!active || !snapshot?.ready || loading || saving || scanning} />
          <small>Use this only if SearchNow cannot find your Minecraft data automatically.</small>
        </label>

        <div class="toggle-list">
          <label class="toggle-row">
            <div><strong>Include Minecraft Preview</strong><span>Also check Minecraft Preview content.</span></div>
            <input bind:checked={includePreview} type="checkbox" disabled={!active || !snapshot?.ready || loading || saving || scanning} />
          </label>
          <label class="toggle-row">
            <div><strong>Check older Minecraft locations</strong><span>Also look in legacy Windows storage locations.</span></div>
            <input bind:checked={includeLegacyUwp} type="checkbox" disabled={!active || !snapshot?.ready || loading || saving || scanning} />
          </label>
          <label class="toggle-row">
            <div><strong>Include development folders</strong><span>Also include development behavior, resource, and skin-pack folders.</span></div>
            <input bind:checked={includeDevelopmentContent} type="checkbox" disabled={!active || !snapshot?.ready || loading || saving || scanning} />
          </label>
        </div>
      </article>

      <article class="settings-section">
        <div class="settings-section__heading">
          <div><span class="eyebrow">Downloads</span><h2>Bandwidth</h2></div>
        </div>
        <label class="field">
          <span>Download limit (MiB/s)</span>
          <input
            bind:value={bandwidthLimitMib}
            type="number"
            min="0.0625"
            max="1024"
            step="0.25"
            placeholder="Unlimited"
            disabled={!active || !snapshot?.ready || loading || saving || scanning}
          />
          <small>Leave empty for unlimited speed. One global limit is shared fairly by concurrent downloads.</small>
        </label>
      </article>

      <article class="settings-section">
        <div class="settings-section__heading">
          <div><span class="eyebrow">Detected storage</span><h2>Detected Minecraft locations</h2></div>
          <StatePill state={discoveryTone} label={discoveryLabel} />
        </div>
        <p class="section-copy">{discovery?.message ?? "Minecraft locations have not been checked yet."}</p>

        {#if discovery?.roots.length}
          <div class="root-list">
            {#each discovery.roots as root (root.id)}
              <div class="root-row">
                <div><strong>{minecraftChannelLabel(root.channel)}</strong><span>{minecraftStorageKindLabel(root.storageKind)}</span></div>
                <code>{root.root}</code>
              </div>
            {/each}
          </div>
        {/if}
      </article>

      <DiagnosticsPanel runtimeReady={snapshot?.ready ?? false} {active} />
    </div>

    <aside class="settings-stack">
      <article class="settings-card settings-card--large">
        <span class="settings-card__label">App connection</span>
        <strong>{snapshot?.ready ? "Connected" : "Unavailable"}</strong>
        <p>{snapshot?.summary ?? "Checking app status..."}</p>
      </article>
      <article class="settings-card settings-card--large">
        <span class="settings-card__label">App health</span>
        <strong>{snapshot?.backend?.diagnostics.health.state ?? "Unknown"}</strong>
        <p>{snapshot?.backend ? `${snapshot.backend.diagnostics.health.errorEvents} errors · ${snapshot.backend.diagnostics.health.warningEvents} warnings` : "Health information is not available yet."}</p>
      </article>
    </aside>
  </div>
</section>