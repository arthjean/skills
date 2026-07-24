use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::plan::{DiagnosticSeverity as PlanSeverity, Owner, Plan, PlanAction, PlanEntry};
use crate::platform::path_key;
use crate::provider::{ENVIRONMENT_EXIT_CODE, ProviderId};

pub const OUTPUT_SCHEMA_VERSION: u16 = 1;
pub const SUCCESS_EXIT_CODE: u8 = 0;
pub const USAGE_EXIT_CODE: u8 = 2;
pub const CONFLICT_EXIT_CODE: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStatus {
    Success,
    Noop,
    Blocked,
    Failed,
    RecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OutputDiagnostic {
    pub code: String,
    pub severity: OutputSeverity,
    pub message: String,
    pub path_utf8: Option<String>,
    pub path_bytes_hex: Option<String>,
    pub remediation: Option<String>,
}

impl OutputDiagnostic {
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        remediation: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: OutputSeverity::Error,
            message: message.into(),
            path_utf8: None,
            path_bytes_hex: None,
            remediation,
        }
    }

    pub fn with_path(mut self, path_utf8: Option<String>, path_bytes_hex: Option<String>) -> Self {
        self.path_utf8 = path_utf8;
        self.path_bytes_hex = path_bytes_hex;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OutputOperation {
    pub action: PlanAction,
    pub source: String,
    pub destination_utf8: Option<String>,
    pub destination_bytes_hex: Option<String>,
    pub owner: Owner,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssetChange {
    pub action: PlanAction,
    pub label: String,
}

#[derive(Debug)]
struct AggregatedChange {
    label: String,
    source_rank: u8,
    actions: BTreeSet<PlanAction>,
}

impl From<&PlanEntry> for OutputOperation {
    fn from(entry: &PlanEntry) -> Self {
        let (destination_utf8, destination_bytes_hex) = path_fields(&entry.destination);
        Self {
            action: entry.action,
            source: entry.source.clone(),
            destination_utf8,
            destination_bytes_hex,
            owner: entry.owner,
            reason: entry.reason.clone(),
        }
    }
}

pub(crate) fn asset_changes<'a>(
    entries: impl IntoIterator<Item = (PlanAction, &'a str)>,
) -> Vec<AssetChange> {
    let mut changes = BTreeMap::<String, AggregatedChange>::new();
    for (action, source) in entries {
        if action == PlanAction::Noop {
            continue;
        }
        let Some((key, label, source_rank)) = classify_asset(source) else {
            continue;
        };
        let change = changes.entry(key).or_insert_with(|| AggregatedChange {
            label: label.clone(),
            source_rank,
            actions: BTreeSet::new(),
        });
        if source_rank > change.source_rank {
            change.label = label;
            change.source_rank = source_rank;
            change.actions.clear();
        }
        if source_rank == change.source_rank {
            change.actions.insert(action);
        }
    }
    changes
        .into_values()
        .filter_map(|change| {
            representative_action(&change.actions).map(|action| AssetChange {
                action,
                label: change.label,
            })
        })
        .collect()
}

pub(crate) const fn pending_action_label(action: PlanAction) -> &'static str {
    match action {
        PlanAction::Create => "Restore",
        PlanAction::Update => "Update",
        PlanAction::Remove => "Remove",
        PlanAction::Adoptable => "Adopt",
        PlanAction::Drifted => "Drift",
        PlanAction::Conflict => "Conflict",
        PlanAction::RetainedUnmanaged => "Retain",
        PlanAction::RecoveryRequired => "Recover",
        PlanAction::Noop => "Keep",
    }
}

pub(crate) const fn completed_action_label(action: PlanAction) -> &'static str {
    match action {
        PlanAction::Create => "Restored",
        PlanAction::Update => "Updated",
        PlanAction::Remove => "Removed",
        PlanAction::RetainedUnmanaged => "Retained",
        _ => pending_action_label(action),
    }
}

