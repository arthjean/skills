#![forbid(unsafe_code)]

//! EP-001 contracts: every planned asset exposes a deterministic ownership
//! basis, and no import, adoption or projected receipt claims a path from its
//! presence or content equality alone.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use arthur_skills::adoption::{self, CatalogEntry, EntryType, LegacyImportPlan};
use arthur_skills::catalog::{AssetKind, Catalog, Provider as CatalogProvider};
use arthur_skills::lifecycle::{LegacyEvidence, LifecycleRequest, RECEIPT_UNREADABLE, decide};
use arthur_skills::plan::{
    MATCHING_UNMANAGED_WITHOUT_PROOF, OwnershipBasis, OwnershipClaim, PlanAction, PlanEntry,
};
use arthur_skills::provider::{ProviderId, ResolvedRoots, resolve_roots_from};
use arthur_skills::receipt::{Receipt, RootScope};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

const PROVEN_SKILL: &str = "find-skills";

fn roots(home: &TempDir) -> Result<ResolvedRoots, Box<dyn Error>> {
    Ok(resolve_roots_from(
        Some(home.path().as_os_str()),
        None,
        &ProviderId::ALL,
    )?)
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
) -> Result<LegacyImportPlan, Box<dyn Error>> {
    let names = catalog
        .manifest()
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Skill)
        .map(|asset| asset.name.clone())
        .collect::<BTreeSet<_>>();
    adoption::inspect_legacy_import(
        &roots.legacy_lock_path,
        &roots.state_directory.join("vercel-skills-v3-lock.json"),
        &names,
    )?
    .ok_or_else(|| "the legacy lock proves no catalog skill".into())
}

/// Materializes one catalog file byte for byte so that only the missing
/// ownership proof, never a content difference, drives the classification.
fn write_matching_catalog_file(
    catalog: &Catalog,
    source_id: &str,
    destination: &Path,
) -> Result<PathBuf, Box<dyn Error>> {
    let embedded = catalog
        .embedded_file(source_id)
        .ok_or("catalog file is missing")?;
    fs::create_dir_all(destination.parent().ok_or("destination has no parent")?)?;
    fs::write(destination, embedded.bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = catalog
            .manifest()
            .assets
            .iter()
            .flat_map(|asset| asset.files.iter())
            .find(|record| record.relative_path == source_id)
            .ok_or("catalog record is missing")?
            .mode;
        fs::set_permissions(destination, fs::Permissions::from_mode(mode))?;
    }
    Ok(destination.to_path_buf())
}

fn first_file(catalog: &Catalog, kind: AssetKind, provider: Option<CatalogProvider>) -> &str {
    catalog
        .manifest()
        .assets
        .iter()
        .filter(|asset| asset.kind == kind && (provider.is_none() || asset.provider == provider))
        .flat_map(|asset| asset.files.iter())
        .map(|record| record.relative_path.as_str())
        .next()
        .unwrap_or_default()
}

fn entry_for<'a>(entries: &'a [PlanEntry], destination: &Path) -> Option<&'a PlanEntry> {
    entries
        .iter()
        .find(|entry| entry.destination == destination)
}

#[test]
fn a_verified_lock_entry_proves_its_skill_and_nothing_else() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home)?;
    let catalog = Catalog::load()?;

    // The lock names one skill, so its canonical directory carries a proof.
    let proven = roots.canonical_skills.join(PROVEN_SKILL);
    fs::create_dir_all(&proven)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&proven, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(unix)]
    let activation = {
        let skills = home.path().join(".claude/skills");
        fs::create_dir_all(&skills)?;
        let activation = skills.join(PROVEN_SKILL);
        std::os::unix::fs::symlink(format!("../../.agents/skills/{PROVEN_SKILL}"), &activation)?;
        activation
    };

    // A Codex agent and a Claude support file matching the catalog byte for
    // byte, both absent from the v3 lock model.
    let agent_source = first_file(&catalog, AssetKind::Agent, Some(CatalogProvider::Codex));
    let agent = write_matching_catalog_file(
        &catalog,
        agent_source,
        &home
            .path()
            .join(".codex/agents")
            .join(Path::new(agent_source).strip_prefix("agents/codex")?),
    )?;
    let support_source = first_file(&catalog, AssetKind::Support, None);
    let support = write_matching_catalog_file(
        &catalog,
        support_source,
        &home
            .path()
            .join(".claude/skills")
            .join(Path::new(support_source).strip_prefix("shared/claude/skills")?),
    )?;

    write_legacy_lock(&roots, &[PROVEN_SKILL])?;
    let legacy = legacy_import_plan(&catalog, &roots)?;
    let transition = decide(
        &catalog,
        &roots,
        None,
        &LifecycleRequest::Reconcile {
            providers: ProviderId::ALL.to_vec(),
        },
        &LegacyEvidence::Verified(&legacy),
    )?;

    let proven_entry = entry_for(&transition.plan.entries, &proven).ok_or("proven skill absent")?;
    assert_eq!(proven_entry.action, PlanAction::Adoptable);
    assert_eq!(
        proven_entry.ownership_basis(),
        OwnershipBasis::VerifiedLegacy
    );
    assert_eq!(
        proven_entry.ownership.source_id().map(String::as_str),
        Some(PROVEN_SKILL)
    );

    #[cfg(unix)]
    {
        let activation_entry =
            entry_for(&transition.plan.entries, &activation).ok_or("activation absent")?;
        assert_eq!(
            activation_entry.action,
            PlanAction::Adoptable,
            "an activation strictly derivable from the proven source_id shares its proof"
        );
        assert_eq!(
            activation_entry.ownership.source_id().map(String::as_str),
            Some(PROVEN_SKILL)
        );
    }

    for foreign in [&agent, &support] {
        let entry = entry_for(&transition.plan.entries, foreign)
            .ok_or_else(|| format!("{} is absent from the plan", foreign.display()))?;
        assert_eq!(
            entry.action,
            PlanAction::Conflict,
            "{} matches the catalog but the v3 lock cannot prove it",
            foreign.display()
        );
        assert_eq!(entry.ownership, OwnershipClaim::None);
        assert!(transition.plan.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == MATCHING_UNMANAGED_WITHOUT_PROOF
                && diagnostic.path_utf8.as_deref() == foreign.to_str()
        }));
        assert!(
            transition.receipt.owned_asset(foreign).is_none(),
            "an unproven path never reaches the projected receipt"
        );
    }
    assert!(!transition.plan.applicable);
    Ok(())
}

