//! What frames the drawer's image while it is marked: the tools above it, a column of notes beside
//! it (under it where the drawer is too narrow for both), and under them the line Send will type.
use super::marks::{self, Marks, Tool};
use super::send::{self, Request, Sending};
use crate::theme::{Roles, metrics};
use crate::ui::icons::{self, Icon};
use eframe::egui;
use std::sync::Arc;

pub(super) const TOOLBAR: f32 = 36.0;
const ROW: f32 = 28.0;
/// The side of an icon button: the toolbar's tools, Undo and a note's delete.
const ICON: f32 = 22.0;
/// The notes' column beside the image: a marker's disc, a note a few words long and its delete.
/// Fixed, so the image does not change size as notes are written.
pub(super) const COLUMN: f32 = 260.0;
/// The narrowest image the column may leave beside it. Narrower, the column goes under the image,
/// which keeps the image's width and gives up height instead of being squeezed to nothing.
pub(super) const NARROWEST: f32 = 320.0;
/// The band under image and notes that shows the line Send types: three short lines, scrolling
/// past that. Full width, because a copy's path wraps into many lines in the narrower column.
const FOOTER: f32 = 62.0;
/// The marker's disc as its note's row repeats it: the smallest the image ever draws it, so the
/// row's disc is never bigger than the one it names.
const DISC: f32 = marks::DISC_FLOOR;
/// The column's inner margin.
const MARGIN: egui::Vec2 = egui::vec2(8.0, 6.0);
/// The space under the notice, before the notes.
const NOTICE_GAP: f32 = 4.0;

/// Where each part of the drawer's body goes while an image is marked.
pub(super) struct Layout {
    body: egui::Rect,
    pub(super) bar: egui::Rect,
    pub(super) view: egui::Rect,
    pub(super) column: egui::Rect,
    /// The line Send types, only once there is something to send.
    pub(super) footer: Option<egui::Rect>,
}

impl Layout {
    /// The body's own rounded corners the view reaches: only where nothing sits between the view
    /// and the body's edge does the image take the body's corner.
    pub(super) fn corners(&self, round: u8) -> egui::CornerRadius {
        let bottom = self.view.bottom() >= self.body.bottom();
        let reaches = |side: bool| if bottom && side { round } else { 0 };
        egui::CornerRadius {
            sw: reaches(self.view.left() <= self.body.left()),
            se: reaches(self.view.right() >= self.body.right()),
            ..Default::default()
        }
    }
}

/// Whether the body is wide enough for the column beside the narrowest image.
fn side_by_side(body: egui::Rect) -> bool {
    body.width() >= COLUMN + NARROWEST
}

/// Splits the drawer's body into the toolbar, the image's view, the notes' column and, with
/// `something_to_send`, the line under them. Side by side where the body is wide enough for the
/// column and the narrowest image; otherwise the notes go under the image and take what their
/// `rows` need, up to half of what is left, scrolling past that. Either way the column keeps
/// `kept`, the height of what it shows outside its scrolling list (`Fixed::height`), before the
/// line Send types and the image get theirs: a refusal clipped away, or a button reaching into the
/// image where a press starts a stroke, would cost more than a shorter image. The column never
/// grows past the body, so on a drawer too short even for `kept`, `Fixed` gives first (#197).
pub(super) fn split(body: egui::Rect, rows: usize, kept: f32, something_to_send: bool) -> Layout {
    let top = (body.top() + TOOLBAR).min(body.bottom());
    let kept = kept + 2.0 * MARGIN.y;
    let room = body.bottom() - top;
    let footer_height = if something_to_send {
        FOOTER.min(room * 0.5).min((room - kept).max(0.0))
    } else {
        0.0
    };
    let bottom = body.bottom() - footer_height;
    let footer = something_to_send
        .then(|| egui::Rect::from_min_max(egui::pos2(body.left(), bottom), body.max));
    let bar = egui::Rect::from_min_max(body.min, egui::pos2(body.right(), top));
    let rest = egui::Rect::from_min_max(
        egui::pos2(body.left(), top),
        egui::pos2(body.right(), bottom),
    );
    let (view, column) = if side_by_side(body) {
        let x = rest.right() - COLUMN;
        (
            egui::Rect::from_min_max(rest.min, egui::pos2(x, rest.bottom())),
            egui::Rect::from_min_max(egui::pos2(x, rest.top()), rest.max),
        )
    } else {
        let notes = (kept + rows as f32 * ROW)
            .min(rest.height() * 0.5)
            .max(kept)
            .min(rest.height());
        let y = rest.bottom() - notes;
        (
            egui::Rect::from_min_max(rest.min, egui::pos2(rest.right(), y)),
            egui::Rect::from_min_max(egui::pos2(rest.left(), y), rest.max),
        )
    };
    Layout {
        body,
        bar,
        view,
        column,
        footer,
    }
}

