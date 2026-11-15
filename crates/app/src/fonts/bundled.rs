//! The faces Ivy ships: Geist for the interface and Geist Mono for the terminal (D95).
//!
//! **They are the base every font definition is built from**, never a `chosen` face: clearing the
//! configured font, a fallback-discovery round or a Settings preview all rebuild the definitions
//! from here, and a bundled face registered as `chosen` would vanish in each of them. Being part of
//! the binary, they cost no enumeration of the machine (D24 stands).
//!
//! egui's text engine has no weight axis, so each weight the design uses is a face of its own behind
//! a named family.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// The family a medium (500) interface label is laid out with.
pub(crate) const UI_MEDIUM: &str = "ivy-ui-medium";
/// The family a semibold (600) title is laid out with.
pub(crate) const UI_SEMIBOLD: &str = "ivy-ui-semibold";
/// Geist Mono at medium weight, for paths and titles the chrome sets in mono.
pub(crate) const MONO_MEDIUM: &str = "ivy-mono-medium";

const FACES: [(&str, &[u8]); 5] = [
    (
        "Geist",
        include_bytes!("../../../../resources/fonts/Geist-Regular.ttf"),
    ),
    (
        "Geist Medium",
        include_bytes!("../../../../resources/fonts/Geist-Medium.ttf"),
    ),
    (
        "Geist SemiBold",
        include_bytes!("../../../../resources/fonts/Geist-SemiBold.ttf"),
    ),
    (
        "Geist Mono",
        include_bytes!("../../../../resources/fonts/GeistMono-Regular.ttf"),
    ),
    (
        "Geist Mono Medium",
        include_bytes!("../../../../resources/fonts/GeistMono-Medium.ttf"),
    ),
];

/// egui's own definitions with Geist in front of the proportional chain and Geist Mono in front of
/// the monospaced one. egui's faces stay behind them, so a symbol Geist lacks (`⊘`, `∎`, `✓`,
/// braille) still comes from the face D9 measured.
pub(crate) fn base_definitions() -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    for (name, bytes) in FACES {
        definitions
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let behind = |family: FontFamily| -> Vec<String> {
        definitions
            .families
            .get(&family)
            .cloned()
            .unwrap_or_default()
    };
    let proportional = behind(FontFamily::Proportional);
    let monospace = behind(FontFamily::Monospace);
    let chain = |front: &str, rest: &[String]| -> Vec<String> {
        std::iter::once(front.to_owned())
            .chain(rest.iter().cloned())
            .collect()
    };
    let families = [
        (FontFamily::Proportional, chain("Geist", &proportional)),
        (
            FontFamily::Name(UI_MEDIUM.into()),
            chain("Geist Medium", &proportional),
        ),
        (
            FontFamily::Name(UI_SEMIBOLD.into()),
            chain("Geist SemiBold", &proportional),
        ),
        (FontFamily::Monospace, chain("Geist Mono", &monospace)),
        (
            FontFamily::Name(MONO_MEDIUM.into()),
            chain("Geist Mono Medium", &monospace),
        ),
    ];
    for (family, faces) in families {
        definitions.families.insert(family, faces);
    }
    definitions
}

/// The interface family for a design weight: 400, 500 or 600.
pub(crate) fn ui_family(weight: u16) -> FontFamily {
    match weight {
        ..=450 => FontFamily::Proportional,
        451..=550 => FontFamily::Name(UI_MEDIUM.into()),
        _ => FontFamily::Name(UI_SEMIBOLD.into()),
    }
}

/// Installs the bundled faces when nothing else will: a start-up with no configured font never
/// reaches the selection path, which is the only other writer of the definitions.
pub(crate) fn install(ctx: &egui::Context) {
    ctx.set_fonts(base_definitions());
}
