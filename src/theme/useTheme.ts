/**
 * Branchement du thème sur l'état du launcher.
 *
 * Un seul point d'entrée pour l'interface : `useTheme()` applique les couleurs
 * et rend, s'il en existe une, la mise en page composée par le propriétaire.
 *
 * Le repli est la propriété qui compte ici. Document absent, illisible, ou plus
 * récent que ce binaire : dans tous les cas `document` vaut `null` et
 * l'interface garde sa mise en page intégrée — habillée des couleurs du client,
 * qui voyagent séparément. Un thème ne peut donc pas rendre un launcher
 * inutilisable, seulement le décorer.
 */

import { useEffect, useMemo } from 'react';

import { useAppState, useBrand, useInstances, useSelectedAccount } from '../store/AppStore';
import { SCHEMA_VERSION, type ThemeDocument } from './schema';
import { createRuntime, type ThemeRuntime } from './runtime';
import { applyTheme } from './variables';
import type { ThemeMode } from './tokens';

export interface ThemeState {
    /** Mise en page du propriétaire, ou `null` pour la disposition intégrée. */
    document: ThemeDocument | null;
    runtime: ThemeRuntime;
}

/**
 * Vérification minimale avant de confier un document au rendu.
 *
 * La validation complète appartient au panel, qui l'a déjà faite ; ce qui est
 * contrôlé ici, c'est seulement qu'on parle bien du même schéma et que la
 * structure attendue est là. Un binaire ancien face à un document plus récent
 * doit se replier, pas tenter un rendu partiel.
 */
function usable(value: unknown, schemaVersion: number): value is ThemeDocument {
    if (schemaVersion !== SCHEMA_VERSION) return false;
    if (!value || typeof value !== 'object') return false;

    const document_ = value as Partial<ThemeDocument>;
    return (
        typeof document_.window === 'object' &&
        document_.window !== null &&
        typeof document_.screens === 'object' &&
        document_.screens !== null &&
        Array.isArray(document_.screens.home?.nodes)
    );
}

export function useTheme(): ThemeState {
    const { remote, settings, game } = useAppState();
    const { instances, selected } = useInstances();
    const account = useSelectedAccount();
    const brand = useBrand();

    const theme = remote?.theme ?? null;
    const accent = remote?.config.accentColor ?? null;

    const document_ = useMemo(
        () => (theme && usable(theme.document, theme.schemaVersion) ? theme.document : null),
        [theme],
    );

    // Préférence locale du joueur, résolue ici : `system` doit devenir un mode
    // concret avant d'entrer dans le calcul des variables.
    const preferred: ThemeMode = useMemo(() => {
        const choice = settings?.ui.theme ?? 'dark';
        if (choice === 'light' || choice === 'dark') return choice;
        return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }, [settings?.ui.theme]);

    useEffect(() => {
        applyTheme(theme, accent, preferred);
    }, [theme, accent, preferred]);

    const runtime = useMemo(
        () =>
            createRuntime(document_, {
                account,
                instance: selected,
                instanceCount: instances.length,
                game: { running: game.running, mode: game.mode, hasLogs: game.logs.length > 0 },
                remote,
                brandName: brand.name,
            }),
        [document_, account, selected, instances.length, game.running, game.mode, game.logs.length, remote, brand.name],
    );

    return { document: document_, runtime };
}
