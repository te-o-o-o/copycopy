//! Instance unique et commande `--show`.
//!
//! Deux rôles :
//!
//! 1. empêcher deux daemons de capturer en parallèle ;
//! 2. offrir le repli universel au raccourci global — sous Wayland, où aucune
//!    application ne peut capter une touche, l'utilisateur définit un raccourci
//!    dans son compositeur qui lance `copycopy-iced --show`, et ce second
//!    process demande au résident d'ouvrir sa fenêtre.

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
        // Fuite volontaire : la Name doit vivre aussi longtemps que le socket.
        Box::leak(path.into_boxed_str()).to_fs_name::<GenericFilePath>()
    }
}

pub enum Claim {
    /// On est le résident : voici le guichet.
    Primary(interprocess::local_socket::Listener),
    /// Un résident tourne déjà.
    AlreadyRunning,
}

/// Tente de devenir l'instance résidente.
pub fn claim() -> io::Result<Claim> {
    let listener = ListenerOptions::new()
        .name(name()?)
        // Un socket « cadavre » reste après un arrêt brutal : on le remplace,
        // sinon l'application refuserait de redémarrer.
        .reclaim_name(true)
        .create_sync();

    match listener {
        Ok(listener) => Ok(Claim::Primary(listener)),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            // Le nom est pris : soit un résident écoute, soit le socket est
            // mort. On tranche en essayant de s'y connecter.
            match Stream::connect(name()?) {
                Ok(_) => Ok(Claim::AlreadyRunning),
                Err(_) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

/// Envoie une commande au résident et renvoie sa réponse.
pub fn send(command: &str) -> io::Result<String> {
    let mut conn = BufReader::new(Stream::connect(name()?)?);
    conn.get_mut().write_all(format!("{command}\n").as_bytes())?;
    let mut reply = String::new();
    conn.read_line(&mut reply)?;
    Ok(reply.trim().to_string())
}

/// Sert les commandes dans un thread dédié.
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
