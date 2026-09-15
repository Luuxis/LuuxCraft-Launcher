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

1. **bloc à la fin du fichier téléchargé** — le cas d'une AppImage Linux ou d'un `.exe`
   portable, qui *sont* le fichier téléchargé. Sous Linux ce fichier n'est **pas**
   `current_exe()`, qui désigne le binaire extrait au point de montage temporaire de l'AppImage
   (`/tmp/.mount_XXXXXX/usr/bin/…`) : le runtime publie le vrai chemin dans `$APPIMAGE`, d'où
   `provisioning::appimage_path` ;
2. **`provisioning.blob` à côté de l'exécutable** — écrit par `src-tauri/installer-hooks.nsh`,
   le hook NSIS qui relit son propre bloc au moment d'installer ;
3. **`Contents/Resources/provisioning.json`** (macOS) — posé *dans* le bundle par le zip que le
   panel reconstruit à partir du `.app.tar.gz` ;
4. **`provisioning.json` à côté du bundle `.app`** (macOS) — l'emplacement des premiers zips du
   panel, gardé pour ceux déjà téléchargés ;
5. **`provisioning.json` du dossier de données** — la copie persistée, qui survit aux mises à
   jour (le bundle téléchargé par l'updater, lui, n'a ni bloc ni fichier de configuration) ;
6. **`LUUXCRAFT_USER_ID` compilé**, pour un build dédié ;
7. sinon, l'interface demande son code au joueur une seule fois
   (`src/features/provisioning/PairingView.tsx`) — le cas du `.dmg` traditionnel.

Les sources fraîches passent avant la copie persistée : réinstaller avec l'installeur d'un autre
serveur doit changer de serveur. Le sixième cas ne sert qu'aux formats qui ne tolèrent ni bloc
ajouté à la fin ni fichier voisin injectable — le `.dmg` a son trailer `koly` collé à la fin, un
`.deb` porte des sommes de contrôle.

### macOS : un bundle déjà configuré, à glisser dans Applications

Un `.app` est une arborescence de fichiers, pas un binaire avec une « fin » où ajouter un bloc.
Au téléchargement, le panel reconstruit donc un zip à partir du `.app.tar.gz` déjà produit pour
l'updater (`lib/provisioning.ts` → `createProvisionedMacAppZipStream`, via les writers
`lib/tar.ts` / `lib/zip.ts` déjà utilisés pour les sauvegardes), et pose la configuration du
client dans `Contents/Resources/provisioning.json`. Elle voyage ainsi **avec** le bundle : le
joueur n'a rien d'autre à faire que le glisser dans Applications.

