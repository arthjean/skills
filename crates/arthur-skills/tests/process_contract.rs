#![forbid(unsafe_code)]

use std::error::Error;
use std::process::Command;

#[test]
fn help_and_plain_process_contracts_are_stable() -> Result<(), Box<dyn Error>> {
    let binary = env!("CARGO_BIN_EXE_arthur-skills");
    let help = Command::new(binary).arg("--help").output()?;
    assert!(help.status.success());
    let help_text = String::from_utf8(help.stdout)?;
    assert!(help_text.contains("Install and manage the portable Arthur Workflow catalog"));
    assert!(help_text.contains("--plain"));

    let plain = Command::new(binary).arg("--plain").output()?;
    assert!(plain.status.success());
    let plain_text = String::from_utf8(plain.stdout)?;
    assert!(plain_text.contains("Arthur Workflow catalog: 50 skills"));
    assert!(plain_text.contains("Decision: Claude Code, Codex"));
    assert!(!plain_text.contains('\u{1b}'));

    let dumb = Command::new(binary).env("TERM", "dumb").output()?;
    assert!(dumb.status.success());
    assert!(!String::from_utf8(dumb.stdout)?.contains('\u{1b}'));
    Ok(())
}

#[test]
fn portable_skill_root_preserves_spaces_and_unicode() -> Result<(), Box<dyn Error>> {
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
