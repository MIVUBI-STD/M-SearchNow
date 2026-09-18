<script lang="ts">
  import { Search } from "@lucide/svelte";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import { catalogContentTypeLabel, formatDate } from "../app/shared/format";
  import type {
    CatalogContentType,
    CatalogItem,
    CatalogPage,
    CatalogRequest,
    CatalogSort,
    ProviderRuntimeStatus,
  } from "../app/shared/types";
  import CatalogDetailModal from "../components/ui/CatalogDetailModal.svelte";
  import ContentTypeMark from "../components/ui/ContentTypeMark.svelte";
  import Notice from "../components/ui/Notice.svelte";
  import PageState from "../components/ui/PageState.svelte";
  import ResultsBar from "../components/ui/ResultsBar.svelte";

  type ContentFilter = "all" | CatalogContentType;

  let {
    runtimeReady,
    providers,
    active,
  }: {
    runtimeReady: boolean;
    providers: ProviderRuntimeStatus[];
    active: boolean;
  } = $props();

  let query = $state("");
  let contentFilter = $state<ContentFilter>("all");
  let sort = $state<CatalogSort>("relevance");
  let selectedProvider = $state("");
  let selectedItem = $state<CatalogItem | null>(null);
  let page = $state<CatalogPage | null>(null);
  let loading = $state(false);
  let loadingMore = $state(false);
  let downloadBusy = $state(false);
  let error = $state("");
  let downloadMessage = $state("");
  let requestSequence = 0;

  let catalogProviders = $derived(providers.filter((provider) => provider.capabilities.catalog));
  let controlsChanged = $derived(query.trim().length > 0 || contentFilter !== "all" || sort !== "relevance");

  $effect(() => {
    const firstProvider = catalogProviders[0]?.capabilities.provider ?? "";
    const stillAvailable = catalogProviders.some(
      (provider) => provider.capabilities.provider === selectedProvider,
    );
    if (!stillAvailable && selectedProvider !== firstProvider) selectedProvider = firstProvider;
  });

  function makeRequest(
    provider: string,
    text: string,
    filter: ContentFilter,
    selectedSort: CatalogSort,
    cursor: string | null,
  ): CatalogRequest {
    return {
      provider,
      query: {
        text: text.trim() || null,
        filters: {
          contentTypes: filter === "all" ? [] : [filter],
          tags: [],
        },
        sort: selectedSort,
        page: { limit: 30, cursor },
      },
    };
  }

  function mergeUniqueItems(existing: CatalogItem[], incoming: CatalogItem[]): CatalogItem[] {
    const seen = new Set(existing.map((item) => `${item.provider}:${item.itemId}`));
    const merged = existing.slice();
    for (const item of incoming) {
      const key = `${item.provider}:${item.itemId}`;
      if (seen.has(key)) continue;
      seen.add(key);
      merged.push(item);
    }
    return merged;
  }

  function resetControls(): void {
    query = "";
    contentFilter = "all";
    sort = "relevance";
  }

  function openDetails(item: CatalogItem): void {
    selectedItem = item;
    downloadMessage = "";
  }

  function closeDetails(): void {
    if (!downloadBusy) selectedItem = null;
  }

  async function startDownload(): Promise<void> {
    const item = selectedItem;
    if (!item?.download || !item.fileName || downloadBusy) return;

    downloadBusy = true;
    downloadMessage = "";
    const picker = await runtimeProductFacade.chooseDownloadDirectory();
    if (!picker.ok) {
      downloadMessage = picker.error.message;
      downloadBusy = false;
      return;
    }
    if (!picker.data) {
      downloadBusy = false;
      return;
    }

    const result = await runtimeProductFacade.queueCatalogDownload({
      download: item.download,
      displayName: item.title,
      destinationFileName: item.fileName,
      destinationDirectory: picker.data,
      expectedBytes: item.expectedBytes,
      expectedSha256: item.expectedSha256,
    });
    if (result.ok) {
      selectedItem = null;
      downloadMessage = `${item.title} was added to Downloads.`;
    } else {
      downloadMessage = result.error.message;
    }
    downloadBusy = false;
  }

  async function queryCatalog(
    provider: string,
    text: string,
    filter: ContentFilter,
    selectedSort: CatalogSort,
    cursor: string | null = null,
    append = false,
  ): Promise<void> {
    if (!runtimeReady || !active || !provider) return;
    const sequence = ++requestSequence;
    if (append) loadingMore = true;
    else loading = true;
    error = "";

    const result = await runtimeProductFacade.queryCatalog(
      makeRequest(provider, text, filter, selectedSort, cursor),
    );
    if (sequence !== requestSequence) return;

    if (result.ok) {
      if (append && page) {
        page = {
          ...result.data,
          items: mergeUniqueItems(page.items, result.data.items),
        };
      } else {
        page = result.data;
      }
    } else {
      error = result.error.message;
      if (!append) page = null;
    }
    loading = false;
    loadingMore = false;
  }

  function retryCurrentQuery(): void {
    void queryCatalog(selectedProvider, query, contentFilter, sort);
  }

  async function loadMore(): Promise<void> {
    if (!page?.nextCursor || loadingMore) return;
    await queryCatalog(selectedProvider, query, contentFilter, sort, page.nextCursor, true);
  }

  $effect(() => {
    const provider = selectedProvider;
    const text = query;
    const filter = contentFilter;
    const selectedSort = sort;
    const canQuery = active && runtimeReady && provider.length > 0;

    requestSequence += 1;
    loading = false;
    loadingMore = false;
    if (!canQuery) error = "";

    void text;
    void filter;
    void selectedSort;
  });

  $effect(() => {
    const provider = selectedProvider;
    const text = query;
    const filter = contentFilter;
    const selectedSort = sort;
    if (!active || !runtimeReady || !provider) return;
    const timer = setTimeout(() => {
      void queryCatalog(provider, text, filter, selectedSort);
    }, 320);
    return () => clearTimeout(timer);
  });
