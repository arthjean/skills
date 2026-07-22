use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use fs2::FileExt;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use signal_hook::consts::signal::{SIGINT, SIGTERM};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

pub const TRANSACTION_SCHEMA_VERSION: u16 = 1;
pub const TRANSACTION_EXIT_CODE: u8 = 5;
pub const SIGINT_EXIT_CODE: u8 = 130;
pub const SIGTERM_EXIT_CODE: u8 = 143;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathKind {
    Absent,
    File,
    Directory,
    Symlink,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PathSnapshot {
    pub kind: PathKind,
    pub sha256: Option<String>,
    pub mode: Option<u32>,
    pub link_target: Option<PathBuf>,
    pub size: Option<u64>,
    pub device: Option<u64>,
    pub inode: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentityProof {
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub mtime_seconds: i64,
    pub mtime_nanoseconds: i64,
    pub sha256: String,
}

impl PathSnapshot {
    pub const fn absent() -> Self {
        Self {
            kind: PathKind::Absent,
            sha256: None,
            mode: None,
            link_target: None,
            size: None,
            device: None,
            inode: None,
        }
    }

    pub fn file(bytes: &[u8], mode: u32) -> Self {
        Self {
            kind: PathKind::File,
            sha256: Some(hash_bytes(bytes)),
            mode: Some(mode),
            link_target: None,
            size: u64::try_from(bytes.len()).ok(),
            device: None,
            inode: None,
        }
    }

    pub const fn directory(mode: u32) -> Self {
        Self {
            kind: PathKind::Directory,
            sha256: None,
            mode: Some(mode),
            link_target: None,
            size: None,
            device: None,
            inode: None,
        }
    }

    pub fn symlink(target: PathBuf) -> Self {
        Self {
            kind: PathKind::Symlink,
            sha256: None,
            mode: None,
            link_target: Some(target),
            size: None,
            device: None,
            inode: None,
        }
    }

    fn with_identity(mut self, metadata: &fs::Metadata) -> Self {
        self.device = Some(metadata_device(metadata));
        self.inode = Some(metadata_inode(metadata));
        self
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn snapshot_path(path: &Path) -> Result<PathSnapshot, TransactionError> {
    require_utf8(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(PathSnapshot::absent()),
        Err(error) => return Err(TransactionError::io("inspect path", path, error)),
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        let target = fs::read_link(path)
            .map_err(|error| TransactionError::io("read symlink", path, error))?;
        require_utf8(&target)?;
        let after = fs::symlink_metadata(path)
            .map_err(|error| TransactionError::io("reinspect symlink", path, error))?;
        if !after.file_type().is_symlink() || !metadata_is_stable(&metadata, &after) {
            return Err(TransactionError::ConcurrentFilesystemChange(
                path.to_path_buf(),
            ));
        }
        return Ok(PathSnapshot::symlink(target).with_identity(&after));
    }
    if file_type.is_dir() {
        let directory = File::open(path)
            .map_err(|error| TransactionError::io("open directory", path, error))?;
        let opened = directory
            .metadata()
            .map_err(|error| TransactionError::io("inspect open directory", path, error))?;
        let after = fs::symlink_metadata(path)
            .map_err(|error| TransactionError::io("reinspect directory", path, error))?;
        if !opened.file_type().is_dir()
            || !after.file_type().is_dir()
            || !metadata_is_stable(&metadata, &opened)
            || !metadata_is_stable(&opened, &after)
        {
            return Err(TransactionError::ConcurrentFilesystemChange(
                path.to_path_buf(),
            ));
        }
        return Ok(PathSnapshot::directory(metadata_mode(&opened)).with_identity(&opened));
    }
    if !file_type.is_file() {
        return Err(TransactionError::UnexpectedPathType(path.to_path_buf()));
    }

    let mut file =
        File::open(path).map_err(|error| TransactionError::io("open file", path, error))?;
    let opened = file
        .metadata()
        .map_err(|error| TransactionError::io("inspect open file", path, error))?;
    if !opened.file_type().is_file() || !metadata_is_stable(&metadata, &opened) {
        return Err(TransactionError::ConcurrentFilesystemChange(
            path.to_path_buf(),
        ));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| TransactionError::io("hash file", path, error))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let opened_after = file
        .metadata()
        .map_err(|error| TransactionError::io("reinspect open file", path, error))?;
    let path_after = fs::symlink_metadata(path)
        .map_err(|error| TransactionError::io("reinspect file path", path, error))?;
    if !path_after.file_type().is_file()
        || !metadata_is_stable(&opened, &opened_after)
        || !metadata_is_stable(&opened_after, &path_after)
    {
        return Err(TransactionError::ConcurrentFilesystemChange(
            path.to_path_buf(),
        ));
    }
    Ok(PathSnapshot {
        kind: PathKind::File,
        sha256: Some(format!("{:x}", digest.finalize())),
        mode: Some(metadata_mode(&opened_after)),
        link_target: None,
        size: Some(opened_after.len()),
        device: Some(metadata_device(&opened_after)),
        inode: Some(metadata_inode(&opened_after)),
    })
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RootSpec {
    pub id: String,
    pub path: PathBuf,
    pub real: PathBuf,
    pub device: u64,
}

impl RootSpec {
    pub fn new(id: impl Into<String>, path: PathBuf, device: u64) -> Self {
        Self {
            id: id.into(),
            real: path.clone(),
            path,
            device,
        }
    }

    pub fn with_real(mut self, real: PathBuf) -> Self {
        self.real = real;
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "proof")]
pub enum OwnershipProof {
    UnownedDestination,
    Receipt {
        source_id: String,
        sha256: Option<String>,
    },
    Adopted {
        source_id: String,
        sha256: Option<String>,
    },
    TransactionState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum Inverse {
    RemoveCreated,
    RestoreBackup { original: PathSnapshot },
    RestoreMode { mode: u32 },
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    EnsureDirectory,
    WriteFile,
    ReplaceFile,
    SetMode,
    CreateSymlink,
    RemoveOwnedPath,
    RewriteLegacyLock,
    WriteReceipt,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum OperationPayload {
    File {
        bytes: Vec<u8>,
        mode: u32,
    },
    Symlink {
        target: PathBuf,
    },
    Mode(u32),
    Directory(u32),
    None,
    #[default]
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub id: String,
    pub kind: OperationKind,
    pub root: RootSpec,
    pub destination: PathBuf,
    pub device: u64,
    pub precondition: PathSnapshot,
    pub expected_after: PathSnapshot,
    pub inverse: Inverse,
    pub ownership: OwnershipProof,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revalidation: Option<FileIdentityProof>,
    #[serde(skip)]
    pub payload: OperationPayload,
}

impl Operation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        kind: OperationKind,
        root: RootSpec,
        destination: PathBuf,
        precondition: PathSnapshot,
        expected_after: PathSnapshot,
        inverse: Inverse,
        ownership: OwnershipProof,
        payload: OperationPayload,
    ) -> Result<Self, TransactionError> {
        let operation = Self {
            id: id.into(),
            kind,
            device: root.device,
            root,
            destination,
            precondition,
            expected_after,
            inverse,
            ownership,
            revalidation: None,
            payload,
        };
        operation.validate()?;
        Ok(operation)
    }

    pub fn with_revalidation(mut self, proof: FileIdentityProof) -> Result<Self, TransactionError> {
        if self.kind != OperationKind::RewriteLegacyLock {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "only legacy-lock rewrites accept an identity proof",
            ));
        }
        self.revalidation = Some(proof);
        self.validate_for_execution()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), TransactionError> {
        if self.id.is_empty() || self.root.id.is_empty() {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "operation and root identifiers cannot be empty",
            ));
        }
        if !is_safe_identifier(&self.root.id) {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "root identifier must be ASCII alphanumeric, '-' or '_'",
            ));
        }
        require_utf8(&self.root.path)?;
        require_utf8(&self.root.real)?;
        require_utf8(&self.destination)?;
        if !is_normalized_absolute(&self.root.path)
            || !is_normalized_absolute(&self.root.real)
            || !is_normalized_absolute(&self.destination)
        {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "root and destination must be normalized absolute paths",
            ));
        }
        if self.device != self.root.device {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "operation device differs from its root device",
            ));
        }
        if !self.destination.starts_with(&self.root.path) {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "destination is outside its transaction root",
            ));
        }
        let payload_matches = matches!(
            (&self.kind, &self.payload),
            (
                OperationKind::EnsureDirectory,
                OperationPayload::Directory(_)
            ) | (
                OperationKind::WriteFile
                    | OperationKind::ReplaceFile
                    | OperationKind::RewriteLegacyLock
                    | OperationKind::WriteReceipt,
                OperationPayload::File { .. }
            ) | (OperationKind::SetMode, OperationPayload::Mode(_))
                | (
                    OperationKind::CreateSymlink,
                    OperationPayload::Symlink { .. }
                )
                | (OperationKind::RemoveOwnedPath, OperationPayload::None)
                | (_, OperationPayload::Unavailable)
        );
        if !payload_matches {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "operation kind and payload disagree",
            ));
        }
        if self.precondition.kind == PathKind::Absent
            && self.ownership != OwnershipProof::UnownedDestination
            && self.ownership != OwnershipProof::TransactionState
        {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "an absent destination cannot carry existing ownership proof",
            ));
        }
        if self.precondition.kind != PathKind::Absent
            && self.ownership == OwnershipProof::UnownedDestination
        {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "an existing destination requires receipt or adoption proof",
            ));
        }
        if self.kind == OperationKind::WriteReceipt {
            let payload_mode = match &self.payload {
                OperationPayload::File { mode, .. } => Some(*mode),
                OperationPayload::Unavailable => self.expected_after.mode,
                _ => None,
            };
            if payload_mode != Some(0o600) || self.expected_after.mode != Some(0o600) {
                return Err(TransactionError::InvalidOperation(
                    self.id.clone(),
                    "receipt files must use mode 0600",
                ));
            }
        }
        self.validate_inverse()?;
        self.validate_expected_payload()?;
        Ok(())
    }

    fn validate_for_execution(&self) -> Result<(), TransactionError> {
        self.validate()?;
        match (self.kind, &self.revalidation) {
            (OperationKind::RewriteLegacyLock, Some(proof)) => {
                if self.precondition.kind != PathKind::File
                    || self.precondition.sha256.as_deref() != Some(proof.sha256.as_str())
                    || self.precondition.size != Some(proof.size)
                {
                    return Err(TransactionError::InvalidOperation(
                        self.id.clone(),
                        "legacy-lock identity does not match its content precondition",
                    ));
                }
            }
            (OperationKind::RewriteLegacyLock, None) => {
                return Err(TransactionError::InvalidOperation(
                    self.id.clone(),
                    "legacy-lock rewrite requires inode, size, mtime, and hash proof",
                ));
            }
            (_, Some(_)) => {
                return Err(TransactionError::InvalidOperation(
                    self.id.clone(),
                    "identity proof is only valid for legacy-lock rewrites",
                ));
            }
            (_, None) => {}
        }
        if self.kind == OperationKind::WriteReceipt
            && self.precondition.kind != PathKind::Absent
            && self.precondition.sha256 == self.expected_after.sha256
        {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "receipt commit must carry a new transaction identity",
            ));
        }
        Ok(())
    }

    fn validate_inverse(&self) -> Result<(), TransactionError> {
        let valid = if self.precondition.kind == PathKind::Absent {
            matches!(&self.inverse, Inverse::RemoveCreated)
        } else if self.kind == OperationKind::SetMode {
            matches!(&self.inverse, Inverse::RestoreMode { mode } if Some(*mode) == self.precondition.mode)
        } else {
            matches!(&self.inverse, Inverse::RestoreBackup { original } if original == &self.precondition)
        };
        if !valid {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "inverse does not restore the declared precondition",
            ));
        }
        Ok(())
    }

    fn validate_expected_payload(&self) -> Result<(), TransactionError> {
        let expected = match &self.payload {
            OperationPayload::File { bytes, mode } => Some(PathSnapshot::file(bytes, *mode)),
            OperationPayload::Symlink { target } => Some(PathSnapshot::symlink(target.clone())),
            OperationPayload::Mode(mode) => {
                let mut expected = self.precondition.clone();
                expected.mode = Some(*mode);
                Some(expected)
            }
            OperationPayload::Directory(mode) => Some(PathSnapshot::directory(*mode)),
            OperationPayload::None => Some(PathSnapshot::absent()),
            OperationPayload::Unavailable => None,
        };
        if expected
            .as_ref()
            .is_some_and(|expected| expected != &self.expected_after)
        {
            return Err(TransactionError::InvalidOperation(
                self.id.clone(),
                "payload does not produce the declared postcondition",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JournalState {
    Preparing,
    Prepared,
    Applying,
    Committing,
    Committed,
    RollingBack,
    RecoveryRequired,
    RolledBack,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OperationState {
    Prepared,
    Applying,
    Applied,
    RolledBack,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JournalOperation {
    operation: Operation,
    state: OperationState,
    staged_path: Option<PathBuf>,
    backup_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    applied_snapshot: Option<PathSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema_version: u16,
    transaction_id: String,
    state: JournalState,
    receipt_committed: bool,
    operations: Vec<JournalOperation>,
    staging_roots: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationPrimitive {
    CreateDirectory,
    CreateStagedDirectory,
    WriteStagedFile,
    CreateStagedSymlink,
    InstallNoReplace,
    Rename,
    SetMode,
    Remove,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationPoint {
    pub ordinal: usize,
    pub operation_id: Option<String>,
    pub primitive: MutationPrimitive,
}

pub trait FailureInjector {
    fn after_mutation(&mut self, _point: &MutationPoint) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Default)]
pub struct NoFailures;

impl FailureInjector for NoFailures {}

pub struct FailAfterMutation {
    target: usize,
    seen: usize,
    fired: bool,
}

impl FailAfterMutation {
    pub const fn new(target: usize) -> Self {
        Self {
            target,
            seen: 0,
            fired: false,
        }
    }
}

impl FailureInjector for FailAfterMutation {
    fn after_mutation(&mut self, _point: &MutationPoint) -> Result<(), String> {
        self.seen += 1;
        if !self.fired && self.seen == self.target {
            self.fired = true;
            return Err(format!("injected failure after mutation {}", self.target));
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct SignalFlags {
    sigint: Arc<AtomicBool>,
    sigterm: Arc<AtomicBool>,
}

impl SignalFlags {
    pub fn install() -> Result<Self, TransactionError> {
        let flags = Self::default();
        signal_hook::flag::register(SIGINT, Arc::clone(&flags.sigint))
            .map_err(|error| TransactionError::SignalHandler(error.to_string()))?;
        signal_hook::flag::register(SIGTERM, Arc::clone(&flags.sigterm))
            .map_err(|error| TransactionError::SignalHandler(error.to_string()))?;
        Ok(flags)
    }

    pub fn pending_exit_code(&self) -> Option<u8> {
        if self.sigint.load(Ordering::SeqCst) {
            Some(SIGINT_EXIT_CODE)
        } else if self.sigterm.load(Ordering::SeqCst) {
            Some(SIGTERM_EXIT_CODE)
        } else {
            None
        }
    }

    pub fn record_for_test(&self, signal: i32) {
        match signal {
            SIGINT => self.sigint.store(true, Ordering::SeqCst),
            SIGTERM => self.sigterm.store(true, Ordering::SeqCst),
            _ => {}
        }
    }
}

pub struct TransactionLock {
    file: File,
}

impl TransactionLock {
    pub fn acquire(path: &Path) -> Result<Self, TransactionError> {
        require_utf8(path)?;
        let before = match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                    return Err(TransactionError::InsecureStatePath(path.to_path_buf()));
                }
                if metadata_mode(&metadata) != 0o600 {
                    return Err(TransactionError::InsecureStatePath(path.to_path_buf()));
                }
                Some(metadata)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(TransactionError::io(
                    "inspect transaction lock",
                    path,
                    error,
                ));
            }
        };
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options
            .open(path)
            .map_err(|error| TransactionError::io("open transaction lock", path, error))?;
        let opened = file
            .metadata()
            .map_err(|error| TransactionError::io("inspect open transaction lock", path, error))?;
        let after = fs::symlink_metadata(path)
            .map_err(|error| TransactionError::io("reinspect transaction lock", path, error))?;
        if !after.file_type().is_file()
            || after.file_type().is_symlink()
            || metadata_device(&opened) != metadata_device(&after)
            || metadata_inode(&opened) != metadata_inode(&after)
        {
            return Err(TransactionError::InsecureStatePath(path.to_path_buf()));
        }
        if let Some(before_metadata) = before.as_ref()
            && (metadata_device(before_metadata) != metadata_device(&opened)
                || metadata_inode(before_metadata) != metadata_inode(&opened))
        {
            return Err(TransactionError::InsecureStatePath(path.to_path_buf()));
        }
        file.try_lock_exclusive().map_err(|error| {
            if error.kind() == io::ErrorKind::WouldBlock {
                TransactionError::LockBusy
            } else {
                TransactionError::io("lock transaction", path, error)
            }
        })?;
        if metadata_mode(&opened) != 0o600 {
            if before.is_some() {
                return Err(TransactionError::InsecureStatePath(path.to_path_buf()));
            }
            #[cfg(unix)]
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|error| TransactionError::io("set transaction lock mode", path, error))?;
        }
        Ok(Self { file })
    }
}

impl Drop for TransactionLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionOutcome {
    Committed,
    RecoveredRollback,
    RecoveredCleanup,
}

#[derive(Debug)]
pub enum TransactionError {
    LockBusy,
    RecoveryRequired,
    InvalidPath(PathBuf),
    InsecureStatePath(PathBuf),
    UnexpectedPathType(PathBuf),
    ConcurrentFilesystemChange(PathBuf),
    InvalidOperation(String, &'static str),
    DuplicateOperation(String),
    PreconditionsChanged {
        operation_id: String,
        expected: Box<PathSnapshot>,
        observed: Box<PathSnapshot>,
    },
    DeviceMismatch {
        root: PathBuf,
        expected: u64,
        observed: u64,
    },
    RootIdentityChanged {
        root: PathBuf,
    },
    Interrupted(u8),
    InjectedFailure(String),
    SignalHandler(String),
    LegacyLockChanged(PathBuf),
    Journal(String),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl TransactionError {
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Interrupted(code) => *code,
            _ => TRANSACTION_EXIT_CODE,
        }
    }

    fn io(action: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            source,
        }
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockBusy => formatter.write_str("another Arthur Workflow transaction is running"),
            Self::RecoveryRequired => formatter
                .write_str("an incomplete transaction requires recover before another mutation"),
            Self::InvalidPath(path) => {
                write!(formatter, "path is not valid UTF-8: {}", path.display())
            }
            Self::InsecureStatePath(path) => write!(
                formatter,
                "transaction state path is not a private regular node: {}",
                path.display()
            ),
            Self::UnexpectedPathType(path) => {
                write!(
                    formatter,
                    "unsupported filesystem node at {}",
                    path.display()
                )
            }
            Self::ConcurrentFilesystemChange(path) => write!(
                formatter,
                "filesystem node changed while it was inspected: {}",
                path.display()
            ),
            Self::InvalidOperation(id, detail) => {
                write!(formatter, "operation {id} is invalid: {detail}")
            }
            Self::DuplicateOperation(id) => {
                write!(formatter, "operation identifier is duplicated: {id}")
            }
            Self::PreconditionsChanged { operation_id, .. } => {
                write!(
                    formatter,
                    "operation {operation_id} precondition changed before application"
                )
            }
            Self::DeviceMismatch {
                root,
                expected,
                observed,
            } => write!(
                formatter,
                "root {} moved from device {expected} to {observed}",
                root.display()
            ),
            Self::RootIdentityChanged { root } => write!(
                formatter,
                "root identity changed before filesystem mutation: {}",
                root.display()
            ),
            Self::Interrupted(code) => {
                write!(formatter, "transaction interrupted (exit code {code})")
            }
            Self::InjectedFailure(detail) => formatter.write_str(detail),
            Self::SignalHandler(detail) => {
                write!(formatter, "cannot install signal handlers: {detail}")
            }
            Self::LegacyLockChanged(path) => write!(
                formatter,
                "legacy lock changed immediately before replacement: {}",
                path.display()
            ),
            Self::Journal(detail) => write!(formatter, "transaction journal is invalid: {detail}"),
            Self::Io {
                action,
                path,
                source,
            } => {
                write!(formatter, "cannot {action} {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for TransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub struct TransactionEngine {
    state_directory: PathBuf,
    signal_flags: SignalFlags,
}

impl TransactionEngine {
    pub fn new(state_directory: PathBuf, signal_flags: SignalFlags) -> Self {
        Self {
            state_directory,
            signal_flags,
        }
    }

    pub fn apply(
        &self,
        transaction_id: impl Into<String>,
        operations: Vec<Operation>,
    ) -> Result<TransactionOutcome, TransactionError> {
        self.apply_with(transaction_id, operations, &mut NoFailures)
    }

    pub fn apply_with(
        &self,
        transaction_id: impl Into<String>,
        mut operations: Vec<Operation>,
        injector: &mut dyn FailureInjector,
    ) -> Result<TransactionOutcome, TransactionError> {
        let transaction_id = transaction_id.into();
        validate_transaction_id(&transaction_id)?;
        ensure_private_directory(&self.state_directory)?;
        let _lock = TransactionLock::acquire(&self.lock_path())?;
        if path_exists(&self.journal_path())? || path_exists(&self.journal_temporary_path())? {
            return Err(TransactionError::RecoveryRequired);
        }
        validate_and_sort(&mut operations)?;
        let mut mutation_ordinal = 0;
        let mut journal =
            self.prepare(transaction_id, operations, injector, &mut mutation_ordinal)?;

        let result = self.apply_journal(&mut journal, injector, &mut mutation_ordinal);
        if let Err(error) = result {
            if journal.state != JournalState::Committed
                && self
                    .rollback(&mut journal, injector, &mut mutation_ordinal)
                    .is_err()
            {
                return Err(TransactionError::RecoveryRequired);
            }
            return Err(error);
        }
        self.cleanup(&journal)?;
        self.remove_journal()?;
        Ok(TransactionOutcome::Committed)
    }

    pub fn recover(
        &self,
        trusted_roots: &[RootSpec],
    ) -> Result<TransactionOutcome, TransactionError> {
        ensure_private_directory(&self.state_directory)?;
        let _lock = TransactionLock::acquire(&self.lock_path())?;
        let discarded_temporary = self.discard_journal_temporary()?;
        if !path_exists(&self.journal_path())? && discarded_temporary {
            return Ok(TransactionOutcome::RecoveredRollback);
        }
        let mut journal = self.read_journal()?;
        validate_recovery_roots(&journal, trusted_roots)?;
        let mut injector = NoFailures;
        let mut mutation_ordinal = 0;
        if journal.state == JournalState::Preparing {
            self.cleanup(&journal)?;
            self.remove_journal()?;
            return Ok(TransactionOutcome::RecoveredRollback);
        }
        if journal.state == JournalState::Committed || receipt_is_committed(&journal)? {
            journal.state = JournalState::Committed;
            journal.receipt_committed = true;
            self.persist_journal(&journal)?;
            self.cleanup(&journal)?;
            self.remove_journal()?;
            return Ok(TransactionOutcome::RecoveredCleanup);
        }
        self.rollback(&mut journal, &mut injector, &mut mutation_ordinal)?;
        Ok(TransactionOutcome::RecoveredRollback)
    }

    pub fn journal_state(&self) -> Result<Option<JournalState>, TransactionError> {
        if !path_exists(&self.journal_path())? {
            return Ok(None);
        }
        self.read_journal().map(|journal| Some(journal.state))
    }

    fn lock_path(&self) -> PathBuf {
        self.state_directory.join("transaction.lock")
    }

    fn journal_path(&self) -> PathBuf {
        self.state_directory.join("transaction.json")
    }

    fn journal_temporary_path(&self) -> PathBuf {
        self.state_directory.join("transaction.json.next")
    }

    fn prepare(
        &self,
        transaction_id: String,
        operations: Vec<Operation>,
        injector: &mut dyn FailureInjector,
        mutation_ordinal: &mut usize,
    ) -> Result<Journal, TransactionError> {
        let mut staging_roots = BTreeMap::new();
        for operation in &operations {
            if staging_roots.contains_key(&operation.root.id) {
                continue;
            }
            validate_root_device(&operation.root)?;
            let parent = operation.root.path.parent().ok_or_else(|| {
                TransactionError::InvalidOperation(operation.id.clone(), "root has no parent")
            })?;
            let stage = parent.join(format!(
                ".arthur-workflow-{}-{}.stage",
                transaction_id, operation.root.id
            ));
            staging_roots.insert(operation.root.id.clone(), stage);
        }

        let mut journal_operations = Vec::with_capacity(operations.len());
        for (index, operation) in operations.into_iter().enumerate() {
            let stage = staging_roots.get(&operation.root.id).ok_or_else(|| {
                TransactionError::Journal(format!("missing staging root {}", operation.root.id))
            })?;
            let staged_path = matches!(
                operation.kind,
                OperationKind::EnsureDirectory
                    | OperationKind::WriteFile
                    | OperationKind::ReplaceFile
                    | OperationKind::CreateSymlink
                    | OperationKind::RewriteLegacyLock
                    | OperationKind::WriteReceipt
            )
            .then(|| stage.join("prepared").join(format!("{index}.new")));
            let backup_path = matches!(&operation.inverse, Inverse::RestoreBackup { .. })
                .then(|| stage.join("backups").join(format!("{index}.old")));
            journal_operations.push(JournalOperation {
                operation,
                state: OperationState::Prepared,
                staged_path,
                backup_path,
                applied_snapshot: None,
            });
        }
        let mut journal = Journal {
            schema_version: TRANSACTION_SCHEMA_VERSION,
            transaction_id,
            state: JournalState::Preparing,
            receipt_committed: false,
            operations: journal_operations,
            staging_roots,
        };
        validate_journal_paths(&journal)?;
        self.persist_journal(&journal)?;

        let stage_result = (|| {
            for stage in journal.staging_roots.values() {
                let parent = stage.parent().ok_or_else(|| {
                    TransactionError::Journal("staging root has no parent".to_owned())
                })?;
                fs::create_dir(stage)
                    .map_err(|error| TransactionError::io("create staging root", stage, error))?;
                set_file_mode(stage, 0o700)?;
                mutation_completed(
                    injector,
                    mutation_ordinal,
                    None,
                    MutationPrimitive::CreateDirectory,
                )?;
                for child in ["prepared", "backups"] {
                    let directory = stage.join(child);
                    fs::create_dir(&directory).map_err(|error| {
                        TransactionError::io("create staging directory", &directory, error)
                    })?;
                    set_file_mode(&directory, 0o700)?;
                    mutation_completed(
                        injector,
                        mutation_ordinal,
                        None,
                        MutationPrimitive::CreateDirectory,
                    )?;
                }
                sync_directory(parent)?;
            }

            for entry in &journal.operations {
                match &entry.operation.payload {
                    OperationPayload::File { bytes, mode } => {
                        let path = entry.staged_path.as_ref().ok_or_else(|| {
                            TransactionError::InvalidOperation(
                                entry.operation.id.clone(),
                                "staged file path is missing",
                            )
                        })?;
                        write_new_file(path, bytes, *mode)?;
                        mutation_completed(
                            injector,
                            mutation_ordinal,
                            Some(&entry.operation.id),
                            MutationPrimitive::WriteStagedFile,
                        )?;
                        verify_snapshot(
                            path,
                            &entry.operation.expected_after,
                            &entry.operation.id,
                        )?;
                    }
                    OperationPayload::Symlink { target } => {
                        let path = entry.staged_path.as_ref().ok_or_else(|| {
                            TransactionError::InvalidOperation(
                                entry.operation.id.clone(),
                                "staged symlink path is missing",
                            )
                        })?;
                        create_symlink(target, path)?;
                        sync_parent(path)?;
                        mutation_completed(
                            injector,
                            mutation_ordinal,
                            Some(&entry.operation.id),
                            MutationPrimitive::CreateStagedSymlink,
                        )?;
                        verify_snapshot(
                            path,
                            &entry.operation.expected_after,
                            &entry.operation.id,
                        )?;
                    }
                    OperationPayload::Directory(mode) => {
                        let path = entry.staged_path.as_ref().ok_or_else(|| {
                            TransactionError::InvalidOperation(
                                entry.operation.id.clone(),
                                "staged directory path is missing",
                            )
                        })?;
                        fs::create_dir(path).map_err(|error| {
                            TransactionError::io("create staged directory", path, error)
                        })?;
                        set_file_mode(path, *mode)?;
                        sync_node(path)?;
                        mutation_completed(
                            injector,
                            mutation_ordinal,
                            Some(&entry.operation.id),
                            MutationPrimitive::CreateStagedDirectory,
                        )?;
                        verify_snapshot(
                            path,
                            &entry.operation.expected_after,
                            &entry.operation.id,
                        )?;
                    }
                    OperationPayload::Mode(_) | OperationPayload::None => {}
                    OperationPayload::Unavailable => {
                        return Err(TransactionError::InvalidOperation(
                            entry.operation.id.clone(),
                            "forward payload is unavailable",
                        ));
                    }
                }
            }
            for stage in journal.staging_roots.values() {
                sync_directory(&stage.join("prepared"))?;
                sync_directory(stage)?;
            }
            Ok(())
        })();

        if let Err(error) = stage_result {
            if self.cleanup(&journal).is_ok() {
                self.remove_journal()?;
                return Err(error);
            }
            journal.state = JournalState::RecoveryRequired;
            self.persist_journal(&journal)?;
            return Err(TransactionError::RecoveryRequired);
        }

        journal.state = JournalState::Prepared;
        self.persist_journal(&journal)?;
        Ok(journal)
    }

    fn apply_journal(
        &self,
        journal: &mut Journal,
        injector: &mut dyn FailureInjector,
        mutation_ordinal: &mut usize,
    ) -> Result<(), TransactionError> {
        let receipt_index = journal
            .operations
            .iter()
            .position(|entry| entry.operation.kind == OperationKind::WriteReceipt);
        for index in 0..journal.operations.len() {
            if Some(index) == receipt_index {
                journal.state = JournalState::Committing;
            } else {
                journal.state = JournalState::Applying;
            }
            self.persist_journal(journal)?;
            if let Some(code) = self.signal_flags.pending_exit_code() {
                return Err(TransactionError::Interrupted(code));
            }
            validate_operation_destination(&journal.operations[index].operation)?;
            let observed = snapshot_path(&journal.operations[index].operation.destination)?;
            if observed != journal.operations[index].operation.precondition {
                return Err(TransactionError::PreconditionsChanged {
                    operation_id: journal.operations[index].operation.id.clone(),
                    expected: Box::new(journal.operations[index].operation.precondition.clone()),
                    observed: Box::new(observed),
                });
            }
            journal.operations[index].state = OperationState::Applying;
            self.persist_journal(journal)?;
            apply_operation(&journal.operations[index], injector, mutation_ordinal)?;
            let applied_snapshot = verify_snapshot(
                &journal.operations[index].operation.destination,
                &journal.operations[index].operation.expected_after,
                &journal.operations[index].operation.id,
            )?;
            journal.operations[index].applied_snapshot = Some(applied_snapshot);
            journal.operations[index].state = OperationState::Applied;
            if Some(index) == receipt_index {
                journal.receipt_committed = true;
            }
            self.persist_journal(journal)?;
        }
        if receipt_index.is_none() {
            journal.state = JournalState::Committing;
            self.persist_journal(journal)?;
        }
        journal.state = JournalState::Committed;
        self.persist_journal(journal)
    }

    fn rollback(
        &self,
        journal: &mut Journal,
        injector: &mut dyn FailureInjector,
        mutation_ordinal: &mut usize,
    ) -> Result<(), TransactionError> {
        journal.state = JournalState::RollingBack;
        self.persist_journal(journal)?;
        for index in (0..journal.operations.len()).rev() {
            if journal.operations[index].state == OperationState::Prepared
                || journal.operations[index].state == OperationState::RolledBack
            {
                continue;
            }
            if let Err(error) =
                rollback_operation(&journal.operations[index], injector, mutation_ordinal)
            {
                journal.state = JournalState::RecoveryRequired;
                self.persist_journal(journal)?;
                return Err(error);
            }
            journal.operations[index].state = OperationState::RolledBack;
            self.persist_journal(journal)?;
        }
        journal.state = JournalState::RolledBack;
        self.persist_journal(journal)?;
        self.cleanup(journal)?;
        self.remove_journal()
    }

    fn persist_journal(&self, journal: &Journal) -> Result<(), TransactionError> {
        let bytes = serde_json::to_vec_pretty(journal)
            .map_err(|error| TransactionError::Journal(error.to_string()))?;
        let temporary = self.journal_temporary_path();
        write_new_durable_file(&temporary, &bytes, 0o600)?;
        fs::rename(&temporary, self.journal_path()).map_err(|error| {
            TransactionError::io("replace transaction journal", &self.journal_path(), error)
        })?;
        sync_directory(&self.state_directory)
    }

    fn read_journal(&self) -> Result<Journal, TransactionError> {
        let path = self.journal_path();
        validate_private_file(&path, 0o600)?;
        let bytes = fs::read(&path)
            .map_err(|error| TransactionError::io("read transaction journal", &path, error))?;
        let journal: Journal = serde_json::from_slice(&bytes)
            .map_err(|error| TransactionError::Journal(error.to_string()))?;
        if journal.schema_version != TRANSACTION_SCHEMA_VERSION {
            return Err(TransactionError::Journal(format!(
                "unsupported schema {}",
                journal.schema_version
            )));
        }
        validate_journal_paths(&journal)?;
        Ok(journal)
    }

    fn cleanup(&self, journal: &Journal) -> Result<(), TransactionError> {
        validate_journal_paths(journal)?;
        for entry in journal.operations.iter().rev() {
            if let Some(backup) = &entry.backup_path
                && path_exists(backup)?
            {
                let observed =
                    verify_snapshot(backup, &entry.operation.precondition, &entry.operation.id)?;
                remove_verified_path(backup, &observed)?;
            }
            if let Some(staged) = &entry.staged_path
                && path_exists(staged)?
            {
                let observed =
                    verify_snapshot(staged, &entry.operation.expected_after, &entry.operation.id)?;
                remove_verified_path(staged, &observed)?;
            }
        }
        for stage in journal.staging_roots.values() {
            if path_exists(stage)? {
                let metadata = fs::symlink_metadata(stage).map_err(|error| {
                    TransactionError::io("inspect transaction staging", stage, error)
                })?;
                if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                    return Err(TransactionError::Journal(format!(
                        "staging root is not a real directory: {}",
                        stage.display()
                    )));
                }
                for child in ["prepared", "backups"] {
                    let directory = stage.join(child);
                    if path_exists(&directory)? {
                        fs::remove_dir(&directory).map_err(|error| {
                            TransactionError::io(
                                "remove empty transaction staging directory",
                                &directory,
                                error,
                            )
                        })?;
                    }
                }
                fs::remove_dir(stage).map_err(|error| {
                    TransactionError::io("remove empty transaction staging", stage, error)
                })?;
                if let Some(parent) = stage.parent() {
                    sync_directory(parent)?;
                }
            }
        }
        Ok(())
    }

    fn discard_journal_temporary(&self) -> Result<bool, TransactionError> {
        let temporary = self.journal_temporary_path();
        if !path_exists(&temporary)? {
            return Ok(false);
        }
        validate_private_file(&temporary, 0o600)?;
        fs::remove_file(&temporary).map_err(|error| {
            TransactionError::io("discard incomplete transaction journal", &temporary, error)
        })?;
        sync_directory(&self.state_directory)?;
        Ok(true)
    }

    fn remove_journal(&self) -> Result<(), TransactionError> {
        let path = self.journal_path();
        if path_exists(&path)? {
            fs::remove_file(&path).map_err(|error| {
                TransactionError::io("remove transaction journal", &path, error)
            })?;
            sync_directory(&self.state_directory)?;
        }
        Ok(())
    }
}

fn validate_and_sort(operations: &mut [Operation]) -> Result<(), TransactionError> {
    let mut identifiers = BTreeMap::<String, ()>::new();
    let mut roots = BTreeMap::<String, RootSpec>::new();
    let mut receipt_count = 0;
    for operation in operations.iter() {
        operation.validate_for_execution()?;
        if identifiers.insert(operation.id.clone(), ()).is_some() {
            return Err(TransactionError::DuplicateOperation(operation.id.clone()));
        }
        if let Some(existing) = roots.insert(operation.root.id.clone(), operation.root.clone())
            && existing != operation.root
        {
            return Err(TransactionError::InvalidOperation(
                operation.id.clone(),
                "the same root identifier describes different roots",
            ));
        }
        if operation.kind == OperationKind::WriteReceipt {
            receipt_count += 1;
        }
    }
    if receipt_count != 1 {
        return Err(TransactionError::InvalidOperation(
            "write_receipt".to_owned(),
            "a transaction must contain exactly one receipt operation",
        ));
    }
    operations.sort_by(|left, right| {
        let left_receipt = left.kind == OperationKind::WriteReceipt;
        let right_receipt = right.kind == OperationKind::WriteReceipt;
        left_receipt
            .cmp(&right_receipt)
            .then_with(|| left.root.cmp(&right.root))
    });
    Ok(())
}

fn validate_journal_paths(journal: &Journal) -> Result<(), TransactionError> {
    validate_transaction_id(&journal.transaction_id)?;
    let mut roots = BTreeMap::<&str, &RootSpec>::new();
    let mut identifiers = BTreeMap::new();
    let mut receipt_index = None;
    for (index, entry) in journal.operations.iter().enumerate() {
        entry.operation.validate_for_execution()?;
        if identifiers
            .insert(entry.operation.id.as_str(), ())
            .is_some()
        {
            return Err(TransactionError::DuplicateOperation(
                entry.operation.id.clone(),
            ));
        }
        if entry.operation.kind == OperationKind::WriteReceipt
            && receipt_index.replace(index).is_some()
        {
            return Err(TransactionError::Journal(
                "journal contains more than one receipt operation".to_owned(),
            ));
        }
        if let Some(existing) = roots.insert(&entry.operation.root.id, &entry.operation.root)
            && existing != &entry.operation.root
        {
            return Err(TransactionError::Journal(format!(
                "root {} changes identity within the journal",
                entry.operation.root.id
            )));
        }
        let stage = journal
            .staging_roots
            .get(&entry.operation.root.id)
            .ok_or_else(|| {
                TransactionError::Journal(format!(
                    "operation {} has no staging root",
                    entry.operation.id
                ))
            })?;
        let parent = entry.operation.root.path.parent().ok_or_else(|| {
            TransactionError::Journal(format!(
                "root {} has no filesystem parent",
                entry.operation.root.id
            ))
        })?;
        let expected_stage = parent.join(format!(
            ".arthur-workflow-{}-{}.stage",
            journal.transaction_id, entry.operation.root.id
        ));
        if *stage != expected_stage {
            return Err(TransactionError::Journal(format!(
                "staging path for root {} is not the deterministic sibling",
                entry.operation.root.id
            )));
        }
        let expects_staged = matches!(
            entry.operation.kind,
            OperationKind::EnsureDirectory
                | OperationKind::WriteFile
                | OperationKind::ReplaceFile
                | OperationKind::CreateSymlink
                | OperationKind::RewriteLegacyLock
                | OperationKind::WriteReceipt
        );
        let expected_staged =
            expects_staged.then(|| stage.join("prepared").join(format!("{index}.new")));
        if entry.staged_path != expected_staged {
            return Err(TransactionError::Journal(format!(
                "operation {} has an invalid staged path",
                entry.operation.id
            )));
        }
        let expects_backup = matches!(&entry.operation.inverse, Inverse::RestoreBackup { .. });
        let expected_backup =
            expects_backup.then(|| stage.join("backups").join(format!("{index}.old")));
        if entry.backup_path != expected_backup {
            return Err(TransactionError::Journal(format!(
                "operation {} has an invalid backup path",
                entry.operation.id
            )));
        }
        if entry.state == OperationState::Prepared && entry.applied_snapshot.is_some() {
            return Err(TransactionError::Journal(format!(
                "prepared operation {} has an applied identity",
                entry.operation.id
            )));
        }
        if entry.state == OperationState::Applied && entry.applied_snapshot.is_none() {
            return Err(TransactionError::Journal(format!(
                "applied operation {} has no durable identity",
                entry.operation.id
            )));
        }
        if let Some(applied) = &entry.applied_snapshot
            && !snapshot_matches_expected(applied, &entry.operation.expected_after)
        {
            return Err(TransactionError::Journal(format!(
                "operation {} has an invalid applied identity",
                entry.operation.id
            )));
        }
    }
    if roots.len() != journal.staging_roots.len() {
        return Err(TransactionError::Journal(
            "journal contains an unused staging root".to_owned(),
        ));
    }
    if receipt_index.is_none() || receipt_index != journal.operations.len().checked_sub(1) {
        return Err(TransactionError::Journal(
            "journal receipt operation is missing or is not last".to_owned(),
        ));
    }
    Ok(())
}

fn validate_recovery_roots(
    journal: &Journal,
    trusted_roots: &[RootSpec],
) -> Result<(), TransactionError> {
    let mut trusted = BTreeMap::new();
    for root in trusted_roots {
        if trusted.insert(root.id.as_str(), root).is_some() {
            return Err(TransactionError::Journal(format!(
                "trusted root {} is duplicated",
                root.id
            )));
        }
        validate_root_device(root)?;
    }
    let journal_roots = journal
        .operations
        .iter()
        .map(|entry| (&entry.operation.root.id, &entry.operation.root))
        .collect::<BTreeMap<_, _>>();
    if journal_roots.len() > trusted.len()
        || journal_roots.iter().any(|(id, root)| {
            trusted
                .get(id.as_str())
                .is_none_or(|trusted_root| *trusted_root != *root)
        })
    {
        return Err(TransactionError::Journal(
            "journal roots do not match the current trusted environment".to_owned(),
        ));
    }
    Ok(())
}

fn apply_operation(
    entry: &JournalOperation,
    injector: &mut dyn FailureInjector,
    mutation_ordinal: &mut usize,
) -> Result<(), TransactionError> {
    let operation = &entry.operation;
    match operation.kind {
        OperationKind::EnsureDirectory
        | OperationKind::WriteFile
        | OperationKind::ReplaceFile
        | OperationKind::RewriteLegacyLock
        | OperationKind::WriteReceipt
        | OperationKind::CreateSymlink => {
            if operation.kind == OperationKind::RewriteLegacyLock {
                let proof = operation.revalidation.as_ref().ok_or_else(|| {
                    TransactionError::InvalidOperation(
                        operation.id.clone(),
                        "legacy-lock identity proof is missing",
                    )
                })?;
                revalidate_file_identity(&operation.destination, proof)?;
            }
            if operation.precondition.kind != PathKind::Absent {
                move_to_backup(entry, injector, mutation_ordinal)?;
            }
            let staged = entry.staged_path.as_ref().ok_or_else(|| {
                TransactionError::InvalidOperation(
                    operation.id.clone(),
                    "staged payload is missing",
                )
            })?;
            match operation.kind {
                OperationKind::WriteFile
                | OperationKind::ReplaceFile
                | OperationKind::RewriteLegacyLock
                | OperationKind::WriteReceipt => {
                    link_staged_no_replace(staged, operation)?;
                }
                OperationKind::EnsureDirectory | OperationKind::CreateSymlink => {
                    rename_staged_no_replace(staged, operation)?;
                }
                OperationKind::SetMode | OperationKind::RemoveOwnedPath => {
                    return Err(TransactionError::InvalidOperation(
                        operation.id.clone(),
                        "operation cannot install a staged payload",
                    ));
                }
            }
            mutation_completed(
                injector,
                mutation_ordinal,
                Some(&operation.id),
                MutationPrimitive::InstallNoReplace,
            )?;
            Ok(())
        }
        OperationKind::SetMode => {
            let OperationPayload::Mode(mode) = &operation.payload else {
                return Err(TransactionError::InvalidOperation(
                    operation.id.clone(),
                    "replacement mode is unavailable",
                ));
            };
            set_destination_mode(operation, *mode)?;
            mutation_completed(
                injector,
                mutation_ordinal,
                Some(&operation.id),
                MutationPrimitive::SetMode,
            )?;
            Ok(())
        }
        OperationKind::RemoveOwnedPath => {
            move_to_backup(entry, injector, mutation_ordinal)?;
            Ok(())
        }
    }
}

fn move_to_backup(
    entry: &JournalOperation,
    injector: &mut dyn FailureInjector,
    mutation_ordinal: &mut usize,
) -> Result<(), TransactionError> {
    if entry.operation.precondition.kind == PathKind::Directory {
        let mut children = fs::read_dir(&entry.operation.destination).map_err(|error| {
            TransactionError::io(
                "inspect managed directory before backup",
                &entry.operation.destination,
                error,
            )
        })?;
        if children
            .next()
            .transpose()
            .map_err(|error| {
                TransactionError::io(
                    "inspect managed directory entry",
                    &entry.operation.destination,
                    error,
                )
            })?
            .is_some()
        {
            return Err(TransactionError::PreconditionsChanged {
                operation_id: entry.operation.id.clone(),
                expected: Box::new(entry.operation.precondition.clone()),
                observed: Box::new(snapshot_path(&entry.operation.destination)?),
            });
        }
    }
    let backup = entry.backup_path.as_ref().ok_or_else(|| {
        TransactionError::InvalidOperation(
            entry.operation.id.clone(),
            "backup path is missing for a destructive operation",
        )
    })?;
    rename_destination_to_backup(&entry.operation, backup)?;
    mutation_completed(
        injector,
        mutation_ordinal,
        Some(&entry.operation.id),
        MutationPrimitive::Rename,
    )?;
    verify_snapshot(backup, &entry.operation.precondition, &entry.operation.id)?;
    Ok(())
}

fn rollback_operation(
    entry: &JournalOperation,
    injector: &mut dyn FailureInjector,
    mutation_ordinal: &mut usize,
) -> Result<(), TransactionError> {
    let operation = &entry.operation;
    validate_operation_destination(operation)?;
    match &operation.inverse {
        Inverse::RemoveCreated => {
            let observed = snapshot_path(&operation.destination)?;
            if observed.kind == PathKind::Absent {
                return Ok(());
            }
            if !created_path_is_transaction_owned(entry, &observed)? {
                return Err(TransactionError::PreconditionsChanged {
                    operation_id: operation.id.clone(),
                    expected: Box::new(operation.expected_after.clone()),
                    observed: Box::new(observed),
                });
            }
            remove_destination(operation, &observed)?;
            mutation_completed(
                injector,
                mutation_ordinal,
                Some(&operation.id),
                MutationPrimitive::Remove,
            )?;
            Ok(())
        }
        Inverse::RestoreBackup { original } => {
            let backup = entry.backup_path.as_ref().ok_or_else(|| {
                TransactionError::InvalidOperation(
                    operation.id.clone(),
                    "rollback backup is missing",
                )
            })?;
            let backup_snapshot = snapshot_path(backup)?;
            if backup_snapshot.kind == PathKind::Absent {
                if snapshot_path(&operation.destination)? == *original {
                    return Ok(());
                }
                return Err(TransactionError::RecoveryRequired);
            }
            if backup_snapshot != *original {
                return Err(TransactionError::PreconditionsChanged {
                    operation_id: operation.id.clone(),
                    expected: Box::new(original.clone()),
                    observed: Box::new(backup_snapshot),
                });
            }
            let current = snapshot_path(&operation.destination)?;
            if current.kind != PathKind::Absent {
                if !created_path_is_transaction_owned(entry, &current)? {
                    return Err(TransactionError::PreconditionsChanged {
                        operation_id: operation.id.clone(),
                        expected: Box::new(operation.expected_after.clone()),
                        observed: Box::new(current),
                    });
                }
                remove_destination(operation, &current)?;
                mutation_completed(
                    injector,
                    mutation_ordinal,
                    Some(&operation.id),
                    MutationPrimitive::Remove,
                )?;
            }
            rename_backup_to_destination(backup, operation)?;
            mutation_completed(
                injector,
                mutation_ordinal,
                Some(&operation.id),
                MutationPrimitive::Rename,
            )?;
            verify_snapshot(&operation.destination, original, &operation.id)?;
            Ok(())
        }
        Inverse::RestoreMode { mode } => {
            let observed = snapshot_path(&operation.destination)?;
            if observed.kind == PathKind::Absent {
                return Err(TransactionError::RecoveryRequired);
            }
            if !snapshot_matches_expected(&observed, &operation.expected_after) {
                return Err(TransactionError::PreconditionsChanged {
                    operation_id: operation.id.clone(),
                    expected: Box::new(operation.expected_after.clone()),
                    observed: Box::new(observed),
                });
            }
            set_destination_mode(operation, *mode)?;
            mutation_completed(
                injector,
                mutation_ordinal,
                Some(&operation.id),
                MutationPrimitive::SetMode,
            )?;
            verify_snapshot(
                &operation.destination,
                &operation.precondition,
                &operation.id,
            )?;
            Ok(())
        }
        Inverse::None => Ok(()),
    }
}

fn receipt_is_committed(journal: &Journal) -> Result<bool, TransactionError> {
    if journal.receipt_committed {
        return Ok(true);
    }
    let Some(receipt) = journal
        .operations
        .iter()
        .find(|entry| entry.operation.kind == OperationKind::WriteReceipt)
    else {
        return Ok(false);
    };
    let observed = snapshot_path(&receipt.operation.destination)?;
    Ok(snapshot_matches_expected(
        &observed,
        &receipt.operation.expected_after,
    ))
}

fn mutation_completed(
    injector: &mut dyn FailureInjector,
    ordinal: &mut usize,
    operation_id: Option<&str>,
    primitive: MutationPrimitive,
) -> Result<(), TransactionError> {
    *ordinal += 1;
    injector
        .after_mutation(&MutationPoint {
            ordinal: *ordinal,
            operation_id: operation_id.map(str::to_owned),
            primitive,
        })
        .map_err(TransactionError::InjectedFailure)
}

fn verify_snapshot(
    path: &Path,
    expected: &PathSnapshot,
    operation_id: &str,
) -> Result<PathSnapshot, TransactionError> {
    let observed = snapshot_path(path)?;
    if snapshot_matches_expected(&observed, expected) {
        return Ok(observed);
    }
    Err(TransactionError::PreconditionsChanged {
        operation_id: operation_id.to_owned(),
        expected: Box::new(expected.clone()),
        observed: Box::new(observed),
    })
}

fn write_new_file(path: &Path, bytes: &[u8], mode: u32) -> Result<(), TransactionError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .map_err(|error| TransactionError::io("create staged file", path, error))?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| TransactionError::io("set staged file mode", path, error))?;
    file.write_all(bytes)
        .map_err(|error| TransactionError::io("write staged file", path, error))?;
    file.flush()
        .map_err(|error| TransactionError::io("flush staged file", path, error))?;
    file.sync_all()
        .map_err(|error| TransactionError::io("fsync staged file", path, error))?;
    sync_parent(path)
}

fn write_new_durable_file(path: &Path, bytes: &[u8], mode: u32) -> Result<(), TransactionError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(mode);
    let mut file = options
        .open(path)
        .map_err(|error| TransactionError::io("create durable file", path, error))?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| TransactionError::io("set durable file mode", path, error))?;
    file.write_all(bytes)
        .map_err(|error| TransactionError::io("write durable file", path, error))?;
    file.flush()
        .map_err(|error| TransactionError::io("flush durable file", path, error))?;
    file.sync_all()
        .map_err(|error| TransactionError::io("fsync durable file", path, error))
}

fn ensure_private_directory(path: &Path) -> Result<(), TransactionError> {
    require_utf8(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir()
                || metadata.file_type().is_symlink()
                || metadata_mode(&metadata) != 0o700
            {
                return Err(TransactionError::InsecureStatePath(path.to_path_buf()));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|error| {
                TransactionError::io("create transaction state directory", path, error)
            })?;
            set_file_mode(path, 0o700)?;
            sync_parent(path)
        }
        Err(error) => Err(TransactionError::io(
            "inspect transaction state directory",
            path,
            error,
        )),
    }
}

fn path_exists(path: &Path) -> Result<bool, TransactionError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(TransactionError::io("inspect path existence", path, error)),
    }
}

