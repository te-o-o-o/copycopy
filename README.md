# copycopy

A minimal clipboard manager. Real capture, **iced** popup.

It is a **resident process**: it captures continuously and only opens its window
on demand. Without that it would only remember what you copy while you are
looking at it — which was the real hole in the first version.

The history is kept in a SQLite database compiled into the binary, so there is
nothing to install on any system. **Portable mode**: put a `copycopy.conf` next
to the executable and the configuration, the database and the images all live in
that directory, leaving the host machine untouched.

*Une version française de ce document est disponible dans [README.fr.md](README.fr.md).*

## Running

```bash
copycopy                 # start the resident, window closed
copycopy --open          # start and open the window
copycopy --show          # ask the resident to open
copycopy --quit          # stop the resident
copycopy --demo          # multilingual sample data
copycopy --backend wayland|x11|poll
copycopy --screenshot out.png --for 10   # capture the window, then quit
copycopy --hidden ...    # start without a window, even in screenshot mode
copycopy --settings ...  # open on the settings panel, to check it in a screenshot
copycopy --filter code   # open with a type filter, to check the filter band
copycopy --autostart on|off   # start with the session, or stop doing so, then exit

copycopy --headless --for 20  # console capture, no interface
cargo run -p copycopy-platform --example fake_owner -- "text" Firefox 3
```

Navigation: `↑↓` / `Ctrl-N` `Ctrl-P`, `PageUp/Down`, `Enter` to copy,
`Ctrl-B` to pin, `Delete` or `Ctrl-D` to remove — also reachable through the
cross that appears on the hovered and selected rows. `Esc` closes the window;
`Ctrl-Q` (`Cmd-Q` on macOS) stops the resident altogether, like `--quit`.
The gear button opens the settings in the right-hand panel — theme, shortcut,
auto-paste, startup, data folder, and *Quitter copycopy* — and `Esc` closes them
before the window.

**Auto-paste**, off by default. Once an entry is copied, copycopy hands the
focus back to the application you were in and presses `Ctrl+V` for you, so the
entry lands where your cursor was. Windows through `SendInput`, X11 through
XTEST; refused under Wayland, which needs the RemoteDesktop portal for that,
and not available on macOS yet. Simulating keystrokes in another application
is something to switch on, never to discover.

**Starting with the session**, off by default. The switch writes a per-user
entry — the `Run` key under `HKCU` on Windows, with no administrator rights, a
`.desktop` file in `~/.config/autostart` on Linux, a launch agent on macOS — and
copycopy then waits in the background from login, with no window, so the
shortcut has something to talk to the first time you press it. The entry holds
an absolute path, rewritten at every start so moving the executable, as portable
mode invites, does not leave a dead entry behind. Remove it from the Task
Manager and copycopy notices at the next start and stops claiming otherwise.
The header cross hides the window, like `Esc`: it never quits, so nobody stops
capture by reaching for the usual corner.

**Type filters** sit on a band under the search — All, Text, Code, URL, Images,
Files — each with the number of entries it would show for the current search.
They combine with it: *Code* and `select` list only the code containing
"select". `Ctrl-1` to `Ctrl-6` switch filters without leaving the search field,
and the filter goes back to *All* each time the window opens.
`Home`/`End` are left to the search field — inside a text input, moving the
caret is what you expect.

**On Windows no console opens.** A resident has no business owning one, and
closing it used to stop capture without a word. Launched from a terminal, the
output still goes to that terminal; launched any other way — Start menu, login
— it goes to `copycopy.log` beside the database, restarted past a megabyte.

## Opening: global shortcut and IPC

Defaults to **`Ctrl+Alt+V`** (`Cmd+Shift+V` on macOS), configurable in
`~/.config/copycopy/copycopy.conf`.

| System | Mechanism | Status |
|---|---|---|
| Windows | `RegisterHotKey` | verified, in daily use |
| macOS | Carbon `RegisterEventHotKey` | written, never run |
| X11 | `XGrabKey` | verified, including an actual trigger |
| Wayland | **no client-side global shortcut exists** | falls back to IPC |

Thread constraint: the hotkey manager must be created on the main thread
(macOS) and on the thread that owns the event loop (Windows). It is therefore
created inside `boot()`, which satisfies both, and kept alive in the state —
dropping it would unregister the shortcut.

