//! Resolved panel rows carry the theme that gave their colours meaning.
use super::{AttentionState, PanelState, Row, Theme, color, fold, lay_out_head};

/// A layout and its resolved-colour cache key: changing the theme must repaint quiet producers.
pub(super) struct PanelLayout {
    pub(super) width: usize,
    pub(super) theme: Theme,
    /// **The line the app puts in enters the cache key.** A panel that ages changes what has to be
    /// painted without its file having been touched, so a cache that only looked at the width would keep
    /// showing the layout from when the panel was fresh — and the warning would never appear.
    pub(super) warning: Option<String>,
    pub(super) rows: Vec<Row>,
}

impl PanelLayout {
    pub(super) fn new(
        state: &PanelState,
        id: &str,
        width: usize,
        theme: &Theme,
        warning: Option<String>,
        attention: Option<AttentionState>,
    ) -> Self {
        let mut rows = lay_out_head(
            state.last_good.as_deref(),
            id,
            width,
            theme,
            warning.as_deref(),
            attention,
            state.holder.as_ref().map(|held| held.source.as_str()),
        );
        if let Some(panel) = state.last_good.as_deref() {
            fold::append_blocks(&mut rows, panel, &state.folds, width, theme);
        }
        color::flag(&mut rows);
        Self {
            width,
            theme: *theme,
            warning,
            rows,
        }
    }
}
