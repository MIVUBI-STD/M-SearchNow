# 06 — Backend Architecture

Status: **current backend contract**

## Goal

Keep SearchNow backend local-first, testable, bounded, and cheap to run. Source responsibilities are separated without adding process count.

```text
one SearchNow executable
├─ Tauri adapter
└─ RustCore (in-process library)
```

`RustCore` is not a server, daemon, worker process, or second runtime.

## Source ownership

```text
EngineData/Backend/RustCore/src/
├─ settings.rs    typed/versioned settings + staged persistence
├─ storage.rs     crate-private atomic persistence primitive
├─ identity.rs    crate-private provider/resource validation primitive
├─ platform.rs    process/platform path context
├─ minecraft.rs   Bedrock storage discovery
├─ library.rs     bounded read-only local index
├─ package/       bounded read-only package inspection
├─ download/
│  ├─ model.rs       persisted DTO/state contract
│  ├─ manager.rs     job lifecycle/concurrency
│  ├─ store.rs       schema-versioned persistence/recovery
│  ├─ workspace.rs   payload workspace + atomic publication
│  ├─ transport.rs   generic transport contract + local-file
│  ├─ executor.rs    scheduler/execution/progress checkpoints
│  ├─ http.rs        public HTTPS + ephemeral HTTP mechanics
│  └─ resolver.rs    stable provider ref + runtime resource resolution
├─ runtime.rs     backend identity
└─ error.rs       structured backend failures

EngineData/Frontend/RustApp/src-tauri/src/
├─ app_bootstrap.rs   construct/register runtime owners
└─ commands/          Tauri IPC adaptation only
```

Business/filesystem/archive/download/network/provider-resolution behavior must not accumulate in Tauri commands or Svelte pages. Low-level download manager/store/transport/workspace/execution plumbing is crate-private; external consumers receive only the public DTO/provider contracts needed at real boundaries.

## Local Minecraft and library boundaries

Current Bedrock creator storage discovery checks current GDK/account-scoped storage first and optionally falls back to legacy UWP. Preview is opt-in. Normal library scanning is deliberately shallow and read-only:

```text
behavior_packs/
resource_packs/
skin_packs/
minecraftWorlds/
```

Normal discovery does not recursively size/hash content, read entitlement files, derive/upload content keys, contact catalogs, or mutate Minecraft data.

## Package inspection

`RustCore/src/package/` accepts folder, `.mcpack`, and `.mcaddon` inputs for bounded read-only inspection. Manifest primary categories map to Behavior Pack, Resource Pack, Skin Pack, or World Template. BP↔RP relationships come from dependency UUIDs, not folder-name guesses.

Archives are never extracted during inspection. Traversal/absolute escapes, symlinks, duplicate normalized paths, excessive sizes, and extreme compression ratios are rejected.

## Download lifecycle

`download/manager.rs` owns the state machine and bounded concurrency. Persisted source identity remains intentionally narrow:

```text
transport key
resource id
```

Never persist authorization headers, bearer tokens, cookies, signed URLs, provider secrets, or other ephemeral transfer material in `DownloadJob`/download state.

State progression remains:

```text
Queued → Preparing → Transferring → Finalizing → Completed
           │              │
           ├─ failure ────┴→ Failed
           └─ cancel → CancelRequested → Cancelled

Failed(retryable) / Cancelled / Interrupted
→ explicit retry → Queued
```

Default max active jobs is 3; hard validated cap is 16. Default retained jobs is 1,000; hard cap is 5,000. Active jobs recovered after restart become explicit `Interrupted`; silent resume is not implemented.

Progress is updated in memory while disk persistence is checkpointed at 1 MiB plus lifecycle boundaries. Final publication stages into the destination directory, fsyncs, then performs atomic no-overwrite publication.

## Transport execution

`transport.rs` converts a `DownloadSourceRef` into a readable stream plus optional total byte count. It does not own queue state, retries, destination paths, or UI state.

`executor.rs` owns claim → open → transfer → progress → finalize → terminal-state execution on bounded native worker threads in the same process. Transfer buffer size is fixed at 256 KiB. Cooperative cancellation is checked between reads; finalizing remains non-cancellable.

Current registered transport keys:

```text
local-file
https-public
provider-resolved
```

