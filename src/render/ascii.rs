//! Painting a [`SceneModel`] and its caption onto styled lines of text.
//!
//! This layer answers "how should this scene be drawn?" and knows nothing about
//! weather semantics beyond the numbers it prints.

use crate::render::color::{Line, Paint, Span};
use crate::scene::model::{SceneModel, Style};
use crate::weather::{WeatherCondition, WeatherData};

/// Border + one column of padding on each side.
const CHROME: usize = 6;

#[derive(Debug, Clone)]
pub struct Layout {
    /// Total columns the output may occupy.
    pub width: usize,
    /// Drop the frame and use ASCII-only glyphs.
    pub plain: bool,
}

impl Layout {
    pub fn new(width: usize, plain: bool) -> Self {
        Layout { width, plain }
    }

    /// Columns available for the scene and the caption.
    pub fn inner_width(&self) -> usize {
        if self.plain {
            self.width
        } else {
            self.width.saturating_sub(CHROME)
        }
    }

    /// Below this, prose has to give way to abbreviations.
    fn cramped(&self) -> bool {
        self.inner_width() < 18
    }

    fn separator(&self) -> &'static str {
        if self.plain { "  |  " } else { "  ·  " }
    }
}

/// Flatten the scene's layers into a character grid, back to front.
fn composite(scene: &SceneModel) -> Vec<Vec<(char, Option<Style>)>> {
    let mut grid = vec![vec![(' ', None); scene.width]; scene.height];
    for layer in scene.layers() {
        for sprite in layer {
            for (dy, art_line) in sprite.art.iter().enumerate() {
                let y = sprite.y + dy as i32;
                if y < 0 || y as usize >= scene.height {
                    continue;
                }
                for (dx, ch) in art_line.chars().enumerate() {
                    if ch == ' ' {
                        continue;
                    }
                    let x = sprite.x + dx as i32;
                    if x < 0 || x as usize >= scene.width {
                        continue;
                    }
                    grid[y as usize][x as usize] = (ch, Some(sprite.style));
                }
            }
        }
    }
    grid
}

/// The scene as styled lines, each exactly `scene.width` columns wide.
pub fn scene_lines(scene: &SceneModel) -> Vec<Line> {
    composite(scene)
        .into_iter()
        .map(|row| {
            let mut spans: Line = Vec::new();
            for (ch, style) in row {
                let paint = style.map(Paint::Scene).unwrap_or(Paint::None);
                match spans.last_mut() {
                    Some(last) if last.paint == paint => last.text.push(ch),
                    _ => spans.push(Span::new(ch.to_string(), paint)),
                }
            }
            spans
        })
        .collect()
}

fn text_width(line: &Line) -> usize {
    line.iter().map(|s| s.text.chars().count()).sum()
}

fn centered(line: Line, width: usize) -> Line {
    let pad = width.saturating_sub(text_width(&line)) / 2;
    if pad == 0 {
        return line;
    }
    let mut out = vec![Span::plain(" ".repeat(pad))];
    out.extend(line);
    out
}

fn caption(weather: &WeatherData, width: usize) -> Vec<Line> {
    vec![
        centered(
            vec![Span::new(
                format!("{:.0}{}", weather.temperature, weather.units.temp_symbol()),
                Paint::Temperature,
            )],
            width,
        ),
        centered(
            vec![Span::new(weather.location.label(), Paint::Place)],
            width,
        ),
        centered(
            vec![Span::new(weather.condition.label(), Paint::Condition)],
            width,
        ),
    ]
}

