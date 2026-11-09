//! Lossless patches to the shared, hand-edited configuration.
use super::{ConfigLine, Settings, apply, classify};
use crate::keyboard::{ALL, Action, Chord, Keymap};

#[derive(Clone)]
pub(crate) struct Field {
    pub key: String,
    pub value: String,
    original: String,
}

pub(crate) struct Document {
    source: String,
    pub fields: Vec<Field>,
    defaults: bool,
}

/// Semantic spelling, including aliases the startup parser already accepts.
fn canonical(key: &str) -> Option<String> {
    if let Some(name) = key.strip_prefix("key.") {
        if name.eq_ignore_ascii_case("copy") {
            return Some("key.copy".into());
        }
        return Action::from_written(name).map(|a| format!("key.{}", a.as_written()));
    }
    if let Some(index) = key.strip_prefix("color.") {
        if !index.is_empty() && index.chars().all(|c| c.is_ascii_digit()) {
            return index
                .parse::<usize>()
                .ok()
                .filter(|n| *n < 16)
                .map(|n| format!("color.{n}"));
        }
        return None;
    }
    [
        "font",
        "font.ligatures",
        "size",
        "pane.title_size",
        "tab.label_size",
        "theme",
        "cursor.blink",
        "background",
        "foreground",
        "cursor",
        "divider",
        "focus",
        "panel",
    ]
    .contains(&key)
    .then(|| key.to_owned())
}

impl Document {
    pub fn new(source: String) -> Self {
        let mut keys: Vec<String> = [
            "font",
            "font.ligatures",
            "size",
            "pane.title_size",
            "tab.label_size",
            "theme",
            "cursor.blink",
            "background",
            "foreground",
            "cursor",
            "divider",
            "focus",
            "panel",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        keys.extend((0..16).map(|n| format!("color.{n}")));
        keys.extend(ALL.iter().map(|a| format!("key.{}", a.as_written())));
        keys.push("key.copy".into());
        let mut fields: Vec<Field> = keys
            .into_iter()
            .map(|key| Field {
                key,
                value: String::new(),
                original: String::new(),
            })
            .collect();
        for line in source.strip_prefix('\u{feff}').unwrap_or(&source).lines() {
            if let ConfigLine::Pair(key, value) = classify(line)
                && apply(&mut Settings::default(), key, value).is_ok()
                && let Some(key) = canonical(key)
                && let Some(field) = fields.iter_mut().find(|f| f.key == key)
            {
                let displayed =
                    if matches!(key.as_str(), "theme" | "cursor.blink" | "font.ligatures")
                        || (key.starts_with("key.") && value.eq_ignore_ascii_case("none"))
                    {
                        value.to_ascii_lowercase()
                    } else {
                        value.to_owned()
                    };
                field.value = displayed.clone();
                field.original = displayed;
            }
        }
        Self {
            source,
            fields,
            defaults: false,
        }
    }

    pub fn defaults(&mut self) {
        self.defaults = true;
        for field in &mut self.fields {
            field.value.clear();
        }
    }

    pub fn errors(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        let mut bindings = Vec::new();
        let defaults = Keymap::default();
        for field in &self.fields {
            if !field.value.is_empty() {
                if field.value.contains(['\n', '\r']) {
                    errors.push((field.key.clone(), "Use one line.".into()));
                } else if let Err(problem) =
                    apply(&mut Settings::default(), &field.key, &field.value)
                {
                    errors.push((
                        field.key.clone(),
                        super::Warning {
                            line: 1,
                            missing: problem,
                        }
                        .to_string(),
                    ));
                }
            }
            if let Some(name) = field.key.strip_prefix("key.")
                && let Some(action) = Action::from_written(name)
            {
                let chord = if field.value.is_empty() {
                    defaults.chord_for(action)
                } else if field.value.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Chord::from_written(&field.value)
                };
                if let Some(chord) = chord {
                    bindings.push((action, chord));
                }
            }
        }
        for (index, (action, chord)) in bindings.iter().enumerate() {
            for (other, other_chord) in &bindings[index + 1..] {
                if chord == other_chord {
                    errors.push((
                        format!("key.{}", action.as_written()),
                        format!("Also assigned to {}.", other.as_written()),
                    ));
                    errors.push((
                        format!("key.{}", other.as_written()),
                        format!("Also assigned to {}.", action.as_written()),
                    ));
                }
            }
        }
        errors
    }

