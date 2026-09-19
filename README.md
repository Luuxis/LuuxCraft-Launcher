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
| **Moteur** | ce dépôt : l'application tauri, générique | la CI, un artefact portable par plateforme | rien du tout |
| **Installeur** | `bootstrap/` (Windows) et le script `sh` (Linux, macOS) | la CI pour le premier, le panel pour le second | l'identifiant, et rien d'autre |
| **Pack client** | `client_config.json`, `icon.png`, `icon.ico`, `icon.icns` | le panel, à la volée | tout |

Le joueur ne télécharge pas le moteur : il télécharge un **installeur**, qui va chercher le
moteur et pose le pack client à côté.

```
Windows        GET  {panel}/api/v1/launchers/{tenant_id}/bootstrap?os=windows
Linux, macOS   curl -fsSL {panel}/api/v1/launchers/{tenant_id}/install.sh | sh
```

Les deux font le même travail — manifeste, moteur, pack client, intégration au bureau,
lancement — et aucun des deux ne revient ensuite : les raccourcis pointent sur le moteur, qui
se met à jour lui-même.

Le bootstrap Windows est le **seul** fichier de toute cette chaîne dont les octets dépendent du
serveur : le panel remplit son créneau d'identité de 87 octets une fois par client, puis sert le
même fichier à tous ses joueurs. Voir `bootstrap/README.md` — notamment pourquoi ce créneau est
au milieu des données et non en fin de fichier, ce qui laisse la porte ouverte à la signature
Authenticode.

Le moteur, lui, ne connaît son tenant qu'en lisant `client_config.json` dans le pack client posé
à côté de lui. Aucune identité n'est compilée, et rien n'est demandé au joueur : un moteur sans
pack client est une erreur explicite, pas un écran d'appairage.

Le manifeste du tenant décrit ce qu'une installation doit contenir — la version du moteur et les
fichiers du pack, chacun avec son `sha256`. C'est la seule source des trois lecteurs : le
bootstrap, le script, et le moteur quand il se met à jour.

```
GET {api_base}/api/v1/launchers/{tenant_id}/manifest?os=…&arch=…
GET {api_base}/api/v1/launchers/{tenant_id}/manifest?os=…&arch=…&format=text   (pour le script sh)
```

### Où ça s'installe

