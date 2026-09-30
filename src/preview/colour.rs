//! The two colours a window is mostly made of.
//!
//! Nothing here knows about Wayland or GTK: it takes pixels and answers with
//! colours, so the rules it follows can be argued with in a unit test rather
//! than by squinting at a screenshot.
//!
//! The rules, in short. A colour is only ever reported as the average of
//! pixels that were really there, never as the centre of the bucket they fell
//! in. A window that is honestly grey is reported as grey — a chromatic colour
//! is preferred only when the window has enough of one to be worth preferring.
//! And the two colours come back darkest first, so a window whose two colours
//! are close doesn't flip its gradient round between one capture and the next.

/// How many bits of each channel a bucket keeps. Four gives 4096 buckets,
/// which is coarse enough to gather a colour that varies slightly across a
/// window and fine enough to tell two colours apart.
const BITS: u32 = 4;
const STEPS: usize = 1 << BITS;
const BUCKETS: usize = STEPS * STEPS * STEPS;

/// How much of a window a colour has to cover before being preferred for
/// having some colour in it. Below this, whatever covers the most wins,
/// whatever it looks like.
const QUORUM: f32 = 0.05;

/// How colourful a colour has to be to count as one, in Oklab chroma.
const COLOURFUL: f32 = 0.04;

/// How far apart the two colours have to be, as an Oklab distance.
const APART: f32 = 0.15;

/// How dark a colour is allowed to end up. A window that is nearly black is
/// still shown as a tint of its own rather than as a hole in the panel.
const FLOOR: f32 = 0.62;

/// And how light. The marker's title sits over the colour, and a window that
/// is nearly white would leave it nothing to read against. Holding the palest
/// windows down here is what makes it safe to show more of all the others.
const CEILING: f32 = 0.82;

/// A colour, as the sRGB bytes it will be written back out as.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Rgb {
    pub(crate) red: u8,
    pub(crate) green: u8,
    pub(crate) blue: u8,
}

/// What a window is mostly made of: two colours, darkest first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Tint(pub(crate) Rgb, pub(crate) Rgb);

/// Where a colour can be reasoned about: `l` is how light it looks, and `a`
/// and `b` are what colour it is. Oklab, because a step in it is about the
/// same size wherever you take it — which is what makes one distance and one
/// lightness floor work for every hue.
#[derive(Clone, Copy)]
struct Oklab {
    l: f32,
    a: f32,
    b: f32,
}

impl Oklab {
    /// How colourful, regardless of how light.
    fn chroma(self) -> f32 {
        self.a.hypot(self.b)
    }

    fn distance(self, other: Self) -> f32 {
        ((self.l - other.l).powi(2) + (self.a - other.a).powi(2) + (self.b - other.b).powi(2))
            .sqrt()
    }
}

/// Counts what colours a window is made of.
///
/// One entry per bucket, holding how many pixels fell in it and their total,
/// so the colour reported back is the average of real pixels.
pub(crate) struct Histogram {
    buckets: Vec<Bucket>,
    seen: u32,
}

#[derive(Clone, Copy, Default)]
struct Bucket {
    count: u32,
    red: u32,
    green: u32,
    blue: u32,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            buckets: vec![Bucket::default(); BUCKETS],
            seen: 0,
        }
    }
}

impl Histogram {
    pub(crate) fn add(&mut self, red: u8, green: u8, blue: u8) {
        let bucket = &mut self.buckets[index(red, green, blue)];

        bucket.count += 1;
        bucket.red += u32::from(red);
        bucket.green += u32::from(green);
        bucket.blue += u32::from(blue);

        self.seen += 1;
    }

    /// The two colours the window is mostly made of, or `None` when too little
    /// of it was seen to say.
    pub(crate) fn tint(&self) -> Option<Tint> {
        if self.seen < 16 {
            return None;
        }

        let candidates = self.candidates();
        let first = self.pick(&candidates, |_| true)?;
        let far = |colour: &Candidate| colour.lab.distance(first.lab) >= APART;

        // A window with only one colour in it gets a gradient between that
        // colour and itself, which is the honest way to draw it.
        let second = self.pick(&candidates, far).unwrap_or(first);
        let (first, second) = (banded(first.lab), banded(second.lab));

        Some(if first.l <= second.l {
            Tint(rgb(first), rgb(second))
        } else {
            Tint(rgb(second), rgb(first))
        })
    }

