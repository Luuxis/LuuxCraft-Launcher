//! À quel panel ce launcher appartient-il ?
//!
//! Un seul launcher est compilé pour tous les clients : tout ce qui distingue
//! un client d'un autre (l'adresse du panel et sa clé publique) est **ajouté à
//! la fin de l'installeur** par le panel au moment du téléchargement. Ce module
//! retrouve cette information, dans cet ordre :
//!
//! 1. **Bloc ajouté à la fin du fichier téléchargé** — le cas d'une AppImage
//!    Linux ou d'un `.exe` portable, qui *sont* le fichier téléchargé. Sous
//!    Linux ce fichier n'est pas `current_exe()`, qui pointe l'intérieur du
//!    point de montage de l'AppImage : voir `appimage_path`.
//! 2. **`provisioning.blob` à côté de l'exécutable** — écrit par le hook NSIS
//!    de l'installeur Windows, qui a relu son propre bloc au moment
//!    d'installer.
//! 3. **`Contents/Resources/provisioning.json`** (macOS) — posé *dans* le
//!    bundle par le zip que le panel reconstruit à partir du `.app.tar.gz`.
//!    C'est ce qui permet au joueur de ne rien faire d'autre que glisser le
//!    `.app` dans Applications : la configuration voyage avec lui.
//! 4. **`provisioning.json` à côté du bundle `.app`** (macOS) — l'emplacement
//!    des premiers zips du panel, gardé pour ceux déjà téléchargés.
//! 5. **`provisioning.json` du dossier de données** — la copie persistée, qui
//!    survit aux mises à jour (le bundle téléchargé par l'updater, lui, n'a ni
//!    bloc ni fichier de configuration) et au renommage du dossier
//!    d'installation.
//! 6. **`LUUXCRAFT_USER_ID` compilé** — pour qui veut vraiment un build dédié.
//! 7. Rien : l'interface demande son code au joueur (le cas du `.dmg` macOS,
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
/// En-tête de la seconde ligne du bloc, celle que lit aussi `installer-hooks.nsh`.
/// Doit rester identique à `PROVISIONING_BRAND_MAGIC` du panel.
const BRAND_MAGIC: &str = "LUUXCRAFT-BRAND-V1";
/// Taille fixe du bloc : NSIS doit pouvoir se placer à `-FOOTER_SIZE` de la fin
/// sans connaître la longueur de la charge utile.
const FOOTER_SIZE: u64 = 512;
const PAD: char = '.';
const FIELD_SEPARATOR: char = '|';

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

/// Nom et logo du client, lus dans la seconde ligne du bloc.
///
/// C'est la même ligne que celle dont le hook NSIS tire le nom du raccourci, et
/// elle sert ici à deux choses que `/config` ne peut pas rendre à temps :
///
/// - ouvrir la fenêtre au nom du client **avant** la première requête, y
///   compris au tout premier démarrage où aucun snapshot n'est encore en
///   cache ;
/// - poser l'entrée de bureau Linux dès la première exécution, même hors
///   ligne (voir `desktop`).
///
/// Elle est facultative : un client sans nom exploitable n'en a pas, et le
/// launcher retombe alors sur ce que le panel lui dira.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Brand {
    pub name: String,
    /// URL absolue https du logo, ou `None`.
    pub icon_url: Option<String>,
}

/// Ce que le bloc a livré : le provisionnement, d'où il vient, et la marque
/// quand elle l'accompagnait.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub provisioning: Provisioning,
    pub source: ProvisioningSource,
    pub brand: Option<Brand>,
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

/// Chemin de l'AppImage en cours d'exécution.
///
/// Une AppImage se monte avant de s'exécuter : `current_exe()` y désigne le
/// binaire *à l'intérieur* du point de montage temporaire
/// (`/tmp/.mount_XXXXXX/usr/bin/…`), pas le fichier `.AppImage` que le joueur a
/// téléchargé et auquel le panel a ajouté le bloc. Le runtime AppImage publie
/// le vrai chemin dans `$APPIMAGE`, et c'est le seul endroit où le trouver.
#[cfg(target_os = "linux")]
pub fn appimage_path() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("APPIMAGE")?);
    if !path.is_absolute() {
        return None;
    }
    Some(path)
}

/// Ailleurs, l'exécutable courant *est* le fichier téléchargé.
#[cfg(not(target_os = "linux"))]
pub fn appimage_path() -> Option<PathBuf> {
    None
}

