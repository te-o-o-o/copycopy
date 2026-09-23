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
//!   copycopy --settings        # open on the settings, to check them
//!   copycopy --filter code     # open with a type filter, to check the band
//!
//! Ctrl+Q (Cmd+Q on macOS) in the window stops the resident, like `--quit`.

// No console window on Windows: a resident has no business owning one, and
// closing it used to stop capture without a word. See `console`.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod config;
mod console;
mod drag;
mod fonts;
mod hotkey;
mod autostart;
mod icon;
mod ipc;
mod theme;
mod view;

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use copycopy_core::{store::Store, ClipEvent, ClipItem, History, Kind, Payload};
use copycopy_platform::{BackendKind, Capture, Setter};
use iced::widget::scrollable;
use iced::Animation;
use iced::{keyboard, window, Subscription, Task, Theme};

use config::Config;

const CAPACITY: usize = 1000;
const SEARCH_ID: &str = "search";
const SCROLL_ID: &str = "clips";
const PREVIEW_ID: &str = "preview";
/// Wide enough for the list and the detail panel side by side.
const WINDOW_SIZE: (f32, f32) = (980.0, 560.0);
/// Below this, the panel squeezes the list under what a row needs. A width
/// remembered from before the panel existed is widened to it on open.
const MIN_WIDTH: f32 = 860.0;
/// Past this, the panel stops at a notice. A text widget lays out everything it
/// is given, and a clipboard can hold megabytes: previewing a whole log file
/// would stall every resize.
const PREVIEW_CHARS: usize = 10_000;
/// A compositor may report a focus loss right after opening; without this
/// grace period the window would close again immediately.
const FOCUS_GRACE: Duration = Duration::from_millis(600);
/// How long the copied row stays highlighted before the window closes. Long
/// enough to register, short enough not to feel like a wait — 160 ms was the
/// former, and it is below what the eye catches on a colour change alone.
const COPY_FLASH: Duration = Duration::from_millis(260);
/// Cadence of the Matrix rain: about twelve frames a second. The rain falls a
/// row or three per second, so more frames would redraw the whole window for
/// no visible gain.
const RAIN_TICK: Duration = Duration::from_millis(83);
/// How long quitting waits for the event loop to stop on its own before the
/// process is ended regardless. See `State::quit`.
const QUIT_GRACE: Duration = Duration::from_millis(1500);
/// Between handing the focus back and pressing Ctrl+V for the user: long
/// enough for the previous application to be active again, short enough to
/// feel like the entry landed straight away.
const PASTE_DELAY: Duration = Duration::from_millis(150);
/// Cross-fade of the selection highlight. Short enough to feel immediate,
/// long enough to read as a movement rather than a jump.
const SELECT_FADE: Duration = Duration::from_millis(110);

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
    /// What the list shows, materialised. Below three characters it is the
    /// in-memory window filtered; at three or more it is the answer of a
    /// trigram query over the whole database — which is what lets the history
    /// outgrow memory. Rebuilt only on a query change or a capture, never in
    /// `view()`.
    pub visible: Vec<ClipItem>,
    pub query: String,
    /// What the detail panel shows for the selected entry.
    pub preview: Preview,
    /// Content hash of the entry `preview` was built for, so it is rebuilt only
    /// when the selection really lands on another entry — not on every arrow
    /// press that ends where it started, nor on a refilter that kept it.
    pub preview_key: Option<u64>,
    /// Seconds since start, sampled on each rain tick. The rain is drawn from
    /// this alone, so it only moves when a tick says so.
    pub rain_t: f32,
    /// The right-hand panel shows the settings instead of the preview.
    pub settings_open: bool,
    /// Only entries of this type are listed; `None` lists them all.
    pub kind_filter: Option<Kind>,
    /// Per-type counts for the filter pills, worked out in `refilter`.
    pub counts: FilterCounts,
    /// Where configuration, database, images and log live. Found once at boot:
    /// finding it checks the disk for a portable configuration, which has no
    /// place in a frame.
    pub data_dir: Option<std::path::PathBuf>,
    pub selected: usize,
    /// The selected index, animated. Each row derives its highlight from the
    /// distance to this value, so the outgoing row fades out while the
    /// incoming one fades in — a slide would need an overlay, and overlays
    /// break repainting here.
    pub selection: Animation<f32>,
    pub hovered: Option<usize>,
    pub scroll_y: f32,
    pub viewport_h: f32,
    pub flash: Option<(String, Instant)>,
    /// The entry that was just copied: highlighted until the window closes,
    /// and moved back to the top once it has. Held as an id rather than a row
    /// index so it survives any reordering.
    pub copied: Option<Copied>,
    setter: Setter,
    /// Absent when the database could not be opened: the application keeps
    /// working in memory rather than refusing to start.
    store: Option<Store>,
    config: Config,
    /// The window is created **once**, on the first open, then shown and
    /// hidden rather than destroyed: a palette has to appear instantly, and
    /// recreating a surface on every shortcut press is not free.
    window: Option<window::Id>,
    window_shown: bool,
    /// Set while a press on an image row might turn into a drag-out; see
    /// [`DragWatch`].
    drag_watch: Option<DragWatch>,
    opened_at: Option<Instant>,
    close_on_blur: bool,
    shot_path: Option<std::path::PathBuf>,
    shot_after: Duration,
    shot_done: bool,
    /// Kept alive: dropping it would unregister the global shortcut.
    _hotkeys: hotkey::Hotkeys,
}