fn validate_private_file(path: &Path, mode: u32) -> Result<(), TransactionError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| TransactionError::io("inspect private state file", path, error))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata_mode(&metadata) != mode
    {
        return Err(TransactionError::InsecureStatePath(path.to_path_buf()));
    }
    Ok(())
}

fn metadata_is_stable(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    metadata_device(before) == metadata_device(after)
        && metadata_inode(before) == metadata_inode(after)
        && before.len() == after.len()
        && metadata_mode(before) == metadata_mode(after)
        && metadata_mtime_seconds(before) == metadata_mtime_seconds(after)
        && metadata_mtime_nanoseconds(before) == metadata_mtime_nanoseconds(after)
        && before.file_type().is_file() == after.file_type().is_file()
        && before.file_type().is_dir() == after.file_type().is_dir()
        && before.file_type().is_symlink() == after.file_type().is_symlink()
}

fn snapshot_matches_expected(observed: &PathSnapshot, expected: &PathSnapshot) -> bool {
    observed.kind == expected.kind
        && observed.sha256 == expected.sha256
        && observed.mode == expected.mode
        && observed.link_target == expected.link_target
        && observed.size == expected.size
        && expected
            .device
            .is_none_or(|device| observed.device == Some(device))
        && expected
            .inode
            .is_none_or(|inode| observed.inode == Some(inode))
}

