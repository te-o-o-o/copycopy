//! copycopy — a clipboard manager.
//!
//! It is a **resident process**: it captures continuously and only opens its
//! window on demand, through the global shortcut or `--show` from a second
//! process. Without that it would only remember what you copy while you are
//! looking at it.
//!
//!   copycopy                  # start the resident, window closed
//!   copycopy --open           # start and open the window
//!   copycopy --show           # ask the resident to open
//!   copycopy --quit           # stop the resident
//!   copycopy --headless       # console capture, no interface (debug)
//!   copycopy --demo           # multilingual sample data
//!   copycopy --backend wayland|x11|poll
//!   copycopy --screenshot out.png --for 10
//!   copycopy --scroll 400      # open scrolled, to check virtualisation

mod config;
mod fonts;
mod hotkey;
mod ipc;
mod theme;
mod view;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use copycopy_core::{ClipEvent, History};
use copycopy_platform::{BackendKind, Capture, Setter};
use iced::widget::scrollable;
use iced::{keyboard, window, Subscription, Task, Theme};

use config::Config;

const CAPACITY: usize = 1000;
const SEARCH_ID: &str = "search";
const SCROLL_ID: &str = "clips";
const WINDOW_SIZE: (f32, f32) = (760.0, 520.0);
/// A compositor may report a focus loss right after opening; without this
/// grace period the window would close again immediately.
const FOCUS_GRACE: Duration = Duration::from_millis(600);
/// How long the copied row stays highlighted before the window closes. Long
/// enough to register, short enough not to feel like a wait.
const COPY_FLASH: Duration = Duration::from_millis(160);

static BOOT: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
/// Wake-ups from the global shortcut and from the IPC. Parked here because
/// `Subscription::run` only takes a function pointer, with no captures.
static WAKE: Mutex<Option<std::sync::mpsc::Receiver<Wake>>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub enum Wake {
    Hotkey,
    Command(String),
}

// ------------------------------------------------------------------ state

pub struct State {
    pub history: History,
    /// Indices into `history`, recomputed only when the query changes or a
    /// capture arrives — never inside `view()`.
    pub filtered: Vec<usize>,
    pub query: String,
    pub selected: usize,
    pub hovered: Option<usize>,
    pub scroll_y: f32,
    pub viewport_h: f32,
    pub backend: Option<BackendKind>,
    pub flash: Option<(String, Instant)>,
    /// Id of the entry that was just copied, highlighted until the window
    /// closes. Stored as an id rather than a row index so it survives any
    /// reordering.
    pub copied: Option<u64>,
    pub fonts_note: String,
    pub hotkey_note: String,
    setter: Setter,
    config: Config,
    /// The window is created **once**, on the first open, then shown and
    /// hidden rather than destroyed: a palette has to appear instantly, and
    /// recreating a surface on every shortcut press is not free.
    window: Option<window::Id>,
    visible: bool,
    opened_at: Option<Instant>,
    close_on_blur: bool,
    shot_path: Option<std::path::PathBuf>,
    shot_after: Duration,
    shot_done: bool,
    /// Kept alive: dropping it would unregister the global shortcut.
    _hotkeys: hotkey::Hotkeys,
}

#[derive(Debug, Clone)]
pub enum Message {
    Query(String),
    Select(usize),
    Hover(Option<usize>),
    Activate,
    /// The copy confirmation has been shown long enough; close.
    FinishCopy,
    Scrolled(scrollable::Viewport),
    Key(keyboard::Event),
    Backend(BackendKind),
    Captured(Capture),
    Wake(Wake),
    DragWindow,
    ResizeFrom(window::Direction),
    WindowOpened(window::Id),
    WindowEvent(window::Id, window::Event),
    Shot,
    Screenshotted(iced::window::Screenshot),
}

impl State {
    fn refilter(&mut self) {
        let q = self.query.trim().to_lowercase();
        self.filtered = (0..self.history.len())
            .filter(|&i| {
                q.is_empty()
                    || self
                        .history
                        .get(i)
                        .is_some_and(|it| subsequence(&it.preview.to_lowercase(), &q))
            })
            .collect();

        pinned_first(&self.history, &mut self.filtered);
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.flash = Some((msg.into(), Instant::now()));
    }

