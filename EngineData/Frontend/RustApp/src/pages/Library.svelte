<script lang="ts">
  import { Archive, CheckSquare, FileSearch, FolderOpen, RefreshCw, Search, X } from "@lucide/svelte";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import { localContentTypeLabel } from "../app/shared/format";
  import type { LocalBackendSnapshot, LocalContentItem, LocalContentType, PackageInspection } from "../app/shared/types";
  import ContentTypeMark from "../components/ui/ContentTypeMark.svelte";
  import LocalContentDetailModal from "../components/ui/LocalContentDetailModal.svelte";
  import MetricCard from "../components/ui/MetricCard.svelte";
  import Notice from "../components/ui/Notice.svelte";
  import PackageInspectionModal from "../components/ui/PackageInspectionModal.svelte";
  import PageState from "../components/ui/PageState.svelte";
  import ResultsBar from "../components/ui/ResultsBar.svelte";

  type LibraryFilter =
    | "all"
    | LocalContentType
    | "issues"
    | "duplicates"
    | "missingDependencies"
    | "outdatedDependencies";
  type LibrarySort = "nameAsc" | "nameDesc" | "type" | "status";

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
  let success = $state("");
  let error = $state("");
  let query = $state("");
  let filter = $state<LibraryFilter>("all");
  let rootFilter = $state("all");
  let sort = $state<LibrarySort>("nameAsc");

  let duplicateIds = $derived(findDuplicateIds(snapshot?.library.items ?? []));
  let missingDependencyIds = $derived(findMissingDependencyIds(snapshot?.library.items ?? []));
  let outdatedDependencyIds = $derived(findOutdatedDependencyIds(snapshot?.library.items ?? []));
  let selectedDuplicates = $derived(findDuplicatesForItem(selectedItem, snapshot?.library.items ?? []));
  let filteredItems = $derived(
    (snapshot?.library.items ?? [])
      .filter((item) => matchesCurrentFilter(item))
      .slice()
      .sort(compareItems),
  );
  let controlsChanged = $derived(
    query.trim().length > 0 ||
    filter !== "all" ||
    rootFilter !== "all" ||
    sort !== "nameAsc",
  );
  let packCount = $derived(
    snapshot
      ? snapshot.library.summary.behaviorPacks + snapshot.library.summary.resourcePacks + snapshot.library.summary.skinPacks
      : "—",
  );

  function findDuplicatesForItem(
    selected: LocalContentItem | null,
    items: LocalContentItem[],
  ): LocalContentItem[] {
    if (!selected?.manifestUuid) return [];
    const uuid = selected.manifestUuid.toLowerCase();
    return items.filter(
      (item) =>
        item.id !== selected.id &&
        item.manifestUuid !== null &&
        item.manifestUuid.toLowerCase() === uuid,
    );
  }

  function compareVersionParts(left: number[], right: number[]): number {
    const length = Math.max(left.length, right.length);
    for (let index = 0; index < length; index += 1) {
      const leftPart = left[index] ?? 0;
      const rightPart = right[index] ?? 0;
      if (leftPart > rightPart) return 1;
      if (leftPart < rightPart) return -1;
    }
    return 0;
  }

  function findOutdatedDependencyIds(items: LocalContentItem[]): Set<string> {
    const installedByRoot = new Map<string, Map<string, LocalContentItem>>();
    for (const item of items) {
      if (!item.manifestUuid) continue;
      const byUuid = installedByRoot.get(item.rootId) ?? new Map<string, LocalContentItem>();
      byUuid.set(item.manifestUuid.toLowerCase(), item);
      installedByRoot.set(item.rootId, byUuid);
    }

    const outdated = new Set<string>();
    for (const item of items) {
      const installed = installedByRoot.get(item.rootId);
      if (!installed) continue;
      if (
        item.dependencies.some((dependency) => {
          if (dependency.version.length === 0) return false;
          const target = installed.get(dependency.uuid.toLowerCase());
          if (!target || target.version.length === 0) return false;
          return compareVersionParts(target.version, dependency.version) < 0;
        })
      ) {
        outdated.add(item.id);
      }
    }
    return outdated;
  }

  function findMissingDependencyIds(items: LocalContentItem[]): Set<string> {
    const installedByRoot = new Map<string, Set<string>>();
    for (const item of items) {
      if (!item.manifestUuid) continue;
      const uuids = installedByRoot.get(item.rootId) ?? new Set<string>();
      uuids.add(item.manifestUuid.toLowerCase());
      installedByRoot.set(item.rootId, uuids);
    }

    const missing = new Set<string>();
    for (const item of items) {
      const installed = installedByRoot.get(item.rootId) ?? new Set<string>();
      if (item.dependencies.some((dependency) => !installed.has(dependency.uuid.toLowerCase()))) {
        missing.add(item.id);
      }
    }
    return missing;
  }

  function findDuplicateIds(items: LocalContentItem[]): Set<string> {
    const byUuid = new Map<string, LocalContentItem[]>();
    for (const item of items) {
      if (!item.manifestUuid) continue;
      const key = item.manifestUuid.toLowerCase();
      const group = byUuid.get(key) ?? [];
      group.push(item);
      byUuid.set(key, group);
    }
    const ids = new Set<string>();
    for (const group of byUuid.values()) {
      if (group.length < 2) continue;
      for (const item of group) ids.add(item.id);
    }
    return ids;
  }

  function rootLabel(rootId: string): string {
    const root = snapshot?.minecraft.roots.find((candidate) => candidate.id === rootId);
    if (!root) return "Unknown storage";
    return root.accountHint ?? root.storageKind;
  }

  function matchesCurrentFilter(item: LocalContentItem): boolean {
    if (rootFilter !== "all" && item.rootId !== rootFilter) return false;
    if (filter === "issues" && item.status !== "invalidMetadata") return false;
    if (filter === "duplicates" && !duplicateIds.has(item.id)) return false;
    if (filter === "missingDependencies" && !missingDependencyIds.has(item.id)) return false;
    if (filter === "outdatedDependencies" && !outdatedDependencyIds.has(item.id)) return false;
    if (
      filter !== "all" &&
      filter !== "issues" &&
      filter !== "duplicates" &&
      filter !== "missingDependencies" &&
      filter !== "outdatedDependencies" &&
      item.contentType !== filter
    ) return false;
    const needle = query.trim().toLowerCase();
    if (!needle) return true;
    return `${item.title} ${item.description ?? ""}`.toLowerCase().includes(needle);
  }

  function compareItems(left: LocalContentItem, right: LocalContentItem): number {
    if (sort === "nameDesc") return right.title.localeCompare(left.title);
    if (sort === "type") {
      return localContentTypeLabel(left.contentType).localeCompare(localContentTypeLabel(right.contentType)) || left.title.localeCompare(right.title);
    }
    if (sort === "status") {
      const leftNeedsReview = left.status === "invalidMetadata" ? 0 : 1;
      const rightNeedsReview = right.status === "invalidMetadata" ? 0 : 1;
      return leftNeedsReview - rightNeedsReview || left.title.localeCompare(right.title);
    }
    return left.title.localeCompare(right.title);
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

  async function exportSelectedBatch(): Promise<void> {
    if (selectedIds.length === 0 || batchBusy) return;
    batchBusy = true;
    error = "";
    success = "";
    const result = await runtimeProductFacade.exportLocalContentBatch(selectedIds);
    if (result.ok && result.data) {
      success = `${result.data.length} backup${result.data.length === 1 ? "" : "s"} exported successfully.`;
      exitSelectionMode();
    } else if (!result.ok) {
      error = result.error.message;
    }
    batchBusy = false;
  }

  function closeDetails(): void {
    if (!actionBusy) selectedItem = null;
  }

  async function exportSelectedContent(): Promise<void> {
    const item = selectedItem;
    if (!item || actionBusy) return;
    actionBusy = true;
    error = "";
    success = "";
    const result = await runtimeProductFacade.exportLocalContent(item.id);
    if (result.ok && result.data) {
      success = `${result.data} exported successfully.`;
    } else if (!result.ok) {
      error = result.error.message;
    }
    actionBusy = false;
  }

  async function removeSelectedContent(): Promise<void> {
    const item = selectedItem;
    if (!item || actionBusy) return;
    actionBusy = true;
    error = "";
    success = "";
    const result = await runtimeProductFacade.removeLocalContent(item.id);
    if (result.ok) {
      snapshot = result.data;
      success = `${item.title} removed from this device.`;
      selectedItem = null;
    } else {
      error = result.error.message;
    }
    actionBusy = false;
  }

  async function openSelectedFolder(): Promise<void> {
    const item = selectedItem;
    if (!item || actionBusy) return;
    actionBusy = true;
    const result = await runtimeProductFacade.openLocalContentDirectory(item.id);
    if (!result.ok) error = result.error.message;
    else error = "";
    actionBusy = false;
  }

  async function inspectPackage(): Promise<void> {
    if (!runtimeReady || inspectionBusy) return;
    inspectionBusy = true;
    error = "";
    success = "";
    const result = await runtimeProductFacade.chooseAndInspectPackage();
    if (result.ok) packageInspection = result.data;
    else error = result.error.message;
    inspectionBusy = false;
  }

  async function inspectPackageFolder(): Promise<void> {
    if (!runtimeReady || inspectionBusy) return;
    inspectionBusy = true;
    error = "";
    success = "";
    const result = await runtimeProductFacade.chooseAndInspectPackageFolder();
    if (result.ok) packageInspection = result.data;
    else error = result.error.message;
    inspectionBusy = false;
  }

  async function updateInspectedPackage(rootId: string): Promise<void> {
    const inspection = packageInspection;
    if (!inspection || importBusy) return;
    importBusy = true;
    error = "";
    success = "";
    const result = await runtimeProductFacade.replacePackage({
      sourcePath: inspection.sourcePath,
      rootId,
    });
    if (result.ok) {
      const name = result.data.imported[0]?.name ?? "Pack";
      success = `${name} updated successfully.`;
      packageInspection = null;
      loaded = false;
      await refresh();
    } else {
      error = result.error.message;
    }
    importBusy = false;
  }

  async function importInspectedPackage(rootId: string): Promise<void> {
    const inspection = packageInspection;
    if (!inspection || importBusy) return;
    importBusy = true;
    error = "";
    success = "";
    const result = await runtimeProductFacade.importPackage({
      sourcePath: inspection.sourcePath,
      rootId,
    });
    if (result.ok) {
      const importedNames = result.data.world
        ? [result.data.world.name]
        : result.data.imported.map((item) => item.name);
      success = importedNames.length === 1
        ? `${importedNames[0]} imported successfully.`
        : `${importedNames.length} items imported successfully.`;
      packageInspection = null;
      loaded = false;
      await refresh();
    } else {
      error = result.error.message;
    }
    importBusy = false;
  }

  async function refresh(): Promise<void> {
    if (!runtimeReady || loading) return;
    loading = true;
    error = "";
    const result = await runtimeProductFacade.loadLibrary();
    if (result.ok) snapshot = result.data;
    else error = result.error.message;
    loaded = true;
    loading = false;
  }

  $effect(() => {
    if (runtimeReady) return;
    loaded = false;
    error = "";
    success = "";
    actionBusy = false;
    selectedItem = null;
    packageInspection = null;
    inspectionBusy = false;
    importBusy = false;
    selectionMode = false;
    selectedIds = [];
    batchBusy = false;
  });

  $effect(() => {
    if (!active || !runtimeReady) return;
    if (!loaded && !loading) void refresh();
  });
</script>

<section class="page" hidden={!active}>
  <div class="page-heading page-heading--actions">
    <div>
      <h1>Library</h1>
      <p>Browse Minecraft Bedrock worlds and packs detected on this device.</p>
    </div>
    <div class="page-heading__actions">
      <button class="button button--secondary" type="button" onclick={inspectPackage} disabled={!runtimeReady || inspectionBusy}>
        <FileSearch size={15} aria-hidden="true" />
        {inspectionBusy ? "Inspecting…" : "Inspect package"}
      </button>
      <button class="button button--secondary" type="button" onclick={inspectPackageFolder} disabled={!runtimeReady || inspectionBusy}>
        <FolderOpen size={15} aria-hidden="true" />
        Inspect folder
      </button>
      <button
        class="button button--secondary"
        type="button"
        onclick={() => {
          selectionMode = !selectionMode;
          if (!selectionMode) selectedIds = [];
        }}
        disabled={!runtimeReady || batchBusy || (snapshot?.library.items.length ?? 0) === 0}
      >
        <CheckSquare size={15} aria-hidden="true" />
        {selectionMode ? "Selecting…" : "Select"}
      </button>
      <button class="button button--secondary" type="button" onclick={refresh} disabled={!runtimeReady || loading || batchBusy}>
        <RefreshCw size={15} class={loading ? "spin" : ""} aria-hidden="true" />
        {loading ? "Scanning" : "Scan again"}
      </button>
    </div>
  </div>

  <div class="metric-grid metric-grid--four">
    <MetricCard label="Total" value={snapshot?.library.summary.total ?? "—"} detail="Detected content" />
    <MetricCard label="Worlds" value={snapshot?.library.summary.worlds ?? "—"} detail="Minecraft worlds" />
    <MetricCard label="Packs" value={packCount} detail="Behavior, resource, and skin packs" />
    <MetricCard label="Needs review" value={snapshot?.library.summary.invalidItems ?? "—"} detail="Content with metadata issues" />
  </div>

  {#if snapshot}
    <div class="toolbar">
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

  {#if success}
    <Notice tone="success" title="Import complete" message={success} />
  {/if}

  {#if error}
    <Notice tone="error" title="Library needs attention" message={error} actionLabel="Try again" onAction={refresh} />
  {/if}

  {#if !runtimeReady}
    <PageState marker="01" title="Library unavailable" message="SearchNow cannot read your Minecraft content right now." />
  {:else if loading && !loaded}
    <PageState kind="loading" title="Scanning content" message="Checking your Minecraft locations." />
  {:else if snapshot && filteredItems.length > 0}
    <div class="content-grid">
      {#each filteredItems as item (item.id)}
        <button
          class="content-card content-card--interactive w-full p-0 text-left"
          class:ring-2={selectedIds.includes(item.id)}
          type="button"
          aria-pressed={selectionMode ? selectedIds.includes(item.id) : undefined}
          onclick={() => openDetails(item)}
        >
          <div class="content-card__preview">
            <ContentTypeMark kind={item.contentType} />
          </div>
          <div class="content-card__body">
            <div class="content-card__meta">
              <span>{localContentTypeLabel(item.contentType)}</span>
              {#if item.isDevelopment}<span class="chip">Development</span>{/if}
              {#if snapshot.minecraft.roots.length > 1}<span class="chip">{rootLabel(item.rootId)}</span>{/if}
              {#if duplicateIds.has(item.id)}<span class="chip">Duplicate UUID</span>{/if}
              {#if missingDependencyIds.has(item.id)}<span class="chip">Missing dependency</span>{/if}
              {#if outdatedDependencyIds.has(item.id)}<span class="chip">Outdated dependency</span>{/if}
            </div>
            <h2 title={item.title}>{item.title}</h2>
            <div class="content-card__footer">
              <span class:state-text--warning={item.status === "invalidMetadata"} class="state-text">
                {item.status === "ready" ? "Ready" : "Needs review"}
              </span>
              {#if item.version.length}<span>v{item.version.join(".")}</span>{/if}
            </div>
          </div>
        </button>
      {/each}
    </div>
  {:else if snapshot}
    <PageState
      marker="01"
      title={snapshot.library.items.length ? "No matching content" : "No Minecraft content found"}
      message={snapshot.library.items.length ? "Change the search or filters to see other content." : snapshot.minecraft.message}
      actionLabel={snapshot.library.items.length && controlsChanged ? "Reset" : null}
      onAction={snapshot.library.items.length && controlsChanged ? resetControls : null}
    />
  {/if}
</section>

<LocalContentDetailModal
  item={selectedItem}
  libraryItems={snapshot?.library.items ?? []}
  duplicates={selectedDuplicates}
  open={selectedItem !== null}
  onClose={closeDetails}
  onOpenFolder={openSelectedFolder}
  onExport={exportSelectedContent}
  onRemove={removeSelectedContent}
  onSelectRelated={(item) => (selectedItem = item)}
  {actionBusy}
/>

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
  {importBusy}
/>
