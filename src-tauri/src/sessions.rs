//! Automatic session renewal.
//!
//! A background task keeps the stored sessions valid while the launcher runs:
//! Microsoft accounts are renewed through their refresh token before the
//! Minecraft token expires, AZauth sessions are re-verified periodically, and
//! every account gets one renewal pass at startup so the profile (name, skin,
//! cape) is up to date. The renewed list is pushed to the interface.

use std::collections::HashMap;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::accounts::{AccountKind, AccountSummary};
use crate::auth::ensure_fresh_account;
use crate::state::AppState;
use crate::util;

/// Emitted with the fresh `AccountSummary` list whenever the renewal changed
/// something (never carries tokens).
pub const ACCOUNTS_CHANGED_EVENT: &str = "accounts://changed";

/// How often the accounts are examined.
const TICK: Duration = Duration::from_secs(5 * 60);
/// A Microsoft session is renewed once it expires within this window.
const EXPIRY_MARGIN_SECS: u64 = 30 * 60;
/// Never attempt two renewals of the same account closer than this.
const MIN_ATTEMPT_INTERVAL_SECS: u64 = 15 * 60;
/// AZauth announces no expiry: re-verify on this cadence.
const AZAUTH_INTERVAL_SECS: u64 = 6 * 60 * 60;
/// How long the first pass waits for the panel snapshot (the Microsoft client
/// id and the AZauth site come from it).
const SNAPSHOT_WAIT: Duration = Duration::from_secs(30);

/// Starts the renewal task. Returns immediately.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        wait_for_snapshot(&app).await;
        let mut last_attempt: HashMap<String, u64> = HashMap::new();
        loop {
            refresh_due(&app, &mut last_attempt).await;
            tokio::time::sleep(TICK).await;
        }
    });
}

