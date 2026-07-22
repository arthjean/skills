#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Previous,
    Next,
    Toggle,
    Confirm,
    Cancel,
    Resize(u16, u16),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Outcome {
    Continue,
    Finished,
}

#[derive(Debug)]
pub struct App {
    enabled: [bool; 2],
    selected: usize,
    finished: bool,
    terminal_size: Option<(u16, u16)>,
    skill_count: usize,
}

impl App {
    pub const fn new(skill_count: usize) -> Self {
        Self {
            enabled: [true, true],
            selected: 0,
            finished: false,
            terminal_size: None,
            skill_count,
        }
    }

    pub fn update(&mut self, action: Action) -> Outcome {
        match action {
            Action::Previous => {
                self.selected = self.selected.saturating_sub(1);
            }
            Action::Next => {
                self.selected = (self.selected + 1).min(Provider::ALL.len() - 1);
            }
            Action::Toggle => self.enabled[self.selected] = !self.enabled[self.selected],
            Action::Confirm | Action::Cancel => self.finished = true,
            Action::Resize(width, height) => self.terminal_size = Some((width, height)),
        }

        if self.finished {
            Outcome::Finished
        } else {
            Outcome::Continue
        }
    }

    pub const fn enabled(&self, index: usize) -> bool {
        self.enabled[index]
    }

    pub const fn selected(&self) -> usize {
        self.selected
    }

    pub const fn skill_count(&self) -> usize {
        self.skill_count
    }

    pub const fn terminal_size(&self) -> Option<(u16, u16)> {
        self.terminal_size
    }

    pub fn selection_summary(&self) -> String {
        Provider::ALL
            .iter()
            .enumerate()
            .filter(|(index, _)| self.enabled[*index])
            .map(|(_, provider)| provider.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, App, Outcome};

    #[test]
    fn selection_state_handles_navigation_toggle_and_resize() {
        let mut app = App::new(50);
        assert_eq!(app.update(Action::Next), Outcome::Continue);
        assert_eq!(app.selected(), 1);
        assert_eq!(app.update(Action::Toggle), Outcome::Continue);
        assert!(!app.enabled(1));
        assert_eq!(app.selection_summary(), "Claude Code");
        assert_eq!(app.update(Action::Previous), Outcome::Continue);
        assert_eq!(app.update(Action::Resize(120, 40)), Outcome::Continue);
        assert_eq!(app.terminal_size(), Some((120, 40)));
        assert_eq!(app.update(Action::Confirm), Outcome::Finished);
    }

    #[test]
    fn navigation_stays_within_provider_bounds() {
        let mut app = App::new(50);
        app.update(Action::Previous);
        assert_eq!(app.selected(), 0);
        app.update(Action::Next);
        app.update(Action::Next);
        assert_eq!(app.selected(), 1);
        assert_eq!(app.update(Action::Cancel), Outcome::Finished);
    }
}
