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
    use super::render;
    use crate::app::App;

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
}
