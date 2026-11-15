//! Title sizes share measured line heights with paint and input geometry.
use crate::theme::metrics;
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

    /// A tab: the kit's 34 points, grown when the configured label needs more.
    pub(crate) fn tab_height(self) -> f32 {
        metrics::TAB_HEIGHT.max(self.tab_line_height + 12.0)
    }

    /// The title bar that holds the tabs: the kit's 40 points, keeping the same air above a tab
    /// that a larger label made taller.
    pub(crate) fn bar_height(self) -> f32 {
        self.tab_height() + (metrics::TITLE_BAR_HEIGHT - metrics::TAB_HEIGHT)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A tab label whose line needs more than the kit's 34 points grows the tab, and the bar keeps
    /// the same six points above it.
    #[test]
    fn a_large_tab_label_grows_the_tab_and_the_bar_with_it() {
        let default = Sizes::default();
        assert_eq!(default.tab_height(), 34.0);
        assert_eq!(default.bar_height(), 40.0);
        let large = Sizes {
            tab_line_height: 40.0,
            ..Sizes::new(12.0, 32.0)
        };
        assert_eq!(large.tab_height(), 52.0);
        assert_eq!(large.bar_height(), 58.0);
    }
}