**Universal fallback, and the only route under Wayland**: bind a shortcut in
your compositor that runs `copycopy --show`. That second process detects the
resident through a local socket (`interprocess`), hands over the command and
exits. The same mechanism guarantees that only one daemon captures.

The `org.freedesktop.portal.GlobalShortcuts` portal is still to be done. It is
the clean route under Wayland, and the only one that also solves focus — a
Wayland client cannot focus itself, it needs an `xdg-activation-v1` token that
only the portal hands out.

### Under WSL, the shortcut cannot work from Windows applications

`XGrabKey` only sees keys that reach the WSLg X server. A `Ctrl+Alt+V` pressed
inside a Windows application is handled by Windows and never reaches it. This
is structural; no code change fixes it. Running a native Windows build is the
answer — and it removes the WSLg clipboard bridge along the way.

Two further WSLg limitations, each found the hard way:

- **Cursor shapes are never applied.** The resize handles ask for
  `ResizingHorizontally` and friends, and the search field asks for a text
  caret; neither shows up. The code is correct — `MouseArea` only overrides the
  cursor when its child reports `Interaction::None`, which a `Space` does. It
  is WSLg that ignores the request.
- **The window is a Wayland surface, not an X11 one.** The process holds a
  `wayland` file descriptor and never appears in `_NET_CLIENT_LIST`. XTEST can
  therefore drive the keyboard, which is how the global shortcut was tested,
  but not the pointer over this window — so drag-to-move and drag-to-resize
  cannot be tested here at all.

## The window

No decorations, always on top. It closes on `Esc` and on focus loss — that is
palette behaviour, not document-window behaviour. **No maximise**: a clipboard
list gains nothing from full screen; what you want is to move it, resize it,
and find it where you left it.

- **Square corners, deliberately.** Rounded corners were tried and do work
  technically (on top of `transparent(true)` you also need an application
  `style()` with a transparent `background_color`, otherwise iced paints the
  theme colour across the whole surface and the rounding ends up sitting on an
  opaque rectangle). But on an undecorated window the corners reveal the
  desktop behind, which reads as a black outline. The window is therefore
  opaque all the way to the edge, with a 1 px border.
- **Moving**: drag anywhere on the header (`window::drag`). `mouse_area` lets
  the child capture first, so clicking the search field does not move the
  window.
- **Resizing**: eight 6 px bands around the rim, invisible because they let the
  card background through, placed **by layout** rather than as an overlay — a
  `stack` on top stops rows from repainting (see below).
- **Geometry**: size and position are remembered, but written separately. Some
  environments (WSLg) never report a real position and would return 0,0, which
  would open the window in a corner instead of centred. Position is therefore
  only written once an actual move has been observed.
- **Detail panel**: the right-hand side shows the selected entry in full —
  text up to 10,000 characters, monospace for code, pictures scaled down to
  fit, file lists. It is built when the selection lands on another entry, never
  inside `view()`: reading an image file or walking a long text on every frame
  would cost the frame rate. The window opens at 980×560, and a width
  remembered from before the panel existed is widened to 860.

## Copying

`Enter` or double-click → the content goes to the clipboard and the window
closes. The copied row flashes green for 160 ms first: the window disappearing
confirms that *something* happened, but not *which* entry reached the clipboard.
No toast beyond that — one would mean keeping the window open at the exact moment
you want to be back in your application, pasting.

The real goal is still automatic **pasting** (simulating `Ctrl+V` after
closing), still to be done: `SendInput` on Windows, XTEST on X11, CGEvent on
macOS with Accessibility permission — and impossible under Wayland without the
RemoteDesktop portal.

## Why iced and not egui

egui was the first choice, being the lightest, then ruled out on a single
criterion: **it cannot render colour emoji**. This is not a missed setting —
epaint has no notion of a colour glyph (no COLR, no CBDT, no sbix) and picks
fonts character by character, which also breaks ZWJ sequences: `👨‍👩‍👧‍👦` came
out as four monochrome glyphs.

iced renders text with cosmic-text (rustybuzz + fontdb + swash), which solves
both: colour, ZWJ ligatures, skin tones, and **automatic font fallback** — the
187 lines that enumerated font paths per OS by hand are gone.

