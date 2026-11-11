// SPDX-License-Identifier: GPL-3.0-or-later
//! The laser pointer a phone moves in presentation mode (`docs/protocol/remote.md`): a bright
//! dot in a small round X11 window that stays above everything, lets clicks through and hides
//! when the phone lets go. Plain X11 (`x11rb`, like `clipwatch`): Broche runs through XWayland,
//! and a second egui window would freeze while the main one is minimized.
use std::sync::mpsc;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Arc, ClipOrdering, ConfigureWindowAux, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask,
    Rectangle, StackMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;

type Error = Box<dyn std::error::Error>;

/// The dot's size in pixels.
const SIZE: u16 = 28;
/// Hidden when no position arrives for this long (the phone stopped sending).
const IDLE: Duration = Duration::from_millis(1500);

/// Moves or hides the dot; cheap to call for every update.
#[derive(Clone, Debug)]
pub struct Laser(mpsc::Sender<Option<(f32, f32)>>);

impl Laser {
    /// Starts the overlay's thread (it opens its X11 connection when first needed).
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Err(e) = run(&rx) {
                eprintln!("laser pointer unavailable: {e}");
                // Keep taking updates so senders never block or fail.
                while rx.recv().is_ok() {}
            }
        });
        Self(tx)
    }

    /// `on` with `x`, `y` as fractions of the primary screen (`0.0..=1.0`); off hides it.
    pub fn set(&self, on: bool, x: f32, y: f32) {
        let _ = self.0.send(on.then_some((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))));
    }
}

/// Where the dot's window goes: the fraction of `screen` (x, y, width, height), centred.
fn place(screen: (i16, i16, u16, u16), x: f32, y: f32) -> (i32, i32) {
    let (sx, sy, w, h) = screen;
    let half = i32::from(SIZE) / 2;
    (
        i32::from(sx) + (x * f32::from(w)).round() as i32 - half,
        i32::from(sy) + (y * f32::from(h)).round() as i32 - half,
    )
}

/// The primary monitor's area, or the whole root window if RandR doesn't say.
pub(crate) fn primary(conn: &RustConnection, root: Window, fallback: (u16, u16)) -> (i16, i16, u16, u16) {
    let found = (|| -> Result<_, Error> {
        let output = conn.randr_get_output_primary(root)?.reply()?.output;
        let info = conn.randr_get_output_info(output, 0)?.reply()?;
        let crtc = conn.randr_get_crtc_info(info.crtc, 0)?.reply()?;
        Ok((crtc.x, crtc.y, crtc.width, crtc.height))
    })();
    found.ok().filter(|g| g.2 > 0 && g.3 > 0).unwrap_or((0, 0, fallback.0, fallback.1))
}

fn run(rx: &mpsc::Receiver<Option<(f32, f32)>>) -> Result<(), Error> {
    // Nothing to set up until the phone first uses the laser.
    let mut first = Some(rx.recv()?);
    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let (root, size) = (screen.root, (screen.width_in_pixels, screen.height_in_pixels));
    let (white, red) = (0x00ff_ffff, 0x00ff_2d55);

    let win = conn.generate_id()?;
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        win,
        root,
        0,
        0,
        SIZE,
        SIZE,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        // Override-redirect: no frame, not in the task list, kept above by us.
        &CreateWindowAux::new().override_redirect(1).background_pixel(white).event_mask(EventMask::EXPOSURE),
    )?;
    let full = Arc { x: 0, y: 0, width: SIZE, height: SIZE, angle1: 0, angle2: 360 * 64 };
    // Round: a one-bit mask with a filled circle is the window's shape.
    let mask = conn.generate_id()?;
    conn.create_pixmap(1, mask, win, SIZE, SIZE)?;
    let mask_gc = conn.generate_id()?;
    conn.create_gc(mask_gc, mask, &CreateGCAux::new().foreground(0))?;
    conn.poly_fill_rectangle(mask, mask_gc, &[Rectangle { x: 0, y: 0, width: SIZE, height: SIZE }])?;
    conn.change_gc(mask_gc, &x11rb::protocol::xproto::ChangeGCAux::new().foreground(1))?;
    conn.poly_fill_arc(mask, mask_gc, &[full])?;
    conn.shape_mask(shape::SO::SET, shape::SK::BOUNDING, win, 0, 0, mask)?;
    // An empty input shape: clicks go to whatever is under the dot.
    conn.shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, win, 0, 0, &[])?;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, win, &CreateGCAux::new().foreground(red))?;
    let inner = Arc { x: 4, y: 4, width: SIZE - 8, height: SIZE - 8, angle1: 0, angle2: 360 * 64 };
    // A white rim around a red centre reads on dark and light slides alike.
    let draw = |conn: &RustConnection| -> Result<(), Error> {
        conn.poly_fill_arc(win, gc, &[inner])?;
        conn.flush()?;
        Ok(())
    };

    let mut shown = false;
    let mut area = primary(&conn, root, size);
    let mut last = Instant::now();
    loop {
        let update = match first.take() {
            Some(u) => Ok(u),
            None => rx.recv_timeout(Duration::from_millis(100)),
        };
        match update {
            Ok(Some((x, y))) => {
                if !shown {
                    area = primary(&conn, root, size);
                    conn.map_window(win)?;
                    shown = true;
                }
                let (px, py) = place(area, x, y);
                conn.configure_window(
                    win,
                    &ConfigureWindowAux::new().x(px).y(py).stack_mode(StackMode::ABOVE),
                )?;
                draw(&conn)?;
                last = Instant::now();
            }
            Ok(None) => {
                if shown {
                    conn.unmap_window(win)?;
                    conn.flush()?;
                    shown = false;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if shown && last.elapsed() > IDLE {
                    conn.unmap_window(win)?;
                    conn.flush()?;
                    shown = false;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
        while let Some(event) = conn.poll_for_event()? {
            if matches!(event, Event::Expose(_)) && shown {
                draw(&conn)?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_is_centred_on_the_fraction_of_the_primary_screen() {
        let half = i32::from(SIZE) / 2;
        assert_eq!(place((0, 0, 1920, 1080), 0.5, 0.5), (960 - half, 540 - half));
        assert_eq!(place((0, 0, 1920, 1080), 0.0, 0.0), (-half, -half));
        // A primary monitor to the right of another one.
        assert_eq!(place((1920, 0, 2560, 1440), 1.0, 1.0), (1920 + 2560 - half, 1440 - half));
    }
}
