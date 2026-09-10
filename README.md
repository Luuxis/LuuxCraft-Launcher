# LuuxCraft Launcher

Launcher Minecraft de LuuxCraft, construit sur [Tauri 2](https://v2.tauri.app), React et
[`crust_core` 1.0.3](https://docs.rs/crust_core/1.0.3). L'interface reprend le design system
« LuuxCraft Forge » du panel (voir `CHARTE-GRAPHIQUE.md`).

Le projet est une **base réutilisable** : un autre launcher se crée en changeant
`launcher.config.json`, sans toucher au code.

## Configuration centrale

`launcher.config.json` (racine du dépôt) est embarqué côté Rust (`include_str!`) et importé
côté frontend. Il contient :

| Clé | Rôle |
|---|---|
| `userId` | Identifiant du launcher sur le panel (`/api/user/{userId}/...`) |
| `api.baseUrl` | Base de l'API LuuxCraft (`https://luuxcraft.fr/api`) |
| `brand` | Nom, wordmark (`Luux` + `Craft`), sous-titre, site |
| `dataDirectory` | Dossier du jeu par défaut (remplacé par `dataDirectory` du panel s'il existe) |
| `updater.endpoints` | Serveurs de mise à jour Tauri (vide = auto-update désactivé) |
| `auth.yggdrasilServer` | Serveur Yggdrasil optionnel (`Mojang` legacy est arrêté par Mojang) |
| `news`, `serverStatus`, `downloads`, `memory`, `gameWindow` | Valeurs par défaut des réglages |
| `links` | Liens affichés quand le panel n'en publie pas |
| `modules` | Modules activés par défaut (le panel peut les surcharger) |

## Données du panel

Routes consommées (mêmes routes que les launchers de référence LuuxCraft) :

- `GET {baseUrl}/user/{userId}/config` — `maintenance`, `maintenance_message`, `dataDirectory`,
  `online` (`true` = Microsoft, `false` = hors ligne, URL = site Azuriom/AZauth), `client_id`,
  `socialLinks`, plus tout champ futur (conservé dans `extra`, `modules`/`features` pour les
  bascules de modules).
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
launcher.config.json        configuration centrale (Rust + frontend)
src-tauri/src/
  config.rs                 chargement/validation de la config, chemins
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
  features/*                accueil, instances, comptes, skin 3D (skinview3d/three), paramètres
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

Build signé :

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/luuxcraft-launcher.key)"   # contenu de la clé
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run tauri build
```

Les artefacts `*.sig` produits avec les bundles alimentent un manifeste statique :

```json
{
  "version": "1.1.0",
  "notes": "Nouveautés…",
  "pub_date": "2026-09-13T12:00:00Z",
  "platforms": {
    "darwin-aarch64": { "signature": "…", "url": "https://…/LuuxCraft.app.tar.gz" },
    "darwin-x86_64":  { "signature": "…", "url": "https://…/LuuxCraft.app.tar.gz" },
    "windows-x86_64": { "signature": "…", "url": "https://…/LuuxCraft-setup.exe" },
    "linux-x86_64":   { "signature": "…", "url": "https://…/LuuxCraft.AppImage" }
  }
}
```

Renseignez son URL dans `launcher.config.json` → `updater.endpoints`
(variables `{{target}}`, `{{arch}}`, `{{current_version}}` disponibles).

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
