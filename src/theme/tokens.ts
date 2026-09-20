/**
 * Jeu de tokens d'un launcher, et sa traduction en variables CSS.
 *
 * Le launcher n'embarque plus aucune couleur de marque : il applique le
 * dictionnaire produit ici sur `:root` et tout son habillage suit. C'est ce qui
 * permet de changer l'accentuation — ou de repeindre entièrement les surfaces —
 * sans recompiler ni redéployer un seul binaire.
 *
 * Les noms de variables sont exactement ceux que `src/styles/index.css` du
 * launcher consomme. Ce fichier et cette feuille sont les deux moitiés d'un
 * même contrat : ajouter une variable ici sans l'utiliser là ne fait rien,
 * l'utiliser là sans la produire ici laisse la déclaration vide.
 */

import {
    alphaColor,
    buildRamp,
    gamutMap,
    mixHex,
    normalizeHex,
    oklchToRgb,
    parseHex,
    readableOn,
    rgbToOklch,
    toHex,
    type ColorRamp,
    type Oklch,
} from './color';

/** Familles embarquées dans le launcher : disponibles hors ligne, sans requête. */
export const BUNDLED_FONTS = [
    'Inter',
    'Space Grotesk',
    'JetBrains Mono',
    'Poppins',
    'Rubik',
    'Outfit',
    'Sora',
    'Bebas Neue',
    'Minecraftia',
] as const;

export type BundledFont = (typeof BUNDLED_FONTS)[number];

export type ThemeMode = 'dark' | 'light';

export interface FontTokens {
    /** Titres et éléments d'accroche. */
    display: string;
    /** Corps de texte, boutons, champs. */
    body: string;
    /** Console du jeu, chemins, valeurs techniques. */
    mono: string;
}

export interface RadiusTokens {
    sm: number;
    md: number;
    lg: number;
    xl: number;
    /** Rayon des pastilles et boutons arrondis ; 9999 = totalement rond. */
    pill: number;
}

export interface EffectTokens {
    /** Intensité des halos colorés, 0 (aucun) → 1 (marqué). */
    glow: number;
    /** Flou des surfaces en verre, en pixels. */
    blur: number;
    /** Opacité du quadrillage de fond, 0 → 1. */
    grid: number;
}

export interface ThemeTokens {
    mode: ThemeMode;
    /** Couleur d'accentuation principale : boutons, états actifs, focus. */
    accent: string;
    /** Accentuation secondaire, utilisée dans les dégradés et les surbrillances. */
    accent2: string;
    /** Teinte de base de toutes les surfaces (fonds, cartes, barres). */
    surface: string;
    /** Couleur du texte le plus contrasté ; le reste de l'échelle en découle. */
    text: string;
    danger: string;
    warning: string;
    success: string;
    fonts: FontTokens;
    radius: RadiusTokens;
    effects: EffectTokens;
}

export const DEFAULT_TOKENS: ThemeTokens = {
    mode: 'dark',
    accent: '#22c55e',
    accent2: '#22d3ee',
    surface: '#0a0e13',
    text: '#ffffff',
    danger: '#ef4444',
    warning: '#f59e0b',
    success: '#22c55e',
    fonts: { display: 'Space Grotesk', body: 'Inter', mono: 'JetBrains Mono' },
    radius: { sm: 8, md: 12, lg: 14, xl: 16, pill: 9999 },
    effects: { glow: 0.55, blur: 20, grid: 0.4 },
};

/**
 * La charte : les couleurs exactes de la feuille de style intégrée du launcher,
 * qui sont aussi celles du panel (« obsidienne froide »).
 *
 * Elle n'est pas une palette figée mais un **étalon**. Le jeu de tokens par
 * défaut la reproduit au pixel — un client qui n'a rien changé retrouve son
 * launcher tel qu'il le connaît —, et un jeu personnalisé en conserve les
 * rapports : les mêmes écarts de clarté entre les surfaces, les mêmes
 * proportions de chroma, la même façon dont les gris empruntent la teinte du
 * fond. Dériver « à +2 % de clarté » sans étalon donnait des gris neutres et
 * ternes là où la charte a des gris ardoise ; c'est ce que ce tableau corrige.
 */
