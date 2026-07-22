use std::fmt;

use crate::adoption::{AdoptionPlan, EntryType};
use crate::plan::{
    AllowedRoot, ClaudeSymlinkPolicy, DesiredAsset, ExpectedNode, NodeKind, OwnedAssetState,
    PathPolicy, Plan, RemovalPolicy, build_plan_with_removal_policy,
};
use crate::provider::{ProviderId, ResolvedRoots};
use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt, ReceiptError, ReceiptState};

#[derive(Debug)]
pub enum EngineError {
    Receipt(ReceiptError),
    RecoveryRequired,
    AdoptionBlocked,
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Receipt(error) => error.fmt(formatter),
            Self::RecoveryRequired => formatter.write_str(
                "installation state requires recovery before another mutation can be planned",
            ),
            Self::AdoptionBlocked => {
                formatter.write_str("legacy installation is not eligible for complete adoption")
            }
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Receipt(error) => Some(error),
            Self::RecoveryRequired | Self::AdoptionBlocked => None,
        }
    }
}

impl From<ReceiptError> for EngineError {
    fn from(error: ReceiptError) -> Self {
        Self::Receipt(error)
    }
}

pub fn plan_desired_state(
    roots: &ResolvedRoots,
    receipt: Option<&Receipt>,
    desired: &[DesiredAsset],
) -> Result<Plan, EngineError> {
    plan_desired_state_with_removal_policy(roots, receipt, desired, RemovalPolicy::BlockOnDrift)
}

pub fn plan_desired_state_with_removal_policy(
    roots: &ResolvedRoots,
    receipt: Option<&Receipt>,
    desired: &[DesiredAsset],
    removal_policy: RemovalPolicy,
) -> Result<Plan, EngineError> {
    let owned = match receipt {
        Some(receipt) => {
            receipt.validate()?;
            receipt.validate_roots(roots)?;
            if receipt.state == ReceiptState::RecoveryRequired {
                return Err(EngineError::RecoveryRequired);
            }
            receipt.assets.iter().map(owned_state).collect::<Vec<_>>()
        }
        None => Vec::new(),
    };
    let allowed_roots = roots
        .allowed_top_level_roots()
        .map(|root| AllowedRoot {
            lexical: root.lexical.clone(),
            real: root.real.clone(),
        })
        .collect();
    let claude_symlinks = roots.provider(ProviderId::Claude).and_then(|provider| {
        provider.skills.as_ref().map(|skills| ClaudeSymlinkPolicy {
            link_root: skills.clone(),
            canonical_root: roots.canonical_skills.clone(),
        })
    });
    Ok(build_plan_with_removal_policy(
        desired,
        &owned,
        &PathPolicy {
            allowed_roots,
            claude_symlinks,
        },
        removal_policy,
    ))
}

pub fn receipt_after_adoption(
    receipt: &Receipt,
    roots: &ResolvedRoots,
    adoption: &AdoptionPlan,
) -> Result<Receipt, EngineError> {
    if !adoption.applicable {
        return Err(EngineError::AdoptionBlocked);
    }
    receipt.validate()?;
    receipt.validate_roots(roots)?;

    let mut next = receipt.clone();
    for entry in &adoption.entries {
        if next.owned_asset(&entry.destination).is_some() {
            continue;
        }
        let references = roots
            .providers
            .iter()
            .filter(|provider| {
                entry.destination.starts_with(&provider.root.lexical)
                    || entry.destination.starts_with(&roots.canonical.lexical)
            })
            .map(|provider| provider.id)
            .collect();
        let (kind, hash, mode, link_target) = match entry.entry_type {
            EntryType::File => (
                OwnedAssetKind::File,
                Some(entry.hash.clone()),
                Some(entry.mode),
                None,
            ),
            EntryType::Directory => (OwnedAssetKind::Directory, None, Some(entry.mode), None),
            EntryType::Symlink => (
                OwnedAssetKind::Symlink,
                None,
                None,
                entry.link_target.clone(),
            ),
        };
        next.assets.push(OwnedAsset {
            source_id: entry.source_id.clone(),
            destination: entry.destination.clone(),
            kind,
            hash,
            mode,
            link_target,
            references,
        });
    }
    next.assets.sort_by(|left, right| {
        left.destination
            .cmp(&right.destination)
            .then(left.source_id.cmp(&right.source_id))
    });
    next.validate()?;
    Ok(next)
}