The price, measured at equal scope: **14.98 MiB for the egui version against
19.70 MiB for iced**. After deleting the spike crates the iced binary dropped
to **14.31 MiB**: one of them requested `iced` with the `image` feature, and
cargo unifies features across a workspace — the main binary was carrying iced's
whole image decoding stack for nothing. At genuinely equal scope the gap is
negligible. Startup, memory and idle CPU are equivalent. The UI itself is
shorter: 672 lines against 1,064, with `fonts.rs` going from 187 to 38.

### The list must be virtualised by hand

`scrollable` does not virtualise: it builds every child on each `view()`.
Measured with a forced redraw every frame:

| entries | full list | virtualised |
|---|---|---|
| 1,000 | 59 fps · view 2.20 ms | 58 fps · **view 0.08 ms** |
| 5,000 | **0 fps** (unusable) | 59 fps · view 0.16 ms |
| 20,000 | **0 fps** | 59 fps · view 0.42 ms |
| 100,000 | — | **59 fps** · view 1.38 ms |

Rows have a fixed height, so we know exactly which ones are visible: a slice
framed by two spacers that preserve the total height (`view.rs`, ~15 lines).
The ceiling disappears and unlimited history becomes conceivable. The filtered
list is also held in the state and recomputed only when the query changes —
never inside `view()`.

Two pitfalls in that virtualisation, both fixed: the pitch from one row to the
next must be **exactly** `ROW_H` (no `spacing` on the column, no vertical
margin), otherwise the spacers drift away from the real scroll position.
`--scroll N` exists to check this in a screenshot.

### Known defect, still open

The "source · age" line of a row sometimes fails to draw. It was declared fixed
after six passes of a single scenario: that was premature — the scenario had
simply stopped triggering it. It still occurs, intermittently, on other paths:
freshly captured entries lose the line in some runs and keep it in others, with
identical data.

Ruled out, each by isolating it: `clip`, the resize frame, a `stack` overlay, an
empty source, the payload type, and an image thumbnail in the badge slot.
`view()` always produces the right string — verified by tracing — and the header
and footer, outside the `scrollable`, always draw.

Weighed against that: **this has only ever been seen under WSLg, by tooling,
never by someone using the application.** The same workbench has produced three
other phantoms — cursor shapes that never apply, a shortcut invisible to Windows
applications, a Wayland surface XTEST cannot drive. Treat it as a likely fourth
until someone sees it on a target platform. Do not spend another session on it
without that.

**Next is a minimal reproduction**, roughly twenty lines with a `scrollable`
whose rows hold two stacked texts, to find out whether an iced bug should be
reported or a misuse corrected. Chasing it inside the application has cost
several sessions and produced only eliminations.

### Rendering pitfall: `stack` freezes what it covers

An overlay placed on a row to draw an end-of-line fade stopped that row from
repainting: the text stayed frozen on its first render, then vanished.
Diagnosed by tracing `view()` — which produced the right string — then removing
the overlay as the single changed variable. The fade was dropped, so previews
are cut off flat at the edge, at a constant x. Worth revisiting with a custom
widget if the need returns.

### Three layout rules

1. **Vertical centring.** Each band (header, row, footer) is a fixed-height
   `container` that centres its content with `center_y`. A
   `row.align_y(Center)` is not enough: it aligns children relative to each
   other but leaves the row stuck to the top of its container.
2. **No overlays.** See above: anything that needs to sit on top goes through
   layout, never through `stack`.
3. **A row and its highlight are two different things.** The row keeps exactly
   `ROW_H` — the virtualisation maths depends on it — and it is the background,
   inside, that is shrunk by `ROW_GAP`. That is where the breathing room
   between highlights comes from, without touching the list pitch.

The magnifier in the search bar is drawn on a `canvas` rather than taken from a
font: crisp at every scale and independent of which glyphs happen to exist.

## Capture

One trait (`ClipboardBackend`), one native backend per system, and polling as a
last resort — never as the default choice.

