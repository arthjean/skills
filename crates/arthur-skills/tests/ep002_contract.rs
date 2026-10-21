//! EP-002 contracts: one canonical decision, consumed without divergence by
//! every command, renderer and control-plane operation.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use arthur_skills::adoption::{self, LegacyImportPlan};
use arthur_skills::app::{App, Review, blocked_review_remediation};
use arthur_skills::catalog::{AssetKind, Catalog};
use arthur_skills::lifecycle::{
    LEGACY_LOCK_UNSUPPORTED, LegacyEvidence, LifecycleDecision, LifecycleRequest,
    ReceiptChangeReason, decide,
};
use arthur_skills::operations::operations_for_plan;
use arthur_skills::output::{Envelope, write_human, write_json};
use arthur_skills::plain;
use arthur_skills::plan::{
    LEGACY_ENTRY_DOES_NOT_MATCH_CATALOG, MATCHING_UNMANAGED_WITHOUT_PROOF, OwnershipBasis,
    OwnershipClaim, PlanAction,
};
use arthur_skills::provider::{ProviderId, ResolvedRoots, resolve_roots_from};
use arthur_skills::receipt::Receipt;
use arthur_skills::transaction::{
    FailAfterMutation, SignalFlags, TransactionEngine, TransactionOutcome, hash_bytes,
};
use arthur_skills::ui::render;
use arthur_skills::workflow::assess_decision;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn roots(home: &TempDir, providers: &[ProviderId]) -> Result<ResolvedRoots, Box<dyn Error>> {
    Ok(resolve_roots_from(
        Some(home.path().as_os_str()),
        None,
        providers,
    )?)
}

fn reconcile(providers: &[ProviderId]) -> LifecycleRequest {
    LifecycleRequest::Reconcile {
        providers: providers.to_vec(),
    }
}

fn apply(
    roots: &ResolvedRoots,
    decision: &LifecycleDecision,
    transaction_id: &str,
) -> Result<TransactionOutcome, Box<dyn Error>> {
    let transaction =
        operations_for_plan(&decision.plan, roots, &decision.receipt, transaction_id)?;
    let engine = TransactionEngine::new(roots.state_directory.clone(), SignalFlags::default());
    Ok(engine.apply(transaction_id, transaction.operations, &transaction.claims)?)
}

/// Installs the whole catalog and returns the committed receipt.
fn install(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    transaction_id: &str,
) -> Result<Receipt, Box<dyn Error>> {
    let decision = decide(
        catalog,
        roots,
        None,
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Absent,
    )?;
    assert!(decision.applicable(), "{:?}", decision.plan.diagnostics);
    assert_eq!(
        apply(roots, &decision, transaction_id)?,
        TransactionOutcome::Committed
    );
    Ok(Receipt::decode(&fs::read(&roots.receipt_path)?)?)
}

fn write_legacy_lock(roots: &ResolvedRoots, names: &[&str]) -> TestResult {
    let mut skills = serde_json::Map::new();
    for name in names {
        skills.insert(
            (*name).to_owned(),
            serde_json::json!({
                "source": "arthjean/skills",
                "sourceType": "github",
                "skillFolderHash": "0123456789012345678901234567890123456789",
                "installedAt": "2026-01-01T00:00:00.000Z",
                "updatedAt": "2026-01-01T00:00:00.000Z"
            }),
        );
    }
    fs::create_dir_all(
        roots
            .legacy_lock_path
            .parent()
            .ok_or("legacy lock has no parent")?,
    )?;
    fs::write(
        &roots.legacy_lock_path,
        serde_json::to_vec_pretty(&serde_json::json!({ "version": 3, "skills": skills }))?,
    )?;
    Ok(())
}

fn legacy_plan(
    catalog: &Catalog,
    roots: &ResolvedRoots,
) -> Result<Option<LegacyImportPlan>, Box<dyn Error>> {
    let names = catalog
        .manifest()
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Skill)
        .map(|asset| asset.name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    Ok(adoption::inspect_legacy_import(
        &roots.legacy_lock_path,
        &roots.state_directory.join("vercel-skills-v3-lock.json"),
        &names,
    )?)
}

