# Next Action

Status: `REMOTE_FOUNDATION_HARDENING_COMPLETE`

## Current checkpoint

`develop` contains the provider-independent SearchNow application foundation and the latest remote hardening work.

Current implemented scope includes:

- one Tauri-managed `SearchNowBackendRuntime` as the application backend authority;
- provider-neutral catalog/session/resolver composition without a fake production provider;
- bounded recoverable downloads with explicit destination-directory handling;
- fail-closed inbound settings/catalog/download IPC contracts;
- crate-private storage, identity, download execution, persistence, transport and workspace plumbing where no external boundary requires them;
- Library, Discover, Downloads and Settings frontend surfaces behind one `runtimeProductFacade` and one raw `runtimeApi` bridge;
- contextual per-operation UI feedback, modal focus containment/busy-state semantics, reachable compact-desktop layout behavior, and executable UI-contract validation;
- Minecraft manual-location recovery from Library/package inspection when automatic discovery is insufficient;
- explicit file/folder import plus safe Tauri webview drag-and-drop package inspection through the same RustCore validator;
- settings schema v2 with compatible v1 migration, default export-directory preference, and explicit `keepBoth` / `stopOnConflict` backup behavior;
- export archives re-inspected before success, with truthful verification/rollback cleanup failures;
- bounded deterministic Library container scanning instead of collecting an unbounded directory before truncation;
- Linux repository/backend/frontend verification plus hosted Windows release executable launch, NSIS build, install/launch/uninstall lifecycle, uninstall user-state preservation, and version-upgrade state preservation.

## Verification authority

Do **not** treat this document as the authority for the latest commit SHA, workflow run number, or pass/fail result.

For exact current verification state, use the `Repository Verify` run attached to the current `develop` HEAD. A queued, running, cancelled, skipped, or failed run is not a verified baseline.

This file owns continuation intent only. GitHub Actions owns exact executable verification evidence.

## Current engineering priority

Remote-foundation hardening is now at its intended ceiling. Keep the current `develop` HEAD behind `Repository Verify`, keep repository memory truthful, and do not add architecture-only layers. Further work should produce new product/runtime evidence rather than more speculative abstractions.

## Remaining product evidence

The following remain unproven until exercised directly:

- installed application launch and interaction on the target Windows machine;
- target-machine AppData/Minecraft discovery and representative real local libraries;
- settings/download/export/folder-picker/drag-drop behavior through an installed Tauri window;
- a real provider login/session/catalog/resolver integration;
- production TLS/CDN/provider behavior;
- representative performance and scale;
- owner target-machine installer interaction, branding/signing, clean-machine and release acceptance.

## Next functional paths

```text
REMOTE_FOUNDATION_HARDENING_COMPLETE
├── TARGET_WINDOWS_ACCEPTANCE
└── REAL_PROVIDER_INTEGRATION
```

Do not create placeholder providers, speculative credential abstractions, new managers, new policy layers, or additional architecture documentation to avoid those evidence dependencies.
