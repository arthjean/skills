use std::error::Error;
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use arthur_skills::adoption::{self, LegacyImportPlan};
use arthur_skills::catalog::{AssetKind, Catalog, Provider as CatalogProvider};
use arthur_skills::lifecycle::{
    LegacyEvidence, LifecycleDecision, LifecycleNoticeCode, LifecycleRequest, UNSAFE_CONTAINER,
    decide,
};
use arthur_skills::operations::{operations_for_import, operations_for_plan};
use arthur_skills::plan::{
    MATCHING_UNMANAGED_WITHOUT_PROOF, OwnershipBasis, OwnershipClaim, PlanAction, PlanReason,
};
use arthur_skills::provider::{ProviderId, ResolvedRoots, resolve_roots_from};
use arthur_skills::receipt::{OwnedAsset, OwnedAssetKind, Receipt, RetainedUnmanagedAsset};
use arthur_skills::transaction::{
    FailAfterMutation, SignalFlags, TransactionEngine, TransactionOutcome, hash_bytes,
};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

#[cfg(unix)]
fn observed_mode(path: &Path) -> Result<u32, std::io::Error> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o777)
}

#[cfg(windows)]
fn observed_mode(path: &Path) -> Result<u32, std::io::Error> {
    Ok(if fs::metadata(path)?.permissions().readonly() {
        0o444
    } else {
        0o644
    })
}

#[cfg(unix)]
fn set_directory_mode(path: &Path, mode: u32) -> Result<(), std::io::Error> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(windows)]
fn set_directory_mode(_path: &Path, _mode: u32) -> Result<(), std::io::Error> {
    Ok(())
}

fn roots(home: &TempDir, providers: &[ProviderId]) -> Result<ResolvedRoots, Box<dyn Error>> {
    Ok(resolve_roots_from(
        Some(home.path().as_os_str()),
        None,
        providers,
    )?)
}

fn apply(
    roots: &ResolvedRoots,
    transition: &LifecycleDecision,
    transaction_id: &str,
) -> TestResult {
    let transaction =
        operations_for_plan(&transition.plan, roots, &transition.receipt, transaction_id)?;
    let engine = TransactionEngine::new(roots.state_directory.clone(), SignalFlags::default());
    assert_eq!(
        engine.apply(transaction_id, transaction.operations, &transaction.claims)?,
        TransactionOutcome::Committed
    );
    Ok(())
}

