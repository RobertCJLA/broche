// SPDX-License-Identifier: GPL-3.0-or-later
//! Dragging files out of Broche into other apps: the sending side of XDND, the X11 drag and
//! drop protocol (Broche runs through XWayland; GNOME carries an X11 drag on to Wayland apps).
//! winit only receives drops, so this runs on an X11 connection of its own. The mouse button
//! is already held on Broche's window, which owns the pointer meanwhile, so the pointer is
//! followed by asking where it is rather than by grabbing it.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ClipOrdering, ConfigureWindowAux, ConnectionExt as _, CoordMode,
    CreateGCAux, CreateWindowAux, EventMask, KeyButMask, Point, PolyShape, PropMode, Rectangle,
    SelectionNotifyEvent, SelectionRequestEvent, StackMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

type Error = Box<dyn std::error::Error>;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        XdndAware,
        XdndProxy,
        XdndSelection,
        XdndEnter,
        XdndPosition,
        XdndStatus,
        XdndLeave,
        XdndDrop,
        XdndFinished,
        XdndActionCopy,
        TARGETS,
        URI_LIST: b"text/uri-list",
    }
}

/// The XDND version spoken, at most.
const VERSION: u32 = 5;

/// Starts dragging `paths` (the mouse button must be down on a Broche window). Returns at
/// once; the drag runs on a thread of its own until the button is let go.
pub fn start(paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        if let Err(e) = run(&paths) {
            eprintln!("drag failed: {e}");
        }
    });
}

/// The files as `text/uri-list`: one `file://` address per line, each line ending in CRLF.
fn uri_list(paths: &[PathBuf]) -> String {
    paths.iter().filter_map(|p| url::Url::from_file_path(p).ok()).map(|u| format!("{u}\r\n")).collect()
}

/// A drop target: the window that said it takes drops, the window its messages go to
/// (itself, or its proxy), and the version both sides speak.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Target {
    window: Window,
    to: Window,
    version: u32,
}

fn run(paths: &[PathBuf]) -> Result<(), Error> {
    let data = uri_list(paths);
    let (conn, screen) = RustConnection::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let atoms = Atoms::new(&conn)?.reply()?;

    // The source: an unmapped window that owns the drag's selection and gets the replies.
    let source = conn.generate_id()?;
    conn.create_window(
        0,
        source,
        root,
        -10,
        -10,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new().override_redirect(1).event_mask(EventMask::PROPERTY_CHANGE),
    )?;
    conn.set_selection_owner(source, atoms.XdndSelection, CURRENT_TIME)?;
    let (icon, draw_icon) = icon(&conn, root, conn.setup().roots[screen].white_pixel)?;
    conn.flush()?;

    let mut target: Option<Target> = None;
    let mut accepted = false;
    let result = (|| -> Result<(), Error> {
        loop {
            while let Some(event) = conn.poll_for_event()? {
                match event {
                    Event::SelectionRequest(req) => answer(&conn, &atoms, &req, &data)?,
                    Event::ClientMessage(m) if m.type_ == atoms.XdndStatus => {
                        accepted = m.data.as_data32()[1] & 1 == 1;
                    }
                    _ => {}
                }
            }
            let ptr = conn.query_pointer(root)?.reply()?;
            conn.configure_window(
                icon,
                &ConfigureWindowAux::new()
                    .x(i32::from(ptr.root_x) + 14)
                    .y(i32::from(ptr.root_y) + 10)
                    .stack_mode(StackMode::ABOVE),
            )?;
            draw_icon(&conn)?;
            if !ptr.mask.contains(KeyButMask::BUTTON1) {
                // Let go: drop where it was accepted, otherwise leave.
                if let Some(t) = target {
                    if accepted {
                        send(&conn, &atoms, source, t, atoms.XdndDrop, [0, CURRENT_TIME, 0])?;
                        wait_finished(&conn, &atoms, &data)?;
                    } else {
                        send(&conn, &atoms, source, t, atoms.XdndLeave, [0, 0, 0])?;
                    }
                }
                return Ok(());
            }
            let under = aware_under(&conn, &atoms, root)?;
            if under.map(|t| t.window) != target.map(|t| t.window) {
                if let Some(t) = target {
                    send(&conn, &atoms, source, t, atoms.XdndLeave, [0, 0, 0])?;
                }
                target = under;
                accepted = false;
                if let Some(t) = target {
                    send(&conn, &atoms, source, t, atoms.XdndEnter, [t.version << 24, atoms.URI_LIST, 0])?;
                }
            }
            if let Some(t) = target {
                let at = ((ptr.root_x as u32) << 16) | (ptr.root_y as u16 as u32);
                send(&conn, &atoms, source, t, atoms.XdndPosition, [0, at, CURRENT_TIME])?;
            }
            conn.flush()?;
            std::thread::sleep(Duration::from_millis(16));
        }
    })();
    let _ = conn.set_selection_owner(NONE, atoms.XdndSelection, CURRENT_TIME);
    let _ = conn.destroy_window(icon);
    let _ = conn.destroy_window(source);
    let _ = conn.flush();
    result
}

