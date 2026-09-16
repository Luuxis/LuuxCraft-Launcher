//! Le pack client : à quel tenant ce moteur appartient-il ?
//!
//! Le binaire est **générique**. Rien du tenant n'est compilé dedans, rien
//! n'est demandé au réseau pour le découvrir : tout est attendu à côté de lui,
//! sur le disque.
//!
//! ```text
//! <racine installée>/
//!   engine/          le binaire du moteur (celui-ci)
//!   client/          le pack client
//!     client_config.json
//!     icon.png
//! ```
//!
//! Cette arborescence est écrite par l'installeur : le bootstrap Windows
//! (`bootstrap/` dans ce dépôt) ou le script `sh` que le panel sert pour Linux
//! et macOS. Ni l'un ni l'autre ne revient ensuite — le moteur se met à jour
//! lui-même, et ne remplace jamais que `engine/`.
//!
//! Le pack est cherché **relativement à l'exécutable**, jamais dans un dossier
//! de données : c'est ce qui fait que deux clients installés côte à côte sur la
//! même machine lisent chacun le leur, et qu'un moteur remplacé retrouve son
//! tenant sans rien réécrire — seul `engine/` change, le pack posé à côté reste
//! en place.
//!
//! Sous Linux le moteur est une AppImage : `current_exe()` y désigne le binaire
//! *à l'intérieur* du point de montage temporaire, pas le fichier installé. Le
//! runtime AppImage publie le vrai chemin dans `$APPIMAGE`, et c'est le seul
//! endroit où le trouver (voir `appimage_path`).
//!
//! ## macOS : le pack vit hors du bundle
//!
//! Un `.app` est remplacé **en entier** quand le moteur se met à jour : tout ce
//! qui serait posé dedans disparaîtrait avec lui, et y écrire casserait de
//! toute façon sa signature. Le pack est donc rangé à côté, sous le dossier de
//! données de l'utilisateur, dans un dossier dont le nom est l'empreinte du
//! chemin du bundle :
//!
//! ```text
//! ~/Library/Application Support/luuxcraft/installs/<clé>/client_config.json
//! ```
//!
//! La clé est déterministe — le script d'installation calcule la même — donc il
//! n'y a aucun index partagé à tenir à jour, rien à verrouiller, et deux
//! serveurs installés côte à côte ne peuvent pas se confondre. Déplacer le
//! `.app` change sa clé : le repli ci-dessous rattrape ce cas tant qu'une seule
//! installation existe.
//!
//! Il n'y a **aucun autre repli** : pas d'appairage, pas de tenant compilé. Un
//! pack absent ou illisible est une erreur fatale et explicite — un moteur qui
//! démarre sans savoir à qui il parle n'a rien à afficher, et deviner le
//! mènerait à parler au mauvais panel.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Nom du fichier d'identité dans le pack, figé par le contrat.
pub const CLIENT_CONFIG_FILE: &str = "client_config.json";
/// Dossier du pack client, à côté de `engine/`.
const CLIENT_DIR: &str = "client";
/// Dossier des identités d'installation macOS, sous le dossier de données.
/// Doit rester identique à `MACOS_INSTALL_DIR` côté panel.
#[cfg(target_os = "macos")]
const MACOS_INSTALL_DIR: &str = "luuxcraft/installs";
/// Longueur de la clé d'installation macOS, en caractères hexadécimaux.
/// Doit rester identique à `INSTALL_KEY_LENGTH` côté panel.
const INSTALL_KEY_LENGTH: usize = 32;
/// Seule version du format que ce moteur sait lire.
const SCHEMA: u32 = 1;
/// Au-delà, ce n'est plus un fichier d'identité : on ne le charge pas.
const MAX_CONFIG_BYTES: u64 = 64 * 1024;
/// Longueur maximale du slug, alignée sur `launcher_configs.slug` du panel.
const MAX_SLUG_LEN: usize = 48;

/// Titre et icône de la fenêtre, tels que le panel les a figés dans le pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowIdentity {
    pub title: String,
    /// Nom de fichier **simple**, à résoudre dans le dossier du pack.
    pub icon: String,
}

