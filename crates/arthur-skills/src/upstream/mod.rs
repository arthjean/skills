mod git;
mod model;
mod sync;

#[cfg(test)]
mod tests;

use std::env;
use std::path::{Path, PathBuf};

use serde_json::json;
use tempfile::{Builder, TempDir};

use self::git::{FetchedSkill, GitFetcher, SourceFetcher};
use self::model::{
    LoadedConfiguration, SkillReport, SkillState, SourceLock, UpstreamError, content_sha256,
    load_configuration, lock_skills, lock_sources,
};
use crate::cli::{UpstreamArgs, UpstreamCommand, UpstreamSyncArgs};
use crate::output::{
    CONFLICT_EXIT_CODE, Envelope, OutputDiagnostic, OutputStatus, USAGE_EXIT_CODE,
};

const MANIFEST_NAME: &str = "upstreams.toml";

struct PreparedUpdate {
    name: String,
    source_id: String,
    source_revision: String,
    source_directory: PathBuf,
    latest_tree_sha1: String,
    latest_content_sha256: String,
    pinned_content_sha256: String,
}

struct PreparedInspection {
    root: PathBuf,
    configuration: LoadedConfiguration,
    reports: Vec<SkillReport>,
    updates: Vec<PreparedUpdate>,
    source_count: usize,
    _workspace: TempDir,
}

pub fn execute(arguments: &UpstreamArgs) -> Envelope {
    let root = match discover_repository_root() {
        Ok(root) => root,
        Err(error) => return error_envelope(error),
    };
    execute_at(&root, arguments, &GitFetcher)
}

fn execute_at(root: &Path, arguments: &UpstreamArgs, fetcher: &dyn SourceFetcher) -> Envelope {
    match &arguments.command {
        UpstreamCommand::Check => run_check(root, fetcher),
        UpstreamCommand::Sync(sync) => run_sync(root, sync, fetcher),
    }
}

fn run_check(root: &Path, fetcher: &dyn SourceFetcher) -> Envelope {
    match inspect_at(root, fetcher) {
        Ok(inspection) => report_envelope(&inspection, "check", false, None, &[]),
        Err(error) => error_envelope(error),
    }
}

fn run_sync(root: &Path, arguments: &UpstreamSyncArgs, fetcher: &dyn SourceFetcher) -> Envelope {
    if !arguments.yes && !arguments.dry_run {
        let mut envelope = Envelope::failure(
            Some("upstream"),
            OutputStatus::Failed,
            USAGE_EXIT_CODE,
            "confirmation_required",
            "upstream sync requires --yes, or --dry-run for inspection only",
        );
        envelope.diagnostics[0].remediation =
            Some("Review `arthur-skills upstream check`, then rerun sync with --yes.".to_owned());
        return envelope;
    }

    let inspection = match inspect_at(root, fetcher) {
        Ok(inspection) => inspection,
        Err(error) => return error_envelope(error),
    };
    if arguments.dry_run {
        return report_envelope(&inspection, "sync", true, None, &[]);
    }
    if has_blockers(&inspection) {
        return report_envelope(&inspection, "sync", false, None, &[]);
    }
    if inspection.updates.is_empty() {
        return report_envelope(&inspection, "sync", false, None, &[]);
    }

    match sync::apply(&inspection) {
        Ok(applied) => report_envelope(&inspection, "sync", false, Some("synced"), &applied),
        Err(error) => error_envelope(error),
    }
}

fn report_envelope(
    inspection: &PreparedInspection,
    action: &str,
    dry_run: bool,
    result: Option<&str>,
    applied: &[String],
) -> Envelope {
    let mut envelope = Envelope::new(Some("upstream"));
    for report in &inspection.reports {
        let key = report.state.summary_key();
        *envelope.summary.entry(key.to_owned()).or_insert(0) += 1;
    }
    if has_blockers(inspection) {
        envelope.status = OutputStatus::Blocked;
        envelope.exit_code = CONFLICT_EXIT_CODE;
        envelope.diagnostics.push(OutputDiagnostic::error(
            "upstream_sync_blocked",
            "local drift or a removed upstream path must be resolved before synchronization",
            Some(
                "Restore the pinned local snapshot or update the manifest and lock explicitly."
                    .to_owned(),
            ),
        ));
    } else if inspection.updates.is_empty() {
        envelope.status = OutputStatus::Noop;
    }
    envelope.data = json!({
        "kind": "upstream",
        "action": action,
        "dry_run": dry_run,
        "result": result,
        "sources": inspection.source_count,
        "skills": inspection.reports,
        "applied": applied,
    });
    envelope
}

fn error_envelope(error: UpstreamError) -> Envelope {
    Envelope::failure(
        Some("upstream"),
        OutputStatus::Failed,
        error.exit_code,
        error.code,
        error.message,
    )
}

fn discover_repository_root() -> Result<PathBuf, UpstreamError> {
    let current = env::current_dir().map_err(|error| {
        UpstreamError::configuration(format!("cannot resolve current directory: {error}"))
    })?;
    discover_repository_root_from(&current)
}

