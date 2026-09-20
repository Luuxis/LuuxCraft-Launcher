/**
 * Rendu d'un document de thème.
 *
 * Le pendant du rendu de l'éditeur, en React, sur exactement le même moteur :
 * `css.ts` est une copie à l'identique du fichier du panel, et un test échoue
 * si les deux divergent. Ce que le propriétaire a composé est donc ce que le
 * joueur voit.
 *
 * Deux propriétés de ce rendu méritent d'être soulignées :
 *
 *  - **Aucun recalcul au redimensionnement.** Les ancrages sont traduits en
 *    `left/right/%` par `constraintsToCss` : agrandir la fenêtre est réglé par
 *    le moteur de mise en page du navigateur, sans qu'une ligne de JavaScript
 *    s'exécute.
 *  - **Aucun contenu inventé.** Un composant lié ne reçoit du document que son
 *    apparence ; ses données viennent du launcher. Un thème ne peut donc pas
 *    afficher un faux nombre de joueurs ni un faux bouton Jouer.
 */

import { memo, type CSSProperties, type ReactNode } from 'react';

import { DEFAULT_SKIN_URL } from '../lib/defaultSkin';
import { fitToObjectFit, hugAxes, nodeToCss, paintsToCss, type RenderContext } from './css';
import type { FrameNode, ImageNode, Paint, ScreenId, TextNode, ThemeDocument, ThemeNode } from './schema';
import { isVisible, type ThemeRuntime } from './runtime';
import { WIDGETS } from './widgets';

interface ThemedScreenProps {
    document: ThemeDocument;
    screen: ScreenId;
    runtime: ThemeRuntime;
    /** Superpose la couche commune (barre de titre, menu…). */
    shared?: boolean;
}

/**
 * `nodeToCss` rend des noms de propriétés en camelCase, ce que React accepte
 * tel quel pour un style en ligne. La conversion se réduit donc à un changement
 * de type.
 */
function toStyle(props: Record<string, string>): CSSProperties {
    return props as unknown as CSSProperties;
}

/**
 * Mur de skins : une mosaïque de têtes qui défile lentement en fond.
 *
 * Monté comme des éléments plutôt qu'en `background-image` parce qu'il doit
 * s'animer et piocher dans une liste ; c'est pour cela que `css.ts` l'écarte
 * de la pile des remplissages.
 */
const SkinWall = memo(function SkinWall({ paint, runtime }: { paint: Extract<Paint, { type: 'skin-wall' }>; runtime: ThemeRuntime }) {
    const ids = paint.mediaIds.length ? paint.mediaIds : runtime.skinIds();
    // Sans skin dans la médiathèque, le mur est fait de Steve : un fond vivant
    // qui montre le motif, plutôt qu'un fond vide qui ressemble à une panne.
    const urls = ids.map((id) => runtime.mediaUrl(id)).filter((url): url is string => Boolean(url));
    if (urls.length === 0) urls.push(DEFAULT_SKIN_URL);

    // Assez de tuiles pour couvrir une grande fenêtre sans en monter des
    // milliers : au-delà, le coût de mise en page dépasse largement le gain
    // visuel, et la répétition ne se voit pas.
    const total = Math.min(240, Math.max(40, urls.length * 6));
    const tiles = Array.from({ length: total }, (_, index) => urls[index % urls.length]!);

    const step = paint.tile + paint.gap;

    return (
        // Deux couches, et non une : la rotation et le défilement sont tous
        // deux des `transform`, et sur un même élément la seconde effacerait
        // la première.
        <div
            aria-hidden
            style={{
                position: 'absolute',
                inset: -paint.tile,
                overflow: 'hidden',
                opacity: paint.opacity ?? 1,
                transform: `rotate(${paint.angle}deg)`,
                pointerEvents: 'none',
            }}
        >
            <div
                style={{
                    display: 'flex',
                    flexWrap: 'wrap',
                    gap: paint.gap,
                    // La boucle translate d'exactement une tuile : à la reprise
                    // l'image est identique, donc le raccord ne se voit pas.
                    ['--skin-wall-step' as string]: `${step}px`,
                    animation: paint.speed > 0 ? `skin-wall-drift ${Math.max(4, step / paint.speed)}s linear infinite` : undefined,
                }}
            >
                {tiles.map((url, index) => (
                    <span
                        key={`${url}-${index}`}
                        style={{
                            width: paint.tile,
                            height: paint.tile,
                            flexShrink: 0,
                            borderRadius: 4,
                            backgroundImage: `url("${encodeURI(url)}")`,
                            // La tête occupe 8×8 pixels à l'offset (8, 8) d'une
                            // planche de 64 : la planche est mise à l'échelle en
                            // pixels — huit tuiles de large — et décalée d'une
                            // tuile. En pourcentages, l'arithmétique de
                            // `background-position` cadrerait à côté.
                            backgroundSize: paint.render === 'face' ? `${paint.tile * 8}px ${paint.tile * 8}px` : 'contain',
                            backgroundPosition: paint.render === 'face' ? `-${paint.tile}px -${paint.tile}px` : 'center',
                            backgroundRepeat: 'no-repeat',
                            imageRendering: 'pixelated',
                        }}
                    />
                ))}
            </div>
        </div>
    );
});

function skinWalls(paints: Paint[], runtime: ThemeRuntime): ReactNode {
    return paints
        .filter((paint): paint is Extract<Paint, { type: 'skin-wall' }> => paint.type === 'skin-wall' && paint.visible !== false)
        .map((paint, index) => <SkinWall key={index} paint={paint} runtime={runtime} />);
}

