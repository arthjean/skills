use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use serde::de::{DeserializeOwned, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

const LEGACY_LOCK_VERSION: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EntryType {
    File,
    Directory,
    Symlink,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogEntry {
    pub source_id: String,
    pub destination: PathBuf,
    pub entry_type: EntryType,
    pub sha256: Option<String>,
    pub mode: u32,
    pub link_target: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdoptedEntry {
    pub source_id: String,
    pub destination: PathBuf,
    pub entry_type: EntryType,
    pub hash: String,
    pub mode: u32,
    pub link_target: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockIdentity {
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub mtime_seconds: i64,
    pub mtime_nanoseconds: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevalidationToken {
    pub identity: LockIdentity,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    EmptyCatalog,
    InvalidCatalogEntry,
    DuplicateDestination,
    MissingLegacyEntry,
    DestinationMissing,
    UnsupportedFileType,
    TypeMismatch,
    ModeMismatch,
    HashMismatch,
    LinkTargetMismatch,
    UnexpectedDirectoryEntry,
    ArchiveAlreadyExists,
    ConcurrentAssetChange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdoptionDiagnostic {
    pub code: DiagnosticCode,
    pub source_id: Option<String>,
    pub destination: Option<PathBuf>,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdoptionPlan {
    pub entries: Vec<AdoptedEntry>,
    pub original_identity: LockIdentity,
    pub original_bytes: Vec<u8>,
    pub original_hash: String,
    pub archive_path: PathBuf,
    pub residual_bytes: Vec<u8>,
    pub applicable: bool,
    pub diagnostics: Vec<AdoptionDiagnostic>,
}

impl AdoptionPlan {
    #[must_use]
    pub fn revalidation_token(&self) -> RevalidationToken {
        RevalidationToken {
            identity: self.original_identity.clone(),
            sha256: self.original_hash.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdoptionDecision {
    Apply,
    DryRun,
    Declined,
    Blocked,
}

/// Purely decides whether a previously inspected plan may reach the transaction layer.
#[must_use]
pub const fn decision(
    plan: &AdoptionPlan,
    dry_run: bool,
    confirmation_granted: bool,
) -> AdoptionDecision {
    if !plan.applicable {
        AdoptionDecision::Blocked
    } else if dry_run {
        AdoptionDecision::DryRun
    } else if !confirmation_granted {
        AdoptionDecision::Declined
    } else {
        AdoptionDecision::Apply
    }
}

#[derive(Debug)]
pub enum AdoptionError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    InvalidPath {
        role: &'static str,
        bytes_hex: String,
    },
    UnsafePath {
        role: &'static str,
        path: PathBuf,
    },
    InvalidLegacyLock(String),
    LegacyLockChanged {
        expected: Box<RevalidationToken>,
        actual: Box<RevalidationToken>,
    },
}

impl fmt::Display for AdoptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} {}: {source}", path.display()),
            Self::InvalidPath { role, bytes_hex } => {
                write!(formatter, "{role} is not UTF-8 (hex: {bytes_hex})")
            }
            Self::UnsafePath { role, path } => {
                write!(
                    formatter,
                    "{role} must be absolute without traversal: {}",
                    path.display()
                )
            }
            Self::InvalidLegacyLock(detail) => {
                write!(formatter, "unsupported .skill-lock.json v3: {detail}")
            }
            Self::LegacyLockChanged { .. } => formatter.write_str(
                "legacy lock changed during adoption; stop other skill managers and retry",
            ),
        }
    }
}

impl std::error::Error for AdoptionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Inspects a Vercel Skills v3 lock and every selected filesystem entry.
///
/// This function is read-only. A failed entry clears the returned ownership entries,
/// making partial adoption impossible even if a caller mishandles the diagnostics.
pub fn inspect(
    lock_path: &Path,
    archive_path: &Path,
    catalog: &[CatalogEntry],
) -> Result<AdoptionPlan, AdoptionError> {
    require_safe_absolute(lock_path, "legacy lock path")?;
    require_safe_absolute(archive_path, "legacy lock archive path")?;

    let snapshot = read_stable_regular_file(lock_path, "read legacy lock")?;
    let mut legacy = parse_legacy_lock(&snapshot.bytes)?;
    let mut diagnostics = Vec::new();
    let mut adopted = Vec::new();

    if catalog.is_empty() {
        diagnostics.push(diagnostic(
            DiagnosticCode::EmptyCatalog,
            None,
            None,
            "no catalog entries were selected for adoption",
        ));
    }

    match fs::symlink_metadata(archive_path) {
        Ok(_) => diagnostics.push(diagnostic(
            DiagnosticCode::ArchiveAlreadyExists,
            None,
            Some(archive_path),
            "the exact legacy-lock archive destination already exists",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(AdoptionError::Io {
                operation: "inspect legacy lock archive destination",
                path: archive_path.to_path_buf(),
                source,
            });
        }
    }

    let mut ordered = catalog.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.source_id
            .cmp(&right.source_id)
            .then_with(|| left.destination.cmp(&right.destination))
    });

    let mut destinations = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    for expected in &ordered {
        source_ids.insert(expected.source_id.as_str());
        validate_catalog_entry(expected, &mut diagnostics);
        if !destinations.insert(expected.destination.clone()) {
            diagnostics.push(diagnostic(
                DiagnosticCode::DuplicateDestination,
                Some(&expected.source_id),
                Some(&expected.destination),
                "the catalog contains the destination more than once",
            ));
        }
    }

    for source_id in &source_ids {
        if !legacy.skills.0.contains_key(*source_id) {
            diagnostics.push(diagnostic(
                DiagnosticCode::MissingLegacyEntry,
                Some(source_id),
                None,
                "the v3 lock does not own this catalog source entry",
            ));
        }
    }

    for expected in ordered {
        match inspect_entry(expected, &destinations) {
            Ok(entry) => adopted.push(entry),
            Err(mut entry_diagnostics) => diagnostics.append(&mut entry_diagnostics),
        }
    }

    for source_id in source_ids {
        legacy.skills.0.remove(source_id);
    }
    let residual_bytes = serde_json::to_vec_pretty(&legacy).map_err(|error| {
        AdoptionError::InvalidLegacyLock(format!("cannot serialize residual lock: {error}"))
    })?;

    let applicable = diagnostics.is_empty();
    if !applicable {
        adopted.clear();
    }

    Ok(AdoptionPlan {
        entries: adopted,
        original_identity: snapshot.identity,
        original_bytes: snapshot.bytes,
        original_hash: snapshot.sha256,
        archive_path: archive_path.to_path_buf(),
        residual_bytes,
        applicable,
        diagnostics,
    })
}

/// Revalidates the inode, size, mtime and bytes immediately before lock replacement.
///
/// This detects observable interference but is not a compare-and-swap against a
/// non-cooperating Vercel Skills process. A writer can still race after this call.
pub fn revalidate(lock_path: &Path, token: &RevalidationToken) -> Result<(), AdoptionError> {
    require_safe_absolute(lock_path, "legacy lock path")?;
    let snapshot = read_stable_regular_file(lock_path, "revalidate legacy lock")?;
    let actual = RevalidationToken {
        identity: snapshot.identity,
        sha256: snapshot.sha256,
    };
    if &actual == token {
        Ok(())
    } else {
        Err(AdoptionError::LegacyLockChanged {
            expected: Box::new(token.clone()),
            actual: Box::new(actual),
        })
    }
}

fn validate_catalog_entry(entry: &CatalogEntry, diagnostics: &mut Vec<AdoptionDiagnostic>) {
    if entry.source_id.is_empty() {
        diagnostics.push(diagnostic(
            DiagnosticCode::InvalidCatalogEntry,
            None,
            Some(&entry.destination),
            "source_id must not be empty",
        ));
    }
    if entry.destination.as_os_str().as_bytes().is_empty()
        || !entry.destination.is_absolute()
        || entry.destination.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        diagnostics.push(diagnostic(
            DiagnosticCode::InvalidCatalogEntry,
            Some(&entry.source_id),
            Some(&entry.destination),
            "destination must be an absolute path",
        ));
    }
    if entry.destination.to_str().is_none() {
        diagnostics.push(diagnostic(
            DiagnosticCode::InvalidCatalogEntry,
            Some(&entry.source_id),
            Some(&entry.destination),
            &format!(
                "destination is not UTF-8 (hex: {})",
                hex(entry.destination.as_os_str().as_bytes())
            ),
        ));
    }
    if entry.mode > 0o777 {
        diagnostics.push(diagnostic(
            DiagnosticCode::InvalidCatalogEntry,
            Some(&entry.source_id),
            Some(&entry.destination),
            "mode must contain only POSIX permission bits",
        ));
    }
    match entry.entry_type {
        EntryType::File
            if entry
                .sha256
                .as_deref()
                .is_none_or(|hash| !valid_sha256(hash)) =>
        {
            diagnostics.push(diagnostic(
                DiagnosticCode::InvalidCatalogEntry,
                Some(&entry.source_id),
                Some(&entry.destination),
                "a file requires a 64-character SHA-256",
            ));
        }
        EntryType::Symlink if entry.link_target.is_none() => diagnostics.push(diagnostic(
            DiagnosticCode::InvalidCatalogEntry,
            Some(&entry.source_id),
            Some(&entry.destination),
            "a symlink requires an exact link target",
        )),
        EntryType::File | EntryType::Directory if entry.link_target.is_some() => {
            diagnostics.push(diagnostic(
                DiagnosticCode::InvalidCatalogEntry,
                Some(&entry.source_id),
                Some(&entry.destination),
                "only symlinks may declare a link target",
            ));
        }
        _ => {}
    }
    if let Some(hash) = &entry.sha256
        && !valid_sha256(hash)
    {
        diagnostics.push(diagnostic(
            DiagnosticCode::InvalidCatalogEntry,
            Some(&entry.source_id),
            Some(&entry.destination),
            "sha256 must contain exactly 64 hexadecimal characters",
        ));
    }
    if let Some(target) = &entry.link_target
        && target.to_str().is_none()
    {
        diagnostics.push(diagnostic(
            DiagnosticCode::InvalidCatalogEntry,
            Some(&entry.source_id),
            Some(&entry.destination),
            &format!(
                "link target is not UTF-8 (hex: {})",
                hex(target.as_os_str().as_bytes())
            ),
        ));
    }
}

fn inspect_entry(
    expected: &CatalogEntry,
    all_destinations: &BTreeSet<PathBuf>,
) -> Result<AdoptedEntry, Vec<AdoptionDiagnostic>> {
    let mut diagnostics = Vec::new();
    let metadata = match fs::symlink_metadata(&expected.destination) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            diagnostics.push(diagnostic(
                DiagnosticCode::DestinationMissing,
                Some(&expected.source_id),
                Some(&expected.destination),
                "catalog destination does not exist",
            ));
            return Err(diagnostics);
        }
        Err(error) => {
            diagnostics.push(diagnostic(
                DiagnosticCode::ConcurrentAssetChange,
                Some(&expected.source_id),
                Some(&expected.destination),
                &format!("cannot inspect destination: {error}"),
            ));
            return Err(diagnostics);
        }
    };

    let actual_type = entry_type(&metadata);
    let Some(actual_type) = actual_type else {
        diagnostics.push(diagnostic(
            DiagnosticCode::UnsupportedFileType,
            Some(&expected.source_id),
            Some(&expected.destination),
            "destination is not a regular file, directory or symlink",
        ));
        return Err(diagnostics);
    };
    if actual_type != expected.entry_type {
        diagnostics.push(diagnostic(
            DiagnosticCode::TypeMismatch,
            Some(&expected.source_id),
            Some(&expected.destination),
            &format!(
                "expected {:?}, found {:?}",
                expected.entry_type, actual_type
            ),
        ));
        return Err(diagnostics);
    }

    let actual_mode = metadata.mode() & 0o7777;
    if actual_mode != expected.mode {
        diagnostics.push(diagnostic(
            DiagnosticCode::ModeMismatch,
            Some(&expected.source_id),
            Some(&expected.destination),
            &format!("expected {:04o}, found {actual_mode:04o}", expected.mode),
        ));
    }

    let (actual_hash, actual_target) = match fingerprint(&expected.destination, actual_type) {
        Ok(value) => value,
        Err(detail) => {
            diagnostics.push(diagnostic(
                DiagnosticCode::ConcurrentAssetChange,
                Some(&expected.source_id),
                Some(&expected.destination),
                &detail,
            ));
            return Err(diagnostics);
        }
    };

    if let Some(expected_hash) = &expected.sha256
        && !actual_hash.eq_ignore_ascii_case(expected_hash)
    {
        diagnostics.push(diagnostic(
            DiagnosticCode::HashMismatch,
            Some(&expected.source_id),
            Some(&expected.destination),
            &format!("expected {expected_hash}, found {actual_hash}"),
        ));
    }
    if actual_target != expected.link_target {
        diagnostics.push(diagnostic(
            DiagnosticCode::LinkTargetMismatch,
            Some(&expected.source_id),
            Some(&expected.destination),
            &format!(
                "expected {:?}, found {:?}",
                expected.link_target, actual_target
            ),
        ));
    }

    if actual_type == EntryType::Directory {
        let has_explicit_descendants = all_destinations
            .iter()
            .any(|path| path != &expected.destination && path.starts_with(&expected.destination));
        if has_explicit_descendants || expected.sha256.is_none() {
            match fs::read_dir(&expected.destination) {
                Ok(children) => {
                    for child in children {
                        match child {
                            Ok(child) if all_destinations.contains(&child.path()) => {}
                            Ok(child) => diagnostics.push(diagnostic(
                                DiagnosticCode::UnexpectedDirectoryEntry,
                                Some(&expected.source_id),
                                Some(&child.path()),
                                "entry is absent from the catalog directory tree",
                            )),
                            Err(error) => diagnostics.push(diagnostic(
                                DiagnosticCode::ConcurrentAssetChange,
                                Some(&expected.source_id),
                                Some(&expected.destination),
                                &format!("cannot enumerate directory: {error}"),
                            )),
                        }
                    }
                }
                Err(error) => diagnostics.push(diagnostic(
                    DiagnosticCode::ConcurrentAssetChange,
                    Some(&expected.source_id),
                    Some(&expected.destination),
                    &format!("cannot enumerate directory: {error}"),
                )),
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(AdoptedEntry {
            source_id: expected.source_id.clone(),
            destination: expected.destination.clone(),
            entry_type: actual_type,
            hash: actual_hash,
            mode: actual_mode,
            link_target: actual_target,
        })
    } else {
        Err(diagnostics)
    }
}

fn fingerprint(path: &Path, entry_type: EntryType) -> Result<(String, Option<PathBuf>), String> {
    match entry_type {
        EntryType::File => read_stable_regular_file(path, "hash catalog file")
            .map(|snapshot| (snapshot.sha256, None))
            .map_err(|error| error.to_string()),
        EntryType::Symlink => {
            let target = fs::read_link(path)
                .map_err(|error| format!("cannot read symlink target: {error}"))?;
            if target.to_str().is_none() {
                return Err(format!(
                    "symlink target is not UTF-8 (hex: {})",
                    hex(target.as_os_str().as_bytes())
                ));
            }
            let hash = format!("{:x}", Sha256::digest(target.as_os_str().as_bytes()));
            Ok((hash, Some(target)))
        }
        EntryType::Directory => hash_directory(path).map(|hash| (hash, None)),
    }
}

/// Directory hashes cover every descendant path, type, mode, content and link target.
/// The root name and root mode are checked separately and are not part of the hash.
fn hash_directory(root: &Path) -> Result<String, String> {
    let mut records = Vec::new();
    collect_directory_records(root, root, &mut records)?;
    records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut digest = Sha256::new();
    for record in records {
        digest.update([match record.entry_type {
            EntryType::File => b'f',
            EntryType::Directory => b'd',
            EntryType::Symlink => b'l',
        }]);
        digest.update(record.relative_path.as_bytes());
        digest.update([0]);
        digest.update(record.mode.to_le_bytes());
        digest.update(record.hash.as_bytes());
        digest.update([0]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

struct DirectoryRecord {
    relative_path: String,
    entry_type: EntryType,
    mode: u32,
    hash: String,
}

fn collect_directory_records(
    root: &Path,
    directory: &Path,
    records: &mut Vec<DirectoryRecord>,
) -> Result<(), String> {
    let children = fs::read_dir(directory)
        .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?;
    for child in children {
        let child = child.map_err(|error| format!("cannot enumerate directory entry: {error}"))?;
        let path = child.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("cannot derive directory-relative path: {error}"))?;
        let relative = relative.to_str().ok_or_else(|| {
            format!(
                "catalog path is not UTF-8 (hex: {})",
                hex(relative.as_os_str().as_bytes())
            )
        })?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        let kind = entry_type(&metadata)
            .ok_or_else(|| format!("{} has an unsupported filesystem type", path.display()))?;
        let (hash, _) = fingerprint(&path, kind)?;
        records.push(DirectoryRecord {
            relative_path: relative.to_owned(),
            entry_type: kind,
            mode: metadata.mode() & 0o7777,
            hash,
        });
        if kind == EntryType::Directory {
            collect_directory_records(root, &path, records)?;
        }
    }
    Ok(())
}

fn entry_type(metadata: &Metadata) -> Option<EntryType> {
    let file_type = metadata.file_type();
    if file_type.is_file() {
        Some(EntryType::File)
    } else if file_type.is_dir() {
        Some(EntryType::Directory)
    } else if file_type.is_symlink() {
        Some(EntryType::Symlink)
    } else {
        None
    }
}

struct StableFile {
    identity: LockIdentity,
    bytes: Vec<u8>,
    sha256: String,
}

fn read_stable_regular_file(
    path: &Path,
    operation: &'static str,
) -> Result<StableFile, AdoptionError> {
    let before = fs::symlink_metadata(path).map_err(|source| AdoptionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })?;
    if !before.file_type().is_file() {
        return Err(AdoptionError::InvalidLegacyLock(format!(
            "{} must be a regular file and must not be a symlink",
            path.display()
        )));
    }
    let mut file = File::open(path).map_err(|source| AdoptionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })?;
    let opened = file.metadata().map_err(|source| AdoptionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })?;
    if file_identity(&before) != file_identity(&opened) {
        return Err(concurrent_read_error(path));
    }

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| AdoptionError::Io {
            operation,
            path: path.to_path_buf(),
            source,
        })?;
    let after_handle = file.metadata().map_err(|source| AdoptionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })?;
    let after_path = fs::symlink_metadata(path).map_err(|source| AdoptionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })?;
    let identity = file_identity(&before);
    if identity != file_identity(&after_handle)
        || identity != file_identity(&after_path)
        || u64::try_from(bytes.len()).ok() != Some(identity.size)
    {
        return Err(concurrent_read_error(path));
    }

    Ok(StableFile {
        identity,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes,
    })
}

