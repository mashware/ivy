//! Resolved panel rows carry the theme that gave their colours meaning.
use super::head::{lay_out_buttons, lay_out_head, lay_out_notices, separate};
use super::{AttentionState, Line, PanelState, Row, Style, Theme, color, fold, identity};
use crate::theme::Roles;

/// Which panel the window paints.
///
/// **One value for the whole decision**, mapped once in [`PanelLayout::new`] into everything that
/// differs: the claim's row, the identity's form, the marks' inks, the headers, the order of the
/// head and the buttons' frames. Passing it in pieces would let a layout mix the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chrome {
    /// The panel as it was before the restyle, which the batches without the integrated chrome
    /// still paint and measure (D13).
    Historical,
    /// The restyled panel: the claim as a strip, the phase as a chip, framed buttons, drawn
    /// chevrons and marks in two inks (D97).
    Integrated,
}

/// What a layout was made for.
///
/// **The line the app puts in enters the key.** A panel that ages changes what has to be painted
/// without its file having been touched, so a cache that only looked at the width would keep
/// showing the layout from when the panel was fresh — and the warning would never appear.
///
/// **And so does the chrome**: the historical panel lays the claim out as a row and the identity
/// with the pane's id, the integrated one omits the claim row, lays the identity out as a chip
/// and splits marks into glyphs, so the rows differ (D97).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LayoutKey {
    pub(super) width: usize,
    pub(super) theme: Theme,
    pub(super) warning: Option<String>,
    pub(super) chrome: Chrome,
}

/// A layout and its resolved-colour cache key: changing the theme must repaint quiet producers.
pub(super) struct PanelLayout {
    pub(super) key: LayoutKey,
    /// The interface roles of the key's theme for the integrated chrome, kept beside it: deriving
    /// them walks contrast ratios, which is work neither a frame nor the historical panel should
    /// pay for.
    pub(super) kit: Option<Roles>,
    pub(super) rows: Vec<Row>,
}

impl PanelLayout {
    pub(super) fn new(
        state: &PanelState,
        id: &str,
        key: LayoutKey,
        attention: Option<AttentionState>,
    ) -> Self {
        let (width, theme) = (key.width, &key.theme);
        let panel = state.last_good.as_deref();
        let lines: &[Line] = panel.map_or(&[], |panel| &panel.lines);
        let held_by = state.displayed_source();
        let mut rows;
        let kit = match key.chrome {
            Chrome::Historical => {
                let warning = key.warning.as_deref();
                rows = lay_out_head(panel, id, width, theme, warning, attention, held_by);
                super::provenance::append_row(&mut rows, state, width, theme);
                fold::append_blocks(&mut rows, lines, &state.folds, width, theme, None);
                None
            }
            Chrome::Integrated => {
                let roles = Roles::of(theme);
                rows = Vec::new();
                if panel.is_none_or(|panel| panel.header) {
                    let phase = panel.and_then(|panel| panel.phase.as_deref());
                    identity::lay_out(&mut rows, id, phase, held_by, width, theme, Some(&roles));
                }
                lay_out_notices(&mut rows, None, key.warning.as_deref(), width, theme);
                super::provenance::append_row(&mut rows, state, width, theme);
                // The design reads identity, title, summary, buttons, sections: a leading title and
                // at most one line under it go above the buttons, and nothing else does, so no body
                // can push the buttons more than two rows down (D97).
                let lead = leading_title_block(lines);
                fold::append_blocks(
                    &mut rows,
                    &lines[..lead],
                    &state.folds,
                    width,
                    theme,
                    Some(&roles),
                );
                lay_out_buttons(&mut rows, panel, width, theme, Some(&roles));
                if lead < lines.len() {
                    separate(&mut rows);
                }
                fold::append_blocks(
                    &mut rows,
                    &lines[lead..],
                    &state.folds,
                    width,
                    theme,
                    Some(&roles),
                );
                Some(roles)
            }
        };
        color::flag(&mut rows);
        Self { key, kit, rows }
    }
}

/// How many lines open the panel as its title block: a first line styled `title`, at most one line
/// under it that is neither a gap nor a section — the design's summary — and the gap that closes
/// them, if one does. Nothing when the panel does not open with a title.
fn leading_title_block(lines: &[Line]) -> usize {
    if lines
        .first()
        .is_none_or(|line| line.style != Some(Style::Title))
    {
        return 0;
    }
    let ordinary = |at: usize| {
        lines
            .get(at)
            .is_some_and(|line| line.style != Some(Style::Section) && !fold::is_gap(line))
    };
    let lead = if ordinary(1) { 2 } else { 1 };
    lead + usize::from(lines.get(lead).is_some_and(fold::is_gap))
}
