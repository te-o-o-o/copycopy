//! Dragging an image row straight out of the window, into another
//! application — a chat window, an editor, a file manager.
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
//! carry nothing on the platform this ships to first. Every image already
//! lives on disk by the time it can be selected (see `Message::Captured` in
//! `main.rs`), which is exactly what `DragItem::Files` wants.

use std::path::PathBuf;

use iced::window::Window;

/// Longest side of the ghost image shown under the cursor while dragging.
/// The source file can be a full screenshot; carrying it around at that size
/// looks like the drag glitched rather than like a hand holding a picture.
#[cfg(any(target_os = "windows", target_os = "macos"))]
const GHOST: u32 = 160;

/// Starts the OS drag for `path` and blocks until it ends — `DoDragDrop`
/// does not return early, and this is called from inside `window::run`
/// precisely so that blocking happens off the update loop's own return path.
///
/// Returns whether the file was actually dropped somewhere, so the caller
/// can close the window exactly when a paste-and-switch by hand would have
/// left it: on a real drop, not on a cancelled one.
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn start(window: &dyn Window, path: PathBuf) -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let icon = ghost(&path).unwrap_or_else(|| drag::Image::File(path.clone()));
    let dropped = Arc::new(AtomicBool::new(false));
    let flag = dropped.clone();
    if let Err(e) = drag::start_drag(
        &window,
        drag::DragItem::Files(vec![path]),
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

/// A scaled-down copy of the picture, encoded in memory. The source file on
/// disk stays the drag payload untouched — only the on-screen ghost shrinks.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn ghost(path: &std::path::Path) -> Option<drag::Image> {
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
pub fn start(_window: &dyn Window, _path: PathBuf) -> bool {
    false
}
