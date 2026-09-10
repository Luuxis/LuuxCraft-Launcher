//! Structured logging through `tauri-plugin-log` and the `log` facade.
//!
//! Files rotate under the launcher log directory; the webview forwards its own
//! logs through the plugin so one file tells the whole story. Nothing secret
//! is ever passed to a log macro (see `util::redact` for game output).

use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const MAX_FILE_SIZE: u128 = 5 * 1024 * 1024;
const KEPT_FILES: usize = 5;

pub fn plugin<R: Runtime>(logs_dir: std::path::PathBuf) -> TauriPlugin<R> {
    let level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    let mut targets = vec![Target::new(TargetKind::Folder {
        path: logs_dir,
        file_name: Some("launcher".to_owned()),
    })];
    if cfg!(debug_assertions) {
        targets.push(Target::new(TargetKind::Stdout));
    }
    tauri_plugin_log::Builder::new()
        .targets(targets)
        .level(level)
        .level_for("hyper", log::LevelFilter::Warn)
        .level_for("hyper_util", log::LevelFilter::Warn)
        .level_for("reqwest", log::LevelFilter::Warn)
        .level_for("rustls", log::LevelFilter::Warn)
        .level_for("zbus", log::LevelFilter::Warn)
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("wry", log::LevelFilter::Warn)
        .level_for("tracing", log::LevelFilter::Warn)
        .max_file_size(MAX_FILE_SIZE)
        .rotation_strategy(RotationStrategy::KeepSome(KEPT_FILES))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}
