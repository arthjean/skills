use std::collections::BTreeMap;

use crate::lifecycle::LifecycleNotice;
use crate::plan::{Plan, PlanAction, PlanEntry};
pub use crate::provider::ProviderId as Provider;
use crate::provider::ResolvedRoots;
use crate::workflow::WorkflowAssessment;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Previous,
    Next,
    Toggle,
    Confirm,
    Cancel,
    Interrupt,
    Resize(u16, u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Selection,
    Review,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Outcome {
    Continue,
    SelectionConfirmed(Vec<Provider>),
    ApplicationConfirmed,
    Cancelled,
    Interrupted,
}

#[derive(Clone, Debug)]
pub struct Review {
    pub groups: BTreeMap<(String, PlanAction), Vec<PlanEntry>>,
    pub applicable: bool,
    pub notices: Vec<LifecycleNotice>,
    pub assessment: Option<WorkflowAssessment>,
}

impl Review {
    pub fn from_plan(plan: &Plan, notices: &[LifecycleNotice], roots: &ResolvedRoots) -> Self {
        Self::build(plan, notices, roots, None)
    }

    pub fn for_workflow(
        plan: &Plan,
        notices: &[LifecycleNotice],
        roots: &ResolvedRoots,
        assessment: WorkflowAssessment,
    ) -> Self {
        Self::build(plan, notices, roots, Some(assessment))
    }

    fn build(
        plan: &Plan,
        notices: &[LifecycleNotice],
        roots: &ResolvedRoots,
        assessment: Option<WorkflowAssessment>,
    ) -> Self {
        let mut groups = BTreeMap::<(String, PlanAction), Vec<PlanEntry>>::new();
        for entry in &plan.entries {
            let root = roots
                .allowed_top_level_roots()
                .filter(|root| entry.destination.starts_with(&root.lexical))
                .max_by_key(|root| root.lexical.components().count())
                .map_or_else(
                    || "unknown root".to_owned(),
                    |root| root.lexical.display().to_string(),
                );
            groups
                .entry((root, entry.action))
                .or_default()
                .push(entry.clone());
        }
        Self {
            groups,
            applicable: plan.applicable,
            notices: notices.to_vec(),
            assessment,
        }
    }
}

#[derive(Debug)]
pub struct App {
    enabled: [bool; 2],
    detected: [bool; 2],
    selected: usize,
    step: Step,
    review: Option<Review>,
    terminal_size: Option<(u16, u16)>,
    skill_count: usize,
    message: Option<String>,
}

impl App {
    pub fn new(skill_count: usize, detected: &[Provider]) -> Self {
        Self {
            enabled: [true, true],
            detected: Provider::ALL.map(|provider| detected.contains(&provider)),
            selected: 0,
            step: Step::Selection,
            review: None,
            terminal_size: None,
            skill_count,
            message: None,
        }
    }

    pub fn with_selection(skill_count: usize, providers: &[Provider]) -> Self {
        let mut app = Self::new(skill_count, providers);
        app.enabled = Provider::ALL.map(|provider| providers.contains(&provider));
        app
    }

    pub fn update(&mut self, action: Action) -> Outcome {
        match action {
            Action::Previous => self.selected = self.selected.saturating_sub(1),
            Action::Next => self.selected = (self.selected + 1).min(Provider::ALL.len() - 1),
            Action::Toggle if self.step == Step::Selection => {
                self.enabled[self.selected] = !self.enabled[self.selected];
                self.message = None;
            }
            Action::Toggle => {}
            Action::Confirm if self.step == Step::Selection => {
                let providers = self.selected_providers();
                if providers.is_empty() {
                    self.message = Some("Select at least one provider.".to_owned());
                } else {
                    return Outcome::SelectionConfirmed(providers);
                }
            }
            Action::Confirm if self.review.as_ref().is_some_and(|review| review.applicable) => {
                return Outcome::ApplicationConfirmed;
            }
            Action::Confirm => {
                self.message = Some(
                    "Application is disabled until every conflict is resolved; use adopt or remove the conflicting destination."
                        .to_owned(),
                );
            }
            Action::Cancel => return Outcome::Cancelled,
            Action::Interrupt => return Outcome::Interrupted,
            Action::Resize(width, height) => self.terminal_size = Some((width, height)),
        }
        Outcome::Continue
    }

    pub fn set_review(&mut self, review: Review) {
        self.review = Some(review);
        self.step = Step::Review;
        self.message = None;
    }

    pub fn selected_providers(&self) -> Vec<Provider> {
        Provider::ALL
            .iter()
            .enumerate()
            .filter(|(index, _)| self.enabled[*index])
            .map(|(_, provider)| *provider)
            .collect()
    }

    pub const fn enabled(&self, index: usize) -> bool {
        self.enabled[index]
    }

    pub const fn detected(&self, index: usize) -> bool {
        self.detected[index]
    }

    pub const fn selected(&self) -> usize {
        self.selected
    }

    pub const fn step(&self) -> Step {
        self.step
    }

    pub const fn review(&self) -> Option<&Review> {
        self.review.as_ref()
    }

    pub const fn skill_count(&self) -> usize {
        self.skill_count
    }

    pub const fn terminal_size(&self) -> Option<(u16, u16)> {
        self.terminal_size
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn selection_summary(&self) -> String {
        self.selected_providers()
            .iter()
            .map(|provider| provider.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, App, Outcome, Provider, Review};

    #[test]
    fn selection_requires_a_provider_and_reports_detected_state() {
        let mut app = App::new(50, &[Provider::Claude]);
        assert!(app.detected(0));
        assert!(!app.detected(1));
        app.update(Action::Toggle);
        app.update(Action::Next);
        app.update(Action::Toggle);
        assert_eq!(app.update(Action::Confirm), Outcome::Continue);
        assert_eq!(app.message(), Some("Select at least one provider."));
    }

    #[test]
    fn navigation_resize_and_interruption_are_deterministic() {
        let mut app = App::new(50, &[]);
        assert_eq!(app.update(Action::Next), Outcome::Continue);
        assert_eq!(app.selected(), 1);
        assert_eq!(app.update(Action::Resize(120, 40)), Outcome::Continue);
        assert_eq!(app.terminal_size(), Some((120, 40)));
        assert_eq!(app.update(Action::Interrupt), Outcome::Interrupted);
    }

    #[test]
    fn blocked_review_ignores_toggle_and_explains_why_apply_is_disabled() {
        let mut app = App::with_selection(50, &[Provider::Codex]);
        assert!(!app.enabled(0));
        assert!(app.enabled(1));
        assert_eq!(app.selection_summary(), "Codex");
        app.set_review(Review {
            groups: Default::default(),
            applicable: false,
            notices: Vec::new(),
            assessment: None,
        });
        assert_eq!(app.update(Action::Toggle), Outcome::Continue);
        assert_eq!(app.update(Action::Confirm), Outcome::Continue);
        assert!(
            app.message()
                .is_some_and(|message| message.contains("disabled"))
        );
    }
}
