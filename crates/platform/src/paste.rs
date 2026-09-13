//! Auto-paste: once an entry is copied, paste it where the user was typing.
//!
//! Nothing reads a cursor position. The keystroke that pastes — Ctrl+V — is
//! simulated for the application that had the focus before the window opened,
//! which puts the content wherever that application's cursor already was.
//!
//! | System  | How                                       | Here      |
//! |---------|-------------------------------------------|-----------|
//! | Windows | `SendInput`, after handing the focus back | done      |
//! | X11     | XTEST fake key events                     | done      |
//! | Wayland | needs the RemoteDesktop portal            | refused   |
//! | macOS   | `CGEvent`, behind the Accessibility grant | not yet   |

pub use imp::{availability, remember_target, restore_target, send_paste};

#[cfg(target_os = "windows")]
mod imp {
    use std::sync::atomic::{AtomicIsize, Ordering};

    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

    /// The window that had the focus when copycopy opened, as an integer so it
    /// fits an atomic.
    static TARGET: AtomicIsize = AtomicIsize::new(0);

    pub fn availability() -> Result<(), &'static str> {
        Ok(())
    }

    /// Notes the foreground window. Call it just before copycopy takes the
    /// focus, never while copycopy is already showing: it would note itself.
    pub fn remember_target() {
        let hwnd = unsafe { GetForegroundWindow() };
        TARGET.store(hwnd as isize, Ordering::Relaxed);
    }

    /// Gives the focus back. Must run while copycopy still holds the foreground:
    /// Windows only lets the foreground process hand it to another.
    pub fn restore_target() {
        let hwnd = TARGET.load(Ordering::Relaxed);
        if hwnd != 0 {
            unsafe { SetForegroundWindow(hwnd as _) };
        }
    }

    pub fn send_paste() -> Result<(), String> {
        const VK_V: u16 = 0x56;
        let key = |vk: u16, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [
            key(VK_CONTROL, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ];
        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        if sent as usize == inputs.len() {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().to_string())
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
    use x11rb::protocol::xtest::ConnectionExt as _;

    const CONTROL_L: u32 = 0xffe3;
    const LOWER_V: u32 = 0x0076;

    /// XTEST reaches X11 applications only. In a Wayland session it would land
    /// on Xwayland windows alone, so it is refused rather than half working.
    pub fn availability() -> Result<(), &'static str> {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return Err("indisponible sous Wayland");
        }
        if std::env::var_os("DISPLAY").is_none() {
            return Err("aucun serveur X");
        }
        Ok(())
    }

    /// Nothing to note: the window manager hands the focus back to the previous
    /// window when copycopy's window hides.
    pub fn remember_target() {}

    pub fn restore_target() {}

    pub fn send_paste() -> Result<(), String> {
        availability().map_err(str::to_string)?;
        let (conn, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        conn.xtest_get_version(2, 2)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?;
        let root = conn.setup().roots[screen].root;
        let (min, max) = (conn.setup().min_keycode, conn.setup().max_keycode);
        let mapping = conn
            .get_keyboard_mapping(min, max - min + 1)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?;
        let per = mapping.keysyms_per_keycode as usize;
        let find = |keysym: u32| {
            mapping
                .keysyms
                .chunks(per)
                .position(|syms| syms.contains(&keysym))
                .map(|i| min + i as u8)
                .ok_or_else(|| format!("no keycode for keysym {keysym:#x}"))
        };
        let (control, v) = (find(CONTROL_L)?, find(LOWER_V)?);
        for (kind, code) in [
            (KEY_PRESS_EVENT, control),
            (KEY_PRESS_EVENT, v),
            (KEY_RELEASE_EVENT, v),
            (KEY_RELEASE_EVENT, control),
        ] {
            conn.xtest_fake_input(kind, code, 0, root, 0, 0, 0)
                .map_err(|e| e.to_string())?;
        }
        conn.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
mod imp {
    pub fn availability() -> Result<(), &'static str> {
        Err("pas encore disponible sur ce système")
    }

    pub fn remember_target() {}

    pub fn restore_target() {}

    pub fn send_paste() -> Result<(), String> {
        Err("auto-paste is not available on this system".to_string())
    }
}