fn classify_asset(source: &str) -> Option<(String, String, u8)> {
    let (source, activation) = source
        .strip_prefix("activation:claude:")
        .map_or((source, false), |source| (source, true));
    let source = source.strip_prefix("directory:").unwrap_or(source);
    if source.starts_with("container:") {
        return None;
    }
    if let Some(path) = source.strip_prefix("skills/") {
        let name = path.split('/').next().filter(|name| !name.is_empty())?;
        return Some((
            format!("skill:{name}"),
            format!("Skill  {name}"),
            u8::from(!activation) + 1,
        ));
    }
    if activation {
        let name = source.split('/').next().filter(|name| !name.is_empty())?;
        return Some((format!("skill:{name}"), format!("Skill  {name}"), 1));
    }
    for (prefix, provider) in [
        ("agents/claude/", "Claude Code"),
        ("agents/codex/", "Codex"),
    ] {
        if let Some(path) = source.strip_prefix(prefix) {
            let name = file_stem(path);
            return Some((
                format!("agent:{provider}:{path}"),
                format!("Agent  {name} ({provider})"),
                2,
            ));
        }
    }
    if let Some(path) = source.strip_prefix("shared/claude/") {
        let name = file_stem(path);
        return Some((
            format!("support:claude:{path}"),
            format!("Support  {name} (Claude Code)"),
            2,
        ));
    }
    Some((format!("asset:{source}"), format!("Asset  {source}"), 2))
}

fn file_stem(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .rsplit_once('.')
        .map_or(path.rsplit('/').next().unwrap_or(path), |(stem, _)| stem)
}

fn representative_action(actions: &BTreeSet<PlanAction>) -> Option<PlanAction> {
    for action in [
        PlanAction::Conflict,
        PlanAction::RecoveryRequired,
        PlanAction::Drifted,
        PlanAction::Adoptable,
        PlanAction::Update,
    ] {
        if actions.contains(&action) {
            return Some(action);
        }
    }
    if actions.contains(&PlanAction::Create) && actions.contains(&PlanAction::Remove) {
        return Some(PlanAction::Update);
    }
    [
        PlanAction::Remove,
        PlanAction::Create,
        PlanAction::RetainedUnmanaged,
    ]
    .into_iter()
    .find(|action| actions.contains(action))
}

#[derive(Debug, Serialize)]
pub struct Envelope {
    pub schema_version: u16,
    pub command: Option<String>,
    pub status: OutputStatus,
    pub exit_code: u8,
    pub catalog_version: String,
    pub transaction_id: Option<String>,
    pub providers: Vec<ProviderId>,
    pub summary: BTreeMap<String, usize>,
    pub operations: Vec<OutputOperation>,
    pub diagnostics: Vec<OutputDiagnostic>,
    pub data: Value,
    #[serde(skip)]
    pub suppress_human_output: bool,
}

impl Envelope {
    pub fn new(command: Option<&str>) -> Self {
        Self {
            schema_version: OUTPUT_SCHEMA_VERSION,
            command: command.map(str::to_owned),
            status: OutputStatus::Success,
            exit_code: SUCCESS_EXIT_CODE,
            catalog_version: env!("CARGO_PKG_VERSION").to_owned(),
            transaction_id: None,
            providers: Vec::new(),
            summary: BTreeMap::new(),
            operations: Vec::new(),
            diagnostics: Vec::new(),
            data: Value::Null,
            suppress_human_output: false,
        }
    }

    pub fn usage(command: Option<&str>, message: impl Into<String>) -> Self {
        let message = message.into();
        let mut envelope = Self::new(command);
        envelope.status = OutputStatus::Failed;
        envelope.exit_code = USAGE_EXIT_CODE;
        envelope.diagnostics.push(OutputDiagnostic::error(
            "usage",
            message,
            Some("Run the command with --help and provide the missing option.".to_owned()),
        ));
        envelope
    }