    fn reveal_selected(&self) -> Task<Message> {
        let top = self.selected as f32 * theme::ROW_H;
        let bottom = top + theme::ROW_H;
        let y = if top < self.scroll_y {
            top
        } else if bottom > self.scroll_y + self.viewport_h {
            bottom - self.viewport_h
        } else {
            return Task::none();
        };
        iced::widget::operation::scroll_to(
            SCROLL_ID,
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: y.max(0.0),
            },
        )
    }

    fn move_selection(&mut self, delta: isize) -> Task<Message> {
        let len = self.filtered.len();
        if len == 0 {
            return Task::none();
        }
        self.selected = ((self.selected as isize + delta).rem_euclid(len as isize)) as usize;
        self.reveal_selected()
    }

    fn show(&mut self) -> Task<Message> {
        if let Some(id) = self.window {
            if self.visible {
                return Task::none();
            }
            self.visible = true;
            self.opened_at = Some(Instant::now());
            return Task::batch([
                window::set_mode(id, window::Mode::Windowed),
                window::gain_focus(id),
                iced::widget::operation::focus(SEARCH_ID),
            ]);
        }
        self.open_window(true)
    }

    /// Creates the window. Called once, on the first open.
    fn open_window(&mut self, visible: bool) -> Task<Message> {
        let size = self
            .config
            .size
            .map(|(w, h)| iced::Size::new(w, h))
            .unwrap_or(iced::Size::new(WINDOW_SIZE.0, WINDOW_SIZE.1));
        let position = self
            .config
            .position
            .map(|(x, y)| window::Position::Specific(iced::Point::new(x, y)))
            .unwrap_or(window::Position::Centered);
        let (id, task) = window::open(window::Settings {
            size,
            min_size: Some(iced::Size::new(420.0, 220.0)),
            position,
            visible,
            decorations: false,
            transparent: false,
            resizable: true,
            level: window::Level::AlwaysOnTop,
            exit_on_close_request: false,
            ..Default::default()
        });
        self.window = Some(id);
        self.visible = visible;
        self.opened_at = Some(Instant::now());
        task.map(Message::WindowOpened)
    }

    /// Hides the window without destroying it, and resets the search so the
    /// next open starts clean. The resident keeps running.
    fn hide(&mut self) -> Task<Message> {
        // Geometry is only written here: no need to touch the disk on every
        // pixel while the window is being dragged.
        self.config.save();
        self.copied = None;
        self.query.clear();
        self.selected = 0;
        self.hovered = None;
        self.scroll_y = 0.0;
        self.refilter();
        if !self.visible {
            return Task::none();
        }
        self.visible = false;
        match self.window {
            Some(id) => window::set_mode(id, window::Mode::Hidden),
            None => Task::none(),
        }
    }

    fn toggle(&mut self) -> Task<Message> {
        if self.visible {
            self.hide()
        } else {
            self.show()
        }
    }

    fn activate(&mut self) -> Task<Message> {
        if self.copied.is_some() {
            return Task::none(); // Already confirming; ignore a second Enter.
        }
        let Some(&idx) = self.filtered.get(self.selected) else {
            return Task::none();
        };
        let Some(item) = self.history.get(idx) else {
            return Task::none();
        };
        let payload = item.payload.clone();
        let id = item.id;
        match self.setter.set(&payload) {
            Ok(()) => {
                // Flash the row, then close. The window disappearing is the
                // real confirmation, but on its own it leaves a doubt about
                // *which* entry went to the clipboard.
                self.copied = Some(id);
                Task::none()
            }
            Err(e) => {
                self.flash(format!("échec de la copie : {e}"));
                Task::none()
            }
        }
    }
}

/// Pinned entries float to the top. The sort is stable, so recency is preserved
/// inside each group. Ordering lives here rather than in `History`, which stays
/// purely chronological — that is what a SQL `ORDER BY pinned DESC, at DESC`
/// will express once persistence lands.
fn pinned_first(history: &History, indices: &mut [usize]) {
    indices.sort_by_key(|&i| !history.get(i).is_some_and(|it| it.pinned));
}

fn subsequence(haystack: &str, needle: &str) -> bool {
    let mut it = haystack.chars();
    needle.chars().all(|c| it.any(|h| h == c))
}

// --------------------------------------------------------------- lifecycle

struct Args {
    open: bool,
    demo: bool,
    query: String,
    screenshot: Option<std::path::PathBuf>,
    seconds: u64,
    /// Scroll offset to apply on open. Only useful to check, in a screenshot,
    /// that the virtualisation spacers stay aligned with the real scroll
    /// position.
    scroll: Option<f32>,
}

