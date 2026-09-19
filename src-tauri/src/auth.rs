//! Sign-in flows and account commands.
//!
//! Microsoft uses the **authorization code** flow of `crust_core` in a
//! dedicated login window, with the panel's Azure `client_id` on
//! `login.live.com` exactly like the previous Electron launcher (the official
//! launcher id when the panel publishes none). The device code flow is not
//! offered: the panel's Azure application refuses it (`AADSTS70002`, public
//! client flows disabled).
//!
//! Azuriom goes through AZauth with its 2FA step, and offline/Yggdrasil
//! accounts through `Yggdrasil`. The panel's `online` field decides which
//! method is on. Sessions are then renewed automatically (see `sessions`).
//!
//! ## Un jeton de rafraîchissement appartient à son application Azure
//!
//! Microsoft n'accepte un `refresh_token` qu'avec le `client_id` qui l'a
//! émis : un jeton obtenu sous l'application du panel est refusé sous celle du
//! launcher officiel, et inversement (`invalid_grant`, « issued for a different
//! client id »). Or l'application peut changer entre deux démarrages — le
//! tenant en publie une nouvelle, ou n'en publie plus — sans que les sessions
//! déjà rangées ne perdent rien de leur validité. Chaque compte retient donc
//! l'identifiant sous lequel sa session a été obtenue (`Account::client_id`),
//! et le renouvellement l'essaie **en premier**, avant celui du panel et celui
//! du launcher officiel. Sans cela, changer d'application déconnectait tous les
//! comptes sauf le dernier ajouté.

use std::sync::Arc;

use crust_core::authenticator::{Account, Authenticator, AzAuth, AzAuthLogin, Yggdrasil};
use crust_core::providers::microsoft::{Authority, MicrosoftOAuth, DEFAULT_CLIENT_ID};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::Notify;

use crate::accounts::{AccountKind, AccountSummary};
use crate::api::{self, AuthMode};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Label of the Microsoft login window.
pub const LOGIN_WINDOW: &str = "microsoft-login";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum AuthEvent {
    /// The login window is open, waiting for the user.
    WindowOpened,
    /// The code came back, the account is being resolved.
    Waiting,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AzAuthOutcome {
    Ok { account: Box<AccountSummary> },
    OtpRequired,
}

/// Sign-in methods enabled for this launcher, from the panel config plus the
/// optional Yggdrasil server of the central configuration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthMethods {
    pub microsoft: bool,
    pub azauth: Option<String>,
    pub offline: bool,
    pub yggdrasil: Option<String>,
}

/// Authenticator for the login window: the panel client id on the Live
/// authority (desktop redirect), or the official launcher client id.
fn window_authenticator(state: &AppState, client_id: Option<&str>) -> AppResult<Authenticator> {
    authenticator_for(state, client_id.unwrap_or(DEFAULT_CLIENT_ID))
}

/// Authenticator for one Azure application id. The official launcher id goes
/// through `crust_core`'s own configuration; any other id is a panel
/// application on the Live authority, like the login window.
fn authenticator_for(state: &AppState, client_id: &str) -> AppResult<Authenticator> {
    let client_id = client_id.trim();
    if client_id.is_empty() || client_id == DEFAULT_CLIENT_ID {
        return Authenticator::new().map_err(AppError::from);
    }
    Ok(Authenticator::with_oauth(
        state.http.clone(),
        MicrosoftOAuth::new(state.http.clone(), client_id, Authority::Live),
    ))
}

/// The application ids a Microsoft session may have been issued under, most
/// likely first: the one recorded on the account, the panel's current one,
/// then the official launcher id. Without duplicates.
fn refresh_client_ids(state: &AppState, account: &Account) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let candidates = [
        account.client_id.clone(),
        panel_client_id(state),
        Some(DEFAULT_CLIENT_ID.to_owned()),
    ];
    for candidate in candidates.into_iter().flatten() {
        let candidate = candidate.trim().to_owned();
        if !candidate.is_empty() && !ids.contains(&candidate) {
            ids.push(candidate);
        }
    }
    ids
}

