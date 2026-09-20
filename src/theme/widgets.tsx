/**
 * Composants liés : ce que le thème place, et que le launcher remplit.
 *
 * Répartition des rôles, valable pour chacun d'eux :
 *  - le **cadre** — position, fond, bordure, rayon, ombres — vient du document
 *    et est déjà posé par le rendu sur l'élément parent ;
 *  - le **contenu et le comportement** viennent d'ici.
 *
 * C'est ce partage qui fait qu'un launcher entièrement redessiné reste un
 * launcher : un auteur peut donner au bouton Jouer la forme qu'il veut, il
 * lancera toujours l'instance sélectionnée, et jamais autre chose.
 *
 * Là où un composant existant est déjà exactement le widget attendu — les
 * actualités, la console, le statut serveur — il est réutilisé tel quel plutôt
 * que réécrit : deux rendus d'une même chose finiraient par diverger.
 */

import type { CSSProperties, ReactNode } from 'react';
import { exit } from '@tauri-apps/plugin-process';

import { t } from '../i18n';
import { formatBytes, formatDuration, formatPercent } from '../lib/format';
import { loaderIcon, loaderLabel } from '../lib/instances';
import { usePlatform, type Platform } from '../lib/platform';
import { useActions, useAppState, useBrand, useInstances, useModules, useSelectedAccount, type View } from '../store/AppStore';
import { Icon } from '../components/ui/Icon';
import { Select } from '../components/ui/forms';
import { WindowControls as WindowControlButtons, type ControlsKind } from '../components/layout/WindowControls';
import { SkinFace } from '../features/accounts/SkinFace';
import { AccountsView } from '../features/accounts/AccountsView';
import { LoginModal } from '../features/accounts/LoginModal';
import { GameConsole } from '../features/home/GameConsole';
import { PlayCard } from '../features/home/PlayCard';
import { Badge } from '../components/ui/primitives';
import { NewsFeed } from '../features/home/NewsFeed';
import { ServerStatusCard } from '../features/home/ServerStatusCard';
import { LinksBar } from '../features/links/LinksBar';
import { SettingsView } from '../features/settings/SettingsView';
import { SkinsView } from '../features/skins/SkinsView';
import { effectsToCss, paintsToCss, radiusToCss, strokesToCss, textPaintToCss, textStyleToCss } from './css';
import type { SlotStyle, TextStyle } from './schema';
import type { ThemeRuntime } from './runtime';

export interface WidgetProps {
    props: Record<string, unknown>;
    slots: Record<string, SlotStyle>;
    runtime: ThemeRuntime;
    /**
     * Axes sur lesquels le nœud épouse son contenu (cadre en disposition
     * automatique, ancrage « ajusté »). Un widget se rend d'ordinaire en
     * remplissant sa boîte ; sur un axe ajusté il n'y a pas de boîte, et c'est
     * lui qui donne sa taille au nœud.
     */
    hug: { width: boolean; height: boolean };
}

type Widget = (props: WidgetProps) => ReactNode;

// ── Outils communs ──────────────────────────────────────────────────────────

const fill: CSSProperties = { position: 'absolute', inset: 0 };
const fillFlex: CSSProperties = { ...fill, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, overflow: 'hidden' };
/** Un widget qui défile déborde de son cadre : il faut le contenir. */
const fillScroll: CSSProperties = { ...fill, overflowY: 'auto', overflowX: 'hidden' };
/** Rendu dans le flux : la hauteur vient du contenu, jamais d'un défilement. */
const grow: CSSProperties = { position: 'relative', width: '100%' };

/** Boîte d'un composant riche : remplie et défilante, ou dans le flux si le nœud est ajusté. */
function host(hug: WidgetProps['hug']): CSSProperties {
    return hug.height ? grow : fillScroll;
}

function str(props: Record<string, unknown>, key: string, fallback = ''): string {
    const value = props[key];
    return typeof value === 'string' ? value : fallback;
}

function bool(props: Record<string, unknown>, key: string, fallback = true): boolean {
    const value = props[key];
    return typeof value === 'boolean' ? value : fallback;
}

function num(props: Record<string, unknown>, key: string, fallback: number): number {
    const value = Number(props[key]);
    return Number.isFinite(value) ? value : fallback;
}

/**
 * Habillage d'un sous-élément d'un composant lié.
 *
 * Le registre déclare quels sous-éléments sont stylables ; l'auteur y pose un
 * fond, une couleur de texte, un rayon. C'est ce qui permet de repeindre une
 * entrée de menu ou le libellé du bouton Jouer sans que le launcher ait à
 * exposer trente propriétés distinctes.
 *
 * Renvoie `null` quand le slot est explicitement masqué, pour que l'appelant
 * n'ait pas à traiter ce cas deux fois.
 */
