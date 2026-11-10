//! How much it costs to repaint N consoles at once, measured.
//!
//! It is the bench that answers the question that decides the UI framework: **egui is immediate mode
//! and repaints the whole screen**, so the doubt is not whether it holds up one console, but what
//! happens when there is more than one producing at once. The roadmap phrases that symptom with two
//! panes, and this milestone has a single one — hence a bench that simulates them.
//!
//! ```text
//! cargo run --release --example sweep -p app -- "work laptop"
//! ```
//!
//! The argument is the batch's label, optional, and follows the same criterion as `host-load`:
//! **whoever measures provides the machine**, because it is the only one who knows it without inventing
//! dependencies. In debug the number comes out several times worse; the one that counts is the
//! `--release` one.
//!
//! **What it measures, and why they are three numbers and not one.**
//!
//! - **Reading** the grid: reserving and filling all its cells. Any framework will carry it, so it
//!   does not judge this one — but if it were added to the rest, it would mask it.
//! - **Enqueuing**: what the call to the painter costs. And it is called enqueuing, not painting,
//!   because that is literally what it does: `Painter::rect_filled` and `Painter::text` **stack shapes
//!   in a list** and return. Turning them into triangles happens afterwards, outside the UI pass,
//!   where a stopwatch placed here does not reach.
//! - **Frame CPU**: what the whole frame really costs — the UI pass **plus the tessellation and the
//!   drawing**, without the vertical-sync wait. `eframe` measures it, which is the one that can: by the
//!   time the tessellation happens, this function has already finished.
//!
//! **The third is the deciding one, and that is why all three are there.** A bench that timed only the
//! call to the painter would publish an optimistic number with every appearance of being the good one:
//! it would count the stacking of shapes and not the work of converting them. Frames per second do not
//! serve alone either: they are capped by the vertical sync, so a "119 fps" only proves the frame fits
//! in its slot, not how much margin is left over.
//!
//! **Three things make the number the worst case, on purpose**:
//!
//! 1. **The screen is full.** The painter skips the spaces and the cells whose background is the
//!    window's, which on a real terminal screen are the majority. Measuring with a realistic screen
//!    would measure above all what the painter does **not** do.
//! 2. **It repaints continuously**, requesting another frame on each one, at whatever rate the screen
//!    gives —which on a 120 Hz monitor is twice as demanding as on a 60 Hz one, and that is why the
//!    frames per second are published alongside the number—. The product does exactly the opposite
//!    —it repaints only when the program writes—, so this is not the normal use: it is the ceiling,
//!    which is what has to be known before deciding.
//! 3. **It calls the real painter**, the same one the application uses. If it duplicated it, its
//!    number would stop being the product's.
//!
//! And one thing that makes the number honest: **if the consoles do not fit whole in the window, the
//! bench publishes nothing**. What is not seen is not tessellated, and a number taken from there would
//! say that everything is fine precisely because nothing was drawn.

use app::painter::{CellMetrics, paint};
use app::theme::Theme;
use eframe::egui;
use std::time::{Duration, Instant};
use term_core::{Screen, Size};

/// The size of each simulated console. **The same for all values of N**: it is what lets the result be
/// read as "cost per console" and see whether two cost twice as much as one, or more.
const GRID: Size = Size { rows: 24, cols: 80 };

/// The values of N that are swept.
///
/// Two, and no more, on purpose: the symptom the roadmap describes talks about two panes, and this
/// milestone does not bring forward work from the following ones. Four and eight will arrive when there
/// is a phase that needs them.
const PLAN: [usize; 2] = [1, 2];

/// Frames thrown away before starting to count in each phase.
///
/// The first pass builds the font atlas and reserves the textures, and comes out several times more
/// expensive than the following ones: counting it would raise the average of one phase without any of
/// that happening again in the others.
const WARMUP: usize = 60;

/// Frames measured in each phase.
///
/// Frames and not seconds are counted because **the screen sets the rate**, not the bench: on a 120 Hz
/// monitor this is five seconds per value of N and on a 60 Hz one, ten. Counting by time would measure
/// a different number of frames on each machine and would make the batches incomparable.
const MEASURED: usize = 600;

