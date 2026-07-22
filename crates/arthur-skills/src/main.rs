#![forbid(unsafe_code)]

use std::process::ExitCode;

use clap::Parser;

use arthur_skills::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match arthur_skills::execute(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("arthur-skills: {error}");
            ExitCode::FAILURE
        }
    }
}