#[test]
fn an_unusable_receipt_blocks_the_decision_without_falling_back_to_disk() -> TestResult {
    let home = tempfile::tempdir()?;
    let current = roots(&home)?;
    let catalog = Catalog::load()?;
    let intent = LifecycleRequest::Reconcile {
        providers: ProviderId::ALL.to_vec(),
    };

    // A receipt this CLI cannot verify produces a complete blocked decision: no
    // observed content may become a fallback ownership proof.
    let mut future = Receipt::new("0.1.0", "a".repeat(64), &current);
    future.schema_version = 2;
    let blocked = decide(
        &catalog,
        &current,
        Some(&future),
        &intent,
        &LegacyEvidence::Absent,
    )?;
    assert!(!blocked.applicable());
    assert!(blocked.plan.entries.is_empty());
    assert!(blocked.plan.operations.is_empty());
    assert!(blocked.receipt.assets.is_empty());
    assert!(
        blocked
            .plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == RECEIPT_UNREADABLE)
    );

    let other_home = tempfile::tempdir()?;
    let other_roots = roots(&other_home)?;
    let foreign = Receipt::new("0.1.0", "a".repeat(64), &other_roots);
    let mismatch = decide(
        &catalog,
        &current,
        Some(&foreign),
        &intent,
        &LegacyEvidence::Absent,
    )?;
    assert!(!mismatch.applicable());
    let diagnostic = mismatch
        .plan
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == RECEIPT_UNREADABLE)
        .ok_or("a foreign root identity must block the decision")?;
    assert!(diagnostic.message.contains(&RootScope::Home.to_string()));
    Ok(())
}

#[test]
fn an_invalid_legacy_entry_reports_its_source_and_claims_nothing() -> TestResult {
    let workspace = tempfile::tempdir()?;
    let valid = workspace.path().join("valid.md");
    let invalid = workspace.path().join("invalid.md");
    fs::write(&valid, b"valid")?;
    fs::write(&invalid, b"changed by the user")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&valid, &invalid] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
        }
    }
    let lock = workspace.path().join(".skill-lock.json");
    fs::write(
        &lock,
        serde_json::to_vec_pretty(&serde_json::json!({
            "version": 3,
            "skills": {
                "valid": {
                    "source": "arthjean/skills",
                    "sourceType": "github",
                    "skillFolderHash": "0123456789012345678901234567890123456789",
                    "installedAt": "2026-01-01T00:00:00.000Z",
                    "updatedAt": "2026-01-01T00:00:00.000Z"
                },
                "invalid": {
                    "source": "arthjean/skills",
                    "sourceType": "github",
                    "skillFolderHash": "0123456789012345678901234567890123456789",
                    "installedAt": "2026-01-01T00:00:00.000Z",
                    "updatedAt": "2026-01-01T00:00:00.000Z"
                }
            }
        }))?,
    )?;
    let entries = [
        CatalogEntry {
            source_id: "valid".to_owned(),
            destination: valid,
            entry_type: EntryType::File,
            sha256: Some(sha256(b"valid")),
            mode: 0o644,
            link_target: None,
        },
        CatalogEntry {
            source_id: "invalid".to_owned(),
            destination: invalid.clone(),
            entry_type: EntryType::File,
            sha256: Some(sha256(b"expected catalog bytes")),
            mode: 0o644,
            link_target: None,
        },
    ];

    let plan = adoption::inspect(&lock, &workspace.path().join("archive.json"), &entries)?;

    assert!(!plan.applicable);
    assert!(
        plan.entries.is_empty(),
        "one invalid entry cancels every claim"
    );
    let diagnostic = plan
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.destination.as_deref() == Some(invalid.as_path()))
        .ok_or("the invalid entry produced no diagnostic")?;
    assert_eq!(diagnostic.source_id.as_deref(), Some("invalid"));
    Ok(())
}

#[test]
fn a_lock_key_that_is_not_one_directory_proves_nothing() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = roots(&home)?;
    let catalog = Catalog::load()?;
    let escaping = "../../escaped";
    // An empty key is already rejected when the lock is parsed.
    write_legacy_lock(&roots, &[PROVEN_SKILL, escaping, "nested/skill", "."])?;

    let legacy = legacy_import_plan(&catalog, &roots)?;

    assert_eq!(legacy.managed_skill_names, vec![PROVEN_SKILL.to_owned()]);
    assert!(legacy.obsolete_skill_names.is_empty());
    let residual = serde_json::from_slice::<serde_json::Value>(&legacy.residual_bytes)?;
    for name in [escaping, "nested/skill", "."] {
        assert!(
            residual["skills"].get(name).is_some(),
            "an unusable key stays foreign in the residual lock: {name}"
        );
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
