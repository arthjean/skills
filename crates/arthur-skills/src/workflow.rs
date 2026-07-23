use std::collections::BTreeMap;
use std::fs;
use std::io;

use serde::Serialize;

use crate::plan::{Plan, PlanAction, PlanEntry};
use crate::receipt::Receipt;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowState {
    FreshInstall,
    Import,
    Update,
    Current,
}

impl WorkflowState {
    pub const fn title(self) -> &'static str {
        match self {
            Self::FreshInstall => "Install Arthur Workflow",
            Self::Import => "Import existing configuration",
            Self::Update => "Update configuration",
            Self::Current => "Everything is up to date",
        }
    }

    pub const fn action(self) -> &'static str {
        match self {
            Self::FreshInstall => "install",
            Self::Import => "import and clean up",
            Self::Update => "update configuration",
            Self::Current => "close",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AssetSummary {
    pub total: usize,
    pub found: usize,
    pub missing: usize,
    pub not_aligned: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkflowAssessment {
    pub state: WorkflowState,
    pub skills: AssetSummary,
    pub agents: AssetSummary,
    pub legacy_skills_to_import: usize,
    pub legacy_skills_to_clean: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AssetKey {
    Skill(String),
    Agent(String),
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
enum Condition {
    #[default]
    Aligned,
    Missing,
    NotAligned,
}

pub fn assess(
    receipt: Option<&Receipt>,
    plan: &Plan,
    legacy_skills_to_import: usize,
    legacy_skills_to_clean: usize,
) -> WorkflowAssessment {
    let mut conditions = BTreeMap::<AssetKey, Condition>::new();
    for entry in &plan.entries {
        let Some(key) = asset_key(entry) else {
            continue;
        };
        let condition = condition(entry);
        conditions
            .entry(key)
            .and_modify(|current| *current = (*current).max(condition))
            .or_insert(condition);
    }

    let skills = summarize(
        conditions
            .iter()
            .filter(|(key, _)| matches!(key, AssetKey::Skill(_)))
            .map(|(_, condition)| *condition),
    );
    let agents = summarize(
        conditions
            .iter()
            .filter(|(key, _)| matches!(key, AssetKey::Agent(_)))
            .map(|(_, condition)| *condition),
    );
    let has_existing = plan
        .entries
        .iter()
        .any(|entry| matches!(entry.action, PlanAction::Adoptable | PlanAction::Conflict))
        || legacy_skills_to_import > 0;
    let state = if receipt.is_none() {
        if has_existing {
            WorkflowState::Import
        } else {
            WorkflowState::FreshInstall
        }
    } else if skills.missing == 0
        && skills.not_aligned == 0
        && agents.missing == 0
        && agents.not_aligned == 0
        && !plan.has_mutations()
    {
        WorkflowState::Current
    } else {
        WorkflowState::Update
    };

    WorkflowAssessment {
        state,
        skills,
        agents,
        legacy_skills_to_import,
        legacy_skills_to_clean,
    }
}

fn summarize(conditions: impl Iterator<Item = Condition>) -> AssetSummary {
    let conditions = conditions.collect::<Vec<_>>();
    let missing = conditions
        .iter()
        .filter(|condition| **condition == Condition::Missing)
        .count();
    let not_aligned = conditions
        .iter()
        .filter(|condition| **condition == Condition::NotAligned)
        .count();
    AssetSummary {
        total: conditions.len(),
        found: conditions.len().saturating_sub(missing),
        missing,
        not_aligned,
    }
}

fn condition(entry: &PlanEntry) -> Condition {
    match entry.action {
        PlanAction::Create => Condition::Missing,
        PlanAction::Drifted
            if matches!(
                fs::symlink_metadata(&entry.destination),
                Err(error) if error.kind() == io::ErrorKind::NotFound
            ) =>
        {
            Condition::Missing
        }
        PlanAction::Update | PlanAction::Drifted | PlanAction::Conflict => Condition::NotAligned,
        PlanAction::Remove
        | PlanAction::Noop
        | PlanAction::Adoptable
        | PlanAction::RetainedUnmanaged
        | PlanAction::RecoveryRequired => Condition::Aligned,
    }
}

fn asset_key(entry: &PlanEntry) -> Option<AssetKey> {
    let source = entry
        .source
        .strip_prefix("directory:")
        .unwrap_or(&entry.source);
    if let Some(name) = source.strip_prefix("activation:claude:") {
        let name = name
            .strip_prefix("directory:skills/")
            .unwrap_or(name)
            .split('/')
            .next()?;
        return non_empty(name).map(|name| AssetKey::Skill(name.to_owned()));
    }
    if let Some(relative) = source.strip_prefix("skills/") {
        let name = relative.split('/').next()?;
        return non_empty(name).map(|name| AssetKey::Skill(name.to_owned()));
    }
    for prefix in ["agents/claude/", "agents/codex/"] {
        if let Some(relative) = source.strip_prefix(prefix) {
            let name = relative.split('/').next()?;
            return non_empty(name).map(|name| AssetKey::Agent(format!("{prefix}{name}")));
        }
    }
    None
}

fn non_empty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{WorkflowState, assess};
    use crate::plan::{Owner, PLAN_SCHEMA_VERSION, Plan, PlanAction, PlanEntry};

    fn entry(action: PlanAction, source: &str, reason: &str) -> PlanEntry {
        PlanEntry {
            action,
            source: source.to_owned(),
            destination: PathBuf::from(format!("/tmp/{source}")),
            owner: Owner::Unmanaged,
            reason: reason.to_owned(),
        }
    }

    #[test]
    fn assessment_groups_files_into_user_facing_skills_and_agents() {
        let plan = Plan {
            schema_version: PLAN_SCHEMA_VERSION,
            applicable: false,
            entries: vec![
                entry(
                    PlanAction::Adoptable,
                    "directory:skills/meta-code",
                    "matching",
                ),
                entry(
                    PlanAction::Conflict,
                    "skills/meta-code/SKILL.md",
                    "different",
                ),
                entry(PlanAction::Create, "skills/new-skill/SKILL.md", "missing"),
                entry(
                    PlanAction::Adoptable,
                    "agents/claude/agent-docs.md",
                    "matching",
                ),
                entry(PlanAction::Conflict, "agents/codex/docs.toml", "different"),
                entry(PlanAction::Create, "shared/claude/support.md", "missing"),
            ],
            operations: Vec::new(),
            diagnostics: Vec::new(),
        };

        let assessment = assess(None, &plan, 5, 3);
        assert_eq!(assessment.state, WorkflowState::Import);
        assert_eq!(assessment.legacy_skills_to_import, 5);
        assert_eq!(assessment.skills.total, 2);
        assert_eq!(assessment.skills.missing, 1);
        assert_eq!(assessment.skills.not_aligned, 1);
        assert_eq!(assessment.agents.total, 2);
        assert_eq!(assessment.agents.not_aligned, 1);
        assert_eq!(assessment.legacy_skills_to_clean, 3);
    }

    #[test]
    fn legacy_lock_entries_force_import_even_when_their_files_are_missing() {
        let plan = Plan {
            schema_version: PLAN_SCHEMA_VERSION,
            applicable: true,
            entries: vec![entry(
                PlanAction::Create,
                "skills/meta-code/SKILL.md",
                "destination does not exist",
            )],
            operations: Vec::new(),
            diagnostics: Vec::new(),
        };

        let assessment = assess(None, &plan, 1, 0);

        assert_eq!(assessment.state, WorkflowState::Import);
        assert_eq!(assessment.legacy_skills_to_import, 1);
    }
}
