use std::io;
use std::time::{Duration, Instant};

use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::{Frame, TerminalOptions, Viewport};

use crate::app::{Action, App, Outcome, Provider, Step};
use crate::output::{Envelope, OutputSeverity, compact_summary};
use crate::transaction::{SIGINT_EXIT_CODE, SignalFlags};
use crate::workflow::{AssetSummary, WorkflowState};

mod logo;

const INLINE_HEIGHT: u16 = 16;
const BRAND_HEADER_HEIGHT: u16 = logo::HEIGHT;
const TEXT_HEADER_HEIGHT: u16 = 3;
const LOGO_HEADER_MIN_WIDTH: u16 = 72;

#[derive(Debug, Eq, PartialEq)]
pub enum UiExit {
    Selected(Vec<Provider>),
    Confirmed,
    Cancelled,
    Interrupted(u8),
}

pub fn select_providers(app: App, signals: &SignalFlags) -> io::Result<UiExit> {
    run(app, false, true, signals)
}

pub fn confirm_plan(app: App, signals: &SignalFlags) -> io::Result<UiExit> {
    run(app, true, false, signals)
}

pub fn show_success(envelope: &Envelope) -> io::Result<()> {
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
    let result = run_success_loop(&mut terminal, envelope, colors);
    ratatui::restore();
    result
}

fn run(
    mut app: App,
    record_providers: bool,
    animate_intro: bool,
    signals: &SignalFlags,
) -> io::Result<UiExit> {
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
        if record_providers {
            let providers = app.selection_summary();
            terminal.insert_before(1, |buffer| {
                Line::from(vec![
                    Span::styled("  Providers  ", secondary_style()),
                    Span::raw(providers),
                ])
                .render(buffer.area, buffer);
            })?;
        }
        run_loop(&mut terminal, &mut app, colors, animate_intro, signals)
    })();
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    colors: bool,
    animate_intro: bool,
    signals: &SignalFlags,
) -> io::Result<UiExit> {
    let animation_started = Instant::now();
    loop {
        let elapsed = animation_started.elapsed();
        terminal.draw(|frame| render_animated(frame, app, colors, elapsed, animate_intro))?;
        if run_debug_probe()? {
            return finish_terminal(terminal, UiExit::Cancelled);
        }

        if let Some(code) = signals.pending_exit_code() {
            return finish_terminal(terminal, UiExit::Interrupted(code));
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
                return finish_terminal(terminal, UiExit::Interrupted(code));
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
            Outcome::SelectionConfirmed(providers) => {
                return finish_terminal(terminal, UiExit::Selected(providers));
            }
            Outcome::ApplicationConfirmed => {
                return finish_terminal(terminal, UiExit::Confirmed);
            }
            Outcome::Cancelled => return finish_terminal(terminal, UiExit::Cancelled),
            Outcome::Interrupted => {
                return finish_terminal(terminal, UiExit::Interrupted(SIGINT_EXIT_CODE));
            }
        }
    }
}

fn run_success_loop(
    terminal: &mut ratatui::DefaultTerminal,
    envelope: &Envelope,
    colors: bool,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| render_success_state(frame, envelope, colors))?;
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        match event::read()? {
            Event::Resize(_, _) => terminal.autoresize()?,
            Event::Key(key)
                if key.kind == KeyEventKind::Press
                    && (key.code == KeyCode::Enter
                        || (key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c'))) =>
            {
                return clear_terminal(terminal);
            }
            _ => {}
        }
    }
}

fn finish_terminal(terminal: &mut ratatui::DefaultTerminal, exit: UiExit) -> io::Result<UiExit> {
    clear_terminal(terminal)?;
    Ok(exit)
}

fn clear_terminal(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    execute!(terminal.backend_mut(), MoveTo(0, 0), Clear(ClearType::All))?;
    terminal.show_cursor()?;
    Ok(())
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
    render_animated(frame, app, colors, logo::PREVIEW_TIME, false);
}

