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

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn start(window: &dyn Window, path: PathBuf) {
    let icon = drag::Image::File(path.clone());
    if let Err(e) = drag::start_drag(
        &window,
        drag::DragItem::Files(vec![path]),
        icon,
        |_result, _cursor_position| {},
        drag::Options::default(),
    ) {
        eprintln!("glisser-déposer : {e}");
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn start(_window: &dyn Window, _path: PathBuf) {}
