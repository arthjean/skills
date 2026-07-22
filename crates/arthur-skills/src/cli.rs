use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "arthur-skills",
    version,
    about = "Install and manage the portable Arthur Workflow catalog"
)]
pub struct Cli {
    /// Render deterministic line-oriented output without terminal control sequences.
    #[arg(long)]
    pub plain: bool,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::Cli;

    #[test]
    fn clap_contract_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_plain_mode() {
        let parsed = Cli::try_parse_from(["arthur-skills", "--plain"]);
        assert!(parsed.is_ok_and(|cli| cli.plain));
    }
}
