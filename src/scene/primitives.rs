//! The visual vocabulary: small composable pieces of art, not one picture per
//! weather condition. The generator decides *which* pieces and *where*.

use crate::scene::rng::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Small,
    Medium,
    Large,
}

/// Single-character glyphs that differ between the default set and the
/// pure-ASCII set used by `--plain`.
#[derive(Debug, Clone, Copy)]
pub struct Glyphs {
    pub drizzle: char,
    pub rain: char,
    pub rain_heavy: char,
    pub slant_right: char,
    pub slant_left: char,
    /// Light, moderate, heavy flakes.
    pub snow: [char; 3],
    pub star: [char; 3],
    pub fog: char,
    pub wind: char,
    pub wind_head: char,
    pub ground: char,
}

impl Glyphs {
    pub fn new(ascii: bool) -> Self {
        if ascii {
            Glyphs {
                drizzle: '\'',
                rain: '|',
                rain_heavy: '|',
                slant_right: '/',
                slant_left: '\\',
                snow: ['.', '*', '+'],
                star: ['.', '*', '+'],
                fog: '~',
                wind: '-',
                wind_head: '>',
                ground: '_',
            }
        } else {
            Glyphs {
                drizzle: '\'',
                rain: '│',
                rain_heavy: '┃',
                slant_right: '╱',
                slant_left: '╲',
                snow: ['·', '*', '✻'],
                star: ['·', '*', '✦'],
                fog: '~',
                wind: '~',
                wind_head: '>',
                ground: '_',
            }
        }
    }
}

fn art(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| l.to_string()).collect()
}

pub fn sun(size: Size) -> Vec<String> {
    match size {
        Size::Large => art(&[
            "  \\   |   /",
            "   .-----.",
            "--(       )--",
            "   `-----'",
            "  /   |   \\",
        ]),
        Size::Medium => art(&[" \\ | /", " .---.", "-(   )-", " `---'", " / | \\"]),
        Size::Small => art(&[" \\|/", "-( )-", " /|\\"]),
    }
}

pub fn moon(size: Size) -> Vec<String> {
    match size {
        Size::Large => art(&[
            "   .---.",
            "  /  .  \\",
            " |   o   |",
            "  \\  .  /",
            "   `---'",
        ]),
        Size::Medium => art(&[" .---.", "( . o )", " `---'"]),
        Size::Small => art(&[".-.", "(o)", "`-'"]),
    }
}

pub fn cloud(size: Size) -> Vec<String> {
    match size {
        Size::Large => art(&["     .-----.", "  .-(       ).", " (____.________)"]),
        Size::Medium => art(&["    .---.", " .-(     ).", "(____._____)"]),
        Size::Small => art(&["  .--.", " (    ).", "(_______)"]),
    }
}

/// A strike: down-left, a jog, then down-left again.
pub fn bolt() -> Vec<String> {
    art(&["  /", " /__", "   /", "  /"])
}

/// A ragged band of haze spanning `width` columns.
pub fn fog_band(width: usize, glyphs: &Glyphs, rng: &mut Rng) -> String {
    let mut line = String::new();
    while line.chars().count() < width {
        let run = rng.range(3, 9) as usize;
        for _ in 0..run {
            line.push(glyphs.fog);
        }
        for _ in 0..rng.range(1, 4) {
            line.push(' ');
        }
    }
    line.chars().take(width).collect()
}

/// `~~~~>` (or `---->`), pointing the way the wind blows.
pub fn wind_streak(length: usize, blows_right: bool, glyphs: &Glyphs) -> String {
    let tail: String = std::iter::repeat_n(glyphs.wind, length.max(1)).collect();
    if blows_right {
        format!("{tail}{}", glyphs.wind_head)
    } else {
        format!(
            "{}{tail}",
            if glyphs.wind_head == '>' {
                '<'
            } else {
                glyphs.wind_head
            }
        )
    }
}

/// A procedurally lumpy ground line — flat runs with the odd rise.
pub fn horizon(width: usize, glyphs: &Glyphs, rng: &mut Rng) -> String {
    let bumps = [".-.", ".--.", "._.", "/\\"];
    let mut line = String::new();
    while line.chars().count() < width {
        for _ in 0..rng.range(3, 10) {
            line.push(glyphs.ground);
        }
        if rng.chance(55) {
            line.push_str(rng.pick(&bumps));
        }
    }
    line.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_blocks_are_non_empty_and_bounded() {
        for size in [Size::Small, Size::Medium, Size::Large] {
            for block in [sun(size), moon(size), cloud(size)] {
                assert!(!block.is_empty());
                assert!(block.iter().all(|l| l.chars().count() <= 16));
            }
        }
        assert_eq!(bolt().len(), 4);
    }

    #[test]
    fn sizes_are_ordered() {
        let w = |b: Vec<String>| b.iter().map(|l| l.chars().count()).max().unwrap();
        assert!(w(cloud(Size::Small)) < w(cloud(Size::Medium)));
        assert!(w(cloud(Size::Medium)) < w(cloud(Size::Large)));
        assert!(w(sun(Size::Small)) < w(sun(Size::Large)));
    }

    #[test]
    fn bands_and_horizons_fill_exactly_the_requested_width() {
        let glyphs = Glyphs::new(true);
        for width in [1usize, 7, 40, 100] {
            let mut rng = Rng::new(9);
            assert_eq!(fog_band(width, &glyphs, &mut rng).chars().count(), width);
            assert_eq!(horizon(width, &glyphs, &mut rng).chars().count(), width);
        }
    }

    #[test]
    fn wind_points_both_ways() {
        let glyphs = Glyphs::new(true);
        assert_eq!(wind_streak(4, true, &glyphs), "---->");
        assert_eq!(wind_streak(4, false, &glyphs), "<----");
    }

    #[test]
    fn plain_art_is_pure_ascii() {
        let glyphs = Glyphs::new(true);
        let mut rng = Rng::new(1);
        let mut all = String::new();
        for size in [Size::Small, Size::Medium, Size::Large] {
            for block in [sun(size), moon(size), cloud(size)] {
                all.push_str(&block.join(""));
            }
        }
        all.push_str(&bolt().join(""));
        all.push_str(&fog_band(30, &glyphs, &mut rng));
        all.push_str(&horizon(30, &glyphs, &mut rng));
        all.push_str(&wind_streak(3, true, &glyphs));
        all.extend(glyphs.snow.iter().chain(glyphs.star.iter()));
        all.push(glyphs.rain);
        assert!(all.is_ascii(), "plain mode must stay ASCII: {all}");
    }
}
