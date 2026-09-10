/** Display helpers (fr-FR). */

const units = ["o", "Ko", "Mo", "Go", "To"];

export function formatBytes(bytes: number, decimals = 1): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "0 o";
  if (bytes < 1024) return `${Math.round(bytes)} o`;
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** index;
  return `${value.toFixed(index === 0 ? 0 : decimals).replace(".", ",")} ${units[index]}`;
}

export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

export function formatDuration(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds < 0) return "—";
  const seconds = Math.round(totalSeconds);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  if (hours > 0) return `${hours} h ${minutes.toString().padStart(2, "0")} min`;
  if (minutes > 0) return `${minutes} min ${rest.toString().padStart(2, "0")} s`;
  return `${rest} s`;
}

export function formatMemory(mb: number): string {
  if (mb >= 1024 && mb % 256 === 0) return `${(mb / 1024).toString().replace(".", ",")} Go`;
  if (mb >= 1024) return `${(mb / 1024).toFixed(1).replace(".", ",")} Go`;
  return `${mb} Mo`;
}

export function formatPercent(part: number, total: number): number {
  if (!total || total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((part / total) * 100)));
}

const dateFormatter = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "long", year: "numeric" });
const dateTimeFormatter = new Intl.DateTimeFormat("fr-FR", {
  day: "numeric",
  month: "short",
  hour: "2-digit",
  minute: "2-digit",
});

export function formatDate(value: string | number | null | undefined): string | null {
  if (value === null || value === undefined || value === "") return null;
  const date = typeof value === "number" ? new Date(value * 1000) : new Date(value);
  if (Number.isNaN(date.getTime())) return typeof value === "string" ? value : null;
  return dateFormatter.format(date);
}

export function formatDateTime(value: string | number | null | undefined): string | null {
  if (value === null || value === undefined || value === "") return null;
  const date = typeof value === "number" ? new Date(value * 1000) : new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return dateTimeFormatter.format(date);
}

export function relativeTime(unixSeconds: number, now = Date.now()): string {
  const delta = Math.max(0, Math.round(now / 1000 - unixSeconds));
  if (delta < 45) return "à l'instant";
  if (delta < 3600) return `il y a ${Math.round(delta / 60)} min`;
  if (delta < 86400) return `il y a ${Math.round(delta / 3600)} h`;
  return `il y a ${Math.round(delta / 86400)} j`;
}

/** Strips HTML tags for previews. */
export function textPreview(html: string, max = 160): string {
  const text = html
    .replace(/<style[\s\S]*?<\/style>/gi, "")
    .replace(/<script[\s\S]*?<\/script>/gi, "")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/\s+/g, " ")
    .trim();
  return text.length > max ? `${text.slice(0, max).trimEnd()}…` : text;
}

export function initials(name: string): string {
  return name.trim().slice(0, 1).toUpperCase() || "?";
}