fn parse_args() -> Args {
    let a: Vec<String> = std::env::args().collect();
    let val = |name: &str| -> Option<String> {
        a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned()
    };
    let screenshot = val("--screenshot").map(std::path::PathBuf::from);
    // `--hidden`: start without a window even in screenshot mode, so we can
    // check that the resident really captures with the window closed.
    let hidden = a.iter().any(|x| x == "--hidden");
    Args {
        open: !hidden && (a.iter().any(|x| x == "--open" || x == "--show") || screenshot.is_some()),
        demo: a.iter().any(|x| x == "--demo"),
        query: val("--query").unwrap_or_default(),
        screenshot,
        seconds: val("--for").and_then(|v| v.parse().ok()).unwrap_or(1),
        scroll: val("--scroll").and_then(|v| v.parse().ok()),
    }
}

fn boot() -> (State, Task<Message>) {
    let args = parse_args();
    let config = Config::load();
    if let Some(path) = config.write_default_if_missing() {
        println!("configuration : {}", path.display());
    }

    // The hotkey manager must be created on the main thread (macOS) and on
    // the thread owning the event loop (Windows): `boot` satisfies both.
    let hotkeys = hotkey::register(&config.hotkey);
    println!(
        "raccourci : {}{}",
        hotkeys.status,
        if hotkeys.registered { "" } else { "  ← non actif" }
    );
    println!("`copycopy --show` ouvre la fenêtre, `--quit` arrête le résident\n");

    let loaded = fonts::extra();
    let fonts_note = if loaded.paths.is_empty() {
        "polices : système".to_string()
    } else {
        format!("polices : système + {} d'appoint", loaded.paths.len())
    };

    let mut history = History::new(CAPACITY);
    if args.demo {
        seed_demo(&mut history);
    }

    let mut state = State {
        history,
        filtered: Vec::new(),
        query: args.query,
        selected: 0,
        hovered: None,
        scroll_y: 0.0,
        viewport_h: 430.0,
        backend: None,
        flash: None,
        copied: None,
        fonts_note,
        hotkey_note: hotkeys.status.clone(),
        setter: Setter::new(),
        config: config.clone(),
        window: None,
        visible: false,
        opened_at: None,
        // In screenshot mode the window must not close on its own for lack of
        // focus.
        close_on_blur: args.screenshot.is_none(),
        shot_path: args.screenshot,
        shot_after: Duration::from_secs(args.seconds),
        shot_done: false,
        _hotkeys: hotkeys,
    };
    state.refilter();

    let mut tasks = Vec::new();
    if args.open {
        tasks.push(state.open_window(true));
    }
    if let Some(y) = args.scroll {
        tasks.push(iced::widget::operation::scroll_to(
            SCROLL_ID,
            scrollable::AbsoluteOffset { x: 0.0, y },
        ));
    }
    (state, Task::batch(tasks))
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Query(q) => {
            state.query = q;
            state.selected = 0;
            state.refilter();
            state.reveal_selected()
        }
        Message::Select(i) => {
            state.selected = i;
            Task::none()
        }
        Message::Hover(i) => {
            state.hovered = i;
            Task::none()
        }
        Message::Activate => state.activate(),
        Message::FinishCopy => state.hide(),
        Message::Scrolled(viewport) => {
            state.scroll_y = viewport.absolute_offset().y;
            state.viewport_h = viewport.bounds().height;
            Task::none()
        }
        Message::Backend(kind) => {
            state.backend = Some(kind);
            Task::none()
        }
        Message::Captured(capture) => {
            state.history.push(capture.event, capture.source);
            state.refilter();
            Task::none()
        }
        Message::Key(event) => handle_key(state, event),
        Message::Wake(Wake::Hotkey) => {
            // Deliberately visible: for anyone starting the resident at login,
            // this is the only way to tell whether the shortcut fires.
            println!("raccourci déclenché");
            state.toggle()
        }
        Message::Wake(Wake::Command(cmd)) => match cmd.as_str() {
            ipc::SHOW => state.show(),
            ipc::QUIT => {
                state.config.save();
                iced::exit()
            }
            _ => Task::none(),
        },
        Message::WindowOpened(id) => {
            state.window = Some(id);
            state.opened_at = Some(Instant::now());
            state.visible = true;
            Task::batch([
                window::gain_focus(id),
                iced::widget::operation::focus(SEARCH_ID),
            ])
        }
        Message::DragWindow => match state.window {
            Some(id) => window::drag(id),
            None => Task::none(),
        },
        Message::ResizeFrom(direction) => match state.window {
            Some(id) => window::drag_resize(id, direction),
            None => Task::none(),
        },
        Message::WindowEvent(id, event) => match event {
            window::Event::Moved(point) => {
                state.config.position = Some((point.x, point.y));
                Task::none()
            }
            window::Event::Resized(size) => {
                state.config.size = Some((size.width, size.height));
                Task::none()
            }
            window::Event::Closed => {
                if state.window == Some(id) {
                    state.window = None;
                    state.visible = false;
                }
                Task::none()
            }
            window::Event::CloseRequested => state.hide(),
            window::Event::Unfocused => {
                let settled = state
                    .opened_at
                    .is_some_and(|t| t.elapsed() > FOCUS_GRACE);
                if state.close_on_blur && settled && state.visible && state.window == Some(id) {
                    state.hide()
                } else {
                    Task::none()
                }
            }
            _ => Task::none(),
        },
        Message::Shot => {
            let elapsed = BOOT.get().map(|b| b.elapsed()).unwrap_or_default();
            if state.shot_path.is_none() || state.shot_done || elapsed < state.shot_after {
                return Task::none();
            }
            let Some(id) = state.window else {
                return Task::none();
            };
            state.shot_done = true;
            window::screenshot(id).map(Message::Screenshotted)
        }
        Message::Screenshotted(shot) => {
            if let Some(path) = state.shot_path.clone() {
                let (w, h) = (shot.size.width, shot.size.height);
                if let Some(raw) = image::RgbaImage::from_raw(w, h, shot.rgba.to_vec()) {
                    let buf = flatten(&raw);
                    match buf.save(&path) {
                        Ok(()) => println!("capture écrite : {} ({w}×{h})", path.display()),
                        Err(e) => eprintln!("échec de la capture : {e}"),
                    }
                }
            }
            iced::exit()
        }
    }
}