/// Every projection a surface can render, computed from the decision alone.
fn projections(
    decision: &LifecycleDecision,
    roots: &ResolvedRoots,
) -> Result<String, Box<dyn Error>> {
    let envelope = Envelope::new(Some("plan")).with_plan(&decision.plan);
    let mut json = Vec::new();
    write_json(&envelope, &mut json)?;
    let mut human = Vec::new();
    write_human(&envelope, &mut human)?;

    let assessment = assess_decision(None, decision, 0, 0);
    let mut app = App::with_selection(54, &decision.selected_providers);
    app.set_review(Review::for_decision(decision, roots, assessment.clone()));
    let mut plain_output = Vec::new();
    plain::render(&app, &mut plain_output)?;

    let mut terminal = Terminal::new(TestBackend::new(100, 24))?;
    terminal.draw(|frame| render(frame, &app, false))?;
    let tui = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect::<String>();

    let operations = decision
        .plan
        .operations
        .iter()
        .map(|mutation| format!("{:?} {}", mutation.kind, mutation.destination.display()))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        String::from_utf8(json)?,
        String::from_utf8(human)?,
        String::from_utf8(plain_output)?,
        tui,
        operations,
        serde_json::to_string(&assessment)?,
    ))
}

/// Normalizes a decision for comparison: only the projected plan and its
/// control-plane convergence are compared, never a transaction identifier.
fn normalized(decision: &LifecycleDecision) -> Result<String, Box<dyn Error>> {
    Ok(serde_json::to_string(&serde_json::json!({
        "applicable": decision.plan.applicable,
        "entries": decision.plan.entries,
        "operations": decision.plan.operations,
        "diagnostics": decision.plan.diagnostics,
        "summary": decision.summary,
        "receipt_change": decision.receipt_change,
        "receipt": decision.receipt,
    }))?)
}

#[test]
fn a_reconcile_decision_carries_every_section_and_is_byte_deterministic() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;

    let first = decide(
        &catalog,
        &roots,
        None,
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Absent,
    )?;
    assert!(first.applicable());
    assert!(!first.plan.entries.is_empty());
    assert!(!first.plan.operations.is_empty());
    assert!(!first.receipt.assets.is_empty());
    assert!(first.receipt_change.required);
    assert!(
        first
            .receipt_change
            .reasons
            .contains(&ReceiptChangeReason::NoCurrentReceipt)
    );
    assert!(first.summary.actions.contains_key("create"));
    assert!(!first.notices.is_empty());
    // Every entry carries a provenance claim, and every claim is provable before
    // it can appear in the projected receipt.
    assert!(
        first
            .plan
            .entries
            .iter()
            .all(|entry| entry.ownership_basis() != OwnershipBasis::None)
    );
    for asset in &first.receipt.assets {
        let entry = first
            .plan
            .entries
            .iter()
            .find(|entry| entry.destination == asset.destination)
            .ok_or("a receipt record has no plan entry")?;
        assert!(entry.ownership_basis().is_provable());
    }

    // Two computations on byte-identical inputs produce the same decision.
    let second = decide(
        &catalog,
        &roots,
        None,
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Absent,
    )?;
    assert_eq!(normalized(&first)?, normalized(&second)?);
    Ok(())
}

#[test]
fn an_unsupported_legacy_lock_returns_a_complete_blocked_decision() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &[ProviderId::Codex])?;
    let catalog = Catalog::load()?;

    let blocked = decide(
        &catalog,
        &roots,
        None,
        &reconcile(&[ProviderId::Codex]),
        &LegacyEvidence::Unsupported {
            detail: "lock schema 4 is not supported".to_owned(),
        },
    )?;

    assert!(!blocked.applicable());
    assert!(blocked.plan.operations.is_empty());
    assert!(blocked.receipt.assets.is_empty());
    assert!(!blocked.receipt_change.required);
    let diagnostic = blocked
        .plan
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == LEGACY_LOCK_UNSUPPORTED)
        .ok_or("the unsupported lock has no diagnostic")?;
    assert_eq!(
        diagnostic.path_utf8.as_deref(),
        roots.legacy_lock_path.to_str()
    );
    // A blocked decision can never reach the executor.
    assert!(operations_for_plan(&blocked.plan, &roots, &blocked.receipt, "tx").is_err());
    Ok(())
}

