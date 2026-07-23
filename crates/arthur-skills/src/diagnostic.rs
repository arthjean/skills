use std::fs;
use std::io;

use serde_json::json;

use crate::catalog::Catalog;
use crate::health::{
    HealthIssue, InstallationHealth, IssueSeverity, inspect_doctor, inspect_status,
};
use crate::output::{
    CONFLICT_EXIT_CODE, Envelope, OutputDiagnostic, OutputSeverity, OutputStatus, path_fields,
};
use crate::provider::{
    ENVIRONMENT_EXIT_CODE, ProviderId, ResolveError, ResolvedRoots, resolve_roots,
};
use crate::receipt::{Receipt, ReceiptState};
use crate::transaction::{SignalFlags, TRANSACTION_EXIT_CODE, TransactionEngine};

struct ReadonlyContext {
    roots: ResolvedRoots,
    receipt: Option<Receipt>,
    receipt_error: Option<String>,
}

pub fn status(catalog: &Catalog) -> Envelope {
    let readonly = match readonly_context("status") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    if let Some(error) = readonly.receipt_error {
        return invalid_receipt_report(catalog, &readonly.roots, "status", error);
    }
    let Some(receipt) = readonly.receipt else {
        let mut envelope = Envelope::new(Some("status"));
        envelope.status = OutputStatus::Noop;
        envelope.data = json!({
            "installed": false,
            "cli_version": env!("CARGO_PKG_VERSION"),
            "catalog_version": env!("CARGO_PKG_VERSION"),
            "receipt": readonly.roots.receipt_path,
            "catalog_sha256": catalog.manifest().catalog_sha256,
        });
        return envelope;
    };
    let health = inspect_status(catalog, &readonly.roots, &receipt);
    let mut envelope = Envelope::new(Some("status"));
    envelope.providers = managed_providers(&receipt);
    envelope.transaction_id.clone_from(&receipt.transaction_id);
    append_health_diagnostics(&mut envelope, &health);
    if receipt.state == ReceiptState::RecoveryRequired {
        envelope.status = OutputStatus::RecoveryRequired;
        envelope.exit_code = TRANSACTION_EXIT_CODE;
        envelope.diagnostics.push(OutputDiagnostic::error(
            "recovery_required",
            "the receipt requires recovery",
            Some("Run recover before another mutation.".to_owned()),
        ));
    } else if !health.roots_match {
        envelope.status = OutputStatus::Blocked;
        envelope.exit_code = ENVIRONMENT_EXIT_CODE;
    }
    envelope.data = json!({
        "installed": true,
        "state": receipt.state,
        "cli_version": env!("CARGO_PKG_VERSION"),
        "installed_cli_version": receipt.cli_version,
        "catalog_version": env!("CARGO_PKG_VERSION"),
        "installed_catalog_sha256": receipt.catalog_sha256,
        "embedded_catalog_sha256": catalog.manifest().catalog_sha256,
        "catalog_current": health.catalog_current,
        "roots_match": health.roots_match,
        "roots": {
            "recorded": receipt.roots,
            "current_home": readonly.roots.home,
            "current_canonical": readonly.roots.canonical,
            "current_providers": readonly.roots.providers.iter().map(|provider| json!({
                "provider": provider.id,
                "root": &provider.root,
            })).collect::<Vec<_>>(),
        },
        "providers": receipt.providers,
        "implicit_codex_visibility": receipt.assets.iter().map(|asset| &asset.destination)
            .chain(receipt.retained_unmanaged.iter().map(|asset| &asset.destination))
            .any(|destination| {
                destination.starts_with(receipt.roots.canonical.lexical.join("skills"))
            }),
        "counts": health.counts,
        "retained_unmanaged": receipt.retained_unmanaged,
        "receipt": readonly.roots.receipt_path,
    });
    envelope
}

pub fn doctor(catalog: &Catalog) -> Envelope {
    let readonly = match readonly_context("doctor") {
        Ok(context) => context,
        Err(envelope) => return *envelope,
    };
    if let Some(error) = readonly.receipt_error {
        return invalid_receipt_report(catalog, &readonly.roots, "doctor", error);
    }
    let engine = TransactionEngine::new(
        readonly.roots.state_directory.clone(),
        SignalFlags::default(),
    );
    let journal = match engine.journal_state() {
        Ok(journal) => journal,
        Err(error) => {
            return Envelope::failure(
                Some("doctor"),
                OutputStatus::Failed,
                error.exit_code(),
                "journal_invalid",
                error.to_string(),
            );
        }
    };
    let receipt_requires_recovery = readonly
        .receipt
        .as_ref()
        .is_some_and(|receipt| receipt.state == ReceiptState::RecoveryRequired);
    let health = readonly
        .receipt
        .as_ref()
        .map(|receipt| inspect_doctor(catalog, &readonly.roots, receipt));
    let healthy = health.as_ref().is_some_and(|health| health.healthy)
        && journal.is_none()
        && !receipt_requires_recovery;
    let mut envelope = Envelope::new(Some("doctor"));
    if let Some(receipt) = &readonly.receipt {
        envelope.providers = managed_providers(receipt);
        envelope.transaction_id.clone_from(&receipt.transaction_id);
    }
    if let Some(health) = &health {
        append_health_diagnostics(&mut envelope, health);
    }
    if !healthy {
        let recovery = journal.is_some() || receipt_requires_recovery;
        let root_mismatch = health.as_ref().is_some_and(|health| !health.roots_match);
        envelope.status = if recovery {
            OutputStatus::RecoveryRequired
        } else {
            OutputStatus::Blocked
        };
        envelope.exit_code = if recovery {
            TRANSACTION_EXIT_CODE
        } else if root_mismatch {
            ENVIRONMENT_EXIT_CODE
        } else {
            CONFLICT_EXIT_CODE
        };
        if readonly.receipt.is_none() || recovery {
            envelope.diagnostics.push(OutputDiagnostic::error(
                if recovery {
                    "recovery_required"
                } else {
                    "not_installed"
                },
                if recovery {
                    "the durable state requires recovery"
                } else {
                    "no Arthur Workflow receipt exists"
                },
                Some(if recovery {
                    "Run recover before another mutation.".to_owned()
                } else {
                    "Run install with an explicit provider selection.".to_owned()
                }),
            ));
        }
    }
    envelope.data = json!({
        "healthy": healthy,
        "journal_state": journal,
        "cli_version": env!("CARGO_PKG_VERSION"),
        "catalog_version": env!("CARGO_PKG_VERSION"),
        "catalog_sha256": catalog.manifest().catalog_sha256,
        "checks": health,
    });
    envelope
}

