//! LuuxCraft launcher backend.
//!
//! Responsibilities are split by module: `client_config` (which tenant this
//! engine serves, read from the pack on disk), `config` (central configuration
//! and per-tenant paths), `branding` (the tenant's name and logo, applied to
//! the window), `api` (panel client and models), `accounts`/`auth`/`sessions`
//! (multi-account, sign-in flows and automatic renewal), `skins`,
//! `instances`/`game` (install and launch through `crust_core`), `java`,
//! `settings`, `status`, `updater`, `system`, `logging`.

mod accounts;
mod api;
mod auth;
mod branding;
mod client_config;
mod commands;
mod config;
#[cfg(target_os = "linux")]
mod desktop;
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

use tauri::{Manager, WebviewWindowBuilder};
use tauri_plugin_window_state::StateFlags;

use crate::client_config::ClientConfig;
use crate::config::{LauncherConfig, Paths};
use crate::state::AppState;

fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Meurt en disant pourquoi.
///
/// Un moteur qui ne sait pas à quel tenant il appartient n'a rien à afficher et
/// personne à qui parler : il n'y a ni appairage ni valeur compilée pour le
/// rattraper, donc rien à faire d'autre que de s'arrêter net.
fn fail(reason: &str) -> ! {
    eprintln!("luuxcraft-launcher: {reason}");
    std::process::exit(1);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Le pack client d'abord : c'est lui qui donne le slug, et le slug décide
    // de tout ce qui doit être isolé entre deux tenants — jusqu'à
    // l'identifiant de l'application, qu'il faut avoir figé avant de construire
    // quoi que ce soit.
    let client = match ClientConfig::resolve() {
        Ok(client) => client,
        Err(error) => fail(&error),
    };
    let config = LauncherConfig::from_client(&client);

    // Deux clients du même moteur doivent pouvoir tourner en même temps. Or
    // l'identifiant du bundle est ce qui sert de clé au verrou d'instance
    // unique (mutex Windows, nom D-Bus Linux, socket macOS) comme au dossier de
    // configuration dont le greffon d'état de fenêtre tire son fichier : le
    // laisser commun, c'est faire croire à tauri que le tenant B *est* le
    // tenant A. On le suffixe donc du slug, une fois, avant tout le reste.
    let mut context = tauri::generate_context!();
    let identifier = format!(
        "{}.{}",
        context.config().identifier.trim_end_matches('.'),
        client.slug
    );
    context.config_mut().identifier = identifier;

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
            let paths = Paths::resolve(app.handle(), &client)?;
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
            log::info!(
                "client pack: {} ({}, {})",
                client.dir.display(),
                client.slug,
                client.api_base_url
            );
            log::info!("launcher data: {}", paths.launcher_dir.display());

            // La fenêtre est déclarée `create: false` : c'est la seule façon de
            // lui donner un profil de webview à part, sans quoi WebView2 et
            // WebKitGTK rangent cookies et `localStorage` de tous les tenants
            // au même endroit.
            let window_config = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .expect("the main window must be declared in tauri.conf.json");
            let window = WebviewWindowBuilder::from_config(app.handle(), &window_config)?
                .data_directory(paths.webview_dir.clone())
                .build()?;

            // Déclarée invisible : le titre et l'icône sont posés avant la
            // première image, pour que le nom générique du moteur ne
            // s'affiche jamais, pas même le temps d'une trame.
            branding::apply(&window, &client, &paths.launcher_dir);
            window.show()?;

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
            skins::skin_library_list,
            skins::skin_file_preview,
            skins::skin_capes_list,
            skins::skin_library_import,
            skins::skin_library_add_current,
            skins::skin_library_update,
            skins::skin_library_remove,
            skins::skin_apply,
            skins::skin_reset,
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
        .build(context)
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
