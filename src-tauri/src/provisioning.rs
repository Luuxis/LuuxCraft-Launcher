//! À quel panel ce launcher appartient-il ?
//!
//! Un seul launcher est compilé pour tous les clients : tout ce qui distingue
//! un client d'un autre (l'adresse du panel et sa clé publique) est **ajouté à
//! la fin de l'installeur** par le panel au moment du téléchargement. Ce module
//! retrouve cette information, dans cet ordre :
//!
//! 1. **Bloc ajouté à la fin de l'exécutable courant** — le cas d'une AppImage
//!    Linux ou d'un `.exe` portable, qui *sont* le fichier téléchargé.
//! 2. **`provisioning.blob` à côté de l'exécutable** — écrit par le hook NSIS
//!    de l'installeur Windows, qui a relu son propre bloc au moment
//!    d'installer.
//! 3. **`provisioning.json` à côté du bundle `.app`** (macOS) — posé par le
//!    zip que le panel reconstruit à partir du `.app.tar.gz`, jamais dans
//!    `Contents/` pour ne pas casser la signature de code du bundle.
//! 4. **`provisioning.json` du dossier de données** — la copie persistée, qui
//!    survit aux mises à jour (l'installeur téléchargé par l'updater, lui, n'a
//!    ni bloc ni fichier voisin) et au renommage du dossier d'installation.
//! 5. **`LUUXCRAFT_USER_ID` compilé** — pour qui veut vraiment un build dédié.
//! 6. Rien : l'interface demande son code au joueur (le cas du `.dmg` macOS,
//!    pour qui préfère l'installeur traditionnel).
//!
//! Les sources fraîches (1 et 2) passent avant la copie persistée : réinstaller
//! avec l'installeur d'un autre serveur doit changer de serveur, pas garder
//! l'ancien pour l'éternité.
//!
//! Renommer, déplacer ou re-télécharger le fichier ne casse rien : l'identité
//! est dans les octets du fichier, jamais dans son nom.

use std::path::{Path, PathBuf};

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::api::LuuxCraftApi;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Doit rester identique à `PROVISIONING_MAGIC` de `src/lib/provisioning.ts`
/// du panel et au `NSIS_HOOK_POSTINSTALL` de `installer-hooks.nsh`.
const MAGIC: &str = "LUUXCRAFT-PROVISIONING-V1";
/// Taille fixe du bloc : NSIS doit pouvoir se placer à `-FOOTER_SIZE` de la fin
/// sans connaître la longueur de la charge utile.
const FOOTER_SIZE: u64 = 512;
const PAD: char = '.';

/// Nom du fichier écrit par le hook NSIS dans le dossier d'installation.
pub const BLOB_FILE: &str = "provisioning.blob";
/// Nom de la copie persistée dans le dossier de données du launcher.
pub const PERSISTED_FILE: &str = "provisioning.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provisioning {
    /// Base de l'API du panel, sans slash final (`https://…/api`).
    pub api_url: String,
    /// Clé publique du client : `clientId` ou `userId` du panel.
    pub key: String,
}

#[derive(Debug, Deserialize)]
struct RawPayload {
    v: u32,
    #[serde(rename = "apiUrl")]
    api_url: Option<String>,
    key: Option<String>,
}

/// D'où vient la configuration effectivement utilisée, pour les logs et l'écran
/// de diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProvisioningSource {
    /// Bloc lu à la fin de l'exécutable courant.
    Executable,
    /// `provisioning.blob` écrit par l'installeur NSIS.
    Installer,
    /// Copie persistée dans le dossier de données.
    Persisted,
    /// Valeur compilée (`LUUXCRAFT_USER_ID`).
    BuiltIn,
}

/// Valide et normalise un couple (adresse, clé).
///
/// L'exigence d'https est la seule protection réelle du bloc, qui n'est pas
/// signé : elle empêche de détourner un launcher vers un panel en clair. Un
/// bloc douteux doit produire `None` et faire retomber l'appelant sur l'écran
/// d'appairage, jamais laisser le launcher parler à un panel devine.
pub fn normalize(api_url: &str, key: &str) -> Option<Provisioning> {
    let api_url = api_url.trim().trim_end_matches('/');
    let key = key.trim();
    if !api_url.starts_with("https://") || api_url.len() < "https://x".len() {
        return None;
    }
    if key.is_empty() || key.len() > 128 {
        return None;
    }
    // La clé voyage dans un chemin d'URL (`/user/{key}/config`) : la borner à
    // l'alphabet des identifiants du panel évite qu'un bloc bricolé n'aille
    // écrire ailleurs dans l'API.
    if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return None;
    }
    Some(Provisioning {
        api_url: api_url.to_owned(),
        key: key.to_owned(),
    })
}

