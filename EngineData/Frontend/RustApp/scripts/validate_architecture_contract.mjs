import { access, readdir, readFile } from "node:fs/promises";
import { extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const backendRoot = resolve(appRoot, "../../Backend/RustCore");
const required = [
  "src/App.svelte",
  "src/app/bridge/runtimeApi.ts",
  "src/app/bridge/runtimeProductFacade.ts",
  "src/app/workflows/catalogDownload.ts",
  "src/app/state/applicationChanges.ts",
  "src/app/state/applicationEvents.ts",
  "src/app/shared/format.ts",
  "src/pages/Library.svelte",
  "src/pages/Discover.svelte",
  "src/pages/Downloads.svelte",
  "src/pages/Settings.svelte",
  "src-tauri/src/main.rs",
  "src-tauri/src/app_bootstrap.rs",
  "src-tauri/src/commands/registry.rs",
  "src-tauri/src/commands/runtime.rs",
  "src-tauri/src/commands/settings.rs",
  "src-tauri/src/commands/minecraft.rs",
  "src-tauri/src/commands/library.rs",
  "src-tauri/src/commands/catalog.rs",
  "src-tauri/src/commands/download.rs",
];
const backendRequired = [
  "Cargo.toml",
  "src/lib.rs",
  "src/app_runtime.rs",
  "src/application_content.rs",
  "src/application_download.rs",
  "src/application_library.rs",
  "src/application_package.rs",
  "src/identity.rs",
  "src/settings.rs",
  "src/minecraft.rs",
  "src/library.rs",
  "src/package/mod.rs",
  "src/package/model.rs",
  "src/package/manifest.rs",
  "src/package/archive.rs",
  "src/package/folder.rs",
  "src/download/mod.rs",
  "src/download/model.rs",
  "src/download/manager.rs",
  "src/download/store.rs",
  "src/download/workspace.rs",
  "src/download/transport.rs",
  "src/download/executor.rs",
  "src/download/http.rs",
  "src/download/resolver.rs",
  "src/catalog/mod.rs",
  "src/catalog/model.rs",
  "src/catalog/provider.rs",
  "src/provider_session/mod.rs",
  "src/provider_session/model.rs",
  "src/provider_session/runtime.rs",
  "src/provider_adapter/mod.rs",
  "src/provider_adapter/model.rs",
  "src/provider_adapter/runtime.rs",
];
const errors = [];

// Executable source-architecture and IPC boundary invariants live here.
// Repository layout, deterministic CI, and promotion/release hygiene live
// in tools/verify_repository.py. Do not duplicate the same invariant in both.

for (const path of required) {
  try { await access(resolve(appRoot, path)); } catch { errors.push(`missing required architecture path: ${path}`); }
}
for (const path of backendRequired) {
  try { await access(resolve(backendRoot, path)); } catch { errors.push(`missing backend core path: EngineData/Backend/RustCore/${path}`); }
}

async function collect(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...await collect(path));
    else if (entry.isFile() && [".ts", ".svelte"].includes(extname(entry.name))) files.push(path);
  }
  return files;
}

for (const path of await collect(resolve(appRoot, "src"))) {
  const rel = relative(appRoot, path).replaceAll("\\", "/");
  const text = await readFile(path, "utf8");
  if (text.includes("@tauri-apps/api/core") && rel !== "src/app/bridge/runtimeApi.ts") errors.push(`${rel}: direct Tauri invoke import is reserved for runtimeApi.ts`);
  if (text.includes("@tauri-apps/api/webview") && rel !== "src/app/bridge/runtimeApi.ts") errors.push(`${rel}: direct Tauri webview import is reserved for runtimeApi.ts`);
  if (rel.startsWith("src/pages/") && text.includes("runtimeApi")) errors.push(`${rel}: pages must not call runtimeApi directly`);
  if (
    rel.startsWith("src/pages/") &&
    text.includes("loadSettings(") &&
    text.includes("queueCatalogDownload(")
  ) {
    errors.push(`${rel}: cross-domain catalog-download orchestration belongs in an application workflow`);
  }
  if (
    rel.startsWith("src/pages/") &&
    (text.includes("download_destination_directory_invalid") ||
      text.includes("download_destination_create_failed"))
  ) {
    errors.push(`${rel}: pages must not branch on low-level download recovery codes`);
  }
}

