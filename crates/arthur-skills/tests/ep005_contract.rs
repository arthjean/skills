#![cfg(unix)]
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use arthur_skills::app::{App, Provider, Review};
use arthur_skills::plan::{Owner, PLAN_SCHEMA_VERSION, Plan, PlanAction, PlanEntry};
use arthur_skills::provider::resolve_roots_from;
use arthur_skills::ui::render;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn run(home: &Path, path: &Path, arguments: &[&str]) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(arguments)
        .env("HOME", home)
        .env("PATH", path)
        .env_remove("CODEX_HOME")
        .output()
}

fn json(output: &Output) -> Result<Value, serde_json::Error> {
    serde_json::from_slice(&output.stdout)
}

fn empty_path(home: &Path) -> Result<PathBuf, std::io::Error> {
    let path = home.join("empty-bin");
    fs::create_dir(&path)?;
    Ok(path)
}

fn assert_human_json_agree(home: &Path, path: &Path, arguments: &[&str]) -> TestResult {
    let mut json_arguments = vec!["--json"];
    json_arguments.extend_from_slice(arguments);
    let mut human_arguments = vec!["--plain"];
    human_arguments.extend_from_slice(arguments);
    let machine = run(home, path, &json_arguments)?;
    let human = run(home, path, &human_arguments)?;
    assert_eq!(
        human.status.code(),
        machine.status.code(),
        "exit code differs for {arguments:?}"
    );

    let envelope = json(&machine)?;
    let human = String::from_utf8(human.stdout)?;
    for (action, count) in envelope["summary"]
        .as_object()
        .ok_or("summary is not an object")?
    {
        assert!(
            human.contains(&format!("{action}: {count}")),
            "human summary omits {action} for {arguments:?}"
        );
    }
    for operation in envelope["operations"]
        .as_array()
        .ok_or("operations are not an array")?
    {
        let destination = operation["destination_utf8"]
            .as_str()
            .or_else(|| operation["destination_bytes_hex"].as_str())
            .ok_or("operation destination is absent")?;
        let reason = operation["reason"]
            .as_str()
            .ok_or("operation reason is absent")?;
        assert!(
            human.lines().any(|line| {
                line.contains(destination)
                    && line.contains(reason)
                    && line
                        .to_ascii_lowercase()
                        .contains(operation["action"].as_str().unwrap_or_default())
            }),
            "human operation differs for {arguments:?}: {operation}"
        );
    }
    for diagnostic in envelope["diagnostics"]
        .as_array()
        .ok_or("diagnostics are not an array")?
    {
        let code = diagnostic["code"]
            .as_str()
            .ok_or("diagnostic code is absent")?;
        let message = diagnostic["message"]
            .as_str()
            .ok_or("diagnostic message is absent")?;
        assert!(
            human.contains(&format!("{code}: {message}")),
            "human diagnostic differs for {arguments:?}"
        );
    }
    if !envelope["data"].is_null() {
        assert!(
            human.contains(&envelope["data"].to_string()),
            "human data differs for {arguments:?}"
        );
    }
    Ok(())
}

#[test]
fn review_projection_labels_unknown_roots_and_renders_applicable_plans() -> TestResult {
    let home = tempfile::tempdir()?;
    let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[Provider::Claude])?;
    let plan = |applicable, destination| Plan {
        schema_version: PLAN_SCHEMA_VERSION,
        applicable,
        entries: vec![PlanEntry {
            action: PlanAction::Noop,
            source: "skills/example/SKILL.md".to_owned(),
            destination,
            owner: Owner::ArthurWorkflow,
            reason: "already current".to_owned(),
        }],
        operations: Vec::new(),
        diagnostics: Vec::new(),
    };

    let unknown = Review::from_plan(
        &plan(false, home.path().join("outside/SKILL.md")),
        &[],
        &roots,
    );
    assert!(
        unknown
            .groups
            .keys()
            .any(|(root, _)| root == "unknown root")
    );

    let applicable = Review::from_plan(
        &plan(true, roots.canonical_skills.join("example/SKILL.md")),
        &[],
        &roots,
    );
    let mut app = App::with_selection(1, &[Provider::Claude]);
    app.set_review(applicable);
    let backend = TestBackend::new(82, 16);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render(frame, &app, true))?;
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Enter apply"));
    Ok(())
}

