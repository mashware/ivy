//! Deterministic title-bar and drawer-divider exercise for the versioned dock measurement regimes.
use eframe::egui;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Stage {
    ClickMarkdown,
    ReleaseMarkdown,
    PressDivider,
    MoveDivider,
    ReleaseDivider,
    ClickBoard,
    ReleaseBoard,
    NavigateToLastCard,
    ActivateCard,
    ReturnToMarkdown,
    ReleaseMarkdownAgain,
    ReturnToBoard,
    ReleaseBoardAgain,
    #[default]
    Done,
}

#[derive(Default)]
pub(crate) struct Probe {
    frame: Option<u64>,
    valid_frames: usize,
    open: bool,
    stage: Stage,
    last_active: Option<Option<super::Tool>>,
    switches: usize,
    /// The drawer's height while it is shown: the divider drag is witnessed by its range.
    minimum_height: f32,
    maximum_height: f32,
}

pub(crate) fn configure(window: &mut crate::Window, measuring: bool, mode: Option<&str>) {
    let mode = if measuring {
        mode.filter(|mode| matches!(*mode, "open" | "closed"))
    } else {
        None
    };
    window.sidebar.enabled = !measuring || mode.is_some();
    let Some(mode) = mode else {
        window.sidebar.probe = None;
        return;
    };
    window.sidebar.active = None;
    window.board.enabled = true;
    window.board.timing.set_mode(mode);
    window.board.probe = None;
    window.sidebar.probe = Some(Probe {
        open: mode == "open",
        stage: if mode == "open" {
            Stage::ClickMarkdown
        } else {
            Stage::Done
        },
        ..Probe::default()
    });
}