Écrire dans `Contents/` n'est possible que parce que le bundle n'est pas signé. `codesign` scelle
le hash de chaque fichier du bundle dans `Contents/_CodeSignature/CodeResources`, et modifier un
bundle scellé le rend « endommagé » aux yeux de Gatekeeper — mais ce sceau n'existe que si une
identité de signature Apple a été fournie au build, ce qui n'est pas le cas ici. La signature
ad-hoc que le linker pose sur le binaire arm64, elle, ne couvre que le Mach-O, auquel on ne
touche pas. **Le jour où la CI signera vraiment**, le panel refuse de personnaliser le bundle
(erreur explicite plutôt qu'une application morte chez le joueur) et il faudra re-signer après
modification — ce qu'un Worker ne peut pas faire.

Les permissions Unix sont reportées du tar vers le zip (`mode` des entrées) : sans elles,
`Contents/MacOS/<binaire>` ressort non exécutable et macOS refuse d'ouvrir l'application.

Le `.dmg` traditionnel reste servi en repli (`?format=dmg`), pour qui préfère l'installeur
classique au prix du code à saisir une fois.

Le bloc n'est pas signé, et n'a pas à l'être : il ne contient que des informations déjà
publiques (la clé client voyage en clair dans toutes les URLs de l'API) et quiconque peut le
réécrire peut de toute façon réécrire l'exécutable entier. La seule garantie utile est le refus
d'une `apiUrl` qui n'est pas en https.

Le format est décrit une fois, dans `src/lib/provisioning.ts` du panel, et relu à l'identique
par `provisioning.rs` et par le hook NSIS. Il tient sur deux lignes de 512 octets au total :

```
LUUXCRAFT-PROVISIONING-V1<base64url du JSON>\n   lu par le launcher
LUUXCRAFT-BRAND-V1|<nom>|<url de l'icône>|\n     lu par le hook NSIS
```

Trois contraintes l'expliquent : taille fixe (NSIS se place à `-512` de la fin sans connaître la
longueur), ASCII imprimable (la conversion ANSI → UTF-16 d'un installeur Unicode est l'identité
sur 0x20–0x7E), et 512 octets et non 1024 (`NSIS_MAX_STRLEN` vaut 1024, et c'est la borne d'*une*
chaîne — d'où le découpage en lignes, `FileRead` s'arrêtant à chaque `\n`). Changer l'un des
trois oblige à changer les trois implémentations.

## Nom et logo du client

Le launcher étant compilé une fois pour tous, son identité visuelle ne vient pas du build mais du
client : `launcherConfigs.name` et l'icône téléversée dans le dashboard. Elle est appliquée à
quatre moments.

| Où | Quoi | Par qui |
| --- | --- | --- |
| Téléchargement | nom du fichier (`Mon Serveur-1.2.0-Setup.exe`) | panel, `clientDownloadFilename` |
| Installation Windows | raccourci du menu Démarrer, icône, entrée « Applications et fonctionnalités » | `installer-hooks.nsh` |
| Installation macOS | nom du `.app`, `CFBundleName`/`CFBundleDisplayName`, `Resources/*.icns` | panel, `createProvisionedMacAppZipStream` |
| 1er lancement Linux | entrée du menu d'applications et icône du thème (XDG) | `src-tauri/src/desktop.rs` |
| Exécution | titre de fenêtre, icône de la barre des tâches, logo de la barre de titre | `src-tauri/src/branding.rs` + `src/config/brand.ts` |

L'icône n'est reprise que si elle est stockée en **PNG** : `.ico` et `.icns` savent embarquer un
PNG tel quel — c'est ce qui rend la conversion possible dans un Worker, sans décodeur d'image —
mais rien ne peut transcoder un JPEG ou un WebP. Un client dont le logo n'est pas un PNG garde
l'icône compilée.

Limites assumées :

- **Windows** — l'icône du `.exe` installé dans l'Explorateur reste celle du build : elle vit dans
  la charge compressée de l'installeur NSIS, hors de portée sans recompiler ;
- **Windows** — le raccourci du Bureau est créé par la page finale de l'installeur, **après** le
  hook : c'est le launcher qui le renomme à son premier démarrage (`branding.rs`) ;
- **macOS** — après une mise à jour automatique, le Dock reprend le nom et l'icône compilés. Le
  paquet de mise à jour est servi **intact** (la signature minisign porte sur ces octets exacts,
  un seul octet en plus la casse), donc le `.app` remplacé est le générique. Le dossier garde son
  nom, le provisionnement survit via la copie persistée, et le titre de fenêtre reste celui du
  client — seuls le nom et l'icône du Dock retombent au générique jusqu'au prochain
  téléchargement depuis le panel.

### Linux : intégration au bureau (XDG)

Windows et macOS portent l'identité du client dans le conteneur : l'installeur NSIS écrit un
raccourci et une entrée de désinstallation, le bundle `.app` porte son `Info.plist` et son
`.icns`. Une AppImage n'est qu'un fichier exécutable : sans rien de plus, le joueur n'a ni entrée
dans son menu d'applications, ni icône dans son dock — juste un fichier dans `Téléchargements`.

`src-tauri/src/desktop.rs` écrit donc, au démarrage, aux emplacements que la spécification XDG
réserve à l'utilisateur (aucun `sudo`, rien de touché dans `/usr`) :

```
~/.local/share/applications/<app_id>.desktop
~/.local/share/icons/hicolor/<taille>/apps/<app_id>.png
```

`app_id` vaut `<identifiant du bundle>.<clé du client>` : deux AppImages de serveurs différents
cohabitent sans que la seconde écrase l'entrée de la première. Le prix est que le nom de l'entrée
ne coïncide plus avec la classe de fenêtre que GNOME utilise pour relier une fenêtre ouverte à son
icône — d'où le `StartupWMClass` renseigné, qui est exactement fait pour ça.

`Exec=` pointe l'AppImage *là où le joueur l'a laissée* (`$APPIMAGE`, jamais `current_exe()` qui
désigne le point de montage temporaire). Rien n'est déplacé dans `~/.local/bin` : bouger un
fichier que le joueur vient de télécharger serait une surprise, et l'entrée est de toute façon
réécrite au lancement suivant si le fichier a changé de place — une empreinte
(`desktop-entry.json`) évite le travail inutile le reste du temps.

Seule l'AppImage est concernée. Les paquets `.deb` et `.rpm` produits par tauri installent déjà
leur propre entrée dans `/usr/share/applications`, qu'on ne peut pas corriger sans droits sur
`/usr` ; et ils ne peuvent pas porter le bloc de provisioning, ce n'est donc pas par eux qu'un
client est servi.

## Un dossier de données par client

Un seul launcher est compilé pour tous les clients du panel, et rien n'empêche un joueur
d'installer celui de deux serveurs différents. Ses réglages, ses comptes, ses skins et ses caches
sont donc rangés sous `clients/<clé du client>/`, dans chacune des racines de l'application
(données, cache, journaux). Sans ce découpage, le second launcher installé écraserait les
réglages du premier — et surtout les deux partageraient les jetons de session d'`accounts.json`.

Deux exceptions volontaires :

- **`provisioning.json`** reste à la racine commune : il faut savoir *quel* client avant de
  pouvoir ouvrir son dossier ;
- **la racine du jeu** reste désignée par le `dataDirectory` du panel. Elle pèse des gigaoctets,
  et deux serveurs qui la partagent partagent aussi versions, bibliothèques et assets déjà
  téléchargés.

Une installation d'avant ce découpage est migrée une fois, au premier démarrage : tout ce qui
traîne à la racine est déplacé sous le client — sauf les noms communs ci-dessus et les autres
racines de l'application, que tauri imbrique parfois l'une dans l'autre (sous Linux `app_log_dir`
est un sous-dossier d'`app_data_dir`, sous Windows d'`app_cache_dir`). La migration a lieu
**avant** que le greffon de journalisation n'ouvre son fichier : Windows refuse de renommer un
fichier ouvert.

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
  desktop.rs                entrée de bureau et icône XDG (Linux)
  api/                      client HTTP du panel + modèles tolérants + commandes remote_*
  accounts/                 multi-comptes dans `<dossier du client>/accounts.json`
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