struct VerifiedParent {
    directory: File,
    leaf: OsString,
}

fn verified_destination_parent(operation: &Operation) -> Result<VerifiedParent, TransactionError> {
    validate_operation_destination(operation)?;
    let relative = operation
        .destination
        .strip_prefix(&operation.root.path)
        .map_err(|_| TransactionError::InvalidPath(operation.destination.clone()))?;
    let expected_destination = operation.root.real.join(relative);
    let expected_parent = expected_destination
        .parent()
        .ok_or_else(|| TransactionError::InvalidPath(operation.destination.clone()))?;
    stable_parent(&operation.destination, Some(expected_parent))
}

fn stable_parent(
    path: &Path,
    expected_real_parent: Option<&Path>,
) -> Result<VerifiedParent, TransactionError> {
    let parent = path
        .parent()
        .ok_or_else(|| TransactionError::InvalidPath(path.to_path_buf()))?;
    let leaf = path
        .file_name()
        .ok_or_else(|| TransactionError::InvalidPath(path.to_path_buf()))?
        .to_os_string();
    require_utf8(parent)?;
    require_utf8(Path::new(&leaf))?;
    let before = fs::symlink_metadata(parent)
        .map_err(|error| TransactionError::io("inspect mutation parent", parent, error))?;
    if !before.file_type().is_dir() || before.file_type().is_symlink() {
        return Err(TransactionError::RootIdentityChanged {
            root: parent.to_path_buf(),
        });
    }
    let directory = File::open(parent)
        .map_err(|error| TransactionError::io("open mutation parent", parent, error))?;
    let opened = directory
        .metadata()
        .map_err(|error| TransactionError::io("inspect open mutation parent", parent, error))?;
    let after = fs::symlink_metadata(parent)
        .map_err(|error| TransactionError::io("reinspect mutation parent", parent, error))?;
    if !opened.file_type().is_dir()
        || !after.file_type().is_dir()
        || after.file_type().is_symlink()
        || !metadata_is_stable(&before, &opened)
        || !metadata_is_stable(&opened, &after)
    {
        return Err(TransactionError::RootIdentityChanged {
            root: parent.to_path_buf(),
        });
    }
    if let Some(expected) = expected_real_parent {
        let real = fs::canonicalize(parent)
            .map_err(|error| TransactionError::io("resolve mutation parent", parent, error))?;
        if real != expected {
            return Err(TransactionError::RootIdentityChanged {
                root: parent.to_path_buf(),
            });
        }
    }
    Ok(VerifiedParent { directory, leaf })
}

