//! Backend Windows événementiel.
//!
//! `AddClipboardFormatListener` fait poster `WM_CLIPBOARDUPDATE` à une fenêtre
//! « message-only » : pas de sondage, pas de fenêtre visible, pas de hook.
//! C'est l'API prévue pour exactement notre cas d'usage.

use std::sync::mpsc::Sender;

use copycopy_core::ClipEvent;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, GetClipboardData, GetClipboardOwner,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowThreadProcessId,
    RegisterClassW, TranslateMessage, CW_USEDEFAULT, HWND_MESSAGE, MSG, WM_CLIPBOARDUPDATE,
    WNDCLASSW,
};

use crate::Capture;

const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIB: u32 = 8;
const MAX_BYTES: usize = 32 * 1024 * 1024;

// Le canal est rangé ici parce que la procédure de fenêtre Win32 est une
// fonction libre : elle n'a pas de `self` où le ranger.
thread_local! {
    static SENDER: std::cell::RefCell<Option<Sender<Capture>>> =
        const { std::cell::RefCell::new(None) };
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn spawn(tx: Sender<Capture>) -> Result<(), String> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("copycopy-win32".into())
        .spawn(move || {
            SENDER.with(|s| *s.borrow_mut() = Some(tx));
            match create_listener_window() {
                Ok(hwnd) => {
                    let _ = ready_tx.send(Ok(()));
                    pump(hwnd);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })
        .map_err(|e| e.to_string())?;

    ready_rx
        .recv()
        .map_err(|_| "le thread Win32 s'est arrêté".to_string())?
}

fn create_listener_window() -> Result<HWND, String> {
    unsafe {
        let class_name = wide("copycopy_clipboard_listener");
        let instance = GetModuleHandleW(std::ptr::null());

        let class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        // Une classe déjà enregistrée n'est pas une erreur : on réutilise.
        RegisterClassW(&class);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide("copycopy").as_ptr(),
            0,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            HWND_MESSAGE, // fenêtre message-only : jamais affichée
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err("CreateWindowExW a échoué".into());
        }
        if AddClipboardFormatListener(hwnd) == 0 {
            return Err("AddClipboardFormatListener a échoué".into());
        }
        Ok(hwnd)
    }
}

fn pump(_hwnd: HWND) {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        if let Some((event, source)) = unsafe { read_clipboard(hwnd) } {
            SENDER.with(|s| {
                if let Some(tx) = s.borrow().as_ref() {
                    let _ = tx.send(Capture { event, source });
                }
            });
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Le presse-papier est une ressource globale verrouillée par un seul process à
/// la fois : une autre application peut le tenir au moment où on arrive.
unsafe fn open_clipboard_retrying(hwnd: HWND) -> bool {
    for attempt in 0..10 {
        if unsafe { OpenClipboard(hwnd) } != 0 {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10 * (attempt + 1)));
    }
    false
}

unsafe fn read_clipboard(hwnd: HWND) -> Option<(ClipEvent, String)> {
    if !unsafe { open_clipboard_retrying(hwnd) } {
        return None;
    }
    let result = unsafe { read_clipboard_locked() };
    unsafe { CloseClipboard() };
    result
}

unsafe fn read_clipboard_locked() -> Option<(ClipEvent, String)> {
    // Les gestionnaires de mots de passe posent ces formats pour demander aux
    // gestionnaires de presse-papier de ne pas retenir le contenu. On obéit.
    let exclude = unsafe { RegisterClipboardFormatW(wide("ExcludeClipboardContentFromMonitorProcessing").as_ptr()) };
    let can_include = unsafe { RegisterClipboardFormatW(wide("CanIncludeInClipboardHistory").as_ptr()) };
    if exclude != 0 && unsafe { IsClipboardFormatAvailable(exclude) } != 0 {
        return None;
    }
    if can_include != 0 && unsafe { IsClipboardFormatAvailable(can_include) } != 0 {
        // Présent avec la valeur 0 = « ne pas historiser ».
        if let Some(bytes) = unsafe { clipboard_bytes(can_include) } {
            if bytes.first().is_some_and(|b| *b == 0) {
                return None;
            }
        }
    }

    let source = unsafe { owner_process_name() }.unwrap_or_default();

    // PNG d'abord : c'est ce que posent les navigateurs et l'outil Capture.
    let png_format = unsafe { RegisterClipboardFormatW(wide("PNG").as_ptr()) };
    if png_format != 0 && unsafe { IsClipboardFormatAvailable(png_format) } != 0 {
        if let Some(png) = unsafe { clipboard_bytes(png_format) } {
            if !png.is_empty() {
                let size = crate::png_size(&png);
                return Some((ClipEvent::Image { png, size }, source));
            }
        }
    }

    if unsafe { IsClipboardFormatAvailable(CF_HDROP) } != 0 {
        if let Some(paths) = unsafe { read_hdrop() } {
            if !paths.is_empty() {
                return Some((ClipEvent::Files(paths), source));
            }
        }
    }

    if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } != 0 {
        if let Some(text) = unsafe { read_unicode_text() } {
            if !text.trim().is_empty() {
                return Some((ClipEvent::Text(text), source));
            }
        }
    }

    // Dernier recours pour les images : un DIB brut, qu'on repackage en BMP
    // (le DIB n'est qu'un BMP privé de ses 14 octets d'en-tête de fichier)
    // pour le convertir en PNG.
    if unsafe { IsClipboardFormatAvailable(CF_DIB) } != 0 {
        if let Some(dib) = unsafe { clipboard_bytes(CF_DIB) } {
            if let Some(png) = dib_to_png(&dib) {
                let size = crate::png_size(&png);
                return Some((ClipEvent::Image { png, size }, source));
            }
        }
    }

    None
}

unsafe fn clipboard_bytes(format: u32) -> Option<Vec<u8>> {
    let handle = unsafe { GetClipboardData(format) };
    if handle.is_null() {
        return None;
    }
    let ptr = unsafe { GlobalLock(handle as _) };
    if ptr.is_null() {
        return None;
    }
    let len = unsafe { GlobalSize(handle as _) };
    let out = if len == 0 || len > MAX_BYTES {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }.to_vec())
    };
    unsafe { GlobalUnlock(handle as _) };
    out
}

