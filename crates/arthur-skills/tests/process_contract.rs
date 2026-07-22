#![forbid(unsafe_code)]

use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn run(home: &std::path::Path, arguments: &[&str]) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(arguments)
        .env("HOME", home)
        .env_remove("CODEX_HOME")
        .env_remove("ARTHUR_SKILLS_PLAIN")
        .env_remove("NO_COLOR")
        .output()
}

fn json_output(output: &Output) -> Result<Value, Box<dyn Error>> {
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn write_v3_lock_for_installed_skills(home: &Path) -> TestResult {
    let mut skills = serde_json::Map::new();
    for root in [home.join(".agents/skills"), home.join(".claude/skills")] {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name().into_string().map_err(|name| {
                format!(
                    "fixture skill name is not UTF-8: {:?}",
                    name.as_encoded_bytes()
                )
            })?;
            skills.insert(
                name.clone(),
                serde_json::json!({
                    "source": name,
                    "sourceType": "github",
                    "skillFolderHash": "0123456789012345678901234567890123456789",
                    "installedAt": "2026-01-01T00:00:00.000Z",
                    "updatedAt": "2026-01-01T00:00:00.000Z"
                }),
            );
        }
    }
    let lock = serde_json::json!({
        "version": 3,
        "skills": skills,
        "dismissed": { "findSkillsPrompt": true },
        "lastSelectedAgents": ["codex", "claude-code"]
    });
    fs::write(
        home.join(".agents/.skill-lock.json"),
        serde_json::to_vec_pretty(&lock)?,
    )?;
    Ok(())
}

#[test]
fn help_documents_commands_modes_and_applicable_flags() -> TestResult {
    let home = tempfile::tempdir()?;
    let help = run(home.path(), &["--help"])?;
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout)?;
    for command in [
        "plan",
        "install",
        "status",
        "doctor",
        "update",
        "uninstall",
        "adopt",
        "recover",
    ] {
        assert!(text.contains(command));
    }
    assert!(text.contains("--plain"));
    assert!(text.contains("--json"));

    let install = run(home.path(), &["install", "--help"])?;
    let install = String::from_utf8(install.stdout)?;
    for flag in ["--provider", "--yes", "--dry-run"] {
        assert!(install.contains(flag));
    }
    Ok(())
}

#[test]
fn json_governs_help_version_usage_and_contradictory_modes() -> TestResult {
    let home = tempfile::tempdir()?;
    let help = run(home.path(), &["--json", "--help"])?;
    assert!(help.status.success());
    let help = json_output(&help)?;
    assert!(help["command"].is_null());
    assert!(
        help["data"]["help"]
            .as_str()
            .is_some_and(|text| text.contains("Usage:"))
    );

    let version = run(home.path(), &["--json", "--version"])?;
    assert!(version.status.success());
    let version = json_output(&version)?;
    assert!(
        version["data"]["version"]
            .as_str()
            .is_some_and(|text| text.contains("arthur-skills"))
    );

    let contradictory = run(home.path(), &["--json", "--plain", "status"])?;
    assert_eq!(contradictory.status.code(), Some(2));
    let contradictory = json_output(&contradictory)?;
    assert_eq!(contradictory["status"], "failed");
    assert_eq!(contradictory["exit_code"], 2);
    assert_eq!(contradictory["command"], "status");
    Ok(())
}

#[test]
fn json_separator_does_not_enable_machine_mode() -> TestResult {
    let home = tempfile::tempdir()?;
    let output = run(
        home.path(),
        &["plan", "--provider", "claude", "--", "--json"],
    )?;
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stdout.starts_with(b"{"));
    Ok(())
}

#[test]
fn unresolved_parse_error_keeps_command_null() -> TestResult {
    let home = tempfile::tempdir()?;
    let output = run(home.path(), &["--json", "--bogus", "status"])?;
    assert_eq!(output.status.code(), Some(2));
    let envelope = json_output(&output)?;
    assert!(envelope["command"].is_null());
    let diagnostic = &envelope["diagnostics"][0];
    assert_eq!(diagnostic["severity"], "error");
    assert!(diagnostic.get("path_utf8").is_some());
    assert!(diagnostic.get("path_bytes_hex").is_some());
    assert!(diagnostic.get("remediation").is_some());
    Ok(())
}

