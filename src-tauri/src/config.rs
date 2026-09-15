//! Launcher identity and built-in defaults.
//!
//! Everything that can be published by the panel is published by the panel:
//! maintenance, sign-in mode, Azure client id, data directory, links, module
//! toggles, instances and news all come from the API at runtime (see `api`).
//!
//! What the API cannot provide is its own address, nor which client of the
//! panel this launcher belongs to. **One binary is built for every client**:
//! the client id is not compiled in, the panel appends it to the installer at
//! download time and `provisioning` reads it back. The build-time
//! `LUUXCRAFT_API_URL` and `LUUXCRAFT_USER_ID` overrides remain for whoever
//! wants a dedicated build, or to point a fork at another panel.
//!
//! The rest of this module is made of plain defaults, applied until the panel
//! (or the user's settings) says otherwise.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::provisioning::Provisioning;

/// LuuxCraft panel: `{API_BASE_URL}/user/{USER_ID}/...`.
///
/// The address stays compiled in because it is generic — it is the same for
/// every client of one panel. Only the client id varies from one to the next.
const API_BASE_URL: &str = match option_env!("LUUXCRAFT_API_URL") {
    Some(url) => url,
    None => "https://luuxcraft.fr/api",
};

/// Empty on purpose: the generic build carries no client id. It is provisioned
/// at download time, or asked for once at first launch. Setting
/// `LUUXCRAFT_USER_ID` at build time pins a launcher to one client.
const USER_ID: &str = match option_env!("LUUXCRAFT_USER_ID") {
    Some(id) => id,
    None => "",
};

/// Game folder under the platform data directory, until the panel announces
/// its own `dataDirectory`.
const DEFAULT_DATA_DIRECTORY: &str = "luuxcraft";

/// Panel route of the dynamic update server, relative to the API base URL.
///
/// Built from the base URL rather than hard-coded so a launcher provisioned
/// against another panel takes its updates from that panel too. The releases
/// are global: there is one compiled launcher, therefore one update chain for
/// all the clients of a panel. The braces are the placeholders
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
    /// LuuxCraft panel user id: `{api.baseUrl}/user/{userId}/...`.
    pub user_id: String,
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
    /// Builds the configuration from the constants above and checks the values
    /// a build can override.
    ///
    /// A missing `user_id` is **not** an error: the generic build starts
    /// unprovisioned and either finds its client id in the installer (see
    /// `provisioning`) or asks the player for it once.
    pub fn load() -> Result<Self, String> {
        let base_url = API_BASE_URL.trim().trim_end_matches('/').to_owned();
        if !base_url.starts_with("https://") {
            return Err(format!("LUUXCRAFT_API_URL must use https: {base_url}"));
        }

        Ok(Self {
            user_id: USER_ID.trim().to_owned(),
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
        })
    }

    /// `true` once the launcher knows which client of the panel it serves.
    /// Until then nothing can be fetched and the UI asks for a pairing code.
    pub fn is_provisioned(&self) -> bool {
        !self.user_id.is_empty()
    }

    /// Applies a resolved provisioning block: panel address and client id.
    ///
    /// The updater endpoints are recomputed, otherwise a launcher provisioned
    /// against another panel would keep asking the compiled-in one for its
    /// updates — and would install that panel's builds.
    pub fn apply_provisioning(&mut self, provisioning: &Provisioning) {
        self.api.base_url = provisioning.api_url.trim_end_matches('/').to_owned();
        self.user_id = provisioning.key.clone();
        self.updater.endpoints = updater_endpoints_for(&self.api.base_url);
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

/// Dossier qui regroupe les données rangées par client, sous chacune des
/// racines de l'application.
const CLIENTS_DIR: &str = "clients";

/// Where the launcher keeps its own files and where the game goes.
///
/// In release builds everything follows the OS conventions resolved by Tauri.
/// In debug builds everything goes to `data/` at the root of the repository so
/// an install can be inspected next to the sources and thrown away at once.
///
/// ## Un dossier par client
///
/// Un seul launcher est compilé pour tous les clients du panel, et rien
/// n'empêche un joueur d'installer celui de deux serveurs différents. Ses
/// paramètres, ses comptes, ses skins et ses caches sont donc rangés sous
/// `clients/<clé du client>/` : sans ça, le second launcher installé
/// écraserait les réglages du premier, et surtout les deux partageraient les
/// jetons de session stockés dans `accounts.json`.
///
/// Ce qui reste **hors** de ce découpage, volontairement :
///
/// - `provisioning.json`, la copie persistée du provisionnement, qui vit à la
///   racine parce qu'il faut savoir *quel* client avant de pouvoir ouvrir son
///   dossier (voir `provisioning::resolve`) ;
/// - la racine du jeu, que le panel désigne par son `dataDirectory` et qui
///   pèse des gigaoctets : deux serveurs qui la partagent partagent aussi les
///   versions, bibliothèques et assets déjà téléchargés, ce qui est le
///   comportement voulu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paths {
    /// Racine commune à tous les clients : c'est là que vit `provisioning.json`.
    pub shared_dir: PathBuf,
    /// `settings.json`, `accounts.json`, les skins et le logo mis en cache.
    pub launcher_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
    /// Platform data directory; the game root is `<base>/<dataDirectory>`.
    pub game_base_dir: PathBuf,
    /// Debug builds pin the game root here (`data/minecraft`).
    pub dev_game_root: Option<PathBuf>,
}

