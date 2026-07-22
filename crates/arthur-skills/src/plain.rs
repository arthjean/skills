use std::io::{self, BufRead, Write};

use crate::app::{Action, App, Outcome, Provider, Step};
use crate::transaction::SignalFlags;

#[derive(Debug, Eq, PartialEq)]
pub enum PlainExit {
    Selected(Vec<Provider>),
    Confirmed,
    Cancelled,
    Interrupted(u8),
}

pub fn select_providers(
    mut app: App,
    input: &mut impl BufRead,
    output: &mut impl Write,
    signals: &SignalFlags,
) -> io::Result<PlainExit> {
    loop {
        render(&app, output)?;
        writeln!(output, "Input: 1 or 2 toggles, Enter continues, q cancels")?;
        output.flush()?;
        let Some(line) = read_line(input, signals)? else {
            return Ok(interrupted_or_cancelled(signals));
        };
        let action = match line.trim() {
            "1" => {
                while app.selected() != 0 {
                    app.update(Action::Previous);
                }
                Action::Toggle
            }
            "2" => {
                while app.selected() != 1 {
                    app.update(Action::Next);
                }
                Action::Toggle
            }
            "" => Action::Confirm,
            "q" | "quit" | "cancel" => Action::Cancel,
            _ => {
                writeln!(output, "Invalid input: choose 1, 2, Enter, or q.")?;
                continue;
            }
        };
        match app.update(action) {
            Outcome::SelectionConfirmed(providers) => return Ok(PlainExit::Selected(providers)),
            Outcome::Cancelled => return Ok(PlainExit::Cancelled),
            Outcome::Interrupted => return Ok(interrupted_or_cancelled(signals)),
            Outcome::Continue | Outcome::ApplicationConfirmed => {}
        }
    }
}

pub fn confirm_plan(
    mut app: App,
    input: &mut impl BufRead,
    output: &mut impl Write,
    signals: &SignalFlags,
) -> io::Result<PlainExit> {
    render(&app, output)?;
    if !app.review().is_some_and(|review| review.applicable) {
        writeln!(
            output,
            "Application disabled: resolve conflicts or run adopt."
        )?;
        return Ok(PlainExit::Cancelled);
    }
    writeln!(output, "Apply this complete plan? [y/N]")?;
    output.flush()?;
    let Some(line) = read_line(input, signals)? else {
        return Ok(interrupted_or_cancelled(signals));
    };
    match line.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => match app.update(Action::Confirm) {
            Outcome::ApplicationConfirmed => Ok(PlainExit::Confirmed),
            _ => Ok(PlainExit::Cancelled),
        },
        _ => Ok(PlainExit::Cancelled),
    }
}

pub fn render(app: &App, output: &mut impl Write) -> io::Result<()> {
    writeln!(
        output,
        "Arthur Workflow catalog: {} skills",
        app.skill_count()
    )?;
    match app.step() {
        Step::Selection => render_selection(app, output),
        Step::Review => render_review(app, output),
    }
}

fn render_selection(app: &App, output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "Provider selection")?;
    for (index, provider) in Provider::ALL.iter().enumerate() {
        let selected = if app.enabled(index) {
            "selected"
        } else {
            "disabled"
        };
        let detected = if app.detected(index) {
            "detected"
        } else {
            "not detected"
        };
        writeln!(
            output,
            "{}. {}: {selected}, {detected}",
            index + 1,
            provider.label()
        )?;
    }
    writeln!(
        output,
        "Codex visibility: canonical skills remain discoverable while $HOME/.agents/skills exists."
    )?;
    if let Some(message) = app.message() {
        writeln!(output, "{message}")?;
    }
    Ok(())
}

fn render_review(app: &App, output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "Plan review: the complete catalog is included")?;
    let Some(review) = app.review() else {
        return writeln!(output, "No plan loaded.");
    };
    for ((root, action), entries) in &review.groups {
        writeln!(output, "{:?} [{}]: {}", action, root, entries.len())?;
        for entry in entries {
            writeln!(
                output,
                "  {}: {} ({})",
                entry.source,
                entry.destination.display(),
                entry.reason
            )?;
        }
    }
    for notice in &review.notices {
        writeln!(output, "Notice: {}", notice.message)?;
    }
    Ok(())
}

fn read_line(input: &mut impl BufRead, signals: &SignalFlags) -> io::Result<Option<String>> {
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(0) => Ok(None),
        Ok(_) if signals.pending_exit_code().is_some() => Ok(None),
        Ok(_) => Ok(Some(line)),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(error) => Err(error),
    }
}

