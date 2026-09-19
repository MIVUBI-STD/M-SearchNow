use searchnow_core::{
    app_runtime::{SearchNowBackendPaths, SearchNowBackendRuntime},
    platform::PlatformContext,
    provider_adapter::IntegratedProvider,
    providers::CurseForgeProvider,
};
use std::sync::Arc;
use tauri::{Emitter, Manager};

const DOWNLOADS_CHANGED_EVENT: &str = "searchnow://downloads-changed";

pub fn configure_application<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let paths =
        SearchNowBackendPaths::from_roots(app.path().app_config_dir()?, app.path().app_data_dir()?);
    let providers = curseforge_provider_from_environment()?;
    let runtime = SearchNowBackendRuntime::new(paths, PlatformContext::from_process(), providers)?;
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


fn curseforge_provider_from_environment(
) -> Result<Vec<Arc<dyn IntegratedProvider>>, Box<dyn std::error::Error>> {
    let Some(api_key) = std::env::var("SEARCHNOW_CURSEFORGE_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(Vec::new());
    };

    Ok(vec![Arc::new(CurseForgeProvider::new(api_key)?)])
}
