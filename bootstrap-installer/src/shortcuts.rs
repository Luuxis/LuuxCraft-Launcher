//! Raccourcis au nom et au logo du tenant.
//!
//! Chaque plateforme a son conteneur d'identité : le `.lnk` sous Windows,
//! l'entrée de bureau sous Linux, le bundle lui-même sous macOS. Tous sont
//! refaits à chaque lancement du bootstrap, ce qui suffit à suivre un
//! changement de nom ou de logo côté panel.
//!
//! Sous Windows et Linux, un échec n'est jamais fatal : le joueur préfère un
//! client installé sans raccourci à une installation interrompue. Sous macOS,
//! au contraire, le bundle **est** l'application : son échec est celui de
//! l'installation.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::manifest::Manifest;
use crate::pack::Pack;
use crate::paths::Layout;

/// Pose les raccourcis et rend ce qu'il faut lancer.
pub fn install(
    layout: &Layout,
    manifest: &Manifest,
    engine_executable: &Path,
    pack: &Pack,
) -> Result<PathBuf> {
    platform(layout, manifest, engine_executable, pack)
}

#[cfg(target_os = "windows")]
fn platform(
    layout: &Layout,
    manifest: &Manifest,
    engine_executable: &Path,
    pack: &Pack,
) -> Result<PathBuf> {
    let icon = pack.asset("icon.ico");
    let file = format!("{}.lnk", file_name(manifest));

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
            Err(error) => warn(&format!(
                "raccourci non créé ({}) : {error}",
                path.display()
            )),
        }
    }

    Ok(engine_executable.to_path_buf())
}

#[cfg(target_os = "linux")]
fn platform(
    _layout: &Layout,
    manifest: &Manifest,
    engine_executable: &Path,
    pack: &Pack,
) -> Result<PathBuf> {
    let icon = pack.asset("icon.png");
    match crate::linux_desktop::write(manifest, engine_executable, icon.as_deref()) {
        Ok(path) => println!("      entrée de bureau : {}", path.display()),
        Err(error) => warn(&format!("entrée de bureau non créée : {error}")),
    }
    Ok(engine_executable.to_path_buf())
}

#[cfg(target_os = "macos")]
fn platform(
    layout: &Layout,
    manifest: &Manifest,
    engine_executable: &Path,
    pack: &Pack,
) -> Result<PathBuf> {
    let app = crate::macos_bundle::generate(layout, manifest, engine_executable, pack)?;
    println!("      application : {}", app.display());
    Ok(app)
}

/// Nom de fichier du raccourci Windows : le nom affiché, débarrassé de ce que
/// le système interdit, avec repli sur le slug.
#[cfg(target_os = "windows")]
fn file_name(manifest: &Manifest) -> String {
    let cleaned: String = manifest
        .display_name()
        .chars()
        .filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_owned();
    if cleaned.is_empty() {
        manifest.slug.clone()
    } else {
        cleaned
    }
}

#[cfg(not(target_os = "macos"))]
fn warn(message: &str) {
    println!("      ! {message}");
}
