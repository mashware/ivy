//! The bench's link gesture: plain, OSC 8 and panel links activated in rotation, at the points
//! the painting reported. **D51**.
//!
//! Its presses reach the same routing a hand's do; what the routing withholds while this gesture
//! runs is the desktop hand-off, so a measured run opens no browser.

use eframe::egui;

use super::InjectedDrags;

impl InjectedDrags {
    /// Keeps, per source, a point the latest painting drew — from whichever of that frame's panes
    /// drew one — and nothing from an older painting.
    ///
    /// **Older points are dropped, not kept as a fallback**: the routing resolves a press against
    /// the painting before it, so a point from further back can land on a label that is no longer
    /// reachable — the panel republishes every second and its next painting is unsettled — and a
    /// press that misses counts as a gesture that opened nothing. With no point, the rotation waits.
    pub(crate) fn links_were_painted(&mut self, links: [Option<egui::Pos2>; 3], frame: u64) {
        if self.links_painted_in != Some(frame) {
            self.links_painted_in = Some(frame);
            self.links_at = [None; 3];
        }
        for (kept, painted) in self.links_at.iter_mut().zip(links) {
            if painted.is_some() {
                *kept = painted;
            }
        }
    }

    /// A move onto the next source's link, its press and its release, one per frame.
    ///
    /// The panel's label takes a bare click and the console's links take `Ctrl`, which is what each
    /// surface owns; `command` travels with `ctrl` because that is what egui reports off macOS.
    pub(super) fn activate_a_link(&mut self, frame_nr: u64) -> Vec<egui::Event> {
        let source = self.completed % self.links_at.len();
        let Some(at) = self.links_at[source] else {
            return Vec::new();
        };
        self.last_frame = Some(frame_nr);
        let step = self.step;
        self.step += 1;
        if step == 0 {
            return vec![egui::Event::PointerMoved(at)];
        }
        let pressed = step == 1;
        if !pressed {
            self.step = 0;
            self.completed += 1;
        }
        let modifiers = if source == 2 {
            egui::Modifiers::NONE
        } else {
            egui::Modifiers {
                ctrl: true,
                command: true,
                ..egui::Modifiers::NONE
            }
        };
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        }]
    }
}
