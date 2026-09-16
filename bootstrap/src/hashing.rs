//! SHA-256 : la seule autorité sur ce qui est déjà installé.
//!
//! Tout le différentiel de la refonte tient là-dessus. Un composant dont
//! l'empreinte correspond n'est jamais retéléchargé, et rien n'est mis en place
//! avant que son empreinte ait été confirmée.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{BootstrapError, Result};

pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).unwrap_or('0'));
    }
    out
}

/// Comparaison insensible à la casse : le panel peut sérialiser l'empreinte en
/// majuscules sans que le joueur retélécharge tout.
pub fn matches(left: &str, right: &str) -> bool {
    left.len() == right.len() && left.eq_ignore_ascii_case(right)
}

/// Empreinte d'un fichier déjà posé sur le disque, ou `None` s'il est absent.
pub fn sha256_file(path: &Path) -> Result<Option<String>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(BootstrapError::io("La lecture", path, &error)),
    };

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| BootstrapError::io("La lecture", path, &error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Some(to_hex(&hasher.finalize()[..])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }

    #[test]
    fn hashes_compare_without_case() {
        assert!(matches("ABCD", "abcd"));
        assert!(!matches("abcd", "abce"));
        assert!(!matches("abcd", "abcd0"));
    }
}