fn install_both(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    transaction_id: &str,
) -> Result<LifecycleDecision, Box<dyn Error>> {
    let transition = decide(
        catalog,
        roots,
        None,
        &LifecycleRequest::Reconcile {
            providers: vec![ProviderId::Claude, ProviderId::Codex],
        },
        &LegacyEvidence::Absent,
    )?;
    assert!(transition.plan.applicable);
    apply(roots, &transition, transaction_id)?;
    Ok(transition)
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

fn legacy_import_plan(
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

fn provider_managed(receipt: &Receipt, provider: ProviderId) -> bool {
    receipt
        .providers
        .iter()
        .find(|entry| entry.provider == provider)
        .is_some_and(|entry| entry.managed_integration)
}

#[test]
fn fresh_install_materializes_the_catalog_without_claiming_foreign_assets() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;

    let personal_skill = roots.canonical_skills.join("personal/SKILL.md");
    let claude_personal = home.path().join(".claude/agents/personal.md");
    let codex_personal = home.path().join(".codex/agents/personal.toml");
    fs::create_dir_all(
        personal_skill
            .parent()
            .ok_or("personal skill has no parent")?,
    )?;
    fs::create_dir_all(
        claude_personal
            .parent()
            .ok_or("Claude agent has no parent")?,
    )?;
    fs::create_dir_all(codex_personal.parent().ok_or("Codex agent has no parent")?)?;
    fs::write(&personal_skill, b"personal")?;
    fs::write(&claude_personal, b"personal")?;
    fs::write(&codex_personal, b"personal")?;

    let transition = install_both(&catalog, &roots, "fresh-install")?;

    assert_eq!(fs::read(&personal_skill)?, b"personal");
    assert_eq!(fs::read(&claude_personal)?, b"personal");
    assert_eq!(fs::read(&codex_personal)?, b"personal");
    assert!(transition.receipt.owned_asset(&personal_skill).is_none());
    assert!(transition.receipt.owned_asset(&claude_personal).is_none());
    assert!(transition.receipt.owned_asset(&codex_personal).is_none());
    assert!(provider_managed(&transition.receipt, ProviderId::Claude));
    assert!(provider_managed(&transition.receipt, ProviderId::Codex));

    for asset in &catalog.manifest().assets {
        match asset.kind {
            AssetKind::Skill => {
                let name = Path::new(&asset.relative_path)
                    .file_name()
                    .ok_or("skill has no name")?;
                let activation = home.path().join(".claude/skills").join(name);
                #[cfg(unix)]
                assert_eq!(
                    fs::read_link(&activation)?,
                    Path::new("../../.agents/skills").join(name)
                );
                #[cfg(windows)]
                assert!(activation.is_dir());
                for record in &asset.files {
                    let relative = Path::new(&record.relative_path).strip_prefix("skills")?;
                    let destination = roots.canonical_skills.join(relative);
                    let embedded = catalog
                        .embedded_file(&record.relative_path)
                        .ok_or("embedded skill file is missing")?;
                    assert_eq!(fs::read(&destination)?, embedded.bytes);
                    assert_eq!(
                        observed_mode(&destination)?,
                        if cfg!(windows) { 0o644 } else { record.mode }
                    );
                    #[cfg(windows)]
                    assert_eq!(
                        fs::read(home.path().join(".claude/skills").join(relative))?,
                        embedded.bytes
                    );
                }
            }
            AssetKind::Agent => {
                let record = asset.files.first().ok_or("agent record is missing")?;
                let destination = match asset.provider {
                    Some(CatalogProvider::Claude) => home
                        .path()
                        .join(".claude/agents")
                        .join(Path::new(&record.relative_path).strip_prefix("agents/claude")?),
                    Some(CatalogProvider::Codex) => home
                        .path()
                        .join(".codex/agents")
                        .join(Path::new(&record.relative_path).strip_prefix("agents/codex")?),
                    None => return Err("agent provider is missing".into()),
                };
                assert_eq!(
                    fs::read(destination)?,
                    catalog
                        .embedded_file(&record.relative_path)
                        .ok_or("embedded agent is missing")?
                        .bytes
                );
            }
            AssetKind::Support => {
                let record = asset.files.first().ok_or("support record is missing")?;
                let relative =
                    Path::new(&record.relative_path).strip_prefix("shared/claude/skills")?;
                assert_eq!(
                    fs::read(home.path().join(".claude/skills").join(relative))?,
                    catalog
                        .embedded_file(&record.relative_path)
                        .ok_or("embedded support is missing")?
                        .bytes
                );
            }
        }
    }
    assert!(!home.path().join(".codex/skills").exists());
    assert!(
        transition
            .notices
            .iter()
            .any(|notice| { notice.code == LifecycleNoticeCode::CodexUsesImplicitSkills })
    );
    Ok(())
}

#[test]
fn homonyms_require_adoption_and_owned_drift_blocks_reconciliation() -> TestResult {
    let collision_home = tempfile::tempdir()?;
    let collision_roots = roots(&collision_home, &[ProviderId::Codex])?;
    let catalog = Catalog::load()?;
    let collision = collision_roots.canonical_skills.join("find-skills");
    fs::create_dir_all(&collision)?;
    set_directory_mode(&collision, 0o755)?;
    let blocked = decide(
        &catalog,
        &collision_roots,
        None,
        &LifecycleRequest::Reconcile {
            providers: vec![ProviderId::Codex],
        },
        &LegacyEvidence::Absent,
    )?;
    assert!(!blocked.plan.applicable);
    assert!(
        blocked.plan.entries.iter().any(|entry| {
            entry.destination == collision
                && entry.action == PlanAction::Conflict
                && entry.ownership == OwnershipClaim::None
        }),
        "a homonym without proof is a conflict, never an adoption candidate"
    );
    assert!(blocked.plan.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == MATCHING_UNMANAGED_WITHOUT_PROOF
            && diagnostic.path_utf8.as_deref() == collision.to_str()
    }));
    assert!(blocked.receipt.owned_asset(&collision).is_none());

    write_legacy_lock(&collision_roots, &["find-skills"])?;
    let legacy = legacy_import_plan(&catalog, &collision_roots)?
        .ok_or("the legacy lock proves no catalog skill")?;
    let proven = decide(
        &catalog,
        &collision_roots,
        None,
        &LifecycleRequest::Reconcile {
            providers: vec![ProviderId::Codex],
        },
        &LegacyEvidence::Verified(&legacy),
    )?;
    let adoptable = proven
        .plan
        .entries
        .iter()
        .find(|entry| entry.destination == collision)
        .ok_or("the proven collision is absent from the plan")?;
    assert_eq!(adoptable.action, PlanAction::Adoptable);
    assert_eq!(adoptable.ownership_basis(), OwnershipBasis::VerifiedLegacy);
    assert_eq!(
        adoptable.ownership.source_id().map(String::as_str),
        Some("find-skills")
    );
    assert!(
        proven
            .plan
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != MATCHING_UNMANAGED_WITHOUT_PROOF)
    );

    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let installed = install_both(&catalog, &roots, "drift-install")?;
    let managed_file = roots.canonical_skills.join("find-skills/SKILL.md");
    fs::write(&managed_file, b"local edit")?;
    let drifted = decide(
        &catalog,
        &roots,
        Some(&installed.receipt),
        &LifecycleRequest::Reconcile {
            providers: vec![ProviderId::Claude, ProviderId::Codex],
        },
        &LegacyEvidence::Absent,
    )?;
    assert!(!drifted.plan.applicable);
    assert!(
        drifted.plan.entries.iter().any(|entry| {
            entry.destination == managed_file && entry.action == PlanAction::Drifted
        })
    );

    let mut prior = installed.receipt.clone();
    let old_bytes = b"prior catalog version";
    fs::write(&managed_file, old_bytes)?;
    let owned = prior
        .assets
        .iter_mut()
        .find(|asset| asset.destination == managed_file)
        .ok_or("managed file ownership is missing")?;
    owned.hash = Some(hash_bytes(old_bytes));
    let update = decide(
        &catalog,
        &roots,
        Some(&prior),
        &LifecycleRequest::Reconcile {
            providers: vec![ProviderId::Claude, ProviderId::Codex],
        },
        &LegacyEvidence::Absent,
    )?;
    assert!(update.plan.applicable);
    assert!(
        update.plan.entries.iter().any(|entry| {
            entry.destination == managed_file && entry.action == PlanAction::Update
        })
    );
    assert_eq!(
        update
            .plan
            .operations
            .iter()
            .filter(|operation| operation.destination == managed_file)
            .count(),
        1
    );
    Ok(())
}

