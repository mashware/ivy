//! Prepare a selected face without changing the active text engine.
use super::*;

/// Preparation can fail; successful registration belongs to the startup result instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelectionError {
    NotInstalled,
    Unusable,
    NoFontsOnTheMachine,
}

impl From<SelectionError> for Chosen {
    fn from(error: SelectionError) -> Self {
        match error {
            SelectionError::NotInstalled => Self::NotInstalled,
            SelectionError::Unusable => Self::Unusable,
            SelectionError::NoFontsOnTheMachine => Self::NoFontsOnTheMachine,
        }
    }
}

/// Readable selected-font assets, held until the saved settings can become active.
pub(crate) struct PreparedSelection {
    chosen: Option<SystemFace>,
}

impl PreparedSelection {
    /// Queue last in the saving pass, after fallback discovery has registered its faces.
    pub(crate) fn queue(&self, discoverer: &Discoverer, ctx: &egui::Context) {
        queue_definitions(
            self.chosen.as_ref(),
            &discoverer.faces,
            discoverer.preview.as_ref(),
            ctx,
        );
    }

    /// Commit only after the queued definitions became effective at the next begin-pass.
    pub(crate) fn activate(self, discoverer: &mut Discoverer) {
        discoverer.chosen = self.chosen;
        discoverer.settled.clear();
        discoverer.failures_in_a_row = 0;
        let icons_active = discoverer.every_face().any(|face| face.for_icons);
        discoverer.icon_family_ready = icons_active;
    }

    pub(crate) fn chosen_name(&self) -> Option<String> {
        self.chosen.as_ref().map(|face| {
            face.family
                .strip_prefix(CHOSEN_PREFIX)
                .expect("selected prefix")
                .to_owned()
        })
    }

    /// Clearing is local; a named family may warm discovery caches but changes no active state.
    pub(crate) fn prepare(
        discoverer: &mut Discoverer,
        name: Option<&str>,
    ) -> Result<Self, SelectionError> {
        let Some(name) = name else {
            return Ok(Self { chosen: None });
        };
        let found = look_for_the_family(discoverer, name);
        discoverer.sources.prune(0, true);
        let (family, data) = found?;
        let family = format!("{CHOSEN_PREFIX}{family}");
        // Egui keeps its existing allocations when definitions are equal. Aliases can resolve to
        // the same canonical face, so keep that allocation ready without comparing bytes per row.
        let data = discoverer
            .chosen
            .as_ref()
            .filter(|face| face.family == family && face.data == data)
            .map_or(data, |face| Arc::clone(&face.data));
        Ok(Self {
            chosen: Some(SystemFace {
                family,
                data,
                for_icons: false,
            }),
        })
    }
}

/// The face of that family the text engine can open, or **why there is none**.
///
/// **The failure carries its reason and that is the whole point of the signature.** A bare
/// `Option` here made three different endings —no fonts at all, no such family, a family whose
/// files cannot be opened— arrive as the same `None`, and the terminal then told whoever
/// configured it that their name was wrong in all three.
pub(super) fn look_for_the_family(
    discoverer: &mut Discoverer,
    name: &str,
) -> Result<(String, Arc<egui::FontData>), SelectionError> {
    if !discoverer.ensure_the_collection() {
        return Err(SelectionError::NoFontsOnTheMachine);
    }
    let collection = discoverer
        .collection
        .as_mut()
        .ok_or(SelectionError::NoFontsOnTheMachine)?;
    let id = collection
        .family_id(name)
        .ok_or(SelectionError::NotInstalled)?;
    // **The name is taken from the collection and not from what was typed**, so what the report
    // declares is the family that is really being painted with: asking for `jetbrains mono` and
    // publishing that lowercase name would have the report saying something the machine does not.
    let (real_name, mut faces) = collection
        .family(id)
        .map(|family| (family.name().to_owned(), family.fonts().to_vec()))
        .ok_or(SelectionError::NotInstalled)?;
    regular_first(&mut faces);
    let sources = &mut discoverer.sources;
    let face = faces.into_iter().find_map(|face| {
        let blob = sources.get(face.source())?;
        // Same check and for the same reason as in `first_that_covers`: bytes epaint cannot parse
        // are a `panic!` **inside its pass**, and these are files from outside.
        skrifa::FontRef::from_index(blob.data(), face.index()).ok()?;
        let mut data = egui::FontData::from_owned(blob.data().to_vec());
        data.index = face.index();
        Some(Arc::new(data))
    });
    // **The family is here and none of its files can be opened.** It is a different answer from
    // «it is not installed» and it has to stay different: whoever gets the wrong one reinstalls a
    // font that was already there and reads the very same line again.
    face.map(|data| (real_name, data))
        .ok_or(SelectionError::Unusable)
}