/// `Feels like 33°C`, `Humidity 87%`, … packed into as few centered rows as fit.
fn details(weather: &WeatherData, layout: &Layout) -> Vec<Line> {
    let width = layout.inner_width();
    let temp = weather.units.temp_symbol();
    let mut items = if layout.cramped() {
        vec![
            format!("Feels {:.0}{temp}", weather.feels_like),
            format!("Hum {}%", weather.humidity),
            format!(
                "Wind {:.0} {}",
                weather.wind_speed,
                weather.units.wind_symbol()
            ),
        ]
    } else {
        vec![
            format!("Feels like {:.0}{temp}", weather.feels_like),
            format!("Humidity {}%", weather.humidity),
            format!(
                "Wind {:.0} {} {}",
                weather.wind_speed,
                weather.units.wind_symbol(),
                weather.wind_cardinal()
            ),
        ]
    };
    if let Some(chance) = weather.precipitation_probability {
        let label = if weather.condition.is_snowy() {
            "Snow"
        } else {
            "Rain"
        };
        items.push(format!("{label} {chance}%"));
    }

    let sep = layout.separator();
    let mut rows: Vec<String> = Vec::new();
    for item in items {
        match rows.last_mut() {
            Some(row)
                if row.chars().count() + sep.chars().count() + item.chars().count() <= width =>
            {
                row.push_str(sep);
                row.push_str(&item);
            }
            _ => rows.push(item),
        }
    }
    rows.into_iter()
        .map(|row| centered(vec![Span::new(row, Paint::Detail)], width))
        .collect()
}

/// Wrap a body in the rounded box, or return it untouched in plain mode.
pub fn framed(body: Vec<Line>, layout: &Layout) -> Vec<Line> {
    if layout.plain {
        return body;
    }
    let mut out = vec![rule('╭', '╮', layout)];
    out.push(frame_line(blank(), layout));
    out.extend(body.into_iter().map(|l| frame_line(l, layout)));
    out.push(frame_line(blank(), layout));
    out.push(rule('╰', '╯', layout));
    out
}

/// Hard guarantee that no line can push past the frame.
fn truncate(line: Line, max: usize) -> Line {
    if text_width(&line) <= max {
        return line;
    }
    let mut out: Line = Vec::new();
    let mut used = 0;
    for span in line {
        let room = max - used;
        let len = span.text.chars().count();
        if len <= room {
            used += len;
            out.push(span);
        } else {
            out.push(Span::new(
                span.text.chars().take(room).collect::<String>(),
                span.paint,
            ));
            break;
        }
    }
    out
}

fn frame_line(content: Line, layout: &Layout) -> Line {
    if layout.plain {
        return content;
    }
    let inner = layout.inner_width();
    let content = truncate(content, inner);
    let pad = inner.saturating_sub(text_width(&content));
    let mut out = vec![Span::new("│", Paint::Frame), Span::plain("  ")];
    out.extend(content);
    out.push(Span::plain(" ".repeat(pad + 2)));
    out.push(Span::new("│", Paint::Frame));
    out
}

fn rule(left: char, right: char, layout: &Layout) -> Line {
    vec![Span::new(
        format!(
            "{left}{}{right}",
            "─".repeat(layout.width.saturating_sub(2))
        ),
        Paint::Frame,
    )]
}

fn blank() -> Line {
    Vec::new()
}

/// The full default output: scene, caption, details, optionally framed.
pub fn panel(weather: &WeatherData, scene: &SceneModel, layout: &Layout) -> Vec<Line> {
    let inner = layout.inner_width();
    let mut body: Vec<Line> = Vec::new();
    body.extend(scene_lines(scene));
    body.push(blank());
    body.extend(caption(weather, inner));
    body.push(blank());
    body.extend(details(weather, layout));

    framed(body, layout)
}

/// `☁│  27°C  ·  Chennai  ·  Rain  ·  87%  ·  14 km/h`
pub fn compact_line(weather: &WeatherData, layout: &Layout) -> Line {
    let sep = layout.separator();
    let mut text = String::new();
    if !layout.plain {
        text.push_str(icon(weather.condition, false));
        text.push(' ');
    }
    text.push_str(&format!(
        "{:.0}{}{sep}{}{sep}{}{sep}{}%{sep}{:.0} {}",
        weather.temperature,
        weather.units.temp_symbol(),
        weather.location.name,
        weather.condition.label(),
        weather.humidity,
        weather.wind_speed,
        weather.units.wind_symbol(),
    ));
    vec![Span::plain(text)]
}

