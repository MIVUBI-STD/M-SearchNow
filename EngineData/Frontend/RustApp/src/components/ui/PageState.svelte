<script lang="ts">
  import { AlertTriangle, RefreshCw } from "@lucide/svelte";

  type PageStateKind = "empty" | "loading" | "error";

  let {
    title,
    message,
    kind = "empty",
    marker = "—",
    actionLabel = null,
    actionDisabled = false,
    onAction = null,
  }: {
    title: string;
    message: string;
    kind?: PageStateKind;
    marker?: string;
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
