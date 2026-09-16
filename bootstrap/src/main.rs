//! Installeur Windows LuuxCraft.
//!
//! Un seul binaire est compilé par (OS, arch) et publié vierge par la CI. Le
//! panel en tire une copie par serveur en remplissant son créneau d'identité
//! (87 octets, voir `identity`), la range dans R2, et sert le même fichier à
//! tous les joueurs de ce serveur.
//!
//! Ce qu'il fait, dans l'ordre :
//!
//! 1. lit l'identifiant du serveur dans son propre créneau ;
//! 2. demande au panel le manifeste de ce serveur ;
//! 3. installe le moteur, puis le pack client, en ne téléchargeant que ce dont
//!    l'empreinte a changé ;
//! 4. pose les raccourcis au nom et au logo du serveur ;
//! 5. lance le moteur et rend la main.
//!
//! **Puis il disparaît du tableau.** Les raccourcis pointent sur le moteur, pas
//! sur lui : les mises à jour suivantes sont l'affaire du moteur lui-même
//! (`src-tauri/src/update.rs`). Le relancer reste utile pour réparer une
//! installation cassée, jamais pour la tenir à jour.
//!
//! Il n'embarque ni tauri ni webview : c'est le premier fichier que le joueur
//! télécharge, il doit arriver en quelques secondes.

mod config;
mod engine;
mod error;
mod hashing;
mod identity;
mod manifest;
mod net;
mod pack;
mod paths;
mod shortcuts;
mod unpack;
// Compilé sur toutes les plateformes pour que ses tests d'alignement tournent
// en intégration continue, même sur une machine Linux ; seul Windows l'appelle.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod windows_lnk;

use std::path::Path;
use std::process::Command;

use error::{BootstrapError, Result};

fn main() {
    if let Err(error) = run() {
        eprintln!();
        eprintln!("{error}");
        eprintln!();
        wait_before_closing();
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    println!("Installation du launcher");

    let tenant_id = identity::tenant_id()?;
    let http = net::Http::new();

    println!("[1/4] Configuration du serveur");
    let manifest = manifest::fetch(&http, &tenant_id)?;
    println!("      {} ({})", manifest.display_name(), manifest.slug);

    let layout = paths::Layout::resolve(&manifest)?;
    layout.prepare()?;
    println!("      dossier : {}", layout.root.display());

    println!("[2/4] Moteur");
    let engine_executable = engine::ensure(&http, &layout, &manifest)?;

    println!("[3/4] Configuration");
    let pack = pack::ensure(&http, &layout, &manifest)?;
    println!(
        "      {} fichier(s) écrit(s), {} déjà à jour",
        pack.updated, pack.kept
    );

    println!("[4/4] Raccourcis");
    let target = shortcuts::install(&layout, &manifest, &engine_executable, &pack);

    layout.cleanup_tmp();

    println!();
    println!("Lancement de {}...", manifest.display_name());
    launch(&layout.root, &target)
}

/// Démarre le moteur, puis rend la main sans le surveiller : aucune fenêtre de
/// console ne doit rester ouverte derrière le launcher.
fn launch(working_dir: &Path, target: &Path) -> Result<()> {
    Command::new(target)
        .current_dir(working_dir)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            BootstrapError::io("Le démarrage du launcher", target, &error).or_hint(
                "Le fichier a peut-être été mis en quarantaine par votre antivirus. \
                 Relancez l'installation après l'avoir autorisé.",
            )
        })
}

/// Windows : l'installeur est lancé par un double-clic, sa console disparaît
/// avec lui. Sans cette pause, le joueur ne verrait jamais le message d'erreur.
#[cfg(target_os = "windows")]
fn wait_before_closing() {
    use std::io::{BufRead, Write};

    print!("Appuyez sur Entrée pour fermer cette fenêtre.");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
}

#[cfg(not(target_os = "windows"))]
fn wait_before_closing() {}
