//! Skins and capes for the 3D viewer.
//!
//! Textures are fetched here (not in the webview) so cross-origin restrictions
//! never apply, then handed over as `data:` URLs. Microsoft textures are
//! cached on disk keyed by the account and the texture URL.

use std::path::PathBuf;

use base64::prelude::*;
use serde::Serialize;
use tauri::State;

use crate::accounts::AccountKind;
use crate::auth::ensure_fresh_account;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

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
}

/// Returns the textures of an account. `refresh` renews the profile first
/// (Microsoft accounts) so a skin changed on minecraft.net shows up.
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
    let summary = state
        .accounts
        .get(&uuid)
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
                Some(url) => fetch_texture(&state, &uuid, "skin", url, refresh).await?,
                None => None,
            };
            let cape = match &summary.cape_url {
                Some(url) => fetch_texture(&state, &uuid, "cape", url, refresh).await?,
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
                return Ok(Some(data_url(&bytes)));
            }
        }
    }

    log::debug!("downloading {kind} texture for {uuid}");
    let response = state
        .http
        .inner()
        .get(url)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await?;
    if !response.status().is_success() {
        log::warn!("{kind} texture answered status {}", response.status());
        return Ok(None);
    }
    let bytes = response.bytes().await?;
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        log::warn!("{kind} texture is not a PNG file, ignoring it");
        return Ok(None);
    }
    if let Err(error) = tokio::fs::create_dir_all(&cache_dir).await {
        log::debug!("skin cache unavailable: {error}");
    } else {
        let _ = tokio::fs::write(&png_path, &bytes).await;
        let _ = tokio::fs::write(&url_path, url).await;
    }
    Ok(Some(data_url(&bytes)))
}

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", BASE64_STANDARD.encode(bytes))
}
