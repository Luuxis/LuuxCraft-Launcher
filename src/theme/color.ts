/**
 * Dérivation de palette à partir d'une seule couleur d'accentuation.
 *
 * Le propriétaire ne choisit qu'un hex. Tout le reste — la rampe 50→950, la
 * couleur de texte lisible par-dessus, le halo, les bordures — en découle ici,
 * une fois, côté panel. Le launcher ne reçoit que des valeurs finies : il n'a
 * aucune copie de cet algorithme, donc aucune dérive possible entre l'aperçu de
 * l'éditeur et le rendu réel.
 *
 * Le travail se fait en OKLCH plutôt qu'en HSL : en HSL, faire varier la
 * luminosité d'un bleu et d'un jaune donne deux rampes qui ne « pèsent » pas
 * pareil, parce que HSL ignore la perception. En OKLCH, une même luminosité
 * donne deux teintes perçues aussi claires l'une que l'autre, ce qui est la
 * seule façon d'obtenir une rampe correcte quelle que soit la couleur choisie.
 */

export interface Oklch {
    /** Luminosité perceptuelle, 0 (noir) → 1 (blanc). */
    l: number;
    /** Chroma : 0 = gris, ~0.37 = le plus saturé représentable en sRGB. */
    c: number;
    /** Teinte en degrés, 0→360. */
    h: number;
}

export type ColorRamp = Record<RampStep, string>;

export const RAMP_STEPS = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950] as const;
export type RampStep = (typeof RAMP_STEPS)[number];

/**
 * Échelle de luminosité de la rampe, relevée sur les palettes Tailwind v4
 * converties en OKLCH. Les garder fixes est ce qui fait qu'un accent rouge et
 * un accent bleu produisent des interfaces d'un contraste comparable : seules
 * la teinte et le chroma suivent le choix de l'utilisateur, jamais le poids.
 */
const RAMP_LIGHTNESS: Record<RampStep, number> = {
    50: 0.971,
    100: 0.941,
    200: 0.885,
    300: 0.808,
    400: 0.723,
    500: 0.646,
    600: 0.578,
    700: 0.503,
    800: 0.432,
    900: 0.378,
    950: 0.264,
};

/**
 * Chroma de chaque marche, en proportion de celui de la couleur choisie.
 *
 * La courbe culmine au milieu : une teinte très claire ou très sombre ne peut
 * pas porter autant de chroma sans sortir du gamut sRGB, et les pastels
 * extrêmes lus à l'écran paraissent délavés plutôt que colorés.
 */
const RAMP_CHROMA: Record<RampStep, number> = {
    50: 0.16,
    100: 0.3,
    200: 0.52,
    300: 0.75,
    400: 0.92,
    500: 1,
    600: 0.97,
    700: 0.86,
    800: 0.73,
    900: 0.63,
    950: 0.45,
};

const HEX_RE = /^#?([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

function clamp01(value: number): number {
    return value < 0 ? 0 : value > 1 ? 1 : value;
}

function srgbToLinear(channel: number): number {
    return channel <= 0.04045 ? channel / 12.92 : Math.pow((channel + 0.055) / 1.055, 2.4);
}

function linearToSrgb(channel: number): number {
    return channel <= 0.0031308 ? channel * 12.92 : 1.055 * Math.pow(channel, 1 / 2.4) - 0.055;
}

/**
 * Accepte #rgb, #rgba, #rrggbb et #rrggbbaa. Renvoie `null` — jamais une
 * couleur de repli — pour que l'appelant décide quoi faire d'une saisie
 * invalide : substituer silencieusement du vert à la couleur d'un client
 * masquerait une erreur de sa part.
 */
export function parseHex(input: string): { r: number; g: number; b: number; a: number } | null {
    const match = HEX_RE.exec(input.trim());
    if (!match?.[1]) return null;

    let hex = match[1];
    if (hex.length === 3 || hex.length === 4) {
        hex = hex
            .split('')
            .map((ch) => ch + ch)
            .join('');
    }

    const r = parseInt(hex.slice(0, 2), 16) / 255;
    const g = parseInt(hex.slice(2, 4), 16) / 255;
    const b = parseInt(hex.slice(4, 6), 16) / 255;
    const a = hex.length === 8 ? parseInt(hex.slice(6, 8), 16) / 255 : 1;
    return { r, g, b, a };
}

function toHexChannel(value: number): string {
    return Math.round(clamp01(value) * 255)
        .toString(16)
        .padStart(2, '0');
}

export function toHex(rgb: { r: number; g: number; b: number; a?: number }): string {
    const base = `#${toHexChannel(rgb.r)}${toHexChannel(rgb.g)}${toHexChannel(rgb.b)}`;
    if (rgb.a === undefined || rgb.a >= 1) return base;
    return `${base}${toHexChannel(rgb.a)}`;
}