/// Renews a Microsoft session, trying each application id in turn.
///
/// Only an authentication refusal moves on to the next id: a network error
/// says nothing about the session and is returned as is. When the refresh
/// token was actually exercised, the id that worked is recorded on the
/// account so the next renewal goes straight to it.
async fn refresh_microsoft(
    state: &AppState,
    account: &Account,
    client_ids: &[String],
) -> AppResult<Account> {
    let mut first_refusal: Option<AppError> = None;
    for client_id in client_ids {
        match authenticator_for(state, client_id)?.refresh(account).await {
            Ok(mut fresh) => {
                if fresh.access_token != account.access_token {
                    if account.client_id.as_deref() != Some(client_id.as_str()) {
                        log::info!(
                            "session of {} renewed under client id {client_id}",
                            account.name
                        );
                    }
                    fresh.client_id = Some(client_id.clone());
                }
                return Ok(fresh);
            }
            Err(error) => {
                let mapped = AppError::from(error);
                if !matches!(mapped.code, "auth_expired" | "auth_denied") {
                    return Err(mapped);
                }
                log::debug!(
                    "session of {} refused under client id {client_id}: {mapped}",
                    account.name
                );
                first_refusal.get_or_insert(mapped);
            }
        }
    }
    Err(first_refusal.unwrap_or_else(|| {
        AppError::new("auth_expired", "no application id to renew the session with")
    }))
}

async fn auth_methods(state: &AppState) -> AppResult<AuthMethods> {
    let snapshot = api::ensure_snapshot(state).await?;
    let mut methods = AuthMethods {
        microsoft: false,
        azauth: None,
        offline: false,
        yggdrasil: yggdrasil_server(state),
    };
    match snapshot.config.auth {
        AuthMode::Microsoft => methods.microsoft = true,
        AuthMode::Offline => methods.offline = true,
        AuthMode::AzAuth { url } => methods.azauth = Some(url),
    }
    Ok(methods)
}

/// The Yggdrasil-compatible server: the panel's if it publishes one, the
/// built-in constant otherwise.
fn yggdrasil_server(state: &AppState) -> Option<String> {
    panel_config(state)
        .and_then(|config| config.yggdrasil)
        .or_else(|| state.config.auth.yggdrasil_server.clone())
}

/// The panel configuration from memory, or from the disk cache when the panel
/// is unreachable. `None` means "we do not know it yet".
fn panel_config(state: &AppState) -> Option<crate::api::RemoteConfig> {
    state
        .snapshot()
        .or_else(|| state.cached_snapshot())
        .map(|snapshot| snapshot.config)
}

fn panel_client_id(state: &AppState) -> Option<String> {
    panel_config(state)
        .and_then(|config| config.client_id)
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
}

#[tauri::command]
pub async fn auth_methods_get(state: State<'_, AppState>) -> AppResult<AuthMethods> {
    auth_methods(&state).await
}