async fn wait_for_snapshot(app: &AppHandle) {
    let deadline = tokio::time::Instant::now() + SNAPSHOT_WAIT;
    while tokio::time::Instant::now() < deadline {
        if app.state::<AppState>().snapshot().is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    log::debug!("session refresher starting without a fresh panel snapshot");
}

/// One pass over the accounts. `last_attempt` throttles the renewals.
async fn refresh_due(app: &AppHandle, last_attempt: &mut HashMap<String, u64>) {
    let before = app.state::<AppState>().accounts.list();
    let now = util::unix_now();
    let mut attempted = false;

    for account in &before {
        if !is_due(account, last_attempt.get(&account.uuid).copied(), now) {
            continue;
        }
        last_attempt.insert(account.uuid.clone(), now);
        attempted = true;
        let state = app.state::<AppState>();
        match ensure_fresh_account(&state, &account.uuid).await {
            Ok(fresh) => log::debug!("session checked for {}", fresh.name),
            Err(error) => {
                log::warn!("automatic renewal failed for {}: {error}", account.name)
            }
        }
    }
    if !attempted {
        return;
    }

    // Push the list when anything visible moved: a new expiry, a renamed
    // profile, a new skin or cape, or a session that now needs a sign-in.
    let after = app.state::<AppState>().accounts.list();
    last_attempt.retain(|uuid, _| after.iter().any(|account| &account.uuid == uuid));
    if differ(&before, &after) {
        log::info!("sessions renewed: {} account(s)", after.len());
        if let Err(error) = app.emit(ACCOUNTS_CHANGED_EVENT, after) {
            log::debug!("could not notify the interface of the renewal: {error}");
        }
    }
}

fn differ(before: &[AccountSummary], after: &[AccountSummary]) -> bool {
    serde_json::to_value(before).ok() != serde_json::to_value(after).ok()
}

/// Whether an account should be renewed now.
fn is_due(account: &AccountSummary, last_attempt: Option<u64>, now: u64) -> bool {
    // A session marked as expired gets one more try per run, no more. The
    // mark may date from a start where the application id that issued the
    // session was not available (see `auth`): this start may have it. A
    // session that is really gone fails again, silently, and is left alone.
    if account.needs_reauth && last_attempt.is_some() {
        return false;
    }
    if let Some(last) = last_attempt {
        if now.saturating_sub(last) < MIN_ATTEMPT_INTERVAL_SECS {
            return false;
        }
    }
    match account.kind {
        // First pass of the session, then shortly before the token expires.
        AccountKind::Microsoft => match (last_attempt, account.expires_at) {
            (None, _) => true,
            (Some(_), Some(expires_at_millis)) => {
                now.saturating_add(EXPIRY_MARGIN_SECS) >= expires_at_millis / 1000
            }
            (Some(_), None) => true,
        },
        AccountKind::AzAuth => {
            last_attempt.is_none_or(|last| now.saturating_sub(last) >= AZAUTH_INTERVAL_SECS)
        }
        // No session to renew.
        AccountKind::Offline | AccountKind::Yggdrasil => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(kind: AccountKind, expires_at: Option<u64>, needs_reauth: bool) -> AccountSummary {
        AccountSummary {
            uuid: "uuid".into(),
            name: "Luuxis".into(),
            kind,
            online: true,
            skin_url: None,
            skin_variant: None,
            skin_data_url: None,
            cape_url: None,
            cape_alias: None,
            expires_at,
            needs_reauth,
            added_at: 0,
            gamertag: None,
            ownership: None,
            extra: None,
        }
    }

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn microsoft_is_renewed_once_then_before_expiry() {
        let far = Some((NOW + 20 * 3600) * 1000);
        let soon = Some((NOW + 10 * 60) * 1000);
        // First pass of the session: always.
        assert!(is_due(
            &account(AccountKind::Microsoft, far, false),
            None,
            NOW
        ));
        // Then only when the token is about to expire.
        let long_ago = Some(NOW - 3600);
        assert!(!is_due(
            &account(AccountKind::Microsoft, far, false),
            long_ago,
            NOW
        ));
        assert!(is_due(
            &account(AccountKind::Microsoft, soon, false),
            long_ago,
            NOW
        ));
        // An unknown expiry is treated as due.
        assert!(is_due(
            &account(AccountKind::Microsoft, None, false),
            long_ago,
            NOW
        ));
    }

    #[test]
    fn renewals_are_throttled() {
        let soon = Some((NOW + 60) * 1000);
        let just_now = Some(NOW - 60);
        assert!(!is_due(
            &account(AccountKind::Microsoft, soon, false),
            just_now,
            NOW
        ));
    }

    /// Une session marquée expirée est retentée une fois par exécution — le
    /// démarrage peut apporter l'application qui l'a émise — puis laissée
    /// tranquille, quelle que soit son échéance.
    #[test]
    fn expired_sessions_get_one_try_per_run() {
        let soon = Some((NOW + 60) * 1000);
        assert!(is_due(
            &account(AccountKind::Microsoft, soon, true),
            None,
            NOW
        ));
        assert!(!is_due(
            &account(AccountKind::Microsoft, soon, true),
            Some(NOW - 24 * 3600),
            NOW
        ));
        assert!(!is_due(
            &account(AccountKind::AzAuth, None, true),
            Some(NOW - 24 * 3600),
            NOW
        ));
    }

    #[test]
    fn azauth_is_verified_on_its_own_cadence() {
        assert!(is_due(
            &account(AccountKind::AzAuth, None, false),
            None,
            NOW
        ));
        assert!(!is_due(
            &account(AccountKind::AzAuth, None, false),
            Some(NOW - 3600),
            NOW
        ));
        assert!(is_due(
            &account(AccountKind::AzAuth, None, false),
            Some(NOW - AZAUTH_INTERVAL_SECS),
            NOW
        ));
    }

    #[test]
    fn local_accounts_are_never_renewed() {
        assert!(!is_due(
            &account(AccountKind::Offline, None, false),
            None,
            NOW
        ));
        assert!(!is_due(
            &account(AccountKind::Yggdrasil, None, false),
            None,
            NOW
        ));
    }
}
