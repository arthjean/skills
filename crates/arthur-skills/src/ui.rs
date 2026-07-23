use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Widget, Wrap};
use ratatui::{Frame, TerminalOptions, Viewport};

use crate::app::{Action, App, Outcome, Provider, Step};
use crate::transaction::{SIGINT_EXIT_CODE, SignalFlags};
use crate::workflow::{AssetSummary, WorkflowState};

const INLINE_HEIGHT: u16 = 16;

#[derive(Debug, Eq, PartialEq)]
pub enum UiExit {
    Selected(Vec<Provider>),
    Confirmed,
    Cancelled,
    Interrupted(u8),
}

pub fn select_providers(app: App, signals: &SignalFlags) -> io::Result<UiExit> {
    run(app, false, signals)
}

pub fn confirm_plan(app: App, signals: &SignalFlags) -> io::Result<UiExit> {
    run(app, true, signals)
}

fn run(mut app: App, append_snapshot: bool, signals: &SignalFlags) -> io::Result<UiExit> {
    let options = TerminalOptions {
        viewport: Viewport::Inline(INLINE_HEIGHT),
    };
    let mut terminal = match ratatui::try_init_with_options(options) {
        Ok(terminal) => terminal,
        Err(error) => {
            ratatui::restore();
            return Err(error);
        }
    };
    let colors = std::env::var_os("NO_COLOR").is_none();
    let result = (|| {
        if append_snapshot {
            let text = review_text(&app);
            terminal.insert_before(INLINE_HEIGHT, |buffer| {
                Paragraph::new(text)
                    .wrap(Wrap { trim: true })
                    .render(buffer.area, buffer);
            })?;
        }
        run_loop(&mut terminal, &mut app, colors, signals)
    })();
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    colors: bool,
    signals: &SignalFlags,
) -> io::Result<UiExit> {
    loop {
        terminal.draw(|frame| render(frame, app, colors))?;
        if run_debug_probe()? {
            return Ok(UiExit::Cancelled);
        }

        if let Some(code) = signals.pending_exit_code() {
            return Ok(UiExit::Interrupted(code));
        }
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }

        let event = match event::read() {
            Ok(event) => event,
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    && signals.pending_exit_code().is_some() =>
            {
                let code = signals.pending_exit_code().unwrap_or(SIGINT_EXIT_CODE);
                return Ok(UiExit::Interrupted(code));
            }
            Err(error) => return Err(error),
        };
        if matches!(event, Event::Resize(_, _)) {
            terminal.autoresize()?;
        }
        let Some(action) = action_for_event(event) else {
            continue;
        };
        match app.update(action) {
            Outcome::Continue => {}
            Outcome::SelectionConfirmed(providers) => return Ok(UiExit::Selected(providers)),
            Outcome::ApplicationConfirmed => return Ok(UiExit::Confirmed),
            Outcome::Cancelled => return Ok(UiExit::Cancelled),
            Outcome::Interrupted => return Ok(UiExit::Interrupted(SIGINT_EXIT_CODE)),
        }
    }
}

fn action_for_event(event: Event) -> Option<Action> {
    match event {
        Event::Key(key)
            if key.kind == KeyEventKind::Press
                && key.modifiers.contains(KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('c') =>
        {
            Some(Action::Interrupt)
        }
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(Action::Previous),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => Some(Action::Next),
            KeyCode::BackTab => Some(Action::Previous),
            KeyCode::Char(' ') => Some(Action::Toggle),
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Esc | KeyCode::Char('q') => Some(Action::Cancel),
            _ => None,
        },
        Event::Resize(width, height) => Some(Action::Resize(width, height)),
        _ => None,
    }
}

#[cfg(debug_assertions)]
fn run_debug_probe() -> io::Result<bool> {
    match std::env::var("ARTHUR_SKILLS_TUI_PROBE").as_deref() {
        Ok("success") => Ok(true),
        Ok("error") => Err(io::Error::other("controlled terminal probe error")),
        Ok("panic") => panic!("controlled terminal probe panic"),
        _ => Ok(false),
    }
}

#[cfg(not(debug_assertions))]
const fn run_debug_probe() -> io::Result<bool> {
    Ok(false)
}

pub fn render(frame: &mut Frame<'_>, app: &App, colors: bool) {
    match app.step() {
        Step::Selection => render_selection(frame, app, colors),
        Step::Review => render_review(frame, app, colors),
    }
}

