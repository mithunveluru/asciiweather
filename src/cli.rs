//! Argument parsing and the small amount of glue that wires the pipeline
//! together: provider -> cache -> scene -> renderer -> stdout.

use std::io::Write;

use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::render::ascii::{self, Layout};
use crate::render::color::{self, Line, Paint, Span};
use crate::render::{animation, terminal};
use crate::scene::generator::{RenderOptions, default_seed, generate};
use crate::weather;
use crate::weather::WeatherData;
use crate::weather::cache::{Cache, now_unix};
use crate::weather::open_meteo::OpenMeteo;

#[derive(Parser, Debug)]
#[command(
    name = "asciiweather",
    version,
    about = "Weather, rendered by your terminal.",
    after_help = "Examples:\n  asciiweather Chennai\n  asciiweather \"New York\" --forecast\n  asciiweather --json | jq .temperature"
)]
pub struct Cli {
    /// Place to look up. Defaults to the configured location.
    pub location: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,

    /// One-line summary instead of a scene
    #[arg(long)]
    pub compact: bool,

    /// No frame, no styling, ASCII art only
    #[arg(long)]
    pub plain: bool,

    /// Disable ANSI colour
    #[arg(long)]
    pub no_color: bool,

    /// Print normalized weather data as JSON
    #[arg(long)]
    pub json: bool,

    /// Render at this many columns instead of detecting
    #[arg(long, value_name = "N")]
    pub width: Option<usize>,

    /// Seed the procedural scene for reproducible output
    #[arg(long, value_name = "N")]
    pub seed: Option<u64>,

    /// Ignore cached weather and fetch fresh data
    #[arg(long)]
    pub no_cache: bool,

    /// Show the next few days instead of current conditions
    #[arg(long)]
    pub forecast: bool,

    /// Animate the scene until a key is pressed
    #[arg(long)]
    pub animate: bool,

    /// Include technical detail in error messages
    #[arg(long)]
    pub debug: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show or change configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Change a setting, e.g. `config set location Chennai`
    Set { key: String, value: String },
    /// Print the path of the config file
    Path,
}

pub fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Some(Command::Config { action }) => config_command(action.as_ref()),
        None => weather_command(cli),
    }
}

fn config_command(action: Option<&ConfigAction>) -> Result<()> {
    match action {
        Some(ConfigAction::Path) => {
            emit(&format!("{}\n", Config::path().display()));
            Ok(())
        }
        Some(ConfigAction::Set { key, value }) => {
            let mut config = Config::load()?;
            config.set(key, value)?;
            config.save()?;
            emit(&format!("{key} = {value}\n{}\n", Config::path().display()));
            Ok(())
        }
        None => {
            let config = Config::load()?;
            let mut out = format!("{}\n\n", Config::path().display());
            for (key, value) in config.entries() {
                out.push_str(&format!("  {key:<12} {value}\n"));
            }
            out.push_str("\nChange a setting:\n  asciiweather config set location Chennai\n");
            emit(&out);
            Ok(())
        }
    }
}

fn weather_command(cli: &Cli) -> Result<()> {
    let config = Config::load()?;
    let query = cli
        .location
        .clone()
        .or_else(|| config.general.location.clone())
        .ok_or(AppError::NoLocation)?;

    let reading = weather::resolve(
        &OpenMeteo::default(),
        &Cache::new(Cache::default_dir()),
        &query,
        config.units(),
        config.ttl_seconds(),
        now_unix(),
        !cli.no_cache,
    )?;
    present(cli, &config, &reading.data, reading.notice)
}

