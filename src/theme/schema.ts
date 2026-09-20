/**
 * Schéma du document de thème du launcher (version 2).
 *
 * Un document décrit **entièrement** ce que le launcher affiche : il n'y a plus
 * de mise en page codée en dur qu'on viendrait habiller, c'est l'arbre de nœuds
 * qui fait l'interface. Le launcher n'est plus qu'un moteur de rendu.
 *
 * Structure, du haut vers le bas :
 *
 *   document
 *   ├── tokens        couleurs, polices, rayons, effets (voir `tokens.ts`)
 *   ├── media[]       images, skins et polices référencées par les nœuds
 *   ├── window        taille du canvas et bornes de redimensionnement
 *   └── screens
 *       ├── shared    nœuds rendus sur tous les écrans (barre de titre, menu…)
 *       ├── boot      écran de démarrage
 *       ├── home      accueil
 *       ├── accounts  comptes
 *       ├── skins     skins
 *       ├── settings  paramètres
 *       └── login     connexion (superposée aux autres)
 *
 * Chaque nœud est soit une primitive libre (cadre, texte, image, forme), soit un
 * **composant lié** : un widget dont le launcher pilote le contenu et le
 * comportement (bouton Jouer, liste de comptes, formulaire de connexion…). Ce
 * sont ces composants liés qui font qu'un launcher entièrement redessiné reste
 * un launcher fonctionnel, et non un joli écran mort.
 *
 * Ce fichier ne contient que des types. La validation — la vraie frontière de
 * confiance — vit dans `validate.ts`.
 */

import type { ThemeTokens } from './tokens';

export const SCHEMA_VERSION = 2;

/** Écrans du launcher. `shared` n'en est pas un : il se superpose à tous. */
export const SCREEN_IDS = ['shared', 'boot', 'home', 'accounts', 'skins', 'settings', 'login'] as const;
export type ScreenId = (typeof SCREEN_IDS)[number];

/** Écrans réellement navigables, dans l'ordre où l'éditeur les propose. */
export const EDITABLE_SCREENS: readonly ScreenId[] = ['home', 'accounts', 'skins', 'settings', 'login', 'boot', 'shared'];

// ── Géométrie ───────────────────────────────────────────────────────────────

export interface Rect {
    x: number;
    y: number;
    width: number;
    height: number;
}

/**
 * Ancrage d'un nœud quand la fenêtre change de taille.
 *
 * Sans ça, un launcher dessiné en 1280×720 serait tronqué dès que le joueur
 * agrandit sa fenêtre. `stretch` colle aux deux bords (une barre latérale),
 * `end` au bord opposé (un bouton en bas à droite), `scale` conserve les
 * proportions (un fond).
 *
 * Dans un cadre en disposition automatique, l'ancrage devient une règle de
 * taille : `stretch` remplit la place qui reste, `hug` épouse le contenu —
 * c'est ce qui permet à la carte de lancement de grandir quand une
 * progression s'y affiche, sans défiler dans une boîte figée. Hors flux, `hug`
 * vaut `start`.
 */
export type ConstraintAxis = 'start' | 'center' | 'end' | 'stretch' | 'scale' | 'hug';

export interface Constraints {
    horizontal: ConstraintAxis;
    vertical: ConstraintAxis;
}

// ── Peintures ───────────────────────────────────────────────────────────────

/**
 * Référence de couleur : soit un hex littéral, soit un renvoi vers la rampe
 * d'un token (`@accent.500`, `@danger.700`, `@text`, `@surface`).
 *
 * Les renvois sont ce qui rend un thème « repeignable » : changer l'accent dans
 * le panneau des tokens met à jour d'un coup tous les nœuds qui l'utilisent, au
 * lieu d'obliger à rouvrir chaque calque.
 */
export type ColorRef = string;

export interface GradientStop {
    /** Position sur l'axe du dégradé, 0 → 1. */
    at: number;
    color: ColorRef;
}

export type Paint =
    | { type: 'solid'; color: ColorRef; opacity?: number; visible?: boolean }
    | {
          type: 'linear';
          /** Angle en degrés, 0 = vers la droite, 90 = vers le bas. */
          angle: number;
          stops: GradientStop[];
          opacity?: number;
          visible?: boolean;
      }
    | {
          type: 'radial';
          /** Centre en fraction du cadre, 0 → 1. */
          cx: number;
          cy: number;
          /** Rayon en fraction de la plus grande dimension du cadre. */
          radius: number;
          stops: GradientStop[];
          opacity?: number;
          visible?: boolean;
      }
    | {
          type: 'image';
          mediaId: string;
          fit: ImageFit;
          /** Cadrage quand `fit` laisse du jeu, en fraction, 0 → 1. */
          focusX?: number;
          focusY?: number;
          opacity?: number;
          visible?: boolean;
          /** Fait tourner plusieurs médias au lieu d'en figer un seul. */
          rotate?: { mediaIds: string[]; seconds: number; transition: 'none' | 'fade' | 'slide' };
      }
    | {
          /**
           * Mur de skins : mosaïque animée construite à partir de la
           * médiathèque de skins du propriétaire. C'est un fond vivant sans
           * coût de bande passante côté joueur, puisque les textures sont déjà
           * servies par le panel.
           */
          type: 'skin-wall';
          /** Skins retenus ; vide = toute la médiathèque. */
          mediaIds: string[];
          /** Rendu de chaque tuile. */
          render: 'face' | 'bust' | 'body';
          /** Taille d'une tuile en pixels de canvas. */
          tile: number;
          gap: number;
          /** Vitesse de défilement, en pixels par seconde ; 0 = figé. */
          speed: number;
          angle: number;
          opacity?: number;
          visible?: boolean;
      };

