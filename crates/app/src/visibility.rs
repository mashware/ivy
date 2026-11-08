//! Geometry and panel refresh for consoles outside the current view.
use crate::{Window, painter::CellMetrics};
use eframe::egui;
use std::time::{Instant, SystemTime};

impl Window {
    /// Gives the consoles of the tabs that are not seen the grid they would have if they were.
    ///
    /// **Without this a background tab is stuck with [`crate::INITIAL_GRID`] for as long as it lasts**,
    /// because `measured` is only written when painting and they are not painted. And that is not
    /// an internal detail: it is the size [`crate::Pane::resize`] announces to the program inside, so an
    /// agent working in a tab you are not looking at formats its output for a window that does not
    /// exist —80 start-up columns when its own are 57— and on going back to that tab what was
    /// written stays fitted to somebody else's width until the program redraws. The milestone's
    /// promise is that the background one keeps running; running with the wrong grid it runs badly.
    ///
    /// **Each one is tiled with its own fraction, not with the active one's**: the divider belongs
    /// to the layout, and using the visible tab's would measure these consoles with a divider that
    /// is not theirs — the same defect that already cost a review when the bar's click was attended
    /// to after tiling.
    ///
    /// **The panel is reread here too, just as in the painting, and the first attempt did not do
    /// it.** How much width the console has left depends on whether there is a panel —the panel
    /// takes 36 columns—, so a background pane whose producer publishes for the first time would be
    /// measured **36 columns too wide** for as long as the background lasts: the same defect this
    /// function comes to fix, with the sign flipped and bigger. Rereading is not expensive by
    /// default: `refresh_its_panel` leaves through an atomic `swap` and only reaches the disk on
    /// the frame in which the watcher warned that **that** `panel.json` changed, which is exactly
    /// when it is needed.
    ///
    /// **And that read is declared**, which is the other half. It falls inside the interface pass,
    /// and here there is no painting to time it: without declaring it, the subtraction would hand
    /// it as a gift to the framework — the figure D3 decides with. It is kept in
    /// `panel_reread_unpainted` and collected by [`Window::close_the_unpainted_rounds`], which runs
    /// right afterwards.
    ///
    /// **The stopwatch only counts if this console has a panel directory.** Without one there is no
    /// file to read and the refresh is the `swap` and nothing else; publishing those nanoseconds
    /// under the «panel» label would be a span with the name changed, and in this project that has
    /// already invalidated a whole measurement.
    ///
    /// With the window minimized or occluded there is no interface pass and this does not run, just
    /// as the painting that measures the visible one does not run: nobody ends up worse off than
    /// the one that is seen.
    pub(crate) fn size_hidden(&mut self, ctx: &egui::Context, area: egui::Rect, m: &CellMetrics) {
        // A single clock for all of them, and here one really is enough: the only thing that
        // depends on the time is whether a pane's panel is stale, and nothing is painted afterwards
        // that could contradict it.
        let now = SystemTime::now();
        let active = self.active;
        // **The console of a tab nobody is looking at is measured the same way as a painted one.**
        // Its strip is not drawn — nothing of that tab is — but the row is reserved all the same, or
        // the program inside would be told a grid one row taller than the one it gets the moment its
        // tab comes to the front.
        let headers = self.headers;
        let integrated = self.chrome.enabled();
        let zoomed = self.pane_view.zoomed(self.active().name);

        for (which, tab) in self.tabs.iter_mut().enumerate() {
            if which == active && zoomed.is_none() {
                continue;
            }

            // **By that tab's tree and not by an assumed tiling of two.** A background tab has
            // whatever layout it has, and giving it a template's would announce to its program a
            // grid that is not its own — which is exactly the defect F2.1 fixed for the tabs and
            // that here would come back through the tree's door.
            let zoom_work = (which == active).then(Instant::now);
            let mut declared = std::time::Duration::ZERO;
            let tiling = tab.root.place(area, m.width());
            for (pane, rect) in tab.panes_mut().into_iter().zip(tiling.parts) {
                if which == active && zoomed == Some(pane.name) {
                    continue;
                }
                let refreshed = pane.refresh_its_panel(ctx);
                let before_the_panel = Instant::now();
                let rect = crate::pane_view::padded_rect(rect, integrated);
                let panel_split = pane.chrome_within(rect, m, now, headers);
                // **The span's boundary is the same as in the painting**, which also closes its
                // stopwatch after tiling: `tile_with_its_panel` asks whether the panel is visible,
                // and that question goes as far as composing the text of a warning when the
                // producer is quiet or broken. Closing earlier would leave that work out, and the
                // two consoles of one and the same report would measure the «panel» span with
                // different boundaries depending on whether they are looked at or not.
                if pane.panel.is_some() {
                    // **The same shape as in the painting, and for the same reason**: the clock
                    // starts after the refresh and what the refresh cost is added back, so the
                    // control channel is never inside a clock the panel's span is built from. A
                    // console of a background tab is drained just the same, and a span that meant
                    // «reading a file» in one place and «reading a file plus applying a document»
                    // in the other would be one number meaning two things.
                    let panel_work = before_the_panel.elapsed();
                    pane.panel_reread_unpainted = refreshed.reading + panel_work;
                    pane.control_unpainted = refreshed.applying;
                    declared += refreshed.reading + refreshed.applying + panel_work;
                }

                pane.measured = m.cells_in(panel_split.console().size());
            }
            if let Some(started) = zoom_work {
                self.chrome
                    .record_work(started.elapsed().saturating_sub(declared));
                self.timed_separately += self.chrome.take_cost();
            }
        }
    }
}