/// Microsoft sign-in in a dedicated login window (authorization code flow).
/// The redirect to `oauth20_desktop.srf` is intercepted, the window closed,
/// and the code exchanged for an account. Closing the window cancels.
#[tauri::command]
pub async fn auth_microsoft_window_login(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<AuthEvent>,
) -> AppResult<AccountSummary> {
    let methods = auth_methods(&state).await?;
    if !methods.microsoft {
        return Err(AppError::new(
            "auth_method_disabled",
            "Microsoft sign-in is not enabled",
        ));
    }
    let client_id = panel_client_id(&state);
    let auth = window_authenticator(&state, client_id.as_deref())?;
    let request = auth.authorize_request();
    let authorize_url = url::Url::parse(&request.url)
        .map_err(|error| AppError::new("internal", format!("invalid authorize url: {error}")))?;
    log::info!(
        "microsoft login window ({} client id, redirect {})",
        if client_id.is_some() {
            "panel"
        } else {
            "official"
        },
        request.redirect_uri
    );

    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = existing.destroy();
    }

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Option<String>>();
    let redirect_prefix = request.redirect_uri.clone();
    let navigation_sender = sender.clone();
    let window = WebviewWindowBuilder::new(&app, LOGIN_WINDOW, WebviewUrl::External(authorize_url))
        .title("Connexion Microsoft")
        .inner_size(520.0, 720.0)
        .center()
        .resizable(true)
        .zoom_hotkeys_enabled(false)
        // Fresh session every time so another Microsoft account can be added.
        .incognito(true)
        // Même profil que la fenêtre principale : WebView2 refuse deux dossiers
        // de données utilisateur différents dans un même processus, et la
        // fenêtre principale en pose déjà un, scopé au tenant.
        .data_directory(state.paths.webview_dir.clone())
        .on_navigation(move |url| {
            if url.as_str().starts_with(&redirect_prefix) {
                let _ = navigation_sender.send(Some(url.to_string()));
                return false;
            }
            true
        })
        .build()
        .map_err(|error| {
            AppError::new("internal", format!("cannot open the login window: {error}"))
        })?;
    let close_sender = sender.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let _ = close_sender.send(None);
        }
    });
    let _ = on_event.send(AuthEvent::WindowOpened);

    let cancel = Arc::new(Notify::new());
    state.set_auth_cancel(Some(cancel.clone()));
    let outcome = tokio::select! {
        received = receiver.recv() => received.flatten(),
        _ = cancel.notified() => None,
    };
    state.set_auth_cancel(None);
    let _ = window.destroy();

    let Some(redirect_url) = outcome else {
        log::info!("microsoft login window closed before completion");
        return Err(AppError::cancelled());
    };
    let _ = on_event.send(AuthEvent::Waiting);
    let mut account = request.login(&auth, &redirect_url).await?;
    // The refresh token is bound to this application: remember which one, so
    // the session can still be renewed once the panel publishes another.
    account.client_id = Some(auth.microsoft().oauth().client_id().to_owned());
    log::info!("microsoft sign-in complete for {}", account.name);
    store_and_select(&state, account)
}

/// Cancels the Microsoft sign-in in progress (closes the login window).
#[tauri::command]
pub fn auth_cancel(app: AppHandle, state: State<'_, AppState>) {
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = window.destroy();
    }
    if let Some(cancel) = state.take_auth_cancel() {
        cancel.notify_waiters();
        cancel.notify_one();
        log::info!("sign-in cancelled by the user");
    }
}

/// Azuriom (AZauth) sign-in with optional one-time code.
#[tauri::command]
pub async fn auth_azauth_login(
    state: State<'_, AppState>,
    email: String,
    password: String,
    code: Option<String>,
) -> AppResult<AzAuthOutcome> {
    let methods = auth_methods(&state).await?;
    let Some(site) = methods.azauth else {
        return Err(AppError::new(
            "auth_method_disabled",
            "AZauth sign-in is not enabled",
        ));
    };
    let email = email.trim();
    if email.is_empty() || password.is_empty() {
        return Err(AppError::new(
            "auth_invalid_credentials",
            "email and password are required",
        ));
    }
    let code = code.map(|c| c.trim().to_owned()).filter(|c| !c.is_empty());
    let client = AzAuth::new(state.http.clone(), &site)?;
    match client.login(email, &password, code.as_deref()).await? {
        AzAuthLogin::TwoFactorRequired => {
            log::info!("azauth requires a one-time code");
            Ok(AzAuthOutcome::OtpRequired)
        }
        AzAuthLogin::Account(account) => {
            log::info!("azauth sign-in complete for {}", account.name);
            let summary = store_and_select(&state, *account)?;
            Ok(AzAuthOutcome::Ok {
                account: Box::new(summary),
            })
        }
    }
}

