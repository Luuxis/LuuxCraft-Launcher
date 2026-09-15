//! Models of the LuuxCraft panel API (`/api/user/{user_id}/...`).
//!
//! The panel evolves: fields get added, renamed or removed. Every model is
//! therefore built from a `serde_json::Value` with tolerant accessors, keeps
//! the unknown fields in `extra`, and never makes the launcher fail because a
//! field it does not need is missing.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `GET /config`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConfig {
    pub maintenance: bool,
    pub maintenance_message: Option<String>,
    pub data_directory: Option<String>,
    pub auth: AuthMode,
    /// Microsoft Azure application id announced by the panel (optional).
    pub client_id: Option<String>,
    pub links: Vec<Link>,
    /// Module toggles published by the panel (`modules`, `features`).
    pub modules: Map<String, Value>,
    /// Launcher identity published by the panel. Anything missing falls back
    /// to the built-in brand, which is what the title bar paints before the
    /// first request completes.
    pub brand: Option<RemoteBrand>,
    /// Tauri updater endpoints published by the panel; they take precedence
    /// over the built-in ones so a release can be pointed elsewhere.
    pub updater_endpoints: Vec<String>,
    /// Yggdrasil-compatible server (authlib-injector style) enabling that
    /// extra sign-in method.
    pub yggdrasil: Option<String>,
    /// Every field the launcher does not model, for future panel features.
    pub extra: Map<String, Value>,
}

/// Launcher identity: `LuuxCraft` in the title bar is `prefix` + `suffix`,
/// the suffix being the part painted in the brand gradient.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBrand {
    pub name: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub subtitle: Option<String>,
    pub website: Option<String>,
    /// Logo du client. Le launcher en fait l'icône de sa fenêtre et de la barre
    /// des tâches à chaud — c'est ce qui donne le bon logo sans compiler un
    /// launcher par client.
    pub icon_url: Option<String>,
}

/// How players sign in, decided by the panel's `online` field:
/// `true` → Microsoft, `false` → offline, a URL → Azuriom (AZauth).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AuthMode {
    Microsoft,
    Offline,
    AzAuth { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub label: String,
    pub url: String,
    pub icon: Option<String>,
    pub order: Option<i64>,
}

/// `GET /articles`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    pub id: String,
    pub title: String,
    /// HTML produced by the panel editor; sanitized by the frontend.
    pub content: String,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub image: Option<String>,
    pub url: Option<String>,
    pub order: Option<i64>,
    pub extra: Map<String, Value>,
}

