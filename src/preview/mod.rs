//! Thumbnails of the windows being switched between.
//!
//! Capturing happens on a thread of its own, with its own Wayland connection,
//! and the results arrive on a channel — the same shape as the compositor's
//! event socket. Nothing on the path from a key press to a focused window ever
//! waits for any of it: a thumbnail that is slow, or never comes, only means a
//! row keeps its placeholder.

mod capture;
mod colour;

use std::collections::HashMap;
use std::thread;

use async_channel::{Receiver, Sender};

use capture::Capturer;
use colour::Histogram;
pub(crate) use colour::{Rgb, Tint};

/// The shape a tile assumes a window has until it has been captured and the
/// compositor won't say how big it is.
pub(crate) const RATIO: f32 = 1.6;

/// The two colours a rectangle of pixels is mostly made of.
///
/// The pixels are as both the compositor and GTK hand them over: four bytes
/// each, blue first, with the colour already multiplied by the alpha.
pub(crate) fn colours(memory: &[u8]) -> Option<Tint> {
    let mut histogram = Histogram::default();

    for pixel in memory.chunks_exact(4) {
        // A transparent pixel is stored premultiplied, so it would vote for
        // black if counted.
        if pixel[3] >= 128 {
            histogram.add(pixel[2], pixel[1], pixel[0]);
        }
    }

    histogram.tint()
}

/// A window's contents, small enough to sit in a list.
pub(crate) struct Thumbnail {
    /// The window it belongs to, as the compositor identifies it.
    pub(crate) identifier: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Bytes in the order GDK's `B8g8r8a8` premultiplied format wants them.
    pub(crate) pixels: Vec<u8>,
    /// The two colours the window is mostly made of, when enough of it was
    /// seen to say.
    pub(crate) tint: Option<Tint>,
}

/// A window to capture, and what to call it if the capture goes wrong.
pub(crate) struct Request {
    pub(crate) identifier: String,
    /// The window as the user would recognise it, for the one line raisin
    /// prints when a capture doesn't come back.
    pub(crate) label: String,
}

enum Command {
    Capture { windows: Vec<Request>, width: u32 },
    Cancel,
}

/// The captures going on in the background.
pub(crate) struct Previews {
    commands: Sender<Command>,
}

impl Previews {
    /// Starts the capturing thread and hands back the thumbnails it produces.
    ///
    /// A compositor that can't capture windows isn't an error: the thread says
    /// so once and stops, and the switcher shows titles alone.
    pub(crate) fn start() -> (Self, Receiver<Thumbnail>) {
        let (commands, incoming) = async_channel::unbounded();
        let (outgoing, thumbnails) = async_channel::unbounded();

        thread::spawn(move || run(&incoming, &outgoing));

        (Self { commands }, thumbnails)
    }

    /// Asks for these windows, in this order, instead of whatever was being
    /// captured before.
    ///
    /// Every call names every window wanted rather than the ones that changed,
    /// which is what lets the next call abandon this one part-way through.
    pub(crate) fn capture(&self, windows: Vec<Request>, width: u32) {
        let _ = self
            .commands
            .send_blocking(Command::Capture { windows, width });
    }

    /// Stops capturing: the switcher has gone.
    pub(crate) fn cancel(&self) {
        let _ = self.commands.send_blocking(Command::Cancel);
    }
}

