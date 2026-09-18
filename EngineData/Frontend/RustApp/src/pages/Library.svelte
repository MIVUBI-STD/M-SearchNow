<script lang="ts">
  import { FileSearch, RefreshCw, Search } from "@lucide/svelte";
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

  type LibraryFilter = "all" | LocalContentType | "issues";
  type LibrarySort = "nameAsc" | "nameDesc" | "type" | "status";

  let { runtimeReady, active }: { runtimeReady: boolean; active: boolean } = $props();
  let loading = $state(false);
  let loaded = $state(false);
  let snapshot = $state<LocalBackendSnapshot | null>(null);
  let selectedItem = $state<LocalContentItem | null>(null);
  let actionBusy = $state(false);
  let inspectionBusy = $state(false);
  let packageInspection = $state<PackageInspection | null>(null);
  let error = $state("");
  let query = $state("");
  let filter = $state<LibraryFilter>("all");
  let sort = $state<LibrarySort>("nameAsc");

  let filteredItems = $derived(
    (snapshot?.library.items ?? [])
      .filter((item) => matchesCurrentFilter(item))
      .slice()
      .sort(compareItems),
  );
  let controlsChanged = $derived(query.trim().length > 0 || filter !== "all" || sort !== "nameAsc");
  let packCount = $derived(
    snapshot
      ? snapshot.library.summary.behaviorPacks + snapshot.library.summary.resourcePacks + snapshot.library.summary.skinPacks
      : "—",
  );

  function matchesCurrentFilter(item: LocalContentItem): boolean {
    if (filter === "issues" && item.status !== "invalidMetadata") return false;
    if (filter !== "all" && filter !== "issues" && item.contentType !== filter) return false;
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
    sort = "nameAsc";
  }

  function openDetails(item: LocalContentItem): void {
    selectedItem = item;
  }

  function closeDetails(): void {
    if (!actionBusy) selectedItem = null;
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
    const result = await runtimeProductFacade.chooseAndInspectPackage();
    if (result.ok) packageInspection = result.data;
    else error = result.error.message;
    inspectionBusy = false;
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
    actionBusy = false;
    selectedItem = null;
    packageInspection = null;
    inspectionBusy = false;
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
      <button class="button button--secondary" type="button" onclick={refresh} disabled={!runtimeReady || loading}>
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
        <option value="issues">Needs review</option>
      </select>
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

  {#if snapshot?.library.warnings.length}
    <Notice
      tone="warning"
      title="Some content could not be read"
      message={snapshot.library.warnings[0].message}
    />
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
        <button class="content-card content-card--interactive w-full p-0 text-left" type="button" onclick={() => openDetails(item)}>
          <div class="content-card__preview">
            <ContentTypeMark kind={item.contentType} />
          </div>
          <div class="content-card__body">
            <div class="content-card__meta">
              <span>{localContentTypeLabel(item.contentType)}</span>
              {#if item.isDevelopment}<span class="chip">Development</span>{/if}
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
  open={selectedItem !== null}
  onClose={closeDetails}
  onOpenFolder={openSelectedFolder}
  {actionBusy}
/>

<PackageInspectionModal
  inspection={packageInspection}
  open={packageInspection !== null}
  onClose={() => (packageInspection = null)}
/>