const applicationEvents = await readFile(resolve(appRoot, "src/app/state/applicationEvents.ts"), "utf8");
for (const needle of [
  "ApplicationEvent",
  "settingsChanged",
  "changesForApplicationEvent",
]) {
  if (!applicationEvents.includes(needle)) {
    errors.push(`bounded application event contract is missing: ${needle}`);
  }
}
if (applicationEvents.includes("Record<string")) {
  errors.push("application events must remain a closed union instead of a free-form event map");
}
for (const unusedEvent of ["providerStateChanged", "packageImported", "packageReplaced", "contentRemoved", "downloadChanged"]) {
  if (applicationEvents.includes(unusedEvent)) {
    errors.push(`application events must not retain events without cross-surface consumers: ${unusedEvent}`);
  }
}
for (const needle of [
  'kind: "settingsChanged"',
  'return settingsChangeSet(event.previous, event.next)',
]) {
  if (!applicationEvents.includes(needle)) {
    errors.push(`application event invalidation mapping is missing: ${needle}`);
  }
}
for (const forbidden of ["eventName: string", "kind: string", "Map<string", "Record<string"]) {
  if (applicationEvents.includes(forbidden)) {
    errors.push(`application events must not become a free-form bus: ${forbidden}`);
  }
}

const applicationChanges = await readFile(resolve(appRoot, "src/app/state/applicationChanges.ts"), "utf8");
for (const needle of [
  "ApplicationChangeSet",
  "publishApplicationChanges",
  "subscribeApplicationChanges",
  "settingsChangeSet",
]) {
  if (!applicationChanges.includes(needle)) {
    errors.push(`application change contract is missing: ${needle}`);
  }
}
for (const forbiddenChangeDomain of [
  "catalog?: boolean",
  "providers?: boolean",
  "library?: boolean",
  "downloads?: boolean",
  "diagnostics?: boolean",
]) {
  if (applicationChanges.includes(forbiddenChangeDomain)) {
    errors.push(`application ChangeSet must not retain unused domain: ${forbiddenChangeDomain}`);
  }
}
for (const needle of [
  "previous.minecraft.rootOverride !== next.minecraft.rootOverride",
  "previous.minecraft.includeDevelopmentContent !== next.minecraft.includeDevelopmentContent",
  "minecraft: minecraftChanged",
]) {
  if (!applicationChanges.includes(needle)) {
    errors.push(`settings ChangeSet semantics are missing: ${needle}`);
  }
}

const productFacade = await readFile(resolve(appRoot, "src/app/bridge/runtimeProductFacade.ts"), "utf8");
for (const needle of ["productQueryCall", "productActionCall", "productCommandCall"]) {
  if (!productFacade.includes(needle)) {
    errors.push(`runtimeProductFacade operation semantics are missing: ${needle}`);
  }
}
if (productFacade.includes("productMutationCall")) {
  errors.push("runtimeProductFacade must use command semantics instead of the legacy mutation helper");
}
if (!productFacade.includes("publishApplicationEvent")) {
  errors.push("state-changing product commands must publish bounded application events");
}
for (const unusedEvent of ["packageImported", "packageReplaced", "contentRemoved", "downloadChanged", "providerStateChanged"]) {
  if (productFacade.includes(unusedEvent)) {
    errors.push(`runtimeProductFacade must not publish command events without cross-surface consumers: ${unusedEvent}`);
  }
}
if (productFacade.includes("publishApplicationChanges")) {
  errors.push("runtimeProductFacade must publish semantic events instead of raw ChangeSets");
}
if (productFacade.includes("ApplicationChangeSet")) {
  errors.push("runtimeProductFacade must not own raw invalidation sets");
}
const libraryPage = await readFile(resolve(appRoot, "src/pages/Library.svelte"), "utf8");
for (const needle of ["settings.data,", "settingsResult.data,", "!changes.minecraft || locationBusy"]) {
  if (!libraryPage.includes(needle)) {
    errors.push(`Library selective invalidation contract is missing: ${needle}`);
  }
}
const downloadsPage = await readFile(resolve(appRoot, "src/pages/Downloads.svelte"), "utf8");
for (const functionName of ["moveInQueue", "pause", "resume", "cancel", "retry"]) {
  const start = downloadsPage.indexOf(`async function ${functionName}(`);
  const next = downloadsPage.indexOf("async function ", start + 1);
  const end = next >= 0 ? next : downloadsPage.indexOf("$effect", start);
  const block = start >= 0 && end > start ? downloadsPage.slice(start, end) : "";
  if (!block) {
    errors.push(`Downloads command handler is missing: ${functionName}`);
  } else if (block.includes("await refresh(false, false)")) {
    errors.push(`Downloads command ${functionName} must reconcile through downloads-changed instead of issuing an immediate snapshot query`);
  }
}