fn render_animated(
    frame: &mut Frame<'_>,
    app: &App,
    colors: bool,
    elapsed: Duration,
    reveal_intro: bool,
) {
    match app.step() {
        Step::Selection => render_selection(frame, app, colors, elapsed, reveal_intro),
        Step::Review => render_review(frame, app, colors),
    }
}

#[cfg(test)]
fn render_success(frame: &mut Frame<'_>, envelope: &Envelope, colors: bool) {
    render_success_state(frame, envelope, colors);
}

fn render_success_state(frame: &mut Frame<'_>, envelope: &Envelope, colors: bool) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(TEXT_HEADER_HEIGHT),
        Constraint::Min(9),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    let success_style = if colors {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    render_text_header(
        frame,
        header,
        "✓ Everything is up to date",
        "All managed skills and agents match the desired state.",
        success_style,
    );
    let mut lines = Vec::new();
    let summary = compact_summary(envelope);
    if !summary.is_empty() {
        lines.push(Line::from(format!("    {}", summary.join("  · "))));
    }
    for diagnostic in &envelope.diagnostics {
        let label = match diagnostic.severity {
            OutputSeverity::Info => "Info",
            OutputSeverity::Warning => "Note",
            OutputSeverity::Error => "Error",
        };
        lines.push(Line::from(vec![
            format!("    {label}  ").into(),
            Span::styled(diagnostic.message.clone(), secondary_style()),
        ]));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body);
    frame.render_widget(
        Paragraph::new(hint_line(
            &[("Enter", "close"), ("Ctrl+C", "interrupt")],
            colors,
        )),
        footer,
    );
}

fn render_selection(
    frame: &mut Frame<'_>,
    app: &App,
    colors: bool,
    elapsed: Duration,
    reveal_intro: bool,
) {
    let [header, providers, explanation, message, footer] = Layout::vertical([
        Constraint::Length(BRAND_HEADER_HEIGHT),
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    let subtitle = format!("Arthur Workflow · {} catalog skills", app.skill_count());
    render_brand_header(
        frame,
        header,
        "Select providers",
        &subtitle,
        Style::default().add_modifier(Modifier::BOLD),
        colors,
        elapsed,
        reveal_intro,
    );
    let provider_lines = Provider::ALL
        .iter()
        .enumerate()
        .map(|(index, provider)| provider_line(app, index, provider, colors))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(provider_lines), providers);
    let mut explanation_lines = vec![
        Line::default(),
        Line::from(vec![
            "  ".into(),
            Span::styled(
                "Provider choices control managed agents, not skill visibility.",
                secondary_style(),
            ),
        ]),
    ];
    let codex_enabled = Provider::ALL
        .iter()
        .position(|provider| *provider == Provider::Codex)
        .is_some_and(|index| app.enabled(index));
    if !codex_enabled {
        explanation_lines.push(Line::from(vec![
            "  ".into(),
            Span::styled(
                "Codex can still discover canonical skills while they remain installed.",
                secondary_style(),
            ),
        ]));
    }
    frame.render_widget(
        Paragraph::new(explanation_lines).wrap(Wrap { trim: false }),
        explanation,
    );
    render_message(frame, app.message(), message, colors);
    frame.render_widget(
        Paragraph::new(selection_footer(colors)).wrap(Wrap { trim: false }),
        footer,
    );
}

fn render_review(frame: &mut Frame<'_>, app: &App, colors: bool) {
    let [header, body, message, footer] = Layout::vertical([
        Constraint::Length(TEXT_HEADER_HEIGHT),
        Constraint::Min(7),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    let assessment = app.review().and_then(|review| review.assessment.as_ref());
    let title = assessment.map_or("Review filesystem plan", |value| value.state.title());
    let subtitle = assessment.map_or(
        "The complete catalog is always included.",
        workflow_subtitle,
    );
    render_text_header(
        frame,
        header,
        title,
        subtitle,
        workflow_title_style(assessment.map(|value| value.state), colors),
    );
    let body = horizontal_inset(body, 2);
    frame.render_widget(
        Paragraph::new(review_text(app, colors)).wrap(Wrap { trim: false }),
        body,
    );
    render_message(frame, app.message(), message, colors);
    let applicable = app.review().is_some_and(|review| review.applicable);
    let action = if let Some(assessment) = assessment {
        assessment.state.action()
    } else if applicable {
        "apply"
    } else {
        "disabled"
    };
    frame.render_widget(
        Paragraph::new(review_footer(action, applicable, colors)).wrap(Wrap { trim: false }),
        footer,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_brand_header(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    subtitle: &str,
    title_style: Style,
    colors: bool,
    elapsed: Duration,
    reveal_intro: bool,
) {
    if area.width < LOGO_HEADER_MIN_WIDTH || area.height < logo::HEIGHT {
        render_text_header(frame, area, title, subtitle, title_style);
        return;
    }

    let logo_area = Rect::new(area.x.saturating_add(2), area.y, logo::WIDTH, logo::HEIGHT);
    frame.render_widget(
        Paragraph::new(logo::lines(elapsed, colors, reveal_intro)),
        logo_area,
    );

    let text_x = logo_area.right().saturating_add(3);
    let text_area = Rect::new(
        text_x,
        area.y.saturating_add(1),
        area.right().saturating_sub(text_x),
        area.height.saturating_sub(1),
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(title.to_owned(), title_style)),
            Line::from(Span::styled(subtitle.to_owned(), secondary_style())),
        ])
        .wrap(Wrap { trim: false }),
        text_area,
    );
}

fn render_text_header(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    subtitle: &str,
    title_style: Style,
) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::default(),
            Line::from(vec![
                "  ".into(),
                Span::styled(title.to_owned(), title_style),
            ]),
            Line::from(vec![
                "  ".into(),
                Span::styled(subtitle.to_owned(), secondary_style()),
            ]),
        ])
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn horizontal_inset(area: Rect, columns: u16) -> Rect {
    let inset = columns.min(area.width / 2);
    Rect::new(
        area.x.saturating_add(inset),
        area.y,
        area.width.saturating_sub(inset.saturating_mul(2)),
        area.height,
    )
}

fn workflow_title_style(state: Option<WorkflowState>, colors: bool) -> Style {
    let style = Style::default().add_modifier(Modifier::BOLD);
    if !colors {
        return style;
    }
    match state {
        Some(WorkflowState::Current) => style.fg(Color::Green),
        Some(WorkflowState::Update) => style.fg(Color::Yellow),
        Some(WorkflowState::FreshInstall | WorkflowState::Import) => style.fg(Color::Cyan),
        None => style,
    }
}

fn provider_line<'a>(app: &App, index: usize, provider: &'a Provider, colors: bool) -> Line<'a> {
    let selected = index == app.selected();
    let marker = if app.enabled(index) { 'x' } else { ' ' };
    let cursor_style = if colors && selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else if selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let status_style = if app.detected(index) && colors {
        Style::default().fg(Color::Green)
    } else {
        secondary_style()
    };
    let status = if app.detected(index) {
        "detected"
    } else {
        "not detected"
    };
    Line::from(vec![
        Span::styled(if selected { "› " } else { "  " }, cursor_style),
        Span::styled(format!("[{marker}] "), cursor_style),
        Span::styled(format!("{:<13}", provider.label()), cursor_style),
        Span::styled(status, status_style),
    ])
}

