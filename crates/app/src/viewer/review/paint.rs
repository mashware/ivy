// Diff row rendering and comment editing for the Git review.
use super::*;

pub(super) fn paint_line(
    review: &mut Review,
    ui: &mut egui::Ui,
    path: &Path,
    line: &DiffLine,
    side: Option<&'static str>,
    width: f32,
    height: f32,
) {
    let side = side.unwrap_or(if line.kind == Change::Removed {
        "old"
    } else {
        "new"
    });
    let number = if side == "old" { line.old } else { line.new };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let ink = match line.kind {
        Change::Added => Some(review.theme.color(term_core::TermColor::Indexed(2))),
        Change::Removed => Some(review.theme.color(term_core::TermColor::Indexed(1))),
        Change::Context => None,
    };
    let tint = {
        let roles = crate::ui::style::roles(ui.ctx(), &review.theme);
        match line.kind {
            Change::Added => Some(roles.add),
            Change::Removed => Some(roles.del),
            Change::Context => None,
        }
    };
    if let Some(tint) = tint {
        ui.painter().rect_filled(rect, 0.0, tint);
    }
    let number_text = |n: Option<u32>| n.map_or_else(String::new, |n| n.to_string());
    let gutter = if side == "old" || side == "new" {
        format!("{:>4}", number_text(number))
    } else {
        String::new()
    };
    let gutter = if review.mode == Mode::Unified {
        format!("{:>4} {:>4}", number_text(line.old), number_text(line.new))
    } else {
        gutter
    };
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let roles = crate::ui::style::roles(ui.ctx(), &review.theme);
    let (marker, marker_ink) = match line.kind {
        Change::Added => ('+', ink.unwrap_or(roles.tx3)),
        Change::Removed => ('−', ink.unwrap_or(roles.tx3)),
        Change::Context => (' ', roles.tx3),
    };
    // The numbers dim, the marker in its change's ink, the code in the console's text.
    let mut job = egui::text::LayoutJob::default();
    let format = |color| egui::TextFormat::simple(font.clone(), color);
    job.append(&format!("{gutter} "), 0.0, format(roles.tx3));
    job.append(&marker.to_string(), 0.0, format(marker_ink));
    job.append(
        &format!("  {}", line.text),
        0.0,
        format(review.theme.foreground()),
    );
    let galley = ui.painter().layout_job(job);
    ui.painter().with_clip_rect(rect).galley(
        egui::pos2(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0),
        galley,
        review.theme.foreground(),
    );
    let plus = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 24.0, rect.top()),
        egui::vec2(24.0, height),
    );
    if number.is_some() && (response.hovered() || response.has_focus()) {
        ui.painter().text(
            plus.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            egui::FontId::proportional(16.0),
            review.theme.foreground(),
        );
    }
    if let Some(number) = number
        && response.clicked()
        && review.draft.is_none()
    {
        review.draft = Some(Draft {
            location: Location {
                path: path.to_path_buf(),
                side,
                line: number,
            },
            text: String::new(),
        });
    }
}

pub(super) fn comments(review: &mut Review, ui: &mut egui::Ui) {
    if let Some(draft) = &mut review.draft {
        ui.separator();
        ui.label(format!(
            "Comment on {} {}:{}",
            quote(&draft.location.path.to_string_lossy()),
            draft.location.side,
            draft.location.line
        ));
        ui.add(egui::TextEdit::multiline(&mut draft.text).desired_width(f32::INFINITY));
        let mut add = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = ui.button("Cancel comment").clicked();
                add = ui
                    .add_enabled(
                        !draft.text.trim().is_empty() && safe(&draft.text),
                        egui::Button::new("Add comment"),
                    )
                    .clicked();
            });
        });
        if !safe(&draft.text) {
            ui.colored_label(
                Color32::RED,
                "Comment contains a forbidden control character.",
            );
        }
        if add {
            let draft = review.draft.take().unwrap();
            review.comments.push(Comment {
                location: draft.location,
                text: draft.text,
            });
            review.preview = false;
        } else if cancel {
            review.draft = None;
        }
    }
    let mut remove = None;
    for (index, comment) in review.comments.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} {}:{} - {}",
                quote(&comment.location.path.to_string_lossy()),
                comment.location.side,
                comment.location.line,
                comment.text
            ));
            if ui.small_button("Remove").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        review.comments.remove(index);
        review.preview = false;
    }
}