function slotCss(slots: Record<string, SlotStyle>, key: string, runtime: ThemeRuntime): CSSProperties | null {
    const slot = slots?.[key];
    if (!slot) return {};
    if (slot.hidden) return null;

    const ctx = { mediaUrl: runtime.mediaUrl, fontFamily: runtime.fontFamily };
    const style: Record<string, string> = {
        ...(slot.fills ? paintsToCss(slot.fills, ctx) : {}),
        ...(slot.strokes ? strokesToCss(slot.strokes) : {}),
        ...(slot.radius ? radiusToCss(slot.radius) : {}),
        ...(slot.effects ? effectsToCss(slot.effects) : {}),
        ...(slot.color ? textPaintToCss(slot.color, ctx) : {}),
    };

    if (slot.text) {
        // Un slot ne fournit qu'une partie de la typographie : on complète avec
        // des valeurs neutres, puis on ne garde que les clés réellement posées,
        // pour ne pas écraser ce dont le composant hérite.
        const full = textStyleToCss({ ...NEUTRAL_TEXT, ...slot.text } as TextStyle, ctx);
        for (const key_ of Object.keys(slot.text)) {
            const mapped = TEXT_KEYS[key_ as keyof TextStyle];
            if (mapped && full[mapped] !== undefined) style[mapped] = full[mapped];
        }
    }

    if (slot.padding) {
        style.paddingTop = `${slot.padding.top}px`;
        style.paddingRight = `${slot.padding.right}px`;
        style.paddingBottom = `${slot.padding.bottom}px`;
        style.paddingLeft = `${slot.padding.left}px`;
    }
    if (slot.gap !== undefined) style.gap = `${slot.gap}px`;

    return style as unknown as CSSProperties;
}

const NEUTRAL_TEXT: TextStyle = {
    font: 'body',
    size: 14,
    weight: 400,
    lineHeight: 1.4,
    letterSpacing: 0,
    align: 'left',
    verticalAlign: 'middle',
    transform: 'none',
    decoration: 'none',
    italic: false,
    maxLines: 0,
};

/** Clé de `TextStyle` → propriété CSS produite par `textStyleToCss`. */
const TEXT_KEYS: Partial<Record<keyof TextStyle, string>> = {
    font: 'fontFamily',
    size: 'fontSize',
    weight: 'fontWeight',
    lineHeight: 'lineHeight',
    letterSpacing: 'letterSpacing',
    align: 'textAlign',
    transform: 'textTransform',
    decoration: 'textDecoration',
    italic: 'fontStyle',
};

// ── Jeu ─────────────────────────────────────────────────────────────────────

/**
 * La carte de lancement complète : le composant de l'accueil, tel quel.
 *
 * C'est le bloc du document par défaut. Un auteur qui veut composer autrement
 * la remplace par ses pièces détachées — bouton, sélecteur, progression —,
 * chacune disponible séparément dans le registre.
 */
const PlayCardWidget: Widget = ({ hug }) => (
    <div style={host(hug)}>
        <PlayCard />
    </div>
);

const PlayButton: Widget = ({ props, slots, runtime }) => {
    const { game, remote } = useAppState();
    const { selected } = useInstances();
    const account = useSelectedAccount();
    const { launch, openLogin } = useActions();

    const maintenance = remote?.config.maintenance ?? false;
    const busy = game.mode !== 'idle';
    const running = Boolean(game.running);
    const disabled = !selected || busy || running || maintenance;

    const label = running
        ? str(props, 'labelRunning', t('home.running'))
        : busy
          ? str(props, 'labelInstalling', '…')
          : str(props, 'label', t('home.play'));

    // Sans compte, le bouton n'est pas désactivé : il ouvre la connexion. Un
    // bouton grisé sans explication est la première cause d'incompréhension
    // au premier démarrage.
    const onClick = () => {
        if (!account) return openLogin(true);
        if (selected && !disabled) void launch(selected.id);
    };

    const percent = game.progress ? formatPercent(game.progress.downloaded, game.progress.total) : 0;
    const showFill = busy && str(props, 'loadingStyle', 'fill') === 'fill';

    return (
        <button
            type="button"
            onClick={onClick}
            disabled={disabled && Boolean(account)}
            style={{
                ...fillFlex,
                border: 'none',
                background: 'transparent',
                borderRadius: 'inherit',
                color: 'var(--on-brand)',
                fontWeight: 700,
                fontSize: 15,
                cursor: disabled && account ? 'not-allowed' : 'pointer',
                opacity: disabled && account ? 0.55 : 1,
            }}
        >
            {showFill ? (
                <span
                    aria-hidden
                    style={{
                        position: 'absolute',
                        inset: 0,
                        width: `${percent}%`,
                        background: 'rgba(255,255,255,0.18)',
                        transition: 'width 240ms var(--ease)',
                        ...slotCss(slots, 'progress', runtime),
                    }}
                />
            ) : null}
            {bool(props, 'showIcon') && slotCss(slots, 'icon', runtime) ? (
                <span style={slotCss(slots, 'icon', runtime)!}>
                    <Icon
                        name={busy ? 'progress_activity' : str(props, 'icon', 'play_arrow')}
                        size={num(props, 'iconSize', 22)}
                        spin={busy && str(props, 'loadingStyle', 'fill') === 'spinner'}
                    />
                </span>
            ) : null}
            {slotCss(slots, 'label', runtime) ? (
                <span style={{ position: 'relative', ...slotCss(slots, 'label', runtime) }}>{label}</span>
            ) : null}
        </button>
    );
};

