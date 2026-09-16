# LuuxCraft Launcher

Launcher Minecraft de LuuxCraft, construit sur [Tauri 2](https://v2.tauri.app), React et
[`crust_core` 1.0.3](https://docs.rs/crust_core/1.0.3). L'interface reprend le design system
« LuuxCraft Forge » du panel (voir `CHARTE-GRAPHIQUE.md`).

Rien n'est compilé par client : un dépôt, une chaîne de build, et autant de serveurs que le
panel en héberge.

## Trois couches

La distribution repose sur trois objets indépendants, produits et servis séparément.

| Couche | Ce que c'est | Qui la produit | Ce qu'elle sait du tenant |
|---|---|---|---|
| **Bootstrap** | `bootstrap-installer/`, un petit binaire Rust sans interface ni webview | la CI, un par OS | 16 octets d'UUID ajoutés au téléchargement |
| **Moteur** | ce dépôt : l'application tauri, générique | la CI, un paquet de mise à jour par plateforme | rien du tout |
| **Pack client** | `client_config.json`, `icon.png`, `icon.ico`, `icon.icns` | le panel, à la volée | tout |

Le joueur ne télécharge **que le bootstrap**. Les octets stockés dans le R2 du panel sont les
mêmes pour tout le monde : le panel ajoute au bootstrap, pendant le streaming, un **overlay de
32 octets** en fin de fichier :

```
[ LXCBOOT1 ][ 16 octets d'UUID ][ 1TOOBCXL ]
```

C'est tout ce qui distingue le téléchargement d'un serveur de celui d'un autre. Le bootstrap
relit ces 32 octets dans `std::env::current_exe()`, ce qui résiste au renommage du fichier,
puis demande au panel le manifeste du tenant :

```
GET {api_base}/api/v1/launchers/{tenant_id}/manifest
```

Le manifeste décrit la version du moteur à installer et les fichiers du pack client, chacun
avec son `sha256`. Le bootstrap installe ce qui manque, remplace ce dont l'empreinte a changé,
et lance le moteur. C'est lui, et lui seul, qui crée les raccourcis, l'entrée de bureau XDG ou
le `.app` macOS — **sur la machine du joueur**, avec le nom et l'icône du serveur. Aucun
repackaging côté serveur, donc aucun octet à re-signer.

Le moteur, lui, ne connaît son tenant qu'en lisant `client_config.json` dans le pack client
posé à côté de lui. Aucune identité n'est compilée, et rien n'est demandé au joueur : un
moteur sans pack client est une erreur explicite, pas un écran d'appairage.

### Où ça s'installe

| OS | Racine installée | Données d'exécution |
|---|---|---|
| Windows | `%LOCALAPPDATA%\Programs\<slug>\` | `%APPDATA%\.<slug>` |
| Linux | `~/.local/share/<slug>/` | `~/.<slug>` |
| macOS | `~/Applications/<nom du serveur>.app/` | `~/.<slug>` |

Le `slug` vient du panel (`launcher_configs.slug`), jamais d'un calcul côté client : c'est lui
qui sépare deux serveurs installés sur la même machine. La racine contient `engine/` (le
moteur), `client/` (le pack client), `.engine-version` et `.pack-version`.

La racine du **jeu** reste partagée entre serveurs, volontairement : elle pèse des gigaoctets
et deux serveurs y réutilisent les mêmes versions, bibliothèques et assets. Les comptes, les
réglages, les caches et le profil webview sont en revanche scopés au tenant — sans quoi deux
launchers ne pourraient pas tourner en même temps.

### Mise à jour différentielle

- **Pack client modifié** → seuls les fichiers dont le `sha256` diffère sont réécrits ; le
  moteur n'est pas retéléchargé.
- **Moteur modifié** → seul le bundle du moteur est remplacé ; `client/` et les dossiers de
  jeu sont intacts.
- **Modpack** → inchangé, déjà différentiel par hash.

## Configuration

**Tout ce qui est configurable vient du panel**, à chaque démarrage : maintenance, mode de
connexion, `client_id` Azure, dossier du jeu, liens sociaux, modules actifs, instances et
actualités. Rien n'est dupliqué dans le dépôt.

Les deux choses que l'API ne peut pas s'auto-annoncer — son adresse et le tenant — viennent du
pack client (`client_config.json`), écrit par le bootstrap à l'installation :

```json
{
  "schema": 1,
  "tenant_id": "…",
  "slug": "mon-serveur",
  "display_name": "Mon Serveur",
  "api_base_url": "https://panel.example/api",
  "window": { "title": "Mon Serveur", "icon": "icon.png" },
  "updated_at": "2026-09-16T10:00:00.000Z"
}
```

Aucune variable de build ne les remplace : il n'y a plus de `LUUXCRAFT_API_URL` ni de
`LUUXCRAFT_USER_ID`. Seul le **bootstrap** porte une adresse de panel compilée, parce qu'il
doit bien commencer quelque part.

`src-tauri/src/config.rs` garde les constantes qu'aucune API ne fournit : `YGGDRASIL_SERVER`
(optionnel, le serveur legacy de Mojang étant arrêté) et les valeurs par défaut des réglages
(mémoire, fenêtre, téléchargements, statut serveur, actualités) appliquées tant que
l'utilisateur n'a rien changé. Les points de mise à jour sont dérivés de l'adresse du panel du
pack client, pas écrits en dur : un launcher installé depuis un autre panel prend ses mises à
jour de ce panel-là.

Le titre de la fenêtre et l'icône sont appliqués depuis le pack **local**, avant le premier
rendu : rien n'est téléchargé pour brander la fenêtre, et le nom générique ne clignote pas au
démarrage.

## Données du panel

Routes consommées (mêmes routes que les launchers de référence LuuxCraft) :

- `GET {baseUrl}/user/{userId}/config` — tout ce qui pilote le launcher :

  | Champ | Rôle | Sans lui |
  |---|---|---|
  | `maintenance`, `maintenance_message` | Bandeau de maintenance, blocage du lancement | pas de maintenance |
  | `online` | `true` = Microsoft, `false` = hors ligne, URL = site Azuriom/AZauth | Microsoft |
  | `client_id` | Application Azure utilisée pour la connexion Microsoft | identifiant du launcher officiel |
  | `dataDirectory` | Dossier du jeu sous le dossier de données de l'OS | `luuxcraft` |
  | `socialLinks` | Liens affichés sur l'accueil | aucun lien |
  | `modules` (ou `features`) | Bascules par module : `news`, `skins`, `links`, `serverStatus`, `accounts`, `settings` | tout est affiché |
  | `brand` | Nom, wordmark (`prefix` + `suffix` en dégradé), sous-titre, site | identité intégrée (`src/config/brand.ts`) |
  | `updater.endpoints` | Surcharge les points de mise à jour (https uniquement) | point dérivé de l'adresse du panel |
  | `yggdrasil` | Serveur Yggdrasil/authlib-injector, ajoute l'onglet de connexion | méthode non proposée |

  Tout champ inconnu est conservé dans `extra`, rien ne casse s'il en manque un.
- `GET {baseUrl}/user/{userId}/articles?limit=N` — actualités (`title`, `content` HTML,
  `author`, `publish_date`, image/lien/ordre si présents).
- `GET {baseUrl}/user/{userId}/instances` — instances (`name`, `url` des fichiers, `loader`,
  `verify`, `ignored`, `whitelist`, `whitelistActive`, `status{nameServer, ip, port}`,
  et champs optionnels `description`, `image`, `java`, `jvm_args`, `memory`, `order`).

Le `userId` de ces routes est le **tenant** du pack client. Les modèles
(`src-tauri/src/api/models.rs`) acceptent plusieurs orthographes de clés et ignorent ce qu'ils
ne connaissent pas : le panel peut ajouter, renommer ou retirer des champs sans casser le
launcher. Le dernier instantané est mis en cache sur disque pour le mode hors ligne.

## Architecture

```
bootstrap-installer/          l'installeur figé : overlay, manifeste, installation, raccourcis
src-tauri/src/
  client_config.rs          lecture du pack client local (tenant, panel, nom, icône)
  config.rs                 défauts intégrés, chemins scopés au tenant
  desktop.rs                entrée de bureau et icône XDG (Linux)
  api/                      client HTTP du panel + modèles tolérants + commandes remote_*
  accounts/                 multi-comptes dans `<dossier du tenant>/accounts.json`
  auth.rs                   Microsoft (fenêtre de connexion), Azuriom/AZauth (+ OTP), hors ligne, Yggdrasil
  sessions.rs               renouvellement automatique des sessions en tâche de fond
  skins.rs                  textures skin/cape → data URL (cache disque)
  java.rs                   détection des Java installés, sonde `-XshowSettings`, version requise
  instances.rs / game.rs    installation, vérification, lancement (crust_core), processus, logs
  settings.rs               paramètres persistants (fusion + bornes)
  status.rs                 ping serveur (crust_core::network::Status)
  updater.rs                tauri-plugin-updater (signatures vérifiées)
  system.rs / logging.rs    infos machine, ouverture de dossiers, journalisation
src/
  store/AppStore.tsx        état global + actions (IPC typé dans lib/ipc.ts)
  features/*                accueil (jouer + instance), comptes, skins (skinview3d/three), paramètres
  components/*              design system (charte) : boutons, cartes, modales, formulaires…
  styles/index.css          tokens Tailwind v4 + composants de la charte, thème clair
  i18n/fr.ts                textes UI et messages d'erreur par code
```

## Développement

```bash
npm install
npm run tauri dev
```

En debug, tout va dans `data/` à la racine du dépôt (`data/launcher`, `data/minecraft`,
`data/logs`, `data/cache`) ; en release, dans les dossiers standards de l'OS, scopés au tenant.

Tests :

```bash
npm test                                                        # vitest (frontend)
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored     # API réelle, Java local, Microsoft
cargo test --manifest-path bootstrap-installer/Cargo.toml
```

## Comptes et connexion Microsoft

Les comptes (jetons compris) sont dans **`<dossier du tenant>/accounts.json`**, au format de
`minecraft-java-core`, fichier lisible par l'utilisateur seul. Le fichier est versionné
(`"version": 1`) pour accueillir un chiffrement ultérieur.

Ils appartiennent au **tenant**, pas à l'installation du jeu : deux serveurs peuvent partager
une racine de jeu — c'est même souhaitable, elle pèse des gigaoctets — mais sûrement pas les
sessions Minecraft du joueur.

La connexion Microsoft utilise le flux *authorization code* de `crust_core` dans une **fenêtre
de connexion** dédiée, avec le `client_id` publié par le panel sur `login.live.com` (ou
l'identifiant du launcher officiel si le panel n'en publie pas). La fenêtre est en mode privé
pour pouvoir ajouter plusieurs comptes ; la fermer annule la connexion.

Le flux *device code* n'est pas proposé : l'application Azure du panel le refuse
(`AADSTS70002`, « public client flows » désactivés).

### Renouvellement automatique

`src-tauri/src/sessions.rs` maintient les sessions valides pendant que le launcher tourne :

- une passe au démarrage pour chaque compte en ligne (profil, pseudo, skin et cape à jour) ;
- ensuite un renouvellement dès que le jeton Minecraft expire dans moins de 30 minutes
  (vérification toutes les 5 minutes, une tentative par compte au maximum toutes les 15 minutes) ;
- les sessions Azuriom sont revérifiées toutes les 6 heures ;
- un renouvellement a également lieu juste avant chaque lancement du jeu.

Les renouvellements sont sérialisés (Microsoft fait tourner les *refresh tokens*) et la liste des
comptes est repoussée à l'interface par l'événement `accounts://changed`. Si le panel est
injoignable, la session stockée est conservée telle quelle au lieu d'être déclarée expirée ; un
compte réellement expiré est marqué « reconnexion requise ».

## Mises à jour automatiques

Le moteur se met à jour par `tauri-plugin-updater`. La clé publique est dans
`src-tauri/tauri.conf.json` (`plugins.updater.pubkey`) ; la clé privée correspondante est
**hors dépôt** : `~/.tauri/luuxcraft-launcher.key` (générée avec `npx tauri signer generate`,
sans mot de passe). Ne la perdez pas : sans elle, aucune mise à jour ne pourra être signée pour
les launchers déjà installés.

Le point de mise à jour n'est pas un manifeste statique mais le **serveur dynamique du panel** :

```
{baseUrl}/launcher/update/{{target}}/{{arch}}/{{current_version}}
```

Le panel répond un `204` quand il n'y a rien à installer, et sinon le JSON attendu par le
plugin (`version`, `notes`, `pub_date`, `url`, `signature`). Un panel peut surcharger la liste
par `updater.endpoints` dans `/config` : ce qu'il publie gagne sur la valeur dérivée.

Les paquets de mise à jour sont servis **intacts** : la signature minisign porte sur ces octets
exacts, un seul octet en plus la casse. C'est l'autre raison pour laquelle l'identité du
serveur vit dans le pack client et non dans le binaire — une mise à jour ne peut rien lui
faire perdre.

Comme le moteur est unique, les mises à jour sont globales : une release, une chaîne de mises à
jour, tous les serveurs. Le pack client, lui, se met à jour de son côté, au démarrage, fichier
par fichier.

## Publication

`.github/workflows/deploy.yml` publie **deux lignes de produit** vers le panel, via
`.github/scripts/panel-release.mjs` :

| Rôle | Ce qui est construit | Formats | Signature minisign |
|---|---|---|---|
| `bootstrap` | `cargo build --release` dans `bootstrap-installer/` | `exe` (windows), `bin` (linux, darwin) | non |
| `engine` | `tauri build`, paquets de mise à jour uniquement | `nsis-zip` (windows), `app-tar-gz` (darwin), `appimage` (linux) | oui |

Il n'y a **ni msi, ni dmg, ni deb, ni rpm** : plus rien n'installe le moteur par le
gestionnaire de paquets de l'OS, c'est le bootstrap qui s'en charge. L'installeur NSIS que
`tauri build` produit au passage reste sur le runner — le script de téléversement n'accepte que
les formats ci-dessus et écarte le reste en le disant dans les logs.

Le bootstrap macOS est un binaire **universel** (`lipo` de `x86_64` + `aarch64`) : la route de
téléchargement du panel ne connaît que l'OS, jamais l'architecture de la machine du visiteur.
Le moteur, lui, est publié par architecture, parce que l'updater substitue `{{arch}}`.

À configurer une fois sur le dépôt :

| Nom | Type | Rôle |
|---|---|---|
| `PANEL_URL` | variable | `https://luuxcraft.fr` |
| `PANEL_BUILD_KEY` | secret | Admin → Configuration → Builds du launcher |
| `TAURI_SIGNING_PRIVATE_KEY` | secret | contenu de `~/.tauri/luuxcraft-launcher.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | secret | vide si la clé n'en a pas |

C'est toute la configuration : **aucun identifiant de stockage à fournir**. Les octets sont
téléversés en multipart vers le panel, qui les écrit par sa propre liaison R2 — la même qui les
relit ensuite pour vérifier. Un artefact accepté est donc forcément un artefact visible, là où
des identifiants S3 mal réglés pourraient écrire dans un bucket que le panel ne lit pas. Le
découpage en parts enlève par ailleurs toute limite de taille (un AppImage embarque webkit2gtk
et dépasserait la limite de corps de requête d'un Worker en un seul envoi).

Le workflow part sur un tag `v*` ou à la main (canal `stable`/`beta`, notes de version, et la
possibilité de laisser la release en préparation). La version publiée est celle de
`src-tauri/tauri.conf.json`, pas celle du tag. La publication est **tout ou rien**, à deux
niveaux : le job de publication dépend des jobs de build, donc une plateforme en échec ne
publie rien ; et le panel refuse en plus un artefact dont les octets manquent ou un paquet de
mise à jour sans signature. La CI, elle, s'arrête avant même de téléverser un paquet moteur
sans `.sig` — inutile d'envoyer des centaines de mégaoctets pour se faire refuser à la fin.

Le bootstrap est refait à chaque release même quand son code n'a pas bougé : une release est un
lot complet, et c'est le lot publié que le panel sert. Changer le bootstrap est en revanche un
acte rare et lourd — il est déjà installé chez les joueurs et ne se met pas à jour tout seul,
seul le moteur le fait.

**Relancer un build écrase la version.** Le job `open-release` supprime les artefacts de
l'exécution précédente — octets R2 compris — et remet la release en préparation, même si elle
était publiée : la release correspond ainsi exactement à ce que cette exécution a produit, sans
qu'un artefact orphelin survive dans un créneau que la nouvelle matrice ne remplit plus. Le prix
à connaître : **entre le début du build et le `publish` final, cette version n'est plus
servie**. Les téléchargements et les launchers installés retombent sur la release publiée
précédente, ou n'ont plus rien si c'était la seule. La CI l'annonce par un `::warning::` quand
la version écrasée était publiée. Pour ne rien interrompre, incrémenter la version dans
`src-tauri/tauri.conf.json` plutôt que reconstruire la même.

Build signé en local, pour vérifier avant de pousser :

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/luuxcraft-launcher.key)"   # contenu de la clé
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run tauri build -- --bundles nsis          # ou app / appimage selon la plateforme
cargo build --release --manifest-path bootstrap-installer/Cargo.toml
```

## Sécurité

- Jetons dans `accounts.json` (permissions utilisateur seul) ; chiffrement au repos prévu.
- Aucun jeton, mot de passe ni code OTP dans les logs ; la sortie de Minecraft est expurgée
  des secrets de session.
- CSP stricte (`src-tauri/tauri.conf.json`), HTML des actualités filtré par liste blanche,
  liens ouverts uniquement en `http(s)` via le navigateur.
- Fichiers d'instance vérifiés par taille et SHA-1 (crust_core), mises à jour signées.
- Tout ce que le bootstrap télécharge est vérifié par SHA-256 avant d'être mis en place, et
  écrit sous un nom temporaire puis renommé : un moteur à moitié téléchargé n'est jamais lancé.
- L'overlay n'est pas signé et n'a pas à l'être : il ne contient qu'un identifiant public, et
  qui peut le réécrire peut de toute façon réécrire l'exécutable entier.

## Logs

`launcher.log` (rotation 5 × 5 Mo) dans le dossier de logs de l'OS (ou `data/logs` en debug),
sortie de Minecraft dans `logs/game/<instance>/latest.log`. Accès direct depuis
Paramètres → Logs & diagnostic.
