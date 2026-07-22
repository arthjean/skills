use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, TerminalOptions, Viewport};

use crate::app::{Action, App, Outcome, Provider};

const INLINE_HEIGHT: u16 = 10;

pub fn run(mut app: App) -> io::Result<()> {
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

    let result = run_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;
        if run_debug_probe()? {
            return Ok(());
        }

        let action = action_for_event(event::read()?);

        if action.is_some_and(|action| app.update(action) == Outcome::Finished) {
            return Ok(());
        }
    }
}

fn action_for_event(event: Event) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(Action::Previous),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::Next),
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

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let [header, providers, explanation, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(4),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    frame.render_widget(
        Paragraph::new(format!(
            "Arthur Workflow catalog: {} skills",
            app.skill_count()
        ))
        .style(Style::default().add_modifier(Modifier::BOLD)),
        header,
    );

    let items = Provider::ALL.iter().enumerate().map(|(index, provider)| {
        let marker = if app.enabled(index) { "[x]" } else { "[ ]" };
        let style = if index == app.selected() {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(Line::from(format!("{marker} {}", provider.label()))).style(style)
    });
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(" Providers ")),
        providers,
    );

    frame.render_widget(
        Paragraph::new(
            "Codex reads the canonical $HOME/.agents/skills catalog whenever it exists. Provider selection controls managed agents and activations, not skill isolation.",
        )
        .wrap(Wrap { trim: true }),
        explanation,
    );
    frame.render_widget(
        Paragraph::new("↑/↓ move  Space toggle  Enter confirm  q cancel"),
        footer,
    );
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::{action_for_event, render, run_debug_probe};
    use crate::app::{Action, App};

    #[test]
    fn test_backend_renders_the_selection_contract() {
        let backend = TestBackend::new(78, 10);
        let terminal = Terminal::new(backend);
        assert!(terminal.is_ok());
        let mut terminal = match terminal {
            Ok(terminal) => terminal,
            Err(error) => panic!("TestBackend construction failed: {error}"),
        };
        let draw = terminal.draw(|frame| render(frame, &App::new(50)));
        assert!(draw.is_ok());

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content
            .chunks(usize::from(buffer.area.width))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>();
        assert_eq!(rendered.len(), 10);
        assert!(rendered[0].contains("Arthur Workflow catalog: 50 skills"));
        assert!(rendered[2].contains("[x] Claude Code"));
        assert!(rendered[3].contains("[x] Codex"));
        assert!(rendered[5].contains("Codex reads the canonical $HOME/.agents/skills"));
        assert!(rendered[8].contains("Enter confirm"));
    }

    #[test]
    fn keyboard_and_resize_events_map_to_shared_actions() {
        let key = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        assert_eq!(action_for_event(key(KeyCode::Up)), Some(Action::Previous));
        assert_eq!(
            action_for_event(key(KeyCode::Char('k'))),
            Some(Action::Previous)
        );
        assert_eq!(action_for_event(key(KeyCode::Down)), Some(Action::Next));
        assert_eq!(
            action_for_event(key(KeyCode::Char('j'))),
            Some(Action::Next)
        );
        assert_eq!(
            action_for_event(key(KeyCode::Char(' '))),
            Some(Action::Toggle)
        );
        assert_eq!(action_for_event(key(KeyCode::Enter)), Some(Action::Confirm));
        assert_eq!(action_for_event(key(KeyCode::Esc)), Some(Action::Cancel));
        assert_eq!(
            action_for_event(key(KeyCode::Char('q'))),
            Some(Action::Cancel)
        );
        assert_eq!(action_for_event(key(KeyCode::Char('x'))), None);
        assert_eq!(
            action_for_event(Event::Resize(120, 40)),
            Some(Action::Resize(120, 40))
        );
        assert_eq!(action_for_event(Event::FocusGained), None);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_probe_defaults_to_continuing_the_event_loop() {
        assert!(matches!(run_debug_probe(), Ok(false)));
    }
}
