#![cfg(unix)]
#![forbid(unsafe_code)]

use std::error::Error;
use std::fs::File;
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

struct ProbeResult {
    initial_flags: LocalFlags,
    final_flags: LocalFlags,
    output: String,
    success: bool,
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
