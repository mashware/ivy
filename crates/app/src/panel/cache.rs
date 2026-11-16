//! Resolved panel rows carry the theme that gave their colours meaning.
use super::{AttentionState, PanelState, Row, Theme, color, fold, lay_out_head};
use crate::theme::Roles;

/// What a layout was made for: the width in cells, the theme, the application's warning line and
/// whether the window paints its integrated chrome.
///
/// **The line the app puts in enters the key.** A panel that ages changes what has to be painted
/// without its file having been touched, so a cache that only looked at the width would keep
/// showing the layout from when the panel was fresh — and the warning would never appear.
///
/// **And so does the chrome**: the measured, historical one keeps the claim as a row of text and
/// the identity as the pane's id, the integrated one paints both outside that text (D97).
pub(super) type LayoutKey = (usize, Theme, Option<String>, bool);

/// A layout and its resolved-colour cache key: changing the theme must repaint quiet producers.
pub(super) struct PanelLayout {
    pub(super) key: LayoutKey,
    /// The interface roles of the key's theme, kept beside it: deriving them walks contrast
    /// ratios, which is work a frame should not repeat for a theme that did not change.
    pub(super) roles: Roles,
    pub(super) rows: Vec<Row>,
}

impl PanelLayout {
    pub(super) fn new(
        state: &PanelState,
        id: &str,
        key: LayoutKey,
        attention: Option<AttentionState>,
    ) -> Self {
        let (width, theme, warning, integrated) = &key;
        let (width, theme, integrated) = (*width, theme, *integrated);
        let roles = Roles::of(theme);
        let mut rows = lay_out_head(
            state.last_good.as_deref(),
            Some(id).filter(|_| !integrated),
            width,
            theme,
            warning.as_deref(),
            attention.filter(|_| !integrated),
            state.displayed_source(),
        );
        super::provenance::append_row(&mut rows, state, width, theme);
        if let Some(panel) = state.last_good.as_deref() {
            fold::append_blocks(&mut rows, panel, &state.folds, width, theme);
        }
        color::flag(&mut rows);
        Self { key, roles, rows }
    }
}