/// Flattens the screenshot onto a neutral background, so any transparent area
/// shows up as grey rather than black.
fn flatten(src: &image::RgbaImage) -> image::RgbaImage {
    const BG: [u8; 3] = [0x33, 0x36, 0x3E];
    let mut out = image::RgbaImage::new(src.width(), src.height());
    for (x, y, px) in src.enumerate_pixels() {
        let a = px[3] as f32 / 255.0;
        let mix = |c: u8, b: u8| (c as f32 * a + b as f32 * (1.0 - a)) as u8;
        out.put_pixel(
            x,
            y,
            image::Rgba([mix(px[0], BG[0]), mix(px[1], BG[1]), mix(px[2], BG[2]), 255]),
        );
    }
    out
}

fn handle_key(state: &mut State, event: keyboard::Event) -> Task<Message> {
    use keyboard::key::Named;
    use keyboard::{Event, Key};

    let Event::KeyPressed { key, modifiers, .. } = event else {
        return Task::none();
    };
    let ctrl = modifiers.control();

    match key {
        // Esc closes the window, not the resident.
        Key::Named(Named::Escape) => state.hide(),
        Key::Named(Named::Enter) => state.activate(),
        Key::Named(Named::ArrowDown) => state.move_selection(1),
        Key::Named(Named::ArrowUp) => state.move_selection(-1),
        Key::Named(Named::PageDown) => state.move_selection(6),
        Key::Named(Named::PageUp) => state.move_selection(-6),
        // Home and End are left to the search field: inside a text input,
        // moving the caret is what you expect.
        Key::Character(c) if ctrl => match c.as_str() {
            "n" => state.move_selection(1),
            "p" => state.move_selection(-1),
            "b" => {
                let Some(&idx) = state.filtered.get(state.selected) else {
                    return Task::none();
                };
                let id = state.history.get(idx).map(|it| it.id);
                state.history.toggle_pin(idx);
                // Without this the order would only change on the next query
                // or capture — the entry would appear to stay put.
                state.refilter();
                // It has just moved to the top, or back down: keep the cursor
                // on the entry rather than on the position it used to hold.
                if let Some(pos) = id.and_then(|id| {
                    state
                        .filtered
                        .iter()
                        .position(|&i| state.history.get(i).is_some_and(|it| it.id == id))
                }) {
                    state.selected = pos;
                }
                state.reveal_selected()
            }
            "d" => {
                if let Some(&idx) = state.filtered.get(state.selected) {
                    state.history.remove(idx);
                    state.refilter();
                }
                Task::none()
            }
            _ => Task::none(),
        },
        _ => Task::none(),
    }
}

// -------------------------------------------------------- subscriptions

