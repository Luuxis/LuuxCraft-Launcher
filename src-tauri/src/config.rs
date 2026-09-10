//! Central launcher configuration.
//!
//! Everything that differentiates one LuuxCraft launcher from another (the
//! panel `user_id`, the API base URL, the brand, the defaults) lives in
//! `launcher.config.json` at the repository root. The file is embedded at
//! compile time here and imported as JSON by the frontend, so there is a single
//! source of truth and no value is hard-coded anywhere else.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const RAW_CONFIG: &str = include_str!("../../launcher.config.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    /// LuuxCraft panel user id: `{api.baseUrl}/user/{userId}/...`.
    pub user_id: String,
    pub api: ApiConfig,
    pub brand: BrandConfig,
    /// Folder name of the game data under the platform data directory, used
    /// when the panel config does not provide `dataDirectory`.
    pub data_directory: String,
    #[serde(default)]
    pub updater: UpdaterConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub news: NewsConfig,
    #[serde(default)]
    pub server_status: ServerStatusConfig,
    #[serde(default)]
    pub downloads: DownloadsConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub game_window: GameWindowConfig,
    /// Links shown when the panel does not publish any (`socialLinks`).
    #[serde(default)]
    pub links: Vec<LinkConfig>,
    /// Default module toggles; the panel config can override any of them.
    #[serde(default)]
    pub modules: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConfig {
    pub base_url: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    15
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandConfig {
    pub name: String,
    pub wordmark: Wordmark,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub website: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wordmark {
    pub prefix: String,
    pub suffix: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterConfig {
    /// Tauri updater endpoints (static JSON manifest or dynamic server).
    /// Empty means "auto-update disabled".
    #[serde(default)]
    pub endpoints: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    /// Optional Yggdrasil-compatible server (authlib-injector style). Mojang's
    /// own `authserver.mojang.com` is discontinued, so this is only offered
    /// when a compatible server is explicitly configured.
    #[serde(default)]
    pub yggdrasil_server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsConfig {
    #[serde(default = "default_news_limit")]
    pub limit: u32,
}

fn default_news_limit() -> u32 {
    12
}

impl Default for NewsConfig {
    fn default() -> Self {
        Self {
            limit: default_news_limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatusConfig {
    #[serde(default = "default_refresh")]
    pub refresh_seconds: u64,
    #[serde(default = "default_status_timeout")]
    pub timeout_ms: u64,
}

fn default_refresh() -> u64 {
    30
}

fn default_status_timeout() -> u64 {
    3000
}

impl Default for ServerStatusConfig {
    fn default() -> Self {
        Self {
            refresh_seconds: default_refresh(),
            timeout_ms: default_status_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadsConfig {
    #[serde(default = "default_concurrency")]
    pub default_concurrency: usize,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
}

fn default_concurrency() -> usize {
    10
}

/// `crust_core` clamps the download concurrency to 1..=30.
fn default_max_concurrency() -> usize {
    30
}

impl Default for DownloadsConfig {
    fn default() -> Self {
        Self {
            default_concurrency: default_concurrency(),
            max_concurrency: default_max_concurrency(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryConfig {
    #[serde(default = "default_min_mb")]
    pub default_min_mb: u64,
    #[serde(default = "default_max_mb")]
    pub default_max_mb: u64,
}

fn default_min_mb() -> u64 {
    1024
}

fn default_max_mb() -> u64 {
    4096
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            default_min_mb: default_min_mb(),
            default_max_mb: default_max_mb(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameWindowConfig {
    #[serde(default = "default_width")]
    pub default_width: u32,
    #[serde(default = "default_height")]
    pub default_height: u32,
}

fn default_width() -> u32 {
    1280
}

fn default_height() -> u32 {
    720
}

impl Default for GameWindowConfig {
    fn default() -> Self {
        Self {
            default_width: default_width(),
            default_height: default_height(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkConfig {
    pub label: String,
    pub url: String,
    #[serde(default)]
    pub icon: Option<String>,
}

impl LauncherConfig {
    /// Parses and validates the embedded `launcher.config.json`.
    pub fn load() -> Result<Self, String> {
        let config: Self = serde_json::from_str(RAW_CONFIG)
            .map_err(|error| format!("launcher.config.json is invalid: {error}"))?;
        if config.user_id.trim().is_empty() {
            return Err("launcher.config.json: `userId` is required".into());
        }
        if !config.api.base_url.starts_with("https://") {
            return Err("launcher.config.json: `api.baseUrl` must use https".into());
        }
        if config.data_directory.trim().is_empty() {
            return Err("launcher.config.json: `dataDirectory` is required".into());
        }
        for endpoint in &config.updater.endpoints {
            if !endpoint.starts_with("https://") {
                return Err(format!(
                    "launcher.config.json: updater endpoint must use https: {endpoint}"
                ));
            }
        }
        Ok(config)
    }

    /// `{baseUrl}/user/{userId}` without a trailing slash.
    pub fn user_api_url(&self) -> String {
        format!(
            "{}/user/{}",
            self.api.base_url.trim_end_matches('/'),
            self.user_id
        )
    }
}

/// Where the launcher keeps its own files and where the game goes.
///
/// In release builds everything follows the OS conventions resolved by Tauri.
/// In debug builds everything goes to `data/` at the root of the repository so
/// an install can be inspected next to the sources and thrown away at once.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paths {
    /// `settings.json` and other launcher-owned files (accounts live with the game).
    pub launcher_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
    /// Platform data directory; the game root is `<base>/<dataDirectory>`.
    pub game_base_dir: PathBuf,
    /// Debug builds pin the game root here (`data/minecraft`).
    pub dev_game_root: Option<PathBuf>,
}

impl Paths {
    pub fn resolve(app: &AppHandle) -> tauri::Result<Self> {
        if cfg!(debug_assertions) {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let root = manifest.parent().unwrap_or(&manifest).join("data");
            return Ok(Self {
                launcher_dir: root.join("launcher"),
                cache_dir: root.join("cache"),
                logs_dir: root.join("logs"),
                game_base_dir: root.clone(),
                dev_game_root: Some(root.join("minecraft")),
            });
        }
        let resolver = app.path();
        Ok(Self {
            launcher_dir: resolver.app_data_dir()?,
            cache_dir: resolver.app_cache_dir()?,
            logs_dir: resolver.app_log_dir()?,
            game_base_dir: resolver.data_dir()?,
            dev_game_root: None,
        })
    }

    pub fn create_all(&self) -> std::io::Result<()> {
        for dir in [&self.launcher_dir, &self.cache_dir, &self.logs_dir] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// The Minecraft root (`versions/`, `libraries/`, `assets/`, `runtime/`,
    /// `instances/`). A user-chosen install path wins, then the debug pin,
    /// then `<platform data dir>/<dataDirectory>`.
    pub fn game_root(&self, install_path: Option<&Path>, data_directory: &str) -> PathBuf {
        if let Some(custom) = install_path {
            return custom.to_path_buf();
        }
        if let Some(dev) = &self.dev_game_root {
            return dev.clone();
        }
        self.game_base_dir.join(sanitize_dir_name(data_directory))
    }

    pub fn settings_file(&self) -> PathBuf {
        self.launcher_dir.join("settings.json")
    }

    pub fn remote_cache_file(&self) -> PathBuf {
        self.cache_dir.join("remote.json")
    }

    pub fn skins_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("skins")
    }

    pub fn game_logs_dir(&self) -> PathBuf {
        self.logs_dir.join("game")
    }
}

/// Keeps a directory name provided by the panel from escaping its parent.
pub fn sanitize_dir_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let cleaned = cleaned.trim_matches('.').trim();
    if cleaned.is_empty() {
        "luuxcraft".to_owned()
    } else {
        cleaned.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_config_is_valid() {
        let config = LauncherConfig::load().expect("valid launcher.config.json");
        assert!(!config.user_id.is_empty());
        assert!(config.user_api_url().ends_with(&config.user_id));
        assert!(config.downloads.max_concurrency <= 30);
    }

    #[test]
    fn directory_names_are_sanitized() {
        assert_eq!(sanitize_dir_name("luuxis"), "luuxis");
        assert_eq!(sanitize_dir_name("../evil"), "evil");
        assert_eq!(sanitize_dir_name("  "), "luuxcraft");
        assert_eq!(sanitize_dir_name("a/b:c"), "abc");
    }
}
