//! Shared launcher state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crust_core::network::HttpClient;
use tokio::sync::Notify;

use crate::accounts::AccountStore;
use crate::api::RemoteSnapshot;
use crate::config::{LauncherConfig, Paths};
use crate::error::{AppError, AppResult};
use crate::game::GameManager;
use crate::provisioning::ProvisioningSource;
use crate::settings::Settings;

pub struct AppState {
    pub config: LauncherConfig,
    /// Where `config.user_id` came from; `None` while the launcher is not
    /// paired to a client yet, which is what makes the UI show the pairing
    /// screen instead of an empty launcher.
    pub provisioning_source: Option<ProvisioningSource>,
    pub paths: Paths,
    pub http: HttpClient,
    pub settings: Mutex<Settings>,
    pub accounts: AccountStore,
    pub remote: Mutex<Option<RemoteSnapshot>>,
    pub game: GameManager,
    pub java_majors: Mutex<HashMap<String, u32>>,
    /// Set while the app is shutting down (bound game being killed).
    pub exiting: AtomicBool,
    /// Serialises session renewals (see `auth::ensure_fresh_account`).
    pub refresh_lock: tokio::sync::Mutex<()>,
    auth_cancel: Mutex<Option<Arc<Notify>>>,
    total_memory_mb: Option<u64>,
}

impl AppState {
    pub fn new(
        config: LauncherConfig,
        provisioning_source: Option<ProvisioningSource>,
        paths: Paths,
    ) -> Result<Self, String> {
        paths
            .create_all()
            .map_err(|error| format!("cannot create the launcher directories: {error}"))?;
        let http = HttpClient::new().map_err(|error| error.to_string())?;
        let total_memory_mb = crate::system::total_memory_mb();

        let mut settings = Settings::load(&paths.settings_file(), &config);
        settings.sanitize(&config, total_memory_mb);

        let data_directory = settings
            .resolved_data_directory
            .clone()
            .unwrap_or_else(|| config.data_directory.clone());
        let game_root = paths.game_root(settings.install_path().as_deref(), &data_directory);
        let accounts = AccountStore::open(game_root);

        Ok(Self {
            config,
            provisioning_source,
            paths,
            http,
            settings: Mutex::new(settings),
            accounts,
            remote: Mutex::new(None),
            game: GameManager::default(),
            java_majors: Mutex::new(HashMap::new()),
            exiting: AtomicBool::new(false),
            refresh_lock: tokio::sync::Mutex::new(()),
            auth_cancel: Mutex::new(None),
            total_memory_mb,
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().expect("settings mutex").clone()
    }

    /// Applies a change, sanitizes and persists the settings.
    pub fn update_settings(&self, apply: impl FnOnce(&mut Settings)) -> AppResult<Settings> {
        let mut guard = self.settings.lock().expect("settings mutex");
        apply(&mut guard);
        guard.sanitize(&self.config, self.total_memory_mb);
        guard.save(&self.paths.settings_file())?;
        Ok(guard.clone())
    }

    pub fn replace_settings(&self, incoming: Settings) -> AppResult<Settings> {
        crate::settings::validate_incoming(&incoming)?;
        let stored = self.update_settings(|settings| *settings = incoming)?;
        // The accounts file lives at the game location: follow it.
        self.accounts.relocate(self.game_root());
        Ok(stored)
    }

    /// The Minecraft root directory for the current settings.
    pub fn game_root(&self) -> PathBuf {
        let settings = self.settings();
        let data_directory = settings
            .resolved_data_directory
            .clone()
            .unwrap_or_else(|| self.config.data_directory.clone());
        self.paths
            .game_root(settings.install_path().as_deref(), &data_directory)
    }

    pub fn remember_data_directory(&self, directory: Option<&str>) {
        let Some(directory) = directory.map(str::trim).filter(|d| !d.is_empty()) else {
            return;
        };
        let current = self.settings().resolved_data_directory;
        if current.as_deref() == Some(directory) {
            return;
        }
        if let Err(error) = self.update_settings(|settings| {
            settings.resolved_data_directory = Some(directory.to_owned());
        }) {
            log::warn!("could not persist the data directory: {error}");
        }
    }

    pub fn set_auth_cancel(&self, cancel: Option<Arc<Notify>>) {
        *self.auth_cancel.lock().expect("auth cancel mutex") = cancel;
    }

    pub fn take_auth_cancel(&self) -> Option<Arc<Notify>> {
        self.auth_cancel.lock().expect("auth cancel mutex").take()
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        AppError::unknown(message)
    }
}
