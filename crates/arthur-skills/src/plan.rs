use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::platform::{
    effective_directory_mode, effective_file_mode, metadata_mode,
    normalize_absolute as normalize_platform_path, path_key as os_path_key,
};

pub const PLAN_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanAction {
    Create,
    Update,
    Remove,
    Noop,
    Adoptable,
    Drifted,
    Conflict,
    RetainedUnmanaged,
    RecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemovalPolicy {
    BlockOnDrift,
    RetainUnmanaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    ArthurWorkflow,
    VercelSkills,
    Unmanaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Directory,
    File,
    Symlink,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExpectedNode {
    pub kind: NodeKind,
    pub sha256: Option<String>,
    pub mode: Option<u32>,
    pub link_target: Option<PathBuf>,
}

impl ExpectedNode {
    pub const fn directory(mode: u32) -> Self {
        Self {
            kind: NodeKind::Directory,
            sha256: None,
            mode: Some(mode),
            link_target: None,
        }
    }

    pub fn file(bytes: &[u8], mode: u32) -> Self {
        Self {
            kind: NodeKind::File,
            sha256: Some(sha256(bytes)),
            mode: Some(mode),
            link_target: None,
        }
    }

    pub fn symlink(target: PathBuf) -> Self {
        Self {
            kind: NodeKind::Symlink,
            sha256: None,
            mode: None,
            link_target: Some(target),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesiredPayload {
    Directory {
        mode: u32,
    },
    File {
        bytes: Vec<u8>,
        mode: u32,
    },
    Symlink {
        target: PathBuf,
        canonical_target: PathBuf,
    },
}

impl DesiredPayload {
    pub fn expected(&self) -> ExpectedNode {
        match self {
            Self::Directory { mode } => ExpectedNode::directory(effective_directory_mode(*mode)),
            Self::File { bytes, mode } => ExpectedNode::file(bytes, effective_file_mode(*mode)),
            Self::Symlink { target, .. } => ExpectedNode::symlink(target.clone()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesiredAsset {
    pub source_id: String,
    pub destination: PathBuf,
    pub payload: DesiredPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedAssetState {
    pub source_id: String,
    pub destination: PathBuf,
    pub expected: ExpectedNode,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PathSnapshot {
    pub kind: NodeKind,
    pub sha256: Option<String>,
    pub mode: Option<u32>,
    pub link_target: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Precondition {
    Missing,
    Matches { snapshot: PathSnapshot },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum PlannedInverse {
    RemoveCreated,
    RestoreBackup,
    RestoreMode { mode: u32 },
    None,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "proof")]
pub enum OwnershipProof {
    UnownedDestination,
    Receipt {
        source_id: String,
        sha256: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    EnsureDirectory,
    WriteFile,
    ReplaceFile,
    SetMode,
    CreateSymlink,
    RemoveOwnedPath,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlannedMutation {
    pub id: String,
    pub kind: MutationKind,
    pub root: PathBuf,
    pub destination: PathBuf,
    pub precondition: Precondition,
    pub inverse: PlannedInverse,
    pub ownership: OwnershipProof,
    pub content_sha256: Option<String>,
    pub mode: Option<u32>,
    pub link_target: Option<PathBuf>,
    #[serde(skip)]
    pub payload: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlanEntry {
    pub action: PlanAction,
    pub source: String,
    pub destination: PathBuf,
    pub owner: Owner,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub path_utf8: Option<String>,
    pub path_bytes_hex: Option<String>,
}

impl Diagnostic {
    fn path_error(code: &str, message: String, path: &Path) -> Self {
        let (path_utf8, path_bytes_hex) = match path.to_str() {
            Some(path) => (Some(path.to_owned()), None),
            None => (None, Some(hex(&os_path_key(path.as_os_str())))),
        };
        Self {
            code: code.to_owned(),
            severity: DiagnosticSeverity::Error,
            message,
            path_utf8,
            path_bytes_hex,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Plan {
    pub schema_version: u16,
    pub applicable: bool,
    pub entries: Vec<PlanEntry>,
    pub operations: Vec<PlannedMutation>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Plan {
    pub fn has_mutations(&self) -> bool {
        !self.operations.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowedRoot {
    pub lexical: PathBuf,
    pub real: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeSymlinkPolicy {
    pub link_root: PathBuf,
    pub canonical_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathPolicy {
    pub allowed_roots: Vec<AllowedRoot>,
    pub claude_symlinks: Option<ClaudeSymlinkPolicy>,
}

pub fn build_plan(
    desired: &[DesiredAsset],
    owned: &[OwnedAssetState],
    policy: &PathPolicy,
) -> Plan {
    build_plan_with_removal_policy(desired, owned, policy, RemovalPolicy::BlockOnDrift)
}

pub fn build_plan_with_removal_policy(
    desired: &[DesiredAsset],
    owned: &[OwnedAssetState],
    policy: &PathPolicy,
    removal_policy: RemovalPolicy,
) -> Plan {
    let mut entries = Vec::new();
    let mut operations = Vec::new();
    let mut diagnostics = Vec::new();
    let mut applicable = true;

    let owned_by_path = index_owned(owned, &mut diagnostics);
    if diagnostics
        .iter()
        .any(|item| item.severity == DiagnosticSeverity::Error)
    {
        applicable = false;
    }

    let mut desired_by_path = BTreeMap::new();
    for asset in desired {
        if desired_by_path
            .insert(path_key(&asset.destination), asset)
            .is_some()
        {
            diagnostics.push(Diagnostic::path_error(
                "duplicate_destination",
                "more than one desired asset targets this path".to_owned(),
                &asset.destination,
            ));
            applicable = false;
        }
    }

    for asset in desired_by_path.values() {
        let Some(root) = validate_destination(asset, policy, &mut diagnostics) else {
            applicable = false;
            entries.push(entry(
                PlanAction::Conflict,
                asset,
                Owner::Unmanaged,
                "destination violates the configured root policy",
            ));
            continue;
        };
        let receipt_asset = owned_by_path.get(&path_key(&asset.destination)).copied();
        match inspect_path(&asset.destination) {
            Ok(None) => {
                if receipt_asset.is_some() {
                    entries.push(entry(
                        PlanAction::Drifted,
                        asset,
                        Owner::ArthurWorkflow,
                        "managed path is missing",
                    ));
                    applicable = false;
                } else {
                    entries.push(entry(
                        PlanAction::Create,
                        asset,
                        Owner::Unmanaged,
                        "destination does not exist",
                    ));
                    operations.push(create_operation(asset, root));
                }
            }
            Ok(Some(snapshot)) => match receipt_asset {
                Some(receipt_asset) => {
                    if !snapshot_matches(&snapshot, &receipt_asset.expected) {
                        entries.push(entry(
                            PlanAction::Drifted,
                            asset,
                            Owner::ArthurWorkflow,
                            "managed path differs from its receipt proof",
                        ));
                        applicable = false;
                    } else if snapshot_matches(&snapshot, &asset.payload.expected()) {
                        entries.push(entry(
                            PlanAction::Noop,
                            asset,
                            Owner::ArthurWorkflow,
                            "managed path already matches the desired state",
                        ));
                    } else {
                        entries.push(entry(
                            PlanAction::Update,
                            asset,
                            Owner::ArthurWorkflow,
                            "managed path is eligible for a verified update",
                        ));
                        operations.extend(update_operations(asset, receipt_asset, snapshot, root));
                    }
                }
                None => {
                    let expected = asset.payload.expected();
                    let matching = snapshot_matches(&snapshot, &expected);
                    let link_health = if snapshot.kind == NodeKind::Symlink {
                        Some(inspect_link_health(&asset.destination))
                    } else {
                        None
                    };
                    if matching
                        && link_health
                            .as_ref()
                            .is_none_or(|health| health.is_healthy())
                    {
                        entries.push(entry(
                            PlanAction::Adoptable,
                            asset,
                            Owner::Unmanaged,
                            "matching unmanaged asset requires explicit adoption",
                        ));
                        applicable = false;
                    } else {
                        let reason = match link_health {
                            Some(LinkHealth::Broken) => "unmanaged symlink is broken",
                            Some(LinkHealth::Cyclic) => "unmanaged symlink is cyclic",
                            Some(LinkHealth::Escaped) => {
                                "unmanaged symlink resolves outside the allowed target"
                            }
                            Some(LinkHealth::Healthy) if !matching => {
                                "unmanaged symlink targets a different canonical asset"
                            }
                            _ => "unmanaged path conflicts with the desired asset",
                        };
                        entries.push(entry(PlanAction::Conflict, asset, Owner::Unmanaged, reason));
                        applicable = false;
                    }
                }
            },
            Err(error) => {
                diagnostics.push(Diagnostic::path_error(
                    "filesystem_scan_failed",
                    format!("cannot inspect path: {error}"),
                    &asset.destination,
                ));
                entries.push(entry(
                    PlanAction::Conflict,
                    asset,
                    receipt_asset.map_or(Owner::Unmanaged, |_| Owner::ArthurWorkflow),
                    "filesystem inspection failed",
                ));
                applicable = false;
            }
        }
    }

    let desired_keys = desired_by_path.keys().cloned().collect::<BTreeSet<_>>();
    let mut removals = owned_by_path
        .into_iter()
        .filter(|(key, _)| !desired_keys.contains(key))
        .collect::<Vec<_>>();
    if removal_policy == RemovalPolicy::RetainUnmanaged {
        removals.sort_by(|left, right| {
            right
                .1
                .destination
                .components()
                .count()
                .cmp(&left.1.destination.components().count())
                .then(right.0.cmp(&left.0))
        });
    }
    let mut removable_paths = BTreeSet::new();
    for (key, receipt_asset) in removals {
        let synthetic = DesiredAsset {
            source_id: receipt_asset.source_id.clone(),
            destination: receipt_asset.destination.clone(),
            payload: payload_from_expected(&receipt_asset.expected),
        };
        let Some(root) =
            validate_destination_path(&receipt_asset.destination, policy, &mut diagnostics)
        else {
            entries.push(entry(
                PlanAction::Conflict,
                &synthetic,
                Owner::ArthurWorkflow,
                "owned destination violates the configured root policy",
            ));
            applicable = false;
            continue;
        };
        match inspect_path(&receipt_asset.destination) {
            Ok(Some(snapshot)) if snapshot_matches(&snapshot, &receipt_asset.expected) => {
                if removal_policy == RemovalPolicy::RetainUnmanaged
                    && snapshot.kind == NodeKind::Directory
                {
                    match directory_children_are_removable(
                        &receipt_asset.destination,
                        &removable_paths,
                    ) {
                        Ok(true) => {}
                        Ok(false) => {
                            entries.push(entry(
                                PlanAction::RetainedUnmanaged,
                                &synthetic,
                                Owner::ArthurWorkflow,
                                "owned directory contains unmanaged content and will be released",
                            ));
                            continue;
                        }
                        Err(error) => {
                            diagnostics.push(Diagnostic::path_error(
                                "filesystem_scan_failed",
                                format!("cannot inspect owned directory: {error}"),
                                &receipt_asset.destination,
                            ));
                            applicable = false;
                            continue;
                        }
                    }
                }
                entries.push(entry(
                    PlanAction::Remove,
                    &synthetic,
                    Owner::ArthurWorkflow,
                    "owned asset is absent from the desired state",
                ));
                operations.push(remove_operation(receipt_asset, snapshot, root));
                removable_paths.insert(key);
            }
            Ok(None) if removal_policy == RemovalPolicy::RetainUnmanaged => {
                entries.push(entry(
                    PlanAction::Remove,
                    &synthetic,
                    Owner::ArthurWorkflow,
                    "owned asset is already absent and its ownership will be released",
                ));
            }
            Ok(_) if removal_policy == RemovalPolicy::RetainUnmanaged => {
                entries.push(entry(
                    PlanAction::RetainedUnmanaged,
                    &synthetic,
                    Owner::ArthurWorkflow,
                    "owned asset is retained and its ownership will be released",
                ));
            }
            Ok(_) => {
                entries.push(entry(
                    PlanAction::Drifted,
                    &synthetic,
                    Owner::ArthurWorkflow,
                    "owned asset cannot be removed because its proof no longer matches",
                ));
                applicable = false;
            }
            Err(error) => {
                diagnostics.push(Diagnostic::path_error(
                    "filesystem_scan_failed",
                    format!("cannot inspect owned path: {error}"),
                    &receipt_asset.destination,
                ));
                applicable = false;
            }
        }
    }

    entries.sort_by(|left, right| {
        path_key(&left.destination)
            .cmp(&path_key(&right.destination))
            .then(left.action.cmp(&right.action))
    });
    operations.sort_by(|left, right| {
        path_key(&left.root)
            .cmp(&path_key(&right.root))
            .then(mutation_phase(left.kind).cmp(&mutation_phase(right.kind)))
            .then_with(|| {
                if left.kind == MutationKind::RemoveOwnedPath
                    && right.kind == MutationKind::RemoveOwnedPath
                {
                    right
                        .destination
                        .components()
                        .count()
                        .cmp(&left.destination.components().count())
                } else {
                    left.destination
                        .components()
                        .count()
                        .cmp(&right.destination.components().count())
                }
            })
            .then(path_key(&left.destination).cmp(&path_key(&right.destination)))
            .then(mutation_rank(left.kind).cmp(&mutation_rank(right.kind)))
            .then(left.id.cmp(&right.id))
    });
    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.path_utf8.cmp(&right.path_utf8))
            .then(left.path_bytes_hex.cmp(&right.path_bytes_hex))
    });

    Plan {
        schema_version: PLAN_SCHEMA_VERSION,
        applicable,
        entries,
        operations,
        diagnostics,
    }
}

fn directory_children_are_removable(
    directory: &Path,
    removable_paths: &BTreeSet<Vec<u8>>,
) -> io::Result<bool> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.to_str().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "non-UTF-8 directory entry cannot be classified safely",
            ));
        }
        if !removable_paths.contains(&path_key(&path)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn index_owned<'a>(
    owned: &'a [OwnedAssetState],
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<Vec<u8>, &'a OwnedAssetState> {
    let mut indexed = BTreeMap::new();
    for asset in owned {
        if indexed
            .insert(path_key(&asset.destination), asset)
            .is_some()
        {
            diagnostics.push(Diagnostic::path_error(
                "duplicate_receipt_destination",
                "receipt contains duplicate ownership for this path".to_owned(),
                &asset.destination,
            ));
        }
    }
    indexed
}

fn validate_destination<'a>(
    asset: &DesiredAsset,
    policy: &'a PathPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'a Path> {
    let path = &asset.destination;
    let root = validate_destination_path(path, policy, diagnostics)?;
    if let DesiredPayload::Symlink {
        target,
        canonical_target,
    } = &asset.payload
    {
        let Some(link_policy) = &policy.claude_symlinks else {
            diagnostics.push(Diagnostic::path_error(
                "symlink_not_allowed",
                "no provider symlink edge is configured".to_owned(),
                path,
            ));
            return None;
        };
        let link_relative = path.strip_prefix(&link_policy.link_root).ok();
        let canonical_relative = canonical_target
            .strip_prefix(&link_policy.canonical_root)
            .ok();
        if link_relative.is_none()
            || link_relative != canonical_relative
            || link_relative.is_none_or(|relative| relative.components().count() != 1)
            || resolve_link_target(path, target).as_deref() != Some(canonical_target.as_path())
        {
            diagnostics.push(Diagnostic::path_error(
                "symlink_escape",
                "Claude symlink must target the exact corresponding canonical skill".to_owned(),
                path,
            ));
            return None;
        }
    }
    Some(root)
}

fn validate_destination_path<'a>(
    path: &Path,
    policy: &'a PathPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'a Path> {
    if path.to_str().is_none() {
        diagnostics.push(Diagnostic::path_error(
            "non_utf8_path",
            "non-UTF-8 paths are not supported in v1".to_owned(),
            path,
        ));
        return None;
    }
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        diagnostics.push(Diagnostic::path_error(
            "unsafe_destination",
            "destination must be absolute and cannot contain parent traversal".to_owned(),
            path,
        ));
        return None;
    }
    let matching_root = policy
        .allowed_roots
        .iter()
        .filter(|root| path.starts_with(&root.lexical))
        .max_by_key(|root| root.lexical.components().count());
    let Some(root) = matching_root else {
        diagnostics.push(Diagnostic::path_error(
            "destination_outside_roots",
            "destination is outside every configured root".to_owned(),
            path,
        ));
        return None;
    };
    if let Err(error) = validate_existing_ancestor(path, root) {
        diagnostics.push(Diagnostic::path_error("symlink_escape", error, path));
        return None;
    }
    Some(root.lexical.as_path())
}

fn validate_existing_ancestor(path: &Path, root: &AllowedRoot) -> Result<(), String> {
    let mut candidate = path.parent();
    while let Some(ancestor) = candidate {
        match fs::canonicalize(ancestor) {
            Ok(real_ancestor) => {
                let suffix = path
                    .strip_prefix(ancestor)
                    .map_err(|error| error.to_string())?;
                let projected = real_ancestor.join(suffix);
                if projected.starts_with(&root.real) {
                    return Ok(());
                }
                return Err("an existing ancestor resolves outside the configured root".to_owned());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                candidate = ancestor.parent();
            }
            Err(error) => return Err(format!("cannot resolve an existing ancestor: {error}")),
        }
    }
    Err("destination has no resolvable ancestor".to_owned())
}

fn inspect_path(path: &Path) -> io::Result<Option<PathSnapshot>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Ok(Some(PathSnapshot {
            kind: NodeKind::Symlink,
            sha256: None,
            mode: None,
            link_target: Some(fs::read_link(path)?),
        }));
    }
    if file_type.is_file() {
        return Ok(Some(PathSnapshot {
            kind: NodeKind::File,
            sha256: Some(sha256(&fs::read(path)?)),
            mode: Some(metadata_mode(&metadata)),
            link_target: None,
        }));
    }
    if file_type.is_dir() {
        return Ok(Some(PathSnapshot {
            kind: NodeKind::Directory,
            sha256: None,
            mode: Some(metadata_mode(&metadata)),
            link_target: None,
        }));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "unsupported filesystem node type",
    ))
}

fn snapshot_matches(snapshot: &PathSnapshot, expected: &ExpectedNode) -> bool {
    snapshot.kind == expected.kind
        && snapshot.sha256 == expected.sha256
        && snapshot.mode == expected.mode
        && snapshot.link_target == expected.link_target
}

fn create_operation(asset: &DesiredAsset, root: &Path) -> PlannedMutation {
    let expected = asset.payload.expected();
    let kind = match asset.payload {
        DesiredPayload::Directory { .. } => MutationKind::EnsureDirectory,
        DesiredPayload::File { .. } => MutationKind::WriteFile,
        DesiredPayload::Symlink { .. } => MutationKind::CreateSymlink,
    };
    PlannedMutation {
        id: operation_id(&asset.source_id, kind, &asset.destination),
        kind,
        root: root.to_path_buf(),
        destination: asset.destination.clone(),
        precondition: Precondition::Missing,
        inverse: PlannedInverse::RemoveCreated,
        ownership: OwnershipProof::UnownedDestination,
        content_sha256: expected.sha256,
        mode: expected.mode,
        link_target: expected.link_target,
        payload: payload_bytes(&asset.payload),
    }
}

fn update_operations(
    asset: &DesiredAsset,
    receipt_asset: &OwnedAssetState,
    snapshot: PathSnapshot,
    root: &Path,
) -> Vec<PlannedMutation> {
    let expected = asset.payload.expected();
    let proof = OwnershipProof::Receipt {
        source_id: receipt_asset.source_id.clone(),
        sha256: receipt_asset.expected.sha256.clone(),
    };
    if snapshot.kind == expected.kind
        && snapshot.sha256 == expected.sha256
        && snapshot.link_target == expected.link_target
        && snapshot.mode != expected.mode
    {
        return vec![PlannedMutation {
            id: operation_id(&asset.source_id, MutationKind::SetMode, &asset.destination),
            kind: MutationKind::SetMode,
            root: root.to_path_buf(),
            destination: asset.destination.clone(),
            precondition: Precondition::Matches {
                snapshot: snapshot.clone(),
            },
            inverse: PlannedInverse::RestoreMode {
                mode: snapshot.mode.unwrap_or(0),
            },
            ownership: proof,
            content_sha256: expected.sha256,
            mode: expected.mode,
            link_target: expected.link_target,
            payload: None,
        }];
    }
    if snapshot.kind == NodeKind::File && expected.kind == NodeKind::File {
        return vec![PlannedMutation {
            id: operation_id(
                &asset.source_id,
                MutationKind::ReplaceFile,
                &asset.destination,
            ),
            kind: MutationKind::ReplaceFile,
            root: root.to_path_buf(),
            destination: asset.destination.clone(),
            precondition: Precondition::Matches { snapshot },
            inverse: PlannedInverse::RestoreBackup,
            ownership: proof,
            content_sha256: expected.sha256,
            mode: expected.mode,
            link_target: None,
            payload: payload_bytes(&asset.payload),
        }];
    }
    let remove = PlannedMutation {
        id: operation_id(
            &asset.source_id,
            MutationKind::RemoveOwnedPath,
            &asset.destination,
        ),
        kind: MutationKind::RemoveOwnedPath,
        root: root.to_path_buf(),
        destination: asset.destination.clone(),
        precondition: Precondition::Matches { snapshot },
        inverse: PlannedInverse::RestoreBackup,
        ownership: proof,
        content_sha256: None,
        mode: None,
        link_target: None,
        payload: None,
    };
    let create = create_operation(asset, root);
    vec![remove, create]
}

fn remove_operation(
    asset: &OwnedAssetState,
    snapshot: PathSnapshot,
    root: &Path,
) -> PlannedMutation {
    PlannedMutation {
        id: operation_id(
            &asset.source_id,
            MutationKind::RemoveOwnedPath,
            &asset.destination,
        ),
        kind: MutationKind::RemoveOwnedPath,
        root: root.to_path_buf(),
        destination: asset.destination.clone(),
        precondition: Precondition::Matches { snapshot },
        inverse: PlannedInverse::RestoreBackup,
        ownership: OwnershipProof::Receipt {
            source_id: asset.source_id.clone(),
            sha256: asset.expected.sha256.clone(),
        },
        content_sha256: None,
        mode: None,
        link_target: None,
        payload: None,
    }
}

fn payload_from_expected(expected: &ExpectedNode) -> DesiredPayload {
    match expected.kind {
        NodeKind::Directory => DesiredPayload::Directory {
            mode: expected.mode.unwrap_or(0o700),
        },
        NodeKind::File => DesiredPayload::File {
            bytes: Vec::new(),
            mode: expected.mode.unwrap_or(0o600),
        },
        NodeKind::Symlink => DesiredPayload::Symlink {
            target: expected.link_target.clone().unwrap_or_default(),
            canonical_target: expected.link_target.clone().unwrap_or_default(),
        },
    }
}

fn payload_bytes(payload: &DesiredPayload) -> Option<Vec<u8>> {
    match payload {
        DesiredPayload::File { bytes, .. } => Some(bytes.clone()),
        _ => None,
    }
}

fn entry(action: PlanAction, asset: &DesiredAsset, owner: Owner, reason: &str) -> PlanEntry {
    PlanEntry {
        action,
        source: asset.source_id.clone(),
        destination: asset.destination.clone(),
        owner,
        reason: reason.to_owned(),
    }
}

fn operation_id(source_id: &str, kind: MutationKind, destination: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(source_id.as_bytes());
    digest.update([0]);
    digest.update(format!("{kind:?}").as_bytes());
    digest.update([0]);
    digest.update(os_path_key(destination.as_os_str()));
    format!("{:x}", digest.finalize())[..16].to_owned()
}

const fn mutation_rank(kind: MutationKind) -> u8 {
    match kind {
        MutationKind::RemoveOwnedPath => 0,
        MutationKind::EnsureDirectory => 1,
        MutationKind::WriteFile => 2,
        MutationKind::ReplaceFile => 3,
        MutationKind::SetMode => 4,
        MutationKind::CreateSymlink => 5,
    }
}

const fn mutation_phase(kind: MutationKind) -> u8 {
    match kind {
        MutationKind::RemoveOwnedPath => 0,
        MutationKind::EnsureDirectory => 1,
        MutationKind::WriteFile
        | MutationKind::ReplaceFile
        | MutationKind::SetMode
        | MutationKind::CreateSymlink => 2,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinkHealth {
    Healthy,
    Broken,
    Cyclic,
    Escaped,
}

impl LinkHealth {
    const fn is_healthy(self) -> bool {
        matches!(self, Self::Healthy)
    }
}

fn inspect_link_health(path: &Path) -> LinkHealth {
    let mut current = path.to_path_buf();
    let mut visited = HashSet::new();
    for _ in 0..64 {
        if !visited.insert(current.clone()) {
            return LinkHealth::Cyclic;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return LinkHealth::Broken,
            Err(_) => return LinkHealth::Escaped,
        };
        if !metadata.file_type().is_symlink() {
            return LinkHealth::Healthy;
        }
        let target = match fs::read_link(&current) {
            Ok(target) => target,
            Err(_) => return LinkHealth::Escaped,
        };
        let Some(next) = resolve_link_target(&current, &target) else {
            return LinkHealth::Escaped;
        };
        current = next;
    }
    LinkHealth::Cyclic
}

fn resolve_link_target(link: &Path, target: &Path) -> Option<PathBuf> {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        link.parent()?.join(target)
    };
    normalize_absolute(&joined)
}

fn normalize_absolute(path: &Path) -> Option<PathBuf> {
    normalize_platform_path(path)
}

fn path_key(path: &Path) -> Vec<u8> {
    os_path_key(path.as_os_str())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(all(test, unix))]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{
        AllowedRoot, ClaudeSymlinkPolicy, DesiredAsset, DesiredPayload, ExpectedNode, MutationKind,
        OwnedAssetState, PathPolicy, PlanAction, PlannedInverse, build_plan, normalize_absolute,
    };

    fn policy(root: &std::path::Path) -> PathPolicy {
        PathPolicy {
            allowed_roots: vec![AllowedRoot {
                lexical: root.to_path_buf(),
                real: root.to_path_buf(),
            }],
            claude_symlinks: None,
        }
    }

    #[test]
    fn planning_is_deterministic_and_read_only() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let destination = temp.path().join("new.txt");
        let desired = vec![DesiredAsset {
            source_id: "skill/new.txt".to_owned(),
            destination: destination.clone(),
            payload: DesiredPayload::File {
                bytes: b"new".to_vec(),
                mode: 0o644,
            },
        }];
        let first = build_plan(&desired, &[], &policy(temp.path()));
        let second = build_plan(&desired, &[], &policy(temp.path()));
        assert_eq!(serde_json::to_vec(&first)?, serde_json::to_vec(&second)?);
        assert_eq!(first.entries[0].action, PlanAction::Create);
        assert!(first.applicable && first.has_mutations());
        assert!(!destination.exists());
        Ok(())
    }

    #[test]
    fn classifies_owned_unowned_and_removed_assets() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let noop = temp.path().join("noop");
        let drifted = temp.path().join("drifted");
        let adoptable = temp.path().join("adoptable");
        let conflict = temp.path().join("conflict");
        let removed = temp.path().join("removed");
        fs::write(&noop, b"same")?;
        fs::write(&drifted, b"changed locally")?;
        fs::write(&adoptable, b"same")?;
        fs::write(&conflict, b"other")?;
        fs::write(&removed, b"old")?;
        for path in [&noop, &drifted, &adoptable, &conflict, &removed] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
        }
        let desired = [
            ("noop", &noop, b"same".as_slice()),
            ("drifted", &drifted, b"same".as_slice()),
            ("adoptable", &adoptable, b"same".as_slice()),
            ("conflict", &conflict, b"same".as_slice()),
        ]
        .into_iter()
        .map(|(source, destination, bytes)| DesiredAsset {
            source_id: source.to_owned(),
            destination: destination.clone(),
            payload: DesiredPayload::File {
                bytes: bytes.to_vec(),
                mode: 0o644,
            },
        })
        .collect::<Vec<_>>();
        let owned = vec![
            OwnedAssetState {
                source_id: "noop".to_owned(),
                destination: noop,
                expected: ExpectedNode::file(b"same", 0o644),
            },
            OwnedAssetState {
                source_id: "drifted".to_owned(),
                destination: drifted,
                expected: ExpectedNode::file(b"same", 0o644),
            },
            OwnedAssetState {
                source_id: "removed".to_owned(),
                destination: removed,
                expected: ExpectedNode::file(b"old", 0o644),
            },
        ];
        let plan = build_plan(&desired, &owned, &policy(temp.path()));
        let actions = plan
            .entries
            .iter()
            .map(|entry| (&entry.source[..], entry.action))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(actions["noop"], PlanAction::Noop);
        assert_eq!(actions["drifted"], PlanAction::Drifted);
        assert_eq!(actions["adoptable"], PlanAction::Adoptable);
        assert_eq!(actions["conflict"], PlanAction::Conflict);
        assert_eq!(actions["removed"], PlanAction::Remove);
        assert!(!plan.applicable);
        Ok(())
    }

    #[test]
    fn emits_every_mutation_kind_for_creates_and_verified_updates() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let canonical = temp.path().join("canonical");
        let links = temp.path().join("links");
        fs::create_dir_all(&canonical)?;
        fs::create_dir_all(&links)?;

        let directory = temp.path().join("new-directory");
        let symlink_path = links.join("linked");
        let mode_only = temp.path().join("mode-only");
        let replace = temp.path().join("replace");
        let change_kind = temp.path().join("change-kind");
        fs::write(&mode_only, b"mode")?;
        fs::write(&replace, b"old")?;
        fs::create_dir(&change_kind)?;
        fs::set_permissions(&mode_only, fs::Permissions::from_mode(0o644))?;
        fs::set_permissions(&replace, fs::Permissions::from_mode(0o644))?;
        fs::set_permissions(&change_kind, fs::Permissions::from_mode(0o755))?;

        let desired = vec![
            DesiredAsset {
                source_id: "directory".to_owned(),
                destination: directory,
                payload: DesiredPayload::Directory { mode: 0o700 },
            },
            DesiredAsset {
                source_id: "symlink".to_owned(),
                destination: symlink_path,
                payload: DesiredPayload::Symlink {
                    target: PathBuf::from("../canonical/linked"),
                    canonical_target: canonical.join("linked"),
                },
            },
            DesiredAsset {
                source_id: "mode".to_owned(),
                destination: mode_only.clone(),
                payload: DesiredPayload::File {
                    bytes: b"mode".to_vec(),
                    mode: 0o755,
                },
            },
            DesiredAsset {
                source_id: "replace".to_owned(),
                destination: replace.clone(),
                payload: DesiredPayload::File {
                    bytes: b"new".to_vec(),
                    mode: 0o644,
                },
            },
            DesiredAsset {
                source_id: "change-kind".to_owned(),
                destination: change_kind.clone(),
                payload: DesiredPayload::File {
                    bytes: b"now a file".to_vec(),
                    mode: 0o644,
                },
            },
        ];
        let owned = vec![
            OwnedAssetState {
                source_id: "mode".to_owned(),
                destination: mode_only,
                expected: ExpectedNode::file(b"mode", 0o644),
            },
            OwnedAssetState {
                source_id: "replace".to_owned(),
                destination: replace,
                expected: ExpectedNode::file(b"old", 0o644),
            },
            OwnedAssetState {
                source_id: "change-kind".to_owned(),
                destination: change_kind,
                expected: ExpectedNode::directory(0o755),
            },
        ];
        let plan = build_plan(
            &desired,
            &owned,
            &PathPolicy {
                allowed_roots: vec![AllowedRoot {
                    lexical: temp.path().to_path_buf(),
                    real: temp.path().to_path_buf(),
                }],
                claude_symlinks: Some(ClaudeSymlinkPolicy {
                    link_root: links,
                    canonical_root: canonical,
                }),
            },
        );

        assert!(plan.applicable);
        let kinds = plan
            .operations
            .iter()
            .map(|operation| operation.kind)
            .collect::<Vec<_>>();
        for kind in [
            MutationKind::EnsureDirectory,
            MutationKind::WriteFile,
            MutationKind::ReplaceFile,
            MutationKind::SetMode,
            MutationKind::CreateSymlink,
            MutationKind::RemoveOwnedPath,
        ] {
            assert!(kinds.contains(&kind), "missing mutation kind {kind:?}");
        }
        let mode_operation = plan
            .operations
            .iter()
            .find(|operation| operation.kind == MutationKind::SetMode)
            .unwrap_or_else(|| panic!("set-mode operation missing"));
        assert_eq!(
            mode_operation.inverse,
            PlannedInverse::RestoreMode { mode: 0o644 }
        );
        Ok(())
    }

    #[test]
    fn duplicate_inputs_and_missing_owned_paths_block_the_plan() {
        let temp = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let duplicate_desired = temp.path().join("duplicate-desired");
        let duplicate_owned = temp.path().join("duplicate-owned");
        let removed_but_missing = temp.path().join("removed-but-missing");
        let desired = vec![
            DesiredAsset {
                source_id: "desired-a".to_owned(),
                destination: duplicate_desired.clone(),
                payload: DesiredPayload::Directory { mode: 0o700 },
            },
            DesiredAsset {
                source_id: "desired-b".to_owned(),
                destination: duplicate_desired,
                payload: DesiredPayload::Directory { mode: 0o755 },
            },
            DesiredAsset {
                source_id: "owned".to_owned(),
                destination: duplicate_owned.clone(),
                payload: DesiredPayload::Directory { mode: 0o700 },
            },
        ];
        let owned = vec![
            OwnedAssetState {
                source_id: "owned-a".to_owned(),
                destination: duplicate_owned.clone(),
                expected: ExpectedNode::directory(0o700),
            },
            OwnedAssetState {
                source_id: "owned-b".to_owned(),
                destination: duplicate_owned,
                expected: ExpectedNode::directory(0o700),
            },
            OwnedAssetState {
                source_id: "removed".to_owned(),
                destination: removed_but_missing,
                expected: ExpectedNode::directory(0o700),
            },
        ];

        let plan = build_plan(&desired, &owned, &policy(temp.path()));

        assert!(!plan.applicable);
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate_destination")
        );
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate_receipt_destination")
        );
        assert_eq!(
            plan.entries
                .iter()
                .filter(|entry| entry.action == PlanAction::Drifted)
                .count(),
            2
        );
    }

    #[test]
    fn executable_mode_difference_is_drift() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("script.sh");
        fs::write(&path, b"#!/bin/sh\n")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
        let desired = vec![DesiredAsset {
            source_id: "script".to_owned(),
            destination: path.clone(),
            payload: DesiredPayload::File {
                bytes: b"#!/bin/sh\n".to_vec(),
                mode: 0o755,
            },
        }];
        let owned = vec![OwnedAssetState {
            source_id: "script".to_owned(),
            destination: path,
            expected: ExpectedNode::file(b"#!/bin/sh\n", 0o755),
        }];
        let plan = build_plan(&desired, &owned, &policy(temp.path()));
        assert_eq!(plan.entries[0].action, PlanAction::Drifted);
        Ok(())
    }

    #[test]
    fn broken_and_cyclic_symlinks_are_explicit_conflicts() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let canonical = temp.path().join("canonical");
        let links = temp.path().join("links");
        fs::create_dir_all(&canonical)?;
        fs::create_dir_all(&links)?;
        let broken = links.join("broken");
        let cycle = links.join("cycle");
        symlink("../canonical/broken", &broken)?;
        symlink("../canonical/cycle", &cycle)?;
        symlink("../links/cycle", canonical.join("cycle"))?;
        let policy = PathPolicy {
            allowed_roots: vec![AllowedRoot {
                lexical: links.clone(),
                real: links.clone(),
            }],
            claude_symlinks: Some(ClaudeSymlinkPolicy {
                link_root: links.clone(),
                canonical_root: canonical.clone(),
            }),
        };
        let desired = vec![
            DesiredAsset {
                source_id: "broken".to_owned(),
                destination: broken,
                payload: DesiredPayload::Symlink {
                    target: PathBuf::from("../canonical/broken"),
                    canonical_target: canonical.join("broken"),
                },
            },
            DesiredAsset {
                source_id: "cycle".to_owned(),
                destination: cycle,
                payload: DesiredPayload::Symlink {
                    target: PathBuf::from("../canonical/cycle"),
                    canonical_target: canonical.join("cycle"),
                },
            },
        ];
        let plan = build_plan(&desired, &[], &policy);
        assert!(
            plan.entries
                .iter()
                .all(|entry| entry.action == PlanAction::Conflict)
        );
        assert!(
            plan.entries
                .iter()
                .any(|entry| entry.reason.contains("broken"))
        );
        assert!(
            plan.entries
                .iter()
                .any(|entry| entry.reason.contains("cyclic"))
        );
        Ok(())
    }

    #[test]
    fn unmanaged_symlinks_distinguish_adoptable_wrong_and_escaped_targets()
    -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let canonical = temp.path().join("canonical");
        let links = temp.path().join("links");
        fs::create_dir_all(&canonical)?;
        fs::create_dir_all(&links)?;
        fs::write(canonical.join("adoptable"), b"owned elsewhere")?;
        fs::write(canonical.join("other"), b"wrong target")?;
        symlink("../canonical/adoptable", links.join("adoptable"))?;
        symlink("../canonical/other", links.join("wrong"))?;
        symlink("../../../../../../../../outside", links.join("escaped"))?;

        let desired = ["adoptable", "wrong", "escaped"]
            .into_iter()
            .map(|name| DesiredAsset {
                source_id: name.to_owned(),
                destination: links.join(name),
                payload: DesiredPayload::Symlink {
                    target: PathBuf::from(format!("../canonical/{name}")),
                    canonical_target: canonical.join(name),
                },
            })
            .collect::<Vec<_>>();
        let plan = build_plan(
            &desired,
            &[],
            &PathPolicy {
                allowed_roots: vec![AllowedRoot {
                    lexical: links.clone(),
                    real: links.clone(),
                }],
                claude_symlinks: Some(ClaudeSymlinkPolicy {
                    link_root: links,
                    canonical_root: canonical,
                }),
            },
        );
        let entries = plan
            .entries
            .iter()
            .map(|entry| (entry.source.as_str(), entry))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(entries["adoptable"].action, PlanAction::Adoptable);
        assert_eq!(entries["wrong"].action, PlanAction::Conflict);
        assert!(entries["wrong"].reason.contains("different canonical"));
        assert_eq!(entries["escaped"].action, PlanAction::Conflict);
        assert!(entries["escaped"].reason.contains("outside"));
        Ok(())
    }

    #[test]
    fn selects_the_most_specific_root_and_removes_children_first() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let nested_root = temp.path().join("nested");
        let parent = nested_root.join("parent");
        let child = parent.join("child");
        fs::create_dir_all(&child)?;
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&child, fs::Permissions::from_mode(0o755))?;
        let owned = [
            OwnedAssetState {
                source_id: "parent".to_owned(),
                destination: parent.clone(),
                expected: ExpectedNode::directory(0o755),
            },
            OwnedAssetState {
                source_id: "child".to_owned(),
                destination: child.clone(),
                expected: ExpectedNode::directory(0o755),
            },
        ];
        let plan = build_plan(
            &[],
            &owned,
            &PathPolicy {
                allowed_roots: vec![
                    AllowedRoot {
                        lexical: temp.path().to_path_buf(),
                        real: temp.path().to_path_buf(),
                    },
                    AllowedRoot {
                        lexical: nested_root.clone(),
                        real: nested_root.clone(),
                    },
                ],
                claude_symlinks: None,
            },
        );

        assert!(plan.applicable);
        assert_eq!(plan.operations.len(), 2);
        assert!(
            plan.operations
                .iter()
                .all(|operation| operation.root == nested_root)
        );
        assert_eq!(plan.operations[0].destination, child);
        assert_eq!(plan.operations[1].destination, parent);
        Ok(())
    }

    #[test]
    fn unsupported_filesystem_nodes_fail_closed() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let destination = temp.path().join("socket");
        let _listener = UnixListener::bind(&destination)?;
        let desired = [DesiredAsset {
            source_id: "socket".to_owned(),
            destination,
            payload: DesiredPayload::File {
                bytes: b"replacement".to_vec(),
                mode: 0o644,
            },
        }];

        let plan = build_plan(&desired, &[], &policy(temp.path()));

        assert!(!plan.applicable);
        assert_eq!(plan.entries[0].action, PlanAction::Conflict);
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "filesystem_scan_failed")
        );
        Ok(())
    }

    #[test]
    fn invalid_owned_roots_and_owned_scan_failures_block_removal() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let managed = temp.path().join("managed");
        fs::create_dir(&managed)?;
        let outside = temp.path().join("outside");
        fs::write(&outside, b"outside")?;
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o644))?;
        let socket = managed.join("socket");
        let _listener = UnixListener::bind(&socket)?;
        let blocker = managed.join("blocker");
        fs::write(&blocker, b"not a directory")?;
        let inaccessible_child = blocker.join("child");
        let desired = [DesiredAsset {
            source_id: "inaccessible-child".to_owned(),
            destination: inaccessible_child,
            payload: DesiredPayload::File {
                bytes: b"new".to_vec(),
                mode: 0o644,
            },
        }];
        let owned = [
            OwnedAssetState {
                source_id: "outside".to_owned(),
                destination: outside.clone(),
                expected: ExpectedNode::file(b"outside", 0o644),
            },
            OwnedAssetState {
                source_id: "socket".to_owned(),
                destination: socket,
                expected: ExpectedNode::file(b"socket", 0o644),
            },
        ];

        let plan = build_plan(&desired, &owned, &policy(&managed));

        assert!(!plan.applicable);
        assert!(plan.entries.iter().any(|entry| {
            entry.destination == outside
                && entry.action == PlanAction::Conflict
                && entry.reason.contains("root policy")
        }));
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "filesystem_scan_failed")
        );
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "destination_outside_roots")
        );
        Ok(())
    }

    #[test]
    fn symlink_policy_and_escaping_ancestors_fail_closed() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let managed = temp.path().join("managed");
        let outside = temp.path().join("outside");
        fs::create_dir(&managed)?;
        fs::create_dir(&outside)?;
        symlink(&outside, managed.join("escape"))?;
        let blocker = managed.join("blocker");
        fs::write(&blocker, b"not a directory")?;
        let desired = vec![
            DesiredAsset {
                source_id: "no-policy".to_owned(),
                destination: managed.join("no-policy"),
                payload: DesiredPayload::Symlink {
                    target: PathBuf::from("target"),
                    canonical_target: managed.join("target"),
                },
            },
            DesiredAsset {
                source_id: "escaped-ancestor".to_owned(),
                destination: managed.join("escape/asset"),
                payload: DesiredPayload::Directory { mode: 0o700 },
            },
            DesiredAsset {
                source_id: "invalid-ancestor".to_owned(),
                destination: blocker.join("child/asset"),
                payload: DesiredPayload::Directory { mode: 0o700 },
            },
        ];

        let plan = build_plan(&desired, &[], &policy(&managed));

        assert!(!plan.applicable);
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "symlink_not_allowed")
        );
        assert_eq!(
            plan.diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "symlink_escape")
                .count(),
            2
        );
        Ok(())
    }

    #[test]
    fn symlink_removal_absolute_targets_and_deep_chains_are_planned_safely()
    -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let canonical = temp.path().join("canonical");
        let links = temp.path().join("links");
        fs::create_dir(&canonical)?;
        fs::create_dir(&links)?;
        let obsolete = links.join("obsolete");
        symlink("../canonical/obsolete", &obsolete)?;
        for index in 0..64 {
            symlink(
                format!("chain-{}", index + 1),
                links.join(format!("chain-{index}")),
            )?;
        }
        let blocker = temp.path().join("blocker");
        fs::write(&blocker, b"not a directory")?;
        symlink("../blocker/child", links.join("blocked"))?;
        let desired = vec![
            DesiredAsset {
                source_id: "absolute".to_owned(),
                destination: links.join("absolute"),
                payload: DesiredPayload::Symlink {
                    target: canonical.join("absolute"),
                    canonical_target: canonical.join("absolute"),
                },
            },
            DesiredAsset {
                source_id: "chain".to_owned(),
                destination: links.join("chain-0"),
                payload: DesiredPayload::Symlink {
                    target: PathBuf::from("../canonical/chain-0"),
                    canonical_target: canonical.join("chain-0"),
                },
            },
            DesiredAsset {
                source_id: "blocked".to_owned(),
                destination: links.join("blocked"),
                payload: DesiredPayload::Symlink {
                    target: PathBuf::from("../canonical/blocked"),
                    canonical_target: canonical.join("blocked"),
                },
            },
        ];
        let owned = [OwnedAssetState {
            source_id: "obsolete".to_owned(),
            destination: obsolete,
            expected: ExpectedNode::symlink(PathBuf::from("../canonical/obsolete")),
        }];
        let plan = build_plan(
            &desired,
            &owned,
            &PathPolicy {
                allowed_roots: vec![AllowedRoot {
                    lexical: links.clone(),
                    real: links.clone(),
                }],
                claude_symlinks: Some(ClaudeSymlinkPolicy {
                    link_root: links,
                    canonical_root: canonical,
                }),
            },
        );

        assert!(
            plan.entries
                .iter()
                .any(|entry| { entry.source == "obsolete" && entry.action == PlanAction::Remove })
        );
        assert!(
            plan.entries
                .iter()
                .any(|entry| { entry.source == "absolute" && entry.action == PlanAction::Create })
        );
        assert!(plan.entries.iter().any(|entry| {
            entry.source == "chain"
                && entry.action == PlanAction::Conflict
                && entry.reason.contains("cyclic")
        }));
        assert!(plan.entries.iter().any(|entry| {
            entry.source == "blocked"
                && entry.action == PlanAction::Conflict
                && entry.reason.contains("outside")
        }));
        assert_eq!(normalize_absolute(Path::new("relative")), None);
        Ok(())
    }

    #[test]
    fn rejects_a_symlink_to_a_different_canonical_skill() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let canonical = temp.path().join("canonical");
        let links = temp.path().join("links");
        fs::create_dir_all(&canonical)?;
        fs::create_dir_all(&links)?;
        let desired = [DesiredAsset {
            source_id: "foo".to_owned(),
            destination: links.join("foo"),
            payload: DesiredPayload::Symlink {
                target: PathBuf::from("../canonical/bar"),
                canonical_target: canonical.join("bar"),
            },
        }];
        let plan = build_plan(
            &desired,
            &[],
            &PathPolicy {
                allowed_roots: vec![AllowedRoot {
                    lexical: links.clone(),
                    real: links,
                }],
                claude_symlinks: Some(ClaudeSymlinkPolicy {
                    link_root: temp.path().join("links"),
                    canonical_root: canonical,
                }),
            },
        );
        assert!(!plan.applicable);
        assert_eq!(plan.entries[0].action, PlanAction::Conflict);
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "symlink_escape")
        );
        Ok(())
    }

    #[test]
    fn unsafe_and_non_utf8_destinations_are_rejected_losslessly() {
        let temp = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let outside = DesiredAsset {
            source_id: "outside".to_owned(),
            destination: PathBuf::from("/tmp/outside"),
            payload: DesiredPayload::Directory { mode: 0o700 },
        };
        let non_utf8 = DesiredAsset {
            source_id: "bytes".to_owned(),
            destination: temp
                .path()
                .join(std::ffi::OsString::from_vec(vec![b'x', 0xff])),
            payload: DesiredPayload::Directory { mode: 0o700 },
        };
        let relative = DesiredAsset {
            source_id: "relative".to_owned(),
            destination: PathBuf::from("relative/path"),
            payload: DesiredPayload::Directory { mode: 0o700 },
        };
        let traversal = DesiredAsset {
            source_id: "traversal".to_owned(),
            destination: temp.path().join("nested/../escape"),
            payload: DesiredPayload::Directory { mode: 0o700 },
        };
        let plan = build_plan(
            &[outside, non_utf8, relative, traversal],
            &[],
            &policy(temp.path()),
        );
        assert!(!plan.applicable);
        assert!(
            plan.diagnostics
                .iter()
                .any(|item| item.code == "non_utf8_path"
                    && item.path_utf8.is_none()
                    && item.path_bytes_hex.is_some())
        );
        assert!(
            plan.diagnostics
                .iter()
                .any(|item| item.code == "destination_outside_roots")
        );
        assert_eq!(
            plan.diagnostics
                .iter()
                .filter(|item| item.code == "unsafe_destination")
                .count(),
            2
        );
    }
}
