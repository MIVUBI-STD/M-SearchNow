<script lang="ts">
  import { Archive, CheckSquare, FileSearch, FolderOpen, RefreshCw, Search, X } from "@lucide/svelte";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import { localContentTypeLabel } from "../app/shared/format";
  import {
    compareLibraryItems,
    findDuplicateIds,
    findDuplicatesForItem,
    findMissingDependencyIds,
    findOutdatedDependencyIds,
    matchesLibraryFilter,
    type LibraryFilter,
    type LibrarySort,
  } from "../app/shared/libraryView";
  import type { LocalBackendSnapshot, LocalContentItem, PackageInspection } from "../app/shared/types";
  import ContentTypeMark from "../components/ui/ContentTypeMark.svelte";
  import LibraryDetailPanel from "../components/library/LibraryDetailPanel.svelte";
  import Notice from "../components/ui/Notice.svelte";
  import PackageInspectionModal from "../components/ui/PackageInspectionModal.svelte";
  import PageState from "../components/ui/PageState.svelte";
  import ResultsBar from "../components/ui/ResultsBar.svelte";

  type LibraryFeedback = {
    tone: "success" | "warning" | "error";
    title: string;
    message: string;
    actionLabel?: string;
    action?: () => void;
  };

  let { runtimeReady, active }: { runtimeReady: boolean; active: boolean } = $props();
  let loading = $state(false);
  let loaded = $state(false);
  let snapshot = $state<LocalBackendSnapshot | null>(null);
  let selectedItem = $state<LocalContentItem | null>(null);
  let actionBusy = $state(false);
  let inspectionBusy = $state(false);
  let packageInspection = $state<PackageInspection | null>(null);
  let importBusy = $state(false);
  let selectionMode = $state(false);
  let selectedIds = $state<string[]>([]);
  let batchBusy = $state(false);
  let locationBusy = $state(false);
  let dropActive = $state(false);
  let feedback = $state<LibraryFeedback | null>(null);
  let query = $state("");
  let filter = $state<LibraryFilter>("all");
  let rootFilter = $state("all");
  let sort = $state<LibrarySort>("nameAsc");

  let duplicateIds = $derived(findDuplicateIds(snapshot?.library.items ?? []));
  let missingDependencyIds = $derived(findMissingDependencyIds(snapshot?.library.items ?? []));
  let outdatedDependencyIds = $derived(findOutdatedDependencyIds(snapshot?.library.items ?? []));
  let filteredItems = $derived(
    (snapshot?.library.items ?? [])
      .filter((item) =>
        matchesLibraryFilter(item, {
          rootFilter,
          filter,
          duplicateIds,
          missingDependencyIds,
          outdatedDependencyIds,
          query,
        }),
      )
      .slice()
      .sort((left, right) => compareLibraryItems(left, right, sort)),
  );
  let detailItem = $derived(
    selectedItem && filteredItems.some((item) => item.id === selectedItem?.id)
      ? selectedItem
      : (filteredItems[0] ?? null),
  );
  let selectedDuplicates = $derived(findDuplicatesForItem(detailItem, snapshot?.library.items ?? []));
  let controlsChanged = $derived(
    query.trim().length > 0 ||
    filter !== "all" ||
    rootFilter !== "all" ||
    sort !== "nameAsc",
  );

  function rootLabel(rootId: string): string {
    const root = snapshot?.minecraft.roots.find((candidate) => candidate.id === rootId);
    if (!root) return "Unknown storage";
    return root.accountHint ?? root.storageKind;
  }

  function resetControls(): void {
    query = "";
    filter = "all";
    rootFilter = "all";
    sort = "nameAsc";
  }

  function openDetails(item: LocalContentItem): void {
    if (selectionMode) {
      toggleSelection(item.id);
      return;
    }
    selectedItem = item;
  }

  function toggleSelection(itemId: string): void {
    selectedIds = selectedIds.includes(itemId)
      ? selectedIds.filter((id) => id !== itemId)
      : [...selectedIds, itemId];
  }

  function exitSelectionMode(): void {
    if (batchBusy) return;
    selectionMode = false;
    selectedIds = [];
  }

  async function recoverExportDirectory(): Promise<"recovered" | "cancelled" | "unavailable"> {
    feedback = null;
    const settings = await runtimeProductFacade.loadSettings();
    if (!settings.ok) {
      feedback = {
        tone: "error",
        title: "Export settings could not be loaded",
        message: settings.error.message,
      };
      return "unavailable";
    }
    if (!settings.data.export.defaultDirectory) {
      return "unavailable";
    }

    const picker = await runtimeProductFacade.chooseExportDirectory();
    if (!picker.ok) {
      feedback = {
        tone: "error",
        title: "Export folder could not be chosen",
        message: picker.error.message,
        actionLabel: "Try again",
        action: () => void recoverExportDirectory(),
      };
      return "unavailable";
    }
    if (!picker.data) {
      feedback = {
        tone: "warning",
        title: "Export not completed",
        message: "The configured export folder is unavailable. Choose a replacement folder before exporting again.",
      };
      return "cancelled";
    }

    const saved = await runtimeProductFacade.saveSettings({
      ...settings.data,
      export: {
        ...settings.data.export,
        defaultDirectory: picker.data,
      },
    });
    if (!saved.ok) {
      feedback = {
        tone: "error",
        title: "Export folder could not be saved",
        message: saved.error.message,
      };
      return "unavailable";
    }

    feedback = {
      tone: "success",
      title: "Export folder updated",
      message: "SearchNow will use the replacement folder for future Library exports.",
    };
    return "recovered";
  }

  async function exportSelectedBatch(): Promise<void> {
    if (selectedIds.length === 0 || batchBusy) return;
    batchBusy = true;
    feedback = null;
    let result = await runtimeProductFacade.exportLocalContentBatch(selectedIds);
    if (!result.ok && result.error.code === "library_export_destination_invalid") {
      const recovery = await recoverExportDirectory();
      if (recovery === "recovered") {
        result = await runtimeProductFacade.exportLocalContentBatch(selectedIds);
      } else if (recovery === "cancelled" || feedback) {
        batchBusy = false;
        return;
      }
    }
    if (result.ok && result.data) {
      feedback = {
        tone: "success",
        title: "Export complete",
        message: `${result.data.length} backup${result.data.length === 1 ? "" : "s"} exported successfully.`,
      };
      exitSelectionMode();
    } else if (!result.ok) {
      feedback = result.error.code === "library_export_destination_exists"
        ? {
            tone: "error",
            title: "Export stopped by a name conflict",
            message: result.error.message,
          }
        : {
            tone: "error",
            title: "Export failed",
            message: result.error.message,
            actionLabel: "Try export again",
            action: () => void exportSelectedBatch(),
          };
    }
    batchBusy = false;
  }


  async function exportSelectedContent(): Promise<void> {
    const item = detailItem;
    if (!item || actionBusy) return;
    actionBusy = true;
    feedback = null;
    let result = await runtimeProductFacade.exportLocalContent(item.id);
    if (!result.ok && result.error.code === "library_export_destination_invalid") {
      const recovery = await recoverExportDirectory();
      if (recovery === "recovered") {
        result = await runtimeProductFacade.exportLocalContent(item.id);
      } else if (recovery === "cancelled" || feedback) {
        actionBusy = false;
        return;
      }
    }
    if (result.ok && result.data) {
      feedback = {
        tone: "success",
        title: "Export complete",
        message: `${result.data} exported successfully.`,
      };
    } else if (!result.ok) {
      feedback = result.error.code === "library_export_destination_exists"
        ? {
            tone: "error",
            title: "Export stopped by a name conflict",
            message: result.error.message,
          }
        : {
            tone: "error",
            title: "Export failed",
            message: result.error.message,
            actionLabel: "Try export again",
            action: () => void exportSelectedContent(),
          };
    }
    actionBusy = false;
  }

  async function removeSelectedContent(): Promise<void> {
    const item = detailItem;
    if (!item || actionBusy) return;
    actionBusy = true;
    feedback = null;
    const result = await runtimeProductFacade.removeLocalContent(item.id);
    if (result.ok) {
      snapshot = result.data;
      feedback = {
        tone: "success",
        title: "Content removed",
        message: `${item.title} was removed from this device.`,
      };
      selectedItem = null;
    } else {
      feedback = {
        tone: "error",
        title: "Remove failed",
        message: result.error.message,
      };
    }
    actionBusy = false;
  }

  async function openSelectedFolder(): Promise<void> {
    const item = detailItem;
    if (!item || actionBusy) return;
    actionBusy = true;
    const result = await runtimeProductFacade.openLocalContentDirectory(item.id);
    if (!result.ok) {
      feedback = {
        tone: "error",
        title: "Folder could not be opened",
        message: result.error.message,
        actionLabel: "Try again",
        action: () => void openSelectedFolder(),
      };
    } else {
      feedback = null;
    }
    actionBusy = false;
  }

  async function inspectPackage(): Promise<void> {
    if (!runtimeReady || inspectionBusy) return;
    inspectionBusy = true;
    feedback = null;
    const result = await runtimeProductFacade.chooseAndInspectPackage();
    if (result.ok) {
      packageInspection = result.data;
    } else {
      feedback = {
        tone: "error",
        title: "File could not be inspected",
        message: result.error.message,
        actionLabel: "Choose another file",
        action: () => void inspectPackage(),
      };
    }
    inspectionBusy = false;
  }

  async function inspectPackageFolder(): Promise<void> {
    if (!runtimeReady || inspectionBusy) return;
    inspectionBusy = true;
    feedback = null;
    const result = await runtimeProductFacade.chooseAndInspectPackageFolder();
    if (result.ok) {
      packageInspection = result.data;
    } else {
      feedback = {
        tone: "error",
        title: "Folder could not be inspected",
        message: result.error.message,
        actionLabel: "Choose another folder",
        action: () => void inspectPackageFolder(),
      };
    }
    inspectionBusy = false;
  }

  async function updateInspectedPackage(rootId: string): Promise<void> {
    const inspection = packageInspection;
    if (!inspection || importBusy) return;
    importBusy = true;
    feedback = null;
    const request = {
      sourcePath: inspection.sourcePath,
      rootId,
    };
    const result = inspection.inputKind === "mcAddon"
      ? await runtimeProductFacade.replacePackageBundle(request)
      : await runtimeProductFacade.replacePackage(request);
    if (result.ok) {
      const names = result.data.imported.map((item) => item.name);
      feedback = {
        tone: "success",
        title: "Update complete",
        message: names.length === 1
          ? `${names[0]} updated successfully.`
          : `${names.length} bundle items updated/imported successfully.`,
      };
      packageInspection = null;
      loaded = false;
      await refresh();
    } else {
      feedback = {
        tone: "error",
        title: "Update failed",
        message: result.error.message,
      };
    }
    importBusy = false;
  }

  async function importInspectedPackage(rootId: string): Promise<void> {
    const inspection = packageInspection;
    if (!inspection || importBusy) return;
    importBusy = true;
    feedback = null;
    const result = await runtimeProductFacade.importPackage({
      sourcePath: inspection.sourcePath,
      rootId,
    });
    if (result.ok) {
      const importedNames = result.data.world
        ? [result.data.world.name]
        : result.data.imported.map((item) => item.name);
      feedback = {
        tone: "success",
        title: "Import complete",
        message: importedNames.length === 1
          ? `${importedNames[0]} imported successfully.`
          : `${importedNames.length} items imported successfully.`,
      };
      packageInspection = null;
      loaded = false;
      await refresh();
    } else {
      feedback = {
        tone: "error",
        title: "Import failed",
        message: result.error.message,
      };
    }
    importBusy = false;
  }

  async function inspectDroppedPackage(path: string): Promise<void> {
    if (!runtimeReady || inspectionBusy || importBusy || packageInspection !== null) return;
    inspectionBusy = true;
    dropActive = false;
    feedback = null;

    const result = await runtimeProductFacade.inspectPackagePath(path);
    if (result.ok) {
      packageInspection = result.data;
    } else {
      feedback = {
        tone: "error",
        title: "Dropped item could not be inspected",
        message: result.error.message,
      };
    }
    inspectionBusy = false;
  }

  async function refresh(): Promise<boolean> {
    if (!runtimeReady || loading) return false;
    loading = true;
    feedback = null;
    const result = await runtimeProductFacade.loadLibrary();
    if (result.ok) {
      snapshot = result.data;
    } else {
      feedback = {
        tone: "error",
        title: "Library scan failed",
        message: result.error.message,
        actionLabel: "Scan again",
        action: () => void refresh(),
      };
    }
    loaded = true;
    loading = false;
    return result.ok;
  }

  async function locateMinecraftRoot(): Promise<void> {
    if (!runtimeReady || locationBusy) return;
    locationBusy = true;
    feedback = null;

    const selected = await runtimeProductFacade.chooseMinecraftDirectory();
    if (!selected.ok) {
      feedback = {
        tone: "error",
        title: "Folder picker failed",
        message: selected.error.message,
        actionLabel: "Try again",
        action: () => void locateMinecraftRoot(),
      };
      locationBusy = false;
      return;
    }
    if (!selected.data) {
      locationBusy = false;
      return;
    }

    const settingsResult = await runtimeProductFacade.loadSettings();
    if (!settingsResult.ok) {
      feedback = {
        tone: "error",
        title: "Settings could not be loaded",
        message: settingsResult.error.message,
      };
      locationBusy = false;
      return;
    }

    const saveResult = await runtimeProductFacade.saveSettings({
      ...settingsResult.data,
      minecraft: {
        ...settingsResult.data.minecraft,
        rootOverride: selected.data,
      },
    });
    if (!saveResult.ok) {
      feedback = {
        tone: "error",
        title: "Minecraft location could not be saved",
        message: saveResult.error.message,
      };
      locationBusy = false;
      return;
    }

    loaded = false;
    const refreshed = await refresh();
    if (refreshed) {
      feedback = snapshot?.minecraft.state === "found"
        ? {
            tone: "success",
            title: "Minecraft location saved",
            message: "SearchNow is now using the selected Minecraft data folder.",
          }
        : {
            tone: "warning",
            title: "No Minecraft storage found there",
            message: "Choose the Minecraft data folder that contains your worlds and packs.",
            actionLabel: "Choose another folder",
            action: () => void locateMinecraftRoot(),
          };
    }
    locationBusy = false;
  }

  $effect(() => {
    if (runtimeReady) return;
    loaded = false;
    feedback = null;
    actionBusy = false;
    selectedItem = null;
    packageInspection = null;
    inspectionBusy = false;
    importBusy = false;
    selectionMode = false;
    selectedIds = [];
    batchBusy = false;
    locationBusy = false;
    dropActive = false;
  });

  $effect(() => {
    if (!active || !runtimeReady) {
      dropActive = false;
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;

    void runtimeProductFacade
      .subscribeDesktopDrops((event) => {
        if (disposed) return;

        if (event.type === "over") {
          dropActive = packageInspection === null && !inspectionBusy && !importBusy;
          return;
        }

        dropActive = false;
        if (event.type !== "drop" || packageInspection !== null || inspectionBusy || importBusy) return;

        if (event.paths.length !== 1) {
          feedback = {
            tone: "warning",
            title: "Drop one item at a time",
            message: "Drop one Minecraft package or unpacked content folder so SearchNow can inspect it safely.",
          };
          return;
        }

        void inspectDroppedPackage(event.paths[0]);
      })
      .then((result) => {
        if (disposed) {
          if (result.ok) result.data();
          return;
        }
        if (result.ok) {
          unlisten = result.data;
        } else {
          feedback = {
            tone: "warning",
            title: "Drag-and-drop unavailable",
            message: "Use Import file or Import folder instead.",
          };
        }
      });

    return () => {
      disposed = true;
      dropActive = false;
      unlisten?.();
    };
  });

  $effect(() => {
    if (!active || !runtimeReady) return;
    if (!loaded && !loading) void refresh();
  });
</script>

<section class="page" hidden={!active}>
  {#if dropActive}
    <div class="library-drop-overlay" role="status" aria-live="polite">
      <div class="library-drop-overlay__panel">
        <Archive size={24} aria-hidden="true" />
        <strong>Drop to inspect</strong>
        <span>.mcpack, .mcaddon, .mcworld, or one unpacked Minecraft content folder</span>
      </div>
    </div>
  {/if}
  <div class="page-heading page-heading--actions">
    <div>
      <h1>Library</h1>
      <p>
        {snapshot?.minecraft.state === "notFound"
          ? "Connect SearchNow to your Minecraft data folder."
          : snapshot?.minecraft.state === "unsupportedPlatform"
            ? "Minecraft storage detection is unavailable on this platform."
            : snapshot?.minecraft.state === "found"
              ? "Worlds and packs found in your Minecraft storage."
              : "Manage local Minecraft worlds and packs."}
      </p>
      <div class="library-summary">
        {#if snapshot?.minecraft.state === "found"}
          <span>{snapshot.library.summary.total} items</span>
          <span>{snapshot.library.summary.worlds} worlds</span>
          <span>{snapshot.library.summary.behaviorPacks + snapshot.library.summary.resourcePacks + snapshot.library.summary.skinPacks} packs</span>
          {#if snapshot.library.summary.invalidItems > 0}
            <span class="library-summary__warning">{snapshot.library.summary.invalidItems} need review</span>
          {/if}
        {:else if snapshot?.minecraft.state === "notFound"}
          <span>Minecraft location required</span>
        {:else}
          <span>Local Minecraft content</span>
        {/if}
      </div>
    </div>
    <div class="page-heading__actions">
      <button class="button button--secondary" type="button" onclick={inspectPackage} disabled={!runtimeReady || inspectionBusy}>
        <FileSearch size={15} aria-hidden="true" />
        {inspectionBusy ? "Inspecting…" : "Import file"}
      </button>
      <button class="button button--secondary" type="button" onclick={inspectPackageFolder} disabled={!runtimeReady || inspectionBusy}>
        <FolderOpen size={15} aria-hidden="true" />
        Import folder
      </button>
      {#if (snapshot?.library.items.length ?? 0) > 0}
        <button
          class="button button--secondary"
          type="button"
          onclick={() => {
            selectionMode = !selectionMode;
            if (!selectionMode) selectedIds = [];
          }}
          disabled={!runtimeReady || batchBusy}
        >
          <CheckSquare size={15} aria-hidden="true" />
          {selectionMode ? "Selecting…" : "Select"}
        </button>
      {/if}
    </div>
  </div>

  {#if snapshot && snapshot.library.items.length > 0}
    <div
      class:toolbar--library-multi-root={snapshot.minecraft.roots.length > 1}
      class="toolbar toolbar--library"
    >
      <label class="search-field">
        <Search size={15} aria-hidden="true" />
        <input bind:value={query} type="search" placeholder="Search your library" aria-label="Search your library" />
      </label>
      <select class="select-field" bind:value={filter} aria-label="Filter content type">
        <option value="all">All content</option>
        <option value="world">Worlds</option>
        <option value="behaviorPack">Behavior packs</option>
        <option value="resourcePack">Resource packs</option>
        <option value="skinPack">Skin packs</option>
        <option value="duplicates">Duplicate UUIDs</option>
        <option value="missingDependencies">Missing dependencies</option>
        <option value="outdatedDependencies">Outdated dependencies</option>
        <option value="issues">Needs review</option>
      </select>
      {#if snapshot.minecraft.roots.length > 1}
        <select class="select-field" bind:value={rootFilter} aria-label="Filter Minecraft storage">
          <option value="all">All storage</option>
          {#each snapshot.minecraft.roots as root}
            <option value={root.id}>{root.accountHint ?? root.storageKind}</option>
          {/each}
        </select>
      {/if}
      <select class="select-field" bind:value={sort} aria-label="Sort library">
        <option value="nameAsc">Name A–Z</option>
        <option value="nameDesc">Name Z–A</option>
        <option value="type">Content type</option>
        <option value="status">Needs review first</option>
      </select>
      <button
        class="icon-button icon-button--quiet library-refresh"
        type="button"
        title="Rescan library"
        aria-label="Rescan library"
        onclick={refresh}
        disabled={!runtimeReady || loading || batchBusy}
      >
        <RefreshCw size={15} class={loading ? "spin" : ""} aria-hidden="true" />
      </button>
    </div>
    <ResultsBar
      label={`${filteredItems.length} of ${snapshot.library.items.length} item${snapshot.library.items.length === 1 ? "" : "s"}`}
      detail={null}
      showReset={controlsChanged}
      resetLabel="Reset"
      onReset={resetControls}
    />
  {/if}

  {#if selectionMode}
    <div class="toolbar">
      <span>{selectedIds.length} selected</span>
      <button
        class="button button--secondary"
        type="button"
        onclick={exportSelectedBatch}
        disabled={selectedIds.length === 0 || batchBusy}
      >
        <Archive size={15} aria-hidden="true" />
        {batchBusy ? "Exporting…" : "Export selected"}
      </button>
      <button class="button button--ghost" type="button" onclick={exitSelectionMode} disabled={batchBusy}>
        <X size={15} aria-hidden="true" />
        Done
      </button>
    </div>
  {/if}

  {#if snapshot?.library.warnings.length}
    <Notice
      tone="warning"
      title="Some content could not be read"
      message={snapshot.library.warnings[0].message}
    />
  {/if}

  {#if feedback}
    <Notice
      tone={feedback.tone}
      title={feedback.title}
      message={feedback.message}
      actionLabel={feedback.actionLabel ?? null}
      onAction={feedback.action ?? null}
    />
  {/if}

  {#if !runtimeReady}
    <PageState marker="01" title="Library unavailable" message="SearchNow cannot read your Minecraft content right now." />
  {:else if loading && !loaded}
    <PageState kind="loading" title="Scanning content" message="Checking your Minecraft locations." />
  {:else if snapshot && filteredItems.length > 0}
    <div class:library-workspace--selection={selectionMode} class="library-workspace">
      <div class="library-list">
      {#each filteredItems as item (item.id)}
        <button
          class="library-row"
          class:library-row--selected={selectedIds.includes(item.id)}
          class:library-row--active={!selectionMode && detailItem?.id === item.id}
          type="button"
          aria-pressed={selectionMode ? selectedIds.includes(item.id) : undefined}
          onclick={() => openDetails(item)}
        >
          <div class="library-row__icon">
            <ContentTypeMark kind={item.contentType} />
          </div>

          <div class="library-row__main">
            <div class="library-row__title-line">
              <h2 title={item.title}>{item.title}</h2>
              {#if item.isDevelopment}<span class="chip">Development</span>{/if}
            </div>
            <div class="library-row__meta">
              <span>{localContentTypeLabel(item.contentType)}</span>
              {#if snapshot.minecraft.roots.length > 1}<span>{rootLabel(item.rootId)}</span>{/if}
              {#if duplicateIds.has(item.id)}<span class="library-row__issue">Duplicate UUID</span>{/if}
              {#if missingDependencyIds.has(item.id)}<span class="library-row__issue">Missing dependency</span>{/if}
              {#if outdatedDependencyIds.has(item.id)}<span class="library-row__issue">Outdated dependency</span>{/if}
            </div>
          </div>

          <div class="library-row__status">
            <span class:state-text--warning={item.status === "invalidMetadata"} class="state-text">
              {item.status === "ready" ? "Ready" : "Needs review"}
            </span>
            {#if item.version.length}<span class="library-row__version">v{item.version.join(".")}</span>{/if}
          </div>
        </button>
      {/each}
      </div>

      {#if !selectionMode}
      <LibraryDetailPanel
        item={detailItem}
        libraryItems={snapshot.library.items}
        duplicates={selectedDuplicates}
        rootLabel={rootLabel}
        onOpenFolder={openSelectedFolder}
        onExport={exportSelectedContent}
        onRemove={removeSelectedContent}
        onSelectRelated={(item) => (selectedItem = item)}
        {actionBusy}
      />
      {/if}
    </div>
  {:else if snapshot}
    <PageState
      marker="01"
      title={snapshot.library.items.length
        ? "No matching content"
        : snapshot.minecraft.state === "notFound"
          ? "Minecraft wasn't detected"
          : "No Minecraft content found"}
      message={snapshot.library.items.length
        ? "Change the search or filters to see other content."
        : snapshot.minecraft.state === "notFound"
          ? "Choose your Minecraft data folder manually, then SearchNow will scan it again."
          : snapshot.minecraft.message}
      actionLabel={snapshot.library.items.length && controlsChanged
        ? "Reset"
        : snapshot.minecraft.state === "notFound"
          ? "Locate Minecraft"
          : null}
      actionDisabled={locationBusy}
      onAction={snapshot.library.items.length && controlsChanged
        ? resetControls
        : snapshot.minecraft.state === "notFound"
          ? locateMinecraftRoot
          : null}
    />
  {/if}
</section>

<PackageInspectionModal
  inspection={packageInspection}
  roots={snapshot?.minecraft.roots ?? []}
  installedItems={snapshot?.library.items ?? []}
  open={packageInspection !== null}
  onClose={() => {
    if (!importBusy) packageInspection = null;
  }}
  onImport={importInspectedPackage}
  onUpdate={updateInspectedPackage}
  onLocateMinecraft={() => void locateMinecraftRoot()}
  {importBusy}
  {locationBusy}
/>
