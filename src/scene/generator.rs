//! Turns weather semantics into a scene. This layer answers "what should this
//! weather look like?" — never "how do I paint a terminal?".

use crate::scene::model::{SceneModel, Sprite, Style, Tier};
use crate::scene::primitives::{self, Glyphs, Size};
use crate::scene::rng::Rng;
use crate::weather::{Intensity, Units, WeatherCondition, WeatherData};

/// Everything the generator needs besides the weather itself.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Scene width in columns.
    pub width: usize,
    /// Scene height; defaults to the tier's natural height.
    pub height: Option<usize>,
    pub seed: u64,
    /// Animation frame. 0 is the canonical still image.
    pub frame: u64,
    /// Restrict art to pure ASCII.
    pub ascii: bool,
}

impl RenderOptions {
    pub fn new(width: usize, seed: u64) -> Self {
        RenderOptions {
            width,
            height: None,
            seed,
            frame: 0,
            ascii: false,
        }
    }
}

/// Stable per-observation seed: the same reading always draws the same scene,
/// a new reading redraws it. Callers can override with `--seed`.
pub fn default_seed(weather: &WeatherData) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in weather
        .location
        .name
        .as_bytes()
        .iter()
        .chain(weather.observed_at.as_bytes())
    {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

const MIN_WIDTH: usize = 14;

pub fn generate(weather: &WeatherData, opts: &RenderOptions) -> SceneModel {
    let glyphs = Glyphs::new(opts.ascii);
    let width = opts.width.max(MIN_WIDTH);
    let tier = Tier::for_width(width);
    let wet = weather.condition.is_rainy() || weather.condition.is_snowy();
    // A dry sky does not need room for falling things.
    let height = opts
        .height
        .unwrap_or_else(|| {
            if wet {
                tier.height()
            } else {
                tier.height() - 3
            }
        })
        .max(5);
    let night = weather.is_night();
    let mut scene = SceneModel::empty(width, height, tier, night);
    let mut rng = Rng::new(opts.seed);

    let (cloud_count, cloud_size) = cloud_plan(weather, tier);
    let show_celestial = cloud_count <= 1 && weather.condition != WeatherCondition::Fog;

    if show_celestial {
        scene.celestial.push(celestial(
            night,
            tier,
            width,
            cloud_count == 0 && !wet,
            &mut rng,
        ));
    }
    scene.clouds = clouds(
        cloud_count,
        cloud_size,
        width,
        height,
        &scene.celestial,
        opts.frame,
        &mut rng,
    );
    if !wet && weather.condition != WeatherCondition::Fog {
        settle(&mut scene, height);
    }

    if night {
        scene.background = stars(
            weather, &scene, width, height, &glyphs, opts.frame, &mut rng,
        );
    }

    let cloud_floor = scene
        .clouds
        .iter()
        .map(|c| c.y + c.height())
        .max()
        .unwrap_or(2);
    let falling = precipitation(weather, &scene, cloud_floor, &glyphs, opts.frame, &mut rng);
    scene.precipitation = falling;
    if weather.condition == WeatherCondition::Thunderstorm {
        let bolt = lightning(&scene.clouds, cloud_floor, opts.frame);
        // Keep the strike legible: nothing falls through it.
        scene
            .precipitation
            .retain(|drop| !bolt.iter().any(|b| b.overlaps(drop, 2)));
        scene.precipitation.extend(bolt);
    }

    if weather.condition == WeatherCondition::Fog {
        scene.atmosphere = fog(cloud_floor, width, height, &glyphs, &mut rng);
    }
    scene.wind = wind(weather, tier, width, height, &glyphs, &mut rng);
    if tier != Tier::Compact {
        scene.foreground.push(Sprite::new(
            0,
            height as i32 - 1,
            Style::Ground,
            vec![primitives::horizon(width, &glyphs, &mut rng)],
        ));
    }

    scene
}

/// How many clouds, and how big. Condition sets the shape, cloud cover nudges it.
fn cloud_plan(weather: &WeatherData, tier: Tier) -> (usize, Size) {
    use WeatherCondition::*;
    let (mut count, mut size) = match weather.condition {
        Clear => (usize::from(weather.cloud_cover > 45), Size::Small),
        PartlyCloudy => (1, Size::Medium),
        Cloudy => (2, Size::Large),
        Fog | Drizzle | Unknown => (1, Size::Medium),
        Rain | Snow => (2, Size::Medium),
        HeavyRain | HeavySnow | Thunderstorm => (2, Size::Large),
    };
    if tier == Tier::Wide && count >= 2 && weather.cloud_cover > 80 {
        count = 3;
    }
    match tier {
        Tier::Compact => {
            count = count.min(1);
            size = Size::Small;
        }
        Tier::Normal => {
            count = count.min(2);
            size = size.min_of(Size::Medium);
        }
        Tier::Wide => count = count.min(3),
    }
    (count, size)
}

trait SizeExt {
    fn min_of(self, cap: Size) -> Size;
}

impl SizeExt for Size {
    fn min_of(self, cap: Size) -> Size {
        let rank = |s: Size| match s {
            Size::Small => 0,
            Size::Medium => 1,
            Size::Large => 2,
        };
        if rank(self) > rank(cap) { cap } else { self }
    }
}

/// `centered` means the sky is otherwise empty, so the sun/moon becomes the
/// subject of the picture rather than a corner detail.
fn celestial(night: bool, tier: Tier, width: usize, centered: bool, rng: &mut Rng) -> Sprite {
    let size = match tier {
        Tier::Compact => Size::Small,
        Tier::Normal | Tier::Wide => Size::Large,
    };
    let art = if night {
        primitives::moon(size)
    } else {
        primitives::sun(size)
    };
    let art_width = art.iter().map(|l| l.chars().count()).max().unwrap_or(1) as i32;
    let span = width as i32 - art_width;
    let x = if centered {
        span / 2 + rng.range(-2, 2)
    } else if rng.chance(50) {
        rng.range(1, 4)
    } else {
        span - rng.range(1, 4)
    };
    let style = if night { Style::Moon } else { Style::Sun };
    Sprite::new(x.clamp(0, span.max(0)), 0, style, art)
}

fn clouds(
    count: usize,
    size: Size,
    width: usize,
    height: usize,
    celestial: &[Sprite],
    frame: u64,
    rng: &mut Rng,
) -> Vec<Sprite> {
    let mut placed: Vec<Sprite> = Vec::new();
    let band = ((height / 3) as i32 - 1).max(0);
    for i in 0..count {
        // The first cloud is the "hero"; later ones shrink so the sky reads as depth.
        let art = primitives::cloud(if i == 0 {
            size
        } else {
            size.min_of(Size::Medium)
        });
        let art_width = art.iter().map(|l| l.chars().count()).max().unwrap_or(1) as i32;
        let span = (width as i32 - art_width).max(0);
        let mut best: Option<Sprite> = None;
        for _ in 0..16 {
            let candidate = Sprite::new(
                rng.range(0, span),
                rng.range(0, band),
                Style::Cloud,
                art.clone(),
            );
            let clashes = celestial
                .iter()
                .chain(placed.iter())
                .any(|other| candidate.overlaps(other, 1));
            if !clashes {
                best = Some(candidate);
                break;
            }
            best.get_or_insert(candidate);
        }
        if let Some(mut sprite) = best {
            // Clouds crawl sideways over time and wrap around the sky.
            let drift = (frame / 6) as i32;
            if drift > 0 {
                sprite.x = (sprite.x + drift).rem_euclid(width as i32 + art_width) - art_width;
            }
            placed.push(sprite);
        }
    }
    placed
}

fn stars(
    weather: &WeatherData,
    scene: &SceneModel,
    width: usize,
    height: usize,
    glyphs: &Glyphs,
    frame: u64,
    rng: &mut Rng,
) -> Vec<Sprite> {
    let openness = (100u32.saturating_sub(weather.cloud_cover as u32)) as f64 / 100.0;
    let sky_rows = height.saturating_sub(3);
    // Even a covered sky keeps a few stars: night must never render as day.
    let count = (((width * sky_rows) as f64 * 0.035 * openness) as usize).max(3);
    let mut out = Vec::new();
    for _ in 0..count * 4 {
        if out.len() >= count {
            break;
        }
        let x = rng.range(0, width as i32 - 1);
        let y = rng.range(0, sky_rows as i32 - 1);
        let occupied = scene
            .clouds
            .iter()
            .chain(scene.celestial.iter())
            .any(|s| s.contains(x, y, 1));
        if occupied {
            continue;
        }
        // Bright stars are rare, and a different one twinkles each beat.
        let glyph = if frame > 0 && (frame / 4 + out.len() as u64).is_multiple_of(9) {
            glyphs.star[2]
        } else {
            match rng.below(10) {
                0 => glyphs.star[2],
                1..=3 => glyphs.star[1],
                _ => glyphs.star[0],
            }
        };
        out.push(Sprite::glyph(x, y, Style::Star, glyph));
    }
    out
}

/// Falling particles. `frame` advances them without changing where they started,
/// so animation is the same scene sampled at a later time.
fn precipitation(
    weather: &WeatherData,
    scene: &SceneModel,
    cloud_floor: i32,
    glyphs: &Glyphs,
    frame: u64,
    rng: &mut Rng,
) -> Vec<Sprite> {
    let (width, height, clouds) = (scene.width, scene.height, &scene.clouds[..]);
    let condition = weather.condition;
    if !condition.is_rainy() && !condition.is_snowy() {
        return Vec::new();
    }
    let top = cloud_floor.max(1);
    let bottom = height as i32 - 1;
    if bottom <= top {
        return Vec::new();
    }
    let span = (bottom - top) as u64;

    let density = match weather.intensity {
        Intensity::Light => 0.06,
        Intensity::Moderate => 0.11,
        Intensity::Heavy => 0.17,
    } * if matches!(
        condition,
        WeatherCondition::HeavyRain | WeatherCondition::HeavySnow
    ) {
        1.25
    } else {
        1.0
    };
    let count = ((width as f64 * span as f64) * density).round() as usize;

    let kmh = match weather.units {
        Units::Metric => weather.wind_speed,
        Units::Imperial => weather.wind_speed * 1.609,
    };
    let slant = if kmh >= 45.0 {
        2
    } else if kmh >= 20.0 {
        1
    } else {
        0
    };
    let rightward = (180..360).contains(&weather.wind_direction);
    let style = if condition.is_snowy() {
        Style::Snow
    } else {
        Style::Rain
    };

    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        // Rain falls out of clouds, not out of clear sky.
        let x0 = match clouds.is_empty() {
            true => rng.range(0, width as i32 - 1),
            false => {
                let cloud = &clouds[rng.below(clouds.len())];
                cloud.x + rng.range(-2, cloud.width() + 1)
            }
        };
        let y0 = rng.below(span.max(1) as usize) as u64;
        // Snow drifts, rain falls.
        let speed = if condition.is_snowy() {
            1
        } else {
            1 + rng.below(2) as u64
        };
        let glyph = if condition.is_snowy() {
            glyphs.snow[match rng.below(6) {
                0 => 2,
                1 | 2 => 1,
                _ => 0,
            }]
        } else if slant > 0 {
            if rightward {
                glyphs.slant_right
            } else {
                glyphs.slant_left
            }
        } else {
            match weather.intensity {
                Intensity::Light => glyphs.drizzle,
                Intensity::Moderate => glyphs.rain,
                Intensity::Heavy => glyphs.rain_heavy,
            }
        };

        let advanced = (y0 + frame * speed) % span.max(1);
        let y = top + advanced as i32;
        let drift = slant * frame as i32 * if rightward { 1 } else { -1 };
        let x = (x0 + drift).rem_euclid(width as i32);
        out.push(Sprite::glyph(x, y, style, glyph));
    }
    out
}

