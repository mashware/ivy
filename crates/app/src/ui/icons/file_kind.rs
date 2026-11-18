//! What a file in the Files tree is, read off its name, and the glyph and colour it is shown with —
//! the way an editor's file-icon theme tells a Rust file from a lock file at a glance (D103).
use crate::theme::{Roles, Theme, project_ink_contrast};
use eframe::egui::Color32;
use egui_phosphor::regular as glyph;
use std::path::Path;

/// The contrast a type's colour must keep against the row it sits on. An icon is a non-text mark,
/// so WCAG's 3:1 for graphical objects (1.4.11) applies, as for the focus border (`green_focus`).
const MARK_CONTRAST: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileKind {
    Lock,
    Dotfile,
    Rust,
    Markdown,
    Json,
    Config,
    Image,
    Svg,
    Js,
    Jsx,
    Ts,
    Tsx,
    Python,
    C,
    Cpp,
    Html,
    Css,
    Sql,
    Code,
    Pdf,
    Archive,
    Text,
    Plain,
}

/// Where a kind's colour comes from: a terminal palette slot, so every theme carries it (D94), or
/// one of the interface's text roles.
enum Tint {
    Ansi(u8),
    Muted,
    Secondary,
}

impl FileKind {
    /// The kind of the file at `path`, by its name alone — the tree never opens a file to decide.
    /// The first rule that matches wins, which is what puts `package-lock.json` among the lock files
    /// and `.eslintrc.json` among the dotfiles.
    pub(crate) fn of(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let extension = Path::new(&name)
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        if extension == "lock" || name == "package-lock.json" || name == "pnpm-lock.yaml" {
            return Self::Lock;
        }
        if name.starts_with('.') {
            return Self::Dotfile;
        }
        match extension.as_str() {
            "rs" => Self::Rust,
            "md" | "markdown" => Self::Markdown,
            "json" | "jsonc" => Self::Json,
            "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" => Self::Config,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => Self::Image,
            "svg" => Self::Svg,
            "js" => Self::Js,
            "jsx" => Self::Jsx,
            "ts" => Self::Ts,
            "tsx" => Self::Tsx,
            "py" => Self::Python,
            "c" => Self::C,
            "cpp" => Self::Cpp,
            "html" => Self::Html,
            "css" => Self::Css,
            "sql" => Self::Sql,
            "h" | "hpp" | "sh" => Self::Code,
            "pdf" => Self::Pdf,
            "zip" | "tar" | "gz" | "tgz" | "xz" | "7z" => Self::Archive,
            "txt" | "log" => Self::Text,
            _ => Self::Plain,
        }
    }

