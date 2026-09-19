//! Le pack client de développement, téléchargé depuis le panel.
//!
//! En debug il n'y a pas d'installation, donc pas d'installeur pour poser le
//! pack dans `data/client/` — et l'écrire à la main à chaque changement de
//! tenant est vite pénible. Le moteur peut donc aller le chercher lui-même :
//!
//! ```sh
//! npm start                          # staging, voir les scripts de package.json
//! npm run start:dev                  # wrangler dev local
//! npm run tauri dev -- -- -- --tenant-id=<users.id> --api-url=http://localhost:8080
//! LUUXCRAFT_TENANT_ID=<users.id> LUUXCRAFT_PANEL_URL=… npm run tauri dev
//! ```
//!
//! Les fichiers sont ceux que l'installeur poserait, servis par la même route
//! (`/api/v1/launchers/<tenant>/pack/<fichier>`), et ils sont écrits dans
//! `data/client/`. Les lancements suivants les relisent sans rien redemander,
//! jusqu'à ce que l'argument soit redonné — c'est ce qui permet de démarrer
//! hors ligne une fois le pack posé.
//!
//! Le panel peut être en `http://` : c'est le cas d'un `wrangler dev` local,
//! et le pack qu'il sert porte alors une `api_base_url` en clair, que seule la
//! politique `Policy::DEVELOPMENT` accepte. Le contrat, lui, ne change pas.
//!
//! Trois `--` à la main : le premier est consommé par npm, le deuxième par la
//! CLI tauri, et le troisième la fait passer ce qui suit au binaire plutôt qu'à
//! `cargo run`. Les scripts de `package.json` n'ont donc que les deux derniers.
//!
//! Rien de tout cela n'est atteignable en release : `client_config::resolve`
//! ne consulte ce module que sous `cfg!(debug_assertions)`. Un moteur installé
//! ne télécharge jamais son identité, et un argument de ligne de commande ne
//! doit pas pouvoir la lui faire changer.

use std::path::Path;
use std::time::Duration;

use crate::client_config::{has_host, ClientConfig, Policy, CLIENT_CONFIG_FILE};

const TENANT_FLAG: &str = "--tenant-id";
const API_URL_FLAG: &str = "--api-url";
const TENANT_ENV: &str = "LUUXCRAFT_TENANT_ID";
const PANEL_ENV: &str = "LUUXCRAFT_PANEL_URL";
/// Le panel de production, quand `--api-url` n'est pas donné.
const DEFAULT_PANEL: &str = "https://luuxcraft.fr";
/// Le logo du pack. Facultatif : un tenant sans PNG n'en a pas.
const ICON_FILE: &str = "icon.png";
/// Au-delà, ce n'est plus un fichier du pack.
const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Ce qu'il faut écrire dans le message d'erreur d'un pack absent.
pub const USAGE: &str = "--tenant-id=<users.id> [--api-url=<origin>] (see `npm start`)";

/// Un tenant à aller chercher, et sur quel panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub tenant_id: String,
    /// Origine du panel, sans slash final ni `/api`.
    pub panel: String,
}

impl Request {
    /// La demande portée par la ligne de commande ou l'environnement, s'il y en
    /// a une. Un argument mal formé est une erreur, pas une absence : celui qui
    /// l'a tapé attend un téléchargement, pas un démarrage silencieux sur
    /// l'ancien pack.
    pub fn from_env() -> Result<Option<Self>, String> {
        Self::parse(std::env::args().skip(1), |key| std::env::var(key).ok())
    }

    fn parse(
        args: impl IntoIterator<Item = String>,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, String> {
        let mut tenant = None;
        let mut panel = None;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if let Some(value) = flag_value(&arg, TENANT_FLAG, &mut args) {
                tenant = Some(value?);
            } else if let Some(value) = flag_value(&arg, API_URL_FLAG, &mut args) {
                panel = Some(value?);
            }
        }

        let tenant = tenant.or_else(|| env(TENANT_ENV));
        let panel = panel.or_else(|| env(PANEL_ENV));

        let Some(tenant_id) = tenant else {
            return Ok(None);
        };
        let tenant_id = tenant_id.trim().to_ascii_lowercase();
        // Les routes de distribution du panel n'acceptent que `users.id`, un
        // UUID. Le code d'appairage à neuf caractères (`users.client_id`, celui
        // des liens publics) y est refusé exprès : il circule en clair sur les
        // sites des clients. Le dire ici évite d'aller chercher un 404 pour
        // conclure à tort que le tenant n'existe pas.
        if !is_uuid(&tenant_id) {
            let hint = if is_pairing_code(&tenant_id) {
                " (this is the pairing code; use the tenant's users.id, the UUID shown in the install command of the dashboard)"
            } else {
                ""
            };
            return Err(format!(
                "{TENANT_FLAG} must be a users.id UUID, got {tenant_id:?}{hint}"
            ));
        }

        let panel = panel.unwrap_or_else(|| DEFAULT_PANEL.to_owned());
        let panel = panel.trim().trim_end_matches('/');
        // Tolère l'adresse de l'API (`…/api`) à la place de l'origine : c'est
        // celle que le pack et le README affichent le plus souvent.
        let panel = panel.strip_suffix("/api").unwrap_or(panel);
        // `http://` pour un panel local ; le pack qu'il sert n'est accepté que
        // par la politique de développement, jamais par une installation.
        if !has_host(panel, "https://") && !has_host(panel, "http://") {
            return Err(format!("{API_URL_FLAG} must be an http(s) origin: {panel:?}"));
        }

        Ok(Some(Self {
            tenant_id,
            panel: panel.to_owned(),
        }))
    }