| System | Mechanism | Wake-up | Source application | Status |
|---|---|---|---|---|
| **Linux / X11** | XFixes `SelectionNotify` | event-driven | `WM_CLASS`, else `_NET_WM_PID` | **tested here** |
| **Linux / Wayland** | `ext-data-control-v1`, falling back to `zwlr-data-control-v1` | event-driven | *unavailable* | written, **never run** |
| **Windows** | `AddClipboardFormatListener` on a message-only window | event-driven | `GetClipboardOwner` → `QueryFullProcessImageNameW` | written, **never run** |
| **macOS** | `NSPasteboard.changeCount` (200 ms) | polling | *unavailable* | written, **never run** |
| *universal fallback* | `arboard` (200 ms) | polling | *unavailable* | tested here |

macOS polling is not a workaround: Apple exposes no change notification at all,
`changeCount` is the API.

### What is actually verified

- X11: run and tested (text, URLs, code, CJK, Arabic, emoji, PNG, files, INCR
  over 400 KiB, deduplication, source attribution).
- Wayland, Windows, macOS: **type-checked against their real targets**
  (`cargo check --workspace --target x86_64-pc-windows-msvc` and
  `aarch64-apple-darwin`). But never executed: this machine is a WSL2. Validate
  on the real platforms before believing any of it.
- Backend selection is tested for real: under WSLg, `WAYLAND_DISPLAY` is set but
  the compositor exposes no data-control protocol, and we fall back to X11
  cleanly while saying why.

### The blind spot: GNOME on Wayland

Wayland does not expose the clipboard to background applications: the standard
`wl_data_device` requires keyboard focus. A data-control protocol is needed —
and **GNOME implements neither** `ext` nor `wlr`. KDE, Sway, Hyprland and COSMIC
do expose one.

On GNOME/Wayland there is therefore no way to watch the clipboard from a
background process without an extension or a portal. The polling fallback
changes nothing: it hits the same wall. That is a platform limitation, not a
project one, and it deserves to be stated rather than papered over.

### Formats and secrets

| | X11 | Wayland | Windows | macOS |
|---|---|---|---|---|
| Text | `UTF8_STRING`, `text/plain;charset=utf-8`, `STRING` | same | `CF_UNICODETEXT` | `NSPasteboardTypeString` |
| Image | `image/png` | `image/png` | `PNG`, else `CF_DIB` → PNG | `…TypePNG`, else TIFF → PNG |
| Files | `text/uri-list` | `text/uri-list` | `CF_HDROP` | `…TypeFileURL` per item |
| Large payloads | **INCR** | socket stream | `GlobalSize` | `NSData` |

**Content marked as secret is never captured**, following each platform's
convention:

- X11 / Wayland: the `x-kde-passwordManagerHint` or
  `org.nspasteboard.ConcealedType` target;
- Windows: the `ExcludeClipboardContentFromMonitorProcessing` format, and
  `CanIncludeInClipboardHistory` set to 0;
- macOS: the `org.nspasteboard.ConcealedType`, `AutoGeneratedType` and
  `TransientType` types.

This is the single most important rule in the project: a clipboard manager that
remembers passwords is malware by accident.

### Capture reliability under WSLg

The WSLg clipboard bridge takes the selection back behind every application,
with a delay. On a burst of five chained copies from applications that only
live 2 s, an entry is sometimes lost (2 runs out of 3, never the same one).
That is specific to this test environment — revalidate on a real Linux desktop
before concluding anything from it.

### Known behavioural difference

The polling backend captures whatever is **already** in the clipboard at
startup; the event-driven backends do not, they only learn about content on the
next change. To be unified (initial read at startup).

### Two pitfalls hit, and fixed

1. **Dropped notifications.** Waiting for a read's `SelectionNotify` while
   ignoring other events loses the `XFixesSelectionNotify` that arrive
   meanwhile — that is, copies made in quick succession. They are now queued
   and handled right after.
2. **Duplicate notifications.** The WSLg clipboard bridge (and any third-party
   manager) takes the selection back right after the source application, so two
   to three notifications arrive per copy. A repeat is only forwarded when it
   brings the source attribution that was missing.

## Testing capture without taking my word for it

```bash
cargo build --release --workspace --examples
./target/release/copycopy --headless --for 12 &
./target/release/examples/fake_owner "One" Firefox 2
./target/release/examples/fake_owner "Two" DBeaver 2
```