    pub fn failure(
        command: Option<&str>,
        status: OutputStatus,
        exit_code: u8,
        code: &str,
        message: impl Into<String>,
    ) -> Self {
        let mut envelope = Self::new(command);
        envelope.status = status;
        envelope.exit_code = exit_code;
        envelope
            .diagnostics
            .push(OutputDiagnostic::error(code, message, None));
        envelope
    }

    pub fn with_plan(mut self, plan: &Plan) -> Self {
        self.operations = plan.entries.iter().map(OutputOperation::from).collect();
        self.summary = summarize(plan);
        self.diagnostics
            .extend(plan.diagnostics.iter().map(|diagnostic| OutputDiagnostic {
                code: diagnostic.code.clone(),
                severity: match diagnostic.severity {
                    PlanSeverity::Error => OutputSeverity::Error,
                    PlanSeverity::Warning => OutputSeverity::Warning,
                },
                message: diagnostic.message.clone(),
                path_utf8: diagnostic.path_utf8.clone(),
                path_bytes_hex: diagnostic.path_bytes_hex.clone(),
                remediation: Some(
                    "Resolve the reported path before applying this plan.".to_owned(),
                ),
            }));
        if !plan.applicable {
            self.status = OutputStatus::Blocked;
            self.exit_code = CONFLICT_EXIT_CODE;
        } else if !plan.has_mutations() {
            self.status = OutputStatus::Noop;
        }
        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path_bytes_hex.is_some())
        {
            self.status = OutputStatus::Failed;
            self.exit_code = ENVIRONMENT_EXIT_CODE;
        }
        self
    }
}

pub fn clap_envelope(command: Option<&str>, error: &clap::Error) -> Envelope {
    use clap::error::ErrorKind;

    let display = error.to_string();
    match error.kind() {
        ErrorKind::DisplayHelp => {
            let mut envelope = Envelope::new(command);
            envelope.data = json!({ "help": display });
            envelope
        }
        ErrorKind::DisplayVersion => {
            let mut envelope = Envelope::new(command);
            envelope.data = json!({ "version": display.trim_end() });
            envelope
        }
        _ => {
            let mut envelope = Envelope::usage(command, display.clone());
            envelope.data = json!({ "help": display });
            envelope
        }
    }
}

pub fn write_json(envelope: &Envelope, output: &mut impl Write) -> io::Result<()> {
    serde_json::to_writer(&mut *output, envelope).map_err(io::Error::other)?;
    writeln!(output)
}

pub fn write_human(envelope: &Envelope, output: &mut impl Write) -> io::Result<()> {
    write_human_with_detail(envelope, output, true)
}

pub fn write_human_compact(envelope: &Envelope, output: &mut impl Write) -> io::Result<()> {
    write_human_with_detail(envelope, output, false)
}

