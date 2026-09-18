# 04 — Verification and Promotion

## Iteration proof

During normal `develop` work, run the cheapest check that can falsify the changed claim.

Current repository-only baseline:

```bash
python tools/verify_repository.py
```

When executable source is introduced, add module-specific formatting, static analysis, build, unit, integration, and runtime/UI tests according to the selected stack. Do not invent gates before the underlying implementation exists.

## Promotion boundary

### develop → Local

Requires:

- dedicated PR from `develop`;
- Local Promotion Verify PASS;
- one coherent approved logical update;
- squash merge;
- post-merge synchronization of `develop` to resulting `Local` HEAD.

### Local → main

Requires:

- explicit stable PR from `Local`;
- Stable Release Verify PASS;
- normal merge commit.

### Enforcement boundary

Promotion workflows are intentionally PR-only. They validate source branch and executable repository evidence.

They do **not** enforce direct-push prevention, review requirements, or the final merge method by themselves. Those controls belong to GitHub repository branch protection/rulesets and required status checks.

Do not describe promotion governance as fully enforced unless those repository-level controls are actually enabled for `develop`, `Local`, and `main`.

## Proof language

Use only:

```text
implemented
repository/static verified
integration verified
runtime/UI verified
not verified
```

A passing repository structure check does not prove application runtime behavior.
