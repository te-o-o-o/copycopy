//! Dragging a row straight out of the window, into another application — a
//! chat window, an editor, a file manager.
//!
//! Built on the `drag` crate, which wraps the native drag-source APIs
//! (Windows OLE `IDropSource`, macOS `NSDraggingSession`). Its own
//! documentation rules Linux out: "winit currently cannot leverage this
//! crate on Linux yet" — it needs a real GTK window there, which an iced
//! window is not. Windows and macOS only; a row still copies normally
//! everywhere, drag or no drag.
//!
//! Only files travel: the crate's `DragItem::Data` — raw bytes, no file on
//! disk — is a documented no-op on Windows, real only on macOS, so it would
//! carry nothing on the platform this ships to first. That leaves images and
//! file entries — the two kinds a row can point at real paths for. An image
//! lives on disk by the time it can be selected (see `Message::Captured` in
//! `main.rs`); a file entry already carries the paths it was copied with.
//! Text, code and URL rows stay copy-only rather than routed through a
//! throwaway temp file for a platform this can't yet verify.

use std::path::PathBuf;

use iced::window::Window;

/// Longest side of the ghost image shown under the cursor while dragging.
/// The source file can be a full screenshot; carrying it around at that size
/// looks like the drag glitched rather than like a hand holding a picture.
#[cfg(any(target_os = "windows", target_os = "macos"))]
const GHOST: u32 = 160;

/// Starts the OS drag for `paths` and blocks until it ends — `DoDragDrop`
/// does not return early, and this is called from inside `window::run`
/// precisely so that blocking happens off the update loop's own return path.
///
/// Returns whether something was actually dropped somewhere, so the caller
/// can close the window exactly when a paste-and-switch by hand would have
/// left it: on a real drop, not on a cancelled one — and not on a drop back
/// into our own window either, on Windows (see `win32::contains`): nothing
/// was exported, so it must count the same as a cancel.
#[cfg(target_os = "windows")]
pub fn start(window: &dyn Window, paths: Vec<PathBuf>) -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let hwnd = win32::hwnd(window);
    let icon = ghost(&paths);
    let dropped = Arc::new(AtomicBool::new(false));
    let flag = dropped.clone();
    if let Err(e) = drag::start_drag(
        &window,
        drag::DragItem::Files(paths),
        icon,
        move |result, cursor_position| {
            let real = matches!(result, drag::DragResult::Dropped)
                && !hwnd.is_some_and(|h| win32::contains(h, &cursor_position));
            flag.store(real, Ordering::SeqCst);
        },
        drag::Options::default(),
    ) {
        eprintln!("glisser-déposer : {e}");
        return false;
    }
    dropped.load(Ordering::SeqCst)
}

#[cfg(target_os = "macos")]
pub fn start(window: &dyn Window, paths: Vec<PathBuf>) -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let icon = ghost(&paths);
    let dropped = Arc::new(AtomicBool::new(false));
    let flag = dropped.clone();
    if let Err(e) = drag::start_drag(
        &window,
        drag::DragItem::Files(paths),
        icon,
        move |result, _cursor_position| {
            flag.store(
                matches!(result, drag::DragResult::Dropped),
                Ordering::SeqCst,
            );
        },
        drag::Options::default(),
    ) {
        eprintln!("glisser-déposer : {e}");
        return false;
    }
    dropped.load(Ordering::SeqCst)
}

/// Whether a screen point still falls inside our own window — queried fresh
/// at drop time rather than cached, since the window can itself be dragged
/// or resized during the gesture. The raw `HWND`, not `window`, is what
/// crosses into the drop closure: that closure must be `'static`, and a
/// borrowed `&dyn Window` cannot be.
#[cfg(target_os = "windows")]
mod win32 {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

    pub fn hwnd(window: &dyn super::Window) -> Option<isize> {
        use iced::window::raw_window_handle::RawWindowHandle;
        match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
            _ => None,
        }
    }

    pub fn contains(hwnd: isize, pos: &drag::CursorPosition) -> bool {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let ok = unsafe { GetWindowRect(hwnd as _, &mut rect) };
        ok != 0
            && pos.x >= rect.left
            && pos.x < rect.right
            && pos.y >= rect.top
            && pos.y < rect.bottom
    }
}

/// The ghost shown under the cursor. A single picture gets a scaled-down
/// copy of itself, encoded in memory — the source file on disk stays the
/// drag payload untouched, only this preview shrinks. Anything else (several
/// files, or one that is not a picture) falls back to that first path's own
/// file, which the OS already knows how to draw an icon for.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn ghost(paths: &[PathBuf]) -> drag::Image {
    if let [only] = paths {
        if let Some(thumb) = thumbnail(only) {
            return thumb;
        }
    }
    // `paths` is never empty: both callers build it from a `ClipItem` that
    // is already known to carry at least one path.
    drag::Image::File(paths[0].clone())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn thumbnail(path: &std::path::Path) -> Option<drag::Image> {
    let scaled = image::open(path).ok()?.thumbnail(GHOST, GHOST);
    let mut bytes = Vec::new();
    scaled
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .ok()?;
    Some(drag::Image::Raw(bytes))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn start(_window: &dyn Window, _paths: Vec<PathBuf>) -> bool {
    false
}
