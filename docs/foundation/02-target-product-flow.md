# 02 — Target Product Flow

## Goal

SearchNow should make Minecraft Bedrock content management understandable without requiring users to know PlayFab, UUIDs, entitlement internals, encryption details, or package implementation details.

The primary interface should answer four questions:

1. What content do I have?
2. What content can I discover?
3. What is currently downloading/processing?
4. Where did my exported content go?

## Primary navigation

Keep the top-level application to four user-facing areas:

```text
Library | Discover | Downloads | Settings
```

### Library

Shows content already available to the user.

Sources may include:

- installed Minecraft content;
- user-imported local packs;
- completed downloads;
- local projects/exports.

Default presentation should use a compact content-management list. Library is a management surface, not a dashboard: avoid large metric cards, oversized content cards, and repeated decorative containers when a dense row communicates the same state more clearly.

Each row should prioritize title, content type, meaningful status, version, and only the minimum contextual metadata needed to distinguish storage or issues.

Example information:

```text
Adventure World
World · v1.4                         Ready

Furniture Pack
Resource Pack · Development          Needs review
```

Use visual cards primarily in Discover, where artwork and browsing are part of the task.

Advanced identifiers should only appear under an expandable `Technical details` section.

### Discover

Search and filter catalog content.

Core controls:

- search;
- type filter;
- source/status filter;
- sorting;
- content preview;
- one primary action based on content state.

Suggested type filters:

```text
All | World | Add-On | Resource Pack | Skin | Persona
```

Search should be debounced so each keystroke does not produce an unnecessary request.

### Downloads

A real queue, not a modal blocking operation.

Each item should expose a meaningful stage:

```text
Queued
Downloading
Validating
Extracting
Packaging
Finished
Failed
```

The progress model must be tied to actual work rather than arbitrary UI percentages.

User controls:

- pause/resume where supported;
- retry;
- cancel;
- open output;
- clear completed items.

### Settings

Keep technical configuration outside the normal workflow.

Sections:

#### Minecraft
- detected installation;
- Minecraft data location;
- rescan;
- manual location override.

#### Downloads
- download directory;
- export directory;
- overwrite/duplicate policy.

#### Privacy
- local-only behavior by default;
- optional diagnostics;
- clear description of any data leaving the machine.

#### Advanced
- developer logs;
- raw identifiers;
- network diagnostics;
- package metadata inspector.

## First-run experience

First launch should be deterministic and transparent.

```text
Start SearchNow
  ↓
Detect Minecraft
  ↓
Show detection result
  ↓
Confirm local/output folders
  ↓
Open Library
```

No account-derived data should be transmitted merely because the application launched.

Example state:

```text
Minecraft Bedrock
✓ Installation detected
✓ Local content folder detected

[Continue]
```

If detection fails:

```text
Minecraft Bedrock wasn't detected automatically.

[Locate manually]   [Retry]
```

## One-primary-action rule

The interface should not ask users to choose between implementation operations such as download/decrypt/export modes during normal use.

The primary action depends on the state of the item.

```text
AVAILABLE REMOTELY  → Download
DOWNLOADING         → View progress
DOWNLOADED          → Open
LOCAL PACK          → Export
INVALID PACK        → Review / Repair
```

Secondary or developer operations belong in an overflow/advanced menu.

## Content detail model

Example:

```text
Adventure Furniture
Add-On

Status        Installed
Resource Pack ✓
Behavior Pack ✓
Manifest      Valid

[Open]

Technical details ›
```

Do not place UUIDs, keys, endpoint names, or raw JSON in the default detail view.

## Import flow

Support drag-and-drop and an explicit import action.

```text
Import file
  ↓
Detect format
  ↓
Inspect archive
  ↓
Validate manifest
  ↓
Identify content type
  ↓
Show result
```

Possible inputs:

- `.mcpack`;
- `.mcaddon`;
- `.mctemplate`;
- `.zip`;
- unpacked local pack folder where supported.

The importer should never silently modify the source file.

## Export flow

```text
Select local/owned content
  ↓
Validate structure
  ↓
Determine package type
  ↓
Package
  ↓
Verify output
  ↓
Show output location
```

Before destructive modification, work from a copy or temporary staging directory.

## Error model

User-facing message first, technical detail second.

Bad:

```text
NullReferenceException
HTTP 403
```

Preferred:

```text
Could not retrieve this item.
The service rejected the request.

[Retry]

Technical details ›
HTTP 403
...
```

Every recoverable failure should offer a next action.

## Background work

Only tasks that are clearly expected by the user should continue in the background, such as downloads already started by the user.

Startup scans should be bounded and cancellable. Heavy scans should run asynchronously so the UI remains responsive.

## Target user experience

A normal user should be able to:

1. install/open SearchNow;
2. see Minecraft detected;
3. find their content;
4. search available content;
5. start a permitted download/import/export operation;
6. see accurate progress;
7. open the finished result;

without learning internal Minecraft service architecture.
