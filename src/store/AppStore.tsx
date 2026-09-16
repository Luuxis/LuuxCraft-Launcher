/**
 * Application store: one reducer for the UI state, actions that talk to the
 * backend through `ipc`, and a boot sequence. Everything data-driven comes
 * from the panel snapshot (`remote`) and the persisted settings.
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  type ReactNode,
} from "react";

import { resolveBrand, type Brand } from "../config/brand";
import { describeError, t } from "../i18n";
import { ipc, toAppError } from "../lib/ipc";
import { allowedInstances, moduleEnabled, pickInstance } from "../lib/instances";
import { logger } from "../lib/logger";
import type {
  AccountSummary,
  AppError,
  Bootstrap,
  Instance,
  InstanceStatus,
  LaunchEvent,
  RemoteSnapshot,
  RunningGame,
  Settings,
  UpdateCheck,
} from "../lib/types";

export type View = "home" | "accounts" | "skins" | "settings";

export interface LogLine {
  id: number;
  stream: "stdout" | "stderr" | "launcher";
  text: string;
}

export interface GameState {
  mode: "idle" | "install" | "launch";
  instanceId: string | null;
  stage: string | null;
  progress: { downloaded: number; total: number; element: string } | null;
  check: { checked: number; total: number; element: string } | null;
  speed: number | null;
  eta: number | null;
  logs: LogLine[];
  running: RunningGame | null;
  error: AppError | null;
  lastExit: { code: number | null; success: boolean; instanceId: string } | null;
}

export interface Toast {
  id: number;
  kind: "success" | "error" | "warning" | "info";
  title: string;
  message?: string;
}

export interface State {
  phase: "booting" | "ready" | "fatal";
  bootMessage: string;
  fatal: AppError | null;
  bootstrap: Bootstrap | null;
  settings: Settings | null;
  accounts: AccountSummary[];
  remote: RemoteSnapshot | null;
  remoteError: AppError | null;
  remoteLoading: boolean;
  statuses: Record<string, InstanceStatus>;
  game: GameState;
  view: View;
  toasts: Toast[];
  update: UpdateCheck | null;
  updateProgress: { downloaded: number; total: number | null } | null;
  updateInstalling: boolean;
  updateDismissed: boolean;
  loginOpen: boolean;
}

const MAX_LOG_LINES = 3000;
let nextId = 1;

const initialGame: GameState = {
  mode: "idle",
  instanceId: null,
  stage: null,
  progress: null,
  check: null,
  speed: null,
  eta: null,
  logs: [],
  running: null,
  error: null,
  lastExit: null,
};

const initialState: State = {
  phase: "booting",
  bootMessage: t("boot.starting"),
  fatal: null,
  bootstrap: null,
  settings: null,
  accounts: [],
  remote: null,
  remoteError: null,
  remoteLoading: false,
  statuses: {},
  game: initialGame,
  view: "home",
  toasts: [],
  update: null,
  updateProgress: null,
  updateInstalling: false,
  updateDismissed: false,
  loginOpen: false,
};

type Action =
  | { type: "boot/message"; message: string }
  | { type: "boot/done"; bootstrap: Bootstrap }
  | { type: "boot/fatal"; error: AppError }
  | { type: "settings"; settings: Settings }
  | { type: "accounts"; accounts: AccountSummary[] }
  | { type: "remote/loading" }
  | { type: "remote/ok"; remote: RemoteSnapshot }
  | { type: "remote/error"; error: AppError }
  | { type: "statuses"; statuses: InstanceStatus[] }
  | { type: "game/begin"; mode: "install" | "launch"; instanceId: string }
  | { type: "game/event"; event: LaunchEvent }
  | { type: "game/error"; error: AppError | null }
  | { type: "game/running"; running: RunningGame | null }
  | { type: "game/clearLogs" }
  | { type: "view"; view: View }
  | { type: "toast/add"; toast: Toast }
  | { type: "toast/remove"; id: number }
  | { type: "update/result"; update: UpdateCheck }
  | { type: "update/progress"; progress: { downloaded: number; total: number | null } | null }
  | { type: "update/installing"; installing: boolean }
  | { type: "update/dismiss" }
  | { type: "login/open"; open: boolean };

function appendLogs(logs: LogLine[], lines: LogLine[]): LogLine[] {
  const merged = logs.concat(lines);
  return merged.length > MAX_LOG_LINES ? merged.slice(merged.length - MAX_LOG_LINES) : merged;
}

function gameReducer(game: GameState, event: LaunchEvent): GameState {
  switch (event.event) {
    case "stage":
      return { ...game, stage: event.data.stage };
    case "check":
      return { ...game, stage: "checking", check: event.data };
    case "progress":
      return { ...game, stage: "downloading", progress: event.data };
    case "speed":
      return { ...game, speed: event.data.bytesPerSecond };
    case "estimated":
      return { ...game, eta: event.data.seconds };
    case "extract":
      return {
        ...game,
        stage: "extracting",
        logs: appendLogs(game.logs, [{ id: nextId++, stream: "launcher", text: event.data.file }]),
      };
    case "patch":
      return {
        ...game,
        stage: "patching",
        logs: appendLogs(game.logs, [{ id: nextId++, stream: "launcher", text: event.data.line }]),
      };
    case "warning":
      return {
        ...game,
        logs: appendLogs(game.logs, [{ id: nextId++, stream: "stderr", text: `[launcher] ${event.data.message}` }]),
      };
    case "installed":
      return { ...game, mode: "idle", stage: null, progress: null, check: null, speed: null, eta: null };
    case "started":
      return {
        ...game,
        mode: "idle",
        stage: "running",
        progress: null,
        check: null,
        speed: null,
        eta: null,
        running: {
          instanceId: game.instanceId ?? "",
          instanceName: "",
          pid: event.data.pid,
          startedAt: Math.floor(Date.now() / 1000),
          bound: false,
          hiddenLauncher: false,
        },
      };
    case "log":
      return {
        ...game,
        logs: appendLogs(
          game.logs,
          event.data.lines.map((text) => ({ id: nextId++, stream: event.data.stream, text })),
        ),
      };
    case "exited":
      return {
        ...game,
        mode: "idle",
        stage: null,
        running: null,
        lastExit: { code: event.data.code, success: event.data.success, instanceId: game.instanceId ?? "" },
        logs: appendLogs(game.logs, [
          {
            id: nextId++,
            stream: "launcher",
            text:
              event.data.code === null
                ? t("home.exited")
                : t("home.exitedWithCode", { code: event.data.code }),
          },
        ]),
      };
    default:
      return game;
  }
}

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "boot/message":
      return { ...state, bootMessage: action.message };
    case "boot/done":
      return {
        ...state,
        phase: "ready",
        bootstrap: action.bootstrap,
        settings: action.bootstrap.settings,
        accounts: action.bootstrap.accounts,
      };
    case "boot/fatal":
      return { ...state, phase: "fatal", fatal: action.error };
    case "settings":
      return { ...state, settings: action.settings };
    case "accounts":
      return { ...state, accounts: action.accounts };
    case "remote/loading":
      return { ...state, remoteLoading: true };
    case "remote/ok":
      return { ...state, remote: action.remote, remoteError: null, remoteLoading: false };
    case "remote/error":
      return { ...state, remoteError: action.error, remoteLoading: false };
    case "statuses": {
      const statuses: Record<string, InstanceStatus> = {};
      for (const status of action.statuses) statuses[status.id] = status;
      return { ...state, statuses };
    }
    case "game/begin":
      return {
        ...state,
        game: {
          ...initialGame,
          logs: [],
          running: state.game.running,
          mode: action.mode,
          instanceId: action.instanceId,
          stage: "resolving",
        },
      };
    case "game/event":
      return { ...state, game: gameReducer(state.game, action.event) };
    case "game/error":
      return {
        ...state,
        game: {
          ...state.game,
          mode: "idle",
          stage: null,
          progress: null,
          check: null,
          speed: null,
          eta: null,
          error: action.error,
        },
      };
    case "game/running":
      return { ...state, game: { ...state.game, running: action.running, stage: action.running ? "running" : null } };
    case "game/clearLogs":
      return { ...state, game: { ...state.game, logs: [] } };
    case "view":
      return { ...state, view: action.view };
    case "toast/add":
      return { ...state, toasts: [...state.toasts.slice(-3), action.toast] };
    case "toast/remove":
      return { ...state, toasts: state.toasts.filter((toast) => toast.id !== action.id) };
    case "update/result":
      return { ...state, update: action.update };
    case "update/progress":
      return { ...state, updateProgress: action.progress };
    case "update/installing":
      return { ...state, updateInstalling: action.installing };
    case "update/dismiss":
      return { ...state, updateDismissed: true };
    case "login/open":
      return { ...state, loginOpen: action.open };
    default:
      return state;
  }
}

export interface Actions {
  refreshRemote: (quiet?: boolean) => Promise<void>;
  refreshStatuses: () => Promise<void>;
  saveSettings: (patch: Partial<Settings> | ((current: Settings) => Settings)) => Promise<Settings | null>;
  resetSettings: () => Promise<void>;
  reloadAccounts: () => Promise<void>;
  selectAccount: (uuid: string) => Promise<void>;
  removeAccount: (uuid: string) => Promise<void>;
  refreshAccount: (uuid: string) => Promise<AccountSummary | null>;
  accountAdded: (account: AccountSummary) => Promise<void>;
  selectInstance: (id: string) => Promise<void>;
  install: (instanceId: string) => Promise<void>;
  launch: (instanceId: string) => Promise<void>;
  cancelInstall: () => Promise<void>;
  killGame: () => Promise<void>;
  clearLogs: () => void;
  navigate: (view: View) => void;
  toast: (kind: Toast["kind"], title: string, message?: string) => void;
  toastError: (error: unknown, title?: string) => void;
  dismissToast: (id: number) => void;
  checkUpdate: () => Promise<UpdateCheck | null>;
  installUpdate: () => Promise<void>;
  dismissUpdate: () => void;
  openLogin: (open: boolean) => void;
  openExternal: (url: string) => Promise<void>;
  openFolder: (target: string) => Promise<void>;
}

const StateContext = createContext<State>(initialState);
const ActionsContext = createContext<Actions | null>(null);

function applyTheme(theme: Settings["ui"]["theme"], reduceMotion: boolean) {
  const root = document.documentElement;
  const effective =
    theme === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : theme;
  root.classList.remove("dark", "light", "theme-dark", "theme-light");
  root.classList.add(effective, `theme-${effective}`);
  root.classList.toggle("reduce-motion", reduceMotion);
  try {
    localStorage.setItem("theme", theme);
    localStorage.setItem("luuxcraft-theme", theme);
  } catch {
    /* storage may be unavailable */
  }
  window.dispatchEvent(new CustomEvent("themechange", { detail: { theme: effective } }));
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(reducer, initialState);
  const stateRef = useRef(state);
  stateRef.current = state;

  const toast = useCallback((kind: Toast["kind"], title: string, message?: string) => {
    const id = nextId++;
    dispatch({ type: "toast/add", toast: { id, kind, title, message } });
    window.setTimeout(() => dispatch({ type: "toast/remove", id }), kind === "error" ? 8000 : 5000);
  }, []);

  const toastError = useCallback(
    (error: unknown, title?: string) => {
      const appError = toAppError(error);
      if (appError.code === "cancelled") return;
      logger.warn(`error shown: [${appError.code}] ${appError.message}`);
      toast("error", title ?? t("errors.title"), describeError(appError));
    },
    [toast],
  );

  const refreshStatuses = useCallback(async () => {
    try {
      dispatch({ type: "statuses", statuses: await ipc.instancesStatus() });
    } catch (error) {
      logger.warn(`instance statuses unavailable: ${toAppError(error).message}`);
    }
  }, []);

  const refreshRemote = useCallback(
    async (quiet = false) => {
      dispatch({ type: "remote/loading" });
      try {
        const remote = await ipc.remoteFetch();
        dispatch({ type: "remote/ok", remote });
        if (!quiet) toast("success", t("toasts.remoteRefreshed"));
        await refreshStatuses();
      } catch (error) {
        const appError = toAppError(error);
        dispatch({ type: "remote/error", error: appError });
        if (!quiet) toastError(appError);
      }
    },
    [refreshStatuses, toast, toastError],
  );

  const saveSettings = useCallback(
    async (patch: Partial<Settings> | ((current: Settings) => Settings)) => {
      const current = stateRef.current.settings;
      if (!current) return null;
      const next = typeof patch === "function" ? patch(current) : { ...current, ...patch };
      dispatch({ type: "settings", settings: next });
      try {
        const stored = await ipc.settingsUpdate(next);
        dispatch({ type: "settings", settings: stored });
        return stored;
      } catch (error) {
        dispatch({ type: "settings", settings: current });
        toastError(error);
        return null;
      }
    },
    [toastError],
  );

  const resetSettings = useCallback(async () => {
    try {
      dispatch({ type: "settings", settings: await ipc.settingsReset() });
      toast("success", t("toasts.settingsReset"));
    } catch (error) {
      toastError(error);
    }
  }, [toast, toastError]);

  const reloadAccounts = useCallback(async () => {
    try {
      dispatch({ type: "accounts", accounts: await ipc.accountsList() });
    } catch (error) {
      toastError(error);
    }
  }, [toastError]);

  const selectAccount = useCallback(
    async (uuid: string) => {
      try {
        await ipc.accountsSelect(uuid);
        const settings = await ipc.settingsGet();
        dispatch({ type: "settings", settings });
        const account = stateRef.current.accounts.find((a) => a.uuid === uuid);
        if (account) toast("success", t("toasts.accountSelected", { name: account.name }));
      } catch (error) {
        toastError(error);
      }
    },
    [toast, toastError],
  );

  const removeAccount = useCallback(
    async (uuid: string) => {
      try {
        await ipc.accountsRemove(uuid);
        dispatch({ type: "accounts", accounts: await ipc.accountsList() });
        dispatch({ type: "settings", settings: await ipc.settingsGet() });
        toast("info", t("toasts.accountRemoved"));
      } catch (error) {
        toastError(error);
      }
    },
    [toast, toastError],
  );

  const refreshAccount = useCallback(
    async (uuid: string) => {
      try {
        const account = await ipc.accountsRefresh(uuid);
        dispatch({ type: "accounts", accounts: await ipc.accountsList() });
        return account;
      } catch (error) {
        dispatch({ type: "accounts", accounts: await ipc.accountsList().catch(() => stateRef.current.accounts) });
        toastError(error);
        return null;
      }
    },
    [toastError],
  );

  const accountAdded = useCallback(
    async (account: AccountSummary) => {
      dispatch({ type: "accounts", accounts: await ipc.accountsList().catch(() => [...stateRef.current.accounts, account]) });
      dispatch({ type: "settings", settings: await ipc.settingsGet().catch(() => stateRef.current.settings!) });
      toast("success", t("login.success", { name: account.name }));
    },
    [toast],
  );

  const selectInstance = useCallback(
    async (id: string) => {
      await saveSettings({ selectedInstance: id });
    },
    [saveSettings],
  );

  const handleLaunchEvent = useCallback(
    (event: LaunchEvent) => {
      dispatch({ type: "game/event", event });
      const instanceId = stateRef.current.game.instanceId;
      const instance = stateRef.current.remote?.instances.find((i) => i.id === instanceId);
      if (event.event === "installed") {
        toast("success", t("toasts.installed", { name: instance?.name ?? "" }));
        void refreshStatuses();
      } else if (event.event === "started") {
        void ipc
          .gameRunning()
          .then((running) => dispatch({ type: "game/running", running }))
          .catch(() => undefined);
        void refreshStatuses();
      } else if (event.event === "exited") {
        if (event.data.success || event.data.code === 0) toast("info", t("toasts.exited"));
        else toast("warning", t("toasts.crashed"), t("home.crashed", { code: event.data.code ?? "?" }));
      }
    },
    [refreshStatuses, toast],
  );

  const runGame = useCallback(
    async (instanceId: string, mode: "install" | "launch") => {
      if (stateRef.current.game.mode !== "idle") {
        toast("warning", t("launch.busy"));
        return;
      }
      dispatch({ type: "game/begin", mode, instanceId });
      logger.info(`${mode} requested for instance ${instanceId}`);
      try {
        if (mode === "install") await ipc.instanceInstall(instanceId, handleLaunchEvent);
        else await ipc.instanceLaunch(instanceId, handleLaunchEvent);
      } catch (error) {
        const appError = toAppError(error);
        dispatch({ type: "game/error", error: appError.code === "cancelled" ? null : appError });
        if (appError.code === "cancelled") toast("info", t("launch.cancelled"));
        else toastError(appError);
        if (appError.code === "auth_expired") await reloadAccounts();
      }
    },
    [handleLaunchEvent, reloadAccounts, toast, toastError],
  );

  const install = useCallback((instanceId: string) => runGame(instanceId, "install"), [runGame]);
  const launch = useCallback((instanceId: string) => runGame(instanceId, "launch"), [runGame]);

  const cancelInstall = useCallback(async () => {
    try {
      await ipc.instanceCancel();
    } catch (error) {
      toastError(error);
    }
  }, [toastError]);

  const killGame = useCallback(async () => {
    try {
      await ipc.gameKill();
    } catch (error) {
      toastError(error);
    }
  }, [toastError]);

  const clearLogs = useCallback(() => dispatch({ type: "game/clearLogs" }), []);
  const navigate = useCallback((view: View) => dispatch({ type: "view", view }), []);
  const dismissToast = useCallback((id: number) => dispatch({ type: "toast/remove", id }), []);
  const openLogin = useCallback((open: boolean) => dispatch({ type: "login/open", open }), []);

  const checkUpdate = useCallback(async () => {
    try {
      const update = await ipc.updateCheck();
      dispatch({ type: "update/result", update });
      return update;
    } catch (error) {
      logger.warn(`update check failed: ${toAppError(error).message}`);
      toastError(error);
      return null;
    }
  }, [toastError]);

  const installUpdate = useCallback(async () => {
    dispatch({ type: "update/installing", installing: true });
    dispatch({ type: "update/progress", progress: { downloaded: 0, total: null } });
    try {
      await ipc.updateInstall((event) => {
        if (event.event === "started") {
          dispatch({ type: "update/progress", progress: { downloaded: 0, total: event.data.contentLength } });
        } else if (event.event === "progress") {
          dispatch({
            type: "update/progress",
            progress: { downloaded: event.data.downloaded, total: event.data.contentLength },
          });
        }
      });
    } catch (error) {
      dispatch({ type: "update/installing", installing: false });
      dispatch({ type: "update/progress", progress: null });
      toastError(error);
    }
  }, [toastError]);

  const dismissUpdate = useCallback(() => dispatch({ type: "update/dismiss" }), []);

  const openExternal = useCallback(
    async (url: string) => {
      try {
        await ipc.openExternal(url);
      } catch (error) {
        toastError(error);
      }
    },
    [toastError],
  );

  const openFolder = useCallback(
    async (target: string) => {
      try {
        await ipc.openFolder(target);
      } catch (error) {
        toastError(error);
      }
    },
    [toastError],
  );

  // Boot sequence.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const bootstrap = await ipc.bootstrap();
        if (cancelled) return;
        applyTheme(bootstrap.settings.ui.theme, bootstrap.settings.ui.reduceMotion);
        dispatch({ type: "boot/done", bootstrap });
        logger.info(`ui ready (launcher ${bootstrap.system.launcherVersion})`);
        dispatch({ type: "boot/message", message: t("boot.loadingPanel") });
        await refreshRemote(true);
        const running = await ipc.gameRunning().catch(() => null);
        if (running) dispatch({ type: "game/running", running });
        if (bootstrap.settings.checkUpdatesOnStartup && updaterConfigured(stateRef.current)) {
          try {
            dispatch({ type: "update/result", update: await ipc.updateCheck() });
          } catch (error) {
            logger.warn(`startup update check failed: ${toAppError(error).message}`);
          }
        }
      } catch (error) {
        if (!cancelled) dispatch({ type: "boot/fatal", error: toAppError(error) });
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The backend renews the sessions on its own: adopt the fresh list and warn
  // when one of them now requires a new sign-in.
  useEffect(() => {
    const unlisten = ipc.onAccountsChanged((accounts) => {
      const before = stateRef.current.accounts;
      dispatch({ type: "accounts", accounts });
      for (const account of accounts) {
        const previous = before.find((candidate) => candidate.uuid === account.uuid);
        if (account.needsReauth && previous && !previous.needsReauth) {
          toast("warning", account.name, t("accounts.needsReauthHint"));
        }
      }
    });
    return () => {
      void unlisten.then((stop) => stop()).catch(() => undefined);
    };
  }, [toast]);

  useEffect(() => {
    if (state.settings) applyTheme(state.settings.ui.theme, state.settings.ui.reduceMotion);
  }, [state.settings?.ui.theme, state.settings?.ui.reduceMotion, state.settings]);

  useEffect(() => {
    if (state.settings?.ui.theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const listener = () => applyTheme("system", state.settings?.ui.reduceMotion ?? false);
    media.addEventListener("change", listener);
    return () => media.removeEventListener("change", listener);
  }, [state.settings?.ui.theme, state.settings?.ui.reduceMotion]);

  const actions = useMemo<Actions>(
    () => ({
      refreshRemote,
      refreshStatuses,
      saveSettings,
      resetSettings,
      reloadAccounts,
      selectAccount,
      removeAccount,
      refreshAccount,
      accountAdded,
      selectInstance,
      install,
      launch,
      cancelInstall,
      killGame,
      clearLogs,
      navigate,
      toast,
      toastError,
      dismissToast,
      checkUpdate,
      installUpdate,
      dismissUpdate,
      openLogin,
      openExternal,
      openFolder,
    }),
    [
      refreshRemote,
      refreshStatuses,
      saveSettings,
      resetSettings,
      reloadAccounts,
      selectAccount,
      removeAccount,
      refreshAccount,
      accountAdded,
      selectInstance,
      install,
      launch,
      cancelInstall,
      killGame,
      clearLogs,
      navigate,
      toast,
      toastError,
      dismissToast,
      checkUpdate,
      installUpdate,
      dismissUpdate,
      openLogin,
      openExternal,
      openFolder,
    ],
  );

  return (
    <StateContext.Provider value={state}>
      <ActionsContext.Provider value={actions}>{children}</ActionsContext.Provider>
    </StateContext.Provider>
  );
}

export function useAppState(): State {
  return useContext(StateContext);
}

export function useActions(): Actions {
  const actions = useContext(ActionsContext);
  if (!actions) throw new Error("useActions must be used inside AppProvider");
  return actions;
}

/** The account selected in the settings, if it still exists. */
export function useSelectedAccount(): AccountSummary | null {
  const { settings, accounts } = useAppState();
  const uuid = settings?.selectedAccount;
  return useMemo(() => accounts.find((account) => account.uuid === uuid) ?? null, [accounts, uuid]);
}

/** Instances the selected account may see, plus the one to show by default. */
export function useInstances(): { instances: Instance[]; hidden: number; selected: Instance | null } {
  const { remote, settings } = useAppState();
  const account = useSelectedAccount();
  return useMemo(() => {
    const all = remote?.instances ?? [];
    const instances = allowedInstances(all, account?.name);
    return {
      instances,
      hidden: all.length - instances.length,
      selected: pickInstance(instances, settings?.selectedInstance),
    };
  }, [remote, account?.name, settings?.selectedInstance]);
}

/** Auto-update needs at least one endpoint, from the panel or built in. */
function updaterConfigured(state: State): boolean {
  return (
    (state.remote?.config.updaterEndpoints.length ?? 0) > 0 ||
    (state.bootstrap?.config.updater.endpoints.length ?? 0) > 0
  );
}

/** The launcher identity: the panel's if it publishes one, built-in otherwise. */
export function useBrand(): Brand {
  const { remote } = useAppState();
  const remoteBrand = remote?.config.brand ?? null;
  return useMemo(() => resolveBrand(remoteBrand), [remoteBrand]);
}

export function useUpdaterConfigured(): boolean {
  return updaterConfigured(useAppState());
}

/** Module toggles published by the panel; unknown modules are shown. */
export function useModules(): (name: string) => boolean {
  const { remote } = useAppState();
  const remoteModules = remote?.config.modules;
  return useCallback(
    (name: string) => moduleEnabled(name, remoteModules),
    [remoteModules],
  );
}