/// The type filters, in the order of their pills and of Ctrl+1 to Ctrl+6.
pub const FILTERS: [(&str, Option<Kind>); 6] = [
    ("Tout", None),
    ("Texte", Some(Kind::Text)),
    ("Code", Some(Kind::Code)),
    ("URL", Some(Kind::Url)),
    ("Images", Some(Kind::Image)),
    ("Fichiers", Some(Kind::Files)),
];

/// How many entries each type filter would show for the current search.
#[derive(Debug, Clone, Copy, Default)]
pub struct FilterCounts {
    pub all: usize,
    text: usize,
    url: usize,
    code: usize,
    image: usize,
    files: usize,
}

impl FilterCounts {
    fn of_items(items: &[ClipItem]) -> Self {
        let mut counts = Self {
            all: items.len(),
            ..Self::default()
        };
        for item in items {
            match item.kind {
                Kind::Text => counts.text += 1,
                Kind::Url => counts.url += 1,
                Kind::Code => counts.code += 1,
                Kind::Image => counts.image += 1,
                Kind::Files => counts.files += 1,
            }
        }
        counts
    }

    pub fn of(&self, kind: Kind) -> usize {
        match kind {
            Kind::Text => self.text,
            Kind::Url => self.url,
            Kind::Code => self.code,
            Kind::Image => self.image,
            Kind::Files => self.files,
        }
    }
}

/// The entry a copy is currently confirming.
///
/// The hash rides along because the two things done with it need different
/// keys: the in-memory list is addressed by id, its database row by content
/// hash. Looking the hash up later would not work — a search result carries
/// the row id, which matches nothing in the loaded window.
#[derive(Debug, Clone, Copy)]
pub struct Copied {
    pub id: u64,
    pub hash: u64,
}

/// A row (image or files) that might be the start of a drag-out, armed on
/// press and resolved on the next few moves.
///
/// `origin` starts empty: `mouse_area`'s `on_press` carries no position, so
/// the first move sample after the press is taken as the baseline instead of
/// the press point itself — close enough, since a press and its first move
/// land a frame apart at most.
#[derive(Debug, Clone, Copy)]
struct DragWatch {
    index: usize,
    origin: Option<iced::Point>,
}

/// The detail panel's content, ready to draw.
pub enum Preview {
    Empty,
    Text {
        /// At most `PREVIEW_CHARS` characters of the entry.
        body: String,
        code: bool,
        /// The language the detector recognised, when it was sure of one.
        lang: Option<copycopy_core::lang::Lang>,
        /// Byte ranges of the web links inside `body`, found when the preview
        /// is built so the view only has to slice.
        links: Vec<std::ops::Range<usize>>,
        /// Counted once, on the whole entry, when the preview is built.
        chars: usize,
        lines: usize,
        cut: bool,
    },
    Image {
        /// Holds the PNG bytes; iced decodes them on its side and caches the
        /// result, so drawing it again costs nothing.
        handle: iced::widget::image::Handle,
        size: Option<(u32, u32)>,
    },
    Files(Vec<String>),
    /// The entry exists but its content could not be read — an image whose
    /// file has gone, typically.
    Unavailable(String),
}

