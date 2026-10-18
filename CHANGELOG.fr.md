# Journal des modifications

## 0.2.0 (2026-10-18)

### Ajouts

- Une popup redessinée, avec deux nouveaux thèmes : **Minuit** (indigo, accent
  corail) et **Surligneur** (blanc, sélection jaune fluo). La paire
  **Clinique** reste disponible dans les réglages, et l'icône lune/soleil
  bascule entre le thème sombre et le thème clair d'une même paire.
- Chaque entrée indique l'application d'où elle a été copiée : sa vraie icône
  sur macOS, un carré d'une couleur qui lui est propre ailleurs.
- Recopier une entrée de fichiers met les fichiers eux-mêmes dans le
  presse-papier, plus leurs chemins : collés dans le Finder ou l'Explorateur,
  ils sont copiés ; dans une messagerie ou un mail, ils sont joints.
- Les entrées dont les fichiers ont été déplacés ou supprimés sont marquées
  « introuvable », et ne peuvent plus être glissées.
- macOS : collage automatique dans l'application d'où l'on vient.
- Les GIF sont prévisualisés, et une image seule copiée comme fichier est
  prévisualisée comme image.

### Corrections

- L'icône de l'application s'affiche désormais dans la barre des tâches de
  Windows, le Dock de macOS et les docks Linux.
- macOS : glisser certaines entrées de fichiers ne fait plus planter
  l'application, et l'icône glissée a toujours la taille d'une miniature.
- macOS : les fichiers copiés dans le Finder sont enregistrés avec leur vrai
  chemin et non des références `/.file/id=…` inutilisables, et gardent leur
  fichier plutôt que leur icône.
- macOS : une socket laissée par une exécution précédente n'empêche plus
  `--show` et le raccourci d'atteindre la copie en cours.
- La fenêtre s'ouvre centrée quand l'écran où elle se trouvait n'existe plus.

## 0.1.0 (2026-09-25)

Première version.
