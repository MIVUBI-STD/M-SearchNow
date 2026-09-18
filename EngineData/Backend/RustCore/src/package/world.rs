use super::{
    model::{PackageIssue, WorldSummary},
    MAX_MANIFEST_BYTES,
};
use crate::error::{BackendError, BackendResult};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

const MAX_LEVEL_NAME_BYTES: u64 = 4 * 1024;

pub(crate) fn inspect_world_archive(
    path: &Path,
    issues: &mut Vec<PackageIssue>,
) -> BackendResult<Option<WorldSummary>> {
    let file = File::open(path).map_err(|error| {
        BackendError::from_io(
            "world_archive_open_failed",
            "SearchNow could not reopen the world archive.",
            error,
        )
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        BackendError::new(
            "world_archive_invalid",
            format!("World package is not a readable ZIP archive: {error}"),
        )
    })?;

    let mut roots = Vec::<PathBuf>::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            BackendError::new(
                "world_archive_entry_invalid",
                format!("SearchNow could not inspect world archive entry {index}: {error}"),
            )
        })?;
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("level.dat"))
        {
            roots.push(
                path.parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from(".")),
            );
        }
    }
    roots.sort();
    roots.dedup();

    if roots.is_empty() {
        issues.push(PackageIssue::error(
            "world_level_dat_missing",
            "No level.dat was found in this .mcworld package.",
            None,
        ));
        return Ok(None);
    }
    if roots.len() > 1 {
        issues.push(PackageIssue::error(
            "world_root_ambiguous",
            "The .mcworld contains more than one possible world root.",
            None,
        ));
        return Ok(None);
    }

    let root = roots.remove(0);
    let level_name_path = if root == Path::new(".") {
        PathBuf::from("levelname.txt")
    } else {
        root.join("levelname.txt")
    };
    let name = read_small_entry(&mut archive, &level_name_path)
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("Imported world")
                .to_string()
        });

    let level_name_key = level_name_path.to_string_lossy().replace('\\', "/");
    let has_level_name = archive.by_name(&level_name_key).is_ok();

    Ok(Some(WorldSummary {
        world_root: if root == Path::new(".") {
            ".".into()
        } else {
            root.to_string_lossy().replace('\\', "/")
        },
        name,
        has_level_dat: true,
        has_level_name,
    }))
}

fn read_small_entry(archive: &mut ZipArchive<File>, path: &Path) -> Option<String> {
    let name = path.to_string_lossy().replace('\\', "/");
    let mut entry = archive.by_name(&name).ok()?;
    if entry.size() > MAX_LEVEL_NAME_BYTES.min(MAX_MANIFEST_BYTES) {
        return None;
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes).ok()?;
    String::from_utf8(bytes).ok()
}
