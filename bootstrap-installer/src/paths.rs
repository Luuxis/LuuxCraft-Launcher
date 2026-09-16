//! Disposition locale de l'installation (contrat §6).
//!
//! | OS | Racine |
//! |---|---|
//! | Windows | `%LOCALAPPDATA%\Programs\<slug>\` |
//! | Linux | `~/.local/share/<slug>/` |
//! | macOS | `~/Applications/<display_name>.app/` |
//!
//! Le slug vient du panel, jamais d'une dérivation locale : c'est lui qui
//! isole deux clients installés sur la même machine. La racine contient
//! `engine/`, `client/`, `.engine-version` et `.pack-version` ; les données de
//! jeu, elles, vivent ailleurs (`~/.<slug>`) et ne sont jamais touchées ici.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{BootstrapError, Result};
use crate::manifest::Manifest;

pub struct Layout {
    pub root: PathBuf,
    pub engine_dir: PathBuf,
    pub client_dir: PathBuf,
    pub tmp_dir: PathBuf,
}

impl Layout {
    pub fn resolve(manifest: &Manifest) -> Result<Self> {
        let root = install_root(manifest)?;
        Ok(Self {
            engine_dir: root.join("engine"),
            client_dir: root.join("client"),
            tmp_dir: root.join(".tmp"),
            root,
        })
    }

    /// Empreinte et version du bundle moteur installé (contrat §7).
    pub fn engine_state(&self) -> PathBuf {
        self.root.join(".engine-version")
    }

    /// Empreinte agrégée du pack client installé.
    pub fn pack_state(&self) -> PathBuf {
        self.root.join(".pack-version")
    }

    /// Crée la racine et repart d'un dossier temporaire vide : ce qu'une
    /// tentative précédente y a laissé n'a plus aucune valeur.
    pub fn prepare(&self) -> Result<()> {
        create_dir(&self.root)?;
        create_dir(&self.client_dir)?;
        let _ = fs::remove_dir_all(&self.tmp_dir);
        create_dir(&self.tmp_dir)
    }

    pub fn cleanup_tmp(&self) {
        let _ = fs::remove_dir_all(&self.tmp_dir);
    }

    pub fn tmp(&self, name: &str) -> PathBuf {
        self.tmp_dir.join(name)
    }
}

#[cfg(target_os = "windows")]
fn install_root(manifest: &Manifest) -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(missing_home)?;
    Ok(base.join("Programs").join(&manifest.slug))
}

#[cfg(target_os = "linux")]
fn install_root(manifest: &Manifest) -> Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(missing_home)?;
    Ok(base.join(&manifest.slug))
}

/// macOS : la racine **est** le bundle, que le bootstrap fabrique lui-même
/// (contrat §0.2). Deux clients coexistent parce que leurs `.app` portent des
/// noms et des identifiants de bundle différents.
#[cfg(target_os = "macos")]
fn install_root(manifest: &Manifest) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(missing_home)?;
    Ok(home
        .join("Applications")
        .join(format!("{}.app", bundle_file_name(manifest.display_name()))))
}

fn missing_home() -> BootstrapError {
    BootstrapError::new("impossible de trouver votre dossier personnel.").hint(
        "Ouvrez une session utilisateur normale avant de relancer l'installation.",
    )
}

/// Nom de bundle macOS : le nom affiché, débarrassé de ce qu'un nom de fichier
/// ne supporte pas. `:` est interdit (le Finder l'affiche comme `/`).
#[cfg(target_os = "macos")]
pub fn bundle_file_name(display_name: &str) -> String {
    let cleaned: String = display_name
        .chars()
        .filter(|c| !matches!(c, '/' | ':' | '\\') && !c.is_control())
        .take(60)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_owned();
    if cleaned.is_empty() {
        "LuuxCraft".to_owned()
    } else {
        cleaned
    }
}

pub fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| BootstrapError::io("La création du dossier", path, &error))
}

/// Chemin relatif sûr extrait d'un nom venu du réseau (entrée d'archive ou
/// `path` du pack client) : tout ce qui pourrait sortir de la racine est
/// refusé, y compris les `..` et les préfixes de lecteur Windows.
pub fn safe_relative(name: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for part in name.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains(':') {
            return None;
        }
        depth += 1;
        if depth > 16 {
            return None;
        }
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Met un fichier en place depuis le dossier temporaire.
///
/// `rename` écrase la cible et est atomique tant que source et destination
/// partagent le même système de fichiers — le dossier temporaire est pour cela
/// dans la racine d'installation, et non dans celui du système.
pub fn replace_file(staged: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        create_dir(parent)?;
    }
    fs::rename(staged, target).map_err(|error| {
        BootstrapError::io("La mise en place du fichier", target, &error).or_hint(
            "Fermez le launcher s'il est déjà ouvert, puis relancez l'installation.",
        )
    })
}

/// Échange un dossier fraîchement préparé avec celui en place.
///
/// L'ancien est d'abord écarté puis supprimé seulement une fois le nouveau
/// posé : si le second renommage échoue, on remet l'ancien, et le joueur garde
/// un client qui démarre.
pub fn replace_dir(staged: &Path, target: &Path, trash: &Path) -> Result<()> {
    let _ = fs::remove_dir_all(trash);
    if target.exists() {
        fs::rename(target, trash).map_err(|error| {
            BootstrapError::io("Le remplacement du moteur", target, &error).or_hint(
                "Fermez le launcher s'il est déjà ouvert, puis relancez l'installation.",
            )
        })?;
    }
    if let Some(parent) = target.parent() {
        create_dir(parent)?;
    }
    match fs::rename(staged, target) {
        Ok(()) => {
            let _ = fs::remove_dir_all(trash);
            Ok(())
        }
        Err(error) => {
            let _ = fs::rename(trash, target);
            Err(BootstrapError::io("Le remplacement du moteur", target, &error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_that_escape_the_root() {
        assert!(safe_relative("../etc/passwd").is_none());
        assert!(safe_relative("..\\windows\\system32").is_none());
        assert!(safe_relative("C:\\windows").is_none());
        assert!(safe_relative("").is_none());
        assert!(safe_relative("/").is_none());
    }

    #[test]
    fn keeps_plain_relative_paths() {
        assert_eq!(
            safe_relative("client_config.json"),
            Some(PathBuf::from("client_config.json"))
        );
        assert_eq!(
            safe_relative("a/b/icon.png"),
            Some(PathBuf::from("a").join("b").join("icon.png"))
        );
        assert_eq!(
            safe_relative("./icon.png"),
            Some(PathBuf::from("icon.png"))
        );
    }
}
