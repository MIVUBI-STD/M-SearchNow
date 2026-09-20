<script lang="ts">
  import { onMount } from "svelte";
  import { RefreshCw } from "@lucide/svelte";
  import { runtimeProductFacade } from "./app/bridge/runtimeProductFacade";
  import { isAppRoute } from "./app/shared/navigation";
  import type { AppRoute, ProductRuntimeSnapshot } from "./app/shared/types";
  import Sidebar from "./components/layout/Sidebar.svelte";
  import ActivityDialog from "./components/ui/ActivityDialog.svelte";
  import PageState from "./components/ui/PageState.svelte";
  import StatusBadge from "./components/ui/StatusBadge.svelte";
  import Discover from "./pages/Discover.svelte";
  import Downloads from "./pages/Downloads.svelte";
  import Library from "./pages/Library.svelte";
  import Settings from "./pages/Settings.svelte";

  const ROUTE_STORAGE_KEY = "searchnow:last-route";

  let route = $state<AppRoute>("library");
  let booting = $state(true);
  let refreshing = $state(false);
  let snapshot = $state<ProductRuntimeSnapshot | null>(null);
  let activityOpen = $state(false);

  let health = $derived(snapshot?.backend?.diagnostics.health.state ?? "unknown");
  let runtimeLabel = $derived(
    booting
      ? "Checking"
      : !snapshot?.ready
        ? "Not ready"
        : health === "degraded"
          ? "Needs attention"
          : "Ready",
  );
  let runtimeTone = $derived<"ready" | "warning" | "muted">(
    booting ? "muted" : snapshot?.ready && health !== "degraded" ? "ready" : "warning",
  );

  async function refreshRuntime(): Promise<void> {
    if (refreshing) return;
    refreshing = true;
    snapshot = await runtimeProductFacade.loadProductRuntimeSnapshot();
    booting = false;
    refreshing = false;
  }

  function navigate(next: AppRoute): void {
    route = next;
    sessionStorage.setItem(ROUTE_STORAGE_KEY, next);
  }

  function handleAppShortcut(event: KeyboardEvent): void {
    const target = event.target;
    const editing = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement;

    if (event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey && event.key.toLowerCase() === "f") {
      if (route === "settings") return;
      const search = Array.from(document.querySelectorAll<HTMLInputElement>("[data-page-search]"))
        .find((element) => element.offsetParent !== null);
      if (!search) return;
      event.preventDefault();
      search.focus();
      search.select();
      return;
    }

    if (editing || !event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const shortcuts: Record<string, AppRoute> = {
      "1": "library",
      "2": "discover",
      "3": "downloads",
      "4": "settings",
    };
    const next = shortcuts[event.key];
    if (!next) return;
    event.preventDefault();
    navigate(next);
  }

  onMount(() => {
    const storedRoute = sessionStorage.getItem(ROUTE_STORAGE_KEY);
    if (isAppRoute(storedRoute)) route = storedRoute;
    void refreshRuntime();
  });
</script>

<svelte:window onkeydown={handleAppShortcut} />

<div class="app-shell" aria-busy={booting || refreshing}>
  <Sidebar {route} onNavigate={navigate} onOpenActivity={() => (activityOpen = true)} />

  <main class="app-main">
    {#if !booting && (refreshing || runtimeTone === "warning")}
      <header class="runtime-strip">
        <div class="runtime-strip__message">
          <span>{refreshing ? "Refreshing SearchNow status" : "SearchNow needs attention"}</span>
          <StatusBadge label={runtimeLabel} tone={runtimeTone} />
        </div>
        <button class="icon-button icon-button--quiet" type="button" title="Refresh app status" aria-label="Refresh app status" onclick={refreshRuntime} disabled={refreshing}>
          <RefreshCw size={15} class={refreshing ? "spin" : ""} aria-hidden="true" />
        </button>
      </header>
    {/if}

    <div class="content-frame">
      {#if booting}
        <section class="page">
          <PageState kind="loading" title="Starting SearchNow" message="Loading your Minecraft content." />
        </section>
      {:else}
        <Library runtimeReady={snapshot?.ready ?? false} active={route === "library"} />
        <Discover runtimeReady={snapshot?.ready ?? false} providers={snapshot?.backend?.providers ?? []} active={route === "discover"} />
        <Downloads runtimeReady={snapshot?.ready ?? false} active={route === "downloads"} />
        <Settings {snapshot} active={route === "settings"} />
      {/if}
    </div>
  </main>
</div>

<ActivityDialog
  open={activityOpen}
  runtimeReady={snapshot?.ready ?? false}
  onClose={() => (activityOpen = false)}
/>