fn rename_staged_no_replace(staged: &Path, operation: &Operation) -> Result<(), TransactionError> {
    let source = stable_parent(staged, None)?;
    let destination = verified_destination_parent(operation)?;
    rustix::fs::renameat_with(
        &source.directory,
        &source.leaf,
        &destination.directory,
        &destination.leaf,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        TransactionError::io(
            "install staged path without replacement",
            &operation.destination,
            error.into(),
        )
    })?;
    source
        .directory
        .sync_all()
        .map_err(|error| TransactionError::io("fsync staging parent", staged, error))?;
    destination.directory.sync_all().map_err(|error| {
        TransactionError::io("fsync destination parent", &operation.destination, error)
    })
}

fn link_staged_no_replace(staged: &Path, operation: &Operation) -> Result<(), TransactionError> {
    let source = stable_parent(staged, None)?;
    let destination = verified_destination_parent(operation)?;
    rustix::fs::linkat(
        &source.directory,
        &source.leaf,
        &destination.directory,
        &destination.leaf,
        AtFlags::empty(),
    )
    .map_err(|error| {
        TransactionError::io(
            "install staged file without replacement",
            &operation.destination,
            error.into(),
        )
    })?;
    destination.directory.sync_all().map_err(|error| {
        TransactionError::io("fsync destination parent", &operation.destination, error)
    })
}

