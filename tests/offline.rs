//! Cache and provider-failure behaviour, exercised through a fake provider so
//! the suite never touches the network.

use std::cell::Cell;

use asciiweather::error::{AppError, Result};
use asciiweather::weather::cache::Cache;
use asciiweather::weather::*;

struct FakeProvider {
    outcome: Box<dyn Fn() -> Result<WeatherData>>,
    calls: Cell<usize>,
}

impl FakeProvider {
    fn ok(temperature: f64) -> Self {
        FakeProvider {
            outcome: Box::new(move || Ok(sample(temperature))),
            calls: Cell::new(0),
        }
    }

    fn failing(err: fn() -> AppError) -> Self {
        FakeProvider {
            outcome: Box::new(move || Err(err())),
            calls: Cell::new(0),
        }
    }
}

impl WeatherProvider for FakeProvider {
    fn geocode(&self, query: &str) -> Result<Option<Location>> {
        self.calls.set(self.calls.get() + 1);
        if query == "Nowhereville" {
            return Ok(None);
        }
        match (self.outcome)() {
            Ok(data) => Ok(Some(data.location)),
            Err(err) => Err(err),
        }
    }

    fn fetch(&self, _location: &Location, _units: Units) -> Result<WeatherData> {
        (self.outcome)()
    }
}

fn sample(temperature: f64) -> WeatherData {
    WeatherData {
        location: Location {
            name: "Chennai".into(),
            country: "India".into(),
            latitude: 13.08784,
            longitude: 80.27847,
        },
        observed_at: "2026-08-20T14:15".into(),
        temperature,
        feels_like: 32.7,
        humidity: 87,
        wind_speed: 14.0,
        wind_direction: 225,
        cloud_cover: 92,
        precipitation: 1.4,
        precipitation_probability: Some(72),
        condition: WeatherCondition::Rain,
        intensity: Intensity::Moderate,
        sunrise: Some("2026-08-20T05:56".into()),
        sunset: Some("2026-08-20T18:22".into()),
        units: Units::Metric,
        forecast: vec![],
    }
}

/// A cache directory of our own, removed when the test ends.
struct TempCache(std::path::PathBuf);

impl TempCache {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("asciiweather-it-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        TempCache(dir)
    }

    fn cache(&self) -> Cache {
        Cache::new(self.0.clone())
    }
}

impl Drop for TempCache {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const TTL: u64 = 600;

#[test]
fn a_fresh_cache_entry_skips_the_network_entirely() {
    let temp = TempCache::new("fresh");
    let cache = temp.cache();
    let provider = FakeProvider::ok(27.3);

    let first = resolve(
        &provider,
        &cache,
        "Chennai",
        Units::Metric,
        TTL,
        1_000,
        true,
    )
    .unwrap();
    assert_eq!(first.data.temperature, 27.3);
    assert!(first.notice.is_none());
    assert_eq!(provider.calls.get(), 1);

    // 5 minutes later, still inside the TTL.
    let second = resolve(
        &provider,
        &cache,
        "Chennai",
        Units::Metric,
        TTL,
        1_300,
        true,
    )
    .unwrap();
    assert_eq!(second.data.temperature, 27.3);
    assert_eq!(
        provider.calls.get(),
        1,
        "cache hit must not call the provider"
    );
}

#[test]
fn an_expired_entry_is_refreshed() {
    let temp = TempCache::new("expired");
    let cache = temp.cache();

    let warm = FakeProvider::ok(27.3);
    resolve(&warm, &cache, "Chennai", Units::Metric, TTL, 1_000, true).unwrap();

    let fresh = FakeProvider::ok(31.0);
    let reading = resolve(&fresh, &cache, "Chennai", Units::Metric, TTL, 9_999, true).unwrap();
    assert_eq!(reading.data.temperature, 31.0);
    assert!(reading.notice.is_none());
    assert_eq!(fresh.calls.get(), 1);
}

#[test]
fn no_cache_forces_a_request_even_when_the_entry_is_fresh() {
    let temp = TempCache::new("nocache");
    let cache = temp.cache();
    resolve(
        &FakeProvider::ok(27.3),
        &cache,
        "Chennai",
        Units::Metric,
        TTL,
        1_000,
        true,
    )
    .unwrap();

    let provider = FakeProvider::ok(31.0);
    let reading = resolve(
        &provider,
        &cache,
        "Chennai",
        Units::Metric,
        TTL,
        1_100,
        false,
    )
    .unwrap();
    assert_eq!(reading.data.temperature, 31.0);
    assert_eq!(provider.calls.get(), 1);
}

#[test]
fn a_network_outage_falls_back_to_stale_data_with_a_notice() {
    let temp = TempCache::new("outage");
    let cache = temp.cache();
    resolve(
        &FakeProvider::ok(27.3),
        &cache,
        "Chennai",
        Units::Metric,
        TTL,
        1_000,
        true,
    )
    .unwrap();

    // 7 minutes later the entry has expired (TTL 5 min) and the network is gone.
    let offline = FakeProvider::failing(|| AppError::Network("dns failure".into()));
    let reading = resolve(
        &offline,
        &cache,
        "Chennai",
        Units::Metric,
        300,
        1_000 + 420,
        true,
    )
    .unwrap();

    assert_eq!(reading.data.temperature, 27.3);
    let notice = reading.notice.expect("a stale reading must say so");
    assert!(notice.contains("No network connection."));
    assert!(notice.contains("7 minutes ago"), "{notice}");
}

#[test]
fn an_outage_with_no_cache_at_all_is_an_error() {
    let temp = TempCache::new("cold");
    let offline = FakeProvider::failing(|| AppError::Service("HTTP 503".into()));
    let err = resolve(
        &offline,
        &temp.cache(),
        "Chennai",
        Units::Metric,
        TTL,
        1_000,
        true,
    )
    .unwrap_err();

    assert_eq!(err.exit_code(), 4);
    assert_eq!(
        err.headline(),
        "Weather service is temporarily unavailable."
    );
}

#[test]
fn an_unknown_place_is_never_papered_over_with_stale_data() {
    let temp = TempCache::new("unknown");
    let cache = temp.cache();
    resolve(
        &FakeProvider::ok(27.3),
        &cache,
        "Chennai",
        Units::Metric,
        TTL,
        1_000,
        true,
    )
    .unwrap();

    let err = resolve(
        &FakeProvider::ok(27.3),
        &cache,
        "Nowhereville",
        Units::Metric,
        TTL,
        1_000,
        true,
    )
    .unwrap_err();

    assert_eq!(err.exit_code(), 3);
    assert!(err.headline().contains("Nowhereville"));
    assert!(!err.is_transient());
}

#[test]
fn cached_data_survives_a_round_trip_through_json() {
    let temp = TempCache::new("json");
    let cache = temp.cache();
    let original = resolve(
        &FakeProvider::ok(27.3),
        &cache,
        "Chennai",
        Units::Metric,
        TTL,
        1_000,
        true,
    )
    .unwrap()
    .data;

    let encoded = serde_json::to_string(&original).unwrap();
    let decoded: WeatherData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, original);

    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["condition"], "rain");
    assert_eq!(value["units"], "metric");
    assert_eq!(value["location"]["name"], "Chennai");
    assert_eq!(value["temperature"], 27.3);
}
