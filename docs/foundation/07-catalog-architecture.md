# 07 — Catalog Architecture

Status: **current catalog-domain contract**

## Goal

Keep discovery/catalog work provider-neutral, bounded, credential-safe, and separate from download execution.

Catalog answers **what resources are discoverable**. Download owns **how selected resource bytes are transferred**.

```text
CatalogRequest
    ↓
CatalogService
    ↓
CatalogProviderRegistry
    ↓
CatalogProvider
    ↓
validated CatalogPage
    ↓ user selects an item later
CatalogDownloadRef
    ↓
DownloadSourceRef
    ↓
existing DownloadExecutionRuntime
```

The catalog layer does not own retries, download workspaces, transfer progress, authentication persistence, or UI state.

## Source ownership

```text
EngineData/Backend/RustCore/src/catalog/
├─ mod.rs       public module boundary
├─ model.rs     serializable query/page/item/download-reference contracts
├─ provider.rs  provider registry, query coordinator, bounds and normalization
└─ tests.rs     deterministic fake-provider fixtures
```

`RustCore/src/download/` remains the owner of download/source identity and execution. Catalog can map a validated item into that existing boundary but must not duplicate it.

## Query contract

`CatalogRequest` contains:

```text
provider
query
├─ text
├─ filters
│  ├─ contentTypes
│  └─ tags
├─ sort
└─ page
   ├─ limit
   └─ cursor
```

Supported provider-neutral content types:

```text
World
Addon
ResourcePack
Skin
Persona
Other
```

Supported provider-neutral sort intents:

```text
Relevance
Newest
Oldest
NameAsc
NameDesc
```

A provider translates these intents into its own supported query shape. SearchNow core does not encode provider endpoint syntax in domain DTOs. Catalog request structs reject unknown JSON fields so frontend/backend contract drift fails explicitly instead of silently changing query intent.

## Bounds

Request bounds:

```text
provider key             64 bytes
query text               256 bytes
content-type filters     16 values
tag filters               32 values
tag text                  64 bytes
page size                 1..100 (default 30)
cursor                    512 bytes
```

Provider result bounds:

```text
items/page                <= requested page size and <= 100
item id                   256 bytes
title                     256 bytes
description               4 KiB
item tags                 32 values
tag text                  64 bytes
total title/description/tag text per item   8 KiB
```

Duplicate item ids within one provider page fail closed. Invalid cursors, oversized result pages, malformed text, or invalid download references become `catalog_provider_data_invalid` rather than being silently truncated or coerced.

These are safety/performance bounds and can be raised only with real provider/workload evidence.

## Provider contract

A catalog adapter implements:

```text
CatalogProvider
├─ key()
└─ query(CatalogQuery)
    → CatalogProviderPage
```

`CatalogProviderRegistry` owns provider lookup and duplicate-key prevention. `CatalogService` validates the request before calling a provider, then validates provider output before returning the public `CatalogPage`.

Provider-specific raw response payloads do not become public SearchNow DTOs. An adapter must translate them into `CatalogProviderItem` first.

## Failure contract

Provider failures contain only:

```text
stable code
retryable flag
```

SearchNow supplies the generic safe message. Unsupported/unsafe provider failure codes collapse to `catalog_provider_query_failed`, so provider exception text, URLs, tokens, response bodies, or session data cannot become catalog error messages by default.

Missing providers return explicit non-retryable `catalog_provider_unavailable`.

## Download identity

A catalog item may be non-downloadable or expose one of two explicit identities:

```text
CatalogDownloadRef::PublicHttps
    → ordinary query-free public HTTPS identity
    → https-public

CatalogDownloadRef::ProviderResolved
    → stable provider + opaque resource id
    → provider-resolved
    → runtime ResourceResolver
```

Public HTTPS identities are validated using the same download boundary as ordinary public downloads. Query-bearing/signed public URLs are rejected from catalog DTOs.

Provider-resolved identities use the existing `ProviderResourceRef` rules. Runtime URL/query/header/auth material is not a catalog field.

## Credential boundary

Catalog DTOs must never own:

```text
Authorization headers
bearer/access tokens
cookies
signed URLs
runtime request headers
provider session secrets
```

Repository guards scan catalog domain DTO ownership for these credential fields. A real provider adapter may need runtime session material internally, but that belongs behind a separate provider-session boundary and must not alter `CatalogQuery`, `CatalogItem`, or persisted `DownloadJob` contracts.

## Current runtime status

There is currently **no real catalog provider registered**. The provider-neutral domain and fake-provider fixtures are complete first so later provider work has a narrow integration target.

No PlayFab/Marketplace endpoint, provider login, or provider-specific credential implementation is part of this catalog slice.

## Verification boundary

REMOTE_GITHUB currently proves:

- bounded request validation happens before provider calls;
- fake provider search/filter/sort/cursor pagination works;
- missing provider handling is explicit;
- malformed provider items fail closed;
- unsafe provider failure text is normalized without secret leakage;
- provider-resolved catalog items map to the existing `provider-resolved` download boundary;
- ordinary public items map to `https-public` and query-bearing signed URLs are rejected;
- catalog source paths are required by architecture/repository guards;
- catalog domain DTO ownership is checked for runtime credential fields.

PROVIDER evidence is still required before claiming compatibility with any real remote catalog.