const CHARTER = {
    surface: {
        deep: '#05080c',
        primary: '#0a0e13',
        secondary: '#0f141b',
        tertiary: '#161d26',
        elevated: '#1d2632',
        sidebar: '#040609',
        footer: '#03060a',
    },
    border: '#1f2a36',
    text: '#ffffff',
    /** Gris de texte, du plus clair au plus sombre. `slate` sert aux bordures. */
    grey: {
        secondaryButton: '#e5e7eb',
        tertiary: '#e2e8f0',
        label: '#d1d5db',
        body: '#9ca3af',
        slate: '#94a3b8',
        meta: '#6b7280',
        placeholder: '#4b5563',
    },
} as const;

function oklchOf(hex: string): Oklch {
    return rgbToOklch(parseHex(hex)!);
}

/** Point de référence de la charte : sa surface principale et son texte. */
const REFERENCE_SURFACE = oklchOf(CHARTER.surface.primary);
const REFERENCE_TEXT = oklchOf(CHARTER.text);

function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

/**
 * Échelle de surfaces dérivée d'une seule teinte.
 *
 * Chaque surface de la charte est exprimée **relativement** à la surface
 * principale — écart de clarté, rapport de chroma, écart de teinte — puis ces
 * écarts sont rejoués sur la teinte choisie. Une surface crème, un fond
 * anthracite et l'obsidienne d'origine produisent ainsi la même hiérarchie,
 * chacune dans sa couleur ; et la teinte d'origine redonne exactement la
 * charte.
 */
function surfaceScale(surface: string, mode: ThemeMode): Record<string, string> {
    const seed = oklchOf(normalizeHex(surface) ?? DEFAULT_TOKENS.surface);

    // En sombre, les surfaces « montent » vers la lumière quand elles
    // s'élèvent ; en clair elles descendent. Le signe est le seul paramètre qui
    // change, la hiérarchie reste la même.
    const dir = mode === 'dark' ? 1 : -1;
    const at = (hex: string) => shiftedSurface(seed, oklchOf(hex), dir);

    const tertiary = at(CHARTER.surface.tertiary);
    return {
        '--bg-deep': at(CHARTER.surface.deep),
        '--bg-primary': at(CHARTER.surface.primary),
        '--bg-secondary': at(CHARTER.surface.secondary),
        '--bg-tertiary': tertiary,
        '--bg-elevated': at(CHARTER.surface.elevated),
        '--bg-sidebar': at(CHARTER.surface.sidebar),
        '--bg-footer': at(CHARTER.surface.footer),
        '--bg-card': alphaColor(tertiary, mode === 'dark' ? 0.85 : 0.9),
    };
}

/** Une couleur de la charte, transposée depuis la surface de référence vers `seed`. */
function shiftedSurface(seed: Oklch, target: Oklch, dir: 1 | -1): string {
    // La référence a une chroma faible mais non nulle ; le rapport transmet la
    // façon dont la charte sature un peu plus ses surfaces à mesure qu'elles
    // s'éclaircissent. Un fond gris neutre (chroma nulle) reste neutre.
    const chromaRatio = REFERENCE_SURFACE.c > 0.001 ? target.c / REFERENCE_SURFACE.c : 1;
    return toHex(
        gamutMap({
            l: clamp(seed.l + (target.l - REFERENCE_SURFACE.l) * dir, 0.02, 0.99),
            c: seed.c * chromaRatio,
            h: seed.h + (target.h - REFERENCE_SURFACE.h),
        }),
    );
}

/**
 * Échelle de texte dérivée de la couleur la plus contrastée.
 *
 * Chaque gris de la charte est repéré par sa position entre le texte et la
 * surface — « à 34 % du chemin vers le fond » — et par sa teinte ardoise, qui
 * est celle du fond. Sur une paire texte/surface différente, le gris garde sa
 * position sur le chemin, et emprunte la teinte de la nouvelle surface : des
 * gris chauds sur un fond brun, neutres sur un fond gris.
 */
function textScale(text: string, surface: string): Record<string, string> {
    const textHex = normalizeHex(text) ?? DEFAULT_TOKENS.text;
    const textSeed = oklchOf(textHex);
    const surfaceSeed = oklchOf(normalizeHex(surface) ?? DEFAULT_TOKENS.surface);
    const grey = (hex: string) => shiftedGrey(textSeed, surfaceSeed, oklchOf(hex));

    return {
        '--text-primary': textHex,
        '--text-strong': textHex,
        '--text-secondary': alphaColor(textHex, 0.85),
        '--text-label': grey(CHARTER.grey.label),
        '--text-secondary-button': grey(CHARTER.grey.secondaryButton),
        '--text-tertiary': alphaColor(grey(CHARTER.grey.tertiary), 0.65),
        '--text-body': grey(CHARTER.grey.body),
        '--text-muted': alphaColor(grey(CHARTER.grey.slate), 0.65),
        '--text-meta': grey(CHARTER.grey.meta),
        '--text-placeholder': grey(CHARTER.grey.placeholder),
    };
}

