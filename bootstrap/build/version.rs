//! Ressource `VS_VERSIONINFO` : l'onglet « Détails » des propriétés du fichier.
//!
//! Un arbre de blocs de même forme, chacun aligné sur 4 octets :
//!
//! ```text
//! wLength       u16   taille du bloc, enfants compris
//! wValueLength  u16   taille de la valeur (en octets, ou en mots si texte)
//! wType         u16   0 = binaire, 1 = texte
//! szKey         UTF-16, terminé par zéro
//! [remplissage] jusqu'à 4 octets
//! Value
//! [remplissage]
//! Children      d'autres blocs, chacun aligné sur 4 octets
//! ```
//!
//! ```text
//! VS_VERSION_INFO            valeur : VS_FIXEDFILEINFO (52 octets)
//! ├── StringFileInfo
//! │   └── 040904B0           anglais, Unicode
//! │       ├── CompanyName    …
//! │       └── ProductName    …
//! └── VarFileInfo
//!     └── Translation        0x0409, 0x04B0
//! ```

pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    pub fn from_cargo() -> Self {
        let part = |name: &str| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(0)
        };
        Self {
            major: part("CARGO_PKG_VERSION_MAJOR"),
            minor: part("CARGO_PKG_VERSION_MINOR"),
            patch: part("CARGO_PKG_VERSION_PATCH"),
        }
    }
}

const BINARY: u16 = 0;
const TEXT: u16 = 1;

/// Langue et jeu de caractères de la table de chaînes : anglais (0x0409),
/// Unicode (0x04B0 = 1200). La clé de la table est la même paire en hexadécimal.
const TRANSLATION: [u8; 4] = [0x09, 0x04, 0xB0, 0x04];
const STRING_TABLE_KEY: &str = "040904B0";

pub fn info(version: Version, strings: &[(&str, &str)]) -> Vec<u8> {
    let table: Vec<Vec<u8>> = strings
        .iter()
        .map(|(key, value)| string(key, value))
        .collect();
    let string_table = block(STRING_TABLE_KEY, TEXT, &[], 0, &table);
    let string_file_info = block("StringFileInfo", TEXT, &[], 0, &[string_table]);

    let translation = block(
        "Translation",
        BINARY,
        &TRANSLATION,
        TRANSLATION.len() as u16,
        &[],
    );
    let var_file_info = block("VarFileInfo", TEXT, &[], 0, &[translation]);

    let fixed = fixed_file_info(&version);
    block(
        "VS_VERSION_INFO",
        BINARY,
        &fixed,
        fixed.len() as u16,
        &[string_file_info, var_file_info],
    )
}

fn fixed_file_info(version: &Version) -> Vec<u8> {
    let most = (u32::from(version.major) << 16) | u32::from(version.minor);
    let least = u32::from(version.patch) << 16;
    let fields: [u32; 13] = [
        0xFEEF_04BD,  // dwSignature
        0x0001_0000,  // dwStrucVersion
        most,         // dwFileVersionMS
        least,        // dwFileVersionLS
        most,         // dwProductVersionMS
        least,        // dwProductVersionLS
        0x0000_003F,  // dwFileFlagsMask
        0,            // dwFileFlags
        0x0004_0004,  // dwFileOS : VOS_NT_WINDOWS32
        0x0000_0001,  // dwFileType : VFT_APP
        0,            // dwFileSubtype
        0,            // dwFileDateMS
        0,            // dwFileDateLS
    ];
    fields.iter().flat_map(|field| field.to_le_bytes()).collect()
}

/// Un bloc texte : la valeur est la chaîne, terminée par zéro, et
/// `wValueLength` se compte en mots UTF-16, terminateur compris.
fn string(key: &str, value: &str) -> Vec<u8> {
    let wide = utf16z(value);
    block(key, TEXT, &wide, (wide.len() / 2) as u16, &[])
}

/// Un bloc et ses enfants. Le remplissage entre deux enfants appartient au
/// parent ; la longueur d'un bloc ne compte jamais ce qui le suit.
fn block(key: &str, kind: u16, value: &[u8], value_length: u16, children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0]); // wLength, écrit à la fin
    out.extend_from_slice(&value_length.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&utf16z(key));
    pad(&mut out);
    out.extend_from_slice(value);
    for child in children {
        pad(&mut out);
        out.extend_from_slice(child);
    }
    let length = out.len() as u16;
    out[..2].copy_from_slice(&length.to_le_bytes());
    out
}

fn utf16z(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}
