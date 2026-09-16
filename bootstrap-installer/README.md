# `bootstrap-installer`

L'installeur figé que télécharge le joueur. Un binaire Rust par (OS, arch), sans tauri, sans
webview, sans interface : il affiche sa progression sur la sortie standard, installe le moteur
et le pack client, pose les raccourcis, lance le moteur et rend la main.

Il est **compilé une fois** et publié tel quel dans le R2 du panel. Ce n'est pas un build par
client : les octets servis à tous les serveurs sont les mêmes, à 32 octets près.

## L'overlay de 32 octets

C'est tout ce qui distingue le téléchargement d'un serveur de celui d'un autre. Le panel lit
le bootstrap dans R2, **streame** ses octets, puis émet en fin de flux :

```
[ MAGIC_START 8 o ][ UUID 16 o bruts ][ MAGIC_END 8 o ]
```

| Champ | Valeur | Octets |
|---|---|---|
| `MAGIC_START` | ASCII `LXCBOOT1` | `4C 58 43 42 4F 4F 54 31` |
| `UUID` | `users.id`, ordre canonique RFC 4122 (gros boutiste) | 16 octets bruts, **jamais** la forme textuelle |
| `MAGIC_END` | ASCII `1TOOBCXL` | `31 54 4F 4F 42 43 58 4C` |

Taille totale constante : **32 octets**. Ajoutés en fin de fichier, ils ne perturbent ni un
PE Windows, ni un ELF, ni un Mach-O : les trois lisent leur table de sections depuis l'en-tête
et ignorent ce qui traîne après.

Le bootstrap relit ces octets dans `std::env::current_exe()` — pas dans `argv[0]` — donc le
joueur peut renommer le fichier téléchargé sans rien casser. Si les deux magies ne sont pas
là, ou si l'UUID est nul, il s'arrête avec un message explicite : **aucun repli**, un
installeur sans identité ne sait pas quel client installer.

L'overlay n'est pas signé et n'a pas à l'être : il ne contient qu'un identifiant public, et
qui peut le réécrire peut de toute façon réécrire l'exécutable entier.

### Fabriquer un installeur de test à la main

```bash
cargo build --release
python3 - <<'PY'
import uuid
tenant = uuid.UUID("00000000-0000-4000-8000-000000000000")   # users.id
engine = open("target/release/luuxcraft-bootstrap", "rb").read()
open("/tmp/mon-serveur-installeur", "wb").write(
    engine + b"LXCBOOT1" + tenant.bytes + b"1TOOBCXL"
)
PY
chmod +x /tmp/mon-serveur-installeur && /tmp/mon-serveur-installeur
```

## Ce qu'il fait, dans l'ordre

1. **Identité** — lit l'UUID du tenant dans ses 32 derniers octets (`overlay.rs`).
2. **Manifeste** — `GET {panel}/api/v1/launchers/{tenant_id}/manifest?os=…&arch=…`
   (`manifest.rs`). Le panel y décrit le bundle moteur à installer et les fichiers du pack
   client, chacun avec son `sha256`. Le schéma est vérifié, ainsi que la concordance du
   `tenant_id` : un manifeste écrit pour un autre client est refusé.
3. **Moteur** (`engine.rs`) — retéléchargé **seulement** si `.engine-version` ne correspond
   plus. Empreinte vérifiée avant toute mise en place, décompression dans un dossier
   temporaire, puis échange du dossier `engine/`.
4. **Pack client** (`pack.rs`) — fichier par fichier, seule une empreinte différente déclenche
   un téléchargement. C'est ce qui fait qu'un changement de logo coûte quelques kilo-octets.
5. **Raccourcis** (`shortcuts.rs`) — au nom et à l'icône du client.
6. **Lancement** (`launch.rs`) — le moteur est démarré, le bootstrap sort.

Rien n'est mis en place avant d'avoir été vérifié : chaque téléchargement va dans
`<racine>/.tmp`, son SHA-256 est calculé pendant l'écriture, comparé à celui du manifeste,
puis le fichier est renommé — sur le même système de fichiers, donc atomiquement. Un moteur à
moitié téléchargé n'est jamais lancé.

## Ce qu'il écrit sur la machine du joueur

