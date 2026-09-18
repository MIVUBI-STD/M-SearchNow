# Security

SearchNow is being developed from static analysis of a legacy executable that handled Minecraft account/content metadata and network services. Security/privacy behavior must therefore be explicit.

## Never commit

- credentials, API secrets, tokens, cookies, or private keys;
- Minecraft account identifiers when not intentionally sanitized;
- entitlement-derived secrets or protected-content decryption keys;
- raw private user data or diagnostic dumps containing such data;
- third-party credentials recovered from the legacy executable.

If a secret is found in source/history, treat it as exposed and rotate/revoke it through the relevant provider. Do not preserve it merely for compatibility.

## Product boundary

SearchNow must not implement hidden user/account-derived data upload, DRM bypass, paid-to-free conversion, or protected-content key pooling/distribution.

Network actions involving user/account-derived data must be documented and visible. Local-first processing is the default.

## Reporting

For a security-sensitive issue, document the minimum reproducible evidence without publishing credentials or protected data. Fix the first canonical owner rather than masking the symptom downstream.

## Dependency maintenance

Committed lockfiles define reproducible dependency graphs. Dependabot checks npm, both Cargo manifests, and GitHub Actions weekly. Dependency PRs must pass the same repository verification gates as ordinary changes; do not auto-merge security or version updates without reviewing compatibility and release notes.
