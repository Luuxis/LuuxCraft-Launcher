//! Ce que `build.rs` a produit : vérifié par les tests, embarqué par l'exécutable.
//!
//! Les ressources Windows (icône, manifeste, informations de version) sont
//! attachées au binaire par l'éditeur de liens et ne se voient pas d'ici. Le
//! PNG de 256 px, lui, est embarqué tel quel : c'est le logo montré par la
//! fenêtre tant que celui du serveur n'est pas arrivé.

/// Le logo générique, réduit à 256 px par le script de build.
#[cfg_attr(not(windows), allow(dead_code))]
pub const LOGO_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/logo-256.png"));

/// Les fichiers intermédiaires du build, relus pour vérifier qu'ils ont la
/// forme que Windows attend — sous Linux aussi, où rien d'autre ne les lirait.
#[cfg(test)]
mod tests {
    const ICO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/logo.ico"));
    const RES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bootstrap.res"));
    const MANIFEST: &[u8] = include_bytes!("../app.manifest");

    const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    const RT_VERSION: u16 = 16;
    const RT_MANIFEST: u16 = 24;

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    /// Une entrée du `.ico` : (taille annoncée, données).
    fn ico_entries() -> Vec<(u8, &'static [u8])> {
        assert_eq!(u16_at(ICO, 0), 0, "réservé");
        assert_eq!(u16_at(ICO, 2), 1, "type icône");
        let count = u16_at(ICO, 4) as usize;
        (0..count)
            .map(|index| {
                let entry = 6 + index * 16;
                let size = u32_at(ICO, entry + 8) as usize;
                let offset = u32_at(ICO, entry + 12) as usize;
                (ICO[entry], &ICO[offset..offset + size])
            })
            .collect()
    }

    #[test]
    fn the_icon_has_every_size_windows_looks_for() {
        let sizes: Vec<u8> = ico_entries().iter().map(|(size, _)| *size).collect();
        // 0 veut dire 256.
        assert_eq!(sizes, vec![16, 24, 32, 48, 64, 0]);
    }

    /// Les petites tailles sont des DIB (en-tête de 40 octets, hauteur
    /// doublée pour le masque), la grande un PNG.
    #[test]
    fn small_sizes_are_bitmaps_and_the_large_one_a_png() {
        for (size, data) in ico_entries() {
            if size == 0 {
                assert!(data.starts_with(&PNG_SIGNATURE));
                assert_eq!(u32_at(data, 16).swap_bytes(), 256, "largeur du PNG");
            } else {
                let side = u32::from(size);
                assert_eq!(u32_at(data, 0), 40, "biSize");
                assert_eq!(u32_at(data, 4), side, "biWidth");
                assert_eq!(u32_at(data, 8), side * 2, "biHeight, XOR + AND");
                assert_eq!(u16_at(data, 14), 32, "biBitCount");
                let mask_stride = side.div_ceil(32) * 4;
                assert_eq!(
                    data.len() as u32,
                    40 + side * side * 4 + mask_stride * side,
                    "taille du DIB"
                );
            }
        }
    }

    /// Le PNG embarqué pour la fenêtre est la même image que l'entrée 256 de
    /// l'icône.
    #[test]
    fn the_embedded_logo_is_the_largest_icon_frame() {
        let (_, largest) = ico_entries().into_iter().last().expect("une entrée");
        assert_eq!(super::LOGO_PNG, largest);
    }

    /// Une ressource du `.res` : (type, identifiant, données).
    fn resources() -> Vec<(u16, u16, &'static [u8])> {
        // Le premier enregistrement, vide, est la signature du format.
        assert_eq!(u32_at(RES, 0), 0);
        assert_eq!(u32_at(RES, 4), 32);

        let mut out = Vec::new();
        let mut at = 32;
        while at < RES.len() {
            let data_size = u32_at(RES, at) as usize;
            let header_size = u32_at(RES, at + 4) as usize;
            assert_eq!(header_size, 32, "type et nom ordinaux");
            assert_eq!(u16_at(RES, at + 8), 0xFFFF);
            assert_eq!(u16_at(RES, at + 12), 0xFFFF);
            let kind = u16_at(RES, at + 10);
            let id = u16_at(RES, at + 14);
            let data = &RES[at + header_size..at + header_size + data_size];
            out.push((kind, id, data));
            at += header_size + data_size;
            at = (at + 3) & !3;
        }
        out
    }