#[test]
fn noninteractive_decisions_fail_before_filesystem_mutation() -> TestResult {
    let home = tempfile::tempdir()?;
    let implicit_install = run(home.path(), &["--json"])?;
    assert_eq!(implicit_install.status.code(), Some(2));
    assert!(json_output(&implicit_install)?["command"].is_null());

    let missing_provider = run(home.path(), &["--json", "install", "--yes"])?;
    assert_eq!(missing_provider.status.code(), Some(2));
    let envelope = json_output(&missing_provider)?;
    assert!(
        envelope["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("--provider"))
    );

    let missing_confirmation = run(home.path(), &["--json", "install", "--provider", "claude"])?;
    assert_eq!(missing_confirmation.status.code(), Some(2));
    let envelope = json_output(&missing_confirmation)?;
    assert!(
        envelope["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("--yes"))
    );
    assert!(!home.path().join(".agents").exists());
    assert!(!home.path().join(".claude").exists());
    Ok(())
}

#[test]
fn dry_run_is_deterministic_and_does_not_create_user_state() -> TestResult {
    let home = tempfile::tempdir()?;
    let arguments = ["--json", "install", "--provider", "claude", "--dry-run"];
    let first = run(home.path(), &arguments)?;
    let second = run(home.path(), &arguments)?;
    assert!(first.status.success());
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
    let envelope = json_output(&first)?;
    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(envelope["command"], "install");
    assert!(envelope["transaction_id"].is_null());
    assert_eq!(envelope["providers"], serde_json::json!(["claude"]));
    assert!(
        envelope["operations"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(!home.path().join(".agents").exists());
    assert!(!home.path().join(".claude").exists());
    Ok(())
}

#[test]
fn fresh_machine_commands_return_closed_noninteractive_outcomes() -> TestResult {
    let home = tempfile::tempdir()?;
    for (arguments, code, status) in [
        (vec!["--json", "status"], 0, "noop"),
        (vec!["--json", "doctor"], 3, "blocked"),
        (vec!["--json", "update", "--yes"], 3, "blocked"),
        (vec!["--json", "uninstall", "--all", "--yes"], 0, "noop"),
        (vec!["--json", "recover"], 0, "noop"),
        (
            vec!["--json", "adopt", "--provider", "claude", "--dry-run"],
            0,
            "noop",
        ),
        (vec!["--json", "plan"], 2, "failed"),
    ] {
        let output = run(home.path(), &arguments)?;
        assert_eq!(output.status.code(), Some(code), "{arguments:?}");
        assert_eq!(json_output(&output)?["status"], status, "{arguments:?}");
    }

    let plan = run(
        home.path(),
        &["--json", "plan", "--provider", "claude,codex"],
    )?;
    assert!(plan.status.success());
    assert_eq!(
        json_output(&plan)?["providers"],
        serde_json::json!(["claude", "codex"])
    );
    Ok(())
}

#[test]
fn documented_exit_codes_cover_environment_and_conflict_failures() -> TestResult {
    let home = tempfile::tempdir()?;
    let environment = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["--json", "status"])
        .env("HOME", "relative")
        .output()?;
    assert_eq!(environment.status.code(), Some(4));
    assert_eq!(json_output(&environment)?["status"], "failed");

    let missing_home = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["--json", "status"])
        .env_remove("HOME")
        .output()?;
    assert_eq!(missing_home.status.code(), Some(4));

    let home_file = tempfile::NamedTempFile::new()?;
    let wrong_kind = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["--json", "plan", "--provider", "claude"])
        .env("HOME", home_file.path())
        .output()?;
    assert_eq!(wrong_kind.status.code(), Some(4));

    let empty_codex_home = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["--json", "plan", "--provider", "codex"])
        .env("HOME", home.path())
        .env("CODEX_HOME", "")
        .output()?;
    assert_eq!(empty_codex_home.status.code(), Some(4));

    let recover_with_empty_codex_home = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["--json", "recover"])
        .env("HOME", home.path())
        .env("CODEX_HOME", "")
        .output()?;
    assert_eq!(recover_with_empty_codex_home.status.code(), Some(4));

    fs::create_dir_all(home.path().join(".agents/skills/meta-code"))?;
    fs::write(
        home.path().join(".agents/skills/meta-code/SKILL.md"),
        b"foreign",
    )?;
    let conflict = run(
        home.path(),
        &["--json", "install", "--provider", "claude", "--dry-run"],
    )?;
    assert_eq!(conflict.status.code(), Some(3));
    assert_eq!(json_output(&conflict)?["status"], "blocked");
    Ok(())
}

#[test]
fn non_utf8_environment_path_has_lossless_json_diagnostics() -> TestResult {
    let parent = tempfile::tempdir()?;
    let home = parent
        .path()
        .join(OsString::from_vec(b"arthur-home-\xff".to_vec()));
    fs::create_dir(&home)?;
    let output = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["--json", "status"])
        .env("HOME", &home)
        .output()?;
    assert_eq!(output.status.code(), Some(4));
    let envelope = json_output(&output)?;
    let diagnostic = &envelope["diagnostics"][0];
    assert!(diagnostic["path_utf8"].is_null());
    assert!(
        diagnostic["path_bytes_hex"]
            .as_str()
            .is_some_and(|path| path.ends_with("ff"))
    );
    Ok(())
}

