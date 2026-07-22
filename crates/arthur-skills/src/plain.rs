use std::io::{self, Write};

use crate::app::{App, Provider};

pub fn render(app: &App, output: &mut impl Write) -> io::Result<()> {
    writeln!(
        output,
        "Arthur Workflow catalog: {} skills",
        app.skill_count()
    )?;
    writeln!(output, "Provider selection")?;
    for (index, provider) in Provider::ALL.iter().enumerate() {
        let state = if app.enabled(index) {
            "selected"
        } else {
            "disabled"
        };
        writeln!(output, "- {}: {state}", provider.label())?;
    }
    writeln!(
        output,
        "Codex visibility: canonical skills remain discoverable while $HOME/.agents/skills exists."
    )?;
    writeln!(output, "Decision: {}", app.selection_summary())
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::render;
    use crate::app::{Action, App};

    struct FailAfter {
        remaining: usize,
    }

    impl Write for FailAfter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::other("controlled write failure"));
            }
            let written = self.remaining.min(buffer.len());
            self.remaining -= written;
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn plain_renderer_is_line_oriented_and_control_free() {
        let mut output = Vec::new();
        let result = render(&App::new(50), &mut output);
        assert!(result.is_ok());
        let rendered = String::from_utf8_lossy(&output);
        assert_eq!(rendered.lines().count(), 6);
        assert!(rendered.contains("Claude Code: selected"));
        assert!(rendered.contains("Codex: selected"));
        assert!(rendered.contains("Decision: Claude Code, Codex"));
        assert!(!rendered.contains('\u{1b}'));
    }

    #[test]
    fn plain_renderer_reports_disabled_providers() {
        let mut app = App::new(1);
        assert!(matches!(
            app.update(Action::Toggle),
            crate::app::Outcome::Continue
        ));
        let mut output = Vec::new();

        assert!(render(&app, &mut output).is_ok());
        assert!(String::from_utf8_lossy(&output).contains("Claude Code: disabled"));
    }

    #[test]
    fn plain_renderer_propagates_early_and_late_write_failures() {
        let mut early = FailAfter { remaining: 0 };
        assert!(render(&App::new(1), &mut early).is_err());

        let mut late = FailAfter { remaining: 95 };
        assert!(render(&App::new(1), &mut late).is_err());
        assert!(late.flush().is_ok());
    }
}