fn render_selection(frame: &mut Frame<'_>, app: &App, colors: bool) {
    let [header, providers, explanation, message, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(4),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    frame.render_widget(
        Paragraph::new(format!(
            "Arthur Workflow: {} skills\nSelect providers",
            app.skill_count()
        ))
        .style(Style::default().add_modifier(Modifier::BOLD)),
        header,
    );
    let items = Provider::ALL.iter().enumerate().map(|(index, provider)| {
        let marker = if app.enabled(index) { "[x]" } else { "[ ]" };
        let detected = if app.detected(index) {
            "detected"
        } else {
            "not detected"
        };
        let style = if colors && index == app.selected() {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if index == app.selected() {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        ListItem::new(Line::from(format!(
            "{marker} {} ({detected})",
            provider.label()
        )))
        .style(style)
    });
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(" Providers ")),
        providers,
    );
    frame.render_widget(
        Paragraph::new("Codex reads $HOME/.agents/skills whenever it exists. Provider selection controls managed agents and activations, not skill isolation.")
            .wrap(Wrap { trim: true }),
        explanation,
    );
    frame.render_widget(Paragraph::new(app.message().unwrap_or("")), message);
    frame.render_widget(
        Paragraph::new("Tab/↑/↓ move  Space toggle  Enter continue  Esc cancel  Ctrl+C interrupt"),
        footer,
    );
}

fn render_review(frame: &mut Frame<'_>, app: &App, colors: bool) {
    let [header, body, message, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(8),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let assessment = app.review().and_then(|review| review.assessment.as_ref());
    let title = assessment.map_or("Review filesystem plan", |value| value.state.title());
    let subtitle = assessment.map_or(
        "The complete catalog is always included.",
        workflow_subtitle,
    );
    frame.render_widget(
        Paragraph::new(format!("{title}\n{subtitle}"))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        header,
    );
    frame.render_widget(
        Paragraph::new(review_text(app)).wrap(Wrap { trim: false }),
        body,
    );
    frame.render_widget(Paragraph::new(app.message().unwrap_or("")), message);
    let applicable = app.review().is_some_and(|review| review.applicable);
    let footer_style = if colors && applicable {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let footer_text = if let Some(assessment) = assessment {
        format!(
            "Enter {}  Esc cancel  Ctrl+C interrupt",
            assessment.state.action()
        )
    } else if applicable {
        "Enter apply  Esc cancel  Ctrl+C interrupt".to_owned()
    } else {
        "Apply disabled: resolve conflicts or run adopt  Esc cancel".to_owned()
    };
    frame.render_widget(Paragraph::new(footer_text).style(footer_style), footer);
}

fn review_text(app: &App) -> Text<'static> {
    let Some(review) = app.review() else {
        return Text::from("No plan loaded.");
    };
    if let Some(assessment) = &review.assessment {
        let mut lines = vec![
            summary_line("Skills", assessment.skills),
            summary_line("Agents", assessment.agents),
        ];
        if assessment.legacy_skills_to_import > 0 {
            lines.push(Line::from(format!(
                "Legacy skills to import: {}",
                assessment.legacy_skills_to_import
            )));
        }
        if assessment.legacy_skills_to_clean > 0 {
            lines.push(Line::from(format!(
                "Legacy skills to clean up: {}",
                assessment.legacy_skills_to_clean
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(workflow_explanation(assessment.state)));
        for notice in &review.notices {
            lines.push(Line::from(format!("Notice: {}", notice.message)));
        }
        return Text::from(lines);
    }
    let mut lines = Vec::new();
    for ((root, action), entries) in &review.groups {
        lines.push(Line::from(format!(
            "{:?} [{}]: {}",
            action,
            root,
            entries.len()
        )));
        for entry in entries.iter().take(2) {
            lines.push(Line::from(format!("  {}", entry.destination.display())));
        }
        if entries.len() > 2 {
            lines.push(Line::from(format!(
                "  ... {} more; use plan --plain for every path",
                entries.len() - 2
            )));
        }
    }
    for notice in &review.notices {
        lines.push(Line::from(format!("Notice: {}", notice.message)));
    }
    Text::from(lines)
}

fn summary_line(label: &str, summary: AssetSummary) -> Line<'static> {
    Line::from(format!(
        "{label}: {}/{} found, {} missing, {} not aligned",
        summary.found, summary.total, summary.missing, summary.not_aligned
    ))
}

const fn workflow_subtitle(assessment: &crate::workflow::WorkflowAssessment) -> &'static str {
    match assessment.state {
        WorkflowState::FreshInstall => "No existing Arthur configuration was found.",
        WorkflowState::Import => "Existing Arthur assets were found on this machine.",
        WorkflowState::Update => "Your managed configuration needs reconciliation.",
        WorkflowState::Current => "Your skills and agents match the embedded catalog.",
    }
}

const fn workflow_explanation(state: WorkflowState) -> &'static str {
    match state {
        WorkflowState::FreshInstall => {
            "The catalog skills and selected provider agents will be installed transactionally."
        }
        WorkflowState::Import => {
            "Matching assets will be imported. Misaligned assets will be replaced, and obsolete Arthur entries will be cleaned up. Unrelated assets are preserved."
        }
        WorkflowState::Update => {
            "Missing assets will be restored and misaligned managed assets will be updated transactionally."
        }
        WorkflowState::Current => {
            "Everything is already current. You can close Arthur Workflow safely."
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::{action_for_event, render, review_text};
    use crate::app::{Action, App, Provider, Review};
    use crate::plan::{Owner, Plan, PlanAction, PlanEntry};
    use crate::provider::resolve_roots_from;

    #[test]
    fn test_backend_renders_selection_accessibly() -> Result<(), Box<dyn std::error::Error>> {
        let backend = TestBackend::new(82, 16);
        let mut terminal = Terminal::new(backend)?;
        assert!(
            terminal
                .draw(|frame| render(frame, &App::new(50, &[Provider::Claude]), false))
                .is_ok()
        );
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .chunks(usize::from(terminal.backend().buffer().area.width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Claude Code (detected)"));
        assert!(rendered.contains("Codex (not detected)"));
        assert!(rendered.contains("Ctrl+C interrupt"));
        Ok(())
    }

    #[test]
    fn keyboard_resize_and_interrupt_events_map_to_shared_actions() {
        let key = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        assert_eq!(action_for_event(key(KeyCode::Tab)), Some(Action::Next));
        assert_eq!(
            action_for_event(key(KeyCode::BackTab)),
            Some(Action::Previous)
        );
        assert_eq!(
            action_for_event(key(KeyCode::Char(' '))),
            Some(Action::Toggle)
        );
        assert_eq!(action_for_event(key(KeyCode::Enter)), Some(Action::Confirm));
        assert_eq!(action_for_event(key(KeyCode::Esc)), Some(Action::Cancel));
        assert_eq!(
            action_for_event(Event::Resize(120, 40)),
            Some(Action::Resize(120, 40))
        );
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(action_for_event(ctrl_c), Some(Action::Interrupt));
        for code in [KeyCode::Up, KeyCode::Char('k')] {
            assert_eq!(action_for_event(key(code)), Some(Action::Previous));
        }
        for code in [KeyCode::Down, KeyCode::Char('j')] {
            assert_eq!(action_for_event(key(code)), Some(Action::Next));
        }
        assert_eq!(
            action_for_event(key(KeyCode::Char('q'))),
            Some(Action::Cancel)
        );
        assert_eq!(action_for_event(key(KeyCode::F(1))), None);
        assert_eq!(action_for_event(Event::FocusGained), None);
        assert_eq!(
            action_for_event(Event::Key(KeyEvent::new_with_kind(
                KeyCode::Enter,
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ))),
            None
        );
    }

    #[test]
    fn colored_selection_and_empty_review_have_stable_fallbacks()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(1, &[Provider::Codex]);
        app.update(Action::Next);
        let backend = TestBackend::new(82, 16);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| render(frame, &app, true))?;
        assert!(review_text(&app).to_string().contains("No plan loaded"));
        Ok(())
    }

    #[test]
    fn compact_review_keeps_conflicts_actionable_with_a_text_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let home = tempfile::tempdir()?;
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[Provider::Claude])?;
        let entries = (0..3)
            .map(|index| PlanEntry {
                action: PlanAction::Conflict,
                source: format!("skill-{index}"),
                destination: roots
                    .canonical_skills
                    .join(format!("skill-{index}/SKILL.md")),
                owner: Owner::Unmanaged,
                reason: "foreign content".to_owned(),
            })
            .collect::<Vec<_>>();
        let plan = Plan {
            schema_version: 1,
            applicable: false,
            entries,
            operations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let mut app = App::with_selection(50, &[Provider::Claude]);
        app.set_review(Review::from_plan(&plan, &[], &roots));
        let backend = TestBackend::new(64, 16);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| render(frame, &app, false))?;
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .chunks(usize::from(terminal.backend().buffer().area.width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("plan --plain"));
        assert!(rendered.contains("Apply disabled"));
        Ok(())
    }
}