#[test]
fn process_lifecycle_preserves_explicit_uninstall_scope() -> TestResult {
    let home = tempfile::tempdir()?;
    let install = run(
        home.path(),
        &["--json", "install", "--provider", "claude", "--yes"],
    )?;
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stdout)
    );
    assert!(
        home.path()
            .join(".agents/.arthur-workflow/receipt.json")
            .exists()
    );

    let state_directory = home.path().join(".agents/.arthur-workflow");
    let invalid_journal = state_directory.join("transaction.json");
    fs::write(&invalid_journal, b"not json")?;
    let invalid_state = run(home.path(), &["--json", "doctor"])?;
    assert!(!invalid_state.status.success());
    assert_eq!(json_output(&invalid_state)?["status"], "failed");
    fs::remove_file(invalid_journal)?;

    let receipt_path = home.path().join(".agents/.arthur-workflow/receipt.json");
    let committed_receipt = fs::read(&receipt_path)?;
    let mut recovery_receipt = serde_json::from_slice::<Value>(&committed_receipt)?;
    recovery_receipt["state"] = Value::String("recovery_required".to_owned());
    fs::write(&receipt_path, serde_json::to_vec_pretty(&recovery_receipt)?)?;
    for command in ["status", "doctor"] {
        let output = run(home.path(), &["--json", command])?;
        assert!(!output.status.success());
        assert_eq!(json_output(&output)?["status"], "recovery_required");
    }
    for arguments in [
        vec!["--json", "plan", "--provider", "claude"],
        vec!["--json", "install", "--provider", "claude", "--dry-run"],
        vec!["--json", "update", "--dry-run"],
        vec!["--json", "uninstall", "--all", "--dry-run"],
        vec!["--json", "adopt", "--provider", "claude", "--dry-run"],
    ] {
        let output = run(home.path(), &arguments)?;
        assert_eq!(output.status.code(), Some(3), "{arguments:?}");
        assert_eq!(json_output(&output)?["status"], "blocked");
    }
    fs::write(&receipt_path, committed_receipt)?;

    let missing_scope = run(home.path(), &["--json", "uninstall", "--yes"])?;
    assert_eq!(missing_scope.status.code(), Some(2));

    let foreign_provider = run(
        home.path(),
        &["--json", "uninstall", "--provider", "codex", "--yes"],
    )?;
    assert_eq!(foreign_provider.status.code(), Some(2));
    assert!(
        home.path()
            .join(".agents/skills/meta-code/SKILL.md")
            .exists()
    );

    for arguments in [
        vec!["--json", "status"],
        vec!["--json", "doctor"],
        vec!["--json", "update", "--yes"],
    ] {
        let output = run(home.path(), &arguments)?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    let already_current = run(
        home.path(),
        &["--json", "install", "--provider", "claude", "--yes"],
    )?;
    assert!(already_current.status.success());
    assert_eq!(json_output(&already_current)?["status"], "noop");

    let add_codex = run(
        home.path(),
        &["--json", "install", "--provider", "claude,codex", "--yes"],
    )?;
    assert!(add_codex.status.success());

    let ambiguous_scope = run(
        home.path(),
        &["--json", "uninstall", "--provider", "claude,codex", "--yes"],
    )?;
    assert_eq!(ambiguous_scope.status.code(), Some(2));

    let remove_codex = run(
        home.path(),
        &["--json", "uninstall", "--provider", "codex", "--yes"],
    )?;
    assert!(remove_codex.status.success());

    let uninstall = run(home.path(), &["--json", "uninstall", "--all", "--yes"])?;
    assert!(
        uninstall.status.success(),
        "{}",
        String::from_utf8_lossy(&uninstall.stdout)
    );
    let recover = run(home.path(), &["--json", "recover"])?;
    assert!(recover.status.success());
    assert_eq!(json_output(&recover)?["status"], "noop");
    Ok(())
}

