# copycopy

Gestionnaire de presse-papier minimaliste. Capture réelle, popup **iced**.

C'est un **résident** : il capture en permanence et n'ouvre sa fenêtre qu'à la
demande. Sans ça il ne retiendrait que ce qu'on copie pendant qu'on le regarde —
c'était le vrai trou de la première version.

L'historique est conservé dans une base SQLite compilée dans le binaire, donc
rien à installer sur aucun système. **Mode portable** : placez un
`copycopy.conf` à côté de l'exécutable et la configuration, la base et les images
vivent toutes dans ce dossier, sans rien toucher sur la machine hôte.

*An English version of this document is available in [README.md](README.md).*

## Installer

Des paquets prêts à l'emploi sont attachés à chaque
[release](../../releases). Rien n'est signé — c'est à quoi ressemble un
certificat qu'on ne paie pas : Windows affiche un avertissement SmartScreen
(*Informations complémentaires* → *Exécuter quand même*) et macOS refuse la
première ouverture (clic droit sur l'app → *Ouvrir*, ou
`xattr -d com.apple.quarantine /Applications/copycopy.app`).

| Système | Fichier | Remarques |
|---|---|---|
| Windows | `copycopy-windows-x86_64.exe` | installation par utilisateur, sans administrateur ; propose l'entrée d'ouverture de session |
| Windows | `…-portable.zip` | l'exécutable seul ; un `copycopy.conf` à côté et rien ne touche la machine |
| Linux | `copycopy-linux-x86_64.AppImage` | `chmod +x`, puis lancer |
| Linux | `copycopy-linux-x86_64.deb` | Debian et Ubuntu, avec l'entrée de bureau et l'icône |
| macOS | `copycopy-macos-universal.dmg` | un seul binaire pour Apple Silicon et Intel |

Compiler depuis les sources ne demande rien d'autre qu'une toolchain Rust —
SQLite est compilé dans le binaire. Les paquets sont fabriqués par
[`.github/workflows/release.yml`](.github/workflows/release.yml) sur un tag, et
les descriptions d'installeurs vivent dans [`packaging/`](packaging).

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
copycopy --settings ...  # ouvre sur les réglages, pour les vérifier en capture
copycopy --filter code   # ouvre filtré sur un type, pour vérifier la barre de filtres
copycopy --autostart on|off   # lance ou non copycopy à l'ouverture de session, puis sort

