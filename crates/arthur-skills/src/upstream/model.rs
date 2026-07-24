use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::provider::ENVIRONMENT_EXIT_CODE;
use crate::transaction::TRANSACTION_EXIT_CODE;

const MANIFEST_NAME: &str = "upstreams.toml";
const LOCK_NAME: &str = "upstreams.lock.json";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpstreamManifest {
    pub(super) schema_version: u16,
    pub(super) source: Vec<SourceManifest>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceManifest {
    pub(super) id: String,
    pub(super) repository: String,
    pub(super) track: String,
    pub(super) skill: Vec<SkillManifest>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SkillManifest {
    pub(super) name: String,
    pub(super) path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpstreamLock {
    pub(super) schema_version: u16,
    pub(super) sources: Vec<SourceLock>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceLock {
    pub(super) id: String,
    pub(super) revision: String,
    pub(super) skills: Vec<SkillLock>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SkillLock {
    pub(super) name: String,
    pub(super) tree_sha1: String,
    pub(super) content_sha256: String,
}

pub(super) struct LoadedConfiguration {
    pub(super) manifest: UpstreamManifest,
    pub(super) lock: UpstreamLock,
    pub(super) manifest_path: PathBuf,
    pub(super) manifest_sha256: String,
    pub(super) lock_path: PathBuf,
    pub(super) lock_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SkillState {
    Current,
    UpdateAvailable,
    Drifted,
    RemovedUpstream,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SkillReport {
    pub(super) name: String,
    pub(super) source: String,
    pub(super) state: SkillState,
    pub(super) pinned_tree_sha1: String,
    pub(super) latest_tree_sha1: Option<String>,
    pub(super) local_content_sha256: Option<String>,
    pub(super) reason: Option<String>,
}

#[derive(Debug)]
pub(super) struct UpstreamError {
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) exit_code: u8,
}

impl UpstreamError {
    pub(super) fn configuration(message: impl Into<String>) -> Self {
        Self {
            code: "upstream_configuration_invalid",
            message: message.into(),
            exit_code: ENVIRONMENT_EXIT_CODE,
        }
    }

    pub(super) fn fetch(message: impl Into<String>) -> Self {
        Self {
            code: "upstream_fetch_failed",
            message: message.into(),
            exit_code: TRANSACTION_EXIT_CODE,
        }
    }

    pub(super) fn synchronization(message: impl Into<String>) -> Self {
        Self {
            code: "upstream_sync_failed",
            message: message.into(),
            exit_code: TRANSACTION_EXIT_CODE,
        }
    }
}

impl fmt::Display for UpstreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for UpstreamError {}

impl SkillState {
    pub(super) fn summary_key(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::UpdateAvailable => "update",
            Self::Drifted => "drifted",
            Self::RemovedUpstream => "removed",
        }
    }

    pub(super) fn reason(self, path: &str) -> Option<String> {
        match self {
            Self::Current => None,
            Self::UpdateAvailable => Some("upstream tree differs from the pinned tree".to_owned()),
            Self::Drifted => Some("vendored content differs from the locked snapshot".to_owned()),
            Self::RemovedUpstream => Some(format!("upstream path {path} no longer exists")),
        }
    }

    pub(super) fn is_blocker(self) -> bool {
        matches!(self, Self::Drifted | Self::RemovedUpstream)
    }
}

pub(super) fn load_configuration(root: &Path) -> Result<LoadedConfiguration, UpstreamError> {
    let manifest_path = root.join(MANIFEST_NAME);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
        UpstreamError::configuration(format!("{}: cannot read: {error}", manifest_path.display()))
    })?;
    let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|error| {
        UpstreamError::configuration(format!(
            "{}: manifest is not UTF-8: {error}",
            manifest_path.display()
        ))
    })?;
    let manifest: UpstreamManifest = toml::from_str(manifest_text).map_err(|error| {
        UpstreamError::configuration(format!(
            "{}: invalid TOML: {error}",
            manifest_path.display()
        ))
    })?;

    let lock_path = root.join(LOCK_NAME);
    let lock_bytes = fs::read(&lock_path).map_err(|error| {
        UpstreamError::configuration(format!("{}: cannot read: {error}", lock_path.display()))
    })?;
    let lock: UpstreamLock = serde_json::from_slice(&lock_bytes).map_err(|error| {
        UpstreamError::configuration(format!("{}: invalid JSON: {error}", lock_path.display()))
    })?;
    validate_configuration(&manifest, &lock)?;
    Ok(LoadedConfiguration {
        manifest,
        lock,
        manifest_path,
        manifest_sha256: hash_bytes(&manifest_bytes),
        lock_path,
        lock_sha256: hash_bytes(&lock_bytes),
    })
}

pub(super) fn validate_configuration(
    manifest: &UpstreamManifest,
    lock: &UpstreamLock,
) -> Result<(), UpstreamError> {
    if manifest.schema_version != 1 || lock.schema_version != 1 {
        return Err(UpstreamError::configuration(
            "upstream manifest and lock must use schema version 1",
        ));
    }
    if manifest.source.is_empty() {
        return Err(UpstreamError::configuration(
            "upstream manifest must contain at least one source",
        ));
    }

    let mut source_ids = BTreeSet::new();
    let mut repositories = BTreeSet::new();
    let mut skill_names = BTreeSet::new();
    for source in &manifest.source {
        validate_source(source, &mut source_ids, &mut repositories, &mut skill_names)?;
    }
    validate_lock(manifest, lock)
}

fn validate_source<'a>(
    source: &'a SourceManifest,
    source_ids: &mut BTreeSet<&'a str>,
    repositories: &mut BTreeSet<&'a str>,
    skill_names: &mut BTreeSet<&'a str>,
) -> Result<(), UpstreamError> {
    if !source_ids.insert(source.id.as_str()) {
        return Err(UpstreamError::configuration(format!(
            "{}: duplicate source identifier",
            source.id
        )));
    }
    validate_repository(&source.repository)?;
    if !repositories.insert(source.repository.as_str()) {
        return Err(UpstreamError::configuration(format!(
            "{}: duplicate repository; group its skills under one source",
            source.repository
        )));
    }
    validate_track(&source.track)?;
    if source.skill.is_empty() {
        return Err(UpstreamError::configuration(format!(
            "{}: source has no skills",
            source.id
        )));
    }
    for skill in &source.skill {
        if !valid_skill_name(&skill.name) {
            return Err(UpstreamError::configuration(format!(
                "{}: invalid skill name",
                skill.name
            )));
        }
        if !skill_names.insert(skill.name.as_str()) {
            return Err(UpstreamError::configuration(format!(
                "{}: duplicate skill",
                skill.name
            )));
        }
        validate_relative_path(&skill.path)?;
    }
    Ok(())
}

fn validate_lock(manifest: &UpstreamManifest, lock: &UpstreamLock) -> Result<(), UpstreamError> {
    let mut locked_source_ids = BTreeSet::new();
    for source in &lock.sources {
        if !locked_source_ids.insert(source.id.as_str()) {
            return Err(UpstreamError::configuration(format!(
                "{}: duplicate source in lock",
                source.id
            )));
        }
    }
    let locked_sources = lock_sources(lock);
    if lock.sources.len() != manifest.source.len() {
        return Err(UpstreamError::configuration(
            "manifest and lock source counts differ",
        ));
    }
    for source in &manifest.source {
        let locked = locked_sources.get(source.id.as_str()).ok_or_else(|| {
            UpstreamError::configuration(format!("{}: source is absent from lock", source.id))
        })?;
        validate_source_lock(source, locked)?;
    }
    Ok(())
}

fn validate_source_lock(source: &SourceManifest, locked: &SourceLock) -> Result<(), UpstreamError> {
    if !valid_hex(&locked.revision, 40) {
        return Err(UpstreamError::configuration(format!(
            "{}: lock revision must be a lowercase Git SHA-1",
            source.id
        )));
    }
    let mut locked_skill_names = BTreeSet::new();
    for skill in &locked.skills {
        if !locked_skill_names.insert(skill.name.as_str()) {
            return Err(UpstreamError::configuration(format!(
                "{}: duplicate skill in lock",
                skill.name
            )));
        }
    }
    let locked_skills = lock_skills(locked);
    if locked.skills.len() != source.skill.len() {
        return Err(UpstreamError::configuration(format!(
            "{}: manifest and lock skill counts differ",
            source.id
        )));
    }
    for skill in &source.skill {
        let locked_skill = locked_skills.get(skill.name.as_str()).ok_or_else(|| {
            UpstreamError::configuration(format!("{}: skill is absent from lock", skill.name))
        })?;
        if !valid_hex(&locked_skill.tree_sha1, 40) || !valid_hex(&locked_skill.content_sha256, 64) {
            return Err(UpstreamError::configuration(format!(
                "{}: lock hashes are invalid",
                skill.name
            )));
        }
    }
    Ok(())
}

pub(super) fn lock_sources(lock: &UpstreamLock) -> BTreeMap<&str, &SourceLock> {
    lock.sources
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect()
}

pub(super) fn lock_skills(source: &SourceLock) -> BTreeMap<&str, &SkillLock> {
    source
        .skills
        .iter()
        .map(|skill| (skill.name.as_str(), skill))
        .collect()
}

fn validate_repository(repository: &str) -> Result<(), UpstreamError> {
    let slug = repository
        .strip_prefix("https://github.com/")
        .and_then(|value| value.strip_suffix(".git"))
        .ok_or_else(|| {
            UpstreamError::configuration(format!(
                "{repository}: repository must be an HTTPS GitHub clone URL ending in .git"
            ))
        })?;
    let components = slug.split('/').collect::<Vec<_>>();
    if components.len() != 2
        || components
            .iter()
            .any(|component| !valid_repository_component(component))
    {
        return Err(UpstreamError::configuration(format!(
            "{repository}: repository owner/name is invalid"
        )));
    }
    Ok(())
}

fn valid_repository_component(component: &str) -> bool {
    !component.is_empty()
        && !component.starts_with('-')
        && component
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn validate_track(track: &str) -> Result<(), UpstreamError> {
    if track.is_empty()
        || track.starts_with('-')
        || track.contains("..")
        || track.contains("@{")
        || track.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
        })
    {
        return Err(UpstreamError::configuration(format!(
            "{track}: tracked branch is invalid"
        )));
    }
    Ok(())
}

