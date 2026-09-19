//! Le déroulé de l'installation, indépendant de l'écran qui le montre.
//!
//! Deux temps, parce que la fenêtre a besoin d'une pause entre les deux :
//!
//! 1. `prepare` — lire l'identité, demander le manifeste, décider du dossier.
//!    **Rien n'est écrit sur le disque** : c'est ce qui permet à la fenêtre
//!    d'annoncer le serveur, de montrer son logo et de laisser le joueur
//!    choisir ses raccourcis avant que quoi que ce soit ne soit installé.
//! 2. `install` — moteur, pack client, raccourcis ; puis `launch`.
//!
//! La console (hors Windows, pour les tests) enchaîne les deux sans s'arrêter.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine;
use crate::error::{BootstrapError, Result};
use crate::identity;
use crate::manifest::{self, Manifest};
use crate::net::Http;
use crate::pack;
use crate::paths::Layout;
use crate::report::Report;
use crate::shortcuts;

/// Ce que le joueur a choisi.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Un raccourci sur le Bureau, en plus de celui du menu Démarrer. Lu par
    /// les raccourcis Windows seulement : ailleurs, ce crate n'en pose aucun.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub desktop_shortcut: bool,
}

/// Tout ce qu'il faut savoir avant d'installer.
pub struct Prepared {
    pub manifest: Manifest,
    pub layout: Layout,
}

pub fn prepare(http: &Http, report: &dyn Report) -> Result<Prepared> {
    let tenant_id = identity::tenant_id()?;

    report.step(1, "Configuration du serveur");
    let manifest = manifest::fetch(http, &tenant_id)?;
    report.detail(&format!("{} ({})", manifest.display_name(), manifest.slug));

    let layout = Layout::resolve(&manifest)?;
    Ok(Prepared { manifest, layout })
}

/// Installe tout et rend ce qu'il faut lancer.
pub fn install(
    http: &Http,
    prepared: &Prepared,
    options: &Options,
    report: &dyn Report,
) -> Result<PathBuf> {
    let Prepared { manifest, layout } = prepared;
    layout.prepare()?;

    report.step(2, "Moteur");
    let engine_executable = engine::ensure(http, layout, manifest, report)?;

    report.step(3, "Configuration");
    let pack = pack::ensure(http, layout, manifest, report)?;
    report.detail(&format!(
        "{} fichier(s) écrit(s), {} déjà à jour",
        pack.updated, pack.kept
    ));

    report.step(4, "Raccourcis");
    let target = shortcuts::install(layout, manifest, &engine_executable, &pack, options, report);

    layout.cleanup_tmp();
    Ok(target)
}

/// Démarre le moteur, puis rend la main sans le surveiller : l'installeur
/// n'a plus rien à faire une fois le launcher à l'écran.
pub fn launch(prepared: &Prepared, target: &Path, report: &dyn Report) -> Result<()> {
    report.step(
        5,
        &format!("Lancement de {}", prepared.manifest.display_name()),
    );
    Command::new(target)
        .current_dir(&prepared.layout.root)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            BootstrapError::io("Le démarrage du launcher", target, &error).or_hint(
                "Le fichier a peut-être été mis en quarantaine par votre antivirus. \
                 Relancez l'installation après l'avoir autorisé.",
            )
        })
}
