//! Écriture d'un raccourci `.lnk` Windows, en Rust pur.
//!
//! Aucun appel à PowerShell ni à COM : un installeur qui lance
//! `powershell.exe` est bloqué par la moitié des antivirus grand public, et
//! demander `IShellLinkW` ferait entrer tout le bindings Windows dans un
//! binaire qui doit rester minuscule. Le format `.lnk` (MS-SHLLINK) est
//! entièrement écrit ici.
//!
//! Le raccourci est décrit par son `LinkInfo` : chemin local du binaire, en
//! ANSI **et** en UTF-16 — le dossier d'installation contient le nom de la
//! session Windows, qui n'est pas toujours représentable en ANSI.
//!
//! Un échec n'est jamais fatal (voir `shortcuts`) : sans raccourci, le client
//! est installé et lancé quand même.

use std::fs;
use std::io;
use std::path::Path;

/// `HeaderSize` fixe de la spécification.
const HEADER_SIZE: u32 = 0x4C;
/// CLSID `{00021401-0000-0000-C000-000000000046}`, sérialisé en petit boutiste.
const LINK_CLSID: [u8; 16] = [
    0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

const HAS_LINK_INFO: u32 = 0x0000_0002;
const HAS_NAME: u32 = 0x0000_0004;
const HAS_WORKING_DIR: u32 = 0x0000_0010;
const HAS_ICON_LOCATION: u32 = 0x0000_0040;
const IS_UNICODE: u32 = 0x0000_0080;

const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
const SW_SHOWNORMAL: u32 = 1;

/// `LinkInfoHeaderSize` avec les deux champs de décalage Unicode.
const LINK_INFO_HEADER_SIZE: u32 = 0x24;
const VOLUME_ID_AND_LOCAL_BASE_PATH: u32 = 0x0000_0001;
/// `VolumeID` : quatre entiers puis une étiquette de volume vide.
const VOLUME_ID_SIZE: u32 = 17;
const DRIVE_FIXED: u32 = 3;

/// Longueur maximale des champs `StringData`, imposée par la spécification.
const MAX_STRING_UNITS: usize = 259;

pub fn write(
    lnk: &Path,
    target: &Path,
    working_dir: &Path,
    icon: Option<&Path>,
    name: &str,
) -> io::Result<()> {
    if let Some(parent) = lnk.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(lnk, build(target, working_dir, icon, name))
}

fn build(target: &Path, working_dir: &Path, icon: Option<&Path>, name: &str) -> Vec<u8> {
    let mut flags = HAS_LINK_INFO | HAS_NAME | HAS_WORKING_DIR | IS_UNICODE;
    if icon.is_some() {
        flags |= HAS_ICON_LOCATION;
    }
    let target_size = fs::metadata(target)
        .map(|meta| meta.len().min(u32::MAX as u64) as u32)
        .unwrap_or(0);

    let mut out = Vec::with_capacity(512);
    out.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    out.extend_from_slice(&LINK_CLSID);
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&FILE_ATTRIBUTE_NORMAL.to_le_bytes());
    // CreationTime, AccessTime, WriteTime : zéro signifie « inconnu ».
    out.extend_from_slice(&[0u8; 24]);
    out.extend_from_slice(&target_size.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes()); // IconIndex
    out.extend_from_slice(&SW_SHOWNORMAL.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // HotKey
    out.extend_from_slice(&0u16.to_le_bytes()); // Reserved1
    out.extend_from_slice(&0u32.to_le_bytes()); // Reserved2
    out.extend_from_slice(&0u32.to_le_bytes()); // Reserved3
    debug_assert_eq!(out.len(), HEADER_SIZE as usize);

    out.extend_from_slice(&link_info(&target.to_string_lossy()));

    // L'ordre des `StringData` est imposé : NAME, RELATIVE_PATH, WORKING_DIR,
    // ARGUMENTS, ICON_LOCATION. Les blocs absents sont simplement omis.
    push_string(&mut out, name);
    push_string(&mut out, &working_dir.to_string_lossy());
    if let Some(icon) = icon {
        push_string(&mut out, &icon.to_string_lossy());
    }

    out.extend_from_slice(&0u32.to_le_bytes()); // TerminalBlock
    out
}

fn link_info(target: &str) -> Vec<u8> {
    let ansi = ansi_bytes(target);
    let wide = utf16_bytes(target);

    let volume_id_offset = LINK_INFO_HEADER_SIZE;
    let local_base_path_offset = volume_id_offset + VOLUME_ID_SIZE;
    let common_path_suffix_offset = local_base_path_offset + ansi.len() as u32 + 1;
    let local_base_path_offset_unicode = common_path_suffix_offset + 1;
    let common_path_suffix_offset_unicode =
        local_base_path_offset_unicode + wide.len() as u32 + 2;
    let total = common_path_suffix_offset_unicode + 2;

    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&LINK_INFO_HEADER_SIZE.to_le_bytes());
    out.extend_from_slice(&VOLUME_ID_AND_LOCAL_BASE_PATH.to_le_bytes());
    out.extend_from_slice(&volume_id_offset.to_le_bytes());
    out.extend_from_slice(&local_base_path_offset.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // CommonNetworkRelativeLinkOffset
    out.extend_from_slice(&common_path_suffix_offset.to_le_bytes());
    out.extend_from_slice(&local_base_path_offset_unicode.to_le_bytes());
    out.extend_from_slice(&common_path_suffix_offset_unicode.to_le_bytes());

    // VolumeID
    out.extend_from_slice(&VOLUME_ID_SIZE.to_le_bytes());
    out.extend_from_slice(&DRIVE_FIXED.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DriveSerialNumber
    out.extend_from_slice(&0x10u32.to_le_bytes()); // VolumeLabelOffset
    out.push(0); // étiquette de volume vide

    out.extend_from_slice(&ansi);
    out.push(0);
    out.push(0); // CommonPathSuffix vide
    out.extend_from_slice(&wide);
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&[0, 0]); // CommonPathSuffixUnicode vide

    debug_assert_eq!(out.len(), total as usize);
    out
}

/// Chaîne `StringData` : nombre d'unités UTF-16 puis les unités, sans
/// terminateur (le drapeau `IsUnicode` est posé).
fn push_string(out: &mut Vec<u8>, value: &str) {
    let mut units: Vec<u16> = value.encode_utf16().take(MAX_STRING_UNITS).collect();
    // Une troncature ne doit pas laisser une moitié de paire de substitution.
    if units
        .last()
        .map(|unit| (0xD800..=0xDBFF).contains(unit))
        .unwrap_or(false)
    {
        units.pop();
    }
    out.extend_from_slice(&(units.len() as u16).to_le_bytes());
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
}

/// Repli ANSI du chemin. Windows moderne lit le champ Unicode ; celui-ci n'est
/// là que parce que la structure l'exige.
fn ansi_bytes(value: &str) -> Vec<u8> {
    value
        .chars()
        .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
        .collect()
}

fn utf16_bytes(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_info_offsets_match_its_own_length() {
        let info = link_info("C:\\Users\\joueur\\AppData\\Local\\Programs\\mon-serveur\\engine\\a.exe");
        let total = u32::from_le_bytes(info[0..4].try_into().unwrap());
        assert_eq!(total as usize, info.len());
        assert_eq!(u32::from_le_bytes(info[4..8].try_into().unwrap()), 0x24);
    }

    #[test]
    fn header_is_exactly_76_bytes() {
        let bytes = build(
            Path::new("C:\\a.exe"),
            Path::new("C:\\"),
            None,
            "Mon Serveur",
        );
        assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 0x4C);
        assert_eq!(&bytes[4..20], &LINK_CLSID);
        assert!(bytes.ends_with(&[0, 0, 0, 0]));
    }
}