fn interrupted_or_cancelled(signals: &SignalFlags) -> PlainExit {
    signals
        .pending_exit_code()
        .map_or(PlainExit::Cancelled, PlainExit::Interrupted)
}

#[cfg(test)]
mod tests {
    use std::io::{self, BufRead, Cursor, Read};

    use super::{PlainExit, confirm_plan, read_line, render_review, select_providers};
    use crate::app::{App, Provider, Review};
    use crate::transaction::SignalFlags;

    #[test]
    fn plain_selection_is_numbered_line_oriented_and_control_free() {
        let mut input = Cursor::new(b"2\n\n");
        let mut output = Vec::new();
        let result = select_providers(
            App::new(50, &[Provider::Claude]),
            &mut input,
            &mut output,
            &SignalFlags::default(),
        );
        assert!(matches!(
            result,
            Ok(PlainExit::Selected(providers)) if providers == vec![Provider::Claude]
        ));
        let rendered = String::from_utf8_lossy(&output);
        assert!(rendered.contains("1. Claude Code: selected, detected"));
        assert!(rendered.contains("2. Codex: selected, not detected"));
        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\r'));
    }

    #[test]
    fn plain_end_of_input_cancels_before_mutation() {
        let mut input = Cursor::new([]);
        let mut output = Vec::new();
        assert!(matches!(
            select_providers(
                App::new(1, &[]),
                &mut input,
                &mut output,
                &SignalFlags::default(),
            ),
            Ok(PlainExit::Cancelled)
        ));
    }

    #[test]
    fn pending_sigterm_interrupts_plain_selection_with_shell_code() {
        let flags = SignalFlags::default();
        flags.record_for_test(signal_hook::consts::signal::SIGTERM);
        let mut input = Cursor::new(b"\n");
        let mut output = Vec::new();
        assert!(matches!(
            select_providers(App::new(1, &[]), &mut input, &mut output, &flags),
            Ok(PlainExit::Interrupted(143))
        ));
    }

    #[test]
    fn plain_navigation_can_move_back_and_block_an_empty_selection() {
        let mut input = Cursor::new(b"2\n1\n\nq\n");
        let mut output = Vec::new();
        assert!(matches!(
            select_providers(
                App::new(1, &[]),
                &mut input,
                &mut output,
                &SignalFlags::default(),
            ),
            Ok(PlainExit::Cancelled)
        ));
        assert!(String::from_utf8_lossy(&output).contains("Select at least one provider"));
    }

    #[test]
    fn plain_review_without_an_applicable_plan_is_disabled() {
        let mut output = Vec::new();
        assert!(matches!(
            confirm_plan(
                App::new(1, &[]),
                &mut Cursor::new(b"y\n"),
                &mut output,
                &SignalFlags::default(),
            ),
            Ok(PlainExit::Cancelled)
        ));
        assert!(String::from_utf8_lossy(&output).contains("Application disabled"));

        let mut review_output = Vec::new();
        assert!(render_review(&App::new(1, &[]), &mut review_output).is_ok());
        assert!(String::from_utf8_lossy(&review_output).contains("No plan loaded"));

        let mut app = App::new(1, &[]);
        app.set_review(Review {
            groups: Default::default(),
            applicable: true,
            notices: Vec::new(),
        });
        let flags = SignalFlags::default();
        flags.record_for_test(signal_hook::consts::signal::SIGTERM);
        assert!(matches!(
            confirm_plan(app, &mut Cursor::new(b"y\n"), &mut Vec::new(), &flags,),
            Ok(PlainExit::Interrupted(143))
        ));
    }

    #[test]
    fn line_reader_distinguishes_interrupts_from_other_errors() {
        struct FailedInput(io::ErrorKind);

        impl Read for FailedInput {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(self.0))
            }
        }

        impl BufRead for FailedInput {
            fn fill_buf(&mut self) -> io::Result<&[u8]> {
                Err(io::Error::from(self.0))
            }

            fn consume(&mut self, _amount: usize) {}

            fn read_line(&mut self, _buffer: &mut String) -> io::Result<usize> {
                Err(io::Error::from(self.0))
            }
        }

        assert!(
            read_line(
                &mut FailedInput(io::ErrorKind::Interrupted),
                &SignalFlags::default()
            )
            .is_ok_and(|line| line.is_none())
        );
        assert!(
            read_line(
                &mut FailedInput(io::ErrorKind::BrokenPipe),
                &SignalFlags::default()
            )
            .is_err()
        );
    }
}
