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
- contextual per-operation UI feedback, modal focus containment/busy-state semantics, compact/DPI-scaled desktop layout behavior, keyboard search/navigation, user-facing Activity, and executable UI/design-system/motion/state-coverage validation;
- Minecraft manual-location recovery from Library/package inspection when automatic discovery is insufficient;
- explicit file/folder import plus safe Tauri webview drag-and-drop package inspection through the same RustCore validator;
- settings schema v2 with compatible v1 migration, default export-directory preference, and explicit `keepBoth` / `stopOnConflict` backup behavior;
- export archives re-inspected before success, with truthful verification/rollback cleanup failures;
- bounded deterministic Library container scanning instead of collecting an unbounded directory before truncation;
- Linux repository/backend/frontend verification plus hosted Windows release executable launch, required Library/Discover/Downloads/Settings viewport evidence, NSIS build, install/launch/uninstall lifecycle, uninstall user-state preservation, and version-upgrade state preservation;
- deterministic verification-route startup control for hosted Windows UI evidence, removing focus/keyboard-dependent route capture;
- change-scoped routine CI so documentation-only commits avoid rebuilding the full Windows runtime/installer while source-affecting changes still receive the exhaustive gate;
- TypeScript unused-local/unused-parameter enforcement, including removal of stale Settings state discovered by the stricter gate.

## Verification authority

Do **not** treat this document as the authority for the latest commit SHA, workflow run number, or pass/fail result.

For exact current verification state, use the `Repository Verify` run attached to the current `develop` HEAD together with the unchanged source parent when HEAD is documentation-only. Documentation-only HEADs run the lightweight repository contract; source-affecting HEADs run the full gate. A queued, running, cancelled, skipped, or failed required run is not a verified baseline.

This file owns continuation intent only. GitHub Actions owns exact executable verification evidence.

## Current engineering priority

Remote-foundation hardening is now at its intended ceiling and the remote UI/product hardening pass is complete. Do not add more remote-only polish, abstractions, managers, policy layers, or speculative architecture before local acceptance.

The next work session should start from **target-Windows local acceptance** against the current verified `develop` baseline. Any further remote changes should be driven by concrete findings from that local test.

## Remaining product evidence

The following remain unproven until exercised directly:

- installed application launch and interaction on the target Windows machine;
- target-machine AppData/Minecraft discovery and representative real local libraries;
- settings/download/export/folder-picker/drag-drop behavior through an installed Tauri window;
- a real provider login/session/catalog/resolver integration;
- production TLS/CDN/provider behavior;
- representative performance and scale;
- owner target-machine installer interaction, branding/signing, clean-machine and release acceptance.

## Next local acceptance checklist

When local testing resumes, use this order:

1. Launch SearchNow from the verified `develop` baseline on the target Windows machine.
2. Check Windows scaling at 100%, 125%, and 150%, including resize down to 960×640.
3. Verify keyboard flows: Alt+1..4, Ctrl+F, Arrow Up/Down, Home/End, Esc, and Ctrl+A in selection mode.
4. Verify Minecraft auto-detection and manual folder recovery.
5. Verify package import for `.mcpack`, `.mcaddon`, `.mcworld`, unpacked folders, and Explorer drag-and-drop.
6. Verify update, duplicate detection, dependency warnings, export/backup, remove confirmation, and recovery behavior.
7. Verify Downloads pause/resume/retry/cancel and network interruption recovery.
8. Verify Activity and Diagnostics behavior.
9. Test with a representative large Minecraft library and watch startup, scan, search, list scrolling, and memory behavior.
10. Record only concrete runtime/UI issues; fix those back on `develop`.

## Next functional paths

```text
REMOTE_FOUNDATION_HARDENING_COMPLETE
└── TARGET_WINDOWS_ACCEPTANCE
    ├── LOCAL_UI_INTERACTION
    ├── REAL_MINECRAFT_STORAGE
    ├── LARGE_LIBRARY_SCALE
    └── REAL_PROVIDER_INTEGRATION (later)
```

Do not create placeholder providers, speculative credential abstractions, new managers, new policy layers, or additional architecture documentation to avoid those evidence dependencies.