    pub fn text(&self) -> String {
        let bom = self.source.starts_with('\u{feff}');
        let body = self.source.strip_prefix('\u{feff}').unwrap_or(&self.source);
        let mut result = if bom {
            "\u{feff}".into()
        } else {
            String::new()
        };
        let dirty: Vec<_> = self
            .fields
            .iter()
            .filter(|f| self.defaults || f.value != f.original)
            .collect();
        for line in body.split_inclusive('\n') {
            // Empty values are assignments too: removing an override must not resurrect them.
            let key = line
                .trim()
                .split_once('=')
                .and_then(|(k, _)| canonical(k.trim()));
            if key
                .as_ref()
                .is_some_and(|key| dirty.iter().any(|f| &f.key == key))
            {
                continue;
            }
            result.push_str(line);
        }
        let ending = if self.source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        if body.trim().is_empty() {
            if !result.is_empty() && !result.ends_with('\n') && result != "\u{feff}" {
                result.push_str(ending);
            }
            result.push_str(&format!("version={}{ending}", super::VERSION));
        }
        for field in dirty.into_iter().filter(|f| !f.value.is_empty()) {
            if !result.is_empty() && !result.ends_with('\n') && result != "\u{feff}" {
                result.push_str(ending);
            }
            result.push_str(&format!("{} = {}{}", field.key, field.value, ending));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn set(doc: &mut Document, key: &str, value: &str) {
        doc.fields.iter_mut().find(|f| f.key == key).unwrap().value = value.into();
    }
    #[test]
    fn title_sizes_preserve_other_lines_and_reset_independently() {
        let source = "# keep\npane.title_size = 14\ntab.label_size = 10\nunknown = value\n";
        let mut doc = Document::new(source.into());
        assert_eq!(doc.text(), source);
        set(&mut doc, "pane.title_size", "24");
        let text = doc.text();
        assert!(text.contains("# keep\n"));
        assert!(text.contains("tab.label_size = 10\n"));
        assert!(text.contains("unknown = value\n"));
        let (settings, warnings) = super::super::interpret(&text).unwrap();
        assert_eq!(settings.pane_title_size, Some(24.0));
        assert_eq!(settings.tab_label_size, Some(10.0));
        assert_eq!(warnings.kept.len(), 1);
        set(&mut doc, "pane.title_size", "");
        let (settings, _) = super::super::interpret(&doc.text()).unwrap();
        assert_eq!(settings.pane_title_size, None);
        assert_eq!(settings.tab_label_size, Some(10.0));
    }

    #[test]
    fn ligature_edits_preserve_comments_unknown_keys_and_restore_the_off_default() {
        let source =
            "# keep\r\nfont.ligatures=ON\r\nfont.ligatures=invalid\r\nfuture=yes\r\nsize=19\r\n";
        let mut doc = Document::new(source.into());
        assert_eq!(doc.text(), source);
        assert_eq!(
            doc.fields
                .iter()
                .find(|f| f.key == "font.ligatures")
                .unwrap()
                .value,
            "on"
        );
        set(&mut doc, "font.ligatures", "off");
        let text = doc.text();
        assert_eq!(
            text,
            "# keep\r\nfuture=yes\r\nsize=19\r\nfont.ligatures = off\r\n"
        );
        assert_eq!(
            super::super::interpret(&text).unwrap().0.ligatures,
            Some(false)
        );
        set(&mut doc, "font.ligatures", "maybe");
        assert!(doc.errors().iter().any(|(key, _)| key == "font.ligatures"));
        doc.defaults();
        assert_eq!(doc.text(), "# keep\r\nfuture=yes\r\n");
        assert_eq!(
            super::super::interpret(&doc.text()).unwrap().0.ligatures,
            None
        );
    }

    #[test]
    fn unchanged_bytes_and_dirty_aliases_are_distinct() {
        let source =
            "\u{feff}# keep\r\ncolor.01 = #111111\r\ncolor.1 = bad\r\nfuture = yes\r\nsize = 15";
        let mut doc = Document::new(source.into());
        assert_eq!(doc.text(), source);
        set(&mut doc, "color.1", "#abcdef");
        let text = doc.text();
        assert!(text.starts_with("\u{feff}# keep\r\nfuture = yes\r\nsize = 15\r\n"));
        assert!(!text.contains("color.01"));
        assert_eq!(
            super::super::interpret(&text).unwrap().0.colors.palette[1],
            Some(eframe::egui::Color32::from_rgb(171, 205, 239))
        );
    }
    #[test]
    fn defaults_remove_invalid_assignments_and_copy_aliases() {
        let mut doc = Document::new(
            "# keep\nkey.COPY = none\nkey.copy = bad\nsize =\nfont = abc\nfuture=x\nversion=1\n"
                .into(),
        );
        doc.defaults();
        assert_eq!(doc.text(), "# keep\nfuture=x\nversion=1\n");
        assert_eq!(
            super::super::interpret(&doc.text()).unwrap().0,
            Settings::default()
        );
    }
    #[test]
    fn validation_sees_defaults_conflicts_and_invalid_values() {
        let mut doc = Document::new(String::new());
        set(&mut doc, "key.new-tab", "ctrl+shift+d");
        assert!(doc.errors().iter().any(|(key, _)| key == "key.split-right"));
        set(&mut doc, "key.split-right", "none");
        assert!(doc.errors().is_empty());
        for (key, value) in [
            ("size", "97"),
            ("font", "x\nsize=20"),
            ("background", "red"),
            ("key.copy", "on"),
        ] {
            set(&mut doc, key, value);
            assert!(doc.errors().iter().any(|(k, _)| k == key));
            set(&mut doc, key, "");
        }
    }
    #[test]
    fn case_insensitive_choices_are_selected_without_rewriting_the_file() {
        let source = "theme=TokyoNight\ncursor.blink=ON\nkey.copy=NONE\nkey.new-tab=NoNe\n";
        let doc = Document::new(source.into());
        for (key, expected) in [
            ("theme", "tokyonight"),
            ("cursor.blink", "on"),
            ("key.copy", "none"),
            ("key.new-tab", "none"),
        ] {
            assert_eq!(
                doc.fields.iter().find(|f| f.key == key).unwrap().value,
                expected
            );
        }
        assert_eq!(doc.text(), source);
    }

    #[test]
    fn a_new_or_blank_saved_configuration_declares_its_format_version() {
        for source in ["", "\n", "\u{feff}"] {
            let doc = Document::new(source.into());
            let text = doc.text();
            assert!(
                text.trim_start_matches('\u{feff}')
                    .lines()
                    .any(|l| l == "version=1")
            );
            assert_eq!(
                super::super::interpret(&text).unwrap().0,
                Settings::default()
            );
        }
    }

    #[test]
    fn only_the_single_header_mark_is_removed_like_the_startup_reader() {
        let source = "\u{feff}\u{feff}size=18\n# keep\n";
        assert_eq!(super::super::interpret(source).unwrap().0.font_size, None);
        let mut doc = Document::new(source.into());
        assert!(
            doc.fields
                .iter()
                .find(|f| f.key == "size")
                .unwrap()
                .value
                .is_empty()
        );
        assert_eq!(doc.text(), source);
        set(&mut doc, "foreground", "#abcdef");
        assert!(doc.text().starts_with(source));
        assert_eq!(
            super::super::interpret(&doc.text()).unwrap().0.font_size,
            None
        );
    }

    #[test]
    fn saved_values_round_trip_without_runtime_application() {
        let mut doc = Document::new(String::new());
        for (key, value) in [
            ("size", "18"),
            ("cursor.blink", "off"),
            ("theme", "tokyonight"),
            ("key.copy", "none"),
        ] {
            set(&mut doc, key, value);
        }
        assert!(doc.errors().is_empty());
        let saved = super::super::interpret(&doc.text()).unwrap().0;
        assert_eq!(saved.font_size, Some(18.0));
        assert_eq!(saved.blink, Some(false));
        assert_eq!(saved.copy, Some(false));
        assert!(saved.theme.is_some());
    }

    #[test]
    fn every_appearance_override_survives_both_theme_bases_and_removal() {
        use eframe::egui::Color32;
        use term_core::TermColor;
        for (name, named) in [
            ("tokyonight", crate::theme::NamedTheme::TokyoNight),
            ("gruvbox-dark", crate::theme::NamedTheme::GruvboxDark),
        ] {
            let mut doc = Document::new(String::new());
            set(&mut doc, "theme", name);
            set(&mut doc, "font", "A Family With Spaces");
            let own = [
                "background",
                "foreground",
                "cursor",
                "divider",
                "focus",
                "panel",
            ];
            for (index, key) in (0..16)
                .map(|n| format!("color.{n}"))
                .chain(own.map(str::to_owned))
                .enumerate()
            {
                set(&mut doc, &key, &format!("#{:06x}", 0x102030 + index));
            }
            assert!(doc.errors().is_empty());
            let saved = super::super::interpret(&doc.text()).unwrap().0;
            assert_eq!(saved.font.as_deref(), Some("A Family With Spaces"));
            let theme = saved.resolved_theme();
            for index in 0..16 {
                assert_eq!(
                    theme.color(TermColor::Indexed(index)),
                    Color32::from_rgb(0x10, 0x20, 0x30 + index)
                );
            }
            for (index, actual) in [
                theme.background(),
                theme.foreground(),
                theme.color(TermColor::Cursor),
                theme.divider(),
                theme.focus(),
                theme.panel_background(),
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(actual, Color32::from_rgb(0x10, 0x20, 0x40 + index as u8));
            }
            let mut reopened = Document::new(doc.text());
            for key in (0..16)
                .map(|n| format!("color.{n}"))
                .chain(own.map(str::to_owned))
            {
                set(&mut reopened, &key, "");
            }
            let reset = super::super::interpret(&reopened.text()).unwrap().0;
            assert_eq!(reset.colors, super::super::Colors::default());
            assert_eq!(reset.theme, Some(named));
            assert_eq!(
                reset.resolved_theme().background(),
                named.theme().background()
            );
        }
    }

    #[test]
    fn every_action_can_be_rebound_unbound_and_restored_without_changing_copy() {
        for action in ALL {
            let key = format!("key.{}", action.as_written());
            let mut doc = Document::new("key.copy=none\n".into());
            set(&mut doc, &key, "alt+F12");
            assert!(doc.errors().is_empty());
            let saved = super::super::interpret(&doc.text()).unwrap().0;
            let mut keymap = Keymap::default();
            for (action, chord) in saved.shortcuts {
                keymap.bind(action, chord);
            }
            assert_eq!(
                keymap.action_for(Chord::from_written("alt+F12").unwrap()),
                Some(action)
            );
            assert_eq!(saved.copy, Some(false));
            set(&mut doc, &key, "none");
            let saved = super::super::interpret(&doc.text()).unwrap().0;
            let mut keymap = Keymap::default();
            for (action, chord) in saved.shortcuts {
                keymap.bind(action, chord);
            }
            assert_eq!(keymap.chord_for(action), None);
            let mut reopened = Document::new(doc.text());
            set(&mut reopened, &key, "");
            set(&mut reopened, "key.copy", "");
            let restored = super::super::interpret(&reopened.text()).unwrap().0;
            assert!(restored.shortcuts.is_empty());
            assert_eq!(restored.copy, None);
        }
    }

    #[test]
    fn cursor_inheritance_and_font_size_boundaries_round_trip() {
        let mut doc = Document::new("cursor.blink=off\nsize=18\n".into());
        for (value, expected) in [("on", Some(true)), ("off", Some(false)), ("", None)] {
            set(&mut doc, "cursor.blink", value);
            assert_eq!(
                super::super::interpret(&doc.text()).unwrap().0.blink,
                expected
            );
        }
        for value in ["4", "96"] {
            set(&mut doc, "size", value);
            assert!(doc.errors().is_empty());
            assert_eq!(
                super::super::interpret(&doc.text()).unwrap().0.font_size,
                value.parse().ok()
            );
        }
        for value in ["3.99", "96.01", "NaN", "inf"] {
            set(&mut doc, "size", value);
            assert!(doc.errors().iter().any(|(key, _)| key == "size"));
        }
    }
}
