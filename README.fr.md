# copycopy

Gestionnaire de presse-papier minimaliste. Capture réelle, popup **iced**.

C'est un **résident** : il capture en permanence et n'ouvre sa fenêtre qu'à la
demande. Sans ça il ne retiendrait que ce qu'on copie pendant qu'on le regarde —
c'était le vrai trou de la première version.

Historique en mémoire pour l'instant : **la persistance SQLite n'est pas encore
écrite**, tout est perdu à l'arrêt du résident.

*An English version of this document is available in [README.md](README.md).*

## Lancer

```bash
copycopy                 # démarre le résident, fenêtre fermée
copycopy --open          # démarre et ouvre la fenêtre
copycopy --show          # demande au résident d'ouvrir
copycopy --quit          # arrête le résident
copycopy --demo          # données multilingues
copycopy --backend wayland|x11|poll
copycopy --screenshot out.png --for 10   # capture la fenêtre puis quitte
copycopy --hidden ...    # démarre sans fenêtre, même en mode capture

copycopy --headless --for 20  # capture en console, sans interface
cargo run -p copycopy-platform --example fake_owner -- "texte" Firefox 3
```

Navigation : `↑↓` / `Ctrl-N` `Ctrl-P`, `PageUp/Down`, `Enter` copier,
`Ctrl-B` épingler, `Ctrl-D` supprimer, `Esc` fermer. `Home`/`End` restent au
champ de recherche — dans une zone de saisie, c'est le curseur qu'on attend.

## Ouverture : raccourci global et IPC

Par défaut **`Ctrl+Alt+V`** (`Cmd+Shift+V` sur macOS), modifiable dans
`~/.config/copycopy/copycopy.conf`.

| Système | Mécanisme | État |
|---|---|---|
| Windows | `RegisterHotKey` | écrit, jamais exécuté |
| macOS | Carbon `RegisterEventHotKey` | écrit, jamais exécuté |
| X11 | `XGrabKey` | vérifié, déclenchement compris |
| Wayland | **aucun raccourci global côté client** | repli par l'IPC |

Contrainte de thread : le gestionnaire doit naître sur le thread principal
(macOS) et sur celui qui porte la boucle d'événements (Windows). Il est donc
créé dans `boot()`, qui remplit les deux conditions, et gardé vivant dans
l'état — le lâcher désenregistrerait le raccourci.

**Repli universel, et seule voie sous Wayland** : définir dans son compositeur
un raccourci qui lance `copycopy --show`. Ce second process détecte le résident
par un socket local (`interprocess`), lui transmet la commande et s'efface. Le
même mécanisme garantit qu'un seul daemon capture.

Le portail `org.freedesktop.portal.GlobalShortcuts` reste à faire : c'est la
voie propre sous Wayland, et la seule qui règle aussi le focus — un client
Wayland ne peut pas se donner le focus lui-même, il lui faut un jeton
`xdg-activation-v1` que seul le portail délivre.

### Sous WSL, le raccourci ne peut pas marcher depuis Windows

`XGrabKey` ne voit que les touches qui atteignent le serveur X de WSLg. Un
`Ctrl+Alt+V` frappé dans une application Windows est traité par Windows et ne
lui parvient jamais. C'est structurel, aucune modification du code n'y changera
rien. La réponse est un binaire Windows natif — qui supprime au passage le pont
presse-papier de WSLg.

Deux autres limites de WSLg, découvertes à la dure :

- **Les formes de curseur ne sont jamais appliquées.** Les poignées de
  redimensionnement demandent `ResizingHorizontally` et consorts, le champ de
  recherche demande un curseur texte ; aucun n'apparaît. Le code est correct —
  `MouseArea` ne remplace le curseur que si son enfant renvoie
  `Interaction::None`, ce que fait un `Space`. C'est WSLg qui ignore la demande.
- **La fenêtre est une surface Wayland, pas X11.** Le process détient un
  descripteur `wayland` et n'apparaît jamais dans `_NET_CLIENT_LIST`. XTEST peut
  donc piloter le clavier, ce qui a permis de tester le raccourci global, mais
  pas le pointeur sur cette fenêtre — le glisser pour déplacer ou redimensionner
  est donc intestable ici.

## La fenêtre

Sans décorations, toujours au-dessus. Elle se ferme sur `Esc` et à la perte du
focus — c'est le comportement d'une palette, pas d'une fenêtre de document.
**Pas de maximiser** : une liste de presse-papier ne gagne rien en plein écran ;
ce qu'on veut, c'est la déplacer, la redimensionner, et la retrouver où on l'a
laissée.

