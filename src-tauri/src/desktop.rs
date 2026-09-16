//! Intégration au bureau Linux (standard XDG).
//!
//! Sous Windows et macOS, l'identité du tenant est portée par le conteneur : le
//! bootstrap écrit un raccourci et une entrée de désinstallation, le bundle
//! `.app` porte son `Info.plist` et son `.icns`. Une AppImage, elle, n'est
//! qu'un fichier exécutable : le bureau ne sait rien d'elle tant que personne
//! ne le lui a dit. Sans ce module, le joueur n'a ni entrée dans son menu
//! d'applications, ni icône dans son dock — juste un fichier à double-cliquer.
//!
//! Ce qui est écrit, aux emplacements que la spécification XDG réserve à
//! l'utilisateur (donc sans `sudo`, et sans toucher à ce qui est installé
//! par la distribution) :
//!
//! ```text
//! ~/.local/share/applications/<app_id>.desktop
//! ~/.local/share/icons/hicolor/<taille>/apps/<app_id>.png
//! ```
//!
//! ### Pourquoi une entrée par tenant
//!
//! Un seul moteur est compilé pour tous les tenants, et deux AppImages de
//! serveurs différents peuvent cohabiter. `app_id` est l'identifiant de
//! l'application, que `lib::run` suffixe du slug du tenant avant de construire
//! l'application : sans ça, la seconde installation écraserait l'entrée de la
//! première, et le joueur se retrouverait avec un seul raccourci pour deux
//! serveurs.
//!
//! Le prix à payer est que le nom de l'entrée ne coïncide plus avec la classe
//! de fenêtre, que GNOME utilise pour relier une fenêtre ouverte à son icône.
//! C'est exactement ce à quoi sert `StartupWMClass`, qu'on renseigne donc.
//!
//! Rien ici ne remonte d'erreur : une intégration de bureau qui échoue est un
//! défaut cosmétique, jamais une raison d'empêcher le launcher de servir.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

// Le chemin réel de l'AppImage, et non `current_exe()` qui désigne le point de
// montage temporaire : c'est le même besoin que pour trouver le pack client,
// d'où la fonction partagée. Un point de montage disparaît à la fermeture et ne
// peut pas servir d'`Exec=`.
use crate::client_config::appimage_path;

/// Tailles des dossiers `hicolor` de la spécification d'icônes XDG.
const HICOLOR_SIZES: [u32; 13] = [16, 22, 24, 32, 36, 48, 64, 72, 96, 128, 192, 256, 512];

/// Empreinte de la dernière entrée écrite, pour ne pas réécrire à chaque
/// démarrage : le logo d'un client ne change qu'exceptionnellement, mais
/// l'AppImage, elle, peut avoir été déplacée entre deux lancements.
const STAMP_FILE: &str = "desktop-entry.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    app_id: String,
    name: String,
    exec: String,
    /// Taille du PNG écrit : suffit à repérer un logo changé côté panel.
    icon_bytes: usize,
}

fn data_home() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(explicit);
        if path.is_absolute() {
            return Some(path);
        }
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    if !home.is_absolute() {
        return None;
    }
    Some(home.join(".local/share"))
}

/// Identifiant de l'entrée : celui de l'application, tel que `lib::run` l'a
/// déjà scopé au tenant.
///
/// Reverse-DNS parce que c'est ce qu'attendent les environnements de bureau
/// modernes, et lu sur la configuration plutôt qu'écrit en dur pour qu'un fork
/// qui change le sien n'ait rien à changer ici.
fn app_id(app: &AppHandle) -> String {
    app.config().identifier.trim_matches('.').to_owned()
}

/// Classe de fenêtre, telle que le serveur d'affichage la voit.
///
/// C'est le nom du binaire, pas celui de l'AppImage : le joueur peut renommer
/// le fichier téléchargé, l'exécutable à l'intérieur ne bouge pas.
///
/// Meilleure approximation possible, et non une certitude : la classe réelle
/// dépend de ce que GTK tire d'`argv[0]` ou de l'identifiant d'application. Se
/// tromper ne coûte qu'une association fenêtre ↔ icône manquée dans le dock de
/// GNOME ; l'entrée et son icône, elles, restent correctes.
fn startup_wm_class() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.file_stem()?.to_string_lossy().into_owned())
}

/// Réduit un nom publié par le panel à ce qu'une valeur de fichier `.desktop`
/// accepte : une seule ligne, sans caractères de contrôle.
fn entry_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(128)
        .collect()
}

/// Échappe un chemin pour le champ `Exec`.
///
/// La spécification Desktop Entry fait passer cette valeur par deux niveaux :
/// la chaîne du fichier `.desktop`, puis un découpage façon interpréteur de
/// commandes. Les guillemets règlent le second (un chemin contenant une espace
/// reste un seul argument), et l'échappement des quatre caractères réservés le
/// premier.
fn quote_exec(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    format!("\"{escaped}\"")
}