    #[test]
    fn the_res_carries_icons_a_group_a_version_and_the_manifest() {
        let all = resources();
        let icons: Vec<u16> = all
            .iter()
            .filter(|(kind, _, _)| *kind == RT_ICON)
            .map(|(_, id, _)| *id)
            .collect();
        assert_eq!(icons, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(
            all.iter().filter(|(kind, _, _)| *kind == RT_GROUP_ICON).count(),
            1
        );
        assert_eq!(all.iter().filter(|(kind, _, _)| *kind == RT_VERSION).count(), 1);
        assert_eq!(
            all.iter().filter(|(kind, _, _)| *kind == RT_MANIFEST).count(),
            1
        );
    }

    #[test]
    fn the_manifest_resource_is_the_manifest_file() {
        let (_, id, data) = resources()
            .into_iter()
            .find(|(kind, _, _)| *kind == RT_MANIFEST)
            .expect("manifeste");
        assert_eq!(id, 1, "CREATEPROCESS_MANIFEST_RESOURCE_ID");
        assert_eq!(data, MANIFEST);
    }

    /// Chaque entrée du groupe désigne une ressource `RT_ICON` qui existe,
    /// avec la bonne taille de données.
    #[test]
    fn the_icon_group_points_at_the_icons() {
        let all = resources();
        let (_, _, group) = all
            .iter()
            .find(|(kind, _, _)| *kind == RT_GROUP_ICON)
            .expect("groupe");
        let count = u16_at(group, 4) as usize;
        assert_eq!(count, ico_entries().len());
        for index in 0..count {
            let entry = 6 + index * 14;
            let bytes = u32_at(group, entry + 8) as usize;
            let id = u16_at(group, entry + 12);
            let (_, _, icon) = all
                .iter()
                .find(|(kind, found, _)| *kind == RT_ICON && *found == id)
                .unwrap_or_else(|| panic!("RT_ICON {id} manquante"));
            assert_eq!(icon.len(), bytes);
        }
    }

    /// Lit un bloc `VS_VERSIONINFO` : (longueur, longueur de valeur, type,
    /// clé, décalage du contenu après la clé).
    fn version_block(bytes: &[u8]) -> (usize, usize, u16, String, usize) {
        let length = u16_at(bytes, 0) as usize;
        let value_length = u16_at(bytes, 2) as usize;
        let kind = u16_at(bytes, 4);
        let mut key = Vec::new();
        let mut at = 6;
        loop {
            let unit = u16_at(bytes, at);
            at += 2;
            if unit == 0 {
                break;
            }
            key.push(unit);
        }
        at = (at + 3) & !3;
        (length, value_length, kind, String::from_utf16_lossy(&key), at)
    }

    #[test]
    fn the_version_info_is_well_formed() {
        let (_, _, data) = resources()
            .into_iter()
            .find(|(kind, _, _)| *kind == RT_VERSION)
            .expect("version");

        let (length, value_length, kind, key, value_at) = version_block(data);
        assert_eq!(length, data.len());
        assert_eq!(key, "VS_VERSION_INFO");
        assert_eq!(kind, 0);
        assert_eq!(value_length, 52, "VS_FIXEDFILEINFO");
        assert_eq!(u32_at(data, value_at), 0xFEEF_04BD, "signature");
        assert_eq!(u32_at(data, value_at + 36), 0x0000_0001, "VFT_APP");

        // Les deux enfants, dans l'ordre, chacun aligné sur 4 octets.
        let mut at = (value_at + value_length + 3) & !3;
        let (first_length, _, _, first_key, _) = version_block(&data[at..]);
        assert_eq!(first_key, "StringFileInfo");
        at = (at + first_length + 3) & !3;
        let (second_length, _, _, second_key, _) = version_block(&data[at..]);
        assert_eq!(second_key, "VarFileInfo");
        assert_eq!((at + second_length + 3) & !3, data.len());
    }

    #[test]
    fn the_version_strings_name_the_product() {
        let (_, _, data) = resources()
            .into_iter()
            .find(|(kind, _, _)| *kind == RT_VERSION)
            .expect("version");
        let (_, value_length, _, _, value_at) = version_block(data);
        let string_file_info = (value_at + value_length + 3) & !3;
        let (_, _, _, _, table_at) = version_block(&data[string_file_info..]);
        let table = &data[string_file_info + table_at..];
        let (table_length, _, _, table_key, mut at) = version_block(table);
        assert_eq!(table_key, "040904B0");

        let mut strings = Vec::new();
        while at < table_length {
            let (length, value_length, kind, key, value_at) = version_block(&table[at..]);
            assert_eq!(kind, 1, "chaîne");
            let value: Vec<u16> = (0..value_length.saturating_sub(1))
                .map(|index| u16_at(table, at + value_at + index * 2))
                .collect();
            strings.push((key, String::from_utf16_lossy(&value)));
            at = (at + length + 3) & !3;
        }

        let find = |wanted: &str| {
            strings
                .iter()
                .find(|(key, _)| key == wanted)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(find("ProductName"), Some("LuuxCraft Launcher"));
        assert_eq!(find("FileVersion"), Some(env!("CARGO_PKG_VERSION")));
        assert_eq!(find("OriginalFilename"), Some("luuxcraft-bootstrap.exe"));
    }
}
