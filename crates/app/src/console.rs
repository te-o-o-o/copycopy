//! Where the program's messages go.
//!
//! On Windows the binary is built for the GUI subsystem, so no console window
//! opens with it — the one that used to, and that stopped the resident when
//! someone closed it. Its messages still have to land somewhere:
//!
//! - streams inherited from the parent — `cargo run`, a pipe, a redirection —
//!   are kept as they are;
//! - otherwise, launched from a terminal, the output attaches to that terminal;
//! - otherwise — Start menu, login — it goes to `copycopy.log` beside the
//!   database, so a failure still leaves a trace to read.
//!
//! Elsewhere a graphical program keeps its standard streams, and this does
//! nothing.

#[cfg(target_os = "windows")]
pub fn prepare() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_OUTPUT_HANDLE,
    };

    /// A single file, restarted once it passes this size: enough to read back
    /// what went wrong, never enough to matter on disk.
    const MAX_LOG_BYTES: u64 = 1024 * 1024;

    // Rust's standard streams look their handle up on every write, so pointing
    // the process handles somewhere here covers every later `println!`,
    // `eprintln!` and panic message.
    fn redirect(file: std::fs::File) {
        let handle = file.as_raw_handle();
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, handle as _);
            SetStdHandle(STD_ERROR_HANDLE, handle as _);
        }
        // Kept open for the life of the process: the handles point at it.
        std::mem::forget(file);
    }

    let inherited = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if !inherited.is_null() && inherited != INVALID_HANDLE_VALUE {
        return;
    }

    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0 {
        // Attaching does not set the standard handles of a GUI process: the
        // console's output buffer has to be opened by name.
        if let Ok(console) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
            redirect(console);
        }
        return;
    }

    let Some(dir) = crate::config::base_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("copycopy.log");
    let too_big = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES);
    let mut options = std::fs::OpenOptions::new();
    options.create(true);
    if too_big {
        options.write(true).truncate(true);
    } else {
        options.append(true);
    }
    if let Ok(log) = options.open(&path) {
        redirect(log);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn prepare() {}
