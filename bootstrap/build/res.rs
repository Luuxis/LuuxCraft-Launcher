//! Fichier `.res` : le format de ressources compilées de Windows.
//!
//! C'est ce que `rc.exe` produit à partir d'un `.rc`, et ce que `link.exe`
//! (comme `lld-link`) sait attacher à un exécutable sans autre outil. Le
//! format est trivial — une suite d'en-têtes de taille fixe suivis de leurs
//! données — et l'écrire ici évite de dépendre d'un compilateur de ressources
//! dont l'emplacement change d'un kit Windows à l'autre.
//!
//! ```text
//! [ en-tête vide ][ en-tête + données ][ en-tête + données ] …
//!      32 o          alignées sur 4 o
//! ```
//!
//! Chaque en-tête (`RESOURCEHEADER`) :
//!
//! ```text
//! DataSize        u32   taille des données qui suivent
//! HeaderSize      u32   32 quand type et nom sont des ordinaux
//! Type            u16 0xFFFF, u16 identifiant   (RT_ICON = 3, …)
//! Name            u16 0xFFFF, u16 identifiant
//! DataVersion     u32   0
//! MemoryFlags     u16   indicatifs, ignorés par les éditeurs de liens modernes
//! LanguageId      u16
//! Version         u32   0
//! Characteristics u32   0
//! ```
//!
//! Le premier enregistrement, vide, est la signature du format : c'est à lui
//! que l'éditeur de liens reconnaît un `.res`.

pub const RT_ICON: u16 = 3;
pub const RT_GROUP_ICON: u16 = 14;
pub const RT_VERSION: u16 = 16;
pub const RT_MANIFEST: u16 = 24;

/// Identifiant de la première ressource `RT_ICON` ; les suivantes se suivent.
pub const FIRST_ICON_ID: u16 = 1;

/// Anglais (États-Unis), la langue par défaut de `rc.exe`. Windows retombe de
/// toute façon sur la seule langue disponible quand il cherche une ressource.
const LANGUAGE: u16 = 0x0409;

const HEADER_SIZE: u32 = 32;

pub struct Entry<'a> {
    pub kind: u16,
    pub id: u16,
    pub data: &'a [u8],
}

pub fn file(entries: &[Entry<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    header(&mut out, 0, 0, 0, 0, 0);
    for entry in entries {
        header(
            &mut out,
            entry.data.len() as u32,
            entry.kind,
            entry.id,
            memory_flags(entry.kind),
            LANGUAGE,
        );
        out.extend_from_slice(entry.data);
        pad(&mut out);
    }
    out
}

fn header(out: &mut Vec<u8>, data_size: u32, kind: u16, id: u16, memory_flags: u16, language: u16) {
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&memory_flags.to_le_bytes());
    out.extend_from_slice(&language.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
}

/// Les mêmes indicateurs que `rc.exe` : `MOVEABLE` (0x10), `PURE` (0x20),
/// `DISCARDABLE` (0x1000). Ils datent de Windows 16 bits et n'ont plus
/// d'effet, mais les reproduire rend le fichier identique à ce qu'un outil
/// Microsoft aurait écrit.
fn memory_flags(kind: u16) -> u16 {
    match kind {
        RT_ICON => 0x1010,
        RT_GROUP_ICON => 0x1030,
        _ => 0x0030,
    }
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}