#[test]
fn status_and_doctor_prove_owned_state_and_detect_drift() -> TestResult {
    let home = tempfile::tempdir()?;
    let path = empty_path(home.path())?;
    let install = run(
        home.path(),
        &path,
        &["--json", "install", "--provider", "codex", "--yes"],
    )?;
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stdout)
    );

    let status = run(home.path(), &path, &["--json", "status"])?;
    assert!(status.status.success());
    let status_json = json(&status)?;
    assert_eq!(status_json["data"]["catalog_current"], true);
    assert_eq!(status_json["data"]["roots_match"], true);
    assert_eq!(status_json["data"]["implicit_codex_visibility"], true);
    assert_eq!(status_json["data"]["counts"]["drifted"], 0);
    assert_eq!(status_json["data"]["counts"]["missing"], 0);
    assert_eq!(status_json["data"]["counts"]["conflicting"], 0);

    let doctor = run(home.path(), &path, &["--json", "doctor"])?;
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stdout)
    );
    let doctor_json = json(&doctor)?;
    assert_eq!(doctor_json["data"]["healthy"], true);
    assert!(
        doctor_json["diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| {
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic["code"] == "provider_cli_missing")
            })
    );

    let managed = home.path().join(".agents/skills/baseline-ui/SKILL.md");
    let original = fs::read(&managed)?;
    fs::write(&managed, b"local drift")?;
    let status = run(home.path(), &path, &["--json", "status"])?;
    assert!(status.status.success());
    assert_eq!(json(&status)?["data"]["counts"]["drifted"], 1);
    let doctor = run(home.path(), &path, &["--json", "doctor"])?;
    assert_eq!(doctor.status.code(), Some(3));
    assert_eq!(json(&doctor)?["data"]["healthy"], false);

    fs::write(&managed, original)?;
    let receipt_path = home.path().join(".agents/.arthur-workflow/receipt.json");
    let mut receipt = serde_json::from_slice::<Value>(&fs::read(&receipt_path)?)?;
    receipt["assets"]
        .as_array_mut()
        .ok_or("receipt assets are not an array")?
        .retain(|asset| asset["destination"] != managed.to_string_lossy().as_ref());
    fs::remove_file(&managed)?;
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    let doctor = run(home.path(), &path, &["--json", "doctor"])?;
    assert_eq!(doctor.status.code(), Some(3));
    let doctor = json(&doctor)?;
    assert!(doctor["data"]["checks"]["counts"]["missing"].as_u64() > Some(0));
    assert!(doctor["diagnostics"].as_array().is_some_and(|diagnostics| {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "catalog_reconciliation_required")
    }));
    Ok(())
}

