//! Mise à jour du moteur, une fois installé.
//!
//! Le bootstrap Windows et le script Linux/macOS ne servent qu'à la **première**
//! installation : les raccourcis pointent sur le moteur, pas sur eux, et plus
//! rien ne les rappelle ensuite. C'est donc le moteur qui se met à jour, et
//! c'est ce module qui s'en charge.
//!
//! ## Pourquoi pas `tauri-plugin-updater`
//!
//! Le greffon officiel a été essayé, puis écarté, pour deux raisons qui ne se
//! contournent pas :
//!
//! 1. **Il ne sait pas mettre à jour un exécutable portable sous Windows.** Son
//!    installateur attend un NSIS ou un MSI, y compris à l'intérieur d'un zip.
//!    Or un installeur d'OS écrit dans `Program Files` — un emplacement
//!    partagé — alors que chaque serveur doit vivre dans son propre dossier.
//!    Adopter NSIS reviendrait à abandonner l'isolation par serveur, qui est la
//!    raison d'être de toute cette distribution.
//! 2. **Il impose une paire de clés minisign** et un second manifeste à son
//!    format. Cela vaudrait la peine pour les trois plateformes ; pour deux, ça
//!    ferait deux mécanismes de mise à jour à maintenir, deux formats de
//!    manifeste, et une clé privée de plus à protéger — alors que Windows
//!    resterait de toute façon à la charge de ce module.
//!
//! Ce qui est utilisé à la place existe déjà et sert aussi à l'installation :
//! le **manifeste du serveur**, servi en https, qui annonce la version du
//! moteur, son URL et son `sha256`. Rien n'est écrit sur le disque avant que
//! l'empreinte téléchargée corresponde à celle annoncée.
//!
//! Le jour où le moteur Windows serait distribué autrement, le greffon
//! officiel redevient le bon choix pour les trois plateformes : il suffira de
//! signer les artefacts et de servir un `latest.json`.
//!
//! ## Ce qui est remplacé, et ce qui ne l'est jamais
//!
//! | Système | Remplacé | Laissé intact |
//! |---|---|---|
//! | Windows | `engine\<Nom>.exe` | `client\`, `%APPDATA%\.<slug>` |
//! | Linux | l'AppImage installée | `client/`, `~/.<slug>` |
//! | macOS | le bundle `.app` | `~/Library/Application Support/luuxcraft/installs/<clé>`, `~/.<slug>` |
//!
//! L'identité du serveur est **toujours** hors de ce qui est remplacé : c'est
//! la garantie qu'une mise à jour ne peut pas faire perdre au launcher le
//! serveur auquel il appartient.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::client_config::installed_exe;
use crate::state::AppState;

/// Émis quand une nouvelle version est posée sur le disque. Elle prend effet au
/// prochain démarrage ; l'interface peut le dire, ou l'ignorer.
pub const UPDATE_INSTALLED_EVENT: &str = "updater://installed";

/// Délai avant la vérification.
///
/// Le démarrage est déjà chargé (pack, réglages, comptes, snapshot du panel) et
/// rien ne presse : une mise à jour ne prend effet qu'au lancement suivant.
const START_DELAY: Duration = Duration::from_secs(20);

/// Un moteur vaut quelques dizaines de méga-octets ; au-delà, c'est autre chose.
const MAX_ENGINE_BYTES: u64 = 512 * 1024 * 1024;

const HTTP_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Deserialize)]
struct Manifest {
    schema: u32,
    engine: Engine,
}

#[derive(Debug, Deserialize)]
struct Engine {
    version: String,
    url: String,
    sha256: String,
    format: String,
}

/// Lance la vérification en tâche de fond. Rend la main tout de suite.
///
/// Tout échec est journalisé et abandonné : une mise à jour ratée doit laisser
/// le launcher exactement tel qu'il était, jamais l'empêcher de démarrer.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(START_DELAY).await;
        match check_and_install(&app).await {
            Ok(Some(version)) => {
                log::info!("engine {version} installed, it will be used on the next start");
                let _ = app.emit(UPDATE_INSTALLED_EVENT, version);
            }
            Ok(None) => log::debug!("engine is up to date"),
            Err(reason) => log::warn!("engine update skipped: {reason}"),
        }
    });
}

