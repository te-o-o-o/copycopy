//! Backend Wayland événementiel.
//!
//! Wayland n'expose pas le presse-papier aux applications en arrière-plan : le
//! `wl_data_device` standard exige le focus clavier. Il faut un protocole
//! « data-control », qui existe en deux variantes de forme identique :
//!
//! - `ext_data_control_v1` : la version standardisée (staging), à préférer ;
//! - `zwlr_data_control_unstable_v1` : l'historique wlroots, encore ce
//!   qu'exposent KDE, Sway, Hyprland, COSMIC.
//!
//! **GNOME n'expose ni l'un ni l'autre** : sur GNOME/Wayland il n'existe aucun
//! moyen de surveiller le presse-papier depuis un process en arrière-plan sans
//! extension ni portail. `start()` bascule alors sur le sondage.
//!
//! L'application source n'est pas récupérable : aucun protocole Wayland ne
//! l'expose, contrairement à `WM_CLASS` sous X11.

use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::time::Duration;

use copycopy_core::ClipEvent;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_registry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::Capture;

const READ_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BYTES: usize = 32 * 1024 * 1024;

/// Marqueurs « ce contenu est un secret » : on ne capture pas.
const SECRET_MIMES: &[&str] = &[
    "x-kde-passwordManagerHint",
    "org.nspasteboard.ConcealedType",
    "application/x-nextcloud-talk-secret",
];

/// Par ordre de préférence.
fn pick_mime(mimes: &[String]) -> Option<&String> {
    const ORDER: &[&str] = &[
        "image/png",
        "text/uri-list",
        "text/plain;charset=utf-8",
        "UTF8_STRING",
        "text/plain",
        "STRING",
        "TEXT",
    ];
    ORDER
        .iter()
        .find_map(|want| mimes.iter().find(|m| m.eq_ignore_ascii_case(want)))
        .or_else(|| mimes.iter().find(|m| m.starts_with("text/")))
}

fn to_event(mime: &str, data: Vec<u8>) -> Option<ClipEvent> {
    if mime.eq_ignore_ascii_case("image/png") {
        if data.is_empty() {
            return None;
        }
        let size = crate::png_size(&data);
        return Some(ClipEvent::Image { png: data, size });
    }
    let text = String::from_utf8_lossy(&data).into_owned();
    if mime.eq_ignore_ascii_case("text/uri-list") {
        let paths: Vec<std::path::PathBuf> = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| std::path::PathBuf::from(crate::percent_decode(l.trim_start_matches("file://"))))
            .collect();
        if !paths.is_empty() {
            return Some(ClipEvent::Files(paths));
        }
    }
    if text.trim().is_empty() {
        return None;
    }
    Some(ClipEvent::Text(text))
}