    /// Every bucket that anything fell into, scored with its neighbours so a
    /// colour split across a boundary is counted once rather than twice.
    fn candidates(&self) -> Vec<Candidate> {
        self.buckets
            .iter()
            .enumerate()
            .filter(|(_, bucket)| bucket.count > 0)
            .map(|(at, bucket)| {
                let score = self.around(at);
                let mean = [
                    bucket.red / bucket.count,
                    bucket.green / bucket.count,
                    bucket.blue / bucket.count,
                ];

                Candidate {
                    score,
                    lab: oklab(mean[0] as u8, mean[1] as u8, mean[2] as u8),
                }
            })
            .collect()
    }

    /// What fell into a bucket and into the twenty-six around it.
    fn around(&self, at: usize) -> u32 {
        let (red, green, blue) = coordinates(at);
        let mut total = 0;

        for dr in -1i32..=1 {
            for dg in -1i32..=1 {
                for db in -1i32..=1 {
                    let (Some(r), Some(g), Some(b)) =
                        (step(red, dr), step(green, dg), step(blue, db))
                    else {
                        continue;
                    };

                    total += self.buckets[(r * STEPS + g) * STEPS + b].count;
                }
            }
        }

        total
    }

    /// The best of the candidates `wanted` allows: the most colourful one that
    /// covers enough of the window, or failing that simply the largest.
    fn pick(
        &self,
        candidates: &[Candidate],
        wanted: impl Fn(&Candidate) -> bool,
    ) -> Option<Candidate> {
        let quorum = (self.seen as f32 * QUORUM) as u32;
        let allowed = || candidates.iter().copied().filter(|colour| wanted(colour));

        let colourful = allowed()
            .filter(|colour| {
                colour.score >= quorum
                    && colour.lab.chroma() >= COLOURFUL
                    && (0.2..=0.9).contains(&colour.lab.l)
            })
            .max_by_key(|colour| colour.score);

        colourful.or_else(|| allowed().max_by_key(|colour| colour.score))
    }
}

/// A colour the window contains, and how much of the window is near it.
#[derive(Clone, Copy)]
struct Candidate {
    score: u32,
    lab: Oklab,
}

fn index(red: u8, green: u8, blue: u8) -> usize {
    let shift = 8 - BITS;
    let (r, g, b) = (
        (red >> shift) as usize,
        (green >> shift) as usize,
        (blue >> shift) as usize,
    );

    (r * STEPS + g) * STEPS + b
}

fn coordinates(at: usize) -> (usize, usize, usize) {
    (at / (STEPS * STEPS), (at / STEPS) % STEPS, at % STEPS)
}

fn step(at: usize, by: i32) -> Option<usize> {
    let moved = at as i32 + by;

    (0..STEPS as i32).contains(&moved).then_some(moved as usize)
}

/// Brings a colour into the band the markers are drawn in, without changing
/// what colour it is.
fn banded(colour: Oklab) -> Oklab {
    Oklab {
        l: colour.l.clamp(FLOOR, CEILING),
        ..colour
    }
}

fn srgb_to_linear(channel: u8) -> f32 {
    let channel = f32::from(channel) / 255.0;

    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(channel: f32) -> u8 {
    let channel = channel.clamp(0.0, 1.0);
    let channel = if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (channel * 255.0).round().clamp(0.0, 255.0) as u8
    }
}

fn oklab(red: u8, green: u8, blue: u8) -> Oklab {
    let (r, g, b) = (
        srgb_to_linear(red),
        srgb_to_linear(green),
        srgb_to_linear(blue),
    );

    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_5 * b).cbrt();

    Oklab {
        l: 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    }
}

