/**
 * Typed view of `launcher.config.json` — the same file the Rust side embeds.
 * Nothing about the LuuxCraft user id, API or defaults is hard-coded in the UI.
 */
import raw from "../../launcher.config.json";

export interface LauncherConfig {
  userId: string;
  api: { baseUrl: string; timeoutSeconds: number };
  brand: {
    name: string;
    wordmark: { prefix: string; suffix: string };
    subtitle: string;
    website?: string | null;
  };
  dataDirectory: string;
  updater: { endpoints: string[] };
  auth: { yggdrasilServer?: string | null };
  news: { limit: number };
  serverStatus: { refreshSeconds: number; timeoutMs: number };
  downloads: { defaultConcurrency: number; maxConcurrency: number };
  memory: { defaultMinMb: number; defaultMaxMb: number };
  gameWindow: { defaultWidth: number; defaultHeight: number };
  links: { label: string; url: string; icon?: string | null }[];
  modules: Record<string, unknown>;
}

export const launcherConfig: LauncherConfig = raw as LauncherConfig;
