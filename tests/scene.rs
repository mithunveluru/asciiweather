//! The rendering pipeline, end to end, without a network, a terminal or a clock.
//!
//! `WeatherData -> SceneGenerator -> SceneModel -> lines`

use asciiweather::render::ascii::{Layout, panel, scene_lines};
use asciiweather::render::color;
use asciiweather::scene::{RenderOptions, SceneModel, Style, default_seed, generate};
use asciiweather::weather::*;

fn weather(condition: WeatherCondition, intensity: Intensity, night: bool) -> WeatherData {
    WeatherData {
        location: Location {
            name: "Chennai".into(),
            country: "India".into(),
            latitude: 13.08784,
            longitude: 80.27847,
        },
        observed_at: if night {
            "2026-08-20T22:10"
        } else {
            "2026-08-20T14:15"
        }
        .into(),
        temperature: 27.3,
        feels_like: 32.7,
        humidity: 87,
        wind_speed: 9.0,
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

fn scene(
    condition: WeatherCondition,
    intensity: Intensity,
    night: bool,
    width: usize,
) -> SceneModel {
    generate(
        &weather(condition, intensity, night),
        &RenderOptions::new(width, 42),
    )
}

fn styles(scene: &SceneModel) -> Vec<Style> {
    scene
        .layers()
        .iter()
        .flat_map(|layer| layer.iter().map(|s| s.style))
        .collect()
}

fn text(scene: &SceneModel) -> String {
    color::to_string(&scene_lines(scene), false, scene.night)
}

const ALL: [(WeatherCondition, Intensity); 11] = {
    use Intensity::*;
    use WeatherCondition::*;
    [
        (Clear, Light),
        (PartlyCloudy, Light),
        (Cloudy, Moderate),
        (Fog, Moderate),
        (Drizzle, Light),
        (Rain, Moderate),
        (HeavyRain, Heavy),
        (Snow, Moderate),
        (HeavySnow, Heavy),
        (Thunderstorm, Heavy),
        (Unknown, Moderate),
    ]
};

#[test]
fn every_condition_draws_something_inside_its_canvas() {
    for (condition, intensity) in ALL {
        for night in [false, true] {
            let scene = scene(condition, intensity, night, 46);
            assert!(
                scene.sprite_count() > 0,
                "{condition:?} night={night} produced an empty scene"
            );
            for layer in scene.layers() {
                for sprite in layer {
                    let (x0, y0, x1, y1) = sprite.bbox();
                    assert!(
                        x0 >= 0
                            && y0 >= 0
                            && x1 <= scene.width as i32 + 1
                            && y1 <= scene.height as i32,
                        "{condition:?} sprite {sprite:?} escapes {}x{}",
                        scene.width,
                        scene.height
                    );
                }
            }
        }
    }
}

#[test]
fn each_condition_uses_the_right_vocabulary() {
    use WeatherCondition::*;
    let has = |c, i, n, style| styles(&scene(c, i, n, 46)).contains(&style);

    assert!(has(Clear, Intensity::Light, false, Style::Sun));
    assert!(has(Clear, Intensity::Light, true, Style::Moon));
    assert!(has(PartlyCloudy, Intensity::Light, false, Style::Cloud));
    assert!(has(Cloudy, Intensity::Moderate, false, Style::Cloud));
    assert!(!has(Cloudy, Intensity::Moderate, false, Style::Sun));
    assert!(has(Fog, Intensity::Moderate, false, Style::Fog));
    assert!(has(Rain, Intensity::Moderate, false, Style::Rain));
    assert!(has(HeavyRain, Intensity::Heavy, false, Style::Rain));
    assert!(has(Snow, Intensity::Moderate, false, Style::Snow));
    assert!(has(HeavySnow, Intensity::Heavy, false, Style::Snow));
    assert!(has(Thunderstorm, Intensity::Heavy, false, Style::Bolt));
    assert!(!has(Rain, Intensity::Moderate, false, Style::Snow));
    assert!(!has(Snow, Intensity::Moderate, false, Style::Rain));
    assert!(!has(Clear, Intensity::Light, false, Style::Rain));
}

#[test]
fn night_never_renders_the_same_as_day() {
    for (condition, intensity) in ALL {
        assert_ne!(
            text(&scene(condition, intensity, false, 46)),
            text(&scene(condition, intensity, true, 46)),
            "{condition:?} looks identical by day and by night"
        );
    }
}

#[test]
fn night_swaps_the_sun_for_a_moon_and_adds_stars() {
    let night = scene(WeatherCondition::Clear, Intensity::Light, true, 46);
    assert!(night.night);
    assert!(styles(&night).contains(&Style::Star));
    assert!(!styles(&night).contains(&Style::Sun));
}

#[test]
fn intensity_drives_precipitation_density() {
    let count = |intensity| {
        scene(WeatherCondition::Rain, intensity, false, 46)
            .precipitation
            .len()
    };
    assert!(count(Intensity::Light) < count(Intensity::Moderate));
    assert!(count(Intensity::Moderate) < count(Intensity::Heavy));
}

#[test]
fn wind_slants_the_rain_and_only_shows_up_when_it_blows() {
    let mut calm = weather(WeatherCondition::Rain, Intensity::Moderate, false);
    calm.wind_speed = 3.0;
    let mut gale = calm.clone();
    gale.wind_speed = 60.0;

    let calm_scene = generate(&calm, &RenderOptions::new(46, 42));
    let gale_scene = generate(&gale, &RenderOptions::new(46, 42));

    assert!(calm_scene.wind.is_empty(), "a breeze should not be drawn");
    assert!(!gale_scene.wind.is_empty(), "a gale should be drawn");
    assert!(text(&calm_scene).contains('│'));
    assert!(
        text(&gale_scene).contains('╱'),
        "wind should slant the rain"
    );

    // Wind direction is where it comes *from*: a westerly blows to the right.
    let mut easterly = gale.clone();
    easterly.wind_direction = 90;
    assert!(text(&generate(&easterly, &RenderOptions::new(46, 42))).contains('╲'));
}

#[test]
fn same_seed_same_scene_different_seed_different_scene() {
    let data = weather(WeatherCondition::Rain, Intensity::Moderate, false);
    let at = |seed| generate(&data, &RenderOptions::new(46, seed));

    assert_eq!(at(42), at(42));
    assert_eq!(text(&at(42)), text(&at(42)));
    assert_ne!(text(&at(42)), text(&at(43)));

    // …and the default seed is stable for a given observation.
    assert_eq!(default_seed(&data), default_seed(&data.clone()));
    let mut later = data.clone();
    later.observed_at = "2026-08-20T14:30".into();
    assert_ne!(default_seed(&data), default_seed(&later));
}

#[test]
fn animation_frames_move_without_reshaping_the_sky() {
    let data = weather(WeatherCondition::Rain, Intensity::Moderate, false);
    let frame = |n| {
        generate(
            &data,
            &RenderOptions {
                frame: n,
                ..RenderOptions::new(46, 42)
            },
        )
    };
    assert_eq!(frame(0).clouds, frame(5).clouds, "clouds must not jitter");
    assert_ne!(frame(0).precipitation, frame(5).precipitation);
    assert_ne!(
        frame(0).clouds,
        frame(30).clouds,
        "clouds should drift slowly"
    );
    assert_eq!(frame(3), frame(3));
}

#[test]
fn narrow_terminals_simplify_rather_than_overflow() {
    let tiny = scene(WeatherCondition::Rain, Intensity::Moderate, false, 20);
    let roomy = scene(WeatherCondition::Rain, Intensity::Moderate, false, 90);

    assert!(tiny.width <= 20 && roomy.width == 90);
    assert!(tiny.sprite_count() < roomy.sprite_count());
    assert!(tiny.foreground.is_empty(), "compact drops decoration");
    assert!(!roomy.foreground.is_empty());
    for line in text(&tiny).lines() {
        assert!(line.chars().count() <= 20);
    }
}

#[test]
fn plain_mode_produces_only_ascii() {
    for (condition, intensity) in ALL {
        for night in [false, true] {
            let scene = generate(
                &weather(condition, intensity, night),
                &RenderOptions {
                    ascii: true,
                    ..RenderOptions::new(46, 42)
                },
            );
            let out = color::to_string(
                &panel(
                    &weather(condition, intensity, night),
                    &scene,
                    &Layout::new(52, true),
                ),
                false,
                night,
            );
            assert!(
                out.chars().all(|c| c.is_ascii() || c == '°'),
                "{condition:?} night={night} leaked non-ASCII:\n{out}"
            );
        }
    }
}

/// One golden file, on purpose. It catches accidental changes to the visual
/// language; the assertions above cover behaviour.
#[test]
fn rain_at_seed_42_is_stable() {
    let data = weather(WeatherCondition::Rain, Intensity::Moderate, false);
    let layout = Layout::new(52, false);
    let scene = generate(&data, &RenderOptions::new(layout.inner_width(), 42));
    let actual = color::to_string(&panel(&data, &scene, &layout), false, false);

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rain_seed42.txt"
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(path).expect("golden file");
    assert_eq!(
        actual, expected,
        "scene changed; re-record with UPDATE_GOLDEN=1 cargo test"
    );
}
