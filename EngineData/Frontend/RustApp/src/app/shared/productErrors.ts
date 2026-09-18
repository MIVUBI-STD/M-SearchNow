import type { ProductError } from "./types";

const FRIENDLY_MESSAGES: Record<string, string> = {
  catalog_provider_unavailable: "The content source is currently unavailable.",
  catalog_provider_query_failed: "The content source could not complete this request.",
  catalog_provider_data_invalid: "The content source returned unsupported data.",
  catalog_query_invalid: "The search request could not be processed.",

  download_transfer_timeout: "The download timed out. Try again.",
  download_transfer_read_failed: "The download was interrupted. Try again.",
  download_transfer_invalid_data: "The downloaded data could not be read safely.",
  download_transfer_incomplete: "The download ended before the file was complete.",
  download_payload_missing: "The temporary download file is no longer available.",
  download_payload_invalid: "The temporary download file could not be used safely.",
  download_payload_open_failed: "The completed download could not be prepared for saving.",
  download_payload_create_failed: "SearchNow could not create the temporary download file.",
  download_payload_reset_failed: "SearchNow could not restart the temporary download file.",
  download_payload_write_failed: "The file could not be written to disk. Check free space and try again.",
  download_payload_sync_failed: "The completed file could not be saved safely. Check the drive and try again.",
  download_destination_name_invalid: "The file name is not supported on this device.",
  download_destination_name_exhausted: "Too many files with this name already exist in the selected folder.",
  download_destination_directory_invalid: "The selected save location is not valid.",
  download_destination_create_failed: "The selected folder is unavailable or cannot be written to.",
  download_finalize_incomplete: "The download is not complete yet.",
  download_finalize_stage_failed: "The file could not be prepared in the selected folder. Check free space and permissions.",
  download_finalize_stage_invalid: "The selected folder contains an unsafe temporary file.",
  download_finalize_stage_cleanup_failed: "SearchNow could not clean an interrupted temporary file in the selected folder.",
  download_finalize_commit_failed: "The completed file could not be saved to the selected folder.",
  download_workspace_create_failed: "SearchNow could not prepare its temporary download folder.",
  download_workspace_invalid: "SearchNow's temporary download folder is unavailable.",
  download_workspace_cleanup_failed: "SearchNow could not clean its temporary download files.",
  download_scheduler_failed: "Downloads are temporarily paused. Refresh to continue.",
  download_transport_unavailable: "This download source is currently unavailable.",
  download_state_lock_failed: "Downloads are temporarily unavailable. Try again.",

  dialog_path_invalid: "The selected folder could not be used.",
  directory_unavailable: "This folder is no longer available.",
  directory_open_failed: "This folder could not be opened.",
  library_item_not_found: "This Minecraft content is no longer available.",
  library_root_unavailable: "The Minecraft storage for this content is no longer available.",
  library_remove_path_rejected: "SearchNow refused to remove this content because its location is not safe.",
  library_remove_failed: "SearchNow could not remove this Minecraft content. Check permissions and try again.",
  download_directory_not_ready: "This download folder is available after the download completes.",
  download_directory_unavailable: "This download no longer has an available save folder.",
  settings_load_failed: "SearchNow could not load your settings.",
  settings_save_failed: "SearchNow could not save your settings.",
  package_import_conflict: "This package is already installed in the selected Minecraft storage.",
  package_import_not_ready: "This package must pass inspection before it can be imported.",
  package_import_root_unavailable: "The selected Minecraft storage is no longer available.",
  package_import_kind_unsupported: "This Minecraft content type is not supported by automatic import yet.",
};

export function toProductError(error: unknown, fallbackMessage: string): ProductError {
  let code = "runtime_command_failed";
  let rawMessage = "";

  if (typeof error === "object" && error !== null) {
    const candidate = error as { code?: unknown; message?: unknown };
    if (typeof candidate.code === "string" && candidate.code.trim()) code = candidate.code;
    if (typeof candidate.message === "string") rawMessage = candidate.message.trim();
  } else if (typeof error === "string") {
    rawMessage = error.trim();
  }

  return {
    code,
    message: FRIENDLY_MESSAGES[code] ?? (rawMessage || fallbackMessage),
  };
}
