/**
 * Minimal i18n: one dictionary today (fr), a `t()` helper with `{param}`
 * interpolation and a pluralization shortcut. Adding a language means adding
 * a dictionary with the same shape.
 */
import { fr } from "./fr";
import type { AppError } from "../lib/types";

type Params = Record<string, string | number>;

function lookup(path: string): unknown {
  return path.split(".").reduce<unknown>((node, key) => {
    if (node && typeof node === "object" && key in (node as Record<string, unknown>)) {
      return (node as Record<string, unknown>)[key];
    }
    return undefined;
  }, fr);
}

export function t(key: string, params?: Params): string {
  const value = lookup(key);
  let text = typeof value === "string" ? value : key;
  if (params) {
    for (const [name, raw] of Object.entries(params)) {
      text = text.replaceAll(`{${name}}`, String(raw));
    }
    if ("count" in params && !("plural" in params)) {
      text = text.replaceAll("{plural}", Number(params.count) > 1 ? "s" : "");
    }
  }
  return text;
}

/** Readable message for a backend error code. */
export function describeError(error: AppError | null | undefined): string {
  if (!error) return t("errors.unknown");
  const key = `errors.${error.code}`;
  const known = lookup(key);
  if (typeof known === "string") {
    return known.replaceAll("{message}", error.message);
  }
  return error.message || t("errors.unknown");
}

export const dictionary = fr;
