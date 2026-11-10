//! Live identities and literal producer content, grouped by project, independent of screen geometry.
use crate::{Window, panel::summary::Snapshot, project::Key};
use eframe::egui::Color32;
use std::{cmp::Reverse, time::SystemTime};

/// The section of every console outside a Git repository, folders included (D85, B1).
pub(super) const OTHER: &str = "Other consoles";

pub(super) struct Entry {
    pub(super) pane: usize,
    pub(super) console_label: String,
    pub(super) tab_label: String,
    pub(super) snapshot: Snapshot,
}

pub(super) enum Kind {
    Repository { ink: Color32 },
    Other,
}

pub(super) struct Section {
    pub(super) name: String,
    pub(super) kind: Kind,
    pub(super) entries: Vec<Entry>,
}

impl Section {
    pub(super) fn ink(&self) -> Option<Color32> {
        match self.kind {
            Kind::Repository { ink } => Some(ink),
            Kind::Other => None,
        }
    }
}

/// The repository a console belongs to, as its header already resolved it.
pub(super) struct Membership {
    pub(super) key: Key,
    pub(super) name: String,
    pub(super) ink: Color32,
}

pub(super) fn sections(window: &Window, now: SystemTime) -> Vec<Section> {
    let mut consoles = Vec::new();
    for tab in &window.tabs {
        for pane in tab.panes() {
            // A folder identity keeps its header badge but is not a project here: D85 defines the
            // project as the repository that holds its roadmap, and a folder holds none.
            let membership = pane
                .project
                .as_ref()
                .filter(|badge| matches!(badge.key, Key::Repository(_)))
                .map(|badge| Membership {
                    key: badge.key.clone(),
                    name: badge.name.clone(),
                    ink: badge.ink,
                });
            consoles.push((
                membership,
                Entry {
                    pane: pane.name,
                    console_label: crate::pane_view::display_label(pane).into_owned(),
                    tab_label: display(tab.label.as_deref(), "Tab", tab.name),
                    snapshot: crate::panel::summary::snapshot(&pane.panel_state, now),
                },
            ));
        }
    }
    group(consoles)
}

/// Consoles arrive in tab and tree order; that order breaks every tie below.
pub(super) fn group(consoles: Vec<(Option<Membership>, Entry)>) -> Vec<Section> {
    let mut sections: Vec<(Option<Key>, Section)> = Vec::new();
    for (membership, entry) in consoles {
        let key = membership.as_ref().map(|m| m.key.clone());
        let at = match sections.iter().position(|(known, _)| *known == key) {
            Some(at) => at,
            None => {
                sections.push((
                    key,
                    Section {
                        name: membership
                            .as_ref()
                            .map_or_else(|| OTHER.to_owned(), |m| m.name.clone()),
                        kind: membership
                            .as_ref()
                            .map_or(Kind::Other, |m| Kind::Repository { ink: m.ink }),
                        entries: Vec::new(),
                    },
                ));
                sections.len() - 1
            }
        };
        sections[at].1.entries.push(entry);
    }
    for (_, section) in &mut sections {
        order(&mut section.entries);
    }
    // Stable: equal sections keep the order of their first console. After `order`, a section's
    // first entry carries its strongest claim.
    sections.sort_by_key(|(key, section)| {
        (
            key.is_none(),
            Reverse(section.entries.first().and_then(|e| e.snapshot.attention)),
        )
    });
    sections.into_iter().map(|(_, section)| section).collect()
}

fn order(entries: &mut [Entry]) {
    entries.sort_by_key(|entry| Reverse(entry.snapshot.attention));
}

fn display(label: Option<&str>, kind: &str, name: usize) -> String {
    label
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{kind} {}", name + 1))
}

pub(super) fn reconcile(selected: Option<usize>, ordinal: usize, order: &[usize]) -> Option<usize> {
    selected.filter(|id| order.contains(id)).or_else(|| {
        order
            .get(ordinal.min(order.len().saturating_sub(1)))
            .copied()
    })
}

