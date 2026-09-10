//! HTTP client of the LuuxCraft panel API.
//!
//! Documented routes (<https://luuxcraft.fr/docs>) and the routes consumed by
//! the reference launchers: `config`, `articles?limit=`, `instances` under
//! `{baseUrl}/user/{userId}`. Every response is validated before use.

use std::time::Duration;

use serde_json::Value;

use super::models::{ApiFailure, Article, Instance, RemoteConfig};
use crate::config::LauncherConfig;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct LuuxCraftApi {
    http: reqwest::Client,
    base_url: String,
    timeout: Duration,
}

impl LuuxCraftApi {
    pub fn new(http: reqwest::Client, config: &LauncherConfig) -> Self {
        Self {
            http,
            base_url: config.user_api_url(),
            timeout: Duration::from_secs(config.api.timeout_seconds.clamp(3, 120)),
        }
    }

    pub async fn config(&self) -> AppResult<RemoteConfig> {
        let value = self.get_json(&format!("{}/config", self.base_url)).await?;
        RemoteConfig::from_value(value).map_err(|reason| AppError::new("api_invalid", reason))
    }

    pub async fn articles(&self, limit: u32) -> AppResult<Vec<Article>> {
        let value = self
            .get_json(&format!("{}/articles?limit={limit}", self.base_url))
            .await?;
        Article::list_from_value(value).map_err(|reason| AppError::new("api_invalid", reason))
    }

    pub async fn instances(&self) -> AppResult<Vec<Instance>> {
        let value = self
            .get_json(&format!("{}/instances", self.base_url))
            .await?;
        Instance::list_from_value(value).map_err(|reason| AppError::new("api_invalid", reason))
    }

    async fn get_json(&self, url: &str) -> AppResult<Value> {
        log::debug!("api GET {url}");
        let response = self
            .http
            .get(url)
            .header("Accept", "application/json")
            .timeout(self.timeout)
            .send()
            .await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        let value: Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(error) => {
                log::warn!("api {url} answered status {status} with a non-JSON body");
                let code = if status >= 500 {
                    "api_unavailable"
                } else if status == 404 {
                    "api_not_found"
                } else if status == 429 {
                    "api_rate_limited"
                } else {
                    "api_invalid"
                };
                return Err(AppError::new(code, format!("status {status}: {error}")));
            }
        };
        if let Some(message) = ApiFailure::detect(&value) {
            return Err(AppError::new("api_error", message));
        }
        if !(200..300).contains(&status) {
            let code = match status {
                404 => "api_not_found",
                429 => "api_rate_limited",
                500..=599 => "api_unavailable",
                _ => "api_invalid",
            };
            return Err(AppError::new(code, format!("status {status}")));
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api() -> LuuxCraftApi {
        let config = LauncherConfig::load().expect("valid config");
        let http = crust_core::network::HttpClient::new().expect("http client");
        LuuxCraftApi::new(http.inner().clone(), &config)
    }

    /// Talks to the real panel: `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn fetches_the_real_panel() {
        let api = api();
        let config = api.config().await.expect("config");
        assert!(config.data_directory.is_some());
        let instances = api.instances().await.expect("instances");
        assert!(!instances.is_empty());
        for instance in &instances {
            assert!(
                !instance.minecraft_version.is_empty(),
                "{} has a version",
                instance.name
            );
            assert!(
                instance.files_url.is_some(),
                "{} has a files url",
                instance.name
            );
        }
        let articles = api.articles(5).await.expect("articles");
        assert!(articles.len() <= 5);
    }

    #[tokio::test]
    #[ignore]
    async fn unknown_user_is_not_found() {
        let mut config = LauncherConfig::load().expect("valid config");
        config.user_id = "00000000-0000-0000-0000-000000000000".into();
        let http = crust_core::network::HttpClient::new().expect("http client");
        let api = LuuxCraftApi::new(http.inner().clone(), &config);
        let error = api.config().await.expect_err("must fail");
        assert!(
            matches!(error.code, "api_not_found" | "api_invalid" | "api_error"),
            "{error}"
        );
    }
}