#[tauri::command]
pub async fn auth_offline_login(
    state: State<'_, AppState>,
    username: String,
) -> AppResult<AccountSummary> {
    let methods = auth_methods(&state).await?;
    if !methods.offline {
        return Err(AppError::new(
            "auth_method_disabled",
            "offline accounts are not enabled",
        ));
    }
    let username = username.trim();
    if !is_valid_username(username) {
        return Err(AppError::new(
            "auth_invalid_username",
            "username must be 3 to 16 letters, digits or underscores",
        ));
    }
    let account = Yggdrasil::offline(username);
    log::info!("offline account created for {username}");
    store_and_select(&state, account)
}

/// Sign-in against a Yggdrasil-compatible server (only when configured).
#[tauri::command]
pub async fn auth_yggdrasil_login(
    state: State<'_, AppState>,
    username: String,
    password: String,
) -> AppResult<AccountSummary> {
    let Some(server) = yggdrasil_server(&state) else {
        return Err(AppError::new(
            "auth_method_disabled",
            "no Yggdrasil server is configured",
        ));
    };
    let username = username.trim();
    if username.is_empty() || password.is_empty() {
        return Err(AppError::new(
            "auth_invalid_credentials",
            "username and password are required",
        ));
    }
    let client = Yggdrasil::with_api(state.http.clone(), server);
    let account = client.login(username, Some(&password)).await?;
    log::info!("yggdrasil sign-in complete for {}", account.name);
    store_and_select(&state, account)
}

#[tauri::command]
pub fn accounts_list(state: State<'_, AppState>) -> Vec<AccountSummary> {
    state.accounts.list()
}

#[tauri::command]
pub fn accounts_select(state: State<'_, AppState>, uuid: String) -> AppResult<()> {
    if state.accounts.get(&uuid).is_none() {
        return Err(AppError::new("account_unknown", "unknown account"));
    }
    state.update_settings(|settings| settings.selected_account = Some(uuid.clone()))?;
    Ok(())
}

#[tauri::command]
pub async fn accounts_remove(state: State<'_, AppState>, uuid: String) -> AppResult<()> {
    if let Some(summary) = state.accounts.get(&uuid) {
        // Invalidate the session server-side when the protocol has a way to.
        if summary.kind == AccountKind::AzAuth {
            if let Ok(account) = state.accounts.load_account(&uuid) {
                if let Some(AuthMode::AzAuth { url }) = state.snapshot().map(|s| s.config.auth) {
                    if let Ok(client) = AzAuth::new(state.http.clone(), &url) {
                        if let Err(error) = client.signout(&account).await {
                            log::debug!("azauth signout failed (ignored): {error}");
                        }
                    }
                }
            }
        }
    }
    state.accounts.remove(&uuid)?;
    state.update_settings(|settings| {
        if settings.selected_account.as_deref() == Some(uuid.as_str()) {
            settings.selected_account = None;
        }
    })?;
    Ok(())
}

/// Renews the session of an account and refreshes its profile (skins, capes).
#[tauri::command]
pub async fn accounts_refresh(
    state: State<'_, AppState>,
    uuid: String,
) -> AppResult<AccountSummary> {
    let account = ensure_fresh_account(&state, &uuid).await?;
    state
        .accounts
        .get(&account.uuid)
        .ok_or_else(|| AppError::new("account_unknown", "unknown account"))
}

