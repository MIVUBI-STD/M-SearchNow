# 10 — Application Backend Runtime Architecture

Status: **current application backend composition contract**

## Goal

SearchNow has one application-level Rust backend owner. Tauri manages that owner as one state object and does not assemble settings, provider, transport, or download engines inside individual commands.

```text
Tauri
  ↓ one managed state
SearchNowBackendRuntime
├── SettingsStore
├── PlatformContext
├── local discovery / library facade
├── ProviderAdapterRuntime
│   ├── ProviderSessionManager
│   ├── CatalogService
│   └── ResourceResolverRegistry
└── DownloadExecutionRuntime
    └── ProviderResolvedTransport
        └── SAME ResourceResolverRegistry
```

This boundary remains provider-neutral. No PlayFab, Marketplace, provider credential, or protected-content mechanism is implemented here.

## Source ownership

```text
EngineData/Backend/RustCore/src/app_runtime.rs
→ application backend construction
→ canonical backend paths
→ safe aggregate runtime snapshot
→ delegation into existing local/provider/download owners

EngineData/Frontend/RustApp/src-tauri/src/app_bootstrap.rs
→ resolve application directories
→ construct SearchNowBackendRuntime
→ app.manage(runtime)

EngineData/Frontend/RustApp/src-tauri/src/commands/*.rs
→ thin IPC adapters using State<SearchNowBackendRuntime>
```

`app_runtime.rs` composes existing owners; it does not replace their domain responsibilities.

## Construction order

Application construction is fail-closed:

```text
resolve paths
  ↓
create provider-neutral HTTP transport
  ↓
compose ProviderAdapterRuntime
  ↓
register local-file + public HTTPS + provider-resolved transports
  ↓
attach provider-resolved transport to providers.resolvers()
  ↓
recover DownloadExecutionRuntime
  ↓
construct SettingsStore + retain PlatformContext
  ↓
return SearchNowBackendRuntime
```

If provider composition, transport registration, download-state recovery, or another required construction step fails, no healthy application runtime is returned.

## One Tauri state owner

Tauri must not accumulate separate managed backend owners as features grow.

Allowed:

```text
State<SearchNowBackendRuntime>
```

Do not independently manage or construct in Tauri commands:

```text
SettingsStore
DownloadExecutionRuntime
DownloadTransportRegistry
ResourceResolverRegistry
ProviderAdapterRuntime
HttpTransport
```

Commands call methods on `SearchNowBackendRuntime`; domain logic remains in RustCore. Product-intent request DTOs that cross Tauri must deserialize fail-closed when the frontend sends unknown fields, so Rust/TypeScript contract drift becomes an explicit error instead of silently dropping user intent. This applies to download, catalog, and settings input contracts.

## Blocking work

Application consolidation does not move blocking filesystem/network work onto the UI thread.

Tauri commands that perform filesystem-heavy local operations clone the application runtime and delegate through `tauri::async_runtime::spawn_blocking`. Download execution keeps its existing bounded native worker/thread behavior.

## Persistence ownership

There is no second application-state database.

```text
settings.json
→ SettingsStore remains canonical

downloads/state.json
→ DownloadStore remains canonical
```

`SearchNowBackendRuntime` owns references/composition, not duplicate persisted copies of those states.

## Provider/download identity guarantee

The provider resolver registry used by application downloads is exactly the registry produced by the composed provider runtime:

```text
ProviderAdapterRuntime::resolvers()
        ↓ same Arc
ProviderResolvedTransport
        ↓
DownloadExecutionRuntime
```

This prevents catalog/provider registration from diverging from the resolver set used by actual queued downloads.

## Safe runtime snapshot

`BackendRuntimeSnapshot` is serializable because it contains only safe aggregate state:

```text
RuntimeStatus
MinecraftDiscoverySnapshot
Vec<ProviderRuntimeStatus>
DownloadManagerSnapshot
```

It does not contain provider session material, Authorization headers, cookies, signed URLs, runtime request headers, or provider-defined opaque session payloads.

The snapshot is diagnostic/application-state information; it is not a second persistence owner.

## Local facade rule

Settings, Minecraft discovery, and local library scan are reachable through `SearchNowBackendRuntime`, while package inspection remains a standalone RustCore domain capability until a concrete product interaction requires application-runtime exposure. Consolidation does not introduce broad caching or duplicate library state.

## Security boundary

Do not:

- place provider protocols or credentials in Tauri commands;
- persist runtime provider/session material in the aggregate snapshot;
- create a second download/catalog/session runtime in a command;
- bypass `ProviderAdapterRuntime` when registering provider resolvers;
- create a second settings/download database for the application runtime;
- hardcode legacy BlueCoin title/provider secrets;
- implement entitlement-key sharing, DRM bypass, or protected-content decryption.

## Verification boundary

REMOTE_GITHUB proves with deterministic fixtures:

- application runtime constructs successfully with no providers;
- safe aggregate snapshot reports runtime/local/provider/download state without credential material;
- a resolver contributed by `ProviderAdapterRuntime` is the resolver actually used by the application's provider-resolved download transport;
- provider-resolved application download reaches a deterministic HTTP fixture and final atomic file publication;
- invalid provider composition prevents application runtime construction;
- all Tauri feature commands delegate through `State<SearchNowBackendRuntime>`;
- architecture validation rejects old per-command/sub-runtime ownership patterns and checks the critical download-destination Rust/TypeScript request mirror;
- all prior backend regressions remain passing.

Hosted CI still does **not** prove installed Windows Tauri execution, real AppData/Minecraft behavior, production network/provider behavior, or real provider authentication. Those require TARGET_WINDOWS / NETWORK / PROVIDER evidence.
