//! Paint one complete pane: terminal snapshot, notices, panel and application chrome.
#[cfg(test)]
use super::Keys;
use super::{Blink, CellMetrics, CursorInk, notices, paint_with_ligatures};
use crate::theme::Theme;
use eframe::egui::{Painter, Rect};

/// The three things the frame resolves before it walks the panes, travelling together.
///
/// **It is an argument count and it is said so**, rather than dressed up as a concept: painting a
/// console reached eight arguments with the strip's decision, and the lint that guards this crate
/// stops at seven. Riding them in one value is the same answer [`Blink::keys`] already gave one
/// level down, for the same reason its docblock gives — an eighth argument is where a reader stops
/// being able to tell which value is which.
///
/// **None of the three is purely the session's**: the strip's decision is, the blink arrives
/// **narrowed per console** with [`Blink::on`] because whether the cursor is lit depends on which
/// console has the keyboard, and the button depends on how many consoles the tab being painted has.
#[derive(Clone, Copy)]
pub(crate) struct HowToPaint {
    pub(crate) integrated: bool,
    pub(crate) maximized: Option<usize>,
    /// Whether this console's cursor is lit, and for how much longer.
    pub(crate) blink: Blink,
    /// Whether a strip is painted over it. See [`crate::header::Headers`].
    pub(crate) headers: crate::header::Headers,
    /// Whether that strip carries the button that closes it. See [`crate::header::Button`].
    pub(crate) button: crate::header::Button,
    /// Whether batch W needs painted points for its injector.
    pub(crate) measure_links: bool,
    pub(crate) ligatures: bool,
}

impl HowToPaint {
    /// The three of them, in the order the frame resolves them.
    ///
    /// **It exists to keep the call site one line long**, which is not a style preference here:
    /// `lib.rs` is a ratchet that may only shrink (**D28**), and the literal form of this value
    /// costs that file three lines for nothing. The tests build it as a literal, which is fine —
    /// they are not counted.
    /// **The third arrives as the consoles of the tab being painted**, which is what the caller is
    /// holding — it is walking their rectangles — and what [`crate::header::Button::in_a_tab_of`]
    /// turns into the answer. Handing over the list rather than the verdict is what keeps the one
    /// production call site inside the line that `lib.rs`'s ratchet pays for; handing over the list
    /// rather than its **length** is what stops another `usize` in scope reaching this argument.
    pub(crate) fn of(
        blink: Blink,
        headers: crate::header::Headers,
        consoles: &[eframe::egui::Rect],
    ) -> Self {
        Self {
            integrated: false,
            maximized: None,
            blink,
            headers,
            button: crate::header::Button::in_a_tab_of(consoles),
            measure_links: false,
            ligatures: false,
        }
    }

    pub(crate) fn ligatures(mut self, enabled: bool) -> Self {
        self.ligatures = enabled;
        self
    }

    pub(crate) fn measuring_links(mut self, yes: bool) -> Self {
        self.measure_links = yes;
        self
    }

    pub(crate) fn integrated(mut self, yes: bool) -> Self {
        self.integrated = yes;
        self
    }
    pub(crate) fn maximized(mut self, pane: Option<usize>) -> Self {
        self.maximized = pane;
        self
    }
}

/// Paints a console whole: its grid, and over it whatever the **application** has to say.
///
/// **It exists so that `paint` can stay what it is** — a pure function from a view and a corner —
/// which is what the measurement bench and this module's own tests call. When it was written, a
/// second entry was cheaper than an argument on `paint` because `lib.rs` sat against **D28**'s
/// recorded number and the call site had to keep exactly the shape it had; that constraint went with
/// the delivery that moved [`paint_pane`] into this module, and `paint` does take the focus.
#[cfg(test)]
pub(crate) fn paint_a_console(
    painter: &Painter,
    console: &crate::Pane,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
    discoverer: &mut crate::fonts::Discoverer,
    ink: CursorInk,
) -> bool {
    let view = console.screen.view();
    let blinking =
        paint_a_console_view(painter, &view, theme, metrics, area, discoverer, ink, false);
    // This entry takes an ink rather than a blink, and an outlined cursor says nobody types here.
    // It cannot tell an absent window from another console, so what the window hands the painter
    // is tested through the window, never through this.
    let keys = if matches!(ink, CursorInk::Outline) {
        Keys::Elsewhere
    } else {
        Keys::Here
    };
    notices::paint_them_over_it(painter, console, theme, metrics, area, keys);
    blinking
}

