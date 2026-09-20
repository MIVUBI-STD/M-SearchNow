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
    hasDuplicate,
    hasMissingDependency,
    hasOutdatedDependency,
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
    hasDuplicate: boolean;
    hasMissingDependency: boolean;
    hasOutdatedDependency: boolean;
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

  function versionCompare(left: number[], right: number[]): number {
    const length = Math.max(left.length, right.length);
    for (let index = 0; index < length; index += 1) {
      const leftPart = left[index] ?? 0;
      const rightPart = right[index] ?? 0;
      if (leftPart > rightPart) return 1;
      if (leftPart < rightPart) return -1;
    }
    return 0;
  }

  function dependencyState(uuid: string, requiredVersion: number[]): "missing" | "outdated" | "ready" {
    const installed = dependencyItem(uuid);
    if (!installed) return "missing";
    if (
      requiredVersion.length > 0 &&
      installed.version.length > 0 &&
      versionCompare(installed.version, requiredVersion) < 0
    ) {
      return "outdated";
    }
    return "ready";
  }

  function requiredByItems(): LocalContentItem[] {
    if (!item?.manifestUuid) return [];
    const uuid = item.manifestUuid.toLowerCase();
    return libraryItems.filter(
      (candidate) =>
        candidate.id !== item.id &&
        candidate.rootId === item.rootId &&
        candidate.dependencies.some((dependency) => dependency.uuid.toLowerCase() === uuid),
    );
  }

  function needsReview(): boolean {
    return item?.status === "invalidMetadata" || hasDuplicate || hasMissingDependency || hasOutdatedDependency;
  }

  function removeImpactTitle(dependents: LocalContentItem[]): string {
    if (dependents.length > 0) {
      return `Remove content used by ${dependents.length} other item${dependents.length === 1 ? "" : "s"}?`;
    }
    if (hasDuplicate) return "Remove this duplicate installation?";
    return "Remove this content?";
  }

  function removeImpactMessage(dependents: LocalContentItem[]): string {
    if (dependents.length > 0) {
      const names = dependents.slice(0, 3).map((dependent) => dependent.title).join(", ");
      const remaining = dependents.length - Math.min(dependents.length, 3);
      return `This content is required by ${names}${remaining > 0 ? ` and ${remaining} more` : ""}. Removing it may leave those items incomplete.`;
    }
    if (hasDuplicate) {
      return "Another installed item uses the same manifest UUID. Verify which copy you want to keep before removing this one.";
    }
    return "This permanently removes the selected Minecraft content folder from this device.";
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
          <strong class:state-text--warning={needsReview()} class="state-text">
            {needsReview() ? "Needs review" : "Healthy"}
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

    <div class="library-detail__section">
      <span class="library-detail__label">Health</span>
      <div class="library-detail__health-list">
        <div class:library-detail__health-item--warning={item.status !== "ready"} class="library-detail__health-item">
          <span>{item.status === "ready" ? "Metadata valid" : "Metadata needs review"}</span>
          <small>{item.status === "ready" ? "Manifest metadata was read successfully." : (item.issue ?? "SearchNow could not validate this content metadata.")}</small>
        </div>
        {#if hasMissingDependency}
          <div class="library-detail__health-item library-detail__health-item--warning">
            <span>Missing dependency</span>
            <small>Install the required pack before relying on this content.</small>
          </div>
        {/if}
        {#if hasOutdatedDependency}
          <div class="library-detail__health-item library-detail__health-item--warning">
            <span>Dependency version too old</span>
            <small>Update the required pack to at least the requested version.</small>
          </div>
        {/if}
        {#if hasDuplicate}
          <div class="library-detail__health-item library-detail__health-item--warning">
            <span>Duplicate installation</span>
            <small>Another installed item uses the same manifest UUID.</small>
          </div>
        {/if}
        {#if !needsReview()}
          <div class="library-detail__health-item">
            <span>No known issues</span>
            <small>Dependencies and duplicate checks are clear.</small>
          </div>
        {/if}
      </div>
    </div>

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
            {@const state = dependencyState(dependency.uuid, dependency.version)}
            {#if installed}
              <button
                class="library-detail__link"
                class:library-detail__link--missing={state === "outdated"}
                type="button"
                onclick={() => onSelectRelated(installed)}
              >
                <span>{installed.title}</span>
                <small>
                  {state === "outdated"
                    ? `Installed v${installed.version.join(".")} · requires v${dependency.version.join(".")}+`
                    : dependency.version.length
                      ? `Installed · requires v${dependency.version.join(".")}+`
                      : "Installed"}
                </small>
              </button>
            {:else}
              <div class="library-detail__link library-detail__link--missing">
                <span>Missing dependency</span>
                <small>{dependency.version.length ? `Required v${dependency.version.join(".")}+` : dependency.uuid}</small>
              </div>
            {/if}
          {/each}
        </div>
      </div>
    {/if}

    {@const dependents = requiredByItems()}
    {#if dependents.length}
      <div class="library-detail__section">
        <span class="library-detail__label">Required by</span>
        <div class="library-detail__links">
          {#each dependents as dependent}
            <button class="library-detail__link" type="button" onclick={() => onSelectRelated(dependent)}>
              <span>{dependent.title}</span>
              <small>{localContentTypeLabel(dependent.contentType)}</small>
            </button>
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
        <div class:library-detail__danger-impact={dependents.length > 0 || hasDuplicate}>
          <strong>{removeImpactTitle(dependents)}</strong>
          <span>{removeImpactMessage(dependents)}</span>
          {#if dependents.length > 0}
            <small>Recommended: export a backup first, or review the dependent items above before continuing.</small>
          {:else if hasDuplicate}
            <small>Recommended: compare both copies first so you do not remove the version you intend to keep.</small>
          {/if}
        </div>
        <div class="library-detail__danger-actions">
          <button
            class="button button--ghost"
            type="button"
            onclick={() => (confirmRemove = false)}
            disabled={actionBusy}
          >
            Cancel
          </button>
          <button class="button button--ghost library-detail__remove-confirm" type="button" onclick={onRemove} disabled={actionBusy}>
            <Trash2 size={14} aria-hidden="true" />
            {actionBusy ? "Removing…" : dependents.length > 0 || hasDuplicate ? "Remove anyway" : "Remove permanently"}
          </button>
        </div>
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
