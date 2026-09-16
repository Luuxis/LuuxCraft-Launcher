//! Installeur figé LuuxCraft.
//!
//! Un seul binaire est compilé par (OS, arch) et publié dans R2. Le panel le
//! sert tel quel, en lui collant 32 octets d'overlay au vol : l'UUID du tenant,
//! et rien d'autre (contrat §2). Ce binaire ne sait donc rien du client avant
//! d'avoir lu sa propre fin, puis le manifeste.
//!
//! Ce qu'il fait, dans l'ordre :
//!
//! 1. lit l'UUID du tenant dans son propre fichier ;
//! 2. demande au panel le manifeste de ce tenant ;
//! 3. installe ou met à jour le moteur, puis le pack client, en ne
//!    téléchargeant que ce dont l'empreinte a changé (contrat §7) ;
//! 4. pose les raccourcis au nom du client — et sous macOS fabrique le `.app` ;
//! 5. lance le moteur et rend la main.
//!
//! Il n'embarque ni tauri ni webview : il doit rester un petit fichier que le
//! joueur télécharge en quelques secondes.

mod config;
mod engine;
mod error;
mod hashing;
mod launch;
mod manifest;
mod net;
mod overlay;
mod pack;
mod paths;
mod shortcuts;
mod unpack;

#[cfg(target_os = "linux")]
mod linux_desktop;
#[cfg(target_os = "macos")]
mod macos_bundle;
// Compilé sur toutes les plateformes pour que ses tests d'alignement tournent
// en intégration continue, même sur une machine Linux ; seul Windows l'appelle.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod windows_lnk;

use error::Result;

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
    println!("Installation LuuxCraft");

    let tenant_id = overlay::read_tenant_id()?.to_string();
    let http = net::Http::new();

    println!("[1/4] Configuration du client");
    let manifest = manifest::fetch(&http, &tenant_id)?;
    println!("      {} ({})", manifest.display_name(), manifest.slug);

    let layout = paths::Layout::resolve(&manifest)?;
    layout.prepare()?;
    println!("      dossier : {}", layout.root.display());

    println!("[2/4] Moteur");
    let engine_executable = engine::ensure(&http, &layout, &manifest)?;

    println!("[3/4] Pack client");
    let pack = pack::ensure(&http, &layout, &manifest)?;
    println!(
        "      {} fichier(s) installé(s), {} déjà à jour",
        pack.updated, pack.kept
    );

    println!("[4/4] Raccourcis");
    let target = shortcuts::install(&layout, &manifest, &engine_executable, &pack)?;

    layout.cleanup_tmp();

    println!();
    println!("Lancement de {}...", manifest.display_name());
    launch::launch(&layout, &target, &manifest)
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
