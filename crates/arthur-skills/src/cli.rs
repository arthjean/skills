use std::ffi::{OsStr, OsString};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::provider::ProviderId;

#[derive(Debug, Parser)]
#[command(
    name = "arthur-skills",
    version,
    about = "Install and manage the portable Arthur Workflow catalog",
    subcommand_required = false
)]
pub struct Cli {
    /// Render deterministic line-oriented output without terminal control sequences.
    #[arg(long, global = true, conflicts_with = "json")]
    pub plain: bool,

    /// Emit exactly one versioned JSON envelope on stdout.
    #[arg(long, global = true, conflicts_with = "plain")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect the exact filesystem plan without mutating it.
    Plan(ProviderArgs),
    /// Install or reconcile the complete catalog for selected providers.
    Install(MutationArgs),
    /// Report receipt and provider ownership state.
    Status,
    /// Diagnose the receipt, transaction journal, and provider state.
    Doctor,
    /// Reconcile managed providers to the embedded catalog.
    Update(ConfirmationArgs),
    /// Remove one provider integration, or all managed integrations.
    Uninstall(UninstallArgs),
    /// Adopt compatible entries from a Vercel Skills v3 installation.
    Adopt(MutationArgs),
    /// Resume rollback or cleanup from the durable transaction journal.
    Recover,
}

impl Command {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Plan(_) => "plan",
            Self::Install(_) => "install",
            Self::Status => "status",
            Self::Doctor => "doctor",
            Self::Update(_) => "update",
            Self::Uninstall(_) => "uninstall",
            Self::Adopt(_) => "adopt",
            Self::Recover => "recover",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, ValueEnum)]
pub enum ProviderArg {
    Claude,
    Codex,
}

impl From<ProviderArg> for ProviderId {
    fn from(provider: ProviderArg) -> Self {
        match provider {
            ProviderArg::Claude => Self::Claude,
            ProviderArg::Codex => Self::Codex,
        }
    }
}

#[derive(Args, Debug, Default)]
pub struct ProviderArgs {
    /// Provider to manage. Repeat for both providers.
    #[arg(long, value_enum, value_delimiter = ',')]
    pub provider: Vec<ProviderArg>,
}

impl ProviderArgs {
    pub fn providers(&self) -> Vec<ProviderId> {
        let mut providers = self
            .provider
            .iter()
            .copied()
            .map(ProviderId::from)
            .collect::<Vec<_>>();
        providers.sort();
        providers.dedup();
        providers
    }
}

#[derive(Args, Debug, Default)]
pub struct ConfirmationArgs {
    /// Apply without asking for confirmation.
    #[arg(long)]
    pub yes: bool,

    /// Calculate and render the plan without changing the filesystem.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug, Default)]
pub struct MutationArgs {
    #[command(flatten)]
    pub providers: ProviderArgs,

    #[command(flatten)]
    pub confirmation: ConfirmationArgs,
}

#[derive(Args, Debug, Default)]
pub struct UninstallArgs {
    #[command(flatten)]
    pub providers: ProviderArgs,

    /// Remove every managed provider integration.
    #[arg(long, conflicts_with = "provider")]
    pub all: bool,

    #[command(flatten)]
    pub confirmation: ConfirmationArgs,
}

pub fn json_requested(args: &[OsString]) -> bool {
    args.iter()
        .skip(1)
        .take_while(|argument| argument.as_os_str() != OsStr::new("--"))
        .any(|argument| argument.as_os_str() == OsStr::new("--json"))
}

pub fn command_before_separator(args: &[OsString]) -> Option<&'static str> {
    const COMMANDS: [&str; 8] = [
        "plan",
        "install",
        "status",
        "doctor",
        "update",
        "uninstall",
        "adopt",
        "recover",
    ];
    for argument in args
        .iter()
        .skip(1)
        .take_while(|argument| argument.as_os_str() != OsStr::new("--"))
    {
        if matches!(
            argument.to_str(),
            Some("--json" | "--plain" | "--help" | "-h" | "--version" | "-V")
        ) {
            continue;
        }
        if argument.to_string_lossy().starts_with('-') {
            return None;
        }
        if let Some(command) = COMMANDS
            .iter()
            .find(|command| argument.as_os_str() == OsStr::new(command))
        {
            return Some(command);
        }
        return None;
    }
    None
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, command_before_separator, json_requested};

    #[test]
    fn clap_contract_is_valid_and_exposes_all_commands() {
        Cli::command().debug_assert();
        let help = Cli::command().render_long_help().to_string();
        for command in [
            "plan",
            "install",
            "status",
            "doctor",
            "update",
            "uninstall",
            "adopt",
            "recover",
        ] {
            assert!(help.contains(command));
        }
    }

    #[test]
    fn parses_machine_and_mutation_options() {
        let parsed = Cli::try_parse_from([
            "arthur-skills",
            "--json",
            "install",
            "--provider",
            "claude,codex",
            "--yes",
            "--dry-run",
        ]);
        let Ok(cli) = parsed else {
            panic!("valid command was rejected");
        };
        let Some(Command::Install(arguments)) = cli.command else {
            panic!("install command was not resolved");
        };
        assert!(cli.json);
        assert_eq!(arguments.providers.providers().len(), 2);
        assert!(arguments.confirmation.yes);
        assert!(arguments.confirmation.dry_run);
    }

    #[test]
    fn json_pre_scan_stops_at_the_separator() {
        let before = [
            OsString::from("arthur-skills"),
            OsString::from("--json"),
            OsString::from("plan"),
        ];
        assert!(json_requested(&before));
        assert_eq!(command_before_separator(&before), Some("plan"));

        let after = [
            OsString::from("arthur-skills"),
            OsString::from("plan"),
            OsString::from("--"),
            OsString::from("--json"),
        ];
        assert!(!json_requested(&after));

        let unresolved = [
            OsString::from("arthur-skills"),
            OsString::from("--json"),
            OsString::from("--bogus"),
            OsString::from("status"),
        ];
        assert_eq!(command_before_separator(&unresolved), None);
    }
}