const InstancePicker: Widget = ({ props }) => {
    const { game } = useAppState();
    const { instances, selected } = useInstances();
    const { selectInstance } = useActions();

    if (bool(props, 'hideWhenSingle') && instances.length <= 1) return null;

    return (
        <div style={{ ...fill, display: 'flex', alignItems: 'center' }}>
            <Select
                value={selected?.id ?? null}
                placeholder={instances.length === 0 ? t('home.noInstance') : t('home.chooseInstance')}
                disabled={game.mode !== 'idle' || instances.length === 0}
                onChange={(id) => void selectInstance(id)}
                options={instances.map((instance) => ({
                    value: instance.id,
                    label: instance.name,
                    description: bool(props, 'showVersion')
                        ? `Minecraft ${instance.minecraftVersion} · ${loaderLabel(instance.loader.kind)}`
                        : undefined,
                    icon: bool(props, 'showImage') ? loaderIcon(instance.loader.kind) : undefined,
                }))}
            />
        </div>
    );
};

const LaunchProgressWidget: Widget = ({ props, slots, runtime }) => {
    const { game } = useAppState();
    if (game.mode === 'idle') return null;

    const stage = game.stage ?? 'resolving';
    const download = stage === 'downloading' ? game.progress : null;
    const check = stage === 'checking' ? game.check : null;
    const percent = download
        ? formatPercent(download.downloaded, download.total)
        : check
          ? formatPercent(check.checked, check.total)
          : 0;

    const details = [
        bool(props, 'showSpeed') && game.speed ? `${formatBytes(game.speed)}/s` : null,
        bool(props, 'showEta') && game.eta ? formatDuration(game.eta) : null,
        bool(props, 'showFile', false) && download?.element ? download.element : null,
    ].filter(Boolean);

    const style = str(props, 'barStyle', 'bar');
    const thickness = num(props, 'thickness', 8);

    return (
        <div style={{ ...fill, display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: 6, overflow: 'hidden' }}>
            {bool(props, 'showStage') || bool(props, 'showPercent') ? (
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, fontSize: 12 }}>
                    {bool(props, 'showStage') && slotCss(slots, 'stage', runtime) ? (
                        <span style={{ color: 'var(--text-secondary)', ...slotCss(slots, 'stage', runtime) }}>{t(`home.stage.${stage}`)}</span>
                    ) : null}
                    {bool(props, 'showPercent') ? (
                        <span style={{ color: 'var(--text-primary)', fontWeight: 600 }}>{percent}%</span>
                    ) : null}
                </div>
            ) : null}

            {style === 'bar' || style === 'segments' ? (
                <div style={{ height: thickness, borderRadius: 999, background: 'var(--bg-secondary)', overflow: 'hidden', ...slotCss(slots, 'track', runtime) }}>
                    <div
                        style={{
                            width: `${percent}%`,
                            height: '100%',
                            borderRadius: 999,
                            background: 'var(--grad-progress)',
                            transition: 'width 240ms var(--ease)',
                            ...slotCss(slots, 'fill', runtime),
                            // Segments : le même remplissage, simplement masqué
                            // en bandes — inutile de monter N éléments.
                            ...(style === 'segments'
                                ? { maskImage: 'repeating-linear-gradient(90deg, #000 0 10px, transparent 10px 14px)' }
                                : {}),
                        }}
                    />
                </div>
            ) : null}

            {style === 'ring' ? (
                <div
                    style={{
                        width: thickness * 5,
                        height: thickness * 5,
                        borderRadius: '50%',
                        margin: '0 auto',
                        background: `conic-gradient(var(--accent-500) ${percent * 3.6}deg, var(--bg-secondary) 0)`,
                        mask: `radial-gradient(circle, transparent calc(50% - ${thickness}px), #000 calc(50% - ${thickness}px))`,
                    }}
                />
            ) : null}

            {details.length && slotCss(slots, 'detail', runtime) ? (
                <span style={{ fontSize: 11, color: 'var(--text-meta)', ...slotCss(slots, 'detail', runtime) }}>{details.join(' · ')}</span>
            ) : null}
        </div>
    );
};

const ServerStatus: Widget = ({ hug }) => {
    const { selected } = useInstances();
    return (
        <div style={host(hug)}>
            <ServerStatusCard instance={selected} />
        </div>
    );
};

const Console: Widget = ({ props, hug }) => {
    const { game } = useAppState();
    if (bool(props, 'onlyWhenRunning') && !game.running) return null;
    return (
        <div style={host(hug)}>
            <GameConsole />
        </div>
    );
};