#[test]
fn an_adopt_decision_only_selects_verified_candidates() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &[ProviderId::Codex])?;
    let catalog = Catalog::load()?;

    // One skill the lock proves, one identical skill it does not.
    let proven = roots.canonical_skills.join("find-skills");
    let unproven = roots.canonical_skills.join("better-auth-best-practices");
    fs::create_dir_all(&proven)?;
    fs::create_dir_all(&unproven)?;
    write_legacy_lock(&roots, &["find-skills"])?;
    let legacy = legacy_plan(&catalog, &roots)?.ok_or("the lock proves no catalog skill")?;

    let adopt = decide(
        &catalog,
        &roots,
        None,
        &LifecycleRequest::Adopt {
            providers: vec![ProviderId::Codex],
        },
        &LegacyEvidence::Verified(&legacy),
    )?;
    assert!(adopt.applicable());
    assert!(
        adopt
            .plan
            .entries
            .iter()
            .all(|entry| entry.action == PlanAction::Adoptable)
    );
    assert!(
        adopt
            .plan
            .entries
            .iter()
            .any(|entry| entry.destination == proven)
    );
    assert!(
        adopt
            .plan
            .entries
            .iter()
            .all(|entry| entry.destination != unproven),
        "an unproven path must stay outside the adoption plan"
    );
    assert!(
        adopt
            .plan
            .diagnostics
            .iter()
            .all(|diagnostic| { diagnostic.path_utf8.as_deref() != unproven.to_str() })
    );
    assert!(!adopt.receipt_change.required);
    // The transfer claims each verified entry only after the lock is reverified,
    // so this decision projects no ownership of its own.
    assert!(
        adopt.receipt.assets.is_empty(),
        "an adopt decision must not project a claim outside its scope"
    );

    // The same state under a reconcile request keeps the unproven path as a
    // conflict with an executable remediation.
    let reconciled = decide(
        &catalog,
        &roots,
        None,
        &reconcile(&[ProviderId::Codex]),
        &LegacyEvidence::Verified(&legacy),
    )?;
    assert!(!reconciled.applicable());
    assert_eq!(reconciled.summary.verified_legacy_candidates, 1);
    assert_eq!(reconciled.summary.matching_unmanaged, 1);
    assert!(reconciled.plan.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == MATCHING_UNMANAGED_WITHOUT_PROOF
            && diagnostic.path_utf8.as_deref() == unproven.to_str()
    }));
    Ok(())
}

#[test]
fn a_non_conforming_legacy_entry_reports_its_source_and_destination() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &[ProviderId::Codex])?;
    let catalog = Catalog::load()?;

    let divergent = roots.canonical_skills.join("find-skills/SKILL.md");
    fs::create_dir_all(divergent.parent().ok_or("skill has no parent")?)?;
    fs::write(&divergent, b"not the bundled catalog")?;
    write_legacy_lock(&roots, &["find-skills"])?;
    let legacy = legacy_plan(&catalog, &roots)?.ok_or("the lock proves no catalog skill")?;

    let decision = decide(
        &catalog,
        &roots,
        None,
        &reconcile(&[ProviderId::Codex]),
        &LegacyEvidence::Verified(&legacy),
    )?;

    assert!(!decision.applicable());
    let diagnostic = decision
        .plan
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == LEGACY_ENTRY_DOES_NOT_MATCH_CATALOG
                && diagnostic.path_utf8.as_deref() == divergent.to_str()
        })
        .ok_or("the divergent legacy path has no diagnostic")?;
    assert_eq!(diagnostic.source_id.as_deref(), Some("find-skills"));
    assert!(
        decision
            .plan
            .entries
            .iter()
            .any(|entry| entry.destination == divergent
                && entry.action == PlanAction::Conflict
                && entry.ownership_basis() == OwnershipBasis::None)
    );
    assert!(decision.receipt.owned_asset(&divergent).is_none());
    Ok(())
}

#[test]
fn every_surface_projects_the_same_decision_without_reading_the_filesystem() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;
    let receipt = install(&catalog, &roots, "projection-install")?;

    let managed = roots.canonical_skills.join("find-skills/SKILL.md");
    fs::write(&managed, b"local edit")?;
    let decision = decide(
        &catalog,
        &roots,
        Some(&receipt),
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Absent,
    )?;
    assert!(!decision.applicable());

    let before = projections(&decision, &roots)?;
    // Mutating the filesystem after the decision cannot change any projection:
    // no renderer and no executor reclassifies an asset.
    fs::remove_file(&managed)?;
    fs::write(&managed, b"another edit")?;
    fs::remove_dir_all(roots.canonical_skills.join("better-auth-best-practices"))?;
    assert_eq!(before, projections(&decision, &roots)?);

    // The blocked review disables apply and never recommends adopt without a
    // verified candidate.
    let app_remediation = blocked_review_remediation(decision.summary.verified_legacy_candidates);
    assert!(!app_remediation.contains("adopt"));
    assert!(before.contains("Application is disabled") || !before.contains("adopt"));
    assert!(
        before.contains("managed path differs from its receipt proof"),
        "the drift reason must reach the surfaces"
    );
    Ok(())
}

