//! The only place that asks the operating system anything about the terminal.

use std::io::IsTerminal;

pub const FALLBACK_WIDTH: usize = 80;

/// Auto-detected terminals get a card-sized panel rather than the full width —
/// a scene stretched across 200 columns is not a better scene. `--width` opts out.
pub const AUTO_MAX_WIDTH: usize = 52;

/// Terminal width in columns, or `None` when stdout is not a terminal.
pub fn detect_width() -> Option<usize> {
    crossterm::terminal::size()
        .ok()
        .map(|(cols, _)| cols as usize)
}

pub fn is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Resolve the width to draw at: an explicit flag is taken literally, otherwise
/// the terminal width capped to a comfortable card.
pub fn resolve_width(explicit: Option<usize>) -> usize {
    match explicit {
        Some(width) => width.clamp(20, 200),
        None => detect_width()
            .filter(|_| is_tty())
            .unwrap_or(FALLBACK_WIDTH)
            .min(AUTO_MAX_WIDTH)
            .clamp(20, 200),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_width_wins_and_is_clamped() {
        assert_eq!(resolve_width(Some(50)), 50);
        assert_eq!(resolve_width(Some(100)), 100);
        assert_eq!(resolve_width(Some(3)), 20);
        assert_eq!(resolve_width(Some(9999)), 200);
    }

    #[test]
    fn piped_output_falls_back_to_a_card() {
        // Not a TTY under `cargo test`, so this exercises the fallback path.
        assert_eq!(resolve_width(None), AUTO_MAX_WIDTH);
    }
}