const FolderButton: Widget = ({ props, slots, runtime }) => {
    const { selected } = useInstances();
    const { openFolder } = useActions();
    const target = str(props, 'target', 'instance');
    const resolved = target === 'instance' && selected ? `instance:${selected.id}` : target;

    return (
        <IconAction
            icon={str(props, 'icon', 'folder_open')}
            iconSize={num(props, 'iconSize', 20)}
            label={str(props, 'label')}
            onClick={() => void openFolder(resolved)}
            slots={slots}
            runtime={runtime}
        />
    );
};

/**
 * Le « Bonjour Luuxis » de l'accueil.
 *
 * Un calque texte ne sait pas ne peindre qu'un mot en dégradé : d'où un
 * composant, qui garde la salutation libellable et le pseudo vivant.
 */
const Greeting: Widget = ({ props, slots, runtime }) => {
    const account = useSelectedAccount();
    const size = num(props, 'size', 36);
    const textStyle = slotCss(slots, 'text', runtime);
    const nameStyle = slotCss(slots, 'name', runtime);

    return (
        <div style={{ ...fill, display: 'flex', alignItems: 'center', overflow: 'hidden' }}>
            <h1
                className="font-display"
                style={{ margin: 0, fontSize: size, fontWeight: 700, lineHeight: 1.1, whiteSpace: 'nowrap', color: 'var(--text-primary)', ...textStyle }}
            >
                {account ? (
                    <>
                        {str(props, 'text', t('home.greeting'))}{' '}
                        <span className={bool(props, 'gradientName') ? 'text-gradient' : ''} style={nameStyle ?? undefined}>
                            {account.name}
                        </span>
                    </>
                ) : (
                    str(props, 'guestText', t('home.greetingGuest'))
                )}
            </h1>
        </div>
    );
};

const InstanceBadge: Widget = ({ props }) => {
    const { selected } = useInstances();
    const { game } = useAppState();
    return (
        <div style={{ ...fill, display: 'flex', alignItems: 'center', gap: 8, overflow: 'hidden' }}>
            {bool(props, 'showRunning') && game.running ? (
                <Badge variant="brand" dot pulse>
                    {t('home.running')}
                </Badge>
            ) : null}
            {selected ? <Badge variant="neutral">{selected.name}</Badge> : null}
        </div>
    );
};

// ── Compte ──────────────────────────────────────────────────────────────────

const AccountChip: Widget = ({ props, slots, runtime }) => {
    const account = useSelectedAccount();
    const { navigate, openLogin } = useActions();
    const size = num(props, 'headSize', 32);
    const shape = str(props, 'headShape', 'rounded');
    const radius = shape === 'circle' ? 9999 : shape === 'square' ? 0 : 'var(--radius-sm)';

    return (
        <button
            type="button"
            onClick={() => (account ? navigate('accounts') : openLogin(true))}
            style={{
                ...fill,
                display: 'flex',
                alignItems: 'center',
                gap: 10,
                padding: '0 8px',
                border: 'none',
                background: 'transparent',
                borderRadius: 'inherit',
                cursor: 'pointer',
                textAlign: 'left',
                overflow: 'hidden',
            }}
        >
            {bool(props, 'showHead') && slotCss(slots, 'head', runtime) ? (
                account ? (
                    <SkinFace account={account} size={size} className="shrink-0" />
                ) : (
                    <span
                        className="icon-chip shrink-0"
                        style={{ width: size, height: size, borderRadius: radius, display: 'grid', placeItems: 'center' }}
                    >
                        <Icon name="person_add" size={Math.round(size * 0.5)} />
                    </span>
                )
            ) : null}

            <span style={{ minWidth: 0, flex: 1, display: 'flex', flexDirection: 'column' }}>
                {bool(props, 'showName') && slotCss(slots, 'name', runtime) ? (
                    <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)', ...slotCss(slots, 'name', runtime) }} className="truncate">
                        {account?.name ?? t('nav.noAccount')}
                    </span>
                ) : null}
                {bool(props, 'showKind') && slotCss(slots, 'kind', runtime) ? (
                    <span style={{ fontSize: 10, color: 'var(--text-meta)', ...slotCss(slots, 'kind', runtime) }} className="truncate">
                        {account ? t(`accounts.kinds.${account.kind}`) : t('nav.addAccount')}
                    </span>
                ) : null}
            </span>

            {bool(props, 'showStatusDot') && account ? (
                <span
                    aria-hidden
                    style={{
                        width: 6,
                        height: 6,
                        borderRadius: '50%',
                        flexShrink: 0,
                        background: account.needsReauth ? 'var(--warning-500)' : 'var(--accent-500)',
                    }}
                />
            ) : null}
        </button>
    );
};

const PlayerHead: Widget = ({ props, runtime }) => {
    const account = useSelectedAccount();
    const fallback = typeof props.fallback === 'string' ? runtime.mediaUrl(props.fallback) : null;

    if (!account) {
        return fallback ? (
            <img src={fallback} alt="" style={{ ...fill, objectFit: 'cover', borderRadius: 'inherit' }} className="pixelated" />
        ) : null;
    }
    return (
        <div style={{ ...fill, display: 'grid', placeItems: 'stretch', borderRadius: 'inherit', overflow: 'hidden' }}>
            <SkinFace account={account} size={256} className="w-full h-full" />
        </div>
    );
};