/// The rows the scrolling list takes when the notes go under the image: one per marker, and the
/// general note's label and field.
pub(super) fn rows(marks: &Marks) -> usize {
    marks.numbered().count() + 2
}

/// What the column keeps whole, outside the list that scrolls: the notice at its head and the
/// Send button at its foot. Laid out before the drawer is split and painted as laid out, so the
/// room `split` keeps for them is the room they take. The notice wraps whole while the column can
/// hold it beside the button; where the drawer is too short for that, it is cut to one line with
/// the whole of it on hover, because the button keeps its full height: a button squeezed to a
/// sliver, or painted up over the toolbar, was the defect (#197).
pub(super) struct Fixed {
    notice: Option<Arc<egui::Galley>>,
    /// The whole notice, when the one painted is cut to a line.
    cut: Option<String>,
    send: Option<Arc<egui::Galley>>,
    padding: egui::Vec2,
    spacing: f32,
}

impl Fixed {
    /// `send` is the console Send names and whether a Send is on its way, once there is
    /// something to send.
    pub(super) fn new(
        ctx: &egui::Context,
        style: &egui::Style,
        body: egui::Rect,
        notice: Option<&str>,
        refused: egui::Color32,
        send: Option<(&str, bool)>,
    ) -> Self {
        let column = if side_by_side(body) {
            COLUMN
        } else {
            body.width()
        };
        let width = (column - 2.0 * MARGIN.x).max(0.0);
        let padding = style.spacing.button_padding;
        let lay = |text: egui::WidgetText, wrap: egui::text::TextWrapping, font| {
            text.into_galley_impl(ctx, style, wrap, font, egui::Align::Min)
        };
        let wrapped = |width: f32| egui::text::TextWrapping::wrap_at_width(width.max(1.0));
        let said =
            |notice: &str| egui::WidgetText::from(egui::RichText::new(notice).color(refused));
        let send = send.map(|(console, busy)| {
            let font = egui::TextStyle::Button.into();
            let label = send_label(console, busy).into();
            lay(label, wrapped(width - 2.0 * padding.x), font)
        });
        let mut fixed = Self {
            notice: notice.map(|n| lay(said(n), wrapped(width), egui::FontSelection::Default)),
            cut: None,
            send,
            padding,
            spacing: style.spacing.item_spacing.y,
        };
        let room = body.height() - TOOLBAR - 2.0 * MARGIN.y;
        if let Some(notice) = notice
            && fixed.height() > room
        {
            let line = egui::text::TextWrapping::truncate_at_width(width.max(1.0));
            fixed.notice = Some(lay(said(notice), line, egui::FontSelection::Default));
            fixed.cut = Some(notice.to_owned());
        }
        fixed
    }

    /// The height the notice and the button take together, with the space each leaves after it.
    pub(super) fn height(&self) -> f32 {
        let notice = self.notice.as_ref().map_or(0.0, |notice| {
            notice.size().y + NOTICE_GAP + 2.0 * self.spacing
        });
        let send = self
            .send
            .as_ref()
            .map_or(0.0, |send| self.button_height(send) + self.spacing);
        notice + send
    }

    /// The button's text with its padding and its one-point border on both sides.
    fn button_height(&self, label: &egui::Galley) -> f32 {
        (label.size().y + 2.0 * (self.padding.y + 1.0)).max(ROW + 4.0)
    }
}

