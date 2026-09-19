//! Open-Meteo implementation of [`WeatherProvider`].
//!
//! This is the only file in the project that knows Open-Meteo URLs, JSON field
//! names or WMO weather codes.

use std::time::Duration;

use serde::Deserialize;

use crate::error::{AppError, Result};
use crate::weather::model::{
    DailyForecast, Intensity, Location, Units, WeatherCondition, WeatherData,
};
use crate::weather::provider::WeatherProvider;

const GEOCODE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
const FORECAST_DAYS: usize = 5;

pub struct OpenMeteo {
    agent: ureq::Agent,
}

impl OpenMeteo {
    pub fn new(timeout: Duration) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .user_agent(concat!("asciiweather/", env!("CARGO_PKG_VERSION")))
            .build();
        OpenMeteo {
            agent: config.into(),
        }
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let mut response = self.agent.get(url).call().map_err(classify)?;
        response
            .body_mut()
            .read_json::<T>()
            .map_err(|e| AppError::Service(format!("malformed response: {e}")))
    }
}

impl Default for OpenMeteo {
    fn default() -> Self {
        Self::new(Duration::from_secs(8))
    }
}

fn classify(err: ureq::Error) -> AppError {
    match err {
        ureq::Error::StatusCode(code) => {
            AppError::Service(format!("weather service returned HTTP {code}"))
        }
        other => AppError::Network(other.to_string()),
    }
}

/// Minimal percent-encoding for a query-string value.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

impl WeatherProvider for OpenMeteo {
    fn geocode(&self, query: &str) -> Result<Option<Location>> {
        let url = format!(
            "{GEOCODE_URL}?name={}&count=1&language=en&format=json",
            encode(query.trim())
        );
        let body: GeocodeResponse = self.get_json(&url)?;
        Ok(body.results.into_iter().next().map(|hit| Location {
            name: hit.name,
            country: hit.country.unwrap_or_default(),
            latitude: hit.latitude,
            longitude: hit.longitude,
        }))
    }

    fn fetch(&self, location: &Location, units: Units) -> Result<WeatherData> {
        let unit_params = match units {
            Units::Metric => "",
            Units::Imperial => {
                "&temperature_unit=fahrenheit&wind_speed_unit=mph&precipitation_unit=inch"
            }
        };
        let url = format!(
            "{FORECAST_URL}?latitude={:.4}&longitude={:.4}\
             &current=temperature_2m,relative_humidity_2m,apparent_temperature,precipitation,\
             weather_code,cloud_cover,wind_speed_10m,wind_direction_10m\
             &hourly=precipitation_probability\
             &daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,\
             precipitation_probability_max\
             &forecast_days={FORECAST_DAYS}&timezone=auto{unit_params}",
            location.latitude, location.longitude
        );
        let body: ForecastResponse = self.get_json(&url)?;
        Ok(normalize(body, location.clone(), units))
    }
}

fn normalize(body: ForecastResponse, location: Location, units: Units) -> WeatherData {
    let current = body.current;
    let (condition, intensity) = decode_wmo(current.weather_code);

    // The hourly series is aligned to whole hours; the current reading is not.
    let precipitation_probability = body.hourly.and_then(|hourly| {
        let hour = current.time.get(..13)?;
        let index = hourly.time.iter().position(|t| t.starts_with(hour))?;
        hourly
            .precipitation_probability
            .get(index)
            .copied()
            .flatten()
    });

    let daily = body.daily.unwrap_or_default();
    let today = daily
        .time
        .first()
        .filter(|d| current.time.starts_with(d.as_str()));

    WeatherData {
        observed_at: current.time,
        temperature: current.temperature_2m,
        feels_like: current.apparent_temperature,
        humidity: current.relative_humidity_2m,
        wind_speed: current.wind_speed_10m,
        wind_direction: current.wind_direction_10m.round() as u16 % 360,
        cloud_cover: current.cloud_cover,
        precipitation: current.precipitation,
        precipitation_probability,
        condition,
        intensity,
        sunrise: today.and(daily.sunrise.first().cloned()),
        sunset: today.and(daily.sunset.first().cloned()),
        units,
        forecast: build_forecast(&daily),
        location,
    }
}

fn build_forecast(daily: &Daily) -> Vec<DailyForecast> {
    (0..daily.time.len())
        .filter_map(|i| {
            Some(DailyForecast {
                date: daily.time.get(i)?.clone(),
                condition: decode_wmo(*daily.weather_code.get(i)?).0,
                temp_max: *daily.temperature_2m_max.get(i)?,
                temp_min: *daily.temperature_2m_min.get(i)?,
                precipitation_probability: daily
                    .precipitation_probability_max
                    .get(i)
                    .copied()
                    .flatten(),
            })
        })
        .collect()
}