/// L'identité du tenant, lue une fois au démarrage et partagée en lecture seule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// `users.id` du panel. C'est lui qui voyage dans `/user/{id}/…`.
    pub tenant_id: String,
    /// `launcher_configs.slug` : nom de dossier, jamais dérivé côté client.
    pub slug: String,
    pub display_name: String,
    /// Base de l'API du panel, sans slash final (`https://…/api`).
    pub api_base_url: String,
    pub window: WindowIdentity,
    pub updated_at: Option<String>,
    /// Dossier d'où le pack a été lu, pour y résoudre l'icône.
    pub dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawClientConfig {
    schema: u32,
    tenant_id: String,
    slug: String,
    display_name: String,
    api_base_url: String,
    #[serde(default)]
    window: Option<RawWindow>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawWindow {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    icon: Option<String>,
}

impl ClientConfig {
    /// Retrouve le pack à côté de l'exécutable et le lit.
    ///
    /// En debug il n'y a pas d'installation : le pack est celui posé dans
    /// `data/client/` du dépôt, au même endroit que le reste des données de
    /// développement (voir `config::Paths::resolve`).
    pub fn resolve() -> Result<Self, String> {
        if cfg!(debug_assertions) {
            return Self::read(&dev_pack_dir());
        }

        let exe = installed_exe().ok_or_else(|| {
            "cannot locate the engine executable, so the client pack cannot be found".to_owned()
        })?;
        let mut candidates = pack_dir_candidates(&exe);
        candidates.extend(external_pack_dirs(&exe));
        for dir in &candidates {
            if dir.join(CLIENT_CONFIG_FILE).is_file() {
                return Self::read(dir);
            }
        }
        Err(format!(
            "no {CLIENT_CONFIG_FILE} found next to {}; looked in: {}",
            exe.display(),
            candidates
                .iter()
                .map(|dir| dir.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ))
    }

    /// Lit et valide `<dir>/client_config.json`.
    pub fn read(dir: &Path) -> Result<Self, String> {
        let path = dir.join(CLIENT_CONFIG_FILE);
        let length = std::fs::metadata(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?
            .len();
        if length > MAX_CONFIG_BYTES {
            return Err(format!(
                "{} is {length} bytes, which is not a client configuration",
                path.display(),
            ));
        }
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Self::parse(&bytes, dir).map_err(|reason| format!("{}: {reason}", path.display()))
    }

    /// Valide le contenu du fichier. Séparée de la lecture pour être testable
    /// sans toucher au disque.
    pub fn parse(bytes: &[u8], dir: &Path) -> Result<Self, String> {
        let raw: RawClientConfig =
            serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if raw.schema != SCHEMA {
            return Err(format!(
                "unsupported schema {}, this engine reads {SCHEMA}",
                raw.schema,
            ));
        }

        let tenant_id = raw.tenant_id.trim().to_owned();
        // L'identifiant voyage dans un chemin d'URL : le borner à l'alphabet des
        // identifiants du panel évite qu'un pack bricolé n'aille interroger
        // autre chose que la configuration de son tenant.
        if tenant_id.is_empty()
            || tenant_id.len() > 128
            || !tenant_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err("tenant_id is not a panel identifier".to_owned());
        }

        let slug = raw.slug.trim().to_owned();
        // Le slug est un nom de dossier de données : il ne doit pas pouvoir
        // remonter d'un cran, ni désigner autre chose qu'un segment de chemin.
        if slug.is_empty()
            || slug.len() > MAX_SLUG_LEN
            || slug.starts_with('-')
            || slug.ends_with('-')
            || !slug
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err("slug must be lowercase [a-z0-9-] without leading or trailing dash"
                .to_owned());
        }

        let display_name = raw.display_name.trim().to_owned();
        if display_name.is_empty() {
            return Err("display_name is empty".to_owned());
        }

        let api_base_url = raw.api_base_url.trim().trim_end_matches('/').to_owned();
        // L'exigence d'https est la seule protection réelle du pack, qui n'est
        // pas signé : elle empêche de détourner un moteur vers un panel en clair.
        if !api_base_url.starts_with("https://") || api_base_url.len() <= "https://".len() {
            return Err(format!("api_base_url must use https: {api_base_url}"));
        }

        let raw_window = raw.window.unwrap_or_default();
        let title = raw_window
            .title
            .map(|title| title.trim().to_owned())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| display_name.clone());
        let icon = raw_window
            .icon
            .map(|icon| icon.trim().to_owned())
            .filter(|icon| !icon.is_empty())
            .unwrap_or_else(|| "icon.png".to_owned());
        // L'icône est jointe au dossier du pack : un nom composé y ferait lire
        // un fichier arbitraire de la machine du joueur.
        if !is_plain_file_name(&icon) {
            return Err(format!("window.icon must be a plain file name: {icon}"));
        }

        Ok(Self {
            tenant_id,
            slug,
            display_name,
            api_base_url,
            window: WindowIdentity { title, icon },
            updated_at: raw.updated_at,
            dir: dir.to_path_buf(),
        })
    }

    /// Chemin de l'icône de fenêtre dans le pack.
    pub fn icon_path(&self) -> PathBuf {
        self.dir.join(&self.window.icon)
    }

    #[cfg(test)]
    pub fn sample() -> Self {
        Self {
            tenant_id: "00000000-0000-4000-8000-000000000001".to_owned(),
            slug: "mon-serveur".to_owned(),
            display_name: "Mon Serveur".to_owned(),
            api_base_url: "https://luuxcraft.fr/api".to_owned(),
            window: WindowIdentity {
                title: "Mon Serveur".to_owned(),
                icon: "icon.png".to_owned(),
            },
            updated_at: None,
            dir: PathBuf::from("/tmp/luuxcraft-sample-client"),
        }
    }
}