| OS | Racine installée | Pack client | Données d'exécution |
|---|---|---|---|
| Windows | `%LOCALAPPDATA%\Programs\<slug>\` | `<racine>\client\` | `%APPDATA%\.<slug>` |
| Linux | `$XDG_DATA_HOME/<slug>/` | `<racine>/client/` | `~/.<slug>` |
| macOS | `~/Applications/<nom du serveur>.app` | `~/Library/Application Support/luuxcraft/installs/<clé>/` | `~/.<slug>` |

Aucune de ces racines n'exige de droits administrateur, et aucune n'est partagée entre serveurs.

Le `slug` vient du panel (`launcher_configs.slug`), jamais d'un calcul côté client : c'est lui
qui sépare deux serveurs installés sur la même machine. Sous Windows et Linux, la racine contient
`engine/` (le moteur), `client/` (le pack client), `.engine-version` et `.pack-version`.

**macOS est l'exception, et c'est délibéré.** Un `.app` est remplacé en entier quand le moteur se
met à jour : un pack posé dedans disparaîtrait avec lui, et y écrire casserait de toute façon la
signature du bundle. Le pack vit donc à côté, dans un dossier dont le nom est l'empreinte du
chemin du bundle — une clé déterministe, que le script d'installation et le moteur calculent
chacun de leur côté, ce qui évite un index partagé. Le bundle publié n'est jamais modifié : il
est seulement **renommé** au nom du serveur, ce qui ne touche aucun de ses octets.

La racine du **jeu** reste partagée entre serveurs, volontairement : elle pèse des gigaoctets
et deux serveurs y réutilisent les mêmes versions, bibliothèques et assets. Les comptes, les
réglages, les caches et le profil webview sont en revanche scopés au tenant — sans quoi deux
launchers ne pourraient pas tourner en même temps.

### Mise à jour différentielle

- **Pack client modifié** → seuls les fichiers dont le `sha256` diffère sont réécrits ; le
  moteur n'est pas retéléchargé. Un changement de nom ou de logo coûte donc quelques
  kilo-octets, pas cinquante méga-octets.
- **Moteur modifié** → seul l'exécutable du moteur est remplacé ; `client/` et les dossiers de
  jeu restent intacts.
- **Modpack** → déjà différentiel par hash, pris en charge par le moteur.

## Configuration

**Tout ce qui est configurable vient du panel**, à chaque démarrage : maintenance, mode de
connexion, `client_id` Azure, dossier du jeu, liens sociaux, modules actifs, instances et
actualités. Rien n'est dupliqué dans le dépôt.

Les deux choses que l'API ne peut pas s'auto-annoncer — son adresse et le tenant — viennent du
pack client (`client_config.json`), attendu à côté du binaire :

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
`LUUXCRAFT_USER_ID`, et aucune adresse de panel n'est compilée nulle part dans le moteur. C'est
ce qui rend le même binaire utilisable par tous les serveurs. Ce fichier arrive sur le disque du
joueur par l'installeur — le bootstrap Windows ou le script `sh` — et il est le seul à savoir
quel serveur installer.

`src-tauri/src/config.rs` garde les constantes qu'aucune API ne fournit : `YGGDRASIL_SERVER`
(optionnel, le serveur legacy de Mojang étant arrêté) et les valeurs par défaut des réglages
(mémoire, fenêtre, téléchargements, statut serveur, actualités) appliquées tant que
l'utilisateur n'a rien changé. Le moteur ne connaît aucune adresse de mise à jour et ne se met
pas à jour lui-même.

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
bootstrap/                  installeur Windows autonome (voir bootstrap/README.md)
src-tauri/src/
  client_config.rs          lecture du pack client local (tenant, panel, nom, icône)
  update.rs                 mise à jour du moteur par lui-même
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

Le pack client est attendu dans `data/client/`. Plutôt que de l'y écrire à la main, le moteur
peut le télécharger depuis le panel — par la route que l'installeur utilise — avec l'identifiant
du tenant (`users.id`). C'est l'UUID qui figure dans la commande d'installation affichée par le
dashboard (onglet Téléchargement du launcher), **pas** le code d'appairage `xxx-xxx-xxx` : le
panel refuse ce dernier sur ses routes de distribution. Les scripts de `package.json` portent le
panel et le tenant de chaque environnement :

```bash
npm start            # staging  (https://staging.luuxcraft.fr)
npm run start:dev    # wrangler dev local (http://localhost:8080)
```

Les mêmes informations se passent aussi à la main :

```bash
npm run tauri dev -- -- -- --tenant-id=<users.id>                                     # panel de production
npm run tauri dev -- -- -- --tenant-id=<users.id> --api-url=http://localhost:8080     # autre panel
LUUXCRAFT_TENANT_ID=<users.id> LUUXCRAFT_PANEL_URL=… npm run tauri dev             # par l'environnement
```

Trois `--` : le premier pour npm, le deuxième pour la CLI tauri, le troisième pour que ce qui
suit aille au binaire et non à `cargo run` (les scripts de `package.json` n'ont que les deux
derniers). Le pack téléchargé reste dans
`data/client/` : les lancements suivants le relisent sans argument, jusqu'à ce qu'on le redonne
(pour rafraîchir, ou changer de tenant). Un panel local en `http://` est accepté **en debug
seulement** : le pack qu'il sert porte une `api_base_url` en clair, que le contrat refuse et
que seule la politique de développement (`client_config::Policy::DEVELOPMENT`) laisse passer.
Le flag ne s'appelle pas `--client-id` parce que `client_id` désigne déjà l'application Azure
et le code d'appairage — voir `src-tauri/src/dev_pack.rs`.

Tests :

