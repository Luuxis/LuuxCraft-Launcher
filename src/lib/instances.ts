/** Pure helpers around panel instances. */
import type { Instance } from "./types";

/** Whether `player` may see/play the instance (panel whitelist). */
export function instanceAllows(instance: Instance, player: string | null | undefined): boolean {
  if (!instance.whitelistActive) return true;
  if (!player) return false;
  return instance.whitelist.includes(player);
}

export function allowedInstances(instances: Instance[], player: string | null | undefined): Instance[] {
  return instances.filter((instance) => instanceAllows(instance, player));
}

/** The instance to show by default: the remembered one when still allowed, else the first allowed. */
export function pickInstance(instances: Instance[], remembered: string | null | undefined): Instance | null {
  if (instances.length === 0) return null;
  const found = remembered ? instances.find((instance) => instance.id === remembered) : undefined;
  return found ?? instances[0] ?? null;
}

export const LOADER_LABELS: Record<string, string> = {
  forge: "Forge",
  neoforge: "NeoForge",
  fabric: "Fabric",
  legacyfabric: "Legacy Fabric",
  quilt: "Quilt",
  mcp: "MCP",
  none: "Vanilla",
  vanilla: "Vanilla",
};

export const LOADER_ICONS: Record<string, string> = {
  forge: "construction",
  neoforge: "precision_manufacturing",
  fabric: "grid_view",
  legacyfabric: "grid_view",
  quilt: "view_quilt",
  mcp: "code_blocks",
  none: "deployed_code",
  vanilla: "deployed_code",
};

export function loaderLabel(kind: string): string {
  const key = kind.trim().toLowerCase();
  return LOADER_LABELS[key] ?? (key ? key.charAt(0).toUpperCase() + key.slice(1) : "Vanilla");
}

export function loaderIcon(kind: string): string {
  return LOADER_ICONS[kind.trim().toLowerCase()] ?? "deployed_code";
}

/**
 * Module toggles published by the panel. A module the panel says nothing about
 * is shown: the launcher ships no defaults of its own.
 */
export function moduleEnabled(name: string, remote: Record<string, unknown> | undefined): boolean {
  const value = remote?.[name];
  if (value === undefined || value === null) return true;
  if (typeof value === "boolean") return value;
  if (typeof value === "object") {
    const enabled = (value as { enabled?: unknown }).enabled;
    return enabled === undefined ? true : Boolean(enabled);
  }
  if (typeof value === "string") return !["false", "0", "off", "no"].includes(value.toLowerCase());
  if (typeof value === "number") return value !== 0;
  return true;
}
