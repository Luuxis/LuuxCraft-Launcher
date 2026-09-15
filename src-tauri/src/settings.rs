//! Persistent user settings (`settings.json`).
//!
//! Defaults come from the built-in launcher configuration (`config.rs`), the
//! file is merged over them so new fields get sensible values when the
//! launcher is updated, and every value is clamped to a safe range before use.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::LauncherConfig;
use crate::error::{AppError, AppResult};

/// Lower bound of the JVM heap the launcher accepts, in MiB.
pub const MIN_MEMORY_MB: u64 = 512;
/// Smallest game window the launcher accepts.
pub const MIN_WINDOW_SIZE: u32 = 320;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub selected_account: Option<String>,
    pub selected_instance: Option<String>,
    /// User-chosen Minecraft root; `None` means the platform default.
    pub install_path: Option<String>,
    /// `dataDirectory` last announced by the panel, kept so the game root does
    /// not move when the launcher starts offline.
    pub resolved_data_directory: Option<String>,
    pub download_concurrency: usize,
    pub memory: MemorySettings,
    pub game_window: GameWindowSettings,
    pub launcher_behavior: LauncherBehavior,
    pub java: JavaSettings,
    pub jvm_args: Vec<String>,
    pub intel_enabled_mac: bool,
    /// Per-instance overrides, keyed by instance id.
    pub instances: HashMap<String, InstanceSettings>,
    pub ui: UiSettings,
    pub server_status_refresh_seconds: u64,
    pub check_updates_on_startup: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            selected_account: None,
            selected_instance: None,
            install_path: None,
            resolved_data_directory: None,
            download_concurrency: 10,
            memory: MemorySettings {
                min_mb: 1024,
                max_mb: 4096,
            },
            game_window: GameWindowSettings::default(),
            launcher_behavior: LauncherBehavior::Stay,
            java: JavaSettings::default(),
            jvm_args: Vec::new(),
            intel_enabled_mac: false,
            instances: HashMap::new(),
            ui: UiSettings::default(),
            server_status_refresh_seconds: 30,
            check_updates_on_startup: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySettings {
    pub min_mb: u64,
    pub max_mb: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameWindowSettings {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fullscreen: bool,
}

/// What the launcher does once Minecraft runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LauncherBehavior {
    /// Stay open, do nothing.
    Stay,
    /// Hide the window while the game runs, show it again when it exits.
    Hide,
    /// Minecraft depends on the launcher: closing the launcher kills the game.
    Bound,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JavaSettings {
    pub mode: JavaMode,
    /// Path to a `java` executable when `mode` is `Custom`.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JavaMode {
    /// Let `crust_core` pick the Mojang runtime (Azul Zulu fallback).
    #[default]
    Auto,
    /// Use the executable in `path`.
    Custom,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceSettings {
    pub memory: Option<MemorySettings>,
    pub java: Option<JavaSettings>,
    pub jvm_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiSettings {
    /// `dark`, `light` or `system`.
    pub theme: String,
    pub reduce_motion: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_owned(),
            reduce_motion: false,
        }
    }
}

impl Settings {
    /// Defaults derived from the central configuration.
    pub fn from_config(config: &LauncherConfig) -> Self {
        Self {
            download_concurrency: config.downloads.default_concurrency,
            memory: MemorySettings {
                min_mb: config.memory.default_min_mb,
                max_mb: config.memory.default_max_mb,
            },
            game_window: GameWindowSettings {
                width: Some(config.game_window.default_width),
                height: Some(config.game_window.default_height),
                fullscreen: false,
            },
            server_status_refresh_seconds: config.server_status.refresh_seconds,
            ..Self::default()
        }
    }

    /// Loads the file, merging it over the configuration defaults. A missing or
    /// unreadable file yields the defaults (and a warning in the logs) so a
    /// corrupted settings file never prevents the launcher from starting.
    pub fn load(path: &Path, config: &LauncherConfig) -> Self {
        let defaults = Self::from_config(config);
        let raw = match std::fs::read(path) {
            Ok(raw) => raw,
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("settings.json is unreadable, using defaults: {error}");
                }
                return defaults;
            }
        };
        let stored: Value = match serde_json::from_slice(&raw) {
            Ok(value) => value,
            Err(error) => {
                log::warn!("settings.json is corrupted, using defaults: {error}");
                return defaults;
            }
        };
        let mut merged = serde_json::to_value(&defaults).unwrap_or(Value::Null);
        merge(&mut merged, stored);
        match serde_json::from_value::<Self>(merged) {
            Ok(settings) => settings,
            Err(error) => {
                log::warn!("settings.json has invalid values, using defaults: {error}");
                defaults
            }
        }
    }

    /// Writes the file atomically (temporary file + rename).
    pub fn save(&self, path: &Path) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, json)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }

    /// Clamps every value to a safe range. `total_memory_mb` bounds the heap
    /// so the machine cannot be starved.
    pub fn sanitize(&mut self, config: &LauncherConfig, total_memory_mb: Option<u64>) {
        let max_concurrency = config.downloads.max_concurrency.clamp(1, 30);
        self.download_concurrency = self.download_concurrency.clamp(1, max_concurrency);
        self.memory = sanitize_memory(self.memory, total_memory_mb);
        for instance in self.instances.values_mut() {
            if let Some(memory) = instance.memory {
                instance.memory = Some(sanitize_memory(memory, total_memory_mb));
            }
            if let Some(java) = &mut instance.java {
                sanitize_java(java);
            }
        }
        sanitize_java(&mut self.java);
        if let Some(width) = self.game_window.width {
            self.game_window.width = Some(width.max(MIN_WINDOW_SIZE));
        }
        if let Some(height) = self.game_window.height {
            self.game_window.height = Some(height.max(MIN_WINDOW_SIZE));
        }
        if !matches!(self.ui.theme.as_str(), "dark" | "light" | "system") {
            self.ui.theme = "dark".to_owned();
        }
        self.server_status_refresh_seconds = self.server_status_refresh_seconds.clamp(10, 600);
        self.jvm_args.retain(|arg| !arg.trim().is_empty());
        if let Some(path) = &self.install_path {
            if path.trim().is_empty() {
                self.install_path = None;
            }
        }
    }

    pub fn install_path(&self) -> Option<PathBuf> {
        self.install_path.as_deref().map(PathBuf::from)
    }

    /// Effective values for one instance (instance override or global).
    pub fn for_instance(&self, instance_id: &str) -> EffectiveSettings {
        let overrides = self.instances.get(instance_id);
        EffectiveSettings {
            memory: overrides.and_then(|o| o.memory).unwrap_or(self.memory),
            java: overrides
                .and_then(|o| o.java.clone())
                .unwrap_or_else(|| self.java.clone()),
            jvm_args: overrides
                .and_then(|o| o.jvm_args.clone())
                .unwrap_or_else(|| self.jvm_args.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EffectiveSettings {
    pub memory: MemorySettings,
    pub java: JavaSettings,
    pub jvm_args: Vec<String>,
}

fn sanitize_memory(mut memory: MemorySettings, total_memory_mb: Option<u64>) -> MemorySettings {
    let ceiling = total_memory_mb
        .map(|total| (total.saturating_sub(1024)).max(MIN_MEMORY_MB))
        .unwrap_or(u64::MAX);
    memory.max_mb = memory.max_mb.clamp(MIN_MEMORY_MB, ceiling);
    memory.min_mb = memory.min_mb.clamp(MIN_MEMORY_MB, memory.max_mb);
    memory
}

fn sanitize_java(java: &mut JavaSettings) {
    if let Some(path) = &java.path {
        if path.trim().is_empty() {
            java.path = None;
        }
    }
    if java.mode == JavaMode::Custom && java.path.is_none() {
        java.mode = JavaMode::Auto;
    }
}

/// Deep-merges `overlay` into `base` (objects recursively, everything else replaced).
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

/// Validates a settings object coming from the frontend before it is stored.
pub fn validate_incoming(settings: &Settings) -> AppResult<()> {
    if settings.memory.min_mb > settings.memory.max_mb {
        return Err(AppError::new(
            "settings_invalid",
            "minimum memory is above maximum memory",
        ));
    }
    if let Some(path) = &settings.install_path {
        let path = Path::new(path);
        if !path.is_absolute() {
            return Err(AppError::new(
                "settings_invalid",
                "install path must be absolute",
            ));
        }
    }
    if settings.java.mode == JavaMode::Custom {
        match &settings.java.path {
            Some(path) if Path::new(path).is_file() => {}
            _ => {
                return Err(AppError::new(
                    "java_missing",
                    "the custom Java executable does not exist",
                ))
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_defaults_for_missing_keys() {
        let config = LauncherConfig::load().unwrap();
        let defaults = Settings::from_config(&config);
        let mut merged = serde_json::to_value(&defaults).unwrap();
        merge(
            &mut merged,
            serde_json::json!({ "memory": { "maxMb": 8192 }, "unknownKey": 1 }),
        );
        let settings: Settings = serde_json::from_value(merged).unwrap();
        assert_eq!(settings.memory.max_mb, 8192);
        assert_eq!(settings.memory.min_mb, config.memory.default_min_mb);
        assert_eq!(
            settings.download_concurrency,
            config.downloads.default_concurrency
        );
    }

    #[test]
    fn sanitize_clamps_values() {
        let config = LauncherConfig::load().unwrap();
        let mut settings = Settings::from_config(&config);
        settings.download_concurrency = 500;
        settings.memory = MemorySettings {
            min_mb: 9000,
            max_mb: 100,
        };
        settings.game_window.width = Some(10);
        settings.sanitize(&config, Some(8192));
        assert_eq!(settings.download_concurrency, 30);
        assert_eq!(settings.memory.max_mb, MIN_MEMORY_MB);
        assert_eq!(settings.memory.min_mb, MIN_MEMORY_MB);
        assert_eq!(settings.game_window.width, Some(MIN_WINDOW_SIZE));
    }

    #[test]
    fn instance_overrides_win() {
        let config = LauncherConfig::load().unwrap();
        let mut settings = Settings::from_config(&config);
        settings.instances.insert(
            "abc".into(),
            InstanceSettings {
                memory: Some(MemorySettings {
                    min_mb: 2048,
                    max_mb: 6144,
                }),
                java: None,
                jvm_args: None,
            },
        );
        assert_eq!(settings.for_instance("abc").memory.max_mb, 6144);
        assert_eq!(
            settings.for_instance("other").memory.max_mb,
            config.memory.default_max_mb
        );
    }
}