/// Le corps du backend, identique pour les deux protocoles. Les noms `Manager`,
/// `Device`, `Offer`, `dev`, `offer` et `INTERFACE` sont résolus dans le module
/// appelant : c'est ce qui permet de n'écrire la logique qu'une fois.
macro_rules! impl_data_control {
    () => {
        use super::*;

        #[derive(Default)]
        struct State {
            seat: Option<WlSeat>,
            manager: Option<Manager>,
            mimes: HashMap<ObjectId, Vec<String>>,
            pending: Option<Offer>,
            finished: bool,
        }

        impl Dispatch<wl_registry::WlRegistry, ()> for State {
            fn event(
                state: &mut Self,
                registry: &wl_registry::WlRegistry,
                event: wl_registry::Event,
                _: &(),
                _: &Connection,
                qh: &QueueHandle<Self>,
            ) {
                let wl_registry::Event::Global {
                    name,
                    interface,
                    version,
                } = event
                else {
                    return;
                };
                if interface == INTERFACE {
                    state.manager = Some(registry.bind::<Manager, _, _>(name, version.min(1), qh, ()));
                } else if interface == "wl_seat" {
                    state.seat = Some(registry.bind::<WlSeat, _, _>(name, version.min(7), qh, ()));
                }
            }
        }

        impl Dispatch<WlSeat, ()> for State {
            fn event(
                _: &mut Self,
                _: &WlSeat,
                _: <WlSeat as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }

        impl Dispatch<Manager, ()> for State {
            fn event(
                _: &mut Self,
                _: &Manager,
                _: <Manager as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }

        impl Dispatch<Device, ()> for State {
            fn event(
                state: &mut Self,
                _: &Device,
                event: <Device as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                match event {
                    dev::Event::DataOffer { id } => {
                        state.mimes.insert(id.id(), Vec::new());
                    }
                    dev::Event::Selection { id } => {
                        state.pending = id;
                    }
                    dev::Event::Finished => state.finished = true,
                    _ => {}
                }
            }

            // L'événement `data_offer` (opcode 0) crée un nouvel objet : il faut
            // dire à wayland-rs comment l'accueillir.
            wayland_client::event_created_child!(State, Device, [
                0 => (Offer, ()),
            ]);
        }

        impl Dispatch<Offer, ()> for State {
            fn event(
                state: &mut Self,
                proxy: &Offer,
                event: <Offer as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                if let offer::Event::Offer { mime_type } = event {
                    state.mimes.entry(proxy.id()).or_default().push(mime_type);
                }
            }
        }

        pub fn run(conn: Connection, tx: Sender<Capture>) {
            let mut queue = conn.new_event_queue();
            let qh = queue.handle();
            conn.display().get_registry(&qh, ());

            let mut state = State::default();
            if let Err(e) = queue.roundtrip(&mut state) {
                eprintln!("Wayland : {e}");
                return;
            }
            let (Some(manager), Some(seat)) = (state.manager.clone(), state.seat.clone()) else {
                eprintln!("Wayland : ni {INTERFACE} ni wl_seat");
                return;
            };
            let _device = manager.get_data_device(&seat, &qh, ());
            if queue.roundtrip(&mut state).is_err() {
                return;
            }

            loop {
                if queue.blocking_dispatch(&mut state).is_err() || state.finished {
                    return;
                }
                let Some(offer) = state.pending.take() else {
                    continue;
                };
                let id = offer.id();
                let mimes = state.mimes.remove(&id).unwrap_or_default();

                let secret = mimes
                    .iter()
                    .any(|m| SECRET_MIMES.iter().any(|s| m.eq_ignore_ascii_case(s)));
                if secret {
                    offer.destroy();
                    continue;
                }

                if let Some(mime) = pick_mime(&mimes).cloned() {
                    match receive(&conn, &offer, &mime) {
                        Ok(data) => {
                            if let Some(event) = to_event(&mime, data) {
                                if tx
                                    .send(Capture {
                                        event,
                                        // Aucun protocole Wayland n'expose
                                        // l'application source.
                                        source: String::new(),
                                    })
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }
                        Err(e) => eprintln!("lecture de la sélection Wayland : {e}"),
                    }
                }
                offer.destroy();
            }
        }

        /// Le compositeur écrit le contenu dans un descripteur qu'on lui passe.
        fn receive(conn: &Connection, offer: &Offer, mime: &str) -> Result<Vec<u8>, String> {
            let (reader, writer) = UnixStream::pair().map_err(|e| e.to_string())?;
            offer.receive(mime.to_string(), writer.as_fd());
            conn.flush().map_err(|e| e.to_string())?;
            // Notre extrémité fermée, l'EOF viendra de la fermeture par le
            // compositeur une fois le contenu écrit.
            drop(writer);

            reader
                .set_read_timeout(Some(READ_TIMEOUT))
                .map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            let mut chunk = [0u8; 8192];
            let mut reader = reader;
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        if out.len() + n > MAX_BYTES {
                            return Err(format!("contenu > {} Mio, ignoré", MAX_BYTES / 1048576));
                        }
                        out.extend_from_slice(&chunk[..n]);
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(out)
        }
    };
}

mod ext_backend {
    use std::os::fd::AsFd;

    use wayland_protocols::ext::data_control::v1::client::{
        ext_data_control_device_v1::{self as dev, ExtDataControlDeviceV1 as Device},
        ext_data_control_manager_v1::ExtDataControlManagerV1 as Manager,
        ext_data_control_offer_v1::{self as offer, ExtDataControlOfferV1 as Offer},
    };

    pub const INTERFACE: &str = "ext_data_control_manager_v1";
    impl_data_control!();
}

mod wlr_backend {
    use std::os::fd::AsFd;

    use wayland_protocols_wlr::data_control::v1::client::{
        zwlr_data_control_device_v1::{self as dev, ZwlrDataControlDeviceV1 as Device},
        zwlr_data_control_manager_v1::ZwlrDataControlManagerV1 as Manager,
        zwlr_data_control_offer_v1::{self as offer, ZwlrDataControlOfferV1 as Offer},
    };

    pub const INTERFACE: &str = "zwlr_data_control_manager_v1";
    impl_data_control!();
}

/// Quelle variante le compositeur courant expose-t-il ?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Ext,
    Wlr,
}

struct Probe {
    ext: bool,
    wlr: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Probe {
    fn event(
        state: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { interface, .. } = event {
            match interface.as_str() {
                "ext_data_control_manager_v1" => state.ext = true,
                "zwlr_data_control_manager_v1" => state.wlr = true,
                _ => {}
            }
        }
    }
}

/// Renvoie la variante disponible, ou une erreur explicite si le compositeur
/// n'expose aucun data-control (GNOME, WSLg…).
pub fn probe() -> Result<Variant, String> {
    let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());
    let mut probe = Probe {
        ext: false,
        wlr: false,
    };
    queue.roundtrip(&mut probe).map_err(|e| e.to_string())?;

    if probe.ext {
        Ok(Variant::Ext)
    } else if probe.wlr {
        Ok(Variant::Wlr)
    } else {
        Err("le compositeur n'expose pas de protocole data-control \
             (GNOME notamment) : impossible de surveiller le presse-papier en \
             arrière-plan"
            .into())
    }
}

pub fn spawn(variant: Variant, tx: Sender<Capture>) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
    std::thread::Builder::new()
        .name("copycopy-wayland".into())
        .spawn(move || match variant {
            Variant::Ext => ext_backend::run(conn, tx),
            Variant::Wlr => wlr_backend::run(conn, tx),
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