/// Loads an account and renews its session when the protocol allows it:
/// Microsoft (refresh token), AZauth (`verify`). Offline accounts are returned
/// as is. A session that cannot be renewed marks the account as needing a new
/// sign-in and fails with `auth_expired`.
pub async fn ensure_fresh_account(state: &AppState, uuid: &str) -> AppResult<Account> {
    // One renewal at a time: Microsoft rotates refresh tokens, so a launch and
    // the background refresher must not race each other.
    let _guard = state.refresh_lock.lock().await;
    let summary = state
        .accounts
        .get(uuid)
        .ok_or_else(|| AppError::new("account_unknown", "unknown account"))?;
    let account = state.accounts.load_account(uuid)?;
    let refreshed: AppResult<Account> = match summary.kind {
        AccountKind::Microsoft => {
            // Refresh tokens are bound to the application that issued them.
            // An account that does not remember its own only has the panel's
            // to go on: without the panel, keep the stored session rather
            // than risk a false "expired" under the official id.
            if account.client_id.is_none() && panel_config(state).is_none() {
                log::debug!("panel configuration unknown: keeping the stored Microsoft session");
                return Ok(account);
            }
            let client_ids = refresh_client_ids(state, &account);
            refresh_microsoft(state, &account, &client_ids).await
        }
        AccountKind::AzAuth => {
            let site = match state.snapshot().map(|s| s.config.auth) {
                Some(AuthMode::AzAuth { url }) => url,
                _ => {
                    // Site unknown while offline: keep the stored session.
                    return Ok(account);
                }
            };
            let client = AzAuth::new(state.http.clone(), &site)?;
            client.verify(&account).await.map_err(AppError::from)
        }
        AccountKind::Offline | AccountKind::Yggdrasil => return Ok(account),
    };
    match refreshed {
        Ok(fresh) => {
            state.accounts.upsert(fresh.clone())?;
            Ok(fresh)
        }
        Err(mapped) => {
            // A network problem is not an expired session: keep the account usable.
            if matches!(mapped.code, "network" | "timeout" | "api_unavailable") {
                log::warn!("session refresh postponed for {}: {mapped}", summary.name);
                return Ok(account);
            }
            log::warn!("session of {} could not be renewed: {mapped}", summary.name);
            state.accounts.set_needs_reauth(uuid, true);
            Err(AppError::new("auth_expired", mapped.message).with_details(mapped.code))
        }
    }
}

fn store_and_select(state: &AppState, account: Account) -> AppResult<AccountSummary> {
    let summary = state.accounts.upsert(account)?;
    state.update_settings(|settings| settings.selected_account = Some(summary.uuid.clone()))?;
    Ok(summary)
}

fn is_valid_username(username: &str) -> bool {
    let length = username.chars().count();
    (3..=16).contains(&length)
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_usernames() {
        assert!(is_valid_username("Luuxis"));
        assert!(is_valid_username("a_b_c"));
        assert!(!is_valid_username("ab"));
        assert!(!is_valid_username("with space"));
        assert!(!is_valid_username("waytoolongusername_"));
    }

    /// Documents why the launcher only offers the login window: the panel's
    /// Azure application refuses the device code flow on both authorities,
    /// while the official launcher client id accepts it.
    /// Run with `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn device_code_request_with_each_client_id() {
        let config = crate::config::LauncherConfig::from_client(&crate::client_config::ClientConfig::sample());
        let http = crust_core::network::HttpClient::new().expect("http");
        let api = crate::api::LuuxCraftApi::new(http.inner().clone(), &config);
        let remote = api.config().await.expect("panel config");
        let panel_client_id = remote.client_id.expect("panel client id");

        let official = Authenticator::new().expect("authenticator");
        match official.device_code().await {
            Ok(flow) => println!("official client id: OK ({})", flow.verification_uri()),
            Err(error) => println!("official client id: ERROR {error}"),
        }
        let entra = Authenticator::with_client_id(&panel_client_id).expect("authenticator");
        match entra.device_code().await {
            Ok(flow) => println!("panel client id (Entra): OK ({})", flow.verification_uri()),
            Err(error) => println!("panel client id (Entra): ERROR {error}"),
        }
        let live = Authenticator::with_oauth(
            http.clone(),
            MicrosoftOAuth::new(http.clone(), panel_client_id, Authority::Live),
        );
        match live.device_code().await {
            Ok(flow) => println!("panel client id (Live): OK ({})", flow.verification_uri()),
            Err(error) => println!("panel client id (Live): ERROR {error}"),
        }
        let request = live.authorize_request();
        println!("authorize url (Live): {}", request.url);
    }
}
