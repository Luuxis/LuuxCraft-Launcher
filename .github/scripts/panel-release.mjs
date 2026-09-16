#!/usr/bin/env node
/**
 * Publication d'une version du launcher vers le panel LuuxCraft.
 *
 * Une release porte **deux lignes de produit**, et rien d'autre :
 *
 *   bootstrap  l'installeur figé (`bootstrap-installer/`), un par OS. C'est lui
 *              que le joueur télécharge ; le panel lui ajoute à la volée les 32
 *              octets qui désignent le tenant. Il ne contient aucune identité.
 *   engine     le moteur tauri générique, livré en artefact **portable**
 *              (`exe-zip`, `app-tar-gz`, `appimage`) : il n'est jamais installé
 *              par un installeur d'OS, c'est le bootstrap qui le pose et qui le
 *              remplace quand le SHA-256 du manifeste ne correspond plus.
 *
 * Aucun artefact n'est spécifique à un client : ni ici, ni dans les octets
 * stockés. L'identité vient du pack client, servi à part par le panel.
 *
 * Trois sous-commandes, appelées par `.github/workflows/deploy.yml` :
 *
 *   open     ouvre la release de cette version, en écrasant ce qui existait
 *   upload   téléverse les artefacts d'un couple (rôle, plateforme)
 *   publish  publie la release — c'est là que les launchers installés la voient
 *
 * **`open` écrase** : relancer le workflow sur une version déjà construite —
 * publiée ou non — supprime ses artefacts et la remet en préparation, pour que
 * la release corresponde exactement à ce que cette exécution a produit. Tant
 * que le `publish` final n'a pas eu lieu, cette version n'est plus servie.
 *
 * `upload` et `publish`, eux, sont sans effet de bord à rejouer : téléverser
 * remplace le créneau d'une plateforme sans toucher aux autres, et publier une
 * release déjà publiée ne fait rien.
 *
 * Aucune dépendance : `fetch`, `node:fs` et `node:crypto` suffisent.
 */

import { createHash } from 'node:crypto'
import { open, readFile, stat } from 'node:fs/promises'
import { basename } from 'node:path'

const BUILD_KEY_HEADER = 'X-Launcher-Build-Key'

/**
 * Formats publiables, par rôle (contrat §5).
 *
 * Cette table est un **filtre**, pas seulement une nomenclature : un build peut
 * laisser d'autres fichiers à côté de l'artefact attendu, et rien de ce qui
 * n'est pas listé ici ne doit partir vers le panel.
 *
 * Les suffixes sont testés dans l'ordre, du plus spécifique au plus général :
 * `.app.tar.gz` doit gagner sur `.tar.gz`, et `.zip` passe en dernier pour ne
 * jamais rafler un nom que les entrées précédentes reconnaissent déjà.
 */
const FORMATS = {
    bootstrap: [
        ['.exe', 'exe'],
        ['.bin', 'bin'],
    ],
    engine: [
        ['.app.tar.gz', 'app-tar-gz'],
        ['.appimage', 'appimage'],
        ['.zip', 'exe-zip'],
    ],
}

function env(name, { required = true } = {}) {
    const value = process.env[name]?.trim()
    if (!value && required) {
        fail(`la variable d'environnement ${name} est vide`)
    }
    return value ?? ''
}

function fail(message) {
    console.error(`::error::${message}`)
    process.exit(1)
}

function panelBase() {
    const raw = env('PANEL_URL').replace(/\/+$/, '')
    if (!/^https:\/\//.test(raw)) {
        fail(`PANEL_URL doit être en https (reçu : ${raw})`)
    }
    return raw
}

/**
 * Appelle une route de build du panel.
 *
 * `body` est envoyé en JSON ; `raw` envoie des octets tels quels (les parts du
 * téléversement). Le corps d'erreur est remonté tel quel : le panel renvoie un
 * code machine (`already_published`, `missing_upload`…) et le voir dans les
 * logs de la CI évite d'avoir à deviner ce qui a été refusé.
 */
async function callPanel(path, { method = 'POST', body, raw } = {}) {
    const url = `${panelBase()}${path}`
    const response = await fetch(url, {
        method,
        headers: {
            [BUILD_KEY_HEADER]: env('PANEL_BUILD_KEY'),
            Accept: 'application/json',
            ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
            ...(raw === undefined ? {} : { 'Content-Type': 'application/octet-stream' }),
        },
        body: raw !== undefined ? raw : body === undefined ? undefined : JSON.stringify(body),
    })

    const text = await response.text()
    let parsed
    try {
        parsed = text ? JSON.parse(text) : {}
    } catch {
        parsed = { raw: text }
    }

    if (!response.ok) {
        // Une exception, pas `fail()` : `process.exit` couperait court aux
        // `catch`/`finally` des appelants, et le téléversement en cours
        // resterait à l'abandon dans R2 au lieu d'être annulé.
        throw new Error(`${method} ${path} → ${response.status} ${response.statusText}: ${text || '(corps vide)'}`)
    }
    return parsed
}