/** Un gris de la charte, transposé sur une autre paire texte/surface. */
function shiftedGrey(textSeed: Oklch, surfaceSeed: Oklch, target: Oklch): string {
    const span = REFERENCE_TEXT.l - REFERENCE_SURFACE.l;
    const toward = (REFERENCE_TEXT.l - target.l) / span;
    // Rapport borné : une surface très saturée ne doit pas produire des gris
    // qui n'en sont plus.
    const chromaRatio = REFERENCE_SURFACE.c > 0.001 ? clamp(surfaceSeed.c / REFERENCE_SURFACE.c, 0, 2) : 1;
    return toHex(
        gamutMap({
            l: clamp(textSeed.l - toward * (textSeed.l - surfaceSeed.l), 0.02, 0.99),
            c: target.c * chromaRatio,
            h: target.h + (surfaceSeed.h - REFERENCE_SURFACE.h),
        }),
    );
}

/** Bordures : la ligne neutre suit les surfaces, les états actifs l'accent. */
function borderScale(surface: string, text: string, accent: ColorRamp, mode: ThemeMode): Record<string, string> {
    const textSeed = oklchOf(normalizeHex(text) ?? DEFAULT_TOKENS.text);
    const surfaceSeed = oklchOf(normalizeHex(surface) ?? DEFAULT_TOKENS.surface);
    const slate = shiftedGrey(textSeed, surfaceSeed, oklchOf(CHARTER.grey.slate));
    return {
        '--border': shiftedSurface(surfaceSeed, oklchOf(CHARTER.border), mode === 'dark' ? 1 : -1),
        '--border-subtle': alphaColor(slate, 0.1),
        '--border-default': alphaColor(slate, 0.18),
        '--border-strong': alphaColor(slate, 0.3),
        '--border-hover': alphaColor(accent[500], 0.45),
        '--border-focus': alphaColor(accent[500], 0.7),
    };
}

/**
 * Variables CSS complètes d'un thème.
 *
 * Renvoyées sous forme de dictionnaire plutôt que de texte CSS : l'éditeur les
 * pose une par une sur l'élément d'aperçu (`style.setProperty`) et le launcher
 * fait de même sur `:root`, sans jamais injecter de feuille de style construite
 * par concaténation — donc sans surface d'injection.
 */
