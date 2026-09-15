/**
 * Visual identity of this launcher.
 *
 * The panel can publish its own (`brand` in `/config`), and it wins as soon as
 * the snapshot arrives. The constants below are what the title bar paints from
 * the very first frame, before any request has completed, and what fills in
 * any field the panel leaves out.
 */
import type { RemoteBrand } from "../lib/types";

export interface Brand {
  name: string;
  /** The title bar paints `prefix` plain and `suffix` in the brand gradient. */
  wordmark: { prefix: string; suffix: string };
  subtitle: string;
  website: string | null;
}

export const builtInBrand: Brand = {
  name: "LuuxCraft",
  wordmark: { prefix: "Luux", suffix: "Craft" },
  subtitle: "Launcher",
  website: "https://luuxcraft.fr",
};

/** Merges what the panel publishes over the built-in identity. */
export function resolveBrand(remote: RemoteBrand | null | undefined): Brand {
  if (!remote) return builtInBrand;
  const name = remote.name ?? builtInBrand.name;
  // A panel that sends a name but no wordmark gets the whole name in the
  // gradient-free half rather than an arbitrary split.
  const prefix = remote.prefix ?? (remote.name ? name : builtInBrand.wordmark.prefix);
  const suffix = remote.suffix ?? (remote.name ? "" : builtInBrand.wordmark.suffix);
  return {
    name,
    wordmark: { prefix, suffix },
    subtitle: remote.subtitle ?? builtInBrand.subtitle,
    website: remote.website ?? builtInBrand.website,
  };
}
