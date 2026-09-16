//! Installation différentielle du moteur (contrat §7).
//!
//! Le moteur est **générique** : un seul bundle est publié pour tous les
//! clients, et c'est le pack client qui porte l'identité. Il n'est donc
//! retéléchargé que lorsque le panel en publie un autre — un changement de nom
//! ou d'icône ne coûte que quelques kilo-octets au joueur.
//!
//! `.engine-version` garde trace de ce qui est installé, sur trois lignes :
//!
//! ```text
//! 1.2.3
//! 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
//! luuxcraft-launcher.exe
//! ```
//!
//! version, empreinte du bundle téléchargé, puis chemin de l'exécutable dans
//! `engine/`. La troisième ligne évite de re-parcourir l'arborescence à chaque
//! démarrage ; si elle manque ou ne pointe plus sur rien, le binaire est
//! redétecté.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{BootstrapError, Result};
use crate::hashing;
use crate::manifest::Manifest;
use crate::net::Http;
use crate::paths::{self, Layout};
use crate::unpack;

/// Installe le moteur si nécessaire et rend le chemin de son exécutable.
pub fn ensure(http: &Http, layout: &Layout, manifest: &Manifest) -> Result<PathBuf> {
    let wanted = &manifest.engine;

    if let Some(state) = read_state(&layout.engine_state()) {
        let same = state.version == wanted.version && hashing::matches(&state.sha256, &wanted.sha256);
        if same {
            if let Some(executable) = installed_executable(layout, &state) {
                println!("      moteur {} déjà à jour", wanted.version);
                return Ok(executable);
            }
        }
    }

    println!("      moteur {} à installer", wanted.version);
    let archive = layout.tmp("engine-bundle");
    let hash = http.download(&wanted.url, &archive, wanted.size, "moteur")?;
    if !hashing::matches(&hash, &wanted.sha256) {
        return Err(corrupted());
    }

    let staging = layout.tmp("engine-stage");
    let prepared = layout.tmp("engine-new");
    let _ = fs::remove_dir_all(&staging);
    let _ = fs::remove_dir_all(&prepared);
    let executable = prepare(&archive, &staging, &prepared, manifest)?;

    // L'état est effacé avant l'échange : une coupure de courant au milieu doit
    // laisser une installation « à refaire », jamais une installation déclarée
    // bonne alors que `engine/` est à moitié remplacé.
    let _ = fs::remove_file(layout.engine_state());
    paths::replace_dir(&prepared, &layout.engine_dir, &layout.tmp("engine-old"))?;

    write_state(
        &layout.engine_state(),
        &wanted.version,
        &wanted.sha256,
        &executable,
    )?;
    Ok(layout.engine_dir.join(executable))
}

/// Décompresse le bundle et prépare le futur dossier `engine/`.
///
/// Rend le chemin de l'exécutable, relatif à ce dossier.
fn prepare(
    archive: &Path,
    staging: &Path,
    prepared: &Path,
    manifest: &Manifest,
) -> Result<PathBuf> {
    match normalize_format(&manifest.engine.format).as_str() {
        "nsiszip" | "zip" => {
            unpack::zip(archive, prepared)?;
            find_executable(prepared, manifest)
        }
        "apptargz" | "targz" | "tgz" => {
            unpack::tar_gz(archive, staging)?;
            flatten_macos_bundle(staging, prepared, manifest)
        }
        "appimage" | "bin" | "binary" | "raw" | "exe" => {
            paths::create_dir(prepared)?;
            let name = binary_name(manifest);
            let target = prepared.join(&name);
            fs::rename(archive, &target)
                .map_err(|error| BootstrapError::io("La mise en place du moteur", &target, &error))?;
            unpack::make_executable(&target)?;
            Ok(PathBuf::from(name))
        }
        _ => Err(BootstrapError::new(format!(
            "cet installeur ne sait pas lire un moteur au format « {} ».",
            manifest.engine.format
        ))
        .hint("Retéléchargez l'installeur depuis le panel : celui-ci est trop ancien.")),
    }
}

/// macOS : on ne garde du bundle publié que le Mach-O.
///
/// Le `.app` visible par le joueur est fabriqué juste après, avec l'identité du
/// tenant (`macos_bundle`). Conserver en plus le `.app` générique extrait de
/// l'archive ne servirait qu'à faire apparaître un second launcher, sans nom,
/// dans le Finder.
fn flatten_macos_bundle(staging: &Path, prepared: &Path, manifest: &Manifest) -> Result<PathBuf> {
    let found = find_executable(staging, manifest)?;
    let source = staging.join(&found);
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| binary_name(manifest));

    paths::create_dir(prepared)?;
    let target = prepared.join(&name);
    fs::rename(&source, &target)
        .map_err(|error| BootstrapError::io("La mise en place du moteur", &target, &error))?;
    unpack::make_executable(&target)?;
    Ok(PathBuf::from(name))
}

