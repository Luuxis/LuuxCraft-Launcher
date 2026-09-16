//! Installation différentielle du pack client (contrat §7).
//!
//! Le pack porte toute l'identité du tenant : `client_config.json`, lu par le
//! moteur au démarrage, et les icônes utilisées pour les raccourcis. C'est la
//! partie qui bouge — un changement de nom ou de logo ne doit coûter que ces
//! quelques kilo-octets, jamais un moteur de 50 Mo. Fichier par fichier, seule
//! une empreinte différente déclenche un téléchargement.

use std::fs;
use std::path::PathBuf;

use crate::error::{BootstrapError, Result};
use crate::hashing;
use crate::manifest::Manifest;
use crate::net::Http;
use crate::paths::{self, Layout};

pub struct Pack {
    pub dir: PathBuf,
    pub updated: usize,
    pub kept: usize,
}

impl Pack {
    /// Chemin d'un fichier du pack, s'il a bien été installé. Les icônes sont
    /// facultatives : un raccourci sans icône vaut mieux qu'une installation
    /// interrompue.
    pub fn asset(&self, name: &str) -> Option<PathBuf> {
        let path = self.dir.join(name);
        path.is_file().then_some(path)
    }
}

pub fn ensure(http: &Http, layout: &Layout, manifest: &Manifest) -> Result<Pack> {
    paths::create_dir(&layout.client_dir)?;

    let mut updated = 0;
    let mut kept = 0;

    for (index, file) in manifest.client_pack.files.iter().enumerate() {
        let relative = paths::safe_relative(&file.path).ok_or_else(|| {
            BootstrapError::new(format!("chemin de pack client invalide : {}", file.path))
        })?;
        let target = layout.client_dir.join(&relative);

        if let Some(current) = hashing::sha256_file(&target)? {
            if hashing::matches(&current, &file.sha256) {
                kept += 1;
                continue;
            }
        }

        let staged = layout.tmp(&format!("pack-{index}"));
        let hash = http.download(&file.url, &staged, file.size, &file.path)?;
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
