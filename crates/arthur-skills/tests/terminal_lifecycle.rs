#![cfg(unix)]
#![forbid(unsafe_code)]

use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use nix::fcntl::{FcntlArg, OFlag, fcntl};
use nix::pty::{Winsize, openpty};
use nix::sys::termios::{LocalFlags, tcgetattr};

#[test]
fn inline_terminal_restores_after_success_error_and_panic() -> Result<(), Box<dyn Error>> {
    for probe in ["success", "error", "panic"] {
        let result = run_probe(probe)?;
        assert_eq!(result.initial_flags, result.final_flags, "probe {probe:?}");
        assert!(
            !result.output.contains("\u{1b}[?1049h"),
            "alternate screen entered"
        );
        if let Some(hidden) = result.output.find("\u{1b}[?25l") {
            let shown = result.output.rfind("\u{1b}[?25h");
            assert!(
                shown.is_some_and(|shown| shown > hidden),
                "cursor was not restored"
            );
        }
        assert_eq!(result.success, probe == "success", "probe {probe:?}");
    }
    Ok(())
}

#[test]
fn ctrl_c_before_confirmation_restores_terminal_and_mutates_nothing() -> Result<(), Box<dyn Error>>
{
    let home = tempfile::tempdir()?;
    let result = run_ctrl_c(home.path())?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(130));
    assert!(result.output.contains("Select providers"));
    assert!(!home.path().join(".agents").exists());
    assert!(!home.path().join(".claude").exists());
    assert!(!home.path().join(".codex").exists());
    Ok(())
}

#[test]
fn plain_install_covers_selection_validation_review_and_confirmation() -> Result<(), Box<dyn Error>>
{
    let cancelled_selection = tempfile::tempdir()?;
    let result = run_install_session(
        cancelled_selection.path(),
        &["install"],
        Interaction::Plain(b"invalid\n1\n2\n\nq\n"),
    )?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.output.contains("Invalid input"));
    assert!(result.output.contains("Select at least one provider"));
    assert!(!cancelled_selection.path().join(".agents").exists());

    let cancelled_review = tempfile::tempdir()?;
    let result = run_install_session(
        cancelled_review.path(),
        &["install"],
        Interaction::Plain(b"\nn\n"),
    )?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.output.contains("Apply this complete plan?"));
    assert!(!cancelled_review.path().join(".agents").exists());

    let confirmed = tempfile::tempdir()?;
    let result = run_install_session(confirmed.path(), &["install"], Interaction::Plain(b"\ny\n"))?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(0));
    assert!(
        confirmed
            .path()
            .join(".agents/.arthur-workflow/receipt.json")
            .exists()
    );

    let uninstall =
        run_install_session(confirmed.path(), &["uninstall"], Interaction::Plain(b"n\n"))?;
    assert_eq!(uninstall.exit_code, Some(0));
    assert!(uninstall.output.contains("Apply this complete plan?"));
    assert!(
        confirmed
            .path()
            .join(".agents/.arthur-workflow/receipt.json")
            .exists()
    );
    Ok(())
}

#[test]
fn tui_install_selects_reviews_and_commits_inline() -> Result<(), Box<dyn Error>> {
    let home = tempfile::tempdir()?;
    let result = run_install_session(home.path(), &["install"], Interaction::TuiConfirm)?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.output.contains("Select providers"));
    assert!(result.output.contains("Review filesystem plan"));
    assert!(!result.output.contains("\u{1b}[?1049h"));
    assert!(
        home.path()
            .join(".agents/.arthur-workflow/receipt.json")
            .exists()
    );
    Ok(())
}

#[test]
fn tui_interrupt_during_review_exits_before_mutation() -> Result<(), Box<dyn Error>> {
    let home = tempfile::tempdir()?;
    let result = run_install_session(home.path(), &["install"], Interaction::TuiInterruptReview)?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(130));
    assert!(result.output.contains("Review filesystem plan"));
    assert!(!home.path().join(".agents").exists());
    Ok(())
}

