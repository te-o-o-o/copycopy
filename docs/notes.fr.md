# Comment fonctionne copycopy

La présentation courte est dans le [README](../README.fr.md). Cette page
explique comment le programme est construit, où il tourne, et ce qu'il ne sait
pas faire.

*An English version is available in [notes.md](notes.md).*

## L'idée

copycopy est un **processus résident** : il capture le presse-papier en
permanence et n'ouvre sa fenêtre que quand on l'appelle. Un gestionnaire qui ne
retiendrait que ce qu'on copie pendant que sa fenêtre est ouverte raterait
presque tout.

L'historique vit dans une base SQLite intégrée au binaire : rien à installer.
Les images sont enregistrées en fichiers à côté. **Mode portable** : posez un
`copycopy.conf` à côté de l'exécutable, et tout vit dans ce dossier.

## Où ça tourne

| | Windows | macOS | Linux X11 | Linux Wayland |
|---|---|---|---|---|
| État | utilisé au quotidien | utilisé au quotidien (Apple Silicon) | lancé et testé | compilé par le CI, jamais lancé |
| Capture | sur événement | sondage toutes les 200 ms, la seule API qu'Apple propose | sur événement | sur événement, pas sous GNOME (voir plus bas) |
| Application d'origine | oui | oui, avec son icône | oui | non |
| Raccourci global | `Ctrl+Alt+V` | `Cmd+Shift+V` | `Ctrl+Alt+V` | aucun : liez `copycopy --show` dans votre compositeur |
| Collage automatique | oui | oui, après l'autorisation Accessibilité | oui | non |
| Glisser hors de la liste | oui | oui | non | non |
| Paquet de release installé à la main | pas noté | image disque 0.5.0, glissée sur son lien Applications | pas noté | pas noté |

« Lancé » veut dire que quelqu'un l'a utilisé sur ce système. « Compilé par le
CI » veut seulement dire que ça compile.

## Les choix à connaître

- **Les secrets ne sont jamais capturés.** Quand un gestionnaire de mots de
  passe marque une copie comme confidentielle, chaque système a son propre
  marqueur pour ça, et copycopy écarte le contenu avant de l'enregistrer. C'est
  la règle autour de laquelle tout le reste s'organise.
- **iced, pas egui.** egui ne sait pas dessiner les emoji en couleur. iced, via
  cosmic-text, dessine les emoji, les tons de peau et les séquences de famille,
  et trouve seul les polices de secours.
- **La liste est virtualisée à la main.** Seules les rangées visibles sont
  construites, donc 100 000 entrées défilent encore à 59 i/s. Ça ne marche que
  parce que toutes les rangées ont exactement la même hauteur.
- **Recherche par fragment.** À partir de trois caractères, la recherche
  parcourt toute la base avec un index de trigrammes, qui gère aussi les
  langues écrites sans espaces.
- **Le code est coloré sans connaître son fichier.** Un extrait copié n'a pas
  d'extension : le langage est deviné d'après le contenu, et rien n'est coloré
  quand la supposition n'est pas sûre.
- **Les fichiers repartent comme des fichiers.** Copier une entrée de fichiers
  met les fichiers dans le presse-papier, pas leurs chemins. Ceux qui ont été
  déplacés ou supprimés depuis sont signalés et ne peuvent pas être glissés.
- **Le logo est dessiné par le code.** L'icône de la fenêtre, celle du Dock et
  celles des installeurs sortent toutes d'une même fonction : elles ne peuvent
  pas diverger.
- **Deux paires de thèmes.** Minuit et Lavande par défaut, construits à partir
  des couleurs du logo, et Clinique dans les réglages.

## Limites connues

- **GNOME sous Wayland** ne laisse aucune application en arrière-plan lire le
  presse-papier. KDE, Sway, Hyprland et COSMIC le permettent. C'est une limite
  de la plateforme, pas un bug.
- **macOS redemande l'autorisation Accessibilité** après chaque nouveau build
  non signé, pour le collage automatique.
- **Les binaires ne sont pas signés.** Windows affiche un avertissement
  SmartScreen, et macOS demande `xattr -dr com.apple.quarantine` au premier
  lancement.
- **Sous WSL**, le raccourci global ne peut pas être déclenché depuis les
  applications Windows. Utilisez le build Windows natif.

## Vérifier par soi-même

De petits outils de diagnostic sont livrés comme exemples, pour vérifier une
affirmation plutôt que de la croire :

| Exemple | Ce qu'il vérifie |
|---|---|
| `fake_owner` | la capture et l'application d'origine, avec une fausse application X11 |
| `press_key` | le raccourci global, avec une frappe synthétique |
| `echo` | que ce que copycopy réécrit n'est pas recapturé comme nouveau |
| `targets` | pourquoi une copie n'a produit aucune entrée : ce que le presse-papier proposait |
| `wl_globals` | les protocoles Wayland exposés par le compositeur |
| `icon` | rend le logo en PNG, à n'importe quelle taille |

```bash
cargo run --release -p copycopy-platform --example echo -- "du texte"
copycopy --demo          # données d'exemple, séparées de votre historique
copycopy --headless --for 20   # capture dans la console, sans fenêtre
```

## Crédits

Les palettes sont propres à copycopy. Des versions précédentes empruntaient
[Base16 Purpledream](https://github.com/tinted-theming/schemes/blob/spec-0.11/base16/purpledream.yaml)
de malet (MIT) et [Aalto Light](https://github.com/emacs-jp/replace-colorthemes/blob/master/aalto-light-theme.el)
de Jari Aalto, portée par Syohei Yoshida (GPL-3.0+).

## Licence

MIT. Voir [LICENSE](../LICENSE).