unsafe fn read_unicode_text() -> Option<String> {
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if handle.is_null() {
        return None;
    }
    let ptr = unsafe { GlobalLock(handle as _) } as *const u16;
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while len < MAX_BYTES / 2 && unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = String::from_utf16_lossy(slice);
    unsafe { GlobalUnlock(handle as _) };
    Some(text)
}

unsafe fn read_hdrop() -> Option<Vec<std::path::PathBuf>> {
    let handle = unsafe { GetClipboardData(CF_HDROP) };
    if handle.is_null() {
        return None;
    }
    let hdrop = handle as HDROP;
    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, std::ptr::null_mut(), 0) };
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count {
        let needed = unsafe { DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0) };
        if needed == 0 {
            continue;
        }
        let mut buf = vec![0u16; needed as usize + 1];
        let written = unsafe { DragQueryFileW(hdrop, i, buf.as_mut_ptr(), buf.len() as u32) };
        if written > 0 {
            buf.truncate(written as usize);
            paths.push(std::path::PathBuf::from(String::from_utf16_lossy(&buf)));
        }
    }
    Some(paths)
}

/// Nom de l'exécutable qui possède le presse-papier, sans chemin ni extension.
unsafe fn owner_process_name() -> Option<String> {
    let owner = unsafe { GetClipboardOwner() };
    if owner.is_null() {
        return None;
    }
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(owner, &mut pid) };
    if pid == 0 {
        return None;
    }
    let handle: HANDLE = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buf = vec![0u16; 512];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len) };
    unsafe { CloseHandle(handle) };
    if ok == 0 {
        return None;
    }
    buf.truncate(len as usize);
    let full = String::from_utf16_lossy(&buf);
    std::path::Path::new(&full)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
}

/// Un CF_DIB est un BMP amputé de son en-tête de fichier : on le lui rend, puis
/// on laisse `image` faire la conversion.
fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    let bit_count = u16::from_le_bytes(dib[14..16].try_into().ok()?) as usize;
    let clr_used = u32::from_le_bytes(dib[32..36].try_into().ok()?) as usize;
    let palette = if bit_count <= 8 {
        let entries = if clr_used == 0 { 1usize << bit_count } else { clr_used };
        entries * 4
    } else {
        0
    };

    let file_size = 14 + dib.len();
    let pixel_offset = 14 + header_size + palette;
    let mut bmp = Vec::with_capacity(file_size);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&0u16.to_le_bytes());
    bmp.extend_from_slice(&0u16.to_le_bytes());
    bmp.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);

    let decoded = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp).ok()?;
    let mut png = Vec::new();
    decoded
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}