</script>

<section class="page" hidden={!active}>
  <div class="page-heading">
    <div>
      <span class="eyebrow">Catalog</span>
      <h1>Discover</h1>
      <p>Find Minecraft content from connected sources.</p>
    </div>
  </div>

  {#if catalogProviders.length > 0}
    <div class="toolbar toolbar--catalog">
      <label class="search-field search-field--wide">
        <Search size={15} aria-hidden="true" />
        <input bind:value={query} type="search" placeholder="Search content" aria-label="Search content" />
      </label>
      {#if catalogProviders.length > 1}
        <select class="select-field" bind:value={selectedProvider} aria-label="Content source">
          {#each catalogProviders as provider (provider.capabilities.provider)}
            <option value={provider.capabilities.provider}>{provider.capabilities.provider}</option>
          {/each}
        </select>
      {/if}
      <select class="select-field" bind:value={contentFilter} aria-label="Content type">
        <option value="all">All types</option>
        <option value="world">Worlds</option>
        <option value="addon">Add-Ons</option>
        <option value="resourcePack">Resource Packs</option>
        <option value="skin">Skins</option>
        <option value="persona">Persona</option>
      </select>
      <select class="select-field" bind:value={sort} aria-label="Sort results">
        <option value="relevance">Most relevant</option>
        <option value="newest">Newest first</option>
        <option value="oldest">Oldest first</option>
        <option value="nameAsc">Name A–Z</option>
        <option value="nameDesc">Name Z–A</option>
      </select>
    </div>
    {#if page}
      <ResultsBar
        label={`${page.items.length} result${page.items.length === 1 ? "" : "s"}`}
        detail={page.nextCursor ? "More results available" : "All loaded results shown"}
        showReset={controlsChanged}
        resetLabel="Reset"
        onReset={resetControls}
      />
    {/if}
  {:else}
    <div class="search-shell" aria-disabled="true">
      <span><Search size={14} aria-hidden="true" /> Search content</span>
      <kbd>Source unavailable</kbd>
    </div>
  {/if}

  {#if downloadMessage}
    <Notice tone={selectedItem ? "warning" : "success"} title={selectedItem ? "Download could not start" : "Download started"} message={downloadMessage} />
  {/if}

  {#if error}
    <Notice
      tone="warning"
      title="Content unavailable"
      message={error}
      actionLabel="Retry"
      actionDisabled={loading}
      onAction={retryCurrentQuery}
    />
  {/if}

  {#if !runtimeReady}
    <PageState marker="02" title="Discover unavailable" message="SearchNow cannot access connected content sources right now." />
  {:else if catalogProviders.length === 0}
    <PageState marker="02" title="No content source connected" message="Connect a supported source to browse downloadable content." />
  {:else if loading && !page}
    <PageState kind="loading" title="Searching" message="Loading content from the selected source." />
  {:else if page && page.items.length > 0}
    <div class="content-grid content-grid--catalog" aria-busy={loading}>
      {#each page.items as item (`${item.provider}:${item.itemId}`)}
        <article class="content-card content-card--catalog catalog-card">
          <div class="content-card__preview catalog-card__thumbnail">
            {#if item.thumbnailUrl}
              <img src={item.thumbnailUrl} alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" />
            {:else}
              <ContentTypeMark kind={item.contentType} />
            {/if}
            <span class="catalog-card__type">{catalogContentTypeLabel(item.contentType)}</span>
          </div>
          <div class="content-card__body">
            <h2 title={item.title}>{item.title}</h2>
            <div class="catalog-card__creator">{item.creatorName ? `By ${item.creatorName}` : item.provider}</div>
            <div class="catalog-card__facts">
              {#if item.publishedAtMs}<span>Released {formatDate(item.publishedAtMs)}</span>{/if}
              {#if item.updatedAtMs && item.updatedAtMs !== item.publishedAtMs}<span>Updated {formatDate(item.updatedAtMs)}</span>{/if}
            </div>
            <div class="content-card__footer">
              <span class="state-text">{item.download && item.fileName ? "Download available" : "View details"}</span>
              <button class="button button--secondary button--compact" type="button" onclick={() => openDetails(item)}>View details</button>
            </div>
          </div>
        </article>
      {/each}
    </div>
    {#if page.nextCursor}
      <div class="center-actions">
        <button class="button button--secondary" type="button" onclick={loadMore} disabled={loadingMore}>{loadingMore ? "Loading" : "Load more"}</button>
      </div>
    {/if}
  {:else if page}
    <PageState
      marker="02"
      title="No matching content"
      message="Try a different search or reset the current filters."
      actionLabel={controlsChanged ? "Reset" : null}
      onAction={controlsChanged ? resetControls : null}
    />
  {/if}
</section>

<CatalogDetailModal
  item={selectedItem}
  open={selectedItem !== null}
  onClose={closeDetails}
  onDownload={selectedItem?.download && selectedItem.fileName ? startDownload : null}
  {downloadBusy}
/>
