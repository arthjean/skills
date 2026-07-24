#![forbid(unsafe_code)]

use std::ffi::OsStr;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

use clap::Parser;

use arthur_skills::cli::{Cli, command_before_separator, json_requested};
use arthur_skills::output;
use arthur_skills::transaction::TRANSACTION_EXIT_CODE;

fn main() -> ExitCode {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    let json = json_requested(&arguments);
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error) if json => {
            let envelope = output::clap_envelope(command_before_separator(&arguments), &error);
            let exit_code = envelope.exit_code;
            if output::write_json(&envelope, &mut io::stdout().lock()).is_err() {
                return ExitCode::from(TRANSACTION_EXIT_CODE);
            }
            return ExitCode::from(exit_code);
        }
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            if error.print().is_err() {
                return ExitCode::from(TRANSACTION_EXIT_CODE);
            }
            return ExitCode::from(code);
        }
    };

    let envelope = arthur_skills::command::execute(&cli);
    let exit_code = envelope.exit_code;
    let plain_environment =
        std::env::var_os("ARTHUR_SKILLS_PLAIN").as_deref() == Some(OsStr::new("1"));
    let compact = exit_code == 0
        && arthur_skills::should_use_tui(
            cli.plain || plain_environment,
            cli.json,
            std::env::var_os("TERM").as_deref(),
            io::stdin().is_terminal(),
            io::stdout().is_terminal(),
            std::env::var("CI").as_deref() == Ok("true"),
        );
    let committed = envelope
        .data
        .get("result")
        .and_then(serde_json::Value::as_str)
        == Some("committed");
    let write_result = if cli.json {
        output::write_json(&envelope, &mut io::stdout().lock())
    } else if envelope.suppress_human_output {
        Ok(())
    } else if compact && committed {
        arthur_skills::ui::show_success(&envelope)
            .or_else(|_| output::write_human_compact(&envelope, &mut io::stdout().lock()))
    } else if compact {
        output::write_human_compact(&envelope, &mut io::stdout().lock())
    } else {
        output::write_human(&envelope, &mut io::stdout().lock())
    };
    if write_result.is_err() {
        ExitCode::from(TRANSACTION_EXIT_CODE)
    } else {
        ExitCode::from(exit_code)
    }
}
