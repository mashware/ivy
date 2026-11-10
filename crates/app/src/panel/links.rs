//! A line's `link`: the compact label painted beside or below it, and the region a click on it
//! reaches. **D51**.
//!
//! A region is recorded only for an address the application's boundary in `crate::links` accepts,
//! and it holds that boundary's form of it — the one the browser will load. The rule for what may
//! open lives there, once; this module asks it rather than repeating it.

use eframe::egui::{Align2, Color32, Painter, Pos2, Rect};
use panel_contract::Mark;

use super::{Drawn, MARGIN, PanelState, Row, sanitize};
use crate::painter::CellMetrics;
use crate::theme::Theme;

/// A panel link as it was drawn: the address a click opens, and only the ink that represented it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PaintedLink {
    target: String,
    area: Rect,
}

/// The link adornment attached beside a line or hanging below it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LaidOutLink {
    pub(super) column: usize,
    right_aligned: bool,
    label: String,
    target: String,
}

impl PanelState {
    /// The address under `point`, if the last painting was settled.
    ///
    /// Returning the target owned by [`Drawn`] is what makes this lookup independent of a newer
    /// document already committed to the state. The caller owns the desktop hand-off.
    pub fn link_at(&self, point: Pos2) -> Option<&str> {
        self.drawn
            .as_ref()
            .filter(|drawn| drawn.settled)?
            .links
            .iter()
            .find(|link| link.area.contains(point))
            .map(|link| link.target.as_str())
    }

    /// The link labels the last painting drew, for measurement and input injection.
    pub fn links_drawn(&self) -> impl Iterator<Item = Rect> + '_ {
        self.drawn
            .iter()
            .flat_map(|drawn| drawn.links.iter().map(|link| link.area))
    }
}

/// Gives a panel destination the same compact, deterministic label as the prototype.
///
/// Presentation only: it is built from the boundary's form when the address has one, and from the
/// producer's text when the address is refused, whose label stays painted and inert.
pub(super) fn link_label(target: &str) -> String {
    for (needle, prefix) in [
        ("/merge_requests/", "!"),
        ("/pull/", "#"),
        ("/issues/", "#"),
        ("/-/pipelines/", "pipeline "),
    ] {
        if let Some(number) = decimal_path_component(target, needle) {
            return format!("{prefix}{number} ↗");
        }
    }

    let without_scheme = target
        .strip_prefix("https://")
        .or_else(|| target.strip_prefix("http://"))
        .unwrap_or(target);
    let without_www = without_scheme
        .strip_prefix("www.")
        .unwrap_or(without_scheme);
    let host = without_www.split('/').next().unwrap_or_default();
    let trimmed = target.trim_end_matches('/');
    let tail = trimmed.rsplit('/').next().unwrap_or_default();
    if tail.is_empty() {
        format!("{host} ↗")
    } else {
        format!("{host}/…/{} ↗", tail.chars().take(12).collect::<String>())
    }
}

fn decimal_path_component<'a>(target: &'a str, needle: &str) -> Option<&'a str> {
    let rest = target.split_once(needle)?.1;
    let number = rest
        .split(|character: char| !character.is_ascii_digit())
        .next()
        .unwrap_or_default();
    (!number.is_empty()).then_some(number)
}

/// Attaches a line's link to the rows that line was laid out into.
///
/// The prototype's compact rule: keep the destination on the first row when two cells of separation
/// and the whole label fit; otherwise hang it under the line at the body's indent.
pub(super) fn attach(
    rows: &mut Vec<Row>,
    target: &str,
    indent: usize,
    width: usize,
    color: Color32,
) {
    let shown = crate::links::WebTarget::parse(target);
    let label = sanitize(&link_label(
        shown.as_ref().map_or(target, |shown| shown.as_str()),
    ));
    let label_width = label.chars().count();
    let first_width = rows
        .first()
        .map(|row| row.column + row.text.chars().count())
        .unwrap_or_default();
    let link = |column, right_aligned| LaidOutLink {
        column,
        right_aligned,
        label: label.clone(),
        target: target.to_owned(),
    };

    if first_width + 2 + label_width <= width {
        if let Some(first) = rows.first_mut() {
            first.link = Some(link(width.saturating_sub(label_width), true));
        }
    } else {
        let mut below = Row::text(0, String::new(), color);
        below.link = Some(link(indent, false));
        rows.push(below);
    }
}