/// `GET /instances` entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub image: Option<String>,
    /// URL of the custom files list (`path`, `hash`, `size`, `url`).
    pub files_url: Option<String>,
    pub minecraft_version: String,
    pub loader: InstanceLoader,
    pub verify: bool,
    pub ignored: Vec<String>,
    pub whitelist: Vec<String>,
    pub whitelist_active: bool,
    pub server: Option<InstanceServer>,
    /// Java major version required by the panel (overrides Mojang's manifest).
    pub java_version: Option<String>,
    pub jvm_args: Vec<String>,
    pub memory: Option<InstanceMemory>,
    pub order: Option<i64>,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceLoader {
    /// `forge`, `neoforge`, `fabric`, `legacyfabric`, `quilt`, `mcp` or `none`.
    pub kind: String,
    /// `latest`, `recommended` or an exact build.
    pub version: String,
    pub mcp_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceServer {
    pub name: Option<String>,
    pub host: String,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceMemory {
    pub min_mb: Option<u64>,
    pub max_mb: Option<u64>,
}

// ---------------------------------------------------------------------------
// Tolerant accessors
// ---------------------------------------------------------------------------

fn pick<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter()
        .find_map(|key| object.get(*key))
        .filter(|value| !value.is_null())
}

fn pick_string(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    let value = pick(object, keys)?;
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn pick_bool(object: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    match pick(object, keys)? {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => number.as_i64().map(|n| n != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn pick_i64(object: &Map<String, Value>, keys: &[&str]) -> Option<i64> {
    match pick(object, keys)? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn pick_u64(object: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
    pick_i64(object, keys).filter(|n| *n >= 0).map(|n| n as u64)
}

fn pick_string_list(object: &Map<String, Value>, keys: &[&str]) -> Vec<String> {
    match pick(object, keys) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
                _ => None,
            })
            .collect(),
        Some(Value::String(text)) => text.split_whitespace().map(str::to_owned).collect(),
        _ => Vec::new(),
    }
}

fn pick_object<'a>(
    object: &'a Map<String, Value>,
    keys: &[&str],
) -> Option<&'a Map<String, Value>> {
    pick(object, keys).and_then(Value::as_object)
}

fn remaining(object: &Map<String, Value>, known: &[&str]) -> Map<String, Value> {
    object
        .iter()
        .filter(|(key, _)| !known.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// Memory values may be announced in MiB (`4096`), GiB (`4`) or strings (`4G`, `4096M`).
fn parse_memory_mb(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => {
            let n = number.as_u64()?;
            Some(if n <= 64 { n * 1024 } else { n })
        }
        Value::String(text) => {
            let text = text.trim().to_ascii_uppercase();
            if let Some(number) = text.strip_suffix('G') {
                return number.trim().parse::<u64>().ok().map(|n| n * 1024);
            }
            if let Some(number) = text.strip_suffix('M') {
                return number.trim().parse::<u64>().ok();
            }
            let n = text.parse::<u64>().ok()?;
            Some(if n <= 64 { n * 1024 } else { n })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

impl RemoteConfig {
    const KNOWN: &'static [&'static str] = &[
        "maintenance",
        "maintenance_message",
        "maintenanceMessage",
        "dataDirectory",
        "data_directory",
        "online",
        "client_id",
        "clientId",
        "socialLinks",
        "social_links",
        "links",
        "modules",
        "features",
        "brand",
        "updater",
        "updaterEndpoints",
        "updater_endpoints",
        "yggdrasil",
        "yggdrasilServer",
        "yggdrasil_server",
    ];

    pub fn from_value(value: Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "config is not a JSON object".to_owned())?;
        let auth = match pick(object, &["online"]) {
            Some(Value::Bool(true)) | None => AuthMode::Microsoft,
            Some(Value::Bool(false)) => AuthMode::Offline,
            Some(Value::String(url)) => {
                let url = url.trim();
                if url.starts_with("http://") || url.starts_with("https://") {
                    AuthMode::AzAuth {
                        url: url.to_owned(),
                    }
                } else {
                    match url.to_ascii_lowercase().as_str() {
                        "false" | "0" | "offline" | "crack" => AuthMode::Offline,
                        _ => AuthMode::Microsoft,
                    }
                }
            }
            Some(_) => AuthMode::Microsoft,
        };
        let links = pick(object, &["socialLinks", "social_links", "links"])
            .map(Link::list_from_value)
            .unwrap_or_default();
        let modules = pick_object(object, &["modules", "features"])
            .cloned()
            .unwrap_or_default();
        Ok(Self {
            maintenance: pick_bool(object, &["maintenance"]).unwrap_or(false),
            maintenance_message: pick_string(
                object,
                &["maintenance_message", "maintenanceMessage"],
            ),
            data_directory: pick_string(object, &["dataDirectory", "data_directory"]),
            auth,
            client_id: pick_string(object, &["client_id", "clientId"]),
            links,
            modules,
            brand: RemoteBrand::from_object(object),
            updater_endpoints: updater_endpoints(object),
            yggdrasil: pick_string(
                object,
                &["yggdrasil", "yggdrasilServer", "yggdrasil_server"],
            )
            .filter(|url| url.starts_with("https://") || url.starts_with("http://")),
            extra: remaining(object, Self::KNOWN),
        })
    }
}

impl RemoteBrand {
    /// Accepts `{"name": …, "wordmark": {"prefix": …, "suffix": …}}` as well as
    /// a flat `{"name": …, "prefix": …, "suffix": …}`.
    fn from_object(object: &Map<String, Value>) -> Option<Self> {
        let brand = pick_object(object, &["brand"])?;
        let wordmark = pick_object(brand, &["wordmark"]).unwrap_or(brand);
        let parsed = Self {
            name: pick_string(brand, &["name", "title"]),
            prefix: pick_string(wordmark, &["prefix"]),
            suffix: pick_string(wordmark, &["suffix"]),
            subtitle: pick_string(brand, &["subtitle", "tagline"]),
            website: pick_string(brand, &["website", "url", "site"]),
            icon_url: pick_string(brand, &["iconUrl", "icon_url", "icon", "logo"])
                .filter(|url| url.starts_with("https://") || url.starts_with("http://")),
        };
        (parsed != Self::default()).then_some(parsed)
    }
}

/// `"updater": {"endpoints": [...]}`, `"updater": "https://…"` or a flat
/// `"updaterEndpoints": [...]`; only https URLs are kept.
fn updater_endpoints(object: &Map<String, Value>) -> Vec<String> {
    let mut found = match pick(object, &["updater"]) {
        Some(Value::Object(updater)) => {
            pick_string_list(updater, &["endpoints", "urls", "endpoint", "url"])
        }
        Some(Value::String(url)) => vec![url.trim().to_owned()],
        Some(Value::Array(_)) => pick_string_list(object, &["updater"]),
        _ => Vec::new(),
    };
    if found.is_empty() {
        found = pick_string_list(object, &["updaterEndpoints", "updater_endpoints"]);
    }
    found.retain(|url| url.starts_with("https://"));
    found
}

impl Link {
    /// Accepts an array of objects, an array of URLs or an object map.
    pub fn list_from_value(value: &Value) -> Vec<Self> {
        let items: Vec<&Value> = match value {
            Value::Array(items) => items.iter().collect(),
            Value::Object(map) => map.values().collect(),
            _ => Vec::new(),
        };
        let mut links: Vec<Self> = items
            .iter()
            .filter_map(|item| Self::from_value(item))
            .collect();
        links.sort_by_key(|link| link.order.unwrap_or(i64::MAX));
        links
    }

    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(url) => {
                let url = url.trim();
                if !is_web_url(url) {
                    return None;
                }
                Some(Self {
                    label: url.to_owned(),
                    url: url.to_owned(),
                    icon: None,
                    order: None,
                })
            }
            Value::Object(object) => {
                let url = pick_string(object, &["url", "link", "href"])?;
                if !is_web_url(&url) {
                    return None;
                }
                let icon = pick_string(object, &["icon", "type", "platform", "network", "kind"]);
                let label = pick_string(object, &["label", "name", "title"])
                    .or_else(|| icon.clone())
                    .unwrap_or_else(|| url.clone());
                Some(Self {
                    label,
                    url,
                    icon,
                    order: pick_i64(object, &["order", "position", "sort"]),
                })
            }
            _ => None,
        }
    }
}

pub fn is_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

impl Article {
    const KNOWN: &'static [&'static str] = &[
        "id",
        "title",
        "content",
        "body",
        "author",
        "publish_date",
        "publishDate",
        "published_at",
        "publishedAt",
        "date",
        "created_at",
        "createdAt",
        "image",
        "thumbnail",
        "banner",
        "cover",
        "url",
        "link",
        "order",
        "position",
        "priority",
    ];

    pub fn list_from_value(value: Value) -> Result<Vec<Self>, String> {
        let items: Vec<Value> = match value {
            Value::Array(items) => items,
            Value::Object(mut map) => match map.remove("articles").or_else(|| map.remove("data")) {
                Some(Value::Array(items)) => items,
                _ => return Err("articles response is not a list".to_owned()),
            },
            _ => return Err("articles response is not a list".to_owned()),
        };
        let mut articles: Vec<Self> = items
            .into_iter()
            .enumerate()
            .filter_map(|(index, item)| Self::from_value(item, index))
            .collect();
        articles.sort_by_key(|article| article.order.unwrap_or(i64::MAX));
        Ok(articles)
    }

    fn from_value(value: Value, index: usize) -> Option<Self> {
        let object = value.as_object()?;
        let title = pick_string(object, &["title"])?;
        Some(Self {
            id: pick_string(object, &["id"]).unwrap_or_else(|| format!("article-{index}")),
            title,
            content: pick_string(object, &["content", "body"]).unwrap_or_default(),
            author: pick_string(object, &["author"]),
            published_at: pick_string(
                object,
                &[
                    "publish_date",
                    "publishDate",
                    "published_at",
                    "publishedAt",
                    "date",
                    "created_at",
                    "createdAt",
                ],
            ),
            image: pick_string(object, &["image", "thumbnail", "banner", "cover"])
                .filter(|u| is_web_url(u)),
            url: pick_string(object, &["url", "link"]).filter(|u| is_web_url(u)),
            order: pick_i64(object, &["order", "position", "priority"]),
            extra: remaining(object, Self::KNOWN),
        })
    }
}

