<script lang="ts">
  import { X } from "@lucide/svelte";
  import { focusFirstInDialog, trapDialogFocus } from "../../app/shared/dialogFocus";
  import { catalogContentTypeLabel, formatBytes, formatDate } from "../../app/shared/format";
  import type { CatalogItem } from "../../app/shared/types";
  import ContentTypeMark from "./ContentTypeMark.svelte";

  let {
    item,
    open,
    onClose,
    onDownload = null,
    downloadBusy = false,
  }: {
    item: CatalogItem | null;
    open: boolean;
    onClose: () => void;
    onDownload?: (() => void) | null;
    downloadBusy?: boolean;
  } = $props();

  let canDownload = $derived(Boolean(item?.download && item?.fileName && onDownload));
  let dialogElement: HTMLElement | null = $state(null);

  $effect(() => {
    if (!open || !item) return;
    const focusToRestore = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    queueMicrotask(() => focusFirstInDialog(dialogElement));

    return () => {
      queueMicrotask(() => focusToRestore?.focus());
    };
  });

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === event.currentTarget && !downloadBusy) onClose();
  }

  function availabilityTitle(): string {
    if (canDownload) return "Ready to download";
    if (!item?.fileName) return "Download file unavailable";
    if (!item?.download) return "Download source unavailable";
    return "Download unavailable";
  }

  function availabilityMessage(): string {
    if (canDownload) {
      return "SearchNow has enough provider metadata to start this download safely.";
    }
    if (!item?.fileName) {
      return "This source did not provide a safe output filename for this item.";
    }
    if (!item?.download) {
      return "This source did not provide a permitted download reference for this item.";
    }
    return "This item can be previewed, but SearchNow cannot start a download from the available metadata.";
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (!open) return;
    if (event.key === "Escape" && !downloadBusy) {
      event.preventDefault();
      onClose();
      return;
    }
    trapDialogFocus(event, dialogElement);
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && item}
  <div class="catalog-modal__backdrop" role="presentation" onclick={handleBackdrop}>
    <div bind:this={dialogElement} class="catalog-modal" role="dialog" aria-modal="true" aria-labelledby="catalog-modal-title" aria-busy={downloadBusy} tabindex="-1">
      <button class="catalog-modal__close" type="button" aria-label="Close details" onclick={onClose} disabled={downloadBusy}>
        <X size={18} aria-hidden="true" />
      </button>

      <div class="catalog-modal__media">
        {#if item.thumbnailUrl}
          <img src={item.thumbnailUrl} alt="" decoding="async" referrerpolicy="no-referrer" />
        {:else}
          <ContentTypeMark kind={item.contentType} />
        {/if}
        <span class="catalog-modal__type">{catalogContentTypeLabel(item.contentType)}</span>
      </div>

      <div class="catalog-modal__content">
        <div class="catalog-modal__heading">
          <div>
            <h2 id="catalog-modal-title">{item.title}</h2>
            <p>{item.creatorName ? `By ${item.creatorName}` : item.provider}</p>
          </div>
        </div>

        <div class:catalog-modal__availability--ready={canDownload} class="catalog-modal__availability">
          <strong>{availabilityTitle()}</strong>
          <span>{availabilityMessage()}</span>
        </div>

        <div class="catalog-modal__facts">
          {#if item.publishedAtMs}
            <div><span>Released</span><strong>{formatDate(item.publishedAtMs)}</strong></div>
          {/if}
          {#if item.updatedAtMs && item.updatedAtMs !== item.publishedAtMs}
            <div><span>Updated</span><strong>{formatDate(item.updatedAtMs)}</strong></div>
          {/if}
          {#if item.expectedBytes}
            <div><span>Size</span><strong>{formatBytes(item.expectedBytes)}</strong></div>
          {/if}
          <div><span>Source</span><strong>{item.provider}</strong></div>
        </div>

        {#if item.description}
          <p class="catalog-modal__description">{item.description}</p>
        {/if}

        {#if item.tags.length}
          <div class="catalog-modal__tags" aria-label="Tags">
            {#each item.tags as tag (tag)}<span>{tag}</span>{/each}
          </div>
        {/if}

        {#if onDownload && item.download && item.fileName}
          <div class="catalog-modal__footer">
            <button class="button button--primary" type="button" onclick={() => onDownload?.()} disabled={!canDownload || downloadBusy}>
              {downloadBusy ? "Starting…" : "Download"}
            </button>
          </div>
        {:else if !item.download || !item.fileName}
          <div class="catalog-modal__footer">
            <span class="catalog-modal__unavailable">{availabilityTitle()}</span>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}