fn owned_state(asset: &OwnedAsset) -> OwnedAssetState {
    let kind = match asset.kind {
        OwnedAssetKind::File => NodeKind::File,
        OwnedAssetKind::Directory => NodeKind::Directory,
        OwnedAssetKind::Symlink => NodeKind::Symlink,
    };
    OwnedAssetState {
        source_id: asset.source_id.clone(),
        destination: asset.destination.clone(),
        expected: ExpectedNode {
            kind,
            sha256: asset.hash.clone(),
            mode: asset.mode,
            link_target: asset.link_target.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    use sha2::Digest;
    use tempfile::tempdir;

    use super::{EngineError, plan_desired_state, receipt_after_adoption};
    use crate::adoption::{AdoptedEntry, AdoptionPlan, EntryType, LockIdentity};
    use crate::plan::{DesiredAsset, DesiredPayload, PlanAction};
    use crate::provider::{ProviderId, resolve_roots_from};
    use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt, ReceiptError, ReceiptState};

    #[test]
    fn engine_errors_have_stable_messages_and_receipt_sources() {
        let receipt = EngineError::from(ReceiptError::MissingField("cli_version"));
        assert_eq!(
            receipt.to_string(),
            "receipt field cli_version cannot be empty"
        );
        assert!(receipt.source().is_some());

        let recovery = EngineError::RecoveryRequired;
        assert_eq!(
            recovery.to_string(),
            "installation state requires recovery before another mutation can be planned"
        );
        assert!(recovery.source().is_none());

        let adoption = EngineError::AdoptionBlocked;
        assert_eq!(
            adoption.to_string(),
            "legacy installation is not eligible for complete adoption"
        );
        assert!(adoption.source().is_none());
    }

    #[test]
    fn shared_planner_uses_receipt_ownership_and_blocks_root_changes() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Claude])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        fs::create_dir_all(&roots.canonical.lexical)
            .unwrap_or_else(|error| panic!("fixture root failed: {error}"));
        let desired = vec![DesiredAsset {
            source_id: "skills/example/SKILL.md".to_owned(),
            destination: roots.canonical_skills.join("example/SKILL.md"),
            payload: DesiredPayload::File {
                bytes: b"example".to_vec(),
                mode: 0o644,
            },
        }];
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        let plan = plan_desired_state(&roots, Some(&receipt), &desired)
            .unwrap_or_else(|error| panic!("planning failed: {error}"));
        assert_eq!(plan.entries[0].action, PlanAction::Create);

        let mut changed = roots.clone();
        changed.canonical.device += 1;
        assert!(matches!(
            plan_desired_state(&changed, Some(&receipt), &desired),
            Err(EngineError::Receipt(_))
        ));
    }

    #[test]
    fn recovery_receipt_blocks_planning() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        receipt.state = ReceiptState::RecoveryRequired;
        assert!(matches!(
            plan_desired_state(&roots, Some(&receipt), &[]),
            Err(EngineError::RecoveryRequired)
        ));
    }

    #[test]
    fn receipt_asset_kinds_are_reconstructed_as_noop_plan_entries() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Claude])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let canonical_file = roots.canonical_skills.join("file/SKILL.md");
        let canonical_directory = roots.canonical_skills.join("directory");
        let claude_skills = roots
            .provider(ProviderId::Claude)
            .and_then(|provider| provider.skills.as_ref())
            .unwrap_or_else(|| panic!("Claude skills root missing"));
        let claude_link = claude_skills.join("linked");
        fs::create_dir_all(
            canonical_file
                .parent()
                .unwrap_or(roots.canonical_skills.as_path()),
        )
        .unwrap_or_else(|error| panic!("file parent fixture failed: {error}"));
        fs::create_dir_all(&canonical_directory)
            .unwrap_or_else(|error| panic!("directory fixture failed: {error}"));
        fs::create_dir_all(claude_skills)
            .unwrap_or_else(|error| panic!("Claude root fixture failed: {error}"));
        fs::write(&canonical_file, b"file")
            .unwrap_or_else(|error| panic!("file fixture failed: {error}"));
        fs::set_permissions(&canonical_file, fs::Permissions::from_mode(0o644))
            .unwrap_or_else(|error| panic!("file mode fixture failed: {error}"));
        fs::set_permissions(&canonical_directory, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("directory mode fixture failed: {error}"));
        let link_target = PathBuf::from("../../.agents/skills/linked");
        symlink(&link_target, &claude_link)
            .unwrap_or_else(|error| panic!("symlink fixture failed: {error}"));

        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        receipt.assets = vec![
            OwnedAsset {
                source_id: "directory".to_owned(),
                destination: canonical_directory.clone(),
                kind: OwnedAssetKind::Directory,
                hash: None,
                mode: Some(0o755),
                link_target: None,
                references: vec![ProviderId::Claude],
            },
            OwnedAsset {
                source_id: "file".to_owned(),
                destination: canonical_file.clone(),
                kind: OwnedAssetKind::File,
                hash: Some(format!("{:x}", sha2::Sha256::digest(b"file"))),
                mode: Some(0o644),
                link_target: None,
                references: vec![ProviderId::Claude],
            },
            OwnedAsset {
                source_id: "linked".to_owned(),
                destination: claude_link.clone(),
                kind: OwnedAssetKind::Symlink,
                hash: None,
                mode: None,
                link_target: Some(link_target.clone()),
                references: vec![ProviderId::Claude],
            },
        ];
        let desired = vec![
            DesiredAsset {
                source_id: "directory".to_owned(),
                destination: canonical_directory,
                payload: DesiredPayload::Directory { mode: 0o755 },
            },
            DesiredAsset {
                source_id: "file".to_owned(),
                destination: canonical_file,
                payload: DesiredPayload::File {
                    bytes: b"file".to_vec(),
                    mode: 0o644,
                },
            },
            DesiredAsset {
                source_id: "linked".to_owned(),
                destination: claude_link,
                payload: DesiredPayload::Symlink {
                    target: link_target,
                    canonical_target: roots.canonical_skills.join("linked"),
                },
            },
        ];

        let plan = plan_desired_state(&roots, Some(&receipt), &desired)
            .unwrap_or_else(|error| panic!("planning failed: {error}"));

        assert!(plan.applicable);
        assert!(!plan.has_mutations());
        assert!(
            plan.entries
                .iter()
                .all(|entry| entry.action == PlanAction::Noop)
        );
    }

    #[test]
    fn adoption_adds_verified_entries_without_rewriting_files() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Codex])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let destination = roots.canonical_skills.join("example/SKILL.md");
        fs::create_dir_all(destination.parent().unwrap_or(home.path()))
            .unwrap_or_else(|error| panic!("fixture root failed: {error}"));
        fs::write(&destination, b"example")
            .unwrap_or_else(|error| panic!("fixture file failed: {error}"));
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o644))
            .unwrap_or_else(|error| panic!("fixture mode failed: {error}"));
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        let adoption = AdoptionPlan {
            entries: vec![AdoptedEntry {
                source_id: "example".to_owned(),
                destination: destination.clone(),
                entry_type: EntryType::File,
                hash: format!("{:x}", sha2::Sha256::digest(b"example")),
                mode: 0o644,
                link_target: None,
            }],
            original_identity: LockIdentity {
                device: 1,
                inode: 1,
                size: 1,
                mtime_seconds: 1,
                mtime_nanoseconds: 1,
            },
            original_bytes: b"{}".to_vec(),
            original_hash: "hash".to_owned(),
            archive_path: home.path().join("archive"),
            residual_bytes: b"{}".to_vec(),
            applicable: true,
            diagnostics: Vec::new(),
        };
        let next = receipt_after_adoption(&receipt, &roots, &adoption)
            .unwrap_or_else(|error| panic!("adoption receipt failed: {error}"));
        assert_eq!(next.assets.len(), 1);
        assert_eq!(
            fs::read(&destination).ok().as_deref(),
            Some(b"example".as_slice())
        );
    }

    #[test]
    fn adoption_maps_all_entry_types_references_and_existing_ownership() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(
            Some(home.path().as_os_str()),
            None,
            &[ProviderId::Claude, ProviderId::Codex],
        )
        .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let claude_link = roots
            .provider(ProviderId::Claude)
            .and_then(|provider| provider.skills.as_ref())
            .map(|skills| skills.join("linked"))
            .unwrap_or_else(|| panic!("Claude skills root missing"));
        let codex_file = roots
            .provider(ProviderId::Codex)
            .map(|provider| provider.agents.join("agent.toml"))
            .unwrap_or_else(|| panic!("Codex root missing"));
        let directory = roots.canonical_skills.join("directory");
        let existing = roots.canonical_skills.join("existing/SKILL.md");
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        receipt.assets.push(OwnedAsset {
            source_id: "existing".to_owned(),
            destination: existing.clone(),
            kind: OwnedAssetKind::File,
            hash: Some("e".repeat(64)),
            mode: Some(0o644),
            link_target: None,
            references: vec![ProviderId::Codex],
        });
        let adoption = AdoptionPlan {
            entries: vec![
                AdoptedEntry {
                    source_id: "existing-replacement".to_owned(),
                    destination: existing.clone(),
                    entry_type: EntryType::File,
                    hash: "f".repeat(64),
                    mode: 0o600,
                    link_target: None,
                },
                AdoptedEntry {
                    source_id: "directory".to_owned(),
                    destination: directory.clone(),
                    entry_type: EntryType::Directory,
                    hash: String::new(),
                    mode: 0o755,
                    link_target: None,
                },
                AdoptedEntry {
                    source_id: "linked".to_owned(),
                    destination: claude_link.clone(),
                    entry_type: EntryType::Symlink,
                    hash: String::new(),
                    mode: 0,
                    link_target: Some(PathBuf::from("../../.agents/skills/linked")),
                },
                AdoptedEntry {
                    source_id: "agent".to_owned(),
                    destination: codex_file.clone(),
                    entry_type: EntryType::File,
                    hash: "c".repeat(64),
                    mode: 0o644,
                    link_target: None,
                },
            ],
            original_identity: LockIdentity {
                device: 1,
                inode: 2,
                size: 3,
                mtime_seconds: 4,
                mtime_nanoseconds: 5,
            },
            original_bytes: b"legacy".to_vec(),
            original_hash: "d".repeat(64),
            archive_path: roots.state_directory.join("legacy.json"),
            residual_bytes: b"{}".to_vec(),
            applicable: true,
            diagnostics: Vec::new(),
        };

        let next = receipt_after_adoption(&receipt, &roots, &adoption)
            .unwrap_or_else(|error| panic!("adoption receipt failed: {error}"));

        assert_eq!(next.assets.len(), 4);
        assert_eq!(
            next.owned_asset(&existing)
                .map(|asset| asset.source_id.as_str()),
            Some("existing")
        );
        let directory_asset = next
            .owned_asset(&directory)
            .unwrap_or_else(|| panic!("directory ownership missing"));
        assert_eq!(directory_asset.kind, OwnedAssetKind::Directory);
        assert_eq!(
            directory_asset.references,
            vec![ProviderId::Claude, ProviderId::Codex]
        );
        let link_asset = next
            .owned_asset(&claude_link)
            .unwrap_or_else(|| panic!("symlink ownership missing"));
        assert_eq!(link_asset.kind, OwnedAssetKind::Symlink);
        assert_eq!(link_asset.references, vec![ProviderId::Claude]);
        assert_eq!(
            next.owned_asset(&codex_file)
                .map(|asset| asset.references.as_slice()),
            Some([ProviderId::Codex].as_slice())
        );
        assert!(
            next.assets
                .windows(2)
                .all(|pair| pair[0].destination <= pair[1].destination)
        );
    }

    #[test]
    fn blocked_and_invalid_adoptions_fail_before_producing_a_receipt() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        let mut adoption = AdoptionPlan {
            entries: Vec::new(),
            original_identity: LockIdentity {
                device: 1,
                inode: 1,
                size: 0,
                mtime_seconds: 0,
                mtime_nanoseconds: 0,
            },
            original_bytes: Vec::new(),
            original_hash: "b".repeat(64),
            archive_path: roots.state_directory.join("legacy.json"),
            residual_bytes: Vec::new(),
            applicable: false,
            diagnostics: Vec::new(),
        };
        assert!(matches!(
            receipt_after_adoption(&receipt, &roots, &adoption),
            Err(EngineError::AdoptionBlocked)
        ));

        adoption.applicable = true;
        let mut invalid_receipt = receipt;
        invalid_receipt.catalog_sha256 = "invalid".to_owned();
        assert!(matches!(
            receipt_after_adoption(&invalid_receipt, &roots, &adoption),
            Err(EngineError::Receipt(_))
        ));
    }
}
