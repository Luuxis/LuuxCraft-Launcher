//! Le logo du serveur, pour la fenêtre — avant d'installer quoi que ce soit.
//!
//! Le pack client contient `icon.png` quand le serveur a téléversé un logo en
//! PNG (voir `pack`). Le manifeste en donne l'URL et l'empreinte : le fichier
//! est téléchargé **en mémoire** et vérifié, sans rien poser sur le disque
//! tant que le joueur n'a pas cliqué sur « Installer ». Il sera retéléchargé
//! avec le pack — quelques kilo-octets, pas de quoi compliquer le différentiel.
//!
//! Un échec n'est jamais fatal : la fenêtre garde le logo générique.

use sha2::{Digest, Sha256};

use crate::hashing;
use crate::manifest::Manifest;
use crate::net::Http;

/// Au-delà, ce n'est pas un logo.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

pub fn fetch(http: &Http, manifest: &Manifest) -> Option<Vec<u8>> {
    let file = manifest
        .client_pack
        .files
        .iter()
        .find(|file| file.path == "icon.png")?;
    if file.size > MAX_BYTES {
        return None;
    }

    let bytes = http.get_bytes(&file.url, MAX_BYTES).ok()?;
    if !bytes.starts_with(&PNG_SIGNATURE) {
        return None;
    }

    // La même empreinte que le pack : un logo qui n'est pas celui que le
    // manifeste annonce n'est pas affiché.
    let digest = hashing::to_hex(&Sha256::digest(&bytes)[..]);
    hashing::matches(&digest, &file.sha256).then_some(bytes)
}