#[test]
fn provider_minimum_root_identity_and_downgrade_fail_closed() -> TestResult {
    let home = tempfile::tempdir()?;
    let path = home.path().join("bin");
    fs::create_dir(&path)?;
    let codex = path.join("codex");
    fs::write(&codex, b"#!/bin/sh\nprintf 'codex-cli 0.143.9\\n'\n")?;
    fs::set_permissions(&codex, fs::Permissions::from_mode(0o755))?;
    let install = run(
        home.path(),
        &path,
        &["--json", "install", "--provider", "codex", "--yes"],
    )?;
    assert!(install.status.success());

    let doctor = run(home.path(), &path, &["--json", "doctor"])?;
    assert_eq!(doctor.status.code(), Some(3));
    let doctor_json = json(&doctor)?;
    assert!(
        doctor_json["diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| {
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic["code"] == "provider_incompatible")
            })
    );

    fs::set_permissions(&codex, fs::Permissions::from_mode(0o777))?;
    let unsafe_path = run(home.path(), &path, &["--json", "doctor"])?;
    assert_eq!(unsafe_path.status.code(), Some(3));
    assert!(
        json(&unsafe_path)?["diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .any(|diagnostic| diagnostic["code"] == "provider_path_unsafe"))
    );

    fs::write(&codex, b"#!/bin/sh\nexit 7\n")?;
    fs::set_permissions(&codex, fs::Permissions::from_mode(0o755))?;
    let failed_probe = run(home.path(), &path, &["--json", "doctor"])?;
    assert_eq!(failed_probe.status.code(), Some(3));
    assert!(
        json(&failed_probe)?["diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .any(|diagnostic| diagnostic["code"] == "provider_cli_failed"))
    );

    fs::write(&codex, b"#!/bin/sh\nprintf 'codex-cli 0.144\\n'\n")?;
    let invalid_probe = run(home.path(), &path, &["--json", "doctor"])?;
    assert_eq!(invalid_probe.status.code(), Some(3));
    assert!(
        json(&invalid_probe)?["diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| diagnostics
                .iter()
                .any(|diagnostic| diagnostic["code"] == "provider_incompatible"))
    );

    fs::write(&codex, b"#!/bin/sh\nprintf 'codex-cli 0.144.6\\n'\n")?;
    fs::set_permissions(&codex, fs::Permissions::from_mode(0o755))?;
    assert!(
        run(home.path(), &path, &["--json", "doctor"])?
            .status
            .success()
    );

    let alternate_codex = home.path().join("alternate-codex");
    fs::create_dir(&alternate_codex)?;
    let managed_before = fs::read(home.path().join(".agents/skills/baseline-ui/SKILL.md"))?;
    for arguments in [
        &["--json", "status"][..],
        &["--json", "doctor"][..],
        &["--json", "update", "--yes"][..],
        &["--json", "recover"][..],
    ] {
        let root_mismatch = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
            .args(arguments)
            .env("HOME", home.path())
            .env("CODEX_HOME", &alternate_codex)
            .env("PATH", &path)
            .output()?;
        assert_eq!(root_mismatch.status.code(), Some(4), "{arguments:?}");
        assert!(
            json(&root_mismatch)?["diagnostics"]
                .as_array()
                .is_some_and(|diagnostics| diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic["code"] == "root_mismatch"))
        );
    }
    assert_eq!(
        fs::read(home.path().join(".agents/skills/baseline-ui/SKILL.md"))?,
        managed_before,
    );

    let receipt_path = home.path().join(".agents/.arthur-workflow/receipt.json");
    let mut receipt = serde_json::from_slice::<Value>(&fs::read(&receipt_path)?)?;
    receipt["cli_version"] = Value::String("9.0.0".to_owned());
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    let before = fs::read(home.path().join(".agents/skills/baseline-ui/SKILL.md"))?;
    let update = run(home.path(), &path, &["--json", "update", "--yes"])?;
    assert_eq!(update.status.code(), Some(3));
    assert_eq!(
        json(&update)?["diagnostics"][0]["code"],
        "downgrade_refused"
    );
    assert_eq!(
        fs::read(home.path().join(".agents/skills/baseline-ui/SKILL.md"))?,
        before
    );

    receipt["cli_version"] = Value::String("invalid".to_owned());
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    let invalid_version = run(home.path(), &path, &["--json", "update", "--dry-run"])?;
    assert_eq!(invalid_version.status.code(), Some(3));
    assert_eq!(
        json(&invalid_version)?["diagnostics"][0]["code"],
        "installed_version_invalid"
    );

    receipt["cli_version"] = Value::String(env!("CARGO_PKG_VERSION").to_owned());
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    let journal = home
        .path()
        .join(".agents/.arthur-workflow/transaction.json");
    fs::write(&journal, b"not json")?;
    for arguments in [
        &["--json", "update", "--dry-run"][..],
        &["--json", "recover"][..],
    ] {
        let output = run(home.path(), &path, arguments)?;
        assert_eq!(output.status.code(), Some(5));
    }
    fs::remove_file(journal)?;

    let invalid_codex_home = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["--json", "update", "--dry-run"])
        .env("HOME", home.path())
        .env("CODEX_HOME", "")
        .env("PATH", &path)
        .output()?;
    assert_eq!(invalid_codex_home.status.code(), Some(4));
    Ok(())
}

