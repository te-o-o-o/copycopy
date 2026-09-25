//! Probe: lists the Wayland globals exposed by the current compositor. Tells
//! you whether an event-driven backend (data-control) is possible here.
//!
//! Linux only. The tool drives an X11 or Wayland server, so it has nothing to
//! talk to elsewhere — but it still has to *build* everywhere, or
//! `--all-targets` breaks the Windows and macOS runs of the CI.

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use wayland_client::protocol::wl_registry;
    use wayland_client::{Connection, Dispatch, QueueHandle};

    struct State {
        globals: Vec<(String, u32)>,
    }

    impl Dispatch<wl_registry::WlRegistry, ()> for State {
        fn event(
            state: &mut Self,
            _: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                interface, version, ..
            } = event
            {
                state.globals.push((interface, version));
            }
        }
    }

    let conn = Connection::connect_to_env()?;
    let display = conn.display();
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    display.get_registry(&qh, ());
    let mut state = State {
        globals: Vec::new(),
    };
    queue.roundtrip(&mut state)?;

    state.globals.sort();
    for (name, version) in &state.globals {
        let interesting = name.contains("data_control") || name.contains("data_device");
        println!(
            "{}{name} (v{version})",
            if interesting { ">>> " } else { "    " }
        );
    }
    println!("\n{} globaux", state.globals.len());
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("wl_globals : sonde Wayland, sans objet sur ce système");
}
