//! The mark handed to each system: the window icon, the Windows taskbar's,
//! the macOS Dock's. The drawing itself lives in `mark`.

use crate::mark::rgba;

/// The icon iced hands to the window system. `None` rather than a panic: a
/// window without its mark is a blemish, not a reason to refuse to start.
pub fn window() -> Option<iced::window::Icon> {
    const SIZE: u32 = 64;
    iced::window::icon::from_rgba(rgba(SIZE), SIZE, SIZE).ok()
}

/// Sets the big icon (`ICON_BIG`), the one the taskbar shows. iced only passes
/// `window()` to winit's `with_window_icon`, which on Windows is `ICON_SMALL`
/// alone — the title bar's — and winit's `with_taskbar_icon` is not reachable
/// through iced's settings. The `.exe` now carries the mark as a resource
/// (`build.rs`), but the taskbar reads the window's icon first.
#[cfg(target_os = "windows")]
pub fn set_taskbar(window: &dyn iced::window::Window) {
    use iced::window::raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateIcon, GetSystemMetrics, SendMessageW, ICON_BIG, SM_CXICON, WM_SETICON,
    };

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    // Drawn at the size the system asks for, which follows the DPI, rather
    // than scaled down from a fixed one.
    let size = (unsafe { GetSystemMetrics(SM_CXICON) }).clamp(16, 256) as u32;
    // `CreateIcon` wants BGRA; the alpha channel carries the shape, so the
    // AND mask stays empty — its rows padded to 16 bits, as a monochrome
    // bitmap's are, or 40 pixels at 125 % would read past the end.
    let mut bgra = rgba(size);
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let mask = vec![0u8; (size.div_ceil(16) * 2 * size) as usize];
    let icon = unsafe {
        CreateIcon(
            std::ptr::null_mut(),
            size as i32,
            size as i32,
            1,
            32,
            mask.as_ptr(),
            bgra.as_ptr(),
        )
    };
    if icon.is_null() {
        return;
    }
    // Never destroyed: the window keeps it for the life of the process.
    unsafe {
        SendMessageW(
            handle.hwnd.get() as _,
            WM_SETICON,
            ICON_BIG as usize,
            icon as isize,
        )
    };
}

/// Puts the mark in the Dock. winit ignores the window icon on macOS, and a
/// binary run outside a bundle — `target/release/copycopy` — has no `.icns`
/// either, so the Dock showed the generic executable.
///
/// The tile is drawn at 80 % of the canvas, centred: macOS icons leave that
/// margin, and a tile filling the whole square looks a size too big beside
/// the others.
#[cfg(target_os = "macos")]
pub fn set_dock() {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};

    const CANVAS: u32 = 512;
    const TILE: u32 = CANVAS * 4 / 5;

    // AppKit only on the main thread; anywhere else, no icon rather than a crash.
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let tile = rgba(TILE);
    let offset = (CANVAS - TILE) / 2;
    let mut canvas = image::RgbaImage::new(CANVAS, CANVAS);
    let line = (TILE * 4) as usize;
    for row in 0..TILE {
        let from = row as usize * line;
        let to = (((offset + row) * CANVAS + offset) * 4) as usize;
        canvas.as_mut()[to..to + line].copy_from_slice(&tile[from..from + line]);
    }
    let mut png = std::io::Cursor::new(Vec::new());
    if canvas.write_to(&mut png, image::ImageFormat::Png).is_err() {
        return;
    }
    let data = NSData::from_vec(png.into_inner());
    let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return;
    };
    // SAFETY: a real image, never `None`, which is what the setter is wary of.
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&icon)) };
}