    fn file_url(&self, file: &str) -> String {
        format!(
            "{}/api/v1/launchers/{}/pack/{file}",
            self.panel, self.tenant_id
        )
    }

    /// Télécharge le pack dans `dir`. Bloque : le moteur n'a encore rien
    /// construit, et sans pack il n'a rien à construire.
    pub fn install(&self, dir: &Path) -> Result<(), String> {
        eprintln!(
            "luuxcraft-launcher: fetching the client pack of {} from {}",
            self.tenant_id, self.panel
        );
        tauri::async_runtime::block_on(self.download(dir))?;
        eprintln!("luuxcraft-launcher: client pack written to {}", dir.display());
        Ok(())
    }

    async fn download(&self, dir: &Path) -> Result<(), String> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;

        // Le pack est validé **avant** la moindre écriture : un panel qui
        // répond de travers ne doit pas laisser un `data/client/` inutilisable
        // à la place de celui qui marchait.
        let config = fetch(&http, &self.file_url(CLIENT_CONFIG_FILE))
            .await?
            .ok_or_else(|| {
                format!(
                    "{} has no pack for tenant {} (unknown tenant, or no launcher configured)",
                    self.panel, self.tenant_id
                )
            })?;
        let parsed = ClientConfig::parse_with(&config, dir, Policy::DEVELOPMENT)?;
        if parsed.tenant_id != self.tenant_id {
            return Err(format!(
                "the panel served the pack of {} instead of {}",
                parsed.tenant_id, self.tenant_id
            ));
        }
        let icon = fetch(&http, &self.file_url(ICON_FILE)).await?;

        std::fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        write(&dir.join(CLIENT_CONFIG_FILE), &config)?;
        match icon {
            Some(bytes) => write(&dir.join(ICON_FILE), &bytes)?,
            // Le tenant n'a pas de logo : celui de l'ancien tenant ne doit pas
            // lui rester.
            None => remove(&dir.join(ICON_FILE))?,
        }
        Ok(())
    }
}

