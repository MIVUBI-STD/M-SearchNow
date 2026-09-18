# Next Action

Status: `REMOTE_FOUNDATION_HARDENING`

## Current checkpoint

`develop` contains the provider-independent SearchNow application foundation and the latest remote hardening work.

Current implemented scope includes:

- one Tauri-managed `SearchNowBackendRuntime` as the application backend authority;
- provider-neutral catalog/session/resolver composition without a fake production provider;
- bounded recoverable downloads with explicit destination-directory handling;
- fail-closed inbound settings/catalog/download IPC contracts;
- crate-private storage, identity, download execution, persistence, transport and workspace plumbing where no external boundary requires them;
- Library, Discover, Downloads and Settings frontend surfaces behind one `runtimeProductFacade` and one raw `runtimeApi` bridge;
- Linux repository/backend/frontend verification and a hosted Windows RustCore/Tauri compile gate.

## Verification authority

Do **not** treat this document as the authority for the latest commit SHA, workflow run number, or pass/fail result.

For exact current verification state, use the `Repository Verify` run attached to the current `develop` HEAD. A queued, running, cancelled, skipped, or failed run is not a verified baseline.

This file owns continuation intent only. GitHub Actions owns exact executable verification evidence.

## Current engineering priority

Finish remote-foundation hygiene before new functional architecture:

1. current `develop` HEAD must pass `Repository Verify`;
2. repository memory must remain truthful and avoid hard-coded stale verification identifiers;
3. repository governance should enforce the documented branch model where repository administration permits it;
4. after remote hygiene is clean, stop architecture-only refinement.

## Remaining product evidence

The following remain unproven until exercised directly:

- installed application launch and interaction on the target Windows machine;
- target-machine AppData/Minecraft discovery and representative real local libraries;
- settings/download/folder-picker behavior through an installed Tauri window;
- a real provider login/session/catalog/resolver integration;
- production TLS/CDN/provider behavior;
- representative performance and scale;
- installer, branding, clean-machine and release acceptance.

## Next functional paths

```text
REMOTE_FOUNDATION_HARDENING
├── TARGET_WINDOWS_RUNTIME_SMOKE
└── REAL_PROVIDER_INTEGRATION
```

Do not create placeholder providers, speculative credential abstractions, new managers, new policy layers, or additional architecture documentation to avoid those evidence dependencies.
