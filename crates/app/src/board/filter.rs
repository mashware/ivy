//! What the reader asked the Board to show: an arrangement and a narrowing, never a reclassification.
use super::{
    model::{Entry, Section},
    observation::Column,
};

/// The two Board arrangements kept side by side while the owner picks one (D93, amended by #93).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Arrangement {
    #[default]
    ByState,
    ByMilestone,
}

#[derive(Default)]
pub(super) struct Filter {
    pub(super) claiming_only: bool,
    pub(super) query: String,
    /// Whether typed text goes to the query instead of being swallowed by the Board.
    pub(super) editing: bool,
}

impl Filter {
    pub(super) fn is_active(&self) -> bool {
        self.claiming_only || !self.query.trim().is_empty()
    }
}

/// What a console claims, judged on its effective column — the projection the columns, the card's
/// pill and the project header all paint. Finished counts: D18 makes «finished, nobody looked» a
/// claim on the reader, ranked below waiting and stuck.
pub(super) fn claims(column: Column) -> bool {
    matches!(column, Column::Wait | Column::Block | Column::Done)
}

/// The most a query holds. A paste is the likeliest way text reaches the field, and the whole query
/// is matched against every console each frame.
pub(super) const QUERY_LIMIT: usize = 256;

fn matches(entry: &Entry, query: &str) -> bool {
    query.is_empty()
        || [
            entry.console_label.as_str(),
            entry.tab_label.as_str(),
            super::cards::title(entry),
        ]
        .iter()
        .any(|text| text.to_lowercase().contains(query))
}

/// Narrows the sections in place. With a narrowing active, a project left without consoles is
/// dropped — its roadmap included — because what the reader asked for is consoles.
pub(super) fn apply(
    sections: &mut Vec<Section>,
    filter: &Filter,
    column: impl Fn(&Entry) -> Column,
) {
    if !filter.is_active() {
        return;
    }
    let query = filter.query.trim().to_lowercase();
    for section in sections.iter_mut() {
        section.entries.retain(|entry| {
            (!filter.claiming_only || claims(column(entry))) && matches(entry, &query)
        });
    }
    sections.retain(|section| !section.entries.is_empty());
}

/// Typed text edits the query; the Board consumes those events either way (D93's keyboard focus).
pub(super) fn edit(filter: &mut Filter, event: &eframe::egui::Event) -> bool {
    use eframe::egui::{Event, Key};
    if !filter.editing {
        return false;
    }
    match event {
        Event::Text(text) | Event::Paste(text) => {
            let room = QUERY_LIMIT.saturating_sub(filter.query.chars().count());
            filter
                .query
                .extend(text.chars().filter(|c| !c.is_control()).take(room));
            true
        }
        // Enter leaves the field rather than opening the selected card under it.
        Event::Key {
            key: Key::Enter,
            pressed: true,
            ..
        } => {
            filter.editing = false;
            true
        }
        Event::Key {
            key: Key::Backspace,
            pressed: true,
            ..
        } => {
            filter.query.pop();
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::model::Kind;

    fn entry(pane: usize, console: &str, tab: &str) -> Entry {
        Entry {
            pane,
            console_label: console.into(),
            tab_label: tab.into(),
            snapshot: crate::panel::summary::Snapshot {
                document: None,
                warning: None,
                attention: None,
                outside: false,
            },
        }
    }
    fn section(entries: Vec<Entry>) -> Section {
        Section {
            key: None,
            kind: Kind::Other,
            entries,
            roadmap: None,
            ink: None,
        }
    }
    fn panes(sections: &[Section]) -> Vec<usize> {
        sections
            .iter()
            .flat_map(|s| &s.entries)
            .map(|e| e.pane)
            .collect()
    }
    fn column_of(pane: usize) -> Column {
        match pane {
            1 => Column::Wait,
            2 => Column::Block,
            3 => Column::Working,
            5 => Column::Done,
            _ => Column::Idle,
        }
    }

    #[test]
    fn claiming_keeps_waiting_stuck_and_unread_finished_consoles_and_drops_emptied_projects() {
        let mut sections = vec![
            section(vec![entry(1, "api", "Billing"), entry(3, "web", "Billing")]),
            section(vec![entry(4, "ci", "Ops")]),
            section(vec![
                entry(2, "tokens", "Dashboard"),
                entry(5, "docs", "Dashboard"),
            ]),
        ];
        let filter = Filter {
            claiming_only: true,
            ..Filter::default()
        };
        apply(&mut sections, &filter, |e| column_of(e.pane));
        assert_eq!(panes(&sections), [1, 2, 5]);
        assert_eq!(
            sections.len(),
            2,
            "the project with only an idle console is gone"
        );
    }

    #[test]
    fn the_query_matches_console_tab_and_task_names_ignoring_case() {
        let mut sections = vec![section(vec![
            entry(1, "refund-api", "Billing"),
            entry(3, "scratch", "OPS"),
            entry(4, "other", "Dashboard"),
        ])];
        let filter = Filter {
            query: "  ops ".into(),
            ..Filter::default()
        };
        apply(&mut sections, &filter, |e| column_of(e.pane));
        assert_eq!(panes(&sections), [3]);
        let mut sections = vec![section(vec![entry(1, "refund-api", "Billing")])];
        let filter = Filter {
            query: "REFUND".into(),
            ..Filter::default()
        };
        apply(&mut sections, &filter, |e| column_of(e.pane));
        assert_eq!(panes(&sections), [1]);
        // The task name a producer published, matched where neither label carries the word.
        let mut titled = entry(4, "shell", "Tab");
        titled.snapshot.document = Some(std::sync::Arc::new(
            panel_contract::parse_document(
                r#"{"lines":[{"text":"Refund webhooks","style":"title"}]}"#,
            )
            .unwrap(),
        ));
        let mut sections = vec![section(vec![titled, entry(3, "web", "Tab")])];
        let filter = Filter {
            query: "webhooks".into(),
            ..Filter::default()
        };
        apply(&mut sections, &filter, |e| column_of(e.pane));
        assert_eq!(panes(&sections), [4]);
    }

    #[test]
    fn no_narrowing_leaves_plan_only_projects_in_place() {
        let mut sections = vec![section(vec![]), section(vec![entry(4, "ci", "Ops")])];
        apply(&mut sections, &Filter::default(), |e| column_of(e.pane));
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn typing_edits_the_query_only_while_the_field_is_being_edited() {
        use eframe::egui::{Event, Key, Modifiers};
        let backspace = Event::Key {
            key: Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let mut filter = Filter::default();
        assert!(!edit(&mut filter, &Event::Text("a".into())));
        assert_eq!(filter.query, "");
        filter.editing = true;
        assert!(edit(&mut filter, &Event::Text("ab\u{7}".into())));
        assert!(edit(&mut filter, &backspace));
        assert_eq!(filter.query, "a");
        assert!(edit(
            &mut filter,
            &Event::Paste("x".repeat(QUERY_LIMIT * 4))
        ));
        assert_eq!(
            filter.query.chars().count(),
            QUERY_LIMIT,
            "a paste stops at the limit"
        );
        let enter = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(edit(&mut filter, &enter));
        assert!(!filter.editing, "Enter leaves the field");
    }
}