/// `8-4-4-4-12` hexadécimal, en minuscules : la forme que le panel exige d'un
/// `tenantId` sur `/api/v1/launchers/…`.
fn is_uuid(value: &str) -> bool {
    let groups: Vec<&str> = value.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(group, len)| group.len() == len && group.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// `xxx-xxx-xxx` : le code d'appairage `users.client_id`.
fn is_pairing_code(value: &str) -> bool {
    let groups: Vec<&str> = value.split('-').collect();
    groups.len() == 3
        && groups
            .iter()
            .all(|group| group.len() == 3 && group.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// `--flag=valeur` ou `--flag valeur`. `None` si l'argument n'est pas ce flag.
///
/// `--api_url` vaut `--api-url` : le nom du flag est comparé tirets et
/// soulignés confondus, la valeur est prise telle quelle.
fn flag_value(
    arg: &str,
    flag: &str,
    rest: &mut impl Iterator<Item = String>,
) -> Option<Result<String, String>> {
    let (name, value) = match arg.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (arg, None),
    };
    if name.replace('_', "-") != flag {
        return None;
    }
    match value {
        Some(value) => Some(Ok(value.to_owned())),
        None => Some(rest.next().ok_or_else(|| format!("{flag} expects a value"))),
    }
}

/// Un fichier du pack, ou `None` si le panel dit ne pas l'avoir (404 JSON).
/// Toute autre réponse non-2xx est une erreur, avec le message du panel quand
/// il en donne un — c'est lui qui dit « abonnement expiré » ou « tenant
/// inconnu ».
async fn fetch(http: &reqwest::Client, url: &str) -> Result<Option<Vec<u8>>, String> {
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|error| format!("{url}: {error}"))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("{url}: {error}"))?;
    if !status.is_success() {
        let message = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|body| body.get("message")?.as_str().map(str::to_owned));
        // Un 404 du panel (JSON) dit « pas ce fichier pour ce tenant » ; un
        // 404 sans corps JSON vient d'un panel qui n'a pas cette route.
        return match (status, message) {
            (reqwest::StatusCode::NOT_FOUND, Some(_)) => Ok(None),
            (_, Some(message)) => Err(format!("{url} answered {}: {message}", status.as_u16())),
            (_, None) => Err(format!(
                "{url} answered {} without a panel error: does this panel serve launcher packs?",
                status.as_u16()
            )),
        };
    }
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("{url} is {} bytes, which is not a pack file", bytes.len()));
    }
    Ok(Some(bytes.to_vec()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn remove(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot remove {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TENANT: &str = "00000000-0000-4000-8000-000000000001";

    fn parse(args: &[&str]) -> Result<Option<Request>, String> {
        Request::parse(args.iter().map(|arg| (*arg).to_owned()), |_| None)
    }

    #[test]
    fn without_the_flag_nothing_is_requested() {
        assert_eq!(parse(&[]), Ok(None));
        assert_eq!(parse(&["--api-url=https://a.fr"]), Ok(None));
    }

    #[test]
    fn the_flag_takes_both_spellings() {
        let expected = Some(Request {
            tenant_id: TENANT.to_owned(),
            panel: DEFAULT_PANEL.to_owned(),
        });
        assert_eq!(parse(&[&format!("--tenant-id={TENANT}")]), Ok(expected.clone()));
        assert_eq!(parse(&["--tenant-id", TENANT]), Ok(expected.clone()));
        assert_eq!(parse(&[&format!("--tenant_id={TENANT}")]), Ok(expected));
    }

    /// `--api_url` et `--api-url` sont le même flag ; la valeur, elle, n'est
    /// pas retouchée.
    #[test]
    fn underscores_in_a_flag_name_are_dashes() {
        let request = parse(&[&format!("--tenant-id={TENANT}"), "--api_url=https://a_b.example/api"])
            .expect("valid request")
            .expect("a request");
        assert_eq!(request.panel, "https://a_b.example");
    }

    #[test]
    fn the_panel_is_an_origin_even_when_given_as_an_api_base() {
        for panel in [
            "https://panel.example",
            "https://panel.example/",
            "https://panel.example/api",
            "https://panel.example/api/",
        ] {
            let request = parse(&[&format!("--tenant-id={TENANT}"), &format!("--api-url={panel}")])
                .expect("valid request")
                .expect("a request");
            assert_eq!(request.panel, "https://panel.example", "from {panel}");
            assert_eq!(
                request.file_url(CLIENT_CONFIG_FILE),
                format!("https://panel.example/api/v1/launchers/{TENANT}/pack/client_config.json"),
            );
        }
    }

    /// Un `wrangler dev` local parle en clair : c'est le cas d'usage premier de
    /// `--api-url`. Ce qui n'est pas une origine http(s) reste refusé.
    #[test]
    fn a_local_panel_in_plain_http_is_accepted() {
        let request = parse(&[&format!("--tenant-id={TENANT}"), "--api-url=http://localhost:8080/"])
            .expect("valid request")
            .expect("a request");
        assert_eq!(request.panel, "http://localhost:8080");
        for panel in ["localhost:8080", "ftp://panel.example", "http://", "https://"] {
            assert!(
                parse(&[&format!("--tenant-id={TENANT}"), &format!("--api-url={panel}")]).is_err(),
                "{panel} must be refused",
            );
        }
    }

    /// Le panel n'ouvre ses routes de distribution qu'à `users.id` : tout ce
    /// qui n'est pas un UUID est refusé avant d'interroger quoi que ce soit.
    #[test]
    fn a_tenant_that_is_not_a_uuid_is_refused() {
        assert!(parse(&["--tenant-id=../admin"]).is_err());
        assert!(parse(&["--tenant-id="]).is_err());
        assert!(parse(&["--tenant-id"]).is_err());
        assert!(parse(&["--tenant-id=00000000-0000-4000-8000-00000000000g"]).is_err());
    }

    /// Le code d'appairage est l'erreur la plus probable : c'est l'identifiant
    /// que le panel affiche partout ailleurs. Le message doit le nommer.
    #[test]
    fn the_pairing_code_is_named_when_given_by_mistake() {
        let error = parse(&["--tenant-id=mt1-lno-1ib"]).expect_err("refused");
        assert!(error.contains("pairing code"), "{error}");
        assert!(is_pairing_code("mt1-lno-1ib"));
        assert!(!is_pairing_code("mt1-lno-1ibx"));
    }

    /// L'UUID est normalisé en minuscules : c'est ainsi que le panel le compare.
    #[test]
    fn an_uppercase_uuid_is_lowercased() {
        let request = parse(&[&format!("--tenant-id={}", TENANT.to_ascii_uppercase())])
            .expect("valid request")
            .expect("a request");
        assert_eq!(request.tenant_id, TENANT);
    }

    #[test]
    fn the_environment_stands_in_for_the_flags() {
        let request = Request::parse(std::iter::empty(), |key| match key {
            TENANT_ENV => Some(TENANT.to_owned()),
            PANEL_ENV => Some("https://env.example/".to_owned()),
            _ => None,
        })
        .expect("valid request")
        .expect("a request");
        assert_eq!(request.tenant_id, TENANT);
        assert_eq!(request.panel, "https://env.example");
    }

    #[test]
    fn the_flags_win_over_the_environment() {
        let request = Request::parse([format!("--tenant-id={TENANT}")], |key| {
            (key == TENANT_ENV).then(|| "other".to_owned())
        })
        .expect("valid request")
        .expect("a request");
        assert_eq!(request.tenant_id, TENANT);
    }
}
