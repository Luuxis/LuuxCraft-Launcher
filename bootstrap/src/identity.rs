//! Créneau d'identité : à quel serveur cet installeur appartient-il ?
//!
//! Le bootstrap est compilé **une fois par (OS, architecture)** et publié
//! vierge par la CI. Il réserve dans ses données une zone de taille fixe,
//! repérée par une chaîne magique :
//!
//! ```text
//! [ LXCBOOTSTRAP-TENANT-V1 ][ longueur 1 o ][ valeur 64 o ]
//! ```
//!
//! Le panel en fait une copie par serveur, écrit l'identifiant dans le créneau,
//! et range le résultat dans R2 (`lib/bootstrapSlot.ts` côté panel). C'est ce
//! fichier-là que le joueur télécharge.
//!
//! ## Pourquoi pas la fin du fichier, ni le nom du fichier
//!
//! **Pas le nom du fichier** : le joueur renomme très souvent ce qu'il
//! télécharge. `Mon-Serveur.exe` devenu `setup.exe` doit continuer à installer
//! le bon serveur, et c'est ici la seule garantie qui tienne.
//!
//! **Pas la fin du fichier** non plus, ce que faisait le modèle précédent :
//!
//! - une signature Authenticode porte sur tout le fichier sauf sa table de
//!   certificats, donc ajouter des octets **après** signature la casse ;
//! - et une fois signé, un exécutable ne finit plus par ses propres données
//!   mais par cette table : « lire les 32 derniers octets » devient faux.
//!
//! Un créneau au milieu des données ne souffre d'aucun des deux. Il est rempli
//! **avant** la signature — l'ordre imposé est copie, injection, signature — et
//! il est lu ici en mémoire, sans jamais ouvrir le fichier de l'installeur.
//!
//! ## Pourquoi `read_volatile`
//!
//! Le compilateur connaît la valeur initiale du tableau et pourrait remplacer
//! sa lecture par cette constante, ce qui rendrait l'injection invisible. Une
//! lecture volatile n'est jamais remplacée : elle force un accès mémoire réel,
//! donc la valeur telle qu'elle est dans le fichier exécuté.

use crate::error::{BootstrapError, Result};

/// Longueur du repère. Écrite à part pour que le type de `MAGIC` ci-dessous la
/// vérifie à la compilation : changer la chaîne sans changer ce nombre ne
/// compile pas.
const MAGIC_LEN: usize = 22;
/// Repère du créneau. Doit rester identique à `SLOT_MAGIC` côté panel.
const MAGIC: &[u8; MAGIC_LEN] = b"LXCBOOTSTRAP-TENANT-V1";
/// Taille de la valeur, au-delà de l'UUID pour laisser place à un jeton opaque.
const VALUE_LEN: usize = 64;
const SLOT_LEN: usize = MAGIC_LEN + 1 + VALUE_LEN;
/// Octet de remplissage du créneau vierge.
///
/// Non nul à dessein : un tableau entièrement à zéro serait rangé par
/// l'éditeur de liens dans la zone non initialisée, qui **n'existe pas dans le
/// fichier**. Il n'y aurait alors rien à trouver, et rien à remplir.
const FILLER: u8 = b'.';

/// Le créneau lui-même.
///
/// `#[used]` et `#[no_mangle]` empêchent l'éditeur de liens de l'écarter : rien
/// dans le programme n'écrit dedans, et une optimisation de section le
/// supprimerait sans eux.
#[used]
#[no_mangle]
pub static LUUXCRAFT_TENANT_SLOT: [u8; SLOT_LEN] = blank_slot();

const fn blank_slot() -> [u8; SLOT_LEN] {
    let mut slot = [FILLER; SLOT_LEN];
    let mut index = 0;
    while index < MAGIC_LEN {
        slot[index] = MAGIC[index];
        index += 1;
    }
    // Longueur nulle : le créneau vierge ne porte aucune identité.
    slot[MAGIC_LEN] = 0;
    slot
}

