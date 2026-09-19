//! Colour is an accent, never load-bearing. Every scene must read correctly
//! with all of it stripped out.

use std::io::IsTerminal;

use crate::scene::model::Style;

/// What a run of characters means, for the purpose of styling it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    Scene(Style),
    Frame,
    Temperature,
    Place,
    Condition,
    Detail,
    Notice,
    None,
}

impl Paint {
    fn ansi(self, night: bool) -> &'static str {
        match self {
            Paint::Scene(Style::Sun) => "\x1b[93m",
            Paint::Scene(Style::Moon) => "\x1b[97m",
            Paint::Scene(Style::Star) => "\x1b[90m",
            Paint::Scene(Style::Cloud) => {
                if night {
                    "\x1b[90m"
                } else {
                    "\x1b[37m"
                }
            }
            Paint::Scene(Style::Rain) => "\x1b[94m",
            Paint::Scene(Style::Snow) => "\x1b[96m",
            Paint::Scene(Style::Bolt) => "\x1b[93m",
            Paint::Scene(Style::Fog) | Paint::Scene(Style::Ground) => "\x1b[90m",
            Paint::Scene(Style::Wind) => "\x1b[36m",
            Paint::Frame | Paint::Detail => "\x1b[90m",
            Paint::Temperature => "\x1b[1m",
            Paint::Condition => "\x1b[2m",
            Paint::Notice => "\x1b[33m",
            Paint::Place | Paint::None => "",
        }
    }
}

const RESET: &str = "\x1b[0m";

/// A run of same-styled characters.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub paint: Paint,
}

impl Span {
    pub fn new(text: impl Into<String>, paint: Paint) -> Self {
        Span {
            text: text.into(),
            paint,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Span::new(text, Paint::None)
    }
}

pub type Line = Vec<Span>;

/// Flatten styled lines into the string that goes to stdout.
pub fn to_string(lines: &[Line], color: bool, night: bool) -> String {
    let mut out = String::new();
    for line in lines {
        for span in line {
            let code = if color { span.paint.ansi(night) } else { "" };
            if code.is_empty() {
                out.push_str(&span.text);
            } else {
                out.push_str(code);
                out.push_str(&span.text);
                out.push_str(RESET);
            }
        }
        // Trailing blanks are noise when the output is piped or copied.
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
    }
    out
}

/// Colour is on only when it was asked for, allowed, and going to a terminal.
pub fn should_colorize(no_color_flag: bool) -> bool {
    if no_color_flag || std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false) {
        return false;
    }
    std::io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_color_the_text_is_untouched() {
        let lines = vec![vec![
            Span::new("27°C", Paint::Temperature),
            Span::new(" Rain", Paint::Condition),
        ]];
        assert_eq!(to_string(&lines, false, false), "27°C Rain\n");
    }

    #[test]
    fn with_color_every_run_is_reset() {
        let lines = vec![vec![Span::new("x", Paint::Scene(Style::Rain))]];
        assert_eq!(to_string(&lines, true, false), "\x1b[94mx\x1b[0m\n");
    }

    #[test]
    fn trailing_padding_is_trimmed() {
        let lines = vec![vec![Span::plain("hi     ")]];
        assert_eq!(to_string(&lines, false, false), "hi\n");
    }

    #[test]
    fn clouds_dim_at_night() {
        assert_ne!(
            Paint::Scene(Style::Cloud).ansi(true),
            Paint::Scene(Style::Cloud).ansi(false)
        );
    }
}