/// Un nom de fichier et rien d'autre : ni séparateur, ni remontée de dossier.
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
}

/// Chemin de l'AppImage en cours d'exécution.
///
/// Une AppImage se monte avant de s'exécuter : `current_exe()` y désigne le
/// binaire *à l'intérieur* du point de montage temporaire
/// (`/tmp/.mount_XXXXXX/usr/bin/…`), qui disparaît à la fermeture. Le runtime
/// AppImage publie le vrai chemin du fichier installé dans `$APPIMAGE` — c'est
/// à la fois là qu'il faut chercher le pack client, et la seule valeur qu'une
/// entrée de bureau puisse mettre dans son `Exec=` (voir `desktop`).
#[cfg(target_os = "linux")]
pub fn appimage_path() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("APPIMAGE")?);
    if !path.is_absolute() {
        return None;
    }
    Some(path)
}

/// Ailleurs, l'exécutable courant *est* le fichier installé.
#[cfg(not(target_os = "linux"))]
pub fn appimage_path() -> Option<PathBuf> {
    None
}

/// Le binaire tel qu'il est posé sur le disque, AppImage comprise.
pub fn installed_exe() -> Option<PathBuf> {
    appimage_path().or_else(|| std::env::current_exe().ok())
}

/// Dossier du dépôt qui tient lieu d'installation en debug.
fn dev_pack_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap_or(&manifest)
        .join("data")
        .join(CLIENT_DIR)
}

/// Où chercher le pack, à partir du chemin du binaire installé.
///
/// Pure fonction de chemins, testable sans dépendre de l'OS courant. L'ordre
/// compte : la disposition du contrat (`engine/` et `client/` côte à côte) est
/// essayée avant celle du bundle macOS, elle-même avant le cas d'un moteur posé
/// directement à la racine de son installation.
pub fn pack_dir_candidates(exe: &Path) -> Vec<PathBuf> {
    let Some(dir) = exe.parent() else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    if let Some(root) = dir.parent() {
        candidates.push(root.join(CLIENT_DIR));
        // `<bundle>.app/Contents/MacOS/<binaire>` → `Contents/Resources/client`,
        // le seul endroit d'un bundle où poser des ressources.
        candidates.push(root.join("Resources").join(CLIENT_DIR));
        // `<bundle>.app/client` : le pack posé à la racine du bundle, à côté de
        // `Contents`, plutôt qu'à l'intérieur.
        if let Some(bundle) = root.parent() {
            candidates.push(bundle.join(CLIENT_DIR));
        }
    }
    candidates.push(dir.join(CLIENT_DIR));
    candidates
}

/// Clé d'identité d'une installation macOS : l'empreinte de son chemin.
///
/// Déterministe et calculée des deux côtés — ici, et dans le script
/// d'installation servi par le panel. C'est ce qui évite un index partagé, avec
/// les entrées périmées et les écritures concurrentes qu'il traînerait.
/// Compilée partout pour que son test de conformité au script d'installation
/// tourne aussi sur l'intégration continue Linux ; seul macOS l'appelle.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub fn install_key(path: &Path) -> String {
    let digest = Sha256::digest(path.to_string_lossy().as_bytes());
    let mut key = String::with_capacity(INSTALL_KEY_LENGTH);
    for byte in digest.iter() {
        if key.len() >= INSTALL_KEY_LENGTH {
            break;
        }
        key.push_str(&format!("{byte:02x}"));
    }
    key.truncate(INSTALL_KEY_LENGTH);
    key
}