/// Capture runs on its own thread and is bridged into the iced runtime as a
/// stream. No polling on the UI side: a capture wakes the resident up.
fn clipboard_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(64, async |mut output| {
        use iced::futures::{SinkExt, StreamExt};

        let backend = std::env::args()
            .position(|a| a == "--backend")
            .and_then(|i| std::env::args().nth(i + 1));
        let watcher = match copycopy_platform::start(backend.as_deref()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("capture indisponible : {e}");
                return;
            }
        };
        let _ = output.send(Message::Backend(watcher.kind)).await;

        let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
        std::thread::Builder::new()
            .name("copycopy-bridge".into())
            .spawn(move || {
                while let Ok(capture) = watcher.rx.recv() {
                    if tx.unbounded_send(capture).is_err() {
                        return;
                    }
                }
            })
            .expect("thread pont");

        while let Some(capture) = rx.next().await {
            if output.send(Message::Captured(capture)).await.is_err() {
                return;
            }
        }
    })
}

/// Wake-ups from the global shortcut and the IPC, parked in `WAKE` by `main`.
fn wake_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(16, async |mut output| {
        use iced::futures::{SinkExt, StreamExt};

        let Some(rx) = WAKE.lock().ok().and_then(|mut slot| slot.take()) else {
            return;
        };
        let (tx, mut arx) = iced::futures::channel::mpsc::unbounded();
        std::thread::Builder::new()
            .name("copycopy-wake".into())
            .spawn(move || {
                while let Ok(wake) = rx.recv() {
                    if tx.unbounded_send(wake).is_err() {
                        return;
                    }
                }
            })
            .expect("thread réveil");

        while let Some(wake) = arx.next().await {
            if output.send(Message::Wake(wake)).await.is_err() {
                return;
            }
        }
    })
}

fn subscription(state: &State) -> Subscription<Message> {
    let mut subs = vec![
        // `keyboard::listen()` only sees events the widgets ignored. The
        // search field has focus and captures Escape, so the window would not
        // close on the first press.
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Keyboard(event) => Some(Message::Key(event)),
            _ => None,
        }),
        window::events().map(|(id, event)| Message::WindowEvent(id, event)),
        Subscription::run(clipboard_stream),
        Subscription::run(wake_stream),
    ];
    if state.copied.is_some() {
        // Fires once: the subscription disappears with `copied` when the
        // window closes.
        subs.push(iced::time::every(COPY_FLASH).map(|_| Message::FinishCopy));
    }
    if state.shot_path.is_some() && !state.shot_done {
        subs.push(iced::time::every(Duration::from_millis(300)).map(|_| Message::Shot));
    }
    Subscription::batch(subs)
}

fn theme_of(_state: &State, _window: window::Id) -> Theme {
    Theme::Dark
}

fn root_style(_state: &State, _theme: &Theme) -> iced::theme::Style {
    theme::root()
}

fn title(_state: &State, _window: window::Id) -> String {
    "copycopy".to_string()
}

fn seed_demo(history: &mut History) {
    const SEED: &[(&str, &str)] = &[
        ("Emoji simples: 🔥 ✅ 🚀 ⚡ 🎯 💡 📋 — ça doit être lisible", "discord"),
        ("Emoji ZWJ + teintes: 👨‍👩‍👧‍👦 👩🏽‍💻 🧑🏿‍🚀 🏳️‍🌈 — le cas le plus dur", "discord"),
        ("日本語のテキストです。クリップボードマネージャーのテスト。漢字とひらがな。", "slack"),
        ("https://github.com/iced-rs/iced/blob/master/core/src/text.rs#L181", "firefox"),
        ("简体中文的测试文本，用于检查字体回退是否正常工作。", "wechat"),
        ("한국어 텍스트 테스트입니다. 클립보드 관리자.", "kakaotalk"),
        ("مرحبا بالعالم، هذا نص عربي لاختبار الاتجاه من اليمين إلى اليسار", "telegram"),
        ("שלום עולם — טקסט בעברית לבדיקה", "telegram"),
        ("fn main() {\n    let watcher = copycopy_platform::start(None)?;\n}", "zed"),
        ("Bonjour — voilà un texte français avec des accents, œufs, et une citation « longue » qui devrait être tronquée proprement", "firefox"),
        ("Mixed 混合 混ぜる mixed العربية mixed 🎉 sur une seule ligne", "notes"),
    ];
    for (text, source) in SEED.iter().rev() {
        history.push(ClipEvent::Text((*text).to_string()), (*source).to_string());
    }
    // One pinned entry, so the demo exercises the marker and the reordering
    // rather than only the text rendering.
    history.toggle_pin(4);
}

