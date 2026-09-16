//! Launcher identity and built-in defaults.
//!
//! Everything that can be published by the panel is published by the panel:
//! maintenance, sign-in mode, Azure client id, data directory, links, module
//! toggles, instances and news all come from the API at runtime (see `api`).
//!
//! Ce que l'API ne peut pas fournir, c'est sa propre adresse ni le tenant à qui
//! ce moteur appartient. Rien de tout cela n'est compilé : le bootstrap pose
//! ces deux valeurs sur le disque, dans le pack client, et
//! `client_config::ClientConfig` les relit au démarrage.
//!
//! The rest of this module is made of plain defaults, applied until the panel
//! (or the user's settings) says otherwise.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::client_config::ClientConfig;

/// Game folder under the platform data directory, until the panel announces
/// its own `dataDirectory`.
const DEFAULT_DATA_DIRECTORY: &str = "luuxcraft";

/// Panel route of the dynamic update server, relative to the API base URL.
///
/// Built from the base URL rather than hard-coded so a launcher pointed at
/// another panel takes its updates from that panel too. Les versions sont
/// globales : il n'y a qu'un moteur compilé, donc une seule chaîne de mise à
/// jour pour tous les tenants d'un panel. The braces are the placeholders
/// `tauri-plugin-updater` substitutes itself.
fn updater_endpoints_for(base_url: &str) -> Vec<String> {
    vec![format!(
        "{}/launcher/update/{{{{target}}}}/{{{{arch}}}}/{{{{current_version}}}}",
        base_url.trim_end_matches('/')
    )]
}

/// Optional Yggdrasil-compatible server (authlib-injector style). Mojang's own
/// `authserver.mojang.com` is discontinued, so that sign-in method is only
/// offered when a compatible server is set here.
const YGGDRASIL_SERVER: Option<&str> = None;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    /// LuuxCraft panel user id (`users.id`): `{api.baseUrl}/user/{userId}/...`.
    pub user_id: String,
    /// Slug du tenant, qui nomme aussi son dossier de données.
    pub slug: String,
    pub display_name: String,
    pub api: ApiConfig,
    /// Folder name of the game data under the platform data directory, used
    /// when the panel config does not provide `dataDirectory`.
    pub data_directory: String,
    pub updater: UpdaterConfig,
    pub auth: AuthConfig,
    pub news: NewsConfig,
    pub server_status: ServerStatusConfig,
    pub downloads: DownloadsConfig,
    pub memory: MemoryConfig,
    pub game_window: GameWindowConfig,
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

impl LauncherConfig {
    /// Construit la configuration à partir du pack client et des valeurs par
    /// défaut ci-dessus.
    ///
    /// Infaillible : c'est `ClientConfig` qui valide l'adresse du panel et
    /// l'identifiant du tenant, avant même que le moteur n'ouvre une fenêtre.
    pub fn from_client(client: &ClientConfig) -> Self {
        let base_url = client.api_base_url.clone();
        Self {
            user_id: client.tenant_id.clone(),
            slug: client.slug.clone(),
            display_name: client.display_name.clone(),
            api: ApiConfig {
                base_url: base_url.clone(),
                timeout_seconds: default_timeout(),
            },
            data_directory: DEFAULT_DATA_DIRECTORY.to_owned(),
            updater: UpdaterConfig {
                endpoints: updater_endpoints_for(&base_url),
            },
            auth: AuthConfig {
                yggdrasil_server: YGGDRASIL_SERVER.map(str::to_owned),
            },
            news: NewsConfig::default(),
            server_status: ServerStatusConfig::default(),
            downloads: DownloadsConfig::default(),
            memory: MemoryConfig::default(),
            game_window: GameWindowConfig::default(),
        }
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
/// In debug builds everything goes to `data/` at the root of the repository so
/// an install can be inspected next to the sources and thrown away at once.
///
/// ## Un dossier par tenant
///
/// Un seul moteur est compilé pour tous les tenants, et rien n'empêche un
/// joueur d'installer celui de deux serveurs différents — c'est même le cas
/// qu'il faut faire marcher. Tout ce qui appartient au tenant vit donc sous
/// `%APPDATA%\.<slug>` (Windows) ou `~/.<slug>` (ailleurs) : réglages, comptes,
/// skins, caches, journaux, et jusqu'au profil de la webview. Sans ce
/// découpage, le second launcher installé écraserait les réglages du premier,
/// et surtout les deux partageraient les jetons de session d'`accounts.json` et
/// les cookies de la webview.
///
/// Ce qui reste **hors** de ce découpage, volontairement : la racine du jeu,
/// que le panel désigne par son `dataDirectory` et qui pèse des gigaoctets.
/// Deux serveurs qui la partagent partagent aussi les versions, bibliothèques
/// et assets déjà téléchargés, ce qui est le comportement voulu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paths {
    /// `settings.json`, `accounts.json`, les skins et l'empreinte de l'entrée
    /// de bureau.
    pub launcher_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
    /// Profil de la webview (WebView2 sous Windows, WebKitGTK sous Linux).
    /// Scopé au tenant, sinon deux clients partageraient leurs cookies et leur
    /// `localStorage`.
    pub webview_dir: PathBuf,
    /// Platform data directory; the game root is `<base>/<dataDirectory>`.
    pub game_base_dir: PathBuf,
    /// Debug builds pin the game root here (`data/minecraft`).
    pub dev_game_root: Option<PathBuf>,
}

impl Paths {
    /// Les chemins du tenant décrit par le pack client.
    pub fn resolve(app: &AppHandle, client: &ClientConfig) -> tauri::Result<Self> {
        if cfg!(debug_assertions) {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let root = manifest.parent().unwrap_or(&manifest).join("data");
            return Ok(Self {
                launcher_dir: root.join("launcher"),
                cache_dir: root.join("cache"),
                logs_dir: root.join("logs"),
                webview_dir: root.join("webview"),
                game_base_dir: root.clone(),
                dev_game_root: Some(root.join("minecraft")),
            });
        }
        let root = tenant_root(app, &client.slug)?;
        Ok(Self {
            launcher_dir: root.clone(),
            cache_dir: root.join("cache"),
            logs_dir: root.join("logs"),
            webview_dir: root.join("webview"),
            game_base_dir: app.path().data_dir()?,
            dev_game_root: None,
        })
    }