/// What the Send button says: the console it types into, or that a Send is on its way, which
/// is why the notes cannot be changed until it is done.
fn send_label(console: &str, busy: bool) -> String {
    if busy {
        "Sending…".to_owned()
    } else {
        format!("Send to {console} and press Enter")
    }
}

/// An icon button of the set, named by its tooltip and to a screen reader alike: the glyph is a
/// private-use codepoint. A `chosen` one carries the accent, as a pressed segment does.
fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    name: &str,
    chosen: bool,
    roles: &Roles,
) -> egui::Response {
    let button = if chosen {
        egui::Button::new("")
            .fill(roles.accent_soft)
            .stroke(egui::Stroke::new(1.0, roles.accent))
    } else {
        egui::Button::new("").frame_when_inactive(false)
    };
    let response = ui.add(button.min_size(egui::vec2(ICON, ICON)));
    let ink = if chosen {
        roles.accent
    } else {
        ui.style().interact(&response).fg_stroke.color
    };
    icons::paint(ui.painter(), response.rect.center(), icon, 16.0, ink);
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, chosen, name)
    });
    response.on_hover_text(name)
}

/// The tools, grouped, then Undo and Clear, none of which answers while `frozen`.
pub(super) fn toolbar(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    marks: &mut Marks,
    frozen: bool,
    roles: &Roles,
) {
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(8.0, 4.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    if frozen {
        ui.disable();
    }
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, roles.line2))
        .corner_radius(metrics::RADIUS_CONTROL)
        .inner_margin(2)
        .show(&mut ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (tool, icon, name) in [
                (Tool::Arrow, Icon::ArrowUpRight, "Arrow"),
                (Tool::Ellipse, Icon::Ellipse, "Ellipse"),
                (Tool::Rectangle, Icon::Rectangle, "Rectangle"),
                (Tool::Marker, Icon::Marker, "Marker"),
            ] {
                if icon_button(ui, icon, name, marks.tool == tool, roles).clicked() {
                    marks.tool = tool;
                }
            }
        });
    ui.separator();
    let undo = if cfg!(target_os = "macos") {
        "Undo (Cmd+Z)"
    } else {
        "Undo (Ctrl+Z)"
    };
    let empty = marks.marks().is_empty();
    if ui
        .add_enabled_ui(!empty, |ui| icon_button(ui, Icon::Undo, undo, false, roles))
        .inner
        .clicked()
    {
        marks.undo();
    }
    let clear = egui::Button::new("Clear").min_size(egui::vec2(0.0, ICON));
    if ui.add_enabled(!marks.is_empty(), clear).clicked() {
        marks.clear();
    }
}

/// What the notes show besides the marks.
pub(super) struct Panel<'a> {
    /// The notice and the button, as laid out before the split.
    pub(super) fixed: Fixed,
    pub(super) roles: Roles,
    /// The colour a refusal is said in.
    pub(super) refused: egui::Color32,
    /// The opening of the image the notes belong to.
    pub(super) document: u64,
    pub(super) origin: &'a crate::viewer::Source,
    pub(super) sending: &'a Sending,
}

