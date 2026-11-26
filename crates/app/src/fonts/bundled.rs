//! The faces Ivy ships: Geist for the interface and Geist Mono for the terminal (D95), and
//! Phosphor for the interface's icons (D103).
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
/// The interface's icons (D103): Phosphor Regular and nothing else. Phosphor maps the lowercase letters to blank
/// glyphs and lives in the private-use area the terminal's icon family serves, so it is never a
/// member of a family that lays out text, and no text face stands behind it.
pub(crate) const UI_ICONS: &str = "ivy-icons";
const ICON_FACE: &str = "Phosphor";

pub(crate) const FACES: [(&str, &[u8]); 5] = [
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
    let mut proportional = behind(FontFamily::Proportional);
    let monospace = behind(FontFamily::Monospace);
    // Hack covers title symbols such as ◐ that none of the proportional faces carry.
    // Keep it behind every existing proportional fallback so their supported glyphs still win.
    for face in &monospace {
        if !proportional.contains(face) {
            proportional.push(face.clone());
        }
    }
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
    definitions.font_data.insert(
        ICON_FACE.into(),
        Arc::new(FontData::from_static(
            egui_phosphor::variants::bytes::regular::FONT,
        )),
    );
    definitions
        .families
        .insert(FontFamily::Name(UI_ICONS.into()), vec![ICON_FACE.into()]);
    definitions
}

/// The icon family, or `None` where the bundled definitions were never installed — a bare test
/// context: asking egui for a family it was never given panics.
pub(crate) fn ui_icon_family(ctx: &egui::Context) -> Option<FontFamily> {
    let family = FontFamily::Name(UI_ICONS.into());
    ctx.fonts(|fonts| fonts.definitions().families.contains_key(&family))
        .then_some(family)
}

/// The interface family for a design weight (400, 500 or 600), or the regular one when the bundled
/// faces are not installed in this context: asking egui for a family it was never given panics.
pub(crate) fn ui_weight(ctx: &egui::Context, weight: u16) -> FontFamily {
    let family = match weight {
        ..=450 => return FontFamily::Proportional,
        451..=550 => FontFamily::Name(UI_MEDIUM.into()),
        _ => FontFamily::Name(UI_SEMIBOLD.into()),
    };
    if ctx.fonts(|fonts| fonts.definitions().families.contains_key(&family)) {
        family
    } else {
        FontFamily::Proportional
    }
}

/// Installs the base before anything else writes the definitions: start-up calls it first, and a
/// configured family then queues its own definitions on top of it.
pub(crate) fn install(ctx: &egui::Context) {
    // Keep raster coverage intact: egui's dark-mode boost thickens partial edge pixels.
    // Set both themes once, so changing appearance cannot restore the heavier atlas.
    ctx.all_styles_mut(|style| {
        style.visuals.text_options.color_transfer_function =
            egui::epaint::FontColorTransferFunction::Off;
    });
    ctx.set_fonts(base_definitions());
}

