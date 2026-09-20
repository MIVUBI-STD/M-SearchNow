<script lang="ts">
  import { Check, FolderOpen, RefreshCw, Save, X } from "@lucide/svelte";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import { minecraftChannelLabel, minecraftStorageKindLabel } from "../app/shared/format";
  import type {
    AppSettings,
    ExportDuplicatePolicy,
    MinecraftDiscoverySnapshot,
    ProductRuntimeSnapshot,
  } from "../app/shared/types";
  import DiagnosticsPanel from "../components/settings/DiagnosticsPanel.svelte";
  import Notice from "../components/ui/Notice.svelte";
  import StatePill from "../components/ui/StatePill.svelte";

  type SettingsFeedback = {
    tone: "success" | "error";
    title: string;
    message: string;
    actionLabel?: string;
    action?: () => void;
  };

  let { snapshot, active }: { snapshot: ProductRuntimeSnapshot | null; active: boolean } = $props();
  let schemaVersion = $state(1);
  let rootOverride = $state("");
  let includePreview = $state(false);
  let includeLegacyUwp = $state(true);
  let includeDevelopmentContent = $state(false);
  let bandwidthLimitMib = $state("");
  let defaultDownloadDirectory = $state("");
  let defaultExportDirectory = $state("");
  let exportDuplicatePolicy = $state<ExportDuplicatePolicy>("keepBoth");
  let minecraftDirectoryBusy = $state(false);
  let exportDirectoryBusy = $state(false);
  let discovery = $state<MinecraftDiscoverySnapshot | null>(null);
  let baselineSettings = $state<AppSettings | null>(null);
  let loading = $state(false);
  let loaded = $state(false);
  let saving = $state(false);
  let scanning = $state(false);
  let feedback = $state<SettingsFeedback | null>(null);
  let saved = $state(false);

  let dirty = $derived(
    baselineSettings !== null &&
      (rootOverride.trim() !== (baselineSettings.minecraft.rootOverride ?? "") ||
        includePreview !== baselineSettings.minecraft.includePreview ||
        includeLegacyUwp !== baselineSettings.minecraft.includeLegacyUwp ||
        includeDevelopmentContent !== baselineSettings.minecraft.includeDevelopmentContent ||
        bandwidthLimitMib !== formatBandwidthLimit(baselineSettings.download.bandwidthLimitBytesPerSecond) ||
        defaultDownloadDirectory !== (baselineSettings.download.defaultDirectory ?? "") ||
        defaultExportDirectory !== (baselineSettings.export.defaultDirectory ?? "") ||
        exportDuplicatePolicy !== baselineSettings.export.duplicatePolicy),
  );
  let minecraftDirty = $derived(
    baselineSettings !== null &&
      (rootOverride.trim() !== (baselineSettings.minecraft.rootOverride ?? "") ||
        includePreview !== baselineSettings.minecraft.includePreview ||
        includeLegacyUwp !== baselineSettings.minecraft.includeLegacyUwp ||
        includeDevelopmentContent !== baselineSettings.minecraft.includeDevelopmentContent),
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
    defaultDownloadDirectory = settings.download.defaultDirectory ?? "";
    defaultExportDirectory = settings.export.defaultDirectory ?? "";
    exportDuplicatePolicy = settings.export.duplicatePolicy;
    baselineSettings = settings;
  }

  function revertChanges(): void {
    if (!baselineSettings || saving || scanning) return;
    applySettings(baselineSettings);
    feedback = null;
    saved = false;
  }

  async function load(): Promise<void> {
    if (!active || !snapshot?.ready || loading) return;
    loading = true;
    feedback = null;
    const result = await runtimeProductFacade.loadSettings();
    if (result.ok) {
      applySettings(result.data);
    } else {
      feedback = {
        tone: "error",
        title: "Settings could not be loaded",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void load(),
      };
    }
    loaded = true;
    loading = false;
  }

  async function save(): Promise<void> {
    if (!active || !snapshot?.ready || saving || scanning || !dirty || bandwidthLimitInvalid) return;
    const requiresMinecraftRescan = minecraftDirty;
    saving = true;
    saved = false;
    feedback = null;
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
        defaultDirectory: defaultDownloadDirectory || null,
      },
      export: {
        defaultDirectory: defaultExportDirectory || null,
        duplicatePolicy: exportDuplicatePolicy,
      },
    });
    if (result.ok) {
      applySettings(result.data);
      saved = true;
      feedback = {
        tone: "success",
        title: "Settings saved",
        message: requiresMinecraftRescan
          ? "Minecraft settings changed. Scan again to refresh detected locations."
          : "Your download and export preferences are now active.",
      };
    } else {
      feedback = {
        tone: "error",
        title: "Settings could not be saved",
        message: result.error.message,
        actionLabel: "Try saving again",
        action: () => void save(),
      };
    }
    saving = false;
  }

  async function chooseMinecraftDirectory(): Promise<void> {
    if (!active || !snapshot?.ready || saving || scanning || minecraftDirectoryBusy) return;
    minecraftDirectoryBusy = true;
    const result = await runtimeProductFacade.chooseMinecraftDirectory();
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Minecraft folder could not be chosen",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void chooseMinecraftDirectory(),
      };
      minecraftDirectoryBusy = false;
      return;
    }
    if (result.data) {
      rootOverride = result.data;
      feedback = null;
      saved = false;
    }
    minecraftDirectoryBusy = false;
  }

  async function chooseExportDirectory(): Promise<void> {
    if (!active || !snapshot?.ready || saving || scanning || exportDirectoryBusy) return;
    exportDirectoryBusy = true;
    const result = await runtimeProductFacade.chooseExportDirectory();
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Export folder could not be chosen",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void chooseExportDirectory(),
      };
      exportDirectoryBusy = false;
      return;
    }
    if (result.data) {
      defaultExportDirectory = result.data;
      feedback = null;
      saved = false;
    }
    exportDirectoryBusy = false;
  }

  async function chooseDefaultDownloadDirectory(): Promise<void> {
    if (!active || !snapshot?.ready || saving || scanning) return;
    const result = await runtimeProductFacade.chooseDownloadDirectory();
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Download folder could not be chosen",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void chooseDefaultDownloadDirectory(),
      };
      return;
    }
    if (result.data) {
      defaultDownloadDirectory = result.data;
      feedback = null;
    }
  }

  async function rescan(): Promise<void> {
    if (!active || !snapshot?.ready || scanning || saving || dirty) return;
    scanning = true;
    feedback = null;
    const result = await runtimeProductFacade.discoverMinecraft();
    if (result.ok) {
      discovery = result.data;
    } else {
      feedback = {
        tone: "error",
        title: "Minecraft scan failed",
        message: result.error.message,
        actionLabel: "Scan again",
        action: () => void rescan(),
      };
    }
    scanning = false;
  }

  $effect(() => {
    if (snapshot?.ready) return;
    feedback = null;
    saved = false;
    minecraftDirectoryBusy = false;
    exportDirectoryBusy = false;
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
    if (!dirty) return;
    saved = false;
    if (feedback?.tone === "success") feedback = null;
  });
