import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const read = (path) => readFile(resolve(appRoot, path), "utf8");
const errors = [];

const library = await read("src/pages/Library.svelte");
const settings = await read("src/pages/Settings.svelte");
const downloads = await read("src/pages/Downloads.svelte");
const discover = await read("src/pages/Discover.svelte");
const workspace = await read("src/styles/workspace.css");
const catalogModal = await read("src/components/ui/CatalogDetailModal.svelte");
const packageModal = await read("src/components/ui/PackageInspectionModal.svelte");
const dialogFocus = await read("src/app/shared/dialogFocus.ts");
const runtimeApi = await read("src/app/bridge/runtimeApi.ts");
const facade = await read("src/app/bridge/runtimeProductFacade.ts");
const registry = await read("src-tauri/src/commands/registry.rs");
const tauriConfig = JSON.parse(await read("src-tauri/tauri.conf.json"));

for (const [label, text] of [
  ["Catalog detail modal", catalogModal],
  ["Package inspection modal", packageModal],
]) {
  for (const needle of ["trapDialogFocus", "bind:this={dialogElement}", 'tabindex="-1"']) {
    if (!text.includes(needle)) errors.push(`${label} is missing dialog focus contract: ${needle}`);
  }
}
if (!dialogFocus.includes("active === last || !container.contains(active)")) {
  errors.push("Dialog focus trap must recover forward Tab when focus escapes the dialog");
}
for (const needle of ['aria-busy={downloadBusy}', 'disabled={downloadBusy}']) {
  if (!catalogModal.includes(needle)) errors.push(`Catalog modal busy-state contract is missing: ${needle}`);
}
if (!packageModal.includes('aria-busy={importBusy}')) {
  errors.push("Package inspection modal must expose import busy state");
}

for (const needle of [
  "toolbar--library-multi-root",
  "LibraryFeedback",
  "Locate Minecraft",
  "locateMinecraftRoot",
  "library-workspace--selection",
  "Drop to inspect",
  "subscribeDesktopDrops",
  "inspectDroppedPackage",
]) {
  if (!library.includes(needle)) errors.push(`Library UX contract is missing: ${needle}`);
}

for (const needle of [
  "chooseMinecraftDirectory",
  'id="settings-privacy"',
  "Local-first by default",
  "Use automatic detection",
  "chooseExportDirectory",
  "defaultExportDirectory",
  "exportDuplicatePolicy",
  "Keep both with a new file name",
]) {
  if (!settings.includes(needle)) errors.push(`Settings UX contract is missing: ${needle}`);
}

if (!runtimeApi.includes("choose_minecraft_directory")) {
  errors.push("runtimeApi is missing choose_minecraft_directory");
}
if (!runtimeApi.includes("choose_export_directory")) {
  errors.push("runtimeApi is missing choose_export_directory");
}
if (!runtimeApi.includes("inspect_package_path") || !runtimeApi.includes("onDragDropEvent")) {
  errors.push("runtimeApi is missing the package drag/drop boundary");
}
if (!registry.includes("choose_minecraft_directory")) {
  errors.push("Tauri registry is missing choose_minecraft_directory");
}
if (!registry.includes("choose_export_directory")) {
  errors.push("Tauri registry is missing choose_export_directory");
}
if (!registry.includes("inspect_package_path")) {
  errors.push("Tauri registry is missing inspect_package_path");
}
if (!facade.includes("chooseMinecraftDirectory")) {
  errors.push("runtimeProductFacade is missing Minecraft directory picker");
}
if (!facade.includes("chooseExportDirectory")) {
  errors.push("runtimeProductFacade is missing export directory picker");
}
if (!facade.includes("inspectPackagePath") || !facade.includes("subscribeDesktopDrops")) {
  errors.push("runtimeProductFacade is missing package drag/drop support");
}

const minWidth = Number(tauriConfig?.app?.windows?.[0]?.minWidth ?? 0);
const responsiveWidths = [...workspace.matchAll(/@media \(max-width: (\d+)px\)/g)]
  .map((match) => Number(match[1]));
if (!responsiveWidths.some((width) => width >= minWidth)) {
  errors.push(`Desktop responsive rules are unreachable at configured minWidth ${minWidth}px`);
}

if (!workspace.includes(".toolbar--library-multi-root")) {
  errors.push("Library multi-root toolbar requires an explicit five-control grid owner");
}

for (const needle of ["DownloadFeedback", "Queue order could not be changed", "Download folder could not be opened"]) {
  if (!downloads.includes(needle)) errors.push(`Downloads feedback contract is missing: ${needle}`);
}
if (downloads.includes('title="Something went wrong"')) {
  errors.push("Downloads must not fall back to the generic Something went wrong notice");
}

for (const needle of ["SettingsFeedback", "minecraftDirty", "Settings could not be saved", "Your download and export preferences are now active."]) {
  if (!settings.includes(needle)) errors.push(`Settings feedback contract is missing: ${needle}`);
}
if (settings.includes('title="Could not update settings."')) {
  errors.push("Settings must not use the generic Could not update settings notice");
}

for (const needle of ["DiscoverFeedback", "Download folder could not be chosen", "Download could not start"]) {
  if (!discover.includes(needle)) errors.push(`Discover feedback contract is missing: ${needle}`);
}
if (discover.includes("downloadMessage")) {
  errors.push("Discover must use structured download feedback instead of downloadMessage");
}

if (errors.length) {
  for (const error of errors) console.error(`ERROR: ${error}`);
  process.exit(1);
}
console.log("SearchNow UI contract: PASS");
