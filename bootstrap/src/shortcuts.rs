//! Raccourcis au nom et au logo du serveur.
//!
//! Deux emplacements : le Bureau et le menu Démarrer. Les deux pointent sur
//! l'exécutable du **moteur**, jamais sur l'installeur — c'est ce qui fait que
//! le bootstrap ne sert qu'à la première installation et ne se remet plus
//! jamais entre le joueur et son launcher.
//!
//! Le raccourci est refait à chaque passage de l'installeur, ce qui suffit à
//! suivre un changement de nom ou de logo côté panel.
//!
//! Un échec n'est jamais fatal : le joueur préfère un launcher installé sans
//! raccourci à une installation interrompue à la dernière étape.

use std::path::{Path, PathBuf};

use crate::manifest::Manifest;
use crate::pack::Pack;
use crate::paths::Layout;

/// Pose les raccourcis et rend ce qu'il faut lancer.
pub fn install(layout: &Layout, manifest: &Manifest, engine_executable: &Path, pack: &Pack) -> PathBuf {
    place(layout, manifest, engine_executable, pack);
    engine_executable.to_path_buf()
}

#[cfg(target_os = "windows")]
fn place(layout: &Layout, manifest: &Manifest, engine_executable: &Path, pack: &Pack) {
    // L'icône du serveur, au format que Windows attend pour un raccourci. Son
    // absence n'est pas une erreur : le pack ne contient d'`icon.ico` que si le
    // serveur a téléversé un logo PNG.
    let icon = pack.asset("icon.ico");
    let file = format!(
        "{}.lnk",
        crate::paths::safe_file_name(manifest.display_name())
    );

    let start_menu = dirs::data_dir().map(|base| {
        base.join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
    });

    for directory in [dirs::desktop_dir(), start_menu].into_iter().flatten() {
        let path = directory.join(&file);
        match crate::windows_lnk::write(
            &path,
            engine_executable,
            &layout.root,
            icon.as_deref(),
            manifest.display_name(),
        ) {
            Ok(()) => println!("      raccourci : {}", path.display()),
            Err(error) => println!("      ! raccourci non créé ({}) : {error}", path.display()),
        }
    }
}

/// Hors Windows, ce crate n'est jamais distribué (voir `paths`).
#[cfg(not(target_os = "windows"))]
fn place(_layout: &Layout, _manifest: &Manifest, _engine: &Path, _pack: &Pack) {
    println!("      raccourcis : sans objet hors Windows");
}
