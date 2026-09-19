<script lang="ts">
  import { Archive, FolderOpen, Trash2 } from "@lucide/svelte";
  import { localContentTypeLabel } from "../../app/shared/format";
  import type { LocalContentItem } from "../../app/shared/types";
  import ContentTypeMark from "../ui/ContentTypeMark.svelte";
  import TechnicalDetails from "../ui/TechnicalDetails.svelte";

  let {
    item,
    libraryItems,
    duplicates,
    rootLabel,
    onOpenFolder,
    onExport,
    onRemove,
    onSelectRelated,
    actionBusy = false,
  }: {
    item: LocalContentItem | null;
    libraryItems: LocalContentItem[];
    duplicates: LocalContentItem[];
    rootLabel: (rootId: string) => string;
    onOpenFolder: () => void;
    onExport: () => void;
    onRemove: () => void;
    onSelectRelated: (item: LocalContentItem) => void;
    actionBusy?: boolean;
  } = $props();

  let confirmRemove = $state(false);

  $effect(() => {
    void item?.id;
    confirmRemove = false;
  });

  function dependencyItem(uuid: string): LocalContentItem | null {
    if (!item) return null;
    return libraryItems.find(
      (candidate) =>
        candidate.rootId === item.rootId &&
        candidate.manifestUuid?.toLowerCase() === uuid.toLowerCase(),
    ) ?? null;
  }
</script>

<aside class="library-detail" aria-label="Selected content details">
  {#if item}
    <div class="library-detail__hero">
      <div class="library-detail__mark">
        <ContentTypeMark kind={item.contentType} />
      </div>
      <div class="library-detail__heading">
        <span>{localContentTypeLabel(item.contentType)}</span>
        <h2>{item.title}</h2>
        <div class="library-detail__status">
          <strong class:state-text--warning={item.status === "invalidMetadata"} class="state-text">
            {item.status === "ready" ? "Ready" : "Needs review"}
          </strong>
          {#if item.version.length}<span>v{item.version.join(".")}</span>{/if}
        </div>
      </div>
    </div>

    <div class="library-detail__actions">
      <button class="button button--secondary" type="button" onclick={onOpenFolder} disabled={actionBusy}>
        <FolderOpen size={15} aria-hidden="true" />
        Open folder
      </button>
      <button class="button button--secondary" type="button" onclick={onExport} disabled={actionBusy}>
        <Archive size={15} aria-hidden="true" />
        Export
      </button>
    </div>

    {#if item.description}
      <p class="library-detail__description">{item.description}</p>
    {/if}

    {#if item.issue}
      <div class="library-detail__notice">
        <strong>Needs review</strong>
        <span>{item.issue}</span>
      </div>
    {/if}

    <dl class="library-detail__facts">
      <div>
        <dt>Storage</dt>
        <dd>{rootLabel(item.rootId)}</dd>
      </div>
      <div>
        <dt>Source</dt>
        <dd>{item.isDevelopment ? "Development folder" : "Installed content"}</dd>
      </div>
      <div>
        <dt>Dependencies</dt>
        <dd>{item.dependencies.length}</dd>
      </div>
    </dl>

    {#if item.dependencies.length}
      <div class="library-detail__section">
        <span class="library-detail__label">Dependencies</span>
        <div class="library-detail__links">
          {#each item.dependencies as dependency}
            {@const installed = dependencyItem(dependency.uuid)}
            {#if installed}
              <button class="library-detail__link" type="button" onclick={() => onSelectRelated(installed)}>
                <span>{installed.title}</span>
                <small>{dependency.version.length ? `Requires v${dependency.version.join(".")}` : "Installed"}</small>
              </button>
            {:else}
              <div class="library-detail__link library-detail__link--missing">
                <span>Missing dependency</span>
                <small>{dependency.uuid}</small>
              </div>
            {/if}
          {/each}
        </div>
      </div>
    {/if}

    {#if duplicates.length}
      <div class="library-detail__section">
        <span class="library-detail__label">Duplicate UUID</span>
        <div class="library-detail__links">
          {#each duplicates as duplicate}
            <button class="library-detail__link" type="button" onclick={() => onSelectRelated(duplicate)}>
              <span>{duplicate.title}</span>
              <small>{duplicate.version.length ? `v${duplicate.version.join(".")}` : "Version unknown"}</small>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <TechnicalDetails
      items={[
        { label: "Location", value: item.path },
        { label: "Storage root", value: item.rootId },
        { label: "Manifest UUID", value: item.manifestUuid },
      ]}
    />

    <div class="library-detail__danger">
      {#if confirmRemove}
        <div>
          <strong>Remove this content?</strong>
          <span>This permanently removes the selected Minecraft content folder from this device.</span>
        </div>
        <button class="button button--ghost library-detail__remove-confirm" type="button" onclick={onRemove} disabled={actionBusy}>
          <Trash2 size={14} aria-hidden="true" />
          {actionBusy ? "Removing…" : "Remove permanently"}
        </button>
      {:else}
        <button class="button button--ghost" type="button" onclick={() => (confirmRemove = true)} disabled={actionBusy}>
          <Trash2 size={14} aria-hidden="true" />
          Remove
        </button>
      {/if}
    </div>
  {:else}
    <div class="library-detail__empty">
      <strong>Select content</strong>
      <span>Choose an item to inspect it without leaving your library.</span>
    </div>
  {/if}
</aside>