async fn check_and_install(app: &AppHandle) -> Result<Option<String>, String> {
    // En debug il n'y a pas d'installation à remplacer : le binaire est celui
    // que cargo vient de produire.
    if cfg!(debug_assertions) {
        return Ok(None);
    }

    let (api_base, tenant_id) = {
        let state = app.state::<AppState>();
        (
            state.config.api.base_url.clone(),
            state.config.user_id.clone(),
        )
    };

    let manifest = fetch_manifest(&api_base, &tenant_id).await?;
    if manifest.schema != 1 {
        return Err(format!("unsupported manifest schema {}", manifest.schema));
    }

    let running = app.package_info().version.to_string();
    // Égalité et non comparaison : le panel ne publie qu'une version courante
    // par canal, et un retour arrière se fait en dépubliant. « Différent de ce
    // qui tourne » est donc exactement « ce qu'il faut installer ».
    if manifest.engine.version == running {
        return Ok(None);
    }

    // L'URL doit rester sur le panel du pack client : le `sha256` vient de la
    // même réponse, donc il ne protégerait de rien si l'URL pouvait pointer
    // ailleurs.
    let origin = origin_of(&api_base)?;
    if !same_origin(&manifest.engine.url, &origin) {
        return Err(format!("engine url outside {origin}"));
    }
    if manifest.engine.sha256.len() != 64
        || !manifest.engine.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("engine sha256 is not a digest".to_owned());
    }

    // Une mise à jour pendant que Minecraft tourne remplacerait les fichiers
    // sous les pieds du joueur : elle attendra le prochain démarrage.
    if app.state::<AppState>().game.is_busy() {
        return Err("the game is running".to_owned());
    }

    let target = installed_exe().ok_or("cannot locate the installed engine")?;
    let staging = staging_dir(&target)?;
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;

    let outcome = install(&manifest.engine, &target, &staging).await;
    let _ = std::fs::remove_dir_all(&staging);
    outcome?;

    Ok(Some(manifest.engine.version))
}

async fn install(engine: &Engine, target: &Path, staging: &Path) -> Result<(), String> {
    log::info!("downloading engine {} ({})", engine.version, engine.format);
    let bundle = staging.join("bundle");
    download_verified(&engine.url, &bundle, &engine.sha256).await?;

    let replacement = unpack(&bundle, staging, &engine.format)?;
    swap(&replacement, target, staging)
}

async fn fetch_manifest(api_base: &str, tenant_id: &str) -> Result<Manifest, String> {
    // `api_base` finit par `/api` : la route de distribution est `/api/v1/...`.
    let url = format!(
        "{}/v1/launchers/{}/manifest?os={}&arch={}",
        api_base.trim_end_matches('/'),
        tenant_id,
        panel_os(),
        std::env::consts::ARCH,
    );
    log::debug!("update check {url}");

    let response = client()?
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("manifest answered {}", response.status().as_u16()));
    }
    response
        .json::<Manifest>()
        .await
        .map_err(|error| error.to_string())
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|error| error.to_string())
}

/// Télécharge vers un fichier temporaire et ne le rend que si son empreinte
/// correspond. Rien n'est jamais mis en place avant cette vérification.
async fn download_verified(url: &str, destination: &Path, expected: &str) -> Result<(), String> {
    let response = client()?
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download answered {}", response.status().as_u16()));
    }
    if let Some(length) = response.content_length() {
        if length > MAX_ENGINE_BYTES {
            return Err(format!("engine announces {length} bytes"));
        }
    }

    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_ENGINE_BYTES {
        return Err(format!("engine is {} bytes", bytes.len()));
    }

    let digest = Sha256::digest(&bytes);
    let got: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    if !got.eq_ignore_ascii_case(expected) {
        return Err("downloaded engine does not match its digest".to_owned());
    }

    std::fs::write(destination, &bytes).map_err(|error| error.to_string())
}

