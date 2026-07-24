#![forbid(unsafe_code)]

pub mod adoption;
pub mod app;
pub mod catalog;
pub mod cli;
pub mod command;
mod diagnostic;
pub mod engine;
pub mod health;
pub mod lifecycle;
pub mod operations;
pub mod output;
pub mod plain;
pub mod plan;
mod platform;
pub mod provider;
pub mod provider_health;
pub mod receipt;
pub mod transaction;
pub mod ui;
pub mod upstream;
pub mod workflow;

use std::ffi::OsStr;

pub fn should_use_tui(
    plain: bool,
    json: bool,
    term: Option<&OsStr>,
    stdin_is_terminal: bool,
    stdout_is_terminal: bool,
    ci: bool,
) -> bool {
    let dumb_terminal = term.is_some_and(|value| value == OsStr::new("dumb"));
    !plain && !json && !dumb_terminal && !ci && stdin_is_terminal && stdout_is_terminal
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::should_use_tui;

    #[test]
    fn presentation_mode_requires_two_terminals() {
        assert!(should_use_tui(
            false,
            false,
            Some(OsStr::new("xterm")),
            true,
            true,
            false
        ));
        assert!(!should_use_tui(
            true,
            false,
            Some(OsStr::new("xterm")),
            true,
            true,
            false
        ));
        assert!(!should_use_tui(
            false,
            true,
            Some(OsStr::new("xterm")),
            true,
            true,
            false
        ));
        assert!(!should_use_tui(
            false,
            false,
            Some(OsStr::new("dumb")),
            true,
            true,
            false
        ));
        assert!(!should_use_tui(false, false, None, false, true, false));
        assert!(!should_use_tui(false, false, None, true, false, false));
        assert!(!should_use_tui(false, false, None, true, true, true));
    }
}