fn concurrent_read_error(path: &Path) -> AdoptionError {
    AdoptionError::InvalidLegacyLock(format!(
        "{} changed while it was being inspected",
        path.display()
    ))
}

fn file_identity(metadata: &Metadata) -> LockIdentity {
    LockIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.size(),
        mtime_seconds: metadata.mtime(),
        mtime_nanoseconds: metadata.mtime_nsec(),
    }
}

fn require_utf8(path: &Path, role: &'static str) -> Result<(), AdoptionError> {
    if path.to_str().is_some() {
        Ok(())
    } else {
        Err(AdoptionError::InvalidPath {
            role,
            bytes_hex: hex(path.as_os_str().as_bytes()),
        })
    }
}

fn require_safe_absolute(path: &Path, role: &'static str) -> Result<(), AdoptionError> {
    require_utf8(path, role)?;
    if path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        Ok(())
    } else {
        Err(AdoptionError::UnsafePath {
            role,
            path: path.to_path_buf(),
        })
    }
}

fn diagnostic(
    code: DiagnosticCode,
    source_id: Option<&str>,
    destination: Option<&Path>,
    detail: &str,
) -> AdoptionDiagnostic {
    AdoptionDiagnostic {
        code,
        source_id: source_id.map(str::to_owned),
        destination: destination.map(Path::to_path_buf),
        detail: detail.to_owned(),
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacyLockV3 {
    version: u8,
    skills: StrictMap<LegacySkillEntry>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    dismissed: Option<StrictMap<bool>>,
    #[serde(
        rename = "lastSelectedAgents",
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    last_selected_agents: Option<Vec<String>>,
}

impl<'de> Deserialize<'de> for LegacyLockV3 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            version: u8,
            skills: StrictMap<LegacySkillEntry>,
            #[serde(default, deserialize_with = "deserialize_present")]
            dismissed: Option<StrictMap<bool>>,
            #[serde(
                rename = "lastSelectedAgents",
                default,
                deserialize_with = "deserialize_present"
            )]
            last_selected_agents: Option<Vec<String>>,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            version: wire.version,
            skills: wire.skills,
            dismissed: wire.dismissed,
            last_selected_agents: wire.last_selected_agents,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacySkillEntry {
    source: String,
    #[serde(rename = "sourceType")]
    source_type: String,
    #[serde(
        rename = "sourceUrl",
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    source_url: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    r#ref: Option<String>,
    #[serde(
        rename = "skillPath",
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    skill_path: Option<String>,
    #[serde(rename = "skillFolderHash")]
    skill_folder_hash: String,
    #[serde(
        rename = "pluginName",
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    plugin_name: Option<String>,
    #[serde(rename = "installedAt")]
    installed_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

fn parse_legacy_lock(bytes: &[u8]) -> Result<LegacyLockV3, AdoptionError> {
    let lock: LegacyLockV3 = serde_json::from_slice(bytes)
        .map_err(|error| AdoptionError::InvalidLegacyLock(error.to_string()))?;
    if lock.version != LEGACY_LOCK_VERSION {
        return Err(AdoptionError::InvalidLegacyLock(format!(
            "expected version {LEGACY_LOCK_VERSION}, found {}",
            lock.version
        )));
    }
    for (name, entry) in &lock.skills.0 {
        if name.is_empty()
            || entry.source.is_empty()
            || entry.source_type.is_empty()
            || entry.installed_at.is_empty()
            || entry.updated_at.is_empty()
        {
            return Err(AdoptionError::InvalidLegacyLock(format!(
                "skill entry {name:?} contains an empty required field"
            )));
        }
    }
    Ok(lock)
}

fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug)]
struct StrictMap<T>(BTreeMap<String, T>);

