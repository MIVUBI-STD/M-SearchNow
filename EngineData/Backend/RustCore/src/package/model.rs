use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackageInputKind {
    Folder,
    McPack,
    McAddon,
    McWorld,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackageSafety {
    Safe,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackageInspectionStatus {
    Ready,
    Issues,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackageIssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackKind {
    BehaviorPack,
    ResourcePack,
    SkinPack,
    WorldTemplate,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageIssue {
    pub severity: PackageIssueSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl PackageIssue {
    pub(crate) fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            severity: PackageIssueSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path,
        }
    }

    pub(crate) fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            severity: PackageIssueSeverity::Error,
            code: code.into(),
            message: message.into(),
            path,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackDependency {
    pub uuid: Option<String>,
    pub module_name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackManifestSummary {
    pub manifest_path: String,
    pub pack_root: String,
    pub name: String,
    pub description: Option<String>,
    pub uuid: Option<String>,
    pub version: Option<String>,
    pub format_version: Option<String>,
    pub kind: PackKind,
    pub module_types: Vec<String>,
    pub dependencies: Vec<PackDependency>,
    pub has_scripts: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackageRelationshipKind {
    BehaviorRequiresResource,
    ResourceRequiresBehavior,
    PackDependency,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageRelationship {
    pub source_manifest: String,
    pub target_manifest: String,
    pub dependency_uuid: String,
    pub kind: PackageRelationshipKind,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSummary {
    pub entries: usize,
    pub files: usize,
    pub directories: usize,
    pub compressed_bytes: u64,
    pub uncompressed_bytes: u64,
    pub nested_archives: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorldSummary {
    pub world_root: String,
    pub name: String,
    pub has_level_dat: bool,
    pub has_level_name: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageInspection {
    pub source_path: PathBuf,
    pub input_kind: PackageInputKind,
    pub status: PackageInspectionStatus,
    pub safety: PackageSafety,
    pub world: Option<WorldSummary>,
    pub packs: Vec<PackManifestSummary>,
    pub relationships: Vec<PackageRelationship>,
    pub issues: Vec<PackageIssue>,
    pub archive: Option<ArchiveSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageImportRequest {
    pub source_path: PathBuf,
    pub root_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportedPack {
    pub name: String,
    pub kind: PackKind,
    pub destination_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageImportResult {
    pub source_path: PathBuf,
    pub root_id: String,
    pub imported: Vec<ImportedPack>,
}