/// Fichier susceptible de porter le bloc : l'AppImage sous Linux, l'exécutable
/// courant partout ailleurs.
fn downloaded_file() -> Option<PathBuf> {
    appimage_path().or_else(|| std::env::current_exe().ok())
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

/// Relit la ligne de marque du bloc, `LUUXCRAFT-BRAND-V1|<nom>|<url>|`.
///
/// Même découpe que celle du hook NSIS, et mêmes garanties : les deux champs
/// sont bornés côté panel à l'alphabet de `sanitizeBrandName` et d'une URL
/// https, donc sans `|` qui casserait le partage. Une ligne absente, tronquée
/// ou sans nom donne `None` — la marque est un agrément, jamais une condition.
pub fn parse_brand(bytes: &[u8]) -> Option<Brand> {
    let cleaned: Vec<u8> = bytes.iter().copied().filter(|byte| *byte != 0).collect();
    let magic = BRAND_MAGIC.as_bytes();
    let start = cleaned
        .windows(magic.len())
        .position(|window| window == magic)?
        + magic.len();

    // La ligne s'arrête à la fin de ligne, ou au remplissage quand le panel n'a
    // pas eu la place d'en écrire une.
    let line: String = cleaned[start..]
        .iter()
        .copied()
        .take_while(|byte| !matches!(*byte, b'\n' | b'\r') && *byte != PAD as u8)
        .map(char::from)
        .collect();

    let mut fields = line.split(FIELD_SEPARATOR).skip(1);
    let name = fields.next()?.trim();
    if name.is_empty() {
        return None;
    }
    let icon_url = fields
        .next()
        .map(str::trim)
        .filter(|url| url.starts_with("https://"))
        .map(str::to_owned);

    Some(Brand {
        name: name.to_owned(),
        icon_url,
    })
}

/// Lit les derniers `FOOTER_SIZE` octets d'un fichier et y cherche le bloc.
fn read_footer(path: &Path) -> Option<(Provisioning, Option<Brand>)> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if length <= FOOTER_SIZE {
        return None;
    }
    file.seek(SeekFrom::End(-(FOOTER_SIZE as i64))).ok()?;
    let mut buffer = vec![0_u8; FOOTER_SIZE as usize];
    file.read_exact(&mut buffer).ok()?;
    Some((parse_blob(&buffer)?, parse_brand(&buffer)))
}

/// `provisioning.blob`, écrit par le hook NSIS. Il ne contient que la première
/// ligne du bloc — sous Windows, le nom du client est déjà passé dans le
/// raccourci et dans le registre au moment de l'installation.
fn read_blob_file(path: &Path) -> Option<(Provisioning, Option<Brand>)> {
    // Le hook NSIS écrit 512 octets ; la borne large tolère un fichier déposé à
    // la main pour un diagnostic, tout en refusant de charger n'importe quoi.
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 8 * 1024 {
        return None;
    }
    Some((parse_blob(&bytes)?, parse_brand(&bytes)))
}

fn read_persisted(path: &Path) -> Option<Provisioning> {
    let raw = std::fs::read(path).ok()?;
    let stored: Provisioning = serde_json::from_slice(&raw).ok()?;
    normalize(&stored.api_url, &stored.key)
}

/// `Contents/Resources` du bundle, à partir du chemin de son exécutable
/// (`<bundle>.app/Contents/MacOS/<binaire>`). Pure fonction de chemins,
/// testable sans dépendre de l'OS courant.
fn app_bundle_resources_dir(exe: &Path) -> Option<PathBuf> {
    let contents = exe.parent()?.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    Some(contents.join("Resources"))
}

/// Dossier qui contient aussi le bundle `.app`, à partir du chemin de son
/// exécutable (trois niveaux plus haut).
fn app_bundle_sibling_dir(exe: &Path) -> Option<PathBuf> {
    exe.parent()?.parent()?.parent().map(Path::to_path_buf)
}

