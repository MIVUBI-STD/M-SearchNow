# 03 — Implementation Roadmap

## Objective

Build SearchNow as a small, maintainable Minecraft Bedrock content-management desktop application with one native backend owner and thin UI/IPC boundaries.

The old BlueCoin WinForms structure remains evidence only; it is not the source architecture to recreate.

## Approved implementation shape

```text
EngineData/
├── Backend/RustCore/
│   └── src/
│       ├── app_runtime.rs
│       ├── identity.rs
│       ├── settings.rs + storage.rs
│       ├── minecraft.rs + library.rs
│       ├── package/
│       ├── download/
│       ├── catalog/
│       ├── provider_session/
│       ├── provider_adapter/
│       └── diagnostics.rs
└── Frontend/RustApp/
    ├── src/                 Svelte product UI + facade/bridge
    └── src-tauri/src/       thin Tauri bootstrap + commands
```

## Runtime rule

```text
Svelte UI
→ product facade
→ thin runtime API
→ Tauri command
→ SearchNowBackendRuntime
→ RustCore domain/runtime owner
```

Do not add Python, a local HTTP server, or another backend process unless a concrete capability cannot be served cleanly by the Rust process and an explicit architecture decision revises this rule.

## Sequencing note

Phase numbers describe capability/verification boundaries. Hosted Windows CI now covers executable and NSIS lifecycle smoke. Phase 8 remains the owner-machine acceptance boundary for real Minecraft/AppData discovery, native interaction, representative data, and performance; hosted CI does not waive that evidence.

## Phase status

### Phase 0 — Evidence / Development System

**Complete.** Legacy architecture, product/safety boundaries, branch model, and development routing are documented.

### Phase 1 — Application Architecture Scaffold

**Complete at repository/static level.** Tauri/Svelte/Rust stack, four product surfaces, thin bridge, source-size contracts, and runtime-status vertical slice are established.

### Phase 2 — Minecraft Discovery + Local Library

**Backend complete; provider-independent frontend wiring complete remotely.** RustCore discovers current Bedrock GDK/account roots, optional Preview, and legacy UWP fallback, then performs bounded local indexing. Library now renders the actual runtime snapshot with summary/search/filter/warning states. Target-machine behavior remains unproven until Phase 8.

### Phase 3 — Package Inspection

**Backend complete for read-only inspection.** Folder / `.mcpack` / `.mcaddon` metadata inspection, manifest classification, BP/RP relationship detection, and archive safety checks are implemented without extraction/mutation.

Package file-picker/import UX remains deferred until a concrete target-runtime interaction path is approved and tested.

### Phase 4 — Download Runtime

**Backend foundation complete; queue frontend wired remotely.** Persistent bounded queue, cancellation/retry, fail-closed persisted-state validation, startup crash reconciliation, Windows-safe destination naming, staged no-overwrite finalization, public HTTPS transport, and provider-resolved transport are implemented. Downloads renders actual queue/progress/error state and exposes allowed lifecycle actions.

The deterministic `local-file` transport remains test/development-only and is not registered by the production application runtime.

### Phase 5 — Provider-Neutral Runtime

**Backend foundation complete.** Catalog domain, provider-session manager, resolver registry, integrated-provider composition, shared canonical provider/resource identity validation, and secret-safe public state are implemented.

No real provider endpoint/login is implemented yet.

### Phase 6 — Application Runtime + Observability

**Remote foundation complete.** Tauri manages one `SearchNowBackendRuntime`; Settings/Downloads share one atomic persistence owner; current health is separated from historical diagnostics; scheduler continuation failures are surfaced; raw transport selection is kept behind the backend boundary; hosted Linux verification and hosted Windows executable/installer lifecycle gates are established.

Settings now exposes safe runtime diagnostics without introducing a second logging system.

### Phase 7 — Deterministic Build / Verification

**Complete for repository verification.** npm and both Rust application scopes have committed lockfiles. Repository, Local-promotion, and stable-release workflows use `npm ci` and Cargo `--locked`, with current GitHub Actions runtime versions.

### Phase 8 — Owner Target-Windows Acceptance

**Required, currently deferred by owner.** Run non-destructive owner-machine evidence for real AppData resolution, current GDK/UWP discovery, settings save/reload, native folder-picker/permission behavior, package inspection, representative download finalization, diagnostics, UI interaction, and representative performance.

Hosted Windows CI now proves a bounded executable + NSIS lifecycle, but it is not a substitute for this owner-machine acceptance phase.

### Phase 9 — Real Provider Integration

**Not started.** Define and implement concrete provider credential/session acquisition, endpoint contracts, timeout/retry/backoff behavior, and Marketplace/PlayFab-specific adapters without changing the established product/transport boundary.

This work may begin before Phase 8 only if the owner explicitly chooses that sequencing; it still cannot claim target-machine validation.

### Phase 10 — Discover / Downloads / Settings Product Wiring

**Provider-independent portion complete remotely.** Library, Downloads, Settings, runtime health, diagnostics, and the provider-neutral Discover query surface are wired through the existing facade/API/Tauri/runtime path.

Live Discover results still depend on Phase 9. A Discover download action must not guess destination filename/package metadata; it should be completed only after the provider contract supplies a truthful output descriptor.

### Phase 11 — Release Acceptance

Representative large-library/network fixtures, final branding/code signing, owner clean-machine verification, and Local → main promotion.

## Quality rules

1. Fix the first wrong owner.
2. No runtime truth duplicated in Svelte.
3. No Tauri `invoke` calls from pages/components.
4. No business logic inside command wrappers.
5. Keep source-size budgets green; split ownership before god-objects form.
6. Network operations must map to identifiable product behavior.
7. Hosted Windows lifecycle proof never upgrades itself to owner-machine acceptance proof.
8. Do not freeze temporary workarounds into repository contracts.
9. Verification must use committed dependency graphs; update lockfiles only as an explicit dependency change.
10. Frontend must not fabricate provider/local-runtime capabilities merely to make a screen look complete.