    pub(super) fn glyph(self) -> &'static str {
        match self {
            Self::Lock => glyph::FILE_LOCK,
            Self::Dotfile => glyph::GEAR_SIX,
            Self::Rust => glyph::FILE_RS,
            Self::Markdown => glyph::FILE_MD,
            Self::Json => glyph::BRACKETS_CURLY,
            Self::Config => glyph::FILE_INI,
            Self::Image => glyph::FILE_IMAGE,
            Self::Svg => glyph::FILE_SVG,
            Self::Js => glyph::FILE_JS,
            Self::Jsx => glyph::FILE_JSX,
            Self::Ts => glyph::FILE_TS,
            Self::Tsx => glyph::FILE_TSX,
            Self::Python => glyph::FILE_PY,
            Self::C => glyph::FILE_C,
            Self::Cpp => glyph::FILE_CPP,
            Self::Html => glyph::FILE_HTML,
            Self::Css => glyph::FILE_CSS,
            Self::Sql => glyph::FILE_SQL,
            Self::Code => glyph::FILE_CODE,
            Self::Pdf => glyph::FILE_PDF,
            Self::Archive => glyph::FILE_ZIP,
            Self::Text => glyph::FILE_TEXT,
            Self::Plain => glyph::FILE,
        }
    }

    fn tint(self) -> Tint {
        match self {
            Self::Lock | Self::Dotfile => Tint::Muted,
            Self::Rust | Self::Pdf => Tint::Ansi(1),
            Self::Markdown => Tint::Ansi(4),
            Self::Json => Tint::Ansi(3),
            Self::Config => Tint::Ansi(6),
            Self::Image | Self::Svg => Tint::Ansi(5),
            Self::Js
            | Self::Jsx
            | Self::Ts
            | Self::Tsx
            | Self::Python
            | Self::C
            | Self::Cpp
            | Self::Html
            | Self::Css
            | Self::Sql
            | Self::Code => Tint::Ansi(2),
            Self::Archive | Self::Text | Self::Plain => Tint::Secondary,
        }
    }

    /// The colour this kind is painted in on a Files row: its own, unless that would not read on the
    /// panel, on a hovered row or on the chosen one — the last two are translucent and are judged
    /// as they land on the panel — and then the secondary text colour every file used to have.
    pub(crate) fn color(self, theme: &Theme, roles: &Roles) -> Color32 {
        let wanted = match self.tint() {
            Tint::Ansi(slot) => theme.color(term_core::TermColor::Indexed(slot)),
            Tint::Muted => roles.tx3,
            Tint::Secondary => return roles.tx2,
        };
        let grounds = [
            roles.panel,
            roles.panel.blend(roles.list_hover),
            roles.panel.blend(roles.list_sel),
        ];
        if grounds
            .iter()
            .all(|ground| project_ink_contrast(wanted, *ground) >= MARK_CONTRAST)
        {
            wanted
        } else {
            roles.tx2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::NamedTheme;

    /// `theme` with palette slot `slot` set to `color`, the way a settings file would set it.
    fn with_slot(theme: Theme, slot: usize, color: Color32) -> Theme {
        let mut colors = crate::settings::Colors::default();
        colors.palette[slot] = Some(color);
        theme.with_colors(&colors)
    }

    #[test]
    fn a_file_is_known_by_its_name() {
        for (name, kind, shown) in [
            ("main.rs", FileKind::Rust, glyph::FILE_RS),
            ("Cargo.lock", FileKind::Lock, glyph::FILE_LOCK),
            ("package-lock.json", FileKind::Lock, glyph::FILE_LOCK),
            ("README.md", FileKind::Markdown, glyph::FILE_MD),
            (".gitignore", FileKind::Dotfile, glyph::GEAR_SIX),
            (".eslintrc.json", FileKind::Dotfile, glyph::GEAR_SIX),
            ("panel.json", FileKind::Json, glyph::BRACKETS_CURLY),
            ("Cargo.toml", FileKind::Config, glyph::FILE_INI),
            ("ci.YML", FileKind::Config, glyph::FILE_INI),
            ("logo.PNG", FileKind::Image, glyph::FILE_IMAGE),
            ("icon.svg", FileKind::Svg, glyph::FILE_SVG),
            ("app.tsx", FileKind::Tsx, glyph::FILE_TSX),
            ("build.sh", FileKind::Code, glyph::FILE_CODE),
            ("guide.pdf", FileKind::Pdf, glyph::FILE_PDF),
            ("dump.tar.gz", FileKind::Archive, glyph::FILE_ZIP),
            ("notes.txt", FileKind::Text, glyph::FILE_TEXT),
            ("notes.xyz", FileKind::Plain, glyph::FILE),
            ("Makefile", FileKind::Plain, glyph::FILE),
        ] {
            let found = FileKind::of(Path::new("/work/repo").join(name).as_path());
            assert_eq!(found, kind, "{name}");
            assert_eq!(found.glyph(), shown, "{name}");
        }
    }

    #[test]
    fn a_type_keeps_its_own_colour_where_it_reads() {
        let theme = NamedTheme::IvyNight.theme();
        let roles = Roles::of(&theme);
        let red = theme.color(term_core::TermColor::Indexed(1));
        assert_eq!(FileKind::Rust.color(&theme, &roles), red);
        assert_ne!(red, roles.tx2);
        assert_eq!(FileKind::Plain.color(&theme, &roles), roles.tx2);
    }

    /// A type colour too close to the panel falls back to the secondary text colour.
    #[test]
    fn a_type_colour_lost_in_the_panel_falls_back() {
        let theme = NamedTheme::IvyDay.theme();
        let roles = Roles::of(&theme);
        let theme = with_slot(theme, 3, roles.panel);
        let roles = Roles::of(&theme);
        assert_eq!(FileKind::Json.color(&theme, &roles), roles.tx2);
    }

    /// A colour that reads on the panel but not on the chosen row, as the chosen row lands on it,
    /// falls back too: the translucent selection is judged blended, not as stored.
    #[test]
    fn a_type_colour_lost_on_the_chosen_row_falls_back() {
        let theme = NamedTheme::IvyNight.theme();
        let roles = Roles::of(&theme);
        let chosen = roles.panel.blend(roles.list_sel);
        let between = (0..=255u8)
            .flat_map(|r| {
                (0..=255u8)
                    .step_by(5)
                    .map(move |g| Color32::from_rgb(r, g, 0))
            })
            .find(|c| {
                project_ink_contrast(*c, roles.panel) >= MARK_CONTRAST
                    && project_ink_contrast(*c, chosen) < MARK_CONTRAST
            })
            .expect("the chosen row is lighter than the panel, so such a colour exists");
        let theme = with_slot(theme, 1, between);
        let roles = Roles::of(&theme);
        assert_eq!(FileKind::Rust.color(&theme, &roles), roles.tx2);
    }

    /// In every bundled theme, every type is painted in a colour that reads on all three grounds —
    /// or in the secondary text colour.
    #[test]
    fn every_bundled_theme_paints_every_type_legibly() {
        for named in NamedTheme::ALL {
            let theme = named.theme();
            let roles = Roles::of(&theme);
            let grounds = [
                roles.panel,
                roles.panel.blend(roles.list_hover),
                roles.panel.blend(roles.list_sel),
            ];
            for name in [
                "a.rs",
                "a.md",
                "a.json",
                "a.toml",
                "a.png",
                "a.ts",
                "a.pdf",
                "Cargo.lock",
            ] {
                let color = FileKind::of(Path::new(name)).color(&theme, &roles);
                assert!(
                    color == roles.tx2
                        || grounds
                            .iter()
                            .all(|g| project_ink_contrast(color, *g) >= MARK_CONTRAST),
                    "{named:?} {name}"
                );
            }
        }
    }
}
