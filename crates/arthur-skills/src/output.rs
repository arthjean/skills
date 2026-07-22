use std::collections::BTreeMap;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::plan::{DiagnosticSeverity as PlanSeverity, Owner, Plan, PlanAction, PlanEntry};
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
    if !envelope.operations.is_empty() {
        for (action, count) in &envelope.summary {
            writeln!(output, "{action}: {count}")?;
        }
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
        None => (None, Some(hex(path.as_os_str().as_bytes()))),
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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::{self, Write};
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    use serde_json::Value;

    use super::{
        ENVIRONMENT_EXIT_CODE, Envelope, OutputOperation, OutputStatus, USAGE_EXIT_CODE,
        path_fields, write_human, write_json,
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
    }
}
