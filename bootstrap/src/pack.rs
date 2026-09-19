//! Installation du pack client.
//!
//! Le pack porte toute l'identité du serveur : `client_config.json`, lu par le
//! moteur au démarrage, et les icônes utilisées pour les raccourcis. C'est la
//! partie qui bouge — un changement de nom ou de logo ne doit coûter que ces
//! quelques kilo-octets, jamais un moteur de cinquante méga-octets. Fichier par
//! fichier, seule une empreinte différente déclenche un téléchargement.
//!
//! Il est posé dans `client/`, à côté de `engine/` et **hors** de lui : c'est
//! ce qui fait qu'une mise à jour du moteur — qui ne touche qu'à `engine/` — ne
//! peut pas faire perdre au launcher le serveur auquel il appartient.

use std::fs;
use std::path::PathBuf;

use crate::error::{BootstrapError, Result};
use crate::hashing;
use crate::manifest::Manifest;
use crate::net::Http;
use crate::paths::{self, Layout};
use crate::report::Report;

pub struct Pack {
    /// Seuls les raccourcis Windows y puisent (l'icône `.ico`), d'où le
    /// `cfg_attr` : ailleurs ce crate ne sert qu'à faire tourner ses tests.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub dir: PathBuf,
    pub updated: usize,
    pub kept: usize,
}

impl Pack {
    /// Chemin d'un fichier du pack, s'il a bien été installé. Les icônes sont
    /// facultatives : un raccourci sans icône vaut mieux qu'une installation
    /// interrompue.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn asset(&self, name: &str) -> Option<PathBuf> {
        let path = self.dir.join(name);
        path.is_file().then_some(path)
    }
}

pub fn ensure(
    http: &Http,
    layout: &Layout,
    manifest: &Manifest,
    report: &dyn Report,
) -> Result<Pack> {
    paths::create_dir(&layout.client_dir)?;

    let mut updated = 0;
    let mut kept = 0;

    for (index, file) in manifest.client_pack.files.iter().enumerate() {
        // `validate` a déjà rejeté tout ce qui n'est pas dans la liste fermée ;
        // la jointure repasse quand même par `safe_relative`, parce que c'est
        // ici que le chemin devient un vrai chemin sur le disque.
        let relative = paths::safe_relative(&file.path).ok_or_else(|| {
            BootstrapError::new(format!("chemin de configuration invalide : {}", file.path))
        })?;
        let target = layout.client_dir.join(&relative);

        if let Some(current) = hashing::sha256_file(&target)? {
            if hashing::matches(&current, &file.sha256) {
                kept += 1;
                continue;
            }
        }

        let staged = layout.tmp(&format!("pack-{index}"));
        let hash = http.download(&file.url, &staged, file.size, &file.path, report)?;
        if !hashing::matches(&hash, &file.sha256) {
            return Err(BootstrapError::new(format!(
                "le fichier « {} » est arrivé corrompu (empreinte incorrecte).",
                file.path
            ))
            .hint("Relancez l'installation."));
        }
        paths::replace_file(&staged, &target)?;
        updated += 1;
    }

    let state = layout.pack_state();
    fs::write(&state, format!("{}\n", manifest.client_pack.version))
        .map_err(|error| BootstrapError::io("L'écriture", &state, &error))?;

    Ok(Pack {
        dir: layout.client_dir.clone(),
        updated,
        kept,
    })
}