const settingsPage = await readFile(resolve(appRoot, "src/pages/Settings.svelte"), "utf8");
for (const needle of ["Promise.all([", "runtimeProductFacade.loadSettings()", "runtimeProductFacade.discoverMinecraft()"]) {
  if (!settingsPage.includes(needle)) {
    errors.push(`Settings lazy bootstrap contract is missing: ${needle}`);
  }
}
if (settingsPage.includes("snapshot?.backend?.minecraft") || settingsPage.includes("snapshot.backend.minecraft")) {
  errors.push("Settings must not depend on eager Minecraft discovery from the global runtime snapshot");
}

for (const needle of ["previousSettings", "saveSettings({", "}, previousSettings)", "changes.settings"]) {
  if (!settingsPage.includes(needle)) {
    errors.push(`Settings selective invalidation contract is missing: ${needle}`);
  }
}

for (const page of ["Library.svelte", "Settings.svelte"]) {
  const pageSource = await readFile(resolve(appRoot, `src/pages/${page}`), "utf8");
  if (!pageSource.includes("subscribeApplicationChanges")) {
    errors.push(`${page}: selective invalidation subscription is missing`);
  }
}

const tauriManifest = await readFile(resolve(appRoot, "src-tauri/Cargo.toml"), "utf8");
if (!tauriManifest.includes('searchnow-core = { path = "../../../Backend/RustCore" }')) errors.push("Tauri runtime must link the in-process RustCore backend");

const tauriConfig = JSON.parse(await readFile(resolve(appRoot, "src-tauri/tauri.conf.json"), "utf8"));
const defaultCapability = JSON.parse(await readFile(resolve(appRoot, "src-tauri/capabilities/default.json"), "utf8"));
const capabilityPermissions = defaultCapability.permissions ?? [];
if (capabilityPermissions.length !== 1 || capabilityPermissions[0] !== "core:default") {
  errors.push("default Tauri capability must remain minimal; frontend shell/filesystem/network permissions require explicit review");
}
const csp = tauriConfig?.app?.security?.csp ?? "";
for (const directive of ["object-src 'none'", "frame-src 'none'", "base-uri 'none'", "form-action 'none'"]) {
  if (!csp.includes(directive)) errors.push(`Tauri CSP must retain ${directive}`);
}
for (const forbidden of ["shell:", "filesystem:", "http:", "https:"]) {
  if (capabilityPermissions.some((permission) => String(permission).includes(forbidden))) {
    errors.push(`default Tauri capability must not expose broad frontend permission: ${forbidden}`);
  }
}

const commandPaths = [
  "runtime.rs",
  "settings.rs",
  "minecraft.rs",
  "library.rs",
  "package.rs",
  "catalog.rs",
  "download.rs",
];
const forbiddenCommandOwners = [
  "SettingsStore",
  "PlatformContext",
  "DownloadExecutionRuntime",
  "DownloadTransportRegistry",
  "ResourceResolverRegistry",
  "HttpTransport",
  "ProviderAdapterRuntime",
  "IntegratedProvider",
];
for (const file of commandPaths) {
  const text = await readFile(resolve(appRoot, `src-tauri/src/commands/${file}`), "utf8");
  if (!text.includes("SearchNowBackendRuntime")) errors.push(`${file}: Tauri command must delegate through SearchNowBackendRuntime`);
  for (const forbidden of forbiddenCommandOwners) {
    if (text.includes(forbidden)) errors.push(`${file}: Tauri command must not own or construct backend component ${forbidden}`);
  }
}

const downloadCommand = await readFile(resolve(appRoot, "src-tauri/src/commands/download.rs"), "utf8");
const desktopCommand = await readFile(resolve(appRoot, "src-tauri/src/commands/desktop.rs"), "utf8");
if (/\bDownloadRequest\b/.test(downloadCommand)) errors.push("download.rs: raw DownloadRequest/transport selection must not cross the Tauri IPC boundary");
if (!downloadCommand.includes("QueueCatalogDownloadRequest")) errors.push("download.rs: download IPC must accept provider-neutral catalog download intent");
if (!desktopCommand.includes("local_content_directory") || !desktopCommand.includes("item_id: String")) {
  errors.push("desktop.rs: local-content folder opening must resolve an item id through the backend runtime");
}
if (!desktopCommand.includes("completed_download_directory") || !desktopCommand.includes("job_id: String")) {
  errors.push("desktop.rs: open-download-directory IPC must resolve a completed download job instead of accepting an arbitrary frontend path");
}
if (desktopCommand.includes("PathBuf::from(directory)")) {
  errors.push("desktop.rs: frontend-provided directory paths must not drive shell folder opening");
}

