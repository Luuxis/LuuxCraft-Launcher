/**
 * Contexte d'exécution d'un thème : ce que le document ne sait pas et que seul
 * le launcher peut lui dire.
 *
 * Trois services :
 *  - **les états** (`VisibilityFlag`), pour décider si un calque conditionnel
 *    doit être monté ;
 *  - **les jetons** (`{player}`, `{instance}`…), remplacés dans les textes ;
 *  - **les médias**, dont le document ne connaît que l'identifiant.
 *
 * Les trois sont rassemblés ici parce qu'ils partagent la même source — l'état
 * global du launcher — et qu'un composant de rendu ne doit avoir à consulter
 * qu'un seul objet.
 */

import type { MediaRef, ThemeDocument, VisibilityCondition, VisibilityFlag } from './schema';
import type { AccountSummary, Instance, RemoteSnapshot, RunningGame } from '../lib/types';
import { fontStackFor } from './variables';

export interface RuntimeInput {
    account: AccountSummary | null;
    instance: Instance | null;
    instanceCount: number;
    game: { running: RunningGame | null; mode: string; hasLogs: boolean };
    remote: RemoteSnapshot | null;
    brandName: string;
}

export interface ThemeRuntime {
    flags: ReadonlySet<VisibilityFlag>;
    /** Remplace les jetons d'un texte par les valeurs du moment. */
    interpolate(content: string): string;
    /** URL d'un média déclaré par le document, ou `null`. */
    mediaUrl(id: string): string | null;
    /** Identifiants des skins de la médiathèque, pour le mur de skins. */
    skinIds(): string[];
    fontFamily(token: string): string;
}

/** États du launcher auxquels un calque peut se lier. */
export function computeFlags(input: RuntimeInput): Set<VisibilityFlag> {
    const flags = new Set<VisibilityFlag>();

    if (input.account) flags.add('account-connected');
    else flags.add('account-missing');

    if (input.game.running) flags.add('game-running');
    else flags.add('game-idle');

    // La console a de quoi montrer dès qu'une partie tourne — ou en a laissé
    // un journal, que le joueur veut souvent relire après un plantage.
    if (input.game.running || input.game.hasLogs) flags.add('game-logs');

    // « Téléchargement en cours » couvre toute la préparation — résolution,
    // vérification, extraction — et pas seulement le transfert : c'est le
    // moment où l'auteur veut montrer une barre, quelle qu'en soit l'étape.
    if (input.game.mode !== 'idle' && !input.game.running) flags.add('download-active');

    if (input.remote?.config.maintenance) flags.add('maintenance');
    if (input.remote?.stale) flags.add('offline');

    if (input.instance) flags.add('instance-selected');
    if (input.instanceCount > 1) flags.add('multiple-instances');

    return flags;
}

/**
 * Un calque sans condition est toujours monté. Avec une condition, les trois
 * clauses se combinent : toutes celles de `all`, au moins une de `any`, aucune
 * de `none`.
 */
export function isVisible(condition: VisibilityCondition | undefined, flags: ReadonlySet<VisibilityFlag>): boolean {
    if (!condition) return true;
    if (condition.all?.some((flag) => !flags.has(flag))) return false;
    if (condition.any?.length && !condition.any.some((flag) => flags.has(flag))) return false;
    if (condition.none?.some((flag) => flags.has(flag))) return false;
    return true;
}

const TOKEN_RE = /\{(\w+)\}/g;

export function createRuntime(document_: ThemeDocument | null, input: RuntimeInput): ThemeRuntime {
    const flags = computeFlags(input);
    const media = new Map<string, MediaRef>((document_?.media ?? []).map((entry) => [entry.id, entry]));

    const values: Record<string, string> = {
        player: input.account?.name ?? '—',
        instance: input.instance?.name ?? '—',
        version: input.instance?.minecraftVersion ?? '—',
        brand: input.brandName,
        online: '—',
        max: '—',
    };

    return {
        flags,
        interpolate: (content) =>
            // Un jeton inconnu est laissé tel quel : l'auteur voit sa faute de
            // frappe plutôt qu'un trou dans sa phrase.
            content.replace(TOKEN_RE, (match, key: string) => values[key] ?? match),
        mediaUrl: (id) => media.get(id)?.url ?? null,
        skinIds: () => (document_?.media ?? []).filter((entry) => entry.kind === 'skin').map((entry) => entry.id),
        fontFamily: fontStackFor,
    };
}