/// Nom de fichier du moteur quand le bundle n'en impose aucun.
fn binary_name(manifest: &Manifest) -> String {
    if cfg!(target_os = "windows") {
        format!("{}.exe", manifest.slug)
    } else if normalize_format(&manifest.engine.format) == "appimage" {
        format!("{}.AppImage", manifest.slug)
    } else {
        manifest.slug.clone()
    }
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

/// Repère l'exécutable du moteur dans une arborescence fraîchement extraite.
///
/// Le manifeste ne nomme pas le binaire : il décrit un bundle générique, dont
/// le nom change avec les versions. On le cherche donc, avec une règle par
/// plateforme, et on rend une erreur explicite plutôt qu'un lancement au hasard.
fn find_executable(root: &Path, manifest: &Manifest) -> Result<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, 8, &mut files);

    let chosen = pick(&files, manifest).ok_or_else(|| {
        BootstrapError::new("le moteur téléchargé ne contient aucun exécutable reconnaissable.")
            .hint("Prévenez le propriétaire du serveur : la version publiée est cassée.")
    })?;

    chosen
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| BootstrapError::new("le moteur téléchargé a une arborescence inattendue."))
}

#[cfg(target_os = "windows")]
fn pick(files: &[PathBuf], manifest: &Manifest) -> Option<PathBuf> {
    // Un `.nsis.zip` ne contient qu'un exécutable ; on écarte quand même les
    // désinstalleurs, qui effaceraient l'installation au lieu de la lancer.
    let candidates: Vec<PathBuf> = files
        .iter()
        .filter(|path| has_extension(path.as_path(), "exe"))
        .filter(|path| !stem_lowercase(path.as_path()).contains("uninstall"))
        .cloned()
        .collect();
    best_named(candidates, manifest)
}

#[cfg(target_os = "macos")]
fn pick(files: &[PathBuf], manifest: &Manifest) -> Option<PathBuf> {
    // Dans un `.app`, l'exécutable est le fichier de `Contents/MacOS`.
    let candidates: Vec<PathBuf> = files
        .iter()
        .filter(|path| in_macos_dir(path.as_path()))
        .cloned()
        .collect();
    if !candidates.is_empty() {
        return best_named(candidates, manifest);
    }
    best_named(files.to_vec(), manifest)
}

#[cfg(target_os = "linux")]
fn pick(files: &[PathBuf], manifest: &Manifest) -> Option<PathBuf> {
    best_named(files.to_vec(), manifest)
}

/// Départage les candidats : d'abord un nom qui ressemble à celui du client,
/// sinon le plus gros fichier — l'exécutable d'un launcher pèse toujours plus
/// que les ressources qui l'accompagnent.
fn best_named(candidates: Vec<PathBuf>, manifest: &Manifest) -> Option<PathBuf> {
    if candidates.len() <= 1 {
        return candidates.into_iter().next();
    }

    let slug = compact(&manifest.slug);
    let display = compact(manifest.display_name());
    for path in &candidates {
        let stem = compact(&stem_lowercase(path.as_path()));
        if !stem.is_empty() && (stem == slug || stem == display) {
            return Some(path.clone());
        }
    }

    candidates
        .into_iter()
        .max_by_key(|path| fs::metadata(path).map(|meta| meta.len()).unwrap_or(0))
}

fn compact(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn stem_lowercase(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn has_extension(path: &Path, wanted: &str) -> bool {
    path.extension()
        .map(|extension| extension.eq_ignore_ascii_case(wanted))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn in_macos_dir(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    parent.file_name().map(|name| name == "MacOS").unwrap_or(false)
        && parent
            .parent()
            .and_then(Path::file_name)
            .map(|name| name == "Contents")
            .unwrap_or(false)
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

fn write_state(path: &Path, version: &str, sha256: &str, executable: &Path) -> Result<()> {
    let content = format!(
        "{version}\n{sha256}\n{}\n",
        executable.to_string_lossy().replace('\\', "/")
    );
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
        assert_eq!(normalize_format("nsis-zip"), "nsiszip");
        assert_eq!(normalize_format("app.tar.gz"), "apptargz");
        assert_eq!(normalize_format("AppImage"), "appimage");
    }

    #[test]
    fn state_needs_two_lines_at_least() {
        assert!(read_state(Path::new("/nonexistent-engine-state")).is_none());
    }
}
