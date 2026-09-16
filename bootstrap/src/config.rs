//! Constantes figées à la compilation.
//!
//! Le créneau d'identité ne porte **que** l'identifiant du serveur (voir
//! `identity`). L'adresse du panel, elle, est compilée : un seul binaire est
//! publié par (OS, arch), et le panel n'a qu'un créneau à remplir pour en tirer
//! l'installeur d'un serveur.
//!
//! Ce partage est délibéré. L'adresse du panel est la même pour tous ses
//! clients — c'est ce panel qui sert l'installeur — alors que l'identifiant du
//! serveur change à chaque client. Mettre les deux dans le créneau obligerait à
//! y réserver une URL entière sans rien apporter ; la compiler laisse le
//! créneau minuscule et le fichier identique jusqu'aux 87 octets près.

/// Origine du panel, **sans** `/api`. Le manifeste est lu sur
/// `{panel}/api/v1/launchers/{tenant}/manifest`.
///
/// `PANEL_URL` est accepté en second : c'est le nom de la variable de dépôt
/// utilisée par la CI, et un bootstrap compilé contre le mauvais panel ne se
/// verrait qu'une fois chez les joueurs.
const PANEL_BASE: &str = match option_env!("LUUXCRAFT_PANEL_URL") {
    Some(url) => url,
    None => match option_env!("PANEL_URL") {
        Some(url) => url,
        None => "https://luuxcraft.fr",
    },
};

pub const USER_AGENT: &str = concat!("LuuxCraftBootstrap/", env!("CARGO_PKG_VERSION"));

/// Tolère une valeur de build passée avec un `/` ou un `/api` final : une URL
/// doublée en `/api/api/v1` ne produirait qu'un 404 incompréhensible pour le
/// joueur.
pub fn panel_base() -> &'static str {
    let base = PANEL_BASE.trim_end_matches('/');
    match base.strip_suffix("/api") {
        Some(base) => base,
        None => base,
    }
}

/// Vocabulaire d'OS du panel (`windows` | `linux` | `macos`).
///
/// Seul `windows` a un bootstrap ; les deux autres valeurs n'existent que pour
/// que ce crate compile — et que ses tests tournent — sur les machines Linux de
/// l'intégration continue.
pub const OS: &str = if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "macos") {
    "macos"
} else {
    "linux"
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_base_never_carries_api_twice() {
        assert!(!panel_base().ends_with('/'));
        assert!(!panel_base().ends_with("/api"));
    }
}