#[test]
fn plain_adoption_reviews_and_commits_verified_entries() -> Result<(), Box<dyn Error>> {
    let home = tempfile::tempdir()?;
    prepare_adoption(home.path())?;
    let result = run_install_session(
        home.path(),
        &["adopt", "--provider", "claude"],
        Interaction::Plain(b"y\n"),
    )?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.output.contains("Apply this complete plan?"));
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
fn tui_adoption_interrupts_during_review_before_mutation() -> Result<(), Box<dyn Error>> {
    let home = tempfile::tempdir()?;
    prepare_adoption(home.path())?;
    let result = run_install_session(
        home.path(),
        &["adopt", "--provider", "claude"],
        Interaction::TuiInterruptAdoption,
    )?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(130));
    assert!(result.output.contains("Review filesystem plan"));
    assert!(
        !home
            .path()
            .join(".agents/.arthur-workflow/receipt.json")
            .exists()
    );
    Ok(())
}

#[test]
fn tui_adoption_can_cancel_then_confirm_the_same_verified_plan() -> Result<(), Box<dyn Error>> {
    let home = tempfile::tempdir()?;
    prepare_adoption(home.path())?;

    let cancelled = run_install_session(
        home.path(),
        &["adopt", "--provider", "claude"],
        Interaction::TuiCancelAdoption,
    )?;
    assert_eq!(cancelled.initial_flags, cancelled.final_flags);
    assert_eq!(cancelled.exit_code, Some(0));
    assert!(
        !home
            .path()
            .join(".agents/.arthur-workflow/receipt.json")
            .exists()
    );

    let confirmed = run_install_session(
        home.path(),
        &["adopt", "--provider", "claude"],
        Interaction::TuiConfirmAdoption,
    )?;
    assert_eq!(confirmed.initial_flags, confirmed.final_flags);
    assert_eq!(confirmed.exit_code, Some(0));
    assert!(
        home.path()
            .join(".agents/.arthur-workflow/receipt.json")
            .exists()
    );
    Ok(())
}

#[test]
fn sigterm_interrupts_plain_selection_before_mutation() -> Result<(), Box<dyn Error>> {
    let home = tempfile::tempdir()?;
    let result = run_install_session(home.path(), &["install"], Interaction::PlainSignal)?;
    assert_eq!(result.initial_flags, result.final_flags);
    assert_eq!(result.exit_code, Some(143));
    assert!(result.output.contains("Provider selection"));
    assert!(!home.path().join(".agents").exists());
    Ok(())
}

struct ProbeResult {
    initial_flags: LocalFlags,
    final_flags: LocalFlags,
    output: String,
    success: bool,
}

struct InterruptResult {
    initial_flags: LocalFlags,
    final_flags: LocalFlags,
    output: String,
    exit_code: Option<i32>,
}

#[derive(Clone, Copy)]
enum Interaction<'a> {
    Plain(&'a [u8]),
    PlainSignal,
    TuiConfirm,
    TuiInterruptReview,
    TuiCancelAdoption,
    TuiConfirmAdoption,
    TuiInterruptAdoption,
}

