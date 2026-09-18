use crate::error::{BackendError, BackendResult};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

pub(crate) fn export_directory(source: &Path, destination: &Path) -> BackendResult<()> {
    if destination.exists() {
        return Err(BackendError::new(
            "library_export_destination_exists",
            "The selected export file already exists.",
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        BackendError::new(
            "library_export_destination_invalid",
            "The selected export destination is not valid.",
        )
    })?;
    if !parent.is_dir() {
        return Err(BackendError::new(
            "library_export_destination_invalid",
            "The selected export folder is not available.",
        ));
    }

    let temporary = temporary_export_path(destination);
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }

    let result = (|| {
        let file = File::create(&temporary).map_err(|error| {
            BackendError::from_io(
                "library_export_create_failed",
                "SearchNow could not create the temporary backup file.",
                error,
            )
        })?;
        let mut writer = ZipWriter::new(file);
        write_directory(&mut writer, source, source)?;
        let mut file = writer.finish().map_err(|error| {
            BackendError::new(
                "library_export_finalize_failed",
                format!("SearchNow could not finalize the backup archive: {error}"),
            )
        })?;
        file.flush().map_err(|error| {
            BackendError::from_io(
                "library_export_finalize_failed",
                "SearchNow could not flush the backup archive.",
                error,
            )
        })?;
        file.sync_all().map_err(|error| {
            BackendError::from_io(
                "library_export_finalize_failed",
                "SearchNow could not sync the backup archive safely.",
                error,
            )
        })?;
        drop(file);
        fs::rename(&temporary, destination).map_err(|error| {
            BackendError::from_io(
                "library_export_commit_failed",
                "SearchNow could not commit the completed backup archive.",
                error,
            )
        })
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_directory(
    writer: &mut ZipWriter<File>,
    root: &Path,
    directory: &Path,
) -> BackendResult<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            BackendError::from_io(
                "library_export_read_failed",
                "SearchNow could not read content while creating the backup.",
                error,
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            BackendError::from_io(
                "library_export_read_failed",
                "SearchNow could not enumerate content while creating the backup.",
                error,
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            BackendError::from_io(
                "library_export_metadata_failed",
                "SearchNow could not verify a file while creating the backup.",
                error,
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(BackendError::new(
                "library_export_symlink_rejected",
                "SearchNow will not export Minecraft content that contains symbolic links.",
            ));
        }
        let relative = path.strip_prefix(root).map_err(|_| {
            BackendError::new(
                "library_export_path_rejected",
                "SearchNow refused to export a file outside the selected content folder.",
            )
        })?;
        let name = relative.to_string_lossy().replace('\\', "/");
        if metadata.is_dir() {
            if !name.is_empty() {
                writer
                    .add_directory(
                        format!("{name}/"),
                        SimpleFileOptions::default()
                            .compression_method(CompressionMethod::Deflated),
                    )
                    .map_err(|error| {
                        BackendError::new(
                            "library_export_write_failed",
                            format!("SearchNow could not write a backup directory entry: {error}"),
                        )
                    })?;
            }
            write_directory(writer, root, &path)?;
        } else if metadata.is_file() {
            writer
                .start_file(
                    name,
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
                )
                .map_err(|error| {
                    BackendError::new(
                        "library_export_write_failed",
                        format!("SearchNow could not create a backup file entry: {error}"),
                    )
                })?;
            let mut input = File::open(&path).map_err(|error| {
                BackendError::from_io(
                    "library_export_read_failed",
                    "SearchNow could not open a file while creating the backup.",
                    error,
                )
            })?;
            io::copy(&mut input, writer).map_err(|error| {
                BackendError::from_io(
                    "library_export_write_failed",
                    "SearchNow could not write a file into the backup archive.",
                    error,
                )
            })?;
        }
    }
    Ok(())
}

fn temporary_export_path(destination: &Path) -> PathBuf {
    let mut name = destination
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_else(|| "searchnow-backup".into());
    name.push(".searchnow-part");
    destination.with_file_name(name)
}
