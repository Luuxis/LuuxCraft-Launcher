//! Constantes figées à la compilation.
//!
//! L'overlay ajouté au binaire ne porte **que** l'UUID du tenant (32 octets,
//! voir `overlay`). Tout le reste — l'adresse du panel, le préfixe de bundle
//! macOS — est donc compilé dans le bootstrap : un seul binaire par (OS, arch)
//! est publié, et le panel se contente de lui coller 32 octets à la volée.

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

/// Préfixe du `CFBundleIdentifier` macOS, complété par le slug du tenant.
/// C'est ce qui permet à deux clients d'exister côte à côte dans le Dock.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const BUNDLE_ID_PREFIX: &str = match option_env!("LUUXCRAFT_BUNDLE_ID_PREFIX") {
    Some(prefix) => prefix,
    None => "com.luuxcraft.launcher",
};

pub const USER_AGENT: &str = concat!("LuuxCraftBootstrap/", env!("CARGO_PKG_VERSION"));

/// Tolère une valeur de build passée avec un `/` ou un `/api` final : l'ancien
/// pipeline injectait `https://panel/api`, et une URL doublée en `/api/api/v1`
/// ne produirait qu'un 404 incompréhensible pour le joueur.
pub fn panel_base() -> &'static str {
    let base = PANEL_BASE.trim_end_matches('/');
    match base.strip_suffix("/api") {
        Some(base) => base,
        None => base,
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn bundle_id_prefix() -> &'static str {
    BUNDLE_ID_PREFIX.trim_end_matches('.')
}

/// Vocabulaire d'OS du contrat (`windows` | `linux` | `macos`), qui n'est pas
/// celui de `std::env::consts::OS` (`macos` y est `macos`, mais `windows` y est
/// `windows` et Linux `linux` — on fige la table pour ne pas en dépendre).
pub const OS: &str = if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "macos") {
    "macos"
} else {
    "linux"
};