/// Console mode: checks capture without depending on the interface. It does
/// not claim the socket, so it can run alongside a resident.
fn headless(seconds: Option<u64>) {
    let backend = std::env::args()
        .position(|a| a == "--backend")
        .and_then(|i| std::env::args().nth(i + 1));
    let watcher = match copycopy_platform::start(backend.as_deref()) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("capture indisponible : {e}");
            return;
        }
    };
    println!("capture active — backend : {}", watcher.kind.label());
    println!("copiez quelque chose, Ctrl-C pour arrêter\n");

    let mut history = History::new(CAPACITY);
    let deadline = seconds.map(|s| Instant::now() + Duration::from_secs(s));

    loop {
        let timeout = match deadline {
            Some(d) => match d.checked_duration_since(Instant::now()) {
                Some(left) => left,
                None => break,
            },
            None => Duration::from_secs(3600),
        };
        match watcher.rx.recv_timeout(timeout) {
            Ok(capture) => {
                let source = if capture.source.is_empty() {
                    "—".to_string()
                } else {
                    capture.source.clone()
                };
                let fresh = history.push(capture.event, capture.source);
                let item = history.get(0).expect("entrée insérée");
                println!(
                    "[{}] {:<5} {:<14} {}",
                    if fresh { "NEW" } else { "DUP" },
                    format!("{:?}", item.kind),
                    source,
                    item.preview.chars().take(90).collect::<String>()
                );
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    println!("\n{} entrées retenues", history.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_entries_float_to_the_top() {
        let mut history = History::new(10);
        for name in ["a", "b", "c", "d"] {
            history.push(ClipEvent::Text(name.into()), "test".into());
        }
        // Newest first at this point: d, c, b, a.
        history.toggle_pin(3); // "a", the oldest
        history.toggle_pin(1); // "c"

        let mut indices: Vec<usize> = (0..history.len()).collect();
        pinned_first(&history, &mut indices);

        let order: Vec<&str> = indices
            .iter()
            .map(|&i| history.get(i).expect("entry").preview.as_str())
            .collect();
        assert_eq!(order, ["c", "a", "d", "b"], "pinned first, recency kept inside each group");
    }
}

fn main() -> iced::Result {
    let _ = BOOT.set(Instant::now());
    let argv: Vec<String> = std::env::args().collect();

    if argv.iter().any(|a| a == "--headless") {
        let seconds = argv
            .iter()
            .position(|a| a == "--for")
            .and_then(|i| argv.get(i + 1))
            .and_then(|v| v.parse().ok());
        headless(seconds);
        return Ok(());
    }

    let wants_quit = argv.iter().any(|a| a == "--quit");
    let wants_show = argv.iter().any(|a| a == "--show");

    // Single instance. When a resident is already running, this process is
    // just a remote control: it forwards the command and exits.
    let listener = match ipc::claim() {
        Ok(ipc::Claim::Primary(listener)) => Some(listener),
        Ok(ipc::Claim::AlreadyRunning) => {
            let command = if wants_quit { ipc::QUIT } else { ipc::SHOW };
            match ipc::send(command) {
                Ok(reply) => println!("copycopy déjà lancé — {command} : {reply}"),
                Err(e) => eprintln!("le résident ne répond pas : {e}"),
            }
            return Ok(());
        }
        Err(e) => {
            eprintln!("IPC indisponible ({e}) — `--show` ne fonctionnera pas");
            None
        }
    };
    if wants_quit && listener.is_some() {
        println!("aucun résident à arrêter");
        return Ok(());
    }
    if wants_show {
        println!("aucun résident : démarrage avec la fenêtre ouverte");
    }

    let (wake_tx, wake_rx) = std::sync::mpsc::channel();
    *WAKE.lock().expect("wake") = Some(wake_rx);

    if let Some(listener) = listener {
        let tx = wake_tx.clone();
        ipc::spawn_server(listener, move |command| {
            let _ = tx.send(Wake::Command(command));
        });
    }
    hotkey::spawn_bridge(move || {
        let _ = wake_tx.send(Wake::Hotkey);
    });

    let mut app = iced::daemon(boot, update, view::view)
        .subscription(subscription)
        .theme(theme_of)
        .style(root_style)
        .title(title);

    for font in fonts::extra().bytes {
        app = app.font(font);
    }
    app.run()
}
