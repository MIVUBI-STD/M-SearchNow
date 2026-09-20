<script lang="ts">
  import { AlertTriangle, Archive, Download, FolderOpen, RefreshCw, Search } from "@lucide/svelte";

  type PageStateKind = "empty" | "loading" | "error";
  type PageStateIcon = "library" | "search" | "download" | "folder";

  let {
    title,
    message,
    kind = "empty",
    marker = "—",
    icon = null,
    actionLabel = null,
    actionDisabled = false,
    onAction = null,
  }: {
    title: string;
    message: string;
    kind?: PageStateKind;
    marker?: string;
    icon?: PageStateIcon | null;
    actionLabel?: string | null;
    actionDisabled?: boolean;
    onAction?: (() => void) | null;
  } = $props();
</script>

<article
  class:empty-panel--error={kind === "error"}
  class:empty-panel--loading={kind === "loading"}
  class="empty-panel"
  role={kind === "error" ? "alert" : kind === "loading" ? "status" : undefined}
  aria-live={kind === "loading" ? "polite" : undefined}
  aria-atomic={kind === "loading" ? "true" : undefined}
  aria-busy={kind === "loading"}
>
  <div class="empty-panel__icon" aria-hidden="true">
    {#if kind === "loading"}
      <RefreshCw size={18} class="spin" />
    {:else if kind === "error"}
      <AlertTriangle size={18} />
    {:else if icon === "library"}
      <Archive size={18} />
    {:else if icon === "search"}
      <Search size={18} />
    {:else if icon === "download"}
      <Download size={18} />
    {:else if icon === "folder"}
      <FolderOpen size={18} />
    {:else}
      {marker}
    {/if}
  </div>
  <div>
    <h2>{title}</h2>
    <p>{message}</p>
    {#if actionLabel && onAction}
      <button class="button button--secondary button--compact" type="button" onclick={onAction} disabled={actionDisabled}>
        {actionLabel}
      </button>
    {/if}
  </div>
</article>