/// The letter's body size.
///
/// The same one the application uses, and for now written here just like in the test screen: the
/// product has it in a private constant. If the two stopped matching, this number would stop being the
/// product's without anything warning — unifying it is work for when the loop is inside.
const BODY: f32 = 14.0;

fn main() -> eframe::Result {
    let label = std::env::args().nth(1);

    let mut screens = Vec::new();
    for _ in 0..PLAN.iter().copied().max().unwrap_or(1) {
        let mut screen = Screen::new(GRID);
        screen.advance(script(GRID).as_bytes());
        screens.push(screen);
    }

    eframe::run_native(
        "Ivy — repaint bench",
        eframe::NativeOptions {
            // Maximized: the bench needs all the room the screen gives, and how much that is cannot be
            // known from here. With a size chosen by eye, on a screen with a high scale factor the two
            // consoles do not fit and the batch is lost to a badly set constant.
            viewport: egui::ViewportBuilder::default().with_maximized(true),
            ..Default::default()
        },
        Box::new(move |_cc| {
            Ok(Box::new(Sweep {
                screens,
                theme: Theme::default(),
                label,
                phase: 0,
                seen: 0,
                reads: Vec::new(),
                enqueues: Vec::new(),
                frames: Vec::new(),
                intervals: Vec::new(),
                previous: None,
                results: Vec::new(),
                doesnt_fit: None,
            }))
        }),
    )
}

/// What is written in each simulated console.
///
/// **Deterministic**: what comes out on row `i` depends only on `i`, so that two batches from two days
/// or two different machines are comparable. And **full of color on purpose**: each cell carries its
/// own background, which is what forces the painter to draw a rectangle per cell in addition to the
/// glyph. A real terminal screen is half empty; this one is the ceiling.
fn script(grid: Size) -> String {
    let mut text = String::new();
    for row in 0..grid.rows {
        // Six backgrounds and six letter colors, rotating at different rates so the letter never falls
        // on its own color: with the two at the same rate, one in every six rows came out blue on blue.
        // It does not change the number —the glyph is drawn the same, whether it is read or not— but an
        // illegible band on the screen invites the belief that those rows were not painted.
        let background = 41 + (row % 6);
        let foreground = 31 + ((row + 1) % 6);
        text.push_str(&format!("\x1b[{foreground};{background}m"));
        for col in 0..grid.cols {
            // Letters, never spaces: the painter skips the spaces, and a screen of spaces would measure
            // above all the empty loop.
            text.push((b'a' + ((row + col) % 26) as u8) as char);
        }
        text.push_str("\x1b[0m");
        // No line break after the last row: it would scroll the screen and the first would stop being
        // seen, so the bench would measure 23 rows believing it measured 24.
        if row + 1 < grid.rows {
            text.push_str("\r\n");
        }
    }
    text
}

/// How the consoles are laid out so they fit, or nothing if they do not fit any way.
///
/// Returns the offset from one console to the next: in a row or in a column. **It is not a convenience,
/// it is what makes the bench runnable**: two eighty-column consoles need more than 1,300 points of
/// width, and a screen with the desktop scale factor turned up does not reach it — but it does have
/// height to spare. And it cannot be chosen before opening the window, because until then neither the
/// letter's size nor what the screen gives is known.
fn layout(available: egui::Vec2, console: egui::Vec2, count: usize) -> Option<egui::Vec2> {
    let count = count as f32;
    if available.x >= console.x * count && available.y >= console.y {
        Some(egui::vec2(console.x, 0.0))
    } else if available.y >= console.y * count && available.x >= console.x {
        Some(egui::vec2(0.0, console.y))
    } else {
        None
    }
}

/// What is measured in one phase of the sweep.
struct PhaseResult {
    consoles: usize,
    read: Summary,
    enqueue: Summary,
    /// The CPU cost of the whole frame, tessellation and drawing included. The deciding number.
    frame: Summary,
    frames_per_second: f64,
}

