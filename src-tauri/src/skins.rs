//! Skins: textures for the 3D viewer, the local skin library, and the
//! Minecraft Services calls that change the skin worn by an account.
//!
//! Textures are fetched here (not in the webview) so cross-origin restrictions
//! never apply, then handed over as `data:` URLs. Microsoft textures are
//! cached on disk keyed by the account and the texture URL.
//!
//! The library lives in the launcher directory (`skins/library.json` plus one
//! PNG per entry); it is user data, so it survives a cache wipe. Applying a
//! skin uploads the PNG to `api.minecraftservices.com` with the account's
//! Minecraft token — the same call the official launcher makes — and stores
//! the profile it returns.

use std::path::PathBuf;
use std::time::Duration;

use base64::prelude::*;
use crust_core::providers::mojang::Profile;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::accounts::AccountKind;
use crate::auth::ensure_fresh_account;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util;

const MINECRAFT_SERVICES: &str = "https://api.minecraftservices.com";
const LIBRARY_FILE: &str = "library.json";
const LIBRARY_VERSION: u32 = 1;
/// A Minecraft skin is a few kilobytes; anything larger is not one.
const MAX_SKIN_BYTES: usize = 512 * 1024;
const MAX_LIBRARY_ENTRIES: usize = 200;
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinData {
    pub uuid: String,
    pub name: String,
    /// `data:image/png;base64,...` or `None` (the UI draws a default skin).
    pub skin: Option<String>,
    pub cape: Option<String>,
    /// `default`, `slim` or `auto`.
    pub model: String,
    pub cape_alias: Option<String>,
    pub source: AccountKind,
    /// Whether this account can have its skin changed from the launcher.
    pub can_change: bool,
}

/// One entry of `library.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySkin {
    pub id: String,
    pub name: String,
    /// `classic` or `slim`.
    pub variant: String,
    /// The cape worn with this skin, `None` meaning "no cape". Entries saved
    /// before capes existed default to that.
    #[serde(default)]
    pub cape_id: Option<String>,
    pub added_at: u64,
}

/// A library entry with its texture, for the webview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub id: String,
    pub name: String,
    pub variant: String,
    pub cape_id: Option<String>,
    pub added_at: u64,
    /// `data:image/png;base64,...`
    pub texture: String,
}

/// A cape the account owns, ready to be shown in the picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapeOption {
    pub id: String,
    pub alias: Option<String>,
    /// `data:image/png;base64,...`
    pub texture: String,
    /// Whether the account is wearing it right now.
    pub active: bool,
}

impl LibraryEntry {
    fn new(skin: LibrarySkin, texture: String) -> Self {
        Self {
            id: skin.id,
            name: skin.name,
            variant: skin.variant,
            cape_id: skin.cape_id,
            added_at: skin.added_at,
            texture,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    skins: Vec<LibrarySkin>,
}

// ── Reading the textures of an account ──────────────────────────────────────

/// Returns the textures of an account. `refresh` renews the profile first
/// (Microsoft accounts) so a skin changed elsewhere shows up.
#[tauri::command]
pub async fn skin_get(
    state: State<'_, AppState>,
    uuid: String,
    refresh: bool,
) -> AppResult<SkinData> {
    if refresh {
        match ensure_fresh_account(&state, &uuid).await {
            Ok(_) => {}
            Err(error) if error.code == "auth_expired" => return Err(error),
            Err(error) => log::warn!("profile refresh skipped: {error}"),
        }
    }
    load_skin_data(&state, &uuid, refresh).await
}

async fn load_skin_data(state: &AppState, uuid: &str, force: bool) -> AppResult<SkinData> {
    let summary = state
        .accounts
        .get(uuid)
        .ok_or_else(|| AppError::new("account_unknown", "unknown account"))?;

    let model = match summary.skin_variant.as_deref() {
        Some("SLIM") => "slim",
        Some("CLASSIC") => "default",
        _ => "auto",
    }
    .to_owned();

    let (skin, cape) = match summary.kind {
        AccountKind::Microsoft => {
            let skin = match &summary.skin_url {
                Some(url) => fetch_texture(state, uuid, "skin", url, force).await?,
                None => None,
            };
            let cape = match &summary.cape_url {
                Some(url) => fetch_texture(state, uuid, "cape", url, force).await?,
                None => None,
            };
            (skin, cape)
        }
        AccountKind::AzAuth => (summary.skin_data_url.clone(), None),
        AccountKind::Offline | AccountKind::Yggdrasil => (None, None),
    };

    Ok(SkinData {
        uuid: summary.uuid,
        name: summary.name,
        skin,
        cape,
        model,
        cape_alias: summary.cape_alias,
        can_change: summary.kind == AccountKind::Microsoft,
        source: summary.kind,
    })
}

/// Downloads a texture as a data URL, through a small disk cache.
async fn fetch_texture(
    state: &AppState,
    uuid: &str,
    kind: &str,
    url: &str,
    force: bool,
) -> AppResult<Option<String>> {
    Ok(texture_bytes(state, uuid, kind, url, force)
        .await?
        .map(|bytes| data_url(&bytes)))
}

async fn texture_bytes(
    state: &AppState,
    uuid: &str,
    kind: &str,
    url: &str,
    force: bool,
) -> AppResult<Option<Vec<u8>>> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Ok(None);
    }
    let cache_dir = state.paths.skins_cache_dir();
    let png_path: PathBuf = cache_dir.join(format!("{uuid}-{kind}.png"));
    let url_path: PathBuf = cache_dir.join(format!("{uuid}-{kind}.url"));

