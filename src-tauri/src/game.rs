//! Installing and running an instance with `crust_core`, and everything
//! around the game process: output capture (redacted), exit tracking, the
//! launcher behaviour while the game runs, and the "bound" mode where closing
//! the launcher kills Minecraft.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crust_core::authenticator::Yggdrasil;
use crust_core::foundation::events::{Event, EventHandler};
use crust_core::launcher::{Launch, LaunchOptions, LaunchPlan, LoaderKind};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Notify;

use crate::api::Instance;
use crate::error::{AppError, AppResult};
use crate::settings::{JavaMode, LauncherBehavior};
use crate::state::AppState;
use crate::util;

/// Progress events fire on every chunk: the webview gets at most one every
/// `PROGRESS_INTERVAL` (plus the final one).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// Game output is batched so a chatty start-up does not flood the IPC.
const LOG_FLUSH_INTERVAL: Duration = Duration::from_millis(150);
const MAX_LOG_LINE: usize = 4000;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum LaunchEvent {
    /// `resolving`, `preparing`, `starting`.
    Stage {
        stage: String,
    },
    Check {
        checked: usize,
        total: usize,
        element: String,
    },
    Progress {
        downloaded: u64,
        total: u64,
        element: String,
    },
    Speed {
        bytes_per_second: f64,
    },
    Estimated {
        seconds: f64,
    },
    Extract {
        file: String,
    },
    Patch {
        line: String,
    },
    Warning {
        message: String,
    },
    Installed,
    Started {
        pid: Option<u32>,
    },
    Log {
        stream: &'static str,
        lines: Vec<String>,
    },
    Exited {
        code: Option<i32>,
        success: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    Install,
    Launch,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningGame {
    pub instance_id: String,
    pub instance_name: String,
    pub pid: Option<u32>,
    pub started_at: u64,
    pub bound: bool,
    pub hidden_launcher: bool,
    #[serde(skip)]
    kill: Arc<Notify>,
}

#[derive(Default)]
pub struct GameManager {
    running: Mutex<Option<RunningGame>>,
    busy: AtomicBool,
    cancel: Mutex<Option<Arc<Notify>>>,
}

impl GameManager {
    pub fn running(&self) -> Option<RunningGame> {
        self.running.lock().expect("running mutex").clone()
    }

    pub fn is_running(&self) -> bool {
        self.running.lock().expect("running mutex").is_some()
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    fn begin(&self) -> AppResult<()> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(AppError::new(
                "busy",
                "an installation is already in progress",
            ));
        }
        Ok(())
    }

    fn end(&self) {
        self.busy.store(false, Ordering::SeqCst);
        *self.cancel.lock().expect("cancel mutex") = None;
    }

    pub fn cancel(&self) -> bool {
        match self.cancel.lock().expect("cancel mutex").as_ref() {
            Some(cancel) => {
                cancel.notify_one();
                true
            }
            None => false,
        }
    }

    /// Asks the running game to stop (SIGKILL / TerminateProcess).
    pub fn kill(&self) -> bool {
        match self.running() {
            Some(running) => {
                log::warn!("killing the game process ({:?})", running.pid);
                running.kill.notify_one();
                true
            }
            None => false,
        }
    }
}

/// Installs (`LaunchMode::Install`) or installs then starts an instance.
pub async fn run(
    app: &AppHandle,
    state: &AppState,
    instance_id: &str,
    mode: LaunchMode,
    channel: Channel<LaunchEvent>,
) -> AppResult<()> {
    state.game.begin()?;
    let result = run_inner(app, state, instance_id, mode, channel).await;
    state.game.end();
    result
}

async fn run_inner(
    app: &AppHandle,
    state: &AppState,
    instance_id: &str,
    mode: LaunchMode,
    channel: Channel<LaunchEvent>,
) -> AppResult<()> {
    if mode == LaunchMode::Launch && state.game.is_running() {
        return Err(AppError::new("game_running", "the game is already running"));
    }
    let _ = channel.send(LaunchEvent::Stage {
        stage: "resolving".into(),
    });
    let snapshot = crate::api::ensure_snapshot(state).await?;
    let instance = snapshot.instance(instance_id).cloned().ok_or_else(|| {
        AppError::new(
            "instance_invalid",
            format!("unknown instance {instance_id}"),
        )
    })?;

    let settings = state.settings();
    let account = match (mode, settings.selected_account.as_deref()) {
        (LaunchMode::Launch, None) => {
            return Err(AppError::new(
                "account_required",
                "select an account before playing",
            ));
        }
        (LaunchMode::Launch, Some(uuid)) => {
            if snapshot.config.maintenance {
                return Err(AppError::new(
                    "maintenance",
                    snapshot
                        .config
                        .maintenance_message
                        .clone()
                        .unwrap_or_else(|| "the server is under maintenance".into()),
                ));
            }
            crate::auth::ensure_fresh_account(state, uuid).await?
        }
        (LaunchMode::Install, Some(uuid)) => match state.accounts.load_account(uuid) {
            Ok(account) => account,
            Err(_) => Yggdrasil::offline("Player"),
        },
        (LaunchMode::Install, None) => Yggdrasil::offline("Player"),
    };
    if mode == LaunchMode::Launch && !instance.allows(Some(&account.name)) {
        return Err(AppError::new(
            "instance_whitelisted",
            "this account is not allowed on this instance",
        ));
    }

    let root = state.game_root();
    let options = build_options(state, &instance, root.clone())?;
    let behavior = settings.launcher_behavior;
    log::info!(
        "{} {} ({}): minecraft {} loader {}/{} root {} java {:?} memory {}-{} concurrency {}",
        match mode {
            LaunchMode::Install => "installing",
            LaunchMode::Launch => "launching",
        },
        instance.name,
        instance.id,
        options.version,
        instance.loader.kind,
        options.loader.build,
        root.display(),
        options
            .java
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .or(options.java.version.clone()),
        options.memory.min,
        options.memory.max,
        options.concurrency()
    );

    let launch = Launch::new(options, account)?.with_events(bridge(channel.clone()));

    let cancel = Arc::new(Notify::new());
    *state.game.cancel.lock().expect("cancel mutex") = Some(cancel.clone());
    let _ = channel.send(LaunchEvent::Stage {
        stage: "preparing".into(),
    });
    let prepared = tokio::select! {
        result = launch.prepare() => result?,
        _ = cancel.notified() => {
            log::info!("installation of {} cancelled", instance.name);
            return Err(AppError::cancelled());
        }
    };
    log::info!(
        "instance {} ready (java {})",
        instance.name,
        prepared.java.display()
    );
    if mode == LaunchMode::Install {
        let _ = channel.send(LaunchEvent::Installed);
        return Ok(());
    }

    let _ = channel.send(LaunchEvent::Stage {
        stage: "starting".into(),
    });
    let plan = launch.plan(&prepared)?;
    let secrets = launch.secrets();
    log::debug!(
        "command: {}",
        plan.redacted_command(&secrets.iter().map(String::as_str).collect::<Vec<_>>())
    );
    spawn_game(app, state, &instance, plan, secrets, behavior, channel).await
}

/// Mirrors the reference `crust_core` runner: instance files from the panel,
/// loader under the root, per-user memory/java/screen settings.
fn build_options(state: &AppState, instance: &Instance, root: PathBuf) -> AppResult<LaunchOptions> {
    let settings = state.settings();
    let effective = settings.for_instance(&instance.id);

    let kind = instance.loader.kind.trim().to_ascii_lowercase();
    let (loader_kind, mcp) = match kind.as_str() {
        "mcp" => (None, instance.loader.mcp_file.clone()),
        "" | "none" | "vanilla" => (None, None),
        other => (
            Some(LoaderKind::parse(other).ok_or_else(|| {
                AppError::new("instance_invalid", format!("unknown loader type {other}"))
            })?),
            None,
        ),
    };

    let mut options = LaunchOptions::new(root, instance.minecraft_version.trim());
    options.url = instance.files_url.clone();
    options.instance = Some(instance.id.clone());
    options.intel_enabled_mac = settings.intel_enabled_mac;
    options.ignore_log4j = true;
    options.ignored = instance.ignored.clone();
    options.download_concurrency = settings.download_concurrency;
    options.verify = instance.verify;
    options.mcp = mcp;
    options.loader.kind = loader_kind;
    options.loader.build = instance.loader.version.clone();
    options.loader.enable = loader_kind.is_some();
    options.loader.path = "./".to_owned();
    options.jvm_args = instance
        .jvm_args
        .iter()
        .chain(effective.jvm_args.iter())
        .filter(|arg| !arg.trim().is_empty())
        .cloned()
        .collect();
    options.memory.min = format!("{}M", effective.memory.min_mb);
    options.memory.max = format!("{}M", effective.memory.max_mb);
    match effective.java.mode {
        JavaMode::Custom => {
            options.java.path = effective.java.path.as_deref().map(PathBuf::from);
        }
        JavaMode::Auto => {
            if let Some(forced) = instance.java_version.as_deref().and_then(util::java_major) {
                options.java.version = Some(forced.to_string());
            }
        }
    }
    options.screen.width = settings.game_window.width;
    options.screen.height = settings.game_window.height;
    options.screen.fullscreen = settings.game_window.fullscreen;
    options.detached = settings.launcher_behavior != LauncherBehavior::Bound;
    Ok(options)
}

/// Bridges `crust_core` events to the IPC channel, throttling the noisy ones.
fn bridge(channel: Channel<LaunchEvent>) -> EventHandler {
    let last_progress = Mutex::new(Instant::now() - PROGRESS_INTERVAL);
    let last_check = Mutex::new(Instant::now() - PROGRESS_INTERVAL);
    Arc::new(move |event| {
        let payload = match event {
            Event::Progress {
                downloaded,
                total,
                element,
            } => {
                let done = total > 0 && downloaded >= total;
                let mut last = last_progress.lock().expect("progress mutex");
                if !done && last.elapsed() < PROGRESS_INTERVAL {
                    return;
                }
                *last = Instant::now();
                LaunchEvent::Progress {
                    downloaded,
                    total,
                    element,
                }
            }
            Event::Check {
                checked,
                total,
                element,
            } => {
                let done = checked >= total;
                let mut last = last_check.lock().expect("check mutex");
                if !done && last.elapsed() < PROGRESS_INTERVAL {
                    return;
                }
                *last = Instant::now();
                LaunchEvent::Check {
                    checked,
                    total,
                    element,
                }
            }
            Event::Speed(bytes_per_second) => LaunchEvent::Speed { bytes_per_second },
            Event::Estimated(seconds) => LaunchEvent::Estimated { seconds },
            Event::Extract(file) => LaunchEvent::Extract { file },
            Event::Patch(line) => LaunchEvent::Patch { line },
            Event::Error(message) => {
                log::warn!("crust_core: {message}");
                LaunchEvent::Warning { message }
            }
        };
        let _ = channel.send(payload);
    })
}

async fn spawn_game(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
    plan: LaunchPlan,
    secrets: Vec<String>,
    behavior: LauncherBehavior,
    channel: Channel<LaunchEvent>,
) -> AppResult<()> {
    tokio::fs::create_dir_all(&plan.working_dir).await?;
    let mut command = tokio::process::Command::new(&plan.java);
    command
        .args(&plan.args)
        .current_dir(&plan.working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false);
    let detached = behavior != LauncherBehavior::Bound;
    #[cfg(unix)]
    if detached {
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW, plus DETACHED_PROCESS when the game must survive the launcher.
        let mut flags: u32 = 0x0800_0000;
        if detached {
            flags |= 0x0000_0008;
        }
        command.creation_flags(flags);
    }

    let mut child = command.spawn().map_err(|error| {
        AppError::new("launch_failed", format!("could not start Java: {error}"))
            .with_details(plan.java.display().to_string())
    })?;
    let pid = child.id();
    log::info!("game started for {} (pid {:?})", instance.name, pid);

    let kill = Arc::new(Notify::new());
    let running = RunningGame {
        instance_id: instance.id.clone(),
        instance_name: instance.name.clone(),
        pid,
        started_at: util::unix_now(),
        bound: behavior == LauncherBehavior::Bound,
        hidden_launcher: behavior == LauncherBehavior::Hide,
        kill: kill.clone(),
    };
    *state.game.running.lock().expect("running mutex") = Some(running);
    let _ = channel.send(LaunchEvent::Started { pid });

    if behavior == LauncherBehavior::Hide {
        if let Some(window) = app.get_webview_window("main") {
            if let Err(error) = window.hide() {
                log::warn!("could not hide the launcher window: {error}");
            }
        }
    }

    // Output capture.
    let log_file = open_game_log(state, &instance.id).await;
    let buffer: Arc<Mutex<Vec<(&'static str, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let secrets = Arc::new(secrets);
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(tauri::async_runtime::spawn(read_stream(
            "stdout",
            BufReader::new(stdout),
            buffer.clone(),
            secrets.clone(),
        )));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(tauri::async_runtime::spawn(read_stream(
            "stderr",
            BufReader::new(stderr),
            buffer.clone(),
            secrets.clone(),
        )));
    }
    let flusher_buffer = buffer.clone();
    let flusher_channel = channel.clone();
    let flusher_stop = Arc::new(Notify::new());
    let flusher_stop_signal = flusher_stop.clone();
    let flusher = tauri::async_runtime::spawn(async move {
        let mut file = log_file;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(LOG_FLUSH_INTERVAL) => {}
                _ = flusher_stop_signal.notified() => {
                    flush_logs(&flusher_buffer, &flusher_channel, &mut file).await;
                    break;
                }
            }
            flush_logs(&flusher_buffer, &flusher_channel, &mut file).await;
        }
        if let Some(file) = file.as_mut() {
            let _ = file.flush().await;
        }
    });

    // Exit tracking.
    let app = app.clone();
    let instance_name = instance.name.clone();
    tauri::async_runtime::spawn(async move {
        let status = tokio::select! {
            status = child.wait() => status,
            _ = kill.notified() => {
                if let Err(error) = child.start_kill() {
                    log::warn!("could not kill the game: {error}");
                }
                child.wait().await
            }
        };
        for reader in readers {
            let _ = reader.await;
        }
        flusher_stop.notify_one();
        let _ = flusher.await;

        let code = match &status {
            Ok(status) => status.code(),
            Err(error) => {
                log::error!("waiting for the game failed: {error}");
                None
            }
        };
        let success = status.as_ref().map(|s| s.success()).unwrap_or(false);
        log::info!("game exited for {instance_name} with {code:?}");

        let state = app.state::<AppState>();
        let running = state.game.running.lock().expect("running mutex").take();
        let _ = channel.send(LaunchEvent::Exited { code, success });

        if state.exiting.load(Ordering::SeqCst) {
            log::info!("bound game stopped, exiting the launcher");
            app.exit(0);
            return;
        }
        if running.map(|r| r.hidden_launcher).unwrap_or(false) {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
    });
    Ok(())
}

