//! Deterministic keyboard exercise for matched Board measurement variants.
use eframe::egui;

#[derive(Default)]
pub(crate) struct Probe {
    frame: Option<u64>,
    ticks: usize,
    rich: bool,
}

pub(crate) fn configure(window: &mut crate::Window, measuring: bool, mode: Option<&str>) {
    window.board.enabled = !measuring || matches!(mode, Some("open" | "closed"));
    if measuring && let Some(mode) = mode {
        window.board.timing.set_mode(mode);
        window.board.probe = (mode == "open").then(|| Probe {
            rich: std::env::var_os("MEASURE_BOARD_RICH").is_some(),
            ..Probe::default()
        });
    }
}

pub(crate) fn inject(window: &mut crate::Window, raw: &mut egui::RawInput, ctx: &egui::Context) {
    let Some(probe) = &mut window.board.probe else {
        return;
    };
    let frame = ctx.cumulative_frame_nr();
    if probe.frame == Some(frame) {
        return;
    }
    let started = std::time::Instant::now();
    probe.frame = Some(frame);
    probe.ticks += 1;
    let tick = probe.ticks % 40;
    let rich = probe.rich;
    let open = window.board.is_open();
    if rich && open && matches!(tick, 3 | 6 | 11 | 15 | 19 | 23 | 27 | 31 | 35 | 39) {
        let target = window
            .board
            .surface
            .child_hits
            .iter()
            .find(|(_, command)| match command {
                super::cards::Command::Expand(_) => true,
                super::cards::Command::Body(_, crate::panel::body::Action::Fold(_, _)) => {
                    tick % 8 == 7
                }
                _ => false,
            })
            .map(|(rect, _)| rect.center());
        if let Some(pos) = target {
            raw.events.push(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                raw.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        } else {
            raw.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, if tick < 20 { -120.0 } else { 120.0 }),
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
    let chord = egui::Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    let key = match tick {
        1 | 13 | 25 if !open => Some((egui::Key::O, chord)),
        // The grid's arrows walk `layout::vertical`, which Home and End never reach.
        4 if open => Some((egui::Key::ArrowDown, egui::Modifiers::NONE)),
        16 if open => Some((egui::Key::ArrowUp, egui::Modifiers::NONE)),
        8 if open => Some((egui::Key::End, egui::Modifiers::NONE)),
        20 if open => Some((egui::Key::Home, egui::Modifiers::NONE)),
        9 | 21 if open => Some((egui::Key::Enter, egui::Modifiers::NONE)),
        _ => None,
    };
    if let Some((key, modifiers)) = key {
        for pressed in [true, false] {
            raw.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            });
        }
    }
    window.spent_injecting += started.elapsed();
}