fn rename_destination_to_backup(
    operation: &Operation,
    backup: &Path,
) -> Result<(), TransactionError> {
    let source = verified_destination_parent(operation)?;
    let destination = stable_parent(backup, None)?;
    rustix::fs::renameat_with(
        &source.directory,
        &source.leaf,
        &destination.directory,
        &destination.leaf,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| TransactionError::io("move path to backup", backup, error.into()))?;
    source.directory.sync_all().map_err(|error| {
        TransactionError::io("fsync destination parent", &operation.destination, error)
    })?;
    destination
        .directory
        .sync_all()
        .map_err(|error| TransactionError::io("fsync backup parent", backup, error))
}

fn rename_backup_to_destination(
    backup: &Path,
    operation: &Operation,
) -> Result<(), TransactionError> {
    let source = stable_parent(backup, None)?;
    let destination = verified_destination_parent(operation)?;
    rustix::fs::renameat_with(
        &source.directory,
        &source.leaf,
        &destination.directory,
        &destination.leaf,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        TransactionError::io(
            "restore transaction backup",
            &operation.destination,
            error.into(),
        )
    })?;
    source
        .directory
        .sync_all()
        .map_err(|error| TransactionError::io("fsync backup parent", backup, error))?;
    destination.directory.sync_all().map_err(|error| {
        TransactionError::io("fsync destination parent", &operation.destination, error)
    })
}

fn remove_destination(
    operation: &Operation,
    expected: &PathSnapshot,
) -> Result<(), TransactionError> {
    let parent = verified_destination_parent(operation)?;
    unlink_verified(&parent, &operation.destination, expected)
}

fn remove_verified_path(path: &Path, expected: &PathSnapshot) -> Result<(), TransactionError> {
    let parent = stable_parent(path, None)?;
    unlink_verified(&parent, path, expected)
}

fn unlink_verified(
    parent: &VerifiedParent,
    path: &Path,
    expected: &PathSnapshot,
) -> Result<(), TransactionError> {
    let stat = rustix::fs::statat(&parent.directory, &parent.leaf, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| {
            TransactionError::io(
                "revalidate transaction-owned destination",
                path,
                error.into(),
            )
        })?;
    if expected.device != numeric_u64(stat.st_dev) || expected.inode != numeric_u64(stat.st_ino) {
        return Err(TransactionError::ConcurrentFilesystemChange(
            path.to_path_buf(),
        ));
    }
    let flags = if expected.kind == PathKind::Directory {
        AtFlags::REMOVEDIR
    } else {
        AtFlags::empty()
    };
    rustix::fs::unlinkat(&parent.directory, &parent.leaf, flags).map_err(|error| {
        TransactionError::io("remove transaction-owned destination", path, error.into())
    })?;
    parent
        .directory
        .sync_all()
        .map_err(|error| TransactionError::io("fsync destination parent", path, error))
}

fn set_destination_mode(operation: &Operation, mode: u32) -> Result<(), TransactionError> {
    let parent = verified_destination_parent(operation)?;
    let mut flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    if operation.precondition.kind == PathKind::Directory {
        flags |= OFlags::DIRECTORY;
    }
    let node = rustix::fs::openat(&parent.directory, &parent.leaf, flags, Mode::empty()).map_err(
        |error| {
            TransactionError::io(
                "open destination for mode change",
                &operation.destination,
                error.into(),
            )
        },
    )?;
    let stat = rustix::fs::fstat(&node).map_err(|error| {
        TransactionError::io(
            "revalidate destination for mode change",
            &operation.destination,
            error.into(),
        )
    })?;
    if operation.precondition.device != numeric_u64(stat.st_dev)
        || operation.precondition.inode != numeric_u64(stat.st_ino)
    {
        return Err(TransactionError::ConcurrentFilesystemChange(
            operation.destination.clone(),
        ));
    }
    rustix::fs::fchmod(&node, Mode::from_raw_mode(mode)).map_err(|error| {
        TransactionError::io("set destination mode", &operation.destination, error.into())
    })?;
    rustix::fs::fsync(&node).map_err(|error| {
        TransactionError::io("fsync destination", &operation.destination, error.into())
    })?;
    parent.directory.sync_all().map_err(|error| {
        TransactionError::io("fsync destination parent", &operation.destination, error)
    })
}

fn created_path_is_transaction_owned(
    entry: &JournalOperation,
    observed: &PathSnapshot,
) -> Result<bool, TransactionError> {
    if let Some(applied) = &entry.applied_snapshot {
        return Ok(observed == applied);
    }
    let Some(staged) = &entry.staged_path else {
        return Ok(false);
    };
    match observed.kind {
        PathKind::File => Ok(snapshot_path(staged)? == *observed),
        PathKind::Directory | PathKind::Symlink => Ok(snapshot_path(staged)?.kind
            == PathKind::Absent
            && snapshot_matches_expected(observed, &entry.operation.expected_after)),
        PathKind::Absent => Ok(false),
    }
}

fn numeric_u64<T>(value: T) -> Option<u64>
where
    T: TryInto<u64>,
{
    value.try_into().ok()
}

#[cfg(test)]
fn remove_path(path: &Path) -> Result<(), TransactionError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| TransactionError::io("inspect path for removal", path, error))?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir(path).map_err(|error| TransactionError::io("remove directory", path, error))
    } else {
        fs::remove_file(path).map_err(|error| TransactionError::io("remove file", path, error))
    }
}

#[cfg(unix)]
fn create_symlink(target: &Path, path: &Path) -> Result<(), TransactionError> {
    std::os::unix::fs::symlink(target, path)
        .map_err(|error| TransactionError::io("create staged symlink", path, error))
}

#[cfg(not(unix))]
fn create_symlink(_target: &Path, path: &Path) -> Result<(), TransactionError> {
    Err(TransactionError::InvalidOperation(
        path.display().to_string(),
        "symlink operations are unsupported on this platform",
    ))
}

#[cfg(unix)]
fn metadata_mode(metadata: &fs::Metadata) -> u32 {
    metadata.mode() & 0o7777
}

#[cfg(not(unix))]
fn metadata_mode(metadata: &fs::Metadata) -> u32 {
    if metadata.permissions().readonly() {
        0o444
    } else {
        0o644
    }
}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) -> Result<(), TransactionError> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| TransactionError::io("set POSIX mode", path, error))
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) -> Result<(), TransactionError> {
    Ok(())
}

fn sync_parent(path: &Path) -> Result<(), TransactionError> {
    let parent = path
        .parent()
        .ok_or_else(|| TransactionError::InvalidPath(path.to_path_buf()))?;
    sync_directory(parent)
}

fn sync_node(path: &Path) -> Result<(), TransactionError> {
    let node = File::open(path)
        .map_err(|error| TransactionError::io("open path for fsync", path, error))?;
    node.sync_all()
        .map_err(|error| TransactionError::io("fsync path", path, error))
}

fn sync_directory(path: &Path) -> Result<(), TransactionError> {
    let directory = File::open(path)
        .map_err(|error| TransactionError::io("open directory for fsync", path, error))?;
    directory
        .sync_all()
        .map_err(|error| TransactionError::io("fsync directory", path, error))
}

fn validate_root_device(root: &RootSpec) -> Result<(), TransactionError> {
    let existing = nearest_existing(&root.path)?;
    let metadata = fs::metadata(existing)
        .map_err(|error| TransactionError::io("inspect root device", existing, error))?;
    let observed = metadata_device(&metadata);
    if observed != root.device {
        return Err(TransactionError::DeviceMismatch {
            root: root.path.clone(),
            expected: root.device,
            observed,
        });
    }
    if project_real_path(&root.path)? != root.real {
        return Err(TransactionError::RootIdentityChanged {
            root: root.path.clone(),
        });
    }
    Ok(())
}

fn validate_operation_destination(operation: &Operation) -> Result<(), TransactionError> {
    validate_root_device(&operation.root)?;
    let projected = project_destination_path(&operation.destination)?;
    if !projected.starts_with(&operation.root.real) {
        return Err(TransactionError::RootIdentityChanged {
            root: operation.root.path.clone(),
        });
    }
    Ok(())
}

fn project_destination_path(path: &Path) -> Result<PathBuf, TransactionError> {
    let parent = path
        .parent()
        .ok_or_else(|| TransactionError::InvalidPath(path.to_path_buf()))?;
    let existing = nearest_existing(parent)?;
    let real_existing = fs::canonicalize(existing)
        .map_err(|error| TransactionError::io("resolve destination identity", existing, error))?;
    let suffix = path
        .strip_prefix(existing)
        .map_err(|_| TransactionError::InvalidPath(path.to_path_buf()))?;
    Ok(real_existing.join(suffix))
}

fn project_real_path(path: &Path) -> Result<PathBuf, TransactionError> {
    let existing = nearest_existing(path)?;
    let real_existing = fs::canonicalize(existing)
        .map_err(|error| TransactionError::io("resolve filesystem identity", existing, error))?;
    let suffix = path
        .strip_prefix(existing)
        .map_err(|_| TransactionError::InvalidPath(path.to_path_buf()))?;
    Ok(real_existing.join(suffix))
}

fn nearest_existing(path: &Path) -> Result<&Path, TransactionError> {
    path.ancestors()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| TransactionError::InvalidPath(path.to_path_buf()))
}

#[cfg(unix)]
fn metadata_device(metadata: &fs::Metadata) -> u64 {
    metadata.dev()
}

#[cfg(not(unix))]
fn metadata_device(_metadata: &fs::Metadata) -> u64 {
    0
}

fn require_utf8(path: &Path) -> Result<(), TransactionError> {
    if path.to_str().is_none() {
        return Err(TransactionError::InvalidPath(path.to_path_buf()));
    }
    Ok(())
}

fn is_normalized_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
}

