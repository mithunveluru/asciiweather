//! Normalized weather domain model.
//!
//! Nothing in here knows about HTTP, JSON shapes or provider condition codes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    Metric,
    Imperial,
}

impl Units {
    pub fn temp_symbol(self) -> &'static str {
        match self {
            Units::Metric => "°C",
            Units::Imperial => "°F",
        }
    }

    pub fn wind_symbol(self) -> &'static str {
        match self {
            Units::Metric => "km/h",
            Units::Imperial => "mph",
        }
    }
}

impl std::str::FromStr for Units {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "metric" | "c" | "celsius" => Ok(Units::Metric),
            "imperial" | "f" | "fahrenheit" => Ok(Units::Imperial),
            other => Err(format!(
                "unknown units `{other}` (expected metric or imperial)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
}

impl Location {
    /// `Chennai, India` — or just the name when the country is unknown.
    pub fn label(&self) -> String {
        if self.country.is_empty() {
            self.name.clone()
        } else {
            format!("{}, {}", self.name, self.country)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherCondition {
    Clear,
    PartlyCloudy,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    HeavyRain,
    Snow,
    HeavySnow,
    Thunderstorm,
    Unknown,
}

impl WeatherCondition {
    /// Human label used in the scene caption.
    pub fn label(self) -> &'static str {
        match self {
            WeatherCondition::Clear => "Clear",
            WeatherCondition::PartlyCloudy => "Partly Cloudy",
            WeatherCondition::Cloudy => "Cloudy",
            WeatherCondition::Fog => "Fog",
            WeatherCondition::Drizzle => "Drizzle",
            WeatherCondition::Rain => "Rain",
            WeatherCondition::HeavyRain => "Heavy Rain",
            WeatherCondition::Snow => "Snow",
            WeatherCondition::HeavySnow => "Heavy Snow",
            WeatherCondition::Thunderstorm => "Thunderstorm",
            WeatherCondition::Unknown => "Unknown",
        }
    }

    pub fn is_rainy(self) -> bool {
        matches!(
            self,
            WeatherCondition::Drizzle
                | WeatherCondition::Rain
                | WeatherCondition::HeavyRain
                | WeatherCondition::Thunderstorm
        )
    }

    pub fn is_snowy(self) -> bool {
        matches!(self, WeatherCondition::Snow | WeatherCondition::HeavySnow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Intensity {
    Light,
    Moderate,
    Heavy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyForecast {
    /// Local calendar date, `YYYY-MM-DD`.
    pub date: String,
    pub condition: WeatherCondition,
    pub temp_max: f64,
    pub temp_min: f64,
    pub precipitation_probability: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherData {
    pub location: Location,
    /// Local observation time, `YYYY-MM-DDTHH:MM`.
    pub observed_at: String,
    pub temperature: f64,
    pub feels_like: f64,
    pub humidity: u8,
    pub wind_speed: f64,
    pub wind_direction: u16,
    pub cloud_cover: u8,
    pub precipitation: f64,
    pub precipitation_probability: Option<u8>,
    pub condition: WeatherCondition,
    pub intensity: Intensity,
    /// Local sunrise/sunset for the observation day, `YYYY-MM-DDTHH:MM`.
    pub sunrise: Option<String>,
    pub sunset: Option<String>,
    pub units: Units,
    #[serde(default)]
    pub forecast: Vec<DailyForecast>,
}

impl WeatherData {
    /// Day/night comes from sunrise/sunset, never from the host clock.
    ///
    /// Both timestamps are local ISO-8601 with the same shape, so a lexicographic
    /// compare is a correct chronological compare — no date library needed.
    pub fn is_night(&self) -> bool {
        match (&self.sunrise, &self.sunset) {
            (Some(rise), Some(set)) if same_day(&self.observed_at, rise) => {
                self.observed_at < *rise || self.observed_at >= *set
            }
            // ponytail: no usable sun times (polar day/night, odd provider data)
            // -> assume day rather than inventing a clock-based guess.
            _ => false,
        }
    }

    pub fn wind_cardinal(&self) -> &'static str {
        const POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
        POINTS[(((self.wind_direction as f64 + 22.5) % 360.0) / 45.0) as usize % 8]
    }
}

fn same_day(a: &str, b: &str) -> bool {
    a.len() >= 10 && b.len() >= 10 && a[..10] == b[..10]
}
