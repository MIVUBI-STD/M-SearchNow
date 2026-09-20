import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const read = (path) => readFile(resolve(appRoot, path), "utf8");
const errors = [];

const app = await read("src/App.svelte");
const library = await read("src/pages/Library.svelte");
const settings = await read("src/pages/Settings.svelte");
const downloads = await read("src/pages/Downloads.svelte");
const discover = await read("src/pages/Discover.svelte");
const diagnosticsPanel = await read("src/components/settings/DiagnosticsPanel.svelte");
const activityDialog = await read("src/components/ui/ActivityDialog.svelte");
const pageState = await read("src/components/ui/PageState.svelte");
const sidebar = await read("src/components/layout/Sidebar.svelte");
const libraryDetail = await read("src/components/library/LibraryDetailPanel.svelte");
const downloadView = await read("src/app/shared/downloadView.ts");
const sharedTypes = await read("src/app/shared/types.ts");
const tokens = await read("src/styles/tokens.css");
const workspace = await read("src/styles/workspace.css");
const presentation = await read("src/styles/presentation.css");
const appStyles = await read("src/styles/app.css");
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
for (const needle of ["handleAppShortcut", '"1": "library"', '"4": "settings"', "event.altKey", "[data-page-search]"]) {
  if (!app.includes(needle)) {
    errors.push(`Desktop keyboard navigation contract is missing: ${needle}`);
  }
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
if (Number(windowConfig.minWidth ?? 0) > 960 || Number(windowConfig.minHeight ?? 0) > 640) {
  errors.push("Main Tauri window minimum size is too large for DPI-scaled Windows desktops");
}
const minWidth = Number(windowConfig.minWidth ?? 0);
const responsiveWidths = [...workspace.matchAll(/@media \(max-width: (\d+)px\)/g)]
  .map((match) => Number(match[1]));
if (!responsiveWidths.some((width) => width >= minWidth)) {
  errors.push(`Desktop responsive rules are unreachable at configured minWidth ${minWidth}px`);
}

if (!appStyles.includes("min-height: clamp(240px, 34vh, 320px)")) {
  errors.push("Empty states must own enough vertical space to avoid floating at the top of the window");
}
if (!workspace.includes(".library-setup-state .empty-panel")) {
  errors.push("Library Minecraft recovery must use the focused setup-state layout");
}
if (!pageState.includes('type PageStateIcon') || !pageState.includes('icon?: PageStateIcon | null')) {
  errors.push("PageState must support semantic empty-state icons");
}
for (const [label, text, marker] of [
  ["Library", library, 'marker="01"'],
  ["Discover", discover, 'marker="02"'],
  ["Downloads", downloads, 'marker="03"'],
]) {
  if (text.includes(marker)) errors.push(`${label} must not use numeric empty-state markers`);
}
const headingActionBlock = workspace.slice(
  workspace.indexOf(".page-heading__actions .button"),
  workspace.indexOf(".runtime-strip"),
);
if (/background\s*:/.test(headingActionBlock)) {
  errors.push("Heading action container must not override semantic button backgrounds");
}

if (!workspace.includes(".toolbar--library-multi-root")) {
  errors.push("Library multi-root toolbar requires an explicit five-control grid owner");
}
for (const needle of ["--sn-font-body", "--sn-font-caption", "--sn-motion-fast", "--sn-surface-workspace"]) {
  if (!tokens.includes(needle)) errors.push(`Design token spine is missing: ${needle}`);
}
for (const forbidden of [
  "--sn-content-width: 1360px",
  "--sn-sidebar-width: 188px",
  "--sn-page-padding-x: clamp(28px, 3vw, 48px)",
  "--sn-page-padding-y: 28px",
]) {
  if (workspace.includes(forbidden)) errors.push(`Workspace CSS duplicates a global design token: ${forbidden}`);
}
for (const needle of ["@media (max-width: 1040px)", "grid-template-columns: 1fr", "repeat(2, minmax(0, 1fr))"]) {
  if (!workspace.includes(needle)) errors.push(`Windows scaled-layout contract is missing: ${needle}`);
}
if (!workspace.includes("border-left: 0")) {
  errors.push("Library detail must collapse below the list on narrow Windows viewports");
}
for (const needle of ["queryInput", "setTimeout(() =>", "query = value", "160)"]) {
  if (!library.includes(needle)) errors.push(`Library scale contract is missing: ${needle}`);
}
for (const needle of ["content-visibility: auto", "contain-intrinsic-size: 64px", "contain-intrinsic-size: 78px"]) {
  if (!workspace.includes(needle)) errors.push(`Large-list rendering contract is missing: ${needle}`);
}
if (!appStyles.includes("scrollbar-gutter: stable")) {
  errors.push("Windows scroll layout must reserve stable scrollbar space");
}
if (!presentation.includes("@media (forced-colors: active)") || !presentation.includes("outline: 2px solid Highlight")) {
  errors.push("Windows high-contrast focus contract is missing");
}

for (const needle of ["data-page-search", 'aria-keyshortcuts="Control+F"']) {
  if (!library.includes(needle) || !discover.includes(needle) || !downloads.includes(needle)) {
    errors.push(`Search shortcut contract is missing on one or more searchable surfaces: ${needle}`);
  }
}
for (const needle of ["handleLibraryShortcut", "handleRowKeydown", "focusLibraryItem", 'event.key === "Escape"', 'event.key.toLowerCase() === "a"', '"ArrowDown"', '"Home"', "data-library-id"]) {
  if (!library.includes(needle)) errors.push(`Library keyboard selection contract is missing: ${needle}`);
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

for (const needle of [
  "SettingsFeedback",
  "minecraftDirty",
  "Settings could not be saved",
  "Your download and export preferences are now active.",
  'class="settings-save-state"',
  'role="status"',
  'class="button button--primary"',
  'saving ? "Saving" : "Save changes"',
]) {
  if (!settings.includes(needle)) errors.push(`Settings feedback contract is missing: ${needle}`);
}
if (settings.includes('title="Could not update settings."')) {
  errors.push("Settings must not use the generic Could not update settings notice");
}

if (discover.includes("Source unavailable") || discover.includes('class="search-shell" aria-disabled="true"')) {
  errors.push("Discover must not render a fake search control when no content source exists");
}
for (const needle of ["No content source available", "does not have a catalog provider enabled yet"]) {
  if (!discover.includes(needle)) errors.push(`Discover unavailable-state copy contract is missing: ${needle}`);
}
if (!library.includes('"Import content"') || !library.includes("inspectPackage")) {
  errors.push("Empty Library must offer a real import recovery action");
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
if (!presentation.includes("transform: scale(.98)") || !catalogModal.includes("catalog-modal__backdrop")) {
  errors.push("Restrained interaction polish contract is missing");
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

for (const needle of ["Export report", "exportDiagnosticsReport", "Diagnostics exported"]) {
  if (!diagnosticsPanel.includes(needle)) errors.push(`Diagnostics support workflow is missing: ${needle}`);
}
for (const needle of ["Recent activity", "session-only", "Activity unavailable"]) {
  if (!activityDialog.includes(needle)) errors.push(`Activity workflow is missing: ${needle}`);
}
if (!sidebar.includes("Recent activity") || !sidebar.includes("onOpenActivity") || !sidebar.includes('aria-haspopup="dialog"') || !sidebar.includes("<Blocks")) {
  errors.push("Sidebar must expose Recent Activity as a dialog utility without adding another product route");
}
for (const needle of ['aria-keyshortcuts=', 'aria-current=']) {
  if (!sidebar.includes(needle)) errors.push(`Sidebar keyboard/accessibility contract is missing: ${needle}`);
}
if (!sharedTypes.includes('| "package"')) {
  errors.push("Frontend diagnostic component contract is missing package events");
}
for (const needle of ["Required by", "Remove anyway", "Duplicate installation", "Dependency version too old", "library-detail__health-list"]) {
  if (!libraryDetail.includes(needle)) errors.push(`Library relationship/safety contract is missing: ${needle}`);
}
if (library.includes('item.status === "ready" ? "Ready" : "Needs review"') || library.includes('? "Needs review" : "Healthy"')) {
  errors.push("Library rows must keep healthy state visually quiet");
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
