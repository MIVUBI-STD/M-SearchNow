use crate::minecraft::MinecraftStorageRoot;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_ITEMS_PER_CONTAINER: usize = 5_000;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_LEVEL_NAME_BYTES: u64 = 4 * 1024;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalContentType {
    BehaviorPack,
    ResourcePack,
    SkinPack,
    World,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalContentStatus {
    Ready,
    InvalidMetadata,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalContentItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub content_type: LocalContentType,
    pub status: LocalContentStatus,
    pub issue: Option<String>,
    pub path: PathBuf,
    pub root_id: String,
    pub manifest_uuid: Option<String>,
    pub version: Vec<u32>,
    pub is_development: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySummary {
    pub total: usize,
    pub behavior_packs: usize,
    pub resource_packs: usize,
    pub skin_packs: usize,
    pub worlds: usize,
    pub invalid_items: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryWarning {
    pub code: String,
    pub message: String,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub items: Vec<LocalContentItem>,
    pub warnings: Vec<LibraryWarning>,
    pub summary: LibrarySummary,
    pub scanned_roots: usize,
}

pub fn scan_library(
    roots: &[MinecraftStorageRoot],
    include_development_content: bool,
) -> LibrarySnapshot {
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    for root in roots {
        for spec in container_specs(include_development_content) {
            scan_container(root, spec, &mut items, &mut warnings);
        }
    }
    items.sort_by(|left, right| {
        left.title
            .to_lowercase()
            .cmp(&right.title.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    let summary = summarize(&items);
    LibrarySnapshot {
        items,
        warnings,
        summary,
        scanned_roots: roots.len(),
    }
}

#[derive(Clone, Copy)]
struct ContainerSpec {
    folder: &'static str,
    content_type: LocalContentType,
    development: bool,
}

fn container_specs(include_development_content: bool) -> Vec<ContainerSpec> {
    let mut specs = vec![
        ContainerSpec {
            folder: "behavior_packs",
            content_type: LocalContentType::BehaviorPack,
            development: false,
        },
        ContainerSpec {
            folder: "resource_packs",
            content_type: LocalContentType::ResourcePack,
            development: false,
        },
        ContainerSpec {
            folder: "skin_packs",
            content_type: LocalContentType::SkinPack,
            development: false,
        },
        ContainerSpec {
            folder: "minecraftWorlds",
            content_type: LocalContentType::World,
            development: false,
        },
    ];
    if include_development_content {
        specs.extend([
            ContainerSpec {
                folder: "development_behavior_packs",
                content_type: LocalContentType::BehaviorPack,
                development: true,
            },
            ContainerSpec {
                folder: "development_resource_packs",
                content_type: LocalContentType::ResourcePack,
                development: true,
            },
            ContainerSpec {
                folder: "development_skin_packs",
                content_type: LocalContentType::SkinPack,
                development: true,
            },
        ]);
    }
    specs
}

fn scan_container(
    root: &MinecraftStorageRoot,
    spec: ContainerSpec,
    items: &mut Vec<LocalContentItem>,
    warnings: &mut Vec<LibraryWarning>,
) {
    let container = root.root.join(spec.folder);
    if !container.is_dir() {
        return;
    }
    let entries = match fs::read_dir(&container) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(LibraryWarning {
                code: "library_container_read_failed".into(),
                message: format!("Could not read {}: {error}", spec.folder),
                path: Some(container),
            });
            return;
        }
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());
    if entries.len() > MAX_ITEMS_PER_CONTAINER {
        warnings.push(LibraryWarning {
            code: "library_container_limit".into(),
            message: format!(
                "Only the first {MAX_ITEMS_PER_CONTAINER} items in {} were indexed.",
                spec.folder
            ),
            path: Some(container.clone()),
        });
        entries.truncate(MAX_ITEMS_PER_CONTAINER);
    }
    for entry in entries {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        items.push(index_item(root, &entry.path(), spec));
    }
}

fn index_item(root: &MinecraftStorageRoot, path: &Path, spec: ContainerSpec) -> LocalContentItem {
    if spec.content_type == LocalContentType::World {
        let title = read_small_text(&path.join("levelname.txt"), MAX_LEVEL_NAME_BYTES)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| folder_name(path));
        return LocalContentItem {
            id: item_id(path),
            title: title.trim().to_string(),
            description: None,
            content_type: spec.content_type,
            status: LocalContentStatus::Ready,
            issue: None,
            path: path.to_path_buf(),
            root_id: root.id.clone(),
            manifest_uuid: None,
            version: Vec::new(),
            is_development: spec.development,
        };
    }

    match read_manifest(&path.join("manifest.json")) {
        Ok(manifest) => LocalContentItem {
            id: item_id(path),
            title: manifest.header.name.unwrap_or_else(|| folder_name(path)),
            description: manifest.header.description,
            content_type: spec.content_type,
            status: LocalContentStatus::Ready,
            issue: None,
            path: path.to_path_buf(),
            root_id: root.id.clone(),
            manifest_uuid: manifest.header.uuid,
            version: manifest.header.version,
            is_development: spec.development,
        },
        Err(issue) => LocalContentItem {
            id: item_id(path),
            title: folder_name(path),
            description: None,
            content_type: spec.content_type,
            status: LocalContentStatus::InvalidMetadata,
            issue: Some(issue),
            path: path.to_path_buf(),
            root_id: root.id.clone(),
            manifest_uuid: None,
            version: Vec::new(),
            is_development: spec.development,
        },
    }
}

#[derive(Debug, Deserialize)]
struct ManifestDocument {
    header: ManifestHeader,
}

#[derive(Debug, Deserialize)]
struct ManifestHeader {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    uuid: Option<String>,
    #[serde(default)]
    version: Vec<u32>,
}

fn read_manifest(path: &Path) -> Result<ManifestDocument, String> {
    let metadata = fs::metadata(path).map_err(|_| "manifest.json is missing.".to_string())?;
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err("manifest.json is larger than the supported metadata limit.".into());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("manifest.json could not be read: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("manifest.json is invalid: {error}"))
}

fn read_small_text(path: &Path, limit: u64) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > limit {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".into())
}

pub(crate) fn valid_local_content_id(value: &str) -> bool {
    value
        .strip_prefix("local-")
        .is_some_and(|hash| hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn item_id(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let mut hash = 0xcbf29ce484222325u64;
    for byte in normalized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("local-{hash:016x}")
}

fn summarize(items: &[LocalContentItem]) -> LibrarySummary {
    let mut summary = LibrarySummary {
        total: items.len(),
        ..LibrarySummary::default()
    };
    for item in items {
        match item.content_type {
            LocalContentType::BehaviorPack => summary.behavior_packs += 1,
            LocalContentType::ResourcePack => summary.resource_packs += 1,
            LocalContentType::SkinPack => summary.skin_packs += 1,
            LocalContentType::World => summary.worlds += 1,
        }
        if item.status != LocalContentStatus::Ready {
            summary.invalid_items += 1;
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::{MinecraftChannel, MinecraftStorageKind};

    fn root(path: &Path) -> MinecraftStorageRoot {
        MinecraftStorageRoot {
            id: "fixture-root".into(),
            channel: MinecraftChannel::Stable,
            storage_kind: MinecraftStorageKind::GdkShared,
            root: path.to_path_buf(),
            account_hint: None,
        }
    }

    #[test]
    fn indexes_manifest_pack_and_world() {
        let directory = tempfile::tempdir().expect("tempdir");
        let pack = directory.path().join("resource_packs/example");
        fs::create_dir_all(&pack).expect("pack");
        fs::write(pack.join("manifest.json"), r#"{"header":{"name":"Example Pack","description":"Fixture","uuid":"abc","version":[1,2,3]}}"#).expect("manifest");
        let world = directory.path().join("minecraftWorlds/world-one");
        fs::create_dir_all(&world).expect("world");
        fs::write(world.join("levelname.txt"), "My World").expect("level name");
        let snapshot = scan_library(&[root(directory.path())], false);
        assert_eq!(snapshot.summary.total, 2);
        assert!(snapshot
            .items
            .iter()
            .any(|item| item.title == "Example Pack"));
        assert!(snapshot.items.iter().any(|item| item.title == "My World"));
    }

    #[test]
    fn invalid_manifest_is_truthful_not_fatal() {
        let directory = tempfile::tempdir().expect("tempdir");
        let pack = directory.path().join("behavior_packs/broken");
        fs::create_dir_all(&pack).expect("pack");
        fs::write(pack.join("manifest.json"), "not-json").expect("manifest");
        let snapshot = scan_library(&[root(directory.path())], false);
        assert_eq!(snapshot.summary.invalid_items, 1);
        assert_eq!(
            snapshot.items[0].status,
            LocalContentStatus::InvalidMetadata
        );
    }

    #[test]
    fn development_content_is_opt_in() {
        let directory = tempfile::tempdir().expect("tempdir");
        let pack = directory.path().join("development_resource_packs/dev");
        fs::create_dir_all(&pack).expect("pack");
        fs::write(
            pack.join("manifest.json"),
            r#"{"header":{"name":"Dev","version":[1,0,0]}}"#,
        )
        .expect("manifest");
        assert_eq!(
            scan_library(&[root(directory.path())], false).summary.total,
            0
        );
        assert_eq!(
            scan_library(&[root(directory.path())], true).summary.total,
            1
        );
    }

    #[test]
    fn oversized_manifest_is_reported_without_reading_unbounded_metadata() {
        let directory = tempfile::tempdir().expect("tempdir");
        let pack = directory.path().join("resource_packs/oversized");
        fs::create_dir_all(&pack).expect("pack");
        fs::write(
            pack.join("manifest.json"),
            vec![b'x'; (MAX_MANIFEST_BYTES + 1) as usize],
        )
        .expect("manifest");

        let snapshot = scan_library(&[root(directory.path())], false);
        assert_eq!(snapshot.summary.total, 1);
        assert_eq!(snapshot.summary.invalid_items, 1);
        assert_eq!(
            snapshot.items[0].status,
            LocalContentStatus::InvalidMetadata
        );
        assert_eq!(
            snapshot.items[0].issue.as_deref(),
            Some("manifest.json is larger than the supported metadata limit.")
        );
    }

    #[test]
    fn oversized_world_name_falls_back_to_folder_name() {
        let directory = tempfile::tempdir().expect("tempdir");
        let world = directory.path().join("minecraftWorlds/world-folder");
        fs::create_dir_all(&world).expect("world");
        fs::write(
            world.join("levelname.txt"),
            vec![b'w'; (MAX_LEVEL_NAME_BYTES + 1) as usize],
        )
        .expect("level name");

        let snapshot = scan_library(&[root(directory.path())], false);
        assert_eq!(snapshot.summary.total, 1);
        assert_eq!(snapshot.items[0].title, "world-folder");
        assert_eq!(snapshot.items[0].status, LocalContentStatus::Ready);
    }

    #[test]
    fn item_identity_preserves_case_to_avoid_case_sensitive_collisions() {
        let upper = item_id(Path::new("/minecraft/resource_packs/Pack"));
        let lower = item_id(Path::new("/minecraft/resource_packs/pack"));
        assert_ne!(upper, lower);
    }

    #[test]
    fn local_content_id_contract_rejects_malformed_values() {
        assert!(valid_local_content_id("local-0123456789abcdef"));
        assert!(!valid_local_content_id("local-0123456789abcde"));
        assert!(!valid_local_content_id("local-0123456789abcdeg"));
        assert!(!valid_local_content_id("not-local-0123456789abcdef"));
    }
}
