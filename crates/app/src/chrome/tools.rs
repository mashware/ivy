//! The title bar's doors to Files, Markdown and the Board: Files at the left end, the other two
//! left of the right-hand group, where the artboards put them once the right rail went (D72).
use super::{Action, button_of, shortcut_label, strip};
use crate::{Window, sidebar::Tool};
use eframe::egui::{self, Color32, Rect, Vec2};

/// The tool a title-bar button opens, read from its hover text (the Board's carries its chord).
pub(super) fn tool_of(hint: &str) -> Option<Tool> {
    [Tool::Files, Tool::Markdown, Tool::Board]
        .into_iter()
        .find(|tool| hint.starts_with(tool.label()))
}

fn slot(row: Rect, x: f32) -> Rect {
    Rect::from_min_size(
        egui::pos2(x, row.top() + 7.0),
        Vec2::new(strip::BUTTON, (row.height() - 14.0).max(0.0)),
    )
}

/// The x of the separator between the tools and the right-hand group.
pub(super) fn separator(row: Rect) -> f32 {
    first(row) + 3.0
}

fn first(row: Rect) -> f32 {
    row.right() - strip::RIGHT_MARGIN - strip::right_buttons() as f32 * strip::BUTTON - 6.0
}

/// Paint the three buttons of a full bar and record where they fell, for the probe and tests.
pub(super) fn show(
    window: &mut Window,
    ui: &egui::Ui,
    geometry: &strip::Strip,
    hover: Color32,
    fg: Color32,
) -> Option<Action> {
    let row = geometry.row;
    let board = window
        .keymap
        .chord_for(crate::keyboard::Action::Board)
        .map_or_else(
            || "Board".to_owned(),
            |chord| format!("Board ({})", shortcut_label(chord)),
        );
    let places = [
        (
            Tool::Files,
            "Files".to_owned(),
            slot(row, row.left() + strip::left_offset()),
        ),
        (Tool::Board, board, slot(row, first(row) - strip::BUTTON)),
        (
            Tool::Markdown,
            "Markdown".to_owned(),
            slot(row, first(row) - 2.0 * strip::BUTTON),
        ),
    ];
    let mut action = None;
    let mut painted = Vec::new();
    for (tool, hint, rect) in places {
        if rect.right() > row.right() || (tool != Tool::Files && rect.left() < geometry.left_end) {
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
        let (clicked, response) = button_of(ui, rect, "", &hint, hover, fg, on);
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