#[test]
fn matching_vercel_v3_installation_can_be_adopted_atomically() -> TestResult {
    let home = tempfile::tempdir()?;
    let install = run(
        home.path(),
        &["--json", "install", "--provider", "claude", "--yes"],
    )?;
    assert!(install.status.success());

    fs::rename(
        home.path().join(".agents/.arthur-workflow/receipt.json"),
        home.path()
            .join(".agents/.arthur-workflow/pre-adoption-receipt.json"),
    )?;

    let missing_lock = run(
        home.path(),
        &["--json", "adopt", "--provider", "claude", "--dry-run"],
    )?;
    assert_eq!(missing_lock.status.code(), Some(3));
    assert_eq!(json_output(&missing_lock)?["status"], "blocked");

    fs::write(
        home.path().join(".agents/.skill-lock.json"),
        br#"{"version":3,"skills":{}}"#,
    )?;
    let blocked = run(
        home.path(),
        &["--json", "adopt", "--provider", "claude", "--dry-run"],
    )?;
    assert_eq!(blocked.status.code(), Some(3));
    assert_eq!(json_output(&blocked)?["status"], "blocked");

    write_v3_lock_for_installed_skills(home.path())?;

    let dry_run = run(
        home.path(),
        &["--json", "adopt", "--provider", "claude", "--dry-run"],
    )?;
    assert!(
        dry_run.status.success(),
        "{}",
        String::from_utf8_lossy(&dry_run.stdout)
    );
    let envelope = json_output(&dry_run)?;
    assert_eq!(envelope["status"], "success");
    assert_eq!(envelope["data"]["applied"], false);

    let missing_confirmation = run(home.path(), &["--json", "adopt", "--provider", "claude"])?;
    assert_eq!(missing_confirmation.status.code(), Some(2));

    let adopt = run(
        home.path(),
        &["--json", "adopt", "--provider", "claude", "--yes"],
    )?;
    assert!(
        adopt.status.success(),
        "{}",
        String::from_utf8_lossy(&adopt.stdout)
    );
    let envelope = json_output(&adopt)?;
    assert_eq!(envelope["data"]["applied"], true);
    assert!(
        home.path()
            .join(".agents/.arthur-workflow/receipt.json")
            .exists()
    );
    assert!(
        home.path()
            .join(".agents/.arthur-workflow/vercel-skills-v3-lock.json")
            .exists()
    );
    Ok(())
}

#[test]
fn malformed_and_unreadable_receipts_fail_closed() -> TestResult {
    let malformed_home = tempfile::tempdir()?;
    let receipt = malformed_home
        .path()
        .join(".agents/.arthur-workflow/receipt.json");
    fs::create_dir_all(receipt.parent().ok_or("receipt has no parent")?)?;
    fs::write(&receipt, b"not json")?;
    for arguments in [
        vec!["--json", "status"],
        vec!["--json", "doctor"],
        vec!["--json", "update", "--dry-run"],
        vec!["--json", "uninstall", "--all", "--dry-run"],
        vec!["--json", "recover"],
        vec!["--json", "plan", "--provider", "claude"],
        vec!["--json", "install", "--provider", "claude", "--dry-run"],
        vec!["--json", "adopt", "--provider", "claude", "--dry-run"],
    ] {
        let malformed = run(malformed_home.path(), &arguments)?;
        assert!(!malformed.status.success(), "{arguments:?}");
        assert_eq!(json_output(&malformed)?["status"], "failed");
    }

    let unreadable_home = tempfile::tempdir()?;
    let receipt = unreadable_home
        .path()
        .join(".agents/.arthur-workflow/receipt.json");
    fs::create_dir_all(&receipt)?;
    let unreadable = run(unreadable_home.path(), &["--json", "status"])?;
    assert!(!unreadable.status.success());
    assert_eq!(json_output(&unreadable)?["status"], "failed");
    Ok(())
}

#[test]
fn human_pipe_and_plain_environment_emit_no_terminal_controls() -> TestResult {
    let home = tempfile::tempdir()?;
    for arguments in [
        vec!["plan", "--provider", "claude"],
        vec!["--plain", "plan", "--provider", "claude"],
    ] {
        let output = run(home.path(), &arguments)?;
        assert!(output.status.success());
        assert!(!output.stdout.contains(&b'\r'));
        assert!(!output.stdout.contains(&0x1b));
    }
    let no_color = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["plan", "--provider", "claude"])
        .env("HOME", home.path())
        .env("NO_COLOR", "1")
        .output()?;
    assert!(!no_color.stdout.contains(&0x1b));
    Ok(())
}

#[test]
fn portable_skill_root_preserves_spaces_and_unicode() -> TestResult {
    let home = "/tmp/Arthur Équipe";
    let output = Command::new("sh")
        .arg("-c")
        .arg(concat!(
            "VERCEL_SKILL_DIR=\"${VERCEL_SKILL_DIR:-$HOME/.agents/skills/vercel-cli}\"; ",
            "printf '%s' \"$VERCEL_SKILL_DIR/scripts/vercel-api.sh\""
        ))
        .env("HOME", home)
        .env_remove("VERCEL_SKILL_DIR")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        format!("{home}/.agents/skills/vercel-cli/scripts/vercel-api.sh")
    );
    Ok(())
}