const AccountsList: Widget = ({ hug }) => (
    <div style={host(hug)}>
        <AccountsView />
    </div>
);

const AddAccountButton: Widget = ({ props, slots, runtime }) => {
    const { openLogin } = useActions();
    return (
        <IconAction
            icon={str(props, 'icon', 'person_add')}
            iconSize={num(props, 'iconSize', 20)}
            label={str(props, 'label', t('accounts.add'))}
            showIcon={bool(props, 'showIcon')}
            onClick={() => openLogin(true)}
            slots={slots}
            runtime={runtime}
        />
    );
};

/**
 * Formulaire de connexion.
 *
 * Le même composant que la fenêtre de connexion, rendu sans son habillage de
 * modale : le cadre, le titre et le voile appartiennent au thème. Une seule
 * implémentation du flux d'authentification, donc un seul endroit où corriger
 * un jour un comportement.
 */
const LoginForm: Widget = ({ props, hug }) => {
    const { openLogin } = useActions();
    return (
        <div style={host(hug)}>
            <LoginModal
                open
                inline
                onClose={() => openLogin(false)}
                title={str(props, 'title', t('login.title'))}
                subtitle={str(props, 'subtitle', t('login.subtitle'))}
            />
        </div>
    );
};

// ── Skins ───────────────────────────────────────────────────────────────────

const SkinLibrary: Widget = ({ hug }) => (
    <div style={host(hug)}>
        <SkinsView />
    </div>
);

/**
 * Aperçu 3D.
 *
 * Le rendu tridimensionnel vit dans l'écran Skins, qui porte l'état de
 * l'essayage et le chargement des textures. Le sortir de là pour le poser
 * ailleurs demanderait de dupliquer cet état ; le thème contrôle donc sa
 * position et son cadre, et l'écran Skins reste maître de son contenu.
 */
const SkinViewerWidget: Widget = ({ hug }) => (
    <div style={host(hug)}>
        <SkinsView />
    </div>
);

// ── Contenu ─────────────────────────────────────────────────────────────────

const News: Widget = ({ hug }) => (
    <div style={host(hug)}>
        <NewsFeed />
    </div>
);

const Links: Widget = ({ props }) => (
    <div
        style={{
            ...fill,
            display: 'flex',
            flexDirection: str(props, 'direction', 'row') === 'column' ? 'column' : 'row',
            flexWrap: str(props, 'direction', 'row') === 'grid' ? 'wrap' : 'nowrap',
            alignItems: 'center',
            justifyContent: 'center',
            gap: num(props, 'gap', 8),
            overflow: 'hidden',
        }}
    >
        <LinksBar compact />
    </div>
);

const BrandLogo: Widget = ({ props, runtime }) => {
    const brand = useBrand();
    const source = str(props, 'source', 'config');
    const url = source === 'media' && typeof props.mediaId === 'string' ? runtime.mediaUrl(props.mediaId) : brand.iconUrl;

    if (!url) {
        return (
            <span style={{ ...fillFlex, borderRadius: 'inherit', color: 'var(--text-placeholder)' }}>
                <Icon name="rocket_launch" size={22} />
            </span>
        );
    }
    return (
        <img
            src={url}
            alt=""
            style={{ ...fill, width: '100%', height: '100%', objectFit: str(props, 'fit', 'contain') as CSSProperties['objectFit'], borderRadius: 'inherit' }}
        />
    );
};

const BrandWordmark: Widget = ({ props, slots, runtime }) => {
    const brand = useBrand();
    return (
        <div style={{ ...fill, display: 'flex', flexDirection: 'column', justifyContent: 'center', overflow: 'hidden' }}>
            <span className="font-display" style={{ fontWeight: 700, fontSize: 18, lineHeight: 1.15 }}>
                <span style={{ color: 'var(--text-primary)', ...slotCss(slots, 'prefix', runtime) }}>{brand.wordmark.prefix}</span>
                {brand.wordmark.suffix ? (
                    <span
                        className={bool(props, 'gradientSuffix') ? 'text-gradient' : ''}
                        style={{ ...(bool(props, 'gradientSuffix') ? {} : { color: 'var(--text-primary)' }), ...slotCss(slots, 'suffix', runtime) }}
                    >
                        {brand.wordmark.suffix}
                    </span>
                ) : null}
            </span>
            {bool(props, 'showSubtitle', false) && slotCss(slots, 'subtitle', runtime) ? (
                <span style={{ fontSize: 11, color: 'var(--text-meta)', ...slotCss(slots, 'subtitle', runtime) }}>{brand.subtitle}</span>
            ) : null}
        </div>
    );
};

// ── Navigation et système ───────────────────────────────────────────────────

const NAV_ITEMS: { id: View; module: string; icon: string; label: string }[] = [
    { id: 'home', module: 'home', icon: 'space_dashboard', label: 'nav.home' },
    { id: 'accounts', module: 'accounts', icon: 'manage_accounts', label: 'nav.accounts' },
    { id: 'skins', module: 'skins', icon: 'person', label: 'nav.skins' },
    { id: 'settings', module: 'settings', icon: 'settings', label: 'nav.settings' },
];

