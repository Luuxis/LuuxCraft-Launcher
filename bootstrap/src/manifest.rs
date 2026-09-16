//! Manifeste du serveur : tout ce que le bootstrap sait du client.
//!
//! Le créneau d'identité ne porte qu'un identifiant ; le nom affiché, le slug,
//! la version du moteur et la liste du pack client viennent d'ici. Changer le
//! nom ou le logo d'un serveur ne demande donc **jamais** de republier un
//! installeur : le manifeste suivant portera les nouvelles valeurs.
//!
//! La réponse est validée avant usage : `slug` sert de nom de dossier et les
//! `path` du pack de noms de fichiers. Un panel compromis ou simplement buggé
//! ne doit pas pouvoir écrire ailleurs que dans la racine d'installation.

use serde::Deserialize;

use crate::config;
use crate::error::{BootstrapError, Result};
use crate::net::Http;
use crate::paths;

/// Version de schéma que ce bootstrap sait lire.
const SCHEMA: u32 = 1;

/// Fichiers que le pack client a le droit de contenir.
///
/// Liste fermée, la même que `PACK_PATHS` côté panel : rien qui vienne du
/// réseau ne décide d'un nom de fichier sur le disque du joueur.
const ALLOWED_PACK_FILES: &[&str] = &["client_config.json", "icon.png", "icon.ico", "icon.icns"];

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
    /// Empreinte agrégée du pack, écrite dans `.pack-version`.
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

/// Va chercher le manifeste du serveur.
///
/// `os` et `arch` sont transmis en requête : le manifeste décrit un bundle
/// moteur précis, le panel doit donc savoir pour quelle machine il répond.
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
        BootstrapError::new(format!("réponse inattendue du panel ({failure}).\n{url}"))
            .hint("Le panel est peut-être en maintenance. Réessayez dans quelques minutes.")
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

        // Le panel doit répondre pour le serveur demandé, et pour aucun autre :
        // une redirection mal placée ne doit pas pouvoir installer le client de
        // quelqu'un d'autre.
        if !self.tenant_id.eq_ignore_ascii_case(tenant_id) {
            return Err(BootstrapError::new(
                "le panel a répondu pour un autre serveur que celui de cet installeur.",
            )
            .hint("Retéléchargez l'installeur depuis l'espace de votre serveur."));
        }

        if !is_slug(&self.slug) {
            return Err(unexpected("identifiant de serveur invalide"));
        }

        if !is_panel_url(&self.engine.url) || !is_sha256(&self.engine.sha256) {
            return Err(unexpected("description du moteur invalide"));
        }

        if self.client_pack.files.is_empty() {
            return Err(unexpected("configuration de serveur vide"));
        }

        for file in &self.client_pack.files {
            if !ALLOWED_PACK_FILES.contains(&file.path.as_str()) {
                return Err(unexpected("fichier de configuration inattendu"));
            }
            if !is_panel_url(&file.url) || !is_sha256(&file.sha256) {
                return Err(unexpected("fichier de configuration invalide"));
            }
            // Doublement : la liste fermée suffirait, mais c'est ce chemin-là
            // qui sera joint à la racine d'installation.
            if paths::safe_relative(&file.path).is_none() {
                return Err(unexpected("chemin de configuration invalide"));
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

/// Le slug est produit par le panel ; on ne le dérive jamais côté client, on
/// vérifie seulement qu'il est utilisable comme nom de dossier.
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

/// Une URL de téléchargement doit rester sur le panel compilé dans ce binaire.
///
/// Sans ce contrôle, un manifeste altéré enverrait chercher l'exécutable
/// n'importe où — et le `sha256` qui l'accompagne viendrait de la même source,
/// donc ne protégerait de rien.
fn is_panel_url(value: &str) -> bool {
    let base = config::panel_base();
    value.len() > base.len() && value.starts_with(base) && value[base.len()..].starts_with('/')
}

/// Manifeste d'exemple, partagé par les tests des autres modules.
#[cfg(test)]
pub mod tests_support {
    use super::*;

    pub fn sample() -> Manifest {
        Manifest {
            schema: 1,
            tenant_id: "11111111-2222-4333-8444-555555555555".into(),
            slug: "mon-serveur".into(),
            display_name: "Mon Serveur".into(),
            engine: Engine {
                version: "1.4.0".into(),
                url: format!("{}/api/v1/engine/1.4.0/windows/x86_64", config::panel_base()),
                sha256: "a".repeat(64),
                size: 0,
                format: "exe-zip".into(),
            },
            client_pack: ClientPack {
                version: "v".into(),
                files: vec![PackFile {
                    path: "client_config.json".into(),
                    url: format!("{}/api/v1/launchers/x/pack/client_config.json", config::panel_base()),
                    sha256: "b".repeat(64),
                    size: 10,
                }],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(files: Vec<PackFile>) -> Manifest {
        Manifest {
            schema: 1,
            tenant_id: "abc".into(),
            slug: "mon-serveur".into(),
            display_name: "Mon Serveur".into(),
            engine: Engine {
                version: "1.4.0".into(),
                url: format!("{}/api/v1/engine/1.4.0/windows/x86_64", config::panel_base()),
                sha256: "a".repeat(64),
                size: 0,
                format: "exe-zip".into(),
            },
            client_pack: ClientPack {
                version: "v".into(),
                files,
            },
        }
    }

    fn pack_file(path: &str) -> PackFile {
        PackFile {
            path: path.into(),
            url: format!("{}/api/v1/launchers/abc/pack/{path}", config::panel_base()),
            sha256: "b".repeat(64),
            size: 10,
        }
    }

    #[test]
    fn a_well_formed_manifest_is_accepted() {
        assert!(manifest(vec![pack_file("client_config.json")])
            .validate("abc")
            .is_ok());
    }

    /// Le panel doit répondre pour le serveur demandé, et pour aucun autre.
    #[test]
    fn a_manifest_for_another_tenant_is_refused() {
        assert!(manifest(vec![pack_file("client_config.json")])
            .validate("def")
            .is_err());
    }

    /// Le `sha256` vient de la même réponse que l'URL : si l'URL peut pointer
    /// ailleurs, l'empreinte ne protège de rien.
    #[test]
    fn a_download_outside_the_panel_is_refused() {
        let mut broken = manifest(vec![pack_file("client_config.json")]);
        broken.engine.url = "https://evil.example/engine.zip".into();
        assert!(broken.validate("abc").is_err());

        // Un préfixe qui ressemble au panel n'en est pas un.
        broken.engine.url = format!("{}.evil.example/engine.zip", config::panel_base());
        assert!(broken.validate("abc").is_err());
    }

    /// La liste des fichiers du pack est fermée : rien d'autre ne s'écrit.
    #[test]
    fn an_unexpected_pack_file_is_refused() {
        for path in ["evil.exe", "../../evil", "client_config.json.bak", ""] {
            assert!(
                manifest(vec![pack_file(path)]).validate("abc").is_err(),
                "{path:?} must be refused",
            );
        }
    }

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
        let mut blank = manifest(vec![pack_file("icon.png")]);
        blank.display_name = "   ".into();
        assert_eq!(blank.display_name(), "mon-serveur");
    }
}