Les comptes (jetons compris) sont dans **`<dossier du client>/accounts.json`**, au format de
`minecraft-java-core`, fichier lisible par l'utilisateur seul. Le fichier est versionné
(`"version": 1`) pour accueillir un chiffrement ultérieur.

Ils appartiennent au **client**, pas à l'installation du jeu : deux serveurs peuvent partager une
racine de jeu — c'est même souhaitable, elle pèse des gigaoctets — mais sûrement pas les sessions
Minecraft du joueur. Changer de dossier d'installation ne fait donc plus perdre ses comptes. Un
`accounts.json` laissé à l'ancien emplacement (`<dossier du jeu>/accounts.json`) est **déplacé**
au premier démarrage, pas copié : des jetons valides oubliés dans un dossier que le launcher ne
lit plus seraient relus par un autre client.

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

**Relancer un build écrase la version.** Le job `open-release` supprime les artefacts de
l'exécution précédente — octets R2 compris — et remet la release en préparation, même si elle
était publiée : la release correspond ainsi exactement à ce que cette exécution a produit, sans
qu'un artefact orphelin survive dans un créneau que la nouvelle matrice ne remplit plus. Le prix à
connaître : **entre le début du build et le `publish` final, cette version n'est plus servie**. Les
launchers installés et la page de téléchargement retombent sur la release publiée précédente, ou
n'ont plus rien si c'était la seule. La CI l'annonce par un `::warning::` quand la version écrasée
était publiée. Pour ne rien interrompre, incrémenter la version dans `src-tauri/tauri.conf.json`
plutôt que reconstruire la même.

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
