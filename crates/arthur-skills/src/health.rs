use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::catalog::Catalog;
use crate::lifecycle::{LifecycleIntent, prepare_lifecycle_transition};
use crate::plan::PlanAction;
use crate::platform::{effective_directory_mode, effective_file_mode, metadata_mode};
use crate::provider::{ProviderId, ResolvedRoots};
use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt};
use crate::transaction::{PathKind, snapshot_path};

mod probes;

pub use probes::{CapabilityProbe, ProviderProbe};
use probes::{detect_providers, inspect_capabilities, inspect_providers};

pub(crate) fn detected_providers() -> Vec<ProviderId> {
    detect_providers()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HealthIssue {
    pub code: String,
    pub severity: IssueSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AssetCounts {
    pub healthy: usize,
    pub drifted: usize,
    pub missing: usize,
    pub conflicting: usize,
    pub foreign: usize,
    pub retained_unmanaged: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallationHealth {
    pub counts: AssetCounts,
    pub roots_match: bool,
    pub catalog_current: bool,
    pub healthy: bool,
    pub provider_probes: Vec<ProviderProbe>,
    pub capability_probes: Vec<CapabilityProbe>,
    pub issues: Vec<HealthIssue>,
}

pub fn inspect_status(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    receipt: &Receipt,
) -> InstallationHealth {
    inspect(catalog, roots, receipt, false)
}

pub fn inspect_doctor(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    receipt: &Receipt,
) -> InstallationHealth {
    inspect(catalog, roots, receipt, true)
}

fn inspect(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    receipt: &Receipt,
    execute_providers: bool,
) -> InstallationHealth {
    let mut counts = AssetCounts {
        retained_unmanaged: receipt.retained_unmanaged.len(),
        ..AssetCounts::default()
    };
    let mut issues = Vec::new();
    let roots_match = match receipt.validate_roots(roots) {
        Ok(()) => true,
        Err(error) => {
            issues.push(issue(
                "root_mismatch",
                IssueSeverity::Error,
                error.to_string(),
                None,
            ));
            false
        }
    };
    inspect_permissions(roots, &mut issues);
    let (provider_probes, capability_probes) = if roots_match {
        for asset in &receipt.assets {
            inspect_asset(asset, &mut counts, &mut issues);
        }
        inspect_catalog_coverage(catalog, roots, receipt, &mut counts, &mut issues);
        counts.foreign = count_foreign(receipt, &mut issues);
        (
            inspect_providers(catalog, receipt, execute_providers, &mut issues),
            inspect_capabilities(catalog, &mut issues),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let catalog_current = receipt.catalog_sha256 == catalog.manifest().catalog_sha256;
    if !catalog_current {
        issues.push(issue(
            "catalog_update_available",
            IssueSeverity::Error,
            "the installed receipt targets a different embedded catalog",
            None,
        ));
    }
    let healthy = roots_match
        && catalog_current
        && counts.drifted == 0
        && counts.missing == 0
        && counts.conflicting == 0
        && !issues
            .iter()
            .any(|issue| issue.severity == IssueSeverity::Error);

    InstallationHealth {
        counts,
        roots_match,
        catalog_current,
        healthy,
        provider_probes,
        capability_probes,
        issues,
    }
}

fn inspect_asset(asset: &OwnedAsset, counts: &mut AssetCounts, issues: &mut Vec<HealthIssue>) {
    let snapshot = match snapshot_path(&asset.destination) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            counts.conflicting += 1;
            issues.push(issue(
                "asset_inspection_failed",
                IssueSeverity::Error,
                error.to_string(),
                Some(asset.destination.clone()),
            ));
            return;
        }
    };
    if snapshot.kind == PathKind::Absent {
        counts.missing += 1;
        issues.push(issue(
            "asset_missing",
            IssueSeverity::Error,
            "managed asset is missing",
            Some(asset.destination.clone()),
        ));
        return;
    }
    let expected_kind = match asset.kind {
        OwnedAssetKind::File => PathKind::File,
        OwnedAssetKind::Directory => PathKind::Directory,
        OwnedAssetKind::Symlink => PathKind::Symlink,
    };
    if snapshot.kind != expected_kind {
        counts.conflicting += 1;
        issues.push(issue(
            "asset_type_conflict",
            IssueSeverity::Error,
            "managed asset has an unexpected filesystem type",
            Some(asset.destination.clone()),
        ));
        return;
    }
    let drifted = match asset.kind {
        OwnedAssetKind::File => snapshot.sha256 != asset.hash || snapshot.mode != asset.mode,
        OwnedAssetKind::Directory => snapshot.mode != asset.mode,
        OwnedAssetKind::Symlink => snapshot.link_target != asset.link_target,
    };
    if drifted {
        counts.drifted += 1;
        issues.push(issue(
            "asset_drifted",
            IssueSeverity::Error,
            "managed asset content, mode, or symlink target differs from the receipt",
            Some(asset.destination.clone()),
        ));
    } else {
        counts.healthy += 1;
    }
}

fn inspect_catalog_coverage(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    receipt: &Receipt,
    counts: &mut AssetCounts,
    issues: &mut Vec<HealthIssue>,
) {
    let providers = receipt
        .providers
        .iter()
        .filter(|provider| provider.managed_integration)
        .map(|provider| provider.provider)
        .collect::<Vec<_>>();
    let transition = match prepare_lifecycle_transition(
        catalog,
        roots,
        Some(receipt),
        &LifecycleIntent::Install { providers },
    ) {
        Ok(transition) => transition,
        Err(error) => {
            issues.push(issue(
                "catalog_inspection_failed",
                IssueSeverity::Error,
                error.to_string(),
                None,
            ));
            return;
        }
    };
    let recorded = receipt
        .assets
        .iter()
        .map(|asset| asset.destination.as_path())
        .collect::<BTreeSet<_>>();
    let mut deviations = 0;
    for entry in transition.plan.entries.iter().filter(|entry| {
        !matches!(
            entry.action,
            PlanAction::Noop | PlanAction::RetainedUnmanaged
        )
    }) {
        deviations += 1;
        if recorded.contains(entry.destination.as_path()) {
            continue;
        }
        match entry.action {
            PlanAction::Create => counts.missing += 1,
            PlanAction::Adoptable | PlanAction::Conflict => counts.conflicting += 1,
            _ => {}
        }
    }
    if deviations != 0 {
        issues.push(issue(
            "catalog_reconciliation_required",
            IssueSeverity::Error,
            format!("{deviations} planned entries differ from the embedded catalog"),
            None,
        ));
    }
}

fn count_foreign(receipt: &Receipt, issues: &mut Vec<HealthIssue>) -> usize {
    let owned = receipt
        .assets
        .iter()
        .map(|asset| asset.destination.clone())
        .collect::<BTreeSet<_>>();
    let retained = receipt
        .retained_unmanaged
        .iter()
        .map(|asset| asset.destination.clone())
        .collect::<BTreeSet<_>>();
    managed_surfaces(receipt)
        .iter()
        .map(|surface| count_foreign_under(surface, &owned, &retained, issues))
        .sum()
}

fn managed_surfaces(receipt: &Receipt) -> BTreeSet<PathBuf> {
    let mut surfaces = BTreeSet::from([receipt.roots.canonical.lexical.join("skills")]);
    for provider in receipt
        .providers
        .iter()
        .filter(|provider| provider.managed_integration)
    {
        let Some(root) = &provider.root else {
            continue;
        };
        match provider.provider {
            ProviderId::Claude => {
                surfaces.insert(root.lexical.join("skills"));
                surfaces.insert(root.lexical.join("agents"));
            }
            ProviderId::Codex => {
                surfaces.insert(root.lexical.join("agents"));
            }
        }
    }
    surfaces
}

fn count_foreign_under(
    directory: &Path,
    owned: &BTreeSet<PathBuf>,
    retained: &BTreeSet<PathBuf>,
    issues: &mut Vec<HealthIssue>,
) -> usize {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return 0,
        Err(error) => {
            issues.push(issue(
                "surface_unreadable",
                IssueSeverity::Error,
                error.to_string(),
                Some(directory.to_path_buf()),
            ));
            return 0;
        }
    };
    let mut foreign = 0;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                issues.push(issue(
                    "surface_unreadable",
                    IssueSeverity::Error,
                    error.to_string(),
                    Some(directory.to_path_buf()),
                ));
                continue;
            }
        };
        let path = entry.path();
        if retained.contains(&path) {
            continue;
        }
        let contains_known = owned
            .iter()
            .chain(retained.iter())
            .any(|candidate| candidate.starts_with(&path));
        if owned.contains(&path) || contains_known {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                foreign += count_foreign_under(&path, owned, retained, issues);
            }
        } else {
            foreign += 1;
        }
    }
    foreign
}

