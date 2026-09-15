//! Launcher auto-update through `tauri-plugin-updater`.
//!
//! Endpoints come from the panel (`updater.endpoints` of `/config`) and fall
//! back to the built-in `UPDATER_ENDPOINTS`; the public key comes from
//! `tauri.conf.json`. Every package is signature-checked before installation.

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_updater::UpdaterExt;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// `false` when no endpoint is configured.
    pub configured: bool,
    pub update: Option<UpdateInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum UpdateEvent {
    Started {
        content_length: Option<u64>,
    },
    Progress {
        downloaded: u64,
        content_length: Option<u64>,
    },
    Finished,
    Installed,
}

/// The panel wins over the built-in list, so a release can be redirected
/// without shipping a new launcher.
fn configured_endpoints(state: &AppState) -> Vec<String> {
    let from_panel = state
        .snapshot()
        .or_else(|| state.cached_snapshot())
        .map(|snapshot| snapshot.config.updater_endpoints)
        .unwrap_or_default();
    if from_panel.is_empty() {
        state.config.updater.endpoints.clone()
    } else {
        from_panel
    }
}

fn endpoints(state: &AppState) -> AppResult<Vec<url::Url>> {
    configured_endpoints(state)
        .iter()
        .map(|endpoint| {
            url::Url::parse(endpoint).map_err(|error| {
                AppError::new(
                    "update_not_configured",
                    format!("invalid updater endpoint: {error}"),
                )
            })
        })
        .collect()
}

async fn check(
    app: &AppHandle,
    state: &AppState,
) -> AppResult<Option<tauri_plugin_updater::Update>> {
    let endpoints = endpoints(state)?;
    if endpoints.is_empty() {
        return Ok(None);
    }
    let updater = app
        .updater_builder()
        .endpoints(endpoints)?
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    Ok(updater.check().await?)
}

#[tauri::command]
pub async fn update_check(app: AppHandle, state: State<'_, AppState>) -> AppResult<UpdateCheck> {
    if configured_endpoints(&state).is_empty() {
        log::info!("auto-update disabled: no endpoint configured");
        return Ok(UpdateCheck {
            configured: false,
            update: None,
        });
    }
    let update = check(&app, &state).await?.map(|update| UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        body: update.body.clone(),
        date: update.date.map(|date| date.to_string()),
    });
    match &update {
        Some(info) => log::info!(
            "update available: {} (current {})",
            info.version,
            info.current_version
        ),
        None => log::info!("launcher is up to date"),
    }
    Ok(UpdateCheck {
        configured: true,
        update,
    })
}

/// Downloads, verifies and installs the update, then restarts the launcher.
/// On Windows the installer exits the app by itself.
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<UpdateEvent>,
) -> AppResult<()> {
    if state.game.is_running() {
        return Err(AppError::new(
            "game_running",
            "close the game before updating the launcher",
        ));
    }
    let Some(update) = check(&app, &state).await? else {
        return Err(AppError::new("update_failed", "no update available"));
    };
    log::info!("installing launcher update {}", update.version);
    let mut downloaded: u64 = 0;
    let mut started = false;
    let progress = on_event.clone();
    update
        .download_and_install(
            |chunk, content_length| {
                if !started {
                    started = true;
                    let _ = progress.send(UpdateEvent::Started { content_length });
                }
                downloaded += chunk as u64;
                let _ = progress.send(UpdateEvent::Progress {
                    downloaded,
                    content_length,
                });
            },
            || {
                let _ = on_event.send(UpdateEvent::Finished);
            },
        )
        .await?;
    let _ = on_event.send(UpdateEvent::Installed);
    log::info!("update installed, restarting the launcher");
    app.restart();
}
