//! Manifeste du tenant : tout ce que le bootstrap sait du client.
//!
//! C'est le seul point d'entrée du bootstrap. L'overlay ne porte qu'un UUID ;
//! le nom affiché, le slug, la version du moteur et la liste du pack client
//! viennent d'ici, donc changer l'identité d'un client ne demande jamais de
//! republier un installeur.
//!
//! La réponse est validée avant usage : `slug` sert de nom de dossier et les
//! `path` du pack de noms de fichiers, un panel compromis ou buggé ne doit pas
//! pouvoir écrire ailleurs que dans la racine d'installation.

use serde::Deserialize;

use crate::config;
use crate::error::{BootstrapError, Result};
use crate::net::Http;
use crate::paths;

/// Version de schéma que ce bootstrap sait lire (contrat §3).
const SCHEMA: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub tenant_id: String,
    pub slug: String,
    pub display_name: String,
    pub engine: Engine,
    pub client_pack: ClientPack,
}

#[derive(Debug, Deserialize)]
pub struct Engine {
    pub version: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size: u64,
    pub format: String,
}

#[derive(Debug, Deserialize)]
pub struct ClientPack {
    /// Empreinte agrégée du pack, écrite dans `.pack-version` (contrat §3).
    pub version: String,
    pub files: Vec<PackFile>,
}

#[derive(Debug, Deserialize)]
pub struct PackFile {
    pub path: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size: u64,
}

/// Va chercher le manifeste du tenant.
///
/// `os` et `arch` sont transmis en query : le manifeste décrit un bundle moteur
/// précis (`…/engine/1.2.3/windows/x86_64`), le panel doit donc savoir pour
/// quelle machine il répond.
pub fn fetch(http: &Http, tenant_id: &str) -> Result<Manifest> {
    let url = format!(
        "{}/api/v1/launchers/{}/manifest?os={}&arch={}",
        config::panel_base(),
        tenant_id,
        config::OS,
        std::env::consts::ARCH
    );

    let body = http.get_text(&url)?;
    let manifest: Manifest = serde_json::from_str(&body).map_err(|failure| {
        BootstrapError::new(format!("réponse inattendue du panel ({failure}).\n{url}")).hint(
            "Le panel est peut-être en maintenance. Réessayez dans quelques minutes.",
        )
    })?;

    manifest.validate(tenant_id)?;
    Ok(manifest)
}

impl Manifest {
    fn validate(&self, tenant_id: &str) -> Result<()> {
        if self.schema != SCHEMA {
            return Err(BootstrapError::new(format!(
                "ce panel publie un manifeste de version {} alors que cet installeur lit la version {SCHEMA}.",
                self.schema
            ))
            .hint("Retéléchargez l'installeur depuis le panel : celui-ci est trop ancien."));
        }

        if !self.tenant_id.eq_ignore_ascii_case(tenant_id) {
            return Err(BootstrapError::new(
                "le panel a répondu pour un autre client que celui de cet installeur.",
            )
            .hint("Retéléchargez l'installeur depuis votre espace client."));
        }

        if !is_slug(&self.slug) {
            return Err(unexpected("identifiant de client invalide"));
        }

        if self.engine.url.is_empty() || !is_sha256(&self.engine.sha256) {
            return Err(unexpected("description du moteur invalide"));
        }

        if self.client_pack.files.is_empty() {
            return Err(unexpected("pack client vide"));
        }

        for file in &self.client_pack.files {
            if file.url.is_empty() || !is_sha256(&file.sha256) {
                return Err(unexpected("fichier de pack client invalide"));
            }
            if paths::safe_relative(&file.path).is_none() {
                return Err(unexpected("chemin de pack client invalide"));
            }
        }

        Ok(())
    }

    /// Nom montré au joueur, avec repli sur le slug si le panel l'a laissé vide.
    pub fn display_name(&self) -> &str {
        let name = self.display_name.trim();
        if name.is_empty() {
            &self.slug
        } else {
            name
        }
    }
}

fn unexpected(what: &str) -> BootstrapError {
    BootstrapError::new(format!("le panel a renvoyé un manifeste incohérent ({what})."))
        .hint("Prévenez le propriétaire du serveur : sa configuration de launcher est cassée.")
}

/// Le slug est produit par le panel (contrat §1) ; on ne le dérive jamais côté
/// client, on vérifie seulement qu'il est utilisable comme nom de dossier.
fn is_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_follow_the_contract() {
        assert!(is_slug("mon-serveur"));
        assert!(is_slug("a1"));
        assert!(!is_slug(""));
        assert!(!is_slug("-mon"));
        assert!(!is_slug("mon-"));
        assert!(!is_slug("Mon-Serveur"));
        assert!(!is_slug("mon/serveur"));
        assert!(!is_slug(".."));
    }

    #[test]
    fn sha256_must_be_64_hex_chars() {
        assert!(is_sha256(&"a".repeat(64)));
        assert!(!is_sha256(&"a".repeat(63)));
        assert!(!is_sha256(&"z".repeat(64)));
    }

    #[test]
    fn display_name_falls_back_to_the_slug() {
        let manifest = Manifest {
            schema: 1,
            tenant_id: "id".into(),
            slug: "mon-serveur".into(),
            display_name: "   ".into(),
            engine: Engine {
                version: "1.0.0".into(),
                url: "https://panel/engine".into(),
                sha256: "a".repeat(64),
                size: 0,
                format: "appimage".into(),
            },
            client_pack: ClientPack {
                version: "v".into(),
                files: Vec::new(),
            },
        };
        assert_eq!(manifest.display_name(), "mon-serveur");
    }
}