impl Paths {
    /// Chemins connus avant de savoir à quel client ce launcher appartient.
    ///
    /// Tant que le provisionnement n'est pas résolu, tout pointe sur les
    /// racines communes : c'est suffisant pour y chercher `provisioning.json`,
    /// et c'est aussi ce que voit un launcher jamais appairé, qui n'affiche que
    /// l'écran de saisie de code et n'a donc rien à ranger.
    pub fn resolve(app: &AppHandle) -> tauri::Result<Self> {
        if cfg!(debug_assertions) {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let root = manifest.parent().unwrap_or(&manifest).join("data");
            return Ok(Self {
                shared_dir: root.join("launcher"),
                launcher_dir: root.join("launcher"),
                cache_dir: root.join("cache"),
                logs_dir: root.join("logs"),
                game_base_dir: root.clone(),
                dev_game_root: Some(root.join("minecraft")),
            });
        }
        let resolver = app.path();
        let launcher_dir = resolver.app_data_dir()?;
        Ok(Self {
            shared_dir: launcher_dir.clone(),
            launcher_dir,
            cache_dir: resolver.app_cache_dir()?,
            logs_dir: resolver.app_log_dir()?,
            game_base_dir: resolver.data_dir()?,
            dev_game_root: None,
        })
    }

    /// La même arborescence, rangée sous le client donné.
    ///
    /// Appelé dès que le provisionnement est résolu, donc **avant** que le
    /// greffon de journalisation n'ouvre un fichier : la migration de
    /// l'installation héritée déplace des fichiers, et Windows refuse de
    /// renommer un fichier ouvert.
    pub fn scoped_to(&self, key: &str) -> Self {
        let slug = client_dir_name(key);
        let launcher_dir = self.launcher_dir.join(CLIENTS_DIR).join(&slug);
        let cache_dir = self.cache_dir.join(CLIENTS_DIR).join(&slug);
        let logs_dir = self.logs_dir.join(CLIENTS_DIR).join(&slug);

        // Les trois racines se passent la liste complète : tauri en imbrique
        // certaines l'une dans l'autre (sous Linux `app_log_dir` est un
        // sous-dossier d'`app_data_dir`, sous Windows d'`app_cache_dir`), et
        // sans ça la migration des données emporterait le dossier des journaux.
        let roots = [
            self.launcher_dir.as_path(),
            self.cache_dir.as_path(),
            self.logs_dir.as_path(),
        ];
        // `provisioning.json` est l'autre exclusion, et elle n'est pas
        // négociable : c'est lui qui permet de retrouver ce client au démarrage
        // suivant, il ne peut donc pas être rangé dans son dossier.
        adopt_legacy_layout(
            &self.launcher_dir,
            &launcher_dir,
            &[crate::provisioning::PERSISTED_FILE, CLIENTS_DIR],
            &roots,
        );
        adopt_legacy_layout(&self.cache_dir, &cache_dir, &[CLIENTS_DIR], &roots);
        adopt_legacy_layout(&self.logs_dir, &logs_dir, &[CLIENTS_DIR], &roots);

        Self {
            shared_dir: self.launcher_dir.clone(),
            launcher_dir,
            cache_dir,
            logs_dir,
            game_base_dir: self.game_base_dir.clone(),
            dev_game_root: self.dev_game_root.clone(),
        }
    }

