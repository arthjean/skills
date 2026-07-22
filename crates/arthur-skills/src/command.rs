use std::collections::BTreeSet;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::adoption::{self, CatalogEntry, EntryType};
use crate::app::{App, Review};
use crate::catalog::Catalog;
use crate::cli::{Cli, Command, ConfirmationArgs, MutationArgs, ProviderArgs, UninstallArgs};
use crate::lifecycle::{LifecycleIntent, LifecycleTransition, prepare_lifecycle_transition};
use crate::operations::{operations_for_adoption, operations_for_plan};
use crate::output::{
    CONFLICT_EXIT_CODE, Envelope, OutputDiagnostic, OutputSeverity, OutputStatus, path_fields,
};
use crate::plain::{self, PlainExit};
use crate::provider::{
    ENVIRONMENT_EXIT_CODE, ProviderId, ResolveError, ResolvedRoots, resolve_roots,
};
use crate::receipt::{Receipt, ReceiptState};
use crate::transaction::{
    PathKind, RootSpec, SignalFlags, TRANSACTION_EXIT_CODE, TransactionEngine, TransactionOutcome,
    snapshot_path,
};
use crate::ui::{self, UiExit};
use crate::{engine, plan};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Presentation {
    Tui,
    PlainInteractive,
    NonInteractive,
}

type CommandResult<T> = Result<T, Box<Envelope>>;

struct ApplyRequest<'a> {
    catalog: &'a Catalog,
    cli: &'a Cli,
    command: &'a str,
    confirmation: &'a ConfirmationArgs,
    presentation: Presentation,
    signals: &'a SignalFlags,
}

pub fn execute(cli: &Cli) -> Envelope {
    let resolved_command = cli.command.as_ref().map(Command::name);
    let command_name = resolved_command.unwrap_or("install");
    let catalog = match Catalog::load() {
        Ok(catalog) => catalog,
        Err(error) => {
            return Envelope::failure(
                resolved_command,
                OutputStatus::Failed,
                TRANSACTION_EXIT_CODE,
                "catalog_invalid",
                error.to_string(),
            );
        }
    };
    let signals = match SignalFlags::install() {
        Ok(signals) => signals,
        Err(error) => {
            let mut envelope = transaction_error(
                command_name,
                Vec::new(),
                error.exit_code(),
                error.to_string(),
            );
            envelope.command = resolved_command.map(str::to_owned);
            return envelope;
        }
    };
    let presentation = presentation(cli);
    let mut envelope = match cli.command.as_ref() {
        None => run_install(
            &catalog,
            cli,
            &MutationArgs::default(),
            presentation,
            &signals,
        ),
        Some(Command::Plan(arguments)) => {
            run_plan(&catalog, cli, arguments, presentation, &signals)
        }
        Some(Command::Install(arguments)) => {
            run_install(&catalog, cli, arguments, presentation, &signals)
        }
        Some(Command::Status) => run_status(&catalog, "status"),
        Some(Command::Doctor) => run_doctor(&catalog),
        Some(Command::Update(arguments)) => {
            run_update(&catalog, cli, arguments, presentation, &signals)
        }
        Some(Command::Uninstall(arguments)) => {
            run_uninstall(&catalog, cli, arguments, presentation, &signals)
        }
        Some(Command::Adopt(arguments)) => {
            run_adopt(&catalog, cli, arguments, presentation, &signals)
        }
        Some(Command::Recover) => run_recover(&catalog, &signals),
    };
    if cli.command.is_none() {
        envelope.command = None;
    }
    envelope
}

fn run_plan(
    catalog: &Catalog,
    _cli: &Cli,
    arguments: &ProviderArgs,
    presentation: Presentation,
    signals: &SignalFlags,
) -> Envelope {
    let providers = match providers_or_interactive(
        catalog,
        arguments.providers(),
        presentation,
        "plan",
        signals,
    ) {
        Ok(providers) => providers,
        Err(envelope) => return *envelope,
    };
    let (roots, current) = match context(&providers, "plan") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    match prepare_lifecycle_transition(
        catalog,
        &roots,
        current.as_ref(),
        &LifecycleIntent::Install {
            providers: providers.clone(),
        },
    ) {
        Ok(transition) => report_transition("plan", transition),
        Err(error) => lifecycle_error("plan", providers, error.to_string()),
    }
}