/** Version du bundle, lue là où tauri la lit. */
async function readVersion() {
    const config = JSON.parse(await readFile('src-tauri/tauri.conf.json', 'utf8'))
    const version = config.version?.trim()
    if (!version) fail('src-tauri/tauri.conf.json ne porte pas de version')
    return version
}

/** Format attendu par le panel pour ce fichier, ou `null` s'il est hors périmètre. */
function formatOf(filename, role) {
    const name = filename.toLowerCase()
    for (const [suffix, format] of FORMATS[role]) {
        if (name.endsWith(suffix)) return format
    }
    return null
}

/**
 * Trie les chemins produits par le build : ce qui part, ce qui est écarté.
 *
 * Les répertoires sont écartés : sous macOS la liste peut contenir le bundle
 * `.app` lui-même, qui est une arborescence et non un fichier. Ce n'est pas un
 * oubli qu'il ne soit pas téléversé — c'est le `.app.tar.gz` voisin qui porte
 * le même contenu sous forme de fichier, et c'est lui que le bootstrap déballe
 * pour fabriquer le `.app` du tenant sur la machine du joueur.
 */
async function collectArtifacts(paths, role) {
    const artifacts = []

    for (const path of paths) {
        const stats = await stat(path).catch(() => null)
        if (!stats) {
            console.warn(`::warning::chemin introuvable, ignoré : ${basename(path)}`)
            continue
        }
        if (!stats.isFile()) {
            console.log(`  – ${basename(path)} ignoré (répertoire)`)
            continue
        }
        const format = formatOf(basename(path), role)
        if (!format) {
            console.log(`  – ${basename(path)} ignoré (hors périmètre ${role})`)
            continue
        }
        artifacts.push({ path, format })
    }

    return artifacts
}

