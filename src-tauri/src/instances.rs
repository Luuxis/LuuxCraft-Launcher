//! Instance commands: listing with install state, install/verify, launch,
//! cancellation and process control.

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::api::{self, Instance};
use crate::error::{AppError, AppResult};
use crate::game::{self, LaunchEvent, LaunchMode, RunningGame};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceStatus {
    pub id: String,
    /// The client jar of the Minecraft version is present.
    pub installed: bool,
    pub game_dir: String,
    /// The instance directory (custom files, saves, options) exists.
    pub has_game_dir: bool,
}

fn status_of(state: &AppState, instance: &Instance) -> InstanceStatus {
    let root = state.game_root();
    let version = instance.minecraft_version.trim();
    let client_jar = root
        .join("versions")
        .join(version)
        .join(format!("{version}.jar"));
    let game_dir = root.join("instances").join(&instance.id);
    InstanceStatus {
        id: instance.id.clone(),
        installed: client_jar.is_file(),
        game_dir: game_dir.display().to_string(),
        has_game_dir: game_dir.is_dir(),
    }
}

#[tauri::command]
pub async fn instances_list(state: State<'_, AppState>) -> AppResult<Vec<Instance>> {
    Ok(api::ensure_snapshot(&state).await?.instances)
}

#[tauri::command]
pub async fn instances_status(state: State<'_, AppState>) -> AppResult<Vec<InstanceStatus>> {
    let snapshot = api::ensure_snapshot(&state).await?;
    Ok(snapshot
        .instances
        .iter()
        .map(|instance| status_of(&state, instance))
        .collect())
}

#[tauri::command]
pub async fn instance_status(
    state: State<'_, AppState>,
    instance_id: String,
) -> AppResult<InstanceStatus> {
    let instance = state.instance(&instance_id).await?;
    Ok(status_of(&state, &instance))
}

/// Downloads/verifies every file of the instance without starting the game.
#[tauri::command]
pub async fn instance_install(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    on_event: Channel<LaunchEvent>,
) -> AppResult<()> {
    game::run(&app, &state, &instance_id, LaunchMode::Install, on_event).await
}

/// Verifies the files, then starts Minecraft. Returns once the JVM runs; the
/// exit is reported on the channel as `exited`.
#[tauri::command]
pub async fn instance_launch(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    on_event: Channel<LaunchEvent>,
) -> AppResult<()> {
    let result = game::run(&app, &state, &instance_id, LaunchMode::Launch, on_event).await;
    if result.is_ok() {
        if let Err(error) = state.update_settings(|settings| {
            settings.selected_instance = Some(instance_id.clone());
        }) {
            log::warn!("could not remember the selected instance: {error}");
        }
    }
    result
}

/// Cancels the installation in progress (downloads are aborted).
#[tauri::command]
pub fn instance_cancel(state: State<'_, AppState>) -> bool {
    state.game.cancel()
}

#[tauri::command]
pub fn game_running(state: State<'_, AppState>) -> Option<RunningGame> {
    state.game.running()
}

#[tauri::command]
pub fn game_busy(state: State<'_, AppState>) -> bool {
    state.game.is_busy()
}

/// Force-stops the running game.
#[tauri::command]
pub fn game_kill(state: State<'_, AppState>) -> AppResult<()> {
    if state.game.kill() {
        Ok(())
    } else {
        Err(AppError::new("game_not_running", "no game is running"))
    }
}
