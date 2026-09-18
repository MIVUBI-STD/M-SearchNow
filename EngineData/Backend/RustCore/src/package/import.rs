use super::{
    archive::{MAX_SINGLE_ENTRY_BYTES, MAX_TOTAL_UNCOMPRESSED_BYTES},
    inspect_package,
    model::{
        ImportedPack, PackKind, PackManifestSummary, PackageImportResult, PackageInspectionStatus,
        PackageInputKind, PackageSafety,
    },
};
use crate::error::{BackendError, BackendResult};
use std::{
    collections::HashSet,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

pub(crate) fn import_archive(
    source: &Path,
    minecraft_root: &Path,
    root_id: &str,
) -> BackendResult<PackageImportResult> {
    let inspection = inspect_package(source)?;
    if inspection.safety != PackageSafety::Safe
        || inspection.status != PackageInspectionStatus::Ready
    {
        return Err(BackendError::new(
            "package_import_not_ready",
            "Only packages that pass inspection without findings can be imported.",
        ));
    }
    if !matches!(
        inspection.input_kind,
        PackageInputKind::McPack | PackageInputKind::McAddon
    ) {
        return Err(BackendError::new(
            "package_import_input_unsupported",
            "Safe import currently supports .mcpack and .mcaddon archives.",
        ));
    }

    let mut plans = Vec::new();
    let mut reserved = HashSet::new();
    for pack in &inspection.packs {
        let container = container_for(pack.kind).ok_or_else(|| {
            BackendError::new(
                "package_import_kind_unsupported",
                "This package contains a pack type that SearchNow does not import automatically.",
            )
        })?;
        let container_root = minecraft_root.join(container);
        fs::create_dir_all(&container_root).map_err(|error| {
            BackendError::from_io(
                "package_import_destination_failed",
                "SearchNow could not prepare the Minecraft destination folder.",
                error,
            )
        })?;
        let base_name = destination_name(&pack.name, pack.uuid.as_deref());
        let destination = next_available_destination(&container_root, &base_name, &mut reserved)?;
        plans.push((pack.clone(), destination));
    }

    let staging_root = minecraft_root.join(format!(
        ".searchnow-import-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    fs::create_dir(&staging_root).map_err(|error| {
        BackendError::from_io(
            "package_import_staging_failed",
            "SearchNow could not create a temporary import workspace.",
            error,
        )
    })?;

    let result = (|| {
        extract_plans(source, &staging_root, &plans)?;
        let mut imported = Vec::new();
        for (index, (pack, destination)) in plans.iter().enumerate() {
            let staged = staging_root.join(index.to_string());
            fs::rename(&staged, destination).map_err(|error| {
                BackendError::from_io(
                    "package_import_commit_failed",
                    "SearchNow could not move an inspected pack into Minecraft storage.",
                    error,
                )
            })?;
            imported.push(ImportedPack {
                name: pack.name.clone(),
                kind: pack.kind,
                destination_path: destination.clone(),
            });
        }
        Ok(PackageImportResult {
            source_path: source.to_path_buf(),
            root_id: root_id.to_string(),
            imported,
        })
    })();

    let _ = fs::remove_dir_all(&staging_root);
    result
}

fn extract_plans(
    source: &Path,
    staging_root: &Path,
    plans: &[(PackManifestSummary, PathBuf)],
) -> BackendResult<()> {
    let file = File::open(source).map_err(|error| {
        BackendError::from_io(
            "package_import_open_failed",
            "SearchNow could not reopen the inspected package.",
            error,
        )
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        BackendError::new(
            "package_import_archive_invalid",
            format!("The package archive changed or is no longer readable: {error}"),
        )
    })?;
    let mut total = 0_u64;
    let mut written = HashSet::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            BackendError::new(
                "package_import_entry_invalid",
                format!("SearchNow could not read archive entry {index}: {error}"),
            )
        })?;
        if is_symlink(entry.unix_mode()) {
            return Err(BackendError::new(
                "package_import_symlink_rejected",
                "Package import rejected a symbolic-link archive entry.",
            ));
        }
        let safe_path = entry.enclosed_name().ok_or_else(|| {
            BackendError::new(
                "package_import_path_rejected",
                "Package import rejected an archive path that could escape its destination.",
            )
        })?;
        if entry.size() > MAX_SINGLE_ENTRY_BYTES {
            return Err(BackendError::new(
                "package_import_entry_too_large",
                "Package import rejected an oversized archive entry.",
            ));
        }
        total = total.saturating_add(entry.size());
        if total > MAX_TOTAL_UNCOMPRESSED_BYTES {
            return Err(BackendError::new(
                "package_import_uncompressed_limit",
                "Package import exceeded the uncompressed-size safety limit.",
            ));
        }

        for (plan_index, (pack, _)) in plans.iter().enumerate() {
            let Some(relative) = relative_to_pack(&safe_path, &pack.pack_root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            let target = staging_root.join(plan_index.to_string()).join(relative);
            if !written.insert(target.clone()) {
                return Err(BackendError::new(
                    "package_import_duplicate_path",
                    "Package import rejected duplicate archive paths.",
                ));
            }
            if entry.is_dir() {
                fs::create_dir_all(&target).map_err(|error| {
                    BackendError::from_io(
                        "package_import_write_failed",
                        "SearchNow could not create an imported package folder.",
                        error,
                    )
                })?;
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        BackendError::from_io(
                            "package_import_write_failed",
                            "SearchNow could not prepare an imported package folder.",
                            error,
                        )
                    })?;
                }
                let mut output = File::create(&target).map_err(|error| {
                    BackendError::from_io(
                        "package_import_write_failed",
                        "SearchNow could not create an imported package file.",
                        error,
                    )
                })?;
                io::copy(&mut entry, &mut output).map_err(|error| {
                    BackendError::from_io(
                        "package_import_write_failed",
                        "SearchNow could not extract an inspected package file.",
                        error,
                    )
                })?;
            }
            break;
        }
    }

    for (index, _) in plans.iter().enumerate() {
        if !staging_root.join(index.to_string()).is_dir() {
            return Err(BackendError::new(
                "package_import_pack_missing",
                "The inspected package contents no longer match the archive being imported.",
            ));
        }
    }
    Ok(())
}