interface NodeProps {
    node: ThemeNode;
    runtime: ThemeRuntime;
    ctx: RenderContext;
    container: { width: number; height: number };
    /** `false` pour un nœud positionné, sinon le sens du cadre parent en flux. */
    flow: false | 'row' | 'column';
}

function ThemedNode({ node, runtime, ctx, container, flow }: NodeProps) {
    if (!node.visible) return null;
    if (!isVisible(node.when, runtime.flags)) return null;

    const style = toStyle(nodeToCss(node, ctx, container, flow ? { inFlow: true, direction: flow } : {}));
    const walls = skinWalls(node.fills, runtime);

    switch (node.kind) {
        case 'frame': {
            const frame = node as FrameNode;
            const inner = { width: frame.rect.width, height: frame.rect.height };
            return (
                <div style={style} data-node={frame.id}>
                    {walls}
                    {frame.children.map((child) => (
                        <ThemedNode
                            key={child.id}
                            node={child}
                            runtime={runtime}
                            ctx={ctx}
                            container={inner}
                            flow={frame.layout ? frame.layout.direction : false}
                        />
                    ))}
                </div>
            );
        }

        case 'text': {
            const text = node as TextNode;
            return (
                <div style={style} data-node={text.id}>
                    {walls}
                    <span style={{ width: '100%' }}>{runtime.interpolate(text.content)}</span>
                </div>
            );
        }

        case 'image': {
            const image = node as ImageNode;
            const url = image.mediaId ? runtime.mediaUrl(image.mediaId) : null;
            return (
                <div style={style} data-node={image.id}>
                    {walls}
                    {url ? (
                        <img
                            src={url}
                            alt=""
                            draggable={false}
                            style={{
                                width: '100%',
                                height: '100%',
                                objectFit: fitToObjectFit(image.fit) as CSSProperties['objectFit'],
                                objectPosition: `${image.focusX * 100}% ${image.focusY * 100}%`,
                                borderRadius: 'inherit',
                                imageRendering: image.pixelated ? 'pixelated' : 'auto',
                            }}
                        />
                    ) : null}
                </div>
            );
        }

        case 'shape':
            return (
                <div style={{ ...style, clipPath: shapeClip(node) }} data-node={node.id}>
                    {walls}
                </div>
            );

        case 'component': {
            const Widget = WIDGETS[node.ref];
            // Un composant que ce launcher ne connaît pas — thème plus récent
            // que le binaire installé — laisse simplement son cadre, qui reste
            // stylé. Mieux vaut un bloc vide qu'un écran cassé.
            return (
                <div style={style} data-node={node.id} data-component={node.ref}>
                    {walls}
                    {Widget ? <Widget props={node.props} slots={node.slots} runtime={runtime} hug={hugAxes(node.constraints, flow)} /> : null}
                </div>
            );
        }

        default:
            return null;
    }
}

function shapeClip(node: Extract<ThemeNode, { kind: 'shape' }>): string | undefined {
    switch (node.shape) {
        case 'ellipse':
            return 'ellipse(50% 50% at 50% 50%)';
        case 'triangle':
            return 'polygon(50% 0%, 100% 100%, 0% 100%)';
        case 'line':
            return 'inset(calc(50% - 1px) 0)';
        case 'polygon':
            return polygon(node.sides, 0.5);
        case 'star':
            return star(node.sides, node.innerRadius);
        default:
            return undefined;
    }
}

function polygon(sides: number, radius: number): string {
    const points = Array.from({ length: sides }, (_, index) => {
        const angle = (index / sides) * Math.PI * 2 - Math.PI / 2;
        return `${(50 + Math.cos(angle) * radius * 100).toFixed(2)}% ${(50 + Math.sin(angle) * radius * 100).toFixed(2)}%`;
    });
    return `polygon(${points.join(', ')})`;
}

function star(points: number, innerRadius: number): string {
    const total = points * 2;
    const coords = Array.from({ length: total }, (_, index) => {
        const radius = index % 2 === 0 ? 0.5 : 0.5 * innerRadius;
        const angle = (index / total) * Math.PI * 2 - Math.PI / 2;
        return `${(50 + Math.cos(angle) * radius * 100).toFixed(2)}% ${(50 + Math.sin(angle) * radius * 100).toFixed(2)}%`;
    });
    return `polygon(${coords.join(', ')})`;
}

export function ThemedScreen({ document: theme, screen, runtime, shared = true }: ThemedScreenProps) {
    const layer = theme.screens[screen];
    if (!layer) return null;

    const container = { width: theme.window.width, height: theme.window.height };
    const ctx: RenderContext = { mediaUrl: runtime.mediaUrl, fontFamily: runtime.fontFamily };

    // `paintsToCss` écarte déjà les murs de skins, montés juste après comme
    // éléments : l'ordre des couches reste donc celui de l'éditeur.
    const background = toStyle({
        position: 'absolute',
        inset: '0',
        ...paintsToCss(layer.background, ctx),
    });

    return (
        <div style={{ position: 'absolute', inset: 0, overflow: 'hidden', isolation: 'isolate' }}>
            <div style={background} aria-hidden>
                {skinWalls(layer.background, runtime)}
            </div>

            {layer.nodes.map((node) => (
                <ThemedNode key={node.id} node={node} runtime={runtime} ctx={ctx} container={container} flow={false} />
            ))}

            {/* La couche commune passe devant l'écran, comme dans le launcher :
                barre de titre, menu, bandeaux et notifications sont de la
                chrome, et la chrome recouvre le contenu — jamais l'inverse. */}
            {shared && screen !== 'shared'
                ? theme.screens.shared.nodes.map((node) => (
                      <ThemedNode key={node.id} node={node} runtime={runtime} ctx={ctx} container={container} flow={false} />
                  ))
                : null}
        </div>
    );
}

