//! Fabrication du `.app` macOS sur la machine du joueur (contrat §0.2, §6).
//!
//! Le serveur ne repackage rien : il publie un moteur générique, et c'est ici
//! que le bundle prend l'identité du tenant. C'est ce qui permet à deux clients
//! de coexister — deux `.app` de noms différents, deux `CFBundleIdentifier`
//! différents, donc deux entrées distinctes dans le Dock, dans Launchpad et
//! dans l'état des fenêtres.
//!
//! La racine d'installation **est** le bundle : `Contents/` côtoie `engine/` et
//! `client/`. Le Finder ne demande qu'un `Contents/Info.plist` et l'exécutable
//! qu'il désigne ; ce qui traîne à côté ne le gêne pas.
//!
//! `Contents/MacOS/<moteur>` est un lien physique vers `engine/<moteur>` : le
//! Mach-O n'existe qu'une fois sur le disque, et les deux chemins restent
//! valides. Le lien est refait à chaque installation, puisqu'un remplacement du
//! moteur crée un nouvel inode.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config;
use crate::error::{BootstrapError, Result};
use crate::manifest::Manifest;
use crate::pack::Pack;
use crate::paths::{self, Layout};

const ICON_FILE: &str = "icon.icns";

/// Écrit le bundle et rend le chemin du `.app` à lancer.
pub fn generate(
    layout: &Layout,
    manifest: &Manifest,
    engine_executable: &Path,
    pack: &Pack,
) -> Result<PathBuf> {
    let contents = layout.root.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    paths::create_dir(&macos)?;
    paths::create_dir(&resources)?;

    let name = engine_executable
        .file_name()
        .ok_or_else(|| BootstrapError::new("le moteur installé n'a pas de nom de fichier."))?
        .to_string_lossy()
        .into_owned();

    // Un moteur renommé entre deux versions laisserait sinon un exécutable
    // mort dans le bundle, que le Finder pourrait proposer de lancer.
    clear_directory(&macos);
    let executable = macos.join(&name);
    link_or_copy(engine_executable, &executable)?;

    let icon = pack.asset(ICON_FILE);
    if let Some(icon) = &icon {
        let target = resources.join(ICON_FILE);
        let _ = fs::remove_file(&target);
        fs::copy(icon, &target)
            .map_err(|error| BootstrapError::io("La copie de l'icône", &target, &error))?;
    }

    link_pack(&layout.client_dir, &resources.join("client"));

    let plist = contents.join("Info.plist");
    fs::write(
        &plist,
        info_plist(manifest, &name, icon.is_some()),
    )
    .map_err(|error| BootstrapError::io("L'écriture", &plist, &error))?;

    let pkginfo = contents.join("PkgInfo");
    let _ = fs::write(pkginfo, "APPL????");

    Ok(layout.root.clone())
}

/// Lien physique d'abord : le Mach-O pèse plusieurs dizaines de méga-octets, le
/// dupliquer n'apporterait rien. La copie ne sert que si le lien est refusé.
fn link_or_copy(source: &Path, target: &Path) -> Result<()> {
    let _ = fs::remove_file(target);
    if fs::hard_link(source, target).is_ok() {
        return Ok(());
    }
    fs::copy(source, target)
        .map_err(|error| BootstrapError::io("La mise en place du moteur", target, &error))?;
    crate::unpack::make_executable(target)
}

/// Rend le pack client visible depuis l'intérieur du bundle.
///
/// Le contrat pose `client/` à la racine d'installation, qui sous macOS **est**
/// le `.app` ; le moteur, lui, cherche ses ressources là où un bundle en pose,
/// c'est-à-dire `Contents/Resources/client`. Un lien symbolique relatif
/// réconcilie les deux sans dupliquer le pack, et survit au déplacement du
/// bundle. À défaut, on recopie les fichiers : un moteur sans pack client
/// s'arrête au démarrage.
fn link_pack(client_dir: &Path, target: &Path) {
    if target.is_symlink() {
        let _ = fs::remove_file(target);
    } else if target.is_dir() {
        let _ = fs::remove_dir_all(target);
    }

    if std::os::unix::fs::symlink(Path::new("../../client"), target).is_ok() {
        return;
    }

    let _ = fs::create_dir_all(target);
    let Ok(entries) = fs::read_dir(client_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().is_file() {
            let _ = fs::copy(entry.path(), target.join(entry.file_name()));
        }
    }
}

fn clear_directory(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        } else {
            let _ = fs::remove_file(&path);
        }
    }
}

fn info_plist(manifest: &Manifest, executable: &str, with_icon: bool) -> String {
    let display = escape(manifest.display_name());
    let identifier = escape(&bundle_identifier(&manifest.slug));
    let version = escape(&plist_version(&manifest.engine.version));
    let icon = if with_icon {
        format!("\t<key>CFBundleIconFile</key>\n\t<string>{ICON_FILE}</string>\n")
    } else {
        String::new()
    };

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>CFBundleInfoDictionaryVersion</key>\n\t<string>6.0</string>\n\
         \t<key>CFBundlePackageType</key>\n\t<string>APPL</string>\n\
         \t<key>CFBundleName</key>\n\t<string>{display}</string>\n\
         \t<key>CFBundleDisplayName</key>\n\t<string>{display}</string>\n\
         \t<key>CFBundleIdentifier</key>\n\t<string>{identifier}</string>\n\
         \t<key>CFBundleExecutable</key>\n\t<string>{executable}</string>\n\
         {icon}\
         \t<key>CFBundleShortVersionString</key>\n\t<string>{version}</string>\n\
         \t<key>CFBundleVersion</key>\n\t<string>{version}</string>\n\
         \t<key>LSMinimumSystemVersion</key>\n\t<string>10.15</string>\n\
         \t<key>NSHighResolutionCapable</key>\n\t<true/>\n\
         </dict>\n\
         </plist>\n",
        executable = escape(executable)
    )
}

/// `com.luuxcraft.launcher.<slug>` : le slug est déjà en `[a-z0-9-]`, ce que
/// les identifiants de bundle acceptent tels quels.
fn bundle_identifier(slug: &str) -> String {
    format!("{}.{}", config::bundle_id_prefix(), slug)
}

/// `CFBundleVersion` n'accepte que des chiffres et des points ; une version
/// avec suffixe (`1.2.3-beta`) ferait refuser le bundle par le Finder.
fn plist_version(version: &str) -> String {
    let cleaned: String = version
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let cleaned = cleaned.trim_matches('.').to_owned();
    if cleaned.is_empty() {
        "1.0".to_owned()
    } else {
        cleaned
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_reduced_to_digits_and_dots() {
        assert_eq!(plist_version("1.2.3"), "1.2.3");
        assert_eq!(plist_version("1.2.3-beta.1"), "1.2.3");
        assert_eq!(plist_version("beta"), "1.0");
        assert_eq!(plist_version(""), "1.0");
    }

    #[test]
    fn xml_special_characters_are_escaped() {
        assert_eq!(escape("Tom & <Jerry>"), "Tom &amp; &lt;Jerry&gt;");
    }
}
