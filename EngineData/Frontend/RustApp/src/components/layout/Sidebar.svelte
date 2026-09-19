<script lang="ts">
  import { Download, Library, Search, Settings } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { APP_ROUTES } from "../../app/shared/navigation";
  import type { AppRoute } from "../../app/shared/types";

  let {
    route,
    onNavigate,
  }: {
    route: AppRoute;
    onNavigate: (route: AppRoute) => void;
  } = $props();

  const icons: Record<AppRoute, Component> = {
    library: Library,
    discover: Search,
    downloads: Download,
    settings: Settings,
  };

  const primaryRoutes = APP_ROUTES.filter((item) => item.id !== "settings");
  const settingsRoute = APP_ROUTES.find((item) => item.id === "settings");
</script>

<aside class="sidebar">
  <div class="brand">
    <div class="brand__mark" aria-hidden="true">S</div>
    <strong>SearchNow</strong>
  </div>

  <nav class="sidebar__nav" aria-label="Primary navigation">
    {#each primaryRoutes as item}
      {@const Icon = icons[item.id]}
      <button
        class:nav-item--active={route === item.id}
        class="nav-item"
        type="button"
        aria-current={route === item.id ? "page" : undefined}
        title={item.label}
        onclick={() => onNavigate(item.id)}
      >
        <Icon size={18} strokeWidth={1.8} aria-hidden="true" />
        <span>{item.label}</span>
      </button>
    {/each}
  </nav>

  <div class="sidebar__spacer"></div>

  {#if settingsRoute}
    {@const SettingsIcon = icons[settingsRoute.id]}
    <nav class="sidebar__utility" aria-label="Application">
      <button
        class:nav-item--active={route === settingsRoute.id}
        class="nav-item"
        type="button"
        aria-current={route === settingsRoute.id ? "page" : undefined}
        title={settingsRoute.label}
        onclick={() => onNavigate(settingsRoute.id)}
      >
        <SettingsIcon size={18} strokeWidth={1.8} aria-hidden="true" />
        <span>{settingsRoute.label}</span>
      </button>
    </nav>
  {/if}
</aside>
