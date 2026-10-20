# How copycopy works

The short presentation is in the [README](../README.md). This page explains
how the program is built, where it runs, and what it cannot do.

*Une version française est disponible dans [notes.fr.md](notes.fr.md).*

## The idea

copycopy is a **resident process**: it captures the clipboard all the time and
opens its window only when you call it. A manager that only remembered what you
copied while its window was open would miss almost everything.

The history lives in a SQLite database built into the binary, so there is
nothing to install. Pictures are stored as files beside it. **Portable mode**:
put a `copycopy.conf` next to the executable and everything lives in that
folder.

## Where it runs

| | Windows | macOS | Linux X11 | Linux Wayland |
|---|---|---|---|---|
| Status | in daily use | in daily use (Apple Silicon) | run and tested | built by the CI, never run |
| Capture | event-driven | polling every 200 ms, the only API Apple offers | event-driven | event-driven, not on GNOME (see below) |
| Source application | yes | yes, with its icon | yes | no |
| Global shortcut | `Ctrl+Alt+V` | `Cmd+Shift+V` | `Ctrl+Alt+V` | none: bind `copycopy --show` in your compositor |
| Auto-paste | yes | yes, after granting Accessibility | yes | no |
| Drag out of the list | yes | yes | no | no |
| Release package installed by hand | not recorded | 0.5.0 disk image, dragged onto its Applications link | not recorded | not recorded |

"Run" means someone used it on that system. "Built by the CI" only means it
compiles.

## Choices worth knowing

- **Secrets are never captured.** When a password manager marks a copy as
  confidential, each system has its own flag for it, and copycopy drops the
  content before it is stored. This is the rule the rest of the project bends
  around.
- **iced, not egui.** egui cannot draw colour emoji. iced, through cosmic-text,
  draws emoji, skin tones and family sequences, and finds fallback fonts on its
  own.
- **The list is virtualised by hand.** Only the visible rows are built, so
  100,000 entries still scroll at 59 fps. It only works because every row has
  exactly the same height.
- **Search by fragment.** Three characters or more search the whole database
  with a trigram index, which also handles languages written without spaces.
- **Code is coloured without knowing its file.** A copied snippet has no
  extension, so the language is guessed from its content, and left uncoloured
  when the guess is not sure.
- **Files go back as files.** Copying a file entry puts the files on the
  clipboard, not their paths. Files moved or deleted since are marked as
  missing and cannot be dragged.
- **The logo is drawn by code.** The window icon, the Dock icon and every
  installer icon come from one function, so they cannot drift apart.
- **Three pairs of themes.** Plain greys by default, with one muted
  blue-green accent; Minuit and Lavande, built from the logo's colours, and
  Clinique in the settings.

## Known limits

- **GNOME on Wayland** lets no background application read the clipboard.
  KDE, Sway, Hyprland and COSMIC do. This is a platform limit, not a bug.
- **macOS asks again for Accessibility** after every new unsigned build, for
  auto-paste.
- **The binaries are not signed.** Windows shows a SmartScreen warning and
  macOS needs `xattr -dr com.apple.quarantine` on first launch.
- **Under WSL**, the global shortcut cannot be pressed from Windows
  applications. Use the native Windows build.

## Checking things yourself

Small diagnostic tools ship as examples, so a claim can be checked rather than
believed:

| Example | What it checks |
|---|---|
| `fake_owner` | capture and source attribution, with a fake X11 application |
| `press_key` | the global shortcut, with a synthesised keystroke |
| `echo` | that what copycopy writes back is not captured again as new |
| `targets` | why a copy produced no entry: what the clipboard offered |
| `wl_globals` | which Wayland protocols the compositor exposes |
| `icon` | renders the logo to a PNG, at any size |

```bash
cargo run --release -p copycopy-platform --example echo -- "some text"
copycopy --demo          # sample data, kept apart from your history
copycopy --headless --for 20   # capture in the console, no window
```

## Credits

The palettes are copycopy's own. Earlier versions borrowed
[Base16 Purpledream](https://github.com/tinted-theming/schemes/blob/spec-0.11/base16/purpledream.yaml)
by malet (MIT) and [Aalto Light](https://github.com/emacs-jp/replace-colorthemes/blob/master/aalto-light-theme.el)
by Jari Aalto, ported by Syohei Yoshida (GPL-3.0+).

## Licence

MIT. See [LICENSE](../LICENSE).