async function commandOpen() {
    const version = await readVersion()
    const result = await callPanel('/api/launcher/build/release', {
        body: {
            version,
            channel: env('RELEASE_CHANNEL', { required: false }) || 'stable',
            notes: env('RELEASE_NOTES', { required: false }) || null,
            commitSha: env('GITHUB_SHA', { required: false }) || null,
            runUrl:
                env('GITHUB_SERVER_URL', { required: false }) && env('GITHUB_REPOSITORY', { required: false })
                    ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`
                    : null,
        },
    })
    if (result.created) {
        console.log(`release ${version} créée`)
        return version
    }

    // Relancer un build écrase la version : le panel a supprimé les artefacts
    // de l'exécution précédente et repassé la release en préparation. C'est
    // voulu, mais ça mérite d'être lisible dans les logs — surtout quand la
    // version était publiée, puisqu'elle disparaît alors des téléchargements et
    // des mises à jour jusqu'au `publish` de fin de workflow.
    if (result.wasPublished) {
        console.warn(
            `::warning::la version ${version} était publiée : elle est retirée des téléchargements` +
                ` le temps du build, et ses ${result.replaced} artefact(s) ont été supprimés`,
        )
    } else if (result.replaced > 0) {
        console.log(`release ${version} rouverte, ${result.replaced} artefact(s) précédent(s) supprimé(s)`)
    } else {
        console.log(`release ${version} rouverte`)
    }
    return version
}

/**
 * Remplit `buffer` autant que le fichier le permet.
 *
 * R2 exige des parts de taille **égale**, sauf la dernière. Une lecture courte
 * en milieu de fichier — que `read()` a le droit de renvoyer — produirait une
 * part intermédiaire plus petite et ferait échouer l'assemblage ; boucler
 * jusqu'à remplissage l'évite. Renvoie le nombre d'octets lus (0 en fin de
 * fichier).
 */
async function readFull(handle, buffer) {
    let filled = 0
    while (filled < buffer.byteLength) {
        const { bytesRead } = await handle.read(buffer, filled, buffer.byteLength - filled, null)
        if (bytesRead === 0) break
        filled += bytesRead
    }
    return filled
}

/**
 * Téléverse un artefact : enregistrement, parts, assemblage.
 *
 * Les octets passent par la **liaison R2 du Worker**, en multipart. Deux
 * raisons plutôt qu'un envoi direct par URL présignée S3 :
 *
 * - aucun identifiant S3 à configurer, et surtout aucun risque d'écrire dans
 *   un bucket que le panel ne relit pas — c'est la même liaison qui assemble
 *   et vérifie ;
 * - le découpage en parts enlève tout plafond : un AppImage tauri embarque
 *   webkit2gtk et dépasserait la limite de corps de requête d'un Worker.
 *
 * Le fichier est lu part par part, jamais entièrement en mémoire, et son
 * empreinte SHA-256 est calculée au passage — d'où son envoi à l'assemblage
 * plutôt qu'à l'enregistrement, ce qui évite une seconde lecture complète.
 */
async function uploadOne(version, artifact, { role, target, arch }) {
    const stats = await stat(artifact.path)
    const filename = basename(artifact.path)
    if (stats.size === 0) fail(`artefact vide : ${filename}`)

    const registered = await callPanel(`/api/launcher/build/release/${encodeURIComponent(version)}/artifact`, {
        body: { filename, target, arch, size: stats.size },
    })

    // Le panel déduit rôle et format du nom de fichier : s'il ne range pas
    // l'artefact là où la CI croit l'envoyer, mieux vaut s'en apercevoir ici
    // que devant une release publiée avec un moteur dans le créneau des
    // bootstraps.
    if (registered.format !== artifact.format || (registered.role && registered.role !== role)) {
        fail(
            `${filename} : le panel l'a rangé en ${registered.role ?? '?'}/${registered.format ?? '?'},` +
                ` attendu ${role}/${artifact.format}`,
        )
    }

    const { uploadId, partSize } = await callPanel(`/api/launcher/build/artifact/${registered.artifactId}/upload`)

    const handle = await open(artifact.path, 'r')
    const digest = createHash('sha256')
    const parts = []
    const totalParts = Math.max(1, Math.ceil(stats.size / partSize))
    try {
        const buffer = Buffer.allocUnsafe(partSize)
        for (let partNumber = 1; ; partNumber++) {
            const filled = await readFull(handle, buffer)
            if (filled === 0) break
            const chunk = buffer.subarray(0, filled)
            digest.update(chunk)
            const { part } = await callPanel(
                `/api/launcher/build/artifact/${registered.artifactId}/part` +
                    `?uploadId=${encodeURIComponent(uploadId)}&partNumber=${partNumber}`,
                { method: 'PUT', raw: chunk },
            )
            parts.push(part)
            if (totalParts > 1) console.log(`    part ${partNumber}/${totalParts}`)
            if (filled < partSize) break
        }
    } catch (error) {
        // Sans abandon explicite, les parts déjà envoyées resteraient dans R2,
        // facturées et invisibles.
        await callPanel(`/api/launcher/build/artifact/${registered.artifactId}/abort`, { body: { uploadId } }).catch(
            () => undefined,
        )
        throw error
    } finally {
        await handle.close()
    }

    await callPanel(`/api/launcher/build/artifact/${registered.artifactId}/complete`, {
        body: { uploadId, parts, sha256: digest.digest('hex') },
    })

    console.log(`  ✓ ${filename} — ${role}/${artifact.format}, ${(stats.size / 1024 / 1024).toFixed(1)} Mio`)
}

async function commandUpload() {
    const version = await readVersion()
    const role = env('ARTIFACT_ROLE')
    if (!Object.hasOwn(FORMATS, role)) {
        fail(`rôle inconnu : ${role} — attendu ${Object.keys(FORMATS).join(', ')}`)
    }
    const target = env('ARTIFACT_TARGET')
    const arch = env('ARTIFACT_ARCH')

    let paths
    try {
        paths = JSON.parse(env('ARTIFACT_PATHS'))
    } catch (error) {
        fail(`ARTIFACT_PATHS n'est pas un tableau JSON : ${error.message}`)
    }
    if (!Array.isArray(paths) || paths.length === 0) {
        fail('aucun artefact produit par le build')
    }

    const artifacts = await collectArtifacts(paths, role)
    if (artifacts.length === 0) fail(`aucun artefact ${role} pour ${target}/${arch}`)

    console.log(`${artifacts.length} artefact(s) ${role} pour ${target}/${arch} :`)

    // En série, volontairement : plusieurs centaines de Mio en parallèle sur un
    // runner GitHub ne gagnent rien et rendent les échecs illisibles.
    for (const artifact of artifacts) {
        await uploadOne(version, artifact, { role, target, arch })
    }
}

async function commandPublish() {
    const version = await readVersion()
    await callPanel(`/api/launcher/build/release/${encodeURIComponent(version)}/publish`)
    console.log(`release ${version} publiée : les launchers installés la verront au prochain démarrage`)
}

const COMMANDS = { open: commandOpen, upload: commandUpload, publish: commandPublish }

const command = process.argv[2]
if (!Object.hasOwn(COMMANDS, command)) {
    fail(`sous-commande inconnue : ${command ?? '(aucune)'} — attendu ${Object.keys(COMMANDS).join(', ')}`)
}

// Les erreurs remontent jusqu'ici pour que les nettoyages des appelants aient
// tourné avant l'arrêt du processus.
try {
    await COMMANDS[command]()
} catch (error) {
    fail(error instanceof Error ? error.message : String(error))
}