fn inspect_permissions(roots: &ResolvedRoots, issues: &mut Vec<HealthIssue>) {
    for root in roots.allowed_top_level_roots() {
        inspect_owner_writable(&root.lexical, issues);
    }
    inspect_private_mode(&roots.state_directory, 0o700, issues);
    inspect_private_mode(&roots.receipt_path, 0o600, issues);
}

fn inspect_owner_writable(path: &Path, issues: &mut Vec<HealthIssue>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if !owner_writable(&metadata) {
        issues.push(issue(
            "root_not_writable",
            IssueSeverity::Error,
            "managed root is not owner-writable",
            Some(path.to_path_buf()),
        ));
    }
}

#[cfg(unix)]
fn owner_writable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o200 != 0
}

#[cfg(windows)]
fn owner_writable(metadata: &fs::Metadata) -> bool {
    !metadata.permissions().readonly()
}

fn inspect_private_mode(path: &Path, expected: u32, issues: &mut Vec<HealthIssue>) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    let expected = if metadata.is_dir() {
        effective_directory_mode(expected)
    } else {
        effective_file_mode(expected)
    };
    let observed = metadata_mode(&metadata) & 0o777;
    if observed != expected {
        issues.push(issue(
            "state_permissions_invalid",
            IssueSeverity::Error,
            format!("expected mode {expected:04o}, observed {observed:04o}"),
            Some(path.to_path_buf()),
        ));
    }
}

