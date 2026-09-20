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
    importBusy = false,
  }: {
    inspection: PackageInspection | null;
    open: boolean;
    roots: MinecraftStorageRoot[];
    installedItems: LocalContentItem[];
    onClose: () => void;
    onImport: (rootId: string) => void;
    onUpdate: (rootId: string) => void;
    importBusy?: boolean;
  } = $props();

  let selectedRootId = $state("");
  let dialogElement: HTMLElement | null = $state(null);
  let previousFocus: HTMLElement | null = null;

  $effect(() => {
    if (!open || !inspection) return;
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    queueMicrotask(() => focusFirstInDialog(dialogElement));

    return () => {
      queueMicrotask(() => previousFocus?.focus());
    };
  });

  $effect(() => {
    if (!open || roots.length === 0) return;
    if (!roots.some((root) => root.id === selectedRootId)) selectedRootId = roots[0].id;
  });

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === event.currentTarget && !importBusy) onClose();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (!open) return;
    if (event.key === "Escape" && !importBusy) {
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

  function canAutoUpdate(): boolean {
    return canAutoUpdateSingle() || canAutoUpdateBundle();
  }

  function canAutoImport(): boolean {
    if (!inspection || inspection.status !== "ready" || conflictingItems().length > 0) return false;
    if (inspection.inputKind === "mcWorld") return inspection.world !== null;
    return inspection.packs.every((pack) =>
      pack.kind === "behaviorPack" || pack.kind === "resourcePack" || pack.kind === "skinPack",
    );
  }

  function statusLabel(): string {
    if (!inspection) return "";
    if (inspection.status === "rejected") return "Rejected";
    if (inspection.status === "issues") return "Needs review";
    if (canAutoUpdateBundle()) return "Bundle update available";
    if (canAutoUpdateSingle()) return "Update available";
    if (conflictVersionState() === "older") return "Older package";
    if (conflictingItems().length > 0) return "Already installed";
    return canAutoImport() ? "Ready to import" : "Inspection passed";
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && inspection}
  <div class="catalog-modal__backdrop" role="presentation" onclick={handleBackdrop}>
    <div bind:this={dialogElement} class="catalog-modal" role="dialog" aria-modal="true" aria-labelledby="package-inspection-title" tabindex="-1">
      <button class="catalog-modal__close" type="button" aria-label="Close package inspection" onclick={onClose} disabled={importBusy}>
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

        <div class="catalog-modal__facts">
          <div><span>{inspection.inputKind === "mcWorld" ? "World" : "Packs"}</span><strong>{inspection.inputKind === "mcWorld" ? (inspection.world ? "1" : "0") : inspection.packs.length}</strong></div>
          <div><span>Issues</span><strong>{inspection.issues.length}</strong></div>
          {#if inspection.archive}<div><span>Files</span><strong>{inspection.archive.files}</strong></div>{/if}
        </div>

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

        {#if conflictingItems().length}
          <div class="catalog-modal__issue">
            <strong>{canAutoUpdate() ? "Update available" : conflictVersionState() === "older" ? "Older package" : "Already installed"}</strong>
            {#if canAutoUpdate() && updateTarget()}
              <span>{updateTarget()!.title}: v{updateTarget()!.version.join(".")} → v{inspection.packs[0].version}</span>
            {:else if conflictVersionState() === "older"}
              <span>The installed version is newer than v{inspection.packs[0].version}; automatic downgrade is disabled.</span>
            {:else}
              <span>{conflictingItems().map((item) => item.title).join(", ")} uses the same manifest UUID in this Minecraft storage.</span>
            {/if}
          </div>
        {/if}

        {#if roots.length > 1}
          <label>
            <span>Import target</span>
            <select class="select-field" bind:value={selectedRootId} disabled={importBusy}>
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
          {#if canAutoUpdate()}
            <button
              class="button button--primary"
              type="button"
              disabled={roots.length === 0 || importBusy}
              onclick={() => onUpdate(selectedRootId)}
            >
              {importBusy ? "Updating…" : inspection.inputKind === "mcAddon" ? "Update add-on bundle" : "Update installed pack"}
            </button>
          {:else}
            <button
              class="button button--primary"
              type="button"
              disabled={!canAutoImport() || roots.length === 0 || importBusy}
              onclick={() => onImport(selectedRootId)}
            >
              {importBusy ? "Importing…" : "Import package"}
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}
