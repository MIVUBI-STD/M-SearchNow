<script lang="ts">
  import { FileArchive, X } from "@lucide/svelte";
  import type { MinecraftStorageRoot, PackageInspection, PackKind } from "../../app/shared/types";
  import TechnicalDetails from "./TechnicalDetails.svelte";

  let {
    inspection,
    open,
    roots,
    onClose,
    onImport,
    importBusy = false,
  }: {
    inspection: PackageInspection | null;
    open: boolean;
    roots: MinecraftStorageRoot[];
    onClose: () => void;
    onImport: (rootId: string) => void;
    importBusy?: boolean;
  } = $props();

  let selectedRootId = $state("");

  $effect(() => {
    if (!open || roots.length === 0) return;
    if (!roots.some((root) => root.id === selectedRootId)) selectedRootId = roots[0].id;
  });

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === event.currentTarget && !importBusy) onClose();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (open && event.key === "Escape" && !importBusy) onClose();
  }

  function inputKindLabel(): string {
    if (!inspection) return "";
    if (inspection.inputKind === "mcPack") return ".mcpack";
    if (inspection.inputKind === "mcAddon") return ".mcaddon";
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

  function canAutoImport(): boolean {
    if (!inspection || inspection.status !== "ready") return false;
    return inspection.packs.every((pack) =>
      pack.kind === "behaviorPack" || pack.kind === "resourcePack" || pack.kind === "skinPack",
    );
  }

  function statusLabel(): string {
    if (!inspection) return "";
    if (inspection.status === "rejected") return "Rejected";
    if (inspection.status === "issues") return "Needs review";
    return canAutoImport() ? "Ready to import" : "Inspection passed";
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && inspection}
  <div class="catalog-modal__backdrop" role="presentation" onclick={handleBackdrop}>
    <div class="catalog-modal" role="dialog" aria-modal="true" aria-labelledby="package-inspection-title">
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
          <div><span>Packs</span><strong>{inspection.packs.length}</strong></div>
          <div><span>Issues</span><strong>{inspection.issues.length}</strong></div>
          {#if inspection.archive}<div><span>Files</span><strong>{inspection.archive.files}</strong></div>{/if}
        </div>

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
          <button
            class="button button--primary"
            type="button"
            disabled={!canAutoImport() || roots.length === 0 || importBusy}
            onclick={() => onImport(selectedRootId)}
          >
            {importBusy ? "Importing…" : "Import package"}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