copycopy --headless --for 20  # capture en console, sans interface
cargo run -p copycopy-platform --example fake_owner -- "texte" Firefox 3
```

Navigation : `↑↓` / `Ctrl-N` `Ctrl-P`, `PageUp/Down`, `Enter` copier,
`Ctrl-B` épingler, `Suppr` ou `Ctrl-D` supprimer — accessible aussi par la croix
qui apparaît sur la rangée survolée et sur la sélectionnée. `Esc` ferme la
fenêtre ; `Ctrl-Q` (`Cmd-Q` sur macOS) arrête complètement le résident, comme
`--quit`. L'engrenage ouvre les réglages dans le panneau de droite — thème,
raccourci, collage automatique, démarrage, dossier des données et
*Quitter copycopy* — et `Esc` les referme avant la fenêtre.

**Collage automatique**, désactivé par défaut. Une fois une entrée copiée,
copycopy rend le focus à l'application où vous étiez et appuie sur `Ctrl+V` à
votre place : l'entrée arrive là où était votre curseur. Sous Windows par
`SendInput`, sous X11 par XTEST ; refusé sous Wayland, qui exige pour cela le
portail RemoteDesktop, et pas encore disponible sous macOS. Simuler des frappes
dans une autre application, ça s'active, ça ne se découvre pas.

**Démarrage avec la session**, désactivé par défaut. L'interrupteur écrit une
entrée par utilisateur — la clé `Run` sous `HKCU` sous Windows, sans droits
administrateur, un fichier `.desktop` dans `~/.config/autostart` sous Linux, un
agent de lancement sous macOS — et copycopy attend alors en fond dès l'ouverture
de session, sans fenêtre, pour que le raccourci trouve quelqu'un au premier
appui. L'entrée contient un chemin absolu, réécrit à chaque démarrage : déplacer
l'exécutable, ce à quoi le mode portable invite, ne laisse pas une entrée morte
derrière soi. Retirez-la depuis le gestionnaire des tâches et copycopy s'en
aperçoit au démarrage suivant, plutôt que de prétendre le contraire.

La croix de l'en-tête masque la fenêtre, comme `Esc` : elle ne quitte
jamais, pour que personne n'arrête la capture en visant le coin habituel.

**Les filtres par type** occupent une barre sous la recherche — Tout, Texte,
Code, URL, Images, Fichiers — chacun avec le nombre d'entrées qu'il afficherait
pour la recherche en cours. Ils se combinent avec elle : *Code* et `select` ne
listent que le code qui contient « select ». `Ctrl-1` à `Ctrl-6` changent de
filtre sans quitter le champ de recherche, et le filtre revient à *Tout* à chaque
ouverture de la fenêtre. `Home`/`End` restent au champ de recherche — dans une zone de saisie,
c'est le curseur qu'on attend.

**Sous Windows, aucune console ne s'ouvre.** Un résident n'a pas à en posséder
une, et la fermer arrêtait la capture sans prévenir. Lancé depuis un terminal,
la sortie va toujours dans ce terminal ; lancé autrement — menu Démarrer,
ouverture de session — elle va dans `copycopy.log` à côté de la base, remis à
zéro au-delà d'un mégaoctet.

## Ouverture : raccourci global et IPC

Par défaut **`Ctrl+Alt+V`** (`Cmd+Shift+V` sur macOS), modifiable dans
`~/.config/copycopy/copycopy.conf`.

| Système | Mécanisme | État |
|---|---|---|
| Windows | `RegisterHotKey` | vérifié, utilisé au quotidien |
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

- **Coins arrondis**, 12 px, sur une fenêtre transparente. Trois choses doivent
  s'accorder, sinon l'effet s'effondre : la fenêtre est créée `transparent`, le
  `style()` d'application efface la surface avec un `background_color`
  transparent — sinon iced peint la couleur du thème partout et la courbe se
  retrouve posée sur un rectangle opaque — et rien d'autre que la carte n'est
  jamais dessiné dans l'angle, ce que la bande de redimensionnement de 10 px
  garantit déjà.

  Encore faut-il que le bureau compose, sans quoi l'angle n'est qu'un trou :
  DWM sous Windows, Wayland et X11 avec un compositeur le font tous. **Exécuté
  sous Windows : les coins sont arrondis et ce qui se trouve derrière la fenêtre
  se voit au travers.** Sous WSLg, le même binaire les peint en noir — c'est ce
  qu'un essai précédent avait constaté ici et pris pour un défaut ; c'est
  l'établi, pas le code. Une session X11 sans compositeur n'a rien avec quoi
  fondre et se comporte pareil ; `corners = square` revient au rectangle opaque,
  et c'est la réponse dans ce cas.
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
- **Panneau de détail** : la partie droite montre l'entrée sélectionnée en
  entier — texte jusqu'à 10 000 caractères, police à chasse fixe pour le code,
  images réduites pour tenir, listes de fichiers. Il est construit quand la
  sélection arrive sur une autre entrée, jamais dans `view()` : lire un fichier
  image ou parcourir un long texte à chaque frame coûterait la fluidité. La
  fenêtre s'ouvre en 980×560, et une largeur retenue d'avant le panneau est
  élargie à 860.

### Reconnaître un langage, et le colorer

Une entrée de presse-papier n'a pas d'extension de fichier, et syntect — ce vers
quoi tout le monde se tourne — ne reconnaît un langage qu'à un shebang ou à un
modeline. Un fragment collé n'a ni l'un ni l'autre : il commence au milieu d'une
fonction. `core/lang.rs` devine donc par le contenu, sur un échantillon borné,
et répond `None` dès qu'il n'est pas sûr ; colorer un extrait dans le mauvais
langage est pire que ne pas le colorer.

La coloration elle-même, `core/highlight.rs`, est un scanner et non un parseur —
c'est le principe, pas un raccourci. Un fragment ne satisfait aucune grammaire,
il n'y a donc rien à parser, et un scanner ne peut pas être désarçonné par la
moitié du fichier qu'on n'a pas copiée. Il trouve six catégories : commentaires,
chaînes, nombres, mots-clés, balises et clés. Six parce que la palette a
justement cinq teintes de badge plus `faint` à leur consacrer — aucune couleur
n'a été inventée pour aucun thème, ce qui fait que la coloration suit un
changement de thème toute seule, Matrix compris.

Deux pièges, devenus deux tests. Une lifetime Rust n'est pas une chaîne :
`&'a str` se referme sur la lifetime suivante neuf octets plus loin, donc une
limite de longueur l'avale sans broncher — un littéral de caractère se reconnaît
à sa *forme*, un caractère ou une échappée. Et un guillemet non refermé s'arrête
en fin de ligne, sans quoi l'apostrophe d'un commentaire français peint tout ce
qui suit.

Le tout tourne une fois par changement de sélection, dans `Preview::of`, jamais
dans `view()` — la règle qui tient déjà la liste filtrée.

## Le copier

`Enter` ou double-clic → le contenu part dans le presse-papier et la fenêtre se
ferme. La ligne copiée passe d'abord en vert pendant 160 ms : la disparition de
la fenêtre confirme qu'il s'est passé *quelque chose*, mais pas *laquelle* des
entrées est partie. Rien de plus — un toast supposerait de garder la fenêtre
ouverte alors qu'on veut justement être revenu dans son application, en train de
coller.

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

### Défaut connu, toujours ouvert

La ligne « source · âge » d'une rangée ne se dessine pas toujours. Elle a été
déclarée corrigée après six passages d'un même scénario : c'était prématuré, ce
scénario avait simplement cessé de la déclencher. Le défaut subsiste, de façon
intermittente, sur d'autres chemins — des entrées fraîchement capturées perdent
la ligne dans certains essais et la gardent dans d'autres, à données identiques.