/// WMO 4677 weather code -> our condition vocabulary.
fn decode_wmo(code: u16) -> (WeatherCondition, Intensity) {
    use Intensity::*;
    use WeatherCondition::*;
    match code {
        0 | 1 => (Clear, Light),
        2 => (PartlyCloudy, Light),
        3 => (Cloudy, Moderate),
        45 | 48 => (Fog, Moderate),
        51 | 56 => (Drizzle, Light),
        53 => (Drizzle, Moderate),
        55 | 57 => (Drizzle, Heavy),
        61 | 66 | 80 => (Rain, Light),
        63 => (Rain, Moderate),
        81 => (Rain, Heavy),
        65 | 67 | 82 => (HeavyRain, Heavy),
        71 | 77 | 85 => (Snow, Light),
        73 => (Snow, Moderate),
        75 | 86 => (HeavySnow, Heavy),
        95 => (Thunderstorm, Moderate),
        96 | 99 => (Thunderstorm, Heavy),
        _ => (Unknown, Moderate),
    }
}

// --- wire types -------------------------------------------------------------

#[derive(Deserialize)]
struct GeocodeResponse {
    #[serde(default)]
    results: Vec<GeocodeHit>,
}

#[derive(Deserialize)]
struct GeocodeHit {
    name: String,
    country: Option<String>,
    latitude: f64,
    longitude: f64,
}

#[derive(Deserialize)]
struct ForecastResponse {
    current: Current,
    hourly: Option<Hourly>,
    daily: Option<Daily>,
}

#[derive(Deserialize)]
struct Current {
    time: String,
    temperature_2m: f64,
    relative_humidity_2m: u8,
    apparent_temperature: f64,
    precipitation: f64,
    weather_code: u16,
    cloud_cover: u8,
    wind_speed_10m: f64,
    wind_direction_10m: f64,
}

#[derive(Deserialize)]
struct Hourly {
    time: Vec<String>,
    precipitation_probability: Vec<Option<u8>>,
}

#[derive(Deserialize, Default)]
struct Daily {
    #[serde(default)]
    time: Vec<String>,
    #[serde(default)]
    weather_code: Vec<u16>,
    #[serde(default)]
    temperature_2m_max: Vec<f64>,
    #[serde(default)]
    temperature_2m_min: Vec<f64>,
    #[serde(default)]
    sunrise: Vec<String>,
    #[serde(default)]
    sunset: Vec<String>,
    #[serde(default)]
    precipitation_probability_max: Vec<Option<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("fixture")
    }

    #[test]
    fn parses_geocoding_response() {
        let body: GeocodeResponse = serde_json::from_str(&fixture("geocode_chennai.json")).unwrap();
        let hit = &body.results[0];
        assert_eq!(hit.name, "Chennai");
        assert_eq!(hit.country.as_deref(), Some("India"));
        assert!((hit.latitude - 13.08784).abs() < 1e-5);
    }

    #[test]
    fn geocoding_miss_yields_no_results() {
        let body: GeocodeResponse = serde_json::from_str(r#"{"generationtime_ms":0.1}"#).unwrap();
        assert!(body.results.is_empty());
    }

    #[test]
    fn normalizes_forecast_response() {
        let body: ForecastResponse = serde_json::from_str(&fixture("forecast_rain.json")).unwrap();
        let location = Location {
            name: "Chennai".into(),
            country: "India".into(),
            latitude: 13.08784,
            longitude: 80.27847,
        };
        let data = normalize(body, location, Units::Metric);

        assert_eq!(data.condition, WeatherCondition::Rain);
        assert_eq!(data.intensity, Intensity::Moderate);
        assert_eq!(data.humidity, 87);
        assert_eq!(data.precipitation_probability, Some(72));
        assert_eq!(data.sunrise.as_deref(), Some("2026-08-20T05:56"));
        assert_eq!(data.forecast.len(), 3);
        assert_eq!(data.forecast[0].precipitation_probability, Some(85));
        assert_eq!(data.wind_cardinal(), "SW");
        assert!(!data.is_night());
    }

    #[test]
    fn wmo_codes_map_to_conditions_and_intensity() {
        assert_eq!(decode_wmo(0).0, WeatherCondition::Clear);
        assert_eq!(decode_wmo(3).0, WeatherCondition::Cloudy);
        assert_eq!(decode_wmo(45).0, WeatherCondition::Fog);
        assert_eq!(
            decode_wmo(65),
            (WeatherCondition::HeavyRain, Intensity::Heavy)
        );
        assert_eq!(
            decode_wmo(75),
            (WeatherCondition::HeavySnow, Intensity::Heavy)
        );
        assert_eq!(decode_wmo(95).0, WeatherCondition::Thunderstorm);
        assert_eq!(decode_wmo(1234).0, WeatherCondition::Unknown);
    }

    #[test]
    fn encodes_query_values() {
        assert_eq!(encode("New York"), "New+York");
        assert_eq!(encode("São Paulo"), "S%C3%A3o+Paulo");
    }
}