export function rgbToOklch(rgb: { r: number; g: number; b: number }): Oklch {
    const r = srgbToLinear(rgb.r);
    const g = srgbToLinear(rgb.g);
    const b = srgbToLinear(rgb.b);

    const lms0 = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    const lms1 = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    const lms2 = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    const l_ = Math.cbrt(lms0);
    const m_ = Math.cbrt(lms1);
    const s_ = Math.cbrt(lms2);

    const L = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_;
    const a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_;
    const bb = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_;

    const c = Math.sqrt(a * a + bb * bb);
    // Une couleur neutre (c ≈ 0) n'a pas de teinte définie ; on la fixe à 0
    // plutôt que de laisser atan2 renvoyer du bruit, sinon deux gris identiques
    // donneraient deux rampes différentes.
    const h = c < 1e-6 ? 0 : ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360;

    return { l: L, c, h };
}

export function oklchToRgb(color: Oklch): { r: number; g: number; b: number } {
    const hRad = (color.h * Math.PI) / 180;
    const a = color.c * Math.cos(hRad);
    const b = color.c * Math.sin(hRad);

    const l_ = color.l + 0.3963377774 * a + 0.2158037573 * b;
    const m_ = color.l - 0.1055613458 * a - 0.0638541728 * b;
    const s_ = color.l - 0.0894841775 * a - 1.291485548 * b;

    const l = l_ * l_ * l_;
    const m = m_ * m_ * m_;
    const s = s_ * s_ * s_;

    return {
        r: linearToSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        g: linearToSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        b: linearToSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
    };
}

function inGamut(rgb: { r: number; g: number; b: number }): boolean {
    const eps = 1e-4;
    return (
        rgb.r >= -eps && rgb.r <= 1 + eps && rgb.g >= -eps && rgb.g <= 1 + eps && rgb.b >= -eps && rgb.b <= 1 + eps
    );
}

/**
 * Ramène une couleur OKLCH dans le gamut sRGB en réduisant son chroma, jamais
 * sa luminosité.
 *
 * Un simple écrêtage canal par canal — la méthode naïve — décale la teinte : un
 * violet trop saturé vire au bleu dès que le canal rouge sature. Chercher le
 * plus grand chroma représentable garde la teinte et la luminosité exactes et
 * ne sacrifie que la saturation, qui est la seule chose réellement impossible
 * à afficher.
 */
export function gamutMap(color: Oklch): { r: number; g: number; b: number } {
    const direct = oklchToRgb(color);
    if (inGamut(direct)) {
        return { r: clamp01(direct.r), g: clamp01(direct.g), b: clamp01(direct.b) };
    }

    let low = 0;
    let high = color.c;
    let best = oklchToRgb({ ...color, c: 0 });

    // 24 bissections : l'écart résiduel est très inférieur au pas d'un canal 8
    // bits, donc invisible, et le coût reste négligeable.
    for (let i = 0; i < 24; i += 1) {
        const mid = (low + high) / 2;
        const candidate = oklchToRgb({ ...color, c: mid });
        if (inGamut(candidate)) {
            best = candidate;
            low = mid;
        } else {
            high = mid;
        }
    }

    return { r: clamp01(best.r), g: clamp01(best.g), b: clamp01(best.b) };
}

