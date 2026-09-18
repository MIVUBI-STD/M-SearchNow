export type AppRoute = "library" | "discover" | "downloads" | "settings";

export type ProductError = {
  code: string;
  message: string;
};

export type ProductResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: ProductError };

export type RuntimeStatus = {
  appReady: boolean;
  appVersion: string;
  platform: string;
  architecture: string;
  backend: string;
};

export type MinecraftChannel = "stable" | "preview" | "custom";
export type MinecraftStorageKind = "gdkShared" | "gdkUser" | "legacyUwp" | "customOverride";
export type MinecraftDiscoveryState = "found" | "notFound" | "unsupportedPlatform";

export type MinecraftStorageRoot = {
  id: string;
  channel: MinecraftChannel;
  storageKind: MinecraftStorageKind;
  root: string;
  accountHint: string | null;
};

export type DiscoveryWarning = {
  code: string;
  message: string;
  path: string | null;
};

export type MinecraftDiscoverySnapshot = {
  state: MinecraftDiscoveryState;
  roots: MinecraftStorageRoot[];
  warnings: DiscoveryWarning[];
  checkedCandidates: number;
  message: string;
};

export type LocalContentType = "behaviorPack" | "resourcePack" | "skinPack" | "world";
export type LocalContentStatus = "ready" | "invalidMetadata";

export type LocalContentDependency = {
  uuid: string;
  version: number[];
};

export type LocalContentItem = {
  id: string;
  title: string;
  description: string | null;
  contentType: LocalContentType;
  status: LocalContentStatus;
  issue: string | null;
  path: string;
  rootId: string;
  manifestUuid: string | null;
  version: number[];
  dependencies: LocalContentDependency[];
  isDevelopment: boolean;
};

export type LibrarySummary = {
  total: number;
  behaviorPacks: number;
  resourcePacks: number;
  skinPacks: number;
  worlds: number;
  invalidItems: number;
};

export type LibraryWarning = {
  code: string;
  message: string;
  path: string | null;
};

export type LibrarySnapshot = {
  items: LocalContentItem[];
  warnings: LibraryWarning[];
  summary: LibrarySummary;
  scannedRoots: number;
};

export type LocalBackendSnapshot = {
  minecraft: MinecraftDiscoverySnapshot;
  library: LibrarySnapshot;
};

export type MinecraftSettings = {
  rootOverride: string | null;
  includePreview: boolean;
  includeLegacyUwp: boolean;
  includeDevelopmentContent: boolean;
};

export type AppSettings = {
  schemaVersion: number;
  minecraft: MinecraftSettings;
};

export type DownloadJobState =
  | "queued"
  | "preparing"
  | "transferring"
  | "pauseRequested"
  | "paused"
  | "finalizing"
  | "cancelRequested"
  | "completed"
  | "failed"
  | "cancelled"
  | "interrupted";

export type DownloadSourceRef = {
  transport: string;
  resourceId: string;
};

export type DownloadProgress = {
  downloadedBytes: number;
  totalBytes: number | null;
};

export type DownloadFailure = {
  code: string;
  message: string;
  retryable: boolean;
};

export type DownloadJob = {
  id: string;
  source: DownloadSourceRef;
  displayName: string;
  destinationFileName: string;
  destinationDirectory: string | null;
  state: DownloadJobState;
  progress: DownloadProgress;
  attempt: number;
  lastError: DownloadFailure | null;
  createdAtMs: number;
  updatedAtMs: number;
};

export type DownloadPolicy = {
  maxActive: number;
  maxJobs: number;
};

export type DownloadManagerSnapshot = {
  policy: DownloadPolicy;
  jobs: DownloadJob[];
  activeJobs: number;
  queuedJobs: number;
  schedulerError: DownloadFailure | null;
};

export type ProviderSessionState = "unavailable" | "available" | "expired" | "refreshing" | "failed";

export type ProviderSessionStatus = {
  provider: string;
  state: ProviderSessionState;
  expiresAtMs: number | null;
  failureCode: string | null;
  retryable: boolean;
};

export type ProviderCapabilities = {
  provider: string;
  session: boolean;
  catalog: boolean;
  resolvedDownload: boolean;
};

export type ProviderRuntimeStatus = {
  capabilities: ProviderCapabilities;
  session: ProviderSessionStatus | null;
};

export type DiagnosticSeverity = "info" | "warning" | "error";
export type DiagnosticComponent =
  | "runtime"
  | "settings"
  | "minecraft"
  | "library"
  | "catalog"
  | "provider"
  | "download";
export type BackendStartupPhase = "starting" | "ready" | "unknown";
export type BackendHealthState = "healthy" | "degraded" | "unknown";

