from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]

REQUIRED = [
    "README.md", "AGENTS.md", "CONTEXT.md", "GITHUB_RULES.md", "CONTRIBUTING.md", "SECURITY.md",
    ".agents/skills/development-brief/SKILL.md",
    "docs/README.md",
    "docs/foundation/00-product-boundaries.md",
    "docs/foundation/01-development-flow.md",
    "docs/foundation/02-target-product-flow.md",
    "docs/foundation/03-implementation-roadmap.md",
    "docs/foundation/04-verification-promotion.md",
    "docs/foundation/05-application-architecture.md",
    "docs/foundation/06-backend-architecture.md",
    "docs/foundation/07-catalog-architecture.md",
    "docs/foundation/08-provider-session-architecture.md",
    "docs/foundation/09-provider-adapter-architecture.md",
    "docs/foundation/10-application-runtime-architecture.md",
    "docs/foundation/11-observability-windows-readiness.md",
    "docs/knowledge/next-action.md", "docs/knowledge/ownership.md", "docs/knowledge/source-authority.md",
    "docs/knowledge/work-routing.md", "docs/knowledge/work-modes/development.md", "docs/knowledge/work-modes/maintenance.md",
    "docs/legacy/01-current-state.md", "docs/legacy/04-recovered-source-architecture.md",
    "docs/legacy/05-recovered-symbol-map.md", "docs/legacy/06-runtime-data-contracts.md", "docs/legacy/07-reconstruction-evidence.md",
    "EngineData/Backend/RustCore/Cargo.toml", "EngineData/Backend/RustCore/Cargo.lock",
    "EngineData/Backend/RustCore/src/lib.rs", "EngineData/Backend/RustCore/src/app_runtime.rs",
    "EngineData/Backend/RustCore/src/diagnostics.rs", "EngineData/Backend/RustCore/src/identity.rs",
    "EngineData/Backend/RustCore/src/settings.rs", "EngineData/Backend/RustCore/src/storage.rs",
    "EngineData/Backend/RustCore/src/minecraft.rs", "EngineData/Backend/RustCore/src/library.rs",
    "EngineData/Backend/RustCore/src/download/store.rs", "EngineData/Backend/RustCore/src/download/resolver.rs",
    "EngineData/Backend/RustCore/src/catalog/mod.rs", "EngineData/Backend/RustCore/src/catalog/model.rs",
    "EngineData/Backend/RustCore/src/catalog/provider.rs",
    "EngineData/Backend/RustCore/src/provider_session/mod.rs", "EngineData/Backend/RustCore/src/provider_session/model.rs",
    "EngineData/Backend/RustCore/src/provider_session/runtime.rs",
    "EngineData/Backend/RustCore/src/provider_adapter/mod.rs", "EngineData/Backend/RustCore/src/provider_adapter/model.rs",
    "EngineData/Backend/RustCore/src/provider_adapter/runtime.rs",
    "EngineData/Frontend/RustApp/package-lock.json",
    "EngineData/Frontend/RustApp/src-tauri/Cargo.lock",
    "EngineData/Frontend/RustApp/src-tauri/build.rs",
    "EngineData/Frontend/RustApp/src-tauri/icons/icon.png",
    "EngineData/Frontend/RustApp/src-tauri/icons/icon.ico",
    "EngineData/Frontend/RustApp/src-tauri/src/app_bootstrap.rs",
    "EngineData/Frontend/RustApp/src-tauri/src/commands/registry.rs",
    "tools/windows_smoke_readiness.ps1",
    ".github/PULL_REQUEST_TEMPLATE.md", ".github/workflows/shared-verify.yml",
    ".github/workflows/repository-verify.yml", ".github/workflows/local-promotion-verify.yml",
    ".github/workflows/release-verify.yml",
]

errors = [f"missing required path: {rel}" for rel in REQUIRED if not (ROOT / rel).exists()]

