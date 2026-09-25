# copycopy

Un gestionnaire de presse-papier en Rust. Tout ce que vous copiez, retrouvé en
une touche.

![La fenêtre, thème sombre : l'historique à gauche, l'entrée sélectionnée en entier à droite.](docs/screenshot.png)

Un résident capture en continu — texte, code, images, fichiers — et la fenêtre
s'ouvre sur **`Ctrl+Alt+V`** (`Cmd+Shift+V` sur macOS). Trois lettres, `Entrée`,
c'est de retour dans le presse-papier.

- **Rien ne sort de la machine.** Pas de compte, pas de nuage, pas de télémétrie.
- **Les secrets ne sont jamais conservés.** Ce qu'un gestionnaire de mots de
  passe marque comme confidentiel est écarté avant d'être écrit.
- **100 000 entrées à 59 fps**, sur une liste virtualisée à la main.
- **Portable** : posez un `copycopy.conf` à côté de l'exécutable et la
  configuration, la base et les images vivent dans ce dossier.

## Installer

Les paquets sont attachés à chaque [release](../../releases). Rien n'est signé :
Windows affiche un avertissement SmartScreen (*Informations complémentaires* →
*Exécuter quand même*) et macOS refuse la première ouverture (clic droit sur
l'app → *Ouvrir*).

| Système | Fichier |
|---|---|
| Windows | `copycopy-windows-x86_64.exe`, ou `…-portable.zip` |
| Linux | `copycopy-linux-x86_64.AppImage`, ou le `.deb` |
| macOS | `copycopy-macos-universal.dmg` |

Depuis les sources, une toolchain Rust suffit — SQLite est compilé dedans :

```bash
cargo build --release -p copycopy
```

## Où ça tourne

| | |
|---|---|
| Windows | utilisé pour de vrai, au quotidien |
| Linux X11 | lancé et testé |
| Linux Wayland | écrit, compilé par le CI, jamais exécuté |
| macOS | écrit, compilé par le CI, jamais exécuté |

Sous Wayland, aucun raccourci global côté client n'existe : liez-en un dans
votre compositeur qui lance `copycopy --show`.

## Structure

| Crate | Rôle |
|---|---|
| `copycopy-core` | modèle, historique, SQLite FTS5, détection et coloration du langage |
| `copycopy-platform` | capture, un backend par système |
| `copycopy` | le binaire : daemon iced, fenêtre, raccourci global |

Les notes de travail — pourquoi chaque décision a été prise, ce qui a été
essayé puis abandonné, ce qui est vérifié où — sont dans
**[docs/notes.fr.md](docs/notes.fr.md)** ([English](docs/notes.md)).

*An English version of this document is available in [README.md](README.md).*

## Licence

MIT. Voir [LICENSE](LICENSE).
