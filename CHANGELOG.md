# Changelog

## 0.7.0 (2026-11-23)

### Added

- Edit a text entry before copying it: the Edit button above the preview, or
  `⌘E` / `Ctrl+E`. `⌘↩` / `Ctrl+Enter` copies the result, which is kept as a
  new entry; the original never changes. Up to 10,000 characters.
- Email addresses and secrets (API keys, tokens, passwords, private keys) are
  masked in what copycopy pastes, as `[email]` and `[secret]`. Two boxes under
  the preview, ticked by default; the preview still shows the entry as it is.
- More languages recognised, short snippets included, and the application a
  copy came from helps tell them apart.

### Changed

- The keyboard shortcuts moved from under the preview to the settings.

### Fixed

- macOS: after a copy, Escape or the close button, the focus goes back to the
  application you came from, auto-paste or not.
- Long file names wrap in the preview instead of being cut off.

## 0.6.0 (2026-10-22)

### Added

- English as well as French. English is the default, whatever the system
  speaks; French is chosen in the settings.

### Changed

- New default themes, light and dark: plain greys with one muted blue-green
  accent. Minuit, Lavande and Clinique stay available in the settings.
- Pinned rows are marked with an amber bar on their left edge instead of a
  tinted background, which made the list look busy.

### Fixed

- macOS: the window closes after a row is dragged out and dropped elsewhere,
  as on Windows. Dropped back onto copycopy, it stays open.

## 0.5.0 (2026-10-19)

### Added

- The search highlights what it matched in each row.

### Changed

- Escape clears the search first, if there is one, before closing the window.

### Fixed

- macOS: the disk image holds a link to Applications, so the app can be dragged
  into place instead of being run from the read-only image, where removing the
  quarantine failed.

## 0.4.0 (2026-10-19)

### Added

- Cleaning up the history, from the settings: everything, one type, or the
  file entries whose files are all gone. Pinned entries are always kept, and
  each cleanup asks for a second click before deleting anything.
- Pinned rows are tinted, so they stand apart from the latest copies.

### Changed

- The window opens on the newest entry rather than the top row, which pins
  may have taken.

### Fixed

- Windows: the executable carries the logo, so Explorer, the Start menu and
  shortcuts no longer show the generic icon.

## 0.3.0 (2026-10-18)

### Changed

- A new logo: an indigo tile with a coral "c" and a lilac "c", each over a
  faded copy of itself. It is the same in the window, the Dock, the taskbar and
  the installers.
- The light theme is now **Lavande**, a lilac-tinged white with a coral accent,
  built from the logo's colours. It replaces Surligneur.

## 0.2.0 (2026-10-18)

### Added

- A redesigned popup, with two new themes: **Minuit** (indigo, coral accent)
  and **Surligneur** (white, highlighter-yellow selection). The former
  **Clinique** pair stays available in the settings, and the moon/sun icon
  switches between the dark and light theme of the same pair.
- Each entry shows the application it was copied from: its real icon on macOS,
  a coloured square of its own everywhere else.
- Copying a file entry back puts the files themselves on the clipboard, not
  their paths: paste into Finder or Explorer to copy them, into a chat or a
  mail to attach them.
- File entries whose files were moved or deleted are marked "introuvable",
  and can no longer be dragged out.
- macOS: auto-paste into the application you came from.
- GIF files are previewed, and a single picture copied as a file is previewed
  as the picture.

### Fixed

- The application icon now shows in the Windows taskbar, the macOS Dock and
  Linux docks.
- macOS: dragging some file entries no longer crashes the application, and the
  dragged icon is always thumbnail-sized.
- macOS: files copied in Finder are stored as real paths instead of unusable
  `/.file/id=…` references, and keep their file instead of their icon.
- macOS: a stale socket left by a previous run no longer stops `--show` and the
  shortcut from reaching the running copy.
- The window opens centred when the screen it was last on is gone.

## 0.1.0 (2026-09-25)

First release.