fn run_install(
    catalog: &Catalog,
    cli: &Cli,
    arguments: &MutationArgs,
    presentation: Presentation,
    signals: &SignalFlags,
) -> Envelope {
    let explicit = arguments.providers.providers();
    if presentation == Presentation::NonInteractive
        && arguments.confirmation.yes
        && explicit.is_empty()
    {
        return Envelope::usage(
            Some("install"),
            "--yes requires an explicit --provider <claude|codex> in non-interactive mode",
        );
    }
    let providers =
        match providers_or_interactive(catalog, explicit, presentation, "install", signals) {
            Ok(providers) => providers,
            Err(envelope) => return *envelope,
        };
    let (roots, current) = match context(&providers, "install") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let transition = match prepare_lifecycle_transition(
        catalog,
        &roots,
        current.as_ref(),
        &LifecycleIntent::Install {
            providers: providers.clone(),
        },
    ) {
        Ok(transition) => transition,
        Err(error) => return lifecycle_error("install", providers, error.to_string()),
    };
    apply_transition(
        ApplyRequest {
            catalog,
            cli,
            command: "install",
            confirmation: &arguments.confirmation,
            presentation,
            signals,
        },
        roots,
        transition,
    )
}

fn run_update(
    catalog: &Catalog,
    cli: &Cli,
    confirmation: &ConfirmationArgs,
    presentation: Presentation,
    signals: &SignalFlags,
) -> Envelope {
    let (_, current) = match context(&[], "update") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let Some(receipt) = current else {
        return Envelope::failure(
            Some("update"),
            OutputStatus::Blocked,
            CONFLICT_EXIT_CODE,
            "not_installed",
            "no Arthur Workflow receipt exists; run install first",
        );
    };
    let providers = managed_providers(&receipt);
    let (roots, current) = match context(&providers, "update") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let transition = match prepare_lifecycle_transition(
        catalog,
        &roots,
        current.as_ref(),
        &LifecycleIntent::Install {
            providers: providers.clone(),
        },
    ) {
        Ok(transition) => transition,
        Err(error) => return lifecycle_error("update", providers, error.to_string()),
    };
    apply_transition(
        ApplyRequest {
            catalog,
            cli,
            command: "update",
            confirmation,
            presentation,
            signals,
        },
        roots,
        transition,
    )
}

fn run_uninstall(
    catalog: &Catalog,
    cli: &Cli,
    arguments: &UninstallArgs,
    presentation: Presentation,
    signals: &SignalFlags,
) -> Envelope {
    let (_, current) = match context(&[], "uninstall") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let Some(receipt) = current else {
        let mut envelope = Envelope::new(Some("uninstall"));
        envelope.status = OutputStatus::Noop;
        envelope.data = json!({ "message": "nothing is installed" });
        return envelope;
    };
    let managed = managed_providers(&receipt);
    let removed = arguments.providers.providers();
    if presentation == Presentation::NonInteractive && !arguments.all && removed.is_empty() {
        return Envelope::usage(
            Some("uninstall"),
            "non-interactive uninstall requires one --provider or explicit --all",
        );
    }
    let intent = if arguments.all {
        LifecycleIntent::UninstallAll
    } else if removed.len() == 1 {
        if !managed.contains(&removed[0]) {
            return Envelope::usage(
                Some("uninstall"),
                format!("provider {} is not managed by this receipt", removed[0]),
            );
        }
        LifecycleIntent::UninstallProvider(removed[0])
    } else if removed.is_empty() {
        LifecycleIntent::UninstallAll
    } else {
        return Envelope::usage(
            Some("uninstall"),
            "use one --provider at a time or pass --all",
        );
    };
    let (roots, current) = match context(&managed, "uninstall") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let transition = match prepare_lifecycle_transition(catalog, &roots, current.as_ref(), &intent)
    {
        Ok(transition) => transition,
        Err(error) => return lifecycle_error("uninstall", managed, error.to_string()),
    };
    apply_transition(
        ApplyRequest {
            catalog,
            cli,
            command: "uninstall",
            confirmation: &arguments.confirmation,
            presentation,
            signals,
        },
        roots,
        transition,
    )
}