fn render_message(
    frame: &mut Frame<'_>,
    message: Option<&str>,
    area: ratatui::layout::Rect,
    colors: bool,
) {
    let Some(message) = message else {
        return;
    };
    let style = if colors {
        Style::default().fg(Color::Red)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            "  ".into(),
            Span::styled("! ", style.add_modifier(Modifier::BOLD)),
            Span::styled(message, style),
        ]))
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn selection_footer(colors: bool) -> Line<'static> {
    hint_line(
        &[
            ("Tab/↑/↓", "move"),
            ("Space", "toggle"),
            ("Enter", "continue"),
            ("Esc", "cancel"),
            ("Ctrl+C", "interrupt"),
        ],
        colors,
    )
}

fn review_footer(action: &str, applicable: bool, colors: bool) -> Line<'static> {
    if !applicable {
        return Line::from(vec![
            "  ".into(),
            Span::styled(
                "Apply disabled: resolve conflicts or run adopt",
                if colors {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled("  Esc cancel  Ctrl+C interrupt", secondary_style()),
        ]);
    }
    hint_line(
        &[
            ("Enter", action),
            ("Esc", "cancel"),
            ("Ctrl+C", "interrupt"),
        ],
        colors,
    )
}

fn hint_line(hints: &[(&str, &str)], colors: bool) -> Line<'static> {
    let key_style = if colors {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    let mut spans = vec!["  ".into()];
    for (index, (key, action)) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", secondary_style()));
        }
        spans.push(Span::styled((*key).to_owned(), key_style));
        spans.push(Span::styled(format!(" {action}"), secondary_style()));
    }
    Line::from(spans)
}