impl<T> Serialize for StrictMap<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for StrictMap<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrictMapVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> Visitor<'de> for StrictMapVisitor<T>
        where
            T: DeserializeOwned,
        {
            type Value = StrictMap<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object without duplicate keys")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = access.next_entry::<String, T>()? {
                    if values.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!("duplicate key {key:?}")));
                    }
                }
                Ok(StrictMap(values))
            }
        }

        deserializer.deserialize_map(StrictMapVisitor(std::marker::PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    use tempfile::TempDir;

    use super::*;

    const HASH_A: &str = "559aead08264d5795d3909718cdd05abd49572e84fe55590eef31a88a08fdffd";

    fn lock(skills: &str) -> String {
        format!(
            r#"{{
  "version": 3,
  "skills": {skills},
  "dismissed": {{"findSkillsPrompt": true}},
  "lastSelectedAgents": ["codex", "claude-code"]
}}"#
        )
    }

    fn skill(source: &str) -> String {
        format!(
            r#"{{"source":"{source}","sourceType":"github","sourceUrl":"https://example.test/{source}.git","skillPath":"skills/{source}/SKILL.md","skillFolderHash":"0123456789012345678901234567890123456789","installedAt":"2026-01-01T00:00:00.000Z","updatedAt":"2026-01-01T00:00:00.000Z"}}"#
        )
    }

    fn minimal_skill(source: &str) -> String {
        format!(
            r#"{{"source":"{source}","sourceType":"github","skillFolderHash":"0123456789012345678901234567890123456789","installedAt":"2026-01-01T00:00:00.000Z","updatedAt":"2026-01-01T00:00:00.000Z"}}"#
        )
    }

    fn write_fixture_lock(temp: &TempDir, skills: &str) -> (PathBuf, PathBuf) {
        let lock_path = temp.path().join(".skill-lock.json");
        let archive_path = temp.path().join("legacy-lock.archive.json");
        assert!(fs::write(&lock_path, lock(skills)).is_ok());
        (lock_path, archive_path)
    }

    fn inspect_or_panic(
        lock_path: &Path,
        archive_path: &Path,
        catalog: &[CatalogEntry],
    ) -> AdoptionPlan {
        match inspect(lock_path, archive_path, catalog) {
            Ok(plan) => plan,
            Err(error) => panic!("inspection failed: {error}"),
        }
    }

    fn filesystem_mode(path: &Path) -> u32 {
        match fs::symlink_metadata(path) {
            Ok(metadata) => metadata.mode() & 0o7777,
            Err(error) => panic!("cannot inspect fixture mode: {error}"),
        }
    }

    fn diagnostic_count(plan: &AdoptionPlan, code: DiagnosticCode) -> usize {
        plan.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == code)
            .count()
    }

    fn invalid_lock_detail(bytes: &[u8]) -> String {
        match parse_legacy_lock(bytes) {
            Err(AdoptionError::InvalidLegacyLock(detail)) => detail,
            Err(error) => panic!("unexpected parser error: {error}"),
            Ok(_) => panic!("invalid legacy lock was accepted"),
        }
    }

    #[test]
    fn adoption_is_all_or_nothing_and_residual_preserves_foreign_entries() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let lock_path = temp.path().join(".skill-lock.json");
        let archive_path = temp.path().join("legacy-lock.archive.json");
        let managed = temp.path().join("managed");
        let foreign = skill("foreign");
        let original = lock(&format!(
            "{{\"managed\":{},\"foreign\":{foreign}}}",
            skill("managed")
        ));
        assert!(fs::write(&lock_path, &original).is_ok());
        assert!(fs::write(&managed, b"A").is_ok());
        assert!(fs::set_permissions(&managed, fs::Permissions::from_mode(0o644)).is_ok());

        let catalog = [CatalogEntry {
            source_id: "managed".to_owned(),
            destination: managed.clone(),
            entry_type: EntryType::File,
            sha256: Some(HASH_A.to_owned()),
            mode: 0o644,
            link_target: None,
        }];
        let plan = match inspect(&lock_path, &archive_path, &catalog) {
            Ok(plan) => plan,
            Err(error) => panic!("inspection failed: {error}"),
        };
        assert!(plan.applicable);
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.original_bytes, original.as_bytes());
        let residual: serde_json::Value = match serde_json::from_slice(&plan.residual_bytes) {
            Ok(value) => value,
            Err(error) => panic!("invalid residual: {error}"),
        };
        assert!(residual["skills"].get("managed").is_none());
        assert_eq!(residual["skills"]["foreign"]["source"], "foreign");
        assert_eq!(decision(&plan, true, true), AdoptionDecision::DryRun);
        assert_eq!(decision(&plan, false, false), AdoptionDecision::Declined);
        assert_eq!(decision(&plan, false, true), AdoptionDecision::Apply);
        assert_eq!(
            fs::read(&lock_path).ok().as_deref(),
            Some(original.as_bytes())
        );

        let mismatched = [CatalogEntry {
            sha256: Some("0".repeat(64)),
            ..catalog[0].clone()
        }];
        let blocked = match inspect(&lock_path, &archive_path, &mismatched) {
            Ok(plan) => plan,
            Err(error) => panic!("inspection failed: {error}"),
        };
        assert!(!blocked.applicable);
        assert!(blocked.entries.is_empty());
        assert_eq!(decision(&blocked, false, true), AdoptionDecision::Blocked);
        assert_eq!(decision(&blocked, true, true), AdoptionDecision::Blocked);
    }

    #[test]
    fn strict_v3_parser_rejects_unknown_keys_and_duplicates() {
        let unknown = lock(&format!(
            "{{\"managed\":{{{},\"future\":true}}}}",
            &skill("managed")[1..skill("managed").len() - 1]
        ));
        let error = parse_legacy_lock(unknown.as_bytes());
        assert!(matches!(error, Err(AdoptionError::InvalidLegacyLock(_))));

        let duplicate = lock(&format!(
            "{{\"managed\":{},\"managed\":{}}}",
            skill("managed"),
            skill("managed")
        ));
        let error = parse_legacy_lock(duplicate.as_bytes());
        assert!(matches!(error, Err(AdoptionError::InvalidLegacyLock(_))));
    }

    #[test]
    fn verifies_symlink_target_and_revalidates_the_original_lock() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let lock_path = temp.path().join(".skill-lock.json");
        let archive_path = temp.path().join("legacy-lock.archive.json");
        let target = PathBuf::from("../../.agents/skills/managed");
        let link = temp.path().join("managed-link");
        assert!(
            fs::write(
                &lock_path,
                lock(&format!("{{\"managed\":{}}}", skill("managed")))
            )
            .is_ok()
        );
        assert!(symlink(&target, &link).is_ok());
        let metadata = match fs::symlink_metadata(&link) {
            Ok(metadata) => metadata,
            Err(error) => panic!("cannot inspect symlink: {error}"),
        };
        let catalog = [CatalogEntry {
            source_id: "managed".to_owned(),
            destination: link,
            entry_type: EntryType::Symlink,
            sha256: None,
            mode: metadata.mode() & 0o777,
            link_target: Some(target),
        }];
        let plan = match inspect(&lock_path, &archive_path, &catalog) {
            Ok(plan) => plan,
            Err(error) => panic!("inspection failed: {error}"),
        };
        assert!(plan.applicable);
        assert!(revalidate(&lock_path, &plan.revalidation_token()).is_ok());
        assert!(fs::write(&lock_path, b"{}").is_ok());
        assert!(matches!(
            revalidate(&lock_path, &plan.revalidation_token()),
            Err(AdoptionError::LegacyLockChanged { .. })
        ));
    }

    #[test]
    fn traversal_paths_are_rejected_before_inspection() {
        let error = inspect(
            Path::new("relative/.skill-lock.json"),
            Path::new("relative/archive.json"),
            &[],
        );
        assert!(matches!(error, Err(AdoptionError::UnsafePath { .. })));
    }

    #[test]
    fn parser_rejects_malformed_versions_shapes_nulls_and_empty_required_fields() {
        assert!(!invalid_lock_detail(b"{").is_empty());

        let wrong_version = lock("{}").replacen("\"version\": 3", "\"version\": 4", 1);
        assert_eq!(
            invalid_lock_detail(wrong_version.as_bytes()),
            "expected version 3, found 4"
        );
        assert!(!invalid_lock_detail(br#"{"version":3,"skills":[]}"#).is_empty());

        let base = skill("managed");
        let empty_entries = [
            base.replace("\"source\":\"managed\"", "\"source\":\"\""),
            base.replace("\"sourceType\":\"github\"", "\"sourceType\":\"\""),
            base.replace(
                "\"installedAt\":\"2026-01-01T00:00:00.000Z\"",
                "\"installedAt\":\"\"",
            ),
            base.replace(
                "\"updatedAt\":\"2026-01-01T00:00:00.000Z\"",
                "\"updatedAt\":\"\"",
            ),
        ];
        for entry in empty_entries {
            let candidate = lock(&format!("{{\"managed\":{entry}}}"));
            assert!(invalid_lock_detail(candidate.as_bytes()).contains("empty required field"));
        }
        let empty_name = lock(&format!("{{\"\":{}}}", skill("managed")));
        assert!(invalid_lock_detail(empty_name.as_bytes()).contains("empty required field"));

        let null_source_url = lock(&format!(
            "{{\"managed\":{}}}",
            base.replace(
                "\"sourceUrl\":\"https://example.test/managed.git\"",
                "\"sourceUrl\":null",
            )
        ));
        assert!(!invalid_lock_detail(null_source_url.as_bytes()).is_empty());
        let null_dismissed = lock("{}").replace(
            "\"dismissed\": {\"findSkillsPrompt\": true}",
            "\"dismissed\": null",
        );
        assert!(!invalid_lock_detail(null_dismissed.as_bytes()).is_empty());
        let null_agents = lock("{}").replace(
            "\"lastSelectedAgents\": [\"codex\", \"claude-code\"]",
            "\"lastSelectedAgents\": null",
        );
        assert!(!invalid_lock_detail(null_agents.as_bytes()).is_empty());

        let duplicate_dismissed = lock("{}").replace(
            "{\"findSkillsPrompt\": true}",
            "{\"findSkillsPrompt\": true,\"findSkillsPrompt\": false}",
        );
        assert!(
            invalid_lock_detail(duplicate_dismissed.as_bytes())
                .contains("duplicate key \"findSkillsPrompt\"")
        );
    }

    #[test]
    fn residual_serialization_omits_absent_optional_fields() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let original = format!(
            "{{\"version\":3,\"skills\":{{\"managed\":{}}}}}",
            minimal_skill("managed")
        );
        let parsed = match parse_legacy_lock(original.as_bytes()) {
            Ok(parsed) => parsed,
            Err(error) => panic!("minimal lock did not parse: {error}"),
        };
        assert!(parsed.dismissed.is_none());
        assert!(parsed.last_selected_agents.is_none());
        let managed = match parsed.skills.0.get("managed") {
            Some(managed) => managed,
            None => panic!("managed skill is absent"),
        };
        assert!(managed.source_url.is_none());
        assert!(managed.r#ref.is_none());
        assert!(managed.skill_path.is_none());
        assert!(managed.plugin_name.is_none());

        let lock_path = temp.path().join(".skill-lock.json");
        let archive_path = temp.path().join("legacy-lock.archive.json");
        let destination = temp.path().join("managed");
        assert!(fs::write(&lock_path, &original).is_ok());
        assert!(fs::write(&destination, b"A").is_ok());
        assert!(fs::set_permissions(&destination, fs::Permissions::from_mode(0o644)).is_ok());
        let plan = inspect_or_panic(
            &lock_path,
            &archive_path,
            &[CatalogEntry {
                source_id: "managed".to_owned(),
                destination,
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o644,
                link_target: None,
            }],
        );
        assert!(plan.applicable);
        let residual: serde_json::Value = match serde_json::from_slice(&plan.residual_bytes) {
            Ok(value) => value,
            Err(error) => panic!("invalid residual: {error}"),
        };
        assert_eq!(residual["version"], 3);
        assert_eq!(residual["skills"], serde_json::json!({}));
        assert!(residual.get("dismissed").is_none());
        assert!(residual.get("lastSelectedAgents").is_none());
    }

    #[test]
    fn catalog_validation_reports_every_invalid_shape() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let non_utf8_destination = temp.path().join(OsString::from_vec(vec![0xff]));
        let non_utf8_target = PathBuf::from(OsString::from_vec(vec![0xfe]));
        let entries = [
            CatalogEntry {
                source_id: String::new(),
                destination: PathBuf::new(),
                entry_type: EntryType::File,
                sha256: None,
                mode: 0o1000,
                link_target: Some(PathBuf::from("ignored-target")),
            },
            CatalogEntry {
                source_id: "traversal".to_owned(),
                destination: PathBuf::from("/tmp/../catalog-entry"),
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o644,
                link_target: None,
            },
            CatalogEntry {
                source_id: "non-utf8-destination".to_owned(),
                destination: non_utf8_destination,
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o644,
                link_target: None,
            },
            CatalogEntry {
                source_id: "bad-hash".to_owned(),
                destination: temp.path().join("bad-hash"),
                entry_type: EntryType::File,
                sha256: Some("z".repeat(64)),
                mode: 0o644,
                link_target: None,
            },
            CatalogEntry {
                source_id: "directory-target".to_owned(),
                destination: temp.path().join("directory-target"),
                entry_type: EntryType::Directory,
                sha256: None,
                mode: 0o755,
                link_target: Some(PathBuf::from("target")),
            },
            CatalogEntry {
                source_id: "file-target".to_owned(),
                destination: temp.path().join("file-target"),
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o644,
                link_target: Some(PathBuf::from("target")),
            },
            CatalogEntry {
                source_id: "missing-target".to_owned(),
                destination: temp.path().join("missing-target"),
                entry_type: EntryType::Symlink,
                sha256: None,
                mode: 0o777,
                link_target: None,
            },
            CatalogEntry {
                source_id: "non-utf8-target".to_owned(),
                destination: temp.path().join("non-utf8-target"),
                entry_type: EntryType::Symlink,
                sha256: None,
                mode: 0o777,
                link_target: Some(non_utf8_target),
            },
        ];
        let mut diagnostics = Vec::new();
        for entry in &entries {
            validate_catalog_entry(entry, &mut diagnostics);
        }

        assert_eq!(diagnostics.len(), 12);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.detail == "destination must be an absolute path")
                .count(),
            2
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.detail == "only symlinks may declare a link target")
                .count(),
            2
        );
        for fragment in [
            "source_id must not be empty",
            "destination is not UTF-8",
            "mode must contain only POSIX permission bits",
            "a file requires a 64-character SHA-256",
            "a symlink requires an exact link target",
            "sha256 must contain exactly 64 hexadecimal characters",
            "link target is not UTF-8",
        ] {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.detail.contains(fragment)),
                "missing diagnostic containing {fragment:?}"
            );
        }
        assert!(valid_sha256(&HASH_A.to_ascii_uppercase()));
        assert!(!valid_sha256("0"));
        assert!(!valid_sha256(&"g".repeat(64)));
    }

    #[test]
    fn safe_path_checks_distinguish_relative_traversal_and_non_utf8_paths() {
        assert!(require_safe_absolute(Path::new("/tmp/lock"), "lock").is_ok());

        let relative = match require_safe_absolute(Path::new("lock"), "legacy lock path") {
            Err(error) => error,
            Ok(()) => panic!("relative path was accepted"),
        };
        assert_eq!(
            relative.to_string(),
            "legacy lock path must be absolute without traversal: lock"
        );
        assert!(std::error::Error::source(&relative).is_none());

        let traversal = require_safe_absolute(Path::new("/tmp/../lock"), "archive");
        assert!(matches!(traversal, Err(AdoptionError::UnsafePath { .. })));

        let non_utf8 = PathBuf::from(OsString::from_vec(vec![b'/', b't', b'm', b'p', b'/', 0xff]));
        let error = match require_safe_absolute(&non_utf8, "legacy lock path") {
            Err(error) => error,
            Ok(()) => panic!("non-UTF-8 path was accepted"),
        };
        assert_eq!(
            error.to_string(),
            "legacy lock path is not UTF-8 (hex: 2f746d702fff)"
        );
        assert!(matches!(error, AdoptionError::InvalidPath { .. }));
        assert!(matches!(
            revalidate(
                &non_utf8,
                &RevalidationToken {
                    identity: LockIdentity {
                        device: 0,
                        inode: 0,
                        size: 0,
                        mtime_seconds: 0,
                        mtime_nanoseconds: 0,
                    },
                    sha256: String::new(),
                }
            ),
            Err(AdoptionError::InvalidPath { .. })
        ));

        let archive_error = inspect(Path::new("/tmp/missing-lock"), Path::new("archive"), &[]);
        assert!(matches!(
            archive_error,
            Err(AdoptionError::UnsafePath {
                role: "legacy lock archive path",
                ..
            })
        ));
    }

    #[test]
    fn archive_empty_catalog_duplicates_and_missing_ownership_block_adoption() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let (lock_path, archive_path) = write_fixture_lock(&temp, "{}");
        assert!(fs::write(&archive_path, b"archive").is_ok());
        let empty = inspect_or_panic(&lock_path, &archive_path, &[]);
        assert!(!empty.applicable);
        assert!(empty.entries.is_empty());
        assert_eq!(diagnostic_count(&empty, DiagnosticCode::EmptyCatalog), 1);
        assert_eq!(
            diagnostic_count(&empty, DiagnosticCode::ArchiveAlreadyExists),
            1
        );
        assert_eq!(
            empty
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == DiagnosticCode::ArchiveAlreadyExists)
                .and_then(|diagnostic| diagnostic.destination.as_deref()),
            Some(archive_path.as_path())
        );
        let residual: serde_json::Value = match serde_json::from_slice(&empty.residual_bytes) {
            Ok(value) => value,
            Err(error) => panic!("invalid residual: {error}"),
        };
        assert_eq!(residual["skills"], serde_json::json!({}));

        let blocking_parent = temp.path().join("not-a-directory");
        assert!(fs::write(&blocking_parent, b"file").is_ok());
        let archive_error = inspect(&lock_path, &blocking_parent.join("archive"), &[]);
        match archive_error {
            Err(AdoptionError::Io {
                operation,
                path,
                source,
            }) => {
                assert_eq!(operation, "inspect legacy lock archive destination");
                assert_eq!(path, blocking_parent.join("archive"));
                assert_ne!(source.kind(), io::ErrorKind::NotFound);
            }
            Err(error) => panic!("unexpected archive error: {error}"),
            Ok(_) => panic!("invalid archive path was accepted"),
        }

        assert!(fs::remove_file(&archive_path).is_ok());
        assert!(
            fs::write(
                &lock_path,
                lock(&format!("{{\"owned\":{}}}", skill("owned")))
            )
            .is_ok()
        );
        let destination = temp.path().join("shared");
        assert!(fs::write(&destination, b"A").is_ok());
        assert!(fs::set_permissions(&destination, fs::Permissions::from_mode(0o644)).is_ok());
        let catalog = [
            CatalogEntry {
                source_id: "owned".to_owned(),
                destination: destination.clone(),
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o644,
                link_target: None,
            },
            CatalogEntry {
                source_id: "missing".to_owned(),
                destination,
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o644,
                link_target: None,
            },
        ];
        let blocked = inspect_or_panic(&lock_path, &archive_path, &catalog);
        assert!(!blocked.applicable);
        assert!(blocked.entries.is_empty());
        assert_eq!(
            diagnostic_count(&blocked, DiagnosticCode::DuplicateDestination),
            1
        );
        assert_eq!(
            diagnostic_count(&blocked, DiagnosticCode::MissingLegacyEntry),
            1
        );
        let residual: serde_json::Value = match serde_json::from_slice(&blocked.residual_bytes) {
            Ok(value) => value,
            Err(error) => panic!("invalid residual: {error}"),
        };
        assert_eq!(residual["skills"], serde_json::json!({}));
        assert_eq!(decision(&blocked, true, false), AdoptionDecision::Blocked);
    }

    #[test]
    fn inspection_reports_missing_unsupported_type_mode_hash_and_target_errors() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let (lock_path, archive_path) =
            write_fixture_lock(&temp, &format!("{{\"managed\":{}}}", skill("managed")));
        let file = temp.path().join("file");
        assert!(fs::write(&file, b"A").is_ok());
        assert!(fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).is_ok());
        let link = temp.path().join("link");
        assert!(symlink("actual-target", &link).is_ok());
        let socket = temp.path().join("socket");
        let _listener = match UnixListener::bind(&socket) {
            Ok(listener) => listener,
            Err(error) => panic!("cannot create socket fixture: {error}"),
        };
        let expected_target = PathBuf::from("expected-target");
        let expected_target_hash = format!(
            "{:x}",
            Sha256::digest(expected_target.as_os_str().as_bytes())
        );
        let catalog = [
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: temp.path().join("missing"),
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o644,
                link_target: None,
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: file.clone(),
                entry_type: EntryType::Directory,
                sha256: None,
                mode: 0o600,
                link_target: None,
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: file.clone(),
                entry_type: EntryType::File,
                sha256: Some("0".repeat(64)),
                mode: 0o644,
                link_target: None,
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: link.clone(),
                entry_type: EntryType::Symlink,
                sha256: Some(expected_target_hash),
                mode: filesystem_mode(&link),
                link_target: Some(expected_target),
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: socket,
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o600,
                link_target: None,
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: file.join("child"),
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_owned()),
                mode: 0o644,
                link_target: None,
            },
        ];
        let plan = inspect_or_panic(&lock_path, &archive_path, &catalog);
        assert!(!plan.applicable);
        assert!(plan.entries.is_empty());
        for (code, expected) in [
            (DiagnosticCode::DestinationMissing, 1),
            (DiagnosticCode::UnsupportedFileType, 1),
            (DiagnosticCode::TypeMismatch, 1),
            (DiagnosticCode::ModeMismatch, 1),
            (DiagnosticCode::HashMismatch, 2),
            (DiagnosticCode::LinkTargetMismatch, 1),
            (DiagnosticCode::ConcurrentAssetChange, 1),
        ] {
            assert_eq!(diagnostic_count(&plan, code), expected, "code {code:?}");
        }
    }

    #[test]
    fn directory_adoption_covers_complete_sealed_and_unexpected_trees() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let (lock_path, archive_path) =
            write_fixture_lock(&temp, &format!("{{\"managed\":{}}}", skill("managed")));
        let root = temp.path().join("tree");
        let subdirectory = root.join("subdirectory");
        let file_a = root.join("a.txt");
        let file_b = subdirectory.join("b.txt");
        let link = root.join("link");
        assert!(fs::create_dir_all(&subdirectory).is_ok());
        assert!(fs::write(&file_a, b"A").is_ok());
        assert!(fs::write(&file_b, b"B").is_ok());
        assert!(symlink("a.txt", &link).is_ok());
        assert!(fs::set_permissions(&root, fs::Permissions::from_mode(0o750)).is_ok());
        assert!(fs::set_permissions(&subdirectory, fs::Permissions::from_mode(0o700)).is_ok());
        assert!(fs::set_permissions(&file_a, fs::Permissions::from_mode(0o640)).is_ok());
        assert!(fs::set_permissions(&file_b, fs::Permissions::from_mode(0o600)).is_ok());

        let root_hash = match hash_directory(&root) {
            Ok(hash) => hash,
            Err(error) => panic!("cannot hash root fixture: {error}"),
        };
        let subdirectory_hash = match hash_directory(&subdirectory) {
            Ok(hash) => hash,
            Err(error) => panic!("cannot hash subdirectory fixture: {error}"),
        };
        let hash_b = format!("{:x}", Sha256::digest(b"B"));
        let link_hash = format!("{:x}", Sha256::digest(b"a.txt"));
        let catalog = [
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: root.clone(),
                entry_type: EntryType::Directory,
                sha256: Some(root_hash),
                mode: 0o750,
                link_target: None,
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: subdirectory.clone(),
                entry_type: EntryType::Directory,
                sha256: Some(subdirectory_hash),
                mode: 0o700,
                link_target: None,
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: file_a,
                entry_type: EntryType::File,
                sha256: Some(HASH_A.to_ascii_uppercase()),
                mode: 0o640,
                link_target: None,
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: file_b,
                entry_type: EntryType::File,
                sha256: Some(hash_b),
                mode: 0o600,
                link_target: None,
            },
            CatalogEntry {
                source_id: "managed".to_owned(),
                destination: link,
                entry_type: EntryType::Symlink,
                sha256: Some(link_hash),
                mode: filesystem_mode(&root.join("link")),
                link_target: Some(PathBuf::from("a.txt")),
            },
        ];
        let complete = inspect_or_panic(&lock_path, &archive_path, &catalog);
        assert!(complete.applicable);
        assert_eq!(complete.entries.len(), catalog.len());
        assert!(
            complete
                .entries
                .windows(2)
                .all(|entries| entries[0].destination <= entries[1].destination)
        );
        for entry_type in [EntryType::File, EntryType::Directory, EntryType::Symlink] {
            assert!(
                complete
                    .entries
                    .iter()
                    .any(|entry| entry.entry_type == entry_type)
            );
        }

        let sealed = temp.path().join("sealed");
        assert!(fs::create_dir(&sealed).is_ok());
        assert!(fs::write(sealed.join("covered-by-hash"), b"A").is_ok());
        let sealed_hash = match hash_directory(&sealed) {
            Ok(hash) => hash,
            Err(error) => panic!("cannot hash sealed fixture: {error}"),
        };
        let sealed_plan = inspect_or_panic(
            &lock_path,
            &archive_path,
            &[CatalogEntry {
                source_id: "managed".to_owned(),
                destination: sealed.clone(),
                entry_type: EntryType::Directory,
                sha256: Some(sealed_hash),
                mode: filesystem_mode(&sealed),
                link_target: None,
            }],
        );
        assert!(sealed_plan.applicable);

        let unexpected = inspect_or_panic(
            &lock_path,
            &archive_path,
            &[CatalogEntry {
                source_id: "managed".to_owned(),
                destination: sealed,
                entry_type: EntryType::Directory,
                sha256: None,
                mode: filesystem_mode(&temp.path().join("sealed")),
                link_target: None,
            }],
        );
        assert!(!unexpected.applicable);
        assert_eq!(
            diagnostic_count(&unexpected, DiagnosticCode::UnexpectedDirectoryEntry),
            1
        );
    }

    #[test]
    fn directory_fingerprinting_rejects_non_utf8_and_unsupported_descendants() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let non_utf8_root = temp.path().join("non-utf8-root");
        assert!(fs::create_dir(&non_utf8_root).is_ok());
        assert!(fs::write(non_utf8_root.join(OsString::from_vec(vec![0xff])), b"A").is_ok());
        let error = match hash_directory(&non_utf8_root) {
            Err(error) => error,
            Ok(_) => panic!("non-UTF-8 descendant was hashed"),
        };
        assert!(error.contains("catalog path is not UTF-8"));

        let unsupported_root = temp.path().join("unsupported-root");
        assert!(fs::create_dir(&unsupported_root).is_ok());
        let socket_path = unsupported_root.join("socket");
        let _listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(error) => panic!("cannot create socket fixture: {error}"),
        };
        let error = match hash_directory(&unsupported_root) {
            Err(error) => error,
            Ok(_) => panic!("unsupported descendant was hashed"),
        };
        assert!(error.contains("unsupported filesystem type"));

        let invalid_target = PathBuf::from(OsString::from_vec(vec![0xfe]));
        let link = temp.path().join("invalid-target-link");
        assert!(symlink(&invalid_target, &link).is_ok());
        let error = match fingerprint(&link, EntryType::Symlink) {
            Err(error) => error,
            Ok(_) => panic!("non-UTF-8 symlink target was hashed"),
        };
        assert!(error.contains("symlink target is not UTF-8"));
        let link_mode = filesystem_mode(&link);
        let destinations = BTreeSet::from([link.clone()]);
        let inspected = inspect_entry(
            &CatalogEntry {
                source_id: "invalid-target".to_owned(),
                destination: link,
                entry_type: EntryType::Symlink,
                sha256: None,
                mode: link_mode,
                link_target: Some(invalid_target),
            },
            &destinations,
        );
        assert!(matches!(
            inspected,
            Err(diagnostics)
                if diagnostics.len() == 1
                    && diagnostics[0].code == DiagnosticCode::ConcurrentAssetChange
                    && diagnostics[0].detail.contains("symlink target is not UTF-8")
        ));
        assert!(fingerprint(&temp.path().join("missing"), EntryType::File).is_err());
        assert!(fingerprint(&temp.path().join("missing"), EntryType::Directory).is_err());
    }

    #[test]
    fn stable_file_revalidation_and_error_displays_cover_all_variants() {
        let temp = match TempDir::new() {
            Ok(temp) => temp,
            Err(error) => panic!("cannot create fixture: {error}"),
        };
        let lock_path = temp.path().join(".skill-lock.json");
        assert!(fs::write(&lock_path, b"A").is_ok());
        let snapshot = match read_stable_regular_file(&lock_path, "read fixture") {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("cannot read stable fixture: {error}"),
        };
        assert_eq!(snapshot.bytes, b"A");
        assert_eq!(snapshot.sha256, HASH_A);
        assert_eq!(snapshot.identity.size, 1);

        let token = RevalidationToken {
            identity: snapshot.identity.clone(),
            sha256: snapshot.sha256.clone(),
        };
        assert!(revalidate(&lock_path, &token).is_ok());
        let wrong_hash = RevalidationToken {
            identity: token.identity.clone(),
            sha256: "0".repeat(64),
        };
        let changed = match revalidate(&lock_path, &wrong_hash) {
            Err(error @ AdoptionError::LegacyLockChanged { .. }) => error,
            Err(error) => panic!("unexpected revalidation error: {error}"),
            Ok(()) => panic!("wrong hash passed revalidation"),
        };
        match &changed {
            AdoptionError::LegacyLockChanged { expected, actual } => {
                assert_eq!(expected.as_ref(), &wrong_hash);
                assert_eq!(actual.as_ref(), &token);
            }
            _ => panic!("wrong error variant"),
        }
        assert_eq!(
            changed.to_string(),
            "legacy lock changed during adoption; stop other skill managers and retry"
        );
        assert!(std::error::Error::source(&changed).is_none());

        assert!(fs::write(&lock_path, b"B").is_ok());
        assert!(matches!(
            revalidate(&lock_path, &token),
            Err(AdoptionError::LegacyLockChanged { .. })
        ));

        let directory_error = read_stable_regular_file(temp.path(), "read directory");
        assert!(matches!(
            directory_error,
            Err(AdoptionError::InvalidLegacyLock(_))
        ));
        let symlink_path = temp.path().join("lock-link");
        assert!(symlink(&lock_path, &symlink_path).is_ok());
        let symlink_error = read_stable_regular_file(&symlink_path, "read symlink");
        assert!(matches!(
            symlink_error,
            Err(AdoptionError::InvalidLegacyLock(_))
        ));
        let missing_error = read_stable_regular_file(&temp.path().join("missing"), "read missing");
        match missing_error {
            Err(error @ AdoptionError::Io { .. }) => {
                assert!(error.to_string().starts_with("read missing "));
                assert!(std::error::Error::source(&error).is_some());
            }
            Err(error) => panic!("unexpected missing-file error: {error}"),
            Ok(_) => panic!("missing file was read"),
        }

        let unreadable_path = temp.path().join("unreadable");
        assert!(fs::write(&unreadable_path, b"A").is_ok());
        assert!(fs::set_permissions(&unreadable_path, fs::Permissions::from_mode(0o000)).is_ok());
        let unreadable = read_stable_regular_file(&unreadable_path, "open unreadable");
        assert!(fs::set_permissions(&unreadable_path, fs::Permissions::from_mode(0o600)).is_ok());
        assert!(matches!(
            unreadable,
            Err(AdoptionError::Io {
                operation: "open unreadable",
                source,
                ..
            }) if source.kind() == io::ErrorKind::PermissionDenied
        ));

        #[cfg(target_os = "linux")]
        {
            assert!(matches!(
                read_stable_regular_file(Path::new("/proc/self/mem"), "read process memory"),
                Err(AdoptionError::Io {
                    operation: "read process memory",
                    ..
                })
            ));
            assert!(matches!(
                read_stable_regular_file(Path::new("/proc/self/cmdline"), "read proc file"),
                Err(AdoptionError::InvalidLegacyLock(detail))
                    if detail.contains("changed while it was being inspected")
            ));
        }

        let io_error = AdoptionError::Io {
            operation: "read",
            path: PathBuf::from("/tmp/lock"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        };
        assert_eq!(io_error.to_string(), "read /tmp/lock: denied");
        assert!(std::error::Error::source(&io_error).is_some());
        let invalid_lock = AdoptionError::InvalidLegacyLock("bad schema".to_owned());
        assert_eq!(
            invalid_lock.to_string(),
            "unsupported .skill-lock.json v3: bad schema"
        );
        assert!(std::error::Error::source(&invalid_lock).is_none());
        assert_eq!(
            concurrent_read_error(Path::new("/tmp/lock")).to_string(),
            "unsupported .skill-lock.json v3: /tmp/lock changed while it was being inspected"
        );
    }
}