async fn read_stream<R: tokio::io::AsyncRead + Unpin>(
    stream: &'static str,
    reader: BufReader<R>,
    buffer: Arc<Mutex<Vec<(&'static str, String)>>>,
    secrets: Arc<Vec<String>>,
) {
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let mut line = util::redact(&line, &secrets);
        if line.chars().count() > MAX_LOG_LINE {
            line = line.chars().take(MAX_LOG_LINE).collect::<String>() + "…";
        }
        buffer
            .lock()
            .expect("log buffer mutex")
            .push((stream, line));
    }
}

async fn flush_logs(
    buffer: &Arc<Mutex<Vec<(&'static str, String)>>>,
    channel: &Channel<LaunchEvent>,
    file: &mut Option<tokio::fs::File>,
) {
    let pending: Vec<(&'static str, String)> =
        std::mem::take(&mut *buffer.lock().expect("log buffer mutex"));
    if pending.is_empty() {
        return;
    }
    if let Some(file) = file.as_mut() {
        let mut text = String::new();
        for (stream, line) in &pending {
            text.push_str(if *stream == "stderr" { "[stderr] " } else { "" });
            text.push_str(line);
            text.push('\n');
        }
        let _ = file.write_all(text.as_bytes()).await;
    }
    let mut by_stream: Vec<(&'static str, Vec<String>)> = Vec::new();
    for (stream, line) in pending {
        match by_stream.last_mut() {
            Some((last, lines)) if *last == stream => lines.push(line),
            _ => by_stream.push((stream, vec![line])),
        }
    }
    for (stream, lines) in by_stream {
        let _ = channel.send(LaunchEvent::Log { stream, lines });
    }
}

async fn open_game_log(state: &AppState, instance_id: &str) -> Option<tokio::fs::File> {
    let dir = state.paths.game_logs_dir().join(instance_id);
    if let Err(error) = tokio::fs::create_dir_all(&dir).await {
        log::warn!("cannot create the game log directory: {error}");
        return None;
    }
    let latest = dir.join("latest.log");
    let previous = dir.join("previous.log");
    if latest.exists() {
        let _ = tokio::fs::rename(&latest, &previous).await;
    }
    match tokio::fs::File::create(&latest).await {
        Ok(file) => Some(file),
        Err(error) => {
            log::warn!("cannot create the game log file: {error}");
            None
        }
    }
}

/// Called from the run loop on `RunEvent::ExitRequested`. In bound mode the
/// exit is postponed until the game has been killed and reaped.
pub fn on_exit_requested(app: &AppHandle, code: Option<i32>, api: &tauri::ExitRequestApi) {
    if code.is_some() {
        // Un code veut dire que la sortie vient du code, pas de l'utilisateur :
        // c'est l'`app.exit(0)` déclenché une fois le jeu lié arrêté. La
        // repousser une seconde fois laisserait le moteur ouvert pour toujours.
        return;
    }
    let state = app.state::<AppState>();
    let Some(running) = state.game.running() else {
        return;
    };
    if !running.bound {
        log::info!("launcher closing while the game keeps running (detached)");
        return;
    }
    if !state.exiting.swap(true, Ordering::SeqCst) {
        log::info!("launcher closing: stopping the bound game first");
        running.kill.notify_one();
    }
    api.prevent_exit();
}