/// The page drawn next to the pointer, in pixels, and its folded corner.
const ICON: (u16, u16) = (26, 32);
const FOLD: i16 = 8;

/// A small page that follows the pointer while dragging: the pointer stays Broche's window's
/// meanwhile, so GNOME shows no drag cursor of its own. Clicks go through it (an empty input
/// shape), so it never becomes the drop target. Returns the window and how to draw it.
type DrawIcon = Box<dyn Fn(&RustConnection) -> Result<(), Error>>;
fn icon(conn: &RustConnection, root: Window, white: u32) -> Result<(Window, DrawIcon), Error> {
    let (w, h) = (ICON.0 as i16, ICON.1 as i16);
    let win = conn.generate_id()?;
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        win,
        root,
        -100,
        -100,
        ICON.0,
        ICON.1,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new().override_redirect(1).background_pixel(white),
    )?;
    let page = [
        Point { x: 0, y: 0 },
        Point { x: w - FOLD, y: 0 },
        Point { x: w - 1, y: FOLD },
        Point { x: w - 1, y: h - 1 },
        Point { x: 0, y: h - 1 },
        Point { x: 0, y: 0 },
    ];
    // The window is shaped like the page (corner cut off) and lets the pointer through.
    let mask = conn.generate_id()?;
    conn.create_pixmap(1, mask, win, ICON.0, ICON.1)?;
    let mask_gc = conn.generate_id()?;
    conn.create_gc(mask_gc, mask, &CreateGCAux::new().foreground(0))?;
    conn.poly_fill_rectangle(mask, mask_gc, &[Rectangle { x: 0, y: 0, width: ICON.0, height: ICON.1 }])?;
    conn.change_gc(mask_gc, &x11rb::protocol::xproto::ChangeGCAux::new().foreground(1))?;
    // A filled polygon leaves out its right and bottom edges: one pixel more, so the outline shows.
    let filled = [
        Point { x: 0, y: 0 },
        Point { x: w - FOLD + 1, y: 0 },
        Point { x: w, y: FOLD - 1 },
        Point { x: w, y: h },
        Point { x: 0, y: h },
    ];
    conn.fill_poly(mask, mask_gc, PolyShape::NONCONVEX, CoordMode::ORIGIN, &filled)?;
    conn.shape_mask(shape::SO::SET, shape::SK::BOUNDING, win, 0, 0, mask)?;
    conn.shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, win, 0, 0, &[])?;
    conn.map_window(win)?;
    let (edge, lines) = (conn.generate_id()?, conn.generate_id()?);
    conn.create_gc(edge, win, &CreateGCAux::new().foreground(0x0050_5560))?;
    conn.create_gc(lines, win, &CreateGCAux::new().foreground(0x00a0_a8b8))?;
    let draw: DrawIcon = Box::new(move |conn| {
        conn.poly_line(CoordMode::ORIGIN, win, edge, &page)?;
        let fold = [Point { x: w - FOLD, y: 0 }, Point { x: w - FOLD, y: FOLD }, Point { x: w - 1, y: FOLD }];
        conn.poly_line(CoordMode::ORIGIN, win, edge, &fold)?;
        let text: Vec<Rectangle> =
            [13, 18, 23].iter().map(|&y| Rectangle { x: 5, y, width: (w - 10) as u16, height: 2 }).collect();
        conn.poly_fill_rectangle(win, lines, &text)?;
        Ok(())
    });
    Ok((win, draw))
}