fn rgb(colour: Oklab) -> Rgb {
    let l = (colour.l + 0.396_337_78 * colour.a + 0.215_803_76 * colour.b).powi(3);
    let m = (colour.l - 0.105_561_346 * colour.a - 0.063_854_17 * colour.b).powi(3);
    let s = (colour.l - 0.089_484_18 * colour.a - 1.291_485_5 * colour.b).powi(3);

    Rgb {
        red: linear_to_srgb(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        green: linear_to_srgb(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        blue: linear_to_srgb(-0.004_196_086 * l - 0.703_418_6 * m + 1.707_614_7 * s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds the histogram `count` pixels of one colour.
    fn fill(histogram: &mut Histogram, colour: (u8, u8, u8), count: usize) {
        for _ in 0..count {
            histogram.add(colour.0, colour.1, colour.2);
        }
    }

    /// Roughly which colour this is, as the largest channel — enough to say
    /// "that's the red one" without pinning down an exact value.
    fn hue(colour: Rgb) -> &'static str {
        let Rgb { red, green, blue } = colour;

        if red > green && red > blue {
            "red"
        } else if blue > red && blue > green {
            "blue"
        } else if green > red && green > blue {
            "green"
        } else {
            "grey"
        }
    }

    #[test]
    fn a_window_of_two_colours_gives_both_of_them() {
        let mut histogram = Histogram::default();
        fill(&mut histogram, (200, 30, 30), 500);
        fill(&mut histogram, (30, 30, 200), 500);

        let Tint(first, second) = histogram.tint().expect("two colours");
        let found = [hue(first), hue(second)];

        assert!(found.contains(&"red"), "{found:?}");
        assert!(found.contains(&"blue"), "{found:?}");
    }

    #[test]
    fn an_accent_wins_over_the_background_it_sits_on() {
        // A terminal: mostly near-black, with enough of one colour on it to be
        // what the window is actually recognised by.
        let mut histogram = Histogram::default();
        fill(&mut histogram, (10, 10, 12), 800);
        fill(&mut histogram, (40, 110, 200), 200);

        let Tint(first, second) = histogram.tint().expect("a colour");

        assert!(
            hue(first) == "blue" || hue(second) == "blue",
            "{first:?} {second:?}"
        );
    }

    #[test]
    fn a_window_with_no_colour_in_it_stays_grey() {
        // Nothing here is colourful enough to be preferred, so the answer is
        // the grey the window really is rather than an invented accent.
        let mut histogram = Histogram::default();
        fill(&mut histogram, (20, 20, 20), 900);
        fill(&mut histogram, (60, 61, 60), 100);

        let Tint(first, _) = histogram.tint().expect("a colour");

        assert_eq!(hue(first), "grey", "{first:?}");
    }

    #[test]
    fn a_dark_window_is_lifted_off_the_floor() {
        let mut histogram = Histogram::default();
        fill(&mut histogram, (4, 4, 6), 1000);

        let Tint(first, second) = histogram.tint().expect("a colour");

        // Dark enough to be nearly invisible on the panel if left alone.
        assert!(first.red > 40, "{first:?}");
        assert_eq!(first, second, "one colour means a flat fill");
    }

    #[test]
    fn a_pale_window_is_held_under_the_ceiling() {
        let mut histogram = Histogram::default();
        fill(&mut histogram, (252, 251, 248), 1000);

        let Tint(first, _) = histogram.tint().expect("a colour");

        // Light enough to leave the title nothing to read against if left
        // alone.
        assert!(first.red < 230, "{first:?}");
    }

    #[test]
    fn one_colour_comes_back_as_a_pair_of_itself() {
        let mut histogram = Histogram::default();
        fill(&mut histogram, (120, 60, 180), 400);

        let Tint(first, second) = histogram.tint().expect("a colour");

        assert_eq!(first, second);
    }

    #[test]
    fn a_colour_split_across_a_boundary_is_still_counted_once() {
        // 127 and 128 fall in neighbouring buckets; together they are the
        // window, and they must not lose to a smaller run that happens to sit
        // inside one bucket.
        let mut histogram = Histogram::default();
        fill(&mut histogram, (127, 127, 127), 300);
        fill(&mut histogram, (128, 128, 128), 300);
        fill(&mut histogram, (20, 20, 20), 350);

        let Tint(first, _) = histogram.tint().expect("a colour");

        assert!(first.red > 100, "the split colour should win: {first:?}");
    }

    #[test]
    fn too_little_to_go_on_says_so() {
        let mut histogram = Histogram::default();
        fill(&mut histogram, (10, 200, 10), 4);

        assert!(histogram.tint().is_none());
    }

    #[test]
    fn a_colour_survives_the_trip_through_oklab() {
        for colour in [(200, 30, 30), (30, 200, 30), (30, 30, 200), (180, 180, 180)] {
            let there_and_back = rgb(oklab(colour.0, colour.1, colour.2));

            assert!(
                there_and_back.red.abs_diff(colour.0) <= 1
                    && there_and_back.green.abs_diff(colour.1) <= 1
                    && there_and_back.blue.abs_diff(colour.2) <= 1,
                "{colour:?} came back as {there_and_back:?}"
            );
        }
    }
}
