<script lang="ts">
  import { Archive, FolderOpen, Trash2, X } from "@lucide/svelte";
  import { localContentTypeLabel } from "../../app/shared/format";
  import type { LocalContentItem } from "../../app/shared/types";
  import ContentTypeMark from "./ContentTypeMark.svelte";
  import TechnicalDetails from "./TechnicalDetails.svelte";

  let {
    item,
    libraryItems,
    duplicates,
    open,
    onClose,
    onOpenFolder,
    onExport,
    onRemove,
    actionBusy = false,
  }: {
    item: LocalContentItem | null;
    libraryItems: LocalContentItem[];
    duplicates: LocalContentItem[];
    open: boolean;
    onClose: () => void;
    onOpenFolder: () => void;
    onExport: () => void;
    onRemove: () => void;
    actionBusy?: boolean;
  } = $props();

  let confirmRemove = $state(false);

  $effect(() => {
    if (!open) confirmRemove = false;
  });

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

  function dependencyLabel(uuid: string, requiredVersion: number[]): string {
    const match = libraryItems.find(
      (candidate) =>
        candidate.rootId === item?.rootId &&
        candidate.manifestUuid?.toLowerCase() === uuid.toLowerCase(),
    );
    if (!match) return "Missing";
    const installed = match.version.length ? `v${match.version.join(".")}` : "version unknown";
    if (
      requiredVersion.length > 0 &&
      match.version.length > 0 &&
      compareVersionParts(match.version, requiredVersion) < 0
    ) {
      return `Outdated · ${installed}`;
    }
    return `Installed · ${installed}`;
  }

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === event.currentTarget && !actionBusy) onClose();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (open && event.key === "Escape" && !actionBusy) onClose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && item}
  <div class="catalog-modal__backdrop" role="presentation" onclick={handleBackdrop}>
    <div class="catalog-modal" role="dialog" aria-modal="true" aria-labelledby="library-modal-title">
      <button class="catalog-modal__close" type="button" aria-label="Close details" onclick={onClose} disabled={actionBusy}>
        <X size={18} aria-hidden="true" />
      </button>

      <div class="catalog-modal__media catalog-modal__media--local">
        <ContentTypeMark kind={item.contentType} />
        <span class="catalog-modal__type">{localContentTypeLabel(item.contentType)}</span>
      </div>

      <div class="catalog-modal__content">
        <div class="catalog-modal__heading">
          <div>
            <h2 id="library-modal-title">{item.title}</h2>
            <p>{item.status === "ready" ? "Ready" : "Needs review"}</p>
          </div>
        </div>

        <div class="catalog-modal__facts">
          <div><span>Type</span><strong>{localContentTypeLabel(item.contentType)}</strong></div>
          {#if item.version.length}<div><span>Version</span><strong>v{item.version.join(".")}</strong></div>{/if}
          {#if item.isDevelopment}<div><span>Source</span><strong>Development folder</strong></div>{/if}
        </div>

        {#if item.description}
          <p class="catalog-modal__description">{item.description}</p>
        {/if}

        {#if item.issue}
          <div class="catalog-modal__issue">
            <strong>Needs review</strong>
            <span>{item.issue}</span>
          </div>
        {/if}

        {#if item.dependencies.length}
          <div class="catalog-modal__issue">
            <strong>Dependencies</strong>
            <span>
              {item.dependencies.map((dependency) =>
                `${dependency.uuid}${dependency.version.length ? ` · requires v${dependency.version.join(".")}` : ""} · ${dependencyLabel(dependency.uuid, dependency.version)}`
              ).join(" · ")}
            </span>
          </div>
        {/if}

        {#if duplicates.length}
          <div class="catalog-modal__issue">
            <strong>Duplicate manifest UUID</strong>
            <span>
              {duplicates.map((duplicate) =>
                `${duplicate.title}${duplicate.version.length ? ` · v${duplicate.version.join(".")}` : ""}`
              ).join(" · ")}
            </span>
          </div>
        {/if}

        <TechnicalDetails
          items={[
            { label: "Location", value: item.path },
            { label: "Storage root", value: item.rootId },
            { label: "Manifest UUID", value: item.manifestUuid },
          ]}
        />

        {#if confirmRemove}
          <div class="catalog-modal__issue">
            <strong>Remove permanently?</strong>
            <span>This removes the selected Minecraft content folder from this device. SearchNow cannot undo this action.</span>
          </div>
        {/if}

        <div class="catalog-modal__footer">
          <button class="button button--secondary" type="button" onclick={onOpenFolder} disabled={actionBusy}>
            <FolderOpen size={15} aria-hidden="true" />
            Open folder
          </button>
          <button class="button button--secondary" type="button" onclick={onExport} disabled={actionBusy}>
            <Archive size={15} aria-hidden="true" />
            Export backup
          </button>
          {#if confirmRemove}
            <button class="button button--primary" type="button" onclick={onRemove} disabled={actionBusy}>
              <Trash2 size={15} aria-hidden="true" />
              {actionBusy ? "Removing…" : "Remove permanently"}
            </button>
          {:else}
            <button class="button button--ghost" type="button" onclick={() => (confirmRemove = true)} disabled={actionBusy}>
              <Trash2 size={15} aria-hidden="true" />
              Remove
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}
