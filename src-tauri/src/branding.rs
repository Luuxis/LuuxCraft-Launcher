//! Nom et logo du client, appliqués à chaud.
//!
//! Un seul launcher est compilé pour tous les clients : son titre de fenêtre et
//! son icône ne peuvent donc pas venir du build. Ils viennent du bloc `brand`
//! que le panel publie dans `/config`, et sont posés sur la fenêtre dès que la
//! configuration arrive.
//!
//! Le logo est gardé en cache dans le dossier de données : au démarrage
//! suivant, la fenêtre est déjà à la bonne identité avant la première requête,
//! et un lancement hors ligne reste correct.
//!
//! Ce qui se joue ailleurs :
//!
//! - **macOS** : le nom et l'icône du Dock viennent d'`Info.plist` et de
//!   `Resources/*.icns`, que le panel a déjà personnalisés dans le zip
//!   téléchargé. `set_icon` n'a de toute façon pas d'effet sur macOS, où une
//!   fenêtre ne porte pas d'icône.
//! - **Windows** : le raccourci du menu Démarrer et l'entrée de désinstallation
//!   sont réglés par le hook NSIS à l'installation. On complète ici ce qu'il ne
//!   pouvait pas faire — réécrire `brand.ico` s'il n'a pas pu le télécharger, et
//!   renommer le raccourci du Bureau, que tauri crée *après* le hook quand le
//!   joueur coche la case de la page finale.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::api::models::RemoteBrand;

/// Copie du logo dans le dossier de données, pour les démarrages suivants.
const CACHED_ICON: &str = "brand-icon.png";
/// Même nom que celui écrit par `installer-hooks.nsh`.
#[cfg(target_os = "windows")]
const WINDOWS_ICON: &str = "brand.ico";

/// Au-delà, ce n'est pas un logo : on ne charge pas le fichier en mémoire.
const MAX_ICON_BYTES: usize = 8 * 1024 * 1024;

/// Applique ce qui est déjà sur le disque : le titre et le logo mis en cache.
///
/// Appelé au démarrage, avant toute requête, pour que la fenêtre ne s'affiche
/// jamais sous le nom du launcher compilé le temps d'un aller-retour réseau.
pub fn apply_cached(app: &AppHandle, brand: Option<&RemoteBrand>, launcher_dir: &Path) {
    let name = brand.and_then(|brand| brand.name.as_deref());
    set_title(app, name);

    let cached = launcher_dir.join(CACHED_ICON);
    let Ok(bytes) = std::fs::read(&cached) else {
        return;
    };
    set_icon(app, &bytes);
}

/// Rafraîchit l'identité depuis le panel : titre, logo téléchargé et mis en
/// cache, et sur Windows les fichiers que l'installeur n'a pas pu écrire.
///
/// Aucune erreur n'est remontée : une identité qui ne s'applique pas est un
/// défaut cosmétique, jamais une raison d'empêcher le launcher de servir.
pub async fn refresh(
    app: AppHandle,
    http: reqwest::Client,
    launcher_dir: PathBuf,
    brand: Option<RemoteBrand>,
) {
    let brand = brand.unwrap_or_default();
    set_title(&app, brand.name.as_deref());

    let Some(url) = brand.icon_url.as_deref() else {
        return;
    };

    let bytes = match download(&http, url).await {
        Some(bytes) => {
            let cached = launcher_dir.join(CACHED_ICON);
            if let Err(error) = std::fs::write(&cached, &bytes) {
                log::debug!("could not cache the brand icon: {error}");
            }
            bytes
        }
        // Panel injoignable : le cache du démarrage précédent fait l'affaire.
        None => match std::fs::read(launcher_dir.join(CACHED_ICON)) {
            Ok(bytes) => bytes,
            Err(_) => return,
        },
    };

    set_icon(&app, &bytes);

    #[cfg(target_os = "windows")]
    refresh_windows_assets(&app, brand.name.as_deref(), &bytes);
}

async fn download(http: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    let response = http
        .get(url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        log::debug!("brand icon {url} answered {}", response.status());
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() > MAX_ICON_BYTES || bytes.is_empty() {
        log::debug!("brand icon {url} is {} bytes, ignored", bytes.len());
        return None;
    }
    Some(bytes.to_vec())
}

fn set_title(app: &AppHandle, name: Option<&str>) {
    let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) else {
        return;
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(name);
    }
}