- **Angles droits, volontairement.** Les coins arrondis ont été essayés et
  fonctionnent techniquement (il faut, en plus de `transparent(true)`, un
  `style()` d'application au `background_color` transparent — sinon iced peint
  la couleur du thème sur toute la surface et l'arrondi se retrouve posé sur un
  rectangle opaque). Mais sur une fenêtre sans décorations, les angles laissent
  voir le bureau derrière, ce qui ressort comme un liseré noir. La fenêtre est
  donc opaque jusqu'au bord, avec un filet de 1 px.
- **Déplacement** : glisser n'importe où sur l'en-tête (`window::drag`). Le
  `mouse_area` laisse l'enfant capturer en premier, donc un clic dans le champ
  de recherche ne déplace pas la fenêtre.
- **Redimensionnement** : huit bandes de 6 px sur le pourtour, invisibles
  puisqu'elles laissent voir le fond de la carte, posées **par la mise en page**
  et non en calque — un `stack` par-dessus empêche les rangées de se redessiner
  (voir plus bas).
- **Géométrie** : taille et position retenues, mais écrites séparément. Certains
  environnements (WSLg) ne rapportent jamais de position réelle et renverraient
  0,0 : la fenêtre s'ouvrirait dans un coin au lieu d'être centrée. La position
  n'est donc écrite que si un déplacement a été observé.

## Le copier

`Enter` ou double-clic → le contenu part dans le presse-papier et la fenêtre se
ferme. **Pas de « Copié »** : la disparition de la fenêtre est la confirmation,
et un toast supposerait de garder la fenêtre ouverte alors qu'on veut justement
être revenu dans son application, en train de coller.

Le vrai objectif reste le **collage** automatique (simuler `Ctrl+V` après
fermeture), à faire : `SendInput` sous Windows, XTEST sous X11, CGEvent sous
macOS avec autorisation Accessibilité — et impossible sous Wayland sans le
portail RemoteDesktop.

## Pourquoi iced et pas egui

egui a été le premier choix, le plus léger, puis écarté sur un seul critère :
**il ne sait pas rendre les emoji en couleur**. Ce n'est pas un réglage raté —
epaint n'a aucune notion de glyphe couleur (ni COLR, ni CBDT, ni sbix) et
sélectionne la police caractère par caractère, ce qui casse en prime les
séquences ZWJ : `👨‍👩‍👧‍👦` sortait en quatre glyphes monochromes.

iced rend son texte avec cosmic-text (rustybuzz + fontdb + swash), qui règle
les deux : couleur, ligatures ZWJ, teintes de peau, et **fallback de police
automatique** — les 187 lignes d'énumération manuelle des chemins par OS ont
disparu.

Le prix, mesuré à périmètre égal : **14,98 Mio pour la version egui contre
19,70 Mio pour iced**. Après suppression des crates de spike, le binaire iced
est retombé à **14,31 Mio** : l'un d'eux demandait `iced` avec la feature
`image`, et cargo unifie les features à l'échelle du workspace — le binaire
principal traînait tout le décodage d'images pour rien. À périmètre réellement
égal, l'écart est donc négligeable. Démarrage, mémoire et CPU au repos sont
équivalents. L'UI elle-même est plus courte : 672 lignes contre 1 064,
`fonts.rs` étant passé de 187 à 38.

### La liste doit être virtualisée à la main

`scrollable` ne virtualise pas : il construit tous les enfants à chaque `view()`.
Mesuré, redessin forcé à chaque frame :

| entrées | liste pleine | virtualisée |
|---|---|---|
| 1 000 | 59 fps · view 2,20 ms | 58 fps · **view 0,08 ms** |
| 5 000 | **0 fps** (inutilisable) | 59 fps · view 0,16 ms |
| 20 000 | **0 fps** | 59 fps · view 0,42 ms |
| 100 000 | — | **59 fps** · view 1,38 ms |

Les rangées étant de hauteur fixe, on sait exactement lesquelles sont visibles :
une tranche encadrée de deux espaceurs qui préservent la hauteur totale
(`view.rs`, ~15 lignes). Le plafond disparaît, l'historique illimité devient
envisageable. La liste filtrée est aussi tenue dans l'état et recalculée
seulement quand la requête change — jamais dans `view()`.

Deux pièges dans cette virtualisation, tous deux corrigés : le pas d'une rangée
à l'autre doit valoir **exactement** `ROW_H` (ni `spacing` sur la colonne, ni
marge verticale), sinon les espaceurs dérivent par rapport à la position de
défilement réelle. `--scroll N` sert à le vérifier dans une capture.

### Défaut corrigé, sans avoir été expliqué

