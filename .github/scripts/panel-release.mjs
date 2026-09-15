#!/usr/bin/env node
/**
 * Publication d'une version du launcher vers le panel LuuxCraft.
 *
 * Un seul launcher est compilé pour tous les clients : la CI publie donc UNE
 * release globale, et c'est le panel qui ajoute la configuration de chaque
 * client à l'installeur au moment du téléchargement. Rien ici n'est spécifique
 * à un client.
 *
 * Trois sous-commandes, appelées par `.github/workflows/deploy.yml` :
 *
 *   open     ouvre (ou retrouve) la release en préparation pour cette version
 *   upload   téléverse les artefacts d'une plateforme et leurs signatures
 *   publish  publie la release — c'est là que les launchers installés la voient
 *
 * `open` et `publish` sont idempotents : la matrice de build lance un job par
 * plateforme, sans ordre garanti, et un job rejoué ne doit rien casser.
 *
 * Aucune dépendance : `fetch`, `node:fs` et `node:crypto` suffisent.
 */

import { createHash } from 'node:crypto'
import { open, readFile, stat } from 'node:fs/promises'
import { basename } from 'node:path'

const BUILD_KEY_HEADER = 'X-Launcher-Build-Key'

/** Les `.sig` accompagnent un artefact, ils n'en sont pas un. */
const SIGNATURE_SUFFIX = '.sig'

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
 * code machine (`already_published`, `missing_signature`…) et le voir dans les
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

/**
 * Sépare les artefacts de leurs signatures.
 *
 * `tauri-action` renvoie un tableau à plat où les `.sig` côtoient les bundles ;
 * le panel, lui, attend la signature *avec* l'artefact qu'elle signe, parce que
 * c'est ce couple qui rend une entrée de mise à jour valide.
 *
 * Les répertoires sont écartés : sous macOS la liste contient le bundle
 * `.app` lui-même, qui est une arborescence et non un fichier. Ce n'est pas un
 * oubli qu'il ne soit pas téléversé — c'est le `.app.tar.gz` voisin qui porte
 * le même contenu sous forme de fichier, et c'est lui que l'updater et le zip
 * macOS du panel utilisent.
 */
async function pairArtifacts(paths) {
    const signatures = new Map()
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
        if (path.endsWith(SIGNATURE_SUFFIX)) {
            signatures.set(path.slice(0, -SIGNATURE_SUFFIX.length), path)
        } else {
            artifacts.push(path)
        }
    }

    const orphans = [...signatures.keys()].filter((base) => !artifacts.includes(base))
    for (const orphan of orphans) {
        console.warn(`::warning::signature sans artefact, ignorée : ${basename(orphan)}${SIGNATURE_SUFFIX}`)
    }

    return artifacts.map((path) => ({ path, signaturePath: signatures.get(path) ?? null }))
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
    console.log(`release ${version} ouverte (${result.created ? 'créée' : 'déjà ouverte'})`)
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
 * - le découpage en parts enlève tout plafond : un AppImage Tauri embarque
 *   webkit2gtk et dépasserait la limite de corps de requête d'un Worker.
 *
 * Le fichier est lu part par part, jamais entièrement en mémoire, et son
 * empreinte SHA-256 est calculée au passage — d'où son envoi à l'assemblage
 * plutôt qu'à l'enregistrement, ce qui évite une seconde lecture complète.
 */
async function uploadOne(version, { path, signaturePath }, { target, arch }) {
    const stats = await stat(path)
    const filename = basename(path)
    if (stats.size === 0) fail(`artefact vide : ${filename}`)
    const signature = signaturePath ? (await readFile(signaturePath, 'utf8')).trim() : null

    const { artifactId, format, role } = await callPanel(
        `/api/launcher/build/release/${encodeURIComponent(version)}/artifact`,
        { body: { filename, target, arch, size: stats.size, signature } },
    )

    const { uploadId, partSize } = await callPanel(`/api/launcher/build/artifact/${artifactId}/upload`)

    const handle = await open(path, 'r')
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
                `/api/launcher/build/artifact/${artifactId}/part` +
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
        await callPanel(`/api/launcher/build/artifact/${artifactId}/abort`, { body: { uploadId } }).catch(
            () => undefined,
        )
        throw error
    } finally {
        await handle.close()
    }

    await callPanel(`/api/launcher/build/artifact/${artifactId}/complete`, {
        body: { uploadId, parts, sha256: digest.digest('hex') },
    })

    const signed = signature ? 'signé' : 'non signé'
    console.log(`  ✓ ${filename} — ${format}/${role}, ${(stats.size / 1024 / 1024).toFixed(1)} Mio, ${signed}`)
}

async function commandUpload() {
    const version = await readVersion()
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

    const pairs = await pairArtifacts(paths)
    if (pairs.length === 0) fail(`aucun fichier téléversable pour ${target}/${arch}`)
    console.log(`${pairs.length} artefact(s) pour ${target}/${arch} :`)

    // En série, volontairement : plusieurs centaines de Mio en parallèle sur un
    // runner GitHub ne gagnent rien et rendent les échecs illisibles.
    for (const pair of pairs) {
        await uploadOne(version, pair, { target, arch })
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
