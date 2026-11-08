//! What a console painted, kept as the authority for the links a pointer reaches in it. **D51**.
//!
//! A click resolves against this snapshot and never against the live grid: output can be
//! interpreted between a paint and a click, and then the live grid is already newer than what the
//! person aimed at.

use eframe::egui::{self, Pos2, Rect, vec2};
use term_core::{Link, LinkSource, View};

use super::{WebTarget, opens_a_console_link};
use crate::painter::CellMetrics;

/// What one console actually painted in its last visible pass.
pub(crate) struct PaintedConsole {
    pub(crate) view: View,
    pub(crate) area: Rect,
    pub(crate) frame: u64,
}

impl PaintedConsole {
    pub(crate) fn is_current(&self, frame: u64) -> bool {
        matches!(self.frame.checked_add(1), Some(next) if next == frame) || self.frame == frame
    }

    pub(crate) fn link_at(&self, point: Pos2, metrics: &CellMetrics, frame: u64) -> Option<Link> {
        // The preceding frame is what is on the glass when a new one starts. The current one is
        // accepted for egui's repeated pass, although in egui 0.36 that pass carries no button
        // events, and the window's once-per-frame barrier would refuse a replayed one.
        // **The painted area bounds the point, not only the grid**: while a panel grows, the grid is
        // resized a frame after the area shrinks, so a cell past the area exists and is not visible.
        if !self.is_current(frame) || !self.area.contains(point) {
            return None;
        }
        let offset = point - self.area.min;
        let column = offset.x / metrics.width();
        let row = offset.y / metrics.height();
        let size = self.view.size();
        if column < 0.0
            || row < 0.0
            || column >= f32::from(size.cols)
            || row >= f32::from(size.rows)
        {
            return None;
        }
        self.view.link_at(row as u16, column as u16)
    }

    pub(crate) fn local_at(
        &self,
        point: Pos2,
        metrics: &CellMetrics,
        frame: u64,
    ) -> Option<super::LocalTarget> {
        if !self.is_current(frame) || !self.area.contains(point) {
            return None;
        }
        let offset = point - self.area.min;
        let col = offset.x / metrics.width();
        let row = offset.y / metrics.height();
        let size = self.view.size();
        if col < 0.0 || row < 0.0 || col >= f32::from(size.cols) || row >= f32::from(size.rows) {
            return None;
        }
        let target = self.view.local_path_at(row as u16, col as u16)?;
        super::LocalTarget::parse(&target, self.view.working_directory())
    }

    /// Shows the exact target under a Ctrl-hovering pointer, before the click that would open it.
    ///
    /// An OSC 8 label may say anything, so this is the one place its destination is visible. It
    /// reads the same snapshot and the same cell geometry a click resolves against, so what is
    /// disclosed is what would open.
    pub(crate) fn disclose_hovered_link(&self, ui: &egui::Ui, metrics: &CellMetrics) {
        let (pointer, chord) = ui
            .ctx()
            .input(|input| (input.pointer.hover_pos(), input.modifiers));
        let Some(pointer) = pointer.filter(|_| opens_a_console_link(chord)) else {
            return;
        };
        if let Some(target) = self.local_at(pointer, metrics, self.frame) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                egui::Id::new(("local-link", &target.path)),
                Rect::from_min_size(pointer, vec2(metrics.width(), metrics.height())),
            )
            .at_pointer()
            .show(|ui| {
                ui.label(target.path.display().to_string());
            });
            return;
        }
        let Some(link) = self.link_at(pointer, metrics, self.frame) else {
            return;
        };
        let Some(target) = WebTarget::parse(&link.target) else {
            return;
        };
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            egui::Id::new(("console-link", target.as_str())),
            Rect::from_min_size(pointer, vec2(metrics.width(), metrics.height())),
        )
        .at_pointer()
        .show(|ui| {
            ui.label(target.as_str());
        });
    }

    /// The centre of the first painted cell of each terminal source, plain first and OSC 8 second,
    /// for the bench's injector.
    ///
    /// **It stops at the first row that completes the pair**: batch W writes both sources on the
    /// top two rows, and walking the rest of the grid every frame would be the bench measuring
    /// itself.
    pub(crate) fn a_point_per_source(&self, metrics: &CellMetrics) -> [Option<Pos2>; 2] {
        let mut found = [None; 2];
        let size = self.view.size();
        for row in 0..size.rows {
            if found.iter().all(Option::is_some) {
                break;
            }
            for col in 0..size.cols {
                let Some(link) = self.view.link_at(row, col) else {
                    continue;
                };
                let at = match link.source {
                    LinkSource::Plain => 0,
                    LinkSource::Osc8 => 1,
                };
                let centre = self.area.min
                    + vec2(
                        (f32::from(col) + 0.5) * metrics.width(),
                        (f32::from(row) + 0.5) * metrics.height(),
                    );
                if found[at].is_none() && self.area.contains(centre) {
                    found[at] = Some(centre);
                }
            }
        }
        found
    }
}

/// What one pane's painting leaves for the bench's link gesture: one point a click would open per
/// source — plain, OSC 8 and panel, in that order — and how many of the three it found.
///
/// **It counts sources found, not links painted**: only the first cell of each source is recorded,
/// and only batch W walks at all, so it says whether the injector had somewhere to click.
#[derive(Clone, Copy, Default)]
pub(crate) struct ForTheBench {
    pub(crate) sources_found: usize,
    pub(crate) points: [Option<Pos2>; 3],
    /// The frame this painting belongs to.
    pub(crate) frame: u64,
}

impl ForTheBench {
    pub(crate) fn of(
        console: [Option<Pos2>; 2],
        panel: &crate::panel::PanelState,
        frame: u64,
    ) -> Self {
        let [plain, osc8] = console;
        // A label a click would open, which excludes one painted on the frame its document changed.
        let panel = panel
            .links_drawn()
            .map(|area| area.center())
            .find(|centre| panel.link_at(*centre).is_some());
        let points = [plain, osc8, panel];
        Self {
            sources_found: points.iter().flatten().count(),
            points,
            frame,
        }
    }
}

impl ForTheBench {
    /// Hands what one pane's painting found to the report's tally and to the bench's injector.
    pub(crate) fn hand_over(
        self,
        report: &mut crate::measure::Links,
        injector: &mut Option<crate::injected::InjectedDrags>,
    ) {
        report.found += self.sources_found;
        if let Some(injector) = injector {
            injector.links_were_painted(self.points, self.frame);
        }
    }
}
