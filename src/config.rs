//! `~/.config/asciiweather/config.toml` — small on purpose.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::weather::Units;

/// The config file names the user's default location; keep it off shared accounts.
#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub render: Render,
    pub cache: Cache,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct General {
    /// Used when no location is given on the command line.
    pub location: Option<String>,
    pub units: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Render {
    pub color: bool,
    /// `"auto"` or a column count.
    pub width: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Cache {
    pub ttl_minutes: u64,
}

impl Default for General {
    fn default() -> Self {
        General {
            location: None,
            units: "metric".into(),
        }
    }
}

impl Default for Render {
    fn default() -> Self {
        Render {
            color: true,
            width: "auto".into(),
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        Cache { ttl_minutes: 10 }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("asciiweather")
            .join("config.toml")
    }

    /// A missing config file is normal, not an error.
    pub fn load() -> Result<Config> {
        let path = Config::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => Config::parse(&raw),
            Err(_) => Ok(Config::default()),
        }
    }

    pub fn parse(raw: &str) -> Result<Config> {
        toml::from_str(raw).map_err(|e| {
            AppError::Config(format!("{} is not valid: {e}", Config::path().display()))
        })
    }

    pub fn save(&self) -> Result<()> {
        let path = Config::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| AppError::Config(format!("cannot create {}: {e}", dir.display())))?;
        }
        let body = toml::to_string_pretty(self)
            .map_err(|e| AppError::Config(format!("cannot serialise config: {e}")))?;
        std::fs::write(&path, body)
            .map_err(|e| AppError::Config(format!("cannot write {}: {e}", path.display())))?;
        restrict_to_owner(&path).map_err(|e| {
            AppError::Config(format!("cannot set permissions on {}: {e}", path.display()))
        })
    }

    pub fn units(&self) -> Units {
        self.general.units.parse().unwrap_or(Units::Metric)
    }

    pub fn ttl_seconds(&self) -> u64 {
        self.cache.ttl_minutes.saturating_mul(60)
    }

    /// `None` means "ask the terminal".
    pub fn width(&self) -> Option<usize> {
        self.render.width.parse().ok()
    }

    /// The settings a user can change, as `(key, value)` pairs.
    pub fn entries(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "location",
                self.general.location.clone().unwrap_or_else(|| "—".into()),
            ),
            ("units", self.general.units.clone()),
            ("color", self.render.color.to_string()),
            ("width", self.render.width.clone()),
            ("ttl_minutes", self.cache.ttl_minutes.to_string()),
        ]
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let invalid =
            |what: &str| AppError::Config(format!("`{value}` is not a valid {key} ({what})"));
        match key {
            "location" => self.general.location = Some(value.to_string()),
            "units" => {
                let units: Units = value.parse().map_err(|_| invalid("metric or imperial"))?;
                self.general.units = match units {
                    Units::Metric => "metric",
                    Units::Imperial => "imperial",
                }
                .into();
            }
            "color" => {
                self.render.color = value.parse().map_err(|_| invalid("true or false"))?;
            }
            "width" => {
                if value != "auto" && value.parse::<usize>().is_err() {
                    return Err(invalid("auto or a column count"));
                }
                self.render.width = value.to_string();
            }
            "ttl_minutes" => {
                self.cache.ttl_minutes =
                    value.parse().map_err(|_| invalid("a number of minutes"))?;
            }
            other => {
                return Err(AppError::Config(format!(
                    "unknown setting `{other}` (try: location, units, color, width, ttl_minutes)"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        let config = Config::default();
        assert_eq!(config.units(), Units::Metric);
        assert_eq!(config.ttl_seconds(), 600);
        assert_eq!(config.width(), None);
        assert!(config.general.location.is_none());
    }

    #[test]
    fn parses_the_documented_file() {
        let config = Config::parse(
            r#"
            [general]
            location = "Chennai"
            units = "imperial"

            [render]
            color = false
            width = "60"

            [cache]
            ttl_minutes = 30
            "#,
        )
        .unwrap();
        assert_eq!(config.general.location.as_deref(), Some("Chennai"));
        assert_eq!(config.units(), Units::Imperial);
        assert!(!config.render.color);
        assert_eq!(config.width(), Some(60));
        assert_eq!(config.ttl_seconds(), 1800);
    }

    #[test]
    fn partial_files_keep_defaults() {
        let config = Config::parse("[general]\nlocation = \"Tokyo\"\n").unwrap();
        assert_eq!(config.units(), Units::Metric);
        assert_eq!(config.ttl_seconds(), 600);
    }

    #[test]
    fn broken_files_are_reported_not_ignored() {
        assert!(Config::parse("[general").is_err());
    }

    #[test]
    fn set_validates_values() {
        let mut config = Config::default();
        config.set("location", "New York").unwrap();
        config.set("units", "F").unwrap();
        config.set("width", "auto").unwrap();
        assert_eq!(config.general.location.as_deref(), Some("New York"));
        assert_eq!(config.general.units, "imperial");

        assert!(config.set("units", "kelvin").is_err());
        assert!(config.set("width", "wide").is_err());
        assert!(config.set("ttl_minutes", "soon").is_err());
        assert!(config.set("nonsense", "1").is_err());
    }

    #[test]
    fn round_trips_through_toml() {
        let mut config = Config::default();
        config.set("location", "Reykjavík").unwrap();
        let raw = toml::to_string_pretty(&config).unwrap();
        assert_eq!(Config::parse(&raw).unwrap(), config);
    }
}
