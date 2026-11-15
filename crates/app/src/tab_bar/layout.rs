//! Card contents share the same bounds in painting and pointer routing.
use super::CLOSE_MIN_WIDTH;
use eframe::egui;

pub(super) struct Layout {
    pub body: egui::Rect,
    pub close: Option<egui::Rect>,
    pub indicator: egui::Rect,
    pub label: egui::Rect,
}

impl Layout {
    pub fn new(card: egui::Rect, size: f32, status: bool, close: bool) -> Self {
        let padding = 6.0_f32.min(card.width() / 2.0);
        let extent = size + 2.0;
        // Attention is reread after input; reserve its slot even before a claim arrives.
        let close = (close
            && card.width() >= CLOSE_MIN_WIDTH
            && card.width() >= padding + extent + 10.0 + 25.0)
            .then(|| {
                egui::Rect::from_min_max(
                    egui::pos2(card.right() - 25.0, card.top()),
                    card.right_bottom(),
                )
            });
        let body = egui::Rect::from_min_max(
            card.min,
            egui::pos2(close.map_or(card.right(), |r| r.left()), card.bottom()),
        );
        let left = body.left() + padding;
        let d = extent
            .min((body.width() - padding * 2.0).max(0.0))
            .min(body.height());
        let indicator = egui::Rect::from_center_size(
            egui::pos2(left + d / 2.0, body.center().y),
            egui::vec2(d, d),
        );
        let label_left = if status {
            indicator.right() + 10.0
        } else {
            left
        };
        let label = egui::Rect::from_min_max(
            egui::pos2(label_left.min(body.right()), body.top()),
            body.right_bottom(),
        );
        Self {
            body,
            close,
            indicator,
            label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_spacing_and_targets_remain_disjoint_at_all_widths() {
        for size in [8.0, 12.0, 20.0, 32.0] {
            for width in [0.0, 2.0, 20.0, 40.0, 64.0, 65.0, 66.0, 73.0, 74.0, 190.0] {
                for status in [false, true] {
                    let card =
                        egui::Rect::from_min_size(egui::pos2(7.0, 9.0), egui::vec2(width, 50.0));
                    let layout = Layout::new(card, size, status, true);
                    assert!(card.contains_rect(layout.body));
                    assert!(layout.body.contains_rect(layout.indicator));
                    assert!(layout.body.contains_rect(layout.label));
                    assert_eq!(layout.indicator.center().y, card.center().y);
                    if width == 190.0 {
                        assert_eq!(layout.indicator.width(), size + 2.0);
                        if status {
                            assert_eq!(layout.label.left() - layout.indicator.right(), 10.0);
                        }
                    }
                    if let Some(close) = layout.close {
                        assert_eq!(layout.body.right(), close.left());
                        assert!(layout.label.right() <= close.left());
                        if status {
                            assert!(layout.indicator.right() + 10.0 <= close.left());
                        }
                    }
                }
            }
        }
    }
}
