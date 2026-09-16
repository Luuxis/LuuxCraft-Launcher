//! Entrée de bureau Linux.
//!
//! Il n'y a pas d'installeur sous Linux : l'AppImage n'est qu'un fichier
//! exécutable. L'entrée `~/.local/share/applications/<slug>.desktop` est donc
//! la seule chose qui fasse apparaître le client dans le menu d'applications,
//! sous son nom et avec son logo.
//!
//! Le fichier est nommé d'après le slug, pas d'après le nom affiché : c'est ce
//! qui permet de le réécrire quand le client change de nom, au lieu d'en
//! accumuler un par renommage.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::manifest::Manifest;

pub fn write(manifest: &Manifest, executable: &Path, icon: Option<&Path>) -> io::Result<PathBuf> {
    let directory = dirs::data_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "aucun dossier de données"))?
        .join("applications");
    fs::create_dir_all(&directory)?;

    let path = directory.join(format!("{}.desktop", manifest.slug));
    fs::write(&path, entry(manifest, executable, icon))?;
    Ok(path)
}

fn entry(manifest: &Manifest, executable: &Path, icon: Option<&Path>) -> String {
    let icon_line = match icon {
        Some(icon) => format!("Icon={}\n", icon.to_string_lossy()),
        None => String::new(),
    };

    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={name}\n\
         Comment=Launcher Minecraft {name}\n\
         Exec={exec}\n\
         {icon_line}\
         Terminal=false\n\
         Categories=Game;\n\
         StartupNotify=true\n\
         StartupWMClass={slug}\n",
        name = single_line(manifest.display_name()),
        exec = exec_value(executable),
        slug = manifest.slug,
    )
}

/// `Exec` est découpé comme une ligne de commande : le chemin est mis entre
/// guillemets (le dossier personnel peut contenir des espaces) et les `%` y
/// sont doublés, sans quoi ils seraient lus comme des codes de substitution.
fn exec_value(executable: &Path) -> String {
    let escaped = executable
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

/// Une valeur de clé tient sur une ligne : un retour chariot casserait le
/// fichier entier.
fn single_line(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_is_quoted_and_percent_escaped() {
        assert_eq!(
            exec_value(Path::new("/home/a b/100% fun/engine")),
            "\"/home/a b/100%% fun/engine\""
        );
    }

    #[test]
    fn names_never_break_the_file() {
        assert_eq!(single_line(" Mon\nServeur "), "Mon Serveur");
    }
}
