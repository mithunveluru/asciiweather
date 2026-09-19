//! Renders every condition, day and night, from fixed weather data.
//!
//! Used to eyeball the art and to produce the samples in the README:
//!     cargo run --example gallery -- 52
use asciiweather::render::ascii::{Layout, panel};
use asciiweather::render::color;
use asciiweather::scene::{RenderOptions, generate};
use asciiweather::weather::*;

fn sample(condition: WeatherCondition, intensity: Intensity, night: bool) -> WeatherData {
    WeatherData {
        location: Location {
            name: "Chennai".into(),
            country: "India".into(),
            latitude: 13.0,
            longitude: 80.0,
        },
        observed_at: if night {
            "2026-08-20T22:10"
        } else {
            "2026-08-20T14:15"
        }
        .into(),
        temperature: if night { 24.0 } else { 31.0 },
        feels_like: 33.0,
        humidity: 78,
        wind_speed: 14.0,
        wind_direction: 225,
        cloud_cover: match condition {
            WeatherCondition::Clear => 5,
            WeatherCondition::PartlyCloudy => 40,
            _ => 92,
        },
        precipitation: 1.4,
        precipitation_probability: Some(72),
        condition,
        intensity,
        sunrise: Some("2026-08-20T05:56".into()),
        sunset: Some("2026-08-20T18:22".into()),
        units: Units::Metric,
        forecast: vec![],
    }
}

fn main() {
    let width: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(52);
    let cases = [
        (WeatherCondition::Clear, Intensity::Light),
        (WeatherCondition::PartlyCloudy, Intensity::Light),
        (WeatherCondition::Cloudy, Intensity::Moderate),
        (WeatherCondition::Fog, Intensity::Moderate),
        (WeatherCondition::Drizzle, Intensity::Light),
        (WeatherCondition::Rain, Intensity::Moderate),
        (WeatherCondition::HeavyRain, Intensity::Heavy),
        (WeatherCondition::Snow, Intensity::Moderate),
        (WeatherCondition::HeavySnow, Intensity::Heavy),
        (WeatherCondition::Thunderstorm, Intensity::Heavy),
    ];
    let layout = Layout::new(width, false);
    for (condition, intensity) in cases {
        for night in [false, true] {
            let weather = sample(condition, intensity, night);
            let scene = generate(&weather, &RenderOptions::new(layout.inner_width(), 42));
            println!(
                "### {} {}",
                condition.label(),
                if night { "night" } else { "day" }
            );
            print!(
                "{}",
                color::to_string(&panel(&weather, &scene, &layout), false, night)
            );
            println!();
        }
    }
}
