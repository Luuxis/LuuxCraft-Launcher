//! Erreurs présentées au joueur.
//!
//! L'installeur est lancé par un double-clic, sans journal ni interface : la
//! seule chose que le joueur verra d'un échec est la ligne écrite ici. Chaque
//! erreur porte donc ce qui a échoué **et** une piste d'action ; un `panic!`
//! Rust, lui, ne veut rien dire pour lui.

use std::fmt;
use std::path::Path;

pub type Result<T> = std::result::Result<T, BootstrapError>;

#[derive(Debug)]
pub struct BootstrapError {
    message: String,
    hint: Option<String>,
}

const DISK_FULL: &str = "Le disque est plein. Libérez de l'espace puis relancez l'installation.";
const DENIED: &str = "L'accès au dossier a été refusé. Fermez le launcher s'il tourne déjà, \
                      vérifiez votre antivirus, puis relancez l'installation.";

impl BootstrapError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Complète une erreur d'entrée-sortie sans écraser le conseil déduit du
    /// code système : « disque plein » est toujours plus utile que le générique.
    pub fn or_hint(mut self, hint: impl Into<String>) -> Self {
        if self.hint.is_none() {
            self.hint = Some(hint.into());
        }
        self
    }

    /// Erreur disque : le message système seul (« os error 28 ») ne dit rien au
    /// joueur, on y accroche la seule action qui le débloque.
    pub fn io(action: &str, path: &Path, error: &std::io::Error) -> Self {
        Self {
            message: format!("{action} a échoué sur {} : {error}", path.display()),
            hint: disk_hint(error),
        }
    }

}

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Erreur : {}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(formatter, "\n{hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for BootstrapError {}

/// `ErrorKind::StorageFull` n'est pas stabilisé : on lit le code système brut.
/// 28 = `ENOSPC`, 112 = `ERROR_DISK_FULL`, 39 = `ERROR_HANDLE_DISK_FULL`.
fn disk_hint(error: &std::io::Error) -> Option<String> {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        return Some(DENIED.to_owned());
    }
    match error.raw_os_error() {
        Some(28) if cfg!(unix) => Some(DISK_FULL.to_owned()),
        Some(112) | Some(39) if cfg!(windows) => Some(DISK_FULL.to_owned()),
        _ => None,
    }
}

/// Traduit un échec HTTP en quelque chose d'actionnable.
///
/// Le détail de `ureq` est lu **avant** le `match` : les variantes de son
/// énumération peuvent bouger d'une version à l'autre, son `Display` non.
pub fn from_http(url: &str, error: ureq::Error) -> BootstrapError {
    let detail = error.to_string();
    match error {
        ureq::Error::Status(404, _) => BootstrapError::new(format!(
            "le panel ne connaît pas ce serveur (404).\n{url}"
        ))
        .hint(
            "Cet installeur a peut-être été supprimé du panel. \
             Retéléchargez-le depuis l'espace de votre serveur.",
        ),
        ureq::Error::Status(401, _) | ureq::Error::Status(402, _) | ureq::Error::Status(403, _) => {
            BootstrapError::new(format!(
                "le panel a refusé la requête ({detail}).\n{url}"
            ))
            .hint(
                "L'abonnement de ce serveur est probablement expiré. \
                 Le propriétaire du serveur doit le réactiver sur le panel.",
            )
        }
        ureq::Error::Status(code, _) if (500..600).contains(&code) => {
            BootstrapError::new(format!("le panel est en erreur ({code}).\n{url}"))
                .hint("Réessayez dans quelques minutes.")
        }
        ureq::Error::Status(code, _) => {
            BootstrapError::new(format!("le panel a répondu {code}.\n{url}"))
                .hint("Réessayez plus tard, ou prévenez le propriétaire du serveur.")
        }
        #[allow(unreachable_patterns)]
        _ => BootstrapError::new(format!("le panel est injoignable ({detail}).\n{url}")).hint(
            "Vérifiez votre connexion internet, votre pare-feu ou votre antivirus, \
             puis relancez l'installation.",
        ),
    }
}