/// macOS uniquement : la configuration que le panel a posée dans le bundle.
///
/// `Contents/Resources/provisioning.json` d'abord — c'est là que le zip
/// reconstruit par le panel l'écrit, à l'intérieur du bundle, pour que glisser
/// le `.app` dans Applications suffise et que rien ne soit à saisir. Le fichier
/// voisin ensuite, pour les zips de la première version du panel, qui le
/// posaient à côté du bundle.
///
/// Écrire dans `Contents/` est possible parce que le bundle n'est pas signé :
/// `codesign` scelle le hash de chaque fichier du bundle dans
/// `Contents/_CodeSignature`, mais ce sceau n'existe que si une identité de
/// signature Apple a été fournie au build. Le panel refuse d'ailleurs de
/// personnaliser un bundle qui en porte un (voir `lib/provisioning.ts`).
#[cfg(target_os = "macos")]
fn read_app_bundle() -> Option<Provisioning> {
    let exe = std::env::current_exe().ok()?;
    if let Some(resources) = app_bundle_resources_dir(&exe) {
        if let Some(found) = read_persisted(&resources.join(PERSISTED_FILE)) {
            return Some(found);
        }
    }
    let sibling = app_bundle_sibling_dir(&exe)?;
    read_persisted(&sibling.join(PERSISTED_FILE))
}

#[cfg(not(target_os = "macos"))]
fn read_app_bundle() -> Option<Provisioning> {
    None
}

/// Écrit la copie persistée, celle qui survivra aux mises à jour.
///
/// `shared_dir` est la racine commune à tous les clients, pas le dossier de
/// l'un d'eux : c'est ce fichier qui dit *lequel* ouvrir au démarrage suivant,
/// il ne peut donc pas être rangé dedans (voir `config::Paths`).
pub fn persist(shared_dir: &Path, provisioning: &Provisioning) -> std::io::Result<()> {
    std::fs::create_dir_all(shared_dir)?;
    let json = serde_json::to_vec_pretty(provisioning).map_err(std::io::Error::other)?;
    std::fs::write(shared_dir.join(PERSISTED_FILE), json)
}

