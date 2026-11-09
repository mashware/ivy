//! Title sizes share measured line heights with paint and input geometry.
use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sizes {
    pane: f32,
    tab: f32,
    tab_line_height: f32,
}

impl Default for Sizes {
    fn default() -> Self {
        Self::new(12.0, 12.0)
    }
}

impl Sizes {
    pub(crate) fn new(pane: f32, tab: f32) -> Self {
        Self {
            pane,
            tab,
            tab_line_height: 0.0,
        }
    }

    pub(crate) fn pane_size(self) -> f32 {
        self.pane
    }

    pub(crate) fn tab_size(self) -> f32 {
        self.tab
    }

    pub(crate) fn tab_height(self) -> f32 {
        39.0_f32.max(self.tab_line_height + 12.0)
    }
}

pub(crate) fn begin_pass(window: &mut crate::Window, ctx: &egui::Context) {
    if !window.chrome.enabled() {
        return;
    }
    let started = std::time::Instant::now();
    let (pane, tab) = ctx.fonts_mut(|fonts| {
        (
            fonts.row_height(&egui::FontId::proportional(
                window.chrome.title_sizes.pane_size(),
            )),
            fonts.row_height(&egui::FontId::proportional(
                window.chrome.title_sizes.tab_size(),
            )),
        )
    });
    window.headers = window
        .headers
        .with_title_metrics(window.chrome.title_sizes.pane_size(), pane);
    window.chrome.title_sizes.tab_line_height = tab;
    window.settings_editor.record_geometry(started.elapsed());
    window.timed_separately += window.settings_editor.take_cost();
}
