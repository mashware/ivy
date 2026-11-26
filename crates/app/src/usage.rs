use crate::{Window, theme::Theme};
use chrono::{Local, NaiveDate, TimeDelta, TimeZone};
use eframe::egui;
use std::{
    io::{self, Write},
    path::PathBuf,
    time::{Duration, Instant},
};
use usage_core::{Cost, DateRange, Day, Service, Snapshot, Status, Totals};

#[derive(Default)]
pub(crate) struct View {
    open: bool,
    blocked_frame: Option<u64>,
    awaiting_release: bool,
    focus: Option<egui::Id>,
    service: Option<Service>,
    snapshot: Option<Snapshot>,
    roots: Vec<PathBuf>,
    new_root: String,
    confirm_clear: bool,
    hide_result: bool,
    period: Period,
    requested_range: Option<DateRange>,
    timing: Timing,
    measured: Option<Measurement>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Period {
    Week,
    #[default]
    Month,
    Quarter,
    Year,
}

impl Period {
    const OPTIONS: [(Self, &'static str); 4] = [
        (Self::Week, "7 days"),
        (Self::Month, "30 days"),
        (Self::Quarter, "90 days"),
        (Self::Year, "365 days"),
    ];

    fn range(self) -> Option<DateRange> {
        let days = match self {
            Self::Week => 7,
            Self::Month => 30,
            Self::Quarter => 90,
            Self::Year => 365,
        };
        let end = Local::now().date_naive().succ_opt()?;
        let start = end.checked_sub_signed(TimeDelta::days(days))?;
        Some(DateRange {
            start_ms: local_boundary(start, &Local)?,
            end_ms: local_boundary(end, &Local)?,
        })
    }
}

fn local_boundary(day: NaiveDate, zone: &impl TimeZone) -> Option<i64> {
    let midnight = day.and_hms_opt(0, 0, 0)?;
    // Some zones move the clock at midnight or skip a calendar day.
    (0..=24 * 60).find_map(|minutes| {
        let local = midnight.checked_add_signed(TimeDelta::minutes(minutes))?;
        zone.from_local_datetime(&local)
            .earliest()
            .map(|instant| instant.timestamp_millis())
    })
}

impl View {
    pub(crate) fn blocks(&self, ctx: &egui::Context) -> bool {
        self.open || self.awaiting_release || self.blocked_frame == Some(ctx.cumulative_frame_nr())
    }

    fn close(&mut self, ctx: &egui::Context) {
        self.open = false;
        self.confirm_clear = false;
        ctx.memory_mut(|memory| {
            memory.stop_text_input();
            if let Some(id) = self.focus.take() {
                memory.request_focus(id);
            }
        });
        ctx.request_repaint();
    }

    fn request(&mut self) {
        if let (Some(service), Some(range)) = (&self.service, self.period.range()) {
            self.requested_range = Some(range);
            service.request(range);
        }
    }

    fn snapshot_matches_range(&self, snapshot: &Snapshot) -> bool {
        self.requested_range.is_some() && self.requested_range == snapshot.range
    }

