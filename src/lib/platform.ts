/**
 * Système hôte, tel que l'interface en a besoin.
 *
 * La barre de titre est dessinée par le launcher (`decorations: false`) : c'est
 * donc à lui de placer les boutons de fenêtre là où le joueur les attend — les
 * feux à gauche sur macOS, les symboles à droite ailleurs. La source de vérité
 * est le système rapporté par le noyau Rust (`std::env::consts::OS`) ; avant
 * qu'il ait répondu, ou si la valeur est inconnue, le navigateur suffit.
 */

import { useAppState } from "../store/AppStore";

export type Platform = "macos" | "windows" | "linux";

function normalize(value: string | null | undefined): Platform | null {
  if (value === "macos" || value === "windows" || value === "linux") return value;
  return null;
}

/** Devine le système depuis le navigateur, faute de réponse du noyau. */
export function detectPlatform(): Platform {
  const hint = `${navigator.platform ?? ""} ${navigator.userAgent ?? ""}`;
  if (/Mac|iPhone|iPad/i.test(hint)) return "macos";
  if (/Linux|X11/i.test(hint) && !/Android/i.test(hint)) return "linux";
  return "windows";
}

export function usePlatform(): Platform {
  const { bootstrap } = useAppState();
  return normalize(bootstrap?.system.platform) ?? detectPlatform();
}
