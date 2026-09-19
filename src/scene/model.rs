//! What a weather scene *is*, independent of how it reaches a terminal.

/// How much room the scene has to work with. Drives art sizes and how much
/// decoration the generator is allowed to spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Compact,
    Normal,
    Wide,
}

impl Tier {
    pub fn for_width(width: usize) -> Tier {
        match width {
            0..=35 => Tier::Compact,
            36..=59 => Tier::Normal,
            _ => Tier::Wide,
        }
    }

    /// Scene height that looks balanced at this tier.
    pub fn height(self) -> usize {
        match self {
            Tier::Compact => 8,
            Tier::Normal => 11,
            Tier::Wide => 13,
        }
    }
}

/// Semantic role of a sprite. The renderer maps these to colors; the generator
/// never picks a color itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Sun,
    Moon,
    Star,
    Cloud,
    Rain,
    Snow,
    Bolt,
    Fog,
    Wind,
    Ground,
}

/// A block of art anchored at `(x, y)`. Spaces are transparent.
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    pub x: i32,
    pub y: i32,
    pub art: Vec<String>,
    pub style: Style,
}

impl Sprite {
    pub fn new(x: i32, y: i32, style: Style, art: Vec<String>) -> Self {
        Sprite { x, y, art, style }
    }

    pub fn glyph(x: i32, y: i32, style: Style, ch: char) -> Self {
        Sprite::new(x, y, style, vec![ch.to_string()])
    }

    pub fn width(&self) -> i32 {
        self.art
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0) as i32
    }

    pub fn height(&self) -> i32 {
        self.art.len() as i32
    }

    /// `(x0, y0, x1, y1)`, exclusive on the far edge.
    pub fn bbox(&self) -> (i32, i32, i32, i32) {
        (
            self.x,
            self.y,
            self.x + self.width(),
            self.y + self.height(),
        )
    }

    pub fn overlaps(&self, other: &Sprite, margin: i32) -> bool {
        let (ax0, ay0, ax1, ay1) = self.bbox();
        let (bx0, by0, bx1, by1) = other.bbox();
        ax0 - margin < bx1 && bx0 < ax1 + margin && ay0 - margin < by1 && by0 < ay1 + margin
    }

    pub fn contains(&self, px: i32, py: i32, margin: i32) -> bool {
        let (x0, y0, x1, y1) = self.bbox();
        px >= x0 - margin && px < x1 + margin && py >= y0 - margin && py < y1 + margin
    }
}

/// The complete description of a drawable weather scene.
///
/// Layers are composited back-to-front in the order returned by [`Self::layers`].
#[derive(Debug, Clone, PartialEq)]
pub struct SceneModel {
    pub width: usize,
    pub height: usize,
    pub tier: Tier,
    pub night: bool,
    /// Stars and other far-background specks.
    pub background: Vec<Sprite>,
    /// Sun or moon.
    pub celestial: Vec<Sprite>,
    pub clouds: Vec<Sprite>,
    pub precipitation: Vec<Sprite>,
    /// Fog banks and haze.
    pub atmosphere: Vec<Sprite>,
    pub wind: Vec<Sprite>,
    /// Horizon line.
    pub foreground: Vec<Sprite>,
}

impl SceneModel {
    pub fn empty(width: usize, height: usize, tier: Tier, night: bool) -> Self {
        SceneModel {
            width,
            height,
            tier,
            night,
            background: Vec::new(),
            celestial: Vec::new(),
            clouds: Vec::new(),
            precipitation: Vec::new(),
            atmosphere: Vec::new(),
            wind: Vec::new(),
            foreground: Vec::new(),
        }
    }

    pub fn layers(&self) -> [&[Sprite]; 7] {
        [
            &self.background,
            &self.celestial,
            &self.clouds,
            &self.precipitation,
            &self.atmosphere,
            &self.wind,
            &self.foreground,
        ]
    }

    pub fn sprite_count(&self) -> usize {
        self.layers().iter().map(|l| l.len()).sum()
    }
}
