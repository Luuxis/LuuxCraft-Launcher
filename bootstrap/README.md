# Bootstrap Windows

L'installeur de première mise en place, sous Windows. Un petit exécutable Rust
sans tauri ni webview : c'est le premier fichier que le joueur télécharge, il
doit arriver en quelques secondes.

Linux et macOS n'en ont pas — ils s'installent par la commande `curl … | sh`
que le panel sert (`src/lib/installScript.ts` côté panel).

```text
bootstrap.exe
     │  lit son créneau d'identité (87 octets, en mémoire)
     ▼
  manifeste du serveur         GET /api/v1/launchers/<id>/manifest?os=&arch=
     │
     ├─► moteur      engine\<Nom du serveur>.exe     (vérifié par SHA-256)
     ├─► pack client client\client_config.json + icônes
     ├─► raccourcis  Bureau + menu Démarrer, au nom et au logo du serveur
     ▼
  lance le moteur, et ne revient jamais
```

## Ce qu'il installe, et où

```text
%LOCALAPPDATA%\Programs\<slug>\
  engine\<Nom du serveur>.exe    le moteur, cible des raccourcis
  client\                        l'identité du serveur, lue par le moteur
    client_config.json
    icon.png  icon.ico  icon.icns
  .engine-version                version + empreinte + nom de l'exécutable
  .pack-version
```

Sous `%LOCALAPPDATA%` et non sous `Program Files` : **aucun droit
administrateur** n'est demandé, et deux serveurs coexistent sans se disputer un
emplacement partagé. Le `slug` vient du panel (`launcher_configs.slug`), jamais
d'un calcul local — c'est lui qui sépare deux installations.

Les données de jeu et les comptes vivent ailleurs (`%APPDATA%\.<slug>`) et ne
sont **jamais** touchés : réinstaller ne fait rien perdre au joueur.

## Le créneau d'identité

Le binaire est compilé **une fois par (OS, architecture)** et publié vierge par
la CI. Il réserve dans ses données une zone de taille fixe, repérée par une
chaîne magique :

```text
[ LXCBOOTSTRAP-TENANT-V1 ][ longueur 1 o ][ valeur 64 o ]
              22 o                              87 o au total
```

Le panel en fait une copie par serveur, écrit l'UUID dans le créneau, et range
le résultat dans R2. Les deux moitiés du contrat :

| | |
|---|---|
| Panel | `src/lib/bootstrapSlot.ts` — trouve le repère, écrit la valeur |
| Bootstrap | `src/identity.rs` — relit la valeur, en mémoire |

### Pourquoi pas le nom du fichier

Le joueur renomme très souvent ce qu'il télécharge. `Mon-Serveur.exe` devenu
`setup.exe` doit continuer à installer le bon serveur ; l'identité doit donc
être **dans les octets**.

### Pourquoi pas la fin du fichier

C'est ce que faisait le modèle précédent — 32 octets collés en fin
d'exécutable, pendant le streaming du téléchargement. Deux défauts, tous deux
rédhibitoires :

1. **La signature.** Une signature Authenticode porte sur tout le fichier sauf
   sa table de certificats. Ajouter des octets après signature la casse, et
   c'est irréparable : il aurait fallu choisir entre l'identité et la signature.
2. **La relecture.** Une fois signé, un exécutable ne finit plus par ses propres
   données mais par la table de certificats. « Lire les 32 derniers octets »
   devient faux au moment précis où l'on signe.

Un créneau au milieu des données ne souffre d'aucun des deux : il est rempli
**avant** la signature, et lu en mémoire sans jamais ouvrir le fichier.

## Signature Authenticode

Il n'y a **aucun certificat sur ce projet aujourd'hui** ; rien n'est signé, ni
le moteur, ni le bootstrap. Ce qui suit décrit où l'étape se branche le jour où
il y en aura un, parce que l'ordre n'est pas négociable :

