use std::fmt;
use std::path::{Path, PathBuf};

use crate::adoption::{AdoptionPlan, LegacyImportPlan, LockIdentity};
use crate::plan::{
    MutationKind, NodeKind, OwnershipProof as PlanOwnership, Plan, PlanAction, PlannedInverse,
    PlannedMutation, Precondition as PlanPrecondition,
};
use crate::platform::effective_file_mode;
use crate::provider::{ResolvedRoots, RootIdentity};
use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt, ReceiptState};
use crate::transaction::{
    FileIdentityProof, Inverse, Operation, OperationKind, OperationPayload, OwnershipProof,
    PathKind, PathSnapshot, RootSpec, TransactionError, snapshot_path,
};

#[derive(Debug)]
pub enum OperationBuildError {
    PlanBlocked,
    AdoptionBlocked,
    UnknownRoot(PathBuf),
    MissingPayload(String),
    StalePlan(PathBuf),
    InvalidReceipt(String),
    ReceiptCoverage {
        destination: PathBuf,
        detail: &'static str,
    },
    Transaction(Box<TransactionError>),
}

impl fmt::Display for OperationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanBlocked => formatter.write_str("blocked plan cannot reach the executor"),
            Self::AdoptionBlocked => {
                formatter.write_str("blocked adoption cannot reach the executor")
            }
            Self::UnknownRoot(path) => {
                write!(
                    formatter,
                    "operation root is not registered: {}",
                    path.display()
                )
            }
            Self::MissingPayload(id) => write!(formatter, "operation {id} has no file payload"),
            Self::StalePlan(path) => {
                write!(
                    formatter,
                    "filesystem changed after planning: {}",
                    path.display()
                )
            }
            Self::InvalidReceipt(detail) => write!(formatter, "cannot encode receipt: {detail}"),
            Self::ReceiptCoverage {
                destination,
                detail,
            } => write!(
                formatter,
                "receipt does not cover {}: {detail}",
                destination.display()
            ),
            Self::Transaction(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for OperationBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transaction(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TransactionError> for OperationBuildError {
    fn from(error: TransactionError) -> Self {
        Self::Transaction(Box::new(error))
    }
}

pub fn operations_for_plan(
    plan: &Plan,
    roots: &ResolvedRoots,
    next_receipt: &Receipt,
    transaction_id: &str,
) -> Result<Vec<Operation>, OperationBuildError> {
    if !plan.applicable {
        return Err(OperationBuildError::PlanBlocked);
    }
    validate_plan_receipt_coverage(plan, next_receipt)?;
    let mut operations = plan
        .operations
        .iter()
        .enumerate()
        .map(|(index, mutation)| operation_for_mutation(index, mutation, roots))
        .collect::<Result<Vec<_>, _>>()?;
    let receipt = receipt_operation(roots, next_receipt, transaction_id)?;
    if receipt.precondition != receipt.expected_after {
        operations.push(receipt);
    }
    Ok(operations)
}

pub fn operations_for_adoption(
    lock_path: &Path,
    adoption: &AdoptionPlan,
    roots: &ResolvedRoots,
    next_receipt: &Receipt,
    transaction_id: &str,
) -> Result<Vec<Operation>, OperationBuildError> {
    if !adoption.applicable {
        return Err(OperationBuildError::AdoptionBlocked);
    }
    validate_adoption_receipt_coverage(adoption, next_receipt)?;
    let mut operations = legacy_lock_operations(
        lock_path,
        &adoption.archive_path,
        &adoption.original_identity,
        &adoption.original_bytes,
        &adoption.original_hash,
        &adoption.residual_bytes,
        roots,
    )?;
    operations.push(receipt_operation(roots, next_receipt, transaction_id)?);
    Ok(operations)
}

pub fn operations_for_import(
    plan: &Plan,
    lock_path: &Path,
    legacy: Option<&LegacyImportPlan>,
    roots: &ResolvedRoots,
    next_receipt: &Receipt,
    transaction_id: &str,
) -> Result<Vec<Operation>, OperationBuildError> {
    if !plan.applicable {
        return Err(OperationBuildError::PlanBlocked);
    }
    validate_plan_receipt_coverage(plan, next_receipt)?;
    let mut operations = match legacy {
        Some(legacy) => legacy_lock_operations(
            lock_path,
            &legacy.archive_path,
            &legacy.original_identity,
            &legacy.original_bytes,
            &legacy.original_hash,
            &legacy.residual_bytes,
            roots,
        )?,
        None => Vec::new(),
    };
    let mut filesystem = plan
        .operations
        .iter()
        .enumerate()
        .map(|(index, mutation)| operation_for_mutation(index, mutation, roots))
        .collect::<Result<Vec<_>, _>>()?;
    for operation in &mut filesystem {
        if let OwnershipProof::Receipt { source_id, sha256 } = &operation.ownership {
            operation.ownership = OwnershipProof::Adopted {
                source_id: source_id.clone(),
                sha256: sha256.clone(),
            };
        }
    }
    operations.append(&mut filesystem);
    operations.push(receipt_operation(roots, next_receipt, transaction_id)?);
    Ok(operations)
}

#[allow(clippy::too_many_arguments)]
fn legacy_lock_operations(
    lock_path: &Path,
    archive_path: &Path,
    original_identity: &LockIdentity,
    original_bytes: &[u8],
    original_hash: &str,
    residual_bytes: &[u8],
    roots: &ResolvedRoots,
) -> Result<Vec<Operation>, OperationBuildError> {
    let archive_root = root_for_destination(roots, archive_path)?;
    let lock_root = root_for_destination(roots, lock_path)?;
    let archive_precondition = snapshot_path(archive_path)?;
    if archive_precondition.kind != PathKind::Absent {
        return Err(OperationBuildError::StalePlan(archive_path.to_path_buf()));
    }
    let private_mode = effective_file_mode(0o600);
    let archive = Operation::new(
        "00000000-adoption-archive",
        OperationKind::WriteFile,
        archive_root,
        archive_path.to_path_buf(),
        PathSnapshot::absent(),
        PathSnapshot::file(original_bytes, private_mode),
        Inverse::RemoveCreated,
        OwnershipProof::UnownedDestination,
        OperationPayload::File {
            bytes: original_bytes.to_vec(),
            mode: private_mode,
        },
    )?;

    let lock_precondition = snapshot_path(lock_path)?;
    if lock_precondition.kind != PathKind::File
        || lock_precondition.sha256.as_deref() != Some(original_hash)
        || lock_precondition.size != Some(original_identity.size)
    {
        return Err(OperationBuildError::StalePlan(lock_path.to_path_buf()));
    }
    let lock_mode = lock_precondition.mode.unwrap_or(0o600);
    let rewrite = Operation::new(
        "00000001-rewrite-legacy-lock",
        OperationKind::RewriteLegacyLock,
        lock_root,
        lock_path.to_path_buf(),
        lock_precondition.clone(),
        PathSnapshot::file(residual_bytes, lock_mode),
        Inverse::RestoreBackup {
            original: lock_precondition,
        },
        OwnershipProof::Adopted {
            source_id: "vercel-skills-v3-lock".to_owned(),
            sha256: Some(original_hash.to_owned()),
        },
        OperationPayload::File {
            bytes: residual_bytes.to_vec(),
            mode: lock_mode,
        },
    )?
    .with_revalidation(FileIdentityProof {
        device: original_identity.device,
        inode: original_identity.inode,
        size: original_identity.size,
        mtime_seconds: original_identity.mtime_seconds,
        mtime_nanoseconds: original_identity.mtime_nanoseconds,
        sha256: original_hash.to_owned(),
    })?;

    Ok(vec![archive, rewrite])
}

fn validate_plan_receipt_coverage(
    plan: &Plan,
    receipt: &Receipt,
) -> Result<(), OperationBuildError> {
    for entry in &plan.entries {
        let owned = receipt.owned_asset(&entry.destination);
        match entry.action {
            PlanAction::Create | PlanAction::Update | PlanAction::Noop => {
                let Some(owned) = owned else {
                    return Err(OperationBuildError::ReceiptCoverage {
                        destination: entry.destination.clone(),
                        detail: "the final managed asset is missing",
                    });
                };
                if owned.source_id != entry.source {
                    return Err(OperationBuildError::ReceiptCoverage {
                        destination: entry.destination.clone(),
                        detail: "the source identity differs from the plan",
                    });
                }
            }
            PlanAction::Remove if owned.is_some() => {
                return Err(OperationBuildError::ReceiptCoverage {
                    destination: entry.destination.clone(),
                    detail: "a removed asset remains owned",
                });
            }
            PlanAction::Remove
            | PlanAction::Adoptable
            | PlanAction::Drifted
            | PlanAction::Conflict
            | PlanAction::RetainedUnmanaged
            | PlanAction::RecoveryRequired => {}
        }
    }

    for mutation in &plan.operations {
        if mutation.kind == MutationKind::RemoveOwnedPath {
            continue;
        }
        let Some(owned) = receipt.owned_asset(&mutation.destination) else {
            return Err(OperationBuildError::ReceiptCoverage {
                destination: mutation.destination.clone(),
                detail: "the mutation has no ownership record",
            });
        };
        if !owned_matches_mutation(owned, mutation) {
            return Err(OperationBuildError::ReceiptCoverage {
                destination: mutation.destination.clone(),
                detail: "the postcondition differs from the ownership record",
            });
        }
    }
    Ok(())
}

fn validate_adoption_receipt_coverage(
    adoption: &AdoptionPlan,
    receipt: &Receipt,
) -> Result<(), OperationBuildError> {
    for entry in &adoption.entries {
        let Some(owned) = receipt.owned_asset(&entry.destination) else {
            return Err(OperationBuildError::ReceiptCoverage {
                destination: entry.destination.clone(),
                detail: "the adopted asset has no ownership record",
            });
        };
        let expected_kind = match entry.entry_type {
            crate::adoption::EntryType::File => OwnedAssetKind::File,
            crate::adoption::EntryType::Directory => OwnedAssetKind::Directory,
            crate::adoption::EntryType::Symlink => OwnedAssetKind::Symlink,
        };
        if owned.source_id != entry.source_id
            || owned.kind != expected_kind
            || (expected_kind == OwnedAssetKind::File
                && owned.hash.as_deref() != Some(entry.hash.as_str()))
            || (expected_kind != OwnedAssetKind::Symlink && owned.mode != Some(entry.mode))
            || owned.link_target != entry.link_target
        {
            return Err(OperationBuildError::ReceiptCoverage {
                destination: entry.destination.clone(),
                detail: "the adoption proof differs from the ownership record",
            });
        }
    }
    Ok(())
}

fn owned_matches_mutation(owned: &OwnedAsset, mutation: &PlannedMutation) -> bool {
    let kind_matches = match mutation.kind {
        MutationKind::EnsureDirectory => owned.kind == OwnedAssetKind::Directory,
        MutationKind::WriteFile | MutationKind::ReplaceFile => owned.kind == OwnedAssetKind::File,
        MutationKind::CreateSymlink => owned.kind == OwnedAssetKind::Symlink,
        MutationKind::SetMode => owned.kind != OwnedAssetKind::Symlink,
        MutationKind::RemoveOwnedPath => false,
    };
    kind_matches
        && owned.hash == mutation.content_sha256
        && owned.mode == mutation.mode
        && owned.link_target == mutation.link_target
}

fn operation_for_mutation(
    index: usize,
    mutation: &PlannedMutation,
    roots: &ResolvedRoots,
) -> Result<Operation, OperationBuildError> {
    let root = root_for_exact_path(roots, &mutation.root)?;
    let precondition = match &mutation.precondition {
        PlanPrecondition::Missing => PathSnapshot::absent(),
        PlanPrecondition::Matches { snapshot } => {
            let observed = snapshot_path(&mutation.destination)?;
            if !snapshot_matches_plan(&observed, snapshot) {
                return Err(OperationBuildError::StalePlan(mutation.destination.clone()));
            }
            observed
        }
    };
    let (kind, expected_after, payload) = match mutation.kind {
        MutationKind::EnsureDirectory => {
            let mode = mutation.mode.unwrap_or(0o700);
            (
                OperationKind::EnsureDirectory,
                PathSnapshot::directory(mode),
                OperationPayload::Directory(mode),
            )
        }
        MutationKind::WriteFile | MutationKind::ReplaceFile => {
            let bytes = mutation
                .payload
                .clone()
                .ok_or_else(|| OperationBuildError::MissingPayload(mutation.id.clone()))?;
            let mode = mutation.mode.unwrap_or(0o600);
            (
                if mutation.kind == MutationKind::WriteFile {
                    OperationKind::WriteFile
                } else {
                    OperationKind::ReplaceFile
                },
                PathSnapshot::file(&bytes, mode),
                OperationPayload::File { bytes, mode },
            )
        }
        MutationKind::SetMode => {
            let mode = mutation.mode.unwrap_or(0o600);
            let mut expected = precondition.clone();
            expected.mode = Some(mode);
            (
                OperationKind::SetMode,
                expected,
                OperationPayload::Mode(mode),
            )
        }
        MutationKind::CreateSymlink => {
            let target = mutation
                .link_target
                .clone()
                .ok_or_else(|| OperationBuildError::MissingPayload(mutation.id.clone()))?;
            (
                OperationKind::CreateSymlink,
                PathSnapshot::symlink(target.clone()),
                OperationPayload::Symlink { target },
            )
        }
        MutationKind::RemoveOwnedPath => (
            OperationKind::RemoveOwnedPath,
            PathSnapshot::absent(),
            OperationPayload::None,
        ),
    };
    let inverse = match &mutation.inverse {
        PlannedInverse::RemoveCreated => Inverse::RemoveCreated,
        PlannedInverse::RestoreBackup => Inverse::RestoreBackup {
            original: precondition.clone(),
        },
        PlannedInverse::RestoreMode { mode } => Inverse::RestoreMode { mode: *mode },
        PlannedInverse::None => Inverse::None,
    };
    let ownership = match &mutation.ownership {
        PlanOwnership::UnownedDestination => OwnershipProof::UnownedDestination,
        PlanOwnership::Receipt { source_id, sha256 } => OwnershipProof::Receipt {
            source_id: source_id.clone(),
            sha256: sha256.clone(),
        },
    };
    Operation::new(
        format!("{index:08}-{}", mutation.id),
        kind,
        root,
        mutation.destination.clone(),
        precondition,
        expected_after,
        inverse,
        ownership,
        payload,
    )
    .map_err(Into::into)
}

fn receipt_operation(
    roots: &ResolvedRoots,
    receipt: &Receipt,
    transaction_id: &str,
) -> Result<Operation, OperationBuildError> {
    let private_mode = effective_file_mode(0o600);
    let mut receipt = receipt.clone();
    receipt.transaction_id = Some(transaction_id.to_owned());
    receipt.state = ReceiptState::Committed;
    receipt
        .validate()
        .map_err(|error| OperationBuildError::InvalidReceipt(error.to_string()))?;
    receipt
        .validate_roots(roots)
        .map_err(|error| OperationBuildError::InvalidReceipt(error.to_string()))?;
    let mut bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| OperationBuildError::InvalidReceipt(error.to_string()))?;
    bytes.push(b'\n');
    let precondition = snapshot_path(&roots.receipt_path)?;
    let (inverse, ownership) = if precondition.kind == PathKind::Absent {
        (Inverse::RemoveCreated, OwnershipProof::TransactionState)
    } else {
        (
            Inverse::RestoreBackup {
                original: precondition.clone(),
            },
            OwnershipProof::Receipt {
                source_id: "receipt-v1".to_owned(),
                sha256: precondition.sha256.clone(),
            },
        )
    };
    Operation::new(
        "zzzzzzzz-write-receipt",
        OperationKind::WriteReceipt,
        root_for_exact_path(roots, &roots.canonical.lexical)?,
        roots.receipt_path.clone(),
        precondition,
        PathSnapshot::file(&bytes, private_mode),
        inverse,
        ownership,
        OperationPayload::File {
            bytes,
            mode: private_mode,
        },
    )
    .map_err(Into::into)
}

fn root_for_destination(
    roots: &ResolvedRoots,
    destination: &Path,
) -> Result<RootSpec, OperationBuildError> {
    let identity = roots
        .allowed_top_level_roots()
        .filter(|root| destination.starts_with(&root.lexical))
        .max_by_key(|root| root.lexical.components().count())
        .ok_or_else(|| OperationBuildError::UnknownRoot(destination.to_path_buf()))?;
    Ok(root_spec(roots, identity))
}

fn root_for_exact_path(
    roots: &ResolvedRoots,
    path: &Path,
) -> Result<RootSpec, OperationBuildError> {
    let identity = roots
        .allowed_top_level_roots()
        .find(|root| root.lexical == path)
        .ok_or_else(|| OperationBuildError::UnknownRoot(path.to_path_buf()))?;
    Ok(root_spec(roots, identity))
}

fn root_spec(roots: &ResolvedRoots, identity: &RootIdentity) -> RootSpec {
    let id = if identity == &roots.canonical {
        "canonical".to_owned()
    } else if roots.legacy_lock_root.as_ref() == Some(identity) {
        "legacy-lock".to_owned()
    } else {
        roots
            .providers
            .iter()
            .find(|provider| provider.root == *identity)
            .map_or_else(
                || "provider".to_owned(),
                |provider| provider.id.as_str().to_owned(),
            )
    };
    RootSpec::new(id, identity.lexical.clone(), identity.device).with_real(identity.real.clone())
}

fn snapshot_matches_plan(observed: &PathSnapshot, planned: &crate::plan::PathSnapshot) -> bool {
    let kind = match planned.kind {
        NodeKind::Directory => PathKind::Directory,
        NodeKind::File => PathKind::File,
        NodeKind::Symlink => PathKind::Symlink,
    };
    observed.kind == kind
        && observed.sha256 == planned.sha256
        && observed.mode == planned.mode
        && observed.link_target == planned.link_target
}

#[cfg(all(test, unix))]
mod tests {
    use std::error::Error as _;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::{
        OperationBuildError, operation_for_mutation, operations_for_adoption, operations_for_plan,
        owned_matches_mutation, root_for_destination, root_spec, snapshot_matches_plan,
        validate_adoption_receipt_coverage, validate_plan_receipt_coverage,
    };
    use crate::adoption::{AdoptedEntry, AdoptionPlan, EntryType, LockIdentity};
    use crate::engine::{plan_desired_state, receipt_after_adoption};
    use crate::plan::{
        DesiredAsset, DesiredPayload, MutationKind, NodeKind, OwnershipProof as PlanOwnership,
        PathSnapshot as PlanSnapshot, Plan, PlanAction, PlanEntry, PlannedInverse, PlannedMutation,
        Precondition as PlanPrecondition,
    };
    use crate::provider::{ProviderId, RootIdentity, resolve_roots_from};
    use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt};
    use crate::transaction::{
        Inverse, OperationKind, OperationPayload, OwnershipProof, PathSnapshot, TransactionError,
    };

    fn empty_plan(applicable: bool) -> Plan {
        Plan {
            schema_version: 1,
            applicable,
            entries: Vec::new(),
            operations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn mutation(root: &Path, destination: PathBuf, kind: MutationKind) -> PlannedMutation {
        PlannedMutation {
            id: format!("{kind:?}"),
            kind,
            root: root.to_path_buf(),
            destination,
            precondition: PlanPrecondition::Missing,
            inverse: PlannedInverse::RemoveCreated,
            ownership: PlanOwnership::UnownedDestination,
            content_sha256: None,
            mode: None,
            link_target: None,
            payload: None,
        }
    }

    fn adoption_fixture(archive_path: PathBuf, applicable: bool) -> AdoptionPlan {
        AdoptionPlan {
            entries: Vec::new(),
            original_identity: LockIdentity {
                device: 1,
                inode: 1,
                size: 6,
                mtime_seconds: 1,
                mtime_nanoseconds: 1,
            },
            original_bytes: b"legacy".to_vec(),
            original_hash: format!("{:x}", Sha256::digest(b"legacy")),
            archive_path,
            residual_bytes: b"{}".to_vec(),
            applicable,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn operation_errors_report_the_blocker_and_preserve_transaction_sources() {
        assert_eq!(
            OperationBuildError::PlanBlocked.to_string(),
            "blocked plan cannot reach the executor"
        );
        assert_eq!(
            OperationBuildError::AdoptionBlocked.to_string(),
            "blocked adoption cannot reach the executor"
        );
        assert_eq!(
            OperationBuildError::UnknownRoot(PathBuf::from("/unknown")).to_string(),
            "operation root is not registered: /unknown"
        );
        assert_eq!(
            OperationBuildError::MissingPayload("op-1".to_owned()).to_string(),
            "operation op-1 has no file payload"
        );
        assert_eq!(
            OperationBuildError::StalePlan(PathBuf::from("/stale")).to_string(),
            "filesystem changed after planning: /stale"
        );
        assert_eq!(
            OperationBuildError::InvalidReceipt("bad hash".to_owned()).to_string(),
            "cannot encode receipt: bad hash"
        );
        assert_eq!(
            OperationBuildError::ReceiptCoverage {
                destination: PathBuf::from("/asset"),
                detail: "missing",
            }
            .to_string(),
            "receipt does not cover /asset: missing"
        );
        let transaction = OperationBuildError::from(TransactionError::InjectedFailure(
            "transaction failed".to_owned(),
        ));
        assert_eq!(transaction.to_string(), "transaction failed");
        assert!(transaction.source().is_some());
        assert!(OperationBuildError::PlanBlocked.source().is_none());
    }

    #[test]
    fn blocked_plans_and_adoptions_never_reach_operation_building() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        assert!(matches!(
            operations_for_plan(&empty_plan(false), &roots, &receipt, "tx"),
            Err(OperationBuildError::PlanBlocked)
        ));
        let adoption = adoption_fixture(roots.state_directory.join("legacy.json"), false);
        assert!(matches!(
            operations_for_adoption(
                &roots.canonical.lexical.join("lock.json"),
                &adoption,
                &roots,
                &receipt,
                "tx",
            ),
            Err(OperationBuildError::AdoptionBlocked)
        ));
    }

    #[test]
    fn receipt_coverage_rejects_missing_wrong_and_retained_ownership() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let destination = roots.canonical_skills.join("example/SKILL.md");
        let mut plan = empty_plan(true);
        plan.entries.push(PlanEntry {
            action: PlanAction::Create,
            source: "expected".to_owned(),
            destination: destination.clone(),
            owner: crate::plan::Owner::Unmanaged,
            reason: String::new(),
        });
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        assert!(matches!(
            validate_plan_receipt_coverage(&plan, &receipt),
            Err(OperationBuildError::ReceiptCoverage {
                detail: "the final managed asset is missing",
                ..
            })
        ));

        receipt.assets.push(OwnedAsset {
            source_id: "wrong".to_owned(),
            destination: destination.clone(),
            kind: OwnedAssetKind::File,
            hash: Some("b".repeat(64)),
            mode: Some(0o644),
            link_target: None,
            references: Vec::new(),
        });
        assert!(matches!(
            validate_plan_receipt_coverage(&plan, &receipt),
            Err(OperationBuildError::ReceiptCoverage {
                detail: "the source identity differs from the plan",
                ..
            })
        ));

        plan.entries[0].action = PlanAction::Remove;
        assert!(matches!(
            validate_plan_receipt_coverage(&plan, &receipt),
            Err(OperationBuildError::ReceiptCoverage {
                detail: "a removed asset remains owned",
                ..
            })
        ));

        plan.entries.clear();
        plan.operations.push(mutation(
            &roots.canonical.lexical,
            destination.clone(),
            MutationKind::RemoveOwnedPath,
        ));
        assert!(validate_plan_receipt_coverage(&plan, &receipt).is_ok());
        plan.operations.clear();

        let mut write = mutation(
            &roots.canonical.lexical,
            destination,
            MutationKind::WriteFile,
        );
        write.content_sha256 = Some("c".repeat(64));
        write.mode = Some(0o600);
        write.payload = Some(b"new".to_vec());
        plan.operations.push(write);
        let empty_receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        assert!(matches!(
            validate_plan_receipt_coverage(&plan, &empty_receipt),
            Err(OperationBuildError::ReceiptCoverage {
                detail: "the mutation has no ownership record",
                ..
            })
        ));
        assert!(matches!(
            validate_plan_receipt_coverage(&plan, &receipt),
            Err(OperationBuildError::ReceiptCoverage {
                detail: "the postcondition differs from the ownership record",
                ..
            })
        ));

        plan.operations.clear();
        plan.entries = [
            PlanAction::Remove,
            PlanAction::Adoptable,
            PlanAction::Drifted,
            PlanAction::Conflict,
            PlanAction::RetainedUnmanaged,
            PlanAction::RecoveryRequired,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, action)| PlanEntry {
            action,
            source: format!("terminal-{index}"),
            destination: roots.canonical_skills.join(format!("terminal-{index}")),
            owner: crate::plan::Owner::Unmanaged,
            reason: String::new(),
        })
        .collect();
        receipt.assets.clear();
        assert!(validate_plan_receipt_coverage(&plan, &receipt).is_ok());
    }

    #[test]
    fn converts_every_planned_mutation_and_rejects_stale_or_incomplete_plans() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        fs::create_dir_all(&roots.canonical.lexical)
            .unwrap_or_else(|error| panic!("fixture root failed: {error}"));
        let root = roots.canonical.lexical.as_path();

        let ensure = mutation(root, root.join("directory"), MutationKind::EnsureDirectory);
        let ensure = operation_for_mutation(0, &ensure, &roots)
            .unwrap_or_else(|error| panic!("directory conversion failed: {error}"));
        assert_eq!(ensure.kind, OperationKind::EnsureDirectory);
        assert_eq!(ensure.payload, OperationPayload::Directory(0o700));

        let mut write = mutation(root, root.join("file"), MutationKind::WriteFile);
        write.payload = Some(b"new".to_vec());
        let write = operation_for_mutation(1, &write, &roots)
            .unwrap_or_else(|error| panic!("file conversion failed: {error}"));
        assert_eq!(
            write.payload,
            OperationPayload::File {
                bytes: b"new".to_vec(),
                mode: 0o600,
            }
        );

        let mut link = mutation(root, root.join("link"), MutationKind::CreateSymlink);
        link.link_target = Some(PathBuf::from("target"));
        let link = operation_for_mutation(2, &link, &roots)
            .unwrap_or_else(|error| panic!("symlink conversion failed: {error}"));
        assert_eq!(link.kind, OperationKind::CreateSymlink);

        let replace_path = root.join("replace");
        fs::write(&replace_path, b"old")
            .unwrap_or_else(|error| panic!("replace fixture failed: {error}"));
        fs::set_permissions(&replace_path, fs::Permissions::from_mode(0o644))
            .unwrap_or_else(|error| panic!("replace mode failed: {error}"));
        let current = PlanSnapshot {
            kind: NodeKind::File,
            sha256: Some(format!("{:x}", Sha256::digest(b"old"))),
            mode: Some(0o644),
            link_target: None,
        };
        let mut replace = mutation(root, replace_path.clone(), MutationKind::ReplaceFile);
        replace.precondition = PlanPrecondition::Matches {
            snapshot: current.clone(),
        };
        replace.inverse = PlannedInverse::RestoreBackup;
        replace.ownership = PlanOwnership::Receipt {
            source_id: "replace".to_owned(),
            sha256: current.sha256.clone(),
        };
        replace.payload = Some(b"replacement".to_vec());
        replace.mode = Some(0o640);
        let replace = operation_for_mutation(3, &replace, &roots)
            .unwrap_or_else(|error| panic!("replace conversion failed: {error}"));
        assert_eq!(replace.kind, OperationKind::ReplaceFile);
        assert!(matches!(replace.inverse, Inverse::RestoreBackup { .. }));
        assert!(matches!(replace.ownership, OwnershipProof::Receipt { .. }));

        let mode_path = root.join("mode");
        fs::write(&mode_path, b"mode")
            .unwrap_or_else(|error| panic!("mode fixture failed: {error}"));
        fs::set_permissions(&mode_path, fs::Permissions::from_mode(0o644))
            .unwrap_or_else(|error| panic!("mode setup failed: {error}"));
        let mut set_mode = mutation(root, mode_path, MutationKind::SetMode);
        set_mode.precondition = PlanPrecondition::Matches {
            snapshot: PlanSnapshot {
                kind: NodeKind::File,
                sha256: Some(format!("{:x}", Sha256::digest(b"mode"))),
                mode: Some(0o644),
                link_target: None,
            },
        };
        set_mode.inverse = PlannedInverse::RestoreMode { mode: 0o644 };
        set_mode.ownership = PlanOwnership::Receipt {
            source_id: "mode".to_owned(),
            sha256: Some(format!("{:x}", Sha256::digest(b"mode"))),
        };
        let set_mode = operation_for_mutation(4, &set_mode, &roots)
            .unwrap_or_else(|error| panic!("mode conversion failed: {error}"));
        assert_eq!(set_mode.payload, OperationPayload::Mode(0o600));
        assert_eq!(set_mode.inverse, Inverse::RestoreMode { mode: 0o644 });

        let remove_path = root.join("remove");
        fs::create_dir(&remove_path)
            .unwrap_or_else(|error| panic!("remove fixture failed: {error}"));
        fs::set_permissions(&remove_path, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("remove mode failed: {error}"));
        let mut remove = mutation(root, remove_path, MutationKind::RemoveOwnedPath);
        remove.precondition = PlanPrecondition::Matches {
            snapshot: PlanSnapshot {
                kind: NodeKind::Directory,
                sha256: None,
                mode: Some(0o755),
                link_target: None,
            },
        };
        remove.inverse = PlannedInverse::RestoreBackup;
        remove.ownership = PlanOwnership::Receipt {
            source_id: "remove".to_owned(),
            sha256: None,
        };
        let remove = operation_for_mutation(5, &remove, &roots)
            .unwrap_or_else(|error| panic!("remove conversion failed: {error}"));
        assert_eq!(remove.kind, OperationKind::RemoveOwnedPath);
        assert_eq!(remove.payload, OperationPayload::None);

        let missing_payload = mutation(root, root.join("missing"), MutationKind::WriteFile);
        assert!(matches!(
            operation_for_mutation(6, &missing_payload, &roots),
            Err(OperationBuildError::MissingPayload(_))
        ));
        let mut stale = mutation(root, replace_path.clone(), MutationKind::ReplaceFile);
        stale.precondition = PlanPrecondition::Matches {
            snapshot: PlanSnapshot {
                kind: NodeKind::File,
                sha256: Some("0".repeat(64)),
                mode: Some(0o644),
                link_target: None,
            },
        };
        assert!(matches!(
            operation_for_mutation(7, &stale, &roots),
            Err(OperationBuildError::StalePlan(path)) if path == replace_path
        ));
        let unknown = mutation(
            &root.join("unregistered"),
            root.join("unregistered/file"),
            MutationKind::WriteFile,
        );
        assert!(matches!(
            operation_for_mutation(8, &unknown, &roots),
            Err(OperationBuildError::UnknownRoot(_))
        ));

        let mut invalid_inverse = mutation(
            root,
            root.join("invalid-inverse"),
            MutationKind::EnsureDirectory,
        );
        invalid_inverse.inverse = PlannedInverse::None;
        assert!(matches!(
            operation_for_mutation(9, &invalid_inverse, &roots),
            Err(OperationBuildError::Transaction(_))
        ));
    }

    #[test]
    fn destination_root_selection_prefers_the_longest_registered_root() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let nested_codex = home.path().join(".agents/codex-home");
        let roots = resolve_roots_from(
            Some(home.path().as_os_str()),
            Some(nested_codex.as_os_str()),
            &[ProviderId::Codex],
        )
        .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let selected = root_for_destination(&roots, &nested_codex.join("agents/example.toml"))
            .unwrap_or_else(|error| panic!("root selection failed: {error}"));
        assert_eq!(selected.id, "codex");
        assert_eq!(selected.path, nested_codex);
        assert!(matches!(
            root_for_destination(&roots, Path::new("/outside")),
            Err(OperationBuildError::UnknownRoot(path)) if path == Path::new("/outside")
        ));

        let detached = RootIdentity {
            lexical: home.path().join("detached"),
            real: home.path().join("detached"),
            device: roots.home.device,
        };
        assert_eq!(root_spec(&roots, &detached).id, "provider");
    }

    #[test]
    fn mutation_matching_covers_every_kind_and_symlink_snapshot() {
        let destination = PathBuf::from("/managed");
        let cases = [
            (
                MutationKind::EnsureDirectory,
                OwnedAssetKind::Directory,
                None,
                Some(0o755),
                None,
                true,
            ),
            (
                MutationKind::WriteFile,
                OwnedAssetKind::File,
                Some("a".repeat(64)),
                Some(0o644),
                None,
                true,
            ),
            (
                MutationKind::ReplaceFile,
                OwnedAssetKind::File,
                Some("b".repeat(64)),
                Some(0o600),
                None,
                true,
            ),
            (
                MutationKind::SetMode,
                OwnedAssetKind::Directory,
                None,
                Some(0o700),
                None,
                true,
            ),
            (
                MutationKind::CreateSymlink,
                OwnedAssetKind::Symlink,
                None,
                None,
                Some(PathBuf::from("target")),
                true,
            ),
            (
                MutationKind::RemoveOwnedPath,
                OwnedAssetKind::File,
                None,
                None,
                None,
                false,
            ),
        ];
        for (kind, owned_kind, hash, mode, link_target, expected) in cases {
            let owned = OwnedAsset {
                source_id: "asset".to_owned(),
                destination: destination.clone(),
                kind: owned_kind,
                hash: hash.clone(),
                mode,
                link_target: link_target.clone(),
                references: Vec::new(),
            };
            let mut planned = mutation(Path::new("/"), destination.clone(), kind);
            planned.content_sha256 = hash;
            planned.mode = mode;
            planned.link_target = link_target;
            assert_eq!(owned_matches_mutation(&owned, &planned), expected);
        }

        assert!(snapshot_matches_plan(
            &PathSnapshot::symlink(PathBuf::from("target")),
            &PlanSnapshot {
                kind: NodeKind::Symlink,
                sha256: None,
                mode: None,
                link_target: Some(PathBuf::from("target")),
            },
        ));
    }

    #[test]
    fn adoption_coverage_and_filesystem_preconditions_fail_closed() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        fs::create_dir_all(&roots.state_directory)
            .unwrap_or_else(|error| panic!("state fixture failed: {error}"));
        let destination = roots.canonical_skills.join("adopted");
        let mut adoption = adoption_fixture(roots.state_directory.join("archive.json"), true);
        adoption.entries.push(AdoptedEntry {
            source_id: "file".to_owned(),
            destination: destination.clone(),
            entry_type: EntryType::File,
            hash: "a".repeat(64),
            mode: 0o644,
            link_target: None,
        });
        let mut receipt = Receipt::new("0.1.0", "b".repeat(64), &roots);
        assert!(matches!(
            validate_adoption_receipt_coverage(&adoption, &receipt),
            Err(OperationBuildError::ReceiptCoverage {
                detail: "the adopted asset has no ownership record",
                ..
            })
        ));
        receipt.assets.push(OwnedAsset {
            source_id: "wrong".to_owned(),
            destination: destination.clone(),
            kind: OwnedAssetKind::Directory,
            hash: None,
            mode: Some(0o755),
            link_target: None,
            references: Vec::new(),
        });
        assert!(matches!(
            validate_adoption_receipt_coverage(&adoption, &receipt),
            Err(OperationBuildError::ReceiptCoverage {
                detail: "the adoption proof differs from the ownership record",
                ..
            })
        ));

        adoption.entries = vec![
            AdoptedEntry {
                source_id: "directory".to_owned(),
                destination: roots.canonical_skills.join("directory"),
                entry_type: EntryType::Directory,
                hash: String::new(),
                mode: 0o755,
                link_target: None,
            },
            AdoptedEntry {
                source_id: "link".to_owned(),
                destination: roots.canonical_skills.join("link"),
                entry_type: EntryType::Symlink,
                hash: String::new(),
                mode: 0,
                link_target: Some(PathBuf::from("target")),
            },
        ];
        receipt.assets = vec![
            OwnedAsset {
                source_id: "directory".to_owned(),
                destination: adoption.entries[0].destination.clone(),
                kind: OwnedAssetKind::Directory,
                hash: None,
                mode: Some(0o755),
                link_target: None,
                references: Vec::new(),
            },
            OwnedAsset {
                source_id: "link".to_owned(),
                destination: adoption.entries[1].destination.clone(),
                kind: OwnedAssetKind::Symlink,
                hash: None,
                mode: None,
                link_target: Some(PathBuf::from("target")),
                references: Vec::new(),
            },
        ];
        assert!(validate_adoption_receipt_coverage(&adoption, &receipt).is_ok());

        let archive_path = roots.state_directory.join("occupied.json");
        fs::write(&archive_path, b"occupied")
            .unwrap_or_else(|error| panic!("archive fixture failed: {error}"));
        let stale_archive = adoption_fixture(archive_path.clone(), true);
        let empty_receipt = Receipt::new("0.1.0", "b".repeat(64), &roots);
        assert!(matches!(
            operations_for_adoption(
                &roots.canonical.lexical.join("missing-lock.json"),
                &stale_archive,
                &roots,
                &empty_receipt,
                "tx",
            ),
            Err(OperationBuildError::StalePlan(path)) if path == archive_path
        ));

        let missing_lock = roots.canonical.lexical.join("missing-lock.json");
        let stale_lock = adoption_fixture(roots.state_directory.join("free.json"), true);
        assert!(matches!(
            operations_for_adoption(
                &missing_lock,
                &stale_lock,
                &roots,
                &empty_receipt,
                "tx",
            ),
            Err(OperationBuildError::StalePlan(path)) if path == missing_lock
        ));
    }

    #[test]
    fn replacing_an_existing_receipt_uses_receipt_ownership() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        fs::create_dir_all(&roots.state_directory)
            .unwrap_or_else(|error| panic!("state fixture failed: {error}"));
        let previous = Receipt::new("0.1.0", "a".repeat(64), &roots);
        let mut bytes = serde_json::to_vec_pretty(&previous)
            .unwrap_or_else(|error| panic!("receipt encoding failed: {error}"));
        bytes.push(b'\n');
        fs::write(&roots.receipt_path, bytes)
            .unwrap_or_else(|error| panic!("receipt fixture failed: {error}"));
        fs::set_permissions(&roots.receipt_path, fs::Permissions::from_mode(0o600))
            .unwrap_or_else(|error| panic!("receipt mode failed: {error}"));

        let operations = operations_for_plan(&empty_plan(true), &roots, &previous, "new-tx")
            .unwrap_or_else(|error| panic!("receipt operation failed: {error}"));

        assert_eq!(operations.len(), 1);
        assert!(matches!(
            operations[0].inverse,
            Inverse::RestoreBackup { .. }
        ));
        assert!(matches!(
            operations[0].ownership,
            OwnershipProof::Receipt { .. }
        ));
    }

    #[test]
    fn planner_operations_end_with_a_private_receipt() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Codex])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        fs::create_dir_all(&roots.canonical.lexical)
            .unwrap_or_else(|error| panic!("fixture failed: {error}"));
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        let desired = [DesiredAsset {
            source_id: "skills/example/SKILL.md".to_owned(),
            destination: roots.canonical_skills.join("example/SKILL.md"),
            payload: DesiredPayload::File {
                bytes: b"example".to_vec(),
                mode: 0o644,
            },
        }];
        receipt.assets.push(OwnedAsset {
            source_id: desired[0].source_id.clone(),
            destination: desired[0].destination.clone(),
            kind: OwnedAssetKind::File,
            hash: Some(format!("{:x}", Sha256::digest(b"example"))),
            mode: Some(0o644),
            link_target: None,
            references: vec![ProviderId::Codex],
        });
        let plan = plan_desired_state(&roots, None, &desired)
            .unwrap_or_else(|error| panic!("planning failed: {error}"));
        let operations = operations_for_plan(&plan, &roots, &receipt, "tx-1")
            .unwrap_or_else(|error| panic!("operation build failed: {error}"));
        assert_eq!(operations.len(), 2);
        assert_eq!(operations[1].kind, OperationKind::WriteReceipt);
        assert_eq!(operations[1].expected_after.mode, Some(0o600));
    }

    #[test]
    fn planner_rejects_a_mutation_missing_from_the_receipt() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Codex])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        fs::create_dir_all(&roots.canonical.lexical)
            .unwrap_or_else(|error| panic!("fixture failed: {error}"));
        let desired = [DesiredAsset {
            source_id: "skills/example/SKILL.md".to_owned(),
            destination: roots.canonical_skills.join("example/SKILL.md"),
            payload: DesiredPayload::File {
                bytes: b"example".to_vec(),
                mode: 0o644,
            },
        }];
        let plan = plan_desired_state(&roots, None, &desired)
            .unwrap_or_else(|error| panic!("planning failed: {error}"));
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        assert!(matches!(
            operations_for_plan(&plan, &roots, &receipt, "tx-missing"),
            Err(OperationBuildError::ReceiptCoverage { .. })
        ));
    }

    #[test]
    fn adoption_build_is_read_only_and_carries_lock_identity() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Codex])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        fs::create_dir_all(&roots.canonical.lexical)
            .unwrap_or_else(|error| panic!("fixture failed: {error}"));
        let lock_path = roots.canonical.lexical.join(".skill-lock.json");
        fs::write(&lock_path, b"legacy")
            .unwrap_or_else(|error| panic!("lock fixture failed: {error}"));
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600))
            .unwrap_or_else(|error| panic!("mode fixture failed: {error}"));
        let metadata = fs::metadata(&lock_path)
            .unwrap_or_else(|error| panic!("metadata fixture failed: {error}"));
        use std::os::unix::fs::MetadataExt;
        let adoption = AdoptionPlan {
            entries: vec![AdoptedEntry {
                source_id: "example".to_owned(),
                destination: roots.canonical_skills.join("example/SKILL.md"),
                entry_type: EntryType::File,
                hash: "b".repeat(64),
                mode: 0o644,
                link_target: None,
            }],
            original_identity: LockIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
                size: metadata.size(),
                mtime_seconds: metadata.mtime(),
                mtime_nanoseconds: metadata.mtime_nsec(),
            },
            original_bytes: b"legacy".to_vec(),
            original_hash: format!("{:x}", Sha256::digest(b"legacy")),
            archive_path: roots.state_directory.join("legacy-lock.json"),
            residual_bytes: b"residual".to_vec(),
            applicable: true,
            diagnostics: Vec::new(),
        };
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        let next = receipt_after_adoption(&receipt, &roots, &adoption)
            .unwrap_or_else(|error| panic!("receipt build failed: {error}"));
        let operations = operations_for_adoption(&lock_path, &adoption, &roots, &next, "tx-2")
            .unwrap_or_else(|error| panic!("operation build failed: {error}"));
        assert_eq!(operations.len(), 3);
        assert_eq!(operations[1].kind, OperationKind::RewriteLegacyLock);
        assert!(operations[1].revalidation.is_some());
        assert_eq!(
            fs::read(&lock_path).ok().as_deref(),
            Some(b"legacy".as_slice())
        );
        assert!(!adoption.archive_path.exists());
    }
}
