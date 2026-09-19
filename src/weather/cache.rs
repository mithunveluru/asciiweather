//! Filesystem cache for normalized weather data.
//!
//! One small JSON file per location under `~/.cache/asciiweather/`. No database:
//! the working set is a handful of entries and the reader is a short-lived CLI.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::weather::model::{Units, WeatherData};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Unix seconds at which this entry was written.
    pub fetched_at: u64,
    pub data: WeatherData,
}

impl Entry {
    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.fetched_at)
    }

    pub fn is_fresh(&self, now: u64, ttl_seconds: u64) -> bool {
        self.age_seconds(now) < ttl_seconds
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `43 minutes ago`, `2 hours ago` — for the stale-cache notice.
pub fn humanize_age(seconds: u64) -> String {
    match seconds {
        0..=59 => "just now".to_string(),
        60..=5399 => plural(seconds / 60, "minute"),
        5400..=86_399 => plural(seconds / 3600, "hour"),
        _ => plural(seconds / 86_400, "day"),
    }
}

fn plural(n: u64, unit: &str) -> String {
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new(dir: PathBuf) -> Self {
        Cache { dir }
    }

    /// `~/.cache/asciiweather/`, falling back to a temp dir on odd systems.
    pub fn default_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("asciiweather")
    }

    /// Keyed by the *query* the user typed, so an offline run can skip geocoding
    /// as well as the forecast call.
    fn path_for(&self, query: &str, units: Units) -> PathBuf {
        let unit = match units {
            Units::Metric => "m",
            Units::Imperial => "i",
        };
        self.dir.join(format!("{}.{unit}.json", slug(query)))
    }

    pub fn read(&self, query: &str, units: Units) -> Option<Entry> {
        let raw = fs::read_to_string(self.path_for(query, units)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Best effort: a cache write must never break the run.
    pub fn write(&self, query: &str, units: Units, fetched_at: u64, data: &WeatherData) {
        let entry = Entry {
            fetched_at,
            data: data.clone(),
        };
        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = write_atomic(&self.path_for(query, units), &json);
        }
    }
}

/// Filesystem-safe, case-insensitive key for a free-text query.
fn slug(query: &str) -> String {
    let mut out: String = query
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    out.truncate(64);
    if out.is_empty() {
        out.push('-');
    }
    out
}

/// Write to a sibling temp file, then rename — an interrupted run leaves the
/// previous entry intact rather than a truncated one.
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    fs::write(&tmp, contents)?;
    restrict_to_owner(&tmp)?;
    fs::rename(&tmp, path)
}

/// Weather data reveals the user's configured location; keep it off shared accounts.
#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::model::{Intensity, Location, WeatherCondition};

    fn sample() -> WeatherData {
        WeatherData {
            location: Location {
                name: "Chennai".into(),
                country: "India".into(),
                latitude: 13.08784,
                longitude: 80.27847,
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
            condition: WeatherCondition::Rain,
            intensity: Intensity::Moderate,
            sunrise: Some("2026-08-20T05:56".into()),
            sunset: Some("2026-08-20T18:22".into()),
            units: Units::Metric,
            forecast: vec![],
        }
    }

    #[test]
    fn fresh_within_ttl_and_stale_after() {
        let entry = Entry {
            fetched_at: 1_000,
            data: sample(),
        };
        assert!(entry.is_fresh(1_400, 600));
        assert!(!entry.is_fresh(1_600, 600));
        assert_eq!(entry.age_seconds(1_420), 420);
    }

    #[test]
    fn clock_going_backwards_does_not_panic() {
        let entry = Entry {
            fetched_at: 5_000,
            data: sample(),
        };
        assert_eq!(entry.age_seconds(1_000), 0);
    }

    #[test]
    fn round_trips_through_the_filesystem() {
        let dir = std::env::temp_dir().join(format!("asciiweather-test-{}", std::process::id()));
        let cache = Cache::new(dir.clone());
        let data = sample();

        assert!(cache.read("Chennai", Units::Metric).is_none());
        cache.write("Chennai", Units::Metric, now_unix(), &data);

        let entry = cache.read("Chennai", Units::Metric).expect("entry");
        assert_eq!(entry.data, data);
        // Queries are case-insensitive, units are not shared.
        assert!(cache.read("chennai", Units::Metric).is_some());
        assert!(cache.read("Chennai", Units::Imperial).is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn slugs_are_filesystem_safe() {
        assert_eq!(slug("New York"), "new-york");
        assert_eq!(slug(" ../../etc/passwd "), "------etc-passwd");
        assert_eq!(slug("São Paulo"), "s-o-paulo");
        assert_eq!(slug(""), "-");
        assert!(slug(&"x".repeat(500)).len() <= 64);
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(humanize_age(20), "just now");
        assert_eq!(humanize_age(60), "1 minute ago");
        assert_eq!(humanize_age(420), "7 minutes ago");
        assert_eq!(humanize_age(7_200), "2 hours ago");
        assert_eq!(humanize_age(200_000), "2 days ago");
    }
}