/// Paints from a snapshot the caller can retain as the authority for later pointer effects.
#[expect(
    clippy::too_many_arguments,
    reason = "snapshot painting adds the optional mode to existing inputs"
)]
fn paint_a_console_view(
    painter: &Painter,
    view: &term_core::View,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
    discoverer: &mut crate::fonts::Discoverer,
    ink: CursorInk,
    ligatures: bool,
) -> bool {
    paint_with_ligatures(
        painter,
        view,
        theme,
        metrics,
        area.left_top(),
        discoverer,
        ink,
        ligatures,
    )
}

/// Paints a console clipped to its rectangle, and returns what queueing the painting cost.
///
/// **The clip is not cosmetic.** The painter deliberately does not clip the glyphs —so that a
/// character wider than its cell is seen sticking out— and without clipping per console that
/// glyph on the right-hand edge would appear inside the neighboring console, painted by a
/// program that is not its own. It returns what the window needs to know about this pane after
/// painting it. See [`crate::PaneOutcome`].
pub(crate) fn paint_pane(
    ui: &eframe::egui::Ui,
    pane: &mut crate::Pane,
    rect: eframe::egui::Rect,
    theme: &Theme,
    metrics: &CellMetrics,
    discoverer: &mut crate::fonts::Discoverer,
    how: HowToPaint,
) -> crate::PaneOutcome {
    let HowToPaint {
        integrated,
        maximized,
        blink,
        headers,
        button,
        measure_links,
        ligatures,
    } = how;
    // **The panel is timed apart from the console's queueing, with two clocks that are added
    // together: one around reading and tiling, and another around painting it.** They are two
    // and not one because the console's painting falls right in between and has a span of its
    // own. For the frame's subtraction it would make no difference to add it all up —what
    // matters there is that it be declared—, but a span called «queueing the painting» that
    // also included reading a file off the disk is a number with the label changed, and in this
    // project that has already invalidated a whole measurement.
    //
    // **Rereading goes before tiling**, because how much width the console has left depends on
    // whether there is a panel or not: a console whose producer has just published for the
    // first time narrows in this very frame, not in the next one.
    let refreshed = pane.refresh_its_panel(ui.ctx());
    // **The clock starts AFTER the refresh and what the refresh cost is added back below**, rather
    // than spanning it and subtracting the channel's share out. The boundary of the «panel» span is
    // unchanged — it is still the read plus the tiling plus the border — but the control channel is
    // now never inside any clock this span is built from, so there is no subtraction for a later
    // caller to forget. It was written the other way first, and mutating it proved nothing failed.
    let before_the_panel = std::time::Instant::now();
    // A single clock for tiling and for painting: if the panel were to age between the two
    // questions, a strip would be reserved for a warning that never gets painted.
    let now = std::time::SystemTime::now();
    // **The strip is cut here and the panel is tiled over what is left**, so the console's
    // rectangle comes out of one split and not out of two agreeing by accident. See
    // [`crate::Pane::chrome_within`].
    let chrome = pane.chrome_within(rect, metrics, now, headers);
    let mut panel_split = chrome.split();

    // **The panel's border is attended to here, before measuring the grid, and the tiling is
    // redone in this very pass.** With the gesture attended to afterwards, the frame would
    // paint the panel in the old place and —worse— would announce the old grid to its program,
    // which is the one the pumping sends it on the next frame: a double readjustment, and a
    // visible one in any full-screen program.
    //
    // **And the tree's tiling does not have to be redone.** This gesture tiles *inside* one
    // pane, not between panes: the others' rectangles never find out. That is what tells it
    // apart from dragging a divider of the tree or from dropping one console onto another,
    // which do change it.
    //
    // The timed span encloses it for the same reason it encloses reading the file and sharing
    // out the width: it is the panel's work, and what is not declared is taken by the frame's
    // subtraction — which is the interface framework cost, the figure D3 has to decide on. **It
    // is called without a panel too, and that is what makes unfolding possible**: folded there
    // is no panel rectangle, but there is a grip flush against the edge to open it again by.
    // **The width is compared before and after, and what the gesture returns is not believed.**
    // The gesture returns «I have changed something», and folding and unfolding also change
    // something — without touching the width. Since the number this datum feeds is published in
    // the report under the «effects» label and the annex reads it as tilings and layouts,
    // counting a fold in there would be publishing work that did not happen.
    let width_before = pane.panel_width;
    if pane.drag_its_panel_divider(ui, chrome.below(), panel_split.panel, metrics, now) {
        panel_split = pane.tile_with_its_panel(chrome.below(), metrics, now);
    }
    let width_changed = pane.panel_width != width_before;
    let cost_of_reading = refreshed.reading + before_the_panel.elapsed();

    pane.measured = metrics.cells_in(panel_split.console.size());

    let before = std::time::Instant::now();
    // **The engine's bit is asked for here and not in the window's walk**, because reading it costs
    // one call and reading it through a `View` would cost a snapshot of the whole grid. It says what
    // the program wants, never what gets drawn: `paint_cursor` answers that.
    let mut view = pane.screen.view();
    let marking = pane.search.mark(&pane.screen, &mut view);
    let blinking = paint_a_console_view(
        &ui.painter_at(panel_split.console),
        &view,
        theme,
        metrics,
        panel_split.console,
        discoverer,
        blink.ink(pane.screen.cursor_blinks()),
        ligatures,
    );
    notices::paint_them_over_it(
        &ui.painter_at(panel_split.console),
        pane,
        theme,
        metrics,
        panel_split.console,
        blink.keys,
    );
    if let Some(search) = pane.search.open.as_ref() {
        let painter = ui.painter_at(panel_split.console);
        crate::search::paint_the_row(&painter, search, &view, theme, metrics, panel_split.console);
    }
    let painted = crate::links::PaintedConsole {
        view,
        area: panel_split.console,
        frame: ui.ctx().cumulative_frame_nr(),
    };
    painted.disclose_hovered_link(ui, metrics);
    // **Inside the console's span on purpose**: only batch W walks this, and outside every span it
    // would be published as the framework's cost (**D13**).
    let console_links = measure_links.then(|| painted.a_point_per_source(metrics));
    pane.painted_console = Some(painted);
    // **Rasterizing a color glyph runs INSIDE the painting, so it is subtracted from the
    // painting.** It is the only expensive thing this project does in there: looking up and
    // registering a face are done outside because they rebuild the atlas, but uploading a
    // texture invalidates nothing. Without this subtraction, «queueing the painting» would also
    // measure what decoding a PNG and scaling it down cost.
    let in_the_console = discoverer.color_cost();
    let queued = before
        .elapsed()
        .saturating_sub(in_the_console.spent)
        .saturating_sub(marking.unwrap_or_default());

    let before_painting_it = std::time::Instant::now();
    let mut buttons = std::time::Duration::ZERO;
    // **A pane whose panel is not painted says so, and it is not an afterthought.** The panel is
    // folded away, or the pane has no directory: either way nothing of it is on screen, and without
    // this the previous frame's buttons stay reachable — a click on the console, where one used to
    // be, would run its command with nothing to read.
    if panel_split.panel.is_none() || pane.panel.is_none() {
        pane.panel_state.nothing_was_drawn();
    }
    if let (Some(panel_rect), Some(dir)) = (panel_split.panel, pane.panel.as_ref()) {
        buttons = crate::panel::paint(
            &ui.painter_at(panel_rect),
            &mut pane.panel_state,
            dir.id(),
            theme,
            metrics,
            now,
            discoverer,
        );
    }
    let links = console_links
        .map(|console| {
            crate::links::ForTheBench::of(
                console,
                &pane.panel_state,
                ui.ctx().cumulative_frame_nr(),
            )
        })
        .unwrap_or_default();
    // **Subtracted OUT of the panel's span and not folded into it**, the same shape the colour
    // glyphs above already have. That span means six named things and its docblock enumerates them
    // so that nothing creeps in; letting a seventh in would be a published series quietly changing
    // meaning, which this file records having paid for twice.
    //
    // **The panel rasterizes color glyphs too, and they leave by a read of their own.** Reading the
    // cost empties it, so the one above only ever held the console's: moving that single read down
    // here would subtract the panel's rasterizing from the console, which never did it, and leave it
    // inside this span as well — counted twice.
    let in_the_panel = discoverer.color_cost();
    let panel_cost = cost_of_reading
        + before_painting_it
            .elapsed()
            .saturating_sub(buttons)
            .saturating_sub(in_the_panel.spent);
    let color = crate::fonts::ColorCost {
        spent: in_the_console.spent + in_the_panel.spent,
        rasterized: in_the_console.rasterized + in_the_panel.rasterized,
        failed: in_the_console.failed + in_the_panel.failed,
    };

    // **The strip is painted last of the pane's own painting and timed on its own** (**D13**): the
    // frame's cost is split by subtraction, so a band painted inside somebody else's span would be
    // published as that span's, and one painted inside no span at all as the framework's. It goes
    // after the console because that is the order the notices already follow — chrome the
    // application owns goes over what the program wrote, never under it.
    //
    // **And it is handed back to the window rather than put into the round below**, because the
    // round is only closed on a frame this console produced output in: see
    // [`crate::PaneOutcome::strip_cost`].
    let search = pane.search.cost();
    let strip_cost = chrome.header().map(|header| {
        crate::header::integrated::paint_strip(
            ui,
            pane,
            metrics,
            theme,
            crate::header::integrated::Strip {
                rect,
                header,
                headers,
                button,
                integrated,
                maximized,
                now,
            },
        )
    });
    // **The border is only reported if there is a grip behind it.** With the panel clipped
    // below the grip the gesture registers no control at all —so as not to steal the point from
    // the tree's divider—, and reporting the border anyway made the injected gesture press
    // right on the edge of that divider: the batch spent its life dragging the consoles' tiling
    // **under the label of the panel's gesture**. The script invalidated it for having no
    // effects, but blaming the window.
    let panel_divider = panel_split
        .panel
        .filter(|panel_rect| panel_rect.width() > crate::PANEL_GRIP)
        .map(|panel_rect| panel_rect.left());

    // The closing of the round goes here, in what paints one console, and not in the caller's walk
    // over the panes, because **the queueing always has to be declared**, also when no output
    // reached this console: it was painted just the same, and its cost has to come out of the
    // window's «rest of the frame».
    if let Some((waiting_since, collected_at, interpreted)) = pane.pending_measurement.take() {
        pane.timings.record_round(
            crate::measure::Spans::of_a_round(
                waiting_since,
                collected_at,
                interpreted,
                queued,
                panel_cost,
            )
            .and_its_buttons(buttons)
            .and_its_control(refreshed.applying),
        );
    }

    crate::PaneOutcome {
        panel_divider,
        width_changed,
        buttons_painted: pane.panel_state.buttons_drawn().count(),
        first_button: pane.panel_state.buttons_drawn().next(),
        links,
        color,
        blinking,
        strip_cost,
        search,
    }
}