/// After the drop, the target asks for the files and then says it's done (or gives up).
fn wait_finished(conn: &RustConnection, atoms: &Atoms, data: &str) -> Result<(), Error> {
    conn.flush()?;
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until {
        match conn.poll_for_event()? {
            Some(Event::SelectionRequest(req)) => answer(conn, atoms, &req, data)?,
            Some(Event::ClientMessage(m)) if m.type_ == atoms.XdndFinished => return Ok(()),
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    Ok(())
}

/// The deepest window under the pointer that takes drops (`XdndAware`), if any.
fn aware_under(conn: &RustConnection, atoms: &Atoms, root: Window) -> Result<Option<Target>, Error> {
    let mut window = root;
    loop {
        let child = conn.query_pointer(window)?.reply()?.child;
        if child == NONE {
            return Ok(None);
        }
        window = child;
        let aware = conn.get_property(false, window, atoms.XdndAware, AtomEnum::ATOM, 0, 1)?.reply()?;
        if let Some(theirs) = aware.value32().and_then(|mut v| v.next()) {
            let proxy = conn.get_property(false, window, atoms.XdndProxy, AtomEnum::WINDOW, 0, 1)?.reply()?;
            let to = proxy.value32().and_then(|mut v| v.next()).unwrap_or(window);
            return Ok(Some(Target { window, to, version: theirs.min(VERSION) }));
        }
    }
}

/// One XDND message to `t`: the source, then `rest` (data[1] to data[3]), then the action
/// (only position uses it).
fn send(
    conn: &RustConnection,
    atoms: &Atoms,
    source: Window,
    t: Target,
    kind: u32,
    rest: [u32; 3],
) -> Result<(), Error> {
    let data = message_data(source, kind == atoms.XdndPosition, atoms.XdndActionCopy, rest);
    let event = ClientMessageEvent::new(32, t.window, kind, data);
    conn.send_event(false, t.to, EventMask::NO_EVENT, event)?;
    Ok(())
}

fn message_data(source: Window, position: bool, copy: u32, rest: [u32; 3]) -> [u32; 5] {
    [source, rest[0], rest[1], rest[2], if position { copy } else { 0 }]
}

/// Hands the files to the app the drop landed on (or says which formats there are).
fn answer(
    conn: &RustConnection,
    atoms: &Atoms,
    req: &SelectionRequestEvent,
    data: &str,
) -> Result<(), Error> {
    let property = if req.property == NONE { req.target } else { req.property };
    let given = if req.target == atoms.URI_LIST {
        conn.change_property8(PropMode::REPLACE, req.requestor, property, atoms.URI_LIST, data.as_bytes())?;
        property
    } else if req.target == atoms.TARGETS {
        conn.change_property32(
            PropMode::REPLACE,
            req.requestor,
            property,
            AtomEnum::ATOM,
            &[atoms.TARGETS, atoms.URI_LIST],
        )?;
        property
    } else {
        NONE
    };
    let notify = SelectionNotifyEvent {
        response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: req.time,
        requestor: req.requestor,
        selection: req.selection,
        target: req.target,
        property: given,
    };
    conn.send_event(false, req.requestor, EventMask::NO_EVENT, notify)?;
    conn.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_list_is_file_urls_with_crlf() {
        let list =
            uri_list(&[PathBuf::from("/home/a/Broche/Recibidos/foto 1.jpg"), PathBuf::from("/tmp/b.txt")]);
        assert_eq!(list, "file:///home/a/Broche/Recibidos/foto%201.jpg\r\nfile:///tmp/b.txt\r\n");
    }

    #[test]
    fn only_position_carries_the_action() {
        assert_eq!(message_data(7, true, 99, [0, (10 << 16) | 20, 0]), [7, 0, (10 << 16) | 20, 0, 99]);
        assert_eq!(message_data(7, false, 99, [5 << 24, 42, 0]), [7, 5 << 24, 42, 0, 0]);
    }
}