fn valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn validate_relative_path(path: &str) -> Result<(), UpstreamError> {
    if path.is_empty()
        || path.contains('\\')
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(UpstreamError::configuration(format!(
            "{path}: upstream skill path must be relative and traversal-free"
        )));
    }
    Ok(())
}

pub(super) fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn content_sha256(directory: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("{}: cannot inspect directory: {error}", directory.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{}: skill root must be a real directory",
            directory.display()
        ));
    }
    let skill_file = directory.join("SKILL.md");
    let skill_metadata = fs::symlink_metadata(&skill_file)
        .map_err(|error| format!("{}: SKILL.md is unavailable: {error}", directory.display()))?;
    if skill_metadata.file_type().is_symlink() || !skill_metadata.is_file() {
        return Err(format!(
            "{}: SKILL.md must be a regular file",
            directory.display()
        ));
    }

    let mut entries = vec![SnapshotEntry {
        relative: String::new(),
        path: directory.to_path_buf(),
        kind: EntryKind::Directory,
        mode: permission_mode(&metadata),
    }];
    collect_entries(directory, directory, &mut entries)?;
    entries.sort_by(|left, right| left.relative.cmp(&right.relative));
    let mut digest = Sha256::new();
    digest.update(b"arthur-skills.snapshot.v2\0");
    for entry in entries {
        digest.update([entry.kind as u8]);
        digest.update(entry.mode.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let path_length = entry.relative.len() as u64;
        digest.update(path_length.to_le_bytes());
        digest.update(entry.relative.as_bytes());
        let bytes = if entry.kind == EntryKind::File {
            fs::read(&entry.path)
                .map_err(|error| format!("{}: cannot read file: {error}", entry.path.display()))?
        } else {
            Vec::new()
        };
        #[allow(clippy::cast_possible_truncation)]
        let content_length = bytes.len() as u64;
        digest.update(content_length.to_le_bytes());
        digest.update(bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
enum EntryKind {
    Directory = 1,
    File = 2,
}

struct SnapshotEntry {
    relative: String,
    path: PathBuf,
    kind: EntryKind,
    mode: u32,
}

fn collect_entries(
    root: &Path,
    current: &Path,
    snapshot: &mut Vec<SnapshotEntry>,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|error| format!("{}: cannot read directory: {error}", current.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("{}: cannot read entry: {error}", current.display()))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("{}: cannot inspect entry: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{}: source symlinks are forbidden", path.display()));
        }
        let relative = path.strip_prefix(root).map_err(|error| {
            format!("{}: cannot resolve relative path: {error}", path.display())
        })?;
        let relative = relative
            .to_str()
            .ok_or_else(|| format!("{}: non-UTF-8 path rejected", path.display()))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if metadata.is_dir() {
            snapshot.push(SnapshotEntry {
                relative,
                path: path.clone(),
                kind: EntryKind::Directory,
                mode: permission_mode(&metadata),
            });
            collect_entries(root, &path, snapshot)?;
        } else if metadata.is_file() {
            snapshot.push(SnapshotEntry {
                relative,
                path,
                kind: EntryKind::File,
                mode: permission_mode(&metadata),
            });
        } else {
            return Err(format!("{}: unsupported source type", path.display()));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn permission_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn permission_mode(metadata: &fs::Metadata) -> u32 {
    u32::from(metadata.permissions().readonly())
}

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