/// Le `.app` qui contient cet exécutable, s'il y en a un.
///
/// `<bundle>.app/Contents/MacOS/<binaire>` remonte de trois crans. La
/// vérification du suffixe évite de prendre n'importe quel dossier grand-parent
/// pour un bundle quand le moteur tourne hors bundle (tests, build local).
#[cfg(target_os = "macos")]
fn enclosing_bundle(exe: &Path) -> Option<PathBuf> {
    let bundle = exe.parent()?.parent()?.parent()?;
    let looks_like_bundle = bundle
        .extension()
        .map(|extension| extension.eq_ignore_ascii_case("app"))
        .unwrap_or(false);
    looks_like_bundle.then(|| bundle.to_path_buf())
}

/// Dossiers de pack situés **hors** de l'installation.
///
/// Uniquement macOS : ailleurs, le pack est posé à côté de l'exécutable et rien
/// ne le remplace (voir l'en-tête de module).
#[cfg(target_os = "macos")]
fn external_pack_dirs(exe: &Path) -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let installs = home
        .join("Library")
        .join("Application Support")
        .join(MACOS_INSTALL_DIR);

    let mut candidates = Vec::new();
    if let Some(bundle) = enclosing_bundle(exe) {
        candidates.push(installs.join(install_key(&bundle)));
    }

    // Repli : le joueur a déplacé ou renommé le `.app`, donc sa clé a changé.
    // Tant qu'une seule installation est enregistrée, il n'y a aucune ambiguïté
    // sur celle à laquelle il appartient. À partir de deux, deviner reviendrait
    // à ouvrir le launcher du mauvais serveur — mieux vaut l'erreur explicite.
    let mut registered: Vec<PathBuf> = std::fs::read_dir(&installs)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join(CLIENT_CONFIG_FILE).is_file())
        .collect();
    if registered.len() == 1 {
        candidates.push(registered.remove(0));
    }

    candidates
}

