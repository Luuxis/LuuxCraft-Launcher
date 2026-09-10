//! Server status: a direct TCP ping through `crust_core::network::Status`.

use std::time::Duration;

use crust_core::network::status::DEFAULT_PORT;
use crust_core::network::Status;
use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatusDto {
    pub online: bool,
    pub host: String,
    pub port: u16,
    pub players: u64,
    pub max_players: u64,
    pub latency_ms: Option<u64>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub favicon: Option<String>,
    pub sample: Vec<String>,
    pub error: Option<String>,
    pub checked_at: u64,
}

/// Pings `host:port`. An unreachable server is a normal "offline" answer,
/// not an error; invalid input is.
#[tauri::command]
pub async fn server_status(
    state: State<'_, AppState>,
    host: String,
    port: Option<u16>,
) -> AppResult<ServerStatusDto> {
    let host = host.trim().to_owned();
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err(AppError::new("server_invalid", "invalid server host"));
    }
    let port = port.unwrap_or(DEFAULT_PORT);
    let timeout = Duration::from_millis(state.config.server_status.timeout_ms.clamp(500, 15_000));
    let checked_at = crate::util::unix_now();
    match Status::new(host.clone(), port)
        .with_timeout(timeout)
        .get_status()
        .await
    {
        Ok(status) => Ok(ServerStatusDto {
            online: !status.error,
            host,
            port,
            players: status.players_connect,
            max_players: status.players_max,
            latency_ms: Some(status.ms),
            version: (!status.version.is_empty()).then_some(status.version),
            description: status.description,
            favicon: status.favicon,
            sample: status.players.into_iter().map(|p| p.name).collect(),
            error: None,
            checked_at,
        }),
        Err(error) => {
            log::debug!("server {host}:{port} unreachable: {error}");
            Ok(ServerStatusDto {
                online: false,
                host,
                port,
                players: 0,
                max_players: 0,
                latency_ms: None,
                version: None,
                description: None,
                favicon: None,
                sample: Vec::new(),
                error: Some(error.to_string()),
                checked_at,
            })
        }
    }
}
