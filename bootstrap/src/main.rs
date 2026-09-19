//! Installeur Windows LuuxCraft.
//!
//! Un seul binaire est compilé par (OS, arch) et publié vierge par la CI. Le
//! panel en tire une copie par serveur en remplissant son créneau d'identité
//! (87 octets, voir `identity`), la range dans R2, et sert le même fichier à
//! tous les joueurs de ce serveur.
//!
//! Ce qu'il fait, dans l'ordre (`install`) :
//!
//! 1. lit l'identifiant du serveur dans son propre créneau ;
//! 2. demande au panel le manifeste de ce serveur, et son logo ;
//! 3. montre au joueur ce qu'il va installer, et lui laisse le choix d'un
//!    raccourci sur le Bureau (`win::gui`) ;
//! 4. installe le moteur, puis le pack client, en ne téléchargeant que ce
//!    dont l'empreinte a changé ;
//! 5. pose les raccourcis au nom et au logo du serveur — menu Démarrer
//!    toujours, Bureau si demandé ;
//! 6. lance le moteur et rend la main.
//!
//! **Puis il disparaît du tableau.** Les raccourcis pointent sur le moteur,
//! pas sur lui : les mises à jour suivantes sont l'affaire du moteur lui-même
//! (`src-tauri/src/update.rs`). Le relancer reste utile pour réparer une
//! installation cassée, jamais pour la tenir à jour.
//!
//! Il n'embarque ni tauri ni webview : c'est le premier fichier que le joueur
//! télécharge, il doit arriver en quelques secondes. Sa fenêtre est du Win32
//! natif, et son icône, son manifeste et ses informations de version sont
//! produits par `build.rs`.
//!
//! Hors Windows, le même déroulé tourne en console (`console`) : ce crate ne
//! s'y distribue pas, mais il s'y compile et s'y teste.

// Pas de console derrière la fenêtre. Les tests gardent la leur : sans
// sortie standard, `cargo test` n'afficherait rien.
#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod config;
mod engine;
mod error;
mod hashing;
mod identity;
mod install;
mod manifest;
mod net;
mod pack;
mod paths;
mod report;
mod resources;
mod shortcuts;
mod unpack;

#[cfg(windows)]
mod logo;
#[cfg(windows)]
mod win;

#[cfg(not(windows))]
mod console;

fn main() {
    #[cfg(windows)]
    let code = win::gui::run();
    #[cfg(not(windows))]
    let code = console::run();
    std::process::exit(code);
}
