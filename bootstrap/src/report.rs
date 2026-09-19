//! Compte rendu de l'installation, quel que soit l'écran qui le montre.
//!
//! Le déroulé (`install`) ne sait pas s'il parle à une fenêtre ou à une
//! console : il annonce ses étapes, ses détails et l'avancement de ses
//! téléchargements à un `Report`, et c'est l'écran qui décide comment les
//! montrer. Sous Windows, c'est la fenêtre (`win::gui`), qui reçoit ces appels
//! depuis un fil de travail ; ailleurs, la console (`console`), qui n'existe
//! que pour faire tourner le crate sur une machine de développement.

/// Nombre d'étapes annoncées, du manifeste au lancement.
pub const STEPS: u8 = 5;

pub trait Report {
    /// Une nouvelle étape commence (« Moteur », « Raccourcis »…).
    fn step(&self, index: u8, label: &str);

    /// Une ligne de détail sous l'étape en cours.
    fn detail(&self, text: &str);

    /// Avancement d'un téléchargement ; `total` vaut 0 quand il est inconnu.
    fn transfer(&self, label: &str, done: u64, total: u64);

    /// Le téléchargement en cours est terminé.
    fn transfer_done(&self);
}

const MEGABYTE: f64 = 1024.0 * 1024.0;

/// « 12,3 Mo » — virgule décimale, comme tout le reste de l'interface.
pub fn megabytes(bytes: u64) -> String {
    format!("{:.1} Mo", bytes as f64 / MEGABYTE).replace('.', ",")
}

/// Ligne d'avancement d'un téléchargement, la même partout.
pub fn transfer_text(label: &str, done: u64, total: u64) -> String {
    match done.saturating_mul(100).checked_div(total) {
        Some(percent) => format!(
            "{label} : {} / {} ({} %)",
            megabytes(done),
            megabytes(total),
            percent.min(100)
        ),
        None => format!("{label} : {}", megabytes(done)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_shown_with_a_decimal_comma() {
        assert_eq!(megabytes(0), "0,0 Mo");
        assert_eq!(megabytes(52_428_800), "50,0 Mo");
        assert_eq!(megabytes(1_258_291), "1,2 Mo");
    }

    #[test]
    fn a_transfer_line_shows_the_ratio_when_the_total_is_known() {
        assert_eq!(
            transfer_text("moteur", 13_107_200, 52_428_800),
            "moteur : 12,5 Mo / 50,0 Mo (25 %)"
        );
        assert_eq!(transfer_text("moteur", 13_107_200, 0), "moteur : 12,5 Mo");
    }

    /// Un fichier plus gros qu'annoncé ne doit pas dépasser 100 %.
    #[test]
    fn the_percentage_is_capped() {
        assert!(transfer_text("x", 200, 100).ends_with("(100 %)"));
    }
}
