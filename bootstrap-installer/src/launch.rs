//! Démarrage du moteur, puis sortie.
//!
//! Le bootstrap ne surveille pas le processus qu'il lance : il rend la main
//! tout de suite, pour qu'aucune fenêtre de console ne reste ouverte derrière
//! le launcher.
//!
//! Les variables d'environnement passées ici ne sont qu'un raccourci pour
//! Windows et Linux ; sous macOS, `open` passe par LaunchServices et n'en
//! transmet aucune. Le moteur doit donc de toute façon savoir retrouver
//! `client/` depuis son propre exécutable.

use std::path::Path;
use std::process::Command;

use crate::error::{BootstrapError, Result};
use crate::manifest::Manifest;
use crate::paths::Layout;

pub fn launch(layout: &Layout, target: &Path, manifest: &Manifest) -> Result<()> {
    let mut command = base_command(target);
    command
        .current_dir(&layout.root)
        .env("LUUXCRAFT_INSTALL_ROOT", &layout.root)
        .env("LUUXCRAFT_CLIENT_DIR", &layout.client_dir)
        .env("LUUXCRAFT_TENANT_ID", &manifest.tenant_id)
        .env("LUUXCRAFT_SLUG", &manifest.slug);

    command.spawn().map(|_| ()).map_err(|error| {
        BootstrapError::io("Le démarrage du launcher", target, &error).or_hint(
            "Le fichier a peut-être été mis en quarantaine par votre antivirus. \
             Relancez l'installation après l'avoir autorisé.",
        )
    })
}

/// macOS : on passe par `open` plutôt que par le Mach-O, pour que le client
/// apparaisse dans le Dock sous son nom et son icône, comme une application
/// lancée depuis le Finder.
#[cfg(target_os = "macos")]
fn base_command(target: &Path) -> Command {
    let mut command = Command::new("/usr/bin/open");
    command.arg(target);
    command
}

#[cfg(not(target_os = "macos"))]
fn base_command(target: &Path) -> Command {
    Command::new(target)
}