fn set_icon(app: &AppHandle, bytes: &[u8]) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    match tauri::image::Image::from_bytes(bytes) {
        // macOS renvoie une erreur ou ne fait rien : une fenêtre n'y porte pas
        // d'icône, c'est le bundle qui la donne.
        Ok(image) => {
            if let Err(error) = window.set_icon(image) {
                log::debug!("could not apply the brand icon: {error}");
            }
        }
        Err(error) => log::debug!("brand icon is not a readable image: {error}"),
    }
}

/// Windows : le `.ico` que l'installeur a pu manquer, et le raccourci du Bureau
/// qu'il ne pouvait pas encore renommer.
#[cfg(target_os = "windows")]
fn refresh_windows_assets(app: &AppHandle, name: Option<&str>, png: &[u8]) {
    if let Some(ico) = ico_from_png(png) {
        if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)) {
            let path = dir.join(WINDOWS_ICON);
            // Réécrire à chaque démarrage n'apporterait rien : la taille suffit
            // à repérer un logo qui a changé côté panel.
            let stale = std::fs::metadata(&path).map(|meta| meta.len() as usize != ico.len()).unwrap_or(true);
            if stale {
                if let Err(error) = std::fs::write(&path, &ico) {
                    log::debug!("could not write {WINDOWS_ICON}: {error}");
                }
            }
        }
    }

    if let Some(name) = name.map(safe_file_stem) {
        rename_desktop_shortcut(app, &name);
    }
}

/// Renomme `<produit compilé>.lnk` en `<nom du client>.lnk` sur le Bureau.
///
/// Le hook NSIS ne peut pas le faire : tauri crée ce raccourci depuis la page
/// finale de l'installeur, après le hook, quand le joueur coche la case. Seul
/// le nom change — l'icône d'un `.lnk` demanderait de le réécrire via COM,
/// alors qu'elle est déjà correcte dès que `brand.ico` existe pour le raccourci
/// du menu Démarrer.
#[cfg(target_os = "windows")]
fn rename_desktop_shortcut(app: &AppHandle, name: &str) {
    let product = safe_file_stem(&app.package_info().name);
    if name.is_empty() || name == product {
        return;
    }
    let Ok(desktop) = app.path().desktop_dir() else {
        return;
    };

    let from = desktop.join(format!("{product}.lnk"));
    let to = desktop.join(format!("{name}.lnk"));
    if !from.exists() || to.exists() {
        return;
    }
    match std::fs::rename(&from, &to) {
        Ok(()) => log::info!("desktop shortcut renamed to {name}.lnk"),
        Err(error) => log::debug!("could not rename the desktop shortcut: {error}"),
    }
}

/// Réduit un nom à ce qu'un nom de fichier Windows accepte.
#[cfg(target_os = "windows")]
fn safe_file_stem(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
        .trim()
        .to_owned()
}

/// `.ico` à une seule entrée contenant le PNG tel quel.
///
/// Même conteneur que celui construit par le panel (`src/lib/appIcons.ts`) :
/// depuis Vista, une entrée d'icône peut être un PNG, ce qui évite d'avoir à
/// décoder puis ré-encoder l'image. Le champ de dimension fait un octet, où 0
/// signifie 256 — la convention pour toute icône de 256 px ou plus.
#[cfg(target_os = "windows")]
fn ico_from_png(png: &[u8]) -> Option<Vec<u8>> {
    const MAGIC: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    if png.len() < 24 || png[..8] != MAGIC {
        return None;
    }
    // Dimensions du chunk IHDR, qui suit immédiatement la signature.
    let width = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(png[20..24].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }

    let mut out = Vec::with_capacity(22 + png.len());
    out.extend_from_slice(&0u16.to_le_bytes()); // réservé
    out.extend_from_slice(&1u16.to_le_bytes()); // type : icône
    out.extend_from_slice(&1u16.to_le_bytes()); // une image
    out.push(if width >= 256 { 0 } else { width as u8 });
    out.push(if height >= 256 { 0 } else { height as u8 });
    out.push(0); // palette
    out.push(0); // réservé
    out.extend_from_slice(&1u16.to_le_bytes()); // plans
    out.extend_from_slice(&32u16.to_le_bytes()); // bits par pixel
    out.extend_from_slice(&(png.len() as u32).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes()); // offset des données
    out.extend_from_slice(png);
    Some(out)
}