    fn install_snapshot(&mut self) {
        if let Some(s) = self.service.as_ref().and_then(Service::poll) {
            self.hide_result = self.hide_result && !self.snapshot_matches_range(&s);
            self.snapshot = Some(s);
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let Some(service) = &self.service else {
            ui.label("The local cache directory is unavailable.");
            return;
        };
        let waiting = self.snapshot.is_none();
        let enabled = self.snapshot.as_ref().is_some_and(|s| s.enabled);
        let supported = self
            .snapshot
            .as_ref()
            .is_none_or(|s| s.status != Status::Unsupported);
        let clearing = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.status == Status::WaitingToClear);

        ui.strong("Claude Code · Local, unassigned history");
        if let Some(s) = &self.snapshot {
            ui.label(status_label(s.status));
            if let Some(success) = s
                .last_success_at_ms
                .and_then(chrono::DateTime::from_timestamp_millis)
            {
                ui.label(format!(
                    "Last successful collection: {}",
                    success.with_timezone(&Local).format("%Y-%m-%d %H:%M %Z")
                ));
            }
            let scope = if enabled {
                "Selected"
            } else {
                "Previously selected"
            };
            match s.roots.as_slice() {
                [] => {}
                [root] => {
                    ui.small(format!("{scope} folder: {}", root.display()));
                }
                roots => {
                    ui.collapsing(format!("{scope} folders ({})", roots.len()), |ui| {
                        for root in roots {
                            ui.small(root.display().to_string());
                        }
                    });
                }
            }
            if s.has_history && !enabled {
                ui.small("Previously imported history is available.");
            }
        } else {
            ui.label("Loading local usage settings…");
        }
        if !waiting && supported && !enabled && !clearing {
            ui.label("Choose the local folders containing Claude Code history.");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_root)
                        .hint_text("Absolute folder path")
                        .desired_width(300.0),
                );
                let path = PathBuf::from(self.new_root.trim());
                if ui
                    .add_enabled(
                        path.is_absolute() && !self.roots.contains(&path) && self.roots.len() < 64,
                        egui::Button::new("Add folder"),
                    )
                    .clicked()
                {
                    self.roots.push(path);
                    self.new_root.clear();
                }
            });
            if let Some(default_root) = std::env::home_dir().map(|p| p.join(".claude/projects"))
                && ui
                    .add_enabled(
                        !self.roots.contains(&default_root) && self.roots.len() < 64,
                        egui::Button::new("Add Claude’s default folder"),
                    )
                    .clicked()
            {
                self.roots.push(default_root);
            }
            let mut remove = None;
            for (index, root) in self.roots.iter().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("Remove").clicked() {
                            remove = Some(index);
                        }
                        ui.label(root.display().to_string());
                    });
                });
            }
            if let Some(index) = remove {
                self.roots.remove(index);
            }
            if ui
                .add_enabled(
                    !self.roots.is_empty(),
                    egui::Button::new("Enable collection"),
                )
                .clicked()
            {
                service.enable(std::mem::take(&mut self.roots));
            }
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(enabled && !clearing, egui::Button::new("Stop collection"))
                .clicked()
            {
                service.stop();
            }
            let has_history = self.snapshot.as_ref().is_some_and(|s| {
                s.has_history || s.enabled || !s.roots.is_empty() || s.status == Status::Error
            });
            if ui
                .add_enabled(
                    supported && has_history && !clearing,
                    egui::Button::new("Clear history…"),
                )
                .clicked()
            {
                self.confirm_clear = true;
            }
        });
        if self.confirm_clear {
            ui.label("Clear Ivy's stored accounting? If the original logs are gone, this history cannot be rebuilt.");
            ui.horizontal(|ui| {
                if ui.button("Clear and stop collection").clicked() {
                    service.clear();
                    self.hide_result = true;
                    self.confirm_clear = false;
                }
                if ui.button("Cancel").clicked() {
                    self.confirm_clear = false;
                }
            });
        }
        ui.separator();
        let old_period = self.period;
        ui.horizontal_wrapped(|ui| {
            for (period, label) in Period::OPTIONS {
                ui.selectable_value(&mut self.period, period, label);
            }
        });
        if old_period != self.period {
            self.hide_result = true;
            self.request();
            return;
        }
        if !self.hide_result
            && !clearing
            && let Some(snapshot) = &self.snapshot
            && self.snapshot_matches_range(snapshot)
            && (snapshot.has_history || snapshot.last_success_at_ms.is_some())
        {
            paint_summary(ui, snapshot);
        }
    }
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Disabled => "Collection is off.",
        Status::Importing => "Collecting local history…",
        Status::Ready => "Collection complete for the selected sources.",
        Status::Empty => "No local usage found in the selected scope.",
        Status::Partial => "Partial coverage",
        Status::Limit => "Import limit reached. Previously imported history is retained.",
        Status::WaitingToClear => "Waiting to clear history.",
        Status::Unsupported => "Local history is unavailable on this platform.",
        Status::Error => "Could not read local usage.",
        Status::Stale => "Stale results from the last successful collection.",
    }
}