Écartés, chacun en l'isolant : `clip`, le cadre de redimensionnement, un calque
`stack`, une source vide, le type de contenu, et une vignette d'image dans
l'emplacement du badge. `view()` produit toujours la bonne chaîne — vérifié par
traçage — et l'en-tête comme le pied, hors du `scrollable`, se dessinent
toujours.

À mettre en balance : **ce défaut n'a jamais été observé que sous WSLg, par
l'outillage, jamais par quelqu'un qui se sert de l'application.** Le même établi
a déjà fabriqué quatre mirages — les formes de curseur jamais appliquées, le
raccourci invisible depuis Windows, la surface Wayland que XTEST ne pilote pas,
et les coins de fenêtre noirs qui sortent propres sous Windows. À considérer
comme un cinquième tant que personne ne l'a vu sur une plateforme cible, et à ne
pas reprendre sans ça.

**La suite est un cas minimal reproductible**, une vingtaine de lignes avec un
`scrollable` dont les rangées portent deux textes empilés, pour savoir s'il faut
signaler un bug d'iced ou corriger un mauvais usage. Le traquer à l'intérieur de
l'application a coûté plusieurs sessions et n'a produit que des éliminations.

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

`echo` couvre l'autre sens, l'écriture. Il pose un contenu sur le presse-papier
par le `Setter` même qu'utilise l'application, puis écoute avec le vrai
observateur et dit si chaque capture a été reconnue comme notre propre écriture :

```bash
cargo run --release -p copycopy-platform --example echo -- shot.png
```

Une image revient ré-encodée : ses octets diffèrent de ceux qui sont entrés, et
le hachage du contenu ne peut pas la reconnaître. Sans ce contrôle, chaque
recopie atterrissait dans l'historique comme une nouvelle entrée.

`targets` répond à l'autre question : pourquoi une copie n'a produit aucune
entrée. Il liste ce que le propriétaire du presse-papier annonce et ce que notre
lecteur tire de chaque format, chronométrage compris :

```bash
# copiez quelque chose, puis :
cargo run --release -p copycopy-platform --example targets
```

Il existe parce que le backend est muet sur ce chemin : si un propriétaire
annonce une image qu'il ne parvient ensuite pas à livrer, la copie entière est
abandonnée, alternatives texte comprises, et rien n'est journalisé.

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

## Persistance et recherche

SQLite via la feature `bundled` de rusqlite : la base est embarquée dans le
binaire, aucune bibliothèque système à installer nulle part. Les images sont
écrites en fichiers à côté, et leurs octets ne sont lus qu'au moment de copier —
relire chaque PNG pour dessiner une liste de texte ferait taper le disque à
chaque frappe. Une entrée lâche ses octets dès que son fichier est écrit : rien
n'est gardé deux fois.

Une image porte le nom de son hash de contenu, et deux lignes n'en partagent
jamais une : un fichier que plus rien ne désigne est donc définitivement hors
d'atteinte. Supprimer une entrée et élaguer l'historique emportent le fichier
avec la ligne, et le démarrage balaie ce qui traîne — d'un arrêt brutal entre
l'écriture du fichier et l'insertion de sa ligne, et de toutes les versions qui
élaguaient les lignes sans toucher au disque.

La recherche bascule à trois caractères. En dessous elle filtre en mémoire la
fenêtre chargée, ce qu'un index trigram ne sait pas faire. À partir de trois elle
interroge la base, donc les entrées plus anciennes que cette fenêtre sont
trouvées aussi. Tokeniser **trigram** plutôt que celui par défaut : on cherche un
fragment, pas un mot, et le trigram indexe aussi les langues sans espaces, donc
le CJK reste atteignable. Le seuil change de moteur, pas seulement de finesse —
deux caractères et trois ne filtrent pas pareil.

Les épinglés mènent la liste, la récence ordonne à l'intérieur de chaque groupe,
et l'élagage ne les supprime jamais. Les rangées affichent la taille du contenu
quand il y en a plus que la ligne n'en montre — comptée sur le contenu complet,
pas sur l'aperçu, lui-même plafonné.

## Crédits

Les deux palettes de couleurs sont empruntées à des thèmes existants, adaptées
et non recopiées sous forme de code :

- **Sombre** — [Base16 Purpledream](https://github.com/tinted-theming/schemes/blob/spec-0.11/base16/purpledream.yaml)
  de malet, chez Tinted Theming (MIT). Les valeurs du schéma sont reprises telles
  quelles.
- **Clair** — [Aalto Light](https://github.com/emacs-jp/replace-colorthemes/blob/master/aalto-light-theme.el)
  de Jari Aalto, porté par Syohei Yoshida (GPL-3.0+). Seules des valeurs de
  couleur sont reprises, et le fond est assombri par rapport à la crème d'origine.

`crates/app/src/theme.rs` indique la provenance de chaque valeur.

## Licence

MIT. Voir [LICENSE](LICENSE).