fn secondary_style() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

fn review_text(app: &App, colors: bool) -> Text<'static> {
    let Some(review) = app.review() else {
        return Text::from("No plan loaded.");
    };
    if let Some(assessment) = &review.assessment {
        let mut lines = vec![
            summary_line("Skills", assessment.skills, colors),
            summary_line("Agents", assessment.agents, colors),
        ];
        if assessment.legacy_skills_to_import > 0 || assessment.legacy_skills_to_clean > 0 {
            lines.push(Line::from(format!(
                "Legacy   {} to import  · {} to remove",
                assessment.legacy_skills_to_import, assessment.legacy_skills_to_clean
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::styled(
            workflow_explanation(assessment.state),
            secondary_style(),
        ));
        for notice in &review.notices {
            lines.push(Line::from(vec![
                Span::styled(
                    "Note  ",
                    if colors {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().add_modifier(Modifier::BOLD)
                    },
                ),
                Span::styled(notice.message.clone(), secondary_style()),
            ]));
        }
        return Text::from(lines);
    }
    let mut lines = Vec::new();
    for ((root, action), entries) in &review.groups {
        lines.push(Line::from(format!(
            "{action:?} · {}  {root}",
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

fn summary_line(label: &str, summary: AssetSummary, colors: bool) -> Line<'static> {
    let mut values = vec![format!("{}/{} found", summary.found, summary.total)];
    if summary.missing > 0 {
        values.push(format!("{} missing", summary.missing));
    }
    if summary.not_aligned > 0 {
        values.push(format!("{} to align", summary.not_aligned));
    }
    let status_style = if colors {
        if summary.missing == 0 && summary.not_aligned == 0 {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Yellow)
        }
    } else {
        Style::default()
    };
    Line::from(vec![
        Span::styled(format!("{label:<9}"), Style::default()),
        Span::styled(values.join("  · "), status_style),
    ])
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
            "Matching assets will be imported, managed drift replaced, and obsolete Arthur entries removed. Unrelated assets are preserved."
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
    use ratatui::style::Color;

    use super::{action_for_event, render, render_success, review_text};
    use crate::app::{Action, App, Provider, Review};
    use crate::lifecycle::{LifecycleNotice, LifecycleNoticeCode};
    use crate::output::{Envelope, OutputDiagnostic, OutputSeverity};
    use crate::plan::{Owner, Plan, PlanAction, PlanEntry};
    use crate::provider::resolve_roots_from;
    use crate::workflow::{AssetSummary, WorkflowAssessment, WorkflowState};

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
        assert!(rendered.contains("Select providers"));
        assert!(rendered.contains("Arthur Workflow · 50 catalog skills"));
        assert!(rendered.contains('⣿'));
        assert!(rendered.contains("› [x] Claude Code  detected"));
        assert!(rendered.contains("  [x] Codex        not detected"));
        assert!(!rendered.contains('┌'));
        assert!(rendered.contains("Space toggle"));
        assert!(rendered.contains("Ctrl+C interrupt"));
        Ok(())
    }

    #[test]
    fn success_state_explains_the_outcome_and_waits_for_enter()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut envelope = Envelope::new(Some("install"));
        envelope.summary.insert("noop".to_owned(), 513);
        envelope.summary.insert("update".to_owned(), 13);
        envelope.diagnostics.push(OutputDiagnostic {
            code: "codex_uses_implicit_skills".to_owned(),
            severity: OutputSeverity::Warning,
            message: "Codex reads shared skills directly.".to_owned(),
            path_utf8: None,
            path_bytes_hex: None,
            remediation: None,
        });
        let backend = TestBackend::new(82, 16);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| render_success(frame, &envelope, false))?;
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .chunks(usize::from(terminal.backend().buffer().area.width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Everything is up to date"));
        assert!(rendered.contains("13 updated  · 513 unchanged"));
        assert!(rendered.contains("Note  Codex reads shared skills directly."));
        assert!(rendered.contains("Enter close"));
        assert!(!rendered.contains('⣿'));
        Ok(())
    }

    #[test]
    fn selection_explains_implicit_codex_visibility_when_codex_is_disabled()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(50, &[Provider::Claude]);
        app.update(Action::Next);
        app.update(Action::Toggle);
        let backend = TestBackend::new(82, 16);
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
        assert!(rendered.contains("› [ ] Codex"));
        assert!(
            rendered
                .contains("Codex can still discover canonical skills while they remain installed.")
        );
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
        assert!(
            review_text(&app, false)
                .to_string()
                .contains("No plan loaded")
        );
        Ok(())
    }

    #[test]
    fn workflow_review_omits_logo_zero_noise_and_combines_legacy_counts()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::with_selection(50, &[Provider::Claude, Provider::Codex]);
        app.set_review(Review {
            groups: Default::default(),
            applicable: true,
            notices: Vec::new(),
            assessment: Some(WorkflowAssessment {
                state: WorkflowState::Import,
                skills: AssetSummary {
                    total: 50,
                    found: 50,
                    missing: 0,
                    not_aligned: 5,
                },
                agents: AssetSummary {
                    total: 6,
                    found: 6,
                    missing: 0,
                    not_aligned: 3,
                },
                legacy_skills_to_import: 42,
                legacy_skills_to_clean: 2,
            }),
        });
        let rendered = review_text(&app, false).to_string();
        assert!(rendered.contains("50/50 found  · 5 to align"));
        assert!(rendered.contains("Legacy   42 to import  · 2 to remove"));
        assert!(!rendered.contains("0 missing"));
        let backend = TestBackend::new(82, 16);
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
        assert!(rendered.contains("Import existing configuration"));
        assert!(!rendered.contains('⣿'));
        Ok(())
    }

    #[test]
    fn current_review_keeps_wrapped_notices_inset_and_uses_semantic_colors()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::with_selection(50, &[Provider::Claude, Provider::Codex]);
        app.set_review(Review {
            groups: Default::default(),
            applicable: true,
            notices: vec![LifecycleNotice {
                code: LifecycleNoticeCode::CodexUsesImplicitSkills,
                message: "Codex reads the canonical skills directly; only its agents are managed as an integration."
                    .to_owned(),
            }],
            assessment: Some(WorkflowAssessment {
                state: WorkflowState::Current,
                skills: AssetSummary {
                    total: 50,
                    found: 50,
                    missing: 0,
                    not_aligned: 0,
                },
                agents: AssetSummary {
                    total: 6,
                    found: 6,
                    missing: 0,
                    not_aligned: 0,
                },
                legacy_skills_to_import: 0,
                legacy_skills_to_clean: 0,
            }),
        });
        let backend = TestBackend::new(82, 16);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| render(frame, &app, true))?;
        let buffer = terminal.backend().buffer();
        let rows = buffer
            .content
            .chunks(usize::from(buffer.area.width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>();
        let continuation = rows
            .iter()
            .find(|row| row.contains("an integration."))
            .ok_or("wrapped notice continuation is missing")?;

        assert!(continuation.starts_with("  "));
        assert_eq!(buffer[(2, 1)].fg, Color::Green);
        assert_eq!(buffer[(11, 3)].fg, Color::Green);
        assert_eq!(buffer[(2, 7)].fg, Color::Yellow);
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
