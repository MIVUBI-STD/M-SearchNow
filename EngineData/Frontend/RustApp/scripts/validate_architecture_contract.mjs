import { access, readdir, readFile } from "node:fs/promises";
import { extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const backendRoot = resolve(appRoot, "../../Backend/RustCore");
const required = [
  "src/App.svelte",
  "src/app/bridge/runtimeApi.ts",
  "src/app/bridge/runtimeProductFacade.ts",
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
  if (rel.startsWith("src/pages/") && text.includes("runtimeApi")) errors.push(`${rel}: pages must not call runtimeApi directly`);
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

const bootstrap = await readFile(resolve(appRoot, "src-tauri/src/app_bootstrap.rs"), "utf8");
if (!bootstrap.includes("SearchNowBackendRuntime::new")) errors.push("Tauri bootstrap must construct the consolidated SearchNowBackendRuntime");
if (!bootstrap.includes("app.manage(runtime)")) errors.push("Tauri bootstrap must manage one consolidated backend runtime");
for (const forbidden of ["DownloadExecutionRuntime", "DownloadTransportRegistry", "ResourceResolverRegistry", "HttpTransport", "ProviderAdapterRuntime"]) {
  if (bootstrap.includes(forbidden)) errors.push(`Tauri bootstrap must not construct backend sub-runtime ${forbidden}`);
}

const lib = await readFile(resolve(backendRoot, "src/lib.rs"), "utf8");
if (!lib.includes("pub mod app_runtime")) errors.push("RustCore must expose the application backend runtime");
if (!lib.includes("pub mod catalog")) errors.push("RustCore must expose the provider-neutral catalog domain");
if (!lib.includes("mod identity;") || lib.includes("pub mod identity;")) errors.push("RustCore identity helpers must remain crate-private");
if (!lib.includes("pub mod provider_session")) errors.push("RustCore must expose the shared provider-session runtime boundary");
if (!lib.includes("pub mod provider_adapter")) errors.push("RustCore must expose the integrated provider adapter boundary");
if (!lib.includes("mod storage;") || lib.includes("pub mod storage;")) errors.push("RustCore atomic storage helper must remain crate-private");

const appRuntime = await readFile(resolve(backendRoot, "src/app_runtime.rs"), "utf8");
const frontendTypes = await readFile(resolve(appRoot, "src/app/shared/types.ts"), "utf8");
const settingsModel = await readFile(resolve(backendRoot, "src/settings.rs"), "utf8");
const catalogModel = await readFile(resolve(backendRoot, "src/catalog/model.rs"), "utf8");
const downloadModule = await readFile(resolve(backendRoot, "src/download/mod.rs"), "utf8");
for (const needle of [
  "SearchNowBackendRuntime",
  "SettingsStore::new",
  "ProviderAdapterRuntime::compose",
  "providers.resolvers()",
  "ProviderResolvedTransport::new",
  "DownloadExecutionRuntime::new",
  "BackendRuntimeSnapshot",
  "QueueCatalogDownloadRequest",
]) {
  if (!appRuntime.includes(needle)) errors.push(`application backend runtime is missing composition contract ${needle}`);
}
if (appRuntime.includes("DownloadTransportRegistry::with_local_file")) errors.push("production application runtime must not register local-file fixture transport");

for (const needle of [
  'pub destination_directory: Option<PathBuf>',
  '#[serde(rename_all = "camelCase", deny_unknown_fields)]',
]) {
  if (!appRuntime.includes(needle)) errors.push(`QueueCatalogDownloadRequest Rust contract is missing ${needle}`);
}
for (const needle of [
  'destinationDirectory: string | null',
  'export type QueueCatalogDownloadRequest',
]) {
  if (!frontendTypes.includes(needle)) errors.push(`frontend download request contract is missing ${needle}`);
}
if (frontendTypes.includes('| "package"')) errors.push("frontend DiagnosticComponent must not expose inactive package runtime diagnostics");

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
