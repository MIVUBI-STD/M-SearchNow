---
name: development-brief
description: Front door for non-trivial SearchNow repository/application Development. Recover continuity, identify the first wrong owner, bound the smallest complete change, define falsifiable acceptance criteria and cheapest sufficient proof, then execute.
---

# Development Brief

Use when changing how SearchNow works: product behavior, UI/UX, architecture, application source, policy, tooling, machine contracts, validation, or repository structure.

Root `AGENTS.md` owns work mode, authority, continuity, canonical workflow naming, and branch behavior. `GITHUB_RULES.md` owns GitHub mutation and verification mechanics. `docs/foundation/01-development-flow.md` owns minimum-flow, context, proof, cross-owner, and STOP discipline.

## Development contract

Before writing, establish only:

```text
Goal / actual requirement
First wrong owner
In scope / out of scope
Acceptance criteria: 2–5
Cheapest sufficient proof
Unresolved material decision, only if one remains
```

A user-proposed implementation is evidence about the requirement, not automatically the required architecture.

## Procedure

1. **Recover actual state** — follow Development boot and reconcile continuation against current repository state. Load only context that can change the decision.
2. **Diagnose** — separate product requirement, legacy understanding, architecture/UX, implementation, test, and CI failures; fix the first wrong owner.
3. **Minimize before adding** — accept `no change required`; prefer deletion/consolidation, reuse the canonical owner and existing platform capability, then make the smallest complete addition. Add a new abstraction only for a proven repeated responsibility.
4. **Bound** — preserve valid behavior outside scope and define falsifiable acceptance criteria. Aim for one semantic owner and one coherent outcome.
5. **Use legacy evidence correctly** — `docs/legacy/` may establish observed BlueCoin behavior, but never forces SearchNow to preserve unsafe/undesired behavior.
6. **Execute sequentially across owners** — settle one owner, emit the minimum stable result/handoff, stop that owner, then continue to the next without recomputing prior truth.
7. **Verify** — use the cheapest proof capable of falsifying the changed claim. Distinguish static/source proof, executed tests, hosted integration/compile proof, rendered/live application behavior, and target-Windows installed acceptance.
8. **Update continuity only when needed** — update `next-action.md` only when active continuation materially changes. Do not create status archives or duplicate roadmaps.
9. **Stop** — once requested behavior, first-wrong-owner repair, and sufficient proof are complete. Do not continue into speculative cleanup or future-proofing.

Do not invent a test framework, compatibility layer, fallback path, config surface, subsystem, or Skill solely to make one bounded change look more complete.

## User-facing form

```text
Tujuan:
Hasil yang dituju:
Tidak diubah:
Cara memastikan benar:
```

Stop when requested scope is complete and evidence supports the claim.