const NavMenu: Widget = ({ props, slots, runtime }) => {
    const { view, game } = useAppState();
    const { navigate } = useActions();
    const enabled = useModules();
    const items = NAV_ITEMS.filter((item) => item.id === 'home' || enabled(item.module));

    const active = str(props, 'activeStyle', 'bar');
    const showLabels = bool(props, 'showLabels');
    const iconSize = num(props, 'iconSize', 20);
    const itemStyle = slotCss(slots, 'item', runtime) ?? {};
    const activeStyle = slotCss(slots, 'itemActive', runtime) ?? {};
    const iconStyle = slotCss(slots, 'icon', runtime);

    return (
        <nav
            style={{
                ...fill,
                display: 'flex',
                flexDirection: str(props, 'direction', 'column') === 'row' ? 'row' : 'column',
                gap: num(props, 'gap', 4),
                overflow: 'hidden',
            }}
        >
            {items.map((item) => {
                const on = view === item.id;
                // `sidebar-item` est la classe du menu intégré : en style « barre »,
                // le rendu est donc exactement celui du launcher sans thème.
                const bar = active === 'bar';
                return (
                    <button
                        key={item.id}
                        type="button"
                        onClick={() => navigate(item.id)}
                        aria-current={on ? 'page' : undefined}
                        className={bar ? `sidebar-item ${on ? 'active' : ''}` : undefined}
                        style={{
                            ...(bar
                                ? {}
                                : {
                                      display: 'flex',
                                      alignItems: 'center',
                                      gap: 12,
                                      padding: '10px 12px',
                                      borderRadius: 'var(--radius-sm)',
                                      border: '1px solid transparent',
                                      borderBottom: active === 'underline' ? `2px solid ${on ? 'var(--accent-500)' : 'transparent'}` : undefined,
                                      background: on && active === 'fill' ? 'color-mix(in srgb, var(--accent-500) 16%, transparent)' : 'transparent',
                                      boxShadow: on && active === 'glow' ? 'var(--glow-sm)' : undefined,
                                      color: on ? 'var(--text-primary)' : 'var(--text-label)',
                                      fontSize: 14,
                                      fontWeight: 500,
                                      textAlign: 'left',
                                      cursor: on ? 'default' : 'pointer',
                                  }),
                            flexShrink: 0,
                            ...itemStyle,
                            ...(on ? activeStyle : {}),
                        }}
                    >
                        {iconStyle ? (
                            <span style={iconStyle}>
                                <Icon name={item.icon} size={iconSize} className={on && !bar ? 'text-brand-500' : ''} />
                            </span>
                        ) : null}
                        {showLabels ? <span style={{ flex: 1 }}>{t(item.label)}</span> : null}
                        {bool(props, 'showRunningDot') && item.id === 'home' && game.running ? (
                            <span aria-hidden style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--accent-500)' }} />
                        ) : null}
                    </button>
                );
            })}
        </nav>
    );
};