fn run(commands: &Receiver<Command>, thumbnails: &Sender<Thumbnail>) {
    let mut capturer = match Capturer::connect() {
        Ok(capturer) => capturer,
        Err(error) => {
            eprintln!("raisin: no window previews: {error:#}");
            return;
        }
    };

    // How many times in a row each window has refused to come back.
    let mut stubborn: HashMap<String, u32> = HashMap::new();

    while let Ok(command) = commands.recv_blocking() {
        let Command::Capture { windows, width } = command else {
            continue;
        };

        if let Err(error) = capturer.refresh() {
            eprintln!("raisin: no more window previews: {error:#}");
            return;
        }

        // A window that didn't come back gets one more go at the end, once
        // whatever the compositor was busy with has passed.
        let mut failed = match capture(&mut capturer, windows, width, thumbnails, commands) {
            Ok(failed) => failed,
            Err(Gone) => return,
        };

        // A batch that was given up on doesn't get the second go: whatever
        // replaced it is waiting, and will ask for these windows again. What
        // already failed is still worth saying, though — the loop stops before
        // trying a window rather than after, so nothing here went unattempted.
        if !failed.is_empty() && commands.is_empty() {
            // A window that has failed twice running isn't going to come back
            // this time either. Trying it again costs the whole timeout, and
            // every window still waiting behind it waits that much longer for
            // a picture it would have got.
            let (hopeless, retrying): (Vec<_>, Vec<_>) = failed
                .into_iter()
                .partition(|(window, _)| stubborn.get(&window.identifier).is_some_and(|n| *n >= 2));
            let retrying = retrying.into_iter().map(|(window, _)| window).collect();

            failed = match capture(&mut capturer, retrying, width, thumbnails, commands) {
                Ok(failed) => failed,
                Err(Gone) => return,
            };

            // Said once, when a window first stops coming back, and not again
            // on every switch after that.
            failed.retain(|(window, _)| {
                !hopeless
                    .iter()
                    .any(|(gone, _)| gone.identifier == window.identifier)
            });
        }

        for (window, _) in &failed {
            *stubborn.entry(window.identifier.clone()).or_insert(0) += 1;
        }

        // Anything that did come back starts again from nothing: a window that
        // was off screen a moment ago may well be back.
        stubborn.retain(|identifier, _| {
            failed
                .iter()
                .any(|(window, _)| &window.identifier == identifier)
        });

        complain(&failed);
    }
}

/// Nobody is listening for thumbnails any more.
struct Gone;

/// Captures each window in turn, returning the ones that didn't come back.
///
/// Stops as soon as another request turns up. A capture takes as long as the
/// compositor takes to draw a frame, so a batch overtaken half way through
/// would otherwise keep the one that replaced it waiting on windows nobody is
/// looking at any more.
fn capture(
    capturer: &mut Capturer,
    windows: Vec<Request>,
    width: u32,
    thumbnails: &Sender<Thumbnail>,
    commands: &Receiver<Command>,
) -> Result<Vec<(Request, anyhow::Error)>, Gone> {
    let mut failed = Vec::new();

    for window in windows {
        if !commands.is_empty() {
            break;
        }

        match capturer.capture(&window.identifier, width) {
            Ok(thumbnail) => {
                if thumbnails.send_blocking(thumbnail).is_err() {
                    return Err(Gone);
                }
            }
            Err(error) => failed.push((window, error)),
        }
    }

    Ok(failed)
}

/// One line about the windows that couldn't be captured, however many there
/// were: the point is to say what went wrong once, not once per window.
fn complain(failed: &[(Request, anyhow::Error)]) {
    let Some((window, error)) = failed.first() else {
        return;
    };

    let others = match failed.len() - 1 {
        0 => String::new(),
        1 => " (and one other window)".to_owned(),
        others => format!(" (and {others} other windows)"),
    };

    eprintln!(
        "raisin: couldn't preview {}: {error:#}{others}",
        window.label
    );
}

