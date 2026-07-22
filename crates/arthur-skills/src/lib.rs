#![forbid(unsafe_code)]

pub mod app;
pub mod catalog;
pub mod cli;
pub mod plain;
pub mod provider_health;
pub mod ui;

use std::ffi::OsStr;
use std::io::{self, IsTerminal};

use app::App;
use cli::Cli;

pub fn execute(cli: &Cli) -> Result<(), String> {
    let catalog = catalog::Catalog::load().map_err(|error| error.to_string())?;
    let app = App::new(catalog.skill_count());
    let term = std::env::var_os("TERM");
    let use_tui = should_use_tui(
        cli.plain,
        term.as_deref(),
        io::stdin().is_terminal(),
        io::stdout().is_terminal(),
    );

    if use_tui {
        ui::run(app).map_err(|error| error.to_string())
    } else {
        plain::render(&app, &mut io::stdout().lock()).map_err(|error| error.to_string())
    }
}

pub fn should_use_tui(
    plain: bool,
    term: Option<&OsStr>,
    stdin_is_terminal: bool,
    stdout_is_terminal: bool,
) -> bool {
    let dumb_terminal = term.is_some_and(|value| value == OsStr::new("dumb"));
    !plain && !dumb_terminal && stdin_is_terminal && stdout_is_terminal
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::should_use_tui;

    #[test]
    fn presentation_mode_requires_two_terminals() {
        assert!(should_use_tui(false, Some(OsStr::new("xterm")), true, true));
        assert!(!should_use_tui(true, Some(OsStr::new("xterm")), true, true));
        assert!(!should_use_tui(false, Some(OsStr::new("dumb")), true, true));
        assert!(!should_use_tui(false, None, false, true));
        assert!(!should_use_tui(false, None, true, false));
    }
}
