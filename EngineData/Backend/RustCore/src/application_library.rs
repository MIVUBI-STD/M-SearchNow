use crate::{
    build_local_backend_snapshot,
    diagnostics::{DiagnosticComponent, DiagnosticSeverity, DiagnosticsBuffer},
    error::{BackendError, BackendResult},
    library::{valid_local_content_id, LocalContentType},
    library_export::export_directory,
    package::{inspect_package, PackageInspection},
    platform::PlatformContext,
    settings::{ExportDuplicatePolicy, SettingsStore},
    LocalBackendSnapshot,
};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Clone)]
pub(crate) struct LibraryApplicationService {
    settings: SettingsStore,
    platform: PlatformContext,
    diagnostics: DiagnosticsBuffer,
}

impl LibraryApplicationService {
    pub(crate) fn new(
        settings: SettingsStore,
        platform: PlatformContext,
        diagnostics: DiagnosticsBuffer,
    ) -> Self {
        Self {
            settings,
            platform,
            diagnostics,
        }
    }

    pub(crate) fn local_content_export_file_name(&self, item_id: &str) -> BackendResult<String> {
        let item = self.resolve_local_content(item_id)?;
        let extension = match item.content_type {
            LocalContentType::World => "mcworld",
            LocalContentType::BehaviorPack
            | LocalContentType::ResourcePack
            | LocalContentType::SkinPack => "mcpack",
        };
        let mut base = item
            .title
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.') {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        base = base.trim_matches([' ', '.']).trim().to_string();
        if base.is_empty() {
            base = "Minecraft backup".into();
        }
        Ok(format!("{base}.{extension}"))
    }

    pub(crate) fn export_local_content(
        &self,
        item_id: &str,
        destination: &Path,
    ) -> BackendResult<()> {
        let item = self.resolve_local_content(item_id)?;
        let expected_extension = match item.content_type {
            LocalContentType::World => "mcworld",
            LocalContentType::BehaviorPack
            | LocalContentType::ResourcePack
            | LocalContentType::SkinPack => "mcpack",
        };
        let valid_extension = destination
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(expected_extension));
        if !valid_extension {
            return Err(BackendError::new(
                "library_export_extension_invalid",
                format!("This content must be exported as .{expected_extension}."),
            ));
        }
        export_directory(&item.path, destination)?;

        let verification = inspect_package(destination);
        let valid = verification.as_ref().is_ok_and(|inspection| {
            inspection.safety == crate::package::PackageSafety::Safe
                && match item.content_type {
                    LocalContentType::World => inspection.world.is_some(),
                    LocalContentType::BehaviorPack
                    | LocalContentType::ResourcePack
                    | LocalContentType::SkinPack => !inspection.packs.is_empty(),
                }
        });