#[test]
fn import_and_reconciliation_repair_catalog_assets_without_claiming_personal_assets() -> TestResult
{
    let catalog = Catalog::load()?;

    // A Vercel Skills v3 installation: canonical skills the lock names, an
    // unrelated personal skill, and no Arthur agent or support file.
    let import_home = tempfile::tempdir()?;
    let import_roots = roots(&import_home, &ProviderId::ALL)?;
    let drifted_skill = import_roots.canonical_skills.join("find-skills/SKILL.md");
    let missing_agent = import_home
        .path()
        .join(".codex/agents/docs-researcher.toml");
    let personal = import_roots.canonical_skills.join("personal/SKILL.md");
    for path in [&drifted_skill, &personal] {
        fs::create_dir_all(path.parent().ok_or("legacy skill has no parent")?)?;
    }
    fs::write(&drifted_skill, b"old Arthur content")?;
    fs::write(&personal, b"personal")?;
    write_legacy_lock(&import_roots, &["find-skills"])?;
    let legacy = legacy_import_plan(&catalog, &import_roots)?
        .ok_or("the legacy lock proves no catalog skill")?;

    let imported = decide(
        &catalog,
        &import_roots,
        None,
        &LifecycleRequest::Import {
            providers: ProviderId::ALL.to_vec(),
        },
        &LegacyEvidence::Verified(&legacy),
    )?;
    assert!(
        imported.plan.applicable,
        "{:?}",
        imported
            .plan
            .entries
            .iter()
            .filter(|entry| entry.action == PlanAction::Conflict)
            .collect::<Vec<_>>()
    );
    assert!(
        imported.plan.entries.iter().any(|entry| {
            entry.destination == drifted_skill && entry.action == PlanAction::Update
        })
    );
    assert!(
        imported.plan.entries.iter().any(|entry| {
            entry.destination == missing_agent && entry.action == PlanAction::Create
        })
    );
    // The lock proves one skill; nothing else observed on disk may be claimed.
    assert!(imported.plan.entries.iter().all(|entry| {
        entry.ownership_basis() != OwnershipBasis::VerifiedLegacy
            || entry
                .destination
                .starts_with(import_roots.canonical_skills.join("find-skills"))
            || entry
                .destination
                .starts_with(import_home.path().join(".claude/skills/find-skills"))
    }));
    assert!(imported.receipt.owned_asset(&personal).is_none());

    let transaction = operations_for_import(
        &imported.plan,
        &import_roots.legacy_lock_path,
        Some(&legacy),
        &import_roots,
        &imported.receipt,
        "import-existing",
    )?;
    let engine =
        TransactionEngine::new(import_roots.state_directory.clone(), SignalFlags::default());
    assert_eq!(
        engine.apply(
            "import-existing",
            transaction.operations,
            &transaction.claims
        )?,
        TransactionOutcome::Committed
    );
    assert_eq!(fs::read(&personal)?, b"personal");
    let committed = Receipt::decode(&fs::read(&import_roots.receipt_path)?)?;
    assert!(committed.owned_asset(&personal).is_none());

    // A managed asset that differs from its receipt proof and from the catalog is
    // preserved: reconciliation blocks instead of planning a destructive update.
    let update_home = tempfile::tempdir()?;
    let update_roots = roots(&update_home, &ProviderId::ALL)?;
    let installed = install_both(&catalog, &update_roots, "prepare-update")?;
    let drifted_agent = update_home.path().join(".codex/agents/agent-explorer.toml");
    let original_agent = fs::read(&drifted_agent)?;
    fs::write(&drifted_agent, b"locally misaligned")?;
    let updated = decide(
        &catalog,
        &update_roots,
        Some(&installed.receipt),
        &LifecycleRequest::Reconcile {
            providers: ProviderId::ALL.to_vec(),
        },
        &LegacyEvidence::Absent,
    )?;
    assert!(!updated.plan.applicable);
    assert!(updated.plan.entries.iter().any(|entry| {
        entry.destination == drifted_agent
            && entry.action == PlanAction::Drifted
            && entry.reason == PlanReason::ManagedPathDrifted
    }));
    assert!(
        updated
            .plan
            .operations
            .iter()
            .all(|operation| operation.destination != drifted_agent)
    );
    assert!(
        operations_for_plan(
            &updated.plan,
            &update_roots,
            &updated.receipt,
            "reconcile-existing",
        )
        .is_err()
    );
    assert_eq!(fs::read(&drifted_agent)?, b"locally misaligned");

    // Restoring the recorded proof makes the same request converge again.
    fs::write(&drifted_agent, &original_agent)?;
    let converged = decide(
        &catalog,
        &update_roots,
        Some(&installed.receipt),
        &LifecycleRequest::Reconcile {
            providers: ProviderId::ALL.to_vec(),
        },
        &LegacyEvidence::Absent,
    )?;
    assert!(converged.plan.applicable);
    assert!(converged.is_current());
    Ok(())
}

