//! The seam between "some weather API" and the rest of the program.
//!
//! Everything above this line speaks only `Location` / `WeatherData`.

use crate::error::Result;
use crate::weather::model::{Location, Units, WeatherData};

pub trait WeatherProvider {
    /// Resolve a free-text place name. `Ok(None)` means "no such place".
    fn geocode(&self, query: &str) -> Result<Option<Location>>;

    /// Current conditions plus a short daily outlook for `location`.
    fn fetch(&self, location: &Location, units: Units) -> Result<WeatherData>;
}
