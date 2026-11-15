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
const UI_MEDIUM: &str = "ivy-ui-medium";
/// The family a semibold (600) title is laid out with.
const UI_SEMIBOLD: &str = "ivy-ui-semibold";
/// Geist Mono at medium weight, for paths and titles the chrome sets in mono.
const MONO_MEDIUM: &str = "ivy-mono-medium";

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
    // egui ships both families; without them nothing would stand behind Geist for the symbols and
    // emoji it lacks, so their absence is a broken dependency, not a case to paper over.
    let behind = |family: FontFamily| -> Vec<String> {
        definitions
            .families
            .get(&family)
            .cloned()
            .expect("egui's default font definitions carry this family")
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

/// Installs the base before anything else writes the definitions: start-up calls it first, and a
/// configured family then queues its own definitions on top of it.
pub(crate) fn install(ctx: &egui::Context) {
    ctx.set_fonts(base_definitions());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The family a design weight is laid out with.
    fn ui_family(weight: u16) -> FontFamily {
        match weight {
            400 => FontFamily::Proportional,
            500 => FontFamily::Name(UI_MEDIUM.into()),
            _ => FontFamily::Name(UI_SEMIBOLD.into()),
        }
    }

    fn pass(ctx: &egui::Context) {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }

    /// The raster bounds of `c` laid out at 14 points in `family`: what the eye sees of the face.
    fn drawing(ctx: &egui::Context, c: char, family: FontFamily) -> Option<(u16, u16)> {
        ctx.fonts_mut(|fonts| super::super::drawing_of(fonts, c, family))
    }

    /// With no font configured the window starts on Geist Mono and Geist, with egui's faces behind
    /// them; the machine's fonts are not enumerated for it.
    #[test]
    fn a_start_up_with_no_font_paints_with_the_bundled_faces() {
        let stock = egui::Context::default();
        pass(&stock);
        let ctx = egui::Context::default();
        let mut discoverer = super::super::Discoverer::default();
        let published = super::super::choose_the_font(None, &mut discoverer, &ctx);
        pass(&ctx);
        assert!(matches!(
            published,
            crate::measure::ChoosingTheFont::NotAsked
        ));
        assert!(!discoverer.has_searched_the_machine());
        ctx.fonts(|fonts| {
            let families = &fonts.definitions().families;
            assert_eq!(families[&FontFamily::Monospace][0], "Geist Mono");
            assert_eq!(families[&FontFamily::Monospace][1], "Hack");
            assert_eq!(families[&FontFamily::Proportional][0], "Geist");
            assert_eq!(families[&FontFamily::Proportional][1], "Ubuntu-Light");
        });
        // What is laid out changed, not only the list: Geist Mono's row is taller than Hack's, and
        // Geist's lowercase is wider than Ubuntu Light's.
        let laid_out = |ctx: &egui::Context, text: &str, family: FontFamily| {
            ctx.fonts_mut(|f| {
                f.layout_no_wrap(
                    text.into(),
                    egui::FontId::new(14.0, family),
                    egui::Color32::WHITE,
                )
                .size()
            })
        };
        assert!(
            laid_out(&ctx, "a", FontFamily::Monospace).y
                > laid_out(&stock, "a", FontFamily::Monospace).y
        );
        assert!(
            laid_out(&ctx, "interface", FontFamily::Proportional).x
                > laid_out(&stock, "interface", FontFamily::Proportional).x
        );
    }

    /// Each design weight reaches its own face: a 500 label is not drawn with the 400 or 600 one.
    #[test]
    fn each_interface_weight_is_drawn_with_its_own_face() {
        let ctx = egui::Context::default();
        install(&ctx);
        pass(&ctx);
        let regular = drawing(&ctx, 'M', ui_family(400));
        let medium = drawing(&ctx, 'M', ui_family(500));
        let semibold = drawing(&ctx, 'M', ui_family(600));
        ctx.fonts(|fonts| {
            let families = &fonts.definitions().families;
            assert_eq!(families[&ui_family(500)][0], "Geist Medium");
            assert_eq!(families[&ui_family(600)][0], "Geist SemiBold");
            assert_eq!(
                families[&FontFamily::Name(MONO_MEDIUM.into())][0],
                "Geist Mono Medium"
            );
        });
        assert!(regular.is_some() && medium.is_some() && semibold.is_some());
        let advance = |family: FontFamily| {
            ctx.fonts_mut(|f| f.glyph_width(&egui::FontId::new(14.0, family), 'M'))
        };
        let widths = [
            advance(ui_family(400)),
            advance(ui_family(500)),
            advance(ui_family(600)),
        ];
        assert!(
            widths[0] < widths[1] && widths[1] < widths[2],
            "heavier weights are wider in Geist: {widths:?}"
        );
    }
}
