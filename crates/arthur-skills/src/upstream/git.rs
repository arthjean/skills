use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use super::model::{SourceManifest, UpstreamError, content_sha256, valid_hex};

pub(super) struct FetchedSkill {
    pub(super) directory: Option<PathBuf>,
    pub(super) tree_sha1: Option<String>,
}

pub(super) struct FetchedSource {
    pub(super) revision: String,
    pub(super) skills: BTreeMap<String, FetchedSkill>,
}

pub(super) trait SourceFetcher {
    fn fetch(
        &self,
        source: &SourceManifest,
        destination: &Path,
    ) -> Result<FetchedSource, UpstreamError>;
}

pub(super) struct GitFetcher;

impl SourceFetcher for GitFetcher {
    fn fetch(
        &self,
        source: &SourceManifest,
        destination: &Path,
    ) -> Result<FetchedSource, UpstreamError> {
        clone_source(source, destination)?;
        let revision = git_capture(destination, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        validate_git_hash(
            &revision,
            &format!("{}: git returned an invalid revision", source.id),
        )?;

        let mut skills = BTreeMap::new();
        for skill in &source.skill {
            let directory = destination.join(&skill.path);
            let metadata =
                optional_metadata(fs::symlink_metadata(&directory), &source.id, &skill.path)?;
            let fetched = match metadata {
                None => FetchedSkill {
                    directory: None,
                    tree_sha1: None,
                },
                Some(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                    content_sha256(&directory).map_err(|message| {
                        UpstreamError::fetch(format!("{}: {message}", skill.name))
                    })?;
                    let object = format!("HEAD:{}", skill.path);
                    let tree_sha1 = git_capture(destination, &["rev-parse", "--verify", &object])?;
                    validate_git_hash(
                        &tree_sha1,
                        &format!("{}: git returned an invalid tree hash", skill.name),
                    )?;
                    FetchedSkill {
                        directory: Some(directory),
                        tree_sha1: Some(tree_sha1),
                    }
                }
                Some(_) => {
                    return Err(UpstreamError::fetch(format!(
                        "{}: upstream path is not a real directory",
                        skill.path
                    )));
                }
            };
            skills.insert(skill.name.clone(), fetched);
        }
        Ok(FetchedSource { revision, skills })
    }
}

pub(super) fn validate_git_hash(value: &str, message: &str) -> Result<(), UpstreamError> {
    if valid_hex(value, 40) {
        Ok(())
    } else {
        Err(UpstreamError::fetch(message))
    }
}

pub(super) fn optional_metadata(
    result: io::Result<fs::Metadata>,
    source: &str,
    path: &str,
) -> Result<Option<fs::Metadata>, UpstreamError> {
    match result {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(UpstreamError::fetch(format!(
            "{source}: cannot inspect {path}: {error}"
        ))),
    }
}

fn clone_source(source: &SourceManifest, destination: &Path) -> Result<(), UpstreamError> {
    let output = ProcessCommand::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg("--depth")
        .arg("1")
        .arg("--single-branch")
        .arg("--branch")
        .arg(&source.track)
        .arg("--filter=blob:none")
        .arg("--")
        .arg(&source.repository)
        .arg(destination)
        .output()
        .map_err(map_git_start)?;
    if !output.status.success() {
        return Err(UpstreamError::fetch(format!(
            "{}: git clone failed: {}",
            source.id,
            concise_stderr(&output.stderr)
        )));
    }
    Ok(())
}

pub(super) fn git_capture(directory: &Path, arguments: &[&str]) -> Result<String, UpstreamError> {
    let output = ProcessCommand::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .map_err(map_git_start)?;
    if !output.status.success() {
        return Err(UpstreamError::fetch(format!(
            "git {} failed: {}",
            arguments.join(" "),
            concise_stderr(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| UpstreamError::fetch(format!("git output is not UTF-8: {error}")))
}

pub(super) fn map_git_start(error: io::Error) -> UpstreamError {
    if error.kind() == io::ErrorKind::NotFound {
        UpstreamError::configuration("git is required for upstream check and sync commands")
    } else {
        UpstreamError::fetch(format!("cannot start git: {error}"))
    }
}

pub(super) fn concise_stderr(bytes: &[u8]) -> String {
    let value = String::from_utf8_lossy(bytes);
    value.trim().chars().take(500).collect()
}
