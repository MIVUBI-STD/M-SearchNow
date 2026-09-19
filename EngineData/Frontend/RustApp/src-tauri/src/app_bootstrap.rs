use searchnow_core::{
    app_runtime::{SearchNowBackendPaths, SearchNowBackendRuntime},
    platform::PlatformContext,
};
use tauri::{Emitter, Manager};

const DOWNLOADS_CHANGED_EVENT: &str = "searchnow://downloads-changed";

pub fn configure_application<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let paths =
        SearchNowBackendPaths::from_roots(app.path().app_config_dir()?, app.path().app_data_dir()?);
    let runtime = SearchNowBackendRuntime::new(paths, PlatformContext::from_process(), Vec::new())?;
    let app_handle = app.handle().clone();
    runtime.set_download_change_notifier(move || {
        let _ = app_handle.emit(DOWNLOADS_CHANGED_EVENT, ());
    });
    app.manage(runtime);

    if let Some(window) = app.get_webview_window("main") {
        window.set_title("SearchNow")?;
    }
    Ok(())
}