fn present(
    cli: &Cli,
    config: &Config,
    weather: &WeatherData,
    notice: Option<String>,
) -> Result<()> {
    if cli.json {
        if let Some(text) = &notice {
            eprintln!("{text}");
        }
        let body = serde_json::to_string_pretty(weather)
            .map_err(|e| AppError::Config(format!("cannot serialise weather: {e}")))?;
        emit(&format!("{body}\n"));
        return Ok(());
    }

    let layout = Layout::new(
        terminal::resolve_width(cli.width.or_else(|| config.width())),
        cli.plain,
    );
    let colored = !cli.plain && config.render.color && color::should_colorize(cli.no_color);
    let night = weather.is_night();
    let seed = cli.seed.unwrap_or_else(|| default_seed(weather));

    let mut lines: Vec<Line> = Vec::new();
    if let Some(text) = &notice {
        lines.push(ascii::notice(text, &layout));
        lines.push(Vec::new());
    }

    if cli.compact {
        lines.push(ascii::compact_line(weather, &layout));
    } else if cli.forecast {
        lines.extend(ascii::framed(
            ascii::forecast_lines(weather, &layout),
            &layout,
        ));
    } else if cli.animate && terminal::is_tty() && !cli.plain {
        emit(&color::to_string(&lines, colored, night));
        return animation::play(weather, &layout, colored, seed);
    } else {
        let scene = generate(
            weather,
            &RenderOptions {
                width: layout.inner_width(),
                height: None,
                seed,
                frame: 0,
                ascii: cli.plain,
            },
        );
        lines.extend(ascii::panel(weather, &scene, &layout));
    }

    emit(&color::to_string(&lines, colored, night));
    Ok(())
}

/// Write to stdout, tolerating a closed pipe (`asciiweather X | head`).
fn emit(text: &str) {
    let _ = write!(std::io::stdout(), "{text}");
}

/// Render an error the way a person wants to read it.
pub fn report(err: &AppError, debug: bool) -> String {
    let plain = !color::should_colorize(false);
    let mark = if plain { "x" } else { "✗" };
    let mut lines: Vec<Line> = vec![vec![Span::new(
        format!("{mark} {}", err.headline()),
        Paint::Notice,
    )]];
    let hints = err.hint();
    if !hints.is_empty() {
        lines.push(Vec::new());
        lines.extend(
            hints
                .into_iter()
                .map(|h| vec![Span::new(format!("  {h}"), Paint::Detail)]),
        );
    }
    if debug && let Some(detail) = err.detail() {
        lines.push(Vec::new());
        lines.push(vec![Span::new(
            format!("  detail: {detail}"),
            Paint::Detail,
        )]);
    }
    color::to_string(&lines, false, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn bare_invocation_has_no_location() {
        let cli = Cli::try_parse_from(["asciiweather"]).unwrap();
        assert!(cli.location.is_none() && cli.command.is_none());
    }

    #[test]
    fn positional_location_and_flags_parse() {
        let cli = Cli::try_parse_from([
            "asciiweather",
            "New York",
            "--seed",
            "42",
            "--width",
            "50",
            "--plain",
            "--no-cache",
        ])
        .unwrap();
        assert_eq!(cli.location.as_deref(), Some("New York"));
        assert_eq!(cli.seed, Some(42));
        assert_eq!(cli.width, Some(50));
        assert!(cli.plain && cli.no_cache);
        assert!(!cli.json && !cli.compact);
    }

    #[test]
    fn config_subcommand_beats_the_positional() {
        let cli = Cli::try_parse_from(["asciiweather", "config"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Config { action: None })
        ));

        let cli =
            Cli::try_parse_from(["asciiweather", "config", "set", "location", "Chennai"]).unwrap();
        match cli.command {
            Some(Command::Config {
                action: Some(ConfigAction::Set { key, value }),
            }) => {
                assert_eq!((key.as_str(), value.as_str()), ("location", "Chennai"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn unknown_flags_are_rejected() {
        assert!(Cli::try_parse_from(["asciiweather", "--nope"]).is_err());
    }

    #[test]
    fn errors_read_like_advice_not_stack_traces() {
        let text = report(&AppError::UnknownLocation("Chennnai".into()), false);
        assert!(text.contains("Could not find \"Chennnai\"."));
        assert!(text.contains("asciiweather Chennai"));
        assert!(!text.contains("reqwest") && !text.contains("ureq"));

        let hidden = report(&AppError::Service("HTTP 429".into()), false);
        assert!(!hidden.contains("429"));
        assert!(report(&AppError::Service("HTTP 429".into()), true).contains("429"));
    }
}
