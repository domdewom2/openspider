//! Native adapters for missing crawl-file extension points (K2-61).
use os_app::{dto::*, App};

#[tauri::command]
pub async fn crawl_file_check(path: String) -> Result<(), CmdError> {
    os_app::commands::crawl_files::check(path).await
}

#[tauri::command]
pub async fn crawl_file_config(
    app: tauri::State<'_, App>,
    path: String,
) -> Result<CrawlConfigDto, CmdError> {
    os_app::commands::crawl_files::configuration(&app, path).await
}

#[tauri::command]
pub fn crawl_file_repair(app: tauri::State<'_, App>, path: String) -> OpHandle {
    os_app::commands::crawl_files::repair(&app, path)
}