#[test]
fn provider_uninstall_preserves_references_and_full_uninstall_releases_drift() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;
    let installed = install_both(&catalog, &roots, "lifecycle-install")?;

    let codex_only_removal = decide(
        &catalog,
        &roots,
        Some(&installed.receipt),
        &LifecycleRequest::UninstallProvider(ProviderId::Codex),
        &LegacyEvidence::Absent,
    )?;
    assert!(codex_only_removal.plan.applicable);
    assert!(codex_only_removal.plan.entries.iter().all(|entry| {
        !entry.destination.starts_with(&roots.canonical_skills) || entry.action == PlanAction::Noop
    }));
    assert!(codex_only_removal.notices.iter().any(|notice| {
        notice.code == LifecycleNoticeCode::CodexIntegrationRemovedSkillsRemainVisible
    }));

    let claude_removed = decide(
        &catalog,
        &roots,
        Some(&installed.receipt),
        &LifecycleRequest::UninstallProvider(ProviderId::Claude),
        &LegacyEvidence::Absent,
    )?;
    assert!(claude_removed.plan.applicable);
    assert!(claude_removed.plan.entries.iter().all(|entry| {
        !entry.destination.starts_with(&roots.canonical_skills) || entry.action == PlanAction::Noop
    }));
    apply(&roots, &claude_removed, "remove-claude")?;
    assert!(!provider_managed(
        &claude_removed.receipt,
        ProviderId::Claude
    ));
    assert!(provider_managed(&claude_removed.receipt, ProviderId::Codex));
    assert!(roots.canonical_skills.join("find-skills/SKILL.md").exists());
    assert!(
        home.path()
            .join(".codex/agents/agent-explorer.toml")
            .exists()
    );
    assert!(!home.path().join(".claude/agents/agent-docs.md").exists());
    assert!(!home.path().join(".claude/skills/_shared").exists());
    for asset in claude_removed
        .receipt
        .assets
        .iter()
        .filter(|asset| asset.destination.starts_with(&roots.canonical_skills))
    {
        assert_eq!(asset.references, vec![ProviderId::Codex]);
    }

    let drifted_agent = home.path().join(".codex/agents/agent-explorer.toml");
    let missing_agent = home.path().join(".codex/agents/docs-researcher.toml");
    let personal_agent = home.path().join(".codex/agents/personal.toml");
    fs::write(&drifted_agent, b"local edit")?;
    fs::remove_file(&missing_agent)?;
    fs::write(&personal_agent, b"personal")?;
    let uninstall_all = decide(
        &catalog,
        &roots,
        Some(&claude_removed.receipt),
        &LifecycleRequest::UninstallAll,
        &LegacyEvidence::Absent,
    )?;
    assert!(uninstall_all.plan.applicable);
    assert!(uninstall_all.plan.entries.iter().any(|entry| {
        entry.destination == drifted_agent && entry.action == PlanAction::RetainedUnmanaged
    }));
    assert!(
        uninstall_all.plan.entries.iter().any(|entry| {
            entry.destination == missing_agent && entry.action == PlanAction::Remove
        })
    );
    assert!(uninstall_all.receipt.assets.is_empty());
    assert!(
        uninstall_all
            .receipt
            .retained_unmanaged
            .iter()
            .any(|entry| entry.destination == drifted_agent)
    );
    apply(&roots, &uninstall_all, "remove-all")?;
    assert_eq!(fs::read(&drifted_agent)?, b"local edit");
    assert_eq!(fs::read(&personal_agent)?, b"personal");
    assert!(!roots.canonical_skills.join("find-skills").exists());
    let committed = Receipt::decode(&fs::read(&roots.receipt_path)?)?;
    assert!(committed.assets.is_empty());
    assert!(
        committed
            .retained_unmanaged
            .iter()
            .any(|entry| entry.destination == drifted_agent)
    );
    Ok(())
}