fn run_adopt(
    catalog: &Catalog,
    cli: &Cli,
    arguments: &MutationArgs,
    presentation: Presentation,
    signals: &SignalFlags,
) -> Envelope {
    let providers = match providers_or_interactive(
        catalog,
        arguments.providers.providers(),
        presentation,
        "adopt",
        signals,
    ) {
        Ok(providers) => providers,
        Err(envelope) => return *envelope,
    };
    let (roots, current) = match context(&providers, "adopt") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let transition = match prepare_lifecycle_transition(
        catalog,
        &roots,
        current.as_ref(),
        &LifecycleIntent::Install {
            providers: providers.clone(),
        },
    ) {
        Ok(transition) => transition,
        Err(error) => return lifecycle_error("adopt", providers, error.to_string()),
    };
    let entries = match adoption_entries(&transition, &roots) {
        Ok(entries) => entries,
        Err(message) => return lifecycle_error("adopt", providers, message),
    };
    if entries.is_empty() {
        let mut envelope = report_transition("adopt", transition);
        if envelope.status != OutputStatus::Blocked {
            envelope.status = OutputStatus::Noop;
            envelope.exit_code = 0;
        }
        envelope.data =
            json!({ "adopted": 0, "message": "no matching unmanaged skill entries were found" });
        return envelope;
    }
    let lock_path = roots.canonical.lexical.join(".skill-lock.json");
    let archive_path = roots.state_directory.join("vercel-skills-v3-lock.json");
    let adoption = match adoption::inspect(&lock_path, &archive_path, &entries) {
        Ok(adoption) => adoption,
        Err(error) => return lifecycle_error("adopt", providers, error.to_string()),
    };
    let mut adoption_plan = transition.plan.clone();
    adoption_plan
        .entries
        .retain(|entry| entry.action == plan::PlanAction::Adoptable);
    adoption_plan.operations.clear();
    adoption_plan.applicable = adoption.applicable;
    let mut envelope = Envelope::new(Some("adopt")).with_plan(&adoption_plan);
    envelope.providers.clone_from(&providers);
    if adoption.applicable {
        envelope.status = OutputStatus::Success;
    }
    envelope
        .diagnostics
        .extend(adoption.diagnostics.iter().map(|diagnostic| {
            let (path_utf8, path_bytes_hex) = diagnostic
                .destination
                .as_deref()
                .map_or((None, None), path_fields);
            OutputDiagnostic {
                code: format!("{:?}", diagnostic.code).to_ascii_lowercase(),
                severity: OutputSeverity::Error,
                message: diagnostic.detail.clone(),
                path_utf8,
                path_bytes_hex,
                remediation: Some(
                    "Resolve the legacy lock or destination mismatch before adoption.".to_owned(),
                ),
            }
        }));
    if !adoption.applicable {
        return envelope;
    }
    if arguments.confirmation.dry_run {
        envelope.data = json!({ "adoptable": adoption.entries.len(), "applied": false });
        return envelope;
    }
    if !arguments.confirmation.yes {
        if presentation == Presentation::NonInteractive {
            return Envelope::usage(
                Some("adopt"),
                "non-interactive mutation requires --yes after reviewing the plan",
            );
        }
        let mut app = App::with_selection(catalog.skill_count(), &providers);
        app.set_review(Review::from_plan(
            &adoption_plan,
            &transition.notices,
            &roots,
        ));
        let decision = match presentation {
            Presentation::Tui => ui::confirm_plan(app, signals).map(|exit| match exit {
                UiExit::Confirmed => PlainExit::Confirmed,
                UiExit::Interrupted(code) => PlainExit::Interrupted(code),
                UiExit::Cancelled | UiExit::Selected(_) => PlainExit::Cancelled,
            }),
            Presentation::PlainInteractive => plain::confirm_plan(
                app,
                &mut io::stdin().lock(),
                &mut io::stdout().lock(),
                signals,
            ),
            Presentation::NonInteractive => unreachable!(),
        };
        match decision {
            Ok(PlainExit::Confirmed) => {}
            Ok(PlainExit::Interrupted(code)) => {
                return transaction_error(
                    "adopt",
                    providers,
                    code,
                    "interrupted before mutation".to_owned(),
                );
            }
            Ok(PlainExit::Cancelled | PlainExit::Selected(_)) => return cancelled("adopt"),
            Err(error) => {
                return transaction_error(
                    "adopt",
                    providers,
                    TRANSACTION_EXIT_CODE,
                    error.to_string(),
                );
            }
        }
    }
    let base_receipt = current.unwrap_or_else(|| {
        Receipt::new(
            env!("CARGO_PKG_VERSION"),
            &catalog.manifest().catalog_sha256,
            &roots,
        )
    });
    let next_receipt = match engine::receipt_after_adoption(&base_receipt, &roots, &adoption) {
        Ok(receipt) => receipt,
        Err(error) => return lifecycle_error("adopt", providers, error.to_string()),
    };
    let transaction_id = match transaction_id() {
        Ok(id) => id,
        Err(message) => {
            return transaction_error("adopt", providers, TRANSACTION_EXIT_CODE, message);
        }
    };
    let operations = match operations_for_adoption(
        &lock_path,
        &adoption,
        &roots,
        &next_receipt,
        &transaction_id,
    ) {
        Ok(operations) => operations,
        Err(error) => {
            return transaction_error("adopt", providers, TRANSACTION_EXIT_CODE, error.to_string());
        }
    };
    if !cli.json
        && let Err(error) = writeln!(
            io::stdout().lock(),
            "Progress: adopting {} verified entries",
            adoption.entries.len()
        )
    {
        return transaction_error("adopt", providers, TRANSACTION_EXIT_CODE, error.to_string());
    }
    let engine = TransactionEngine::new(roots.state_directory, signals.clone());
    match engine.apply(transaction_id.clone(), operations) {
        Ok(TransactionOutcome::Committed) => {
            envelope.transaction_id = Some(transaction_id);
            envelope.data = json!({ "adopted": adoption.entries.len(), "applied": true });
            envelope
        }
        Ok(outcome) => transaction_error(
            "adopt",
            providers,
            TRANSACTION_EXIT_CODE,
            format!("unexpected adoption outcome: {outcome:?}"),
        ),
        Err(error) => transaction_error("adopt", providers, error.exit_code(), error.to_string()),
    }
}

