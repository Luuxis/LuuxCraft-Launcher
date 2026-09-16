//! Décompression du bundle moteur.
//!
//! Trois formats, un par plateforme (contrat §5) : `exe-zip` sous Windows,
//! `app-tar-gz` sous macOS, `appimage` sous Linux. Le format est lu dans le
//! manifeste et non déduit de l'extension : c'est le panel qui sait ce qu'il a
//! publié.
//!
//! Les archives viennent du réseau : chaque nom d'entrée repasse par
//! `paths::safe_relative` avant d'être joint à la destination, sans quoi une
//! entrée `../..` écrirait hors de la racine d'installation.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use crate::error::{BootstrapError, Result};
use crate::paths;

/// Décompresse une archive zip (bundle `exe-zip`).
pub fn zip(archive: &Path, destination: &Path) -> Result<()> {
    let file =
        File::open(archive).map_err(|error| BootstrapError::io("La lecture", archive, &error))?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).map_err(|failure| broken(failure))?;

    paths::create_dir(destination)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|failure| broken(failure))?;
        let name = entry.name().to_owned();
        let Some(relative) = paths::safe_relative(&name) else {
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
        // veulent dire quelque chose.
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode().filter(|mode| mode & 0o777 != 0) {
            set_mode(&target, mode & 0o777)?;
        }
    }
    Ok(())
}

/// Décompresse une archive `tar.gz` (bundle `app-tar-gz`).
///
/// Les droits Unix sont conservés : sans le bit d'exécution, le Mach-O du
/// moteur ne serait plus lançable et le `.app` fabriqué juste après ne
/// s'ouvrirait pas.
pub fn tar_gz(archive: &Path, destination: &Path) -> Result<()> {
    let file =
        File::open(archive).map_err(|error| BootstrapError::io("La lecture", archive, &error))?;
    let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut tar = tar::Archive::new(decoder);
    tar.set_preserve_permissions(true);
    tar.set_overwrite(true);

    paths::create_dir(destination)?;
    tar.unpack(destination)
        .map_err(|failure| broken(failure))
}

/// Rend exécutable un binaire brut (AppImage Linux).
#[cfg(unix)]
pub fn make_executable(path: &Path) -> Result<()> {
    set_mode(path, 0o755)
}

#[cfg(not(unix))]
pub fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| BootstrapError::io("Le réglage des droits", path, &error))
}

fn broken(failure: impl std::fmt::Display) -> BootstrapError {
    BootstrapError::new(format!(
        "le moteur téléchargé n'a pas pu être décompressé ({failure})."
    ))
    .hint("Relancez l'installation : le fichier était probablement incomplet.")
}
