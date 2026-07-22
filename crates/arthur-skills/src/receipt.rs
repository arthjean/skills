use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::provider::{
    ENVIRONMENT_EXIT_CODE, ProviderId, ProviderRegistry, ResolvedRoots, RootIdentity,
};

pub const RECEIPT_SCHEMA_VERSION: u16 = 1;
pub const INTEGRITY_EXIT_CODE: u8 = 5;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    Committed,
    RecoveryRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptRoots {
    pub home: RootIdentity,
    pub canonical: RootIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderReceipt {
    pub provider: ProviderId,
    pub managed_integration: bool,
    pub implicit_skill_visibility: bool,
    pub root: Option<RootIdentity>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnedAssetKind {
    File,
    Directory,
    Symlink,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedAsset {
    pub source_id: String,
    pub destination: PathBuf,
    pub kind: OwnedAssetKind,
    pub hash: Option<String>,
    pub mode: Option<u32>,
    pub link_target: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ProviderId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema_version: u16,
    pub cli_version: String,
    pub catalog_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    pub state: ReceiptState,
    pub roots: ReceiptRoots,
    pub providers: Vec<ProviderReceipt>,
    pub assets: Vec<OwnedAsset>,
}

impl Receipt {
    pub fn new(
        cli_version: impl Into<String>,
        catalog_sha256: impl Into<String>,
        roots: &ResolvedRoots,
    ) -> Self {
        let providers = ProviderRegistry::all()
            .iter()
            .map(|definition| {
                let resolved = roots.provider(definition.id);
                ProviderReceipt {
                    provider: definition.id,
                    managed_integration: resolved.is_some(),
                    implicit_skill_visibility: definition.capabilities.implicit_skill_visibility,
                    root: resolved.map(|provider| provider.root.clone()),
                }
            })
            .collect();

        Self {
            schema_version: RECEIPT_SCHEMA_VERSION,
            cli_version: cli_version.into(),
            catalog_sha256: catalog_sha256.into(),
            transaction_id: None,
            state: ReceiptState::Committed,
            roots: ReceiptRoots {
                home: roots.home.clone(),
                canonical: roots.canonical.clone(),
            },
            providers,
            assets: Vec::new(),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ReceiptError> {
        let receipt: Self = serde_json::from_slice(bytes)
            .map_err(|error| ReceiptError::InvalidJson(error.to_string()))?;
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), ReceiptError> {
        if self.schema_version != RECEIPT_SCHEMA_VERSION {
            return Err(ReceiptError::UnsupportedSchema {
                observed: self.schema_version,
            });
        }
        if self.cli_version.is_empty() {
            return Err(ReceiptError::MissingField("cli_version"));
        }
        if !valid_sha256(&self.catalog_sha256) {
            return Err(ReceiptError::InvalidHash("catalog_sha256"));
        }
        if self
            .transaction_id
            .as_deref()
            .is_some_and(|transaction_id| {
                transaction_id.is_empty()
                    || !transaction_id
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            })
        {
            return Err(ReceiptError::InvalidAsset {
                source_id: "receipt".to_owned(),
                detail: "transaction_id must be safe ASCII",
            });
        }
        validate_root("home", &self.roots.home)?;
        validate_root("canonical", &self.roots.canonical)?;

        let mut provider_ids = BTreeSet::new();
        for provider in &self.providers {
            if !provider_ids.insert(provider.provider) {
                return Err(ReceiptError::DuplicateProvider(provider.provider));
            }
            if provider.managed_integration && provider.root.is_none() {
                return Err(ReceiptError::MissingProviderRoot(provider.provider));
            }
            if let Some(root) = &provider.root {
                validate_root(provider.provider.as_str(), root)?;
            }
        }

        let mut destinations = BTreeSet::new();
        for asset in &self.assets {
            validate_asset(asset)?;
            validate_asset_root(asset, self)?;
            if !destinations.insert(asset.destination.as_path()) {
                return Err(ReceiptError::DuplicateDestination(
                    asset.destination.clone(),
                ));
            }
        }
        Ok(())
    }

    pub fn owned_by_destination(&self) -> Result<BTreeMap<&Path, &OwnedAsset>, ReceiptError> {
        let mut owned = BTreeMap::new();
        for asset in &self.assets {
            if owned.insert(asset.destination.as_path(), asset).is_some() {
                return Err(ReceiptError::DuplicateDestination(
                    asset.destination.clone(),
                ));
            }
        }
        Ok(owned)
    }

    pub fn owned_asset(&self, destination: &Path) -> Option<&OwnedAsset> {
        self.assets
            .iter()
            .find(|asset| asset.destination == destination)
    }

    pub fn validate_roots(&self, current: &ResolvedRoots) -> Result<(), ReceiptError> {
        compare_root(RootScope::Home, &self.roots.home, &current.home)?;
        compare_root(
            RootScope::Canonical,
            &self.roots.canonical,
            &current.canonical,
        )?;

        for current_provider in &current.providers {
            let stored = self
                .providers
                .iter()
                .find(|provider| provider.provider == current_provider.id)
                .and_then(|provider| provider.root.as_ref());
            if let Some(stored) = stored {
                compare_root(
                    RootScope::Provider(current_provider.id),
                    stored,
                    &current_provider.root,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootScope {
    Home,
    Canonical,
    Provider(ProviderId),
}

impl fmt::Display for RootScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Home => formatter.write_str("HOME"),
            Self::Canonical => formatter.write_str("canonical root"),
            Self::Provider(provider) => write!(formatter, "{provider} root"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootMismatch {
    pub root: RootScope,
    pub recorded: RootIdentity,
    pub current: RootIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiptError {
    InvalidJson(String),
    UnsupportedSchema {
        observed: u16,
    },
    MissingField(&'static str),
    InvalidHash(&'static str),
    DuplicateProvider(ProviderId),
    MissingProviderRoot(ProviderId),
    DuplicateDestination(PathBuf),
    InvalidPath {
        field: &'static str,
        path: PathBuf,
    },
    InvalidAsset {
        source_id: String,
        detail: &'static str,
    },
    RootMismatch(RootMismatch),
}

impl ReceiptError {
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::InvalidPath { .. } | Self::RootMismatch(_) => ENVIRONMENT_EXIT_CODE,
            _ => INTEGRITY_EXIT_CODE,
        }
    }
}

impl fmt::Display for ReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(detail) => write!(formatter, "receipt JSON is invalid: {detail}"),
            Self::UnsupportedSchema { observed } => write!(
                formatter,
                "unsupported receipt schema {observed}; expected {RECEIPT_SCHEMA_VERSION}"
            ),
            Self::MissingField(field) => write!(formatter, "receipt field {field} cannot be empty"),
            Self::InvalidHash(field) => {
                write!(
                    formatter,
                    "receipt field {field} must be a lowercase SHA-256"
                )
            }
            Self::DuplicateProvider(provider) => {
                write!(
                    formatter,
                    "receipt contains provider {provider} more than once"
                )
            }
            Self::MissingProviderRoot(provider) => write!(
                formatter,
                "managed provider {provider} has no recorded root identity"
            ),
            Self::DuplicateDestination(path) => write!(
                formatter,
                "receipt claims destination more than once: {}",
                path.display()
            ),
            Self::InvalidPath { field, path } => {
                write!(
                    formatter,
                    "receipt {field} is not a safe UTF-8 path: {}",
                    path.display()
                )
            }
            Self::InvalidAsset { source_id, detail } => {
                write!(formatter, "receipt asset {source_id} is invalid: {detail}")
            }
            Self::RootMismatch(mismatch) => write!(
                formatter,
                "{} identity differs from the receipt; restore the original environment before migration",
                mismatch.root
            ),
        }
    }
}

impl std::error::Error for ReceiptError {}

fn compare_root(
    root: RootScope,
    recorded: &RootIdentity,
    current: &RootIdentity,
) -> Result<(), ReceiptError> {
    if recorded == current {
        return Ok(());
    }
    Err(ReceiptError::RootMismatch(RootMismatch {
        root,
        recorded: recorded.clone(),
        current: current.clone(),
    }))
}

fn validate_root(field: &'static str, root: &RootIdentity) -> Result<(), ReceiptError> {
    validate_absolute_path(field, &root.lexical)?;
    validate_absolute_path(field, &root.real)
}

fn validate_absolute_path(field: &'static str, path: &Path) -> Result<(), ReceiptError> {
    let is_normalized = path.is_absolute()
        && path.to_str().is_some()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        });
    if is_normalized {
        Ok(())
    } else {
        Err(ReceiptError::InvalidPath {
            field,
            path: path.to_path_buf(),
        })
    }
}

fn validate_asset(asset: &OwnedAsset) -> Result<(), ReceiptError> {
    if asset.source_id.is_empty() {
        return Err(ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "source_id cannot be empty",
        });
    }
    validate_absolute_path("asset.destination", &asset.destination)?;
    if let Some(target) = &asset.link_target
        && target.to_str().is_none()
    {
        return Err(ReceiptError::InvalidPath {
            field: "asset.link_target",
            path: target.clone(),
        });
    }
    if asset.mode.is_some_and(|mode| mode & !0o777 != 0) {
        return Err(ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "mode contains non-permission bits",
        });
    }
    if asset
        .hash
        .as_deref()
        .is_some_and(|hash| !valid_sha256(hash))
    {
        return Err(ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "hash must be a lowercase SHA-256",
        });
    }

    let valid_shape = match asset.kind {
        OwnedAssetKind::File => {
            asset.hash.is_some() && asset.mode.is_some() && asset.link_target.is_none()
        }
        OwnedAssetKind::Directory => {
            asset.hash.is_none() && asset.mode.is_some() && asset.link_target.is_none()
        }
        OwnedAssetKind::Symlink => {
            asset.hash.is_none() && asset.mode.is_none() && asset.link_target.is_some()
        }
    };
    if !valid_shape {
        return Err(ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "hash, mode, and link_target do not match the asset kind",
        });
    }

    let mut references = BTreeSet::new();
    if asset
        .references
        .iter()
        .any(|provider| !references.insert(*provider))
    {
        return Err(ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "provider references contain duplicates",
        });
    }
    Ok(())
}

fn validate_asset_root(asset: &OwnedAsset, receipt: &Receipt) -> Result<(), ReceiptError> {
    let under_known_root = asset
        .destination
        .starts_with(&receipt.roots.canonical.lexical)
        || receipt.providers.iter().any(|provider| {
            provider
                .root
                .as_ref()
                .is_some_and(|root| asset.destination.starts_with(&root.lexical))
        });
    if !under_known_root {
        return Err(ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "destination is outside every recorded root",
        });
    }
    if asset.kind != OwnedAssetKind::Symlink {
        return Ok(());
    }

    let claude_root = receipt
        .providers
        .iter()
        .find(|provider| provider.provider == ProviderId::Claude)
        .and_then(|provider| provider.root.as_ref())
        .ok_or_else(|| ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "symlink ownership requires a recorded Claude root",
        })?;
    let link_root = claude_root.lexical.join("skills");
    let relative =
        asset
            .destination
            .strip_prefix(&link_root)
            .map_err(|_| ReceiptError::InvalidAsset {
                source_id: asset.source_id.clone(),
                detail: "owned symlink is outside the Claude skills root",
            })?;
    if relative.components().count() != 1 {
        return Err(ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "Claude activation must be one symlink per skill",
        });
    }
    let target = asset
        .link_target
        .as_ref()
        .ok_or_else(|| ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "owned symlink has no target",
        })?;
    let joined = if target.is_absolute() {
        target.clone()
    } else {
        asset
            .destination
            .parent()
            .unwrap_or(link_root.as_path())
            .join(target)
    };
    let actual = normalize_path(&joined).ok_or_else(|| ReceiptError::InvalidAsset {
        source_id: asset.source_id.clone(),
        detail: "owned symlink target escapes the filesystem root",
    })?;
    let expected = receipt
        .roots
        .canonical
        .lexical
        .join("skills")
        .join(relative);
    if actual != expected {
        return Err(ReceiptError::InvalidAsset {
            source_id: asset.source_id.clone(),
            detail: "owned symlink does not target its corresponding canonical skill",
        });
    }
    Ok(())
}