```text
modèle vierge (CI)  ──►  copie  ──►  injection  ──►  signature  ──►  R2  ──►  joueur
                                          ▲              ▲
                                          │              └── signtool / osslsigncode, horodaté
                                          └── le panel, une fois par serveur
```

**Signer le modèle publié par la CI ne sert à rien** : l'injection qui suit
casserait la signature. C'est l'objet **par serveur** qu'il faut signer, donc
après l'injection, donc pas dans le Worker — Cloudflare Workers n'est pas un
environnement de signature Windows.

Deux branchements possibles, à trancher le jour venu :

- **à la génération** — le panel dépose l'objet injecté, un service de signature
  (runner Windows, ou un service HSM/cloud) le reprend, le signe, et le réécrit
  au même emplacement. Le panel n'a rien à changer : il sert ce qui est là ;
- **à la création du client** — le panel demande la génération à la CI, qui
  injecte, signe et téléverse. Plus lourd : chaque création de serveur devient
  un run GitHub Actions.

La disposition R2 est faite pour l'un comme pour l'autre : un objet par
`(serveur, artefact de bootstrap)`, à l'emplacement
`launcher-bootstrap/<tenant>/<id de l'artefact>.exe`, que le panel relit tel
quel sans savoir s'il a été signé entre-temps.

Sans signature, SmartScreen affiche un avertissement au premier lancement. Ce
n'est pas propre au bootstrap : un moteur non signé aurait exactement le même.

## Construire

```bash
cargo build --release --manifest-path bootstrap/Cargo.toml
cargo test  --manifest-path bootstrap/Cargo.toml     # tourne aussi sous Linux
```

Une seule variable d'environnement, lue à la compilation :

| Nom | Défaut | Rôle |
|---|---|---|
| `LUUXCRAFT_PANEL_URL` (ou `PANEL_URL`) | `https://luuxcraft.fr` | Origine du panel, sans `/api` |

L'adresse du panel est compilée, l'identifiant du serveur non : le premier est
le même pour tous les clients d'un panel, le second change à chaque client. Les
mettre tous les deux dans le créneau obligerait à y réserver une URL entière
sans rien apporter.

Le crate est **hors du workspace** de `src-tauri/` : il ne doit rien partager
avec le moteur, sinon le binaire servi à chaque joueur pèserait des dizaines de
méga-octets.

## Publier

Le job `bootstrap` de `.github/workflows/deploy.yml` s'en charge : il construit,
**vérifie que le créneau apparaît exactement une fois**, puis téléverse vers le
panel comme n'importe quel artefact (`bootstrap-exe`, `windows`, `x86_64`).

Le contrôle d'unicité n'est pas décoratif : le panel refuse de publier un modèle
dont le repère apparaît zéro ou plusieurs fois (`bootstrap_invalid`, 502), et le
voir échouer dans la CI vaut mieux que de le découvrir sur un téléchargement.

## Ce qu'il ne fait pas

- **Il ne met pas le launcher à jour.** Les raccourcis pointent sur le moteur,
  pas sur lui. Le moteur se met à jour tout seul (`src-tauri/src/update.rs`).
- **Il ne lit aucune valeur du panel sans la vérifier.** Le slug doit
  correspondre à `[a-z0-9-]`, les empreintes à 64 hexadécimaux, les URL doivent
  être sur le panel compilé, et les fichiers du pack viennent d'une liste
  fermée. Les noms réservés de Windows (`CON`, `NUL`, `COM1`…) sont écartés
  avant de devenir des chemins.
- **Il ne lance jamais ce qu'il n'a pas vérifié.** Tout passe par un dossier
  temporaire, empreinte comparée, puis `rename` — atomique, puisque le
  temporaire est dans la racine d'installation.

## Le relancer

C'est sans danger et c'est la procédure de réparation : il ne retélécharge que
ce dont l'empreinte a changé, refait les raccourcis (ce qui rattrape un
changement de nom ou de logo), et ne touche ni aux comptes ni aux réglages.