`fake_owner` is a real X11 client that advertises `WM_CLASS` and `_NET_WM_PID`
and serves the selection — unlike `xclip`, which exposes neither and therefore
cannot validate source attribution.

`press_key` synthesises a keystroke through the XTEST extension, which is how
the global shortcut was tested without a physical keyboard:

```bash
cargo run -p copycopy-platform --example press_key -- ctrl alt v
```

`echo` covers the other direction — writing. It puts a payload on the clipboard
through the very `Setter` the application uses, then listens with the real
watcher and reports whether each capture was recognised as our own write:

```bash
cargo run --release -p copycopy-platform --example echo -- shot.png
```

An image comes back re-encoded, so its bytes differ from the ones that went in
and content hashing cannot recognise it. Without that check, every paste-back
landed in the history as a new entry.

`targets` answers the other question — why a copy produced no entry at all. It
lists what the clipboard owner advertises and what our reader gets out of each
format, timings included:

```bash
# copy something, then:
cargo run --release -p copycopy-platform --example targets
```

It exists because the backend is silent on that path: when an owner advertises
an image it then fails to deliver, the whole copy is dropped, text alternatives
included, and nothing is logged.

## Layout

```
crates/
├── core/       model, bounded history, dedup, classification — no OS/UI deps
├── platform/   capture: x11.rs, wayland.rs, windows.rs, macos.rs,
│               poll.rs (fallback), setter.rs (writing)
└── app/        the UI
    ├── main.rs   iced daemon, subscriptions, keyboard, window
    ├── config.rs configuration (shortcut, geometry)
    ├── hotkey.rs global shortcut
    ├── ipc.rs    single instance and the --show command
    ├── theme.rs  palette: the whole look is tuned here
    ├── fonts.rs  fallback fonts (systems without CJK/emoji)
    └── view.rs   popup, virtualised list, resize handles
```

`core` knows nothing about the OS or the UI. The bet on that split was tested
when the toolkit changed: **`core` and `platform` did not move a single line**
(1,880 lines); only `app` was rewritten.

## Checking the other OSes yourself

```bash
rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin
cargo check --workspace --target x86_64-pc-windows-msvc
cargo check --workspace --target aarch64-apple-darwin
cargo run -p copycopy-platform --example wl_globals   # Wayland globals exposed
```

## Persistence and search

SQLite through rusqlite's `bundled` feature: the database ships inside the
binary, with no system library to install anywhere. Images are written as files
beside it and their bytes are read only when an entry is copied — reading every
PNG back to draw a list of text rows would make each keystroke hit the disk. An
entry drops its bytes as soon as its file is written, so nothing is held twice.

A picture is named after its content hash, and no two rows share one, so a file
nothing points at can never be reached again. Deleting an entry and pruning the
history therefore take the file with the row, and startup sweeps whatever is
left over — from a crash between writing the file and inserting its row, and
from every version that pruned rows without touching the disk.

Search splits at three characters. Below that it filters the loaded window in
memory, which a trigram index cannot answer. At three or more it queries the
database, so entries older than that window are found too. The **trigram**
tokeniser rather than the default one: a clipboard is searched by fragment, not
by word, and trigram also indexes languages without spaces, so CJK content stays
reachable. The threshold changes engine, not just narrowness — two characters
and three do not filter alike.

Pinned entries lead the list, recency ordering inside each group, and pruning
never drops them. Rows carry the payload size when there is more than the line
shows — counted on the full content, not on the preview, which is itself capped.

## Credits

The two colour palettes are borrowed from existing themes, adapted rather than
copied as code:

- **Dark** — [Base16 Purpledream](https://github.com/tinted-theming/schemes/blob/spec-0.11/base16/purpledream.yaml)
  by malet, from Tinted Theming (MIT). The scheme's values are used as they are.
- **Light** — [Aalto Light](https://github.com/emacs-jp/replace-colorthemes/blob/master/aalto-light-theme.el)
  by Jari Aalto, ported by Syohei Yoshida (GPL-3.0+). Only colour values are
  taken, and the ground is darkened from its original cream.

`crates/app/src/theme.rs` notes where each value comes from.

## Licence

MIT. See [LICENSE](LICENSE).
