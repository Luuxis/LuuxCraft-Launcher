//! LuuxCraft panel API: client, tolerant models and the `remote_*` commands.
//!
//! The launcher keeps the last successful snapshot on disk so it can start
//! (and launch already-installed instances) without a connection.

pub mod client;
pub mod models;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub use client::LuuxCraftApi;
pub use models::{Article, AuthMode, Instance, Link, RemoteConfig, RemoteTheme};

/// Everything the frontend needs from the panel, fetched in one go.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSnapshot {
    pub config: RemoteConfig,
    pub instances: Vec<Instance>,
    pub articles: Vec<Article>,
    /// Panel links, or the central configuration fallback when it has none.
    pub links: Vec<Link>,
    /// Habillage publié par le propriétaire. Absent, l'interface garde sa
    /// mise en page et ses couleurs intégrées.
    #[serde(default)]
    pub theme: Option<RemoteTheme>,
    /// Errors of the optional parts (articles, instances) that did not block.
    pub partial_errors: Vec<PartialError>,
    /// Unix timestamp (seconds) of the fetch.
    pub fetched_at: u64,
    /// `true` when served from the disk cache because the panel is unreachable.
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialError {
    pub part: String,
    pub code: String,
    pub message: String,
}

impl RemoteSnapshot {
    pub fn instance(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|instance| instance.id == id)
    }
}

/// Fetches config, instances and articles concurrently. The config is
/// mandatory; a failure there falls back to the cached snapshot (marked stale)
/// or bubbles up when there is no cache yet.
pub async fn fetch_snapshot(state: &AppState) -> AppResult<RemoteSnapshot> {
    let api = state.api();
    let limit = state.config.news.limit;
    // Les quatre partent ensemble : le thème n'est pas plus lent que le reste,
    // et l'enchaîner après la configuration retarderait la première image de
    // tout un aller-retour.
    let (config, instances, articles, theme) = tokio::join!(
        api.config(),
        api.instances(),
        api.articles(limit),
        api.theme()
    );

    let config = match config {
        Ok(config) => config,
        Err(error) => {
            log::warn!("panel config unavailable: {error}");
            if let Some(mut cached) = state.cached_snapshot() {
                cached.stale = true;
                cached.partial_errors.push(PartialError {
                    part: "config".into(),
                    code: error.code.to_owned(),
                    message: error.message.clone(),
                });
                state.set_snapshot(cached.clone());
                return Ok(cached);
            }
            return Err(error);
        }
    };

    let mut partial_errors = Vec::new();
    let instances = match instances {
        Ok(instances) => instances,
        Err(error) => {
            log::warn!("panel instances unavailable: {error}");
            partial_errors.push(PartialError {
                part: "instances".into(),
                code: error.code.to_owned(),
                message: error.message,
            });
            state
                .cached_snapshot()
                .map(|cached| cached.instances)
                .unwrap_or_default()
        }
    };
    let articles = match articles {
        Ok(articles) => articles,
        Err(error) => {
            log::warn!("panel articles unavailable: {error}");
            partial_errors.push(PartialError {
                part: "articles".into(),
                code: error.code.to_owned(),
                message: error.message,
            });
            Vec::new()
        }
    };

    // Un thème injoignable n'est pas une panne : l'interface retombe sur sa
    // mise en page intégrée, et sur l'instantané en cache si l'on en a un.
    let theme = match theme {
        Ok(theme) => Some(theme),
        Err(error) => {
            log::warn!("panel theme unavailable: {error}");
            partial_errors.push(PartialError {
                part: "theme".into(),
                code: error.code.to_owned(),
                message: error.message,
            });
            state.cached_snapshot().and_then(|cached| cached.theme)
        }
    };

    // The panel is the only source of links (`socialLinks`).
    let links = config.links.clone();

    let snapshot = RemoteSnapshot {
        config,
        instances,
        articles,
        links,
        theme,
        partial_errors,
        fetched_at: crate::util::unix_now(),
        stale: false,
    };
    state.set_snapshot(snapshot.clone());
    state.persist_snapshot(&snapshot);
    state.remember_data_directory(snapshot.config.data_directory.as_deref());
    log::info!(
        "panel snapshot: {} instance(s), {} article(s), auth {:?}, maintenance {}, theme {}",
        snapshot.instances.len(),
        snapshot.articles.len(),
        snapshot.config.auth,
        snapshot.config.maintenance,
        match snapshot.theme.as_ref() {
            Some(theme) if theme.document.is_some() => "custom layout",
            Some(_) => "colours only",
            None => "built-in",
        }
    );
    Ok(snapshot)
}

/// Returns the snapshot in memory, or fetches it.
pub async fn ensure_snapshot(state: &AppState) -> AppResult<RemoteSnapshot> {
    if let Some(snapshot) = state.snapshot() {
        return Ok(snapshot);
    }
    fetch_snapshot(state).await
}

/// Le seul point d'entrée par lequel l'interface rafraîchit la configuration.
///
/// L'identité de la fenêtre ne s'y rejoue pas : elle vient du pack client posé
/// sur le disque, elle est déjà appliquée avant la première image et ne dépend
/// plus d'une réponse du panel (voir `branding`).
#[tauri::command]
pub async fn remote_fetch(state: State<'_, AppState>) -> AppResult<RemoteSnapshot> {
    fetch_snapshot(&state).await
}

#[tauri::command]
pub async fn remote_articles(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> AppResult<Vec<Article>> {
    let limit = limit.unwrap_or(state.config.news.limit).clamp(1, 100);
    state.api().articles(limit).await
}

#[tauri::command]
pub async fn remote_cached(state: State<'_, AppState>) -> AppResult<Option<RemoteSnapshot>> {
    Ok(state.snapshot().or_else(|| state.cached_snapshot()))
}

impl AppState {
    pub fn api(&self) -> LuuxCraftApi {
        LuuxCraftApi::new(self.http.inner().clone(), &self.config)
    }

    pub fn snapshot(&self) -> Option<RemoteSnapshot> {
        self.remote.lock().expect("remote mutex").clone()
    }

    pub fn set_snapshot(&self, snapshot: RemoteSnapshot) {
        *self.remote.lock().expect("remote mutex") = Some(snapshot);
    }

    pub fn cached_snapshot(&self) -> Option<RemoteSnapshot> {
        let raw = std::fs::read(self.paths.remote_cache_file()).ok()?;
        match serde_json::from_slice::<RemoteSnapshot>(&raw) {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                log::warn!("remote cache is unreadable, ignoring it: {error}");
                None
            }
        }
    }

    pub fn persist_snapshot(&self, snapshot: &RemoteSnapshot) {
        let path = self.paths.remote_cache_file();
        let result = std::fs::create_dir_all(&self.paths.cache_dir)
            .and_then(|_| serde_json::to_vec(snapshot).map_err(std::io::Error::other))
            .and_then(|json| std::fs::write(&path, json));
        if let Err(error) = result {
            log::warn!("could not write the remote cache: {error}");
        }
    }

    /// Returns the instance by id from the current snapshot.
    pub async fn instance(&self, id: &str) -> AppResult<Instance> {
        let snapshot = ensure_snapshot(self).await?;
        snapshot
            .instance(id)
            .cloned()
            .ok_or_else(|| AppError::new("instance_invalid", format!("unknown instance {id}")))
    }
}