/// The claim, or the absence of any document; nothing when a document claims nothing.
pub(super) fn status(snapshot: &Snapshot) -> Option<String> {
    match snapshot.attention {
        Some(a) => Some(format!(
            "{} {}",
            crate::panel::attention_symbol(a.class),
            match a.class {
                panel_contract::Attention::Wait => "it is waiting for you",
                panel_contract::Attention::Block => "stuck on something else, not you",
                panel_contract::Attention::Done => "it finished and nobody looked",
            }
        )),
        None if snapshot.document.is_none() => Some("No published state".to_owned()),
        None => None,
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
    use panel_contract::Attention::{Block, Done, Wait};
    fn entry(pane: usize) -> Entry {
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
    fn claiming(pane: usize, class: panel_contract::Attention) -> Entry {
        let mut entry = entry(pane);
        entry.snapshot.attention = Some(crate::panel::AttentionState {
            class,
            backed: true,
        });
        entry
    }
    fn repository(name: &str) -> Option<Membership> {
        Some(Membership {
            key: Key::Repository(format!("/work/{name}/.git").into()),
            name: name.to_owned(),
            ink: Color32::RED,
        })
    }
    fn shape(sections: &[Section]) -> Vec<(String, Vec<usize>)> {
        sections
            .iter()
            .map(|s| (s.name.clone(), s.entries.iter().map(|e| e.pane).collect()))
            .collect()
    }
    #[test]
    fn removed_selection_clamps_without_changing_a_surviving_identity() {
        let order = [8, 3];
        assert_eq!(reconcile(Some(3), 0, &order), Some(3));
        assert_eq!(reconcile(Some(7), 10, &order), Some(3));
        assert_eq!(reconcile(Some(7), 0, &order), Some(8));
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
        assert_eq!(status(&snapshot), None);
        let lines: Vec<_> = excerpt(snapshot.document.as_ref().unwrap()).collect();
        assert_eq!(
            lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            vec!["old blocker", "second", "third"]
        );
        assert_eq!(lines[0].indent, 2);
        assert_eq!(
            status(&entry(0).snapshot).as_deref(),
            Some("No published state")
        );
        assert!(
            status(&claiming(0, Wait).snapshot)
                .unwrap()
                .ends_with("it is waiting for you")
        );
    }
    #[test]
    fn classes_dominate_backing_and_ties_keep_input_tree_order() {
        let mut rows = vec![
            entry(0),
            entry(1),
            entry(2),
            entry(3),
            entry(4),
            entry(5),
            entry(6),
        ];
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
    #[test]
    fn each_repository_is_a_section_and_everything_else_is_the_last_one() {
        let sections = group(vec![
            (None, entry(0)),
            (repository("ivy"), entry(1)),
            (repository("facq"), entry(2)),
            (repository("ivy"), entry(3)),
        ]);
        assert_eq!(
            shape(&sections),
            vec![
                ("ivy".to_owned(), vec![1, 3]),
                ("facq".to_owned(), vec![2]),
                (OTHER.to_owned(), vec![0]),
            ]
        );
        assert_eq!(sections[2].ink(), None);
        assert_eq!(sections[0].ink(), Some(Color32::RED));
    }
    #[test]
    fn sections_follow_their_strongest_claim_and_ties_keep_the_first_console() {
        let sections = group(vec![
            (None, claiming(0, Wait)),
            (repository("x"), claiming(1, Block)),
            (repository("y"), entry(2)),
            (repository("y"), claiming(3, Wait)),
            (repository("z"), claiming(4, Block)),
        ]);
        assert_eq!(
            shape(&sections),
            vec![
                ("y".to_owned(), vec![3, 2]),
                ("x".to_owned(), vec![1]),
                ("z".to_owned(), vec![4]),
                (OTHER.to_owned(), vec![0]),
            ],
            "a waiting console outside any repository still comes last"
        );
    }
}
