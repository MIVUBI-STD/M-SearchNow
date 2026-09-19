# 04 — Verification and Promotion

## Iteration proof

During normal `develop` work, run the cheapest check that can falsify the changed claim.

Current repository contract baseline:

```bash
python tools/verify_repository.py
```

Executable source is now present. Shared verification therefore includes module formatting/static analysis/tests/build plus hosted Windows executable and NSIS lifecycle checks. New gates should still be added only when they falsify a concrete product or release claim.

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
hosted integration/runtime verified
owner target-machine verified
not verified
```

A passing repository structure check does not prove runtime behavior, and a passing hosted Windows lifecycle does not prove owner target-machine behavior.
