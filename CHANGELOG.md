# Changelog

All notable SearchNow repository/product changes will be recorded here.

## Unreleased

### Added

- Recovered BlueCoin 2.4 architecture and runtime documentation.
- PRD-Creator-style development routing and branch model adapted for SearchNow.
- Tauri 2 + Svelte 5 + TypeScript/Vite + Rust desktop scaffold.
- In-process `EngineData/Backend/RustCore` backend library separated from Tauri IPC.
- Typed/versioned settings and Minecraft Bedrock GDK/account-scoped discovery.
- Bounded read-only local behavior/resource/skin-pack and world indexing.
- Read-only folder / `.mcpack` / `.mcaddon` package inspection with manifest classification, BP/RP dependency detection, and ZIP traversal/symlink/size/compression-ratio safeguards.
- Persistent bounded download manager with typed lifecycle, cancellation/retry, restart recovery, progress checkpointing, safe workspace planning, and atomic no-overwrite publication.
- Provider-neutral `https-public` and credential-safe `provider-resolved` transports with timeout, redirect, response-size, resolver-expiry, and sensitive-runtime-material safeguards.
- Provider-neutral catalog, shared runtime-only provider-session manager, resource resolver registry, and integrated-provider composition.
- Consolidated `SearchNowBackendRuntime` as the single application backend owner managed by Tauri.
- Bounded secret-safe diagnostics and hosted Windows RustCore/Tauri compile gates.
- Shared `AtomicFileStore` for staged persistence, backup recovery, and stale temporary-file cleanup.
- Normal committed Tauri PNG/ICO application resources used by standard `tauri_build::build()`.
- Canonical provider/resource identity validation shared by catalog, session, adapter, and resolved-download layers.
- Persisted download-state validation, duplicate-id rejection, stale-sequence reconciliation, destination-stage cleanup, and interrupted-finalization recovery.
- Windows-safe destination filename validation, including reserved DOS device names and trailing dot/space rules.
- Provider-neutral `QueueCatalogDownloadRequest` product intent at the Tauri IPC boundary.
- Sanitized download scheduler continuation-error state exposed through download snapshots.
- Committed `package-lock.json` plus standalone RustCore and Tauri `Cargo.lock` dependency graphs.
- Runtime-connected frontend workspace for Library, Downloads, Settings, runtime health, and safe diagnostics.
- Provider-neutral Discover search/filter/sort/pagination UI plus a thin Tauri `query_catalog` adapter over the existing Rust application runtime.
- Shared frontend DTO/format helpers and a single normalized `runtimeProductFacade` over the raw Tauri bridge.
- Shared frontend UI primitives for loading/empty/error states, notices, metrics, results summaries, content-type marks, user-facing details, technical details, and state pills.
- Responsive narrow-window layout behavior, keyboard focus visibility, reduced-motion support, and semantic download progress.
- Local Library sorting and Download-history search/filter controls.
- Typed frontend `queue_catalog_download` bridge/facade support without exposing raw transport selection.
- Centralized route metadata used by Sidebar and the route-aware topbar.
- Session-only restoration of the last active route.
- User-facing Minecraft channel/storage-kind labels in Settings.
- Dedicated final visual-polish layer for card rhythm, density, metric readability, hover/focus feedback, form affordance, mobile spacing, and loading-state presentation.
- Windows NSIS installer build plus hosted lifecycle proof for silent install, installed launch, uninstall, uninstall user-state preservation, and version upgrade with persisted settings/download state preserved.
- End-to-end package file/folder inspection and safe import UX for `.mcpack`, `.mcaddon`, and `.mcworld`, including single-pack update and transactional `.mcaddon` bundle update wiring through Tauri, product facade, and Library UI.

### Changed

- SettingsStore and DownloadStore no longer maintain separate file-replacement/recovery implementations; both delegate to the shared atomic persistence primitive.
- Production application runtime no longer registers the deterministic `local-file` fixture transport.
- Tauri download commands no longer accept raw `DownloadRequest`/transport selection from the frontend.
- Provider/resource validation no longer carries independent size/key rules across runtime layers.
- Diagnostics current component health is independent from retained historical failure events.
- Download scheduler continuation errors are retained instead of silently discarded.
- Windows build readiness no longer depends on generated `OUT_DIR` placeholder icons or `window_icon_path` build-script injection.
- Repository, Local-promotion, and stable-release verification use current GitHub Actions releases, `npm ci`, Cargo `--locked`, and read-only repository permissions.
- Repository validation requires committed dependency locks and guards the product-intent/production-transport boundary.
- Product pages remain mounted across sidebar navigation so page-local search/filter/sort state is preserved; filesystem scans, catalog queries, download polling, settings loads, and diagnostics reads run only for the active page.
- Library and Discover separate user-facing content details from lower-level technical metadata.
- Discover pagination suppresses duplicate `provider:itemId` entries and exposes retry/reset flows without fabricating provider behavior.
- Downloads exposes search/filter/reset, accessible progress semantics, shared state presentation, and recovery refresh behavior.
- Settings tracks unsaved changes, supports form-only revert, keeps rescan behind saved preferences, and replaces internal Minecraft storage enum values with readable labels.
- Sidebar and topbar share one navigation metadata source to prevent route-label drift.
- Product-facing loading/empty/unavailable copy stays separate from implementation and credential details.
- Card, metric, form, loading, and narrow-window presentation now share a final visual density/polish layer without introducing another runtime or state system.

### Verification

- Repository verification covers RustCore format/tests/Clippy, Tauri formatting, locked frontend installation, frontend architecture/typecheck/build checks, and hosted Windows RustCore/Tauri compilation.
- Exact commit SHA, workflow run number, test count, and current pass/fail state are intentionally not duplicated here; the `Repository Verify` run attached to the relevant branch HEAD is the verification authority.
- Installed target-Windows runtime behavior, real provider/network behavior, performance, installer, branding, and clean-machine release acceptance remain separate evidence boundaries.

### Safety / Architecture

- Runtime credential material remains excluded from persisted download/catalog DTOs and public provider status.
- Provider-session material remains opaque, runtime-only, non-serializable, and non-Debug.
- Product UI/IPC cannot select arbitrary internal download transports.
- Frontend pages/components do not own persistent/runtime truth and do not call raw Tauri `invoke` directly.
- Discover does not fabricate real-provider results or guess package/output filenames in order to expose an unsupported download action.
- Protected-content bypass, key distribution, and hidden entitlement-data transmission remain outside the product boundary.
