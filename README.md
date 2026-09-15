# LuuxCraft Launcher

Launcher Minecraft de LuuxCraft, construit sur [Tauri 2](https://v2.tauri.app), React et
[`crust_core` 1.0.3](https://docs.rs/crust_core/1.0.3). L'interface reprend le design system
« LuuxCraft Forge » du panel (voir `CHARTE-GRAPHIQUE.md`).

Le projet est une **base réutilisable** : un autre launcher se crée en pointant vers un autre
panel, sans toucher au reste du code.

## Configuration

**Tout ce qui est configurable vient du panel**, à chaque démarrage : maintenance, mode de
connexion, `client_id` Azure, dossier du jeu, liens sociaux, modules actifs, instances et
actualités. Rien n'est dupliqué dans le dépôt.

Restent deux choses que l'API ne peut pas s'auto-annoncer : son adresse, et *à quel client du
panel* ce launcher appartient. La première est générique — elle est la même pour tous les
clients d'un panel — et reste donc compilée dans `src-tauri/src/config.rs`. La seconde **n'est
pas compilée** : un seul binaire sert tous les clients, et c'est le panel qui y ajoute la
configuration au moment du téléchargement (voir « Provisionnement » plus bas).

| Variable de build | Rôle | Défaut |
|---|---|---|
| `LUUXCRAFT_API_URL` | Base de l'API (`https://…/api`) | `https://luuxcraft.fr/api` |
| `LUUXCRAFT_USER_ID` | Épingle le launcher à un seul client, au lieu du provisionnement | vide |

```bash
# Build générique, celui que la CI publie : aucun client en dur.
npm run tauri build

# Build dédié à un client, si on y tient vraiment.
LUUXCRAFT_API_URL=https://autre-panel.fr/api LUUXCRAFT_USER_ID=abc-def-ghi npm run tauri build
```

Le même fichier porte les quelques constantes qu'aucune API ne fournit : `YGGDRASIL_SERVER`
(optionnel, le serveur legacy de Mojang étant arrêté) et les valeurs par défaut des réglages
(mémoire, fenêtre, téléchargements, statut serveur, actualités) appliquées tant que
l'utilisateur n'a rien changé. Les points de mise à jour sont dérivés de l'adresse du panel,
pas écrits en dur : un launcher provisionné sur un autre panel prend ses mises à jour de ce
panel-là.
L'identité visuelle (nom, wordmark) est dans `src/config/brand.ts` : elle est peinte dans la
barre de titre dès la première image, avant toute requête.

## Provisionnement

Le launcher n'est **pas compilé par client**. Le panel stocke un seul jeu d'artefacts et, quand
un client télécharge son installeur, il y ajoute un bloc de 512 octets qui dit à quel panel
parler. Renommer, déplacer ou re-télécharger le fichier ne casse rien : l'identité est dans les
octets du fichier, jamais dans son nom.

`src-tauri/src/provisioning.rs` cherche cette configuration dans cet ordre :

1. **bloc à la fin de l'exécutable courant** — le cas d'une AppImage Linux ou d'un `.exe`
   portable, qui *sont* le fichier téléchargé ;
2. **`provisioning.blob` à côté de l'exécutable** — écrit par `src-tauri/installer-hooks.nsh`,
   le hook NSIS qui relit son propre bloc au moment d'installer ;
3. **`provisioning.json` à côté du bundle `.app`** (macOS) — posé par le zip que le panel
   reconstruit à partir du `.app.tar.gz`, **jamais dans `Contents/`** ;
4. **`provisioning.json` du dossier de données** — la copie persistée, qui survit aux mises à
   jour (l'installeur téléchargé par l'updater, lui, n'a ni bloc ni fichier voisin) ;
5. **`LUUXCRAFT_USER_ID` compilé**, pour un build dédié ;
6. sinon, l'interface demande son code au joueur une seule fois
   (`src/features/provisioning/PairingView.tsx`) — le cas du `.dmg` traditionnel.

Les sources fraîches passent avant la copie persistée : réinstaller avec l'installeur d'un autre
serveur doit changer de serveur. Le sixième cas ne sert qu'aux formats qui ne tolèrent ni bloc
ajouté à la fin ni fichier voisin injectable — le `.dmg` a son trailer `koly` collé à la fin, un
`.deb` porte des sommes de contrôle.

### macOS : zip préconfiguré plutôt qu'un code à saisir

Un `.app` est une arborescence de fichiers, pas un binaire avec une « fin » où ajouter un bloc —
et `codesign` scelle le hash de chaque fichier du bundle dans sa signature : y ajouter quoi que
ce soit **dans** `Contents/` après coup la casse (Gatekeeper refuse de lancer l'app, « endommagée
»). Reconstruire la signature demanderait le certificat développeur Apple et un aller-retour de
notarisation, hors de portée d'un Worker.

La solution retenue ne touche donc jamais au bundle : au téléchargement, le panel reconstruit un
zip à partir du `.app.tar.gz` déjà produit pour l'updater (`lib/provisioning.ts` →
`createProvisionedMacZipStream`, via les writers `lib/tar.ts` / `lib/zip.ts` déjà utilisés pour
les sauvegardes) — chaque fichier du bundle est recopié tel quel (mêmes octets, même hash), et un
seul fichier neuf, `provisioning.json`, est ajouté **à côté** de `<App>.app` dans le zip. Au
premier lancement, `provisioning.rs` retrouve ce fichier en remontant de trois niveaux depuis
l'exécutable (`<bundle>.app/Contents/MacOS/<binaire>` → le dossier qui contient aussi
`<bundle>.app`, la racine d'extraction du zip).

Le `.dmg` traditionnel reste servi en repli (`?format=dmg`), pour qui préfère l'installeur
classique au prix du code à saisir une fois.

Le bloc n'est pas signé, et n'a pas à l'être : il ne contient que des informations déjà
publiques (la clé client voyage en clair dans toutes les URLs de l'API) et quiconque peut le
réécrire peut de toute façon réécrire l'exécutable entier. La seule garantie utile est le refus
d'une `apiUrl` qui n'est pas en https.

Le format est décrit une fois, dans `src/lib/provisioning.ts` du panel, et relu à l'identique
par `provisioning.rs` et par le hook NSIS. Trois contraintes l'expliquent : taille fixe (NSIS se
place à `-512` de la fin sans connaître la longueur), ASCII imprimable sans retour à la ligne
(`FileRead` s'arrête au premier `\n`, et la conversion ANSI → UTF-16 d'un installeur Unicode
est l'identité sur 0x20–0x7E), et 512 octets et non 1024 (`NSIS_MAX_STRLEN` vaut 1024).
Changer l'un des trois oblige à changer les trois.

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

Les modèles (`src-tauri/src/api/models.rs`) acceptent plusieurs orthographes de clés et
ignorent ce qu'ils ne connaissent pas : le panel peut ajouter, renommer ou retirer des champs
sans casser le launcher. Le dernier instantané est mis en cache sur disque pour le mode hors
ligne.

## Architecture

```
src-tauri/src/
  config.rs                 adresse du panel, défauts intégrés, chemins
  provisioning.rs           à quel client du panel ce launcher appartient
  api/                      client HTTP du panel + modèles tolérants + commandes remote_*
  accounts/                 multi-comptes dans `<dossier du jeu>/accounts.json`
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
  features/*                accueil (jouer + instance), comptes, skins (skinview3d/three), paramètres,
                            appairage (premier lancement d'un build non provisionné)
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
`data/logs`, `data/cache`) ; en release, dans les dossiers standards de l'OS
(`app_data_dir`, `app_log_dir`, `data_dir/<dataDirectory>`).

Tests :

```bash
npm test                                                        # vitest (frontend)
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored     # API réelle, Java local, Microsoft
```

## Comptes et connexion Microsoft

Les comptes (jetons compris) sont dans **`<dossier du jeu>/accounts.json`**
(`data/minecraft/accounts.json` en debug), au format de `minecraft-java-core`, fichier lisible
par l'utilisateur seul. Le fichier est versionné (`"version": 1`) pour accueillir un chiffrement
ultérieur, et il suit le dossier d'installation choisi dans les paramètres.

La connexion Microsoft utilise le flux *authorization code* de `crust_core` dans une **fenêtre de
connexion** dédiée, avec le `client_id` publié par le panel sur `login.live.com` (ou l'identifiant
du launcher officiel si le panel n'en publie pas), comme l'ancien launcher Electron. La fenêtre est
en mode privé pour pouvoir ajouter plusieurs comptes ; la fermer annule la connexion.

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

Le launcher utilise `tauri-plugin-updater`. La clé publique est dans `src-tauri/tauri.conf.json`
(`plugins.updater.pubkey`) ; la clé privée correspondante est **hors dépôt** :
`~/.tauri/luuxcraft-launcher.key` (générée avec `npx tauri signer generate`, sans mot de passe).
Ne la perdez pas : sans elle, aucune mise à jour ne pourra être signée pour les launchers déjà
installés.

Le point de mise à jour n'est pas un manifeste statique mais le **serveur dynamique du panel** :

```
{baseUrl}/launcher/update/{{target}}/{{arch}}/{{current_version}}
```

Il est dérivé de l'adresse du panel (`config.rs` → `updater_endpoints_for`), donc un launcher
provisionné sur un autre panel prend ses mises à jour de ce panel-là. Le panel répond un `204`
quand il n'y a rien à installer, et sinon le JSON attendu par le plugin (`version`, `notes`,
`pub_date`, `url`, `signature`). Un panel peut aussi surcharger la liste par
`updater.endpoints` dans `/config` : ce qu'il publie gagne sur la valeur dérivée.

Les artefacts de mise à jour sont servis **intacts** : la signature minisign porte sur leurs
octets exacts, et le bloc de provisionnement n'est ajouté que sur les routes de téléchargement.
Sous Linux, l'`.AppImage` est à la fois l'installeur et l'artefact signé — c'est pourquoi les
octets stockés ne sont jamais modifiés.

Comme les mises à jour sont servies par le panel à partir d'un binaire unique, elles sont
globales : une release, une chaîne de mises à jour, tous les clients.

### Publication

`.github/workflows/deploy.yml` construit les quatre plateformes, les signe et les pousse vers le
panel via `.github/scripts/panel-release.mjs`. À configurer une fois sur le dépôt :

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
découpage en parts enlève par ailleurs toute limite de taille (un AppImage embarque webkit2gtk et
dépasserait la limite de corps de requête d'un Worker en un seul envoi).

Le workflow part sur un tag `v*` ou à la main (canal `stable`/`beta`, notes de version, et la
possibilité de laisser la release en préparation). La version publiée est celle de
`src-tauri/tauri.conf.json`, pas celle du tag : c'est elle que tauri utilise pour nommer les
bundles. La publication est **tout ou rien**, à deux niveaux : le job de publication dépend du job de
build, donc une plateforme en échec ne publie rien ; et le panel refuse en plus un artefact dont
les octets manquent ou un artefact de mise à jour sans signature. Si malgré tout une plateforme
n'a pas d'artefact, le manifeste lui répond « à jour » au lieu d'une URL vide.

Build signé en local, pour vérifier avant de pousser :

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/luuxcraft-launcher.key)"   # contenu de la clé
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run tauri build
```

## Sécurité

- Jetons dans `accounts.json` (permissions utilisateur seul) ; chiffrement au repos prévu.
- Aucun jeton, mot de passe ni code OTP dans les logs ; la sortie de Minecraft est expurgée
  des secrets de session.
- CSP stricte (`src-tauri/tauri.conf.json`), HTML des actualités filtré par liste blanche,
  liens ouverts uniquement en `http(s)` via le navigateur.
- Fichiers d'instance vérifiés par taille et SHA-1 (crust_core), mises à jour signées.

## Logs

`launcher.log` (rotation 5 × 5 Mo) dans le dossier de logs de l'OS (ou `data/logs` en debug),
sortie de Minecraft dans `logs/game/<instance>/latest.log`. Accès direct depuis
Paramètres → Logs & diagnostic.
