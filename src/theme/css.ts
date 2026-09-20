/**
 * Traduction d'un nœud de thème en CSS.
 *
 * ┌───────────────────────────────────────────────────────────────────────┐
 * │ FICHIER MIROIR — toute modification doit être reportée à l'identique   │
 * │ dans `LuuxCraft-Launcher/src/theme/css.ts`. Un test d'égalité (voir    │
 * │ `mirror.test.ts`) échoue si les deux copies divergent.                 │
 * └───────────────────────────────────────────────────────────────────────┘
 *
 * Pourquoi un miroir plutôt qu'un paquet partagé : panel et launcher sont deux
 * dépôts distincts, distribués séparément, et ce module est le seul point où
 * leur rendu doit coïncider au pixel près. Publier un paquet pour deux cents
 * lignes de fonctions pures coûterait plus cher — en versions à accorder, en
 * publications à enchaîner — qu'un fichier copié dont un test garantit
 * l'identité. L'aperçu de l'éditeur, lui, en utilise un portage JavaScript
 * (`public/js/studio/css.js`), tenu d'accord par un test de parité.
 *
 * Toutes les fonctions sont pures et sans dépendance au DOM : elles rendent des
 * chaînes CSS, que l'appelant pose où il veut — attribut `style` côté launcher,
 * `element.style` côté éditeur.
 */

import type {
    AutoLayout,
    ColorRef,
    Constraints,
    CornerRadius,
    Effect,
    FrameNode,
    ImageFit,
    Paint,
    Rect,
    Stroke,
    TextStyle,
    ThemeNode,
} from './schema';

/** Ce que le rendu doit demander à son hôte pour résoudre les références. */
export interface RenderContext {
    /** URL d'un média, ou `null` s'il n'est pas disponible ici. */
    mediaUrl(id: string): string | null;
    /** Pile de polices pour un token (`display`, `body`, `mono`) ou une famille. */
    fontFamily(token: string): string;
}

/** Dictionnaire de propriétés CSS, tel que l'appelant le posera. */
export type CssProps = Record<string, string>;

const TOKEN_REF = /^@([a-z][a-z0-9-]{0,40})$/;

/**
 * Une couleur est soit un hex littéral, soit un renvoi vers une variable de
 * thème. Le renvoi devient `var(--nom)`, ce qui fait qu'un changement de token
 * repeint tout ce qui s'y réfère sans retoucher un seul nœud.
 *
 * La validation a déjà garanti la forme ; ce filtre est la seconde barrière —
 * le rendu ne doit jamais émettre une valeur qu'il n'a pas reconnue, même si
 * un document arrivait par un chemin qui aurait manqué la validation.
 */