/// Alphabet base64url : c'est lui qui borne la charge utile, le remplissage
/// `PAD` n'en faisant pas partie.
fn is_base64url(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// Relit un bloc, qu'il vienne de la fin d'un exécutable ou du fichier écrit
/// par l'installeur.
///
/// La recherche se fait sur les **octets**, pas sur une chaîne : la queue d'un
/// exécutable sans bloc n'est pas de l'UTF-8 valide, et un `from_utf8` la
/// rejetterait avant même d'avoir cherché le magic. Les octets nuls sont
/// filtrés au passage, ce qui rend la lecture tolérante à un fichier écrit en
/// UTF-16LE — `FileWrite` de NSIS écrit de l'ANSI, mais on ne veut pas que
/// cette hypothèse soit le seul rempart.
pub fn parse_blob(bytes: &[u8]) -> Option<Provisioning> {
    let cleaned: Vec<u8> = bytes.iter().copied().filter(|byte| *byte != 0).collect();
    let magic = MAGIC.as_bytes();
    let start = cleaned.windows(magic.len()).position(|window| window == magic)? + magic.len();

    let encoded: Vec<u8> = cleaned[start..]
        .iter()
        .copied()
        .take_while(|byte| *byte != PAD as u8 && is_base64url(*byte))
        .collect();
    if encoded.is_empty() {
        return None;
    }

    let decoded = BASE64_URL_SAFE_NO_PAD.decode(&encoded).ok()?;
    let payload: RawPayload = serde_json::from_slice(&decoded).ok()?;
    if payload.v != 1 {
        return None;
    }
    normalize(payload.api_url.as_deref()?, payload.key.as_deref()?)
}

/// Lit les derniers `FOOTER_SIZE` octets d'un fichier et y cherche le bloc.
fn read_footer(path: &Path) -> Option<Provisioning> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if length <= FOOTER_SIZE {
        return None;
    }
    file.seek(SeekFrom::End(-(FOOTER_SIZE as i64))).ok()?;
    let mut buffer = vec![0_u8; FOOTER_SIZE as usize];
    file.read_exact(&mut buffer).ok()?;
    parse_blob(&buffer)
}

fn read_blob_file(path: &Path) -> Option<Provisioning> {
    // Le hook NSIS écrit 512 octets ; la borne large tolère un fichier déposé à
    // la main pour un diagnostic, tout en refusant de charger n'importe quoi.
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 8 * 1024 {
        return None;
    }
    parse_blob(&bytes)
}

fn read_persisted(path: &Path) -> Option<Provisioning> {
    let raw = std::fs::read(path).ok()?;
    let stored: Provisioning = serde_json::from_slice(&raw).ok()?;
    normalize(&stored.api_url, &stored.key)
}

/// Dossier qui contient aussi le bundle `.app`, à partir du chemin de son
/// exécutable (`<bundle>.app/Contents/MacOS/<binaire>`, trois niveaux plus
/// haut). Pure fonction de chemins, testable sans dépendre de l'OS courant ;
/// seul l'appelant (`read_app_bundle_sibling`) est spécifique à macOS.
fn app_bundle_sibling_dir(exe: &Path) -> Option<PathBuf> {
    exe.parent()?.parent()?.parent().map(Path::to_path_buf)
}

/// macOS uniquement : `provisioning.json` posé à côté du bundle `.app` par le
/// zip que le panel reconstruit à partir du `.app.tar.gz` (voir
/// `PublicLauncherController`, route `/api/launcher/download/…/darwin/…`).
///
/// Ce fichier ne touche jamais à `Contents/` : `codesign` scelle le hash de
/// chaque fichier du bundle dans sa signature, et y ajouter quoi que ce soit
/// après coup la casserait (Gatekeeper refuserait de lancer l'application).
/// C'est pour ça que ce provisionnement vit hors du bundle plutôt que dans un
/// footer comme sur Windows/Linux — un `.app` n'a de toute façon pas de
/// « fin » unique où en ajouter un, c'est une arborescence de fichiers.
#[cfg(target_os = "macos")]
fn read_app_bundle_sibling() -> Option<Provisioning> {
    let exe = std::env::current_exe().ok()?;
    let dir = app_bundle_sibling_dir(&exe)?;
    read_persisted(&dir.join(PERSISTED_FILE))
}

#[cfg(not(target_os = "macos"))]
fn read_app_bundle_sibling() -> Option<Provisioning> {
    None
}

/// Écrit la copie persistée, celle qui survivra aux mises à jour.
pub fn persist(launcher_dir: &Path, provisioning: &Provisioning) -> std::io::Result<()> {
    std::fs::create_dir_all(launcher_dir)?;
    let json = serde_json::to_vec_pretty(provisioning).map_err(std::io::Error::other)?;
    std::fs::write(launcher_dir.join(PERSISTED_FILE), json)
}

