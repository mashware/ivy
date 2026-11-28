use crate::{Window, theme::Theme};
use chrono::{Local, NaiveDate, TimeDelta, TimeZone};
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use usage_core::{DateRange, Granularity, Provider, Service, Snapshot, Status};

mod allowances;
mod chip;
mod codex_controls;
mod paint;
mod report;
pub(crate) use chip::{paint as paint_chip, show_summary};
use paint::paint_summary;
pub(crate) use report::configure_measurement;

/// The title bar's slot for the allowance chip, in points; zero when there is no chip.
pub(crate) fn chip_slot(view: &View) -> f32 {
    view.chip.slot()
}

pub(crate) fn chip_rect(view: &View, slot: egui::Rect) -> egui::Rect {
    view.chip.hugged(slot)
}

/// The chip's summary while it is open, for the title bar's pointer claim.
pub(crate) fn chip_summary(view: &View) -> Option<egui::Rect> {
    view.chip.summary()
}

pub(crate) fn close_summary(view: &mut View) {
    view.chip.close_summary();
}

/// Declares what painting the chip and its summary cost, under the chip's own series; the caller
/// adds the returned cost to the window's set-aside work and takes it out of its own.
pub(crate) fn declare_chip(view: &mut View, cost: Duration) -> Duration {
    view.timing.record(report::CHIP, cost)
}
use report::{Measurement, Timing};

#[cfg(target_os = "linux")]
mod quota_broker;
#[cfg(all(test, windows))]
mod quota_broker {
    mod tests {
        crate::windows_unix_omission!(
            the_chip_follows_receipt_order_between_launches_in_the_same_second,
            "unix-only: Claude allowance reporting requires Linux socket peer and process-scope verification"
        );
    }
}
#[cfg(target_os = "linux")]
mod quota_setup;
#[cfg(target_os = "linux")]
mod quota_view;
#[cfg(target_os = "linux")]
pub(crate) use quota_view::{Lease as QuotaLease, Startup as QuotaStartup};

#[derive(Default)]
pub(crate) struct View {
    open: bool,
    blocked_frame: Option<u64>,
    awaiting_release: bool,
    focus: Option<egui::Id>,
    service: Option<Service>,
    snapshot: Option<Snapshot>,
    allowances: allowances::AllowanceView,
    roots: Vec<PathBuf>,
    new_root: String,
    codex: codex_controls::Controls,
    provider: Option<Provider>,
    model_filter: String,
    applied_model: Option<String>,
    granularity: Granularity,
    confirm_clear: bool,
    hide_result: bool,
    period: Period,
    requested_range: Option<DateRange>,
    timing: Timing,
    measured: Option<Measurement>,
    /// A measured run starts no background read: its process would be work nobody declared (D13).
    measuring: bool,
    chip: chip::Chip,
    #[cfg(target_os = "linux")]
    quota: quota_view::QuotaView,
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

fn snapshot_has_partial_codex(snapshot: Option<&Snapshot>) -> bool {
    snapshot.is_some_and(|snapshot| snapshot.codex_enabled && snapshot.status == Status::Partial)
}

fn request_comparison(view: &mut View) {
    if let (Some(service), Some(range)) = (&view.service, view.period.range()) {
        view.requested_range = Some(range);
        service.request_comparison(
            range,
            view.provider,
            view.applied_model.clone(),
            view.granularity,
        );
    }
}

impl View {
    #[cfg(test)]
    pub(crate) fn open_for_test(&mut self) {
        self.open = true;
    }

    pub(crate) fn blocks(&self, ctx: &egui::Context) -> bool {
        self.open || self.awaiting_release || self.blocked_frame == Some(ctx.cumulative_frame_nr())
    }

    fn close(&mut self, ctx: &egui::Context) {
        self.open = false;
        self.confirm_clear = false;
        #[cfg(target_os = "linux")]
        self.quota.close();
        ctx.memory_mut(|memory| {
            memory.stop_text_input();
            if let Some(id) = self.focus.take() {
                memory.request_focus(id);
            }
        });
        ctx.request_repaint();
    }

