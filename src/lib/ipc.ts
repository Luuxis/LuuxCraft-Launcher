/**
 * Typed wrappers around the Tauri commands. Every backend call goes through
 * here so the rest of the UI never touches `invoke` directly.
 */
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AccountSummary,
  AppError,
  Article,
  AuthEvent,
  AuthMethods,
  AzAuthOutcome,
  Bootstrap,
  CapeOption,
  Instance,
  InstanceStatus,
  JavaInstall,
  JavaRequirement,
  LaunchEvent,
  LibrarySkin,
  RemoteSnapshot,
  RunningGame,
  ServerStatus,
  Settings,
  SkinData,
  SkinVariant,
  SystemInfo,
  UpdateCheck,
  UpdateEvent,
} from "./types";

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as AppError).code === "string" &&
    typeof (value as AppError).message === "string"
  );
}

/** Normalizes anything thrown by `invoke` into an `AppError`. */
export function toAppError(error: unknown): AppError {
  if (isAppError(error)) return error;
  if (error instanceof Error) return { code: "unknown", message: error.message };
  return { code: "unknown", message: String(error) };
}

/** The editable state of a library skin, as the editor holds it. */
export interface SkinDraft {
  name: string;
  variant: SkinVariant;
  capeId: string | null;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toAppError(error);
  }
}

// ── App ─────────────────────────────────────────────────────────────────────
export const ipc = {
  bootstrap: () => call<Bootstrap>("app_bootstrap"),
  systemInfo: () => call<SystemInfo>("system_info"),
  gameRoot: () => call<string>("game_root"),
  openFolder: (target: string) => call<void>("open_folder", { target }),
  openExternal: (url: string) => call<void>("open_external", { url }),

  // ── Settings
  settingsGet: () => call<Settings>("settings_get"),
  settingsUpdate: (settings: Settings) => call<Settings>("settings_update", { settings }),
  settingsReset: () => call<Settings>("settings_reset"),

  // ── Panel
  remoteFetch: () => call<RemoteSnapshot>("remote_fetch"),
  remoteCached: () => call<RemoteSnapshot | null>("remote_cached"),
  remoteArticles: (limit?: number) => call<Article[]>("remote_articles", { limit }),

  // ── Auth & accounts
  authMethods: () => call<AuthMethods>("auth_methods_get"),
  /** Microsoft sign-in in a dedicated login window (authorization code flow). */
  authMicrosoftWindow: (onEvent: (event: AuthEvent) => void) => {
    const channel = new Channel<AuthEvent>();
    channel.onmessage = onEvent;
    return call<AccountSummary>("auth_microsoft_window_login", { onEvent: channel });
  },
  authCancel: () => call<void>("auth_cancel"),
  authAzAuth: (email: string, password: string, code?: string) =>
    call<AzAuthOutcome>("auth_azauth_login", { email, password, code: code ?? null }),
  authOffline: (username: string) => call<AccountSummary>("auth_offline_login", { username }),
  authYggdrasil: (username: string, password: string) =>
    call<AccountSummary>("auth_yggdrasil_login", { username, password }),
  accountsList: () => call<AccountSummary[]>("accounts_list"),
  accountsSelect: (uuid: string) => call<void>("accounts_select", { uuid }),
  accountsRemove: (uuid: string) => call<void>("accounts_remove", { uuid }),
  accountsRefresh: (uuid: string) => call<AccountSummary>("accounts_refresh", { uuid }),
  /** Fires when the backend renewed (or invalidated) the stored sessions. */
  onAccountsChanged: (handler: (accounts: AccountSummary[]) => void): Promise<UnlistenFn> =>
    listen<AccountSummary[]>("accounts://changed", (event) => handler(event.payload)),
  skinGet: (uuid: string, refresh = false) => call<SkinData>("skin_get", { uuid, refresh }),

  // ── Skin library
  skinLibraryList: () => call<LibrarySkin[]>("skin_library_list"),
  /** Reads a picked PNG as a data URL without storing it (editor preview). */
  skinFilePreview: (path: string) => call<string>("skin_file_preview", { path }),
  skinCapesList: (uuid: string) => call<CapeOption[]>("skin_capes_list", { uuid }),
  skinLibraryImport: (path: string, draft: SkinDraft) =>
    call<LibrarySkin>("skin_library_import", { path, name: draft.name, variant: draft.variant, capeId: draft.capeId }),
  skinLibraryAddCurrent: (uuid: string, name?: string) =>
    call<LibrarySkin>("skin_library_add_current", { uuid, name: name ?? null }),
  /** Saves the whole editable state; `path` replaces the texture. */
  skinLibraryUpdate: (id: string, draft: SkinDraft, path: string | null = null) =>
    call<LibrarySkin>("skin_library_update", { id, name: draft.name, variant: draft.variant, capeId: draft.capeId, path }),
  skinLibraryRemove: (id: string) => call<void>("skin_library_remove", { id }),
  /** Uploads a library skin to the Minecraft profile of the account. */
  skinApply: (uuid: string, id: string) => call<SkinData>("skin_apply", { uuid, id }),
  skinReset: (uuid: string) => call<SkinData>("skin_reset", { uuid }),

  // ── Java
  javaDetect: () => call<JavaInstall[]>("java_detect"),
  javaProbe: (path: string) => call<JavaInstall>("java_probe", { path }),
  javaRequired: (instanceId: string) => call<JavaRequirement>("java_required", { instanceId }),

  // ── Instances & game
  instancesList: () => call<Instance[]>("instances_list"),
  instancesStatus: () => call<InstanceStatus[]>("instances_status"),
  instanceStatus: (instanceId: string) => call<InstanceStatus>("instance_status", { instanceId }),
  instanceInstall: (instanceId: string, onEvent: (event: LaunchEvent) => void) => {
    const channel = new Channel<LaunchEvent>();
    channel.onmessage = onEvent;
    return call<void>("instance_install", { instanceId, onEvent: channel });
  },
  instanceLaunch: (instanceId: string, onEvent: (event: LaunchEvent) => void) => {
    const channel = new Channel<LaunchEvent>();
    channel.onmessage = onEvent;
    return call<void>("instance_launch", { instanceId, onEvent: channel });
  },
  instanceCancel: () => call<boolean>("instance_cancel"),
  gameRunning: () => call<RunningGame | null>("game_running"),
  gameBusy: () => call<boolean>("game_busy"),
  gameKill: () => call<void>("game_kill"),

  // ── Status & updates
  serverStatus: (host: string, port: number | null) =>
    call<ServerStatus>("server_status", { host, port }),
  updateCheck: () => call<UpdateCheck>("update_check"),
  updateInstall: (onEvent: (event: UpdateEvent) => void) => {
    const channel = new Channel<UpdateEvent>();
    channel.onmessage = onEvent;
    return call<void>("update_install", { onEvent: channel });
  },
};