/// The column: a notice when there is one, a note per marker after its disc with a button that
/// deletes it, the general note, and at its foot the button that sends, which names the console.
/// Then, under image and column, the line it types. Every field is a single line: what is written
/// here goes into one line sent to a console. Answers what a click on Send asked for.
pub(super) fn notes(
    ui: &mut egui::Ui,
    layout: &Layout,
    marks: &mut Marks,
    panel: Panel,
) -> Option<Request> {
    let Panel {
        fixed,
        roles,
        refused,
        document,
        origin,
        sending,
    } = panel;
    let mut column = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(layout.column.shrink2(MARGIN))
            .layout(egui::Layout::bottom_up(egui::Align::Min)),
    );
    // On a drawer too short for the button, its top is cut rather than laid over the toolbar.
    column.set_clip_rect(layout.column.intersect(column.clip_rect()));
    // A Send on its way takes the notes as they were at its click.
    if sending.busy() {
        column.disable();
    }
    // The button is laid out first so it keeps the column's foot however many notes there are;
    // what it asks for is composed after the notes, from what they hold at the end of this pass.
    // The fields carry ids of their own: a button or a notice that comes and goes ahead of them
    // would otherwise renumber egui's automatic ids and take the keyboard from a note mid-word.
    let ready = matches!(sending.target, Some(Ok(_))) && !sending.busy();
    let clicked = fixed.send.as_ref().is_some_and(|label| {
        let height = fixed.button_height(label);
        send_button(&mut column, ready, Arc::clone(label), height, &roles)
    });
    let rest = column.available_size();
    column.allocate_ui_with_layout(rest, egui::Layout::top_down(egui::Align::Min), |ui| {
        // Outside the list that scrolls: a refusal is read without scrolling up to it.
        if let Some(notice) = &fixed.notice {
            let said = ui.add(egui::Label::new(Arc::clone(notice)));
            if let Some(whole) = &fixed.cut {
                said.on_hover_text(whole);
            }
            ui.add_space(NOTICE_GAP);
        }
        egui::ScrollArea::vertical()
            .id_salt("image-notes")
            .auto_shrink(false)
            .show(ui, |ui| list(ui, marks, document, &roles));
    });
    if marks.is_empty() {
        return None;
    }
    let line = match &sending.target {
        Some(Ok(path)) => Ok((compose(path, marks), path)),
        Some(Err(why)) => Err(format!("{why}; nothing can be sent.")),
        None => Err("Nothing can be sent from here.".to_owned()),
    };
    if let Some(rect) = layout.footer {
        footer(ui, rect, &line, refused, &roles);
    }
    let (line, path) = line.ok().filter(|_| clicked)?;
    Some(Request {
        document,
        pane: origin.pane,
        path: path.clone(),
        line,
    })
}

/// The primary action: filled with the accent, the column's width, its `label` laid out wrapped
/// rather than losing the console's name. Answers whether it was clicked.
fn send_button(
    ui: &mut egui::Ui,
    ready: bool,
    label: Arc<egui::Galley>,
    height: f32,
    roles: &Roles,
) -> bool {
    let width = ui.available_width();
    ui.scope(|ui| {
        let v = &mut ui.style_mut().visuals;
        for (widget, amount) in [
            (&mut v.widgets.inactive, 0.0),
            (&mut v.widgets.hovered, 0.12),
            (&mut v.widgets.active, 0.24),
        ] {
            let fill = crate::theme::mix(roles.accent, roles.on_accent, amount);
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            // One border width in every state: egui sizes a button's margin by its current
            // state's border, and a width that changed on hover would move the column.
            widget.bg_stroke = egui::Stroke::new(1.0, fill);
            widget.fg_stroke.color = roles.on_accent;
        }
        let button = egui::Button::new(label).min_size(egui::vec2(width, height));
        ui.add_enabled(ready, button).clicked()
    })
    .inner
}

/// A row per marker, and the general note, of the image opened as `document`.
fn list(ui: &mut egui::Ui, marks: &mut Marks, document: u64, roles: &Roles) {
    let numbered: Vec<_> = marks.numbered().collect();
    let mut deleted = None;
    for (number, index, serial) in numbered {
        ui.horizontal(|ui| {
            let (disc, _) =
                ui.allocate_exact_size(egui::vec2(2.0 * DISC + 2.0, ICON), egui::Sense::hover());
            marks::disc(ui.painter(), disc.center(), DISC, number);
            let field = ui.available_width() - ICON - ui.spacing().item_spacing.x;
            if let Some(note) = marks.note_mut(index) {
                ui.add_sized(
                    [field.max(40.0), ICON],
                    egui::TextEdit::singleline(note)
                        .id(egui::Id::new(("image-note", document, serial)))
                        .hint_text("What this marker shows"),
                );
            }
            let name = format!("Delete marker {number}");
            if icon_button(ui, Icon::Close, &name, false, roles).clicked() {
                deleted = Some(index);
            }
        });
    }
    if let Some(index) = deleted {
        marks.delete(index);
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new("General note").small().color(roles.tx2));
    ui.add_sized(
        [ui.available_width(), ICON],
        egui::TextEdit::singleline(&mut marks.general)
            .id(egui::Id::new(("image-general-note", document)))
            .hint_text("A note on the whole image"),
    );
}