fn pointer(raw: &mut egui::RawInput, pos: egui::Pos2, pressed: bool) {
    raw.events.push(egui::Event::PointerMoved(pos));
    raw.events.push(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
}

fn key(raw: &mut egui::RawInput, key: egui::Key) {
    for pressed in [true, false] {
        raw.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
}

fn button(window: &crate::Window, tool: super::Tool) -> Option<egui::Pos2> {
    window
        .sidebar
        .buttons
        .iter()
        .find(|(which, _)| *which == tool)
        .map(|(_, rect)| rect.center())
}

pub(crate) fn inject(window: &mut crate::Window, raw: &mut egui::RawInput, ctx: &egui::Context) {
    let frame = ctx.cumulative_frame_nr();
    let Some(existing) = &window.sidebar.probe else {
        return;
    };
    if existing.frame == Some(frame) {
        return;
    }
    let started = std::time::Instant::now();
    let active = window.sidebar.active;
    let last_consoles = window.sidebar.last_consoles;
    let markdown_button = button(window, super::Tool::Markdown);
    let board_button = button(window, super::Tool::Board);
    let last_dock = window.sidebar.last_dock;
    let Some(probe) = &mut window.sidebar.probe else {
        return;
    };
    probe.frame = Some(frame);
    if !last_consoles.is_positive() {
        window.spent_injecting += started.elapsed();
        return;
    }
    // Native backends may deliver several frames before the window receives desktop focus.
    // Do not consume the scripted clicks or divider drag during those frames: egui ignores
    // pointer input while unfocused, so advancing here would make later events land out of sync.
    if !raw.focused {
        window.spent_injecting += started.elapsed();
        return;
    }
    // The first valid geometry is a sizing pass: egui may have laid out the rail before its
    // buttons are interactive. Let two complete geometry frames settle before injecting a click.
    probe.valid_frames += 1;
    if probe.valid_frames < 2 {
        window.spent_injecting += started.elapsed();
        return;
    }
    if let Some(previous) = probe.last_active {
        if previous != active {
            probe.switches += 1;
            probe.last_active = Some(active);
        }
    } else {
        probe.last_active = Some(active);
    }
    if let Some(dock) = last_dock.filter(|_| active == Some(super::Tool::Markdown)) {
        let height = dock.height();
        if height > 0.0 {
            if probe.minimum_height == 0.0 || height < probe.minimum_height {
                probe.minimum_height = height;
            }
            probe.maximum_height = probe.maximum_height.max(height);
        }
    }
    if !probe.open {
        window.spent_injecting += started.elapsed();
        return;
    }

    // Button coordinates come from the title bar's painted rectangles; no spacing is re-created.
    match probe.stage {
        Stage::ClickMarkdown => {
            if let Some(pos) = markdown_button {
                pointer(raw, pos, true);
                probe.stage = Stage::ReleaseMarkdown;
            }
        }
        Stage::ReleaseMarkdown => {
            if let Some(pos) = markdown_button {
                pointer(raw, pos, false);
            }
            probe.stage = Stage::PressDivider;
        }
        Stage::PressDivider => {
            if let Some(dock) = last_dock.filter(|_| active == Some(super::Tool::Markdown)) {
                let pos = egui::pos2(dock.center().x, dock.top() - super::DIVIDER / 2.0);
                pointer(raw, pos, true);
                probe.stage = Stage::MoveDivider;
            }
        }
        Stage::MoveDivider => {
            if let Some(dock) = last_dock {
                let pos = egui::pos2(dock.center().x, dock.top() - 43.0);
                raw.events.push(egui::Event::PointerMoved(pos));
                probe.stage = Stage::ReleaseDivider;
            }
        }
        Stage::ReleaseDivider => {
            if let Some(dock) = last_dock {
                let pos = egui::pos2(dock.center().x, dock.top() - super::DIVIDER / 2.0);
                pointer(raw, pos, false);
                probe.stage = Stage::ClickBoard;
            }
        }
        Stage::ClickBoard => {
            if let Some(pos) = board_button {
                pointer(raw, pos, true);
                probe.stage = Stage::ReleaseBoard;
            }
        }
        Stage::ReleaseBoard => {
            if let Some(pos) = board_button {
                pointer(raw, pos, false);
            }
            probe.stage = Stage::NavigateToLastCard;
        }
        Stage::NavigateToLastCard => {
            key(raw, egui::Key::End);
            probe.stage = Stage::ActivateCard;
        }
        Stage::ActivateCard => {
            key(raw, egui::Key::Enter);
            probe.stage = Stage::ReturnToMarkdown;
        }
        Stage::ReturnToMarkdown => {
            if let Some(pos) = markdown_button {
                pointer(raw, pos, true);
                probe.stage = Stage::ReleaseMarkdownAgain;
            }
        }
        Stage::ReleaseMarkdownAgain => {
            if let Some(pos) = markdown_button {
                pointer(raw, pos, false);
            }
            probe.stage = Stage::ReturnToBoard;
        }
        Stage::ReturnToBoard => {
            if let Some(pos) = board_button {
                pointer(raw, pos, true);
                probe.stage = Stage::ReleaseBoardAgain;
            }
        }
        Stage::ReleaseBoardAgain => {
            if let Some(pos) = board_button {
                pointer(raw, pos, false);
                probe.stage = Stage::Done;
            }
        }
        Stage::Done => {}
    }
    window.spent_injecting += started.elapsed();
}

pub(crate) fn publish(window: &crate::Window, to: &mut dyn std::io::Write) -> std::io::Result<()> {
    let probe = window.sidebar.probe.as_ref();
    let selected = window
        .sidebar
        .active
        .map_or_else(|| "none".to_string(), |tool| format!("{tool:?}"));
    let effective_width = window.sidebar.last_dock.map_or(0.0, |rect| rect.width());
    let grids = window
        .all_panes()
        .map(|pane| {
            format!(
                "{}={}x{}",
                pane.name,
                pane.screen.size().cols,
                pane.screen.size().rows
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let identities = window.all_panes().count();
    let switches = probe.map_or(0, |probe| probe.switches);
    let resize_delta = probe.map_or(0.0, |probe| probe.maximum_height - probe.minimum_height);
    writeln!(
        to,
        "sidebar witnesses: selected={selected}, drawer_height={:.0}, effective_width={effective_width:.0}, console_grid={grids}, pane_identities={identities}, switches={switches}, resize_delta={resize_delta:.0}",
        window.sidebar.height
    )
}