export type ImageFit = 'cover' | 'contain' | 'fill' | 'tile' | 'none';

export interface Stroke {
    color: ColorRef;
    width: number;
    /** Position du trait par rapport au bord du cadre. */
    align: 'inside' | 'center' | 'outside';
    style: 'solid' | 'dashed' | 'dotted';
    visible?: boolean;
}

export type Effect =
    | { type: 'drop-shadow'; x: number; y: number; blur: number; spread: number; color: ColorRef; visible?: boolean }
    | { type: 'inner-shadow'; x: number; y: number; blur: number; spread: number; color: ColorRef; visible?: boolean }
    | { type: 'blur'; radius: number; visible?: boolean }
    | { type: 'backdrop-blur'; radius: number; visible?: boolean }
    | { type: 'glow'; radius: number; color: ColorRef; visible?: boolean };

/** Rayons par coin, dans l'ordre horaire depuis le coin supérieur gauche. */
export type CornerRadius = [number, number, number, number];

// ── Mise en page automatique ────────────────────────────────────────────────

/**
 * Auto-layout d'un cadre, dans l'esprit de celui de Figma.
 *
 * Indispensable pour tout ce qui est répété ou de longueur variable : une liste
 * de comptes, une barre de liens sociaux, un menu. Sans lui, ajouter un lien
 * dans le panel décalerait tout le reste de la mise en page du client.
 */
export interface AutoLayout {
    direction: 'row' | 'column';
    gap: number;
    padding: { top: number; right: number; bottom: number; left: number };
    /** Alignement sur l'axe principal. */
    justify: 'start' | 'center' | 'end' | 'space-between';
    /** Alignement sur l'axe secondaire. */
    align: 'start' | 'center' | 'end' | 'stretch';
    wrap: boolean;
    /** Le cadre se redimensionne pour épouser son contenu. */
    hug: { width: boolean; height: boolean };
}

// ── Typographie ─────────────────────────────────────────────────────────────

export interface TextStyle {
    /** Token de police (`display`, `body`, `mono`) ou nom de famille littéral. */
    font: string;
    size: number;
    weight: number;
    lineHeight: number;
    letterSpacing: number;
    align: 'left' | 'center' | 'right' | 'justify';
    verticalAlign: 'top' | 'middle' | 'bottom';
    transform: 'none' | 'uppercase' | 'lowercase' | 'capitalize';
    decoration: 'none' | 'underline' | 'line-through';
    italic: boolean;
    /** Troncature au-delà de N lignes ; 0 = pas de limite. */
    maxLines: number;
}

// ── Nœuds ───────────────────────────────────────────────────────────────────

export type NodeKind = 'frame' | 'text' | 'image' | 'shape' | 'component';

/** Propriétés communes à tous les nœuds. */
export interface BaseNode {
    id: string;
    /** Nom affiché dans l'arbre des calques. */
    name: string;
    kind: NodeKind;
    rect: Rect;
    /** Rotation en degrés autour du centre du cadre. */
    rotation: number;
    constraints: Constraints;
    opacity: number;
    visible: boolean;
    /** Verrou d'édition : ignoré au clic et au déplacement dans l'éditeur. */
    locked: boolean;
    fills: Paint[];
    strokes: Stroke[];
    radius: CornerRadius;
    effects: Effect[];
    /** Mode de fusion CSS (`mix-blend-mode`). */
    blend: BlendMode;
    /**
     * Condition d'affichage à l'exécution. Un nœud sans condition est toujours
     * visible ; sinon le launcher ne le monte que quand l'état correspond —
     * c'est ainsi qu'on dessine un état « téléchargement en cours » ou « aucun
     * compte connecté » sans avoir à créer un écran entier pour chacun.
     */
    when?: VisibilityCondition;
}

export type BlendMode =
    | 'normal'
    | 'multiply'
    | 'screen'
    | 'overlay'
    | 'darken'
    | 'lighten'
    | 'color-dodge'
    | 'color-burn'
    | 'hard-light'
    | 'soft-light'
    | 'difference'
    | 'exclusion'
    | 'hue'
    | 'saturation'
    | 'color'
    | 'luminosity';

/** États de l'exécution auxquels un nœud peut se lier. */
export const VISIBILITY_FLAGS = [
    'account-connected',
    'account-missing',
    'game-running',
    'game-idle',
    /** Une partie tourne, ou en a laissé un journal : la console a de quoi montrer. */
    'game-logs',
    'download-active',
    'maintenance',
    'offline',
    'instance-selected',
    'multiple-instances',
] as const;