/** Luminance relative WCAG, pour les rapports de contraste. */
export function relativeLuminance(rgb: { r: number; g: number; b: number }): number {
    const r = srgbToLinear(rgb.r);
    const g = srgbToLinear(rgb.g);
    const b = srgbToLinear(rgb.b);
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Rapport de contraste WCAG entre deux couleurs (1 → 21). */
export function contrastRatio(a: { r: number; g: number; b: number }, b: { r: number; g: number; b: number }): number {
    const la = relativeLuminance(a);
    const lb = relativeLuminance(b);
    const light = Math.max(la, lb);
    const dark = Math.min(la, lb);
    return (light + 0.05) / (dark + 0.05);
}

/**
 * Couleur de texte à poser sur un fond donné.
 *
 * On ne se contente pas de « clair si le fond est sombre » : on prend une
 * version très sombre et une version très claire de la teinte elle-même, puis
 * celle qui contraste le mieux. Le texte reste ainsi teinté par la marque —
 * un noir verdâtre sur un bouton vert — au lieu d'un noir pur qui jure.
 */
export function readableOn(background: Oklch): string {
    const bg = gamutMap(background);
    const tintedDark = gamutMap({ l: 0.18, c: Math.min(background.c, 0.06), h: background.h });
    const tintedLight = gamutMap({ l: 0.98, c: Math.min(background.c, 0.04), h: background.h });

    const darkRatio = contrastRatio(bg, tintedDark);
    const lightRatio = contrastRatio(bg, tintedLight);

    // À égalité, on privilégie le sombre : les accents sont majoritairement des
    // couleurs vives de milieu de rampe, sur lesquelles un texte sombre est plus
    // lisible qu'un blanc qui bave.
    return toHex(darkRatio >= lightRatio ? tintedDark : tintedLight);
}

/**
 * Échelle de luminosité réancrée sur la couleur choisie.
 *
 * L'échelle fixe suppose que la couleur saisie a la clarté d'une marche 500.
 * Rien ne l'y oblige : un client peut saisir un pastel très clair ou un ton
 * très sombre. Dans ce cas, l'échelle brute produirait une rampe qui n'est plus
 * ordonnée autour de 500 — un `accent-400` plus foncé que l'accent lui-même —,
 * et le survol d'un bouton assombrirait au lieu d'éclaircir.
 *
 * On étire donc l'échelle de part et d'autre de la couleur choisie : les
 * extrêmes restent où ils sont, les écarts relatifs entre marches sont
 * préservés, et 500 tombe exactement sur la clarté saisie. La rampe reste
 * ordonnée quelle que soit la couleur.
 */
function rampLightness(step: RampStep, seedLightness: number): number {
    if (step === 500) return seedLightness;

    const mid = RAMP_LIGHTNESS[500];
    const top = RAMP_LIGHTNESS[50];
    const bottom = RAMP_LIGHTNESS[950];

    // Les bornes ne franchissent jamais la couleur choisie. Un accent déjà plus
    // clair que le haut de l'échelle — un blanc, un pastel extrême — donne donc
    // des marches claires égales à lui, et non plus claires que lui : au-dessus
    // du blanc il n'y a rien, et une interpolation vers une borne plus sombre
    // inverserait l'ordre de la rampe.
    const ceiling = Math.max(top, seedLightness);
    const floor = Math.min(bottom, seedLightness);

    if (RAMP_LIGHTNESS[step] > mid) {
        const ratio = (RAMP_LIGHTNESS[step] - mid) / (top - mid);
        return seedLightness + ratio * (ceiling - seedLightness);
    }

    const ratio = (mid - RAMP_LIGHTNESS[step]) / (mid - bottom);
    return seedLightness - ratio * (seedLightness - floor);
}

/**
 * Rampe complète à partir d'une couleur d'accentuation.
 *
 * La couleur choisie occupe la marche 500 : c'est celle que l'interface emploie
 * pour les boutons pleins et les états actifs, donc celle que le propriétaire
 * croit régler en saisissant son hex. Elle est renvoyée telle quelle, sans
 * retouche, pour qu'un client qui saisit la couleur exacte de sa charte la
 * retrouve à l'identique dans son launcher.
 */
export function buildRamp(accentHex: string): ColorRamp | null {
    const rgb = parseHex(accentHex);
    if (!rgb) return null;

    const seed = rgbToOklch(rgb);
    const ramp = {} as ColorRamp;

    for (const step of RAMP_STEPS) {
        if (step === 500) {
            ramp[step] = toHex(rgb);
            continue;
        }
        ramp[step] = toHex(
            gamutMap({
                l: rampLightness(step, seed.l),
                c: seed.c * RAMP_CHROMA[step],
                h: seed.h,
            }),
        );
    }

    return ramp;
}

/** `rgba(r, g, b, alpha)` à partir d'un hex — pour les halos et les voiles. */
export function alphaColor(hex: string, alpha: number): string {
    const rgb = parseHex(hex);
    if (!rgb) return `rgba(0, 0, 0, ${alpha})`;
    const r = Math.round(clamp01(rgb.r) * 255);
    const g = Math.round(clamp01(rgb.g) * 255);
    const b = Math.round(clamp01(rgb.b) * 255);
    return `rgba(${r}, ${g}, ${b}, ${Math.round(clamp01(alpha) * 1000) / 1000})`;
}

/**
 * Mélange deux hex en sRGB linéaire.
 *
 * Le mélange naïf, canal par canal sur les valeurs gamma, assombrit : à
 * mi-chemin entre un rouge pur et un vert pur il donne un kaki terne au lieu
 * du jaune attendu. Passer par le linéaire est ce qui rend les dégradés et les
 * fonds teintés corrects.
 */
export function mixHex(a: string, b: string, weight: number): string {
    const ca = parseHex(a);
    const cb = parseHex(b);
    if (!ca || !cb) return a;

    const t = clamp01(weight);
    const blend = (x: number, y: number) => linearToSrgb(srgbToLinear(x) * (1 - t) + srgbToLinear(y) * t);

    return toHex({
        r: blend(ca.r, cb.r),
        g: blend(ca.g, cb.g),
        b: blend(ca.b, cb.b),
        a: ca.a * (1 - t) + cb.a * t,
    });
}

/** Normalise une saisie utilisateur en `#rrggbb`, ou `null` si elle est invalide. */
export function normalizeHex(input: unknown): string | null {
    if (typeof input !== 'string') return null;
    const rgb = parseHex(input);
    if (!rgb) return null;
    return toHex({ r: rgb.r, g: rgb.g, b: rgb.b, a: rgb.a });
}
