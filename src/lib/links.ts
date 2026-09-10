/**
 * Link presentation: the panel sends free-form links; known brands get their
 * icon (simple-icons, CC0), everything else a generic Material symbol.
 */
import {
  siBluesky,
  siCurseforge,
  siDiscord,
  siFacebook,
  siGithub,
  siInstagram,
  siKofi,
  siMastodon,
  siModrinth,
  siPatreon,
  siReddit,
  siSteam,
  siTelegram,
  siThreads,
  siTiktok,
  siTwitch,
  siX,
  siYoutube,
} from "simple-icons";

import type { Link } from "./types";

export interface LinkPresentation {
  /** SVG path (24×24) of a brand icon. */
  brandPath?: string;
  brandColor?: string;
  /** Material symbol name when there is no brand icon. */
  symbol: string;
}

const BRANDS: { keys: string[]; icon: { path: string; hex: string } }[] = [
  { keys: ["discord"], icon: siDiscord },
  { keys: ["youtube", "yt"], icon: siYoutube },
  { keys: ["x", "twitter", "x-twitter", "xtwitter"], icon: siX },
  { keys: ["instagram", "insta"], icon: siInstagram },
  { keys: ["tiktok"], icon: siTiktok },
  { keys: ["twitch"], icon: siTwitch },
  { keys: ["github"], icon: siGithub },
  { keys: ["facebook", "fb"], icon: siFacebook },
  { keys: ["reddit"], icon: siReddit },
  { keys: ["telegram"], icon: siTelegram },
  { keys: ["steam"], icon: siSteam },
  { keys: ["bluesky"], icon: siBluesky },
  { keys: ["mastodon"], icon: siMastodon },
  { keys: ["threads"], icon: siThreads },
  { keys: ["kofi", "ko-fi"], icon: siKofi },
  { keys: ["patreon"], icon: siPatreon },
  { keys: ["curseforge"], icon: siCurseforge },
  { keys: ["modrinth"], icon: siModrinth },
];

const SYMBOLS: { keys: string[]; symbol: string }[] = [
  { keys: ["website", "site", "web", "home", "language"], symbol: "language" },
  { keys: ["shop", "store", "boutique", "payments"], symbol: "shopping_bag" },
  { keys: ["wiki", "encyclopedia"], symbol: "menu_book" },
  { keys: ["docs", "documentation", "doc", "help", "faq"], symbol: "description" },
  { keys: ["forum", "community", "communaute", "communauté"], symbol: "forum" },
  { keys: ["map", "dynmap", "carte"], symbol: "map" },
  { keys: ["vote"], symbol: "how_to_vote" },
  { keys: ["support", "ticket"], symbol: "support_agent" },
  { keys: ["mail", "email", "contact"], symbol: "mail" },
  { keys: ["rules", "regles", "règles"], symbol: "gavel" },
  { keys: ["news", "blog"], symbol: "article" },
  { keys: ["download", "telechargement"], symbol: "download" },
];

function hostOf(url: string): string {
  try {
    return new URL(url).hostname.toLowerCase().replace(/^www\./, "");
  } catch {
    return "";
  }
}

const HOST_HINTS: [string, string][] = [
  ["discord.gg", "discord"],
  ["discord.com", "discord"],
  ["youtube.com", "youtube"],
  ["youtu.be", "youtube"],
  ["twitter.com", "x"],
  ["x.com", "x"],
  ["instagram.com", "instagram"],
  ["tiktok.com", "tiktok"],
  ["twitch.tv", "twitch"],
  ["github.com", "github"],
  ["facebook.com", "facebook"],
  ["reddit.com", "reddit"],
  ["t.me", "telegram"],
  ["telegram.me", "telegram"],
  ["steamcommunity.com", "steam"],
  ["bsky.app", "bluesky"],
  ["threads.net", "threads"],
  ["ko-fi.com", "kofi"],
  ["patreon.com", "patreon"],
  ["curseforge.com", "curseforge"],
  ["modrinth.com", "modrinth"],
];

/** Finds the best presentation from the icon key, then the label, then the host. */
export function presentLink(link: Link): LinkPresentation {
  const candidates = [link.icon ?? "", link.label ?? ""].map((s) => s.trim().toLowerCase());
  const host = hostOf(link.url);
  for (const [suffix, key] of HOST_HINTS) {
    if (host === suffix || host.endsWith(`.${suffix}`)) candidates.push(key);
  }
  for (const candidate of candidates) {
    if (!candidate) continue;
    const brand = BRANDS.find((entry) => entry.keys.includes(candidate));
    if (brand) return { brandPath: brand.icon.path, brandColor: `#${brand.icon.hex}`, symbol: "link" };
  }
  for (const candidate of candidates) {
    if (!candidate) continue;
    const symbol = SYMBOLS.find((entry) => entry.keys.some((key) => candidate === key || candidate.includes(key)));
    if (symbol) return { symbol: symbol.symbol };
  }
  // An explicit Material symbol name from the panel is used as is.
  if (link.icon && /^[a-z0-9_]+$/.test(link.icon)) return { symbol: link.icon };
  return { symbol: "link" };
}
