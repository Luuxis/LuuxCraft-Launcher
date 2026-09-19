//! Ressources Windows de l'exécutable, produites à la compilation.
//!
//! Trois choses que Windows lit dans un `.exe` sans jamais le lancer :
//!
//! - **l'icône**, tirée de `logo/logo.png` — celle que l'Explorateur, le
//!   navigateur et la barre des tâches montrent au joueur ;
//! - **le manifeste** (`app.manifest`) — contrôles communs v6 pour que la
//!   fenêtre ait l'apparence de Windows et non celle de 1995, exécution sans
//!   élévation, prise en charge des écrans haute densité ;
//! - **les informations de version** — ce que l'onglet « Détails » des
//!   propriétés du fichier affiche.
//!
//! Tout est écrit ici, en Rust, au format `.res` que l'éditeur de liens de
//! Microsoft accepte directement. Aucun `rc.exe` à trouver, aucun kit Windows
//! à installer : le build est le même sur la machine de la CI que sur un poste
//! Linux qui vérifie la compilation croisée. Voir `build/res.rs`.
//!
//! Le PNG est aussi réduit en 256 px pour servir de logo de repli dans la
//! fenêtre, tant que celui du serveur n'est pas arrivé (`src/resources.rs`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "build/ico.rs"]
mod ico;
#[path = "build/res.rs"]
mod res;
#[path = "build/version.rs"]
mod version;

fn main() {
    // L'adresse du panel est figée à la compilation (voir `src/config.rs`) ;
    // sans ces lignes, cargo ne recompilerait pas quand un build la change.
    println!("cargo:rerun-if-env-changed=LUUXCRAFT_PANEL_URL");
    println!("cargo:rerun-if-env-changed=PANEL_URL");
    println!("cargo:rerun-if-env-changed=WINDRES");
    for file in [
        "logo/logo.png",
        "app.manifest",
        "build/ico.rs",
        "build/res.rs",
        "build/version.rs",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo définit OUT_DIR"));

    let logo = fs::read("logo/logo.png").expect("logo/logo.png est introuvable");
    let frames = ico::frames(&logo).unwrap_or_else(|error| panic!("logo/logo.png : {error}"));
    write(&out.join("logo.ico"), &ico::file(&frames));

    // La plus grande taille est un PNG complet : c'est le logo de repli de la
    // fenêtre, embarqué tel quel par `include_bytes!`.
    let largest = frames
        .iter()
        .max_by_key(|frame| frame.size)
        .expect("au moins une taille d'icône");
    assert!(largest.is_png(), "la plus grande taille doit être un PNG");
    write(&out.join("logo-256.png"), &largest.data);

    let manifest = fs::read("app.manifest").expect("app.manifest est introuvable");
    let version = version::info(
        version::Version::from_cargo(),
        &[
            ("CompanyName", "LuuxCraft"),
            ("FileDescription", "Installation du launcher"),
            ("FileVersion", env!("CARGO_PKG_VERSION")),
            ("InternalName", "luuxcraft-bootstrap"),
            ("OriginalFilename", "luuxcraft-bootstrap.exe"),
            ("ProductName", "LuuxCraft Launcher"),
            ("ProductVersion", env!("CARGO_PKG_VERSION")),
        ],
    );

    let group = ico::group(&frames, res::FIRST_ICON_ID);
    let mut entries: Vec<res::Entry<'_>> = frames
        .iter()
        .enumerate()
        .map(|(index, frame)| res::Entry {
            kind: res::RT_ICON,
            id: res::FIRST_ICON_ID + index as u16,
            data: &frame.data,
        })
        .collect();
    entries.push(res::Entry {
        kind: res::RT_GROUP_ICON,
        id: 1,
        data: &group,
    });
    entries.push(res::Entry {
        kind: res::RT_VERSION,
        id: 1,
        data: &version,
    });
    entries.push(res::Entry {
        kind: res::RT_MANIFEST,
        id: 1,
        data: &manifest,
    });

    let res_path = out.join("bootstrap.res");
    write(&res_path, &res::file(&entries));

    // Produit partout — les tests vérifient ces fichiers sous Linux aussi —
    // mais lié seulement dans un exécutable Windows.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        link(&res_path, &out);
    }
}

fn write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap_or_else(|error| panic!("écriture de {} : {error}", path.display()));
}

/// Attache les ressources à l'exécutable.
///
/// `link.exe` (et `lld-link`) prennent un `.res` tel quel sur leur ligne de
/// commande. L'éditeur de liens GNU, lui, ne le connaît pas : `windres` le
/// convertit en objet COFF quand il est là, sinon le `.res` est passé quand
/// même — `ld.lld` en mode MinGW l'accepte, GNU `ld` le refusera clairement.
fn link(res: &Path, out: &Path) {
    let env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if env == "msvc" {
        println!("cargo:rustc-link-arg-bins={}", res.display());
        return;
    }

    let object = out.join("bootstrap-res.o");
    match windres(res, &object) {
        Ok(()) => println!("cargo:rustc-link-arg-bins={}", object.display()),
        Err(reason) => {
            println!("cargo:warning=ressources passées en .res à l'éditeur de liens ({reason})");
            println!("cargo:rustc-link-arg-bins={}", res.display());
        }
    }
}

fn windres(res: &Path, object: &Path) -> Result<(), String> {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let prefixed = match arch.as_str() {
        "x86" => "i686-w64-mingw32-windres".to_owned(),
        other => format!("{other}-w64-mingw32-windres"),
    };
    let mut candidates = Vec::new();
    if let Ok(explicit) = env::var("WINDRES") {
        candidates.push(explicit);
    }
    candidates.push(prefixed);
    candidates.push("windres".to_owned());

    let mut last = String::from("aucun windres trouvé");
    for candidate in candidates {
        let outcome = Command::new(&candidate)
            .arg("-J")
            .arg("res")
            .arg("-O")
            .arg("coff")
            .arg("-i")
            .arg(res)
            .arg("-o")
            .arg(object)
            .output();
        match outcome {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(output) => {
                last = format!(
                    "{candidate} : {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            Err(error) => last = format!("{candidate} : {error}"),
        }
    }
    Err(last)
}