#[test]
fn a_stale_receipt_converges_through_a_visible_receipt_only_operation() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;
    let committed = install(&catalog, &roots, "converge-install")?;

    // The disk already equals the catalog; only the receipt is stale.
    let mut stale = committed.clone();
    stale.catalog_sha256 = "b".repeat(64);
    let target = roots.canonical_skills.join("find-skills/SKILL.md");
    let record = stale
        .assets
        .iter_mut()
        .find(|asset| asset.destination == target)
        .ok_or("the managed asset is absent from the receipt")?;
    record.hash = Some(hash_bytes(b"a previous catalog version"));

    let decision = decide(
        &catalog,
        &roots,
        Some(&stale),
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Absent,
    )?;

    assert!(decision.applicable());
    assert!(!decision.is_current());
    assert!(decision.receipt_change.required);
    assert!(
        decision
            .receipt_change
            .reasons
            .contains(&ReceiptChangeReason::CatalogVersion)
    );
    assert!(
        decision
            .receipt_change
            .reasons
            .contains(&ReceiptChangeReason::Assets)
    );
    // The stale asset is a no-op: only the receipt proof converges, and its claim
    // carries the fingerprint the decision verified.
    let proven = decision
        .plan
        .entries
        .iter()
        .find(|entry| entry.destination == target)
        .ok_or("the managed asset is absent from the plan")?;
    assert_eq!(proven.action, PlanAction::Noop);
    let OwnershipClaim::Receipt {
        source_id,
        expected,
    } = &proven.ownership
    else {
        return Err("a receipt-proven entry must carry its receipt claim".into());
    };
    assert_eq!(source_id, "skills/find-skills/SKILL.md");
    assert_eq!(
        expected.sha256,
        Some(hash_bytes(b"a previous catalog version")),
        "the claim keeps the fingerprint the receipt recorded"
    );
    assert!(
        decision
            .plan
            .operations
            .iter()
            .all(|mutation| mutation.destination != target)
    );
    // The receipt commit is part of the same plan on every surface.
    let receipt_entry = decision
        .plan
        .entries
        .iter()
        .find(|entry| entry.action == PlanAction::WriteReceipt)
        .ok_or("the receipt convergence is absent from the plan")?;
    assert_eq!(receipt_entry.destination, roots.receipt_path);
    let envelope = Envelope::new(Some("update")).with_plan(&decision.plan);
    assert_eq!(envelope.summary.get("write_receipt"), Some(&1));
    let mut json = Vec::new();
    write_json(&envelope, &mut json)?;
    assert!(String::from_utf8(json)?.contains("write_receipt"));

    let before = asset_mtimes(&roots)?;
    assert_eq!(
        apply(&roots, &decision, "converge-receipt")?,
        TransactionOutcome::Committed
    );
    assert_eq!(
        before,
        asset_mtimes(&roots)?,
        "content must not be rewritten"
    );

    // The same request is now a complete no-op.
    let reread = Receipt::decode(&fs::read(&roots.receipt_path)?)?;
    let again = decide(
        &catalog,
        &roots,
        Some(&reread),
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Absent,
    )?;
    assert!(again.is_current());
    assert!(!again.receipt_change.required);
    assert!(
        again
            .plan
            .entries
            .iter()
            .all(|entry| entry.action == PlanAction::Noop)
    );
    assert!(again.plan.operations.is_empty());
    assert!(
        operations_for_plan(&again.plan, &roots, &again.receipt, "second-run")?
            .operations
            .is_empty(),
        "a semantically current receipt is never rewritten"
    );
    Ok(())
}

