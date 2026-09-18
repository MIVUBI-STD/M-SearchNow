# 01 — Development Flow

SearchNow uses a minimum-flow development discipline adapted to this repository's smaller application architecture.

## Canonical lifecycle

```text
Context Recovery
→ Product Requirements
→ Architecture & UX
→ Implementation
→ Verification
→ Local Promotion
→ Stable Promotion
```

These are human workflow names. Internal implementation states may exist later but do not create alternate workflow vocabulary.

## Context Recovery

Recover only the context that can change the current decision:

```text
AGENTS.md
→ CONTEXT.md
→ docs/knowledge/next-action.md
→ smallest relevant owner
→ exact source/evidence only when needed
```

Do not preload sibling architecture documents, legacy evidence, broad source trees, old reviews, or history as reassurance. Reuse already-fresh evidence instead of rereading it.

Read-only inspection stops after reporting unless change is explicitly requested.

## Product Requirements

Owns what SearchNow must do, must not do, user-facing behavior, privacy/safety boundary, and acceptance intent.

## Architecture & UX

Owns component boundaries, source organization, UI navigation/state model, data ownership, service boundaries, and technical decisions required before implementation.

## Implementation

Owns executable source. Implement only approved requirements/contracts and avoid broad cleanup outside the affected owner.

Use this order before adding machinery:

```text
no change required?
→ delete or consolidate an unnecessary path?
→ reuse the current canonical owner?
→ use an existing platform/dependency capability?
→ make the smallest complete addition
→ add a new abstraction/system only for a proven repeated responsibility
```

A normal change should aim for one semantic owner, one coherent outcome, and the fewest touched files consistent with clean ownership. Do not add speculative extension points, fallback paths without a supported consumer, or configuration for choices the product does not actually expose.

Do not simplify away trust-boundary validation, bounded resource limits, data-loss prevention, recoverability, required error handling, accessibility basics, or explicit user requirements.

## Verification

Use the cheapest proof that can falsify the changed claim. A green unrelated check does not prove the change.

Current proof ladder:

```text
repository/static
→ executed source / unit
→ integration / hosted platform compile
→ rendered or live application behavior
→ target-Windows installed acceptance
```

The proof ceiling is determined by what was actually exercised, not where a command happened to run. Hosted `windows-latest` compilation is not installed-Windows acceptance.

Do not create a new test framework, fixture hierarchy, benchmark system, or review layer solely to prove one bounded change. Add stronger proof infrastructure when the responsibility is repeated or the risk justifies it.

## Cross-owner work

When a task crosses owners, settle them sequentially:

```text
Owner A decides or fixes its boundary
→ produce the minimum stable handoff/result
→ stop Owner A
→ Owner B consumes that result without recomputing Owner A truth
```

Do not keep several owners active on the same decision.

## Local Promotion

`develop → Local` means one coherent update is ready to become the verified working baseline.

- source: `develop`
- gate: Local Promotion Verify
- merge: squash
- result: one new logical milestone commit on `Local`
- then synchronize `develop` to resulting `Local` HEAD

## Stable Promotion

`Local → main` is explicit stable promotion.

- source: `Local`
- gate: Stable Release Verify
- merge: normal merge commit
- stable marker is not synchronized back into lower branches merely for ancestry

Tags/releases are separate explicit actions.

## STOP rule

Stop when the requested behavior is complete, the first wrong owner is fixed, and matching proof is sufficient for the current execution context. Optional optimization, future-proofing, or unrelated cleanup is not completion work.
