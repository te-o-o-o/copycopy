//! Test fixture: a real X11 client that advertises WM_CLASS and _NET_WM_PID,
//! takes ownership of CLIPBOARD and serves the requested text. Used to validate
//! source-application detection, which `xclip` cannot exercise since it exposes
//! neither.
//!
//!   cargo run -p copycopy-platform --example fake_owner -- "text" MyClass 4

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, SelectionNotifyEvent,
    WindowClass, SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let text = args.get(1).cloned().unwrap_or_else(|| "coucou".into());
    let class = args.get(2).cloned().unwrap_or_else(|| "FakeApp".into());
    let secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4);

    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let win = conn.generate_id()?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win,
        screen.root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?;

    let wm_class = format!("{}\0{}\0", class.to_lowercase(), class);
    conn.change_property8(
        PropMode::REPLACE,
        win,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        wm_class.as_bytes(),
    )?;
    let net_wm_pid = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
    conn.change_property32(
        PropMode::REPLACE,
        win,
        net_wm_pid,
        AtomEnum::CARDINAL,
        &[std::process::id()],
    )?;

    let clipboard = conn.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;
    let targets = conn.intern_atom(false, b"TARGETS")?.reply()?.atom;
    let utf8 = conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;

    conn.set_selection_owner(win, clipboard, x11rb::CURRENT_TIME)?;
    conn.flush()?;
    println!("propriétaire de CLIPBOARD, WM_CLASS={class}, pid={}", std::process::id());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        while let Some(event) = conn.poll_for_event()? {
            let Event::SelectionRequest(req) = event else {
                continue;
            };
            let property = if req.target == targets {
                conn.change_property32(
                    PropMode::REPLACE,
                    req.requestor,
                    req.property,
                    AtomEnum::ATOM,
                    &[targets, utf8],
                )?;
                req.property
            } else if req.target == utf8 {
                conn.change_property8(
                    PropMode::REPLACE,
                    req.requestor,
                    req.property,
                    utf8,
                    text.as_bytes(),
                )?;
                req.property
            } else {
                x11rb::NONE
            };

            conn.send_event(
                false,
                req.requestor,
                EventMask::NO_EVENT,
                SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: req.time,
                    requestor: req.requestor,
                    selection: req.selection,
                    target: req.target,
                    property,
                },
            )?;
            conn.flush()?;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Ok(())
}
