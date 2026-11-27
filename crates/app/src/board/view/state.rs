use crate::board::cards;
use eframe::egui::Rect;

#[derive(Default)]
pub(in crate::board) struct ViewState {
    pub(in crate::board) milestones: super::super::milestones::state::Reader,
    pub(in crate::board) cards: std::collections::HashMap<usize, cards::Reader>,
    pub(in crate::board) child_hits: Vec<(Rect, cards::Command)>,
    pub(in crate::board) child_pressed: Option<(Rect, cards::Command)>,
    pub(in crate::board) link_to_open: Option<String>,
    pub(in crate::board) document_to_open: Option<(usize, std::path::PathBuf)>,
    pub(in crate::board) painted: Vec<(usize, Rect)>,
    pub(in crate::board) pressed: Option<(usize, Rect)>,
}

impl ViewState {
    pub(in crate::board) fn apply_card_command(
        &mut self,
        command: &cards::Command,
        timing: &mut crate::measure::Board,
    ) {
        let pane = match command {
            cards::Command::Body(pane, _)
            | cards::Command::Expand(pane)
            | cards::Command::Compact(pane) => *pane,
            cards::Command::Bar(_) => return,
            cards::Command::Source(pane, path) => {
                self.document_to_open = Some((*pane, path.clone()));
                return;
            }
            cards::Command::Milestone(action) => {
                self.milestones.apply(action);
                return;
            }
        };
        if !self.cards.contains_key(&pane) {
            return;
        }
        match command {
            cards::Command::Body(_, crate::panel::body::Action::Link(target)) => {
                self.link_to_open = Some(target.clone())
            }
            cards::Command::Body(_, crate::panel::body::Action::Fold(..)) => {
                self.cards.get_mut(&pane).unwrap().apply(command);
                timing.folded();
            }
            cards::Command::Expand(_) => {
                self.cards.get_mut(&pane).unwrap().apply(command);
                timing.expanded();
            }
            cards::Command::Compact(_) => self.cards.get_mut(&pane).unwrap().apply(command),
            cards::Command::Bar(_) | cards::Command::Milestone(_) | cards::Command::Source(..) => {}
        }
    }
}