/// Paints one row's label, and returns the region it inked with whether all of it is inside the
/// panel.
pub(super) fn paint_label(
    painter: &Painter,
    link: &LaidOutLink,
    y: f32,
    metrics: &CellMetrics,
    theme: &Theme,
    discoverer: &mut crate::fonts::Discoverer,
) -> (PaintedLink, bool) {
    let rect = painter.clip_rect();
    discoverer.note_text(&link.label);
    let color = theme.panel_color(Some(Mark::Pending), None);
    let x = if link.right_aligned {
        let width = painter
            .layout_no_wrap(link.label.clone(), metrics.font(), color)
            .size()
            .x;
        rect.right() - MARGIN * metrics.width() - width
    } else {
        rect.left() + MARGIN * metrics.width() + link.column as f32 * metrics.width()
    };
    let ink = painter.text(
        Pos2::new(x, y),
        Align2::LEFT_TOP,
        &link.label,
        metrics.font(),
        color,
    );
    let area = Rect::from_min_max(
        Pos2::new(ink.left(), y),
        Pos2::new(ink.right(), y + metrics.height()),
    );
    let fits = ink.left() >= rect.left() && ink.right() <= rect.right();
    (
        PaintedLink {
            target: link.target.clone(),
            area,
        },
        fits,
    )
}

/// The regions a click may reach: only labels painted whole inside the panel, over an address the
/// application would open — kept in the form the browser will receive.
///
/// A clipped label and a refused address («file:», «javascript:», a typo) are absence rather than a
/// refusal: no hand, no tooltip, and a click on them routes as if no link were there. The label
/// itself is still painted, because it is the producer's text.
pub(super) fn reachable(painted: Vec<(PaintedLink, bool)>) -> Vec<PaintedLink> {
    painted
        .into_iter()
        .filter(|(link, fits)| *fits && link.area.is_positive())
        .filter_map(|(link, _)| {
            let target = crate::links::WebTarget::parse(&link.target)?;
            Some(PaintedLink {
                target: target.as_str().to_owned(),
                area: link.area,
            })
        })
        .collect()
}

/// Shows the whole address behind a compact label — the form the browser will load.
///
/// The compact text is only an affordance; the tooltip is the disclosure that lets a person inspect
/// a producer-labelled destination before activating it. It uses the same settled, self-contained
/// region as presses, so a stale or half-painted label cannot disclose a different document either.
pub(super) fn disclose_hovered(painter: &Painter, drawn: Option<&Drawn>, pane_id: &str) {
    let Some(drawn) = drawn.filter(|drawn| drawn.settled) else {
        return;
    };
    let Some(pointer) = painter.ctx().input(|input| input.pointer.hover_pos()) else {
        return;
    };
    let Some(link) = drawn.links.iter().find(|link| link.area.contains(pointer)) else {
        return;
    };

    painter
        .ctx()
        .set_cursor_icon(eframe::egui::CursorIcon::PointingHand);
    eframe::egui::Tooltip::always_open(
        painter.ctx().clone(),
        painter.layer_id(),
        eframe::egui::Id::new(("panel-link", pane_id, link.target.as_str())),
        link.area,
    )
    .at_pointer()
    .show(|ui| {
        ui.label(&link.target);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_link_labels_keep_the_prototypes_stable_names() {
        assert_eq!(
            link_label("https://gitlab.example/group/project/-/merge_requests/42"),
            "!42 ↗"
        );
        assert_eq!(
            link_label("https://github.com/mashware/ivy-history-private/pull/257"),
            "#257 ↗"
        );
        assert_eq!(
            link_label("https://ci.example/-/pipelines/913"),
            "pipeline 913 ↗"
        );
        assert_eq!(
            link_label("https://example.test/releases/a-very-long-release-name"),
            "example.test/…/a-very-long- ↗"
        );
    }
}