/// Two-character condition icon. Legible without emoji; ASCII when plain.
pub fn icon(condition: WeatherCondition, plain: bool) -> &'static str {
    use WeatherCondition::*;
    match (condition, plain) {
        (Clear, false) => "☀ ",
        (PartlyCloudy, false) => "☀☁",
        (Cloudy, false) => "☁☁",
        (Fog, false) => "≡≡",
        (Drizzle, false) => "☁'",
        (Rain, false) => "☁│",
        (HeavyRain, false) => "☁┃",
        (Snow, false) => "☁*",
        (HeavySnow, false) => "☁✻",
        (Thunderstorm, false) => "☁⚡",
        (Unknown, false) => "? ",
        (Clear, true) => "* ",
        (PartlyCloudy, true) => "*~",
        (Cloudy, true) => "~~",
        (Fog, true) => "==",
        (Drizzle, true) => "~'",
        (Rain, true) | (HeavyRain, true) => "~|",
        (Snow, true) | (HeavySnow, true) => "~*",
        (Thunderstorm, true) => "~!",
        (Unknown, true) => "? ",
    }
}

/// `Today   ☀☁   31° / 26°   40%`
pub fn forecast_lines(weather: &WeatherData, layout: &Layout) -> Vec<Line> {
    let inner = layout.inner_width();
    let show_chance = inner >= 34
        && weather
            .forecast
            .iter()
            .any(|d| d.precipitation_probability.is_some());

    let mut rows: Vec<Line> = weather
        .forecast
        .iter()
        .enumerate()
        .map(|(index, day)| {
            let name = if index == 0 {
                "Today".to_string()
            } else {
                weekday(&day.date).unwrap_or("—").to_string()
            };
            let mut row = vec![
                Span::new(format!("{name:<6}"), Paint::None),
                Span::new(
                    format!("{}  ", icon(day.condition, layout.plain)),
                    Paint::Condition,
                ),
                Span::new(format!("{:>3.0}°", day.temp_max), Paint::Temperature),
                Span::new(format!(" / {:.0}°", day.temp_min), Paint::Detail),
            ];
            if show_chance {
                row.push(Span::new(
                    match day.precipitation_probability {
                        Some(chance) => format!("{chance:>5}%"),
                        None => "      ".into(),
                    },
                    Paint::Detail,
                ));
            }
            row
        })
        .collect();

    // Align the whole block as one column, then centre it.
    let widest = rows.iter().map(text_width).max().unwrap_or(0);
    let pad = inner.saturating_sub(widest) / 2;
    if pad > 0 {
        for row in &mut rows {
            row.insert(0, Span::plain(" ".repeat(pad)));
        }
    }

    let mut out = vec![
        centered(
            vec![Span::new(
                format!(
                    "{}  ·  {}",
                    weather.location.label().to_uppercase(),
                    weather.units.temp_symbol()
                ),
                Paint::Place,
            )],
            inner,
        ),
        blank(),
    ];
    out.extend(rows);
    out
}

/// Weekday abbreviation for a `YYYY-MM-DD` date (Sakamoto's algorithm).
fn weekday(date: &str) -> Option<&'static str> {
    const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut parts = date.split('-');
    let mut year: i32 = parts.next()?.parse().ok()?;
    let month: usize = parts.next()?.parse().ok()?;
    let day: i32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    if month < 3 {
        year -= 1;
    }
    let index =
        (year + year / 4 - year / 100 + year / 400 + OFFSETS[month - 1] + day).rem_euclid(7);
    Some(NAMES[index as usize])
}