export function buildThemeVariables(input: ThemeTokens): Record<string, string> {
    const tokens = normalizeTokens(input);

    const accent = buildRamp(tokens.accent) ?? buildRamp(DEFAULT_TOKENS.accent)!;
    const accent2 = buildRamp(tokens.accent2) ?? buildRamp(DEFAULT_TOKENS.accent2)!;
    const danger = buildRamp(tokens.danger) ?? buildRamp(DEFAULT_TOKENS.danger)!;
    const warning = buildRamp(tokens.warning) ?? buildRamp(DEFAULT_TOKENS.warning)!;
    const success = buildRamp(tokens.success) ?? buildRamp(DEFAULT_TOKENS.success)!;

    const glow = tokens.effects.glow;
    const surfaces = surfaceScale(tokens.surface, tokens.mode);
    const vars: Record<string, string> = {
        '--theme-mode': tokens.mode,
        'color-scheme': tokens.mode,

        // Rampe complète : l'éditeur y puise pour les remplissages de nœuds, et
        // le launcher pour ses états (survol, actif, désactivé).
        ...rampVariables('accent', accent),
        ...rampVariables('accent2', accent2),
        ...rampVariables('danger', danger),
        ...rampVariables('warning', warning),
        ...rampVariables('success', success),

        '--brand-primary': accent[500],
        '--brand-secondary': accent[600],
        '--brand-accent': accent2[400],
        '--brand-glow': alphaColor(accent[500], 0.4 * glow),
        '--on-brand': readableOn(rgbToOklch(parseHex(accent[500])!)),
        '--on-gold': readableOn(rgbToOklch(parseHex(warning[500])!)),

        ...surfaces,
        ...textScale(tokens.text, tokens.surface),
        ...borderScale(tokens.surface, tokens.text, accent, tokens.mode),

        // Le verre est une surface de la charte rendue translucide, jamais un
        // mélange ad hoc : c'est ce qui le garde dans la même famille de teinte.
        '--glass-bg': alphaColor(surfaces['--bg-secondary']!, 0.75),
        '--glass-bg-light': alphaColor(surfaces['--bg-tertiary']!, 0.55),
        '--glass-border': alphaColor(accent[500], 0.22),
        '--glass-blur': `blur(${tokens.effects.blur}px)`,

        '--glow-sm': `0 0 14px ${alphaColor(accent[500], 0.12 * glow)}`,
        '--glow': `0 0 28px ${alphaColor(accent[500], 0.18 * glow)}`,
        '--glow-lg': `0 0 56px ${alphaColor(accent[500], 0.28 * glow)}`,
        '--glow-strength': String(glow),
        '--grid-opacity': String(tokens.effects.grid),

        '--radius-sm': `${tokens.radius.sm}px`,
        '--radius-md': `${tokens.radius.md}px`,
        '--radius-lg': `${tokens.radius.lg}px`,
        '--radius-xl': `${tokens.radius.xl}px`,
        '--radius-2xl': `${Math.round(tokens.radius.xl * 1.4)}px`,
        '--radius-block': `${Math.max(2, Math.round(tokens.radius.sm * 0.75))}px`,
        '--radius-full': `${tokens.radius.pill}px`,

        '--font-sans': fontStack(tokens.fonts.body, 'system-ui, -apple-system, "Segoe UI", sans-serif'),
        '--font-display': fontStack(tokens.fonts.display, `"${tokens.fonts.body}", sans-serif`),
        '--font-mono': fontStack(tokens.fonts.mono, 'ui-monospace, "SF Mono", Menlo, monospace'),

        '--grad-brand': `linear-gradient(135deg, ${accent[500]} 0%, ${accent[600]} 60%, ${accent[700]} 100%)`,
        '--grad-brand-row': `linear-gradient(to right, ${accent[500]}, ${accent[600]}, ${accent[700]})`,
        '--grad-brand-row-hover': `linear-gradient(to right, ${accent[400]}, ${accent[500]}, ${accent[600]})`,
        '--grad-accent': `linear-gradient(to right, ${accent[400]}, ${accent2[400]})`,
        '--grad-progress': `linear-gradient(to right, ${accent[500]}, ${accent2[400]})`,
        '--grad-gold': `linear-gradient(to right, ${warning[500]}, ${warning[600]})`,
        '--grad-card': `linear-gradient(155deg, ${alphaColor(surfaces['--bg-tertiary']!, 0.92)} 0%, ${alphaColor(surfaces['--bg-primary']!, 0.85)} 100%)`,
    };

    return vars;
}

function rampVariables(name: string, ramp: ColorRamp): Record<string, string> {
    const out: Record<string, string> = {};
    for (const [step, value] of Object.entries(ramp)) {
        out[`--${name}-${step}`] = value;
    }
    return out;
}

/**
 * Pile de polices. Le nom choisi est cité entre guillemets ; toute valeur qui
 * n'est pas un nom de famille plausible est écartée au profit du repli, car
 * cette chaîne finit telle quelle dans une déclaration `font-family`.
 */
function fontStack(family: string, fallback: string): string {
    const safe = /^[A-Za-z0-9 _-]{1,48}$/.test(family) ? family : null;
    return safe ? `"${safe}", ${fallback}` : fallback;
}

function clampNumber(value: unknown, min: number, max: number, fallback: number): number {
    const n = Number(value);
    if (!Number.isFinite(n)) return fallback;
    return Math.min(max, Math.max(min, n));
}

function hexOr(value: unknown, fallback: string): string {
    return normalizeHex(value) ?? fallback;
}

function fontOr(value: unknown, fallback: string): string {
    if (typeof value !== 'string') return fallback;
    const trimmed = value.trim();
    return /^[A-Za-z0-9 _-]{1,48}$/.test(trimmed) ? trimmed : fallback;
}