const ScreenLink: Widget = ({ props, slots, runtime }) => {
    const { navigate, openLogin, openExternal } = useActions();
    const target = str(props, 'target', 'settings');

    const onClick = () => {
        if (target === 'url') {
            const url = str(props, 'url');
            // Une adresse s'ouvre dans le navigateur du joueur, jamais dans la
            // fenêtre du launcher : une page tierce n'a rien à faire dans un
            // contexte qui a accès à ses comptes.
            if (/^https?:\/\//i.test(url)) void openExternal(url);
            return;
        }
        if (target === 'login') return openLogin(true);
        navigate(target as View);
    };

    return (
        <IconAction
            icon={str(props, 'icon', 'settings')}
            iconSize={num(props, 'iconSize', 20)}
            label={str(props, 'label')}
            showIcon={bool(props, 'showIcon')}
            onClick={onClick}
            slots={slots}
            runtime={runtime}
        />
    );
};

const QuitButton: Widget = ({ props, slots, runtime }) => (
    <IconAction
        icon={str(props, 'icon', 'power_settings_new')}
        iconSize={num(props, 'iconSize', 20)}
        label={str(props, 'label')}
        onClick={() => void exit(0)}
        slots={slots}
        runtime={runtime}
    />
);

/** `auto` suit le système ; `traffic` et `symbols` l'imposent. */
function controlsKind(style: string, platform: Platform): ControlsKind {
    if (style === 'traffic') return 'traffic';
    if (style === 'symbols') return 'symbols';
    return platform === 'macos' ? 'traffic' : 'symbols';
}

/**
 * La barre de titre complète : zone de déplacement, boutons de fenêtre placés
 * selon le système du joueur, et au centre — si l'auteur le demande — le logo
 * et le nom. Le même dessin que la barre intégrée (`TitleBar.tsx`) : un thème
 * qui garde le composant par défaut obtient exactement la barre native.
 */
const TitleBarWidget: Widget = ({ props, slots, runtime }) => {
    const platform = usePlatform();
    const brand = useBrand();
    const kind = controlsKind(str(props, 'controls', 'auto'), platform);
    const placement = str(props, 'placement', 'auto');
    const left = placement === 'left' || (placement === 'auto' && kind === 'traffic');

    const controls = (
        <WindowControlButtons
            kind={kind}
            buttonStyle={slotCss(slots, 'button', runtime)}
            closeStyle={slotCss(slots, 'close', runtime)}
        />
    );

    return (
        <div data-tauri-drag-region style={{ ...fill, display: 'flex', alignItems: 'center', borderRadius: 'inherit', overflow: 'hidden' }}>
            {left ? controls : null}
            {/* Tauri ne déclenche le déplacement que sur l'élément qui porte
                l'attribut : l'espace vide en a besoin lui aussi. */}
            <div data-tauri-drag-region style={{ flex: 1, height: '100%' }} />
            {left ? null : controls}

            {bool(props, 'showLogo', false) || bool(props, 'showName', false) ? (
                <div
                    style={{
                        position: 'absolute',
                        left: '50%',
                        top: '50%',
                        transform: 'translate(-50%, -50%)',
                        display: 'flex',
                        alignItems: 'center',
                        gap: 8,
                        pointerEvents: 'none',
                    }}
                >
                    {bool(props, 'showLogo', false) ? (
                        brand.iconUrl ? (
                            <img src={brand.iconUrl} alt="" style={{ width: 18, height: 18, objectFit: 'contain', borderRadius: 4 }} draggable={false} />
                        ) : (
                            <Icon name="rocket_launch" size={16} style={{ color: 'var(--accent-400)' }} />
                        )
                    ) : null}
                    {bool(props, 'showName', false) && slotCss(slots, 'name', runtime) ? (
                        <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-secondary)', ...slotCss(slots, 'name', runtime) }}>{brand.name}</span>
                    ) : null}
                </div>
            ) : null}
        </div>
    );
};

const WindowControls: Widget = ({ props, slots, runtime }) => {
    const platform = usePlatform();
    const kind = controlsKind(str(props, 'style', 'auto'), platform);
    return (
        <div style={{ ...fill, display: 'flex', alignItems: 'stretch', justifyContent: kind === 'traffic' ? 'flex-start' : 'flex-end', borderRadius: 'inherit', overflow: 'hidden' }}>
            <WindowControlButtons
                kind={kind}
                showMinimize={bool(props, 'showMinimize')}
                showMaximize={bool(props, 'showMaximize')}
                iconSize={num(props, 'iconSize', 16)}
                buttonStyle={slotCss(slots, 'button', runtime)}
                closeStyle={slotCss(slots, 'close', runtime)}
            />
        </div>
    );
};

/**
 * Zone de déplacement de la fenêtre.
 *
 * `data-tauri-drag-region` est lu par le moteur au niveau du DOM : aucun
 * gestionnaire à brancher, et le déplacement reste fluide parce qu'il n'est
 * jamais arbitré par JavaScript.
 */
const DragRegion: Widget = () => <div data-tauri-drag-region style={{ ...fill, borderRadius: 'inherit' }} />;

const SettingsPanel: Widget = ({ hug }) => (
    <div style={host(hug)}>
        <SettingsView />
    </div>
);

const MaintenanceBanner: Widget = ({ props, slots, runtime }) => {
    const { remote } = useAppState();
    if (!remote?.config.maintenance) return null;

    const message = (remote.config.maintenanceMessage ?? '')
        .replace(/<br\s*\/?>/gi, ' · ')
        .replace(/<[^>]+>/g, '');

    return (
        <div style={{ ...fill, display: 'flex', alignItems: 'center', gap: 10, padding: '0 14px', borderRadius: 'inherit', overflow: 'hidden' }} role="alert">
            {bool(props, 'showIcon') ? <Icon name={str(props, 'icon', 'engineering')} size={18} /> : null}
            <span style={{ fontWeight: 600, ...slotCss(slots, 'text', runtime) }}>{t('boot.maintenanceTitle')}</span>
            <span className="truncate" style={{ opacity: 0.9, ...slotCss(slots, 'text', runtime) }}>{message}</span>
        </div>
    );
};

const OfflineBanner: Widget = ({ props, slots, runtime }) => {
    const { remote, remoteLoading } = useAppState();
    const { refreshRemote } = useActions();
    if (!remote?.stale) return null;

    return (
        <div style={{ ...fill, display: 'flex', alignItems: 'center', gap: 10, padding: '0 14px', borderRadius: 'inherit', overflow: 'hidden' }} role="status">
            {bool(props, 'showIcon') ? <Icon name="cloud_off" size={18} /> : null}
            <span style={{ fontWeight: 600, ...slotCss(slots, 'text', runtime) }}>{t('boot.offlineTitle')}</span>
            <span className="truncate" style={{ flex: 1, opacity: 0.9, ...slotCss(slots, 'text', runtime) }}>{t('boot.offlineText')}</span>
            {bool(props, 'showRetry') ? (
                <button
                    type="button"
                    onClick={() => void refreshRemote()}
                    disabled={remoteLoading}
                    style={{ border: 'none', background: 'rgba(255,255,255,0.18)', color: 'inherit', borderRadius: 999, padding: '4px 12px', fontSize: 12, cursor: 'pointer' }}
                >
                    {t('common.retry')}
                </button>
            ) : null}
        </div>
    );
};