    pub fn create_all(&self) -> std::io::Result<()> {
        for dir in [
            &self.launcher_dir,
            &self.cache_dir,
            &self.logs_dir,
            &self.webview_dir,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Fichier des comptes, jetons de session compris.
    ///
    /// Il suit le tenant, pas l'installation du jeu : deux serveurs peuvent
    /// partager une racine de jeu — c'est même souhaitable, elle pèse des
    /// gigaoctets — mais sûrement pas les sessions Minecraft du joueur.
    pub fn accounts_file(&self) -> PathBuf {
        self.launcher_dir.join(crate::accounts::ACCOUNTS_FILE)
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

    /// The user's own skin library (PNG files + index). User data, not a
    /// cache: it must survive a cache wipe.
    pub fn skins_library_dir(&self) -> PathBuf {
        self.launcher_dir.join("skins")
    }

    pub fn game_logs_dir(&self) -> PathBuf {
        self.logs_dir.join("game")
    }
}

/// Racine des données du tenant, d'après le contrat de distribution :
/// `%APPDATA%\.<slug>` sous Windows, `~/.<slug>` ailleurs.
///
/// Pas `app_data_dir()` : celui-ci est dérivé de l'identifiant du bundle, que
/// deux tenants du même moteur partagent forcément.
#[cfg(target_os = "windows")]
fn tenant_root(app: &AppHandle, slug: &str) -> tauri::Result<PathBuf> {
    Ok(app.path().config_dir()?.join(format!(".{slug}")))
}

#[cfg(not(target_os = "windows"))]
fn tenant_root(app: &AppHandle, slug: &str) -> tauri::Result<PathBuf> {
    Ok(app.path().home_dir()?.join(format!(".{slug}")))
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
    fn the_client_pack_decides_the_panel_and_the_tenant() {
        let client = ClientConfig::sample();
        let config = LauncherConfig::from_client(&client);
        assert_eq!(config.user_id, client.tenant_id);
        assert_eq!(config.slug, client.slug);
        assert_eq!(config.api.base_url, "https://luuxcraft.fr/api");
        assert!(config.user_api_url().ends_with(&client.tenant_id));
        assert!(config.downloads.max_concurrency <= 30);
    }

    /// Les mises à jour suivent le panel du pack : un moteur pointé sur un
    /// autre panel ne doit pas continuer à installer les versions du premier.
    #[test]
    fn the_updater_follows_the_panel_of_the_pack() {
        let mut client = ClientConfig::sample();
        client.api_base_url = "https://autre-panel.fr/api".into();
        let config = LauncherConfig::from_client(&client);
        assert!(
            config
                .updater
                .endpoints
                .iter()
                .all(|endpoint| endpoint.starts_with("https://autre-panel.fr/api/")),
            "updates must follow the panel: {:?}",
            config.updater.endpoints
        );
    }

    #[test]
    fn updater_endpoints_keep_the_plugin_placeholders() {
        let endpoints = updater_endpoints_for("https://luuxcraft.fr/api");
        assert_eq!(
            endpoints,
            vec!["https://luuxcraft.fr/api/launcher/update/{{target}}/{{arch}}/{{current_version}}"]
        );
    }

    #[test]
    fn directory_names_are_sanitized() {
        assert_eq!(sanitize_dir_name("luuxis"), "luuxis");
        assert_eq!(sanitize_dir_name("../evil"), "evil");
        assert_eq!(sanitize_dir_name("  "), "luuxcraft");
        assert_eq!(sanitize_dir_name("a/b:c"), "abc");
    }
}