/// A bolt below the biggest cloud. It strobes over time but is always present
/// on frame 0, so the still image shows the storm.
fn lightning(clouds: &[Sprite], cloud_floor: i32, frame: u64) -> Vec<Sprite> {
    let anchor = clouds.iter().max_by_key(|c| c.width());
    let x = anchor.map(|c| c.x + c.width() / 2 - 1).unwrap_or(0);
    if frame == 0 || frame % 17 < 3 {
        vec![Sprite::new(
            x.max(0),
            cloud_floor,
            Style::Bolt,
            primitives::bolt(),
        )]
    } else {
        Vec::new()
    }
}

/// Bands of haze stacked below the cloud, every other row.
fn fog(
    cloud_floor: i32,
    width: usize,
    height: usize,
    glyphs: &Glyphs,
    rng: &mut Rng,
) -> Vec<Sprite> {
    let mut out = Vec::new();
    let mut y = cloud_floor.max(1);
    while y < height as i32 - 1 {
        out.push(Sprite::new(
            0,
            y,
            Style::Fog,
            vec![primitives::fog_band(width, glyphs, rng)],
        ));
        y += 2;
    }
    out
}

/// Nudge a dry sky downwards so the art sits between the top and the horizon
/// instead of clinging to the ceiling.
fn settle(scene: &mut SceneModel, height: usize) {
    let bottom = scene
        .celestial
        .iter()
        .chain(scene.clouds.iter())
        .map(|s| s.y + s.height())
        .max()
        .unwrap_or(0);
    let shift = ((height as i32 - 1 - bottom) / 2).max(0);
    if shift == 0 {
        return;
    }
    for sprite in scene.celestial.iter_mut().chain(scene.clouds.iter_mut()) {
        sprite.y += shift;
    }
}

fn wind(
    weather: &WeatherData,
    tier: Tier,
    width: usize,
    height: usize,
    glyphs: &Glyphs,
    rng: &mut Rng,
) -> Vec<Sprite> {
    let kmh = match weather.units {
        Units::Metric => weather.wind_speed,
        Units::Imperial => weather.wind_speed * 1.609,
    };
    if tier == Tier::Compact || kmh < 20.0 {
        return Vec::new();
    }
    let rightward = (180..360).contains(&weather.wind_direction);
    let streaks = if kmh >= 45.0 { 2 } else { 1 };
    (0..streaks)
        .map(|i| {
            let art = primitives::wind_streak(rng.range(3, 6) as usize, rightward, glyphs);
            let span = (width as i32 - art.chars().count() as i32).max(0);
            let y = (height as i32 - 3 - i).max(1);
            Sprite::new(rng.range(0, span), y, Style::Wind, vec![art])
        })
        .collect()
}
