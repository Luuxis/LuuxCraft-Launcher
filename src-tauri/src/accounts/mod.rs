//! Multi-account store.
//!
//! Accounts (with their `crust_core::authenticator::Account`, tokens included)
//! live in one JSON file in the client's own launcher directory (see
//! `config::Paths::accounts_file`). They used to sit at the game root; they
//! follow the client instead, because two servers may legitimately share a
//! game root — it weighs gigabytes — but never the player's sessions.
//! The file is written atomically and readable by the user only. Encryption
//! at rest is planned on top of this format; the structure is versioned for it.
//! Nothing sensitive ever reaches the webview: commands only return
//! `AccountSummary`.

use std::path::PathBuf;
use std::sync::Mutex;

use crust_core::authenticator::{Account, AccountType};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::util;

pub const ACCOUNTS_FILE: &str = "accounts.json";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountKind {
    Microsoft,
    AzAuth,
    Offline,
    Yggdrasil,
}

impl AccountKind {
    pub fn of(account: &Account) -> Self {
        match account.meta.kind {
            AccountType::Xbox => Self::Microsoft,
            AccountType::AzAuth => Self::AzAuth,
            AccountType::Mojang => {
                if account.meta.online.unwrap_or(false) {
                    Self::Yggdrasil
                } else {
                    Self::Offline
                }
            }
        }
    }
}

/// What the webview is allowed to know about an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub uuid: String,
    pub name: String,
    pub kind: AccountKind,
    pub online: bool,
    pub skin_url: Option<String>,
    /// `CLASSIC` or `SLIM` for Microsoft accounts.
    pub skin_variant: Option<String>,
    /// AZauth sites embed the skin as a data URL.
    pub skin_data_url: Option<String>,
    pub cape_url: Option<String>,
    pub cape_alias: Option<String>,
    /// Minecraft token expiry (unix millis) for Microsoft accounts.
    pub expires_at: Option<u64>,
    /// The stored session could not be renewed: the user must sign in again.
    pub needs_reauth: bool,
    pub added_at: u64,
    pub gamertag: Option<String>,
    pub ownership: Option<String>,
    pub extra: Option<AccountExtra>,
}

/// AZauth site information (money, role, ...).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountExtra {
    pub banned: Option<bool>,
    pub money: Option<f64>,
    pub role: Option<serde_json::Value>,
    pub verified: Option<bool>,
}

impl AccountSummary {
    pub fn from_stored(stored: &StoredAccount) -> Self {
        let account = &stored.account;
        let active_skin = account
            .profile
            .skins
            .iter()
            .find(|skin| skin.state.as_deref() == Some("ACTIVE"))
            .or_else(|| account.profile.skins.first());
        let active_cape = account
            .profile
            .capes
            .iter()
            .find(|cape| cape.state.as_deref() == Some("ACTIVE"));
        Self {
            uuid: account.uuid.clone(),
            name: account.name.clone(),
            kind: AccountKind::of(account),
            online: account.meta.online.unwrap_or(true),
            skin_url: active_skin
                .map(|skin| skin.url.clone())
                .filter(|url| !url.is_empty()),
            skin_variant: active_skin.and_then(|skin| skin.variant.clone()),
            skin_data_url: active_skin.and_then(|skin| skin.base64.clone()),
            cape_url: active_cape.map(|cape| cape.url.clone()),
            cape_alias: active_cape.and_then(|cape| cape.alias.clone()),
            expires_at: account.meta.access_token_expires_in,
            needs_reauth: stored.needs_reauth,
            added_at: stored.added_at,
            gamertag: account
                .xbox_account
                .as_ref()
                .and_then(|xbox| xbox.gamertag.clone()),
            ownership: match account.meta.kind {
                AccountType::Xbox => Some(format!("{:?}", account.meta.ownership).to_lowercase()),
                _ => None,
            },
            extra: account.user_info.as_ref().map(|info| AccountExtra {
                banned: info.banned,
                money: info.money,
                role: info.role.clone(),
                verified: info.verified,
            }),
        }
    }
}

/// One entry of `accounts.json`: the full crust_core account (the same JSON
/// shape as minecraft-java-core) plus launcher bookkeeping.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAccount {
    #[serde(flatten)]
    pub account: Account,
    #[serde(default = "util::unix_now")]
    pub added_at: u64,
    #[serde(default)]
    pub needs_reauth: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountsFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    accounts: Vec<StoredAccount>,
}

pub struct AccountStore {
    /// Fixe pour la durée de l'exécution : le fichier appartient au client, que
    /// rien ne change sans redémarrer (voir `provisioning::provisioning_set`).
    path: PathBuf,
    accounts: Mutex<Vec<StoredAccount>>,
}