fn discover_repository_root_from(current: &Path) -> Result<PathBuf, UpstreamError> {
    for candidate in current.ancestors() {
        if candidate.join(MANIFEST_NAME).is_file() && candidate.join("skills").is_dir() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(UpstreamError::configuration(format!(
        "{MANIFEST_NAME} was not found in the current directory or any parent"
    )))
}

fn inspect_at(
    root: &Path,
    fetcher: &dyn SourceFetcher,
) -> Result<PreparedInspection, UpstreamError> {
    let configuration = load_configuration(root)?;
    let workspace = Builder::new()
        .prefix("arthur-skills-upstream-")
        .tempdir()
        .map_err(|error| {
            UpstreamError::fetch(format!("cannot create upstream staging directory: {error}"))
        })?;
    let manifest_locks = lock_sources(&configuration.lock);
    let mut reports = Vec::new();
    let mut updates = Vec::new();

    for (index, source) in configuration.manifest.source.iter().enumerate() {
        let source_lock = required_source_lock(&manifest_locks, &source.id)?;
        let fetched = fetcher.fetch(source, &workspace.path().join(format!("source-{index}")))?;
        inspect_source(
            root,
            source,
            source_lock,
            &fetched,
            &mut reports,
            &mut updates,
        )?;
    }
    reports.sort_by(|left, right| left.name.cmp(&right.name));
    updates.sort_by(|left, right| left.name.cmp(&right.name));
    let source_count = configuration.manifest.source.len();
    Ok(PreparedInspection {
        root: root.to_path_buf(),
        configuration,
        reports,
        updates,
        source_count,
        _workspace: workspace,
    })
}

fn inspect_source(
    root: &Path,
    source: &model::SourceManifest,
    source_lock: &SourceLock,
    fetched: &git::FetchedSource,
    reports: &mut Vec<SkillReport>,
    updates: &mut Vec<PreparedUpdate>,
) -> Result<(), UpstreamError> {
    let skill_locks = lock_skills(source_lock);
    for skill in &source.skill {
        let locked = required_skill_lock(&skill_locks, &skill.name)?;
        let remote = fetched.skills.get(&skill.name).ok_or_else(|| {
            UpstreamError::fetch(format!(
                "{}: fetcher omitted the requested skill",
                skill.name
            ))
        })?;
        let local_hash = content_sha256(&root.join("skills").join(&skill.name)).ok();
        let state = classify_skill(local_hash.as_deref(), locked, remote);
        reports.push(SkillReport {
            name: skill.name.clone(),
            source: source.id.clone(),
            state,
            pinned_tree_sha1: locked.tree_sha1.clone(),
            latest_tree_sha1: remote.tree_sha1.clone(),
            local_content_sha256: local_hash,
            reason: state.reason(&skill.path),
        });
        if state == SkillState::UpdateAvailable {
            updates.push(prepare_update(skill, source, locked, fetched, remote)?);
        }
    }
    Ok(())
}

fn required_source_lock<'a>(
    locks: &'a std::collections::BTreeMap<&str, &'a SourceLock>,
    id: &str,
) -> Result<&'a SourceLock, UpstreamError> {
    locks.get(id).copied().ok_or_else(|| {
        UpstreamError::configuration(format!("{id}: source is absent from the lock"))
    })
}

fn required_skill_lock<'a>(
    locks: &'a std::collections::BTreeMap<&str, &'a model::SkillLock>,
    name: &str,
) -> Result<&'a model::SkillLock, UpstreamError> {
    locks.get(name).copied().ok_or_else(|| {
        UpstreamError::configuration(format!("{name}: skill is absent from the lock"))
    })
}

fn classify_skill(
    local_hash: Option<&str>,
    locked: &model::SkillLock,
    remote: &FetchedSkill,
) -> SkillState {
    if local_hash != Some(&locked.content_sha256) {
        SkillState::Drifted
    } else if remote.tree_sha1.is_none() {
        SkillState::RemovedUpstream
    } else if remote.tree_sha1.as_deref() != Some(&locked.tree_sha1) {
        SkillState::UpdateAvailable
    } else {
        SkillState::Current
    }
}

fn prepare_update(
    skill: &model::SkillManifest,
    source: &model::SourceManifest,
    locked: &model::SkillLock,
    fetched: &git::FetchedSource,
    remote: &FetchedSkill,
) -> Result<PreparedUpdate, UpstreamError> {
    let source_directory = remote.directory.clone().ok_or_else(|| {
        UpstreamError::fetch(format!(
            "{}: updated upstream directory is unavailable",
            skill.name
        ))
    })?;
    let latest_tree_sha1 = remote.tree_sha1.clone().ok_or_else(|| {
        UpstreamError::fetch(format!(
            "{}: updated upstream tree hash is unavailable",
            skill.name
        ))
    })?;
    let latest_content_sha256 = content_sha256(&source_directory)
        .map_err(|message| UpstreamError::fetch(format!("{}: {message}", skill.name)))?;
    Ok(PreparedUpdate {
        name: skill.name.clone(),
        source_id: source.id.clone(),
        source_revision: fetched.revision.clone(),
        source_directory,
        latest_tree_sha1,
        latest_content_sha256,
        pinned_content_sha256: locked.content_sha256.clone(),
    })
}

fn has_blockers(inspection: &PreparedInspection) -> bool {
    inspection
        .reports
        .iter()
        .any(|report| report.state.is_blocker())
}