fn relative_to_pack(path: &Path, pack_root: &str) -> Option<PathBuf> {
    if pack_root == "." {
        return Some(path.to_path_buf());
    }
    path.strip_prefix(Path::new(pack_root))
        .ok()
        .map(Path::to_path_buf)
}

fn container_for(kind: PackKind) -> Option<&'static str> {
    match kind {
        PackKind::BehaviorPack => Some("behavior_packs"),
        PackKind::ResourcePack => Some("resource_packs"),
        PackKind::SkinPack => Some("skin_packs"),
        PackKind::WorldTemplate | PackKind::Mixed | PackKind::Unknown => None,
    }
}

fn destination_name(name: &str, uuid: Option<&str>) -> String {
    let mut safe = String::with_capacity(name.len());
    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.') {
            safe.push(character);
        } else {
            safe.push('_');
        }
    }
    let safe = safe.trim_matches([' ', '.']).trim();
    let base = if safe.is_empty() { "Imported pack" } else { safe };
    let suffix = uuid
        .and_then(|value| value.split('-').next())
        .filter(|value| !value.is_empty())
        .unwrap_or("pack");
    format!("{base} [{suffix}]")
}

fn next_available_destination(
    container: &Path,
    base_name: &str,
    reserved: &mut HashSet<PathBuf>,
) -> BackendResult<PathBuf> {
    for sequence in 1_u32..=u32::MAX {
        let name = if sequence == 1 {
            base_name.to_string()
        } else {
            format!("{base_name} ({sequence})")
        };
        let candidate = container.join(name);
        if !candidate.exists() && reserved.insert(candidate.clone()) {
            return Ok(candidate);
        }
    }
    Err(BackendError::new(
        "package_import_destination_exhausted",
        "SearchNow could not allocate a unique package destination folder.",
    ))
}

fn is_symlink(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170000 == 0o120000)
}