impl Preview {
    fn of(item: &ClipItem) -> Self {
        match &item.payload {
            Payload::Text(text) => {
                let chars = text.chars().count();
                // Bounded to the opening of the text, so this costs the same on
                // a line as on a megabyte of log.
                let lang = copycopy_core::lang::detect(text);
                let body = match text.char_indices().nth(PREVIEW_CHARS) {
                    Some((end, _)) => text[..end].to_string(),
                    None => text.clone(),
                };
                let links = copycopy_core::links::find(&body);
                Preview::Text {
                    body,
                    links,
                    code: lang.is_some() || item.kind == copycopy_core::Kind::Code,
                    lang,
                    chars,
                    lines: text.lines().count().max(1),
                    cut: chars > PREVIEW_CHARS,
                }
            }
            Payload::Image { data, size } => match data.load() {
                Ok(bytes) => Preview::Image {
                    handle: iced::widget::image::Handle::from_bytes(bytes),
                    size: *size,
                },
                Err(e) => Preview::Unavailable(format!("image illisible : {e}")),
            },
            Payload::Files(paths) => {
                Preview::Files(paths.iter().map(|p| p.display().to_string()).collect())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Query(String),
    Select(usize),
    /// Delete the entry at that row, from the cross.
    Delete(usize),
    /// Copy the entry at that row, from its copy button.
    CopyRow(usize),
    /// Switch between the light and dark palettes, from the header icon.
    ToggleTheme,
    /// Enter or leave the Matrix palette, from the footer.
    ToggleMatrix,
    /// Advance the Matrix rain by one frame.
    RainTick,
    /// Show or hide the settings in the right-hand panel, from the ⋮ button.
    ToggleSettings,
    /// Pick a theme from the settings.
    SetTheme(theme::Mode),
    /// Turn auto-paste on or off, from the settings.
    SetAutoPaste(bool),
    /// Start with the session, or stop doing so, from the settings.
    SetAutostart(bool),
    /// List one type only, or everything with `None`, from the filter pills.
    SetKindFilter(Option<Kind>),
    /// Hide the window, from the header cross. Capture carries on.
    Close,
    /// Stop the resident, from the settings.
    Quit,
    /// Open a web link from the preview in the default browser.
    OpenLink(String),
    Hover(usize),
    Unhover(usize),
    /// The pointer has moved over a draggable row (image or files) while a
    /// possible drag-out is being watched.
    RowDragMoved(usize, iced::Point),
    /// The button lifted, or the pointer left, before a drag-out started.
    RowDragReleased,
    /// The native drag session ended; `true` when the row was actually
    /// dropped somewhere, `false` on a cancelled drag.
    RowDragFinished(bool),
    Activate,
    /// The copy confirmation has been shown long enough; close.
    FinishCopy,
    /// A frame tick, only while the selection is animating.
    Redraw,
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
        let query = self.query.trim();
        let long_enough = query.chars().count() >= Store::MIN_QUERY;

        self.visible = match (&self.store, long_enough) {
            // Long enough for the trigram index: ask the database, so entries
            // older than the in-memory window are found too.
            (Some(store), true) => match store.search(query, CAPACITY) {
                Ok(items) => items,
                Err(e) => {
                    eprintln!("search failed ({e}) — falling back to memory");
                    self.filter_memory(query)
                }
            },
            _ => self.filter_memory(query),
        };

        // Counted before the type filter narrows the list: each pill tells how
        // many entries it would show for the current search.
        self.counts = FilterCounts::of_items(&self.visible);
        if let Some(kind) = self.kind_filter {
            self.visible.retain(|it| it.kind == kind);
        }

        pinned_first(&mut self.visible);
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
        self.update_preview();
    }

    /// Pins or unpins an entry in the database and in the loaded window, so
    /// both agree without reloading.
    fn set_pinned(&mut self, hash: u64, pinned: bool) {
        if let Some(store) = &self.store {
            if let Err(e) = store.set_pinned(hash, pinned) {
                eprintln!("could not persist the pin: {e}");
            }
        }
        if let Some(index) = self.history.items().iter().position(|it| it.hash == hash) {
            self.history.toggle_pin(index);
        }
    }

    /// Drops the selected entry, from the database and from the loaded window.
    fn delete_selected(&mut self) -> Task<Message> {
        if let Some(hash) = self.visible.get(self.selected).map(|it| it.hash) {
            self.forget(hash);
            self.refilter();
        }
        Task::none()
    }

    /// Removes an entry from the database and from the loaded window.
    fn forget(&mut self, hash: u64) {
        if let Some(store) = &self.store {
            if let Err(e) = store.delete(hash) {
                eprintln!("could not delete the entry: {e}");
            }
        }
        if let Some(index) = self.history.items().iter().position(|it| it.hash == hash) {
            self.history.remove(index);
        }
    }

    /// The loaded window, filtered by subsequence. Permissive on purpose: it
    /// answers the first two characters, where a trigram index cannot.
    fn filter_memory(&self, query: &str) -> Vec<ClipItem> {
        let q = query.to_lowercase();
        self.history
            .items()
            .iter()
            .filter(|it| q.is_empty() || subsequence(&it.preview.to_lowercase(), &q))
            .cloned()
            .collect()
    }

    /// Moves the selection and starts the cross-fade.
    fn select(&mut self, index: usize) {
        self.selected = index;
        self.selection.go_mut(index as f32, Instant::now());
        self.update_preview();
    }

    /// Rebuilds the panel content when the selection is on another entry.
    ///
    /// Here and not in `view()`: for an image it means reading a file, and for
    /// a long text walking it to count and cut. Done per frame, either would
    /// cost the frame rate the materialised `visible` list exists to protect.
    fn update_preview(&mut self) {
        let item = self.visible.get(self.selected);
        let key = item.map(|it| it.hash);
        if key == self.preview_key {
            return;
        }
        self.preview_key = key;
        self.preview = item.map_or(Preview::Empty, Preview::of);
    }

    /// The colours of the active theme. Read from the configuration, so the
    /// choice lives in one place only.
    pub fn palette(&self) -> theme::Palette {
        self.config.theme.palette()
    }

    /// Corner radius of the card, zero when the configuration asks for square
    /// corners. Read by `view` and by the window's own `transparent` flag, which
    /// has to agree with it: a rounded card over an opaque window would show the
    /// clear colour in the corners.
    pub fn card_radius(&self) -> f32 {
        if self.config.rounded {
            theme::CARD_RADIUS
        } else {
            0.0
        }
    }

    pub fn theme_mode(&self) -> theme::Mode {
        self.config.theme
    }

    pub fn hotkey(&self) -> &str {
        &self.config.hotkey
    }

    pub fn auto_paste(&self) -> bool {
        self.config.auto_paste
    }

    pub fn autostart(&self) -> bool {
        self.config.autostart
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
        let len = self.visible.len();
        if len == 0 {
            return Task::none();
        }
        let next = ((self.selected as isize + delta).rem_euclid(len as isize)) as usize;
        self.select(next);
        self.reveal_selected()
    }

    /// Puts the list back to its opening state: no query, no scroll, selection
    /// on the first row. Done on the way **in**, never on the way out, so none
    /// of it is ever drawn over a window that is still disappearing.
    fn reset_for_open(&mut self) {
        self.query.clear();
        self.settings_open = false;
        self.kind_filter = None;
        self.selected = 0;
        self.selection = Animation::new(0.0).duration(SELECT_FADE);
        self.hovered = None;
        self.scroll_y = 0.0;
        self.refilter();
    }

    fn show(&mut self) -> Task<Message> {
        self.reset_for_open();
        if let Some(id) = self.window {
            if self.window_shown {
                return Task::none();
            }
            // Only when the window really opens: noted while it already shows,
            // the target would be copycopy itself.
            copycopy_platform::paste::remember_target();
            self.window_shown = true;
            self.opened_at = Some(Instant::now());
            return Task::batch([
                window::set_mode(id, window::Mode::Windowed),
                window::gain_focus(id),
                iced::widget::operation::focus(SEARCH_ID),
            ]);
        }
        copycopy_platform::paste::remember_target();
        self.open_window(true)
    }

    /// Creates the window. Called once, on the first open.
    fn open_window(&mut self, visible: bool) -> Task<Message> {
        let size = self
            .config
            .size
            .map(|(w, h)| iced::Size::new(w.max(MIN_WIDTH), h))
            .unwrap_or(iced::Size::new(WINDOW_SIZE.0, WINDOW_SIZE.1));
        let position = self
            .config
            .position
            .map(|(x, y)| window::Position::Specific(iced::Point::new(x, y)))
            .unwrap_or(window::Position::Centered);
        let (id, task) = window::open(window::Settings {
            size,
            min_size: Some(iced::Size::new(MIN_WIDTH, 320.0)),
            position,
            visible,
            decorations: false,
            // Must follow the card radius: the corners are only clean because
            // what sits outside the curve is transparent rather than cleared to
            // a colour. See `theme::CARD_RADIUS`.
            transparent: self.config.rounded,
            resizable: true,
            // The mark the taskbar and Alt-Tab show.
            icon: icon::window(),
            level: window::Level::AlwaysOnTop,
            exit_on_close_request: false,
            ..Default::default()
        });
        self.window = Some(id);
        self.window_shown = visible;
        self.opened_at = Some(Instant::now());
        task.map(Message::WindowOpened)
    }

    /// Hides the window without destroying it, and resets the search so the
    /// next open starts clean. The resident keeps running.
    fn hide(&mut self) -> Task<Message> {
        // Geometry is only written here: no need to touch the disk on every
        // pixel while the window is being dragged.
        self.config.save();
        self.bump_copied();
        // Nothing else visual is reset here. The window needs a rendered frame
        // or two to actually disappear, and whatever is reset now is drawn
        // during them: resetting the selection was seen as the highlight
        // leaving the row just copied and landing on the top one — the first
        // pinned entry, when there is one. `show` starts the next open clean
        // instead. Only `copied` goes now, so its timer stops firing.
        self.copied = None;
        if !self.window_shown {
            return Task::none();
        }
        self.window_shown = false;
        match self.window {
            Some(id) => window::set_mode(id, window::Mode::Hidden),
            None => Task::none(),
        }
    }

    /// Sends the entry that was just copied back to the top, in memory and on
    /// disk.
    ///
    /// Called on the way out, never when the copy happens: moving a row the
    /// instant it is clicked makes the list slip under the cursor, which is
    /// what made this feel wrong the first time round. `visible` is left
    /// exactly as the closing window still shows it — `show` rebuilds it from
    /// the new order on the next open, so the move is real but never seen
    /// happening.
    fn bump_copied(&mut self) {
        let Some(Copied { id, hash }) = self.copied else {
            return;
        };
        let at = SystemTime::now();
        if let Some(store) = &self.store {
            if let Err(e) = store.touch(hash, at) {
                eprintln!("could not move the entry back to the top: {e}");
            }
        }
        self.history.touch(id, at);
    }

    /// Stops the resident for good: the configuration is written and the daemon
    /// exits. Not to be confused with `hide`, which only puts the window away
    /// while capture carries on.
    /// Ends a copy once its confirmation has shown: the window goes and, with
    /// auto-paste on, the entry lands where the user was typing.
    fn finish_copy(&mut self) -> Task<Message> {
        let paste = self.config.auto_paste && copycopy_platform::paste::availability().is_ok();
        if paste {
            // Before hiding, while this window still holds the foreground:
            // Windows only lets the foreground process give the focus away.
            copycopy_platform::paste::restore_target();
        }
        let task = self.hide();
        if paste {
            // A thread, not the UI: the delay must not freeze the interface.
            std::thread::spawn(|| {
                std::thread::sleep(PASTE_DELAY);
                if let Err(e) = copycopy_platform::paste::send_paste() {
                    eprintln!("auto-paste failed: {e}");
                }
            });
        }
        task
    }

    /// Narrows the list to one type, or lists everything again with `None`.
    fn set_kind_filter(&mut self, kind: Option<Kind>) -> Task<Message> {
        self.kind_filter = kind;
        self.select(0);
        self.refilter();
        self.reveal_selected()
    }

    fn quit(&mut self) -> Task<Message> {
        self.config.save();
        // `iced::exit` only files a request that the event loop reads when it
        // next wakes. With a window open something wakes it within a frame;
        // a resident that never opened one sleeps on, and `--quit` was seen
        // answering "ok" and leaving the process running. So a fallback stops
        // the process if the loop has not let go shortly after. What it skips
        // is harmless: the configuration is saved just above, every database
        // write is already committed, and the system drops the global shortcut
        // with the process.
        std::thread::spawn(|| {
            std::thread::sleep(QUIT_GRACE);
            eprintln!("event loop did not stop, exiting the process");
            std::process::exit(0);
        });
        iced::exit()
    }

    fn toggle(&mut self) -> Task<Message> {
        if self.window_shown {
            self.hide()
        } else {
            self.show()
        }
    }

    fn activate(&mut self) -> Task<Message> {
        if self.copied.is_some() {
            eprintln!("copy: already confirming one, ignored");
            return Task::none(); // Already confirming; ignore a second Enter.
        }
        let Some(item) = self.visible.get(self.selected) else {
            eprintln!(
                "copy: nothing at row {} of {}",
                self.selected,
                self.visible.len()
            );
            return Task::none();
        };
        let payload = item.payload.clone();
        let copied = Copied {
            id: item.id,
            hash: item.hash,
        };
        match self.setter.set(&payload) {
            Ok(()) => {
                // Flash the row, then close. The window disappearing is the
                // real confirmation, but on its own it leaves a doubt about
                // *which* entry went to the clipboard. Nothing moves yet: the
                // entry goes back to the top in `hide`, once nobody is looking.
                eprintln!("copy: row {} sent to the clipboard", self.selected);
                self.copied = Some(copied);
                Task::none()
            }
            Err(e) => {
                // Console as well as the footer: a footer line lasts three
                // seconds and is easy to miss, and this is the failure that
                // reads as "nothing happened".
                eprintln!("copy failed: {e}");
                self.flash(format!("échec de la copie : {e}"));
                Task::none()
            }
        }
    }
}

/// Pinned entries float to the top. The sort is stable, so recency is preserved
/// inside each group. The database already returns rows in this order; this
/// keeps the in-memory path consistent with it.
fn pinned_first(items: &mut [ClipItem]) {
    items.sort_by_key(|it| !it.pinned);
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
    /// Open with the settings showing. Only useful to check them in a
    /// screenshot: they are otherwise reached with a click.
    settings: bool,
    /// Open with a type filter applied (`text`, `code`, `url`, `image`, `files`).
    /// Only useful to check the filter band in a screenshot.
    filter: Option<Kind>,
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
        settings: a.iter().any(|x| x == "--settings"),
        filter: val("--filter").and_then(|v| match v.as_str() {
            "text" => Some(Kind::Text),
            "code" => Some(Kind::Code),
            "url" => Some(Kind::Url),
            "image" | "images" => Some(Kind::Image),
            "files" => Some(Kind::Files),
            _ => None,
        }),
    }
}

fn boot() -> (State, Task<Message>) {
    let args = parse_args();
    let mut config = Config::load();
    // Before the file is written, so a preference the system no longer agrees
    // with is corrected on disk in the same breath.
    let autostart = autostart::reconcile(config.autostart);
    if autostart != config.autostart {
        println!("démarrage automatique : entrée retirée hors de l'application");
        config.autostart = autostart;
        config.save();
    }
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
    println!(
        "fonts: system{}",
        if loaded.paths.is_empty() {
            String::new()
        } else {
            format!(" + {} fallback file(s)", loaded.paths.len())
        }
    );

    // `--demo` gets its own database rather than bypassing persistence: the
    // point of the demo is to exercise the real behaviour — pinning, restart,
    // search — without pouring sample data into the real history.
    let dir = config::base_dir().map(|dir| if args.demo { dir.join("demo") } else { dir });
    let store = match &dir {
        Some(dir) => match Store::open(dir) {
            Ok(store) => {
                println!("database: {}", dir.join("copycopy.db").display());
                // Pictures whose rows are long gone: nothing can reach them,
                // and nothing else would ever remove them.
                match store.sweep_orphan_images() {
                    Ok(0) => {}
                    Ok(n) => println!("{n} orphan image file(s) removed"),
                    Err(e) => eprintln!("could not sweep orphan images: {e}"),
                }
                Some(store)
            }
            Err(e) => {
                eprintln!("database unavailable ({e}) — history will not persist");
                None
            }
        },
        None => None,
    };

    let mut history = History::new(CAPACITY);
    match &store {
        Some(store) => {
            if args.demo && store.count().unwrap_or(0) == 0 {
                let mut seed = History::new(CAPACITY);
                seed_demo(&mut seed);
                // Oldest first, so the row ids follow the same order and break
                // ties between entries written in the same second.
                for item in seed.items().iter().rev() {
                    let _ = store.insert(item);
                }
                println!("demo data seeded");
            }
            // Oldest first again: pushing them replays the original order and
            // the in-memory list comes out newest first, as it would have live.
            match store.recent(CAPACITY) {
                Ok(items) => {
                    for item in items.into_iter().rev() {
                        history.push_stored(item);
                    }
                    println!("{} entries restored", history.len());
                }
                Err(e) => eprintln!("could not read the history: {e}"),
            }
        }
        // No database: the demo still has to show something.
        None if args.demo => seed_demo(&mut history),
        None => {}
    }

    let mut state = State {
        history,
        visible: Vec::new(),
        query: args.query,
        preview: Preview::Empty,
        preview_key: None,
        rain_t: 0.0,
        settings_open: args.settings,
        kind_filter: args.filter,
        counts: FilterCounts::default(),
        data_dir: dir.clone(),
        selected: 0,
        selection: Animation::new(0.0).duration(SELECT_FADE),
        hovered: None,
        scroll_y: 0.0,
        viewport_h: 430.0,
        flash: None,
        copied: None,
        setter: Setter::new(),
        store,
        config: config.clone(),
        window: None,
        window_shown: false,
        drag_watch: None,
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

/// Every message passes through here. When one moved the selection onto
/// another entry, the panel goes back to its top: the scroll offset belongs to
/// the widget, not to the entry, so a long text read halfway down would open
/// the next entry halfway down as well.
fn update(state: &mut State, message: Message) -> Task<Message> {
    let before = state.preview_key;
    let task = handle(state, message);
    if state.preview_key == before {
        return task;
    }
    Task::batch([
        task,
        iced::widget::operation::snap_to(PREVIEW_ID, scrollable::RelativeOffset::START),
    ])
}

fn handle(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Query(q) => {
            state.query = q;
            state.select(0);
            state.refilter();
            state.reveal_selected()
        }
        Message::Select(i) => {
            state.select(i);
            state.drag_watch = state
                .visible
                .get(i)
                .filter(|item| matches!(item.kind, Kind::Image | Kind::Files))
                .map(|_| DragWatch {
                    index: i,
                    origin: None,
                });
            Task::none()
        }
        Message::RowDragMoved(index, pos) => {
            let Some(watch) = state.drag_watch.as_mut().filter(|w| w.index == index) else {
                return Task::none();
            };
            let Some(origin) = watch.origin else {
                watch.origin = Some(pos);
                return Task::none();
            };
            // A few pixels of slack: without it, the drag would start on the
            // same tiny jitter that a plain click already produces.
            const THRESHOLD: f32 = 6.0;
            if origin.distance(pos) < THRESHOLD {
                return Task::none();
            }
            state.drag_watch = None;
            let paths = state
                .visible
                .get(index)
                .and_then(|item| match &item.payload {
                    Payload::Image { data, .. } => data.path().map(|p| vec![p.to_path_buf()]),
                    Payload::Files(paths) => Some(paths.clone()),
                    Payload::Text(_) => None,
                });
            let (Some(paths), Some(id)) = (paths, state.window) else {
                return Task::none();
            };
            window::run(id, move |window| drag::start(window, paths.clone()))
                .map(Message::RowDragFinished)
        }
        Message::RowDragReleased => {
            state.drag_watch = None;
            Task::none()
        }
        // Dropped, not merely released: dragging a row out already delivers
        // it, the same way opening a link already does — closing behind it
        // is what a paste-and-switch would have done by hand. A cancelled
        // drag leaves the window open, since nothing actually happened.
        Message::RowDragFinished(dropped) => {
            if dropped {
                state.hide()
            } else {
                Task::none()
            }
        }
        Message::Delete(index) => {
            state.select(index);
            state.delete_selected()
        }
        Message::CopyRow(index) => {
            state.select(index);
            state.activate()
        }
        Message::ToggleTheme => {
            state.config.theme = state.config.theme.toggled();
            // Saved at once, unlike the geometry which waits for the window to
            // close: a toggle is one deliberate click, not a stream of events.
            state.config.save();
            Task::none()
        }
        Message::ToggleMatrix => {
            state.config.theme = state.config.theme.matrix_toggled();
            state.config.save();
            Task::none()
        }
        Message::OpenLink(link) => {
            // Checked again here, whatever built the link: this string came out
            // of the clipboard, and only a web address may reach the system.
            if !copycopy_core::links::is_openable(&link) {
                eprintln!("link refused: {link:?}");
                return Task::none();
            }
            match open::that_detached(&link) {
                // The browser is where the user is going: get out of its way.
                Ok(()) => state.hide(),
                Err(e) => {
                    eprintln!("could not open {link}: {e}");
                    state.flash(format!("impossible d'ouvrir le lien : {e}"));
                    Task::none()
                }
            }
        }
        Message::ToggleSettings => {
            state.settings_open = !state.settings_open;
            Task::none()
        }
        Message::SetTheme(mode) => {
            state.config.theme = mode;
            state.config.save();
            Task::none()
        }
        Message::SetAutoPaste(on) => {
            state.config.auto_paste = on;
            state.config.save();
            Task::none()
        }
        Message::SetAutostart(on) => {
            // The preference only changes if the system accepted it: a switch
            // that moves while nothing was written would be a lie.
            match autostart::set(on) {
                Ok(()) => {
                    state.config.autostart = on;
                    state.config.save();
                }
                Err(e) => {
                    eprintln!("autostart: {e}");
                    state.flash(format!("démarrage automatique : {e}"));
                }
            }
            Task::none()
        }
        Message::SetKindFilter(kind) => state.set_kind_filter(kind),
        Message::Close => state.hide(),
        Message::Quit => state.quit(),
        Message::RainTick => {
            state.rain_t = BOOT.get().map_or(0.0, |boot| boot.elapsed().as_secs_f32());
            Task::none()
        }
        Message::Hover(index) => {
            state.hovered = Some(index);
            Task::none()
        }
        Message::Unhover(index) => {
            // Only clear when the row being left is the one on record. Moving
            // up the list, the row being entered publishes before the one being
            // left — an unconditional clear would wipe the hover that just
            // arrived, which is why hovering looked random.
            if state.hovered == Some(index) {
                state.hovered = None;
            }
            Task::none()
        }
        Message::Activate => state.activate(),
        Message::FinishCopy => state.finish_copy(),
        Message::Redraw => Task::none(),
        Message::Scrolled(viewport) => {
            state.scroll_y = viewport.absolute_offset().y;
            state.viewport_h = viewport.bounds().height;
            Task::none()
        }
        Message::Backend(kind) => {
            // Console rather than the footer: it tells the developer which
            // backend won, and has no place in the window.
            println!("capture backend: {}", kind.label());
            Task::none()
        }
        Message::Captured(capture) => {
            // What we wrote to the clipboard ourselves is not a capture. The
            // content hash cannot tell: an image goes out re-encoded, so it
            // comes back with different bytes and used to land in the history
            // as a new entry on every paste-back.
            if state.setter.echoes(&capture.event) {
                return Task::none();
            }
            state.history.push(capture.event, capture.source);
            let written = match (&state.store, state.history.get(0)) {
                (Some(store), Some(item)) => {
                    match store.insert(item).and_then(|_| store.prune(CAPACITY)) {
                        Ok(_) => matches!(item.payload, Payload::Image { .. })
                            .then(|| store.image_path(item.hash)),
                        Err(e) => {
                            eprintln!("could not persist the entry: {e}");
                            None
                        }
                    }
                }
                _ => None,
            };
            // The picture is on disk now, so the entry can point at the file
            // instead of carrying the bytes: `refilter` clones what it keeps,
            // and it runs on every keystroke.
            if let Some(path) = written {
                state.history.offload_image(0, path);
            }
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
            ipc::QUIT => state.quit(),
            _ => Task::none(),
        },
        Message::WindowOpened(id) => {
            state.window = Some(id);
            state.opened_at = Some(Instant::now());
            state.window_shown = true;
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
                    state.window_shown = false;
                }
                Task::none()
            }
            window::Event::CloseRequested => state.hide(),
            window::Event::Unfocused => {
                let settled = state
                    .opened_at
                    .is_some_and(|t| t.elapsed() > FOCUS_GRACE);
                if state.close_on_blur && settled && state.window_shown && state.window == Some(id) {
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
        Key::Named(Named::Escape) if state.settings_open => {
            // One step back at a time: the settings first, the window next.
            state.settings_open = false;
            Task::none()
        }
        Key::Named(Named::Escape) => state.hide(),
        Key::Named(Named::Enter) => state.activate(),
        Key::Named(Named::ArrowDown) => state.move_selection(1),
        Key::Named(Named::ArrowUp) => state.move_selection(-1),
        Key::Named(Named::PageDown) => state.move_selection(6),
        // Delete alongside Ctrl-D: it is the key one reaches for.
        Key::Named(Named::Delete) => state.delete_selected(),
        Key::Named(Named::PageUp) => state.move_selection(-6),
        // Home and End are left to the search field: inside a text input,
        // moving the caret is what you expect.
        // `command` is the platform's own modifier: Cmd on macOS, Ctrl elsewhere.
        Key::Character(c) if modifiers.command() && c.as_str() == "q" => state.quit(),
        Key::Character(c) if ctrl => match c.as_str() {
            "n" => state.move_selection(1),
            "p" => state.move_selection(-1),
            "b" => {
                let Some(item) = state.visible.get(state.selected) else {
                    return Task::none();
                };
                let (hash, pinned) = (item.hash, !item.pinned);
                state.set_pinned(hash, pinned);
                // Without refiltering, the order would only change on the next
                // query or capture — the entry would appear to stay put.
                state.refilter();
                // It has just moved to the top, or back down: keep the cursor
                // on the entry rather than on the position it used to hold.
                if let Some(pos) = state.visible.iter().position(|it| it.hash == hash) {
                    state.select(pos);
                }
                state.reveal_selected()
            }
            "d" => state.delete_selected(),
            // Ctrl+1 to Ctrl+6: the filter pills, left to right.
            digit @ ("1" | "2" | "3" | "4" | "5" | "6") => {
                let index = digit.parse::<usize>().unwrap_or(1) - 1;
                state.set_kind_filter(FILTERS[index].1)
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
    if state.selection.is_animating(Instant::now()) {
        subs.push(window::frames().map(|_| Message::Redraw));
    }
    if state.copied.is_some() {
        // Fires once: the subscription disappears with `copied` when the
        // window closes.
        subs.push(iced::time::every(COPY_FLASH).map(|_| Message::FinishCopy));
    }
    // Only while it can be seen: in another theme, or with the window hidden,
    // the rain costs nothing at all.
    if state.config.theme == theme::Mode::Matrix && state.window_shown {
        subs.push(iced::time::every(RAIN_TICK).map(|_| Message::RainTick));
    }
    if state.shot_path.is_some() && !state.shot_done {
        subs.push(iced::time::every(Duration::from_millis(300)).map(|_| Message::Shot));
    }
    Subscription::batch(subs)
}

/// Drives the defaults of the widgets iced styles itself — the caret and the
/// text selection of the search field — so they follow the palette too.
fn theme_of(state: &State, _window: window::Id) -> Theme {
    match state.config.theme {
        theme::Mode::Dark | theme::Mode::Purpledream | theme::Mode::Matrix => Theme::Dark,
        theme::Mode::Light | theme::Mode::Aalto => Theme::Light,
    }
}

fn root_style(state: &State, _theme: &Theme) -> iced::theme::Style {
    theme::root(state.palette(), state.config.rounded)
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


fn main() -> iced::Result {
    // First, before anything prints.
    console::prepare();
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

    // Before claiming the pipe: this writes a system entry and exits, it never
    // becomes a resident. An installer turns it on, an uninstaller must be able
    // to turn it off with no window and no running copy.
    if let Some(on) = argv
        .iter()
        .position(|a| a == "--autostart")
        .and_then(|i| argv.get(i + 1))
        .and_then(|v| match v.as_str() {
            "on" | "true" => Some(true),
            "off" | "false" => Some(false),
            _ => None,
        })
    {
        match autostart::set(on) {
            Ok(()) => {
                let mut config = Config::load();
                config.autostart = on;
                config.save();
                println!(
                    "démarrage automatique : {}",
                    if on { "activé" } else { "désactivé" }
                );
            }
            Err(e) => eprintln!("démarrage automatique : {e}"),
        }
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
    let result = app.run();
    // Written as soon as the event loop hands back: if the process then lingers,
    // the log shows the wait is in tearing things down, not in the loop.
    eprintln!("resident stopped");
    result
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

        let mut items: Vec<ClipItem> = history.items().iter().cloned().collect();
        pinned_first(&mut items);

        let order: Vec<&str> = items.iter().map(|it| it.preview.as_str()).collect();
        assert_eq!(
            order,
            ["c", "a", "d", "b"],
            "pinned first, recency kept inside each group"
        );
    }
}
