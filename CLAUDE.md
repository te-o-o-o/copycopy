# copycopy

A clipboard manager written in Rust. A resident process captures the clipboard
continuously and opens an iced popup on demand.

## Layout

| Crate | Role | Rule |
|---|---|---|
| `copycopy-core` | model, bounded history, dedup, classification | knows nothing about the OS or the UI |
| `copycopy-platform` | capture, one backend per system | knows nothing about the UI |
| `copycopy` | the binary: iced daemon, window, global shortcut | |

That split was tested for real: when the toolkit changed from egui to iced,
`core` and `platform` did not move a single line. Keep it that way.

## Commands

```bash
cargo check --workspace                    # in a loop while writing
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt
cargo test --workspace                     # 45 tests
cargo build --release -p copycopy          # → target/release/copycopy
```

**Always run in `--release`.** Rust optimises nothing in debug, and a graphics
stack feels it.

```bash
./target/release/copycopy --demo --open           # window with sample data
./target/release/copycopy --headless --for 20     # console capture, no UI
./target/release/copycopy --screenshot out.png --for 10
```

Full command reference, including every flag: `.claude/cargo.md` — local, not
versioned.

## Conventions

- **English everywhere**: code, comments, commit messages, `README.md`. A
  French translation of the README is kept as `README.fr.md`; keep the two in
  sync when either changes.
- **Conventional Commits**: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`,
  `chore`. Imperative subject, and a body that explains the *why*.
- Comments explain decisions and traps, not syntax.

## Rules that must not be broken again

Each of these cost real debugging time. Re-introducing them is a regression.

1. **Secret content is never captured.** If the clipboard advertises
   `x-kde-passwordManagerHint`, `org.nspasteboard.ConcealedType`,
   `ExcludeClipboardContentFromMonitorProcessing`, or
   `CanIncludeInClipboardHistory` set to 0, the content is dropped. A clipboard
   manager that remembers passwords is malware by accident.
2. **Row pitch must be exactly `ROW_H`.** The virtualisation computes which
   rows are visible from the scroll offset. Any `spacing` on the list column,
   or vertical margin on a row, makes the spacers drift. Spacing between
   highlights comes from shrinking the *background* inside the row, never the
   row itself.
3. **Never use `stack` as an overlay.** A layer placed over a row stops that
   row from repainting: its text freezes on the first render, then disappears.
   Anything that must sit on top goes through layout.
   A layer *underneath* is a different case, and exactly one exists: the Matrix
   rain, built in that theme only. It passed a repaint protocol — rows still
   drawn after a hundred redraws, a capture arriving mid-animation, a scrolled
   open — but under WSLg only. If rows ever freeze in the Matrix theme, suspect
   it first; the other themes never build the stack at all.
4. **Each band centres with `center_y`**, not `row.align_y(Center)`. The latter
   aligns children relative to each other but leaves the band stuck to the top
   of its container.
5. **The filtered list lives in the state**, recomputed only when the query
   changes or a capture arrives — never inside `view()`. That is what holds
   100,000 entries at 59 fps.

## Known defect — still open

The "source · age" line of a row sometimes fails to draw. It was declared fixed
after six consecutive passes of one scenario; that was premature — the scenario
had simply stopped triggering it. It still occurs, intermittently, on other
paths: freshly captured entries lose the line in some runs and keep it in
others, with no difference in the data.

Ruled out, each by isolating it: `clip`, the resize frame, a `stack` overlay, an
empty source, the payload type, and an image thumbnail in the badge slot.
`view()` always produces the right string — verified by tracing — and the header
and footer, outside the `scrollable`, always draw.

**Next step is a minimal reproduction**, roughly twenty lines with a scrollable
whose rows hold two stacked texts, to find out whether this is an iced bug worth
reporting upstream or a misuse. Chasing it inside the application has cost
several sessions and produced only eliminations.

Weighed against that: **this has only ever been seen under WSLg, by tooling,
never by someone using the application.** The same workbench has produced three
other phantoms — cursor shapes that never apply, a shortcut invisible to Windows
applications, a Wayland surface XTEST cannot drive. Treat it as a likely fourth
until someone sees it on a target platform. Do not spend another session on it
without that.

`cargo run --release -p copycopy --example band -- shot.png X0 X1 Y0 Y1` counts
lit pixels in a band, which detects the line without visual inspection.

## Windows traps

Both cost a session, and neither shows up anywhere but on a real Windows run.

- **A taken pipe name is `PermissionDenied`, not `AddrInUse`.** `interprocess`
  creates the named pipe with `FILE_FLAG_FIRST_PIPE_INSTANCE`, and Windows
  refuses with "access denied" when it already exists. Read as a broken IPC,
  every launch started one more resident — five were found running at once,
  none reachable by `--show`, and the window never appeared. `ipc::claim`
  treats both kinds as "name taken".
- **`iced::exit()` does not stop a daemon that never opened a window.** It files
  a request read on the loop's next wake-up, and nothing wakes a windowless
  resident. `--quit` answered "ok" and left the process running. `State::quit`
  therefore ends the process itself if the loop has not let go after
  `QUIT_GRACE`; the log says which of the two happened (`resident stopped` or
  `event loop did not stop`). This matters for autostart, where the resident
  starts without a window.

When something fails on Windows, read `copycopy.log` first, and count the
`copycopy.exe` processes — a duplicate resident explains most "nothing happens".

## What is verified, and what is not

WSL is the development environment, not a target. The program is meant to run
on Windows, native Linux and macOS. Treat WSLg oddities as artefacts of the
workbench, never as product defects — but equally, never take a WSL success as
proof that a target platform works.

- **X11 capture**: run and tested.
- **Windows**: run for real — text and image capture, copying back, and the
  global shortcut in daily use. Built from WSL with MinGW
  (`--target x86_64-pc-windows-gnu`); WSL launches the `.exe` directly as a
  native Windows process. The binary is a GUI-subsystem program: no console.
  Output goes to the parent terminal when there is one, otherwise to
  `copycopy.log` beside the database (`console.rs`) — look there first when
  something fails on Windows.
- **Wayland, macOS**: written, never executed. Do not describe them as working.
  Since rusqlite `bundled` they no longer type-check from Linux either: SQLite
  is C, and compiling it needs the target's own toolchain.
- **Global shortcut**: verified on X11, including an actual trigger, and on
  Windows. Under WSL
  it cannot fire from Windows applications — `XGrabKey` only sees keys reaching
  the WSLg X server. That is structural.
- **Pointer behaviour is untestable under WSLg.** The window is a Wayland
  surface, so XTEST reaches the keyboard but not the pointer over it: dragging
  to move or resize cannot be exercised here. WSLg also never applies requested
  cursor shapes, so a cursor that does not change proves nothing about the
  code. Do not chase either of these again.
- **Nor is anything the window's keyboard triggers.** XTEST can fire the global
  shortcut, because `XGrabKey` lives in the X server — but it cannot deliver a
  key *to the window*, which is a Wayland surface. So Enter-to-copy cannot be
  driven from here, and the X11 winit backend is not a way round it: it aborts
  on a missing `libxkbcommon-x11.so`. Test what the window triggers through the
  layer underneath instead — `Setter` and the watcher, as the `echo` example
  does.

Diagnostic tools live as `examples`: `fake_owner`, `press_key`, `echo` and
`targets` (platform), `band`, `edges`, `pixel`, `zoom`, `compare` (app). Prefer
checking a claim with one of them over asserting it.
