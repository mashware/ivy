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

/// The surfaces a list row can show under its icon: the list's own, and its hovered and chosen
/// fills as they land on it — a translucent fill is judged blended, never as stored.
pub(crate) fn row_grounds(surface: Color32, hover: Color32, chosen: Color32) -> [Color32; 3] {
    [surface, surface.blend(hover), surface.blend(chosen)]
}

/// `wanted` where it keeps 3:1 against every ground, otherwise `fallback`.
pub(crate) fn legible_or(wanted: Color32, grounds: &[Color32; 3], fallback: Color32) -> Color32 {
    if grounds
        .iter()
        .all(|ground| project_ink_contrast(wanted, *ground) >= MARK_CONTRAST)
    {
        wanted
    } else {
        fallback
    }
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

    /// The colour this kind is painted in over `grounds` — every surface its row can show under
    /// it, translucent fills already blended onto what they land on: its own, unless that would not
    /// read on one of them, and then the secondary text colour every file used to have.
    pub(crate) fn color(self, theme: &Theme, roles: &Roles, grounds: &[Color32; 3]) -> Color32 {
        let wanted = match self.tint() {
            Tint::Ansi(slot) => theme.color(term_core::TermColor::Indexed(slot)),
            Tint::Muted => roles.tx3,
            Tint::Secondary => return roles.tx2,
        };
        legible_or(wanted, grounds, roles.tx2)
    }
}

#[cfg(test)]
impl FileKind {
    /// Every kind. The match in `listed` makes a new kind need an arm there, which is the reminder
    /// to list it here too; nothing checks that it was.
    pub(crate) const ALL: [Self; 23] = [
        Self::Lock,
        Self::Dotfile,
        Self::Rust,
        Self::Markdown,
        Self::Json,
        Self::Config,
        Self::Image,
        Self::Svg,
        Self::Js,
        Self::Jsx,
        Self::Ts,
        Self::Tsx,
        Self::Python,
        Self::C,
        Self::Cpp,
        Self::Html,
        Self::Css,
        Self::Sql,
        Self::Code,
        Self::Pdf,
        Self::Archive,
        Self::Text,
        Self::Plain,
    ];

    fn listed(self) -> usize {
        match self {
            Self::Lock => 0,
            Self::Dotfile => 1,
            Self::Rust => 2,
            Self::Markdown => 3,
            Self::Json => 4,
            Self::Config => 5,
            Self::Image => 6,
            Self::Svg => 7,
            Self::Js => 8,
            Self::Jsx => 9,
            Self::Ts => 10,
            Self::Tsx => 11,
            Self::Python => 12,
            Self::C => 13,
            Self::Cpp => 14,
            Self::Html => 15,
            Self::Css => 16,
            Self::Sql => 17,
            Self::Code => 18,
            Self::Pdf => 19,
            Self::Archive => 20,
            Self::Text => 21,
            Self::Plain => 22,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::NamedTheme;

    #[test]
    fn the_list_of_kinds_is_whole() {
        for (index, kind) in FileKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.listed(), index, "{kind:?}");
        }
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

    /// Each kind's own colour, on a ground every one of them reads on: the table of D103, pinned
    /// entry by entry — Ivy Night's palette slots are distinct, so a kind moved to another slot or
    /// role fails here.
    #[test]
    fn each_kind_is_painted_in_its_own_colour_where_it_reads() {
        let theme = NamedTheme::IvyNight.theme();
        let roles = Roles::of(&theme);
        let slot = |n| theme.color(term_core::TermColor::Indexed(n));
        let ground = [Color32::BLACK; 3];
        for kind in FileKind::ALL {
            let expected = match kind {
                FileKind::Rust | FileKind::Pdf => slot(1),
                FileKind::Markdown => slot(4),
                FileKind::Json => slot(3),
                FileKind::Config => slot(6),
                FileKind::Image | FileKind::Svg => slot(5),
                FileKind::Lock | FileKind::Dotfile => roles.tx3,
                FileKind::Archive | FileKind::Text | FileKind::Plain => roles.tx2,
                _ => slot(2),
            };
            assert_eq!(kind.color(&theme, &roles, &ground), expected, "{kind:?}");
        }
        let distinct: std::collections::HashSet<_> = [1, 2, 3, 4, 5, 6]
            .into_iter()
            .map(slot)
            .chain([roles.tx2, roles.tx3])
            .collect();
        assert_eq!(
            distinct.len(),
            8,
            "the slots must differ for the table to be pinned"
        );
    }

    /// The threshold is 3:1 — the grey one step under it falls back and the grey one step over it
    /// is kept — and it is held on every ground given: one ground it fails on is enough.
    #[test]
    fn a_colour_under_three_to_one_on_any_ground_falls_back() {
        assert_eq!(MARK_CONTRAST, 3.0, "WCAG's figure for non-text marks");
        let theme = NamedTheme::IvyNight.theme();
        let ground = theme.color(term_core::TermColor::Indexed(0));
        let ratio = |c: Color32| project_ink_contrast(c, ground);
        let over = (ground.r()..=255)
            .map(|v| Color32::from_rgb(v, v, v))
            .find(|c| ratio(*c) >= MARK_CONTRAST)
            .unwrap();
        let under = Color32::from_rgb(over.r() - 1, over.g() - 1, over.b() - 1);
        assert!(ratio(under) < MARK_CONTRAST, "{under:?} {over:?}");
        let with = |c| {
            let mut colors = crate::settings::Colors::default();
            colors.palette[1] = Some(c);
            theme.with_colors(&colors)
        };
        let kept = with(over);
        let roles = Roles::of(&kept);
        assert_eq!(FileKind::Rust.color(&kept, &roles, &[ground; 3]), over);
        assert_eq!(
            FileKind::Rust.color(&kept, &roles, &[ground, ground, over]),
            roles.tx2,
            "a ground the colour vanishes on, even one of three, sends it to tx2"
        );
        let lost = with(under);
        let roles = Roles::of(&lost);
        assert_eq!(FileKind::Rust.color(&lost, &roles, &[ground; 3]), roles.tx2);
    }

    #[test]
    fn a_translucent_fill_is_judged_where_it_lands() {
        let surface = Color32::from_rgb(20, 20, 20);
        let hover = Color32::from_rgba_unmultiplied(255, 255, 255, 64);
        let chosen = Color32::from_rgba_unmultiplied(0, 255, 0, 96);
        assert_eq!(
            row_grounds(surface, hover, chosen),
            [surface, surface.blend(hover), surface.blend(chosen)]
        );
        assert_ne!(surface.blend(hover), surface.blend(chosen));
        assert_ne!(surface.blend(chosen), chosen);
    }

    /// In every bundled theme, on the Files list's own surface, every kind is painted in a colour
    /// that reads on all three grounds — or in the secondary text colour.
    #[test]
    fn every_bundled_theme_paints_every_kind_legibly() {
        for named in NamedTheme::ALL {
            let theme = named.theme();
            let roles = Roles::of(&theme);
            let grounds = row_grounds(
                roles.panel_or(roles.tx, roles.base),
                roles.list_hover,
                roles.list_sel,
            );
            for kind in FileKind::ALL {
                let color = kind.color(&theme, &roles, &grounds);
                // Whatever is painted reads — the fallback included: a tx2 that vanished too
                // would leave an unreadable icon with this function satisfied.
                assert!(
                    grounds
                        .iter()
                        .all(|g| project_ink_contrast(color, *g) >= MARK_CONTRAST),
                    "{named:?} {kind:?}: {color:?}"
                );
            }
        }
    }
}