/// Oublie le provisionnement : le prochain démarrage redemandera un code.
pub fn forget(launcher_dir: &Path) -> std::io::Result<()> {
    let path = launcher_dir.join(PERSISTED_FILE);
    match std::fs::remove_file(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Retrouve la configuration du client selon l'ordre documenté en tête de
/// module, et persiste ce qui vient d'être découvert.
pub fn resolve(launcher_dir: &Path) -> Option<(Provisioning, ProvisioningSource)> {
    let executable: Option<PathBuf> = std::env::current_exe().ok();

    if let Some(path) = executable.as_deref() {
        if let Some(found) = read_footer(path) {
            let _ = persist(launcher_dir, &found);
            return Some((found, ProvisioningSource::Executable));
        }
    }

    if let Some(directory) = executable.as_deref().and_then(Path::parent) {
        if let Some(found) = read_blob_file(&directory.join(BLOB_FILE)) {
            let _ = persist(launcher_dir, &found);
            return Some((found, ProvisioningSource::Installer));
        }
    }

    if let Some(found) = read_app_bundle_sibling() {
        let _ = persist(launcher_dir, &found);
        return Some((found, ProvisioningSource::Installer));
    }

    if let Some(found) = read_persisted(&launcher_dir.join(PERSISTED_FILE)) {
        return Some((found, ProvisioningSource::Persisted));
    }

    None
}

/// Ce que l'interface a besoin de savoir pour décider entre afficher le
/// launcher et afficher l'écran d'appairage.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisioningStatus {
    pub provisioned: bool,
    pub source: Option<ProvisioningSource>,
    /// Panel auquel le launcher parle (sans la clé), pour l'écran de diagnostic.
    pub api_url: String,
    /// Clé du client, affichée dans les paramètres pour qu'il puisse la relire.
    pub key: Option<String>,
}

#[tauri::command]
pub fn provisioning_status(state: State<'_, AppState>) -> ProvisioningStatus {
    ProvisioningStatus {
        provisioned: state.config.is_provisioned(),
        source: state.provisioning_source,
        api_url: state.config.api.base_url.clone(),
        key: state
            .config
            .is_provisioned()
            .then(|| state.config.user_id.clone()),
    }
}

/// Appaire le launcher au code saisi par le joueur.
///
/// Le code est **vérifié contre le panel avant d'être enregistré** : une faute
/// de frappe doit être rattrapée sur l'écran de saisie, pas au redémarrage
/// suivant, où le joueur n'aurait qu'un launcher vide et aucun moyen de
/// comprendre pourquoi.
///
/// L'adresse du panel reste celle compilée : elle est générique, un client n'a
/// que son code à fournir.
#[tauri::command]
pub async fn provisioning_set(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
) -> AppResult<()> {
    let candidate = normalize(&state.config.api.base_url, &code).ok_or_else(|| {
        AppError::new(
            "provisioning_invalid",
            "the pairing code is not a valid client key",
        )
    })?;

    let mut probe = state.config.clone();
    probe.apply_provisioning(&candidate);
    LuuxCraftApi::new(state.http.inner().clone(), &probe)
        .config()
        .await?;

    persist(&state.paths.launcher_dir, &candidate).map_err(|error| {
        AppError::new("provisioning_write", "cannot save the pairing")
            .with_details(error.to_string())
    })?;
    log::info!("launcher paired with {}", candidate.key);

    // La configuration est lue une fois au démarrage et partagée en lecture
    // seule par tout le backend ; redémarrer est plus simple, et plus sûr, que
    // de la rendre mutable pour un évènement qui n'arrive qu'une fois.
    app.restart();
}

/// Oublie l'appairage et redémarre sur l'écran de saisie.
#[tauri::command]
pub fn provisioning_forget(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    forget(&state.paths.launcher_dir).map_err(|error| {
        AppError::new("provisioning_write", "cannot clear the pairing")
            .with_details(error.to_string())
    })?;
    log::info!("launcher pairing cleared");
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(payload: &str) -> Vec<u8> {
        let encoded = BASE64_URL_SAFE_NO_PAD.encode(payload.as_bytes());
        let body = format!("{MAGIC}{encoded}");
        let mut text = body.clone();
        text.push_str(&PAD.to_string().repeat(FOOTER_SIZE as usize - body.len()));
        text.into_bytes()
    }

    #[test]
    fn reads_a_well_formed_blob() {
        let bytes = blob(r#"{"v":1,"apiUrl":"https://luuxcraft.fr/api","key":"abc-def-ghi"}"#);
        let parsed = parse_blob(&bytes).expect("blob is readable");
        assert_eq!(parsed.api_url, "https://luuxcraft.fr/api");
        assert_eq!(parsed.key, "abc-def-ghi");
    }

    #[test]
    fn trailing_slash_is_dropped() {
        let bytes = blob(r#"{"v":1,"apiUrl":"https://luuxcraft.fr/api/","key":"abc"}"#);
        assert_eq!(parse_blob(&bytes).unwrap().api_url, "https://luuxcraft.fr/api");
    }

    #[test]
    fn plain_http_is_refused() {
        let bytes = blob(r#"{"v":1,"apiUrl":"http://luuxcraft.fr/api","key":"abc"}"#);
        assert!(parse_blob(&bytes).is_none());
    }

    #[test]
    fn a_key_that_would_escape_the_url_is_refused() {
        let bytes = blob(r#"{"v":1,"apiUrl":"https://luuxcraft.fr/api","key":"../admin"}"#);
        assert!(parse_blob(&bytes).is_none());
    }

    #[test]
    fn an_unknown_version_is_refused() {
        let bytes = blob(r#"{"v":2,"apiUrl":"https://luuxcraft.fr/api","key":"abc"}"#);
        assert!(parse_blob(&bytes).is_none());
    }

    #[test]
    fn bytes_without_the_magic_are_ignored() {
        assert!(parse_blob(b"not a provisioning blob at all").is_none());
    }

    /// Le bloc est cherché *dans* les derniers octets, pas exactement à leur
    /// début : un installeur dont la taille a été alignée par l'outil de
    /// signature garde un bloc lisible.
    #[test]
    fn the_magic_is_found_even_when_not_at_offset_zero() {
        let mut bytes = b"\0\0\0\0".to_vec();
        bytes.extend_from_slice(&blob(r#"{"v":1,"apiUrl":"https://a.fr/api","key":"k"}"#));
        assert_eq!(parse_blob(&bytes).unwrap().key, "k");
    }

    /// La queue d'un exécutable n'est pas de l'UTF-8 valide : chercher sur les
    /// octets est ce qui permet de trouver le bloc quand même.
    #[test]
    fn binary_noise_before_the_block_is_skipped() {
        let mut bytes = vec![0xff, 0xfe, 0x00, 0x80, 0x90];
        bytes.extend_from_slice(&blob(r#"{"v":1,"apiUrl":"https://a.fr/api","key":"k"}"#));
        assert_eq!(parse_blob(&bytes).unwrap().key, "k");
    }

    /// Filtrer les octets nuls rend la lecture tolérante à un fichier écrit en
    /// UTF-16LE, au cas où `FileWrite` de NSIS ne ferait pas de l'ANSI.
    #[test]
    fn a_utf16le_blob_file_is_still_readable() {
        let ascii = blob(r#"{"v":1,"apiUrl":"https://a.fr/api","key":"k"}"#);
        let utf16: Vec<u8> = ascii.iter().flat_map(|byte| [*byte, 0]).collect();
        assert_eq!(parse_blob(&utf16).unwrap().key, "k");
    }

    /// `<bundle>.app/Contents/MacOS/<binaire>` → trois niveaux plus haut, le
    /// dossier qui contient aussi `<bundle>.app` — la racine d'extraction du
    /// zip que le panel construit pour macOS.
    #[test]
    fn app_bundle_sibling_dir_is_three_levels_above_the_executable() {
        let exe = Path::new("/Users/joueur/Downloads/LuuxCraft Launcher.app/Contents/MacOS/luuxcraft-launcher");
        assert_eq!(
            app_bundle_sibling_dir(exe),
            Some(PathBuf::from("/Users/joueur/Downloads")),
        );
    }

    #[test]
    fn app_bundle_sibling_dir_is_none_for_a_path_too_shallow_to_be_a_bundle() {
        assert_eq!(app_bundle_sibling_dir(Path::new("/binary")), None);
        assert_eq!(app_bundle_sibling_dir(Path::new("/a/binary")), None);
    }

    /// Le fichier doit être un JSON `{apiUrl, key}` ordinaire, pas le format à
    /// base64 des footers : rien ne le contraint en taille ou en alphabet
    /// puisqu'il n'est jamais embarqué dans un autre binaire.
    #[test]
    fn app_bundle_sibling_reads_plain_json_like_the_persisted_copy() {
        let dir = std::env::temp_dir().join(format!(
            "luuxcraft-provisioning-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        std::fs::write(
            dir.join(PERSISTED_FILE),
            br#"{"apiUrl":"https://luuxcraft.fr/api","key":"abc-def-ghi"}"#,
        )
        .expect("write sibling file");

        let found = read_persisted(&dir.join(PERSISTED_FILE)).expect("readable");
        assert_eq!(found.api_url, "https://luuxcraft.fr/api");
        assert_eq!(found.key, "abc-def-ghi");

        std::fs::remove_dir_all(&dir).ok();
    }
}