    fn snapshot_matches_range(&self, snapshot: &Snapshot) -> bool {
        self.requested_range.is_some()
            && self.requested_range == snapshot.range
            && self.provider == snapshot.provider
            && self.applied_model == snapshot.model_filter
            && self.granularity == snapshot.granularity
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
        let incompatible = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.status == Status::Incompatible);
        if !waiting && supported && !enabled && !clearing && !incompatible {
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
        let codex_enabled = self.codex.paint(
            ui,
            service,
            self.snapshot.as_ref(),
            !waiting && supported && !clearing && !incompatible,
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    (enabled || codex_enabled) && !clearing,
                    egui::Button::new("Stop collection for all providers"),
                )
                .clicked()
            {
                service.stop();
            }
            let has_history = self.snapshot.as_ref().is_some_and(|s| {
                s.has_history
                    || s.enabled
                    || s.codex_enabled
                    || !s.roots.is_empty()
                    || !s.codex_roots.is_empty()
                    || matches!(s.status, Status::Error | Status::Incompatible)
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
    }

    /// The figures come before the configuration: below it they sat out of sight in a scroll area
    /// with no visible bar, and a long status made the window look empty.
    fn figures(&mut self, ui: &mut egui::Ui) {
        if self.service.is_none() {
            return;
        }
        if let Some(snapshot) = &self.snapshot {
            ui.label(status_label(snapshot.status));
        }
        if snapshot_has_partial_codex(self.snapshot.as_ref()) {
            ui.small("Codex coverage can be incomplete. Compressed rollouts and unverified fork history are skipped.");
        }
        let old_period = self.period;
        ui.horizontal_wrapped(|ui| {
            for (period, label) in Period::OPTIONS {
                if self.granularity != Granularity::Hour || period == Period::Week {
                    ui.selectable_value(&mut self.period, period, label);
                }
            }
        });
        let old_provider = self.provider;
        let old_granularity = self.granularity;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.provider, None, "All providers");
            ui.selectable_value(&mut self.provider, Some(Provider::Claude), "Claude Code");
            ui.selectable_value(&mut self.provider, Some(Provider::Codex), "Codex");
        });
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.granularity, Granularity::Hour, "Hourly");
            ui.selectable_value(&mut self.granularity, Granularity::Day, "Daily");
            ui.selectable_value(&mut self.granularity, Granularity::Month, "Monthly");
        });
        let mut model_changed = false;
        ui.horizontal(|ui| {
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.model_filter)
                    .hint_text("Model name")
                    .char_limit(128),
            );
            if ui.button("Filter model").clicked()
                || (input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
            {
                let model = self.model_filter.trim();
                let next = (!model.is_empty()).then(|| model.to_owned());
                model_changed = self.applied_model != next;
                self.applied_model = next;
            }
            if self.applied_model.is_some() && ui.button("Clear model filter").clicked() {
                self.model_filter.clear();
                self.applied_model = None;
                model_changed = true;
            }
        });
        if self.granularity == Granularity::Hour {
            self.period = Period::Week;
        }
        if old_period != self.period
            || old_provider != self.provider
            || old_granularity != self.granularity
            || model_changed
        {
            self.hide_result = true;
            request_comparison(self);
            return;
        }
        if !self.hide_result
            && let Some(snapshot) = &self.snapshot
            && snapshot.status != Status::WaitingToClear
            && self.snapshot_matches_range(snapshot)
            && (snapshot.has_history
                || snapshot.last_success_at_ms.is_some()
                || snapshot.codex_last_success_at_ms.is_some())
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
        Status::Incompatible => "Ivy cannot open this version of the saved history.",
        Status::Error => "Could not read local usage.",
        Status::Stale => "Stale results from the last successful collection.",
    }
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
    #[cfg(target_os = "linux")]
    view.quota.open(ctx);
    view.blocked_frame = Some(ctx.cumulative_frame_nr());
    view.awaiting_release = ctx.input(|input| input.pointer.any_down());
    request_comparison(view);
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
    let ctx = ui.ctx().clone();
    if view.service.is_some() || view.allowances.polling() {
        let started = Instant::now();
        view.install_snapshot();
        view.allowances.poll(started);
        window.timed_separately += view.timing.record(0, started.elapsed());
    }
    if !view.measuring {
        let started = Instant::now();
        view.allowances.tick(&ctx, started, true);
        #[cfg(target_os = "linux")]
        view.quota.chip_frame(true);
        chip::begin_pass(view, &ctx);
        window.timed_separately += view.timing.record(report::CHIP, started.elapsed());
    }
    if !view.blocks(&ctx) {
        return;
    }
    let started = Instant::now();
    #[cfg(target_os = "linux")]
    let mut launch = None;
    if view.open {
        show(
            view,
            &ctx,
            &window.theme,
            #[cfg(target_os = "linux")]
            &mut launch,
        );
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
    #[cfg(target_os = "linux")]
    if let Some(launch) = launch {
        quota_view::launch_in_window(window, &ctx, launch);
    }
}

