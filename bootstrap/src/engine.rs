//! Installation du moteur.
//!
//! Le moteur est **générique** : un seul bundle est publié pour tous les
//! serveurs, et c'est le pack client qui porte l'identité. Il n'est donc
//! téléchargé que lorsque le panel en publie un autre — un changement de nom ou
//! d'icône ne coûte que quelques kilo-octets au joueur.
//!
//! `.engine-version` garde trace de ce qui est installé, sur trois lignes :
//!
//! ```text
//! 1.4.0
//! 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
//! Mon Serveur.exe
//! ```
//!
//! version, empreinte du bundle téléchargé, puis nom de l'exécutable dans
//! `engine/`. La troisième ligne évite de re-parcourir l'arborescence, et le
//! moteur s'en sert à son tour pour se remplacer lui-même quand il se met à
//! jour.
//!
//! Le dossier `engine/` est remplacé **en entier**, jamais complété : c'est ce
//! qui évite qu'un renommage du serveur laisse derrière lui l'exécutable de
//! l'ancien nom.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{BootstrapError, Result};
use crate::hashing;
use crate::manifest::Manifest;
use crate::net::Http;
use crate::paths::{self, Layout};
use crate::report::Report;
use crate::unpack;

/// Installe le moteur si nécessaire et rend le chemin de son exécutable.
pub fn ensure(
    http: &Http,
    layout: &Layout,
    manifest: &Manifest,
    report: &dyn Report,
) -> Result<PathBuf> {
    let wanted = &manifest.engine;

    if let Some(state) = read_state(&layout.engine_state()) {
        if state.version == wanted.version && hashing::matches(&state.sha256, &wanted.sha256) {
            if let Some(executable) = installed_executable(layout, &state) {
                report.detail(&format!("moteur {} déjà à jour", wanted.version));
                return Ok(executable);
            }
        }
    }

    // Le format est lu dans le manifeste et non déduit de l'extension : c'est
    // le panel qui sait ce qu'il a publié. Un bootstrap Windows ne sait lire
    // que l'archive Windows.
    if normalize_format(&wanted.format) != "exezip" {
        return Err(BootstrapError::new(format!(
            "cet installeur ne sait pas lire un moteur au format « {} ».",
            wanted.format
        ))
        .hint("Retéléchargez l'installeur depuis le panel : celui-ci est trop ancien."));
    }

    report.detail(&format!("moteur {} à installer", wanted.version));
    let archive = layout.tmp("engine-bundle");
    let hash = http.download(&wanted.url, &archive, wanted.size, "moteur", report)?;
    if !hashing::matches(&hash, &wanted.sha256) {
        return Err(corrupted());
    }

    let staging = layout.tmp("engine-stage");
    let _ = fs::remove_dir_all(&staging);
    unpack::zip(&archive, &staging)?;

    let extracted = find_executable(&staging)?;
    let name = executable_name(manifest);
    let prepared = layout.tmp("engine-new");
    let _ = fs::remove_dir_all(&prepared);
    paths::create_dir(&prepared)?;
    // Renommé au nom du serveur : c'est ce que le joueur lira dans le
    // gestionnaire des tâches et dans la cible de son raccourci. Renommer ne
    // touche à aucun octet, donc ne casserait aucune signature du moteur.
    fs::rename(&extracted, prepared.join(&name)).map_err(|error| {
        BootstrapError::io("La mise en place du moteur", &prepared.join(&name), &error)
    })?;

    // L'état est effacé avant l'échange : une coupure de courant au milieu doit
    // laisser une installation « à refaire », jamais une installation déclarée
    // bonne alors que `engine/` est à moitié remplacé.
    let _ = fs::remove_file(layout.engine_state());
    paths::replace_dir(&prepared, &layout.engine_dir, &layout.tmp("engine-old"))?;

    write_state(&layout.engine_state(), &wanted.version, &wanted.sha256, &name)?;
    Ok(layout.engine_dir.join(&name))
}

/// Nom donné à l'exécutable du moteur une fois installé.
///
/// Le nom affiché du serveur, débarrassé de ce qu'un nom de fichier Windows ne
/// supporte pas — noms de périphériques réservés compris (voir
/// `paths::safe_file_name`).
fn executable_name(manifest: &Manifest) -> String {
    format!("{}.exe", paths::safe_file_name(manifest.display_name()))
}