checks = {
    "README.md": ["develop", "Local", "main", "Context Recovery", "Stable Promotion"],
    "AGENTS.md": ["first wrong owner", "develop", "Local", "main"],
    "CONTEXT.md": ["Development branch: `develop`", "Verified integration baseline: `Local`", "Stable branch: `main`"],
    "GITHUB_RULES.md": ["PIN", "READ MINIMUM", "WRITE ONCE", "STOP"],
    "docs/foundation/06-backend-architecture.md": ["RustCore", "GDK", "read-only", "Tauri"],
    "docs/foundation/07-catalog-architecture.md": ["CatalogProvider", "CatalogService", "ProviderResourceRef"],
    "docs/foundation/08-provider-session-architecture.md": ["ProviderSessionSource", "ProviderSessionManager", "non-serializable", "refresh storm"],
    "docs/foundation/09-provider-adapter-architecture.md": ["IntegratedProvider", "ProviderAdapterRuntime", "CatalogProvider", "ResourceResolver"],
    "docs/foundation/10-application-runtime-architecture.md": ["SearchNowBackendRuntime", "one managed state", "ProviderResolvedTransport", "BackendRuntimeSnapshot"],
    "docs/foundation/11-observability-windows-readiness.md": ["DiagnosticsBuffer", "bounded", "Windows RustCore + Tauri compile gate", "icons/icon.png", "icons/icon.ico", "AtomicFileStore"],
}

for rel, needles in checks.items():
    path = ROOT / rel
    if not path.exists():
        continue
    text = path.read_text(encoding="utf-8")
    for needle in needles:
        if needle not in text:
            errors.append(f"{rel}: missing contract text {needle!r}")

active_backend = ROOT / "EngineData" / "Backend" / "RustCore" / "src"
for path in active_backend.rglob("*.rs") if active_backend.exists() else []:
    text = path.read_text(encoding="utf-8", errors="replace")
    for forbidden in ["mc-prod.vercel.app", "ContentKey", "keys.tsv", "decryptEntitlementFile"]:
        if forbidden in text:
            errors.append(f"{path.relative_to(ROOT)}: forbidden legacy protected-content dependency {forbidden!r}")

for rel in [
    "EngineData/Backend/RustCore/src/download/model.rs",
    "EngineData/Backend/RustCore/src/download/store.rs",
    "EngineData/Backend/RustCore/src/catalog/model.rs",
    "EngineData/Backend/RustCore/src/provider_adapter/model.rs",
]:
    path = ROOT / rel
    if not path.exists():
        continue
    lowered = path.read_text(encoding="utf-8", errors="replace").lower()
    for forbidden in ["authorization", "bearer_token", "signed_url", "cookie", "headers", "access_token", "refresh_token"]:
        if forbidden in lowered:
            errors.append(f"{rel}: runtime credential field must not enter persisted/public DTO state: {forbidden!r}")

session_runtime = ROOT / "EngineData/Backend/RustCore/src/provider_session/runtime.rs"
if session_runtime.exists():
    text = session_runtime.read_text(encoding="utf-8", errors="replace")
    for forbidden in ["Serialize", "Deserialize", "#[derive(Debug", "impl std::fmt::Debug for ProviderSessionMaterial", "impl std::fmt::Debug for ProviderSessionLease"]:
        if forbidden in text:
            errors.append(f"{session_runtime.relative_to(ROOT)}: runtime session material must remain non-serializable/non-debug: {forbidden!r}")

commands_root = ROOT / "EngineData/Frontend/RustApp/src-tauri/src/commands"
for name in ["runtime.rs", "settings.rs", "minecraft.rs", "library.rs", "download.rs"]:
    path = commands_root / name
    if not path.exists():
        continue
    text = path.read_text(encoding="utf-8", errors="replace")
    if "SearchNowBackendRuntime" not in text:
        errors.append(f"{path.relative_to(ROOT)}: active Tauri feature command must delegate through SearchNowBackendRuntime")
    for forbidden in ["SettingsStore", "PlatformContext", "DownloadExecutionRuntime", "DownloadTransportRegistry", "ResourceResolverRegistry", "HttpTransport", "ProviderAdapterRuntime", "IntegratedProvider"]:
        if forbidden in text:
            errors.append(f"{path.relative_to(ROOT)}: backend sub-runtime ownership belongs in SearchNowBackendRuntime, not Tauri command: {forbidden!r}")

download_command = commands_root / "download.rs"
if download_command.exists():
    text = download_command.read_text(encoding="utf-8", errors="replace")
    if re.search(r"\bDownloadRequest\b", text):
        errors.append("download command must not expose raw DownloadRequest transport selection")
    if "QueueCatalogDownloadRequest" not in text:
        errors.append("download command must accept provider-neutral QueueCatalogDownloadRequest")

bootstrap = ROOT / "EngineData/Frontend/RustApp/src-tauri/src/app_bootstrap.rs"
if bootstrap.exists():
    text = bootstrap.read_text(encoding="utf-8", errors="replace")
    for needle in ["SearchNowBackendRuntime::new", "app.manage(runtime)"]:
        if needle not in text:
            errors.append(f"{bootstrap.relative_to(ROOT)}: missing consolidated backend bootstrap contract {needle!r}")