fn readonly_context(command: &str) -> Result<ReadonlyContext, Box<Envelope>> {
    let base = resolve_roots(&[]).map_err(|error| Box::new(environment_error(command, &error)))?;
    let bytes = match fs::read(&base.receipt_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ReadonlyContext {
                roots: base,
                receipt: None,
                receipt_error: None,
            });
        }
        Err(error) => {
            return Ok(ReadonlyContext {
                receipt_error: Some(format!(
                    "cannot read {}: {error}",
                    base.receipt_path.display()
                )),
                roots: base,
                receipt: None,
            });
        }
    };
    let receipt = match Receipt::decode(&bytes) {
        Ok(receipt) => receipt,
        Err(error) => {
            return Ok(ReadonlyContext {
                roots: base,
                receipt: None,
                receipt_error: Some(error.to_string()),
            });
        }
    };
    let roots = resolve_roots(&managed_providers(&receipt))
        .map_err(|error| Box::new(environment_error(command, &error)))?;
    Ok(ReadonlyContext {
        roots,
        receipt: Some(receipt),
        receipt_error: None,
    })
}

fn append_health_diagnostics(envelope: &mut Envelope, health: &InstallationHealth) {
    envelope
        .diagnostics
        .extend(health.issues.iter().map(output_health_issue));
}

fn output_health_issue(issue: &HealthIssue) -> OutputDiagnostic {
    let (path_utf8, path_bytes_hex) = issue.path.as_deref().map_or((None, None), path_fields);
    let remediation = match issue.code.as_str() {
        "root_mismatch" => Some(
            "Restore HOME and CODEX_HOME to the recorded roots before any mutation.".to_owned(),
        ),
        "catalog_update_available" => Some(
            "Acquire the intended arthur-skills binary, then run update; v1 never self-updates."
                .to_owned(),
        ),
        "asset_inspection_failed" | "surface_unreadable" => {
            Some("Restore read access and run doctor again.".to_owned())
        }
        "asset_missing"
        | "asset_type_conflict"
        | "asset_drifted"
        | "catalog_reconciliation_required" => {
            Some("Restore the managed asset or run update after resolving local drift.".to_owned())
        }
        "provider_incompatible" => {
            Some("Install a provider version at or above the validated minimum.".to_owned())
        }
        "provider_cli_missing" => Some(
            "Install the provider CLI or add its trusted executable directory to PATH.".to_owned(),
        ),
        "capability_missing" => Some(
            "Install the capability command or remove it from the catalog contract.".to_owned(),
        ),
        "capability_untrusted" => Some(
            "Remove unsafe PATH entries or repair their ownership and permissions before retrying."
                .to_owned(),
        ),
        _ => None,
    };
    OutputDiagnostic {
        code: issue.code.clone(),
        severity: match issue.severity {
            IssueSeverity::Warning => OutputSeverity::Warning,
            IssueSeverity::Error => OutputSeverity::Error,
        },
        message: issue.message.clone(),
        path_utf8,
        path_bytes_hex,
        remediation,
    }
}

fn invalid_receipt_report(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    command: &str,
    detail: String,
) -> Envelope {
    let engine = TransactionEngine::new(roots.state_directory.clone(), SignalFlags::default());
    let journal = engine.journal_state();
    let recoverable = matches!(journal, Ok(Some(_)));
    let mut envelope = Envelope::failure(
        Some(command),
        if recoverable {
            OutputStatus::RecoveryRequired
        } else {
            OutputStatus::Failed
        },
        TRANSACTION_EXIT_CODE,
        "receipt_invalid",
        detail,
    );
    if let Err(error) = &journal {
        envelope.diagnostics.push(OutputDiagnostic::error(
            "journal_invalid",
            error.to_string(),
            Some(
                "Preserve the state directory and inspect its backups before mutation.".to_owned(),
            ),
        ));
    } else if recoverable {
        envelope.diagnostics.push(OutputDiagnostic::error(
            "recovery_required",
            "a durable transaction journal is available",
            Some("Run recover before any new mutation.".to_owned()),
        ));
    }
    envelope.data = json!({
        "installed": true,
        "receipt_readable": false,
        "receipt": roots.receipt_path,
        "journal_state": journal.ok().flatten(),
        "cli_version": env!("CARGO_PKG_VERSION"),
        "catalog_version": env!("CARGO_PKG_VERSION"),
        "catalog_sha256": catalog.manifest().catalog_sha256,
        "recover_available": recoverable,
    });
    envelope
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