        if !valid {
            match std::fs::remove_file(destination) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(BackendError::from_io(
                        "library_export_verification_cleanup_failed",
                        "SearchNow could not verify the backup and could not remove the unverified output. Review the export folder before continuing.",
                        error,
                    ))
                }
            }
            return Err(BackendError::new(
                "library_export_verification_failed",
                "SearchNow could not verify the completed backup safely. The unverified output was removed.",
            ));
        }

        Ok(())
    }

    pub(crate) fn export_local_content_to_directory(
        &self,
        item_id: &str,
        destination_directory: &Path,
        duplicate_policy: ExportDuplicatePolicy,
    ) -> BackendResult<String> {
        if !destination_directory.is_dir() {
            return Err(BackendError::new(
                "library_export_destination_invalid",
                "The selected export folder is not available.",
            ));
        }

        let suggested = self.local_content_export_file_name(item_id)?;
        let mut reserved = std::collections::HashSet::new();
        let destination = export_destination(
            destination_directory,
            &suggested,
            duplicate_policy,
            &mut reserved,
        )?;
        let started = Instant::now();
        let result = self.export_local_content(item_id, &destination).map(|()| {
            destination
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or(suggested)
        });
        self.diagnostics.record_outcome(
            DiagnosticComponent::Library,
            started,
            result.is_ok(),
            "library_export_ok",
            "Minecraft content backup exported successfully.",
            "library_export_failed",
            "Minecraft content backup export failed.",
            DiagnosticSeverity::Warning,
        );
        result
    }

    pub(crate) fn export_local_content_batch(
        &self,
        item_ids: &[String],
        destination_directory: &Path,
    ) -> BackendResult<Vec<String>> {
        self.export_local_content_batch_with_policy(
            item_ids,
            destination_directory,
            ExportDuplicatePolicy::KeepBoth,
        )
    }

    pub(crate) fn export_local_content_batch_with_policy(
        &self,
        item_ids: &[String],
        destination_directory: &Path,
        duplicate_policy: ExportDuplicatePolicy,
    ) -> BackendResult<Vec<String>> {
        if item_ids.is_empty() {
            return Err(BackendError::new(
                "library_batch_export_empty",
                "Select at least one Minecraft content item to export.",
            ));
        }
        if item_ids.len() > 100 {
            return Err(BackendError::new(
                "library_batch_export_too_large",
                "SearchNow exports up to 100 Library items at a time.",
            ));
        }
        if !destination_directory.is_dir() {
            return Err(BackendError::new(
                "library_export_destination_invalid",
                "The selected export folder is not available.",
            ));
        }

        let mut destinations = Vec::with_capacity(item_ids.len());
        let mut reserved = std::collections::HashSet::new();
        for item_id in item_ids {
            let item = self.resolve_local_content(item_id)?;
            let suggested = self.local_content_export_file_name(item_id)?;
            let destination = export_destination(
                destination_directory,
                &suggested,
                duplicate_policy,
                &mut reserved,
            )?;
            destinations.push((item.id, destination));
        }

        let mut created = Vec::new();
        let result = (|| {
            for (item_id, destination) in &destinations {
                self.export_local_content(item_id, destination)?;
                created.push(destination.clone());
            }
            Ok(created
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect())
        })();

        if let Err(error) = result {
            let mut cleanup_failure = None;
            for path in created.iter().rev() {
                if let Err(remove_error) = std::fs::remove_file(path) {
                    if remove_error.kind() != std::io::ErrorKind::NotFound
                        && cleanup_failure.is_none()
                    {
                        cleanup_failure = Some(remove_error);
                    }
                }
            }
            if let Some(cleanup_error) = cleanup_failure {
                return Err(BackendError::from_io(
                    "library_batch_export_rollback_failed",
                    "Batch export failed and SearchNow could not remove every newly created backup. Review the export folder before continuing.",
                    cleanup_error,
                ));
            }
            return Err(error);
        }
        result
    }

    pub(crate) fn remove_local_content(
        &self,
        item_id: &str,
    ) -> BackendResult<LocalBackendSnapshot> {
        if !valid_local_content_id(item_id) {
            return Err(BackendError::new(
                "library_item_not_found",
                "The selected Minecraft content is no longer available.",
            ));
        }

        let settings = self.settings.load()?;
        let discovery = discover_minecraft_storage(&settings.minecraft, &self.platform);
        let library = scan_library(
            &discovery.roots,
            settings.minecraft.include_development_content,
        );
        let item = library
            .items
            .iter()
            .find(|item| item.id == item_id)
            .ok_or_else(|| {
                BackendError::new(
                    "library_item_not_found",
                    "The selected Minecraft content is no longer available.",
                )
            })?;
        let root = discovery
            .roots
            .iter()
            .find(|root| root.id == item.root_id)
            .ok_or_else(|| {
                BackendError::new(
                    "library_root_unavailable",
                    "The Minecraft storage root for this content is no longer available.",
                )
            })?;

        if !item.path.starts_with(&root.root) || item.path == root.root {
            return Err(BackendError::new(
                "library_remove_path_rejected",
                "SearchNow refused to remove content outside its detected Minecraft storage.",
            ));
        }

        let metadata = std::fs::symlink_metadata(&item.path).map_err(|error| {
            BackendError::from_io(
                "library_remove_metadata_failed",
                "SearchNow could not verify the selected Minecraft content.",
                error,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(BackendError::new(
                "library_remove_path_rejected",
                "SearchNow only removes detected Minecraft content directories.",
            ));
        }

        let started = Instant::now();
        let removal = std::fs::remove_dir_all(&item.path).map_err(|error| {
            BackendError::from_io(
                "library_remove_failed",
                "SearchNow could not remove the selected Minecraft content.",
                error,
            )
        });
        self.diagnostics.record_outcome(
            DiagnosticComponent::Library,
            started,
            removal.is_ok(),
            "library_remove_ok",
            "Minecraft content removed successfully.",
            "library_remove_failed",
            "Minecraft content removal failed.",
            DiagnosticSeverity::Warning,
        );
        removal?;
        self.scan_local_library_raw()
    }

    pub(crate) fn local_content_directory(&self, item_id: &str) -> BackendResult<PathBuf> {
        Ok(self.resolve_local_content(item_id)?.path)
    }

    fn resolve_local_content(
        &self,
        item_id: &str,
    ) -> BackendResult<crate::library::LocalContentItem> {
        if !valid_local_content_id(item_id) {
            return Err(BackendError::new(
                "library_item_not_found",
                "The selected Minecraft content is no longer available.",
            ));
        }
        let snapshot = self.scan_local_library_raw()?;
        snapshot
            .library
            .items
            .into_iter()
            .find(|item| item.id == item_id)
            .ok_or_else(|| {
                BackendError::new(
                    "library_item_not_found",
                    "The selected Minecraft content is no longer available.",
                )
            })
    }

    fn scan_local_library_raw(&self) -> BackendResult<LocalBackendSnapshot> {
        let settings = self.settings.load()?;
        Ok(build_local_backend_snapshot(&settings, &self.platform))
    }
}

fn export_destination(
    directory: &Path,
    suggested: &str,
    duplicate_policy: ExportDuplicatePolicy,
    reserved: &mut std::collections::HashSet<PathBuf>,
) -> BackendResult<PathBuf> {
    match duplicate_policy {
        ExportDuplicatePolicy::KeepBoth => {
            next_batch_export_destination(directory, suggested, reserved)
        }
        ExportDuplicatePolicy::StopOnConflict => {
            let candidate = directory.join(suggested);
            if candidate.exists() || !reserved.insert(candidate.clone()) {
                return Err(BackendError::new(
                    "library_export_destination_exists",
                    "A backup with this file name already exists.",
                ));
            }
            Ok(candidate)
        }
    }
}

fn next_batch_export_destination(
    directory: &Path,
    suggested: &str,
    reserved: &mut std::collections::HashSet<PathBuf>,
) -> BackendResult<PathBuf> {
    let suggested_path = Path::new(suggested);
    let extension = suggested_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let stem = suggested_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Minecraft backup");

    for sequence in 1_u32..=u32::MAX {
        let file_name = if sequence == 1 {
            suggested.to_string()
        } else if extension.is_empty() {
            format!("{stem} ({sequence})")
        } else {
            format!("{stem} ({sequence}).{extension}")
        };
        let candidate = directory.join(file_name);
        if !candidate.exists() && reserved.insert(candidate.clone()) {
            return Ok(candidate);
        }
    }

    Err(BackendError::new(
        "library_batch_export_destination_exhausted",
        "SearchNow could not allocate a unique backup filename.",
    ))
}
