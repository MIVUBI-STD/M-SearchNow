import type { DiagnosticEvent } from "./types";

const ACTIVITY_CODES = new Set([
  "package_import_ok",
  "package_import_failed",
  "package_replace_ok",
  "package_replace_failed",
  "package_bundle_replace_ok",
  "package_bundle_replace_failed",
  "library_export_ok",
  "library_export_failed",
  "library_remove_ok",
  "library_remove_failed",
  "download_queue_ok",
  "download_queue_failed",
  "download_pause_ok",
  "download_pause_failed",
  "download_resume_ok",
  "download_resume_failed",
  "download_cancel_ok",
  "download_cancel_failed",
  "download_retry_ok",
  "download_retry_failed",
]);

export function activityLabel(event: DiagnosticEvent): string {
  switch (event.code) {
    case "package_import_ok": return "Content installed";
    case "package_import_failed": return "Install failed";
    case "package_replace_ok": return "Pack updated";
    case "package_replace_failed": return "Pack update failed";
    case "package_bundle_replace_ok": return "Add-on bundle updated";
    case "package_bundle_replace_failed": return "Add-on bundle update failed";
    case "library_export_ok": return "Backup exported";
    case "library_export_failed": return "Backup export failed";
    case "library_remove_ok": return "Content removed";
    case "library_remove_failed": return "Content removal failed";
    case "download_queue_ok": return "Download queued";
    case "download_queue_failed": return "Download could not start";
    case "download_pause_ok": return "Download pause requested";
    case "download_pause_failed": return "Download could not pause";
    case "download_resume_ok": return "Download resumed";
    case "download_resume_failed": return "Download could not resume";
    case "download_cancel_ok": return "Download cancellation requested";
    case "download_cancel_failed": return "Download could not cancel";
    case "download_retry_ok": return "Download retry queued";
    case "download_retry_failed": return "Download retry failed";
    default: return event.message;
  }
}

export function recentActivity(events: DiagnosticEvent[], limit = 12): DiagnosticEvent[] {
  return events.filter((event) => ACTIVITY_CODES.has(event.code)).slice(-limit).reverse();
}