/// Où préparer la nouvelle version.
///
/// Toujours sur le même système de fichiers que ce qui sera remplacé : c'est ce
/// qui rend le `rename` final atomique, donc l'échange ininterruptible.
#[cfg(target_os = "macos")]
fn staging_dir(target: &Path) -> Result<PathBuf, String> {
    let bundle = bundle_of(target)?;
    let parent = bundle
        .parent()
        .ok_or("the application bundle has no parent directory")?;
    Ok(parent.join(".luuxcraft-update"))
}

#[cfg(not(target_os = "macos"))]
fn staging_dir(target: &Path) -> Result<PathBuf, String> {
    // `<racine>/engine/<binaire>` : la racine de l'installation est deux crans
    // au-dessus, et c'est là que le bootstrap et le script rangent déjà `.tmp`.
    let root = target
        .parent()
        .and_then(Path::parent)
        .ok_or("the engine is not inside an installation directory")?;
    Ok(root.join(".tmp"))
}

/// Le `.app` qui contient l'exécutable courant.
#[cfg(target_os = "macos")]
fn bundle_of(target: &Path) -> Result<PathBuf, String> {
    let bundle = target
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or("the engine is not inside an application bundle")?;
    if bundle
        .extension()
        .map(|extension| extension.eq_ignore_ascii_case("app"))
        .unwrap_or(false)
    {
        Ok(bundle.to_path_buf())
    } else {
        Err("the engine is not inside an application bundle".to_owned())
    }
}

/// Prépare la nouvelle version et rend ce qu'il faut mettre en place — un
/// fichier sous Windows et Linux, un dossier `.app` sous macOS.
#[cfg(target_os = "windows")]
fn unpack(bundle: &Path, staging: &Path, format: &str) -> Result<PathBuf, String> {
    if normalize(format) != "exezip" {
        return Err(format!("unsupported engine format {format}"));
    }

    let file = std::fs::File::open(bundle).map_err(|error| error.to_string())?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|error| error.to_string())?;
    let extracted = staging.join("engine");
    std::fs::create_dir_all(&extracted).map_err(|error| error.to_string())?;

    let mut found: Option<PathBuf> = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().to_owned();
        // Zip Slip : seul un nom de fichier simple est accepté, et l'archive du
        // moteur Windows n'en contient de toute façon qu'un.
        if name.ends_with('/') || name.contains('/') || name.contains('\\') || name.contains("..") {
            continue;
        }
        if !name.to_ascii_lowercase().ends_with(".exe") {
            continue;
        }
        let out = extracted.join(&name);
        let mut writer = std::fs::File::create(&out).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut writer).map_err(|error| error.to_string())?;
        found = Some(out);
    }

    found.ok_or_else(|| "the engine archive contains no executable".to_owned())
}

#[cfg(target_os = "linux")]
fn unpack(bundle: &Path, staging: &Path, format: &str) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    if normalize(format) != "appimage" {
        return Err(format!("unsupported engine format {format}"));
    }
    // Une AppImage est déjà un exécutable d'un seul fichier : il n'y a rien à
    // décompresser, seulement à rendre exécutable.
    let out = staging.join("engine.AppImage");
    std::fs::rename(bundle, &out).map_err(|error| error.to_string())?;
    std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| error.to_string())?;
    Ok(out)
}

#[cfg(target_os = "macos")]
fn unpack(bundle: &Path, staging: &Path, format: &str) -> Result<PathBuf, String> {
    if normalize(format) != "apptargz" {
        return Err(format!("unsupported engine format {format}"));
    }

    let extracted = staging.join("bundle-out");
    std::fs::create_dir_all(&extracted).map_err(|error| error.to_string())?;
    let file = std::fs::File::open(bundle).map_err(|error| error.to_string())?;
    let decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
    let mut archive = tar::Archive::new(decoder);
    // Les droits Unix sont conservés : sans le bit d'exécution, le Mach-O du
    // moteur ne serait plus lançable.
    archive.set_preserve_permissions(true);
    archive.set_overwrite(true);
    archive
        .unpack(&extracted)
        .map_err(|error| error.to_string())?;

    std::fs::read_dir(&extracted)
        .map_err(|error| error.to_string())?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.is_dir()
                && path
                    .extension()
                    .map(|extension| extension.eq_ignore_ascii_case("app"))
                    .unwrap_or(false)
        })
        .ok_or_else(|| "the engine archive contains no application bundle".to_owned())
}