const catalogCommand = await readFile(resolve(appRoot, "src-tauri/src/commands/catalog.rs"), "utf8");
if (!catalogCommand.includes("CatalogRequest") || !catalogCommand.includes("query_catalog")) errors.push("catalog.rs: catalog IPC must remain provider-neutral and delegate to the application runtime");

const registry = await readFile(resolve(appRoot, "src-tauri/src/commands/registry.rs"), "utf8");
if (!registry.includes("queue_catalog_download")) errors.push("Tauri registry must expose queue_catalog_download instead of raw transport queuing");
if (!registry.includes("query_catalog")) errors.push("Tauri registry must expose the provider-neutral catalog query command");
if (!registry.includes("inspect_package_path")) errors.push("Tauri registry must expose inspect_package_path for OS drag/drop inspection");

const catalogDownloadWorkflow = await readFile(resolve(appRoot, "src/app/workflows/catalogDownload.ts"), "utf8");
for (const needle of [
  "loadSettings()",
  "chooseDownloadDirectory()",
  "queueCatalogDownload(",
  "saveSettings(",
  "download_destination_directory_invalid",
  "download_destination_create_failed",
]) {
  if (!catalogDownloadWorkflow.includes(needle)) {
    errors.push(`catalogDownload workflow is missing orchestration contract: ${needle}`);
  }
}

const runtimeCommand = await readFile(resolve(appRoot, "src-tauri/src/commands/runtime.rs"), "utf8");
const backendSnapshotStart = runtimeCommand.indexOf("pub fn get_backend_snapshot");
const backendSnapshotEnd = runtimeCommand.indexOf("#[tauri::command]", backendSnapshotStart + 1);
const backendSnapshotBlock = backendSnapshotStart >= 0
  ? runtimeCommand.slice(backendSnapshotStart, backendSnapshotEnd >= 0 ? backendSnapshotEnd : runtimeCommand.length)
  : "";
if (!backendSnapshotBlock.includes("state.snapshot()")) {
  errors.push("get_backend_snapshot must directly read the lightweight in-memory runtime snapshot");
}
if (backendSnapshotBlock.includes("spawn_blocking")) {
  errors.push("get_backend_snapshot must not use spawn_blocking for the lightweight in-memory snapshot");
}