#[test]
fn fresh_install_matrix_is_idempotent_and_human_json_plans_agree() -> TestResult {
    for providers in ["claude", "codex", "claude,codex"] {
        let home = tempfile::tempdir()?;
        let path = empty_path(home.path())?;
        let install = run(
            home.path(),
            &path,
            &["--json", "install", "--provider", providers, "--yes"],
        )?;
        assert!(install.status.success(), "provider matrix {providers}");
        let doctor = run(home.path(), &path, &["--json", "doctor"])?;
        assert!(doctor.status.success(), "provider matrix {providers}");
        let doctor = json(&doctor)?;
        let receipt_path = home.path().join(".agents/.arthur-workflow/receipt.json");
        let receipt = serde_json::from_slice::<Value>(&fs::read(&receipt_path)?)?;
        let assets = receipt["assets"]
            .as_array()
            .ok_or("receipt assets are not an array")?;
        assert_eq!(
            doctor["data"]["checks"]["counts"]["healthy"].as_u64(),
            u64::try_from(assets.len()).ok(),
        );

        let modified = assets
            .iter()
            .filter_map(|asset| asset["destination"].as_str())
            .map(|destination| {
                let path = PathBuf::from(destination);
                Ok((path.clone(), fs::symlink_metadata(path)?.modified()?))
            })
            .collect::<Result<BTreeMap<_, _>, std::io::Error>>()?;
        let second = run(
            home.path(),
            &path,
            &["--json", "install", "--provider", providers, "--yes"],
        )?;
        assert!(second.status.success());
        let second = json(&second)?;
        assert_eq!(second["status"], "noop");
        assert!(second["operations"].as_array().is_some_and(|operations| {
            operations
                .iter()
                .all(|operation| operation["action"] == "noop")
        }));
        for (path, timestamp) in modified {
            assert_eq!(fs::symlink_metadata(path)?.modified()?, timestamp);
        }
    }

    let home = tempfile::tempdir()?;
    let path = empty_path(home.path())?;
    let machine = run(
        home.path(),
        &path,
        &["--json", "plan", "--provider", "claude,codex"],
    )?;
    let human = run(
        home.path(),
        &path,
        &["--plain", "plan", "--provider", "claude,codex"],
    )?;
    assert!(machine.status.success() && human.status.success());
    let machine = json(&machine)?;
    let human = String::from_utf8(human.stdout)?;
    for (action, count) in machine["summary"]
        .as_object()
        .ok_or("summary is not an object")?
    {
        assert!(human.contains(&format!("{action}: {count}")));
    }
    Ok(())
}

#[test]
fn every_lifecycle_stage_has_equivalent_human_and_json_evidence() -> TestResult {
    let home = tempfile::tempdir()?;
    let path = empty_path(home.path())?;
    assert_human_json_agree(home.path(), &path, &["plan", "--provider", "claude,codex"])?;
    assert_human_json_agree(
        home.path(),
        &path,
        &["install", "--provider", "claude,codex", "--dry-run"],
    )?;
    assert!(
        run(
            home.path(),
            &path,
            &["--json", "install", "--provider", "claude,codex", "--yes",],
        )?
        .status
        .success()
    );
    assert_human_json_agree(home.path(), &path, &["status"])?;
    assert_human_json_agree(home.path(), &path, &["doctor"])?;
    assert_human_json_agree(home.path(), &path, &["update", "--dry-run"])?;
    assert_human_json_agree(
        home.path(),
        &path,
        &["uninstall", "--provider", "codex", "--dry-run"],
    )?;
    assert!(
        run(
            home.path(),
            &path,
            &["--json", "uninstall", "--provider", "codex", "--yes",],
        )?
        .status
        .success()
    );
    assert_human_json_agree(home.path(), &path, &["uninstall", "--all", "--dry-run"])?;
    Ok(())
}

#[test]
fn future_receipt_keeps_read_only_recovery_evidence() -> TestResult {
    let home = tempfile::tempdir()?;
    let path = empty_path(home.path())?;
    assert!(
        run(
            home.path(),
            &path,
            &["--json", "install", "--provider", "claude", "--yes"],
        )?
        .status
        .success()
    );
    let receipt_path = home.path().join(".agents/.arthur-workflow/receipt.json");
    let mut receipt = serde_json::from_slice::<Value>(&fs::read(&receipt_path)?)?;
    receipt["schema_version"] = Value::from(99);
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;

    for command in ["status", "doctor"] {
        let output = run(home.path(), &path, &["--json", command])?;
        assert_eq!(output.status.code(), Some(5));
        let output = json(&output)?;
        assert_eq!(output["data"]["receipt_readable"], false);
        assert_eq!(output["data"]["recover_available"], false);
        assert_eq!(output["diagnostics"][0]["code"], "receipt_invalid");
    }
    Ok(())
}

#[test]
fn ci_configuration_preserves_quality_contracts() -> TestResult {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ci = fs::read_to_string(workspace.join(".github/workflows/ci.yml"))?;
    for contract in [
        "runner: [ubuntu-24.04, macos-15, windows-2025]",
        "cargo fmt --all -- --check",
        "cargo check --workspace --all-targets --all-features",
        "cargo clippy --workspace --all-targets --all-features",
        "cargo test --workspace --all-targets --all-features",
        "cargo llvm-cov --workspace --all-features --fail-under-regions 90",
        "cargo deny check",
        "cargo test -p arthur-skills --all-targets --all-features",
        "cargo-llvm-cov@0.8.4,cargo-deny@0.19.8",
    ] {
        assert!(ci.contains(contract), "missing CI contract: {contract}");
    }
    Ok(())
}
