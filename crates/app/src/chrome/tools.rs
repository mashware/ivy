//! The title bar's doors to Files, Markdown and the Board: Files at the left end, the other two
//! left of the right-hand group, where the artboards put them once the right rail went (D72).
use super::{Action, button_of, shortcut_label, strip};
use crate::{Window, sidebar::Tool};
use eframe::egui::{self, Color32, Rect};

/// The tool a title-bar button opens, read from its hover text (the Board's carries its chord).
pub(super) fn tool_of(hint: &str) -> Option<Tool> {
    [Tool::Files, Tool::Document, Tool::Board]
        .into_iter()
        .find(|tool| hint.starts_with(tool.label()))
}

/// Paint Files, the Board and Markdown on their slots and record where they fell, for the probe
/// and tests.
pub(super) fn show(
    window: &mut Window,
    ui: &egui::Ui,
    geometry: &strip::Strip,
    hover: Color32,
    fg: Color32,
) -> Option<Action> {
    let slots = strip::slots(geometry.row);
    let frame = crate::ui::style::roles(ui.ctx(), &window.theme).line2;
    let board = window
        .keymap
        .chord_for(crate::keyboard::Action::Board)
        .map_or_else(
            || "Board".to_owned(),
            |chord| format!("Board ({})", shortcut_label(chord)),
        );
    let places = [
        (Tool::Files, "Files".to_owned(), slots.files),
        (Tool::Board, board, slots.board),
        (Tool::Document, "File".to_owned(), slots.markdown),
    ];
    let mut action = None;
    let mut painted = Vec::new();
    for (tool, hint, rect) in places {
        if rect.right() > geometry.row.right()
            || (tool != Tool::Files && rect.left() < geometry.left_end)
        {
            continue;
        }
        let on = match tool {
            Tool::Files => crate::sidebar::files_visible(window),
            tool => crate::sidebar::active(window) == Some(tool),
        };
        let suppressed = match tool {
            Tool::Files => window.sidebar.last_files.is_none(),
            _ => window.sidebar.last_dock.is_none(),
        };
        let (clicked, response) = if tool == Tool::Board {
            labelled(ui, rect, &hint, (hover, frame), fg, on)
        } else {
            button_of(ui, rect, "", &hint, hover, fg, on)
        };
        if let Some(response) = response {
            painted.push((tool, rect));
            if on && suppressed {
                response.on_hover_text(match tool {
                    Tool::Files => "Widen the window to show Files beside the consoles.".to_owned(),
                    tool => format!(
                        "Make the window taller to show {} below the consoles.",
                        tool.label()
                    ),
                });
            }
        }
        if clicked {
            action = Some(Action::Tool(tool));
        }
    }
    window.sidebar.buttons = painted;
    action
}

/// The Board's button as the kit draws it: framed, its grid icon and its name.
fn labelled(
    ui: &egui::Ui,
    rect: Rect,
    hint: &str,
    (hover, frame): (Color32, Color32),
    fg: Color32,
    on: bool,
) -> (bool, Option<egui::Response>) {
    if !rect.is_positive() {
        return (false, None);
    }
    let response = ui
        .interact(
            rect,
            ui.id().with(("integrated-chrome", "Board")),
            egui::Sense::CLICK,
        )
        .on_hover_text(hint);
    let painter = ui.painter_at(rect.expand(1.0));
    let fill = if response.hovered() || on {
        hover
    } else {
        Color32::TRANSPARENT
    };
    painter.rect(
        rect,
        6,
        fill,
        egui::Stroke::new(1.0, frame),
        egui::StrokeKind::Inside,
    );
    let icon = egui::pos2(rect.left() + 8.0 + 6.5, rect.center().y);
    crate::ui::icons::paint(&painter, icon, crate::ui::icons::Icon::Board, 13.0, fg);
    painter.text(
        egui::pos2(icon.x + 6.5 + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Board",
        egui::FontId::proportional(12.0),
        fg,
    );
    (response.clicked(), Some(response))
}