/// The stale-cache / degraded-mode banner.
pub fn notice(message: &str, layout: &Layout) -> Line {
    let mark = if layout.plain { "!" } else { "⚠" };
    vec![Span::new(format!("{mark} {message}"), Paint::Notice)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::color;
    use crate::scene::generator::{RenderOptions, generate};
    use crate::weather::{Intensity, Location, Units};

    fn sample(condition: WeatherCondition) -> WeatherData {
        WeatherData {
            location: Location {
                name: "Chennai".into(),
                country: "India".into(),
                latitude: 13.0,
                longitude: 80.0,
            },
            observed_at: "2026-08-20T14:15".into(),
            temperature: 27.3,
            feels_like: 32.7,
            humidity: 87,
            wind_speed: 14.0,
            wind_direction: 225,
            cloud_cover: 92,
            precipitation: 1.4,
            precipitation_probability: Some(72),
            condition,
            intensity: Intensity::Moderate,
            sunrise: Some("2026-08-20T05:56".into()),
            sunset: Some("2026-08-20T18:22".into()),
            units: Units::Metric,
            forecast: vec![],
        }
    }

    fn rendered(width: usize, plain: bool) -> String {
        let weather = sample(WeatherCondition::Rain);
        let layout = Layout::new(width, plain);
        let scene = generate(
            &weather,
            &RenderOptions {
                width: layout.inner_width(),
                height: None,
                seed: 42,
                frame: 0,
                ascii: plain,
            },
        );
        color::to_string(&panel(&weather, &scene, &layout), false, false)
    }

    #[test]
    fn never_overflows_the_requested_width() {
        for width in [20usize, 24, 32, 40, 52, 80, 120] {
            for plain in [false, true] {
                let out = rendered(width, plain);
                for line in out.lines() {
                    assert!(
                        line.chars().count() <= width,
                        "width {width} plain {plain}: {} cols in {line:?}",
                        line.chars().count()
                    );
                }
            }
        }
    }

    #[test]
    fn framed_output_is_closed_on_all_sides() {
        let out = rendered(52, false);
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with('╭') && lines[0].ends_with('╮'));
        assert!(lines.last().unwrap().starts_with('╰'));
        assert!(
            lines[1..lines.len() - 1]
                .iter()
                .all(|l| l.starts_with('│') && l.ends_with('│'))
        );
    }

    #[test]
    fn plain_output_has_no_frame_and_no_ansi() {
        let out = rendered(52, true);
        assert!(!out.contains('│') && !out.contains('╭'));
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn caption_carries_the_facts() {
        let out = rendered(52, true);
        assert!(out.contains("27°C"));
        assert!(out.contains("Chennai, India"));
        assert!(out.contains("Rain"));
        assert!(out.contains("Feels like 33°C"));
        assert!(out.contains("Humidity 87%"));
        assert!(out.contains("Wind 14 km/h SW"));
    }

    #[test]
    fn compact_is_exactly_one_line() {
        let weather = sample(WeatherCondition::Rain);
        let layout = Layout::new(80, false);
        let out = color::to_string(&[compact_line(&weather, &layout)], false, false);
        assert_eq!(out.lines().count(), 1);
        assert!(out.contains("27°C") && out.contains("Chennai") && out.contains("14 km/h"));
    }

    #[test]
    fn plain_compact_stays_ascii_apart_from_the_degree_sign() {
        let weather = sample(WeatherCondition::Thunderstorm);
        let layout = Layout::new(80, true);
        let out = color::to_string(&[compact_line(&weather, &layout)], false, false);
        assert!(out.chars().all(|c| c.is_ascii() || c == '°'), "{out}");
    }

    #[test]
    fn weekdays_are_computed_without_a_date_library() {
        assert_eq!(weekday("2026-08-20"), Some("Thu"));
        assert_eq!(weekday("2000-02-29"), Some("Tue"));
        assert_eq!(weekday("1999-12-31"), Some("Fri"));
        assert_eq!(weekday("nonsense"), None);
    }
}
