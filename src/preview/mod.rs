//! Thumbnails of the windows being switched between.
//!
//! Capturing happens on a thread of its own, with its own Wayland connection,
//! and the results arrive on a channel — the same shape as the compositor's
//! event socket. Nothing on the path from a key press to a focused window ever
//! waits for any of it: a thumbnail that is slow, or never comes, only means a
//! row keeps its placeholder.

mod capture;

use std::thread;

use async_channel::{Receiver, Sender};

use capture::Capturer;

/// How much wider a thumbnail is than it is tall. A window that isn't this
/// shape is fitted inside, rather than making its row a different height from
/// every other.
pub(crate) const RATIO: f32 = 1.6;

/// A window's contents, small enough to sit in a list.
pub(crate) struct Thumbnail {
    /// The window it belongs to, as the compositor identifies it.
    pub(crate) identifier: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Bytes in the order GDK's `B8g8r8a8` premultiplied format wants them.
    pub(crate) pixels: Vec<u8>,
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
        let mut failed = match capture(&mut capturer, windows, width, thumbnails) {
            Ok(failed) => failed,
            Err(Gone) => return,
        };

        if !failed.is_empty() && commands.is_empty() {
            let retrying = failed.into_iter().map(|(window, _)| window).collect();

            failed = match capture(&mut capturer, retrying, width, thumbnails) {
                Ok(failed) => failed,
                Err(Gone) => return,
            };
        }

        complain(&failed);
    }
}

/// Nobody is listening for thumbnails any more.
struct Gone;

/// Captures each window in turn, returning the ones that didn't come back.
fn capture(
    capturer: &mut Capturer,
    windows: Vec<Request>,
    width: u32,
    thumbnails: &Sender<Thumbnail>,
) -> Result<Vec<(Request, anyhow::Error)>, Gone> {
    let mut failed = Vec::new();

    for window in windows {
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
    let target = target.max(1);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let target_height = (target as f32 / RATIO) as u32;
    let factor = width
        .div_ceil(target)
        .max(height.div_ceil(target_height.max(1)))
        .max(1);
    let step = (factor / 4).max(1);
    let (thumbnail_width, thumbnail_height) = ((width / factor).max(1), (height / factor).max(1));

    let mut pixels = Vec::with_capacity((thumbnail_width * thumbnail_height * 4) as usize);

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

        let thumbnail = scale("w", &memory, 2, 2, 8, 2, true);

        assert_eq!((thumbnail.width, thumbnail.height), (1, 1));
        assert_eq!(thumbnail.pixels, [127, 127, 127, 255]);
    }

    #[test]
    fn a_tall_window_is_fitted_by_its_height_rather_than_its_width() {
        // 100 wide and 400 tall, asked for 100 across: fitting the width
        // alone would leave it 400 tall, four times the height of every other
        // row.
        let memory = vec![0; 100 * 400 * 4];
        let thumbnail = scale("w", &memory, 100, 400, 400, 100, true);

        assert!(
            thumbnail.height <= (100.0 / RATIO) as u32,
            "{}x{}",
            thumbnail.width,
            thumbnail.height
        );
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