fn issue(
    code: &str,
    severity: IssueSeverity,
    message: impl Into<String>,
    path: Option<PathBuf>,
) -> HealthIssue {
    HealthIssue {
        code: code.to_owned(),
        severity,
        message: message.into(),
        path,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::collections::BTreeSet;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    use tempfile::{TempDir, tempdir};

    use super::{AssetCounts, IssueSeverity, inspect_asset, inspect_status};
    use crate::catalog::Catalog;
    use crate::provider::{ProviderId, ResolvedRoots, resolve_roots_from};
    use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt, RetainedUnmanagedAsset};
    use crate::transaction::{hash_bytes, snapshot_path};

    fn fixture(selected: &[ProviderId]) -> (TempDir, ResolvedRoots, Receipt) {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, selected)
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        (home, roots, receipt)
    }

    fn asset(destination: PathBuf, kind: OwnedAssetKind) -> OwnedAsset {
        OwnedAsset {
            source_id: "skills/example/SKILL.md".to_owned(),
            destination,
            kind,
            hash: None,
            mode: None,
            link_target: None,
            references: Vec::new(),
        }
    }

    #[test]
    fn asset_inspection_classifies_every_owned_shape() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let file_path = directory.path().join("file");
        fs::write(&file_path, b"expected")?;
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o640))?;
        let snapshot = snapshot_path(&file_path)?;
        let mut file = asset(file_path.clone(), OwnedAssetKind::File);
        file.hash.clone_from(&snapshot.sha256);
        file.mode = snapshot.mode;

        let directory_path = directory.path().join("directory");
        fs::create_dir(&directory_path)?;
        let snapshot = snapshot_path(&directory_path)?;
        let mut owned_directory = asset(directory_path.clone(), OwnedAssetKind::Directory);
        owned_directory.mode = snapshot.mode;

        let link_path = directory.path().join("link");
        symlink("file", &link_path)?;
        let mut link = asset(link_path.clone(), OwnedAssetKind::Symlink);
        link.link_target = Some(PathBuf::from("file"));

        let mut counts = AssetCounts::default();
        let mut issues = Vec::new();
        inspect_asset(&file, &mut counts, &mut issues);
        inspect_asset(&owned_directory, &mut counts, &mut issues);
        inspect_asset(&link, &mut counts, &mut issues);
        assert_eq!(counts.healthy, 3);

        file.hash = Some(hash_bytes(b"other"));
        owned_directory.mode = Some(0);
        link.link_target = Some(PathBuf::from("other"));
        inspect_asset(&file, &mut counts, &mut issues);
        inspect_asset(&owned_directory, &mut counts, &mut issues);
        inspect_asset(&link, &mut counts, &mut issues);
        assert_eq!(counts.drifted, 3);

        let missing = asset(directory.path().join("missing"), OwnedAssetKind::File);
        inspect_asset(&missing, &mut counts, &mut issues);
        assert_eq!(counts.missing, 1);

        let conflict = asset(file_path, OwnedAssetKind::Directory);
        inspect_asset(&conflict, &mut counts, &mut issues);
        let invalid = asset(
            directory
                .path()
                .join(OsString::from_vec(b"invalid-\xff".to_vec())),
            OwnedAssetKind::File,
        );
        inspect_asset(&invalid, &mut counts, &mut issues);
        assert_eq!(counts.conflicting, 2);
        for code in [
            "asset_drifted",
            "asset_missing",
            "asset_type_conflict",
            "asset_inspection_failed",
        ] {
            assert!(issues.iter().any(|issue| issue.code == code));
        }
        Ok(())
    }

    #[test]
    fn installation_inspection_reports_foreign_permissions_catalog_and_roots()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_home, roots, mut receipt) = fixture(&ProviderId::ALL);
        fs::create_dir_all(&roots.canonical_skills)?;
        for provider in &roots.providers {
            fs::create_dir_all(&provider.agents)?;
            if let Some(skills) = &provider.skills {
                fs::create_dir_all(skills)?;
            }
        }
        fs::create_dir_all(&roots.state_directory)?;
        fs::write(&roots.receipt_path, b"receipt")?;
        fs::set_permissions(&roots.state_directory, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&roots.receipt_path, fs::Permissions::from_mode(0o644))?;
        fs::set_permissions(&roots.canonical.lexical, fs::Permissions::from_mode(0o555))?;

        let known = roots.canonical_skills.join("known/SKILL.md");
        fs::create_dir_all(known.parent().ok_or("known file has no parent")?)?;
        fs::write(&known, b"known")?;
        let snapshot = snapshot_path(&known)?;
        let mut owned = asset(known, OwnedAssetKind::File);
        owned.hash = snapshot.sha256;
        owned.mode = snapshot.mode;
        receipt.assets.push(owned);
        fs::write(roots.canonical_skills.join("foreign.txt"), b"foreign")?;
        let retained = roots.canonical_skills.join("retained");
        fs::create_dir(&retained)?;
        fs::write(retained.join("private.txt"), b"not managed")?;
        receipt.retained_unmanaged.push(RetainedUnmanagedAsset {
            source_id: "legacy".to_owned(),
            destination: retained,
            reason: "preserve".to_owned(),
        });

        let catalog = Catalog::load()?;
        let health = inspect_status(&catalog, &roots, &receipt);
        assert!(!health.healthy);
        assert!(!health.catalog_current);
        assert!(health.roots_match);
        assert_eq!(health.counts.retained_unmanaged, 1);
        assert_eq!(health.counts.foreign, 1);
        for code in [
            "catalog_update_available",
            "root_not_writable",
            "state_permissions_invalid",
        ] {
            assert!(
                health
                    .issues
                    .iter()
                    .any(|issue| { issue.code == code && issue.severity == IssueSeverity::Error })
            );
        }

        let mut mismatched = receipt;
        mismatched.roots.home.lexical.push("changed");
        let health = inspect_status(&catalog, &roots, &mismatched);
        assert!(!health.roots_match);
        assert_eq!(health.counts.healthy, 0);
        assert!(health.provider_probes.is_empty());
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.code == "root_mismatch")
        );

        fs::set_permissions(&roots.canonical.lexical, fs::Permissions::from_mode(0o755))?;
        fs::remove_dir_all(&roots.canonical_skills)?;
        fs::write(&roots.canonical_skills, b"not a directory")?;
        let mut issues = Vec::new();
        assert_eq!(
            super::count_foreign_under(
                &roots.canonical_skills,
                &BTreeSet::new(),
                &BTreeSet::new(),
                &mut issues,
            ),
            0
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "surface_unreadable")
        );
        super::inspect_owner_writable(&roots.canonical_skills.join("absent"), &mut issues);
        super::inspect_private_mode(&roots.state_directory.join("absent"), 0o700, &mut issues);
        Ok(())
    }
}