```bash
npm test                                                        # vitest (frontend)
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored     # API réelle, Java local, Microsoft
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
compte réellement expiré est marqué « reconnexion requise », puis retenté une seule fois à
chaque démarrage.

Un *refresh token* Microsoft n'est accepté qu'avec le `client_id` Azure qui l'a émis. Chaque
compte retient donc l'application sous laquelle sa session a été obtenue (`clientId` dans
`accounts.json`), et le renouvellement l'essaie en premier, avant l'application publiée par le
panel puis celle du launcher officiel. Changer d'application côté panel — ou de panel en
développement — ne déconnecte donc plus les comptes existants. Les comptes rangés avant cette
mécanique n'ont pas de `clientId` : ils sont retentés sous l'application courante à chaque
démarrage, et le premier renouvellement réussi l'enregistre.

## Mises à jour automatiques

Le moteur se met à jour **lui-même** (`src-tauri/src/update.rs`). Vingt secondes après le
démarrage, il compare sa version à celle du manifeste du serveur ; si elles diffèrent, il
télécharge le nouveau bundle, vérifie son `sha256`, et remplace son propre exécutable. La
nouvelle version prend effet au lancement suivant — rien n'est redémarré sous les pieds du
joueur, et rien n'est tenté pendant que Minecraft tourne.

Ce qui est remplacé, et ce qui ne l'est jamais :

| Système | Remplacé | Laissé intact |
|---|---|---|
| Windows | `engine\<Nom>.exe` | `client\`, `%APPDATA%\.<slug>` |
| Linux | l'AppImage installée | `client/`, `~/.<slug>` |
| macOS | le bundle `.app` | `~/Library/Application Support/luuxcraft/installs/<clé>`, `~/.<slug>` |

L'identité du serveur est **toujours** hors de ce qui est remplacé : une mise à jour ne peut pas
faire perdre au launcher le serveur auquel il appartient, ni les comptes du joueur.

### Pourquoi pas `tauri-plugin-updater`

Le greffon officiel a été essayé puis écarté, pour deux raisons qui ne se contournent pas :

1. **Il ne sait pas mettre à jour un exécutable portable sous Windows.** Son installateur attend
   un NSIS ou un MSI, y compris à l'intérieur d'un zip
   ([documentation](https://v2.tauri.app/plugin/updater/)). Or un installeur d'OS écrit dans
   `Program Files` — un emplacement partagé — alors que chaque serveur doit vivre dans son
   propre dossier. L'adopter reviendrait à abandonner l'isolation par serveur, qui est la raison
   d'être de toute cette distribution.
2. **Il impose une paire de clés minisign** et un second manifeste à son format. Cela vaudrait
   la peine pour les trois plateformes ; pour les deux qu'il couvre, ça ferait deux mécanismes
   de mise à jour, deux formats de manifeste et une clé privée de plus à protéger — alors que
   Windows resterait de toute façon à la charge de `update.rs`.

Le jour où le moteur Windows serait distribué autrement, le greffon redevient le bon choix pour
les trois plateformes : il suffira de signer les artefacts et de servir un `latest.json`.

L'intégrité repose donc sur le `sha256` publié dans le manifeste, servi en https, et calculé sur
les octets téléversés par la CI. L'URL de téléchargement est vérifiée comme étant sur le panel du
pack client — sans quoi l'empreinte, qui vient de la même réponse, ne protégerait de rien.

Le moteur est publié en artefact **portable** — un exécutable zippé, un `.app` empaqueté, un
AppImage — jamais en installeur d'OS, pour la même raison qu'au point 1.

## Publication

`.github/workflows/deploy.yml` publie **deux lignes de produit** vers le panel, via
`.github/scripts/panel-release.mjs` :

| Job | Ce qui est construit | Formats |
|---|---|---|
| `engine` | `tauri build`, artefacts portables uniquement | `exe-zip` (windows), `app-tar-gz` (darwin), `appimage` (linux) |
| `bootstrap` | `cargo build` du crate `bootstrap/`, publié **vierge** | `bootstrap-exe` (windows x86_64) |

Il n'y a **ni nsis, ni msi, ni dmg, ni deb, ni rpm** : rien n'installe le moteur par le
gestionnaire de paquets de l'OS, il doit pouvoir se déployer par simple recopie dans le dossier
d'un client. D'où le détail de chaque format :

- **windows** — `tauri build --no-bundle`, puis `LuuxCraft Launcher.exe` zippé seul, à la
  racine de l'archive. `bundle.resources` est vide : l'exécutable se suffit à lui-même.
- **darwin** — `--bundles app` donne un `.app`, que la CI empaquette en `.app.tar.gz` en
  préservant les permissions Unix, sans quoi le Mach-O perdrait son bit exécutable.
- **linux** — l'`.AppImage` est déjà un fichier unique et portable, servi tel quel.
- **bootstrap** — un `.exe` nu, sans identité. Le job vérifie en plus que son créneau
  d'identité apparaît **exactement une fois** dans le binaire : le panel refuse de publier un
  modèle ambigu, et l'apprendre dans la CI vaut mieux que sur un téléchargement.

Le moteur est publié **par architecture**, parce que le manifeste du panel a un créneau par
`target`/`arch`. La route de téléchargement, elle, ne connaît que l'OS — un navigateur ne sait
pas dire l'architecture de la machine — et sert donc l'architecture la plus probable pour la
cible demandée (`universal` d'abord sous macOS, `x86_64` ailleurs).

À configurer une fois sur le dépôt :

| Nom | Type | Rôle |
|---|---|---|
| `PANEL_URL` | variable | `https://luuxcraft.fr` |
| `PANEL_BUILD_KEY` | secret | Admin → Configuration → Builds du launcher |

`PANEL_URL` sert deux fois : à savoir où téléverser, et comme adresse du panel **compilée dans le
bootstrap** (`LUUXCRAFT_PANEL_URL`). Un bootstrap construit contre le mauvais panel ne se verrait
qu'une fois chez les joueurs.