fn paint_summary(ui: &mut egui::Ui, snapshot: &Snapshot) {
    let totals = &snapshot.totals;
    ui.heading(if totals.tokens_known {
        format!("{} tokens", token_label(totals))
    } else {
        "Tokens unavailable".into()
    });
    ui.horizontal_wrapped(|ui| {
        for (name, count, known) in [
            ("Input", totals.uncached_input, totals.uncached_input_known),
            ("Cache read", totals.cache_read, totals.cache_read_known),
            ("Cache write", totals.cache_write, totals.cache_write_known),
            ("Output", totals.output, totals.output_known),
        ] {
            let value = if known {
                count.to_string()
            } else {
                "unavailable".into()
            };
            ui.label(format!("{name} {value}"));
        }
    });
    ui.label(price_label(totals.estimate.as_ref()));
    if !snapshot.pricing_version.is_empty() {
        ui.small(format!(
            "Bundled API list rates · {}",
            snapshot.pricing_version
        ))
        .on_hover_text(&snapshot.pricing_source);
    }
    ui.separator();
    ui.strong("Daily tokens");
    paint_trend(ui, snapshot.range, &snapshot.daily);
    ui.separator();
    ui.strong("By model");
    egui::Grid::new("usage-models")
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Model");
            ui.strong("Tokens");
            ui.strong("Estimated cost");
            ui.end_row();
            for model in &snapshot.models {
                ui.label(&model.label);
                ui.label(token_label(&model.totals));
                ui.label(price_label(model.totals.estimate.as_ref()));
                ui.end_row();
            }
        });
}

fn token_label(totals: &Totals) -> String {
    if !totals.tokens_known {
        "Unavailable".into()
    } else if totals.partial {
        format!("{} known", totals.tokens)
    } else {
        totals.tokens.to_string()
    }
}

fn price_label(cost: Option<&Cost>) -> String {
    let Some(cost) = cost else {
        return "Estimate unavailable".into();
    };
    let basis = if cost.partial {
        "Partial estimate"
    } else {
        "Estimated API cost"
    };
    if cost.usd > 0.0 && cost.usd < 0.000001 {
        format!("{basis} · < USD 0.000001")
    } else {
        format!("{basis} · USD {:.6}", cost.usd)
    }
}

fn paint_trend(ui: &mut egui::Ui, range: Option<DateRange>, daily: &[Day]) {
    let Some(range) = range.filter(|_| !daily.is_empty()) else {
        ui.label("No dated usage in this range.");
        return;
    };
    let local_date = |ms| {
        chrono::DateTime::from_timestamp_millis(ms)
            .map(|instant| instant.with_timezone(&Local).date_naive())
    };
    let (Some(first), Some(last)) = (local_date(range.start_ms), local_date(range.end_ms - 1))
    else {
        return;
    };
    let count = (last - first).num_days().clamp(0, 365) as f32 + 1.0;
    let largest = daily.iter().map(|day| day.totals.tokens).max().unwrap_or(0);
    let width = ui.available_width();
    let (area, _) = ui.allocate_exact_size(egui::vec2(width, 86.0), egui::Sense::hover());
    let step = width / count;
    for day in daily {
        let column = (day.date - first).num_days() as f32;
        let left = area.left() + column * step;
        let slot = egui::Rect::from_min_max(
            egui::pos2(left, area.top()),
            egui::pos2(left + step, area.bottom()),
        );
        let ratio = if largest == 0 {
            0.0
        } else {
            day.totals.tokens as f64 / largest as f64
        };
        let height = if day.totals.tokens_known {
            (ratio as f32 * area.height()).max(1.0)
        } else {
            0.0
        };
        let bar = egui::Rect::from_min_max(
            egui::pos2(left + step * 0.1, area.bottom() - height),
            egui::pos2(left + step * 0.9, area.bottom()),
        );
        let color = if day.totals.partial {
            ui.visuals().weak_text_color()
        } else {
            ui.visuals().selection.bg_fill
        };
        ui.painter().rect_filled(bar, 1.0, color);
        let value = if day.totals.tokens_known {
            format!("{} tokens", token_label(&day.totals))
        } else {
            "Tokens unavailable".into()
        };
        ui.interact(slot, ui.id().with(day.date), egui::Sense::hover())
            .on_hover_text(format!("{} · {value}", day.date));
    }
    ui.horizontal(|ui| {
        ui.small(first.to_string());
        if last != first {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(last.to_string());
            });
        }
    });
}

