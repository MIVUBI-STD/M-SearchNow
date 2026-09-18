use crate::error::{BackendError, BackendResult};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

const MAX_DESTINATION_FILE_NAME_BYTES: usize = 240;
const MAX_JOB_ID_BYTES: usize = 128;
const MAX_KEEP_BOTH_ATTEMPTS: u32 = 9_999;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadWorkspacePlan {
    pub job_id: String,
    pub workspace_dir: PathBuf,
    pub payload_path: PathBuf,
    pub final_path: PathBuf,
}

pub fn validate_destination_file_name(value: &str) -> BackendResult<()> {
    let reserved = windows_reserved_name(value);
    let unsupported_character = value.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
    });

    if value.is_empty()
        || value == "."
        || value == ".."
        || value.len() > MAX_DESTINATION_FILE_NAME_BYTES
        || value.ends_with('.')
        || value.ends_with(' ')
        || unsupported_character
        || reserved
    {
        return Err(BackendError::new(
            "download_destination_name_invalid",
            "Download destination must be one Windows-safe file name.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_job_id(value: &str) -> BackendResult<()> {
    if value.is_empty()
        || value.len() > MAX_JOB_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(BackendError::new(
            "download_job_id_invalid",
            "Download job id contains unsupported characters.",
        ));
    }
    Ok(())
}

pub fn plan_workspace(
    workspace_root: &Path,
    destination_root: &Path,
    job_id: &str,
    destination_file_name: &str,
) -> BackendResult<DownloadWorkspacePlan> {
    validate_job_id(job_id)?;
    validate_destination_file_name(destination_file_name)?;
    let workspace_dir = workspace_root.join(job_id);
    Ok(DownloadWorkspacePlan {
        job_id: job_id.to_string(),
        payload_path: workspace_dir.join("payload.part"),
        workspace_dir,
        final_path: destination_root.join(destination_file_name),
    })
}

pub fn ensure_workspace(plan: &DownloadWorkspacePlan) -> BackendResult<()> {
    if plan.workspace_dir.exists() {
        let metadata = fs::symlink_metadata(&plan.workspace_dir).map_err(|error| {
            BackendError::from_io(
                "download_workspace_metadata_failed",
                "SearchNow could not inspect the download workspace.",
                error,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(BackendError::new(
                "download_workspace_invalid",
                "Download workspace must be a regular non-symlink directory.",
            ));
        }
        return Ok(());
    }

    fs::create_dir_all(&plan.workspace_dir).map_err(|error| {
        BackendError::from_io(
            "download_workspace_create_failed",
            "SearchNow could not create the download workspace.",
            error,
        )
    })
}

pub fn prepare_payload_file(
    plan: &DownloadWorkspacePlan,
    resume_offset: u64,
) -> BackendResult<File> {
    ensure_workspace(plan)?;

    if resume_offset == 0 && plan.payload_path.exists() {
        let metadata = fs::symlink_metadata(&plan.payload_path).map_err(|error| {
            BackendError::from_io(
                "download_payload_metadata_failed",
                "SearchNow could not inspect the existing download payload.",
                error,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(BackendError::new(
                "download_payload_invalid",
                "Download payload must be a regular non-symlink file.",
            ));
        }
        fs::remove_file(&plan.payload_path).map_err(|error| {
            BackendError::from_io(
                "download_payload_reset_failed",
                "SearchNow could not reset the existing download payload.",
                error,
            )
        })?;
    }

    if resume_offset > 0 {
        let metadata = fs::symlink_metadata(&plan.payload_path).map_err(|error| {
            BackendError::from_io(
                "download_resume_payload_missing",
                "SearchNow could not inspect the partial download payload.",
                error,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(BackendError::new(
                "download_resume_payload_invalid",
                "Partial download payload must be a regular non-symlink file.",
            ));
        }
        if metadata.len() < resume_offset {
            return Err(BackendError::new(
                "download_resume_payload_short",
                "Partial download payload is shorter than its persisted progress.",
            ));
        }
        let file = OpenOptions::new()
            .append(true)
            .open(&plan.payload_path)
            .map_err(|error| {
                BackendError::from_io(
                    "download_resume_payload_open_failed",
                    "SearchNow could not reopen the partial download payload.",
                    error,
                )
            })?;
        if metadata.len() > resume_offset {
            file.set_len(resume_offset).map_err(|error| {
                BackendError::from_io(
                    "download_resume_payload_truncate_failed",
                    "SearchNow could not reconcile the partial download payload.",
                    error,
                )
            })?;
        }
        return Ok(file);
    }

    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&plan.payload_path)
        .map_err(|error| {
            BackendError::from_io(
                "download_payload_create_failed",
                "SearchNow could not create the download payload.",
                error,
            )
        })
}

pub fn cleanup_workspace(plan: &DownloadWorkspacePlan) -> BackendResult<()> {
    if !plan.workspace_dir.exists() {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(&plan.workspace_dir).map_err(|error| {
        BackendError::from_io(
            "download_workspace_metadata_failed",
            "SearchNow could not inspect the download workspace.",
            error,
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(BackendError::new(
            "download_workspace_invalid",
            "Download workspace cleanup refused a non-directory or symlink path.",
        ));
    }

    fs::remove_dir_all(&plan.workspace_dir).map_err(|error| {
        BackendError::from_io(
            "download_workspace_cleanup_failed",
            "SearchNow could not clean the download workspace.",
            error,
        )
    })
}

pub fn finalization_stage_path(plan: &DownloadWorkspacePlan) -> BackendResult<PathBuf> {
    validate_job_id(&plan.job_id)?;
    let destination_dir = plan.final_path.parent().ok_or_else(|| {
        BackendError::new(
            "download_destination_path_invalid",
            "Download destination has no parent directory.",
        )
    })?;
    Ok(destination_dir.join(format!(".searchnow-{}.part", plan.job_id)))
}

pub fn cleanup_finalization_stage(plan: &DownloadWorkspacePlan) -> BackendResult<()> {
    let stage_path = finalization_stage_path(plan)?;
    if !stage_path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(&stage_path).map_err(|error| {
        BackendError::from_io(
            "download_finalize_stage_metadata_failed",
            "SearchNow could not inspect its destination staging file.",
            error,
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(BackendError::new(
            "download_finalize_stage_invalid",
            "Download destination staging path is not a regular non-symlink file.",
        ));
    }
    fs::remove_file(&stage_path).map_err(|error| {
        BackendError::from_io(
            "download_finalize_stage_cleanup_failed",
            "SearchNow could not clean an interrupted destination staging file.",
            error,
        )
    })
}

pub fn finalized_file_matches(plan: &DownloadWorkspacePlan, expected_bytes: u64) -> bool {
    let Ok(metadata) = fs::symlink_metadata(&plan.final_path) else {
        return false;
    };
    metadata.file_type().is_file()
        && !metadata.file_type().is_symlink()
        && metadata.len() == expected_bytes
}

fn ensure_destination_directory(destination_dir: &Path) -> BackendResult<()> {
    if destination_dir.exists() {
        let metadata = fs::symlink_metadata(destination_dir).map_err(|error| {
            BackendError::from_io(
                "download_destination_metadata_failed",
                "SearchNow could not inspect the selected download folder.",
                error,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(BackendError::new(
                "download_destination_directory_invalid",
                "The selected download destination is no longer a regular folder.",
            ));
        }
        return Ok(());
    }

    fs::create_dir_all(destination_dir).map_err(|error| {
        BackendError::from_io(
            "download_destination_create_failed",
            "SearchNow could not create the download destination directory.",
            error,
        )
    })?;

    let metadata = fs::symlink_metadata(destination_dir).map_err(|error| {
        BackendError::from_io(
            "download_destination_metadata_failed",
            "SearchNow could not verify the selected download folder.",
            error,
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(BackendError::new(
            "download_destination_directory_invalid",
            "The selected download destination is not a regular folder.",
        ));
    }
    Ok(())
}

pub fn finalize_payload(plan: &DownloadWorkspacePlan) -> BackendResult<PathBuf> {
    validate_destination_file_name(
        plan.final_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
    )?;
    let payload_metadata = fs::symlink_metadata(&plan.payload_path).map_err(|error| {
        BackendError::from_io(
            "download_payload_missing",
            "Download payload is not available for finalization.",
            error,
        )
    })?;
    if !payload_metadata.file_type().is_file() || payload_metadata.file_type().is_symlink() {
        return Err(BackendError::new(
            "download_payload_invalid",
            "Download payload must be a regular non-symlink file.",
        ));
    }

    let destination_dir = plan.final_path.parent().ok_or_else(|| {
        BackendError::new(
            "download_destination_path_invalid",
            "Download destination has no parent directory.",
        )
    })?;
    ensure_destination_directory(destination_dir)?;

    cleanup_finalization_stage(plan)?;
    let stage_path = finalization_stage_path(plan)?;
    let mut source = File::open(&plan.payload_path).map_err(|error| {
        BackendError::from_io(
            "download_payload_open_failed",
            "SearchNow could not open the completed download payload.",
            error,
        )
    })?;
    let mut stage = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stage_path)
        .map_err(|error| {
            BackendError::from_io(
                "download_finalize_stage_failed",
                "SearchNow could not create its destination staging file.",
                error,
            )
        })?;

    if let Err(error) = io::copy(&mut source, &mut stage).and_then(|_| stage.sync_all()) {
        let _ = fs::remove_file(&stage_path);
        return Err(BackendError::from_io(
            "download_finalize_stage_failed",
            "SearchNow could not finish staging the downloaded file.",
            error,
        ));
    }
    drop(stage);

    let published = publish_keep_both(&stage_path, &plan.final_path)?;
    let _ = fs::remove_file(&stage_path);
    Ok(published)
}

fn publish_keep_both(stage_path: &Path, requested_path: &Path) -> BackendResult<PathBuf> {
    for attempt in 0..=MAX_KEEP_BOTH_ATTEMPTS {
        let candidate = if attempt == 0 {
            requested_path.to_path_buf()
        } else {
            keep_both_path(requested_path, attempt + 1)?
        };
        match fs::hard_link(stage_path, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(BackendError::from_io(
                    "download_finalize_commit_failed",
                    "SearchNow could not atomically publish the downloaded file.",
                    error,
                ));
            }
        }
    }

    Err(BackendError::new(
        "download_destination_name_exhausted",
        "SearchNow could not create a unique file name in the selected folder.",
    ))
}

fn keep_both_path(requested_path: &Path, copy_number: u32) -> BackendResult<PathBuf> {
    let file_name = requested_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            BackendError::new(
                "download_destination_path_invalid",
                "Download destination file name is invalid.",
            )
        })?;
    let file_path = Path::new(file_name);
    let stem = file_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name);
    let extension = file_path.extension().and_then(|value| value.to_str());
    let unique_name = match extension {
        Some(extension) if !extension.is_empty() => format!("{stem} ({copy_number}).{extension}"),
        _ => format!("{stem} ({copy_number})"),
    };
    validate_destination_file_name(&unique_name)?;
    Ok(requested_path.with_file_name(unique_name))
}

fn windows_reserved_name(value: &str) -> bool {
    let base = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || matches!(
            base.as_str(),
            "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        )
}