/// Dimensions d'un PNG, lues dans son en-tête `IHDR`.
///
/// Le chunk suit immédiatement la signature, à un décalage fixe : pas besoin
/// de décoder l'image pour savoir dans quel dossier `hicolor` la ranger.
fn png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    const MAGIC: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    if png.len() < 24 || png[..8] != MAGIC {
        return None;
    }
    let width = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(png[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

/// Dossier `hicolor` le plus proche de la taille réelle de l'image.
///
/// Le dossier annonce la taille de ce qu'il contient : y ranger un logo de
/// 64 px sous `512x512` obligerait le bureau à l'agrandir, et il se verrait.
/// Au-delà de la plus grande taille du thème, tout finit dans `512x512`, que
/// les environnements savent réduire proprement.
fn hicolor_dir(width: u32, height: u32) -> String {
    let size = width.max(height);
    let nearest = HICOLOR_SIZES
        .iter()
        .copied()
        .min_by_key(|candidate| candidate.abs_diff(size))
        .unwrap_or(256);
    format!("{nearest}x{nearest}")
}

fn applications_dir(data_home: &Path) -> PathBuf {
    data_home.join("applications")
}

fn icon_path(data_home: &Path, dir: &str, app_id: &str) -> PathBuf {
    data_home
        .join("icons/hicolor")
        .join(dir)
        .join("apps")
        .join(format!("{app_id}.png"))
}

/// Contenu du fichier `.desktop`.
///
/// `Categories=Game;` plutôt que `Utility;` : c'est un launcher Minecraft, et
/// c'est cette catégorie qui décide dans quel sous-menu il apparaît.
fn entry_contents(name: &str, exec: &str, app_id: &str, icon: bool, wm_class: Option<&str>) -> String {
    let mut entry = String::from("[Desktop Entry]\nType=Application\nVersion=1.5\n");
    entry.push_str(&format!("Name={name}\n"));
    entry.push_str(&format!("Exec={exec}\n"));
    if icon {
        entry.push_str(&format!("Icon={app_id}\n"));
    }
    entry.push_str("Terminal=false\nCategories=Game;\n");
    if let Some(class) = wm_class {
        entry.push_str(&format!("StartupWMClass={class}\n"));
    }
    entry.push_str("StartupNotify=true\n");
    entry
}

/// Écrit un fichier en le remplaçant d'un bloc.
///
/// Un `.desktop` à moitié écrit serait lu tel quel par le bureau, qui le
/// garderait en cache jusqu'au prochain rafraîchissement.
fn write_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, contents)?;
    std::fs::rename(&temp, path)
}

/// Prévient le bureau qu'une entrée a changé.
///
/// `update-desktop-database` appartient à `desktop-file-utils`, qui n'est pas
/// installé partout : son absence n'est pas une erreur, les environnements qui
/// surveillent le dossier voient le fichier arriver de toute façon.
fn refresh_database(applications: &Path) {
    let outcome = std::process::Command::new("update-desktop-database")
        .arg(applications)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if let Err(error) = outcome {
        log::debug!("update-desktop-database unavailable: {error}");
    }
}

fn stamp_path(launcher_dir: &Path) -> PathBuf {
    launcher_dir.join(STAMP_FILE)
}

fn read_stamp(launcher_dir: &Path) -> Option<Stamp> {
    serde_json::from_slice(&std::fs::read(stamp_path(launcher_dir)).ok()?).ok()
}

/// Pose (ou met à jour) l'entrée de bureau du tenant.
///
/// Appelé à chaque démarrage : c'est l'empreinte qui évite le travail inutile,
/// pas l'appelant. Un `name` vide laisse tout en place — mieux vaut l'entrée du
/// démarrage précédent qu'une entrée sans nom.
pub fn refresh(app: &AppHandle, launcher_dir: &Path, name: &str, icon_png: Option<&[u8]>) {
    let name = entry_name(name);
    if name.is_empty() {
        return;
    }
    let Some(appimage) = appimage_path() else {
        return;
    };
    let Some(data_home) = data_home() else {
        log::debug!("no XDG data directory: skipping the desktop entry");
        return;
    };

    let app_id = app_id(app);
    let exec = quote_exec(&appimage);
    let stamp = Stamp {
        app_id: app_id.clone(),
        name: name.clone(),
        exec: exec.clone(),
        icon_bytes: icon_png.map(<[u8]>::len).unwrap_or(0),
    };
    if read_stamp(launcher_dir).as_ref() == Some(&stamp) {
        return;
    }

    // L'icône d'abord : le bureau lit `Icon=` dès qu'il voit le `.desktop`
    // arriver, et une entrée qui pointe une icône pas encore écrite reste
    // affichée sans icône jusqu'au rafraîchissement suivant.
    let icon = icon_png.and_then(|png| {
        let (width, height) = png_dimensions(png)?;
        let path = icon_path(&data_home, &hicolor_dir(width, height), &app_id);
        match write_atomically(&path, png) {
            Ok(()) => Some(path),
            Err(error) => {
                log::debug!("could not write {}: {error}", path.display());
                None
            }
        }
    });

    let applications = applications_dir(&data_home);
    let entry = applications.join(format!("{app_id}.desktop"));
    let contents = entry_contents(
        &name,
        &exec,
        &app_id,
        icon.is_some(),
        startup_wm_class().as_deref(),
    );
    if let Err(error) = write_atomically(&entry, contents.as_bytes()) {
        log::debug!("could not write {}: {error}", entry.display());
        return;
    }

    // Le fichier doit être exécutable pour que les bureaux qui vérifient ce bit
    // (GNOME, KDE) le considèrent comme approuvé plutôt que suspect.
    set_executable(&entry);
    refresh_database(&applications);
    let _ = write_atomically(
        &stamp_path(launcher_dir),
        &serde_json::to_vec(&stamp).unwrap_or_default(),
    );
    log::info!("desktop entry written: {}", entry.display());
}

fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    let mut permissions = metadata.permissions();
    permissions.set_mode(permissions.mode() | 0o100);
    let _ = std::fs::set_permissions(path, permissions);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hicolor_picks_the_nearest_standard_size() {
        assert_eq!(hicolor_dir(64, 64), "64x64");
        assert_eq!(hicolor_dir(100, 100), "96x96");
        assert_eq!(hicolor_dir(300, 300), "256x256");
        // Les très grandes icônes vont toutes dans le plus grand dossier du
        // thème : les bureaux savent réduire, pas agrandir proprement.
        assert_eq!(hicolor_dir(1024, 1024), "512x512");
    }

    /// Un logo rectangulaire est rangé d'après son plus grand côté, sinon il
    /// finirait dans un dossier trop petit pour lui.
    #[test]
    fn hicolor_uses_the_longest_side() {
        assert_eq!(hicolor_dir(128, 32), "128x128");
    }

    /// Un chemin avec une espace doit rester un seul argument, et les
    /// caractères que le découpage de la spécification réserve ne doivent pas
    /// être réinterprétés.
    #[test]
    fn exec_survives_a_hostile_path() {
        assert_eq!(
            quote_exec(Path::new("/home/joueur/Mes Téléchargements/Mon Serveur.AppImage")),
            "\"/home/joueur/Mes Téléchargements/Mon Serveur.AppImage\"",
        );
        assert_eq!(
            quote_exec(Path::new("/tmp/a\"b$c`d\\e.AppImage")),
            "\"/tmp/a\\\"b\\$c\\`d\\\\e.AppImage\"",
        );
    }

    /// Le nom vient du panel : il peut contenir n'importe quoi, et une fin de
    /// ligne y injecterait une clé arbitraire dans le fichier `.desktop`.
    #[test]
    fn a_name_cannot_inject_another_key() {
        assert_eq!(entry_name("Mon Serveur\nExec=/bin/sh"), "Mon ServeurExec=/bin/sh");
        assert_eq!(entry_name("  Mon Serveur  "), "Mon Serveur");
        assert_eq!(entry_name("\u{0}\u{7}"), "");
    }

    #[test]
    fn the_entry_carries_the_client_name_and_its_own_icon() {
        let entry = entry_contents("Mon Serveur", "\"/opt/a.AppImage\"", "fr.test.app.abc", true, Some("launcher"));
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("\nName=Mon Serveur\n"));
        assert!(entry.contains("\nExec=\"/opt/a.AppImage\"\n"));
        assert!(entry.contains("\nIcon=fr.test.app.abc\n"));
        assert!(entry.contains("\nStartupWMClass=launcher\n"));
        assert!(entry.contains("\nCategories=Game;\n"));
    }

    /// Sans logo exploitable, l'entrée ne doit pas annoncer une icône qui
    /// n'existe pas : le bureau afficherait un emplacement vide plutôt que
    /// l'icône générique.
    #[test]
    fn no_icon_key_without_an_icon() {
        let entry = entry_contents("Mon Serveur", "\"/opt/a.AppImage\"", "fr.test.app", false, None);
        assert!(!entry.contains("Icon="));
        assert!(!entry.contains("StartupWMClass="));
    }

    #[test]
    fn png_dimensions_are_read_from_the_header() {
        let mut png = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend_from_slice(&[0, 0, 0, 13]); // longueur du chunk
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&512u32.to_be_bytes());
        png.extend_from_slice(&256u32.to_be_bytes());
        assert_eq!(png_dimensions(&png), Some((512, 256)));
    }

    #[test]
    fn anything_that_is_not_a_png_is_refused() {
        assert_eq!(png_dimensions(b"GIF89a"), None);
        assert_eq!(png_dimensions(&[]), None);
    }
}
