# Current Validation

Reviewed: **2026-09-20**

## Purpose

This document describes **what the repository verification pipeline proves** and its proof ceiling.

It intentionally does not hard-code a "latest verified commit" or workflow run number. Exact current evidence belongs to the `Repository Verify` run attached to the current branch HEAD; this document would otherwise become stale after ordinary development commits.

## Remote verification contract

A successful current `Repository Verify` run proves:

- repository contract checks;
- RustCore formatting;
- locked RustCore tests;
- RustCore Clippy with warnings denied;
- Tauri adapter formatting;
- locked frontend dependency installation;
- frontend architecture/source-size checks;
- UI contract, semantic design-system, motion, and product-state coverage gates;
- Svelte/TypeScript checking;
- production frontend build;
- hosted Windows RustCore tests;
- hosted Windows frontend build;
- hosted Windows Tauri compilation;
- release executable build and launch smoke with required Windows UI evidence: Library at 960×640, 1280×720, and 1440×900 plus Discover, Downloads, and Settings at 960×640;
- NSIS installer production and artifact sanity;
- silent install → installed launch → uninstall lifecycle;
- uninstall preservation of user-owned AppData state;
- synthetic prior-version → current-version installer upgrade with persisted settings/download state preserved.

A queued, running, cancelled, skipped, or failed workflow is **not** a verified baseline.

## Current architecture proof boundary

When the gate passes, it supports the repository/static/integration claim for this architecture:

```text
Svelte product UI
→ runtimeProductFacade
→ runtimeApi
→ thin Tauri command adapter
→ SearchNowBackendRuntime
→ RustCore domains
```

The gate also protects current repository contracts around:

- one application backend owner;
- provider-neutral IPC;
- fail-closed inbound product request DTOs;
- crate-private low-level download implementation plumbing;
- deterministic locked dependency installation;
- secret-safe provider/session/download boundaries;
- bounded storage, archive, network and diagnostics behavior covered by tests.

## Not proven by remote CI

Remote verification does **not** prove:

- SearchNow interaction on the owner's actual target Windows machine;
- real AppData/Minecraft discovery against the target installation;
- representative real local libraries at user scale;
- actual native folder picker and filesystem permission behavior;
- production provider authentication/session semantics;
- Marketplace/PlayFab or other real provider endpoint compatibility;
- production TLS/CDN behavior;
- pixel-level aesthetic correctness beyond the captured hosted surfaces and states;
- real-world performance, memory usage or long-running stability;
- code signing, final branding, OS reputation/SmartScreen behavior, and owner clean-machine release acceptance.

## Evidence rule

Use the strongest evidence actually available:

```text
repository/static
→ executed source/unit
→ hosted integration/runtime/installer lifecycle
→ rendered/live interaction
→ owner target-Windows acceptance
```

Do not report a stronger level than the highest level actually exercised.