/// egui's own definitions plus the icon family, for tests written against egui's faces that also
/// need an icon to be painted: installing the whole base would change every layout they measure.
#[cfg(test)]
pub(crate) fn install_icons_only(ctx: &egui::Context) {
    let base = base_definitions();
    let mut definitions = FontDefinitions::default();
    let family = FontFamily::Name(UI_ICONS.into());
    definitions
        .font_data
        .insert(ICON_FACE.into(), base.font_data[ICON_FACE].clone());
    definitions
        .families
        .insert(family.clone(), base.families[&family].clone());
    ctx.set_fonts(definitions);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_preserves_raw_glyph_coverage_in_both_themes() {
        fn mask(ctx: &egui::Context) -> Vec<u8> {
            pass(ctx);
            ctx.fonts_mut(|fonts| {
                let galley = fonts.layout_no_wrap(
                    "Iwm".into(),
                    egui::FontId::monospace(14.666667),
                    egui::Color32::WHITE,
                );
                let image = fonts.image();
                let mut alpha = Vec::new();
                for row in &galley.rows {
                    for glyph in &row.glyphs {
                        for y in glyph.uv_rect.min[1]..glyph.uv_rect.max[1] {
                            for x in glyph.uv_rect.min[0]..glyph.uv_rect.max[0] {
                                alpha.push(image[(usize::from(x), usize::from(y))].a());
                            }
                        }
                    }
                }
                alpha
            })
        }
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let actual = egui::Context::default();
            let mut discoverer = super::super::Discoverer::default();
            super::super::choose_the_font(None, &mut discoverer, &actual);
            actual.set_theme(theme);
            let raw = egui::Context::default();
            raw.set_theme(theme);
            raw.set_fonts(base_definitions());
            raw.all_styles_mut(|style| {
                style.visuals.text_options.color_transfer_function =
                    egui::epaint::FontColorTransferFunction::Off
            });
            let boosted = egui::Context::default();
            boosted.set_theme(theme);
            boosted.set_fonts(base_definitions());
            boosted.all_styles_mut(|style| {
                style.visuals.text_options.color_transfer_function =
                    egui::epaint::FontColorTransferFunction::default()
            });
            let alpha = mask(&actual);
            assert!(alpha.iter().any(|a| *a > 0 && *a < 255));
            assert_eq!(alpha, mask(&raw));
            assert_ne!(alpha, mask(&boosted));
        }
    }

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

    #[test]
    fn pane_titles_paint_claudes_circle_with_the_bundled_fallback() {
        let ctx = egui::Context::default();
        install(&ctx);
        for rebuilt in [false, true] {
            if rebuilt {
                super::super::selection::queue_definitions(None, &[], None, &ctx);
            }
            for weight in [400, 500, 600] {
                for size in [8.0, 14.0, 24.0] {
                    let text = "◐ Siguientes pasos del roadmap";
                    let output = ctx.run_ui(Default::default(), |ui| {
                        crate::header::title_text::paint(
                            ui.painter(),
                            egui::pos2(10.0, 30.0),
                            egui::Align2::LEFT_CENTER,
                            text,
                            egui::FontId::new(size, ui_family(weight)),
                            egui::Color32::WHITE,
                        );
                    });
                    let title = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::epaint::Shape::Text(text) => Some(text.galley.clone()),
                            _ => None,
                        })
                        .expect("the title must be painted");
                    output.drop_without_applying_deltas();
                    assert_eq!(title.job.text, text);
                    let actual = title.rows[0].glyphs[0].uv_rect;
                    let glyph = |c: char, family: FontFamily| {
                        ctx.fonts_mut(|fonts| {
                            fonts
                                .layout_no_wrap(
                                    c.to_string(),
                                    egui::FontId::new(size, family),
                                    egui::Color32::WHITE,
                                )
                                .rows[0]
                                .glyphs[0]
                                .uv_rect
                        })
                    };
                    assert_ne!(
                        actual,
                        glyph('\u{0378}', ui_family(weight)),
                        "replacement box at weight {weight}, size {size}, rebuilt {rebuilt}"
                    );
                    assert_eq!(
                        actual,
                        glyph('◐', FontFamily::Monospace),
                        "the title must reach the bundled face that covers the circle"
                    );
                }
            }
        }
    }

    #[test]
    fn the_extra_ui_fallback_preserves_supported_glyphs_and_metrics() {
        let original = FontDefinitions::default().families[&FontFamily::Proportional].clone();
        let mut before = base_definitions();
        for weight in [400, 500, 600] {
            before
                .families
                .get_mut(&ui_family(weight))
                .unwrap()
                .retain(|face| {
                    FACES.iter().any(|(name, _)| *name == face) || original.contains(face)
                });
        }
        let old = egui::Context::default();
        install(&old);
        old.set_fonts(before);
        pass(&old);
        let new = egui::Context::default();
        install(&new);
        pass(&new);
        let raster = |ctx: &egui::Context, c: char, family: FontFamily, size: f32| {
            ctx.fonts_mut(|fonts| {
                let galley = fonts.layout_no_wrap(
                    c.to_string(),
                    egui::FontId::new(size, family),
                    egui::Color32::WHITE,
                );
                let uv = galley.rows[0].glyphs[0].uv_rect;
                let image = fonts.image();
                let mut pixels = Vec::new();
                for y in uv.min[1]..uv.max[1] {
                    for x in uv.min[0]..uv.max[0] {
                        pixels.push(image[(usize::from(x), usize::from(y))]);
                    }
                }
                (galley.size(), uv.offset, uv.size, pixels)
            })
        };
        for weight in [400, 500, 600] {
            for size in [8.0, 14.0, 24.0] {
                for c in "abcABC012éñ◑".chars() {
                    let previous = raster(&old, c, ui_family(weight), size);
                    assert_ne!(previous, raster(&old, '\u{0378}', ui_family(weight), size));
                    assert_eq!(
                        previous,
                        raster(&new, c, ui_family(weight), size),
                        "existing glyph {c} changed at weight {weight}, size {size}"
                    );
                }
            }
        }
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

    /// Phosphor reaches the interface through its own family alone: no family that lays out text —
    /// the interface's, the terminal's or the one for the private-use area — lists it, and the icon
    /// family holds no text face. Checked on the definitions a start-up with no font installs.
    #[test]
    fn the_icon_face_lives_in_its_own_family_and_nowhere_else() {
        let ctx = egui::Context::default();
        let mut discoverer = super::super::Discoverer::default();
        super::super::choose_the_font(None, &mut discoverer, &ctx);
        pass(&ctx);
        ctx.fonts(|fonts| {
            for (family, faces) in &fonts.definitions().families {
                if *family == FontFamily::Name(UI_ICONS.into()) {
                    assert_eq!(faces, &vec![ICON_FACE.to_owned()]);
                } else {
                    assert!(
                        !faces.iter().any(|f| f == ICON_FACE),
                        "{family:?}: {faces:?}"
                    );
                }
            }
        });
        assert_eq!(
            ui_icon_family(&ctx),
            Some(FontFamily::Name(UI_ICONS.into()))
        );
        // The face really draws there: two Phosphor codepoints lay out two different drawings,
        // which one replacement box for every codepoint would not.
        let glyph = |s: &str| s.chars().next().unwrap();
        let icons = FontFamily::Name(UI_ICONS.into());
        let cross = drawing(&ctx, glyph(egui_phosphor::regular::X), icons.clone());
        let minus = drawing(&ctx, glyph(egui_phosphor::regular::MINUS), icons);
        assert!(cross.is_some() && minus.is_some());
        assert_ne!(cross, minus);
        let bare = egui::Context::default();
        pass(&bare);
        assert_eq!(ui_icon_family(&bare), None);
    }

    /// A heavier weight is asked for only where the bundled faces exist; elsewhere the regular family
    /// stands in, because egui panics on a family it was never given.
    #[test]
    fn a_weight_falls_back_to_regular_where_the_bundled_faces_are_missing() {
        let bare = egui::Context::default();
        pass(&bare);
        assert_eq!(super::ui_weight(&bare, 600), FontFamily::Proportional);
        let ctx = egui::Context::default();
        install(&ctx);
        pass(&ctx);
        assert_eq!(
            super::ui_weight(&ctx, 600),
            FontFamily::Name(UI_SEMIBOLD.into())
        );
        assert_eq!(
            super::ui_weight(&ctx, 500),
            FontFamily::Name(UI_MEDIUM.into())
        );
        assert_eq!(super::ui_weight(&ctx, 400), FontFamily::Proportional);
    }
}