fn write_human_with_detail(
    envelope: &Envelope,
    output: &mut impl Write,
    detailed: bool,
) -> io::Result<()> {
    if envelope.data.get("kind").and_then(Value::as_str) == Some("upstream") {
        return write_upstream(envelope, output);
    }
    if envelope.data.get("result").and_then(Value::as_str) == Some("already_current")
        && let Some(message) = envelope.data.get("message").and_then(Value::as_str)
    {
        writeln!(output, "{message}")?;
        for diagnostic in &envelope.diagnostics {
            writeln!(output, "{}: {}", diagnostic.code, diagnostic.message)?;
        }
        return Ok(());
    }
    let committed = envelope.data.get("result").and_then(Value::as_str) == Some("committed");
    if !detailed && committed {
        writeln!(output, "Done")?;
        let summary = compact_summary(envelope);
        if !summary.is_empty() {
            writeln!(output, "  {}", summary.join("  · "))?;
        }
        for change in asset_changes(
            envelope
                .operations
                .iter()
                .map(|operation| (operation.action, operation.source.as_str())),
        ) {
            writeln!(
                output,
                "  {:<9} {}",
                completed_action_label(change.action),
                change.label
            )?;
        }
        for diagnostic in &envelope.diagnostics {
            let label = match diagnostic.severity {
                OutputSeverity::Info => "Info",
                OutputSeverity::Warning => "Note",
                OutputSeverity::Error => "Error",
            };
            writeln!(output, "  {label}  {}", diagnostic.message)?;
        }
        return Ok(());
    }
    if !envelope.operations.is_empty() {
        for (action, count) in &envelope.summary {
            writeln!(output, "{action}: {count}")?;
        }
        if envelope.command.is_some() {
            for operation in &envelope.operations {
                writeln!(
                    output,
                    "{:?} {} ({})",
                    operation.action,
                    operation
                        .destination_utf8
                        .as_deref()
                        .or(operation.destination_bytes_hex.as_deref())
                        .unwrap_or("<missing path>"),
                    operation.reason
                )?;
            }
        }
    }
    if !envelope.data.is_null() {
        match &envelope.data {
            Value::String(value) => writeln!(output, "{value}")?,
            value => writeln!(output, "{value}")?,
        }
    }
    for diagnostic in &envelope.diagnostics {
        writeln!(output, "{}: {}", diagnostic.code, diagnostic.message)?;
    }
    Ok(())
}

fn write_upstream(envelope: &Envelope, output: &mut impl Write) -> io::Result<()> {
    let action = envelope
        .data
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("check");
    let sources = envelope
        .data
        .get("sources")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let skills = envelope.data.get("skills").and_then(Value::as_array);
    let skill_count = skills.map_or(0, |items| items.len());
    let synced = envelope.data.get("result").and_then(Value::as_str) == Some("synced");
    let applied = envelope
        .data
        .get("applied")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    writeln!(output, "Upstream {action}")?;
    writeln!(output, "Sources {sources}  · Skills {skill_count}")?;

    if let Some(skills) = skills {
        for skill in skills {
            let reported_state = skill
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let name = skill
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            let state = if synced && applied.contains(name) {
                "updated"
            } else {
                reported_state
            };
            if state == "current" {
                continue;
            }
            let source = skill
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            writeln!(output, "{state}  {name}  ({source})")?;
        }
    }
    if synced {
        writeln!(output, "Applied {} upstream updates.", applied.len())?;
    } else if envelope.status == OutputStatus::Noop {
        writeln!(
            output,
            "Every vendored skill matches its pinned upstream tree."
        )?;
    }
    for diagnostic in &envelope.diagnostics {
        writeln!(output, "{}: {}", diagnostic.code, diagnostic.message)?;
    }
    Ok(())
}

pub(crate) fn compact_summary(envelope: &Envelope) -> Vec<String> {
    const ACTIONS: [(&str, &str); 9] = [
        ("create", "created"),
        ("update", "updated"),
        ("remove", "removed"),
        ("adoptable", "adoptable"),
        ("drifted", "drifted"),
        ("conflict", "conflicting"),
        ("retained_unmanaged", "retained"),
        ("recovery_required", "requiring recovery"),
        ("noop", "unchanged"),
    ];
    ACTIONS
        .iter()
        .filter_map(|(action, label)| {
            envelope
                .summary
                .get(*action)
                .filter(|count| **count > 0)
                .map(|count| format!("{count} {label}"))
        })
        .collect()
}

fn summarize(plan: &Plan) -> BTreeMap<String, usize> {
    let mut summary = BTreeMap::new();
    for entry in &plan.entries {
        let action = format!("{:?}", entry.action).to_ascii_lowercase();
        *summary.entry(action).or_insert(0) += 1;
    }
    summary
}