fn normalize_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Prefix(_) => return None,
        }
    }
    Some(normalized)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    use tempfile::{TempDir, tempdir};

    use crate::provider::{
        ENVIRONMENT_EXIT_CODE, ProviderId, ResolvedRoots, RootIdentity, resolve_roots_from,
    };

    use super::{
        INTEGRITY_EXIT_CODE, OwnedAsset, OwnedAssetKind, RECEIPT_SCHEMA_VERSION, Receipt,
        ReceiptError, ReceiptState, RootMismatch, RootScope, normalize_path, validate_asset_root,
    };

    fn receipt_fixture(selected: &[ProviderId]) -> (TempDir, ResolvedRoots, Receipt) {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, selected)
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        (home, roots, receipt)
    }

    fn file_asset(destination: PathBuf) -> OwnedAsset {
        OwnedAsset {
            source_id: "skills/example/SKILL.md".to_owned(),
            destination,
            kind: OwnedAssetKind::File,
            hash: Some("b".repeat(64)),
            mode: Some(0o644),
            link_target: None,
            references: Vec::new(),
        }
    }

    fn symlink_asset(destination: PathBuf, link_target: Option<PathBuf>) -> OwnedAsset {
        OwnedAsset {
            source_id: "skills/example".to_owned(),
            destination,
            kind: OwnedAssetKind::Symlink,
            hash: None,
            mode: None,
            link_target,
            references: vec![ProviderId::Claude],
        }
    }

    fn validation_error(receipt: &Receipt) -> ReceiptError {
        match receipt.validate() {
            Ok(()) => panic!("invalid receipt passed validation"),
            Err(error) => error,
        }
    }

    fn assert_asset_error(receipt: &Receipt, asset: OwnedAsset, expected: ReceiptError) {
        let mut invalid = receipt.clone();
        invalid.assets = vec![asset];
        assert_eq!(validation_error(&invalid), expected);
    }

    fn identity(path: &str, device: u64) -> RootIdentity {
        RootIdentity {
            lexical: PathBuf::from(path),
            real: PathBuf::from(path),
            device,
        }
    }

    #[test]
    fn receipt_indexes_owned_destinations_and_round_trips_v1() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Claude])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        let destination = roots.canonical_skills.join("example/SKILL.md");
        receipt.assets.push(OwnedAsset {
            source_id: "skills/example/SKILL.md".to_owned(),
            destination: destination.clone(),
            kind: OwnedAssetKind::File,
            hash: Some("b".repeat(64)),
            mode: Some(0o644),
            link_target: None,
            references: vec![ProviderId::Claude],
        });
        receipt
            .validate()
            .unwrap_or_else(|error| panic!("receipt validation failed: {error}"));
        let encoded = serde_json::to_vec(&receipt)
            .unwrap_or_else(|error| panic!("receipt encoding failed: {error}"));
        let decoded = Receipt::decode(&encoded)
            .unwrap_or_else(|error| panic!("receipt decoding failed: {error}"));
        let owned = decoded
            .owned_by_destination()
            .unwrap_or_else(|error| panic!("ownership index failed: {error}"));
        assert_eq!(
            owned
                .get(destination.as_path())
                .map(|asset| asset.source_id.as_str()),
            Some("skills/example/SKILL.md")
        );
        assert_eq!(
            decoded
                .owned_asset(destination.as_path())
                .map(|asset| asset.source_id.as_str()),
            Some("skills/example/SKILL.md")
        );
        assert!(decoded.owned_asset(Path::new("/not-owned")).is_none());
    }

    #[test]
    fn new_receipt_records_selected_and_implicit_provider_state() {
        let (_home, roots, receipt) = receipt_fixture(&[ProviderId::Claude]);

        assert_eq!(receipt.schema_version, RECEIPT_SCHEMA_VERSION);
        assert_eq!(receipt.cli_version, "0.1.0");
        assert_eq!(receipt.catalog_sha256, "a".repeat(64));
        assert_eq!(receipt.transaction_id, None);
        assert_eq!(receipt.state, ReceiptState::Committed);
        assert_eq!(receipt.roots.home, roots.home);
        assert_eq!(receipt.roots.canonical, roots.canonical);
        assert!(receipt.assets.is_empty());

        let claude = receipt
            .providers
            .iter()
            .find(|provider| provider.provider == ProviderId::Claude)
            .unwrap_or_else(|| panic!("missing Claude receipt"));
        assert!(claude.managed_integration);
        assert!(!claude.implicit_skill_visibility);
        assert_eq!(
            claude.root.as_ref(),
            roots
                .provider(ProviderId::Claude)
                .map(|provider| &provider.root)
        );

        let codex = receipt
            .providers
            .iter()
            .find(|provider| provider.provider == ProviderId::Codex)
            .unwrap_or_else(|| panic!("missing Codex receipt"));
        assert!(!codex.managed_integration);
        assert!(codex.implicit_skill_visibility);
        assert!(codex.root.is_none());
    }

    #[test]
    fn decode_distinguishes_malformed_json_from_invalid_receipts() {
        assert!(matches!(
            Receipt::decode(b"{"),
            Err(ReceiptError::InvalidJson(_))
        ));

        let (_home, _roots, receipt) = receipt_fixture(&[]);
        let mut value = serde_json::to_value(&receipt)
            .unwrap_or_else(|error| panic!("receipt encoding failed: {error}"));
        value
            .as_object_mut()
            .unwrap_or_else(|| panic!("receipt did not encode as an object"))
            .insert("unexpected".to_owned(), serde_json::Value::Bool(true));
        let unknown_field = serde_json::to_vec(&value)
            .unwrap_or_else(|error| panic!("receipt encoding failed: {error}"));
        assert!(matches!(
            Receipt::decode(&unknown_field),
            Err(ReceiptError::InvalidJson(_))
        ));

        let mut unsupported = receipt;
        unsupported.schema_version += 1;
        let bytes = serde_json::to_vec(&unsupported)
            .unwrap_or_else(|error| panic!("receipt encoding failed: {error}"));
        assert_eq!(
            Receipt::decode(&bytes),
            Err(ReceiptError::UnsupportedSchema {
                observed: RECEIPT_SCHEMA_VERSION + 1,
            })
        );
    }

    #[test]
    fn receipt_header_and_provider_invariants_are_enforced() {
        let (_home, _roots, receipt) = receipt_fixture(&[ProviderId::Claude]);

        let mut invalid = receipt.clone();
        invalid.schema_version += 1;
        assert_eq!(
            validation_error(&invalid),
            ReceiptError::UnsupportedSchema {
                observed: RECEIPT_SCHEMA_VERSION + 1,
            }
        );

        let mut invalid = receipt.clone();
        invalid.cli_version.clear();
        assert_eq!(
            validation_error(&invalid),
            ReceiptError::MissingField("cli_version")
        );

        let mut invalid = receipt.clone();
        invalid.catalog_sha256 = "short".to_owned();
        assert_eq!(
            validation_error(&invalid),
            ReceiptError::InvalidHash("catalog_sha256")
        );

        for transaction_id in ["", "unsafe/id", "non-ascii-é"] {
            let mut invalid = receipt.clone();
            invalid.transaction_id = Some(transaction_id.to_owned());
            assert_eq!(
                validation_error(&invalid),
                ReceiptError::InvalidAsset {
                    source_id: "receipt".to_owned(),
                    detail: "transaction_id must be safe ASCII",
                }
            );
        }

        let mut valid = receipt.clone();
        valid.transaction_id = Some("TX_2026-07-22".to_owned());
        assert_eq!(valid.validate(), Ok(()));

        let mut duplicate = receipt.clone();
        duplicate.providers.push(duplicate.providers[0].clone());
        assert_eq!(
            validation_error(&duplicate),
            ReceiptError::DuplicateProvider(ProviderId::Claude)
        );

        let mut missing_root = receipt.clone();
        let claude = missing_root
            .providers
            .iter_mut()
            .find(|provider| provider.provider == ProviderId::Claude)
            .unwrap_or_else(|| panic!("missing Claude receipt"));
        claude.root = None;
        assert_eq!(
            validation_error(&missing_root),
            ReceiptError::MissingProviderRoot(ProviderId::Claude)
        );

        let mut invalid_provider_root = receipt;
        let claude = invalid_provider_root
            .providers
            .iter_mut()
            .find(|provider| provider.provider == ProviderId::Claude)
            .unwrap_or_else(|| panic!("missing Claude receipt"));
        claude
            .root
            .as_mut()
            .unwrap_or_else(|| panic!("missing Claude root"))
            .real = PathBuf::from("relative");
        assert!(matches!(
            validation_error(&invalid_provider_root),
            ReceiptError::InvalidPath {
                field: "claude",
                ..
            }
        ));
    }

    #[test]
    fn every_recorded_root_path_must_be_absolute_and_normalized() {
        let (_home, _roots, receipt) = receipt_fixture(&[]);
        type ReceiptMutation = (fn(&mut Receipt), &'static str);
        let mutations: [ReceiptMutation; 4] = [
            (
                |receipt| receipt.roots.home.lexical = PathBuf::from("relative"),
                "home",
            ),
            (
                |receipt| receipt.roots.home.real = PathBuf::from("/home/../other"),
                "home",
            ),
            (
                |receipt| receipt.roots.canonical.lexical = PathBuf::from("relative"),
                "canonical",
            ),
            (
                |receipt| receipt.roots.canonical.real = PathBuf::from("/root/../other"),
                "canonical",
            ),
        ];

        for (mutate, expected_field) in mutations {
            let mut invalid = receipt.clone();
            mutate(&mut invalid);
            assert!(matches!(
                validation_error(&invalid),
                ReceiptError::InvalidPath { field, .. } if field == expected_field
            ));
        }
    }

    #[test]
    fn valid_file_directory_and_symlink_shapes_share_one_receipt() {
        let (_home, roots, mut receipt) = receipt_fixture(&[ProviderId::Claude]);
        receipt.transaction_id = Some("recovery_1".to_owned());
        receipt.state = ReceiptState::RecoveryRequired;

        let file = file_asset(roots.canonical_skills.join("example/SKILL.md"));
        let directory = OwnedAsset {
            source_id: "skills/directory".to_owned(),
            destination: roots.canonical_skills.join("directory"),
            kind: OwnedAssetKind::Directory,
            hash: None,
            mode: Some(0o755),
            link_target: None,
            references: vec![ProviderId::Claude, ProviderId::Codex],
        };
        let claude_skills = roots
            .provider(ProviderId::Claude)
            .and_then(|provider| provider.skills.as_ref())
            .unwrap_or_else(|| panic!("missing Claude skills root"));
        let symlink_destination = claude_skills.join("linked");
        let symlink = symlink_asset(
            symlink_destination.clone(),
            Some(PathBuf::from("../../.agents/skills/linked")),
        );
        receipt.assets = vec![file, directory, symlink];

        assert_eq!(receipt.validate(), Ok(()));
        assert!(receipt.owned_asset(&symlink_destination).is_some());

        receipt.assets[2].link_target = Some(roots.canonical_skills.join("linked"));
        assert_eq!(receipt.validate(), Ok(()));

        let encoded = serde_json::to_vec(&receipt)
            .unwrap_or_else(|error| panic!("receipt encoding failed: {error}"));
        let decoded = Receipt::decode(&encoded)
            .unwrap_or_else(|error| panic!("receipt decoding failed: {error}"));
        assert_eq!(decoded.state, ReceiptState::RecoveryRequired);
        assert_eq!(decoded.assets.len(), 3);
    }

    #[test]
    fn malformed_asset_metadata_is_rejected_before_ownership() {
        let (_home, roots, receipt) = receipt_fixture(&[ProviderId::Claude]);
        let destination = roots.canonical_skills.join("example/SKILL.md");

        let mut asset = file_asset(destination.clone());
        asset.source_id.clear();
        assert_asset_error(
            &receipt,
            asset,
            ReceiptError::InvalidAsset {
                source_id: String::new(),
                detail: "source_id cannot be empty",
            },
        );

        let asset = file_asset(PathBuf::from("relative/SKILL.md"));
        assert_asset_error(
            &receipt,
            asset,
            ReceiptError::InvalidPath {
                field: "asset.destination",
                path: PathBuf::from("relative/SKILL.md"),
            },
        );

        let mut asset = file_asset(destination.clone());
        asset.mode = Some(0o1644);
        assert_asset_error(
            &receipt,
            asset,
            ReceiptError::InvalidAsset {
                source_id: "skills/example/SKILL.md".to_owned(),
                detail: "mode contains non-permission bits",
            },
        );

        let mut asset = file_asset(destination.clone());
        asset.hash = Some("A".repeat(64));
        assert_asset_error(
            &receipt,
            asset,
            ReceiptError::InvalidAsset {
                source_id: "skills/example/SKILL.md".to_owned(),
                detail: "hash must be a lowercase SHA-256",
            },
        );

        for kind in [
            OwnedAssetKind::File,
            OwnedAssetKind::Directory,
            OwnedAssetKind::Symlink,
        ] {
            let mut asset = file_asset(destination.clone());
            asset.kind = kind;
            asset.mode = None;
            assert_asset_error(
                &receipt,
                asset,
                ReceiptError::InvalidAsset {
                    source_id: "skills/example/SKILL.md".to_owned(),
                    detail: "hash, mode, and link_target do not match the asset kind",
                },
            );
        }

        let mut asset = file_asset(destination);
        asset.references = vec![ProviderId::Claude, ProviderId::Claude];
        assert_asset_error(
            &receipt,
            asset,
            ReceiptError::InvalidAsset {
                source_id: "skills/example/SKILL.md".to_owned(),
                detail: "provider references contain duplicates",
            },
        );
    }

    #[test]
    fn asset_destinations_and_symlink_targets_stay_inside_owned_roots() {
        let (_home, roots, receipt) = receipt_fixture(&[ProviderId::Claude]);
        let outside = file_asset(PathBuf::from("/outside/SKILL.md"));
        assert_asset_error(
            &receipt,
            outside,
            ReceiptError::InvalidAsset {
                source_id: "skills/example/SKILL.md".to_owned(),
                detail: "destination is outside every recorded root",
            },
        );

        let claude = roots
            .provider(ProviderId::Claude)
            .unwrap_or_else(|| panic!("missing Claude root"));
        let claude_skills = claude
            .skills
            .as_ref()
            .unwrap_or_else(|| panic!("missing Claude skills root"));

        let outside_skills = symlink_asset(
            claude.root.lexical.join("agents/linked"),
            Some(roots.canonical_skills.join("linked")),
        );
        assert_asset_error(
            &receipt,
            outside_skills,
            ReceiptError::InvalidAsset {
                source_id: "skills/example".to_owned(),
                detail: "owned symlink is outside the Claude skills root",
            },
        );

        let nested = symlink_asset(
            claude_skills.join("group/linked"),
            Some(roots.canonical_skills.join("group/linked")),
        );
        assert_asset_error(
            &receipt,
            nested,
            ReceiptError::InvalidAsset {
                source_id: "skills/example".to_owned(),
                detail: "Claude activation must be one symlink per skill",
            },
        );

        let escaping = symlink_asset(
            claude_skills.join("linked"),
            Some(PathBuf::from("/../../escape")),
        );
        assert_asset_error(
            &receipt,
            escaping,
            ReceiptError::InvalidAsset {
                source_id: "skills/example".to_owned(),
                detail: "owned symlink target escapes the filesystem root",
            },
        );

        let mismatched = symlink_asset(
            claude_skills.join("linked"),
            Some(PathBuf::from("/different")),
        );
        assert_asset_error(
            &receipt,
            mismatched,
            ReceiptError::InvalidAsset {
                source_id: "skills/example".to_owned(),
                detail: "owned symlink does not target its corresponding canonical skill",
            },
        );

        let missing_target = symlink_asset(claude_skills.join("linked"), None);
        assert_eq!(
            validate_asset_root(&missing_target, &receipt),
            Err(ReceiptError::InvalidAsset {
                source_id: "skills/example".to_owned(),
                detail: "owned symlink has no target",
            })
        );

        let (_other_home, other_roots, other_receipt) = receipt_fixture(&[]);
        let without_claude = symlink_asset(
            other_roots.canonical_skills.join("linked"),
            Some(other_roots.canonical_skills.join("linked")),
        );
        assert_asset_error(
            &other_receipt,
            without_claude,
            ReceiptError::InvalidAsset {
                source_id: "skills/example".to_owned(),
                detail: "symlink ownership requires a recorded Claude root",
            },
        );
    }

    #[test]
    fn path_normalization_handles_relative_parent_and_root_boundaries() {
        assert_eq!(normalize_path(Path::new("relative")), None);
        assert_eq!(
            normalize_path(Path::new("/one/two/../three")),
            Some(PathBuf::from("/one/three"))
        );
        assert_eq!(normalize_path(Path::new("/../../escape")), None);
        assert_eq!(normalize_path(Path::new("/")), Some(PathBuf::from("/")));
    }

    #[test]
    fn changed_lexical_real_or_device_identity_blocks_mutation() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Codex])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);

        for mutation in [
            |roots: &mut crate::provider::ResolvedRoots| roots.home.lexical.push("other"),
            |roots: &mut crate::provider::ResolvedRoots| roots.canonical.real.push("other"),
            |roots: &mut crate::provider::ResolvedRoots| roots.providers[0].root.device += 1,
        ] {
            let mut changed = roots.clone();
            mutation(&mut changed);
            assert!(matches!(
                receipt.validate_roots(&changed),
                Err(ReceiptError::RootMismatch(_))
            ));
        }
    }

    #[test]
    fn changed_unselected_provider_root_does_not_block_unrelated_mutation() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let both = resolve_roots_from(Some(home.path().as_os_str()), None, &ProviderId::ALL)
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let receipt = Receipt::new("0.1.0", "a".repeat(64), &both);
        let claude_only = resolve_roots_from(
            Some(home.path().as_os_str()),
            Some(OsStr::new("relative-but-ignored")),
            &[ProviderId::Claude],
        )
        .unwrap_or_else(|error| panic!("Claude root resolution failed: {error}"));
        assert_eq!(receipt.validate_roots(&claude_only), Ok(()));
    }

    #[test]
    fn duplicate_destination_is_not_accepted_as_ownership_proof() {
        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        let asset = OwnedAsset {
            source_id: "skill/file".to_owned(),
            destination: home.path().join(".agents/skills/skill/file"),
            kind: OwnedAssetKind::File,
            hash: Some("b".repeat(64)),
            mode: Some(0o644),
            link_target: None,
            references: Vec::new(),
        };
        receipt.assets = vec![asset.clone(), asset];
        assert!(matches!(
            receipt.validate(),
            Err(ReceiptError::DuplicateDestination(_))
        ));
        assert!(matches!(
            receipt.owned_by_destination(),
            Err(ReceiptError::DuplicateDestination(_))
        ));
    }

    #[test]
    fn a_current_provider_absent_from_the_receipt_does_not_create_a_false_mismatch() {
        let (_home, _stored_roots, receipt) = receipt_fixture(&[]);
        let current_home =
            tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let current = resolve_roots_from(
            Some(current_home.path().as_os_str()),
            None,
            &[ProviderId::Codex],
        )
        .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let mut same_environment = current;
        same_environment.home = receipt.roots.home.clone();
        same_environment.canonical = receipt.roots.canonical.clone();

        assert_eq!(receipt.validate_roots(&same_environment), Ok(()));
    }

    #[test]
    fn root_scope_and_receipt_error_messages_have_stable_exit_codes() {
        assert_eq!(RootScope::Home.to_string(), "HOME");
        assert_eq!(RootScope::Canonical.to_string(), "canonical root");
        assert_eq!(
            RootScope::Provider(ProviderId::Claude).to_string(),
            "claude root"
        );

        let errors = [
            (
                ReceiptError::InvalidJson("syntax".to_owned()),
                "receipt JSON is invalid: syntax",
                INTEGRITY_EXIT_CODE,
            ),
            (
                ReceiptError::UnsupportedSchema { observed: 9 },
                "unsupported receipt schema 9; expected 1",
                INTEGRITY_EXIT_CODE,
            ),
            (
                ReceiptError::MissingField("cli_version"),
                "receipt field cli_version cannot be empty",
                INTEGRITY_EXIT_CODE,
            ),
            (
                ReceiptError::InvalidHash("catalog_sha256"),
                "receipt field catalog_sha256 must be a lowercase SHA-256",
                INTEGRITY_EXIT_CODE,
            ),
            (
                ReceiptError::DuplicateProvider(ProviderId::Claude),
                "receipt contains provider claude more than once",
                INTEGRITY_EXIT_CODE,
            ),
            (
                ReceiptError::MissingProviderRoot(ProviderId::Codex),
                "managed provider codex has no recorded root identity",
                INTEGRITY_EXIT_CODE,
            ),
            (
                ReceiptError::DuplicateDestination(PathBuf::from("/owned")),
                "receipt claims destination more than once: /owned",
                INTEGRITY_EXIT_CODE,
            ),
            (
                ReceiptError::InvalidPath {
                    field: "home",
                    path: PathBuf::from("relative"),
                },
                "receipt home is not a safe UTF-8 path: relative",
                ENVIRONMENT_EXIT_CODE,
            ),
            (
                ReceiptError::InvalidAsset {
                    source_id: "skill".to_owned(),
                    detail: "bad metadata",
                },
                "receipt asset skill is invalid: bad metadata",
                INTEGRITY_EXIT_CODE,
            ),
            (
                ReceiptError::RootMismatch(RootMismatch {
                    root: RootScope::Canonical,
                    recorded: identity("/recorded", 1),
                    current: identity("/current", 2),
                }),
                "canonical root identity differs from the receipt; restore the original environment before migration",
                ENVIRONMENT_EXIT_CODE,
            ),
        ];

        for (error, expected_message, expected_code) in errors {
            assert_eq!(error.to_string(), expected_message);
            assert_eq!(error.exit_code(), expected_code);
        }
    }

    #[cfg(unix)]
    #[test]
    fn programmatic_non_utf8_receipt_path_is_rejected() {
        use std::os::unix::ffi::OsStringExt;

        let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[])
            .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        receipt.roots.home.lexical = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff]));
        assert!(matches!(
            receipt.validate(),
            Err(ReceiptError::InvalidPath { .. })
        ));

        let claude_roots =
            resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Claude])
                .unwrap_or_else(|error| panic!("root resolution failed: {error}"));
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &claude_roots);
        let destination = claude_roots
            .provider(ProviderId::Claude)
            .and_then(|provider| provider.skills.as_ref())
            .unwrap_or_else(|| panic!("missing Claude skills root"))
            .join("linked");
        receipt.assets.push(symlink_asset(
            destination,
            Some(PathBuf::from(std::ffi::OsString::from_vec(vec![0xff]))),
        ));
        assert!(matches!(
            receipt.validate(),
            Err(ReceiptError::InvalidPath {
                field: "asset.link_target",
                ..
            })
        ));
    }
}