/// The mean and the worst of a series of times. Both, because they say different things: the mean says
/// how much it costs, and the worst is the one that is seen as a jolt.
struct Summary {
    mean: Duration,
    worst: Duration,
}

impl Summary {
    fn of(times: &[Duration]) -> Self {
        let total: Duration = times.iter().sum();
        Self {
            mean: total / times.len().max(1) as u32,
            worst: times.iter().copied().max().unwrap_or_default(),
        }
    }
}

struct Sweep {
    screens: Vec<Screen>,
    theme: Theme,
    label: Option<String>,
    phase: usize,
    seen: usize,
    reads: Vec<Duration>,
    enqueues: Vec<Duration>,
    /// What `eframe` says the **previous** frame cost of CPU. It goes one frame behind the other two,
    /// which in continuous regime makes no difference and at the phase change is absorbed by the warmup.
    frames: Vec<Duration>,
    intervals: Vec<Duration>,
    previous: Option<Instant>,
    results: Vec<PhaseResult>,
    /// What was needed and what there was, if the consoles ever did not fit in the window. A single
    /// time is enough to invalidate the batch: what is not seen is not drawn, and the number would come
    /// out fine for not having worked.
    doesnt_fit: Option<(egui::Vec2, egui::Vec2)>,
}

impl Sweep {
    /// Closes the current phase and moves to the next. Returns whether the sweep has finished.
    fn close_phase(&mut self) -> bool {
        let interval = Summary::of(&self.intervals).mean.as_secs_f64();
        self.results.push(PhaseResult {
            consoles: PLAN[self.phase],
            read: Summary::of(&self.reads),
            enqueue: Summary::of(&self.enqueues),
            frame: Summary::of(&self.frames),
            frames_per_second: if interval > 0.0 { 1.0 / interval } else { 0.0 },
        });

        self.reads.clear();
        self.enqueues.clear();
        self.frames.clear();
        self.intervals.clear();
        self.seen = 0;
        self.previous = None;
        self.phase += 1;
        self.phase == PLAN.len()
    }

    fn publish(&self) {
        if let Some((needed, had)) = self.doesnt_fit {
            println!(
                "NO NUMBER: the consoles did not fit whole in the window.\n\
                 {:.0}×{:.0} points were needed and there were {:.0}×{:.0}.\n\
                 What is left outside is not drawn, so the measurement would say that everything is fine\n\
                 precisely for not having painted. With a bigger screen the number comes out.",
                needed.x, needed.y, had.x, had.y
            );
            return;
        }

        let profile = if cfg!(debug_assertions) {
            "debug — the number that counts is the --release one"
        } else {
            "release"
        };
        println!("\n── repaint bench ──");
        println!("grid:         {}×{} per console", GRID.rows, GRID.cols);
        println!("letter:       {BODY}-point monospaced");
        println!("renderer:     glow");
        println!("profile:      {profile}");
        println!("measurement:  {MEASURED} frames per value of N, {WARMUP} of warmup");
        if let Some(label) = &self.label {
            println!("machine:      {label}");
        }
        println!();

        for result in &self.results {
            println!(
                "N = {}  ·  read {:>9.3?}  ·  enqueue {:>9.3?}  ·  frame CPU {:>9.3?} (worst {:>9.3?})  ·  {:.1} fps",
                result.consoles,
                result.read.mean,
                result.enqueue.mean,
                result.frame.mean,
                result.frame.worst,
                result.frames_per_second,
            );
        }

        // The milestone's question is not how much one console costs, but **how it grows**: if two cost
        // twice, immediate mode scales and the symptom does not appear; if they cost quite a bit more,
        // that is exactly the symptom the roadmap describes. It is compared against the CPU cost of the
        // whole frame and not against the enqueuing: the enqueuing does not include the tessellation,
        // which is the expensive half of the work and the one that grows most with the number of cells.
        if let ([one], [two]) = (&self.results[..1], &self.results[1..]) {
            let reference = one.frame.mean.as_secs_f64().max(f64::MIN_POSITIVE);
            let factor = two.frame.mean.as_secs_f64() / reference;
            println!("\ntwo consoles cost {factor:.2}× what one costs");

            // How much of the gap between frames is being spent. It is what says whether margin is left
            // over or it goes right to the edge, which the frames per second cannot say: they are capped
            // by the vertical sync, so they only prove the frame fits.
            let available = 1.0 / two.frames_per_second.max(f64::MIN_POSITIVE);
            println!(
                "with two, the frame spends {:.0} % of its slot ({:.2} ms of {:.2} ms)",
                100.0 * two.frame.mean.as_secs_f64() / available,
                two.frame.mean.as_secs_f64() * 1e3,
                available * 1e3,
            );
        }
    }
}