</script>

<section class="page" hidden={!active}>
  <div class="page-heading page-heading--actions">
    <div>
      <span class="eyebrow">Application</span>
      <h1>Settings</h1>
      <p>Configure Minecraft storage, downloads, backups, and advanced behavior.</p>
    </div>
    {#if loaded && !dirty && !saving}
      <div class="settings-save-state" role="status">
        <Check size={15} aria-hidden="true" />
        <span>Saved</span>
      </div>
    {:else}
      <button
        class="button button--primary"
        type="button"
        onclick={save}
        aria-busy={saving}
        disabled={!active || !snapshot?.ready || loading || saving || scanning || exportDirectoryBusy || !dirty || bandwidthLimitInvalid}
      >
        <Save size={15} aria-hidden="true" />
        {saving ? "Saving" : "Save changes"}
      </button>
    {/if}
  </div>

  {#if feedback?.tone === "error"}
    <Notice
      tone="error"
      title={feedback.title}
      message={feedback.message}
      actionLabel={feedback.actionLabel ?? null}
      actionDisabled={loading || saving || scanning || minecraftDirectoryBusy || exportDirectoryBusy}
      onAction={feedback.action ?? null}
    />
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
  {:else if feedback?.tone === "success"}
    <Notice tone="success" title={feedback.title} message={feedback.message} />
  {/if}

  <div class="settings-workspace">
    <nav class="settings-nav" aria-label="Settings sections">
      <a href="#settings-minecraft">Minecraft</a>
      <a href="#settings-downloads">Downloads</a>
      <a href="#settings-storage">Minecraft storage</a>
      <a href="#settings-privacy">Privacy</a>
      <a href="#settings-advanced">Advanced</a>
    </nav>

    <div class="settings-main">
      <section class="settings-section" id="settings-minecraft">
        <div class="settings-section__heading">
          <div><h2>Minecraft</h2><p>Control which Minecraft locations SearchNow scans.</p></div>
          <button
            class="button button--secondary button--compact"
            type="button"
            title={dirty ? "Save changes before scanning again" : "Scan Minecraft locations again"}
            onclick={rescan}
            aria-busy={scanning}
            disabled={!active || !snapshot?.ready || scanning || saving || dirty}
          >
            <RefreshCw size={14} class={scanning ? "spin" : ""} aria-hidden="true" />{scanning ? "Scanning" : "Scan again"}
          </button>
        </div>

        <label class="field">
          <span>Minecraft data folder (optional)</span>
          <input bind:value={rootOverride} type="text" placeholder="Leave empty for automatic detection" disabled={!active || !snapshot?.ready || loading || saving || scanning || minecraftDirectoryBusy} />
          <small>Use this only if SearchNow cannot find your Minecraft data automatically.</small>
        </label>
        <div class="action-row">
          <button class="button button--secondary" type="button" onclick={chooseMinecraftDirectory} aria-busy={minecraftDirectoryBusy} disabled={!active || !snapshot?.ready || loading || saving || scanning || minecraftDirectoryBusy}>
            <FolderOpen size={15} aria-hidden="true" />
            {minecraftDirectoryBusy ? "Choosing…" : "Choose folder"}
          </button>
          {#if rootOverride}
            <button class="button button--ghost" type="button" onclick={() => (rootOverride = "")} disabled={!active || !snapshot?.ready || loading || saving || scanning || minecraftDirectoryBusy}>
              <X size={15} aria-hidden="true" />
              Use automatic detection
            </button>
          {/if}
        </div>

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
      </section>

      <section class="settings-section" id="settings-downloads">
        <div class="settings-section__heading">
          <div><h2>Downloads</h2><p>Choose where downloads go and limit transfer speed when needed.</p></div>
        </div>

        <label class="field">
          <span>Default download folder</span>
          <input
            value={defaultDownloadDirectory}
            type="text"
            placeholder="Ask every time"
            readonly
            disabled={!active || !snapshot?.ready || loading || saving || scanning}
          />
          <small>When set, Discover sends downloads here directly instead of opening the folder picker every time.</small>
        </label>
        <div class="action-row">
          <button class="button button--secondary" type="button" onclick={chooseDefaultDownloadDirectory} disabled={!active || !snapshot?.ready || loading || saving || scanning}>
            <FolderOpen size={15} aria-hidden="true" />
            Choose folder
          </button>
          {#if defaultDownloadDirectory}
            <button class="button button--ghost" type="button" onclick={() => (defaultDownloadDirectory = "")} disabled={!active || !snapshot?.ready || loading || saving || scanning}>
              <X size={15} aria-hidden="true" />
              Ask every time
            </button>
          {/if}
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
            aria-invalid={bandwidthLimitInvalid}
            aria-describedby="bandwidth-limit-help"
            disabled={!active || !snapshot?.ready || loading || saving || scanning}
          />
          <small id="bandwidth-limit-help">{bandwidthLimitInvalid ? "Enter a value from 0.0625 to 1024 MiB/s, or leave it empty." : "Leave empty for unlimited speed. One global limit is shared fairly by concurrent downloads."}</small>
        </label>

        <div class="settings-subsection">
          <div>
            <strong>Exports</strong>
            <span>Optionally send Library backups to one folder without opening a picker each time.</span>
          </div>

          <label class="field">
            <span>Default export folder</span>
            <input
              value={defaultExportDirectory}
              type="text"
              placeholder="Ask every time"
              readonly
              disabled={!active || !snapshot?.ready || loading || saving || scanning || exportDirectoryBusy}
            />
            <small>When empty, SearchNow keeps the existing save dialog behavior.</small>
          </label>
          <div class="action-row">
            <button class="button button--secondary" type="button" onclick={chooseExportDirectory} aria-busy={exportDirectoryBusy} disabled={!active || !snapshot?.ready || loading || saving || scanning || exportDirectoryBusy}>
              <FolderOpen size={15} aria-hidden="true" />
              {exportDirectoryBusy ? "Choosing…" : "Choose export folder"}
            </button>
            {#if defaultExportDirectory}
              <button class="button button--ghost" type="button" onclick={() => (defaultExportDirectory = "")} disabled={!active || !snapshot?.ready || loading || saving || scanning || exportDirectoryBusy}>
                <X size={15} aria-hidden="true" />
                Ask every time
              </button>
            {/if}
          </div>

          <label class="field">
            <span>When an export name already exists</span>
            <select class="select-field settings-select" bind:value={exportDuplicatePolicy} disabled={!active || !snapshot?.ready || loading || saving || scanning}>
              <option value="keepBoth">Keep both with a new file name</option>
              <option value="stopOnConflict">Stop and ask me to resolve it</option>
            </select>
            <small>SearchNow never overwrites an existing backup silently.</small>
          </label>
        </div>
      </section>

      <section class="settings-section" id="settings-storage">
        <div class="settings-section__heading">
          <div><h2>Minecraft storage</h2><p>Locations SearchNow found on this device.</p></div>
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
      </section>

      <section class="settings-section" id="settings-privacy">
        <div class="settings-section__heading">
          <div><h2>Privacy</h2><p>Understand what stays on this device and when SearchNow can contact external services.</p></div>
        </div>

        <div class="settings-privacy-list">
          <div>
            <strong>Local-first by default</strong>
            <span>Your library, settings, download state, and diagnostics are stored locally on this device.</span>
          </div>
          <div>
            <strong>External requests are feature-bound</strong>
            <span>SearchNow only contacts a configured content provider when you explicitly use provider-backed Discover or download features.</span>
          </div>
          <div>
            <strong>Diagnostics stay local</strong>
            <span>Runtime diagnostics are not uploaded automatically. You decide if you want to share them for support.</span>
          </div>
        </div>
      </section>

      <section class="settings-section settings-section--advanced" id="settings-advanced">
        <div class="settings-section__heading">
          <div><h2>Advanced</h2><p>Runtime health and diagnostics for troubleshooting.</p></div>
        </div>

        <div class="settings-health">
          <div>
            <span>Connection</span>
            <strong>{snapshot?.ready ? "Connected" : "Unavailable"}</strong>
            <small>{snapshot?.summary ?? "Checking app status..."}</small>
          </div>
          <div>
            <span>Health</span>
            <strong>{snapshot?.backend?.diagnostics.health.state ?? "Unknown"}</strong>
            <small>{snapshot?.backend ? `${snapshot.backend.diagnostics.health.errorEvents} errors · ${snapshot.backend.diagnostics.health.warningEvents} warnings` : "Health information is not available yet."}</small>
          </div>
        </div>

        <DiagnosticsPanel runtimeReady={snapshot?.ready ?? false} {active} />
      </section>
    </div>
  </div>
</section>