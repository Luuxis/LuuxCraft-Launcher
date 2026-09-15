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
import { readFile } from 'node:fs/promises'
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
 * Le corps d'erreur est remonté tel quel : le panel renvoie un code machine
 * (`already_published`, `missing_signature`…) et le voir dans les logs de la
 * CI évite d'avoir à deviner ce qui a été refusé.
 */
async function callPanel(path, { method = 'POST', body } = {}) {
    const url = `${panelBase()}${path}`
    const response = await fetch(url, {
        method,
        headers: {
            [BUILD_KEY_HEADER]: env('PANEL_BUILD_KEY'),
            Accept: 'application/json',
            ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
    })

    const text = await response.text()
    let parsed
    try {
        parsed = text ? JSON.parse(text) : {}
    } catch {
        parsed = { raw: text }
    }

    if (!response.ok) {
        fail(`${method} ${path} → ${response.status} ${response.statusText}: ${text || '(corps vide)'}`)
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
 */
function pairArtifacts(paths) {
    const signatures = new Map()
    const artifacts = []

    for (const path of paths) {
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
 * Téléverse un artefact : enregistrement, octets, puis confirmation.
 *
 * Les octets vont **directement dans R2** par URL présignée quand le panel en
 * fournit une. C'est ce qui permet de téléverser une AppImage de 100 Mo sans se
 * heurter à la limite de corps de requête d'un Worker Cloudflare ; la route
 * relayée n'est qu'un repli pour un panel sans identifiants S3.
 *
 * La confirmation est obligatoire : elle fait constater au panel la taille
 * réellement stockée, et c'est ce qu'il servira en `Content-Length`.
 */
async function uploadOne(version, { path, signaturePath }, { target, arch }) {
    const bytes = await readFile(path)
    const filename = basename(path)
    const signature = signaturePath ? (await readFile(signaturePath, 'utf8')).trim() : null

    const registered = await callPanel(`/api/launcher/build/release/${encodeURIComponent(version)}/artifact`, {
        body: {
            filename,
            target,
            arch,
            size: bytes.byteLength,
            sha256: createHash('sha256').update(bytes).digest('hex'),
            signature,
        },
    })

    const { artifactId, uploadUrl, uploadFallbackUrl, format, role } = registered

    if (uploadUrl) {
        const put = await fetch(uploadUrl, {
            method: 'PUT',
            headers: { 'Content-Length': String(bytes.byteLength) },
            body: bytes,
        })
        if (!put.ok) {
            fail(`téléversement R2 de ${filename} → ${put.status} ${put.statusText}: ${await put.text()}`)
        }
        await callPanel(`/api/launcher/build/artifact/${artifactId}/confirm`)
    } else {
        console.log(`::notice::pas d'identifiants S3 sur le panel, ${filename} passe par le Worker`)
        const put = await fetch(uploadFallbackUrl, {
            method: 'PUT',
            headers: {
                [BUILD_KEY_HEADER]: env('PANEL_BUILD_KEY'),
                'Content-Type': 'application/octet-stream',
                'Content-Length': String(bytes.byteLength),
            },
            body: bytes,
        })
        if (!put.ok) {
            fail(`téléversement relayé de ${filename} → ${put.status} ${put.statusText}: ${await put.text()}`)
        }
    }

    const signed = signature ? 'signé' : 'non signé'
    console.log(`  ✓ ${filename} — ${format}/${role}, ${(bytes.byteLength / 1024 / 1024).toFixed(1)} Mio, ${signed}`)
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

    const pairs = pairArtifacts(paths)
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

await COMMANDS[command]()