#[test]
fn a_receipt_that_only_differs_by_transaction_identity_is_current() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &[ProviderId::Codex])?;
    let catalog = Catalog::load()?;
    let decision = decide(
        &catalog,
        &roots,
        None,
        &reconcile(&[ProviderId::Codex]),
        &LegacyEvidence::Absent,
    )?;
    assert_eq!(
        apply(&roots, &decision, "identity-install")?,
        TransactionOutcome::Committed
    );
    let committed = Receipt::decode(&fs::read(&roots.receipt_path)?)?;
    assert_eq!(
        committed.transaction_id.as_deref(),
        Some("identity-install")
    );

    let mut renamed = committed.clone();
    renamed.transaction_id = Some("a-different-transaction".to_owned());
    let decision = decide(
        &catalog,
        &roots,
        Some(&renamed),
        &reconcile(&[ProviderId::Codex]),
        &LegacyEvidence::Absent,
    )?;
    assert!(decision.is_current());
    assert!(!decision.receipt_change.required);
    assert!(
        decision
            .plan
            .entries
            .iter()
            .all(|entry| entry.action != PlanAction::WriteReceipt)
    );

    // A receipt that requires recovery is not a convergence: the existing
    // recovery contract takes precedence over any metadata update.
    let mut recovery = committed.clone();
    recovery.state = arthur_skills::receipt::ReceiptState::RecoveryRequired;
    assert!(
        decide(
            &catalog,
            &roots,
            Some(&recovery),
            &reconcile(&[ProviderId::Codex]),
            &LegacyEvidence::Absent,
        )
        .is_err()
    );

    // Every other semantic field forces the convergence instead.
    for (label, mutate) in mutations() {
        let mut mutated = committed.clone();
        mutate(&mut mutated);
        let decision = decide(
            &catalog,
            &roots,
            Some(&mutated),
            &reconcile(&[ProviderId::Codex]),
            &LegacyEvidence::Absent,
        )?;
        assert!(
            decision.receipt_change.required,
            "{label} must force a receipt commit"
        );
        assert!(
            decision
                .plan
                .entries
                .iter()
                .any(|entry| entry.action == PlanAction::WriteReceipt),
            "{label} must appear in the plan"
        );
    }
    Ok(())
}

type ReceiptMutation = (&'static str, fn(&mut Receipt));

fn mutations() -> Vec<ReceiptMutation> {
    vec![
        ("cli_version", |receipt| {
            receipt.cli_version = "0.0.1".to_owned();
        }),
        ("catalog_sha256", |receipt| {
            receipt.catalog_sha256 = "c".repeat(64);
        }),
        ("providers", |receipt| {
            for provider in &mut receipt.providers {
                provider.managed_integration = false;
            }
        }),
        ("implicit_skill_visibility", |receipt| {
            for provider in &mut receipt.providers {
                provider.implicit_skill_visibility = !provider.implicit_skill_visibility;
            }
        }),
        // The record identity itself: dropping a reference keeps the destination
        // proven, so the plan stays applicable and only the receipt converges.
        ("assets", |receipt| {
            if let Some(asset) = receipt.assets.first_mut() {
                asset.references.clear();
            }
        }),
    ]
}

fn asset_mtimes(roots: &ResolvedRoots) -> Result<BTreeMap<String, SystemTime>, Box<dyn Error>> {
    let mut observed = BTreeMap::new();
    collect_mtimes(&roots.canonical_skills, &mut observed)?;
    Ok(observed)
}

fn collect_mtimes(
    directory: &Path,
    observed: &mut BTreeMap<String, SystemTime>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        observed.insert(path.display().to_string(), metadata.modified()?);
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            collect_mtimes(&path, observed)?;
        }
    }
    Ok(())
}

