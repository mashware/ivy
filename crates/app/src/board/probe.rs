//! Deterministic keyboard exercise for matched Overview measurement variants.
use eframe::egui;

#[derive(Default)]
pub(crate) struct Probe {
    frame: Option<u64>,
    ticks: usize,
}

pub(crate) fn configure(window: &mut crate::Window, measuring: bool, mode: Option<&str>) {
    window.summary.enabled = !measuring || matches!(mode, Some("open" | "closed"));
    if measuring && let Some(mode) = mode {
        window.summary.timing.set_mode(mode);
        window.summary.probe = (mode == "open").then(Probe::default);
    }
}

pub(crate) fn inject(window: &mut crate::Window, raw: &mut egui::RawInput, ctx: &egui::Context) {
    let Some(probe) = &mut window.summary.probe else {
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
    let open = window.summary.is_open();
    let chord = egui::Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    let key = match tick {
        1 | 13 | 25 if !open => Some((egui::Key::O, chord)),
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