fn run_status(catalog: &Catalog, command: &str) -> Envelope {
    let (roots, receipt) = match context(&[], command) {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let Some(receipt) = receipt else {
        let mut envelope = Envelope::new(Some(command));
        envelope.status = OutputStatus::Noop;
        envelope.data = json!({
            "installed": false,
            "receipt": roots.receipt_path,
            "catalog_sha256": catalog.manifest().catalog_sha256,
        });
        return envelope;
    };
    let providers = managed_providers(&receipt);
    let mut envelope = Envelope::new(Some(command));
    envelope.providers = providers;
    envelope.transaction_id.clone_from(&receipt.transaction_id);
    if receipt.state == ReceiptState::RecoveryRequired {
        envelope.status = OutputStatus::RecoveryRequired;
        envelope.exit_code = TRANSACTION_EXIT_CODE;
        envelope.diagnostics.push(OutputDiagnostic::error(
            "recovery_required",
            "the receipt requires recovery",
            Some("Run recover before another mutation.".to_owned()),
        ));
    }
    envelope.data = json!({
        "installed": true,
        "state": receipt.state,
        "assets": receipt.assets.len(),
        "receipt": roots.receipt_path,
        "catalog_sha256": receipt.catalog_sha256,
    });
    envelope
}

fn run_doctor(catalog: &Catalog) -> Envelope {
    let (roots, receipt) = match context(&[], "doctor") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let engine = TransactionEngine::new(roots.state_directory.clone(), SignalFlags::default());
    let journal = match engine.journal_state() {
        Ok(journal) => journal,
        Err(error) => {
            return transaction_error("doctor", Vec::new(), error.exit_code(), error.to_string());
        }
    };
    let receipt_requires_recovery = receipt
        .as_ref()
        .is_some_and(|receipt| receipt.state == ReceiptState::RecoveryRequired);
    let healthy = receipt.is_some() && journal.is_none() && !receipt_requires_recovery;
    let mut envelope = Envelope::new(Some("doctor"));
    if let Some(receipt) = &receipt {
        envelope.providers = managed_providers(receipt);
        envelope.transaction_id.clone_from(&receipt.transaction_id);
    }
    if !healthy {
        envelope.status = if journal.is_some() || receipt_requires_recovery {
            OutputStatus::RecoveryRequired
        } else {
            OutputStatus::Blocked
        };
        envelope.exit_code = CONFLICT_EXIT_CODE;
        envelope.diagnostics.push(OutputDiagnostic::error(
            if journal.is_some() || receipt_requires_recovery {
                "recovery_required"
            } else {
                "not_installed"
            },
            if journal.is_some() || receipt_requires_recovery {
                "the durable state requires recovery"
            } else {
                "no Arthur Workflow receipt exists"
            },
            Some(if journal.is_some() || receipt_requires_recovery {
                "Run recover before another mutation.".to_owned()
            } else {
                "Run install with an explicit provider selection.".to_owned()
            }),
        ));
    }
    envelope.data = json!({
        "healthy": healthy,
        "journal_state": journal,
        "catalog_sha256": catalog.manifest().catalog_sha256,
    });
    envelope
}

fn run_recover(catalog: &Catalog, signals: &SignalFlags) -> Envelope {
    let (_, current) = match context(&[], "recover") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let managed = current.as_ref().map_or_else(Vec::new, managed_providers);
    let (roots, _) = match context(&ProviderId::ALL, "recover") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    let trusted = trusted_roots(&roots);
    let engine = TransactionEngine::new(roots.state_directory.clone(), signals.clone());
    match engine.journal_state() {
        Ok(None) => {
            let mut envelope = Envelope::new(Some("recover"));
            envelope.status = OutputStatus::Noop;
            envelope.providers.clone_from(&managed);
            envelope.data = json!({ "outcome": "no_journal" });
            return envelope;
        }
        Ok(Some(_)) => {}
        Err(error) => {
            return transaction_error("recover", managed, error.exit_code(), error.to_string());
        }
    }
    match engine.recover(&trusted) {
        Ok(outcome) => {
            let mut envelope = Envelope::new(Some("recover"));
            envelope.providers = managed;
            envelope.data = json!({
                "outcome": format!("{outcome:?}").to_ascii_lowercase(),
                "catalog_sha256": catalog.manifest().catalog_sha256,
            });
            envelope
        }
        Err(error) => transaction_error("recover", managed, error.exit_code(), error.to_string()),
    }
}

fn apply_transition(
    request: ApplyRequest<'_>,
    roots: ResolvedRoots,
    transition: LifecycleTransition,
) -> Envelope {
    let ApplyRequest {
        catalog,
        cli,
        command,
        confirmation,
        presentation,
        signals,
    } = request;
    let mut envelope = report_transition(command, transition.clone());
    if confirmation.dry_run || !transition.plan.applicable {
        return envelope;
    }
    if transition
        .plan
        .entries
        .iter()
        .all(|entry| entry.action == plan::PlanAction::Noop)
    {
        envelope.status = OutputStatus::Noop;
        envelope.data = json!({ "applied": false, "result": "already_current" });
        return envelope;
    }
    if !confirmation.yes {
        if presentation == Presentation::NonInteractive {
            return Envelope::usage(
                Some(command),
                "non-interactive mutation requires --yes after reviewing the plan",
            );
        }
        let mut app = App::with_selection(catalog.skill_count(), &transition.selected_providers);
        app.set_review(Review::from_plan(
            &transition.plan,
            &transition.notices,
            &roots,
        ));
        let decision = match presentation {
            Presentation::Tui => ui::confirm_plan(app, signals).map(|exit| match exit {
                UiExit::Confirmed => PlainExit::Confirmed,
                UiExit::Interrupted(code) => PlainExit::Interrupted(code),
                UiExit::Cancelled | UiExit::Selected(_) => PlainExit::Cancelled,
            }),
            Presentation::PlainInteractive => plain::confirm_plan(
                app,
                &mut io::stdin().lock(),
                &mut io::stdout().lock(),
                signals,
            ),
            Presentation::NonInteractive => unreachable!(),
        };
        match decision {
            Ok(PlainExit::Confirmed) => {}
            Ok(PlainExit::Interrupted(code)) => {
                return transaction_error(
                    command,
                    transition.selected_providers,
                    code,
                    "interrupted before mutation".to_owned(),
                );
            }
            Ok(PlainExit::Cancelled | PlainExit::Selected(_)) => {
                envelope.status = OutputStatus::Noop;
                envelope.data = json!({ "applied": false, "reason": "cancelled before mutation" });
                return envelope;
            }
            Err(error) => {
                return transaction_error(
                    command,
                    transition.selected_providers,
                    TRANSACTION_EXIT_CODE,
                    error.to_string(),
                );
            }
        }
    }

    let transaction_id = match transaction_id() {
        Ok(id) => id,
        Err(message) => {
            return transaction_error(
                command,
                transition.selected_providers,
                TRANSACTION_EXIT_CODE,
                message,
            );
        }
    };
    let operations = match operations_for_plan(
        &transition.plan,
        &roots,
        &transition.receipt,
        &transaction_id,
    ) {
        Ok(operations) => operations,
        Err(error) => {
            return transaction_error(
                command,
                transition.selected_providers,
                TRANSACTION_EXIT_CODE,
                error.to_string(),
            );
        }
    };
    if !cli.json
        && let Err(error) = writeln!(
            io::stdout().lock(),
            "Progress: applying {} transactional operations",
            operations.len()
        )
    {
        return transaction_error(
            command,
            transition.selected_providers,
            TRANSACTION_EXIT_CODE,
            error.to_string(),
        );
    }
    let engine = TransactionEngine::new(roots.state_directory, signals.clone());
    match engine.apply(transaction_id.clone(), operations) {
        Ok(TransactionOutcome::Committed) => {
            envelope.transaction_id = Some(transaction_id);
            envelope.data = json!({ "applied": true, "result": "committed" });
            envelope
        }
        Ok(outcome) => transaction_error(
            command,
            transition.selected_providers,
            TRANSACTION_EXIT_CODE,
            format!("unexpected apply outcome: {outcome:?}"),
        ),
        Err(error) => transaction_error(
            command,
            transition.selected_providers,
            error.exit_code(),
            error.to_string(),
        ),
    }
}

fn providers_or_interactive(
    catalog: &Catalog,
    providers: Vec<ProviderId>,
    presentation: Presentation,
    command: &str,
    signals: &SignalFlags,
) -> CommandResult<Vec<ProviderId>> {
    if !providers.is_empty() {
        return Ok(providers);
    }
    if presentation == Presentation::NonInteractive {
        return Err(Box::new(Envelope::usage(
            Some(command),
            "provider selection is required; pass --provider <claude|codex>",
        )));
    }
    let detected = context(&[], command)
        .ok()
        .and_then(|(_, receipt)| receipt)
        .map_or_else(Vec::new, |receipt| managed_providers(&receipt));
    let app = App::new(catalog.skill_count(), &detected);
    match presentation {
        Presentation::Tui => match ui::select_providers(app, signals) {
            Ok(UiExit::Selected(providers)) => Ok(providers),
            Ok(UiExit::Interrupted(code)) => Err(Box::new(transaction_error(
                command,
                Vec::new(),
                code,
                "interrupted before mutation".to_owned(),
            ))),
            Ok(UiExit::Cancelled | UiExit::Confirmed) => Err(Box::new(cancelled(command))),
            Err(error) => Err(Box::new(transaction_error(
                command,
                Vec::new(),
                TRANSACTION_EXIT_CODE,
                error.to_string(),
            ))),
        },
        Presentation::PlainInteractive => match plain::select_providers(
            app,
            &mut io::stdin().lock(),
            &mut io::stdout().lock(),
            signals,
        ) {
            Ok(PlainExit::Selected(providers)) => Ok(providers),
            Ok(PlainExit::Interrupted(code)) => Err(Box::new(transaction_error(
                command,
                Vec::new(),
                code,
                "interrupted before mutation".to_owned(),
            ))),
            Ok(PlainExit::Cancelled | PlainExit::Confirmed) => Err(Box::new(cancelled(command))),
            Err(error) => Err(Box::new(transaction_error(
                command,
                Vec::new(),
                TRANSACTION_EXIT_CODE,
                error.to_string(),
            ))),
        },
        Presentation::NonInteractive => unreachable!(),
    }
}

fn adoption_entries(
    transition: &LifecycleTransition,
    roots: &ResolvedRoots,
) -> Result<Vec<CatalogEntry>, String> {
    let mut entries = Vec::new();
    for entry in transition
        .plan
        .entries
        .iter()
        .filter(|entry| entry.action == plan::PlanAction::Adoptable)
    {
        let Some(source_id) = adoption_source_id(&entry.destination, roots) else {
            continue;
        };
        let snapshot = snapshot_path(&entry.destination).map_err(|error| error.to_string())?;
        let entry_type = match snapshot.kind {
            PathKind::File => EntryType::File,
            PathKind::Directory => EntryType::Directory,
            PathKind::Symlink => EntryType::Symlink,
            PathKind::Absent => {
                return Err(format!(
                    "adoptable destination changed type: {}",
                    entry.destination.display()
                ));
            }
        };
        let metadata = fs::symlink_metadata(&entry.destination)
            .map_err(|error| format!("cannot inspect {}: {error}", entry.destination.display()))?;
        entries.push(CatalogEntry {
            source_id,
            destination: entry.destination.clone(),
            entry_type,
            sha256: snapshot.sha256,
            mode: metadata.permissions().mode() & 0o7777,
            link_target: snapshot.link_target,
        });
    }
    entries.sort_by(|left, right| {
        left.source_id
            .cmp(&right.source_id)
            .then(left.destination.cmp(&right.destination))
    });
    Ok(entries)
}

fn adoption_source_id(destination: &Path, roots: &ResolvedRoots) -> Option<String> {
    let mut skill_roots = vec![roots.canonical_skills.as_path()];
    if let Some(claude_skills) = roots
        .provider(ProviderId::Claude)
        .and_then(|provider| provider.skills.as_deref())
    {
        skill_roots.push(claude_skills);
    }
    skill_roots.into_iter().find_map(|root| {
        destination
            .strip_prefix(root)
            .ok()
            .and_then(|relative| relative.components().next())
            .and_then(|component| component.as_os_str().to_str())
            .map(str::to_owned)
    })
}

fn context(
    selected: &[ProviderId],
    command: &str,
) -> CommandResult<(ResolvedRoots, Option<Receipt>)> {
    let base = resolve_roots(&[]).map_err(|error| Box::new(environment_error(command, &error)))?;
    let receipt = read_receipt(&base, command)?;
    let mut required = selected.iter().copied().collect::<BTreeSet<_>>();
    if let Some(receipt) = &receipt {
        required.extend(managed_providers(receipt));
    }
    let roots = resolve_roots(&required.into_iter().collect::<Vec<_>>())
        .map_err(|error| Box::new(environment_error(command, &error)))?;
    Ok((roots, receipt))
}

fn read_receipt(roots: &ResolvedRoots, command: &str) -> CommandResult<Option<Receipt>> {
    match fs::read(&roots.receipt_path) {
        Ok(bytes) => Receipt::decode(&bytes).map(Some).map_err(|error| {
            Box::new(transaction_error(
                command,
                Vec::new(),
                TRANSACTION_EXIT_CODE,
                error.to_string(),
            ))
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Box::new(transaction_error(
            command,
            Vec::new(),
            TRANSACTION_EXIT_CODE,
            format!("cannot read {}: {error}", roots.receipt_path.display()),
        ))),
    }
}

fn managed_providers(receipt: &Receipt) -> Vec<ProviderId> {
    receipt
        .providers
        .iter()
        .filter(|provider| provider.managed_integration)
        .map(|provider| provider.provider)
        .collect()
}

fn environment_error(command: &str, error: &ResolveError) -> Envelope {
    let path = match error {
        ResolveError::NonUtf8Path { path, .. }
        | ResolveError::NotAbsolute { path, .. }
        | ResolveError::EscapesFilesystemRoot { path, .. }
        | ResolveError::NotDirectory { path, .. }
        | ResolveError::Inaccessible { path, .. } => Some(path),
        ResolveError::UnsupportedPlatform
        | ResolveError::MissingHome
        | ResolveError::EmptyPath { .. } => None,
    };
    let mut envelope = Envelope::new(Some(command));
    envelope.status = OutputStatus::Failed;
    envelope.exit_code = ENVIRONMENT_EXIT_CODE;
    envelope.diagnostics.push(
        OutputDiagnostic::error(
            "environment_invalid",
            error.to_string(),
            Some("Set accessible absolute HOME and CODEX_HOME paths, then retry.".to_owned()),
        )
        .with_path(
            path.and_then(|path| path.path_utf8.clone()),
            path.and_then(|path| path.path_bytes_hex.clone()),
        ),
    );
    envelope
}

fn report_transition(command: &str, transition: LifecycleTransition) -> Envelope {
    let mut envelope = Envelope::new(Some(command)).with_plan(&transition.plan);
    envelope.providers = transition.selected_providers;
    envelope.diagnostics.extend(
        transition
            .notices
            .into_iter()
            .map(|notice| OutputDiagnostic {
                code: format!("{:?}", notice.code).to_ascii_lowercase(),
                severity: OutputSeverity::Warning,
                message: notice.message,
                path_utf8: None,
                path_bytes_hex: None,
                remediation: None,
            }),
    );
    envelope
}

fn lifecycle_error(command: &str, providers: Vec<ProviderId>, message: String) -> Envelope {
    let mut envelope = Envelope::failure(
        Some(command),
        OutputStatus::Blocked,
        CONFLICT_EXIT_CODE,
        "plan_blocked",
        message,
    );
    envelope.providers = providers;
    envelope
}

fn transaction_error(
    command: &str,
    providers: Vec<ProviderId>,
    exit_code: u8,
    message: String,
) -> Envelope {
    let status = if message.contains("requires recover") || message.contains("recovery") {
        OutputStatus::RecoveryRequired
    } else {
        OutputStatus::Failed
    };
    let mut envelope = Envelope::failure(
        Some(command),
        status,
        exit_code,
        "transaction_failed",
        message,
    );
    envelope.providers = providers;
    envelope
}

fn cancelled(command: &str) -> Envelope {
    let mut envelope = Envelope::new(Some(command));
    envelope.status = OutputStatus::Noop;
    envelope.data = json!({ "applied": false, "reason": "cancelled before mutation" });
    envelope
}

fn trusted_roots(roots: &ResolvedRoots) -> Vec<RootSpec> {
    roots
        .allowed_top_level_roots()
        .map(|identity| {
            let id = if identity == &roots.canonical {
                "canonical"
            } else {
                roots
                    .providers
                    .iter()
                    .find(|provider| provider.root == *identity)
                    .map_or("provider", |provider| provider.id.as_str())
            };
            RootSpec::new(id, identity.lexical.clone(), identity.device)
                .with_real(identity.real.clone())
        })
        .collect()
}

fn transaction_id() -> Result<String, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?;
    Ok(format!("txn-{}-{}", std::process::id(), elapsed.as_nanos()))
}

fn presentation(cli: &Cli) -> Presentation {
    let interactive = !cli.json
        && std::env::var("CI").as_deref() != Ok("true")
        && io::stdin().is_terminal()
        && io::stdout().is_terminal();
    if !interactive {
        return Presentation::NonInteractive;
    }
    let plain_environment =
        std::env::var_os("ARTHUR_SKILLS_PLAIN").as_deref() == Some(std::ffi::OsStr::new("1"));
    let dumb = std::env::var_os("TERM").as_deref() == Some(std::ffi::OsStr::new("dumb"));
    if cli.plain || plain_environment || dumb {
        Presentation::PlainInteractive
    } else {
        Presentation::Tui
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{
        Presentation, adoption_entries, adoption_source_id, cancelled, environment_error,
        lifecycle_error, transaction_error, trusted_roots,
    };
    use crate::lifecycle::LifecycleTransition;
    use crate::output::OutputStatus;
    use crate::plan::{Owner, Plan, PlanAction, PlanEntry};
    use crate::provider::{PathDiagnostic, ProviderId, ResolveError, resolve_roots_from};
    use crate::receipt::Receipt;
    use crate::should_use_tui;

    #[test]
    fn tui_selection_requires_compatible_terminal_streams() {
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
        assert_ne!(Presentation::Tui, Presentation::NonInteractive);
    }

    #[test]
    fn command_error_envelopes_keep_closed_statuses_and_path_details() {
        let recovery = transaction_error(
            "install",
            vec![ProviderId::Claude],
            6,
            "transaction requires recovery".to_owned(),
        );
        assert_eq!(recovery.status, OutputStatus::RecoveryRequired);
        let failed = transaction_error("install", Vec::new(), 6, "disk full".to_owned());
        assert_eq!(failed.status, OutputStatus::Failed);
        assert_eq!(cancelled("install").status, OutputStatus::Noop);
        let blocked = lifecycle_error(
            "install",
            vec![ProviderId::Claude],
            "foreign destination".to_owned(),
        );
        assert_eq!(blocked.status, OutputStatus::Blocked);
        assert_eq!(blocked.providers, vec![ProviderId::Claude]);

        let missing = environment_error("status", &ResolveError::MissingHome);
        assert!(missing.diagnostics[0].path_utf8.is_none());
        let wrong_kind = environment_error(
            "status",
            &ResolveError::NotDirectory {
                variable: "HOME",
                path: PathDiagnostic {
                    path_utf8: Some("/tmp/home".to_owned()),
                    path_bytes_hex: None,
                },
            },
        );
        assert_eq!(
            wrong_kind.diagnostics[0].path_utf8.as_deref(),
            Some("/tmp/home")
        );
    }

    #[test]
    fn adoption_helpers_limit_sources_and_recovery_to_resolved_roots()
    -> Result<(), Box<dyn std::error::Error>> {
        let home = tempfile::tempdir()?;
        let roots = resolve_roots_from(
            Some(home.path().as_os_str()),
            None,
            &[ProviderId::Claude, ProviderId::Codex],
        )?;
        assert_eq!(
            adoption_source_id(&roots.canonical_skills.join("meta-code/SKILL.md"), &roots),
            Some("meta-code".to_owned())
        );
        assert_eq!(
            adoption_source_id(&home.path().join("foreign"), &roots),
            None
        );
        assert_eq!(trusted_roots(&roots).len(), 3);

        let destination = roots.canonical_skills.join("missing/SKILL.md");
        let transition = LifecycleTransition {
            selected_providers: vec![ProviderId::Claude],
            plan: Plan {
                schema_version: 1,
                applicable: true,
                entries: vec![PlanEntry {
                    action: PlanAction::Adoptable,
                    source: "skills/missing/SKILL.md".to_owned(),
                    destination,
                    owner: Owner::Unmanaged,
                    reason: "matching unmanaged asset".to_owned(),
                }],
                operations: Vec::new(),
                diagnostics: Vec::new(),
            },
            receipt: Receipt::new("0.1.0", "hash", &roots),
            notices: Vec::new(),
        };
        assert!(
            adoption_entries(&transition, &roots)
                .is_err_and(|error| error.contains("changed type"))
        );
        Ok(())
    }
}