La ligne « source · âge » ne se dessinait pas environ 3 fois sur 4 quand la
fenêtre était ouverte après le démarrage du résident. Six exécutions
consécutives de ce scénario la dessinent désormais à chaque fois.

La cause probable : les rangées ne sont plus des indices dans l'historique en
mémoire — `refilter()` matérialise un `Vec<ClipItem>` et la liste est
reconstruite à partir de lui, ce qui a changé la façon dont iced compare l'arbre
de widgets. Ça n'a jamais été isolé : à considérer comme corrigé mais non
expliqué, et c'est là qu'il faudra regarder si ça revient.

### Piège de rendu : `stack` fige ce qu'il recouvre

Un calque posé sur une rangée pour y dessiner un fondu de fin de ligne
empêchait cette rangée de se redessiner : le texte restait figé sur son premier
rendu, puis disparaissait. Diagnostiqué en traçant `view()` — qui produisait
bien la bonne chaîne — puis en retirant le calque, seule variable changée. Le
fondu a donc été abandonné et l'aperçu est coupé net au bord, à une abscisse
constante. À reprendre avec un widget sur mesure si le besoin revient.

### Trois règles de mise en page

1. **Centrage vertical.** Chaque bande (en-tête, rangée, pied) est un
   `container` de hauteur fixe qui centre son contenu avec `center_y`. Un
   `row.align_y(Center)` ne suffit pas : il aligne les enfants entre eux, mais
   laisse la rangée collée en haut de son conteneur.
2. **Pas de calque.** Voir ci-dessus : tout ce qui doit se superposer passe par
   la mise en page, jamais par `stack`.
3. **La rangée et sa surbrillance sont deux choses.** La rangée garde exactement
   `ROW_H` — le calcul de virtualisation en dépend — et c'est le fond, à
   l'intérieur, qui est rétréci de `ROW_GAP`. D'où l'espace entre deux
   surbrillances, sans toucher au pas de la liste.

La loupe de la barre de recherche est dessinée au `canvas`, pas prise dans une
police : nette à toutes les échelles et indépendante des glyphes disponibles.

## Capture

Un seul trait (`ClipboardBackend`), un backend natif par système, et le sondage
comme dernier recours — jamais comme choix par défaut.

| Système | Mécanisme | Réveil | Source de la copie | État |
|---|---|---|---|---|
| **Linux / X11** | XFixes `SelectionNotify` | événementiel | `WM_CLASS`, sinon `_NET_WM_PID` | **testé ici** |
| **Linux / Wayland** | `ext-data-control-v1`, repli `zwlr-data-control-v1` | événementiel | *indisponible* | écrit, **jamais exécuté** |
| **Windows** | `AddClipboardFormatListener` sur fenêtre message-only | événementiel | `GetClipboardOwner` → `QueryFullProcessImageNameW` | écrit, **jamais exécuté** |
| **macOS** | `NSPasteboard.changeCount` (200 ms) | sondage | *indisponible* | écrit, **jamais exécuté** |
| *repli universel* | `arboard` (200 ms) | sondage | *indisponible* | testé ici |

Le sondage sous macOS n'est pas un pis-aller : Apple n'expose aucune
notification de changement, `changeCount` est l'API.

### Ce qui est réellement vérifié

- X11 : exécuté et testé (texte, URL, code, CJK, arabe, emoji, PNG, fichiers,
  INCR sur 400 Kio, déduplication, attribution de la source).
- Wayland, Windows, macOS : **type-checkés contre leurs vraies cibles**
  (`cargo check --workspace --target x86_64-pc-windows-msvc` et
  `aarch64-apple-darwin`). Mais jamais exécutés : cette machine est un WSL2. À
  valider sur les vraies plateformes avant d'y croire.
- Le choix du backend est testé en conditions réelles : sous WSLg,
  `WAYLAND_DISPLAY` est présent mais le compositeur n'expose aucun
  data-control, et on bascule proprement sur X11 en disant pourquoi.

### Le trou noir : GNOME sous Wayland

Wayland n'expose pas le presse-papier aux applications en arrière-plan : le
`wl_data_device` standard exige le focus clavier. Il faut un protocole
data-control — que **GNOME n'implémente pas**, ni en `ext` ni en `wlr`. KDE,
Sway, Hyprland et COSMIC l'exposent.

Sur GNOME/Wayland, il n'existe donc aucun moyen de surveiller le presse-papier
depuis un process en arrière-plan sans extension ni portail. Le repli par
sondage n'y change rien : il bute sur la même limite. C'est une limite de la
plateforme, pas du projet, et elle mérite d'être annoncée plutôt que masquée.

### Formats et secrets