    if !force {
        if let (Ok(cached_url), Ok(bytes)) = (
            tokio::fs::read_to_string(&url_path).await,
            tokio::fs::read(&png_path).await,
        ) {
            if cached_url == url && !bytes.is_empty() {
                return Ok(Some(bytes));
            }
        }
    }

    log::debug!("downloading {kind} texture for {uuid}");
    let response = state
        .http
        .inner()
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .await?;
    if !response.status().is_success() {
        log::warn!("{kind} texture answered status {}", response.status());
        return Ok(None);
    }
    let bytes = response.bytes().await?;
    if !is_png(&bytes) {
        log::warn!("{kind} texture is not a PNG file, ignoring it");
        return Ok(None);
    }
    if let Err(error) = tokio::fs::create_dir_all(&cache_dir).await {
        log::debug!("skin cache unavailable: {error}");
    } else {
        let _ = tokio::fs::write(&png_path, &bytes).await;
        let _ = tokio::fs::write(&url_path, url).await;
    }
    Ok(Some(bytes.to_vec()))
}

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", BASE64_STANDARD.encode(bytes))
}

// ── The skin library ────────────────────────────────────────────────────────

/// Lists the library, newest first. Entries whose PNG disappeared are dropped.
#[tauri::command]
pub fn skin_library_list(state: State<'_, AppState>) -> AppResult<Vec<LibraryEntry>> {
    let stored = read_library(&state);
    let total = stored.len();
    let mut entries = Vec::with_capacity(total);
    let mut alive = Vec::with_capacity(total);
    for skin in stored {
        match std::fs::read(texture_path(&state, &skin.id)) {
            Ok(bytes) if is_png(&bytes) => {
                entries.push(LibraryEntry::new(skin.clone(), data_url(&bytes)));
                alive.push(skin);
            }
            _ => log::warn!("library skin {} has no texture, dropping it", skin.id),
        }
    }
    if alive.len() != total {
        let _ = write_library(&state, &alive);
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.added_at));
    Ok(entries)
}

/// Reads a PNG the user picked and hands it back as a data URL, without
/// storing anything: the editor previews the file before it is saved.
#[tauri::command]
pub async fn skin_file_preview(path: String) -> AppResult<String> {
    let bytes = read_skin_file(&path).await?;
    Ok(data_url(&bytes))
}

/// Adds a PNG file picked by the user.
#[tauri::command]
pub async fn skin_library_import(
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
    variant: Option<String>,
    cape_id: Option<String>,
) -> AppResult<LibraryEntry> {
    let bytes = read_skin_file(&path).await?;
    let fallback = PathBuf::from(&path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "Skin".to_owned());
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or(fallback);
    store_in_library(&state, &bytes, &name, variant.as_deref(), cape_id)
}

async fn read_skin_file(path: &str) -> AppResult<Vec<u8>> {
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        AppError::new("skin_unreadable", format!("cannot read the file: {error}"))
            .with_details(path.to_owned())
    })?;
    validate_skin_png(&bytes)?;
    Ok(bytes)
}

