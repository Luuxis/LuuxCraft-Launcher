//! Bootstrap and settings commands.

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::accounts::AccountSummary;
use crate::config::LauncherConfig;
use crate::error::AppResult;
use crate::settings::Settings;
use crate::state::AppState;
use crate::system::{self, SystemInfo};

/// Everything the UI needs before the first paint, in one round trip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub config: LauncherConfig,
    pub settings: Settings,
    pub accounts: Vec<AccountSummary>,
    pub system: SystemInfo,
    pub paths: BootstrapPaths,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPaths {
    pub launcher_dir: String,
    pub logs_dir: String,
    pub game_root: String,
}

#[tauri::command]
pub fn app_bootstrap(app: AppHandle, state: State<'_, AppState>) -> Bootstrap {
    Bootstrap {
        config: state.config.clone(),
        settings: state.settings(),
        accounts: state.accounts.list(),
        system: system::system_info(app, state.clone()),
        paths: BootstrapPaths {
            launcher_dir: state.paths.launcher_dir.display().to_string(),
            logs_dir: state.paths.logs_dir.display().to_string(),
            game_root: state.game_root().display().to_string(),
        },
    }
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Settings {
    state.settings()
}

/// Replaces the settings (validated, clamped, persisted) and returns the
/// stored version.
#[tauri::command]
pub fn settings_update(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    let stored = state.replace_settings(settings)?;
    log::info!(
        "settings saved: concurrency {}, memory {}-{} MB, behaviour {:?}, java {:?}",
        stored.download_concurrency,
        stored.memory.min_mb,
        stored.memory.max_mb,
        stored.launcher_behavior,
        stored.java.mode
    );
    Ok(stored)
}

#[tauri::command]
pub fn settings_reset(state: State<'_, AppState>) -> AppResult<Settings> {
    state.update_settings(|settings| {
        let keep_account = settings.selected_account.clone();
        let keep_instance = settings.selected_instance.clone();
        let keep_dir = settings.resolved_data_directory.clone();
        *settings = Settings::from_config(&state.config);
        settings.selected_account = keep_account;
        settings.selected_instance = keep_instance;
        settings.resolved_data_directory = keep_dir;
    })
}

/// The game root for the current settings (changes with the install path).
#[tauri::command]
pub fn game_root(state: State<'_, AppState>) -> String {
    state.game_root().display().to_string()
}