fn run_probe(probe: &str) -> Result<ProbeResult, Box<dyn Error>> {
    let dimensions = Winsize {
        ws_row: 24,
        ws_col: 100,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pair = openpty(Some(&dimensions), None)?;
    let initial = tcgetattr(&pair.slave)?;
    let slave = File::from(pair.slave);
    let mut command = Command::new(env!("CARGO_BIN_EXE_arthur-skills"));
    command
        .env("TERM", "xterm-256color")
        .env("ARTHUR_SKILLS_TUI_PROBE", probe)
        .env_remove("CI")
        .env_remove("ARTHUR_SKILLS_PLAIN")
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave.try_clone()?));

    let mut child = command.spawn()?;
    let mut master = File::from(pair.master);
    drop(slave);
    fcntl(&master, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut cursor_queries_answered = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        match master.read(&mut buffer) {
            Ok(0) => {}
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                let cursor_queries = bytes
                    .windows(b"\x1b[6n".len())
                    .filter(|window| *window == b"\x1b[6n")
                    .count();
                while cursor_queries_answered < cursor_queries {
                    master.write_all(b"\x1b[1;1R")?;
                    master.flush()?;
                    cursor_queries_answered += 1;
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(5) => {}
            Err(error) => return Err(error.into()),
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err("terminal probe timed out".into());
        }
        thread::sleep(Duration::from_millis(5));
    };
    let final_state = tcgetattr(&master)?;
    let relevant = LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::ISIG | LocalFlags::IEXTEN;

    Ok(ProbeResult {
        initial_flags: initial.local_flags & relevant,
        final_flags: final_state.local_flags & relevant,
        output: String::from_utf8_lossy(&bytes).into_owned(),
        success: status.success(),
    })
}

fn run_ctrl_c(home: &std::path::Path) -> Result<InterruptResult, Box<dyn Error>> {
    let dimensions = Winsize {
        ws_row: 24,
        ws_col: 100,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pair = openpty(Some(&dimensions), None)?;
    let initial = tcgetattr(&pair.slave)?;
    let slave = File::from(pair.slave);
    let mut child = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .arg("install")
        .env("HOME", home)
        .env("TERM", "xterm-256color")
        .env_remove("CI")
        .env_remove("CODEX_HOME")
        .env_remove("ARTHUR_SKILLS_PLAIN")
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave.try_clone()?))
        .spawn()?;
    let mut master = File::from(pair.master);
    drop(slave);
    fcntl(&master, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut cursor_queries_answered = 0;
    let mut interrupted = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        match master.read(&mut buffer) {
            Ok(0) => {}
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                let cursor_queries = bytes
                    .windows(b"\x1b[6n".len())
                    .filter(|window| *window == b"\x1b[6n")
                    .count();
                while cursor_queries_answered < cursor_queries {
                    master.write_all(b"\x1b[1;1R")?;
                    cursor_queries_answered += 1;
                }
                if !interrupted && String::from_utf8_lossy(&bytes).contains("Select providers") {
                    master.write_all(&[3])?;
                    master.flush()?;
                    interrupted = true;
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(5) => {}
            Err(error) => return Err(error.into()),
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err("Ctrl+C terminal probe timed out".into());
        }
        thread::sleep(Duration::from_millis(5));
    };
    let final_state = tcgetattr(&master)?;
    let relevant = LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::ISIG | LocalFlags::IEXTEN;
    Ok(InterruptResult {
        initial_flags: initial.local_flags & relevant,
        final_flags: final_state.local_flags & relevant,
        output: String::from_utf8_lossy(&bytes).into_owned(),
        exit_code: status.code(),
    })
}