/// Copies the skin currently worn by an account into the library.
#[tauri::command]
pub async fn skin_library_add_current(
    state: State<'_, AppState>,
    uuid: String,
    name: Option<String>,
) -> AppResult<LibraryEntry> {
    let summary = state
        .accounts
        .get(&uuid)
        .ok_or_else(|| AppError::new("account_unknown", "unknown account"))?;
    let bytes = match (&summary.skin_url, &summary.skin_data_url) {
        (Some(url), _) => texture_bytes(&state, &uuid, "skin", url, false).await?,
        (None, Some(data)) => decode_data_url(data),
        _ => None,
    }
    .ok_or_else(|| {
        AppError::new("skin_missing", "this account has no skin texture to copy")
    })?;
    let variant = match summary.skin_variant.as_deref() {
        Some("SLIM") => Some("slim"),
        _ => Some("classic"),
    };
    let name = name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| summary.name.clone());
    // The cape worn right now belongs with the skin being saved.
    let cape_id = active_cape_id(&state, &uuid);
    store_in_library(&state, &bytes, &name, variant, cape_id)
}

/// Saves the whole editable state of a library skin. `path` replaces its
/// texture when the user picked another file.
#[tauri::command]
pub async fn skin_library_update(
    state: State<'_, AppState>,
    id: String,
    name: String,
    variant: String,
    cape_id: Option<String>,
    path: Option<String>,
) -> AppResult<LibraryEntry> {
    check_id(&id)?;
    let replacement = match path {
        Some(path) => Some(read_skin_file(&path).await?),
        None => None,
    };
    let mut library = read_library(&state);
    let entry = library
        .iter_mut()
        .find(|skin| skin.id == id)
        .ok_or_else(|| AppError::new("skin_unknown", "unknown skin"))?;
    entry.name = clean_name(&name);
    entry.variant = normalize_variant(Some(&variant));
    entry.cape_id = clean_cape_id(cape_id);
    let updated = entry.clone();
    write_library(&state, &library)?;

    let bytes = match replacement {
        Some(bytes) => {
            std::fs::write(texture_path(&state, &id), &bytes)?;
            bytes
        }
        None => std::fs::read(texture_path(&state, &id))?,
    };
    Ok(LibraryEntry::new(updated, data_url(&bytes)))
}

#[tauri::command]
pub fn skin_library_remove(state: State<'_, AppState>, id: String) -> AppResult<()> {
    check_id(&id)?;
    let mut library = read_library(&state);
    let before = library.len();
    library.retain(|skin| skin.id != id);
    if library.len() == before {
        return Ok(());
    }
    write_library(&state, &library)?;
    let _ = std::fs::remove_file(texture_path(&state, &id));
    log::info!("library skin removed: {id}");
    Ok(())
}

fn store_in_library(
    state: &AppState,
    bytes: &[u8],
    name: &str,
    variant: Option<&str>,
    cape_id: Option<String>,
) -> AppResult<LibraryEntry> {
    validate_skin_png(bytes)?;
    let mut library = read_library(state);
    if library.len() >= MAX_LIBRARY_ENTRIES {
        return Err(AppError::new(
            "skin_library_full",
            format!("the library holds at most {MAX_LIBRARY_ENTRIES} skins"),
        ));
    }
    let entry = LibrarySkin {
        id: new_id(&library),
        name: clean_name(name),
        variant: normalize_variant(variant),
        cape_id: clean_cape_id(cape_id),
        added_at: util::unix_now(),
    };
    let dir = state.paths.skins_library_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{}.png", entry.id)), bytes)?;
    library.push(entry.clone());
    write_library(state, &library)?;
    log::info!("library skin added: {} ({})", entry.name, entry.variant);
    Ok(LibraryEntry::new(entry, data_url(bytes)))
}