/**
 * Normalise un jeu de tokens venu du client.
 *
 * Contrairement au reste du document, les tokens ne sont jamais rejetés : une
 * couleur illisible est remplacée par celle par défaut. Un thème doit pouvoir
 * s'enregistrer même à moitié rempli — c'est l'état normal d'un document en
 * cours d'édition —, et seules les valeurs qui finissent dans du CSS ont besoin
 * d'être verrouillées, ce que fait ce filtre.
 */
export function normalizeTokens(input: unknown): ThemeTokens {
    const raw = (input && typeof input === 'object' ? input : {}) as Record<string, unknown>;
    const fonts = (raw.fonts && typeof raw.fonts === 'object' ? raw.fonts : {}) as Record<string, unknown>;
    const radius = (raw.radius && typeof raw.radius === 'object' ? raw.radius : {}) as Record<string, unknown>;
    const effects = (raw.effects && typeof raw.effects === 'object' ? raw.effects : {}) as Record<string, unknown>;

    return {
        mode: raw.mode === 'light' ? 'light' : 'dark',
        accent: hexOr(raw.accent, DEFAULT_TOKENS.accent),
        accent2: hexOr(raw.accent2, DEFAULT_TOKENS.accent2),
        surface: hexOr(raw.surface, DEFAULT_TOKENS.surface),
        text: hexOr(raw.text, DEFAULT_TOKENS.text),
        danger: hexOr(raw.danger, DEFAULT_TOKENS.danger),
        warning: hexOr(raw.warning, DEFAULT_TOKENS.warning),
        success: hexOr(raw.success, DEFAULT_TOKENS.success),
        fonts: {
            display: fontOr(fonts.display, DEFAULT_TOKENS.fonts.display),
            body: fontOr(fonts.body, DEFAULT_TOKENS.fonts.body),
            mono: fontOr(fonts.mono, DEFAULT_TOKENS.fonts.mono),
        },
        radius: {
            sm: Math.round(clampNumber(radius.sm, 0, 64, DEFAULT_TOKENS.radius.sm)),
            md: Math.round(clampNumber(radius.md, 0, 64, DEFAULT_TOKENS.radius.md)),
            lg: Math.round(clampNumber(radius.lg, 0, 64, DEFAULT_TOKENS.radius.lg)),
            xl: Math.round(clampNumber(radius.xl, 0, 96, DEFAULT_TOKENS.radius.xl)),
            pill: Math.round(clampNumber(radius.pill, 0, 9999, DEFAULT_TOKENS.radius.pill)),
        },
        effects: {
            glow: clampNumber(effects.glow, 0, 1, DEFAULT_TOKENS.effects.glow),
            blur: Math.round(clampNumber(effects.blur, 0, 60, DEFAULT_TOKENS.effects.blur)),
            grid: clampNumber(effects.grid, 0, 1, DEFAULT_TOKENS.effects.grid),
        },
    };
}

/**
 * Teinte d'aperçu d'un accent, pour les pastilles de la bibliothèque de thèmes.
 * Sortie volontairement minuscule : une vignette n'a pas besoin de la rampe.
 */
export function accentPreview(accentHex: string): { base: string; light: string; dark: string; on: string } {
    const ramp = buildRamp(accentHex) ?? buildRamp(DEFAULT_TOKENS.accent)!;
    const seed = parseHex(ramp[500])!;
    return {
        base: ramp[500],
        light: ramp[300],
        dark: ramp[700],
        on: readableOn(rgbToOklch(seed)),
    };
}

/** Rappelée par l'éditeur pour afficher la rampe générée sous le sélecteur. */
export function previewRamp(accentHex: string): ColorRamp | null {
    return buildRamp(accentHex);
}

/**
 * Couleur OKLCH brute d'un hex, exposée pour les outils de l'éditeur (roue
 * chromatique, curseurs L/C/H) qui manipulent la couleur dans cet espace.
 */
export function toOklch(hex: string): Oklch | null {
    const rgb = parseHex(hex);
    return rgb ? rgbToOklch(rgb) : null;
}

/** Inverse de `toOklch`, avec repli dans le gamut sRGB. */
export function fromOklch(color: Oklch): string {
    return toHex(gamutMap(color));
}

// Réexports : les consommateurs du thème n'ont pas à connaître `color`.
export type { Oklch, ColorRamp } from './color';
export { buildRamp, normalizeHex, alphaColor, mixHex, readableOn, oklchToRgb };
