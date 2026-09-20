/**
 * DTOs shared with the Rust backend (serde `camelCase`).
 */

/**
 * The launcher's own identity and defaults, built in the backend (see
 * `config.rs`). Everything the panel publishes lives in `RemoteConfig`
 * instead; this only carries what the API cannot provide about itself.
 */
export interface LauncherConfig {
  /** `users.id` du tenant, tel que le pack client l'a figé. */
  userId: string;
  /** Slug du tenant : nom de son dossier de données. */
  slug: string;
  displayName: string;
  api: { baseUrl: string; timeoutSeconds: number };
  dataDirectory: string;
  auth: { yggdrasilServer: string | null };
  news: { limit: number };
  serverStatus: { refreshSeconds: number; timeoutMs: number };
  downloads: { defaultConcurrency: number; maxConcurrency: number };
  memory: { defaultMinMb: number; defaultMaxMb: number };
  gameWindow: { defaultWidth: number; defaultHeight: number };
}

export interface AppError {
  code: string;
  message: string;
  details?: string;
}

// ── Settings ────────────────────────────────────────────────────────────────

export type LauncherBehavior = "stay" | "hide" | "bound";
export type JavaMode = "auto" | "custom";

export interface MemorySettings {
  minMb: number;
  maxMb: number;
}

export interface JavaSettings {
  mode: JavaMode;
  path: string | null;
}

export interface InstanceSettings {
  memory: MemorySettings | null;
  java: JavaSettings | null;
  jvmArgs: string[] | null;
}

export interface Settings {
  selectedAccount: string | null;
  selectedInstance: string | null;
  installPath: string | null;
  resolvedDataDirectory: string | null;
  downloadConcurrency: number;
  memory: MemorySettings;
  gameWindow: { width: number | null; height: number | null; fullscreen: boolean };
  launcherBehavior: LauncherBehavior;
  java: JavaSettings;
  jvmArgs: string[];
  intelEnabledMac: boolean;
  instances: Record<string, InstanceSettings>;
  ui: { theme: "dark" | "light" | "system"; reduceMotion: boolean };
  serverStatusRefreshSeconds: number;
}

// ── Accounts ────────────────────────────────────────────────────────────────

export type AccountKind = "microsoft" | "azAuth" | "offline" | "yggdrasil";

export interface AccountSummary {
  uuid: string;
  name: string;
  kind: AccountKind;
  online: boolean;
  skinUrl: string | null;
  skinVariant: string | null;
  skinDataUrl: string | null;
  capeUrl: string | null;
  capeAlias: string | null;
  expiresAt: number | null;
  needsReauth: boolean;
  addedAt: number;
  gamertag: string | null;
  ownership: string | null;
  extra: { banned: boolean | null; money: number | null; role: unknown; verified: boolean | null } | null;
}

export interface AuthMethods {
  microsoft: boolean;
  azauth: string | null;
  offline: boolean;
  yggdrasil: string | null;
}

export type AuthEvent = { event: "windowOpened" } | { event: "waiting" };

export type AzAuthOutcome = { status: "ok"; account: AccountSummary } | { status: "otpRequired" };

// ── Remote (panel) ──────────────────────────────────────────────────────────

export type AuthMode = { type: "microsoft" } | { type: "offline" } | { type: "azAuth"; url: string };

export interface Link {
  label: string;
  url: string;
  icon: string | null;
  order: number | null;
}

/** Launcher identity published by the panel; every field is optional. */
export interface RemoteBrand {
  name: string | null;
  prefix: string | null;
  suffix: string | null;
  subtitle: string | null;
  website: string | null;
  /** Client logo; the backend also makes it the window and taskbar icon. */
  iconUrl: string | null;
}

/** Habillage publié par le panel (`GET /theme`). */
export interface RemoteTheme {
  schemaVersion: number;
  /**
   * Mise en page composée par le propriétaire, `null` s'il n'en a pas.
   *
   * Typé `unknown` ici : son schéma vit dans `src/theme/schema.ts`, copie à
   * l'identique de celui du panel. Le faire transiter par ce fichier de DTO
   * obligerait à le dupliquer une fois de plus.
   */
  document: unknown;
  /** Variables CSS calculées par le panel, repli du calcul local. */
  variables: Record<string, string>;
}

