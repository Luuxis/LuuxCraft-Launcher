//! Machine information and OS integration commands.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub total_memory_mb: Option<u64>,
    pub platform: &'static str,
    pub arch: &'static str,
    pub launcher_version: String,
    /// Where `accounts.json` lives (the game location).
    pub accounts_file: String,
    pub debug: bool,
}

pub fn total_memory_mb() -> Option<u64> {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let bytes = system.total_memory();
    (bytes > 0).then_some(bytes / (1024 * 1024))
}

#[tauri::command]
pub fn system_info(app: AppHandle, state: State<'_, AppState>) -> SystemInfo {
    SystemInfo {
        total_memory_mb: total_memory_mb(),
        platform: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        launcher_version: app.package_info().version.to_string(),
        accounts_file: state.accounts.path().display().to_string(),
        debug: cfg!(debug_assertions),
    }
}

/// Opens a launcher folder in the file manager: `logs`, `gameRoot`,
/// `launcherData` or `instance:<id>`.
#[tauri::command]
pub async fn open_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    target: String,
) -> AppResult<()> {
    let path = match target.as_str() {
        "logs" => state.paths.logs_dir.clone(),
        "gameLogs" => state.paths.game_logs_dir(),
        "gameRoot" => state.game_root(),
        "launcherData" => state.paths.launcher_dir.clone(),
        other => match other.strip_prefix("instance:") {
            Some(id) => {
                let instance = state.instance(id).await?;
                state.game_root().join("instances").join(instance.id)
            }
            None => return Err(AppError::new("invalid_argument", "unknown folder")),
        },
    };
    std::fs::create_dir_all(&path)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|error| AppError::new("io", error.to_string()))
}

#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::new(
            "invalid_argument",
            "only http(s) links can be opened",
        ));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| AppError::new("io", error.to_string()))
}