#[test]
fn retained_unmanaged_receipt_records_fail_closed() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &[ProviderId::Codex])?;
    let destination = roots.canonical_skills.join("retained/SKILL.md");
    let retained = RetainedUnmanagedAsset {
        source_id: "retained".to_owned(),
        destination: destination.clone(),
        reason: "locally modified".to_owned(),
    };

    let mut empty_source = Receipt::new("0.1.0", "a".repeat(64), &roots);
    empty_source
        .retained_unmanaged
        .push(RetainedUnmanagedAsset {
            source_id: String::new(),
            ..retained.clone()
        });
    assert!(empty_source.validate().is_err());

    let mut empty_reason = Receipt::new("0.1.0", "a".repeat(64), &roots);
    empty_reason
        .retained_unmanaged
        .push(RetainedUnmanagedAsset {
            reason: String::new(),
            ..retained.clone()
        });
    assert!(empty_reason.validate().is_err());

    let mut outside = Receipt::new("0.1.0", "a".repeat(64), &roots);
    outside.retained_unmanaged.push(RetainedUnmanagedAsset {
        destination: home.path().join("outside"),
        ..retained.clone()
    });
    assert!(outside.validate().is_err());

    let mut duplicate = Receipt::new("0.1.0", "a".repeat(64), &roots);
    duplicate.retained_unmanaged = vec![retained.clone(), retained.clone()];
    assert!(duplicate.validate().is_err());

    let mut owned_collision = Receipt::new("0.1.0", "a".repeat(64), &roots);
    owned_collision.assets.push(OwnedAsset {
        source_id: "owned".to_owned(),
        destination,
        kind: OwnedAssetKind::File,
        hash: Some("b".repeat(64)),
        mode: Some(0o644),
        link_target: None,
        references: vec![ProviderId::Codex],
    });
    owned_collision.retained_unmanaged.push(retained);
    assert!(owned_collision.validate().is_err());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_foreign_entry_blocks_uninstall_before_mutation() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &[ProviderId::Codex])?;
    let catalog = Catalog::load()?;
    let installed = decide(
        &catalog,
        &roots,
        None,
        &LifecycleRequest::Reconcile {
            providers: vec![ProviderId::Codex],
        },
        &LegacyEvidence::Absent,
    )?;
    apply(&roots, &installed, "non-utf8-install")?;

    let foreign = home
        .path()
        .join(".codex/agents")
        .join(std::ffi::OsString::from_vec(vec![0xff, 0xfe]));
    fs::write(&foreign, b"foreign")?;
    let uninstall = decide(
        &catalog,
        &roots,
        Some(&installed.receipt),
        &LifecycleRequest::UninstallAll,
        &LegacyEvidence::Absent,
    )?;
    assert!(!uninstall.plan.applicable);
    assert!(
        uninstall
            .plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "filesystem_scan_failed")
    );
    assert_eq!(fs::read(foreign)?, b"foreign");
    Ok(())
}

