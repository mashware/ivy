//! Live identities and literal producer content, independent of screen geometry.
use crate::{Window, panel::summary::Snapshot};
use std::time::SystemTime;

pub(super) struct Entry {
    pub(super) pane: usize,
    pub(super) console_label: String,
    pub(super) tab_label: String,
    pub(super) snapshot: Snapshot,
}

pub(super) fn project(window: &Window, now: SystemTime) -> Vec<Entry> {
    let mut entries = Vec::new();
    for tab in &window.tabs {
        for pane in tab.panes() {
            entries.push(Entry {
                pane: pane.name,
                console_label: crate::pane_view::display_label(pane).into_owned(),
                tab_label: display(tab.label.as_deref(), "Tab", tab.name),
                snapshot: crate::panel::summary::snapshot(&pane.panel_state, now),
            });
        }
    }
    // Stable sorting preserves current tab and leaf order for equal claims.
    order(&mut entries);
    entries
}

fn order(entries: &mut [Entry]) {
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.snapshot.attention));
}

fn display(label: Option<&str>, kind: &str, name: usize) -> String {
    label
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{kind} {}", name + 1))
}

pub(super) fn reconcile(selected: Option<usize>, ordinal: usize, rows: &[Entry]) -> Option<usize> {
    selected
        .filter(|id| rows.iter().any(|r| r.pane == *id))
        .or_else(|| {
            rows.get(ordinal.min(rows.len().saturating_sub(1)))
                .map(|r| r.pane)
        })
}

pub(super) fn status(snapshot: &Snapshot) -> String {
    match snapshot.attention {
        Some(a) => format!(
            "{} {}",
            crate::panel::attention_symbol(a.class),
            match a.class {
                panel_contract::Attention::Wait => "it is waiting for you",
                panel_contract::Attention::Block => "stuck on something else, not you",
                panel_contract::Attention::Done => "it finished and nobody looked",
            }
        ),
        None if snapshot.document.is_some() => "No attention requested".to_owned(),
        None => "No published state".to_owned(),
    }
}

pub(super) fn excerpt(
    document: &panel_contract::Panel,
) -> impl Iterator<Item = &panel_contract::Line> {
    document
        .lines
        .iter()
        .filter(|line| !line.text.trim().is_empty())
        .take(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(pane: usize) -> Entry {
        Entry {
            pane,
            console_label: String::new(),
            tab_label: String::new(),
            snapshot: Snapshot {
                document: None,
                warning: None,
                attention: None,
            },
        }
    }
    #[test]
    fn removed_selection_clamps_without_changing_a_surviving_identity() {
        let rows = vec![row(8), row(3)];
        assert_eq!(reconcile(Some(3), 0, &rows), Some(3));
        assert_eq!(reconcile(Some(7), 10, &rows), Some(3));
        assert_eq!(reconcile(Some(7), 0, &rows), Some(8));
        assert_eq!(reconcile(Some(7), 0, &[]), None);
    }
    #[test]
    fn phase_and_body_do_not_declare_attention_and_excerpt_is_literal() {
        let panel=panel_contract::parse_document(r#"{"phase":"done","lines":[{"text":"  "},{"text":"old blocker","mark":"block","indent":2},{"text":"second"},{"text":"third"},{"text":"fourth"}]}"#).unwrap();
        let snapshot = Snapshot {
            document: Some(std::sync::Arc::new(panel)),
            warning: None,
            attention: None,
        };
        assert_eq!(status(&snapshot), "No attention requested");
        let lines: Vec<_> = excerpt(snapshot.document.as_ref().unwrap()).collect();
        assert_eq!(
            lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            vec!["old blocker", "second", "third"]
        );
        assert_eq!(lines[0].indent, 2);
        assert_eq!(
            status(&Snapshot {
                document: None,
                warning: None,
                attention: None
            }),
            "No published state"
        );
    }
    #[test]
    fn classes_dominate_backing_and_ties_keep_input_tree_order() {
        use panel_contract::Attention::{Block, Done, Wait};
        let mut rows = vec![row(0), row(1), row(2), row(3), row(4), row(5), row(6)];
        for (id, class, backed) in [
            (1, Done, true),
            (2, Wait, false),
            (3, Block, true),
            (4, Wait, true),
            (5, Wait, true),
            (6, Done, false),
        ] {
            rows[id].snapshot.attention = Some(crate::panel::AttentionState { class, backed });
        }
        order(&mut rows);
        assert_eq!(
            rows.iter().map(|r| r.pane).collect::<Vec<_>>(),
            vec![4, 5, 2, 3, 1, 6, 0]
        );
    }
}