export type VisibilityFlag = (typeof VISIBILITY_FLAGS)[number];

export interface VisibilityCondition {
    /** Le nœud est monté si **tous** ces états sont vrais. */
    all?: VisibilityFlag[];
    /** …ou si **au moins un** de ceux-ci l'est. */
    any?: VisibilityFlag[];
    /** …et si **aucun** de ceux-là ne l'est. */
    none?: VisibilityFlag[];
}

export interface FrameNode extends BaseNode {
    kind: 'frame';
    children: ThemeNode[];
    /** Rogne les enfants qui dépassent du cadre. */
    clip: boolean;
    layout: AutoLayout | null;
    /** Le cadre défile verticalement quand son contenu dépasse. */
    scroll: boolean;
}

export interface TextNode extends BaseNode {
    kind: 'text';
    /**
     * Contenu, avec jetons dynamiques : `{player}`, `{instance}`, `{version}`,
     * `{online}`, `{max}`, `{brand}`. Le launcher les remplace au rendu, ce qui
     * permet d'écrire « Salut {player} » sans composant dédié.
     */
    content: string;
    text: TextStyle;
    color: Paint[];
}

export interface ImageNode extends BaseNode {
    kind: 'image';
    mediaId: string | null;
    fit: ImageFit;
    focusX: number;
    focusY: number;
    /** Rend l'image sans lissage — nécessaire pour les textures Minecraft. */
    pixelated: boolean;
}

export type ShapeKind = 'rectangle' | 'ellipse' | 'line' | 'triangle' | 'polygon' | 'star';

export interface ShapeNode extends BaseNode {
    kind: 'shape';
    shape: ShapeKind;
    /** Nombre de côtés ou de branches pour `polygon` et `star`. */
    sides: number;
    /** Creux d'une étoile, 0 → 1. */
    innerRadius: number;
}

export interface ComponentNode extends BaseNode {
    kind: 'component';
    /** Clé du registre (`registry.ts`). */
    ref: string;
    /** Propriétés d'apparence autorisées par le registre pour ce composant. */
    props: Record<string, unknown>;
    /**
     * Habillage hérité par les éléments internes du composant : c'est ce qui
     * permet de restyler une liste de comptes sans que le launcher ait à
     * exposer trente propriétés.
     */
    slots: Record<string, SlotStyle>;
}

/** Habillage d'un sous-élément d'un composant lié. */
export interface SlotStyle {
    fills?: Paint[];
    strokes?: Stroke[];
    radius?: CornerRadius;
    effects?: Effect[];
    text?: Partial<TextStyle>;
    color?: Paint[];
    padding?: { top: number; right: number; bottom: number; left: number };
    gap?: number;
    hidden?: boolean;
}

export type ThemeNode = FrameNode | TextNode | ImageNode | ShapeNode | ComponentNode;

// ── Médias ──────────────────────────────────────────────────────────────────

export type MediaKind = 'image' | 'skin' | 'cape' | 'font';

/**
 * Entrée de médiathèque telle que le document la voit.
 *
 * Adressée par contenu (`hash`) et jamais par URL : le document ne stocke aucun
 * lien, c'est l'API qui reconstruit l'adresse vers notre propre origine au
 * moment de servir. Un document exporté puis réimporté chez un autre client ne
 * peut donc pas pointer vers les fichiers du premier.
 */
export interface MediaRef {
    id: string;
    kind: MediaKind;
    hash: string;
    filename: string;
    width: number;
    height: number;
    size: number;
    /** Renseigné à la volée par l'API, absent en base. */
    url?: string;
}

// ── Document ────────────────────────────────────────────────────────────────

export interface ScreenLayer {
    /** Fond de l'écran, sous tous les nœuds. */
    background: Paint[];
    nodes: ThemeNode[];
}

export interface WindowSpec {
    width: number;
    height: number;
    minWidth: number;
    minHeight: number;
    /** Barre de titre dessinée par le thème plutôt que par le système. */
    customTitleBar: boolean;
    resizable: boolean;
}

export interface ThemeDocument {
    schemaVersion: number;
    window: WindowSpec;
    tokens: ThemeTokens;
    media: MediaRef[];
    screens: Record<ScreenId, ScreenLayer>;
}

// ── Erreurs de validation ───────────────────────────────────────────────────

export interface ValidationError {
    /** Chemin JSON du champ fautif, pour que l'éditeur surligne le bon calque. */
    path: string;
    message: string;
}

export type ValidationResult =
    | { ok: true; document: ThemeDocument; repaired: string[] }
    | { ok: false; errors: ValidationError[] };

// ── Bornes ──────────────────────────────────────────────────────────────────

export const LIMITS = {
    maxNodes: 600,
    maxDepth: 12,
    maxMedia: 400,
    maxDocumentBytes: 1024 * 1024,
    maxNameLength: 64,
    maxTextLength: 2000,
    maxPaints: 8,
    maxEffects: 8,
    maxStops: 12,
    canvas: { minWidth: 800, minHeight: 500, maxWidth: 3840, maxHeight: 2160 },
} as const;