app_runtime = ROOT / "EngineData/Backend/RustCore/src/app_runtime.rs"
if app_runtime.exists():
    text = app_runtime.read_text(encoding="utf-8", errors="replace")
    for needle in ["SearchNowBackendRuntime", "ProviderAdapterRuntime::compose", "providers.resolvers()", "ProviderResolvedTransport::new", "DownloadExecutionRuntime::new", "SettingsStore::new", "BackendRuntimeSnapshot", "DiagnosticsBuffer", "QueueCatalogDownloadRequest"]:
        if needle not in text:
            errors.append(f"{app_runtime.relative_to(ROOT)}: missing application composition/observability contract {needle!r}")
    if "DownloadTransportRegistry::with_local_file" in text:
        errors.append(f"{app_runtime.relative_to(ROOT)}: production runtime must not register local-file fixture transport")

storage = ROOT / "EngineData/Backend/RustCore/src/storage.rs"
if storage.exists():
    text = storage.read_text(encoding="utf-8", errors="replace")
    for needle in ["AtomicFileStore", "recover_primary", "sync_all", "cleanup_stale_temps"]:
        if needle not in text:
            errors.append(f"{storage.relative_to(ROOT)}: missing atomic persistence/recovery contract {needle!r}")

build_rs = ROOT / "EngineData/Frontend/RustApp/src-tauri/build.rs"
if build_rs.exists():
    text = build_rs.read_text(encoding="utf-8", errors="replace")
    if "tauri_build::build()" not in text:
        errors.append(f"{build_rs.relative_to(ROOT)}: Tauri build script must use the standard build path")
    for forbidden in ["window_icon_path", "build_placeholder_ico", "OUT_DIR"]:
        if forbidden in text:
            errors.append(f"{build_rs.relative_to(ROOT)}: build-only icon workaround must not return: {forbidden!r}")

shared_workflow_rel = ".github/workflows/shared-verify.yml"
shared_workflow = ROOT / shared_workflow_rel
if shared_workflow.exists():
    text = shared_workflow.read_text(encoding="utf-8", errors="replace")
    for needle in [
        "workflow_call:",
        "windows-latest",
        'node-version: "22"',
        "actions/checkout@v7.0.1",
        "github.event.pull_request.head.sha || github.sha",
        "actions/setup-node@v7.0.0",
        "npm ci --no-audit --no-fund",
        "cargo test --locked --manifest-path EngineData/Backend/RustCore/Cargo.toml",
        "cargo clippy --locked --manifest-path EngineData/Backend/RustCore/Cargo.toml",
        "cargo check --locked --manifest-path EngineData/Frontend/RustApp/src-tauri/Cargo.toml",
    ]:
        if needle not in text:
            errors.append(f"{shared_workflow_rel}: missing deterministic verification contract {needle!r}")
    if "npm install" in text:
        errors.append(f"{shared_workflow_rel}: npm install must not replace the committed package-lock baseline")
    if "contents: write" in text:
        errors.append(f"{shared_workflow_rel}: verification workflow must not retain repository write permission")

workflow_wrapper_checks = {
    ".github/workflows/repository-verify.yml": ["uses: ./.github/workflows/shared-verify.yml"],
    ".github/workflows/local-promotion-verify.yml": [
        "uses: ./.github/workflows/shared-verify.yml",
        'test "${{ github.head_ref }}" = "develop"',
        "needs: promotion-policy",
    ],
    ".github/workflows/release-verify.yml": [
        "uses: ./.github/workflows/shared-verify.yml",
        'test "${{ github.head_ref }}" = "Local"',
        "needs: release-policy",
    ],
}
for workflow_rel, needles in workflow_wrapper_checks.items():
    workflow = ROOT / workflow_rel
    if not workflow.exists():
        continue
    text = workflow.read_text(encoding="utf-8", errors="replace")
    for needle in needles:
        if needle not in text:
            errors.append(f"{workflow_rel}: missing workflow routing contract {needle!r}")
    if "contents: write" in text:
        errors.append(f"{workflow_rel}: verification workflow must not retain repository write permission")

for legacy_old in [
    "docs/01-current-state.md", "docs/04-recovered-source-architecture.md", "docs/05-recovered-symbol-map.md",
    "docs/06-runtime-data-contracts.md", "docs/07-reconstruction-evidence.md",
]:
    if (ROOT / legacy_old).exists():
        errors.append(f"legacy document not normalized into docs/legacy/: {legacy_old}")

if errors:
    for error in errors:
        print(f"ERROR: {error}")
    sys.exit(1)

print("SearchNow repository contracts: PASS")