fn validate_transaction_id(transaction_id: &str) -> Result<(), TransactionError> {
    if !is_safe_identifier(transaction_id) {
        return Err(TransactionError::InvalidOperation(
            transaction_id.to_owned(),
            "transaction identifier must be non-empty ASCII alphanumeric, '-' or '_'",
        ));
    }
    Ok(())
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn revalidate_file_identity(
    path: &Path,
    expected: &FileIdentityProof,
) -> Result<(), TransactionError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| TransactionError::io("revalidate legacy lock", path, error))?;
    if !metadata.file_type().is_file() {
        return Err(TransactionError::LegacyLockChanged(path.to_path_buf()));
    }
    let snapshot = snapshot_path(path)?;
    if metadata_device(&metadata) != expected.device
        || metadata_inode(&metadata) != expected.inode
        || metadata.len() != expected.size
        || metadata_mtime_seconds(&metadata) != expected.mtime_seconds
        || metadata_mtime_nanoseconds(&metadata) != expected.mtime_nanoseconds
        || snapshot.sha256.as_deref() != Some(expected.sha256.as_str())
    {
        return Err(TransactionError::LegacyLockChanged(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_inode(metadata: &fs::Metadata) -> u64 {
    metadata.ino()
}

#[cfg(not(unix))]
fn metadata_inode(_metadata: &fs::Metadata) -> u64 {
    0
}

#[cfg(unix)]
fn metadata_mtime_seconds(metadata: &fs::Metadata) -> i64 {
    metadata.mtime()
}

#[cfg(not(unix))]
fn metadata_mtime_seconds(_metadata: &fs::Metadata) -> i64 {
    0
}

#[cfg(unix)]
fn metadata_mtime_nanoseconds(metadata: &fs::Metadata) -> i64 {
    metadata.mtime_nsec()
}

#[cfg(not(unix))]
fn metadata_mtime_nanoseconds(_metadata: &fs::Metadata) -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};

    use nix::sys::stat::Mode as NixMode;
    use nix::unistd::mkfifo;
    use tempfile::TempDir;

    use super::{
        FailAfterMutation, FailureInjector, Inverse, JournalState, MutationPoint,
        MutationPrimitive, NoFailures, Operation, OperationKind, OperationPayload, OperationState,
        OwnershipProof, PathSnapshot, RootSpec, SIGINT, SIGINT_EXIT_CODE, SIGTERM,
        SIGTERM_EXIT_CODE, SignalFlags, TransactionEngine, TransactionError, TransactionLock,
        TransactionOutcome, apply_operation, ensure_private_directory, metadata_device,
        receipt_is_committed, snapshot_path, validate_and_sort, validate_journal_paths,
    };

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn root(directory: &TempDir, name: &str) -> TestResult<RootSpec> {
        let path = directory.path().join(name);
        fs::create_dir_all(&path)?;
        let device = metadata_device(&fs::metadata(&path)?);
        Ok(RootSpec::new(name, path, device))
    }

    fn create_file(
        id: &str,
        root: &RootSpec,
        relative: &str,
        bytes: &[u8],
    ) -> Result<Operation, TransactionError> {
        Operation::new(
            id,
            OperationKind::WriteFile,
            root.clone(),
            root.path.join(relative),
            PathSnapshot::absent(),
            PathSnapshot::file(bytes, 0o644),
            Inverse::RemoveCreated,
            OwnershipProof::UnownedDestination,
            OperationPayload::File {
                bytes: bytes.to_vec(),
                mode: 0o644,
            },
        )
    }

    fn replace_file(
        id: &str,
        root: &RootSpec,
        relative: &str,
        before: PathSnapshot,
        bytes: &[u8],
    ) -> Result<Operation, TransactionError> {
        Operation::new(
            id,
            OperationKind::ReplaceFile,
            root.clone(),
            root.path.join(relative),
            before.clone(),
            PathSnapshot::file(bytes, 0o644),
            Inverse::RestoreBackup { original: before },
            OwnershipProof::Receipt {
                source_id: id.to_owned(),
                sha256: None,
            },
            OperationPayload::File {
                bytes: bytes.to_vec(),
                mode: 0o644,
            },
        )
    }

    fn receipt(root: &RootSpec, bytes: &[u8]) -> Result<Operation, TransactionError> {
        Operation::new(
            "receipt",
            OperationKind::WriteReceipt,
            root.clone(),
            root.path.join("receipt.json"),
            PathSnapshot::absent(),
            PathSnapshot::file(bytes, 0o600),
            Inverse::RemoveCreated,
            OwnershipProof::TransactionState,
            OperationPayload::File {
                bytes: bytes.to_vec(),
                mode: 0o600,
            },
        )
    }

    fn set_mode_operation(
        root: &RootSpec,
        destination: &Path,
        before: PathSnapshot,
        mode: u32,
    ) -> Result<Operation, TransactionError> {
        let mut after = before.clone();
        after.mode = Some(mode);
        Operation::new(
            "set-mode",
            OperationKind::SetMode,
            root.clone(),
            destination.to_path_buf(),
            before.clone(),
            after,
            Inverse::RestoreMode {
                mode: before.mode.unwrap_or(0o600),
            },
            OwnershipProof::Receipt {
                source_id: "mode-file".to_owned(),
                sha256: before.sha256.clone(),
            },
            OperationPayload::Mode(mode),
        )
    }

    fn remove_owned_operation(
        root: &RootSpec,
        destination: &Path,
        before: PathSnapshot,
    ) -> Result<Operation, TransactionError> {
        Operation::new(
            "remove-owned",
            OperationKind::RemoveOwnedPath,
            root.clone(),
            destination.to_path_buf(),
            before.clone(),
            PathSnapshot::absent(),
            Inverse::RestoreBackup { original: before },
            OwnershipProof::Receipt {
                source_id: "removed".to_owned(),
                sha256: None,
            },
            OperationPayload::None,
        )
    }

    fn engine(directory: &TempDir) -> TransactionEngine {
        TransactionEngine::new(directory.path().join("state"), SignalFlags::default())
    }

    fn prepare_journal(
        engine: &TransactionEngine,
        transaction_id: &str,
        mut operations: Vec<Operation>,
    ) -> Result<super::Journal, TransactionError> {
        ensure_private_directory(&engine.state_directory)?;
        validate_and_sort(&mut operations)?;
        let mut ordinal = 0;
        let mut injector = NoFailures;
        let journal = engine.prepare(
            transaction_id.to_owned(),
            operations,
            &mut injector,
            &mut ordinal,
        )?;
        engine.persist_journal(&journal)?;
        Ok(journal)
    }

    fn apply_entry(
        engine: &TransactionEngine,
        journal: &mut super::Journal,
        index: usize,
        state: JournalState,
    ) -> Result<(), TransactionError> {
        journal.state = state;
        journal.operations[index].state = OperationState::Applying;
        engine.persist_journal(journal)?;
        let mut injector = NoFailures;
        let mut ordinal = 0;
        apply_operation(&journal.operations[index], &mut injector, &mut ordinal)?;
        journal.operations[index].applied_snapshot = Some(snapshot_path(
            &journal.operations[index].operation.destination,
        )?);
        journal.operations[index].state = OperationState::Applied;
        engine.persist_journal(journal)
    }

    #[test]
    fn commits_multiple_roots_with_receipt_last() -> TestResult {
        let directory = tempfile::tempdir()?;
        let first = root(&directory, "first")?;
        let second = root(&directory, "second")?;
        let state_root = root(&directory, "receipt-root")?;
        let engine = engine(&directory);
        let operations = vec![
            create_file("second-file", &second, "asset.txt", b"second")?,
            receipt(&state_root, br#"{"state":"committed"}"#)?,
            create_file("first-file", &first, "asset.txt", b"first")?,
        ];

        assert_eq!(
            engine.apply("nominal", operations)?,
            TransactionOutcome::Committed
        );
        assert_eq!(fs::read(first.path.join("asset.txt"))?, b"first");
        assert_eq!(fs::read(second.path.join("asset.txt"))?, b"second");
        assert_eq!(
            fs::read(state_root.path.join("receipt.json"))?,
            br#"{"state":"committed"}"#
        );
        assert_eq!(engine.journal_state()?, None);
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(state_root.path.join("receipt.json"))?
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        Ok(())
    }

    #[test]
    fn lock_contention_is_nonblocking() -> TestResult {
        let directory = tempfile::tempdir()?;
        let state = directory.path().join("state");
        ensure_private_directory(&state)?;
        let lock_path = state.join("transaction.lock");
        let _first = TransactionLock::acquire(&lock_path)?;
        let started = Instant::now();
        let second = TransactionLock::acquire(&lock_path);
        assert!(matches!(second, Err(TransactionError::LockBusy)));
        assert!(started.elapsed() < Duration::from_millis(250));
        Ok(())
    }

    #[test]
    fn every_injected_mutation_restores_the_initial_snapshot() -> TestResult {
        for target in 1..=8 {
            let directory = tempfile::tempdir()?;
            let asset_root = root(&directory, "managed")?;
            let destination = asset_root.path.join("asset.txt");
            fs::write(&destination, b"before")?;
            #[cfg(unix)]
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o644))?;
            let before = snapshot_path(&destination)?;
            let operations = vec![
                replace_file(
                    "replace",
                    &asset_root,
                    "asset.txt",
                    before.clone(),
                    b"after",
                )?,
                receipt(&asset_root, br#"{"state":"committed"}"#)?,
            ];
            let engine = engine(&directory);
            let mut injector = FailAfterMutation::new(target);

            assert!(
                engine
                    .apply_with(format!("failure-{target}"), operations, &mut injector)
                    .is_err()
            );
            assert_eq!(snapshot_path(&destination)?, before);
            assert_eq!(
                snapshot_path(&asset_root.path.join("receipt.json"))?,
                PathSnapshot::absent()
            );
            assert_eq!(engine.journal_state()?, None);
            let stage_name = format!(".arthur-workflow-failure-{target}-managed.stage");
            assert!(!directory.path().join(stage_name).exists());
        }
        Ok(())
    }

    #[test]
    fn recover_rolls_back_before_receipt_commit() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let engine = engine(&directory);
        let mut journal = prepare_journal(
            &engine,
            "recover-before",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                receipt(&managed, br#"{"state":"committed"}"#)?,
            ],
        )?;
        apply_entry(&engine, &mut journal, 0, JournalState::Applying)?;
        journal.state = JournalState::Committing;
        engine.persist_journal(&journal)?;

        assert_eq!(
            engine.recover(std::slice::from_ref(&managed))?,
            TransactionOutcome::RecoveredRollback
        );
        assert_eq!(
            snapshot_path(&managed.path.join("asset.txt"))?,
            PathSnapshot::absent()
        );
        assert_eq!(engine.journal_state()?, None);
        Ok(())
    }

    #[test]
    fn recover_preparing_only_cleans_owned_staging() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let engine = engine(&directory);
        let mut journal = prepare_journal(
            &engine,
            "recover-preparing",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                receipt(&managed, br#"{"state":"committed"}"#)?,
            ],
        )?;
        journal.state = JournalState::Preparing;
        engine.persist_journal(&journal)?;

        assert_eq!(
            engine.recover(std::slice::from_ref(&managed))?,
            TransactionOutcome::RecoveredRollback
        );
        assert_eq!(
            snapshot_path(&managed.path.join("asset.txt"))?,
            PathSnapshot::absent()
        );
        assert_eq!(engine.journal_state()?, None);
        Ok(())
    }

    #[test]
    fn recover_cleans_up_after_receipt_commit_without_rollback() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let engine = engine(&directory);
        let mut journal = prepare_journal(
            &engine,
            "recover-after",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                receipt(&managed, br#"{"state":"committed"}"#)?,
            ],
        )?;
        apply_entry(&engine, &mut journal, 0, JournalState::Applying)?;
        apply_entry(&engine, &mut journal, 1, JournalState::Committing)?;
        journal.receipt_committed = false;
        engine.persist_journal(&journal)?;

        assert_eq!(
            engine.recover(std::slice::from_ref(&managed))?,
            TransactionOutcome::RecoveredCleanup
        );
        assert_eq!(fs::read(managed.path.join("asset.txt"))?, b"installed");
        assert!(managed.path.join("receipt.json").exists());
        assert_eq!(engine.journal_state()?, None);
        Ok(())
    }

    struct CreatePreconditionRace {
        destination: PathBuf,
        fired: bool,
    }

    impl FailureInjector for CreatePreconditionRace {
        fn after_mutation(&mut self, point: &MutationPoint) -> Result<(), String> {
            if !self.fired && point.primitive == MutationPrimitive::WriteStagedFile {
                fs::write(&self.destination, b"foreign").map_err(|error| error.to_string())?;
                self.fired = true;
            }
            Ok(())
        }
    }

    #[test]
    fn precondition_race_never_rolls_back_the_foreign_path() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let destination = managed.path.join("asset.txt");
        let operations = vec![
            create_file("asset", &managed, "asset.txt", b"installed")?,
            receipt(&managed, br#"{"state":"committed"}"#)?,
        ];
        let mut injector = CreatePreconditionRace {
            destination: destination.clone(),
            fired: false,
        };
        let result = engine(&directory).apply_with("race", operations, &mut injector);

        assert!(matches!(
            result,
            Err(TransactionError::PreconditionsChanged { .. })
        ));
        assert_eq!(fs::read(destination)?, b"foreign");
        Ok(())
    }

    #[test]
    fn recover_rejects_tampered_staging_paths_without_deleting_them() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let victim = directory.path().join("victim");
        fs::create_dir(&victim)?;
        fs::write(victim.join("keep.txt"), b"keep")?;
        let engine = engine(&directory);
        let mut journal = prepare_journal(
            &engine,
            "tampered",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                receipt(&managed, br#"{"state":"committed"}"#)?,
            ],
        )?;
        let Some(valid_stage) = journal.staging_roots.get("managed").cloned() else {
            return Err("missing fixture staging path".into());
        };
        journal
            .staging_roots
            .insert("managed".to_owned(), victim.clone());
        engine.persist_journal(&journal)?;

        assert!(matches!(
            engine.recover(std::slice::from_ref(&managed)),
            Err(TransactionError::Journal(_))
        ));
        assert_eq!(fs::read(victim.join("keep.txt"))?, b"keep");

        journal
            .staging_roots
            .insert("managed".to_owned(), valid_stage);
        engine.cleanup(&journal)?;
        engine.remove_journal()?;
        Ok(())
    }

    #[test]
    fn recovery_requires_current_root_authority() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let engine = engine(&directory);
        let journal = prepare_journal(
            &engine,
            "untrusted",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                receipt(&managed, br#"{"state":"committed"}"#)?,
            ],
        )?;

        assert!(matches!(
            engine.recover(&[]),
            Err(TransactionError::Journal(_))
        ));
        engine.cleanup(&journal)?;
        engine.remove_journal()?;
        Ok(())
    }

    #[test]
    fn journal_temporary_symlink_is_never_followed() -> TestResult {
        let directory = tempfile::tempdir()?;
        let engine = engine(&directory);
        ensure_private_directory(&engine.state_directory)?;
        let victim = directory.path().join("victim.json");
        fs::write(&victim, b"foreign")?;
        let temporary = engine.journal_temporary_path();
        symlink(&victim, &temporary)?;

        assert!(matches!(
            engine.recover(&[]),
            Err(TransactionError::InsecureStatePath(path)) if path == temporary
        ));
        assert_eq!(fs::read(&victim)?, b"foreign");
        fs::remove_file(temporary)?;
        Ok(())
    }

    #[test]
    fn signal_flags_map_to_shell_exit_codes() {
        let interrupt = SignalFlags::default();
        interrupt.record_for_test(SIGINT);
        assert_eq!(interrupt.pending_exit_code(), Some(SIGINT_EXIT_CODE));

        let terminate = SignalFlags::default();
        terminate.record_for_test(SIGTERM);
        assert_eq!(terminate.pending_exit_code(), Some(SIGTERM_EXIT_CODE));
    }

    #[test]
    fn rejects_unsafe_root_ids_and_non_private_receipts() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let mut unsafe_root = managed.clone();
        unsafe_root.id = "../victim".to_owned();
        assert!(create_file("asset", &unsafe_root, "asset.txt", b"asset").is_err());

        let receipt = Operation::new(
            "receipt",
            OperationKind::WriteReceipt,
            managed.clone(),
            managed.path.join("receipt.json"),
            PathSnapshot::absent(),
            PathSnapshot::file(b"{}", 0o644),
            Inverse::RemoveCreated,
            OwnershipProof::TransactionState,
            OperationPayload::File {
                bytes: b"{}".to_vec(),
                mode: 0o644,
            },
        );
        assert!(receipt.is_err());
        Ok(())
    }

    #[test]
    fn managed_directory_rollback_refuses_unexpected_children() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let child = managed.path.join("created");
        fs::create_dir(&child)?;
        fs::write(child.join("foreign.txt"), b"foreign")?;
        let error = super::remove_path(&child);
        assert!(error.is_err());
        assert_eq!(fs::read(child.join("foreign.txt"))?, b"foreign");
        Ok(())
    }

    #[test]
    fn preparation_failure_removes_partial_sibling_staging() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let operations = vec![
            create_file("asset", &managed, "asset.txt", b"installed")?,
            receipt(&managed, br#"{"state":"committed"}"#)?,
        ];
        let mut injector = FailAfterMutation::new(2);
        assert!(
            engine(&directory)
                .apply_with("prepare-failure", operations, &mut injector)
                .is_err()
        );
        assert!(
            !directory
                .path()
                .join(".arthur-workflow-prepare-failure-managed.stage")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn commits_every_mutation_payload_kind() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let created_directory = managed.path.join("created");
        let target = managed.path.join("target.txt");
        let link = managed.path.join("target-link");
        let mode_file = managed.path.join("mode.txt");
        let removed = managed.path.join("removed.txt");
        fs::write(&target, b"target")?;
        fs::write(&mode_file, b"mode")?;
        fs::write(&removed, b"removed")?;
        fs::set_permissions(&mode_file, fs::Permissions::from_mode(0o644))?;
        let mode_before = snapshot_path(&mode_file)?;
        let removed_before = snapshot_path(&removed)?;
        let operations = vec![
            Operation::new(
                "ensure-directory",
                OperationKind::EnsureDirectory,
                managed.clone(),
                created_directory.clone(),
                PathSnapshot::absent(),
                PathSnapshot::directory(0o750),
                Inverse::RemoveCreated,
                OwnershipProof::UnownedDestination,
                OperationPayload::Directory(0o750),
            )?,
            Operation::new(
                "create-symlink",
                OperationKind::CreateSymlink,
                managed.clone(),
                link.clone(),
                PathSnapshot::absent(),
                PathSnapshot::symlink(PathBuf::from("target.txt")),
                Inverse::RemoveCreated,
                OwnershipProof::UnownedDestination,
                OperationPayload::Symlink {
                    target: PathBuf::from("target.txt"),
                },
            )?,
            set_mode_operation(&managed, &mode_file, mode_before, 0o600)?,
            remove_owned_operation(&managed, &removed, removed_before)?,
            receipt(&managed, br#"{"state":"all-kinds"}"#)?,
        ];

        assert_eq!(
            engine(&directory).apply("all-kinds", operations)?,
            TransactionOutcome::Committed
        );
        assert!(created_directory.is_dir());
        assert_eq!(fs::read_link(link)?, PathBuf::from("target.txt"));
        assert_eq!(
            fs::metadata(mode_file)?.permissions().mode() & 0o7777,
            0o600
        );
        assert!(!removed.exists());
        Ok(())
    }

    struct FailOnOperation {
        operation_id: &'static str,
        primitive: MutationPrimitive,
        fired: bool,
    }

    impl FailureInjector for FailOnOperation {
        fn after_mutation(&mut self, point: &MutationPoint) -> Result<(), String> {
            if !self.fired
                && point.operation_id.as_deref() == Some(self.operation_id)
                && point.primitive == self.primitive
            {
                self.fired = true;
                return Err(format!("fail {}", self.operation_id));
            }
            Ok(())
        }
    }

    #[test]
    fn rollback_removes_created_directories_and_symlinks() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let created = managed.path.join("created");
        let link = managed.path.join("created-link");
        let operations = vec![
            Operation::new(
                "created-directory",
                OperationKind::EnsureDirectory,
                managed.clone(),
                created.clone(),
                PathSnapshot::absent(),
                PathSnapshot::directory(0o700),
                Inverse::RemoveCreated,
                OwnershipProof::UnownedDestination,
                OperationPayload::Directory(0o700),
            )?,
            Operation::new(
                "created-symlink",
                OperationKind::CreateSymlink,
                managed.clone(),
                link.clone(),
                PathSnapshot::absent(),
                PathSnapshot::symlink(PathBuf::from("created")),
                Inverse::RemoveCreated,
                OwnershipProof::UnownedDestination,
                OperationPayload::Symlink {
                    target: PathBuf::from("created"),
                },
            )?,
            receipt(&managed, br#"{"state":"rollback"}"#)?,
        ];
        let mut injector = FailOnOperation {
            operation_id: "created-symlink",
            primitive: MutationPrimitive::InstallNoReplace,
            fired: false,
        };

        assert!(
            engine(&directory)
                .apply_with("rollback-created", operations, &mut injector)
                .is_err()
        );
        assert!(!created.exists());
        assert!(!link.exists());
        Ok(())
    }

    #[test]
    fn rollback_restores_modes_and_removed_files() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let mode_file = managed.path.join("mode.txt");
        let removed = managed.path.join("removed.txt");
        fs::write(&mode_file, b"mode")?;
        fs::write(&removed, b"removed")?;
        fs::set_permissions(&mode_file, fs::Permissions::from_mode(0o644))?;
        let operations = vec![
            set_mode_operation(&managed, &mode_file, snapshot_path(&mode_file)?, 0o600)?,
            remove_owned_operation(&managed, &removed, snapshot_path(&removed)?)?,
            receipt(&managed, br#"{"state":"rollback"}"#)?,
        ];
        let mut injector = FailOnOperation {
            operation_id: "remove-owned",
            primitive: MutationPrimitive::Rename,
            fired: false,
        };

        assert!(
            engine(&directory)
                .apply_with("rollback-existing", operations, &mut injector)
                .is_err()
        );
        assert_eq!(
            fs::metadata(mode_file)?.permissions().mode() & 0o7777,
            0o644
        );
        assert_eq!(fs::read(removed)?, b"removed");
        Ok(())
    }

    #[test]
    fn snapshots_cover_directories_symlinks_and_unsupported_nodes() -> TestResult {
        let directory = tempfile::tempdir()?;
        let child = directory.path().join("child");
        fs::create_dir(&child)?;
        fs::set_permissions(&child, fs::Permissions::from_mode(0o750))?;
        let directory_snapshot = snapshot_path(&child)?;
        assert_eq!(directory_snapshot.kind, super::PathKind::Directory);
        assert_eq!(directory_snapshot.mode, Some(0o750));
        assert!(directory_snapshot.inode.is_some());

        let link = directory.path().join("link");
        symlink("child", &link)?;
        let link_snapshot = snapshot_path(&link)?;
        assert_eq!(link_snapshot.kind, super::PathKind::Symlink);
        assert_eq!(link_snapshot.link_target, Some(PathBuf::from("child")));

        let fifo = directory.path().join("fifo");
        mkfifo(&fifo, NixMode::S_IRUSR | NixMode::S_IWUSR)?;
        assert!(matches!(
            snapshot_path(&fifo),
            Err(TransactionError::UnexpectedPathType(path)) if path == fifo
        ));

        let non_utf8 = directory.path().join("non-utf8-link");
        symlink(PathBuf::from(OsString::from_vec(vec![0xff])), &non_utf8)?;
        assert!(matches!(
            snapshot_path(&non_utf8),
            Err(TransactionError::InvalidPath(path)) if path.as_os_str().as_bytes() == [0xff]
        ));
        Ok(())
    }

    #[test]
    fn operation_validation_rejects_each_unsafe_shape() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let valid = create_file("valid", &managed, "asset.txt", b"asset")?;

        let mut candidate = valid.clone();
        candidate.id.clear();
        assert!(candidate.validate().is_err());
        let mut candidate = valid.clone();
        candidate.root.id = "../bad".to_owned();
        assert!(candidate.validate().is_err());
        let mut candidate = valid.clone();
        candidate.root.path = PathBuf::from("relative");
        assert!(candidate.validate().is_err());
        let mut candidate = valid.clone();
        candidate.device = candidate.device.saturating_add(1);
        assert!(candidate.validate().is_err());
        let mut candidate = valid.clone();
        candidate.destination = directory.path().join("outside");
        assert!(candidate.validate().is_err());
        let mut candidate = valid.clone();
        candidate.payload = OperationPayload::None;
        assert!(candidate.validate().is_err());
        let mut candidate = valid.clone();
        candidate.ownership = OwnershipProof::Receipt {
            source_id: "foreign".to_owned(),
            sha256: None,
        };
        assert!(candidate.validate().is_err());
        let mut candidate = valid.clone();
        candidate.inverse = Inverse::None;
        assert!(candidate.validate().is_err());
        let mut candidate = valid.clone();
        candidate.expected_after = PathSnapshot::file(b"different", 0o644);
        assert!(candidate.validate().is_err());

        let before = PathSnapshot::file(b"before", 0o644);
        let mut existing = Operation::new(
            "existing",
            OperationKind::ReplaceFile,
            managed.clone(),
            managed.path.join("existing"),
            before.clone(),
            PathSnapshot::file(b"after", 0o644),
            Inverse::RestoreBackup {
                original: before.clone(),
            },
            OwnershipProof::Receipt {
                source_id: "existing".to_owned(),
                sha256: before.sha256.clone(),
            },
            OperationPayload::File {
                bytes: b"after".to_vec(),
                mode: 0o644,
            },
        )?;
        existing.ownership = OwnershipProof::UnownedDestination;
        assert!(existing.validate().is_err());

        let same_receipt = Operation::new(
            "same-receipt",
            OperationKind::WriteReceipt,
            managed.clone(),
            managed.path.join("same-receipt.json"),
            PathSnapshot::file(b"same", 0o600),
            PathSnapshot::file(b"same", 0o600),
            Inverse::RestoreBackup {
                original: PathSnapshot::file(b"same", 0o600),
            },
            OwnershipProof::Receipt {
                source_id: "receipt".to_owned(),
                sha256: None,
            },
            OperationPayload::File {
                bytes: b"same".to_vec(),
                mode: 0o600,
            },
        )?;
        assert!(same_receipt.validate_for_execution().is_err());
        assert!(
            valid
                .clone()
                .with_revalidation(super::FileIdentityProof {
                    device: 1,
                    inode: 1,
                    size: 5,
                    mtime_seconds: 0,
                    mtime_nanoseconds: 0,
                    sha256: super::hash_bytes(b"asset"),
                })
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn journal_validation_rejects_tampered_state_shapes() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let engine = engine(&directory);
        let journal = prepare_journal(
            &engine,
            "journal-shapes",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                receipt(&managed, br#"{"state":"committed"}"#)?,
            ],
        )?;
        assert!(validate_journal_paths(&journal).is_ok());

        let mut tampered = journal.clone();
        tampered.operations[0].staged_path = Some(directory.path().join("foreign"));
        assert!(validate_journal_paths(&tampered).is_err());
        let mut tampered = journal.clone();
        tampered.operations[0].backup_path = Some(directory.path().join("foreign"));
        assert!(validate_journal_paths(&tampered).is_err());
        let mut tampered = journal.clone();
        tampered.operations[0].applied_snapshot = Some(PathSnapshot::file(b"installed", 0o644));
        assert!(validate_journal_paths(&tampered).is_err());
        let mut tampered = journal.clone();
        tampered.operations[0].state = OperationState::Applied;
        assert!(validate_journal_paths(&tampered).is_err());
        let mut tampered = journal.clone();
        tampered
            .staging_roots
            .insert("unused".to_owned(), directory.path().join("unused"));
        assert!(validate_journal_paths(&tampered).is_err());
        let mut tampered = journal.clone();
        tampered.operations.pop();
        assert!(validate_journal_paths(&tampered).is_err());

        assert!(!receipt_is_committed(&journal)?);
        engine.cleanup(&journal)?;
        engine.remove_journal()?;
        Ok(())
    }

    #[test]
    fn error_messages_and_exit_codes_cover_every_variant() {
        let snapshot = PathSnapshot::absent();
        let path = PathBuf::from("/tmp/example");
        let errors = vec![
            TransactionError::LockBusy,
            TransactionError::RecoveryRequired,
            TransactionError::InvalidPath(path.clone()),
            TransactionError::InsecureStatePath(path.clone()),
            TransactionError::UnexpectedPathType(path.clone()),
            TransactionError::ConcurrentFilesystemChange(path.clone()),
            TransactionError::InvalidOperation("id".to_owned(), "detail"),
            TransactionError::DuplicateOperation("id".to_owned()),
            TransactionError::PreconditionsChanged {
                operation_id: "id".to_owned(),
                expected: Box::new(snapshot.clone()),
                observed: Box::new(snapshot),
            },
            TransactionError::DeviceMismatch {
                root: path.clone(),
                expected: 1,
                observed: 2,
            },
            TransactionError::RootIdentityChanged { root: path.clone() },
            TransactionError::Interrupted(SIGINT_EXIT_CODE),
            TransactionError::InjectedFailure("injected".to_owned()),
            TransactionError::SignalHandler("handler".to_owned()),
            TransactionError::LegacyLockChanged(path.clone()),
            TransactionError::Journal("journal".to_owned()),
            TransactionError::io(
                "read",
                &path,
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
            ),
        ];
        for error in &errors {
            assert!(!error.to_string().is_empty());
            let _ = error.source();
        }
        assert_eq!(errors[0].exit_code(), super::TRANSACTION_EXIT_CODE);
        assert_eq!(errors[11].exit_code(), SIGINT_EXIT_CODE);

        let flags = SignalFlags::default();
        assert_eq!(flags.pending_exit_code(), None);
        flags.record_for_test(0);
        assert_eq!(flags.pending_exit_code(), None);
        assert!(SignalFlags::install().is_ok());
    }

    #[test]
    fn legacy_lock_rewrite_revalidates_identity_before_commit() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let lock = managed.path.join(".skill-lock.json");
        fs::write(&lock, b"original")?;
        fs::set_permissions(&lock, fs::Permissions::from_mode(0o600))?;
        let before = snapshot_path(&lock)?;
        let metadata = fs::metadata(&lock)?;
        let rewrite = Operation::new(
            "rewrite-lock",
            OperationKind::RewriteLegacyLock,
            managed.clone(),
            lock.clone(),
            before.clone(),
            PathSnapshot::file(b"residual", 0o600),
            Inverse::RestoreBackup {
                original: before.clone(),
            },
            OwnershipProof::Adopted {
                source_id: "legacy".to_owned(),
                sha256: before.sha256.clone(),
            },
            OperationPayload::File {
                bytes: b"residual".to_vec(),
                mode: 0o600,
            },
        )?
        .with_revalidation(super::FileIdentityProof {
            device: super::metadata_device(&metadata),
            inode: super::metadata_inode(&metadata),
            size: metadata.len(),
            mtime_seconds: super::metadata_mtime_seconds(&metadata),
            mtime_nanoseconds: super::metadata_mtime_nanoseconds(&metadata),
            sha256: super::hash_bytes(b"original"),
        })?;
        assert_eq!(
            engine(&directory).apply(
                "rewrite-lock",
                vec![rewrite, receipt(&managed, br#"{"state":"adopted"}"#)?],
            )?,
            TransactionOutcome::Committed
        );
        assert_eq!(fs::read(lock)?, b"residual");
        Ok(())
    }

    #[test]
    fn legacy_lock_change_is_detected_at_the_last_boundary() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let lock = managed.path.join(".skill-lock.json");
        fs::write(&lock, b"original")?;
        let before = snapshot_path(&lock)?;
        let metadata = fs::metadata(&lock)?;
        let rewrite = Operation::new(
            "rewrite-lock",
            OperationKind::RewriteLegacyLock,
            managed.clone(),
            lock.clone(),
            before.clone(),
            PathSnapshot::file(b"residual", before.mode.unwrap_or(0o600)),
            Inverse::RestoreBackup {
                original: before.clone(),
            },
            OwnershipProof::Adopted {
                source_id: "legacy".to_owned(),
                sha256: before.sha256.clone(),
            },
            OperationPayload::File {
                bytes: b"residual".to_vec(),
                mode: before.mode.unwrap_or(0o600),
            },
        )?
        .with_revalidation(super::FileIdentityProof {
            device: super::metadata_device(&metadata),
            inode: super::metadata_inode(&metadata),
            size: metadata.len(),
            mtime_seconds: super::metadata_mtime_seconds(&metadata),
            mtime_nanoseconds: super::metadata_mtime_nanoseconds(&metadata),
            sha256: super::hash_bytes(b"original"),
        })?;
        let engine = engine(&directory);
        let journal = prepare_journal(
            &engine,
            "rewrite-race",
            vec![rewrite, receipt(&managed, br#"{"state":"adopted"}"#)?],
        )?;
        fs::write(&lock, b"changed!")?;
        let mut injector = NoFailures;
        let mut ordinal = 0;
        assert!(matches!(
            apply_operation(&journal.operations[0], &mut injector, &mut ordinal),
            Err(TransactionError::LegacyLockChanged(path)) if path == lock
        ));
        engine.cleanup(&journal)?;
        engine.remove_journal()?;
        Ok(())
    }

    #[test]
    fn nonempty_owned_directory_is_preserved() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let owned = managed.path.join("owned-directory");
        fs::create_dir(&owned)?;
        fs::write(owned.join("foreign.txt"), b"foreign")?;
        let operation = remove_owned_operation(&managed, &owned, snapshot_path(&owned)?)?;
        let result = engine(&directory).apply(
            "nonempty-directory",
            vec![operation, receipt(&managed, br#"{"state":"remove"}"#)?],
        );
        assert!(matches!(
            result,
            Err(TransactionError::PreconditionsChanged { .. })
        ));
        assert_eq!(fs::read(owned.join("foreign.txt"))?, b"foreign");
        Ok(())
    }

    #[test]
    fn pending_signal_rolls_back_before_the_first_operation() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let flags = SignalFlags::default();
        flags.record_for_test(SIGTERM);
        let engine = TransactionEngine::new(directory.path().join("state"), flags);
        let result = engine.apply(
            "signal-before-apply",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                receipt(&managed, br#"{"state":"signal"}"#)?,
            ],
        );
        assert!(matches!(
            result,
            Err(TransactionError::Interrupted(SIGTERM_EXIT_CODE))
        ));
        assert!(!managed.path.join("asset.txt").exists());
        assert_eq!(engine.journal_state()?, None);
        Ok(())
    }

    #[test]
    fn incomplete_journal_temporary_without_a_boundary_is_discarded() -> TestResult {
        let directory = tempfile::tempdir()?;
        let engine = engine(&directory);
        ensure_private_directory(&engine.state_directory)?;
        super::write_new_durable_file(&engine.journal_temporary_path(), b"partial", 0o600)?;
        assert_eq!(engine.recover(&[])?, TransactionOutcome::RecoveredRollback);
        assert!(!engine.journal_temporary_path().exists());
        Ok(())
    }

    #[test]
    fn insecure_state_and_lock_nodes_are_rejected() -> TestResult {
        let directory = tempfile::tempdir()?;
        let state_file = directory.path().join("state-file");
        fs::write(&state_file, b"not a directory")?;
        assert!(matches!(
            ensure_private_directory(&state_file),
            Err(TransactionError::InsecureStatePath(path)) if path == state_file
        ));

        let state_directory = directory.path().join("state-directory");
        fs::create_dir(&state_directory)?;
        fs::set_permissions(&state_directory, fs::Permissions::from_mode(0o755))?;
        assert!(ensure_private_directory(&state_directory).is_err());

        let lock_directory = directory.path().join("lock-directory");
        fs::create_dir(&lock_directory)?;
        assert!(TransactionLock::acquire(&lock_directory).is_err());
        let lock_file = directory.path().join("lock-file");
        fs::write(&lock_file, b"")?;
        fs::set_permissions(&lock_file, fs::Permissions::from_mode(0o644))?;
        assert!(TransactionLock::acquire(&lock_file).is_err());
        Ok(())
    }

    #[test]
    fn changed_root_device_or_identity_blocks_preparation() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let mut wrong_device = managed.clone();
        wrong_device.device = wrong_device.device.saturating_add(1);
        assert!(matches!(
            engine(&directory).apply(
                "wrong-device",
                vec![
                    create_file("asset", &wrong_device, "asset.txt", b"asset")?,
                    receipt(&wrong_device, br#"{"state":"device"}"#)?,
                ],
            ),
            Err(TransactionError::DeviceMismatch { .. })
        ));

        let mut wrong_real = managed.clone();
        wrong_real.real = directory.path().join("different-real");
        assert!(matches!(
            engine(&directory).apply(
                "wrong-real",
                vec![
                    create_file("asset", &wrong_real, "asset.txt", b"asset")?,
                    receipt(&wrong_real, br#"{"state":"real"}"#)?,
                ],
            ),
            Err(TransactionError::RootIdentityChanged { .. })
        ));
        Ok(())
    }

    #[test]
    fn validation_rejects_execution_and_transaction_invariants() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let valid = create_file("asset", &managed, "asset.txt", b"asset")?;

        let mut legacy_without_proof = valid.clone();
        legacy_without_proof.kind = OperationKind::RewriteLegacyLock;
        assert!(matches!(
            legacy_without_proof.validate_for_execution(),
            Err(TransactionError::InvalidOperation(_, message))
                if message.contains("requires inode")
        ));

        let mut legacy_with_mismatched_proof = legacy_without_proof.clone();
        legacy_with_mismatched_proof.revalidation = Some(super::FileIdentityProof {
            device: 1,
            inode: 1,
            size: 5,
            mtime_seconds: 0,
            mtime_nanoseconds: 0,
            sha256: super::hash_bytes(b"asset"),
        });
        assert!(
            legacy_with_mismatched_proof
                .validate_for_execution()
                .is_err()
        );

        let mut non_legacy_with_proof = valid.clone();
        non_legacy_with_proof.revalidation = legacy_with_mismatched_proof.revalidation.clone();
        assert!(non_legacy_with_proof.validate_for_execution().is_err());

        let mut injector = NoFailures;
        let mut ordinal = 0;
        let missing_legacy_proof = super::JournalOperation {
            operation: legacy_without_proof,
            state: OperationState::Prepared,
            staged_path: None,
            backup_path: None,
            applied_snapshot: None,
        };
        assert!(matches!(
            apply_operation(&missing_legacy_proof, &mut injector, &mut ordinal),
            Err(TransactionError::InvalidOperation(_, message))
                if message == "legacy-lock identity proof is missing"
        ));

        let missing_staged_payload = super::JournalOperation {
            operation: valid.clone(),
            state: OperationState::Prepared,
            staged_path: None,
            backup_path: None,
            applied_snapshot: None,
        };
        assert!(matches!(
            apply_operation(&missing_staged_payload, &mut injector, &mut ordinal),
            Err(TransactionError::InvalidOperation(_, message))
                if message == "staged payload is missing"
        ));

        let mode_path = managed.path.join("mode-matrix.txt");
        fs::write(&mode_path, b"mode")?;
        fs::set_permissions(&mode_path, fs::Permissions::from_mode(0o644))?;
        let mode_operation =
            set_mode_operation(&managed, &mode_path, snapshot_path(&mode_path)?, 0o600)?;
        let mut unavailable_mode = super::JournalOperation {
            operation: mode_operation.clone(),
            state: OperationState::Prepared,
            staged_path: None,
            backup_path: None,
            applied_snapshot: None,
        };
        unavailable_mode.operation.payload = OperationPayload::Unavailable;
        assert!(matches!(
            apply_operation(&unavailable_mode, &mut injector, &mut ordinal),
            Err(TransactionError::InvalidOperation(_, message))
                if message == "replacement mode is unavailable"
        ));
        assert!(matches!(
            super::rollback_operation(&unavailable_mode, &mut injector, &mut ordinal),
            Err(TransactionError::PreconditionsChanged { .. })
        ));
        fs::remove_file(&mode_path)?;
        assert!(matches!(
            super::rollback_operation(&unavailable_mode, &mut injector, &mut ordinal),
            Err(TransactionError::RecoveryRequired)
        ));

        let existing = managed.path.join("backup-matrix.txt");
        fs::write(&existing, b"before")?;
        let missing_backup = super::JournalOperation {
            operation: replace_file(
                "missing-backup",
                &managed,
                "backup-matrix.txt",
                snapshot_path(&existing)?,
                b"after",
            )?,
            state: OperationState::Applying,
            staged_path: None,
            backup_path: None,
            applied_snapshot: None,
        };
        assert!(matches!(
            super::rollback_operation(&missing_backup, &mut injector, &mut ordinal),
            Err(TransactionError::InvalidOperation(_, message))
                if message == "rollback backup is missing"
        ));

        fs::write(&valid.destination, b"foreign")?;
        assert!(matches!(
            super::rollback_operation(&missing_staged_payload, &mut injector, &mut ordinal),
            Err(TransactionError::PreconditionsChanged { .. })
        ));
        fs::remove_file(&valid.destination)?;

        let mut duplicate = vec![valid.clone(), receipt(&managed, b"{}")?];
        duplicate[1].id = valid.id.clone();
        assert!(matches!(
            validate_and_sort(&mut duplicate),
            Err(TransactionError::DuplicateOperation(id)) if id == "asset"
        ));

        let mut no_receipt = vec![valid.clone()];
        assert!(validate_and_sort(&mut no_receipt).is_err());
        let mut two_receipts = vec![receipt(&managed, b"one")?, receipt(&managed, b"two")?];
        two_receipts[1].id = "receipt-two".to_owned();
        two_receipts[1].destination = managed.path.join("receipt-two.json");
        assert!(validate_and_sort(&mut two_receipts).is_err());

        let other_path = directory.path().join("other");
        fs::create_dir(&other_path)?;
        let conflicting_root = RootSpec::new("managed", other_path.clone(), managed.device);
        let mut conflicting_roots = vec![
            valid,
            create_file("other", &conflicting_root, "asset.txt", b"other")?,
            receipt(&managed, b"{}")?,
        ];
        assert!(validate_and_sort(&mut conflicting_roots).is_err());
        Ok(())
    }

    #[test]
    fn journal_validation_rejects_identity_and_state_tampering() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let engine = engine(&directory);
        let journal = prepare_journal(
            &engine,
            "journal-matrix",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                receipt(&managed, b"{}")?,
            ],
        )?;

        let mut tampered = journal.clone();
        tampered.transaction_id = "../unsafe".to_owned();
        assert!(validate_journal_paths(&tampered).is_err());

        let mut tampered = journal.clone();
        tampered.operations[1].operation.id = "asset".to_owned();
        assert!(matches!(
            validate_journal_paths(&tampered),
            Err(TransactionError::DuplicateOperation(_))
        ));

        let mut tampered = journal.clone();
        tampered.operations[0].operation = tampered.operations[1].operation.clone();
        assert!(validate_journal_paths(&tampered).is_err());

        let mut tampered = journal.clone();
        tampered.staging_roots.remove("managed");
        assert!(validate_journal_paths(&tampered).is_err());

        let mut tampered = journal.clone();
        tampered.operations[0].state = OperationState::Applying;
        tampered.operations[0].applied_snapshot = Some(PathSnapshot::file(b"foreign", 0o644));
        assert!(validate_journal_paths(&tampered).is_err());

        let mut tampered = journal.clone();
        tampered.operations[1].operation.root.path = directory.path().to_path_buf();
        tampered.operations[1].operation.root.real = directory.path().to_path_buf();
        assert!(validate_journal_paths(&tampered).is_err());

        let mut unsupported = journal.clone();
        unsupported.schema_version = super::TRANSACTION_SCHEMA_VERSION + 1;
        engine.persist_journal(&unsupported)?;
        assert!(matches!(
            engine.read_journal(),
            Err(TransactionError::Journal(message)) if message.contains("unsupported schema")
        ));
        engine.cleanup(&journal)?;
        engine.remove_journal()?;
        Ok(())
    }

    #[test]
    fn recovery_and_rollback_cover_idempotent_boundaries() -> TestResult {
        let directory = tempfile::tempdir()?;
        let managed = root(&directory, "managed")?;
        let engine = engine(&directory);
        let existing = managed.path.join("existing.txt");
        fs::write(&existing, b"before")?;
        let mut journal = prepare_journal(
            &engine,
            "recovery-matrix",
            vec![
                create_file("asset", &managed, "asset.txt", b"installed")?,
                replace_file(
                    "replace",
                    &managed,
                    "existing.txt",
                    snapshot_path(&existing)?,
                    b"after",
                )?,
                receipt(&managed, b"{}")?,
            ],
        )?;

        assert!(matches!(
            engine.apply(
                "blocked-by-journal",
                vec![
                    create_file("other", &managed, "other.txt", b"other")?,
                    receipt(&managed, b"new")?,
                ],
            ),
            Err(TransactionError::RecoveryRequired)
        ));
        assert!(engine.recover(&[managed.clone(), managed.clone()]).is_err());
        assert!(!receipt_is_committed(&journal)?);
        journal.receipt_committed = true;
        assert!(receipt_is_committed(&journal)?);
        journal.receipt_committed = false;

        let mut injector = NoFailures;
        let mut ordinal = 0;
        let created_index = journal
            .operations
            .iter()
            .position(|entry| entry.operation.id == "asset")
            .ok_or("missing created operation")?;
        let replace_index = journal
            .operations
            .iter()
            .position(|entry| entry.operation.id == "replace")
            .ok_or("missing replace operation")?;
        super::rollback_operation(
            &journal.operations[created_index],
            &mut injector,
            &mut ordinal,
        )?;
        super::rollback_operation(
            &journal.operations[replace_index],
            &mut injector,
            &mut ordinal,
        )?;
        fs::write(&existing, b"changed")?;
        assert!(matches!(
            super::rollback_operation(
                &journal.operations[replace_index],
                &mut injector,
                &mut ordinal,
            ),
            Err(TransactionError::RecoveryRequired)
        ));
        let backup = journal.operations[replace_index]
            .backup_path
            .as_ref()
            .ok_or("missing backup fixture")?;
        fs::write(backup, b"foreign")?;
        assert!(matches!(
            super::rollback_operation(
                &journal.operations[replace_index],
                &mut injector,
                &mut ordinal,
            ),
            Err(TransactionError::PreconditionsChanged { .. })
        ));
        fs::remove_file(backup)?;

        let mut no_inverse = journal.operations[created_index].clone();
        no_inverse.operation.inverse = Inverse::None;
        super::rollback_operation(&no_inverse, &mut injector, &mut ordinal)?;

        let staged = journal.operations[created_index]
            .staged_path
            .as_ref()
            .ok_or("missing staged fixture")?;
        assert!(super::created_path_is_transaction_owned(
            &journal.operations[created_index],
            &snapshot_path(staged)?,
        )?);
        let mut without_staged = journal.operations[created_index].clone();
        without_staged.staged_path = None;
        assert!(!super::created_path_is_transaction_owned(
            &without_staged,
            &snapshot_path(staged)?,
        )?);

        assert_eq!(super::numeric_u64(-1_i8), None);
        journal.state = JournalState::Committed;
        engine.persist_journal(&journal)?;
        assert_eq!(
            engine.recover(std::slice::from_ref(&managed))?,
            TransactionOutcome::RecoveredCleanup
        );
        Ok(())
    }
}
