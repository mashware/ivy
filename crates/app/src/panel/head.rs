//! The panel's head: the rows the application owns above the producer's body — identity, the
//! claim's row, the warning and the buttons.
//!
//! It came out of its parent under **D28** when D97's two orders for the head (historical and
//! integrated) took that file past its ceiling.

use super::{
    AttentionState, MARGIN, MINIMUM_SLOT, Panel, Row, RowRole, Style, Theme, action_symbol,
    attention_symbol, attention_text, identity, sanitize, wrap, wrap_verbatim,
};
use eframe::egui::Color32;

/// Turns a panel's head into the rows that are going to be painted, at the given width in cells,
/// the way the historical panel lays it out: identity, claim, warning, buttons and a blank row.
///
/// **Here is everything it costs and nothing it draws**, and that separation is what allows remembering
/// the result between frames: as long as the file and the width do not change, the layout is the same.
/// Without it, wrapping the panel would be recalculated sixty times per second while the console beside it
/// dumps. The integrated panel composes the same pieces in its own order (`cache.rs`, D97).
pub(super) fn lay_out_head(
    panel: Option<&Panel>,
    id: &str,
    width: usize,
    theme: &Theme,
    warning: Option<&str>,
    attention: Option<AttentionState>,
    held_by: Option<&str>,
) -> Vec<Row> {
    let mut rows = Vec::new();

    if panel.is_none_or(|panel| panel.header) {
        let phase = panel.and_then(|panel| panel.phase.as_deref());
        identity::lay_out(&mut rows, id, phase, held_by, width, theme, None);
    }
    lay_out_notices(&mut rows, attention, warning, width, theme);
    lay_out_buttons(&mut rows, panel, width, theme, None);

    // The blank row is written by the layout, not by the painting: separated from the body, what the
    // application puts reads as what it is and not as the producer's first line.
    if !rows.is_empty() {
        rows.push(Row::text(0, String::new(), Color32::TRANSPARENT));
    }

    rows
}

/// Adds the application's own rows: the claim, when it is laid out as a row, and the warning.
pub(super) fn lay_out_notices(
    rows: &mut Vec<Row>,
    attention: Option<AttentionState>,
    warning: Option<&str>,
    width: usize,
    theme: &Theme,
) {
    // **The claim goes in the application's row and not in the identity one**, and that frontier is the
    // reason this milestone exists: `header: false` hides the pane's identity, and the answer to «do I
    // have to go there?» cannot be turned off by the producer with a flag of its own. It is the same
    // frontier that the warning below already fulfills. Only the historical panel lays it out as a
    // row; the integrated one paints it as a strip and hands no claim here (D97).
    if let Some(attention) = attention {
        wrap(
            rows,
            0,
            &format!(
                "{} {}",
                attention_symbol(attention.class),
                attention_text(attention.class)
            ),
            theme.attention_color(attention.class, attention.backed),
            width,
        );
    }

    // **The warning goes on top and not at the end**, because below is where the clip eats what does not
    // fit: the warning that a panel is not trustworthy would be exactly the first thing to disappear from
    // a long panel.
    if let Some(warning) = warning {
        wrap(
            rows,
            0,
            &sanitize(warning),
            theme.panel_color(None, Some(Style::Warn)),
            width,
        );
    }
}

/// Pushes a blank row unless the rows are empty or already end in one.
pub(super) fn separate(rows: &mut Vec<Row>) {
    if rows
        .last()
        .is_some_and(|row| !row.text.is_empty() || row.header.is_some())
    {
        rows.push(Row::text(0, String::new(), Color32::TRANSPARENT));
    }
}

/// Adds the producer's buttons: each one's label, then the literal command it sends.
///
/// With the integrated chrome's `kit`, a blank row separates them from what is above, the label
/// sits at the command's inset in the interface face, and the command's first row carries a `›`
/// prompt drawn beside it — never inside the text that is sent (D97).
pub(super) fn lay_out_buttons(
    rows: &mut Vec<Row>,
    panel: Option<&Panel>,
    width: usize,
    theme: &Theme,
    kit: Option<&crate::theme::Roles>,
) {
    // **The buttons go above the body and below the warning, inside the block the application owns.**
    // Below is where the clip eats what does not fit, and a button whose command row got eaten would be
    // a label you can press over a command you cannot read — the one state the rule about seeing a
    // command before it runs forbids.
    // **A panel too narrow to show a command at all declares no buttons.** The wrapping has a floor
    // on its slot — without one it would put zero characters per row and hang — so below that floor
    // it lays the text out wider than the strip and the painting clips what does not fit: the person
    // would see a fragment and the press would send the whole string. Leaving these rows untagged is
    // the same mechanism as a button the clip cut in half: unreachable rather than refused.
    let shows_a_command = width.saturating_sub(MARGIN as usize) >= MINIMUM_SLOT;
    for (nth, action) in panel
        .iter()
        .flat_map(|panel| panel.actions.iter())
        .filter(|_| shows_a_command)
        .enumerate()
    {
        if kit.is_some() && nth == 0 {
            separate(rows);
        }
        let first = rows.len();
        let label = match kit {
            Some(_) => action.label().to_owned(),
            None => format!("{} {}", action_symbol(), action.label()),
        };
        let inset = if kit.is_some() { MARGIN as usize } else { 0 };
        wrap(
            rows,
            inset,
            &sanitize(&label),
            theme.panel_color(None, None),
            width,
        );
        rows[first].column = inset;
        let body = rows.len();
        // **The command is painted in the ordinary ink and not in the dimmed one.** `clip`'s «… N more»
        // uses the dim colour to say «this is elided», and the one sentence on this panel that a person
        // has to read before pressing anything must not be styled as de-emphasis.
        wrap_verbatim(
            rows,
            MARGIN as usize,
            action.send(),
            theme.panel_color(None, None),
            width,
        );
        for row in &mut rows[first..body] {
            row.role = RowRole::ButtonLabel(nth);
            row.ui_face = kit.is_some();
        }
        if let Some(roles) = kit {
            rows[body].glyph = Some(('›', roles.tx3));
        }
        for row in &mut rows[body..] {
            row.role = RowRole::ButtonBody(nth);
        }
    }
}