/// Met la nouvelle version en place.
///
/// L'ancienne est d'abord **écartée** plutôt que supprimée : les trois systèmes
/// acceptent de renommer un exécutable en cours d'exécution, mais aucun
/// n'accepte de l'effacer. Le processus en cours continue donc de tourner
/// depuis le fichier écarté, et la nouvelle version prend effet au prochain
/// démarrage.
///
/// Si le second renommage échoue, l'ancien est remis : le joueur garde un
/// launcher qui démarre.
#[cfg(target_os = "macos")]
fn swap(replacement: &Path, target: &Path, staging: &Path) -> Result<(), String> {
    let bundle = bundle_of(target)?;
    swap_path(replacement, &bundle, &staging.join("previous"))
}

#[cfg(not(target_os = "macos"))]
fn swap(replacement: &Path, target: &Path, staging: &Path) -> Result<(), String> {
    swap_path(replacement, target, &staging.join("previous"))
}

fn swap_path(replacement: &Path, target: &Path, aside: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(aside);
    let _ = std::fs::remove_file(aside);

    let had_previous = target.exists();
    if had_previous {
        std::fs::rename(target, aside).map_err(|error| format!("cannot move the old engine aside: {error}"))?;
    }

    match std::fs::rename(replacement, target) {
        Ok(()) => {
            // Best-effort : le fichier écarté est peut-être encore ouvert par ce
            // processus, auquel cas il partira au prochain nettoyage du staging.
            let _ = std::fs::remove_dir_all(aside);
            let _ = std::fs::remove_file(aside);
            Ok(())
        }
        Err(error) => {
            if had_previous {
                let _ = std::fs::rename(aside, target);
            }
            Err(format!("cannot install the new engine: {error}"))
        }
    }
}

fn normalize(format: &str) -> String {
    format
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Vocabulaire d'OS du panel, qui n'est pas exactement celui de Rust.
fn panel_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// `https://hôte[:port]` extrait d'une URL, sans le chemin.
fn origin_of(url: &str) -> Result<String, String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("not an absolute url: {url}"))?;
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() {
        return Err(format!("not an absolute url: {url}"));
    }
    Ok(format!("{scheme}://{host}"))
}

/// Vraie quand l'URL est sur cette origine, et pas seulement préfixée par elle.
///
/// Le contrôle du `/` qui suit est ce qui distingue `https://panel.fr/…` de
/// `https://panel.fr.evil.example/…`.
fn same_origin(url: &str, origin: &str) -> bool {
    url.len() > origin.len() && url.starts_with(origin) && url[origin.len()..].starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_are_matched_whatever_the_separator() {
        assert_eq!(normalize("exe-zip"), "exezip");
        assert_eq!(normalize("app-tar-gz"), "apptargz");
        assert_eq!(normalize("AppImage"), "appimage");
    }

    #[test]
    fn the_origin_is_read_without_its_path() {
        assert_eq!(origin_of("https://panel.fr/api").unwrap(), "https://panel.fr");
        assert_eq!(
            origin_of("https://panel.fr:8443/api/v1").unwrap(),
            "https://panel.fr:8443"
        );
        assert!(origin_of("panel.fr/api").is_err());
        assert!(origin_of("https:///api").is_err());
    }

    /// Le `sha256` vient de la même réponse que l'URL : si l'URL peut pointer
    /// ailleurs, l'empreinte ne protège de rien.
    #[test]
    fn a_lookalike_host_is_not_the_panel() {
        let origin = "https://panel.fr";
        assert!(same_origin("https://panel.fr/api/v1/engine/1.0.0", origin));
        assert!(!same_origin("https://panel.fr.evil.example/x", origin));
        assert!(!same_origin("https://evil.example/x", origin));
        assert!(!same_origin("https://panel.fr", origin));
    }

    #[test]
    fn the_panel_os_vocabulary_is_one_of_three() {
        assert!(["windows", "macos", "linux"].contains(&panel_os()));
    }
}
