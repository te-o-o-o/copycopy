<h1 align="center">copycopy</h1>

<p align="center">
  Un gestionnaire de presse-papier qui se souvient de tout ce que vous copiez —<br>
  sauf de vos mots de passe. Curieux, pas malveillant.
</p>

<p align="center"><sub>v0.1.0 · Rust · Windows, Linux, macOS · MIT</sub></p>

<p align="center">
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

- **Rien ne sort de la machine.** Pas de compte, pas de nuage, pas de télémétrie.
- **Les secrets ne sont jamais conservés.** Ce qu'un gestionnaire de mots de
  passe marque comme confidentiel est écarté avant d'être écrit.
- **Rapide.** 100 000 entrées, toujours 59 fps.
- **Portable.** Posez un `copycopy.conf` à côté de l'exécutable et tout vit
  dans ce dossier.

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