impl eframe::App for Sweep {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // The sweep has finished and all that remains is for the window to find out. The close is not
        // immediate —the desktop attends to it when it can— so frames still arrive here, and without
        // this exit the next one would look for a phase that no longer exists.
        if self.phase == PLAN.len() {
            return;
        }

        // Continuous regime: the next frame is requested without waiting for anything to change. It is
        // the opposite of what the product does, and that is the point — here the ceiling is measured.
        ui.ctx().request_repaint();

        let now = Instant::now();
        let interval = self.previous.replace(now).map(|prev| now - prev);

        let metrics = CellMetrics::measure(ui.ctx(), egui::FontId::monospace(BODY));
        let consoles = PLAN[self.phase];
        let available = ui.max_rect();
        let console = egui::vec2(
            metrics.width() * GRID.cols as f32,
            metrics.height() * GRID.rows as f32,
        );

        // The layout is decided **with the largest N of the plan and not with this phase's**: if the
        // two consoles are going to end up stacked, the one in the first phase has to be painted in the
        // same place and the same way, or two different things would be compared.
        let largest = PLAN.iter().copied().max().unwrap_or(1);
        let step = layout(available.size(), console, largest);

        // The batch is given up for lost only in the frames that count: during the warmup the window is
        // still maximizing, and giving it up for small there would be throwing it away over something
        // that is about to fix itself.
        if step.is_none() && self.seen >= WARMUP {
            // The **cheaper of the two** layouts is reported, not the product of the two axes: stacking
            // needs the width of one console and the height of all, and saying that both things were
            // needed at once would ask for a screen that is not needed.
            self.doesnt_fit = Some((
                egui::vec2(console.x, console.y * largest as f32),
                available.size(),
            ));
        }
        let step = step.unwrap_or_default();

        let mut read = Duration::ZERO;
        let mut enqueue = Duration::ZERO;
        for (index, screen) in self.screens[..consoles].iter().enumerate() {
            let before = Instant::now();
            let view = screen.view();
            let took = before.elapsed();

            let origin = available.left_top() + step * index as f32;
            let before = Instant::now();
            // **A throwaway discoverer, and on purpose.** This bench measures painting with text that
            // the factory fonts already cover, so there is never anything to discover; and since it does
            // not call `resolve`, a persistent one would only accumulate notes nobody looks at.
            paint(
                ui.painter(),
                &view,
                &self.theme,
                &metrics,
                origin,
                &mut Default::default(),
                app::painter::CursorInk::Filled,
            );

            read += took;
            enqueue += before.elapsed();
        }

        self.seen += 1;
        if self.seen <= WARMUP {
            return;
        }

        self.reads.push(read);
        self.enqueues.push(enqueue);
        // What the previous frame cost of CPU, with its tessellation and its drawing inside. It is the
        // only one of the three that this function cannot time on its own: by the time the tessellation
        // happens, it has already finished.
        if let Some(usage) = frame.info().cpu_usage {
            self.frames.push(Duration::from_secs_f32(usage));
        }
        if let Some(interval) = interval {
            self.intervals.push(interval);
        }

        if self.reads.len() >= MEASURED && self.close_phase() {
            self.publish();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.theme.background().to_normalized_gamma_f32()
    }
}
