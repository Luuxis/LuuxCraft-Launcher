//! Décompression du bundle moteur.
//!
//! Un seul format est décodé : `exe-zip`, l'archive Windows qui contient
//! l'exécutable nu du moteur. Les deux autres formats publiés par la CI
//! (`app-tar-gz`, `appimage`) appartiennent aux plateformes installées par le
//! script `sh` ; embarquer leurs décodeurs ici n'alourdirait que le binaire que
//! le joueur télécharge en premier.
//!
//! L'archive vient du réseau : chaque nom d'entrée repasse par
//! `paths::safe_relative` avant d'être joint à la destination, sans quoi une
//! entrée `../..` écrirait hors de la racine d'installation (Zip Slip).

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use crate::error::{BootstrapError, Result};
use crate::paths;

/// Décompresse une archive zip (bundle `exe-zip`).
pub fn zip(archive: &Path, destination: &Path) -> Result<()> {
    let file =
        File::open(archive).map_err(|error| BootstrapError::io("La lecture", archive, &error))?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).map_err(broken)?;

    paths::create_dir(destination)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(broken)?;
        let name = entry.name().to_owned();
        let Some(relative) = paths::safe_relative(&name) else {
            // Une entrée qui sortirait de la destination est ignorée, pas
            // fatale : une archive peut légitimement contenir des métadonnées
            // que l'on ne veut simplement pas écrire.
            continue;
        };
        let target = destination.join(relative);

        if name.ends_with('/') {
            paths::create_dir(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            paths::create_dir(parent)?;
        }

        let mut out = File::create(&target)
            .map_err(|error| BootstrapError::io("L'extraction", &target, &error))?;
        io::copy(&mut entry, &mut out)
            .map_err(|error| BootstrapError::io("L'extraction", &target, &error))?;

        // Un zip produit sous Windows n'a pas de droits Unix, et une entrée à
        // 0 rendrait le fichier illisible : on ne les applique que s'ils
        // veulent dire quelque chose. Sans objet sous Windows, mais ce crate
        // se compile et se teste aussi sur Linux.
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode().filter(|mode| mode & 0o777 != 0) {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode & 0o777))
                .map_err(|error| BootstrapError::io("Le réglage des droits", &target, &error))?;
        }
    }
    Ok(())
}

fn broken(failure: impl std::fmt::Display) -> BootstrapError {
    BootstrapError::new(format!(
        "le moteur téléchargé n'a pas pu être décompressé ({failure})."
    ))
    .hint("Relancez l'installation : le fichier était probablement incomplet.")
}
