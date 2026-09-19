//! User-facing errors. Raw transport/HTTP detail is kept behind `--debug`.

use std::fmt;

#[derive(Debug)]
pub enum AppError {
    /// Could not reach the weather service at all.
    Network(String),
    /// Reached it, but it answered badly (5xx, rate limit, garbage body).
    Service(String),
    /// Geocoding found nothing for the query.
    UnknownLocation(String),
    /// No location on the command line and none configured.
    NoLocation,
    /// Configuration / cache / filesystem trouble.
    Config(String),
}

impl AppError {
    /// Short headline, always safe to show.
    pub fn headline(&self) -> String {
        match self {
            AppError::Network(_) => "No network connection.".into(),
            AppError::Service(_) => "Weather service is temporarily unavailable.".into(),
            AppError::UnknownLocation(q) => format!("Could not find \"{q}\"."),
            AppError::NoLocation => "No default location configured.".into(),
            AppError::Config(m) => m.clone(),
        }
    }

    /// Extra hint lines printed under the headline.
    pub fn hint(&self) -> Vec<String> {
        match self {
            AppError::Network(_) => vec!["No cached weather for this location yet.".into()],
            AppError::Service(_) => vec!["Try again in a moment.".into()],
            AppError::UnknownLocation(_) => {
                vec!["Try:".into(), "  asciiweather Chennai".into()]
            }
            AppError::NoLocation => vec![
                "Try:".into(),
                "  asciiweather Chennai".into(),
                "  asciiweather config set location Chennai".into(),
            ],
            AppError::Config(_) => vec![],
        }
    }

    /// Technical detail, only shown with `--debug`.
    pub fn detail(&self) -> Option<&str> {
        match self {
            AppError::Network(d) | AppError::Service(d) | AppError::Config(d) => Some(d),
            _ => None,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::UnknownLocation(_) => 3,
            AppError::Network(_) | AppError::Service(_) => 4,
            _ => 1,
        }
    }

    /// Whether stale cached data is an acceptable substitute for this failure.
    pub fn is_transient(&self) -> bool {
        matches!(self, AppError::Network(_) | AppError::Service(_))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.headline())
    }
}

impl std::error::Error for AppError {}

pub type Result<T> = std::result::Result<T, AppError>;
