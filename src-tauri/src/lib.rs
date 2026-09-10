//! LuuxCraft launcher backend.
//!
//! Responsibilities are split by module: `config` (central configuration),
//! `api` (panel client and models), `accounts`/`auth`/`sessions`
//! (multi-account, sign-in flows and automatic renewal), `skins`,
//! `instances`/`game` (install and launch through `crust_core`), `java`,
//! `settings`, `status`, `updater`, `system`, `logging`.

mod accounts;
mod api;
mod auth;
mod commands;
mod config;
mod error;
mod game;
mod instances;
mod java;
mod logging;
mod sessions;
mod settings;
mod skins;
mod state;
mod status;
mod system;
mod updater;
mod util;

use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

use crate::config::{LauncherConfig, Paths};
use crate::state::AppState;

fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = match LauncherConfig::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        // Must be the first plugin: a second launch focuses the running one.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .setup(move |app| {
            let paths = Paths::resolve(app.handle())?;
            paths.create_all()?;
            app.handle()
                .plugin(logging::plugin(paths.logs_dir.clone()))?;
            let package = app.package_info();
            log::info!(
                "{} {} starting on {} {} (crust_core {})",
                package.name,
                package.version,
                std::env::consts::OS,
                std::env::consts::ARCH,
                "1.0.3"
            );
            log::info!("launcher data: {}", paths.launcher_dir.display());
            let state = AppState::new(config, paths)?;
            log::info!("game root: {}", state.game_root().display());
            app.manage(state);
            // Keeps the stored sessions valid while the launcher runs.
            sessions::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_bootstrap,
            commands::settings_get,
            commands::settings_update,
            commands::settings_reset,
            commands::game_root,
            api::remote_fetch,
            api::remote_articles,
            api::remote_cached,
            auth::auth_methods_get,
            auth::auth_microsoft_window_login,
            auth::auth_cancel,
            auth::auth_azauth_login,
            auth::auth_offline_login,
            auth::auth_yggdrasil_login,
            auth::accounts_list,
            auth::accounts_select,
            auth::accounts_remove,
            auth::accounts_refresh,
            skins::skin_get,
            java::java_detect,
            java::java_probe,
            java::java_required,
            instances::instances_list,
            instances::instances_status,
            instances::instance_status,
            instances::instance_install,
            instances::instance_launch,
            instances::instance_cancel,
            instances::game_running,
            instances::game_busy,
            instances::game_kill,
            status::server_status,
            updater::update_check,
            updater::update_install,
            system::system_info,
            system::open_folder,
            system::open_external,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                game::on_exit_requested(app, code, &api);
            }
            tauri::RunEvent::Exit => {
                log::info!("launcher stopped");
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => focus_main_window(app),
            _ => {}
        });
}
