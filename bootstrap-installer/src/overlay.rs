//! Overlay binaire : l'identité du tenant collée en fin d'exécutable.
//!
//! Le panel sert un bootstrap **générique** depuis R2 et lui ajoute 32 octets
//! au vol, pendant le streaming — aucun build par client, aucun fichier
//! temporaire côté serveur :
//!
//! ```text
//! [ LXCBOOT1 8 o ][ UUID 16 o bruts ][ 1TOOBCXL 8 o ]
//! ```
//!
//! L'UUID est celui de `users.id`, dans l'ordre canonique RFC 4122 (gros
//! boutiste), jamais sa forme textuelle. On le relit depuis
//! `current_exe()` et non depuis `argv[0]` : le joueur renomme très souvent le
//! fichier téléchargé, et un renommage ne doit rien casser.
//!
//! Aucun repli n'est prévu : sans overlay valide, ce binaire ne sait pas pour
//! quel client il installe, et le dire franchement vaut mieux qu'installer le
//! mauvais launcher.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use uuid::Uuid;

use crate::error::{BootstrapError, Result};

const MAGIC_START: &[u8; 8] = b"LXCBOOT1";
const MAGIC_END: &[u8; 8] = b"1TOOBCXL";
const OVERLAY_LEN: usize = 32;

/// Lit l'UUID du tenant dans les 32 derniers octets de l'exécutable courant.
pub fn read_tenant_id() -> Result<Uuid> {
    let exe = std::env::current_exe()
        .map_err(|error| BootstrapError::read(&error, "impossible de localiser l'installeur"))?;

    let mut file = File::open(&exe)
        .map_err(|error| BootstrapError::io("La lecture de l'installeur", &exe, &error))?;
    let size = file
        .metadata()
        .map_err(|error| BootstrapError::io("La lecture de l'installeur", &exe, &error))?
        .len();
    if size < OVERLAY_LEN as u64 {
        return Err(corrupted());
    }

    file.seek(SeekFrom::End(-(OVERLAY_LEN as i64)))
        .map_err(|error| BootstrapError::io("La lecture de l'installeur", &exe, &error))?;
    let mut tail = [0u8; OVERLAY_LEN];
    file.read_exact(&mut tail)
        .map_err(|error| BootstrapError::io("La lecture de l'installeur", &exe, &error))?;

    parse(&tail)
}

fn parse(tail: &[u8; OVERLAY_LEN]) -> Result<Uuid> {
    if &tail[..8] != MAGIC_START || &tail[24..] != MAGIC_END {
        return Err(corrupted());
    }

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&tail[8..24]);
    let tenant = Uuid::from_bytes(bytes);
    if tenant.is_nil() {
        return Err(corrupted());
    }
    Ok(tenant)
}

fn corrupted() -> BootstrapError {
    BootstrapError::new("cet installeur ne contient pas d'identité de client valide.").hint(
        "Le fichier est incomplet ou n'a pas été téléchargé depuis le panel. \
         Retéléchargez-le depuis votre espace client, sans passer par une copie \
         ni par une archive.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlay(uuid: &[u8; 16]) -> [u8; OVERLAY_LEN] {
        let mut tail = [0u8; OVERLAY_LEN];
        tail[..8].copy_from_slice(MAGIC_START);
        tail[8..24].copy_from_slice(uuid);
        tail[24..].copy_from_slice(MAGIC_END);
        tail
    }

    #[test]
    fn reads_the_uuid_in_rfc_4122_order() {
        let bytes = [
            0x3f, 0x2a, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
            0xdd, 0xee,
        ];
        let tenant = parse(&overlay(&bytes)).expect("overlay valide");
        assert_eq!(
            tenant.to_string(),
            "3f2a1122-3344-5566-7788-99aabbccddee"
        );
    }

    #[test]
    fn rejects_a_broken_magic() {
        let mut tail = overlay(&[1u8; 16]);
        tail[0] = b'X';
        assert!(parse(&tail).is_err());
    }

    #[test]
    fn rejects_the_nil_uuid() {
        assert!(parse(&overlay(&[0u8; 16])).is_err());
    }
}