Une variable et un secret, c'est tout : **aucune clé de signature** — l'intégrité tient au
`sha256` du manifeste — et **aucun identifiant de stockage à fournir**. Les
octets sont téléversés en multipart vers le panel, qui les écrit par sa propre liaison R2 — la même qui les
relit ensuite pour vérifier. Un artefact accepté est donc forcément un artefact visible, là où
des identifiants S3 mal réglés pourraient écrire dans un bucket que le panel ne lit pas. Le
découpage en parts enlève par ailleurs toute limite de taille (un AppImage embarque webkit2gtk
et dépasserait la limite de corps de requête d'un Worker en un seul envoi).

Le workflow part sur un tag `v*` ou à la main (canal `stable`/`beta`, notes de version, et la
possibilité de laisser la release en préparation). La version publiée est celle de
`src-tauri/tauri.conf.json`, pas celle du tag. La publication est **tout ou rien**, à deux
niveaux : le job de publication dépend des jobs de build, donc une plateforme en échec ne
publie rien ; et le panel refuse en plus un artefact dont les octets manquent ou dont la taille
ne correspond pas à celle annoncée.

**Relancer un build écrase la version.** Le job `open-release` supprime les artefacts de
l'exécution précédente — octets R2 compris — et remet la release en préparation, même si elle
était publiée : la release correspond ainsi exactement à ce que cette exécution a produit, sans
qu'un artefact orphelin survive dans un créneau que la nouvelle matrice ne remplit plus. Le prix
à connaître : **entre le début du build et le `publish` final, cette version n'est plus
servie**. Les téléchargements et les launchers installés retombent sur la release publiée
précédente, ou n'ont plus rien si c'était la seule. La CI l'annonce par un `::warning::` quand
la version écrasée était publiée. Pour ne rien interrompre, incrémenter la version dans
`src-tauri/tauri.conf.json` plutôt que reconstruire la même.

Build local, pour vérifier avant de pousser — rien à signer, rien à exporter :

```bash
npm run tauri build -- --no-bundle            # windows : l'exécutable brut, à zipper
npm run tauri build -- --bundles app          # macOS : le .app, à empaqueter en .app.tar.gz
npm run tauri build -- --bundles appimage     # linux : l'AppImage, portable tel quel
cargo build --release --manifest-path bootstrap/Cargo.toml   # l'installeur Windows
```

### Ajouter une architecture

Trois endroits, dans cet ordre :

1. la matrice du job `engine` dans `.github/workflows/deploy.yml` (`target`, `arch`, `triple`) ;
2. `ARTIFACT_ARCHES` côté panel (`src/lib/launcherArtifacts.ts`), qui refuse ce qu'il ne connaît
   pas — c'est ce qui empêche un job mal câblé de publier dans un créneau fantôme ;
3. `ARCH_PREFERENCE` (`src/services/LauncherReleaseService.ts`) si la nouvelle architecture doit
   servir de repli quand l'appelant n'en annonce pas.

Rien à changer côté client : le bootstrap et le script annoncent déjà leur architecture réelle,
et le manifeste leur répond dans le créneau correspondant.

### Ajouter une plateforme

En plus des trois points ci-dessus : `ARTIFACT_TARGETS`, `FORMAT_BY_SUFFIX` et
`engineFormatForTarget` côté panel, puis un installeur pour cette plateforme — soit une branche
du script `sh` (`src/lib/installScript.ts`), soit un bootstrap, selon ce que la plateforme rend
praticable. Le moteur, lui, n'a rien à apprendre : il lit son pack à côté de lui.

## Sécurité

- Jetons dans `accounts.json` (permissions utilisateur seul) ; chiffrement au repos prévu.
- Aucun jeton, mot de passe ni code OTP dans les logs ; la sortie de Minecraft est expurgée
  des secrets de session.
- CSP stricte (`src-tauri/tauri.conf.json`), HTML des actualités filtré par liste blanche,
  liens ouverts uniquement en `http(s)` via le navigateur.
- Fichiers d'instance vérifiés par taille et SHA-1 (crust_core).
- Le moteur et le pack client ont un SHA-256 publié dans le manifeste du panel, servi en https,
  vérifié par les trois lecteurs : le bootstrap Windows, le script `sh`, et le moteur quand il
  se met à jour. Rien n'est mis en place — ni lancé — avant que l'empreinte corresponde.
- Le script `sh` n'exécute jamais une valeur venue de l'API : slug borné à `[a-z0-9-]`,
  empreintes à 64 hexadécimaux, URL obligatoirement sur le panel, fichiers du pack pris dans une
  liste fermée. Un serveur nommé `Launcher; rm -rf ~` ne produit qu'un nom de dossier étrange.

## Logs

`launcher.log` (rotation 5 × 5 Mo) dans le dossier de logs de l'OS (ou `data/logs` en debug),
sortie de Minecraft dans `logs/game/<instance>/latest.log`. Accès direct depuis
Paramètres → Logs & diagnostic.