/// Under image and column, the line Send types, muted and wrapped, or why nothing can be sent.
fn footer(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    line: &Result<(String, &std::path::PathBuf), String>,
    refused: egui::Color32,
    roles: &Roles,
) {
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(8.0, 4.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    // A scroll area is never laid out shorter than its own minimum, which a short drawer's band
    // is: without this clip its text ran past the drawer's bottom.
    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical()
        .id_salt("image-send-line")
        .auto_shrink(false)
        .show(&mut ui, |ui| match line {
            Ok((line, _)) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Will send:").small().color(roles.tx3));
                    ui.add(
                        egui::Label::new(egui::RichText::new(line).monospace().color(roles.tx3))
                            .wrap(),
                    );
                });
                ui.label(
                    egui::RichText::new(
                        "It is added to whatever is already typed at that console's prompt, \
                         and Enter runs it.",
                    )
                    .small()
                    .color(roles.tx3),
                );
            }
            Err(why) => {
                ui.add(egui::Label::new(egui::RichText::new(why).color(refused)).wrap());
            }
        });
}

fn compose(path: &std::path::Path, marks: &Marks) -> String {
    send::compose(path, &marks.notes(), &marks.general)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(width: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(width, 400.0))
    }

    /// Where the column and the narrowest image fit side by side, the column takes its fixed width
    /// at the right and the image keeps the whole height between the tools and the line Send
    /// types; a point narrower, the notes go under the image, which keeps the body's width.
    #[test]
    fn a_narrow_body_puts_the_notes_under_the_image_and_a_wide_one_beside_it() {
        let wide = split(body(COLUMN + NARROWEST), 6, 0.0, true);
        let footer = wide.footer.unwrap();
        assert_eq!(wide.column.width(), COLUMN);
        assert_eq!(wide.view.right(), wide.column.left());
        assert_eq!(wide.view.width(), NARROWEST);
        assert_eq!(
            (wide.view.top(), wide.view.bottom()),
            (wide.bar.bottom(), footer.top())
        );
        assert_eq!(wide.column.bottom(), footer.top());
        assert_eq!(footer.width(), wide.body.width());

        let narrow = split(body(COLUMN + NARROWEST - 1.0), 6, 0.0, true);
        let footer = narrow.footer.unwrap();
        assert_eq!(narrow.view.width(), narrow.body.width());
        assert_eq!(narrow.column.width(), narrow.body.width());
        assert_eq!(narrow.column.top(), narrow.view.bottom());
        assert_eq!(narrow.column.bottom(), footer.top());
        assert!(
            narrow.view.height() >= narrow.column.height(),
            "{:?}",
            narrow.view
        );

        let quiet = split(body(COLUMN + NARROWEST), 0, 0.0, false);
        assert!(quiet.footer.is_none());
        assert_eq!(quiet.view.bottom(), quiet.body.bottom());
    }

    /// Under the image, two rows of notes take two rows and the margins, not half the body: the
    /// image keeps the rest.
    #[test]
    fn two_rows_under_the_image_take_two_rows() {
        let narrow = split(body(COLUMN + NARROWEST - 1.0), 2, 0.0, false);
        assert_eq!(narrow.column.height(), 2.0 * ROW + 2.0 * MARGIN.y);
        assert_eq!(narrow.view.bottom(), narrow.column.top());
        assert_eq!(narrow.view.top(), narrow.bar.bottom());
    }

    /// What the column keeps outside its list is kept whole before the image and the line Send
    /// types get any: on a drawer too short for half of it, it takes what the image would have.
    #[test]
    fn what_the_column_keeps_comes_before_the_image_and_the_line() {
        let short = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(300.0, 200.0));
        let kept = 120.0;
        let layout = split(short, 4, kept, true);
        assert!(
            layout.column.height() >= kept + 2.0 * MARGIN.y,
            "{:?}",
            layout.column
        );
        assert!(layout.column.bottom() <= layout.footer.unwrap().top());
        assert!(layout.view.height() >= 0.0, "{:?}", layout.view);
    }
}