const BootIndicator: Widget = ({ props, slots, runtime }) => {
    const { bootMessage } = useAppState();
    const style = str(props, 'style', 'spinner');
    const size = num(props, 'size', 40);

    return (
        <div style={{ ...fill, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 10, overflow: 'hidden' }}>
            {style === 'spinner' ? <Icon name="progress_activity" size={size} spin /> : null}
            {style === 'pulse' ? (
                <span className="icon-chip animate-pulse-glow" style={{ width: size, height: size, borderRadius: 16, display: 'grid', placeItems: 'center' }}>
                    <Icon name="rocket_launch" size={Math.round(size * 0.6)} />
                </span>
            ) : null}
            {style === 'bar' ? (
                <div className="progress progress-indeterminate" style={{ width: '70%' }}>
                    <div className="progress-fill" />
                </div>
            ) : null}
            {style === 'dots' ? (
                <div style={{ display: 'flex', gap: 6 }}>
                    {[0, 1, 2].map((index) => (
                        <span
                            key={index}
                            className="animate-pulse"
                            style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--accent-500)', animationDelay: `${index * 0.15}s` }}
                        />
                    ))}
                </div>
            ) : null}
            {bool(props, 'showMessage') && slotCss(slots, 'message', runtime) ? (
                <span style={{ fontSize: 12, color: 'var(--text-body)', ...slotCss(slots, 'message', runtime) }}>{bootMessage}</span>
            ) : null}
        </div>
    );
};

/**
 * Les notifications sont montées une fois par l'ossature de l'application, pas
 * par le thème : deux piles de toasts se recouvriraient. Ce widget n'existe que
 * pour que l'auteur puisse en placer l'ancrage, lu par la pile elle-même.
 */
const ToastHost: Widget = ({ props }) => (
    <div
        data-toast-anchor={str(props, 'position', 'bottom-right')}
        data-toast-max={String(num(props, 'maxVisible', 3))}
        style={fill}
        aria-hidden
    />
);

const UpdateBanner: Widget = () => null;

// ── Bouton générique ────────────────────────────────────────────────────────

function IconAction({
    icon,
    iconSize,
    label,
    showIcon = true,
    onClick,
    slots,
    runtime,
}: {
    icon: string;
    iconSize: number;
    label?: string;
    showIcon?: boolean;
    onClick: () => void;
    slots?: Record<string, SlotStyle>;
    runtime?: ThemeRuntime;
}) {
    const slot = (key: string): CSSProperties | null =>
        slots && runtime ? slotCss(slots, key, runtime) : {};

    const iconStyle = slot('icon');
    const labelStyle = slot('label');

    return (
        <button
            type="button"
            onClick={onClick}
            title={label || undefined}
            style={{ ...fillFlex, border: 'none', background: 'transparent', borderRadius: 'inherit', color: 'var(--text-secondary)', cursor: 'pointer' }}
        >
            {showIcon && iconStyle ? (
                <span style={iconStyle}>
                    <Icon name={icon} size={iconSize} />
                </span>
            ) : null}
            {label && labelStyle ? <span style={{ fontWeight: 600, fontSize: 13, ...labelStyle }}>{label}</span> : null}
        </button>
    );
}

// ── Registre ────────────────────────────────────────────────────────────────

export const WIDGETS: Record<string, Widget> = {
    'play-card': PlayCardWidget,
    'play-button': PlayButton,
    'instance-picker': InstancePicker,
    'launch-progress': LaunchProgressWidget,
    'server-status': ServerStatus,
    'game-console': Console,
    'folder-button': FolderButton,
    'account-chip': AccountChip,
    'player-head': PlayerHead,
    'accounts-list': AccountsList,
    'add-account-button': AddAccountButton,
    'login-form': LoginForm,
    'skin-library': SkinLibrary,
    'skin-viewer': SkinViewerWidget,
    'news-feed': News,
    greeting: Greeting,
    'instance-badge': InstanceBadge,
    'links-bar': Links,
    'brand-logo': BrandLogo,
    'brand-wordmark': BrandWordmark,
    'nav-menu': NavMenu,
    'screen-link': ScreenLink,
    'quit-button': QuitButton,
    'title-bar': TitleBarWidget,
    'window-controls': WindowControls,
    'drag-region': DragRegion,
    'settings-panel': SettingsPanel,
    'maintenance-banner': MaintenanceBanner,
    'offline-banner': OfflineBanner,
    'boot-indicator': BootIndicator,
    'toast-host': ToastHost,
    'update-banner': UpdateBanner,
};
