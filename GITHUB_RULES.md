# GitHub Rules

Canonical GitHub execution rules for AI/Codex/ChatGPT working in SearchNow.

Use the seven core rules in order:

```text
PIN
→ READ MINIMUM
→ DIAGNOSE
→ TOOL + TRANSFER GATE
→ WRITE ONCE
→ VERIFY + FAILURE POLICY
→ STOP
```

## 1. PIN — establish exact authority

Before material GitHub action, know:

```text
repository
exact branch/ref
current HEAD when relevant
requested scope
target writability
```

- Direct branch/file fetch is current-state authority; search is discovery.
- Never silently fall back to the default branch.
- Every write targets the intended branch/ref explicitly.
- Routine development targets `develop`.
- `Local` and `main` are promotion targets, not routine editing branches.
- Existing-file replacement/deletion uses current content/blob state for the exact branch.

## 2. READ MINIMUM — inspect only what can change the decision

Default to the smallest owner set. Do not broad-scan history, reviews, generated output, or adjacent modules without a concrete unresolved question.

A truncated/partial result is incomplete evidence, not proof of absence.

## 3. DIAGNOSE — fix the first wrong owner

```text
requirement wrong                    → Product Requirements owner
legacy understanding wrong           → docs/legacy owner
architecture/UX contract wrong       → architecture/UX owner
implementation wrong                 → exact source owner
implementation correct, test stale   → test
implementation/test correct, CI wrong→ workflow/repository policy
derived artifact wrong               → upstream canonical owner
```

`No change required` is valid. Do not bundle unrelated cleanup.

## 4. TOOL + TRANSFER GATE

Choose the simplest capability that safely produces the requested final state.

```text
current branch/file state
→ direct GitHub fetch

one bounded text file
→ contents create/update

coherent multi-file change / atomicity needed
→ atomic tree+commit or real Git workspace

runtime/UI behavior claim
→ matching runtime capability

CI diagnosis
→ run → failing job/step → relevant log
```

Before first write establish: final logical content, exact repo/ref/path set, current state, and a transfer method that can carry the real change safely.

Do not create placeholder transport files, temporary workflows, loader fragments, alternate repository structures, or history rewrites to work around tool limitations.

## 5. WRITE ONCE — coherent logical delivery

Prepare the complete intended logical state before mutation.

Commit format:

```text
<type>(<optional-scope>): <concise logical outcome>
```

Use `feat`, `fix`, `docs`, `refactor`, `test`, `ci`, `build`, `release`, or bounded `chore` according to the primary outcome.

Split commits only for independent outcomes. Do not split by file, directory, tool call, or transfer limitation.

## 6. VERIFY + FAILURE POLICY

Run the cheapest check that can falsify the changed claim. Full suites belong at integration/promotion boundaries once executable tests exist.

A successful completed run is PASS. Queued/running/pending/cancelled/skipped is not PASS.

Do not weaken a valid gate to obtain green status.

Failure handling:

- known capability mismatch → stop method immediately;
- permission/safety denial → stop unless condition changes;
- 404 → verify exact repo/ref/target once;
- stale/conflict → refetch relevant state once;
- timeout/unknown mutation → inspect target state before retry;
- same-cause operational failure → maximum two attempts with new evidence.

## 7. STOP — completion is terminal

Stop when requested outcome and sufficient proof are satisfied, or when an authoritative capability/policy boundary blocks further action.

Do not automatically continue into unrelated cleanup, promotion, release, backlog work, or adjacent audits.

# Branch and promotion

```text
develop → active Development
Local   → verified integration baseline
main    → stable history
```

- Routine work targets `develop`.
- `develop → Local` requires a dedicated PR, Local Promotion Verify, and squash merge.
- One approved promotion = exactly one new logical milestone commit on `Local`.
- After promotion, synchronize `develop` to resulting `Local` HEAD before new work.
- `Local → main` requires an explicit stable PR and Stable Release Verify.
- Stable promotion uses a normal merge commit.
- Promotion workflows are PR-only evidence. A manually dispatched workflow must never be treated as promotion proof.
- Workflow checks can validate source branch and code evidence, but they cannot by themselves prevent direct pushes or enforce the final merge method. Those controls require GitHub branch protection/rulesets with required status checks.
- Until repository-level enforcement is configured, report promotion governance as documented but not fully enforced.
- Tags/releases are separate publishing actions.
- Do not bypass a failed gate with direct edits to another branch.

# Evidence reporting

Final repository reports distinguish:

```text
implemented
repository/static verified
integration verified
runtime/UI verified
not verified
```

Claim only the strongest level actually proven.
