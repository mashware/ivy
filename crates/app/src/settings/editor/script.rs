//! Opt-in settings measurement uses the same button events as a person.
use eframe::egui;
pub(super) struct Script {
    stage: u8,
    fonts: bool,
}
impl Script {
    pub fn for_mode(mode: Option<&str>) -> Option<Self> {
        match mode {
            Some("open") => Some(Self {
                stage: 0,
                fonts: false,
            }),
            Some("fonts") => Some(Self {
                stage: 0,
                fonts: true,
            }),
            _ => None,
        }
    }
    pub fn next(
        &mut self,
        entry: Option<egui::Rect>,
        font: Option<egui::Rect>,
    ) -> Vec<egui::Event> {
        // Opening the modal and loading its list can move anchored geometry on the next pass.
        if matches!(self.stage, 1 | 3) && self.fonts {
            self.stage += 1;
            return Vec::new();
        }
        let target = match self.stage {
            0 => entry,
            2 | 4 if self.fonts => font,
            _ => None,
        };
        let Some(target) = target else {
            return Vec::new();
        };
        self.stage += 1;
        let pos = target.center();
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
}
