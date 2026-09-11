//! Synthesises a keystroke through the XTEST extension, to test a global
//! shortcut without a physical keyboard.
//!
//!   cargo run -p copycopy-platform --example press_key -- ctrl alt v

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
use x11rb::protocol::xtest::ConnectionExt as _;

/// A few common X11 keysyms.
fn keysym(name: &str) -> Option<u32> {
    Some(match name.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => 0xffe3, // Control_L
        "alt" => 0xffe9,              // Alt_L
        "shift" => 0xffe1,            // Shift_L
        "super" | "meta" | "win" => 0xffeb, // Super_L
        "space" => 0x0020,
        "enter" | "return" => 0xff0d,
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => s.as_bytes()[0] as u32,
        _ => return None,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage : press_key ctrl alt v");
        return Ok(());
    }

    let (conn, screen_num) = x11rb::connect(None)?;
    conn.xtest_get_version(2, 2)?.reply()?;
    let root = conn.setup().roots[screen_num].root;

    // keysym-to-keycode table, to turn "v" into a hardware code.
    let setup = conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let mapping = conn
        .get_keyboard_mapping(min, max - min + 1)?
        .reply()?;
    let per = mapping.keysyms_per_keycode as usize;

    let find = |want: u32| -> Option<u8> {
        mapping.keysyms.chunks(per).enumerate().find_map(|(i, syms)| {
            syms.contains(&want).then(|| min + i as u8)
        })
    };

    let codes: Vec<u8> = args
        .iter()
        .map(|name| {
            let sym = keysym(name).unwrap_or_else(|| panic!("keysym inconnu : {name}"));
            find(sym).unwrap_or_else(|| panic!("aucun keycode pour : {name}"))
        })
        .collect();

    println!("frappe : {} → keycodes {codes:?}", args.join("+"));

    // Press in the given order, release in reverse.
    for code in &codes {
        conn.xtest_fake_input(KEY_PRESS_EVENT, *code, 0, root, 0, 0, 0)?;
    }
    conn.flush()?;
    std::thread::sleep(std::time::Duration::from_millis(60));
    for code in codes.iter().rev() {
        conn.xtest_fake_input(KEY_RELEASE_EVENT, *code, 0, root, 0, 0, 0)?;
    }
    conn.flush()?;
    std::thread::sleep(std::time::Duration::from_millis(60));
    println!("envoyée");
    Ok(())
}
