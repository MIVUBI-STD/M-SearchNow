# Current Validation

Reviewed: **2026-09-18**

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
- Svelte/TypeScript checking;
- production frontend build;
- hosted Windows RustCore tests;
- hosted Windows frontend build;
- hosted Windows Tauri compilation.

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

- installed SearchNow launch/interaction on the owner's target Windows machine;
- real AppData/Minecraft discovery against the target installation;
- representative real local libraries at user scale;
- actual native folder picker and filesystem permission behavior;
- production provider authentication/session semantics;
- Marketplace/PlayFab or other real provider endpoint compatibility;
- production TLS/CDN behavior;
- real-world performance, memory usage or long-running stability;
- installer/signing/update/clean-machine release behavior.

## Evidence rule

Use the strongest evidence actually available:

```text
repository/static
→ executed source/unit
→ integration/hosted compile
→ rendered/live app
→ target-Windows installed acceptance
```

Do not report a stronger level than the highest level actually exercised.