/// Definitions are assembled from current fallback faces, never a preparation-time snapshot.
pub(super) fn queue_definitions(
    chosen: Option<&SystemFace>,
    faces: &[SystemFace],
    preview: Option<&SystemFace>,
    ctx: &egui::Context,
) {
    let mut definitions = egui::FontDefinitions::default();
    let factory = definitions
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let terminal_chain = monospaced(chosen, faces, &factory);
    let icons = icon_ones(chosen, faces, &factory);
    definitions
        .families
        .insert(egui::FontFamily::Monospace, terminal_chain);
    definitions
        .families
        .insert(egui::FontFamily::Name(ICON_FAMILY.into()), icons);
    definitions.families.insert(
        egui::FontFamily::Name("settings-font-preview".into()),
        monospaced(preview, faces, &factory),
    );
    if let Some(face) = chosen {
        let mut ligature_data = face.data.as_ref().clone();
        ligature_data.tweak.subpixel_binning = Some(false);
        definitions
            .font_data
            .insert(super::ligatures::DATA.into(), Arc::new(ligature_data));
        definitions.families.insert(
            egui::FontFamily::Name(super::ligatures::FAMILY.into()),
            vec![super::ligatures::DATA.into()],
        );
    }
    for face in chosen.into_iter().chain(faces).chain(preview) {
        definitions
            .font_data
            .insert(face.family.clone(), Arc::clone(&face.data));
    }
    ctx.set_fonts(definitions);
}

impl Discoverer {
    /// Register only the specimen family; active terminal selection and discovery stay untouched.
    /// Merely opening Settings can reuse the active face, but never loads a different family.
    pub(crate) fn preview_font(
        &mut self,
        name: Option<&str>,
        ctx: &egui::Context,
        allow_load: bool,
    ) -> Result<egui::FontFamily, SelectionError> {
        let face = match name {
            Some(name) => {
                let active = self.chosen.as_ref().filter(|face| {
                    face.family
                        .strip_prefix(CHOSEN_PREFIX)
                        .is_some_and(|active| active.eq_ignore_ascii_case(name))
                });
                let selected = if let Some(active) = active {
                    Some(SystemFace {
                        family: active.family.clone(),
                        data: Arc::clone(&active.data),
                        for_icons: false,
                    })
                } else if allow_load {
                    PreparedSelection::prepare(self, Some(name))?.chosen
                } else {
                    None
                };
                selected.map(|face| SystemFace {
                    family: "settings-preview-face".into(),
                    data: face.data,
                    for_icons: false,
                })
            }
            None => None,
        };
        self.preview = face;
        queue_definitions(
            self.chosen.as_ref(),
            &self.faces,
            self.preview.as_ref(),
            ctx,
        );
        Ok(egui::FontFamily::Name("settings-font-preview".into()))
    }

    /// Discard only draft assets and recompose from the current live selection, never a snapshot.
    pub(crate) fn clear_preview(&mut self, ctx: &egui::Context) {
        if self.preview.take().is_some() {
            queue_definitions(self.chosen.as_ref(), &self.faces, None, ctx);
        }
    }