#[derive(Clone, Copy)]
enum Measurement {
    Open,
    Closed,
}

pub(crate) fn configure_measurement(window: &mut Window, ctx: &egui::Context, measuring: bool) {
    if !measuring {
        return;
    }
    let mode = match std::env::var("MEASURE_USAGE").ok().as_deref() {
        Some("open") => Measurement::Open,
        Some("closed") => Measurement::Closed,
        _ => return,
    };
    let Some(root) = std::env::var_os("MEASURE_USAGE_ROOT").map(PathBuf::from) else {
        return;
    };
    let Some(cache) = std::env::var_os("MEASURE_USAGE_CACHE").map(PathBuf::from) else {
        return;
    };
    let ctx = ctx.clone();
    let service = Service::new(cache, move || ctx.request_repaint());
    service.enable(vec![root]);
    window.usage.service = Some(service);
    window.usage.measured = Some(mode);
    window.usage.open = matches!(mode, Measurement::Open);
    window.usage.request();
}

pub(crate) fn open(window: &mut Window, ctx: &egui::Context) {
    let view = &mut window.usage;
    let started = Instant::now();
    if view.service.is_none() {
        view.service = cache_directory().map(|cache| {
            let ctx = ctx.clone();
            Service::new(cache, move || ctx.request_repaint())
        });
    }
    view.focus = ctx.memory(|memory| memory.focused());
    view.open = true;
    view.blocked_frame = Some(ctx.cumulative_frame_nr());
    view.awaiting_release = ctx.input(|input| input.pointer.any_down());
    view.request();
    window.timed_separately += view.timing.record(0, started.elapsed());
    ctx.request_repaint();
}

fn cache_directory() -> Option<PathBuf> {
    let path = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::home_dir().map(|home| home.join("Library/Caches"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|cache| cache.is_absolute())
            .or_else(|| std::env::home_dir().map(|home| home.join(".cache")))
    }?;
    Some(path.join("ivy-usage"))
}

pub(crate) fn begin_pass(window: &mut Window, ui: &mut egui::Ui) {
    let view = &mut window.usage;
    if view.service.is_some() {
        let started = Instant::now();
        view.install_snapshot();
        window.timed_separately += view.timing.record(0, started.elapsed());
    }
    let ctx = ui.ctx().clone();
    if !view.blocks(&ctx) {
        return;
    }
    let started = Instant::now();
    if view.open {
        show(view, &ctx, &window.theme);
    }
    view.blocked_frame = Some(ctx.cumulative_frame_nr());
    view.awaiting_release = ctx.input(|input| input.focused && input.pointer.any_down());
    window
        .header_input
        .block_for_settings(ctx.cumulative_frame_nr());
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            matches!(
                event,
                egui::Event::ModifiersChanged(_)
                    | egui::Event::WindowFocused(_)
                    | egui::Event::PointerButton { pressed: false, .. }
            )
        });
    });
    ui.disable();
    window.timed_separately += view.timing.record(1, started.elapsed());
}

