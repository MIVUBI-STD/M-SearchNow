use super::{
    manifest::ManifestCandidate,
    model::{ArchiveSummary, PackageIssue, PackageSafety},
    MAX_MANIFEST_BYTES,
};
use crate::error::{BackendError, BackendResult};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Read,
    path::Path,
};
use zip::ZipArchive;

const MAX_ARCHIVE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 20_000;
pub(super) const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub(super) const MAX_SINGLE_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const SUSPICIOUS_RATIO_MIN_BYTES: u64 = 512 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 2_000;
const MAX_MANIFEST_COMPONENTS: usize = 3;

pub(crate) struct ArchiveScan {
    pub candidates: Vec<ManifestCandidate>,
    pub issues: Vec<PackageIssue>,
    pub safety: PackageSafety,
    pub summary: ArchiveSummary,
}

pub(crate) fn scan_archive(path: &Path) -> BackendResult<ArchiveScan> {
    let metadata = fs::metadata(path).map_err(|error| {
        BackendError::from_io(
            "package_archive_metadata_failed",
            "SearchNow could not inspect the package archive.",
            error,
        )
    })?;
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Ok(rejected_scan(
            "archive_file_too_large",
            "Archive is larger than the current package inspection limit.",
        ));
    }

    let file = File::open(path).map_err(|error| {
        BackendError::from_io(
            "package_archive_open_failed",
            "SearchNow could not open the package archive.",
            error,
        )
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        BackendError::new(
            "package_archive_invalid",
            format!("Package is not a readable ZIP archive: {error}"),
        )
    })?;

    if archive.len() > MAX_ARCHIVE_ENTRIES {
        let mut scan = rejected_scan(
            "archive_entry_limit",
            format!("Archive contains more than {MAX_ARCHIVE_ENTRIES} entries."),
        );
        scan.summary.entries = archive.len();
        return Ok(scan);
    }

    let mut scan = ArchiveScan {
        candidates: Vec::new(),
        issues: Vec::new(),
        safety: PackageSafety::Safe,
        summary: ArchiveSummary {
            entries: archive.len(),
            ..ArchiveSummary::default()
        },
    };
    let mut names = HashSet::new();

    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                reject(
                    &mut scan,
                    "archive_entry_invalid",
                    format!("Archive entry {index} could not be inspected: {error}"),
                    None,
                );
                continue;
            }
        };
        let safe_path = match entry.enclosed_name() {
            Some(path) => path,
            None => {
                reject(
                    &mut scan,
                    "archive_path_rejected",
                    "Archive contains a path that could escape its destination.",
                    Some(entry.name().to_string()),
                );
                continue;
            }
        };
        let display_path = safe_path.to_string_lossy().replace('\\', "/");
        let normalized_name = display_path.to_ascii_lowercase();
        if !names.insert(normalized_name) {
            reject(
                &mut scan,
                "archive_duplicate_path",
                "Archive contains duplicate paths, which makes extraction ambiguous.",
                Some(display_path.clone()),
            );
        }
        if is_symlink(entry.unix_mode()) {
            reject(
                &mut scan,
                "archive_symlink_rejected",
                "Archive contains a symbolic-link entry.",
                Some(display_path.clone()),
            );
        }

        if entry.is_dir() {
            scan.summary.directories += 1;
            continue;
        }
        scan.summary.files += 1;
        scan.summary.compressed_bytes = scan
            .summary
            .compressed_bytes
            .saturating_add(entry.compressed_size());
        scan.summary.uncompressed_bytes =
            scan.summary.uncompressed_bytes.saturating_add(entry.size());

        if entry.size() > MAX_SINGLE_ENTRY_BYTES {
            reject(
                &mut scan,
                "archive_entry_too_large",
                "Archive contains an entry larger than the inspection safety limit.",
                Some(display_path.clone()),
            );
        }
        if scan.summary.uncompressed_bytes > MAX_TOTAL_UNCOMPRESSED_BYTES {
            reject(
                &mut scan,
                "archive_uncompressed_limit",
                "Archive expands beyond the current uncompressed-size safety limit.",
                None,
            );
        }

        if is_nested_archive(&safe_path) {
            scan.summary.nested_archives += 1;
        }
        if !is_manifest_path(&safe_path) {
            continue;
        }
        if safe_path.components().count() > MAX_MANIFEST_COMPONENTS {
            scan.issues.push(PackageIssue::warning(
                "manifest_nested_too_deep",
                "A deeply nested manifest.json was not inspected.",
                Some(display_path),
            ));
            continue;
        }
        if entry.size() > MAX_MANIFEST_BYTES {
            scan.issues.push(PackageIssue::error(
                "manifest_too_large",
                "manifest.json is larger than the inspection limit.",
                Some(display_path),
            ));
            continue;
        }

        let mut bytes = Vec::with_capacity(entry.size() as usize);
        let mut limited = entry.by_ref().take(MAX_MANIFEST_BYTES + 1);
        if let Err(error) = limited.read_to_end(&mut bytes) {
            scan.issues.push(PackageIssue::error(
                "manifest_read_failed",
                format!("Could not read manifest.json from the archive: {error}"),
                Some(display_path),
            ));
            continue;
        }
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            scan.issues.push(PackageIssue::error(
                "manifest_too_large",
                "manifest.json expanded beyond the inspection limit.",
                Some(display_path),
            ));
            continue;
        }
        let pack_root = safe_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| ".".into());
        scan.candidates.push(ManifestCandidate {
            manifest_path: display_path,
            pack_root,
            bytes,
        });
    }

    if scan.summary.uncompressed_bytes >= SUSPICIOUS_RATIO_MIN_BYTES
        && scan.summary.compressed_bytes > 0
        && scan.summary.uncompressed_bytes / scan.summary.compressed_bytes > MAX_COMPRESSION_RATIO
    {
        reject(
            &mut scan,
            "archive_compression_ratio_rejected",
            "Archive has an extreme compression ratio and was rejected as unsafe.",
            None,
        );
    }

    if scan.summary.nested_archives > 0 {
        scan.issues.push(PackageIssue::warning(
            "nested_archives_not_expanded",
            "Nested .mcpack/.mcaddon files are reported but are not recursively opened.",
            None,
        ));
    }
    Ok(scan)
}

fn rejected_scan(code: impl Into<String>, message: impl Into<String>) -> ArchiveScan {
    ArchiveScan {
        candidates: Vec::new(),
        issues: vec![PackageIssue::error(code, message, None)],
        safety: PackageSafety::Rejected,
        summary: ArchiveSummary::default(),
    }
}

fn reject(
    scan: &mut ArchiveScan,
    code: impl Into<String>,
    message: impl Into<String>,
    path: Option<String>,
) {
    scan.safety = PackageSafety::Rejected;
    scan.issues.push(PackageIssue::error(code, message, path));
}

fn is_symlink(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170000 == 0o120000)
}

fn is_manifest_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("manifest.json"))
}

fn is_nested_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("mcpack") || extension.eq_ignore_ascii_case("mcaddon")
        })
}