/// Oublie le provisionnement : le prochain démarrage redemandera un code.
///
/// Les données du client, elles, restent en place sous `clients/<clé>` : se
/// réappairer au même serveur doit rendre ses réglages et ses comptes, pas
/// repartir de zéro.
pub fn forget(shared_dir: &Path) -> std::io::Result<()> {
    let path = shared_dir.join(PERSISTED_FILE);
    match std::fs::remove_file(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Retrouve la configuration du client selon l'ordre documenté en tête de
/// module, et persiste ce qui vient d'être découvert.
pub fn resolve(shared_dir: &Path) -> Option<Resolved> {
    if let Some(path) = downloaded_file() {
        if let Some((found, brand)) = read_footer(&path) {
            let _ = persist(shared_dir, &found);
            return Some(Resolved {
                provisioning: found,
                source: ProvisioningSource::Executable,
                brand,
            });
        }
    }

    // Le fichier écrit par l'installeur NSIS est posé à côté de l'exécutable
    // installé — pas à côté de l'AppImage, qui n'a pas d'installeur.
    let executable: Option<PathBuf> = std::env::current_exe().ok();
    if let Some(directory) = executable.as_deref().and_then(Path::parent) {
        if let Some((found, brand)) = read_blob_file(&directory.join(BLOB_FILE)) {
            let _ = persist(shared_dir, &found);
            return Some(Resolved {
                provisioning: found,
                source: ProvisioningSource::Installer,
                brand,
            });
        }
    }

    if let Some(found) = read_app_bundle() {
        let _ = persist(shared_dir, &found);
        return Some(Resolved {
            provisioning: found,
            source: ProvisioningSource::Installer,
            brand: None,
        });
    }

    if let Some(found) = read_persisted(&shared_dir.join(PERSISTED_FILE)) {
        return Some(Resolved {
            provisioning: found,
            source: ProvisioningSource::Persisted,
            brand: None,
        });
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

    persist(&state.paths.shared_dir, &candidate).map_err(|error| {
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
    forget(&state.paths.shared_dir).map_err(|error| {
        AppError::new("provisioning_write", "cannot clear the pairing")
            .with_details(error.to_string())
    })?;
    // L'entrée de bureau Linux porte le nom du client : la laisser derrière
    // laisserait un raccourci vers un launcher qui ne sait plus quoi lancer.
    #[cfg(target_os = "linux")]
    crate::desktop::remove_entry(&app, &state.config.user_id);
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

    /// Le bloc complet tel que le panel l'écrit : la ligne de provisionnement,
    /// puis la ligne de marque, puis le remplissage.
    fn footer(payload: &str, brand: Option<&str>) -> Vec<u8> {
        let encoded = BASE64_URL_SAFE_NO_PAD.encode(payload.as_bytes());
        let mut text = format!("{MAGIC}{encoded}\n");
        if let Some(brand) = brand {
            text.push_str(brand);
        }
        text.push_str(&PAD.to_string().repeat(FOOTER_SIZE as usize - text.len()));
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

    /// La marque voyage dans le même bloc que le provisionnement, sur la
    /// seconde ligne : c'est elle qui donne son nom à l'entrée de bureau Linux
    /// dès la première exécution, avant tout appel au panel.
    #[test]
    fn reads_the_brand_line_next_to_the_provisioning_one() {
        let bytes = footer(
            r#"{"v":1,"apiUrl":"https://luuxcraft.fr/api","key":"abc"}"#,
            Some("LUUXCRAFT-BRAND-V1|Mon Serveur|https://luuxcraft.fr/api/user/abc/icon.ico|\n"),
        );
        let brand = parse_brand(&bytes).expect("brand line is readable");
        assert_eq!(brand.name, "Mon Serveur");
        assert_eq!(
            brand.icon_url.as_deref(),
            Some("https://luuxcraft.fr/api/user/abc/icon.ico")
        );
        // La première ligne reste lisible telle quelle : les deux lecteurs du
        // bloc ne se gênent pas.
        assert_eq!(parse_blob(&bytes).unwrap().key, "abc");
    }

    /// Le panel dégrade la ligne quand elle ne tient pas : d'abord sans l'URL
    /// du logo, puis pas du tout. Les deux doivent rester lisibles.
    #[test]
    fn a_brand_without_an_icon_keeps_its_name() {
        let bytes = footer(
            r#"{"v":1,"apiUrl":"https://a.fr/api","key":"k"}"#,
            Some("LUUXCRAFT-BRAND-V1|Mon Serveur||\n"),
        );
        let brand = parse_brand(&bytes).expect("brand line is readable");
        assert_eq!(brand.name, "Mon Serveur");
        assert_eq!(brand.icon_url, None);
    }

    #[test]
    fn a_block_without_a_brand_line_has_no_brand() {
        let bytes = footer(r#"{"v":1,"apiUrl":"https://a.fr/api","key":"k"}"#, None);
        assert!(parse_brand(&bytes).is_none());
    }

    /// Même exigence que pour l'`apiUrl` : le bloc n'est pas signé, un logo
    /// servi en clair n'a rien à faire dans le thème d'icônes du joueur.
    #[test]
    fn a_plain_http_icon_is_dropped_but_the_name_survives() {
        let bytes = footer(
            r#"{"v":1,"apiUrl":"https://a.fr/api","key":"k"}"#,
            Some("LUUXCRAFT-BRAND-V1|Mon Serveur|http://a.fr/icon.ico|\n"),
        );
        let brand = parse_brand(&bytes).expect("brand line is readable");
        assert_eq!(brand.name, "Mon Serveur");
        assert_eq!(brand.icon_url, None);
    }

    /// `<bundle>.app/Contents/MacOS/<binaire>` → `Contents/Resources`, là où le
    /// panel pose la configuration du client. C'est le chemin qui compte : il
    /// suit le bundle quand le joueur le glisse dans Applications.
    #[test]
    fn app_bundle_resources_dir_sits_next_to_the_macos_dir() {
        let exe = Path::new("/Applications/Mon Serveur.app/Contents/MacOS/luuxcraft-launcher");
        assert_eq!(
            app_bundle_resources_dir(exe),
            Some(PathBuf::from("/Applications/Mon Serveur.app/Contents/Resources")),
        );
    }

    /// Hors d'un bundle (un binaire posé n'importe où), il n'y a pas de
    /// `Contents` : renvoyer un chemin quand même ferait lire un
    /// `provisioning.json` étranger, posé à côté par hasard.
    #[test]
    fn app_bundle_resources_dir_is_none_outside_a_bundle() {
        assert_eq!(app_bundle_resources_dir(Path::new("/usr/local/bin/launcher")), None);
        assert_eq!(app_bundle_resources_dir(Path::new("/binary")), None);
    }

    /// Emplacement des premiers zips du panel, gardé pour ceux déjà
    /// téléchargés : trois niveaux plus haut, le dossier qui contient aussi
    /// `<bundle>.app`.
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