export type DiagnosticEvent = {
  timestampMs: number;
  component: DiagnosticComponent;
  severity: DiagnosticSeverity;
  code: string;
  message: string;
  durationMs: number | null;
};

export type BackendHealthSnapshot = {
  startupPhase: BackendStartupPhase;
  state: BackendHealthState;
  diagnosticsAvailable: boolean;
  retainedEvents: number;
  droppedEvents: number;
  warningEvents: number;
  errorEvents: number;
  lastCode: string | null;
};

export type BackendDiagnosticsSnapshot = {
  health: BackendHealthSnapshot;
  events: DiagnosticEvent[];
};

export type BackendRuntimeSnapshot = {
  runtime: RuntimeStatus;
  minecraft: MinecraftDiscoverySnapshot;
  providers: ProviderRuntimeStatus[];
  downloads: DownloadManagerSnapshot;
  diagnostics: BackendDiagnosticsSnapshot;
};

export type ProductRuntimeSnapshot = {
  ready: boolean;
  summary: string;
  runtime: RuntimeStatus | null;
  backend: BackendRuntimeSnapshot | null;
  error: ProductError | null;
};

export type CatalogContentType = "world" | "addon" | "resourcePack" | "skin" | "persona" | "other";
export type CatalogSort = "relevance" | "newest" | "oldest" | "nameAsc" | "nameDesc";

export type CatalogDownloadRef =
  | { kind: "publicHttps"; url: string }
  | { kind: "providerResolved"; provider: string; resourceId: string };

export type QueueCatalogDownloadRequest = {
  download: CatalogDownloadRef;
  displayName: string;
  destinationFileName: string;
  destinationDirectory: string | null;
  expectedBytes: number | null;
};

export type CatalogItem = {
  provider: string;
  itemId: string;
  title: string;
  creatorName: string | null;
  thumbnailUrl: string | null;
  description: string | null;
  contentType: CatalogContentType;
  tags: string[];
  publishedAtMs: number | null;
  updatedAtMs: number | null;
  fileName: string | null;
  expectedBytes: number | null;
  download: CatalogDownloadRef | null;
};

export type CatalogPage = {
  provider: string;
  items: CatalogItem[];
  nextCursor: string | null;
};

export type CatalogRequest = {
  provider: string;
  query: {
    text: string | null;
    filters: {
      contentTypes: CatalogContentType[];
      tags: string[];
    };
    sort: CatalogSort;
    page: {
      limit: number;
      cursor: string | null;
    };
  };
};

export type PackageInputKind = "folder" | "mcPack" | "mcAddon" | "mcWorld";
export type PackageSafety = "safe" | "rejected";
export type PackageInspectionStatus = "ready" | "issues" | "rejected";
export type PackageIssueSeverity = "warning" | "error";
export type PackKind = "behaviorPack" | "resourcePack" | "skinPack" | "worldTemplate" | "mixed" | "unknown";
export type PackageRelationshipKind = "behaviorRequiresResource" | "resourceRequiresBehavior" | "packDependency";

export type PackageIssue = {
  severity: PackageIssueSeverity;
  code: string;
  message: string;
  path: string | null;
};

export type PackDependency = {
  uuid: string | null;
  moduleName: string | null;
  version: string | null;
};

export type PackManifestSummary = {
  manifestPath: string;
  packRoot: string;
  name: string;
  description: string | null;
  uuid: string | null;
  version: string | null;
  formatVersion: string | null;
  kind: PackKind;
  moduleTypes: string[];
  dependencies: PackDependency[];
  hasScripts: boolean;
};

export type PackageRelationship = {
  sourceManifest: string;
  targetManifest: string;
  dependencyUuid: string;
  kind: PackageRelationshipKind;
};

export type ArchiveSummary = {
  entries: number;
  files: number;
  directories: number;
  compressedBytes: number;
  uncompressedBytes: number;
  nestedArchives: number;
};

export type WorldSummary = {
  worldRoot: string;
  name: string;
  hasLevelDat: boolean;
  hasLevelName: boolean;
};

export type PackageInspection = {
  sourcePath: string;
  inputKind: PackageInputKind;
  status: PackageInspectionStatus;
  safety: PackageSafety;
  world: WorldSummary | null;
  packs: PackManifestSummary[];
  relationships: PackageRelationship[];
  issues: PackageIssue[];
  archive: ArchiveSummary | null;
};

export type PackageImportRequest = {
  sourcePath: string;
  rootId: string;
};

export type ImportedPack = {
  name: string;
  kind: PackKind;
  destinationPath: string;
};

export type ImportedWorld = {
  name: string;
  destinationPath: string;
};

export type PackageImportResult = {
  sourcePath: string;
  rootId: string;
  imported: ImportedPack[];
  world: ImportedWorld | null;
};

export type PackageReplaceRequest = {
  sourcePath: string;
  rootId: string;
};