#[test]
fn provider_container_type_conflicts_block_planning() -> TestResult {
    let catalog = Catalog::load()?;
    let cases = [
        (ProviderId::Codex, ".codex"),
        (ProviderId::Claude, ".claude/skills"),
        (ProviderId::Claude, ".claude/agents"),
        (ProviderId::Claude, ".claude/skills/_shared"),
    ];
    for (provider, relative) in cases {
        let home = tempfile::tempdir()?;
        let roots = roots(&home, &[provider])?;
        let conflict = home.path().join(relative);
        fs::create_dir_all(conflict.parent().ok_or("conflict has no parent")?)?;
        fs::write(&conflict, b"wrong type")?;
        let blocked = decide(
            &catalog,
            &roots,
            None,
            &LifecycleRequest::Reconcile {
                providers: vec![provider],
            },
            &LegacyEvidence::Absent,
        )?;
        assert!(!blocked.applicable());
        assert!(blocked.plan.operations.is_empty());
        assert!(
            blocked
                .plan
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == UNSAFE_CONTAINER)
        );
    }
    Ok(())
}

#[test]
fn uninstall_failure_rolls_back_assets_and_receipt_together() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home, &ProviderId::ALL)?;
    let catalog = Catalog::load()?;
    let installed = install_both(&catalog, &roots, "rollback-install")?;
    let before_receipt = fs::read(&roots.receipt_path)?;
    let claude_agent = home.path().join(".claude/agents/agent-docs.md");
    let before_agent = fs::read(&claude_agent)?;
    let transition = decide(
        &catalog,
        &roots,
        Some(&installed.receipt),
        &LifecycleRequest::UninstallProvider(ProviderId::Claude),
        &LegacyEvidence::Absent,
    )?;
    let transaction = operations_for_plan(
        &transition.plan,
        &roots,
        &transition.receipt,
        "rollback-uninstall",
    )?;
    let engine = TransactionEngine::new(roots.state_directory.clone(), SignalFlags::default());
    let mut injector = FailAfterMutation::new(8);
    assert!(
        engine
            .apply_with(
                "rollback-uninstall",
                transaction.operations,
                &transaction.claims,
                &mut injector,
            )
            .is_err()
    );
    assert_eq!(fs::read(&claude_agent)?, before_agent);
    assert_eq!(fs::read(&roots.receipt_path)?, before_receipt);
    Ok(())
}
