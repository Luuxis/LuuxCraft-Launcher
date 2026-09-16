//! Nom et logo de la fenêtre, lus dans le pack client local.
//!
//! Un seul moteur est compilé pour tous les tenants : son titre et son icône ne
//! peuvent donc pas venir du build. Ils ne viennent pas non plus du réseau —
//! attendre `/config` pour savoir quoi écrire dans la barre de titre ferait
//! s'ouvrir la fenêtre sous le nom générique du moteur, et un lancement hors
//! ligne resterait anonyme pour toujours. Ils viennent de `client_config.json`
//! et de l'icône attendus à côté du binaire — voir `client_config`, qui dit
//! aussi pourquoi plus rien ne les y écrit aujourd'hui.
//!
//! L'identité est donc connue **avant** la première image : la fenêtre est
//! créée invisible, habillée ici, puis affichée (voir `lib::run`).
//!
//! Ce qui se joue ailleurs :
//!
//! - **macOS** : le nom et l'icône du Dock viennent d'`Info.plist` et de
//!   `Resources/*.icns`, écrits dans le bundle. `set_icon` n'a de toute façon
//!   pas d'effet sur macOS, où une fenêtre ne porte pas d'icône.
//! - **Windows** : le raccourci du menu Démarrer et l'entrée de désinstallation
//!   appartiennent à l'installation, pas au moteur — et plus rien ne les pose
//!   depuis le retrait du bootstrap.
//! - **Linux** : le moteur est une AppImage, c'est-à-dire un simple fichier
//!   exécutable dont le bureau ne sait rien. C'est donc ici, au démarrage, que
//!   l'entrée de bureau et l'icône du thème sont posées — voir `desktop`.

use std::path::Path;

use tauri::{AppHandle, Manager, WebviewWindow};

use crate::client_config::ClientConfig;

/// Au-delà, ce n'est pas un logo : on ne charge pas le fichier en mémoire.
const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;

/// Habille la fenêtre à l'identité du tenant.
///
/// Aucune erreur n'est remontée : une identité qui ne s'applique pas est un
/// défaut cosmétique, jamais une raison d'empêcher le moteur de servir.
pub fn apply(window: &WebviewWindow, client: &ClientConfig, launcher_dir: &Path) {
    if let Err(error) = window.set_title(&client.window.title) {
        log::debug!("could not apply the window title: {error}");
    }

    let icon = read_icon(client);
    if let Some(bytes) = icon.as_deref() {
        set_icon(window, bytes);
    }

    refresh_desktop_entry(
        window.app_handle(),
        launcher_dir,
        &client.window.title,
        icon.as_deref(),
    );
}

/// Le logo du pack, s'il est présent et d'une taille plausible.
fn read_icon(client: &ClientConfig) -> Option<Vec<u8>> {
    let path = client.icon_path();
    let length = std::fs::metadata(&path).ok()?.len();
    if length == 0 || length > MAX_ICON_BYTES {
        log::debug!("{} is {length} bytes, ignored", path.display());
        return None;
    }
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            log::debug!("could not read {}: {error}", path.display());
            None
        }
    }
}

fn set_icon(window: &WebviewWindow, bytes: &[u8]) {
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

/// Linux : pose ou met à jour l'entrée de bureau du tenant (voir `desktop`).
#[cfg(target_os = "linux")]
fn refresh_desktop_entry(
    app: &AppHandle,
    launcher_dir: &Path,
    name: &str,
    icon: Option<&[u8]>,
) {
    crate::desktop::refresh(app, launcher_dir, name, icon);
}

/// Ailleurs, l'identité est portée par le conteneur : le raccourci de
/// l'installation sous Windows, le bundle `.app` sous macOS.
#[cfg(not(target_os = "linux"))]
fn refresh_desktop_entry(
    _app: &AppHandle,
    _launcher_dir: &Path,
    _name: &str,
    _icon: Option<&[u8]>,
) {
}
