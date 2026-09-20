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
const diagnosticsPanel = await read("src/components/settings/DiagnosticsPanel.svelte");
const libraryDetail = await read("src/components/library/LibraryDetailPanel.svelte");
const downloadView = await read("src/app/shared/downloadView.ts");
const sharedTypes = await read("src/app/shared/types.ts");
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
if (!packageModal.includes('aria-busy={modalBusy}')) {
  errors.push("Package inspection modal must expose combined import/location busy state");
}
for (const needle of ["Locate Minecraft", "Minecraft storage required", "onLocateMinecraft"]) {
  if (!packageModal.includes(needle)) errors.push(`Package inspection recovery contract is missing: ${needle}`);
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
  "recoverExportDirectory",
  "Connect SearchNow to your Minecraft data folder.",
  'snapshot.library.items.length > 0',
  "Minecraft location required",
  "library-setup-state",
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

const windowConfig = tauriConfig?.app?.windows?.[0] ?? {};
if (windowConfig.theme !== "Dark") {
  errors.push("Main Tauri window must use the dark native theme");
}
const minWidth = Number(windowConfig.minWidth ?? 0);
const responsiveWidths = [...workspace.matchAll(/@media \(max-width: (\d+)px\)/g)]
  .map((match) => Number(match[1]));
if (!responsiveWidths.some((width) => width >= minWidth)) {
  errors.push(`Desktop responsive rules are unreachable at configured minWidth ${minWidth}px`);
}

if (!workspace.includes(".library-setup-state .empty-panel")) {
  errors.push("Library Minecraft recovery must use the focused setup-state layout");
}

if (!workspace.includes(".toolbar--library-multi-root")) {
  errors.push("Library multi-root toolbar requires an explicit five-control grid owner");
}
for (const needle of ["queryInput", "setTimeout(() =>", "query = value", "160)"]) {
  if (!library.includes(needle)) errors.push(`Library scale contract is missing: ${needle}`);
}
for (const needle of ["content-visibility: auto", "contain-intrinsic-size: 64px", "contain-intrinsic-size: 78px"]) {
  if (!workspace.includes(needle)) errors.push(`Large-list rendering contract is missing: ${needle}`);
}

for (const needle of [
  "DownloadFeedback",
  "Queue order could not be changed",
  "Download folder could not be opened",
  "downloadStageDetail",
  "downloadRecoveryHint",
  "Download can be retried",
]) {
  if (!downloads.includes(needle)) errors.push(`Downloads feedback contract is missing: ${needle}`);
}
for (const needle of ["downloadStageDetail", "downloadRecoveryHint", "Verifying and saving the completed file safely."]) {
  if (!downloadView.includes(needle)) errors.push(`Download stage contract is missing: ${needle}`);
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

for (const needle of [
  "DiscoverFeedback",
  "Download folder could not be chosen",
  "Download could not start",
  "Preview only",
  "Available",
]) {
  if (!discover.includes(needle)) errors.push(`Discover feedback contract is missing: ${needle}`);
}
for (const needle of [
  "Ready to download",
  "Download file unavailable",
  "Download source unavailable",
  "safe output filename",
]) {
  if (!catalogModal.includes(needle)) errors.push(`Catalog availability contract is missing: ${needle}`);
}
if (discover.includes("downloadMessage")) {
  errors.push("Discover must use structured download feedback instead of downloadMessage");
}

for (const needle of ["Export report", "exportDiagnosticsReport", "Diagnostics exported", "Recent activity", "This list resets when the app restarts."]) {
  if (!diagnosticsPanel.includes(needle)) errors.push(`Diagnostics support workflow is missing: ${needle}`);
}
if (!sharedTypes.includes('| "package"')) {
  errors.push("Frontend diagnostic component contract is missing package events");
}
for (const needle of ["Required by", "Remove anyway", "Duplicate installation", "Dependency version too old"]) {
  if (!libraryDetail.includes(needle)) errors.push(`Library relationship/safety contract is missing: ${needle}`);
}
for (const needle of ["Ready to install", "Same version already installed", "Older package detected", "Installation conflict", "Bundle changes"]) {
  if (!packageModal.includes(needle)) errors.push(`Package decision contract is missing: ${needle}`);
}
if (!runtimeApi.includes("export_diagnostics_report")) {
  errors.push("runtimeApi is missing export_diagnostics_report");
}
if (!registry.includes("export_diagnostics_report")) {
  errors.push("Tauri registry is missing export_diagnostics_report");
}
if (!facade.includes("exportDiagnosticsReport")) {
  errors.push("runtimeProductFacade is missing diagnostics report export");
}

if (errors.length) {
  for (const error of errors) console.error(`ERROR: ${error}`);
  process.exit(1);
}
console.log("SearchNow UI contract: PASS");
