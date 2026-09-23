//! Single instance and the `--show` command.
//!
//! Two roles:
//!
//! 1. stop two daemons from capturing in parallel;
//! 2. provide the universal fallback for the global shortcut — under Wayland,
//!    where no application can grab a key, the user defines a shortcut in the
//!    compositor that runs `copycopy --show`, and that second process asks the
//!    resident to open its window.

use std::io::{self, BufRead, BufReader, Write};

use interprocess::local_socket::traits::{ListenerExt, Stream as _};
use interprocess::local_socket::{
    prelude::*, GenericFilePath, GenericNamespaced, ListenerOptions, Stream,
};

const SOCKET: &str = "copycopy.sock";

pub const SHOW: &str = "show";
pub const QUIT: &str = "quit";

fn name() -> io::Result<interprocess::local_socket::Name<'static>> {
    if GenericNamespaced::is_supported() {
        SOCKET.to_ns_name::<GenericNamespaced>()
    } else {
        let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let path = format!("{dir}/{SOCKET}");
        // Deliberate leak: the Name must live as long as the socket.
        Box::leak(path.into_boxed_str()).to_fs_name::<GenericFilePath>()
    }
}

pub enum Claim {
    /// We are the resident; here is the listener.
    Primary(interprocess::local_socket::Listener),
    /// A resident is already running.
    AlreadyRunning,
}

/// Attempts to become the resident instance.
pub fn claim() -> io::Result<Claim> {
    let listener = ListenerOptions::new()
        .name(name()?)
        // A stale socket survives an abrupt shutdown; replace it, otherwise
        // the application would refuse to start again.
        .reclaim_name(true)
        .create_sync();

    match listener {
        Ok(listener) => Ok(Claim::Primary(listener)),
        // The name is taken. Unix says so with `AddrInUse`; a Windows named
        // pipe that already exists says `PermissionDenied` instead. Reading the
        // latter as a broken IPC started one more resident on every launch —
        // five were found running side by side, none of them reachable.
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied
            ) =>
        {
            // Either a resident is listening or the socket is dead. Connecting
            // to it settles the question.
            match Stream::connect(name()?) {
                Ok(_) => Ok(Claim::AlreadyRunning),
                Err(_) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

/// Sends a command to the resident and returns its reply.
pub fn send(command: &str) -> io::Result<String> {
    let mut conn = BufReader::new(Stream::connect(name()?)?);
    conn.get_mut()
        .write_all(format!("{command}\n").as_bytes())?;
    let mut reply = String::new();
    conn.read_line(&mut reply)?;
    Ok(reply.trim().to_string())
}

/// Serves commands on a dedicated thread.
pub fn spawn_server(
    listener: interprocess::local_socket::Listener,
    on_command: impl Fn(String) + Send + 'static,
) {
    std::thread::Builder::new()
        .name("copycopy-ipc".into())
        .spawn(move || {
            for conn in listener.incoming() {
                let Ok(conn) = conn else { continue };
                let mut conn = BufReader::new(conn);
                let mut line = String::new();
                if conn.read_line(&mut line).is_err() {
                    continue;
                }
                let _ = conn.get_mut().write_all(b"ok\n");
                on_command(line.trim().to_string());
            }
        })
        .expect("thread IPC");
}
