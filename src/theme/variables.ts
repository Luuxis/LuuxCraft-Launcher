/**
 * Application des variables de thème sur la fenêtre.
 *
 * Le panel publie déjà un dictionnaire de variables tout calculé. Le launcher
 * ne s'en sert pourtant que comme repli, et recalcule les siennes à partir des
 * tokens : c'est ce qui lui permet d'honorer la préférence clair/sombre du
 * joueur quand le propriétaire n'a pas imposé de thème. Le calcul est le même
 * des deux côtés — `tokens.ts` est une copie à l'identique du fichier du
 * panel, et un test y veille —, donc il n'y a pas deux résultats possibles.
 *
 * Les variables sont posées en style en ligne sur `<html>`. Elles gagnent ainsi
 * sur les défauts de la feuille de style sans avoir à en surenchérir la
 * spécificité, et un thème retiré se défait en une boucle.
 */

import { buildThemeVariables, normalizeTokens, type ThemeMode, type ThemeTokens } from './tokens';

/**
 * Ce que le panel renvoie sur `/theme`.
 *
 * `document` reste `unknown` : ce module n'a besoin que de ses tokens, et lui
 * imposer le type complet obligerait chaque appelant à valider le document
 * avant même de pouvoir appliquer une couleur.
 */
export interface RemoteTheme {
    schemaVersion: number;
    document: unknown;
    variables: Record<string, string>;
}

let applied: string[] = [];

/**
 * Tokens effectifs du launcher.
 *
 * Trois sources, par ordre de priorité décroissante : le document composé par
 * le propriétaire, la couleur d'accentuation seule qu'il a réglée, puis les
 * défauts du moteur. `preferred` n'entre en jeu que dans les deux derniers
 * cas : quand un thème a été dessiné, son mode fait partie de la composition
 * et le joueur ne le retourne pas — un fond photographique choisi pour du
 * sombre ne devient pas clair sur un clic de préférence.
 */
export function resolveTokens(
    theme: RemoteTheme | null | undefined,
    accentColor: string | null | undefined,
    preferred: ThemeMode,
): ThemeTokens {
    const document_ = theme?.document;
    const authored = document_ && typeof document_ === 'object' ? (document_ as { tokens?: unknown }).tokens : null;
    if (authored) return normalizeTokens(authored);

    return normalizeTokens({ accent: accentColor ?? undefined, mode: preferred });
}

/** Pose un dictionnaire de variables sur `<html>`, en retirant les précédentes. */
export function applyVariables(variables: Record<string, string>): void {
    const root = document.documentElement;

    // Les variables d'un thème précédent qui ne figurent plus dans le nouveau
    // resteraient sinon en place : un thème sans halo garderait celui d'avant.
    for (const name of applied) {
        if (!(name in variables)) root.style.removeProperty(name);
    }

    const next: string[] = [];
    for (const [name, value] of Object.entries(variables)) {
        if (!name.startsWith('--')) continue;
        root.style.setProperty(name, value);
        next.push(name);
    }
    applied = next;

    if (variables['color-scheme']) root.style.colorScheme = variables['color-scheme'];
}

/**
 * Calcule et applique le thème. Renvoie les tokens retenus, dont le rendu a
 * besoin pour résoudre les polices.
 */
export function applyTheme(
    theme: RemoteTheme | null | undefined,
    accentColor: string | null | undefined,
    preferred: ThemeMode,
): ThemeTokens {
    const tokens = resolveTokens(theme, accentColor, preferred);
    applyVariables(buildThemeVariables(tokens));
    return tokens;
}

/**
 * Pile de polices d'un token de typographie.
 *
 * Les trois noms réservés renvoient aux variables du thème ; toute autre valeur
 * est traitée comme un nom de famille, cité et bordé d'un repli. La citation
 * n'est pas cosmétique : cette chaîne finit dans une déclaration `font-family`.
 */
export function fontStackFor(token: string): string {
    if (token === 'display') return 'var(--font-display-stack)';
    if (token === 'mono') return 'var(--font-mono-stack)';
    if (token === 'body') return 'var(--font-body-stack)';
    return /^[A-Za-z0-9 _-]{1,48}$/.test(token) ? `"${token}", var(--font-body-stack)` : 'var(--font-body-stack)';
}