    pub fn create_all(&self) -> std::io::Result<()> {
        for dir in [&self.launcher_dir, &self.cache_dir, &self.logs_dir] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Fichier des comptes, jetons de session compris.
    ///
    /// Il suit le client, pas l'installation du jeu : deux serveurs peuvent
    /// partager une racine de jeu — c'est même souhaitable, elle pèse des
    /// gigaoctets — mais sûrement pas les sessions Minecraft du joueur. C'est
    /// aussi ce qui fait qu'un changement de dossier d'installation ne fait
    /// plus perdre ses comptes.
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

/// Nom de dossier d'un client, à partir de sa clé.
///
/// `provisioning::normalize` borne déjà la clé à `[A-Za-z0-9_-]`, mais pas la
/// valeur compilée `LUUXCRAFT_USER_ID`, qui n'est jamais passée par là : le
/// filtrage est refait ici pour que ce nom soit à coup sûr un segment de chemin
/// et rien d'autre. Une clé vide ou entièrement filtrée retombe sur un nom
/// fixe, ce qui range ce launcher quelque part de prévisible plutôt que de le
/// laisser écrire à la racine commune.
fn client_dir_name(key: &str) -> String {
    let cleaned: String = key
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "default".to_owned()
    } else {
        cleaned
    }
}

/// Déplace une installation d'avant le découpage par client dans le dossier de
/// ce client, une fois pour toutes.
///
/// Tout ce qui traîne à la racine y est passé, sauf ce qui est explicitement
/// commun : plutôt qu'une liste de fichiers à migrer qu'il faudrait penser à
/// tenir à jour, la règle est « ce qui n'est pas commun appartient au premier
/// client rencontré ». Le premier client rencontré est bien le bon : la racine
/// ne contient des données que si un launcher y a déjà tourné, et il ne pouvait
/// alors en servir qu'un seul.
///
/// Deux exclusions : `reserved`, les noms de fichiers à laisser sur place, et
/// `roots`, les autres racines de l'application — tauri les imbrique parfois
/// l'une dans l'autre, et déplacer le dossier des journaux parce qu'il se
/// trouve être un enfant du dossier de données serait une surprise coûteuse.
///
/// L'existence du dossier de destination suffit à dire que c'est déjà fait. Un
/// échec est sans conséquence — le client repart de réglages par défaut, ce qui
/// est désagréable mais pas cassé — donc rien n'est remonté.
fn adopt_legacy_layout(root: &Path, scoped: &Path, reserved: &[&str], roots: &[&Path]) {
    if scoped.exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let movable: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let named = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !reserved.contains(&name));
            named && !roots.iter().any(|other| other.starts_with(path))
        })
        .collect();
    if movable.is_empty() {
        return;
    }
    if std::fs::create_dir_all(scoped).is_err() {
        return;
    }
    for path in movable {
        let Some(name) = path.file_name() else {
            continue;
        };
        if let Err(error) = std::fs::rename(&path, scoped.join(name)) {
            log::debug!("could not move {} to {}: {error}", path.display(), scoped.display());
        }
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
    fn built_in_config_is_valid() {
        let config = LauncherConfig::load().expect("valid launcher configuration");
        assert!(config.api.base_url.starts_with("https://"));
        assert!(!config.api.base_url.ends_with('/'));
        assert!(config.user_api_url().ends_with(&config.user_id));
        assert!(config.downloads.max_concurrency <= 30);
    }

    /// The generic build starts without a client id — that is the whole point
    /// of not compiling one launcher per client.
    #[test]
    fn a_build_without_a_user_id_is_unprovisioned() {
        let config = LauncherConfig::load().expect("valid launcher configuration");
        assert_eq!(config.is_provisioned(), !config.user_id.is_empty());
    }

    #[test]
    fn provisioning_moves_the_panel_and_the_updater_together() {
        let mut config = LauncherConfig::load().expect("valid launcher configuration");
        config.apply_provisioning(&Provisioning {
            api_url: "https://autre-panel.fr/api/".into(),
            key: "abc-def-ghi".into(),
        });
        assert_eq!(config.api.base_url, "https://autre-panel.fr/api");
        assert_eq!(config.user_id, "abc-def-ghi");
        assert!(config.is_provisioned());
        assert!(config.user_api_url().starts_with("https://autre-panel.fr/api/user/"));
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

    /// La clé du client devient un segment de chemin : elle ne doit pas
    /// pouvoir remonter d'un cran, ni désigner la racine commune.
    #[test]
    fn a_client_key_becomes_a_safe_directory_name() {
        assert_eq!(client_dir_name("abc-def_ghi"), "abc-def_ghi");
        assert_eq!(client_dir_name("../../etc"), "etc");
        assert_eq!(client_dir_name("  "), "default");
        assert_eq!(client_dir_name(""), "default");
    }

    /// L'installation d'avant l'isolation appartient au premier client
    /// rencontré — il n'y en avait qu'un — mais surtout pas `provisioning.json`,
    /// qui doit rester commun pour que le démarrage suivant sache quel client
    /// ouvrir.
    #[test]
    fn the_legacy_layout_is_adopted_by_the_first_client_only() {
        let root = std::env::temp_dir().join(format!("luuxcraft-layout-test-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("skins")).expect("scratch dir");
        std::fs::write(root.join("settings.json"), b"{}").expect("settings");
        std::fs::write(root.join(crate::provisioning::PERSISTED_FILE), b"{}").expect("provisioning");

        let reserved = [crate::provisioning::PERSISTED_FILE, CLIENTS_DIR];
        let first = root.join(CLIENTS_DIR).join("abc");
        adopt_legacy_layout(&root, &first, &reserved, &[root.as_path()]);

        assert!(first.join("settings.json").is_file(), "les réglages suivent le client");
        assert!(first.join("skins").is_dir(), "la bibliothèque de skins aussi");
        assert!(!root.join("settings.json").exists(), "rien ne reste en double");
        assert!(
            root.join(crate::provisioning::PERSISTED_FILE).is_file(),
            "le provisionnement reste à la racine commune",
        );

        // Le client suivant installé ne doit hériter de rien.
        let second = root.join(CLIENTS_DIR).join("xyz");
        adopt_legacy_layout(&root, &second, &reserved, &[root.as_path()]);
        assert!(!second.exists(), "le second client repart de réglages neufs");

        std::fs::remove_dir_all(&root).ok();
    }

    /// Sous Linux, `app_log_dir` est un sous-dossier d'`app_data_dir` ; sous
    /// Windows, d'`app_cache_dir`. Emporter ce dossier avec les données du
    /// client enverrait les journaux là où plus personne ne les cherche.
    #[test]
    fn a_nested_application_root_is_left_where_it_is() {
        let root = std::env::temp_dir().join(format!("luuxcraft-layout-nested-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let logs = root.join("logs");
        std::fs::create_dir_all(logs.join("game")).expect("scratch dir");
        std::fs::write(root.join("settings.json"), b"{}").expect("settings");

        let scoped = root.join(CLIENTS_DIR).join("abc");
        adopt_legacy_layout(&root, &scoped, &[CLIENTS_DIR], &[root.as_path(), logs.as_path()]);

        assert!(scoped.join("settings.json").is_file(), "les réglages suivent le client");
        assert!(logs.join("game").is_dir(), "les journaux restent à leur racine");
        assert!(!scoped.join("logs").exists());

        std::fs::remove_dir_all(&root).ok();
    }

    /// Relancer la migration ne doit pas re-vider la racine par-dessus des
    /// données déjà rangées : l'existence du dossier suffit à dire « c'est
    /// fait ».
    #[test]
    fn the_migration_runs_only_once() {
        let root = std::env::temp_dir().join(format!("luuxcraft-layout-once-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let scoped = root.join(CLIENTS_DIR).join("abc");
        std::fs::create_dir_all(&scoped).expect("scratch dir");
        std::fs::write(scoped.join("settings.json"), b"{\"kept\":true}").expect("settings");
        std::fs::write(root.join("settings.json"), b"{\"stale\":true}").expect("stale settings");

        adopt_legacy_layout(&root, &scoped, &[CLIENTS_DIR], &[root.as_path()]);

        assert_eq!(
            std::fs::read(scoped.join("settings.json")).expect("settings"),
            b"{\"kept\":true}",
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