#[cfg(not(target_os = "macos"))]
fn external_pack_dirs(_exe: &Path) -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
        "schema": 1,
        "tenant_id": "00000000-0000-4000-8000-000000000001",
        "slug": "mon-serveur",
        "display_name": "Mon Serveur",
        "api_base_url": "https://panel.example/api",
        "window": { "title": "Mon Serveur", "icon": "icon.png" },
        "updated_at": "2026-09-16T10:00:00.000Z"
    }"#;

    fn parse(json: &str) -> Result<ClientConfig, String> {
        ClientConfig::parse(json.as_bytes(), Path::new("/opt/mon-serveur/client"))
    }

    #[test]
    fn reads_the_contract_shape() {
        let client = parse(VALID).expect("valid client pack");
        assert_eq!(client.tenant_id, "00000000-0000-4000-8000-000000000001");
        assert_eq!(client.slug, "mon-serveur");
        assert_eq!(client.display_name, "Mon Serveur");
        assert_eq!(client.api_base_url, "https://panel.example/api");
        assert_eq!(client.window.title, "Mon Serveur");
        assert_eq!(
            client.icon_path(),
            PathBuf::from("/opt/mon-serveur/client/icon.png"),
        );
    }

    /// Le bloc `window` est un agrément : sans lui, la fenêtre porte le nom
    /// d'affichage et l'icône par défaut du pack.
    #[test]
    fn the_window_block_is_optional() {
        let client = parse(
            r#"{"schema":1,"tenant_id":"abc","slug":"a","display_name":"Mon Serveur",
                "api_base_url":"https://a.fr/api"}"#,
        )
        .expect("valid client pack");
        assert_eq!(client.window.title, "Mon Serveur");
        assert_eq!(client.window.icon, "icon.png");
    }

    #[test]
    fn a_trailing_slash_is_dropped_from_the_api_base() {
        let client = parse(&VALID.replace(
            "https://panel.example/api",
            "https://panel.example/api/",
        ))
        .expect("valid client pack");
        assert_eq!(client.api_base_url, "https://panel.example/api");
    }

    /// Le pack n'est pas signé : un panel en clair détournerait le moteur sans
    /// que rien ne s'y oppose.
    #[test]
    fn plain_http_is_refused() {
        assert!(parse(&VALID.replace("https://panel", "http://panel")).is_err());
    }

    #[test]
    fn an_unknown_schema_is_refused() {
        assert!(parse(&VALID.replace("\"schema\": 1", "\"schema\": 2")).is_err());
    }

    /// Le slug devient un nom de dossier de données : il ne doit pas pouvoir
    /// remonter d'un cran ni porter de séparateur.
    #[test]
    fn a_slug_that_would_escape_its_parent_is_refused() {
        for slug in ["../etc", "Mon Serveur", "mon/serveur", "-abc", "abc-", ""] {
            assert!(
                parse(&VALID.replace("\"mon-serveur\"", &format!("\"{slug}\""))).is_err(),
                "slug {slug:?} must be refused",
            );
        }
    }

    /// L'identifiant voyage dans un chemin d'URL du panel.
    #[test]
    fn a_tenant_id_that_would_escape_the_url_is_refused() {
        assert!(parse(&VALID.replace("00000000-0000-4000-8000-000000000001", "../admin")).is_err());
    }

    /// L'icône est jointe au dossier du pack : un chemin y ferait lire
    /// n'importe quel fichier de la machine du joueur.
    #[test]
    fn an_icon_outside_the_pack_is_refused() {
        for icon in ["../../secret.png", "/etc/passwd", "C:\\secret.png", ".."] {
            assert!(
                parse(&VALID.replace("\"icon.png\"", &format!("{icon:?}"))).is_err(),
                "icon {icon:?} must be refused",
            );
        }
    }

    /// La clé `client_id` est interdite par le contrat (elle désigne l'id
    /// applicatif Microsoft ailleurs dans le projet) : la voir passer ne doit
    /// surtout pas la faire prendre pour l'identifiant du tenant.
    #[test]
    fn a_client_id_key_is_never_read_as_the_tenant() {
        let client = parse(
            r#"{"schema":1,"client_id":"azure-app-id","tenant_id":"abc","slug":"a",
                "display_name":"A","api_base_url":"https://a.fr/api"}"#,
        )
        .expect("valid client pack");
        assert_eq!(client.tenant_id, "abc");
    }

    /// Disposition du contrat : le moteur dans `engine/`, le pack dans
    /// `client/`, côte à côte sous la racine installée.
    #[test]
    fn the_pack_sits_next_to_the_engine_directory() {
        let candidates = pack_dir_candidates(Path::new("/opt/mon-serveur/engine/launcher"));
        assert_eq!(candidates[0], PathBuf::from("/opt/mon-serveur/client"));
    }

    /// macOS : dans un bundle, le pack va dans `Resources`.
    #[test]
    fn the_mac_bundle_keeps_its_pack_in_resources() {
        let candidates =
            pack_dir_candidates(Path::new("/Users/joueur/Applications/Mon Serveur.app/Contents/MacOS/launcher"));
        assert!(candidates.contains(&PathBuf::from(
            "/Users/joueur/Applications/Mon Serveur.app/Contents/Resources/client"
        )));
    }

    /// Un binaire posé à la racine du système n'a pas de dossier parent :
    /// proposer un candidat au-dessus de lui ferait lire hors du disque.
    #[test]
    fn a_binary_at_the_filesystem_root_has_no_candidate_above_it() {
        assert_eq!(
            pack_dir_candidates(Path::new("/launcher")),
            vec![PathBuf::from("/client")],
        );
    }

    /// La clé d'installation est le contrat entre ce moteur et le script
    /// d'installation servi par le panel : `printf '%s' "$APP" | shasum -a 256
    /// | cut -c1-32`. Une valeur figée ici fait échouer le test le jour où l'un
    /// des deux changerait d'algorithme ou de longueur.
    #[test]
    fn the_install_key_matches_the_install_script() {
        assert_eq!(
            install_key(Path::new("/Users/joueur/Applications/Mon Serveur.app")),
            "684af76fda7ae69dc815782b11099716",
        );
        assert_eq!(install_key(Path::new("/a")).len(), INSTALL_KEY_LENGTH);
    }

    /// Deux serveurs installés côte à côte ont deux clés : sans quoi le second
    /// lirait l'identité du premier.
    #[test]
    fn two_bundles_never_share_a_key() {
        let a = install_key(Path::new("/Users/j/Applications/Serveur A.app"));
        let b = install_key(Path::new("/Users/j/Applications/Serveur B.app"));
        assert_ne!(a, b);
    }
}