export interface RemoteConfig {
  maintenance: boolean;
  maintenanceMessage: string | null;
  dataDirectory: string | null;
  auth: AuthMode;
  clientId: string | null;
  links: Link[];
  modules: Record<string, unknown>;
  brand: RemoteBrand | null;
  yggdrasil: string | null;
  /** Couleur d'accentuation, appliquée même avant l'arrivée du thème. */
  accentColor: string | null;
  extra: Record<string, unknown>;
}

export interface Article {
  id: string;
  title: string;
  content: string;
  author: string | null;
  publishedAt: string | null;
  image: string | null;
  url: string | null;
  order: number | null;
  extra: Record<string, unknown>;
}

export interface Instance {
  id: string;
  name: string;
  description: string | null;
  image: string | null;
  filesUrl: string | null;
  minecraftVersion: string;
  loader: { kind: string; version: string; mcpFile: string | null };
  verify: boolean;
  ignored: string[];
  whitelist: string[];
  whitelistActive: boolean;
  server: { name: string | null; host: string; port: number | null } | null;
  javaVersion: string | null;
  jvmArgs: string[];
  memory: { minMb: number | null; maxMb: number | null } | null;
  order: number | null;
  extra: Record<string, unknown>;
}

export interface RemoteSnapshot {
  config: RemoteConfig;
  instances: Instance[];
  articles: Article[];
  links: Link[];
  theme: RemoteTheme | null;
  partialErrors: { part: string; code: string; message: string }[];
  fetchedAt: number;
  stale: boolean;
}

// ── Instances & game ────────────────────────────────────────────────────────

export interface InstanceStatus {
  id: string;
  installed: boolean;
  gameDir: string;
  hasGameDir: boolean;
}

export type LaunchEvent =
  | { event: "stage"; data: { stage: string } }
  | { event: "check"; data: { checked: number; total: number; element: string } }
  | { event: "progress"; data: { downloaded: number; total: number; element: string } }
  | { event: "speed"; data: { bytesPerSecond: number } }
  | { event: "estimated"; data: { seconds: number } }
  | { event: "extract"; data: { file: string } }
  | { event: "patch"; data: { line: string } }
  | { event: "warning"; data: { message: string } }
  | { event: "installed" }
  | { event: "started"; data: { pid: number | null } }
  | { event: "log"; data: { stream: "stdout" | "stderr"; lines: string[] } }
  | { event: "exited"; data: { code: number | null; success: boolean } };

export interface RunningGame {
  instanceId: string;
  instanceName: string;
  pid: number | null;
  startedAt: number;
  bound: boolean;
  hiddenLauncher: boolean;
}

// ── Java ────────────────────────────────────────────────────────────────────

export interface JavaInstall {
  path: string;
  version: string;
  major: number;
  vendor: string;
  arch: string;
  source: string;
  managed: boolean;
}

export interface JavaRequirement {
  instanceId: string;
  minecraftVersion: string;
  major: number;
  source: "panel" | "mojang";
}

// ── Status & système ────────────────────────────────────────────────────────

export interface ServerStatus {
  online: boolean;
  host: string;
  port: number;
  players: number;
  maxPlayers: number;
  latencyMs: number | null;
  version: string | null;
  description: string | null;
  favicon: string | null;
  sample: string[];
  error: string | null;
  checkedAt: number;
}

export interface SystemInfo {
  totalMemoryMb: number | null;
  platform: string;
  arch: string;
  launcherVersion: string;
  /** Where accounts.json lives (the game location). */
  accountsFile: string;
  debug: boolean;
}

export interface Bootstrap {
  config: LauncherConfig;
  settings: Settings;
  accounts: AccountSummary[];
  system: SystemInfo;
  paths: { launcherDir: string; logsDir: string; gameRoot: string };
}

export interface SkinData {
  uuid: string;
  name: string;
  skin: string | null;
  cape: string | null;
  model: "default" | "slim" | "auto";
  capeAlias: string | null;
  source: AccountKind;
  /** Whether the skin of this account can be changed from the launcher. */
  canChange: boolean;
}

export type SkinVariant = "classic" | "slim";

/** One skin of the local library, with its texture as a data URL. */
export interface LibrarySkin {
  id: string;
  name: string;
  variant: SkinVariant;
  /** The cape worn with this skin, `null` meaning "no cape". */
  capeId: string | null;
  addedAt: number;
  texture: string;
}

/** A cape the account owns. */
export interface CapeOption {
  id: string;
  alias: string | null;
  texture: string;
  active: boolean;
}