fn normalize_format(format: &str) -> String {
    format
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Chemin de l'exécutable déjà installé, s'il est toujours là.
fn installed_executable(layout: &Layout, state: &State) -> Option<PathBuf> {
    let relative = paths::safe_relative(&state.executable)?;
    let path = layout.engine_dir.join(relative);
    path.is_file().then_some(path)
}

/// Repère l'exécutable du moteur dans l'archive fraîchement extraite.
///
/// Le manifeste ne nomme pas le binaire : il décrit un bundle générique, dont
/// le nom change avec les versions. L'archive Windows ne contient que
/// l'exécutable nu, mais on écarte tout de même un éventuel désinstalleur, qui
/// effacerait l'installation au lieu de la lancer.
fn find_executable(root: &Path) -> Result<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, 8, &mut files);

    let mut candidates: Vec<PathBuf> = files
        .into_iter()
        .filter(|path| {
            path.extension()
                .map(|extension| extension.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
        })
        .filter(|path| !stem_lowercase(path).contains("uninstall"))
        .collect();

    if candidates.is_empty() {
        return Err(BootstrapError::new(
            "le moteur téléchargé ne contient aucun exécutable.",
        )
        .hint("Prévenez le propriétaire du serveur : la version publiée est cassée."));
    }

    // Plusieurs exécutables ne devraient pas arriver ; si c'est le cas, le plus
    // gros est le moteur — les outils qui l'accompagnent pèsent toujours moins.
    candidates.sort_by_key(|path| fs::metadata(path).map(|meta| meta.len()).unwrap_or(0));
    Ok(candidates.pop().expect("candidates is not empty"))
}

fn stem_lowercase(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Parcours borné : une archive hostile ne doit pas faire tourner le bootstrap
/// indéfiniment. Les liens symboliques ne sont pas suivis (ni fichier ni
/// dossier pour `file_type`), ce qui coupe aussi les boucles.
fn collect_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 || out.len() >= 4096 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_files(&entry.path(), depth - 1, out);
        } else if kind.is_file() {
            out.push(entry.path());
        }
    }
}

struct State {
    version: String,
    sha256: String,
    executable: String,
}

fn read_state(path: &Path) -> Option<State> {
    let content = fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    Some(State {
        version: lines.next()?.trim().to_owned(),
        sha256: lines.next()?.trim().to_owned(),
        executable: lines.next().unwrap_or_default().trim().to_owned(),
    })
}

fn write_state(path: &Path, version: &str, sha256: &str, executable: &str) -> Result<()> {
    let content = format!("{version}\n{sha256}\n{executable}\n");
    fs::write(path, content).map_err(|error| BootstrapError::io("L'écriture", path, &error))
}

fn corrupted() -> BootstrapError {
    BootstrapError::new("le moteur téléchargé est corrompu (empreinte incorrecte).").hint(
        "Relancez l'installation. Si le problème persiste, votre connexion altère \
         le téléchargement (proxy, antivirus, réseau d'entreprise).",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_are_matched_whatever_the_separator() {
        assert_eq!(normalize_format("exe-zip"), "exezip");
        assert_eq!(normalize_format("EXE_ZIP"), "exezip");
        assert_eq!(normalize_format("AppImage"), "appimage");
    }

    #[test]
    fn state_needs_two_lines_at_least() {
        assert!(read_state(Path::new("/nonexistent-engine-state")).is_none());
    }

    /// Le nom du serveur devient un nom de fichier : il doit rester utilisable
    /// sous Windows, noms de périphériques réservés compris.
    #[test]
    fn the_executable_is_named_after_the_server() {
        let mut manifest = crate::manifest::tests_support::sample();
        manifest.display_name = "Mon Serveur".into();
        assert_eq!(executable_name(&manifest), "Mon Serveur.exe");

        manifest.display_name = "CON".into();
        assert_eq!(executable_name(&manifest), "CON_.exe");

        manifest.display_name = "  ".into();
        assert_eq!(executable_name(&manifest), "mon-serveur.exe");
    }
}
