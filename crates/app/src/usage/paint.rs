//! The saved-history totals and their coverage.

use chrono::Local;
use eframe::egui;
use usage_core::{Cost, DateRange, Day, Snapshot, Totals};

pub(super) fn paint_summary(ui: &mut egui::Ui, snapshot: &Snapshot) {
    ui.small("Saved history includes earlier selections until cleared.");
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

pub(super) fn token_label(totals: &Totals) -> String {
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
