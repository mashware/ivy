//! Shared targets keep the controls that are painted and the presses they receive in agreement.

use eframe::egui;

use crate::{painter::CellMetrics, theme::Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    SplitRight,
    SplitDown,
    TogglePanel,
}

impl Action {
    pub(crate) const ALL: [Self; 3] = [Self::SplitRight, Self::SplitDown, Self::TogglePanel];

    pub(crate) fn index(self) -> usize {
        match self {
            Self::SplitRight => 0,
            Self::SplitDown => 1,
            Self::TogglePanel => 2,
        }
    }

    pub(crate) fn label(self, folded: bool) -> &'static str {
        match self {
            Self::SplitRight => "Split right",
            Self::SplitDown => "Split down",
            Self::TogglePanel if folded => "Show information panel",
            Self::TogglePanel => "Hide information panel",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    Action(Action),
    Menu,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Controls {
    strip: egui::Rect,
    targets: [Option<(Hit, egui::Rect)>; 3],
}

impl Controls {
    pub(crate) fn within(
        strip: egui::Rect,
        metrics: &CellMetrics,
        close: Option<egui::Rect>,
    ) -> Self {
        let mut controls = Self {
            strip,
            targets: [None; 3],
        };
        if strip.height() <= 0.0 {
            return controls;
        }
        let width = super::BUTTON_CELLS * metrics.width();
        let left = strip.left() + super::MARGIN;
        let grip_start = strip.center().x - super::GRIP_CELLS * metrics.width() / 2.0;
        let limit = close.map_or(grip_start, |close| grip_start.min(close.left()));
        let target = |left| {
            egui::Rect::from_min_max(
                egui::pos2(left, strip.top()),
                egui::pos2(left + width, strip.bottom()),
            )
        };
        if left + 3.0 * width + super::MARGIN <= limit {
            for (index, action) in Action::ALL.into_iter().enumerate() {
                controls.targets[index] =
                    Some((Hit::Action(action), target(left + index as f32 * width)));
            }
        } else if left + width + super::MARGIN <= limit {
            controls.targets[0] = Some((Hit::Menu, target(left)));
        }
        controls
    }

    pub(crate) fn targets(&self) -> impl Iterator<Item = (Hit, egui::Rect)> + '_ {
        self.targets.iter().flatten().copied()
    }

    pub(crate) fn hit(&self, at: egui::Pos2) -> Option<Hit> {
        self.targets().find_map(|(hit, rect)| {
            (rect.contains(at) && at.x < rect.right() && at.y < rect.bottom()).then_some(hit)
        })
    }

    pub(crate) fn paint(
        &self,
        ui: &egui::Ui,
        theme: &Theme,
        enabled: [bool; 3],
        folded: bool,
        id: egui::Id,
    ) {
        let painter = ui
            .painter()
            .with_clip_rect(ui.clip_rect().intersect(self.strip));
        for (index, (hit, rect)) in self.targets().enumerate() {
            let available = match hit {
                Hit::Action(action) => enabled[action.index()],
                Hit::Menu => true,
            };
            let label = match hit {
                Hit::Action(action) => action.label(folded),
                Hit::Menu => "Pane actions",
            };
            // Hover cannot take keyboard focus or become a second dispatcher for clicks.
            let response = ui.interact(rect, id.with(index), egui::Sense::hover());
            let explanation = match hit {
                Hit::Action(Action::TogglePanel) if !available => {
                    format!("{label}: no information published")
                }
                Hit::Action(_) if !available => {
                    format!("{label}: not enough room to split this console")
                }
                _ => label.to_owned(),
            };
            response.on_hover_text(explanation);
            let color = if available {
                theme.foreground()
            } else {
                theme.foreground().gamma_multiply(0.35)
            };
            paint_icon(&painter, rect, hit, color, folded);
        }
    }
}

fn paint_icon(
    painter: &egui::Painter,
    target: egui::Rect,
    hit: Hit,
    color: egui::Color32,
    folded: bool,
) {
    let radius = target.width().min(target.height()) * 0.3;
    let icon = egui::Rect::from_center_size(target.center(), egui::Vec2::splat(radius * 2.0));
    let stroke = egui::Stroke::new(1.0, color);
    if hit == Hit::Menu {
        for offset in [-1.0, 0.0, 1.0] {
            painter.circle_filled(icon.center() + egui::vec2(offset * radius, 0.0), 1.0, color);
        }
        return;
    }
    painter.rect_stroke(icon, 0.0, stroke, egui::StrokeKind::Inside);
    match hit {
        Hit::Action(Action::SplitRight) => {
            painter.line_segment(
                [
                    egui::pos2(icon.center().x, icon.top()),
                    egui::pos2(icon.center().x, icon.bottom()),
                ],
                stroke,
            );
        }
        Hit::Action(Action::SplitDown) => {
            painter.line_segment(
                [
                    egui::pos2(icon.left(), icon.center().y),
                    egui::pos2(icon.right(), icon.center().y),
                ],
                stroke,
            );
        }
        Hit::Action(Action::TogglePanel) => {
            let side = egui::Rect::from_min_max(
                egui::pos2(icon.center().x + radius * 0.2, icon.top()),
                icon.max,
            );
            if !folded {
                painter.rect_filled(side, 0.0, color);
            } else {
                painter.line_segment([side.left_top(), side.left_bottom()], stroke);
            }
        }
        Hit::Menu => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controls(width: f32) -> Controls {
        Controls::within(
            egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(width, 22.0)),
            &CellMetrics::of(8.0, 16.0),
            None,
        )
    }

    #[test]
    fn a_wide_strip_exposes_all_three_actions_in_order() {
        let controls = controls(400.0);
        let targets = controls.targets().collect::<Vec<_>>();
        assert_eq!(targets.len(), 3);
        assert_eq!(
            controls.hit(egui::pos2(111.0, 61.0)),
            Some(Hit::Action(Action::SplitRight))
        );
        assert_eq!(
            controls.hit(egui::pos2(127.0, 61.0)),
            Some(Hit::Action(Action::SplitDown))
        );
        assert_eq!(
            controls.hit(egui::pos2(143.0, 61.0)),
            Some(Hit::Action(Action::TogglePanel))
        );
        assert!(
            targets[2].1.right() < 288.0,
            "the central handle begins at x=288"
        );
    }

    #[test]
    fn a_narrow_strip_offers_a_menu_and_a_tiny_one_keeps_the_handle() {
        let compact = controls(120.0);
        assert_eq!(compact.targets().count(), 1);
        assert_eq!(compact.hit(egui::pos2(111.0, 61.0)), Some(Hit::Menu));
        assert_eq!(controls(40.0).targets().count(), 0);
    }

    #[test]
    fn the_shared_vertical_edge_has_one_owner_and_the_console_row_has_none() {
        let controls = controls(400.0);
        assert_eq!(
            controls.hit(egui::pos2(119.0, 61.0)),
            Some(Hit::Action(Action::SplitDown))
        );
        assert_eq!(controls.hit(egui::pos2(151.0, 61.0)), None);
        assert_eq!(controls.hit(egui::pos2(111.0, 72.0)), None);
        assert_eq!(
            controls.hit(egui::pos2(111.0, 71.0)),
            Some(Hit::Action(Action::SplitRight))
        );
    }

    #[test]
    fn a_close_target_can_never_overlap_a_left_control() {
        let strip = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(400.0, 22.0));
        let close = egui::Rect::from_min_size(egui::pos2(140.0, 50.0), egui::vec2(16.0, 22.0));
        let controls = Controls::within(strip, &CellMetrics::of(8.0, 16.0), Some(close));
        assert_eq!(controls.targets().count(), 1);
        assert!(
            controls
                .targets()
                .all(|(_, rect)| rect.right() < close.left())
        );
        assert_eq!(controls.hit(close.center()), None);
    }
}
