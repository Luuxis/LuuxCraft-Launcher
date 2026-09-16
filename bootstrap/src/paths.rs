//! Disposition locale de l'installation.
//!
//! ```text
//! %LOCALAPPDATA%\Programs\<slug>\
//!   engine\<Nom du serveur>.exe    le moteur, cible du raccourci
//!   client\                        l'identité du serveur, lue par le moteur
//!   .engine-version                ce qui est installé, pour le différentiel
//!   .pack-version
//! ```
//!
//! Le `slug` vient du panel, jamais d'un calcul local : c'est lui qui sépare
//! deux serveurs installés sur la même machine. Les données de jeu, elles,
//! vivent ailleurs (`%APPDATA%\.<slug>`) et ne sont jamais touchées ici — c'est
//! ce qui permet à une réinstallation de ne rien effacer des comptes ni des
//! réglages du joueur.
//!
//! Le dossier d'installation est sous `%LOCALAPPDATA%` et non sous
//! `Program Files` : aucun droit administrateur n'est demandé, et deux serveurs
//! peuvent coexister sans se disputer un emplacement partagé.

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
        let root = install_root(&manifest.slug)?;
        Ok(Self {
            engine_dir: root.join("engine"),
            client_dir: root.join("client"),
            tmp_dir: root.join(".tmp"),
            root,
        })
    }

    /// Empreinte et version du bundle moteur installé.
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
fn install_root(slug: &str) -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(missing_home)?;
    Ok(base.join("Programs").join(safe_file_name(slug)))
}

/// Hors Windows, ce crate n'est jamais distribué : cette branche n'existe que
/// pour que le code compile — et que ses tests tournent — sur l'intégration
/// continue Linux.
#[cfg(not(target_os = "windows"))]
fn install_root(slug: &str) -> Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(missing_home)?;
    Ok(base.join(safe_file_name(slug)))
}

fn missing_home() -> BootstrapError {
    BootstrapError::new("impossible de trouver votre dossier utilisateur.").hint(
        "Ouvrez une session Windows normale avant de relancer l'installation.",
    )
}

/// Noms de périphériques réservés par Windows, quelle que soit l'extension.
///
/// `CON`, `PRN`, `AUX`, `NUL`, `COM1`…`COM9`, `LPT1`…`LPT9` désignent des
/// périphériques depuis MS-DOS : `CON.lnk` comme `CON` tout court sont
/// irrecevables, et créer un dossier ainsi nommé échoue sans message utile.
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Nom de fichier ou de dossier tiré d'une valeur du panel.
///
/// Trois choses lui sont retirées, et rien de plus : les caractères que
/// Windows interdit, les points et espaces de fin (l'Explorateur les mange
/// silencieusement, ce qui ferait diverger le chemin écrit du chemin relu), et
/// l'ambiguïté d'un nom réservé.
///
/// Le panel produit déjà des slugs `[a-z0-9-]`, donc ce filtre ne sert
/// normalement à rien — et c'est exactement pourquoi il est là : aucune valeur
/// venue du réseau ne doit décider seule d'un chemin sur le disque du joueur.
pub fn safe_file_name(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    let cleaned = cleaned
        .trim()
        .trim_end_matches(&['.', ' '][..])
        .trim()
        .to_owned();

    if cleaned.is_empty() {
        return "launcher".to_owned();
    }

    // Le nom réservé se juge sur le radical : `con.lnk` est refusé comme `con`.
    let stem = cleaned.split('.').next().unwrap_or(&cleaned).to_ascii_lowercase();
    if RESERVED.contains(&stem.as_str()) {
        return format!("{cleaned}_");
    }
    cleaned
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
    // Windows refuse de renommer sur une cible existante : la retirer d'abord
    // est ce qui rend l'opération idempotente d'une réinstallation à l'autre.
    let _ = fs::remove_file(target);
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
        assert_eq!(safe_relative("./icon.png"), Some(PathBuf::from("icon.png")));
    }

    /// Un serveur nommé « CON » produirait un slug `con`, et
    /// `%LOCALAPPDATA%\Programs\con` ne peut pas exister.
    #[test]
    fn reserved_windows_names_are_pushed_out_of_the_way() {
        for name in ["con", "CON", "nul", "Aux", "com1", "LPT9", "con.lnk"] {
            let safe = safe_file_name(name);
            assert!(safe.ends_with('_'), "{name:?} -> {safe:?} must be escaped");
        }
        assert_eq!(safe_file_name("console"), "console");
        assert_eq!(safe_file_name("mon-serveur"), "mon-serveur");
    }

    /// L'Explorateur mange les points et espaces de fin : un nom écrit
    /// « Serveur. » se relit « Serveur », et le chemin ne correspondrait plus.
    #[test]
    fn trailing_dots_and_spaces_are_dropped() {
        assert_eq!(safe_file_name("Mon Serveur. "), "Mon Serveur");
        assert_eq!(safe_file_name("  "), "launcher");
        assert_eq!(safe_file_name("..."), "launcher");
    }

    #[test]
    fn characters_windows_forbids_are_removed() {
        assert_eq!(safe_file_name("a:b*c?d\"e<f>g|h/i\\j"), "abcdefghij");
    }
}