### `local-file`

Reads a regular non-symlink local file and exists primarily to prove deterministic end-to-end execution.

### `https-public`

`http.rs` owns ordinary unauthenticated HTTPS transfer mechanics using pinned blocking `ureq` + rustls. Production public jobs are HTTPS-only and reject URL userinfo, fragments, and persisted query strings.

Defaults:

```text
connect timeout   10 s
read timeout      15 s
overall deadline  30 min
redirect limit    5
response limit    8 GiB
```

Redirects are manual and every destination is revalidated. The configured response cap applies to declared and unknown lengths. A body shorter than its `Content-Length` cannot finalize successfully. Socket connect/read timeout is owned by the HTTP client; SearchNow separately enforces the overall deadline with a monotonic stream wrapper.

## Runtime Resource Resolver / Provider Adapter Boundary

Authenticated or signed resources use `provider-resolved`; they do **not** weaken the persistence rules of `https-public`.

Persisted identity is a stable opaque reference:

```text
transport = provider-resolved
resourceId = <provider>:<opaque-resource-id>
```

`ProviderResourceRef` validates both parts. The opaque id cannot contain URL schemes, query strings, fragments, control characters, or oversized data. It represents provider-owned stable identity only.

At execution time:

```text
DownloadJob stable identity
        ↓
ResourceResolverRegistry
        ↓
ResourceResolver(provider)
        ↓
ResolvedResource   [memory only]
├─ temporary URL / signed query
├─ bounded temporary headers
└─ optional expiry
        ↓
HTTP runtime request
        ↓
DownloadTransportStream
```

`ResolvedResource` is deliberately runtime-only; it is not a persisted DTO. Resolver failures expose a stable code + retryability while persisted messages remain generic. Runtime HTTP/read errors on sensitive requests are sanitized before reaching download failure persistence.

Expiry semantics are explicit:

- already-expired resolved material is discarded;
- the resolver gets one immediate refresh opportunity before transfer;
- an explicit job retry executes resolution again;
- signed URLs/tokens are never reused from persisted job state.

Runtime headers are bounded and validated. HTTP ownership headers such as `Host`, `Content-Length`, `Connection`, `Transfer-Encoding`, and `Proxy-Authorization` cannot be injected by a resolver. Credential-bearing headers cannot be forwarded through a cross-origin redirect.

The Tauri bootstrap currently constructs an empty `ResourceResolverRegistry` and registers the generic `provider-resolved` transport. This proves the boundary exists while truthfully providing **zero production provider adapters** today.

## Settings / performance / security guardrails

- settings are typed, schema-versioned, and staged to disk;
- at most 64 account-scoped GDK roots per product are inspected;
- at most 5,000 direct entries per local content container are indexed;
- manifest reads are capped at 1 MiB and world-name reads at 4 KiB;
- package/archive work is bounded and read-only;
- queue persistence is capped and concurrency is bounded;
- network transfer has redirect/time/byte bounds;
- persisted download DTO/store ownership is statically guarded against runtime credential fields;
- legacy protected-content/key-sharing dependencies are rejected from active RustCore source;
- no Python worker/local HTTP service/background daemon is introduced.

## Error model

Expected absence remains state rather than exceptional control flow (`Found | NotFound | UnsupportedPlatform`). Corrupt local/package metadata becomes an item/inspection issue. Download/transport/resolver failures use stable codes plus explicit retryability; sensitive provider request details are not persisted as error text.

## Verification boundary

REMOTE_GITHUB currently proves:

- RustCore format/compile/tests/clippy;
- deterministic Minecraft/library/package fixtures;
- download lifecycle/concurrency/recovery/persistence/finalization fixtures;
- local-file execution;
- public HTTP policy, redirect, timeout, size and Content-Length behavior;
- resolver stable-reference validation;
- missing-provider handling;
- expiry refresh and retry re-resolution;
- ephemeral signed-query/Authorization use without state persistence;
- architecture/source-size/frontend regression gates.

TARGET_WINDOWS / REAL_FIXTURE / NETWORK / PROVIDER evidence is still required for installed-app behavior, real Minecraft storage, representative filesystem behavior, real public TLS/CDNs, provider authentication/session lifecycle, and provider-specific APIs.