fn read_library(state: &AppState) -> Vec<LibrarySkin> {
    let path = state.paths.skins_library_dir().join(LIBRARY_FILE);
    match std::fs::read(&path) {
        Ok(raw) => match serde_json::from_slice::<LibraryFile>(&raw) {
            Ok(file) => file.skins,
            Err(error) => {
                log::warn!("{} is unreadable, ignoring it: {error}", path.display());
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
}

fn write_library(state: &AppState, skins: &[LibrarySkin]) -> AppResult<()> {
    let dir = state.paths.skins_library_dir();
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(&LibraryFile {
        version: LIBRARY_VERSION,
        skins: skins.to_vec(),
    })?;
    let path = dir.join(LIBRARY_FILE);
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, &path)?;
    Ok(())
}

fn texture_path(state: &AppState, id: &str) -> PathBuf {
    state.paths.skins_library_dir().join(format!("{id}.png"))
}

/// Ids are generated here (hex), so an id coming back from the webview that is
/// not one cannot be turned into a path.
fn check_id(id: &str) -> AppResult<()> {
    if !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(());
    }
    Err(AppError::new("skin_unknown", "invalid skin id"))
}

fn new_id(existing: &[LibrarySkin]) -> String {
    let base = util::unix_now();
    let mut suffix = 0u32;
    loop {
        let id = if suffix == 0 {
            format!("{base:x}")
        } else {
            format!("{base:x}{suffix:x}")
        };
        if !existing.iter().any(|skin| skin.id == id) {
            return id;
        }
        suffix += 1;
    }
}

fn clean_name(name: &str) -> String {
    let cleaned: String = name.trim().chars().filter(|c| !c.is_control()).take(40).collect();
    if cleaned.is_empty() {
        "Skin".to_owned()
    } else {
        cleaned
    }
}

/// Cape ids come from Mojang (hex or UUID) and end up in a file name and a
/// URL: anything else is treated as "no cape".
fn clean_cape_id(cape_id: Option<String>) -> Option<String> {
    cape_id.filter(|id| {
        !id.is_empty()
            && id.len() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

fn active_cape_id(state: &AppState, uuid: &str) -> Option<String> {
    let account = state.accounts.load_account(uuid).ok()?;
    account
        .profile
        .capes
        .iter()
        .find(|cape| cape.state.as_deref() == Some("ACTIVE"))
        .and_then(|cape| cape.id.clone())
}

fn normalize_variant(variant: Option<&str>) -> String {
    match variant.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("slim") | Some("alex") => "slim".to_owned(),
        _ => "classic".to_owned(),
    }
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[..8] == b"\x89PNG\r\n\x1a\n"
}

/// Reads the width/height out of the IHDR chunk, which a PNG always starts
/// with — enough to reject anything that is not a skin, without an image crate.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if !is_png(bytes) || bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

fn validate_skin_png(bytes: &[u8]) -> AppResult<()> {
    if bytes.len() > MAX_SKIN_BYTES {
        return Err(AppError::new(
            "skin_invalid",
            "this file is too large to be a Minecraft skin",
        ));
    }
    match png_size(bytes) {
        // 64×32 is the legacy layout, still accepted by Minecraft.
        Some((64, 64)) | Some((64, 32)) => Ok(()),
        Some((width, height)) => Err(AppError::new(
            "skin_invalid",
            format!("a skin must be 64×64 (or 64×32), this one is {width}×{height}"),
        )),
        None => Err(AppError::new("skin_invalid", "this file is not a PNG image")),
    }
}

fn decode_data_url(value: &str) -> Option<Vec<u8>> {
    let encoded = value.split_once("base64,").map(|(_, data)| data)?;
    let bytes = BASE64_STANDARD.decode(encoded).ok()?;
    is_png(&bytes).then_some(bytes)
}

// ── Capes ───────────────────────────────────────────────────────────────────

/// The capes the account owns, with their textures, for the picker.
#[tauri::command]
pub async fn skin_capes_list(
    state: State<'_, AppState>,
    uuid: String,
) -> AppResult<Vec<CapeOption>> {
    let summary = state
        .accounts
        .get(&uuid)
        .ok_or_else(|| AppError::new("account_unknown", "unknown account"))?;
    if summary.kind != AccountKind::Microsoft {
        return Ok(Vec::new());
    }
    let account = state.accounts.load_account(&uuid)?;
    let mut options = Vec::with_capacity(account.profile.capes.len());
    for cape in &account.profile.capes {
        let Some(id) = clean_cape_id(cape.id.clone()) else {
            continue;
        };
        let Some(bytes) = texture_bytes(&state, &uuid, &format!("cape-{id}"), &cape.url, false)
            .await?
        else {
            continue;
        };
        options.push(CapeOption {
            active: cape.state.as_deref() == Some("ACTIVE"),
            alias: cape.alias.clone(),
            texture: data_url(&bytes),
            id,
        });
    }
    Ok(options)
}

// ── Changing the skin worn by the account ───────────────────────────────────

/// Uploads a library skin to the Minecraft profile of the account, then puts
/// on the cape saved with it (`None` meaning "no cape", as in the editor).
#[tauri::command]
pub async fn skin_apply(
    state: State<'_, AppState>,
    uuid: String,
    id: String,
) -> AppResult<SkinData> {
    check_id(&id)?;
    let entry = read_library(&state)
        .into_iter()
        .find(|skin| skin.id == id)
        .ok_or_else(|| AppError::new("skin_unknown", "unknown skin"))?;
    let bytes = std::fs::read(texture_path(&state, &id))?;
    validate_skin_png(&bytes)?;

    let token = minecraft_token(&state, &uuid).await?;
    let boundary = format!("----luuxcraft{:x}", util::unix_now());
    let response = state
        .http
        .inner()
        .post(format!("{MINECRAFT_SERVICES}/minecraft/profile/skins"))
        .bearer_auth(&token)
        .header("Accept", "application/json")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .timeout(UPLOAD_TIMEOUT)
        .body(multipart_body(&boundary, &entry.variant, &bytes))
        .send()
        .await?;

    let mut profile = read_profile(&state, &uuid, response).await?;
    log::info!("skin '{}' applied to {}", entry.name, profile.name);

    // Only call Mojang when the cape actually changes: an account without any
    // cape has nothing to take off.
    let worn = profile
        .capes
        .iter()
        .find(|cape| cape.state.as_deref() == Some("ACTIVE"))
        .and_then(|cape| cape.id.clone());
    if worn != entry.cape_id {
        profile = set_cape(&state, &uuid, &token, entry.cape_id.as_deref()).await?;
    }

    store_profile(&state, &uuid, profile)?;
    load_skin_data(&state, &uuid, true).await
}

/// Puts on a cape, or takes the current one off when `cape_id` is `None`.
async fn set_cape(
    state: &AppState,
    uuid: &str,
    token: &str,
    cape_id: Option<&str>,
) -> AppResult<Profile> {
    let url = format!("{MINECRAFT_SERVICES}/minecraft/profile/capes/active");
    let http = state.http.inner();
    let request = match cape_id {
        Some(cape_id) => http
            .put(url)
            .json(&serde_json::json!({ "capeId": cape_id })),
        None => http.delete(url),
    };
    let response = request
        .bearer_auth(token)
        .header("Accept", "application/json")
        .timeout(UPLOAD_TIMEOUT)
        .send()
        .await?;
    log::info!(
        "cape {} for {uuid}",
        cape_id.map(|id| format!("set to {id}")).unwrap_or_else(|| "removed".to_owned())
    );
    read_profile(state, uuid, response).await
}

/// Puts the account back on its default Minecraft skin.
#[tauri::command]
pub async fn skin_reset(state: State<'_, AppState>, uuid: String) -> AppResult<SkinData> {
    let token = minecraft_token(&state, &uuid).await?;
    let response = state
        .http
        .inner()
        .delete(format!("{MINECRAFT_SERVICES}/minecraft/profile/skins/active"))
        .bearer_auth(&token)
        .header("Accept", "application/json")
        .timeout(UPLOAD_TIMEOUT)
        .send()
        .await?;

    let profile = read_profile(&state, &uuid, response).await?;
    log::info!("skin reset for {}", profile.name);
    store_profile(&state, &uuid, profile)?;
    load_skin_data(&state, &uuid, true).await
}

/// The Minecraft token of a Microsoft account, renewed first so the upload is
/// not refused for an expired session.
async fn minecraft_token(state: &AppState, uuid: &str) -> AppResult<String> {
    let summary = state
        .accounts
        .get(uuid)
        .ok_or_else(|| AppError::new("account_unknown", "unknown account"))?;
    if summary.kind != AccountKind::Microsoft {
        return Err(AppError::new(
            "skin_not_supported",
            "only Microsoft accounts can change their skin from the launcher",
        ));
    }
    let account = ensure_fresh_account(state, uuid).await?;
    Ok(account.access_token)
}

fn multipart_body(boundary: &str, variant: &str, png: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(png.len() + 256);
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// Maps a Minecraft Services answer to the updated profile, or to an error the
/// interface can explain.
async fn read_profile(
    state: &AppState,
    uuid: &str,
    response: reqwest::Response,
) -> AppResult<Profile> {
    let status = response.status();
    if status.is_success() {
        return response
            .json::<Profile>()
            .await
            .map_err(|error| AppError::new("api_invalid", error.to_string()));
    }
    let body = response.text().await.unwrap_or_default();
    log::warn!("minecraft services refused the skin change (status {status}): {body}");
    let error = match status.as_u16() {
        401 | 403 => {
            state.accounts.set_needs_reauth(uuid, true);
            AppError::new("auth_expired", "the Minecraft session was refused")
        }
        413 => AppError::new("skin_invalid", "the skin file is too large"),
        415 | 400 => AppError::new("skin_rejected", "Minecraft refused this skin file"),
        429 => AppError::new("api_rate_limited", "too many skin changes, wait a moment"),
        status if status >= 500 => {
            AppError::new("api_unavailable", "the Minecraft servers are unavailable")
        }
        _ => AppError::new("api_invalid", format!("unexpected status {status}")),
    };
    Err(error.with_details(truncate(&body, 300)))
}

/// Stores the profile returned by the skin change, so the interface and the
/// next launch see the new texture without another round trip.
fn store_profile(state: &AppState, uuid: &str, profile: Profile) -> AppResult<()> {
    let mut account = state.accounts.load_account(uuid)?;
    account.profile.skins = profile.skins;
    account.profile.capes = profile.capes;
    state.accounts.upsert(account)?;
    Ok(())
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        text.chars().take(max).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64×64 PNG header is enough for the validation: only IHDR is read.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    #[test]
    fn accepts_both_skin_layouts_only() {
        assert!(validate_skin_png(&png_header(64, 64)).is_ok());
        assert!(validate_skin_png(&png_header(64, 32)).is_ok());
        let wrong = validate_skin_png(&png_header(128, 128)).unwrap_err();
        assert_eq!(wrong.code, "skin_invalid");
        assert_eq!(
            validate_skin_png(b"not a png at all").unwrap_err().code,
            "skin_invalid"
        );
    }

    #[test]
    fn normalizes_the_variant() {
        assert_eq!(normalize_variant(Some("SLIM")), "slim");
        assert_eq!(normalize_variant(Some(" slim ")), "slim");
        assert_eq!(normalize_variant(Some("classic")), "classic");
        assert_eq!(normalize_variant(Some("whatever")), "classic");
        assert_eq!(normalize_variant(None), "classic");
    }

    #[test]
    fn rejects_ids_that_are_not_ours() {
        assert!(check_id("1a2b3c").is_ok());
        assert!(check_id("../../etc/passwd").is_err());
        assert!(check_id("").is_err());
        assert!(check_id("a/b").is_err());
    }

    #[test]
    fn builds_a_multipart_body_minecraft_accepts() {
        // ASCII payload so the body can be read back as text; the real bytes
        // are copied verbatim either way.
        let body = multipart_body("BOUND", "slim", b"PNG-data");
        let text = String::from_utf8(body).expect("ascii body");
        assert!(text.starts_with("--BOUND\r\nContent-Disposition: form-data; name=\"variant\""));
        assert!(text.contains("\r\n\r\nslim\r\n"));
        assert!(text.contains("name=\"file\"; filename=\"skin.png\""));
        assert!(text.contains("Content-Type: image/png\r\n\r\nPNG-data\r\n"));
        assert!(text.ends_with("--BOUND--\r\n"));
    }

    #[test]
    fn keeps_names_printable_and_short() {
        assert_eq!(clean_name("  Pirate  "), "Pirate");
        assert_eq!(clean_name(""), "Skin");
        assert_eq!(clean_name("a\nb"), "ab");
        assert_eq!(clean_name(&"x".repeat(80)).chars().count(), 40);
    }

    #[test]
    fn generates_ids_that_do_not_collide() {
        let first = new_id(&[]);
        let taken = vec![LibrarySkin {
            id: first.clone(),
            name: "One".into(),
            variant: "classic".into(),
            cape_id: None,
            added_at: 0,
        }];
        assert_ne!(new_id(&taken), first);
    }

    #[test]
    fn keeps_only_cape_ids_mojang_could_have_issued() {
        assert_eq!(clean_cape_id(Some("1981aad3".into())).as_deref(), Some("1981aad3"));
        assert_eq!(
            clean_cape_id(Some("9ad0b5f6-9f89-4eb2-9f3a-1d0b36d4d8f5".into())).as_deref(),
            Some("9ad0b5f6-9f89-4eb2-9f3a-1d0b36d4d8f5")
        );
        assert_eq!(clean_cape_id(Some("../../etc".into())), None);
        assert_eq!(clean_cape_id(Some(String::new())), None);
        assert_eq!(clean_cape_id(None), None);
    }
}
