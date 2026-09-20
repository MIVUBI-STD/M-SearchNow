<script lang="ts">
  import { FileArchive, X } from "@lucide/svelte";
  import { focusFirstInDialog, trapDialogFocus } from "../../app/shared/dialogFocus";
  import type { LocalContentItem, MinecraftStorageRoot, PackageInspection, PackKind } from "../../app/shared/types";
  import TechnicalDetails from "./TechnicalDetails.svelte";

  let {
    inspection,
    open,
    roots,
    installedItems,
    onClose,
    onImport,
    onUpdate,
    onLocateMinecraft,
    importBusy = false,
    locationBusy = false,
  }: {
    inspection: PackageInspection | null;
    open: boolean;
    roots: MinecraftStorageRoot[];
    installedItems: LocalContentItem[];
    onClose: () => void;
    onImport: (rootId: string) => void;
    onUpdate: (rootId: string) => void;
    onLocateMinecraft: () => void;
    importBusy?: boolean;
    locationBusy?: boolean;
  } = $props();

  let selectedRootId = $state("");
  let modalBusy = $derived(importBusy || locationBusy);
  let dialogElement: HTMLElement | null = $state(null);

  $effect(() => {
    if (!open || !inspection) return;
    const focusToRestore = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    queueMicrotask(() => focusFirstInDialog(dialogElement));

    return () => {
      queueMicrotask(() => focusToRestore?.focus());
    };
  });

  $effect(() => {
    if (!open || roots.length === 0) return;
    if (!roots.some((root) => root.id === selectedRootId)) selectedRootId = roots[0].id;
  });

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === event.currentTarget && !modalBusy) onClose();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (!open) return;
    if (event.key === "Escape" && !modalBusy) {
      event.preventDefault();
      onClose();
      return;
    }
    trapDialogFocus(event, dialogElement);
  }

  function inputKindLabel(): string {
    if (!inspection) return "";
    if (inspection.inputKind === "mcPack") return ".mcpack";
    if (inspection.inputKind === "mcAddon") return ".mcaddon";
    if (inspection.inputKind === "mcWorld") return ".mcworld";
    return "Folder";
  }

  function packKindLabel(kind: PackKind): string {
    switch (kind) {
      case "behaviorPack": return "Behavior pack";
      case "resourcePack": return "Resource pack";
      case "skinPack": return "Skin pack";
      case "worldTemplate": return "World template";
      case "mixed": return "Mixed";
      default: return "Unknown";
    }
  }

  function conflictingItems(): LocalContentItem[] {
    if (!inspection || !selectedRootId) return [];
    const uuids = new Set(
      inspection.packs
        .map((pack) => pack.uuid?.toLowerCase())
        .filter((uuid): uuid is string => Boolean(uuid)),
    );
    return installedItems.filter(
      (item) =>
        item.rootId === selectedRootId &&
        item.manifestUuid !== null &&
        uuids.has(item.manifestUuid.toLowerCase()),
    );
  }

  function compareVersions(incoming: string | null, installed: number[]): number | null {
    if (!incoming || installed.length === 0) return null;
    const incomingParts = incoming.split(".").map((part) => Number(part));
    if (incomingParts.some((part) => !Number.isInteger(part) || part < 0)) return null;
    const length = Math.max(incomingParts.length, installed.length);
    for (let index = 0; index < length; index += 1) {
      const incomingPart = incomingParts[index] ?? 0;
      const installedPart = installed[index] ?? 0;
      if (incomingPart > installedPart) return 1;
      if (incomingPart < installedPart) return -1;
    }
    return 0;
  }

  function updateTarget(): LocalContentItem | null {
    if (!inspection || inspection.inputKind !== "mcPack" || inspection.packs.length !== 1) return null;
    const conflicts = conflictingItems();
    if (conflicts.length !== 1) return null;
    const comparison = compareVersions(inspection.packs[0].version, conflicts[0].version);
    return comparison === 1 ? conflicts[0] : null;
  }

  function conflictVersionState(): "newer" | "same" | "older" | "unknown" | null {
    if (!inspection || inspection.inputKind !== "mcPack" || inspection.packs.length !== 1) return null;
    const conflicts = conflictingItems();
    if (conflicts.length !== 1) return null;
    const comparison = compareVersions(inspection.packs[0].version, conflicts[0].version);
    if (comparison === null) return "unknown";
    if (comparison > 0) return "newer";
    if (comparison < 0) return "older";
    return "same";
  }

  function canAutoUpdateSingle(): boolean {
    if (!inspection || inspection.status !== "ready") return false;
    const target = updateTarget();
    if (!target) return false;
    const kind = inspection.packs[0].kind;
    return kind === "behaviorPack" || kind === "resourcePack" || kind === "skinPack";
  }

  function canAutoUpdateBundle(): boolean {
    if (
      !inspection ||
      inspection.status !== "ready" ||
      inspection.inputKind !== "mcAddon" ||
      inspection.packs.length < 2 ||
      !selectedRootId
    ) return false;

    const installedByUuid = new Map(
      installedItems
        .filter((item) => item.rootId === selectedRootId && item.manifestUuid !== null)
        .map((item) => [item.manifestUuid!.toLowerCase(), item] as const),
    );
    let hasChange = false;

    for (const pack of inspection.packs) {
      if (!pack.uuid) return false;
      if (pack.kind !== "behaviorPack" && pack.kind !== "resourcePack" && pack.kind !== "skinPack") return false;
      const installed = installedByUuid.get(pack.uuid.toLowerCase());
      if (!installed) {
        hasChange = true;
        continue;
      }
      const comparison = compareVersions(pack.version, installed.version);
      if (comparison === null || comparison < 0) return false;
      if (comparison > 0) hasChange = true;
    }
    return hasChange;
  }

  type PackageDecision =
    | "install"
    | "update"
    | "same"
    | "downgrade"
    | "conflict"
    | "review"
    | "rejected";

  function canAutoUpdate(): boolean {
    return canAutoUpdateSingle() || canAutoUpdateBundle();
  }

  function bundleVersionStates(): Array<{ name: string; state: "new" | "newer" | "same" | "older" | "unknown"; installed: LocalContentItem | null; incoming: string | null }> {
    if (!inspection || inspection.inputKind !== "mcAddon" || !selectedRootId) return [];
    const installedByUuid = new Map(
      installedItems
        .filter((item) => item.rootId === selectedRootId && item.manifestUuid !== null)
        .map((item) => [item.manifestUuid!.toLowerCase(), item] as const),
    );
    return inspection.packs.map((pack) => {
      const installed = pack.uuid ? (installedByUuid.get(pack.uuid.toLowerCase()) ?? null) : null;
      if (!installed) return { name: pack.name, state: "new" as const, installed: null, incoming: pack.version };
      const comparison = compareVersions(pack.version, installed.version);
      const state = comparison === null ? "unknown" : comparison > 0 ? "newer" : comparison < 0 ? "older" : "same";
      return { name: pack.name, state, installed, incoming: pack.version };
    });
  }

  function packageDecision(): PackageDecision {
    if (!inspection) return "review";
    if (inspection.status === "rejected") return "rejected";
    if (inspection.status === "issues") return "review";
    if (canAutoUpdate()) return "update";
    if (inspection.inputKind === "mcPack") {
      const state = conflictVersionState();
      if (state === "same") return "same";
      if (state === "older") return "downgrade";
      if (conflictingItems().length > 0) return "conflict";
    }
    if (inspection.inputKind === "mcAddon") {
      const states = bundleVersionStates();
      if (states.some((item) => item.state === "older")) return "downgrade";
      if (states.length > 0 && states.every((item) => item.state === "same")) return "same";
      if (conflictingItems().length > 0) return "conflict";
    }
    return canAutoImport() ? "install" : "review";
  }

  function decisionTitle(): string {
    switch (packageDecision()) {
      case "install": return "Ready to install";
      case "update": return inspection?.inputKind === "mcAddon" ? "Add-on update available" : "Update available";
      case "same": return "Same version already installed";
      case "downgrade": return "Older package detected";
      case "conflict": return "Installation conflict";
      case "rejected": return "Package rejected";
      default: return "Needs review";
    }
  }

  function decisionMessage(): string {
    switch (packageDecision()) {
      case "install":
        return "This content is not installed in the selected Minecraft storage and can be added safely.";
      case "update":
        return "SearchNow found a newer version and can replace the installed content using the safe update flow.";
      case "same":
        return "The selected package matches the installed version. No update is needed.";
      case "downgrade":
        return "A newer version is already installed. Automatic downgrade is disabled to avoid replacing newer content.";
      case "conflict":
        return "SearchNow found matching content but cannot determine one safe automatic action. Review the installed copies first.";
      case "rejected":
        return "This package failed safety or structure checks and cannot be installed.";
      default:
        return "SearchNow found something that needs attention before this package can be installed safely.";
    }
  }

  function canAutoImport(): boolean {
    if (!inspection || inspection.status !== "ready" || conflictingItems().length > 0) return false;
    if (inspection.inputKind === "mcWorld") return inspection.world !== null;
    return inspection.packs.every((pack) =>
      pack.kind === "behaviorPack" || pack.kind === "resourcePack" || pack.kind === "skinPack",
    );
  }

  function statusLabel(): string {
    return decisionTitle();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && inspection}
  <div class="catalog-modal__backdrop" role="presentation" onclick={handleBackdrop}>
    <div bind:this={dialogElement} class="catalog-modal" role="dialog" aria-modal="true" aria-labelledby="package-inspection-title" aria-busy={modalBusy} tabindex="-1">
      <button class="catalog-modal__close" type="button" aria-label="Close package inspection" onclick={onClose} disabled={modalBusy}>
        <X size={18} aria-hidden="true" />
      </button>

      <div class="catalog-modal__media catalog-modal__media--local">
        <FileArchive size={42} aria-hidden="true" />
        <span class="catalog-modal__type">{inputKindLabel()}</span>
      </div>

      <div class="catalog-modal__content">
        <div class="catalog-modal__heading">
          <div>
            <h2 id="package-inspection-title">Package inspection</h2>
            <p>{statusLabel()}</p>
          </div>
        </div>

        <div class="catalog-modal__decision" data-state={packageDecision()}>
          <strong>{decisionTitle()}</strong>
          <span>{decisionMessage()}</span>
        </div>

        <div class="catalog-modal__facts">
          <div><span>{inspection.inputKind === "mcWorld" ? "World" : "Packs"}</span><strong>{inspection.inputKind === "mcWorld" ? (inspection.world ? "1" : "0") : inspection.packs.length}</strong></div>
          <div><span>Issues</span><strong>{inspection.issues.length}</strong></div>
          {#if inspection.archive}<div><span>Files</span><strong>{inspection.archive.files}</strong></div>{/if}
        </div>

        {#if roots.length === 0}
          <div class="catalog-modal__issue">
            <strong>Minecraft storage required</strong>
            <span>Locate your Minecraft data folder before importing or updating this package.</span>
          </div>
        {/if}

        {#if inspection.world}
          <div class="catalog-modal__issue">
            <strong>Detected world</strong>
            <span>{inspection.world.name}</span>
          </div>
        {/if}

        {#if inspection.packs.length}
          <div class="catalog-modal__issue">
            <strong>Detected content</strong>
            <span>{inspection.packs.map((pack) => `${pack.name} · ${packKindLabel(pack.kind)}`).join(" · ")}</span>
          </div>
        {/if}

        {#if inspection.issues.length}
          <div class="catalog-modal__issue">
            <strong>{inspection.status === "rejected" ? "Package rejected" : "Review findings"}</strong>
            <span>{inspection.issues[0].message}{inspection.issues.length > 1 ? ` (+${inspection.issues.length - 1} more)` : ""}</span>
          </div>
        {/if}

        {#if inspection.inputKind === "mcPack" && conflictingItems().length}
          <div class="catalog-modal__issue">
            <strong>Installed version</strong>
            {#if canAutoUpdate() && updateTarget()}
              <span>{updateTarget()!.title}: v{updateTarget()!.version.join(".")} → v{inspection.packs[0].version}</span>
            {:else if conflictVersionState() === "same"}
              <span>{conflictingItems()[0].title} is already installed at v{conflictingItems()[0].version.join(".")}.</span>
            {:else if conflictVersionState() === "older"}
              <span>Installed v{conflictingItems()[0].version.join(".")} is newer than incoming v{inspection.packs[0].version}.</span>
            {:else}
              <span>{conflictingItems().map((item) => item.title).join(", ")} uses the same manifest UUID in this Minecraft storage.</span>
            {/if}
          </div>
        {/if}

        {#if inspection.inputKind === "mcAddon" && bundleVersionStates().length}
          <div class="catalog-modal__changes">
            <strong>Bundle changes</strong>
            <div>
              {#each bundleVersionStates() as change}
                <div class="catalog-modal__change-row" data-state={change.state}>
                  <span>{change.name}</span>
                  <small>
                    {change.state === "new"
                      ? "New install"
                      : change.state === "newer"
                        ? `v${change.installed?.version.join(".")} → v${change.incoming}`
                        : change.state === "same"
                          ? `Already v${change.incoming}`
                          : change.state === "older"
                            ? `Installed v${change.installed?.version.join(".")} is newer`
                            : "Version could not be compared"}
                  </small>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if roots.length > 1}
          <label>
            <span>Import target</span>
            <select class="select-field" bind:value={selectedRootId} disabled={modalBusy}>
              {#each roots as root}
                <option value={root.id}>{root.accountHint ?? root.storageKind} · {root.root}</option>
              {/each}
            </select>
          </label>
        {/if}

        <TechnicalDetails
          items={[
            { label: "Source", value: inspection.sourcePath },
            { label: "Safety", value: inspection.safety },
            { label: "Relationships", value: String(inspection.relationships.length) },
            { label: "Compressed bytes", value: inspection.archive ? String(inspection.archive.compressedBytes) : null },
            { label: "Uncompressed bytes", value: inspection.archive ? String(inspection.archive.uncompressedBytes) : null },
          ]}
        />

        <div class="catalog-modal__footer">
          {#if roots.length === 0}
            <button
              class="button button--primary"
              type="button"
              disabled={modalBusy}
              onclick={onLocateMinecraft}
            >
              {locationBusy ? "Locating…" : "Locate Minecraft"}
            </button>
          {:else if canAutoUpdate()}
            <button
              class="button button--primary"
              type="button"
              disabled={modalBusy}
              onclick={() => onUpdate(selectedRootId)}
            >
              {importBusy ? "Updating…" : inspection.inputKind === "mcAddon" ? "Update add-on bundle" : "Update installed pack"}
            </button>
          {:else}
            <button
              class="button button--primary"
              type="button"
              disabled={!canAutoImport() || modalBusy}
              onclick={() => onImport(selectedRootId)}
            >
              {importBusy ? "Installing…" : "Install package"}
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}