const runtimeApiSource = await readFile(resolve(appRoot, "src/app/bridge/runtimeApi.ts"), "utf8");
if (!runtimeApiSource.includes("getCurrentWebview") || !runtimeApiSource.includes("onDragDropEvent")) {
  errors.push("runtimeApi.ts must own the Tauri webview drag/drop event boundary");
}
const registeredCommands = new Set(
  [...registry.matchAll(/crate::commands::[a-z_]+::([a-z_]+)/g)].map((match) => match[1]),
);
const invokedCommands = new Set(
  [...runtimeApiSource.matchAll(/invoke(?:<[^>]+>)?\("([a-z_]+)"/g)].map((match) => match[1]),
);
for (const command of registeredCommands) {
  if (!invokedCommands.has(command)) {
    errors.push(`Tauri command is registered but has no runtimeApi bridge: ${command}`);
  }
}
for (const command of invokedCommands) {
  if (!registeredCommands.has(command)) {
    errors.push(`runtimeApi invokes an unregistered Tauri command: ${command}`);
  }
}

const bootstrap = await readFile(resolve(appRoot, "src-tauri/src/app_bootstrap.rs"), "utf8");
if (!bootstrap.includes("SearchNowBackendRuntime::new")) errors.push("Tauri bootstrap must construct the consolidated SearchNowBackendRuntime");
if (!bootstrap.includes("app.manage(runtime)")) errors.push("Tauri bootstrap must manage one consolidated backend runtime");
for (const forbidden of ["DownloadExecutionRuntime", "DownloadTransportRegistry", "ResourceResolverRegistry", "HttpTransport", "ProviderAdapterRuntime"]) {
  if (bootstrap.includes(forbidden)) errors.push(`Tauri bootstrap must not construct backend sub-runtime ${forbidden}`);
}

const lib = await readFile(resolve(backendRoot, "src/lib.rs"), "utf8");
if (!lib.includes("pub mod app_runtime")) errors.push("RustCore must expose the application backend runtime");
if (!lib.includes("pub mod application")) errors.push("RustCore must expose the application lifecycle/capability model");
if (!lib.includes("pub mod catalog")) errors.push("RustCore must expose the provider-neutral catalog domain");
if (!lib.includes("mod identity;") || lib.includes("pub mod identity;")) errors.push("RustCore identity helpers must remain crate-private");
if (!lib.includes("pub mod provider_session")) errors.push("RustCore must expose the shared provider-session runtime boundary");
if (!lib.includes("pub mod provider_adapter")) errors.push("RustCore must expose the integrated provider adapter boundary");
if (!lib.includes("mod storage;") || lib.includes("pub mod storage;")) errors.push("RustCore atomic storage helper must remain crate-private");

const appRuntime = await readFile(resolve(backendRoot, "src/app_runtime.rs"), "utf8");
const applicationContent = await readFile(resolve(backendRoot, "src/application_content.rs"), "utf8");
const applicationDownload = await readFile(resolve(backendRoot, "src/application_download.rs"), "utf8");
const applicationLibrary = await readFile(resolve(backendRoot, "src/application_library.rs"), "utf8");
const applicationPackage = await readFile(resolve(backendRoot, "src/application_package.rs"), "utf8");
const frontendTypes = await readFile(resolve(appRoot, "src/app/shared/types.ts"), "utf8");
const settingsModel = await readFile(resolve(backendRoot, "src/settings.rs"), "utf8");
const catalogModel = await readFile(resolve(backendRoot, "src/catalog/model.rs"), "utf8");
const downloadModule = await readFile(resolve(backendRoot, "src/download/mod.rs"), "utf8");
const diagnosticsModel = await readFile(resolve(backendRoot, "src/diagnostics.rs"), "utf8");
for (const needle of [
  "SearchNowBackendRuntime",
  "SettingsStore::new",
  "ProviderAdapterRuntime::compose",
  "providers.resolvers()",
  "ProviderResolvedTransport::new",
  "DownloadExecutionRuntime::new",
  "BackendRuntimeSnapshot",
  "ApplicationState::from_runtime",
  "QueueCatalogDownloadRequest",
]) {
  if (!appRuntime.includes(needle)) errors.push(`application backend runtime is missing composition contract ${needle}`);
}
if (appRuntime.includes("DownloadTransportRegistry::with_local_file")) errors.push("production application runtime must not register local-file fixture transport");
const backendRuntimeSnapshotStart = appRuntime.indexOf("pub struct BackendRuntimeSnapshot");
const backendRuntimeSnapshotEnd = appRuntime.indexOf("}", backendRuntimeSnapshotStart);
const backendRuntimeSnapshotBlock =
  backendRuntimeSnapshotStart >= 0 && backendRuntimeSnapshotEnd > backendRuntimeSnapshotStart
    ? appRuntime.slice(backendRuntimeSnapshotStart, backendRuntimeSnapshotEnd + 1)
    : "";

if (!backendRuntimeSnapshotBlock) {
  errors.push("BackendRuntimeSnapshot declaration could not be inspected");
} else {
  for (const forbiddenBootstrapField of [
    "pub minecraft: MinecraftDiscoverySnapshot",
    "pub downloads: DownloadManagerSnapshot",
    "pub diagnostics: BackendDiagnosticsSnapshot",
  ]) {
    if (backendRuntimeSnapshotBlock.includes(forbiddenBootstrapField)) {
      errors.push(`BackendRuntimeSnapshot must remain lightweight; move ${forbiddenBootstrapField} to its feature query`);
    }
  }
  if (!backendRuntimeSnapshotBlock.includes("pub health: BackendHealthSnapshot")) {
    errors.push("BackendRuntimeSnapshot must carry health summary instead of full diagnostic events");
  }
}
if (appRuntime.includes("minecraft: self.discover_minecraft_raw()?")) {
  errors.push("backend runtime snapshot must not perform Minecraft discovery during bootstrap");
}
if (appRuntime.includes("downloads: self.downloads.download_snapshot()?")) {
  errors.push("backend runtime snapshot must not load the download snapshot during bootstrap");
}
if (!diagnosticsModel.includes("pub fn health_snapshot(&self) -> BackendHealthSnapshot")) {
  errors.push("diagnostics buffer must expose a health-only snapshot for application bootstrap");
}
for (const forbidden of [
  "scan_library(",
  "replace_single_pack(",
  "replace_bundle(",
  "export_directory(",
  "validate_bundle_dependencies(",
]) {
  if (appRuntime.includes(forbidden)) {
    errors.push(`app_runtime.rs must delegate content/package implementation instead of owning ${forbidden}`);
  }
}
for (const forbiddenDownload of [
  "self.downloads.pause(",
  "self.downloads.resume(",
  "self.downloads.queue_to(",
  "DownloadRequest {",
]) {
  if (appRuntime.includes(forbiddenDownload)) {
    errors.push(`app_runtime.rs must delegate download implementation instead of owning ${forbiddenDownload}`);
  }
}
for (const requiredDownload of [
  "DownloadApplicationService",
  "queue_catalog_download",
  "completed_download_directory",
  "pause_download",
  "retry_download",
]) {
  if (!applicationDownload.includes(requiredDownload)) {
    errors.push(`application_download.rs is missing download ownership contract ${requiredDownload}`);
  }
}

for (const forbiddenContent of [
  "replace_single_pack(",
  "replace_bundle(",
  "export_directory(",
  "validate_bundle_dependencies(",
]) {
  if (applicationContent.includes(forbiddenContent)) {
    errors.push(`application_content.rs must remain a thin facade instead of owning ${forbiddenContent}`);
  }
}
for (const requiredLibrary of [
  "LibraryApplicationService",
  "export_directory(",
  "remove_local_content",
]) {
  if (!applicationLibrary.includes(requiredLibrary)) {
    errors.push(`application_library.rs is missing library ownership contract ${requiredLibrary}`);
  }
}
for (const requiredPackage of [
  "PackageApplicationService",
  "replace_single_pack(",
  "replace_bundle(",
  "validate_bundle_dependencies(",
]) {
  if (!applicationPackage.includes(requiredPackage)) {
    errors.push(`application_package.rs is missing package ownership contract ${requiredPackage}`);
  }
}

for (const needle of [
  'pub destination_directory: Option<PathBuf>',
  '#[serde(rename_all = "camelCase", deny_unknown_fields)]',
]) {
  if (!applicationDownload.includes(needle)) errors.push(`QueueCatalogDownloadRequest Rust contract is missing ${needle}`);
}
for (const needle of [
  'destinationDirectory: string | null',
  'export type QueueCatalogDownloadRequest',
]) {
  if (!frontendTypes.includes(needle)) errors.push(`frontend download request contract is missing ${needle}`);
}
if (!frontendTypes.includes("health: BackendHealthSnapshot;")) {
  errors.push("frontend runtime snapshot must mirror the health-only bootstrap contract");
}
if (frontendTypes.includes("diagnostics: BackendDiagnosticsSnapshot;\n};\n\nexport type ProductRuntimeSnapshot")) {
  errors.push("frontend runtime snapshot must not carry diagnostic event history");
}
if (!frontendTypes.includes('| "package"')) {
  errors.push("frontend DiagnosticComponent must include active package runtime diagnostics");
}
if (!diagnosticsModel.includes("Package,")) {
  errors.push("backend DiagnosticComponent must retain Package while package operations are recorded");
}

for (const needle of [
  'pub struct AppSettings',
  'pub struct MinecraftSettings',
]) {
  const index = settingsModel.indexOf(needle);
  const prefix = index >= 0 ? settingsModel.slice(Math.max(0, index - 160), index) : "";
  if (index < 0 || !prefix.includes("deny_unknown_fields")) errors.push(`settings input contract must fail closed for unknown fields near ${needle}`);
}
for (const needle of [
  'pub struct CatalogFilters',
  'pub struct CatalogPageRequest',
  'pub struct CatalogQuery',
  'pub struct CatalogRequest',
]) {
  const index = catalogModel.indexOf(needle);
  const prefix = index >= 0 ? catalogModel.slice(Math.max(0, index - 160), index) : "";
  if (index < 0 || !prefix.includes("deny_unknown_fields")) errors.push(`catalog input contract must fail closed for unknown fields near ${needle}`);
}

for (const needle of [
  'pub(crate) use executor::{default_download_paths, DownloadExecutionRuntime};',
  'pub(crate) use manager::DownloadManager;',
  'pub(crate) use store::DownloadStore;',
  'pub(crate) use transport::{',
  'pub(crate) use workspace::{',
]) {
  if (!downloadModule.includes(needle)) errors.push(`download implementation plumbing must remain crate-private: ${needle}`);
}

if (errors.length) {
  for (const error of errors) console.error(`ERROR: ${error}`);
  process.exit(1);
}
console.log("SearchNow architecture contract: PASS");