/// Scales a window's contents down to `target` pixels across.
///
/// The pixels that fall into each one are averaged rather than sampled, or a
/// window full of text would come back as noise. Very large windows are
/// sampled within each block so that the work stays bounded whatever the
/// resolution.
fn scale(
    identifier: &str,
    memory: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    target: u32,
    opaque: bool,
) -> Thumbnail {
    // Height is what every thumbnail has in common, so it alone decides how
    // far the window is scaled down; the width it ends up is the window's own
    // proportions at that height.
    let target = target.max(1);
    let factor = height.div_ceil(target).max(1);
    let step = (factor / 4).max(1);
    let (thumbnail_width, thumbnail_height) = ((width / factor).max(1), (height / factor).max(1));

    let mut pixels = Vec::with_capacity((thumbnail_width * thumbnail_height * 4) as usize);
    // Counted from the samples rather than from the thumbnail: averaging a
    // block of pixels into one washes the colour out of it, and what a window
    // is recognised by is exactly the colour that washing removes.
    let mut histogram = Histogram::default();

    for row in 0..thumbnail_height {
        for column in 0..thumbnail_width {
            let (mut blue, mut green, mut red, mut alpha, mut taken) =
                (0u32, 0u32, 0u32, 0u32, 0u32);

            for offset_y in (0..factor).step_by(step as usize) {
                let y = row * factor + offset_y;

                if y >= height {
                    break;
                }

                for offset_x in (0..factor).step_by(step as usize) {
                    let x = column * factor + offset_x;
                    let at = (y * stride + x * 4) as usize;

                    if x >= width || at + 3 >= memory.len() {
                        break;
                    }

                    // A transparent pixel is stored premultiplied, so it
                    // would vote for black if counted.
                    if opaque || memory[at + 3] >= 128 {
                        histogram.add(memory[at + 2], memory[at + 1], memory[at]);
                    }

                    blue += u32::from(memory[at]);
                    green += u32::from(memory[at + 1]);
                    red += u32::from(memory[at + 2]);
                    alpha += u32::from(memory[at + 3]);
                    taken += 1;
                }
            }

            let taken = taken.max(1);
            pixels.push((blue / taken) as u8);
            pixels.push((green / taken) as u8);
            pixels.push((red / taken) as u8);
            pixels.push(if opaque { 0xff } else { (alpha / taken) as u8 });
        }
    }

    Thumbnail {
        identifier: identifier.to_owned(),
        width: thumbnail_width,
        height: thumbnail_height,
        pixels,
        tint: histogram.tint(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_averages_the_pixels_that_fall_into_each_one() {
        // Four pixels: two black, two white, in one 2x2 image.
        let memory = [
            0, 0, 0, 255, 255, 255, 255, 255, // row 0
            0, 0, 0, 255, 255, 255, 255, 255, // row 1
        ];

        let thumbnail = scale("w", &memory, 2, 2, 8, 1, true);

        assert_eq!((thumbnail.width, thumbnail.height), (1, 1));
        assert_eq!(thumbnail.pixels, [127, 127, 127, 255]);
    }

    #[test]
    fn windows_of_different_shapes_come_back_the_same_height() {
        // A tall window and a wide one, both asked for 100 tall: each keeps
        // its own proportions, so only the widths differ.
        let tall = scale("tall", &vec![0; 100 * 400 * 4], 100, 400, 400, 100, true);
        let wide = scale("wide", &vec![0; 800 * 200 * 4], 800, 200, 3200, 100, true);

        assert_eq!(tall.height, 100, "{}x{}", tall.width, tall.height);
        assert_eq!(wide.height, 100, "{}x{}", wide.width, wide.height);
        assert_eq!(tall.width, 25);
        assert_eq!(wide.width, 400);
    }

    /// Says which of the open windows actually come back, which no fixture can
    /// stand in for: whether a compositor will copy a window it isn't drawing
    /// is the whole question. Ignored by default, needs a running compositor,
    /// and only reads.
    ///
    /// Run it with `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs a running compositor"]
    fn every_open_window_can_be_captured() {
        use crate::compositor::Compositor as _;

        let windows = crate::compositor::integrations::hyprland::Compositor
            .get_windows()
            .expect("failed to read the windows");
        let mut capturer = Capturer::connect().expect("failed to connect for captures");
        capturer
            .refresh()
            .expect("failed to list the capturable windows");

        let (mut ok, mut failed) = (0, 0);

        for window in windows.iter().filter(|w| !w.identifier.is_empty()) {
            let name = format!("{} ({})", window.title, window.app_id);

            match capturer.capture(&window.identifier, 105) {
                Ok(thumbnail) => {
                    ok += 1;
                    println!(
                        "  ok    {name:<44} {}x{}",
                        thumbnail.width, thumbnail.height
                    );
                }
                Err(error) => {
                    failed += 1;
                    println!("  FAIL  {name:<44} {error:#}");
                }
            }
        }

        println!("{ok} captured, {failed} failed");
    }

    #[test]
    fn a_window_smaller_than_the_thumbnail_is_left_alone() {
        let memory = [1, 2, 3, 255, 4, 5, 6, 255];
        let thumbnail = scale("w", &memory, 2, 1, 8, 400, true);

        assert_eq!((thumbnail.width, thumbnail.height), (2, 1));
        assert_eq!(thumbnail.pixels, [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn transparency_is_kept_unless_the_format_has_none() {
        let memory = [10, 20, 30, 40];

        assert_eq!(
            scale("w", &memory, 1, 1, 4, 1, false).pixels,
            [10, 20, 30, 40]
        );
        assert_eq!(
            scale("w", &memory, 1, 1, 4, 1, true).pixels,
            [10, 20, 30, 255]
        );
    }
}