fn run_install_session(
    home: &std::path::Path,
    arguments: &[&str],
    interaction: Interaction<'_>,
) -> Result<InterruptResult, Box<dyn Error>> {
    let dimensions = Winsize {
        ws_row: 24,
        ws_col: 100,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pair = openpty(Some(&dimensions), None)?;
    let initial = tcgetattr(&pair.slave)?;
    let slave = File::from(pair.slave);
    let mut command = Command::new(env!("CARGO_BIN_EXE_arthur-skills"));
    if matches!(
        interaction,
        Interaction::Plain(_) | Interaction::PlainSignal
    ) {
        command.arg("--plain");
    }
    let mut child = command
        .args(arguments)
        .env("HOME", home)
        .env("TERM", "xterm-256color")
        .env_remove("CI")
        .env_remove("CODEX_HOME")
        .env_remove("ARTHUR_SKILLS_PLAIN")
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave.try_clone()?))
        .spawn()?;
    let mut master = File::from(pair.master);
    drop(slave);
    fcntl(&master, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

    if let Interaction::Plain(input) = interaction {
        master.write_all(input)?;
        master.flush()?;
    }

    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut cursor_queries_answered = 0;
    let mut tui_stage = 0;
    let mut plain_signal_sent = false;
    let deadline = Instant::now() + Duration::from_secs(180);
    let status = loop {
        match master.read(&mut buffer) {
            Ok(0) => {}
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                let cursor_queries = bytes
                    .windows(b"\x1b[6n".len())
                    .filter(|window| *window == b"\x1b[6n")
                    .count();
                while cursor_queries_answered < cursor_queries {
                    master.write_all(b"\x1b[1;1R")?;
                    cursor_queries_answered += 1;
                }
                if matches!(
                    interaction,
                    Interaction::TuiConfirm | Interaction::TuiInterruptReview
                ) {
                    let output = String::from_utf8_lossy(&bytes);
                    if tui_stage == 0 && output.contains("Select providers") {
                        master.write_all(b"\r")?;
                        master.flush()?;
                        tui_stage = 1;
                    } else if tui_stage == 1 && output.contains("Review filesystem plan") {
                        let input = if matches!(interaction, Interaction::TuiConfirm) {
                            b"\r".as_slice()
                        } else {
                            &[3]
                        };
                        master.write_all(input)?;
                        master.flush()?;
                        tui_stage = 2;
                    }
                }
                if matches!(
                    interaction,
                    Interaction::TuiCancelAdoption
                        | Interaction::TuiConfirmAdoption
                        | Interaction::TuiInterruptAdoption
                ) && tui_stage == 0
                    && String::from_utf8_lossy(&bytes).contains("Review filesystem plan")
                {
                    let input = match interaction {
                        Interaction::TuiCancelAdoption => b"q".as_slice(),
                        Interaction::TuiConfirmAdoption => b"\r".as_slice(),
                        Interaction::TuiInterruptAdoption => &[3],
                        _ => unreachable!(),
                    };
                    master.write_all(input)?;
                    master.flush()?;
                    tui_stage = 2;
                }
                if matches!(interaction, Interaction::PlainSignal)
                    && !plain_signal_sent
                    && String::from_utf8_lossy(&bytes).contains("Provider selection")
                {
                    let signal = Command::new("kill")
                        .args(["-TERM", &child.id().to_string()])
                        .status()?;
                    if !signal.success() {
                        return Err("failed to signal interactive process".into());
                    }
                    master.write_all(b"\n")?;
                    master.flush()?;
                    plain_signal_sent = true;
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(5) => {}
            Err(error) => return Err(error.into()),
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err("interactive install timed out".into());
        }
        thread::sleep(Duration::from_millis(5));
    };
    let final_state = tcgetattr(&master)?;
    let relevant = LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::ISIG | LocalFlags::IEXTEN;
    Ok(InterruptResult {
        initial_flags: initial.local_flags & relevant,
        final_flags: final_state.local_flags & relevant,
        output: String::from_utf8_lossy(&bytes).into_owned(),
        exit_code: status.code(),
    })
}

fn prepare_adoption(home: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let install = Command::new(env!("CARGO_BIN_EXE_arthur-skills"))
        .args(["--json", "install", "--provider", "claude", "--yes"])
        .env("HOME", home)
        .env_remove("CODEX_HOME")
        .output()?;
    if !install.status.success() {
        return Err(format!(
            "adoption fixture install failed: {}",
            String::from_utf8_lossy(&install.stdout)
        )
        .into());
    }
    fs::rename(
        home.join(".agents/.arthur-workflow/receipt.json"),
        home.join(".agents/.arthur-workflow/pre-adoption-receipt.json"),
    )?;

    let mut skills = serde_json::Map::new();
    for root in [home.join(".agents/skills"), home.join(".claude/skills")] {
        for entry in fs::read_dir(root)? {
            let name = entry?.file_name().into_string().map_err(|name| {
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
    fs::write(
        home.join(".agents/.skill-lock.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "version": 3,
            "skills": skills
        }))?,
    )?;
    Ok(())
}