impl AccountStore {
    /// Opens (or creates lazily) `<dir>/accounts.json`.
    pub fn open(dir: PathBuf) -> Self {
        let path = dir.join(ACCOUNTS_FILE);
        let accounts = Self::read(&path);
        log::info!(
            "accounts file: {} ({} account(s))",
            path.display(),
            accounts.len()
        );
        Self {
            path,
            accounts: Mutex::new(accounts),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    fn read(path: &PathBuf) -> Vec<StoredAccount> {
        match std::fs::read(path) {
            Ok(raw) => match serde_json::from_slice::<AccountsFile>(&raw) {
                Ok(file) => file.accounts,
                Err(error) => {
                    log::warn!("{} is unreadable, ignoring it: {error}", path.display());
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        }
    }

    fn persist(&self, accounts: &[StoredAccount]) -> AppResult<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(&AccountsFile {
            version: FORMAT_VERSION,
            accounts: accounts.to_vec(),
        })?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, json)?;
        restrict_permissions(&temp);
        std::fs::rename(&temp, &path)?;
        restrict_permissions(&path);
        Ok(())
    }

    pub fn list(&self) -> Vec<AccountSummary> {
        self.accounts
            .lock()
            .expect("accounts mutex")
            .iter()
            .map(AccountSummary::from_stored)
            .collect()
    }

    pub fn get(&self, uuid: &str) -> Option<AccountSummary> {
        self.accounts
            .lock()
            .expect("accounts mutex")
            .iter()
            .find(|stored| stored.account.uuid == uuid)
            .map(AccountSummary::from_stored)
    }

    /// Stores (or replaces) an account.
    pub fn upsert(&self, account: Account) -> AppResult<AccountSummary> {
        let mut accounts = self.accounts.lock().expect("accounts mutex");
        let existing = accounts
            .iter()
            .position(|stored| stored.account.uuid == account.uuid);
        let stored = StoredAccount {
            added_at: existing
                .map(|index| accounts[index].added_at)
                .unwrap_or_else(util::unix_now),
            needs_reauth: false,
            account,
        };
        let summary = AccountSummary::from_stored(&stored);
        match existing {
            Some(index) => accounts[index] = stored,
            None => accounts.push(stored),
        }
        self.persist(&accounts)?;
        log::info!("account stored: {} ({:?})", summary.name, summary.kind);
        Ok(summary)
    }

    pub fn remove(&self, uuid: &str) -> AppResult<()> {
        let mut accounts = self.accounts.lock().expect("accounts mutex");
        accounts.retain(|stored| stored.account.uuid != uuid);
        self.persist(&accounts)?;
        log::info!("account removed: {uuid}");
        Ok(())
    }

    /// The full account (with tokens), for internal use only.
    pub fn load_account(&self, uuid: &str) -> AppResult<Account> {
        self.accounts
            .lock()
            .expect("accounts mutex")
            .iter()
            .find(|stored| stored.account.uuid == uuid)
            .map(|stored| stored.account.clone())
            .ok_or_else(|| {
                AppError::new(
                    "auth_expired",
                    "the stored session is missing, please sign in again",
                )
            })
    }

    pub fn set_needs_reauth(&self, uuid: &str, needs_reauth: bool) {
        let mut accounts = self.accounts.lock().expect("accounts mutex");
        if let Some(stored) = accounts
            .iter_mut()
            .find(|stored| stored.account.uuid == uuid)
        {
            stored.needs_reauth = needs_reauth;
        }
        if let Err(error) = self.persist(&accounts) {
            log::warn!("could not persist accounts.json: {error}");
        }
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        log::warn!(
            "could not restrict permissions of {}: {error}",
            path.display()
        );
    }
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crust_core::authenticator::Yggdrasil;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("luuxcraft-accounts-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn round_trips_accounts_through_the_json_file() {
        let dir = temp_dir("roundtrip");
        let store = AccountStore::open(dir.clone());
        let steve = Yggdrasil::offline("Steve");
        let summary = store.upsert(steve.clone()).unwrap();
        assert_eq!(summary.kind, AccountKind::Offline);
        assert!(dir.join(ACCOUNTS_FILE).is_file());

        let reopened = AccountStore::open(dir.clone());
        assert_eq!(reopened.list().len(), 1);
        let loaded = reopened.load_account(&steve.uuid).unwrap();
        assert_eq!(loaded, steve);

        reopened.set_needs_reauth(&steve.uuid, true);
        assert!(reopened.get(&steve.uuid).unwrap().needs_reauth);
        reopened.remove(&steve.uuid).unwrap();
        assert!(reopened.list().is_empty());
        assert!(reopened.load_account(&steve.uuid).is_err());
    }

    #[test]
    fn file_keeps_the_minecraft_java_core_shape() {
        let dir = temp_dir("shape");
        let store = AccountStore::open(dir.clone());
        store.upsert(Yggdrasil::offline("Alex")).unwrap();
        let raw = std::fs::read_to_string(dir.join(ACCOUNTS_FILE)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(json["version"], FORMAT_VERSION);
        let account = &json["accounts"][0];
        assert_eq!(account["name"], "Alex");
        assert_eq!(account["meta"]["type"], "Mojang");
        assert!(account["access_token"].is_string());
        assert!(account["addedAt"].is_number());
    }

    /// Deux clients ne partagent pas leurs sessions : chacun ouvre son propre
    /// fichier, dans son propre dossier (voir `config::Paths`).
    #[test]
    fn two_clients_keep_separate_accounts() {
        let first = temp_dir("client-one");
        let second = temp_dir("client-two");
        let one = AccountStore::open(first.clone());
        one.upsert(Yggdrasil::offline("One")).unwrap();

        let two = AccountStore::open(second.clone());
        assert!(two.list().is_empty(), "un client neuf n'hérite de rien");
        two.upsert(Yggdrasil::offline("Two")).unwrap();

        assert!(second.join(ACCOUNTS_FILE).is_file());
        assert_eq!(AccountStore::open(first).list()[0].name, "One");
        assert_eq!(AccountStore::open(second).list()[0].name, "Two");
    }
}
