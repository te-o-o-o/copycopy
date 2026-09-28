<h1 align="center">copycopy</h1>

<p align="center">
  Un gestionnaire de presse-papier qui se souvient de tout ce que vous copiez —<br>
  sauf de vos mots de passe. Curieux, pas malveillant.
</p>

<p align="center"><sub>v0.1.0 · Rust · Windows, Linux, macOS · MIT</sub></p>

<p align="center">
  <a href="#fonctionnalités">Fonctionnalités</a> ·
  <a href="#installer">Installer</a> ·
  <a href="#utiliser">Utiliser</a> ·
  <a href="#où-ça-tourne">Où ça tourne</a> ·
  <a href="docs/notes.fr.md">Notes</a> ·
  <a href="README.md">English</a>
</p>

<table align="center">
  <tr>
    <td align="center"><img src="docs/light.png" width="420" alt="copycopy, thème clair : l'historique à gauche, un extrait Rust épinglé affiché en couleur à droite"><br><sub>Clair</sub></td>
    <td align="center"><img src="docs/dark.png" width="420" alt="copycopy, thème sombre : le même historique"><br><sub>Sombre</sub></td>
  </tr>
</table>

Texte, code, liens, images, fichiers : un petit résident garde tout ce que vous
copiez, et un raccourci le ramène. Anglais, 日本語, emoji 🎉 et code compris.

## Fonctionnalités

- **Capture tout** : texte, code, liens, images et fichiers — et, sous Windows
  et X11, l'application d'où ils viennent.
- **Recherche par n'importe quel fragment**, dans tout l'historique, pas
  seulement ce qui est à l'écran.
- **Filtres** par type — texte, code, URL, images, fichiers — avec
  `Ctrl+1`…`6`.
- **Aperçu complet** : le code en couleur, son langage reconnu d'après
  l'extrait seul ; les images, y compris les fichiers image copiés dans le
  Finder ou l'Explorateur.
- **Épinglez** ce qui resert, supprimez le reste.
- **Collage automatique** (optionnel) : l'entrée arrive là où était votre
  curseur. Partout sauf sous Wayland.
- **Démarre avec la session** (optionnel), en silence, en arrière-plan.
- **Thèmes clair et sombre**.
- **Les secrets ne sont jamais conservés** : ce qu'un gestionnaire de mots de
  passe marque comme confidentiel est écarté avant d'être écrit.
- **Rien ne sort de la machine** : pas de compte, pas de nuage, pas de
  télémétrie.
- **Portable** : posez un `copycopy.conf` à côté de l'exécutable, et tout vit
  dans ce dossier.
- **Rapide** : 100 000 entrées, toujours 59 fps.

## Installer

Prenez le paquet de votre système dans les [releases](../../releases) :

| Système | Fichier |
|---|---|
| macOS | `copycopy-macos-universal.dmg` |
| Windows | `copycopy-windows-x86_64.exe`, ou `…-portable.zip` |
| Linux | `copycopy-linux-x86_64.AppImage`, ou le `.deb` |

Rien n'est signé, donc le premier lancement affiche un avertissement :

- **macOS** : `xattr -dr com.apple.quarantine /Applications/copycopy.app`, ou
  tenter une fois, puis *Réglages Système → Confidentialité et sécurité →
  Ouvrir quand même*.
- **Windows** : *Informations complémentaires* → *Exécuter quand même*.

Ou compilez depuis les sources. Une toolchain Rust suffit :

```bash
cargo build --release -p copycopy && ./target/release/copycopy
```

## Utiliser

| | |
|---|---|
| **`Cmd+Shift+V`** (macOS), **`Ctrl+Alt+V`** (ailleurs) | ouvrir la fenêtre |
| taper | chercher |
| `Entrée` | recopier et fermer |
| `Ctrl+B` | épingler |
| `Échap` | fermer |

## Où ça tourne

| | |
|---|---|
| Windows | utilisé au quotidien |
| macOS | lancé sur Apple Silicon |
| Linux X11 | lancé et testé |
| Linux Wayland | compilé par le CI, jamais lancé : liez un raccourci à `copycopy --show` dans votre compositeur |

Pourquoi c'est construit ainsi, et ce qui est vérifié où :
**[docs/notes.fr.md](docs/notes.fr.md)**.

## Licence

MIT, voir [LICENSE](LICENSE).