| OS | Racine installée | Raccourcis |
|---|---|---|
| Windows | `%LOCALAPPDATA%\Programs\<slug>\` | `.lnk` sur le Bureau et dans le menu Démarrer |
| Linux | `~/.local/share/<slug>/` | `~/.local/share/applications/<slug>.desktop` |
| macOS | `~/Applications/<nom du serveur>.app/` | le `.app` lui-même |

Contenu de la racine :

```
engine/            le moteur décompressé
client/            le pack client (client_config.json, icon.png, icon.ico, icon.icns)
.engine-version    version, empreinte et chemin de l'exécutable installés
.pack-version      empreinte agrégée du pack client installé
```

`.engine-version` tient sur trois lignes — version, `sha256` du bundle téléchargé, chemin de
l'exécutable dans `engine/` :

```
1.2.3
9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
luuxcraft-launcher.exe
```

La troisième ligne évite de reparcourir l'arborescence à chaque démarrage ; si elle manque ou
ne pointe plus sur rien, le binaire est redétecté.

Les données d'exécution (comptes, réglages, caches, profil webview) n'appartiennent pas au
bootstrap : elles vivent dans `~/.<slug>` et sont écrites par le moteur.

### macOS : le `.app` est fabriqué ici

Le serveur ne repackage rien. Le bootstrap écrit lui-même, dans la racine d'installation :

```
Contents/Info.plist          CFBundleIdentifier = com.luuxcraft.launcher.<slug>
Contents/MacOS/<moteur>      lien physique vers engine/<moteur>
Contents/Resources/icon.icns copié depuis le pack client
Contents/Resources/client    lien symbolique vers ../../client
Contents/PkgInfo
```

Le lien `Contents/Resources/client` réconcilie deux exigences : le contrat pose `client/` à la
racine d'installation — qui **est** le `.app` — alors que le moteur cherche ses ressources là
où un bundle en pose. Un lien relatif évite de dupliquer le pack et survit au déplacement du
bundle.

C'est ce qui permet à deux clients de coexister : deux `.app` de noms différents, deux
identifiants de bundle différents, donc deux entrées distinctes dans le Dock et dans
Launchpad. Le lien physique est refait à chaque installation, puisque remplacer le moteur crée
un nouvel inode.

Du bundle générique publié par la CI (`app-tar-gz`), seul le Mach-O est conservé : l'`Info.plist`
et l'icône génériques n'ont aucun intérêt une fois le bundle du tenant écrit.

### Windows : le `.lnk` est écrit en Rust

`windows_lnk.rs` sérialise le format MS-SHLLINK directement. Ni PowerShell — un installeur qui
lance `powershell.exe` est bloqué par la moitié des antivirus grand public — ni COM, qui ferait
entrer tout le binding Windows dans un binaire qui doit rester minuscule. Le chemin cible est
écrit dans le `LinkInfo` en ANSI **et** en UTF-16, le dossier d'installation contenant le nom
de la session Windows.

Un raccourci qui échoue n'interrompt jamais l'installation : le client est installé et lancé
quand même, le problème est simplement écrit dans la console.

## Constantes de build

Le bootstrap ne connaît que l'UUID du tenant. L'adresse du panel, elle, doit bien être figée
quelque part :

| Variable | Défaut | Rôle |
|---|---|---|
| `LUUXCRAFT_PANEL_URL` | `https://luuxcraft.fr` | origine du panel, **sans** `/api` |
| `PANEL_URL` | — | accepté en second, c'est le nom de la variable de dépôt de la CI |
| `LUUXCRAFT_BUNDLE_ID_PREFIX` | `com.luuxcraft.launcher` | préfixe du `CFBundleIdentifier` macOS |

Un `/` ou un `/api` final est retiré : l'ancien pipeline injectait `https://panel/api`, et une
URL doublée en `/api/api/v1` ne produirait qu'un 404 incompréhensible pour le joueur.

## Construire

```bash
cargo build --release                                  # cible de la machine
cargo build --release --target x86_64-pc-windows-msvc  # exe
cargo build --release --target x86_64-unknown-linux-gnu
```

Le binaire produit s'appelle `luuxcraft-bootstrap` (`luuxcraft-bootstrap.exe` sous Windows).
C'est lui que la CI téléverse au panel avec le rôle `bootstrap`, au format `exe` (windows) ou
`bin` (linux, darwin).

macOS attend un binaire **universel** : la route de téléchargement du panel ne connaît que
l'OS, jamais l'architecture du visiteur.

```bash
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo -create -output luuxcraft-bootstrap \
  target/x86_64-apple-darwin/release/luuxcraft-bootstrap \
  target/aarch64-apple-darwin/release/luuxcraft-bootstrap
```

Ce crate n'appartient pas au workspace de `src-tauri/` : il a son propre `Cargo.lock` et ne
partage aucune dépendance avec le moteur.

```bash
cargo test --manifest-path bootstrap-installer/Cargo.toml
```

## Dépendances

`ureq` (HTTP bloquant, TLS rustls embarqué — ni tokio ni magasin de certificats du système),
`serde`/`serde_json`, `sha2`, `uuid`, `dirs`, et `zip` + `flate2`/`tar` pour les trois formats
de bundle moteur. Les trois décodeurs sont compilés sur toutes les plateformes : le format
vient du manifeste et non de l'OS, et une incohérence côté panel doit produire un message
clair plutôt qu'un binaire incapable de la lire.

## Limites connues

- L'exécutable du moteur n'est pas nommé dans le manifeste : il est **détecté** dans le bundle
  décompressé (le `.exe` d'un `exe-zip`, le fichier de `Contents/MacOS` d'un `app-tar-gz`).
  S'il y en a plusieurs, celui dont le nom correspond au client l'emporte, sinon le plus gros.
- macOS : un `Contents/Frameworks` présent dans le bundle publié ne serait pas repris. Le
  moteur tauri n'en embarque pas, la webview venant du système.
- Le pack client n'est jamais élagué : un fichier retiré du manifeste reste sur le disque. Il
  n'est plus référencé par rien, et supprimer des fichiers chez le joueur demande plus de
  précautions que d'en laisser un orphelin.