| | X11 | Wayland | Windows | macOS |
|---|---|---|---|---|
| Texte | `UTF8_STRING`, `text/plain;charset=utf-8`, `STRING` | idem | `CF_UNICODETEXT` | `NSPasteboardTypeString` |
| Image | `image/png` | `image/png` | `PNG`, sinon `CF_DIB` → PNG | `…TypePNG`, sinon TIFF → PNG |
| Fichiers | `text/uri-list` | `text/uri-list` | `CF_HDROP` | `…TypeFileURL` par élément |
| Gros contenus | **INCR** | flux sur socket | `GlobalSize` | `NSData` |

**Les contenus marqués secrets ne sont jamais capturés**, selon la convention de
chaque plateforme :

- X11 / Wayland : cible `x-kde-passwordManagerHint` ou
  `org.nspasteboard.ConcealedType` ;
- Windows : format `ExcludeClipboardContentFromMonitorProcessing`, et
  `CanIncludeInClipboardHistory` à 0 ;
- macOS : types `org.nspasteboard.ConcealedType`, `AutoGeneratedType`,
  `TransientType`.

C'est la règle la plus importante du projet : un gestionnaire de presse-papier
qui retient les mots de passe est un logiciel malveillant par accident.

### Fiabilité de la capture sous WSLg

Le pont presse-papier de WSLg reprend la sélection derrière chaque application,
avec un décalage. Sur une rafale de cinq copies enchaînées d'applications qui ne
vivent que 2 s, il arrive qu'une entrée soit perdue (2 essais sur 3, jamais la
même). C'est propre à cet environnement de test — à revalider sur un vrai
bureau Linux avant d'en conclure quoi que ce soit.

### Différence de comportement connue

Le backend par sondage capture ce qui se trouve **déjà** dans le presse-papier
au démarrage ; les backends événementiels non, ils n'apprennent l'existence d'un
contenu qu'au changement suivant. À uniformiser (lecture initiale au démarrage).

### Deux pièges rencontrés, et corrigés

1. **Notifications jetées.** Attendre le `SelectionNotify` d'une lecture en
   ignorant les autres événements fait perdre les `XFixesSelectionNotify`
   arrivés pendant ce temps — donc les copies en rafale. Elles sont maintenant
   mises en file et traitées juste après.
2. **Notifications en double.** Le pont presse-papier de WSLg (et tout
   gestionnaire tiers) reprend la sélection juste après l'application source :
   on reçoit deux à trois notifications par copie. On ne renvoie la suivante que
   si elle apporte l'attribution qui manquait.

## Tester la capture sans y croire sur parole

```bash
cargo build --release --workspace --examples
./target/release/copycopy --headless --for 12 &
./target/release/examples/fake_owner "Un" Firefox 2
./target/release/examples/fake_owner "Deux" DBeaver 2
```

`fake_owner` est un vrai client X11 qui annonce `WM_CLASS` et `_NET_WM_PID` et
sert la sélection — contrairement à `xclip`, qui n'expose ni l'un ni l'autre et
ne permet donc pas de valider l'attribution.

`press_key` synthétise une frappe via l'extension XTEST : c'est ainsi que le
raccourci global a été testé sans clavier physique.

```bash
cargo run -p copycopy-platform --example press_key -- ctrl alt v
```

## Structure

```
crates/
├── core/       modèle, historique borné, dédup, classification — 0 dep OS/UI
├── platform/   capture : x11.rs, wayland.rs, windows.rs, macos.rs,
│               poll.rs (repli), setter.rs (écriture)
└── app/        l'UI
    ├── main.rs   daemon iced, abonnements, clavier, fenêtre
    ├── config.rs configuration (raccourci, géométrie)
    ├── hotkey.rs raccourci global
    ├── ipc.rs    instance unique et commande --show
    ├── theme.rs  palette : tout le look se règle ici
    ├── fonts.rs  polices d'appoint (systèmes sans CJK/emoji)
    └── view.rs   popup, liste virtualisée, poignées de redimensionnement
```

`core` ne connaît ni l'OS ni l'UI. Le pari du découpage a été vérifié au moment
de changer de toolkit : **`core` et `platform` n'ont pas bougé d'une ligne**
(1 880 lignes), seul `app` a été réécrit.

## Vérifier soi-même les autres OS

```bash
rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin
cargo check --workspace --target x86_64-pc-windows-msvc
cargo check --workspace --target aarch64-apple-darwin
cargo run -p copycopy-platform --example wl_globals   # globaux Wayland exposés
```

## Prochaine étape

Persistance SQLite avec FTS5 pour la recherche — l'historique ne survit pas
encore à l'arrêt du résident.

## Licence

MIT. Voir [LICENSE](LICENSE).
