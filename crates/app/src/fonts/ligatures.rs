//! A single explicitly selected face; no fallback discovery or separate glyph atlas.
use super::*;
use std::time::Duration;

pub(super) const FAMILY: &str = "terminal-ligature-face";

/// Cumulative subset of painting, never an additional framework-cost subtraction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LigatureCost {
    pub spent: Duration,
    pub eligible_runs: usize,
    pub shaped_runs: usize,
    pub fallback_runs: usize,
    pub vertices: usize,
}

impl Discoverer {
    pub fn ligature_cost(&self) -> LigatureCost {
        self.ligature_cost
    }
}

/// Ready only when the active engine holds this exact selected face, including its index.
pub(crate) fn ligature_family(
    discoverer: &Discoverer,
    ctx: &egui::Context,
) -> Option<egui::FontFamily> {
    let chosen = discoverer.chosen.as_ref()?;
    ctx.fonts(|fonts| {
        let definitions = fonts.definitions();
        let active = definitions.font_data.get(&chosen.family)?;
        if !Arc::ptr_eq(active, &chosen.data)
            || !definitions
                .families
                .contains_key(&egui::FontFamily::Name(FAMILY.into()))
        {
            return None;
        }
        Some(egui::FontFamily::Name(FAMILY.into()))
    })
}

pub(crate) fn ligature_covers(discoverer: &Discoverer, text: &str) -> bool {
    let Some(face) = discoverer.chosen.as_ref() else {
        return false;
    };
    let Ok(font) = skrifa::FontRef::from_index(&face.data.font, face.data.index) else {
        return false;
    };
    let map = font.charmap();
    text.chars().all(|c| map.map(c).is_some())
}

/// Explicit benchmark assets use the same selection/registration path without system enumeration.
pub(crate) fn use_measurement_face(
    discoverer: &mut Discoverer,
    data: egui::FontData,
    ctx: &egui::Context,
) {
    discoverer.chosen = Some(SystemFace {
        family: "chosen:measurement-ligatures".into(),
        data: Arc::new(data),
        for_icons: false,
    });
    selection::queue_definitions(
        discoverer.chosen.as_ref(),
        &discoverer.faces,
        discoverer.preview.as_ref(),
        ctx,
    );
}

pub(crate) fn record_ligatures(discoverer: &mut Discoverer, cost: LigatureCost) {
    discoverer.ligature_cost.spent += cost.spent;
    discoverer.ligature_cost.eligible_runs += cost.eligible_runs;
    discoverer.ligature_cost.shaped_runs += cost.shaped_runs;
    discoverer.ligature_cost.fallback_runs += cost.fallback_runs;
    discoverer.ligature_cost.vertices += cost.vertices;
}