    /// Paints with the family whoever configures asked for **by its name**.
    ///
    /// **It is only called when the file names one, and that is what keeps D24 standing.** Resolving a
    /// name forces building the system collection — between 17.3 and 23.2 ms, the very cost D24
    /// decided never to pay at start-up. Whoever configures no font never reaches this line and their
    /// start-up is the one of always, bit for bit.
    ///
    /// **It runs before the first frame**, like restoring the layout: `set_fonts` takes effect on the
    /// next pass, and there has been no pass yet.
    ///
    /// The name is matched **the way `fontique` matches it**, which lowercases both sides
    /// (`family_name.rs`, `NameKey::from_str`), so `jetbrains mono` finds `JetBrains Mono`.
    pub fn use_the_family(&mut self, name: &str, ctx: &egui::Context) -> Chosen {
        let prepared = match PreparedSelection::prepare(self, Some(name)) {
            Ok(prepared) => prepared,
            Err(why) => return why.into(),
        };
        let family = prepared
            .chosen_name()
            .expect("a named selection has a face");
        prepared.queue(self, ctx);
        // Startup has no active pass to await; readiness advances through normal resolve.
        self.chosen = prepared.chosen;
        Chosen::Registered(family)
    }
}

/// A selected face leads text; discovered faces only fill holes behind the factory chain.
pub(super) fn monospaced(
    chosen: Option<&SystemFace>,
    faces: &[SystemFace],
    factory: &[String],
) -> Vec<String> {
    chosen
        .into_iter()
        .map(|face| face.family.clone())
        .chain(factory.iter().cloned())
        .chain(faces.iter().map(|face| face.family.clone()))
        .collect()
}