/// Identifiant du serveur, lu dans le créneau de ce binaire.
pub fn tenant_id() -> Result<String> {
    // SÛRETÉ : lecture d'un `static` immuable de taille connue. La lecture est
    // volatile pour que le compilateur ne la remplace pas par la constante
    // qu'il connaît (voir l'en-tête de module).
    let slot = unsafe { std::ptr::read_volatile(&LUUXCRAFT_TENANT_SLOT) };
    parse(&slot)
}

fn parse(slot: &[u8; SLOT_LEN]) -> Result<String> {
    if &slot[..MAGIC_LEN] != MAGIC {
        return Err(corrupted());
    }

    let length = slot[MAGIC_LEN] as usize;
    if length == 0 || length > VALUE_LEN {
        return Err(corrupted());
    }

    let start = MAGIC_LEN + 1;
    let value = std::str::from_utf8(&slot[start..start + length]).map_err(|_| corrupted())?;

    // L'identifiant voyage dans un chemin d'URL : le borner à l'alphabet des
    // identifiants du panel évite qu'un créneau bricolé n'aille interroger
    // autre chose que la configuration de son serveur.
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(corrupted());
    }

    Ok(value.to_owned())
}

fn corrupted() -> BootstrapError {
    BootstrapError::new("cet installeur ne contient pas d'identité de serveur valide.").hint(
        "Le fichier est incomplet, ou n'a pas été téléchargé depuis le panel. \
         Retéléchargez-le depuis l'espace de votre serveur, sans passer par une \
         copie ni par une archive.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Écrit une identité dans un créneau vierge, exactement comme le panel.
    fn injected(value: &str) -> [u8; SLOT_LEN] {
        let mut slot = blank_slot();
        let bytes = value.as_bytes();
        slot[MAGIC_LEN] = bytes.len() as u8;
        for (index, byte) in slot
            .iter_mut()
            .skip(MAGIC_LEN + 1)
            .take(VALUE_LEN)
            .enumerate()
        {
            *byte = bytes.get(index).copied().unwrap_or(0);
        }
        slot
    }

    #[test]
    fn reads_the_identity_the_panel_wrote() {
        let tenant = "11111111-2222-4333-8444-555555555555";
        assert_eq!(parse(&injected(tenant)).expect("valid slot"), tenant);
    }

    /// Un binaire publié par la CI n'a pas encore d'identité : le lancer doit
    /// dire pourquoi, pas installer au hasard.
    #[test]
    fn a_blank_slot_is_refused() {
        assert!(parse(&blank_slot()).is_err());
    }

    #[test]
    fn a_broken_magic_is_refused() {
        let mut slot = injected("abc");
        slot[0] = b'X';
        assert!(parse(&slot).is_err());
    }

    /// L'identifiant finit dans un chemin d'URL : rien qui puisse en sortir.
    #[test]
    fn an_identity_that_would_escape_the_url_is_refused() {
        for value in ["../admin", "a/b", "a b", "a?b", ""] {
            assert!(parse(&injected(value)).is_err(), "{value:?} must be refused");
        }
    }

    #[test]
    fn a_length_beyond_the_slot_is_refused() {
        let mut slot = injected("abc");
        slot[MAGIC_LEN] = (VALUE_LEN + 1) as u8;
        assert!(parse(&slot).is_err());
    }

    /// Le créneau compilé dans ce binaire doit être celui que le panel sait
    /// trouver : même repère, même taille, même remplissage non nul.
    #[test]
    fn the_compiled_slot_matches_the_contract() {
        let slot = unsafe { std::ptr::read_volatile(&LUUXCRAFT_TENANT_SLOT) };
        assert_eq!(&slot[..MAGIC_LEN], MAGIC);
        assert_eq!(slot.len(), 22 + 1 + 64);
        assert!(slot[MAGIC_LEN + 1..].iter().all(|byte| *byte == FILLER));
    }
}