impl Instance {
    const KNOWN: &'static [&'static str] = &[
        "id",
        "name",
        "description",
        "image",
        "icon",
        "thumbnail",
        "banner",
        "url",
        "loader",
        "loadder",
        "verify",
        "ignored",
        "whitelist",
        "whitelistActive",
        "whitelist_active",
        "status",
        "java",
        "java_version",
        "javaVersion",
        "jvm_args",
        "jvmArgs",
        "memory",
        "order",
        "position",
        "enabled",
        "disabled",
    ];

    /// The panel answers `{ "<uuid>": {...}, ... }`; a list is accepted too.
    pub fn list_from_value(value: Value) -> Result<Vec<Self>, String> {
        let entries: Vec<(Option<String>, Value)> = match value {
            Value::Object(map) => map.into_iter().map(|(k, v)| (Some(k), v)).collect(),
            Value::Array(items) => items.into_iter().map(|v| (None, v)).collect(),
            _ => return Err("instances response is not an object".to_owned()),
        };
        let mut instances: Vec<Self> = entries
            .into_iter()
            .enumerate()
            .filter_map(|(index, (key, value))| Self::from_value(key, value, index))
            .collect();
        instances.sort_by(|a, b| {
            a.order
                .unwrap_or(i64::MAX)
                .cmp(&b.order.unwrap_or(i64::MAX))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(instances)
    }

    fn from_value(key: Option<String>, value: Value, index: usize) -> Option<Self> {
        let object = value.as_object()?;
        if pick_bool(object, &["enabled"]) == Some(false)
            || pick_bool(object, &["disabled"]) == Some(true)
        {
            return None;
        }
        let id = pick_string(object, &["id"])
            .or(key)
            .unwrap_or_else(|| format!("instance-{index}"));
        let name = pick_string(object, &["name"]).unwrap_or_else(|| id.clone());

        let loader_object = pick_object(object, &["loader", "loadder"]);
        let minecraft_version = loader_object
            .and_then(|l| pick_string(l, &["minecraft_version", "minecraftVersion", "version"]))
            .or_else(|| {
                pick_string(
                    object,
                    &["minecraft_version", "minecraftVersion", "version"],
                )
            })?;
        let loader_kind = loader_object
            .and_then(|l| pick_string(l, &["loader_type", "loadder_type", "type", "kind"]))
            .map(|kind| kind.to_ascii_lowercase())
            .unwrap_or_else(|| "none".to_owned());
        let loader_version = loader_object
            .and_then(|l| {
                pick_string(
                    l,
                    &["loader_version", "loadder_version", "build", "version"],
                )
            })
            .filter(|v| !v.eq_ignore_ascii_case("none"))
            .unwrap_or_else(|| "latest".to_owned());
        let mcp_file = loader_object.and_then(|l| pick_string(l, &["mcp_file", "mcpFile"]));

        let server = pick_object(object, &["status", "server"]).and_then(|status| {
            let host = pick_string(status, &["ip", "host", "address"])?;
            Some(InstanceServer {
                name: pick_string(status, &["nameServer", "name", "label"]),
                host,
                port: pick_u64(status, &["port"]).and_then(|p| u16::try_from(p).ok()),
            })
        });

        let java_version = pick_object(object, &["java"])
            .and_then(|java| pick_string(java, &["version", "major"]))
            .or_else(|| pick_string(object, &["java_version", "javaVersion"]));

        let memory = pick_object(object, &["memory"]).map(|memory| InstanceMemory {
            min_mb: pick(memory, &["min", "minMb", "min_mb"]).and_then(parse_memory_mb),
            max_mb: pick(memory, &["max", "maxMb", "max_mb"]).and_then(parse_memory_mb),
        });

        Some(Self {
            id,
            name,
            description: pick_string(object, &["description"]),
            image: pick_string(object, &["image", "icon", "thumbnail", "banner"])
                .filter(|u| is_web_url(u)),
            files_url: pick_string(object, &["url"]).filter(|u| is_web_url(u)),
            minecraft_version,
            loader: InstanceLoader {
                kind: loader_kind,
                version: loader_version,
                mcp_file,
            },
            verify: pick_bool(object, &["verify"]).unwrap_or(false),
            ignored: pick_string_list(object, &["ignored"]),
            whitelist: pick_string_list(object, &["whitelist"]),
            whitelist_active: pick_bool(object, &["whitelistActive", "whitelist_active"])
                .unwrap_or(false),
            server,
            java_version,
            jvm_args: pick_string_list(object, &["jvm_args", "jvmArgs"]),
            memory,
            order: pick_i64(object, &["order", "position"]),
            extra: remaining(object, Self::KNOWN),
        })
    }

    /// Whether `player` may see this instance.
    pub fn allows(&self, player: Option<&str>) -> bool {
        if !self.whitelist_active {
            return true;
        }
        match player {
            Some(name) => self.whitelist.iter().any(|entry| entry == name),
            None => false,
        }
    }
}

/// The panel's error shape (`{ success: false, status: "error", message }`).
#[derive(Debug, Deserialize)]
pub struct ApiFailure {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl ApiFailure {
    pub fn detect(value: &Value) -> Option<String> {
        let failure: ApiFailure = serde_json::from_value(value.clone()).ok()?;
        if failure.success == Some(false) || failure.status.as_deref() == Some("error") {
            Some(
                failure
                    .message
                    .unwrap_or_else(|| "unknown API error".to_owned()),
            )
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_real_config() {
        let config = RemoteConfig::from_value(json!({
            "maintenance": false,
            "maintenance_message": "Le serveur est en maintenance",
            "dataDirectory": "luuxis",
            "online": true,
            "client_id": "13f589e1",
            "socialLinks": []
        }))
        .unwrap();
        assert!(!config.maintenance);
        assert_eq!(config.data_directory.as_deref(), Some("luuxis"));
        assert_eq!(config.auth, AuthMode::Microsoft);
        assert_eq!(config.client_id.as_deref(), Some("13f589e1"));
        assert!(config.links.is_empty());
        // A panel that publishes none of the optional blocks stays valid.
        assert!(config.brand.is_none());
        assert!(config.updater_endpoints.is_empty());
        assert!(config.yggdrasil.is_none());
    }

    #[test]
    fn reads_the_optional_launcher_blocks() {
        let config = RemoteConfig::from_value(json!({
            "online": true,
            "brand": {
                "name": "LuuxCraft",
                "wordmark": { "prefix": "Luux", "suffix": "Craft" },
                "subtitle": "Launcher",
                "website": "https://luuxcraft.fr"
            },
            "updater": { "endpoints": ["https://luuxcraft.fr/launcher/latest.json", "http://insecure"] },
            "yggdrasil": "https://auth.luuxcraft.fr",
            "modules": { "news": true, "skins": false }
        }))
        .unwrap();
        let brand = config.brand.expect("brand");
        assert_eq!(brand.name.as_deref(), Some("LuuxCraft"));
        assert_eq!(brand.prefix.as_deref(), Some("Luux"));
        assert_eq!(brand.suffix.as_deref(), Some("Craft"));
        assert_eq!(brand.website.as_deref(), Some("https://luuxcraft.fr"));
        // Only https endpoints are kept.
        assert_eq!(
            config.updater_endpoints,
            vec!["https://luuxcraft.fr/launcher/latest.json"]
        );
        assert_eq!(config.yggdrasil.as_deref(), Some("https://auth.luuxcraft.fr"));
        assert_eq!(config.modules.get("skins"), Some(&json!(false)));
        // Modelled blocks never leak into `extra`.
        assert!(config.extra.is_empty());
    }

    #[test]
    fn accepts_a_flat_brand_and_a_single_updater_url() {
        let config = RemoteConfig::from_value(json!({
            "brand": { "prefix": "Mon", "suffix": "Serveur" },
            "updater": "https://example.com/latest.json"
        }))
        .unwrap();
        let brand = config.brand.expect("brand");
        assert_eq!(brand.prefix.as_deref(), Some("Mon"));
        assert_eq!(brand.name, None);
        assert_eq!(config.updater_endpoints, vec!["https://example.com/latest.json"]);
    }

    #[test]
    fn online_string_is_azauth() {
        let config = RemoteConfig::from_value(json!({ "online": "https://site.example" })).unwrap();
        assert_eq!(
            config.auth,
            AuthMode::AzAuth {
                url: "https://site.example".into()
            }
        );
        let offline = RemoteConfig::from_value(json!({ "online": false, "unknown": 1 })).unwrap();
        assert_eq!(offline.auth, AuthMode::Offline);
        assert_eq!(offline.extra.get("unknown"), Some(&json!(1)));
    }

    #[test]
    fn links_accept_many_shapes() {
        let links = Link::list_from_value(&json!([
            { "name": "Discord", "url": "https://discord.gg/x", "icon": "discord", "order": 2 },
            { "label": "Shop", "link": "https://shop.example", "order": 1 },
            "https://youtube.com/@x",
            { "url": "javascript:alert(1)" }
        ]));
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].label, "Shop");
        assert_eq!(links[1].icon.as_deref(), Some("discord"));
        assert_eq!(links[2].label, "https://youtube.com/@x");
    }

    #[test]
    fn parses_real_articles() {
        let articles = Article::list_from_value(json!([
            {"id":77,"title":"salut","content":"<h1>hello</h1>","author":"André","publish_date":"2026-07-03T09:03:00.000Z"}
        ]))
        .unwrap();
        assert_eq!(articles[0].id, "77");
        assert_eq!(articles[0].author.as_deref(), Some("André"));
        assert_eq!(
            articles[0].published_at.as_deref(),
            Some("2026-07-03T09:03:00.000Z")
        );
    }

    #[test]
    fn parses_real_instances() {
        let instances = Instance::list_from_value(json!({
            "2442be4f": {"name":"Shalltcraft","url":"https://luuxcraft.fr/api/user/x/instance/2442be4f","loader":{"minecraft_version":"1.21.1","loader_type":"neoforge","loader_version":"21.1.250","mcp_file":null},"verify":false,"ignored":["logs","options.txt"],"whitelist":[],"whitelistActive":false,"status":{"nameServer":"Shalltcraft","ip":"91.197.6.218","port":25638}},
            "18a8136c": {"name":"MITE 1.6.4","url":"https://x/instance/18a8136c","loader":{"minecraft_version":"1.6.4","loader_type":"mcp","loader_version":"none","mcp_file":"minecraft.jar"},"verify":false,"ignored":[],"whitelist":["Luuxis"],"whitelistActive":true,"status":{"nameServer":"","ip":"","port":null}},
            "legacy": {"name":"Old","url":"https://x/instance/legacy","loadder":{"minecraft_version":"1.12.2","loadder_type":"forge","loadder_version":"latest"}},
            "off": {"name":"Off","enabled":false,"loader":{"minecraft_version":"1.0"}}
        }))
        .unwrap();
        assert_eq!(instances.len(), 3);
        let mite = instances.iter().find(|i| i.name == "MITE 1.6.4").unwrap();
        assert_eq!(mite.loader.kind, "mcp");
        assert_eq!(mite.loader.version, "latest");
        assert_eq!(mite.loader.mcp_file.as_deref(), Some("minecraft.jar"));
        assert!(mite.server.is_none());
        assert!(mite.allows(Some("Luuxis")));
        assert!(!mite.allows(Some("Steve")));
        let shallt = instances.iter().find(|i| i.name == "Shalltcraft").unwrap();
        assert_eq!(shallt.server.as_ref().unwrap().port, Some(25638));
        assert_eq!(shallt.ignored.len(), 2);
        let legacy = instances.iter().find(|i| i.name == "Old").unwrap();
        assert_eq!(legacy.loader.kind, "forge");
        assert_eq!(legacy.minecraft_version, "1.12.2");
    }

    #[test]
    fn memory_values_are_normalized() {
        assert_eq!(parse_memory_mb(&json!("4G")), Some(4096));
        assert_eq!(parse_memory_mb(&json!("2048M")), Some(2048));
        assert_eq!(parse_memory_mb(&json!(6)), Some(6144));
        assert_eq!(parse_memory_mb(&json!(3072)), Some(3072));
    }

    #[test]
    fn detects_api_failures() {
        assert_eq!(
            ApiFailure::detect(&json!({"success": false, "message": "nope"})),
            Some("nope".into())
        );
        assert_eq!(ApiFailure::detect(&json!({"maintenance": false})), None);
        assert_eq!(ApiFailure::detect(&json!([])), None);
    }
}