pub fn path_fields(path: &Path) -> (Option<String>, Option<String>) {
    match path.to_str() {
        Some(path) => (Some(path.to_owned()), None),
        None => (None, Some(hex(&path_key(path.as_os_str())))),
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(all(test, unix))]
mod tests {
    use std::ffi::OsString;
    use std::io::{self, Write};
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    use serde_json::Value;

    use super::{
        ENVIRONMENT_EXIT_CODE, Envelope, OutputDiagnostic, OutputOperation, OutputSeverity,
        OutputStatus, USAGE_EXIT_CODE, asset_changes, path_fields, write_human,
        write_human_compact, write_json,
    };
    use crate::plan::{
        Diagnostic, DiagnosticSeverity, Owner, Plan, PlanAction, PlanEntry, PlannedMutation,
    };

    #[test]
    fn json_envelope_always_contains_the_v1_contract() {
        let envelope = Envelope::usage(None, "provide --provider");
        let mut bytes = Vec::new();
        assert!(write_json(&envelope, &mut bytes).is_ok());
        let parsed = serde_json::from_slice::<Value>(&bytes);
        let Ok(Value::Object(object)) = parsed else {
            panic!("output was not one JSON object");
        };
        assert_eq!(object.len(), 11);
        assert_eq!(object["schema_version"], 1);
        assert_eq!(object["exit_code"], USAGE_EXIT_CODE);
        for field in [
            "command",
            "status",
            "catalog_version",
            "transaction_id",
            "providers",
            "summary",
            "operations",
            "diagnostics",
            "data",
        ] {
            assert!(object.contains_key(field));
        }
    }

    #[test]
    fn plan_diagnostics_and_human_output_preserve_paths_and_status() {
        let non_utf8 = PathBuf::from(OsString::from_vec(b"/tmp/path-\xff".to_vec()));
        let plan = Plan {
            schema_version: 1,
            applicable: false,
            entries: vec![PlanEntry {
                action: PlanAction::Conflict,
                source: "skill:test".to_owned(),
                destination: non_utf8.clone(),
                owner: Owner::Unmanaged,
                reason: "foreign destination".to_owned(),
            }],
            operations: Vec::<PlannedMutation>::new(),
            diagnostics: vec![
                Diagnostic {
                    code: "unsafe_path".to_owned(),
                    severity: DiagnosticSeverity::Error,
                    message: "unsafe destination".to_owned(),
                    path_utf8: None,
                    path_bytes_hex: Some("ff".to_owned()),
                },
                Diagnostic {
                    code: "notice".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: "review destination".to_owned(),
                    path_utf8: Some("/tmp/path".to_owned()),
                    path_bytes_hex: None,
                },
            ],
        };
        let mut envelope = Envelope::new(Some("plan")).with_plan(&plan);
        assert_eq!(envelope.status, OutputStatus::Failed);
        assert_eq!(envelope.exit_code, ENVIRONMENT_EXIT_CODE);
        assert_eq!(envelope.summary["conflict"], 1);
        assert!(envelope.operations[0].destination_utf8.is_none());
        assert!(envelope.operations[0].destination_bytes_hex.is_some());

        envelope.operations.push(OutputOperation {
            action: PlanAction::Noop,
            source: "missing".to_owned(),
            destination_utf8: None,
            destination_bytes_hex: None,
            owner: Owner::ArthurWorkflow,
            reason: "missing display path".to_owned(),
        });
        envelope.data = Value::String("done".to_owned());
        let mut output = Vec::new();
        assert!(write_human(&envelope, &mut output).is_ok());
        let output = String::from_utf8_lossy(&output);
        assert!(output.contains("<missing path>"));
        assert!(output.contains("done"));
        assert!(output.contains("unsafe_path: unsafe destination"));
        assert_eq!(path_fields(&non_utf8).0, None);
    }

    #[test]
    fn compact_committed_output_summarizes_without_listing_paths() {
        let mut envelope = Envelope::new(Some("install"));
        envelope.summary.insert("noop".to_owned(), 513);
        envelope.summary.insert("update".to_owned(), 13);
        envelope.operations.push(OutputOperation {
            action: PlanAction::Update,
            source: "skills/test/SKILL.md".to_owned(),
            destination_utf8: Some("/home/user/.agents/skills/test".to_owned()),
            destination_bytes_hex: None,
            owner: Owner::ArthurWorkflow,
            reason: "managed path is eligible for a verified update".to_owned(),
        });
        envelope.data = serde_json::json!({ "applied": true, "result": "committed" });
        envelope.diagnostics.push(OutputDiagnostic {
            code: "codex_uses_implicit_skills".to_owned(),
            severity: OutputSeverity::Warning,
            message: "Codex reads shared skills directly.".to_owned(),
            path_utf8: None,
            path_bytes_hex: None,
            remediation: None,
        });

        let mut output = Vec::new();
        assert!(write_human_compact(&envelope, &mut output).is_ok());
        assert_eq!(
            String::from_utf8_lossy(&output),
            "Done\n  13 updated  · 513 unchanged\n  Updated   Skill  test\n  Note  Codex reads shared skills directly.\n"
        );
    }

    #[test]
    fn asset_changes_group_files_and_provider_activations_by_managed_asset() {
        let changes = asset_changes([
            (PlanAction::Update, "skills/coss/SKILL.md"),
            (PlanAction::Create, "directory:skills/coss/references"),
            (PlanAction::Create, "activation:claude:coss"),
            (PlanAction::Create, "agents/codex/docs-researcher.toml"),
            (PlanAction::Create, "container:codex-agents"),
            (PlanAction::Noop, "skills/current/SKILL.md"),
        ]);

        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].action, PlanAction::Create);
        assert_eq!(changes[0].label, "Agent  docs-researcher (Codex)");
        assert_eq!(changes[1].action, PlanAction::Update);
        assert_eq!(changes[1].label, "Skill  coss");
    }

    #[test]
    fn upstream_human_output_lists_changes_and_results() {
        let mut envelope = Envelope::new(Some("upstream"));
        envelope.data = serde_json::json!({
            "kind": "upstream",
            "action": "check",
            "sources": 1,
            "skills": [
                { "name": "alpha", "source": "owner/repository", "state": "current" },
                {
                    "name": "beta",
                    "source": "owner/repository",
                    "state": "update_available"
                }
            ],
            "result": null,
            "applied": []
        });
        let mut output = Vec::new();
        assert!(write_human(&envelope, &mut output).is_ok());
        let output = String::from_utf8_lossy(&output);
        assert!(output.contains("Upstream check"));
        assert!(output.contains("Sources 1  · Skills 2"));
        assert!(output.contains("update_available  beta  (owner/repository)"));
        assert!(!output.contains("alpha"));

        envelope.data["result"] = Value::String("synced".to_owned());
        envelope.data["applied"] = serde_json::json!(["beta"]);
        let mut output = Vec::new();
        assert!(write_human(&envelope, &mut output).is_ok());
        let output = String::from_utf8_lossy(&output);
        assert!(output.contains("updated  beta  (owner/repository)"));
        assert!(output.contains("Applied 1 upstream updates."));

        envelope.status = OutputStatus::Noop;
        envelope.data["result"] = Value::Null;
        let mut output = Vec::new();
        assert!(write_human(&envelope, &mut output).is_ok());
        assert!(
            String::from_utf8_lossy(&output)
                .contains("Every vendored skill matches its pinned upstream tree.")
        );
    }

    #[test]
    fn writers_propagate_output_failures() {
        struct Reject;

        impl Write for Reject {
            fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("closed"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("closed"))
            }
        }

        let envelope = Envelope::usage(Some("install"), "missing provider");
        assert!(write_json(&envelope, &mut Reject).is_err());
        assert!(write_human(&envelope, &mut Reject).is_err());
        assert!(write_human_compact(&envelope, &mut Reject).is_err());
    }
}