#[test]
fn a_residual_lock_cleanup_plans_its_own_receipt_commit() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;
    let committed = install(&catalog, &roots, "residual-install")?;

    // Every path the lock names is already proven by the receipt, so only the
    // legacy lock has to be archived and rewritten. The receipt commit that
    // closes that transaction is still planned, never implicit.
    write_legacy_lock(&roots, &["find-skills"])?;
    let legacy = legacy_plan(&catalog, &roots)?.ok_or("the lock proves no catalog skill")?;
    let decision = decide(
        &catalog,
        &roots,
        Some(&committed),
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Verified(&legacy),
    )?;
    assert!(decision.applicable());
    assert!(!decision.is_current());
    assert!(
        decision
            .receipt_change
            .reasons
            .contains(&ReceiptChangeReason::LegacyLockRewrite)
    );
    assert!(
        decision
            .plan
            .entries
            .iter()
            .any(|entry| entry.action == PlanAction::WriteReceipt)
    );
    assert!(
        decision
            .plan
            .entries
            .iter()
            .all(|entry| entry.action != PlanAction::Adoptable),
        "a receipt-proven destination is never adoptable"
    );

    let transaction = arthur_skills::operations::operations_for_import(
        &decision.plan,
        &roots.legacy_lock_path,
        Some(&legacy),
        &roots,
        &decision.receipt,
        "residual-cleanup",
    )?;
    assert_eq!(
        transaction
            .operations
            .iter()
            .filter(|operation| operation.destination == roots.receipt_path)
            .count(),
        1,
        "every transaction commits exactly one receipt"
    );
    assert_eq!(
        transaction.operations.len(),
        3,
        "archive, lock rewrite and receipt"
    );
    Ok(())
}

#[test]
fn the_json_contract_stays_v1_and_additive() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;
    let committed = install(&catalog, &roots, "json-install")?;

    // A historical v1 receipt omits every optional field; the new code must keep
    // accepting it without inventing ownership.
    let mut historical = serde_json::to_value(&committed)?;
    let object = historical
        .as_object_mut()
        .ok_or("the receipt is not an object")?;
    object.remove("transaction_id");
    object.remove("retained_unmanaged");
    for asset in object["assets"]
        .as_array_mut()
        .ok_or("receipt assets are not an array")?
    {
        asset
            .as_object_mut()
            .ok_or("a receipt asset is not an object")?
            .remove("references");
    }
    let historical = Receipt::decode(&serde_json::to_vec(&historical)?)?;
    assert_eq!(historical.schema_version, committed.schema_version);
    assert!(historical.transaction_id.is_none());

    let decision = decide(
        &catalog,
        &roots,
        Some(&historical),
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Absent,
    )?;
    assert!(decision.applicable());

    // The plan JSON keeps every v1 field and adds the provenance fields.
    let envelope = Envelope::new(Some("plan")).with_plan(&decision.plan);
    let mut bytes = Vec::new();
    write_json(&envelope, &mut bytes)?;
    let projected = serde_json::from_slice::<serde_json::Value>(&bytes)?;
    let operation = projected["operations"]
        .as_array()
        .ok_or("operations are not an array")?
        .first()
        .ok_or("the plan has no operation")?;
    for field in [
        "action",
        "source",
        "destination_utf8",
        "destination_bytes_hex",
        "owner",
        "reason",
    ] {
        assert!(operation.get(field).is_some(), "{field} disappeared");
    }
    assert!(operation["reason"].is_string());
    assert!(operation["action"].is_string());
    assert!(operation["ownership_basis"].is_string());
    assert!(operation["reason_code"].is_string());
    assert!(operation.get("source_id").is_some());
    assert_eq!(projected["schema_version"], 1);
    Ok(())
}

#[test]
fn a_failed_receipt_commit_rolls_back_without_announcing_success() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;
    let committed = install(&catalog, &roots, "rollback-install")?;
    let before_receipt = fs::read(&roots.receipt_path)?;
    let before_assets = asset_mtimes(&roots)?;

    let mut stale = committed.clone();
    stale.catalog_sha256 = "d".repeat(64);
    let decision = decide(
        &catalog,
        &roots,
        Some(&stale),
        &reconcile(&ProviderId::ALL),
        &LegacyEvidence::Absent,
    )?;
    assert!(decision.receipt_change.required);
    let transaction = operations_for_plan(
        &decision.plan,
        &roots,
        &decision.receipt,
        "failed-receipt-commit",
    )?;
    assert_eq!(
        transaction.operations.len(),
        1,
        "a receipt-only decision plans exactly one operation"
    );

    // Failing at the receipt commit must roll back and keep the prior receipt.
    let engine = TransactionEngine::new(roots.state_directory.clone(), SignalFlags::default());
    let mut injector = FailAfterMutation::new(1);
    assert!(
        engine
            .apply_with(
                "failed-receipt-commit",
                transaction.operations,
                &transaction.claims,
                &mut injector,
            )
            .is_err()
    );
    assert_eq!(fs::read(&roots.receipt_path)?, before_receipt);
    assert_eq!(before_assets, asset_mtimes(&roots)?);
    Ok(())
}
