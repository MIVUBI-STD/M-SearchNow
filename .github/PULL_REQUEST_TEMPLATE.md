## Purpose

Describe the one logical outcome this pull request delivers.

## Target boundary

- [ ] `develop` work / review only
- [ ] `develop` → `Local` verified integration promotion
- [ ] `Local` → `main` stable promotion

## Scope

**Changed owners:**

**Intentionally not changed:**

## Verification

- [ ] Cheapest relevant proof completed
- [ ] Repository Verify when applicable
- [ ] Implementation/build/tests when applicable
- [ ] Local Promotion Verify for `develop` → `Local`
- [ ] Stable Release Verify for `Local` → `main`

Evidence / result:

## Repository hygiene

- [ ] No credentials, private account/user data, entitlement-derived secrets, or protected-content keys added
- [ ] No generated/transfer-only artifact was promoted to source of truth
- [ ] No unrelated cleanup/refactor bundled
- [ ] Product safety/privacy boundaries remain satisfied

## Local promotion contract

Repository note: workflow verification validates the PR source and code evidence; direct-push prevention and merge-method enforcement require repository branch protection/rulesets.

For `develop` → `Local`:

- [ ] source branch is `develop`
- [ ] PR represents one approved logical update
- [ ] merge method will be **Squash and merge**
- [ ] result adds exactly one logical milestone commit to `Local`
- [ ] `develop` will be synchronized to resulting `Local` HEAD before new work

## Stable promotion contract

For `Local` → `main`:

- [ ] source branch is `Local`
- [ ] Stable Release Verify passes
- [ ] merge method will be a normal **merge commit**

Tags/releases are separate explicit publishing actions.
