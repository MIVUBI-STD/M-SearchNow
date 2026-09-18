mod archive;
mod folder;
mod import;
mod manifest;
pub mod model;

use crate::error::{BackendError, BackendResult};
use manifest::ManifestCandidate;
pub use model::*;
use std::{collections::HashMap, fs, path::Path};

pub(crate) const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

pub fn inspect_package(path: &Path) -> BackendResult<PackageInspection> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        BackendError::from_io(
            "package_input_metadata_failed",
            "SearchNow could not inspect the selected package path.",
            error,
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(BackendError::new(
            "package_input_symlink_rejected",
            "Symlink package inputs are not inspected.",
        ));
    }

    let (input_kind, candidates, mut issues, safety, archive) = if metadata.is_dir() {
        let scan = folder::scan_folder(path);
        (
            PackageInputKind::Folder,
            scan.candidates,
            scan.issues,
            PackageSafety::Safe,
            None,
        )
    } else if metadata.is_file() {
        let input_kind = archive_kind(path)?;
        let scan = archive::scan_archive(path)?;
        (
            input_kind,
            scan.candidates,
            scan.issues,
            scan.safety,
            Some(scan.summary),
        )
    } else {
        return Err(BackendError::new(
            "package_input_type_unsupported",
            "Selected package path is not a regular file or directory.",
        ));
    };

    let mut packs = parse_manifests(candidates, &mut issues);
    packs.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
    validate_bundle(input_kind, &packs, &mut issues);
    validate_unique_uuids(&packs, &mut issues);
    let relationships = detect_relationships(&packs);

    let status = if safety == PackageSafety::Rejected {
        PackageInspectionStatus::Rejected
    } else if issues.is_empty() {
        PackageInspectionStatus::Ready
    } else {
        PackageInspectionStatus::Issues
    };

    Ok(PackageInspection {
        source_path: path.to_path_buf(),
        input_kind,
        status,
        safety,
        packs,
        relationships,
        issues,
        archive,
    })
}

fn archive_kind(path: &Path) -> BackendResult<PackageInputKind> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mcpack") => Ok(PackageInputKind::McPack),
        Some("mcaddon") => Ok(PackageInputKind::McAddon),
        _ => Err(BackendError::new(
            "package_extension_unsupported",
            "Package inspector currently accepts folders, .mcpack, and .mcaddon inputs only.",
        )),
    }
}

fn parse_manifests(
    candidates: Vec<ManifestCandidate>,
    issues: &mut Vec<PackageIssue>,
) -> Vec<PackManifestSummary> {
    let mut packs = Vec::new();
    for candidate in candidates {
        let path = candidate.manifest_path.clone();
        match manifest::inspect_manifest(candidate) {
            Ok((pack, manifest_issues)) => {
                packs.push(pack);
                issues.extend(manifest_issues);
            }
            Err(message) => {
                issues.push(PackageIssue::error("manifest_invalid", message, Some(path)))
            }
        }
    }
    packs
}

fn validate_bundle(
    input_kind: PackageInputKind,
    packs: &[PackManifestSummary],
    issues: &mut Vec<PackageIssue>,
) {
    if packs.is_empty() {
        issues.push(PackageIssue::error(
            "package_manifest_missing",
            "No inspectable manifest.json was found in the selected package.",
            None,
        ));
        return;
    }
    if input_kind == PackageInputKind::McPack && packs.len() > 1 {
        issues.push(PackageIssue::warning(
            "mcpack_multiple_manifests",
            "The .mcpack contains multiple pack manifests; it may be an add-on bundle with the wrong extension.",
            None,
        ));
    }
}

fn validate_unique_uuids(packs: &[PackManifestSummary], issues: &mut Vec<PackageIssue>) {
    let mut seen: HashMap<String, &str> = HashMap::new();
    for pack in packs {
        let Some(uuid) = pack.uuid.as_deref() else {
            continue;
        };
        let key = uuid.to_ascii_lowercase();
        if let Some(existing) = seen.insert(key, &pack.manifest_path) {
            issues.push(PackageIssue::error(
                "package_duplicate_uuid",
                format!(
                    "Pack UUID {uuid} is declared by both {existing} and {}.",
                    pack.manifest_path
                ),
                Some(pack.manifest_path.clone()),
            ));
        }
    }
}

fn detect_relationships(packs: &[PackManifestSummary]) -> Vec<PackageRelationship> {
    let by_uuid: HashMap<String, &PackManifestSummary> = packs
        .iter()
        .filter_map(|pack| {
            pack.uuid
                .as_ref()
                .map(|uuid| (uuid.to_ascii_lowercase(), pack))
        })
        .collect();
    let mut relationships = Vec::new();

    for source in packs {
        for dependency in &source.dependencies {
            let Some(uuid) = dependency.uuid.as_deref() else {
                continue;
            };
            let Some(target) = by_uuid.get(&uuid.to_ascii_lowercase()) else {
                continue;
            };
            let kind = match (source.kind, target.kind) {
                (PackKind::BehaviorPack, PackKind::ResourcePack) => {
                    PackageRelationshipKind::BehaviorRequiresResource
                }
                (PackKind::ResourcePack, PackKind::BehaviorPack) => {
                    PackageRelationshipKind::ResourceRequiresBehavior
                }
                _ => PackageRelationshipKind::PackDependency,
            };
            relationships.push(PackageRelationship {
                source_manifest: source.manifest_path.clone(),
                target_manifest: target.manifest_path.clone(),
                dependency_uuid: uuid.to_string(),
                kind,
            });
        }
    }
    relationships.sort_by(|left, right| {
        left.source_manifest
            .cmp(&right.source_manifest)
            .then_with(|| left.target_manifest.cmp(&right.target_manifest))
    });
    relationships.dedup();
    relationships
}

#[cfg(test)]
mod tests;

pub(crate) use import::import_archive;