/// Icons use only faces discovered for private-use characters, followed by the factory net.
pub(super) fn icon_ones(
    chosen: Option<&SystemFace>,
    faces: &[SystemFace],
    factory: &[String],
) -> Vec<String> {
    chosen
        .into_iter()
        .chain(faces)
        .filter(|face| face.for_icons)
        .map(|face| face.family.clone())
        .chain(factory.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(ctx: &egui::Context) {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }

    // Width, height and raster bounds are stable across atlas rebuilds; UV origins are not.
    fn appearance(
        ctx: &egui::Context,
        c: char,
        family: egui::FontFamily,
    ) -> (egui::Vec2, Option<(u16, u16)>) {
        let bounds = ctx.fonts_mut(|fonts| drawing_of(fonts, c, family.clone()));
        let metrics = ctx.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(
                    c.to_string(),
                    egui::FontId::new(14.0, family),
                    egui::Color32::WHITE,
                )
                .size()
        });
        (metrics, bounds)
    }

    #[test]
    fn ligature_alias_disables_binning_without_changing_selected_face_readiness() {
        let ctx = egui::Context::default();
        let mut discoverer = Discoverer::default();
        let mut data = egui::FontData::from_static(include_bytes!(
            "../../tests/fixtures/ligatures/JetBrainsMono-Regular.ttf"
        ));
        data.tweak.subpixel_binning = None;
        super::super::ligatures::use_measurement_face(&mut discoverer, data, &ctx);
        let chosen = Arc::clone(&discoverer.chosen.as_ref().unwrap().data);
        assert_eq!(chosen.tweak.subpixel_binning, None);
        pass(&ctx);
        assert!(super::super::ligatures::ligature_family(&discoverer, &ctx).is_some());
        ctx.fonts(|fonts| {
            let definitions = fonts.definitions();
            assert!(Arc::ptr_eq(
                definitions
                    .font_data
                    .get(&discoverer.chosen.as_ref().unwrap().family)
                    .unwrap(),
                &chosen
            ));
            assert_eq!(
                definitions
                    .font_data
                    .get(super::super::ligatures::DATA)
                    .unwrap()
                    .tweak
                    .subpixel_binning,
                Some(false)
            );
            assert_eq!(
                definitions
                    .families
                    .get(&egui::FontFamily::Name(
                        super::super::ligatures::FAMILY.into()
                    ))
                    .unwrap(),
                &[String::from(super::super::ligatures::DATA)]
            );
        });
    }

    #[test]
    fn equivalent_preparation_preserves_real_shaping_when_the_engine_reuses_definitions() {
        let ctx = egui::Context::default();
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: false,
            ..Default::default()
        });
        collection.register_fonts(
            fontique::Blob::new(Arc::new(
                include_bytes!("../../tests/fixtures/ligatures/JetBrainsMono-Regular.ttf").to_vec(),
            )),
            None,
        );
        let mut discoverer = Discoverer {
            collection: Some(collection),
            ..Default::default()
        };
        let mut screen = term_core::Screen::new(term_core::Size { rows: 1, cols: 24 });
        screen.advance(b"!= => ===\x1b[?25l");
        // Different requested aliases resolve here to the same canonical face and fresh bytes.
        // Prepare twice to pin that handoff independently of the host's installed aliases.
        for _ in 0..2 {
            let prepared =
                PreparedSelection::prepare(&mut discoverer, Some("JetBrains Mono")).unwrap();
            prepared.queue(&discoverer, &ctx);
            pass(&ctx);
            prepared.activate(&mut discoverer);
            let before = discoverer.ligature_cost();
            let output = ctx.run_ui(Default::default(), |ui| {
                let metrics =
                    crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(14.7));
                crate::painter::paint_with_ligatures(
                    ui.painter(),
                    &screen.view(),
                    &crate::theme::Theme::default(),
                    &metrics,
                    egui::Pos2::ZERO,
                    &mut discoverer,
                    crate::painter::CursorInk::Outline,
                    true,
                );
            });
            output.drop_without_applying_deltas();
            let after = discoverer.ligature_cost();
            assert_eq!(
                after.shaped_runs - before.shaped_runs,
                3,
                "equivalent canonical selection stopped drawing real shaped runs"
            );
            assert_eq!(after.fallback_runs, before.fallback_runs);
        }
    }

    #[test]
    fn draft_preview_survives_recomposition_and_clear_preserves_the_latest_live_font() {
        let ctx = egui::Context::default();
        pass(&ctx);
        let mut fonts = fixture_discoverer();
        fonts.use_the_family("Hack", &ctx);
        pass(&ctx);
        let live = appearance(&ctx, 'M', egui::FontFamily::Monospace);
        let preview = fonts.preview_font(Some("Ubuntu"), &ctx, true).unwrap();
        pass(&ctx);
        let sample = appearance(&ctx, 'M', preview.clone());
        assert_ne!(sample, live);
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), live);
        fonts.hand_over_the_faces(&ctx);
        pass(&ctx);
        assert_eq!(appearance(&ctx, 'M', preview.clone()), sample);
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), live);
        let pending = PreparedSelection::prepare(&mut fonts, Some("Ubuntu")).unwrap();
        pending.queue(&fonts, &ctx);
        pass(&ctx);
        pending.activate(&mut fonts);
        let latest = appearance(&ctx, 'M', egui::FontFamily::Monospace);
        assert_ne!(latest, live);
        assert_eq!(appearance(&ctx, 'M', preview), sample);
        fonts.clear_preview(&ctx);
        pass(&ctx);
        assert!(fonts.preview.is_none());
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), latest);
    }

    #[test]
    fn preparation_keeps_active_selection_readiness_and_glyphs_and_clear_stays_lazy() {
        let ctx = egui::Context::default();
        pass(&ctx);
        let mut discoverer = fixture_discoverer();
        assert_eq!(
            discoverer.use_the_family("Ubuntu", &ctx),
            Chosen::Registered("Ubuntu".into())
        );
        pass(&ctx);
        discoverer.icon_family_ready = true;
        discoverer.settled.insert('é');
        discoverer.pending.insert('Ω');
        let before = appearance(&ctx, 'M', egui::FontFamily::Monospace);
        let prepared = PreparedSelection::prepare(&mut discoverer, Some("Hack")).unwrap();
        assert_eq!(discoverer.chosen.as_ref().unwrap().family, "chosen:Ubuntu");
        assert!(discoverer.icon_family_ready);
        assert!(discoverer.settled.contains(&'é'));
        assert!(discoverer.pending.contains(&'Ω'));
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), before);
        pass(&ctx);
        assert_eq!(
            appearance(&ctx, 'M', egui::FontFamily::Monospace),
            before,
            "preparation must not queue a definition either"
        );
        drop(prepared);
        let mut untouched = Discoverer::default();
        PreparedSelection::prepare(&mut untouched, None).unwrap();
        assert!(!untouched.has_searched_the_machine());
        assert!(!untouched.no_system_fonts);
    }
    /// Keep the bundled Ubuntu outlines/metrics but give this fixture only M and one rare glyph.
    /// This creates real coverage gaps without depending on the host's installed fonts.
    fn sparse_fixture(name: &str, rare: char) -> fontique::Blob<u8> {
        let definitions = egui::FontDefinitions::default();
        let mut bytes = definitions.font_data["Ubuntu-Light"].font.to_vec();
        let glyph = skrifa::FontRef::new(&bytes)
            .unwrap()
            .charmap()
            .map('M')
            .unwrap()
            .to_u32() as u16;
        let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
        let record = (0..count)
            .map(|i| 12 + 16 * i)
            .find(|&offset| &bytes[offset..offset + 4] == b"cmap")
            .unwrap();
        let offset =
            u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
        // One Windows Unicode format-4 subtable, three singleton segments, direct glyph deltas.
        let words = [
            0u16,
            1,
            3,
            1,
            0,
            12,
            4,
            40,
            0,
            6,
            4,
            1,
            2,
            'M' as u16,
            rare as u16,
            0xffff,
            0,
            'M' as u16,
            rare as u16,
            0xffff,
            glyph.wrapping_sub('M' as u16),
            glyph.wrapping_sub(rare as u16),
            1,
            0,
            0,
            0,
        ];
        for (i, word) in words.into_iter().enumerate() {
            bytes[offset + 2 * i..offset + 2 * i + 2].copy_from_slice(&word.to_be_bytes());
        }
        let font = skrifa::FontRef::new(&bytes).unwrap();
        assert_eq!(
            font.charmap().map(rare).unwrap().to_u32(),
            glyph as u32,
            "{name}"
        );
        fontique::Blob::new(Arc::new(bytes))
    }

    fn add_fixture(discoverer: &mut Discoverer, name: &str, rare: char) {
        let added = discoverer.collection.as_mut().unwrap().register_fonts(
            sparse_fixture(name, rare),
            Some(fontique::FontInfoOverride {
                family_name: Some(name),
                ..Default::default()
            }),
        );
        assert_eq!(added.len(), 1);
    }

    #[test]
    fn selection_and_same_pass_discovered_icons_take_effect_together_then_reset_to_builtins() {
        let ctx = egui::Context::default();
        pass(&ctx);
        let mut discoverer = fixture_discoverer();
        let builtin = appearance(&ctx, 'M', egui::FontFamily::Monospace);
        assert_eq!(
            discoverer.use_the_family("Hack", &ctx),
            Chosen::Registered("Hack".into())
        );
        pass(&ctx);
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), builtin);
        add_fixture(&mut discoverer, "Fixture icons", '\u{E000}');
        let prepared = PreparedSelection::prepare(&mut discoverer, Some("Ubuntu")).unwrap();
        // Discovery is a competing same-pass writer, after preparation and before the final queue.
        discoverer.note('\u{E000}');
        let round = discoverer.resolve(&ctx);
        assert_eq!(round.registered, 1);
        assert!(!discoverer.icon_family_ready);
        prepared.queue(&discoverer, &ctx);
        assert_eq!(discoverer.chosen.as_ref().unwrap().family, "chosen:Hack");
        assert!(!discoverer.icon_family_ready);
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), builtin);
        pass(&ctx);
        prepared.activate(&mut discoverer);
        let chosen = appearance(&ctx, 'M', egui::FontFamily::Monospace);
        assert_ne!(
            chosen.0, builtin.0,
            "selected font must change actual metrics"
        );
        assert_ne!(
            chosen.1, builtin.1,
            "selected font must change raster bounds"
        );
        let icon = appearance(&ctx, '\u{E000}', discoverer.family_of('\u{E000}'));
        assert_eq!(
            icon, chosen,
            "the fallback fixture maps this icon to Ubuntu M"
        );
        assert!(discoverer.icon_family_ready);
        for _ in 0..3 {
            discoverer.note('\u{E000}');
            assert_eq!(discoverer.resolve(&ctx).searched, 0);
            pass(&ctx);
            assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), chosen);
            assert_eq!(
                appearance(&ctx, '\u{E000}', discoverer.family_of('\u{E000}')),
                icon
            );
        }
        let clear = PreparedSelection::prepare(&mut discoverer, None).unwrap();
        clear.queue(&discoverer, &ctx);
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), chosen);
        pass(&ctx);
        clear.activate(&mut discoverer);
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), builtin);
        assert_eq!(
            appearance(&ctx, '\u{E000}', discoverer.family_of('\u{E000}')),
            icon,
            "clearing selected text must retain discovered icons"
        );
    }

    #[test]
    fn clearing_reconsiders_glyphs_previously_covered_by_the_selected_family() {
        let ctx = egui::Context::default();
        pass(&ctx);
        let mut discoverer = fixture_discoverer();
        add_fixture(&mut discoverer, "Fixture selected", '日');
        let prepared =
            PreparedSelection::prepare(&mut discoverer, Some("Fixture selected")).unwrap();
        prepared.queue(&discoverer, &ctx);
        pass(&ctx);
        prepared.activate(&mut discoverer);
        discoverer.note('日');
        assert_eq!(discoverer.resolve(&ctx).searched, 0);
        assert!(discoverer.settled.contains(&'日'));
        let covered = appearance(&ctx, '日', egui::FontFamily::Monospace);
        assert_ne!(
            covered,
            appearance(&ctx, REPLACEMENT_BOX, egui::FontFamily::Monospace)
        );
        discoverer.failures_in_a_row = MAX_FAILURES_IN_A_ROW;
        discoverer.note('Ω');
        let clear = PreparedSelection::prepare(&mut discoverer, None).unwrap();
        clear.queue(&discoverer, &ctx);
        pass(&ctx);
        clear.activate(&mut discoverer);
        assert!(discoverer.pending.contains(&'Ω'));
        assert_eq!(
            appearance(&ctx, '日', egui::FontFamily::Monospace),
            appearance(&ctx, REPLACEMENT_BOX, egui::FontFamily::Monospace)
        );
        discoverer.note('日');
        let round = discoverer.resolve(&ctx);
        assert_eq!(
            round.registered, 1,
            "old selected-family coverage or failure brake hid the missing glyph"
        );
        pass(&ctx);
        assert_eq!(appearance(&ctx, '日', egui::FontFamily::Monospace), covered);
    }

    #[test]
    fn missing_and_unavailable_selection_fail_without_changing_active_glyphs() {
        let ctx = egui::Context::default();
        pass(&ctx);
        let mut discoverer = fixture_discoverer();
        discoverer.use_the_family("Ubuntu", &ctx);
        pass(&ctx);
        let before = appearance(&ctx, 'M', egui::FontFamily::Monospace);
        assert!(matches!(
            PreparedSelection::prepare(&mut discoverer, Some("Missing fixture")),
            Err(SelectionError::NotInstalled)
        ));
        assert_eq!(discoverer.chosen.as_ref().unwrap().family, "chosen:Ubuntu");
        pass(&ctx);
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), before);
        // Keep a registered family whose only face disappeared from its catalogue.
        // It is installed, but has no usable assets; this must not become NotInstalled.
        add_fixture(&mut discoverer, "Fixture unavailable", '日');
        let collection = discoverer.collection.as_mut().unwrap();
        let family = collection.family_id("Fixture unavailable").unwrap();
        let face = collection.family(family).unwrap().fonts()[0].clone();
        assert!(collection.unregister_font(family, face.width(), face.style(), face.weight()));
        assert!(matches!(
            PreparedSelection::prepare(&mut discoverer, Some("Fixture unavailable")),
            Err(SelectionError::Unusable)
        ));
        assert_eq!(discoverer.chosen.as_ref().unwrap().family, "chosen:Ubuntu");
        pass(&ctx);
        assert_eq!(appearance(&ctx, 'M', egui::FontFamily::Monospace), before);
        discoverer.no_system_fonts = true;
        assert!(matches!(
            PreparedSelection::prepare(&mut discoverer, Some("Ubuntu")),
            Err(SelectionError::NoFontsOnTheMachine)
        ));
        let clear = PreparedSelection::prepare(&mut discoverer, None).unwrap();
        clear.queue(&discoverer, &ctx);
        pass(&ctx);
        clear.activate(&mut discoverer);
        assert!(discoverer.chosen.is_none());
        assert_ne!(appearance(&ctx, 'M', egui::FontFamily::Monospace), before);
    }
}