fn show(view: &mut View, ctx: &egui::Context, theme: &Theme) {
    let available = ctx.content_rect().size();
    let mut close = false;
    egui::Modal::new(egui::Id::new("ai-usage"))
        .frame(
            crate::ui::floating::dialog_frame(&crate::ui::style::roles(ctx, theme))
                .inner_margin(16),
        )
        .show(ctx, |ui| {
            crate::ui::floating::style(ui, theme);
            crate::settings::editor::view::settings_geometry(ui.style_mut());
            ui.set_width((available.x - 64.0).clamp(80.0, 720.0));
            close = crate::settings::editor::view::dialog::header(ui, "AI usage");
            egui::ScrollArea::vertical()
                .id_salt("usage-scroll")
                .max_height((available.y - 150.0).max(40.0))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    view.body(ui);
                });
        });
    if close || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        view.close(ctx);
    }
}

#[derive(Default)]
struct Timing {
    pending: [Option<Duration>; 2],
    series: [crate::measure::Series; 2],
    totals: [Duration; 2],
    frames: [usize; 2],
}

impl Timing {
    fn record(&mut self, index: usize, cost: Duration) -> Duration {
        *self.pending[index].get_or_insert_default() += cost;
        cost
    }
}

impl View {
    pub(crate) fn close_frame(&mut self) {
        let timing = &mut self.timing;
        for (index, pending) in timing.pending.iter_mut().enumerate() {
            if let Some(cost) = pending.take() {
                timing.series[index].record(cost);
                timing.totals[index] += cost;
                timing.frames[index] += 1;
            }
        }
    }

    pub(crate) fn publish(&self, to: &mut dyn Write) -> io::Result<()> {
        let mode = match self.measured {
            Some(Measurement::Open) => "open",
            Some(Measurement::Closed) => "closed",
            None => "none",
        };
        let tokens = self.snapshot.as_ref().map_or(0, |s| s.totals.tokens);
        writeln!(to, "usage measurement: {mode}, tokens={tokens}")?;
        for (index, name) in ["Usage snapshot", "Usage paint"].into_iter().enumerate() {
            let frames = self.timing.frames[index];
            let total = self.timing.totals[index];
            writeln!(to, "{name}: frames={frames}, total={total:?}")?;
            writeln!(to, "{name} series: {}", self.timing.series[index])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_costs_accumulate_across_passes_and_close_once() {
        let mut view = View::default();
        let returned = view.timing.record(0, Duration::from_micros(7))
            + view.timing.record(0, Duration::from_micros(13));
        assert_eq!(returned, Duration::from_micros(20));
        view.timing.record(1, Duration::ZERO);
        view.close_frame();
        view.close_frame();
        let mut report = Vec::new();
        view.publish(&mut report).unwrap();
        let report = String::from_utf8(report).unwrap();
        assert!(report.contains("Usage snapshot: frames=1, total=20µs"));
        assert!(report.contains("Usage paint: frames=1, total=0ns"));
    }

    #[test]
    fn missing_usage_stays_distinct_from_measured_zero() {
        let counts = Totals::default();
        assert_eq!(token_label(&counts), "Unavailable");
        let measured = Totals {
            tokens_known: true,
            ..counts
        };
        assert_eq!(token_label(&measured), "0");
        let partial = Totals {
            tokens: 17,
            partial: true,
            ..measured
        };
        assert_eq!(token_label(&partial), "17 known");
    }

    #[test]
    fn local_boundary_converts_midnight_and_keeps_exclusive_end() {
        let zone = chrono::FixedOffset::west_opt(5 * 60 * 60).unwrap();
        let first = NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
        let end = first.succ_opt().unwrap();
        let start_ms = local_boundary(first, &zone).unwrap();
        let end_ms = local_boundary(end, &zone).unwrap();
        assert_eq!(end_ms - start_ms, 86_400_000);
        assert_eq!(
            chrono::DateTime::from_timestamp_millis(start_ms)
                .unwrap()
                .to_rfc3339(),
            "2026-03-08T05:00:00+00:00"
        );
    }
}
