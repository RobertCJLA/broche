// SPDX-License-Identifier: GPL-3.0-or-later
//! Watches this PC's clipboard so a copy can go to the phone on its own. Needs X11: Broche
//! always runs through XWayland, and GNOME tells X11 clients when anything (Wayland apps too)
//! takes the clipboard. Plain Wayland on GNOME has no way to watch it.
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, CreateWindowAux, WindowClass};
use x11rb::rust_connection::RustConnection;

type Error = Box<dyn std::error::Error>;

/// Calls `on_text` with each new text copied on this PC, from a thread of its own.
pub fn spawn(on_text: impl Fn(String) + Send + 'static) {
    std::thread::spawn(move || {
        if let Err(e) = run(&on_text) {
            eprintln!("clipboard watch stopped: {e}");
        }
    });
}

struct Atoms {
    clipboard: u32,
    targets: u32,
    utf8: u32,
    incr: u32,
    property: u32,
    /// Set by password managers (KeePassXC, KWallet…) on secrets: never sent.
    secret: u32,
}

fn run(on_text: &dyn Fn(String)) -> Result<(), Error> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let win = conn.generate_id()?;
    conn.create_window(0, win, root, 0, 0, 1, 1, 0, WindowClass::INPUT_OUTPUT, 0, &CreateWindowAux::new())?;
    conn.xfixes_query_version(5, 0)?.reply()?;
    let atom = |name: &[u8]| -> Result<u32, Error> { Ok(conn.intern_atom(false, name)?.reply()?.atom) };
    let atoms = Atoms {
        clipboard: atom(b"CLIPBOARD")?,
        targets: atom(b"TARGETS")?,
        utf8: atom(b"UTF8_STRING")?,
        incr: atom(b"INCR")?,
        property: atom(b"BROCHE_CLIPBOARD")?,
        secret: atom(b"x-kde-passwordManagerHint")?,
    };
    conn.xfixes_select_selection_input(win, atoms.clipboard, SelectionEventMask::SET_SELECTION_OWNER)?;
    conn.flush()?;
    loop {
        let Event::XfixesSelectionNotify(ev) = conn.wait_for_event()? else { continue };
        if ev.owner == x11rb::NONE {
            continue; // the clipboard was emptied
        }
        if let Some(text) = read_text(&conn, win, &atoms)? {
            on_text(text);
        }
    }
}

/// The clipboard's text, unless it is marked secret, isn't text or is empty.
fn read_text(conn: &RustConnection, win: u32, atoms: &Atoms) -> Result<Option<String>, Error> {
    let Some((_, targets)) = fetch(conn, win, atoms, atoms.targets)? else { return Ok(None) };
    let targets = as_atoms(&targets);
    if targets.contains(&atoms.secret) || !targets.contains(&atoms.utf8) {
        return Ok(None);
    }
    let Some((kind, bytes)) = fetch(conn, win, atoms, atoms.utf8)? else { return Ok(None) };
    // shortcut: very large copies come in pieces (INCR) and are skipped; read them if asked for.
    if kind == atoms.incr {
        return Ok(None);
    }
    Ok(String::from_utf8(bytes).ok().filter(|t| !t.trim().is_empty()))
}

fn as_atoms(bytes: &[u8]) -> Vec<u32> {
    bytes.as_chunks::<4>().0.iter().map(|c| u32::from_ne_bytes(*c)).collect()
}

/// Asks the clipboard's owner for `target` and waits up to 2 s for it. Returns its type and bytes.
fn fetch(
    conn: &RustConnection,
    win: u32,
    atoms: &Atoms,
    target: u32,
) -> Result<Option<(u32, Vec<u8>)>, Error> {
    conn.convert_selection(win, atoms.clipboard, target, atoms.property, x11rb::CURRENT_TIME)?;
    conn.flush()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match conn.poll_for_event()? {
            Some(Event::SelectionNotify(n)) if n.requestor == win => {
                if n.property == x11rb::NONE {
                    return Ok(None);
                }
                let reply =
                    conn.get_property(true, win, atoms.property, AtomEnum::ANY, 0, u32::MAX / 4)?.reply()?;
                return Ok(Some((reply.type_, reply.value)));
            }
            Some(_) => {}
            None if Instant::now() > deadline => return Ok(None),
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn atoms_from_property_bytes() {
        let bytes: Vec<u8> = [7u32, 300].iter().flat_map(|a| a.to_ne_bytes()).collect();
        assert_eq!(super::as_atoms(&bytes), vec![7, 300]);
    }
}