fn show(
    view: &mut View,
    ctx: &egui::Context,
    theme: &Theme,
    #[cfg(target_os = "linux")] launch: &mut Option<quota_view::Launch>,
) {
    let now = chrono::Utc::now().timestamp_millis();
    if view
        .requested_range
        .is_some_and(|range| now >= range.end_ms)
    {
        request_comparison(view);
    }
    if let Some(range) = view.requested_range
        && let Ok(remaining) = u64::try_from(range.end_ms - now)
    {
        ctx.request_repaint_after(Duration::from_millis(remaining));
    }
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
                    view.figures(ui);
                    ui.separator();
                    #[cfg(target_os = "linux")]
                    {
                        *launch = view.quota.show(ui);
                    }
                    #[cfg(not(target_os = "linux"))]
                    ui.small("Claude allowance unavailable · This platform has not been verified.");
                    ui.separator();
                    view.allowances.show(ui);
                    ui.separator();
                    view.body(ui);
                });
        });
    if close || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        #[cfg(target_os = "linux")]
        {
            launch.take();
        }
        view.close(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::{paint::token_label, *};
    use usage_core::Totals;

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
    fn figures_are_laid_out_above_the_configuration() {
        let cache = std::env::temp_dir().join(format!(
            "ivy-usage-layout-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        ));
        let mut view = View {
            service: Some(Service::new(cache.clone(), || {})),
            ..View::default()
        };
        let range = view.period.range().unwrap();
        view.requested_range = Some(range);
        view.snapshot = Some(Snapshot {
            status: Status::Limit,
            enabled: true,
            range: Some(range),
            provider: view.provider,
            granularity: view.granularity,
            has_history: true,
            ..Snapshot::default()
        });
        view.open = true;
        let theme = Theme::default();
        #[cfg(target_os = "linux")]
        let mut launch = None;
        let ctx = egui::Context::default();
        let mut pass = || {
            // The window size the empty-looking screen was reported at.
            let reported = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 740.0),
                )),
                ..Default::default()
            };
            ctx.run_ui(reported, |ui| {
                show(
                    &mut view,
                    ui.ctx(),
                    &theme,
                    #[cfg(target_os = "linux")]
                    &mut launch,
                )
            })
        };
        // A modal's first pass only measures it; the second one paints.
        pass().textures_delta.clear();
        let mut output = pass();
        output.textures_delta.clear();
        let painted: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        let position = |wanted: &str| painted.iter().position(|text| text == wanted);
        let status = position(status_label(Status::Limit)).expect("status in the first view");
        let summary = position("Tokens unavailable").expect("figures in the first view");
        assert!(status < summary, "{painted:?}");
        drop(view);
        let _ = std::fs::remove_dir_all(cache);
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
