pub mod cache;
pub mod model;
pub mod open_meteo;
pub mod provider;

pub use model::{DailyForecast, Intensity, Location, Units, WeatherCondition, WeatherData};
pub use provider::WeatherProvider;

use crate::error::{AppError, Result};
use cache::{Cache, humanize_age};

/// Weather plus, if it came from a stale cache, the reason why.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub data: WeatherData,
    pub notice: Option<String>,
}

/// The cache/provider decision in one place: fresh cache wins, then the
/// network, then — only for transient failures — whatever the cache still has.
pub fn resolve(
    provider: &dyn WeatherProvider,
    cache: &Cache,
    query: &str,
    units: Units,
    ttl_seconds: u64,
    now: u64,
    use_cache: bool,
) -> Result<Reading> {
    let cached = if use_cache {
        cache.read(query, units)
    } else {
        None
    };
    if let Some(entry) = &cached
        && entry.is_fresh(now, ttl_seconds)
    {
        return Ok(Reading {
            data: entry.data.clone(),
            notice: None,
        });
    }

    let fresh = provider
        .geocode(query)
        .and_then(|hit| hit.ok_or_else(|| AppError::UnknownLocation(query.to_string())))
        .and_then(|location| provider.fetch(&location, units));

    match fresh {
        Ok(data) => {
            cache.write(query, units, now, &data);
            Ok(Reading { data, notice: None })
        }
        Err(err) if err.is_transient() => match cached {
            Some(entry) => Ok(Reading {
                notice: Some(format!(
                    "{} Showing cached weather from {}.",
                    err.headline(),
                    humanize_age(entry.age_seconds(now))
                )),
                data: entry.data,
            }),
            None => Err(err),
        },
        Err(err) => Err(err),
    }
}
