//! Raccourcis au nom et au logo du serveur.
//!
//! Le menu Démarrer en reçoit toujours un. Le Bureau seulement si le joueur
//! l'a demandé — la case est cochée par défaut dans la fenêtre d'installation.
//! Tous pointent sur l'exécutable du **moteur**, jamais sur l'installeur :
//! c'est ce qui fait que le bootstrap ne sert qu'à la première installation
//! et ne se remet plus jamais entre le joueur et son launcher.
//!
//! Les `.lnk` sont écrits par le shell de Windows lui-même
//! (`win::shell_link`), pas à la main. Le format écrit à la main auparavant
//! décrivait la cible par son chemin seul ; or c'est la liste d'identifiants
//! du shell (`LinkTargetIDList`) qu'Explorer suit pour ouvrir un raccourci, et
//! elle manquait : le fichier existait, mais rien ne se lançait. Demander au
//! shell d'écrire le fichier garantit qu'il saura le relire.
//!
//! Le raccourci est refait à chaque passage de l'installeur, ce qui suffit à
//! suivre un changement de nom ou de logo côté panel. Décocher le Bureau ne
//! retire pas un raccourci existant : le joueur a pu le garder exprès.
//!
//! Un échec n'est jamais fatal : le joueur préfère un launcher installé sans
//! raccourci à une installation interrompue à la dernière étape.

use std::path::{Path, PathBuf};

use crate::install::Options;
use crate::manifest::Manifest;
use crate::pack::Pack;
use crate::paths::Layout;
use crate::report::Report;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Place {
    StartMenu,
    Desktop,
}

impl Place {
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn label(self) -> &'static str {
        match self {
            Place::StartMenu => "menu Démarrer",
            Place::Desktop => "Bureau",
        }
    }
}

/// Où poser un raccourci : le menu Démarrer toujours, le Bureau au choix.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn places(options: &Options) -> Vec<Place> {
    let mut places = vec![Place::StartMenu];
    if options.desktop_shortcut {
        places.push(Place::Desktop);
    }
    places
}

/// Pose les raccourcis et rend ce qu'il faut lancer.
pub fn install(
    layout: &Layout,
    manifest: &Manifest,
    engine_executable: &Path,
    pack: &Pack,
    options: &Options,
    report: &dyn Report,
) -> PathBuf {
    place(layout, manifest, engine_executable, pack, options, report);
    engine_executable.to_path_buf()
}

#[cfg(windows)]
fn place(
    layout: &Layout,
    manifest: &Manifest,
    engine_executable: &Path,
    pack: &Pack,
    options: &Options,
    report: &dyn Report,
) {
    use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Programs};

    use crate::win::{known_folder, shell_link};

    // L'icône du serveur, au format que Windows attend pour un raccourci. Son
    // absence n'est pas une erreur : le pack ne contient d'`icon.ico` que si
    // le serveur a téléversé un logo PNG.
    let icon = pack.asset("icon.ico");
    let name = manifest.display_name();
    let file = format!("{}.lnk", crate::paths::safe_file_name(name));

    for place in places(options) {
        let folder = match place {
            Place::StartMenu => known_folder(&FOLDERID_Programs),
            Place::Desktop => known_folder(&FOLDERID_Desktop),
        };
        let Some(folder) = folder else {
            report.detail(&format!(
                "! {} introuvable : raccourci non créé",
                place.label()
            ));
            continue;
        };

        let path = folder.join(&file);
        match shell_link::write(&path, engine_executable, &layout.root, icon.as_deref(), name) {
            Ok(()) => report.detail(&format!("{} : {}", place.label(), path.display())),
            Err(error) => report.detail(&format!(
                "! {} : raccourci non créé ({error})",
                place.label()
            )),
        }
    }

    if !options.desktop_shortcut {
        report.detail("Bureau : pas de raccourci, comme demandé");
    }
}

/// Hors Windows, ce crate n'est jamais distribué (voir `paths`).
#[cfg(not(windows))]
fn place(
    _layout: &Layout,
    _manifest: &Manifest,
    _engine: &Path,
    _pack: &Pack,
    _options: &Options,
    report: &dyn Report,
) {
    report.detail("raccourcis : sans objet hors Windows");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_menu_always_gets_a_shortcut() {
        let without = Options {
            desktop_shortcut: false,
        };
        assert_eq!(places(&without), vec![Place::StartMenu]);
    }

    #[test]
    fn the_desktop_only_when_asked() {
        let with = Options {
            desktop_shortcut: true,
        };
        assert_eq!(places(&with), vec![Place::StartMenu, Place::Desktop]);
    }
}