export function colorToCss(ref: ColorRef): string {
    const token = TOKEN_REF.exec(ref);
    if (token) return `var(--${token[1]})`;
    if (/^#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(ref)) return ref;
    return 'transparent';
}

function stopsToCss(stops: { at: number; color: ColorRef }[]): string {
    return stops.map((stop) => `${colorToCss(stop.color)} ${round(stop.at * 100)}%`).join(', ');
}

function round(value: number, decimals = 2): number {
    const factor = 10 ** decimals;
    return Math.round(value * factor) / factor;
}

/** `object-fit` / `background-size` d'une image selon son ajustement. */
function fitToBackgroundSize(fit: ImageFit): string {
    switch (fit) {
        case 'contain':
            return 'contain';
        case 'fill':
            return '100% 100%';
        case 'tile':
            return 'auto';
        case 'none':
            return 'auto';
        default:
            return 'cover';
    }
}

export function fitToObjectFit(fit: ImageFit): string {
    switch (fit) {
        case 'contain':
            return 'contain';
        case 'fill':
            return 'fill';
        case 'none':
        case 'tile':
            return 'none';
        default:
            return 'cover';
    }
}

/**
 * Une peinture en valeur de `background`.
 *
 * Renvoie `null` pour une peinture invisible ou impossible à résoudre (média
 * absent) : l'appelant l'écarte de la pile plutôt que d'empiler une couche
 * `none` qui décalerait l'ordre des autres.
 */
export function paintToCss(paint: Paint, ctx: RenderContext): { image: string; size?: string; position?: string; repeat?: string } | null {
    if (paint.visible === false) return null;

    switch (paint.type) {
        case 'solid': {
            const color = colorToCss(paint.color);
            // `linear-gradient` d'une seule couleur plutôt que `background-color` :
            // toutes les couches vivent alors dans `background-image` et
            // s'empilent dans un ordre prévisible, quelle que soit leur nature.
            return { image: `linear-gradient(${color}, ${color})` };
        }

        case 'linear':
            return { image: `linear-gradient(${round(paint.angle)}deg, ${stopsToCss(paint.stops)})` };

        case 'radial':
            return {
                image: `radial-gradient(circle at ${round(paint.cx * 100)}% ${round(paint.cy * 100)}%, ${stopsToCss(paint.stops)})`,
                size: `${round(paint.radius * 200)}% ${round(paint.radius * 200)}%`,
                position: `${round(paint.cx * 100)}% ${round(paint.cy * 100)}%`,
                repeat: 'no-repeat',
            };

        case 'image': {
            const url = ctx.mediaUrl(paint.mediaId);
            if (!url) return null;
            return {
                image: `url("${encodeURI(url)}")`,
                size: fitToBackgroundSize(paint.fit),
                position: `${round((paint.focusX ?? 0.5) * 100)}% ${round((paint.focusY ?? 0.5) * 100)}%`,
                repeat: paint.fit === 'tile' ? 'repeat' : 'no-repeat',
            };
        }

        case 'skin-wall':
            // Le mur de skins n'est pas une image CSS : il est monté comme une
            // grille de tuiles par le rendu, qui a besoin d'animer le défilement.
            // Il ne participe donc pas à la pile de `background`.
            return null;

        default:
            return null;
    }
}

/** Pile de peintures : la première du tableau est la plus haute, comme partout ailleurs. */
export function paintsToCss(paints: Paint[], ctx: RenderContext): CssProps {
    const layers = paints.map((paint) => paintToCss(paint, ctx)).filter((layer): layer is NonNullable<typeof layer> => layer !== null);
    if (layers.length === 0) return {};

    return {
        backgroundImage: layers.map((layer) => layer.image).join(', '),
        backgroundSize: layers.map((layer) => layer.size ?? 'cover').join(', '),
        backgroundPosition: layers.map((layer) => layer.position ?? '50% 50%').join(', '),
        backgroundRepeat: layers.map((layer) => layer.repeat ?? 'no-repeat').join(', '),
    };
}

/** Peintures appliquées à du texte, via un dégradé rogné sur les glyphes. */
export function textPaintToCss(paints: Paint[], ctx: RenderContext): CssProps {
    const visible = paints.filter((paint) => paint.visible !== false);
    if (visible.length === 0) return {};

    const first = visible[0];
    if (first?.type === 'solid') return { color: colorToCss(first.color) };

    const layers = paintsToCss(visible, ctx);
    if (!layers.backgroundImage) return {};
    return {
        ...layers,
        // Le texte devient la fenêtre du dégradé : c'est la seule façon d'avoir
        // un titre en dégradé sans dupliquer le contenu dans un pseudo-élément.
        WebkitBackgroundClip: 'text',
        backgroundClip: 'text',
        WebkitTextFillColor: 'transparent',
        color: 'transparent',
    };
}

/**
 * Contours.
 *
 * `outline` plutôt que `border` : une bordure décale la boîte, ce qui ferait
 * bouger le contenu d'un cadre dès qu'on lui ajoute un trait. `outline` se
 * dessine par-dessus, et `outline-offset` porte les trois alignements.
 */
export function strokesToCss(strokes: Stroke[]): CssProps {
    const stroke = strokes.find((s) => s.visible !== false && s.width > 0);
    if (!stroke) return {};

    const offset = stroke.align === 'outside' ? 0 : stroke.align === 'center' ? -stroke.width / 2 : -stroke.width;
    return {
        outline: `${round(stroke.width)}px ${stroke.style} ${colorToCss(stroke.color)}`,
        outlineOffset: `${round(offset)}px`,
    };
}

export function radiusToCss(radius: CornerRadius): CssProps {
    const [tl, tr, br, bl] = radius;
    if (!tl && !tr && !br && !bl) return {};
    return { borderRadius: `${round(tl)}px ${round(tr)}px ${round(br)}px ${round(bl)}px` };
}

/** Ombres, flous et halos, répartis entre `box-shadow`, `filter` et `backdrop-filter`. */
export function effectsToCss(effects: Effect[]): CssProps {
    const shadows: string[] = [];
    const filters: string[] = [];
    const backdrops: string[] = [];

    for (const effect of effects) {
        if (effect.visible === false) continue;
        switch (effect.type) {
            case 'drop-shadow':
                shadows.push(
                    `${round(effect.x)}px ${round(effect.y)}px ${round(effect.blur)}px ${round(effect.spread)}px ${colorToCss(effect.color)}`,
                );
                break;
            case 'inner-shadow':
                shadows.push(
                    `inset ${round(effect.x)}px ${round(effect.y)}px ${round(effect.blur)}px ${round(effect.spread)}px ${colorToCss(effect.color)}`,
                );
                break;
            case 'glow':
                // Un halo est une ombre sans décalage : elle épouse la forme du
                // nœud, là où un `filter: drop-shadow` déborderait du cadre.
                shadows.push(`0 0 ${round(effect.radius)}px ${colorToCss(effect.color)}`);
                break;
            case 'blur':
                filters.push(`blur(${round(effect.radius)}px)`);
                break;
            case 'backdrop-blur':
                backdrops.push(`blur(${round(effect.radius)}px)`);
                break;
        }
    }

    const props: CssProps = {};
    if (shadows.length) props.boxShadow = shadows.join(', ');
    if (filters.length) props.filter = filters.join(' ');
    if (backdrops.length) {
        props.backdropFilter = backdrops.join(' ');
        props.WebkitBackdropFilter = backdrops.join(' ');
    }
    return props;
}

export function textStyleToCss(style: TextStyle, ctx: RenderContext): CssProps {
    const props: CssProps = {
        fontFamily: ctx.fontFamily(style.font),
        fontSize: `${round(style.size)}px`,
        fontWeight: String(Math.round(style.weight)),
        lineHeight: String(round(style.lineHeight, 3)),
        letterSpacing: `${round(style.letterSpacing, 3)}px`,
        textAlign: style.align,
        textTransform: style.transform,
        textDecoration: style.decoration,
        fontStyle: style.italic ? 'italic' : 'normal',
        display: 'flex',
        flexDirection: 'column',
        justifyContent: style.verticalAlign === 'top' ? 'flex-start' : style.verticalAlign === 'bottom' ? 'flex-end' : 'center',
        whiteSpace: 'pre-wrap',
        overflowWrap: 'anywhere',
    };

    if (style.maxLines > 0) {
        // Troncature multi-lignes : `-webkit-line-clamp` reste la seule façon
        // d'obtenir des points de suspension après N lignes, et elle marche
        // dans tous les moteurs que le launcher rencontre.
        props.display = '-webkit-box';
        props.WebkitLineClamp = String(style.maxLines);
        props.WebkitBoxOrient = 'vertical';
        props.overflow = 'hidden';
    }

    return props;
}

/** Auto-layout d'un cadre, traduit en flexbox. */
export function layoutToCss(layout: AutoLayout): CssProps {
    const justify =
        layout.justify === 'space-between'
            ? 'space-between'
            : layout.justify === 'center'
              ? 'center'
              : layout.justify === 'end'
                ? 'flex-end'
                : 'flex-start';

    const align =
        layout.align === 'stretch'
            ? 'stretch'
            : layout.align === 'center'
              ? 'center'
              : layout.align === 'end'
                ? 'flex-end'
                : 'flex-start';

    return {
        display: 'flex',
        flexDirection: layout.direction === 'row' ? 'row' : 'column',
        gap: `${round(layout.gap)}px`,
        justifyContent: justify,
        alignItems: align,
        flexWrap: layout.wrap ? 'wrap' : 'nowrap',
        paddingTop: `${round(layout.padding.top)}px`,
        paddingRight: `${round(layout.padding.right)}px`,
        paddingBottom: `${round(layout.padding.bottom)}px`,
        paddingLeft: `${round(layout.padding.left)}px`,
    };
}

export interface FlowOptions {
    inFlow: boolean;
    /** Sens du cadre parent, quand le nœud est dans son flux. */
    direction?: 'row' | 'column';
    constraints?: Constraints;
}

/**
 * Position d'un nœud dans son conteneur.
 *
 * Un nœud d'auto-layout n'est pas positionné : il prend sa place dans le flux
 * du cadre parent, sans quoi la disposition automatique n'aurait aucun effet.
 * Ses ancrages y changent de sens et deviennent des règles de taille —
 * « étiré » sur l'axe du flux veut dire « prend la place qui reste », sur
 * l'autre axe « épouse la largeur du cadre » ; « ajusté » (`hug`) veut dire
 * « la taille de son contenu ». C'est ce qui permet à une liste d'actualités
 * de grandir quand la console au-dessus d'elle disparaît, et à la carte de
 * lancement de s'allonger le temps d'un téléchargement, comme le ferait la
 * mise en page flex d'origine.
 */
export function rectToCss(rect: Rect, options: FlowOptions = { inFlow: false }): CssProps {
    if (!options.inFlow) {
        return {
            position: 'absolute',
            left: `${round(rect.x)}px`,
            top: `${round(rect.y)}px`,
            width: `${round(rect.width)}px`,
            height: `${round(rect.height)}px`,
        };
    }

    const row = options.direction === 'row';
    const main = options.constraints?.[row ? 'horizontal' : 'vertical'] ?? 'start';
    const cross = options.constraints?.[row ? 'vertical' : 'horizontal'] ?? 'start';

    const props: CssProps = { position: 'relative' };

    if (main === 'stretch') {
        props.flex = '1 1 0';
        // Sans minimum nul, un enfant flex refuse de descendre sous la taille
        // de son contenu et déborde du cadre au lieu de défiler.
        props[row ? 'minWidth' : 'minHeight'] = '0';
    } else if (main === 'hug') {
        // Aucune taille posée : c'est le contenu qui la donne. Le nœud ne doit
        // pas non plus être compressé par ses frères, sinon il déborderait.
        props.flexShrink = '0';
    } else {
        props[row ? 'width' : 'height'] = `${round(row ? rect.width : rect.height)}px`;
        props.flexShrink = '0';
    }

    if (cross === 'stretch') {
        props.alignSelf = 'stretch';
    } else if (cross === 'hug') {
        props.alignSelf = 'flex-start';
    } else {
        props[row ? 'height' : 'width'] = `${round(row ? rect.height : rect.width)}px`;
    }

    return props;
}

/**
 * Axes sur lesquels un nœud épouse son contenu.
 *
 * Un composant lié se dessine d'ordinaire en remplissant sa boîte (position
 * absolue) ; sur un axe ajusté, il n'y a pas de boîte à remplir et le composant
 * doit se rendre dans le flux. Le rendu — React côté launcher, DOM côté
 * éditeur — consulte cette fonction pour choisir.
 */
export function hugAxes(constraints: Constraints, flow: false | 'row' | 'column'): { width: boolean; height: boolean } {
    if (!flow) return { width: false, height: false };
    return { width: constraints.horizontal === 'hug', height: constraints.vertical === 'hug' };
}

/**
 * Ancrage d'un nœud quand la fenêtre change de taille.
 *
 * Exprimé en pourcentages calculés depuis la taille de référence du conteneur :
 * le rendu reste du CSS pur, donc un redimensionnement de fenêtre ne repasse
 * jamais par JavaScript et ne saccade pas.
 */
export function constraintsToCss(rect: Rect, constraints: Constraints, container: { width: number; height: number }): CssProps {
    const props: CssProps = { position: 'absolute' };

    const right = container.width - rect.x - rect.width;
    const bottom = container.height - rect.y - rect.height;

    switch (constraints.horizontal) {
        case 'end':
            props.right = `${round(right)}px`;
            props.width = `${round(rect.width)}px`;
            break;
        case 'center':
            props.left = `${round(((rect.x + rect.width / 2) / container.width) * 100)}%`;
            props.width = `${round(rect.width)}px`;
            props.marginLeft = `${round(-rect.width / 2)}px`;
            break;
        case 'stretch':
            props.left = `${round(rect.x)}px`;
            props.right = `${round(right)}px`;
            break;
        case 'scale':
            props.left = `${round((rect.x / container.width) * 100)}%`;
            props.width = `${round((rect.width / container.width) * 100)}%`;
            break;
        default:
            props.left = `${round(rect.x)}px`;
            props.width = `${round(rect.width)}px`;
    }

    switch (constraints.vertical) {
        case 'end':
            props.bottom = `${round(bottom)}px`;
            props.height = `${round(rect.height)}px`;
            break;
        case 'center':
            props.top = `${round(((rect.y + rect.height / 2) / container.height) * 100)}%`;
            props.height = `${round(rect.height)}px`;
            props.marginTop = `${round(-rect.height / 2)}px`;
            break;
        case 'stretch':
            props.top = `${round(rect.y)}px`;
            props.bottom = `${round(bottom)}px`;
            break;
        case 'scale':
            props.top = `${round((rect.y / container.height) * 100)}%`;
            props.height = `${round((rect.height / container.height) * 100)}%`;
            break;
        default:
            props.top = `${round(rect.y)}px`;
            props.height = `${round(rect.height)}px`;
    }

    return props;
}

/**
 * Style complet d'un nœud.
 *
 * `container` est la boîte de référence pour l'ancrage : la fenêtre pour un
 * nœud de premier niveau, le cadre parent pour un enfant. `inFlow` vaut vrai
 * quand le parent porte un auto-layout, auquel cas ni position ni ancrage ne
 * s'appliquent.
 */
export function nodeToCss(
    node: ThemeNode,
    ctx: RenderContext,
    container: { width: number; height: number },
    options: { inFlow?: boolean; direction?: 'row' | 'column' } = {},
): CssProps {
    const inFlow = options.inFlow === true;

    const props: CssProps = {
        ...(inFlow
            ? rectToCss(node.rect, { inFlow: true, direction: options.direction ?? 'column', constraints: node.constraints })
            : constraintsToCss(node.rect, node.constraints, container)),
        ...paintsToCss(node.fills, ctx),
        ...strokesToCss(node.strokes),
        ...radiusToCss(node.radius),
        ...effectsToCss(node.effects),
    };

    if (node.opacity < 1) props.opacity = String(round(node.opacity, 3));
    if (node.blend !== 'normal') props.mixBlendMode = node.blend;
    if (node.rotation !== 0) {
        props.transform = `rotate(${round(node.rotation)}deg)`;
        // Sans origine explicite, une rotation tournerait autour du coin
        // supérieur gauche et déplacerait le nœud au lieu de l'orienter.
        props.transformOrigin = 'center center';
    }

    if (node.kind === 'frame') {
        const frame = node as FrameNode;
        if (frame.clip) props.overflow = 'hidden';
        if (frame.scroll) {
            props.overflowY = 'auto';
            props.overflowX = 'hidden';
        }
        if (frame.layout) Object.assign(props, layoutToCss(frame.layout));
    }

    if (node.kind === 'text') {
        Object.assign(props, textStyleToCss(node.text, ctx));
        Object.assign(props, textPaintToCss(node.color, ctx));
    }

    return props;
}

/** `Record<string,string>` → texte `style="..."`, pour un rendu côté serveur. */
export function cssPropsToInline(props: CssProps): string {
    return Object.entries(props)
        .map(([key, value]) => `${kebab(key)}:${value}`)
        .join(';');
}

function kebab(key: string): string {
    // `WebkitLineClamp` → `-webkit-line-clamp` : la majuscule initiale marque un
    // préfixe vendeur, qui prend un tiret de tête en CSS.
    const dashed = key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
    return /^[A-Z]/.test(key) ? `-${dashed.slice(1)}` : dashed;
}
