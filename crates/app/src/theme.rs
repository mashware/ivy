//! The color table and the two attribute policies that are taste and not specification.
//!
//! It lives here and not in the terminal engine for a reason that is not stylistic: a color table
//! *is* the theme, and the engine has to survive a rewrite of the interface intact. Nor can the
//! palette be asked of the parser: its own is empty save for what the program sets through escape
//! sequences, and an application that resolves there comes out black.

mod catalog;

use eframe::egui::Color32;
use panel_contract::{Attention, Mark, Style};
use term_core::TermColor;

/// How close to the background a dimmed color moves.
///
/// It is pure taste: the specification says "less intense" and not how much. Half reads as clearly
/// muted without disappearing over this background, which is what is lost past two thirds.
const DIM_FACTOR: f32 = 0.5;

/// The six per-channel levels of the color cube of the 256-color palette.
///
/// They are not equidistant, and that is not a whim: it is the table `xterm` fixes and that the
/// whole ecosystem takes for granted. Spreading them at regular intervals would give colors that do
/// not match the ones the program expects to see.
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// How much the veil that marks the console the dragged one would drop onto covers, from 0 to 255.
///
/// It is taste, but bounded on both sides: it has to be visible over a console full of text and it has to
/// let what is underneath be read, because what is underneath is what the user is looking at to decide
/// whether to drop there. A sixth part meets both.
const DROP_TARGET_ALPHA: u8 = 40;

/// The theme: what screen color corresponds to each color a program asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// The sixteen named entries. From 0 to 7 the normal ones, from 8 to 15 the bright ones.
    palette: [Color32; 16],
    background: Color32,
    foreground: Color32,
    cursor: Color32,
    /// The line that separates two consoles.
    divider: Color32,
    /// What marks the console the keyboard goes to.
    ///
    /// **It is a color of the application, not of the terminal palette**, and that distinction matters: the
    /// palette is asked for by the program inside and describes *its* content. This describes the state of the
    /// application, so a program cannot claim it or change it.
    focus: Color32,
    /// The background of the status panel, which is what separates it visually from its console without a line.
    panel_background: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedTheme {
    TokyoNight,
    GruvboxDark,
    CatppuccinMocha,
    CatppuccinLatte,
    Nord,
    GithubDarkDefault,
    GithubLightDefault,
    RosePine,
    RosePineDawn,
    RosePineMoon,
}

impl NamedTheme {
    pub const ALL: &'static [Self] = &[
        Self::TokyoNight,
        Self::GruvboxDark,
        Self::CatppuccinMocha,
        Self::CatppuccinLatte,
        Self::Nord,
        Self::GithubDarkDefault,
        Self::GithubLightDefault,
        Self::RosePine,
        Self::RosePineDawn,
        Self::RosePineMoon,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::TokyoNight => "tokyonight",
            Self::GruvboxDark => "gruvbox-dark",
            Self::CatppuccinMocha => "catppuccin-mocha",
            Self::CatppuccinLatte => "catppuccin-latte",
            Self::Nord => "nord",
            Self::GithubDarkDefault => "github-dark-default",
            Self::GithubLightDefault => "github-light-default",
            Self::RosePine => "rose-pine",
            Self::RosePineDawn => "rose-pine-dawn",
            Self::RosePineMoon => "rose-pine-moon",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::TokyoNight => "Tokyo Night",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::CatppuccinLatte => "Catppuccin Latte",
            Self::Nord => "Nord",
            Self::GithubDarkDefault => "GitHub Dark Default",
            Self::GithubLightDefault => "GitHub Light Default",
            Self::RosePine => "Rosé Pine",
            Self::RosePineDawn => "Rosé Pine Dawn",
            Self::RosePineMoon => "Rosé Pine Moon",
        }
    }

    pub fn is_dark(self) -> bool {
        !matches!(
            self,
            Self::CatppuccinLatte | Self::GithubLightDefault | Self::RosePineDawn
        )
    }

    pub fn from_written(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|theme| theme.id().eq_ignore_ascii_case(value))
    }

    pub fn theme(self) -> Theme {
        if let Some((palette, background, foreground, cursor)) = catalog::palette(self) {
            let color =
                |rgb: u32| Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
            let mix = |percent: u32| {
                let channel = |shift: u32| -> u32 {
                    let bg = (background >> shift) & 255u32;
                    let fg = (foreground >> shift) & 255u32;
                    (bg * (100 - percent) + fg * percent + 50) / 100
                };
                (channel(16) << 16) | (channel(8) << 8) | channel(0)
            };
            return Theme {
                palette: palette.map(color),
                background: color(background),
                foreground: color(foreground),
                cursor: color(cursor),
                divider: color(mix(25)),
                focus: color(palette[4]),
                panel_background: color(mix(5)),
            };
        }
        // The sixteen terminal entries follow Tokyo Night Night and Gruvbox dark medium.
        // The six application colors below are chosen for this terminal's own surfaces.
        let (palette, background, foreground, cursor, divider, focus, panel) = match self {
            Self::TokyoNight => (
                [
                    0x15161e, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xa9b1d6,
                    0x414868, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0x9d7cd8, 0x7dcfff, 0xc0caf5,
                ],
                0x1a1b26,
                0xc0caf5,
                0xc0caf5,
                0x414868,
                0x7aa2f7,
                0x15161e,
            ),
            Self::GruvboxDark => (
                [
                    0x282828, 0xcc241d, 0x98971a, 0xd79921, 0x458588, 0xb16286, 0x689d6a, 0xa89984,
                    0x928374, 0xfb4934, 0xb8bb26, 0xfabd2f, 0x83a598, 0xd3869b, 0x8ec07c, 0xebdbb2,
                ],
                0x282828,
                0xebdbb2,
                0xebdbb2,
                0x504945,
                0x83a598,
                0x1d2021,
            ),
            _ => unreachable!("external palettes are handled above"),
        };
        let color = |rgb: u32| Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        Theme {
            palette: palette.map(color),
            background: color(background),
            foreground: color(foreground),
            cursor: color(cursor),
            divider: color(divider),
            focus: color(focus),
            panel_background: color(panel),
        }
    }
}

impl Default for Theme {
    /// The aesthetics of the project.
    ///
    /// The prototype contributes **nine** colors, and none of them is red, cyan, magenta or a
    /// bright one: it serves as an aesthetic base, not a palette. The missing ones have been written so they
    /// belong to the same family —desaturated, medium-contrast— and not to reproduce any canonical palette.
    ///
    /// The bright ones come from moving their normal color a quarter of the way toward white, with two
    /// exceptions that come from the prototype and not from that rule: bright black is its muted gray
    /// and bright white is pure white.
    fn default() -> Self {
        Self {
            palette: [
                Color32::from_rgb(0x1E, 0x1E, 0x1E), // 0  black — the same as the background
                Color32::from_rgb(0xCF, 0x6A, 0x4C), // 1  red
                Color32::from_rgb(0x8F, 0x9D, 0x6A), // 2  green
                Color32::from_rgb(0xF9, 0xEE, 0x98), // 3  yellow
                Color32::from_rgb(0x75, 0x87, 0xA6), // 4  blue
                Color32::from_rgb(0x9B, 0x85, 0x9D), // 5  magenta
                Color32::from_rgb(0x7F, 0xA5, 0xA0), // 6  cyan — written, was not there
                Color32::from_rgb(0xA7, 0xA7, 0xA7), // 7  white
                Color32::from_rgb(0x5F, 0x5A, 0x60), // 8  bright black — the gray from the prototype
                Color32::from_rgb(0xDB, 0x8F, 0x79), // 9  bright red
                Color32::from_rgb(0xAB, 0xB6, 0x8F), // 10 bright green
                Color32::from_rgb(0xFB, 0xF2, 0xB2), // 11 bright yellow
                Color32::from_rgb(0x98, 0xA5, 0xBC), // 12 bright blue
                Color32::from_rgb(0xB4, 0xA4, 0xB6), // 13 bright magenta
                Color32::from_rgb(0x9F, 0xBC, 0xB8), // 14 bright cyan
                Color32::from_rgb(0xFF, 0xFF, 0xFF), // 15 bright white — pure white
            ],
            background: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            foreground: Color32::from_rgb(0xA7, 0xA7, 0xA7),
            cursor: Color32::from_rgb(0xA7, 0xA7, 0xA7),
            // The muted gray from the prototype, which is the bright black of the palette: it shows against the
            // background without drawing attention. A divider is not content, so competing with the letters
            // would be exactly what it must not do.
            divider: Color32::from_rgb(0x5F, 0x5A, 0x60),
            // The blue of the family, one step above the divider in presence. It has to be
            // told apart **from the divider** at a glance, because the two are painted next to each other and if they
            // looked alike there would be no way to tell which console listens to the keyboard.
            focus: Color32::from_rgb(0x75, 0x87, 0xA6),
            // One step below the console background, not above: the panel is the margin and the
            // console is the content, so what has to recede is the panel. Raising it —making it
            // lighter— would turn each pane into two competing blocks, and with two panes
            // they would be four.
            panel_background: Color32::from_rgb(0x18, 0x18, 0x18),
        }
    }
}

impl Theme {
    /// A subtle selected chrome surface derived from the configured colors.
    pub(crate) fn chrome_surface(&self) -> Color32 {
        mix(self.background, self.focus, 0.15)
    }
    /// A quiet pane outline derived from the configured background and ink.
    pub(crate) fn chrome_outline(&self) -> Color32 {
        mix(self.background, self.foreground, 0.15)
    }
    /// The factory theme with the colors the configuration file asks for on top.
    ///
    /// **What the file does not say stays factory**, which is what allows changing a single color without
    /// writing all twenty-two. And the palette is overwritten slot by slot for the same reason: asking for color 1 must
    /// not erase the other fifteen.
    pub fn from_colors(colors: &crate::settings::Colors) -> Self {
        Self::default().with_colors(colors)
    }

    pub fn with_colors(mut self, colors: &crate::settings::Colors) -> Self {
        for (slot, asked_for) in self.palette.iter_mut().zip(colors.palette.iter()) {
            if let Some(color) = asked_for {
                *slot = *color;
            }
        }
        for (slot, asked_for) in [
            (&mut self.background, colors.background),
            (&mut self.foreground, colors.foreground),
            (&mut self.cursor, colors.cursor),
            (&mut self.divider, colors.divider),
            (&mut self.focus, colors.focus),
            (&mut self.panel_background, colors.panel),
        ] {
            if let Some(color) = asked_for {
                *slot = color;
            }
        }
        self
    }
}

/// The colors the status panel is painted with.
///
/// **They come from the terminal palette on purpose, not from a second family.** The two are seen at once
/// and a hand's width from each other; with two different sets of greens or reds, the screen would read
/// as two applications stuck together. What changes with respect to the console is *who* chooses the color: here
/// the contract vocabulary chooses it, not the program inside.
impl Theme {
    /// The background of the status panel.
    pub fn panel_background(&self) -> Color32 {
        self.panel_background
    }

    /// The color of a panel line, according to what the producer has said about it.
    ///
    /// **`style` rules over `mark` when both come**, and that precedence is the contract's: two of
    /// the nine real files in the repo combine them on purpose, to say "this line is
    /// information, and its color *is* the data". If `mark` won, that color would be lost.
    ///
    /// Without either of the two, the color of the console's normal text: a line without adjectives is not
    /// a muted line.
    pub fn panel_color(&self, mark: Option<Mark>, style: Option<Style>) -> Color32 {
        match (style, mark) {
            (Some(style), _) => self.by_style(style),
            (None, Some(mark)) => self.by_mark(mark),
            (None, None) => self.foreground,
        }
    }

    /// The seven styles of the contract.
    fn by_style(&self, style: Style) -> Color32 {
        match style {
            Style::Normal => self.foreground,
            // Muted is "this is here just in case": the same gesture as the terminal's dim,
            // which already means that same thing to the program inside.
            Style::Dim => self.toward_background(self.foreground),
            // The title is the only thing painted lighter than normal text. It carries no color of its own:
            // a colored title would compete with the marks, which do use it to say something.
            Style::Title => self.palette[15],
            Style::Accent => self.palette[4],
            Style::Ok => self.palette[2],
            Style::Warn => self.palette[3],
            Style::Error => self.palette[1],
        }
    }

    /// The six marks of the contract.
    ///
    /// **`done` and `pending` go muted, and both for the same reason**: what is finished and what has not even
    /// started are the two states in which there is nothing to do. What has to jump out
    /// is `current` —where the thing is going— and `block` —what stops it—.
    fn by_mark(&self, mark: Mark) -> Color32 {
        match mark {
            Mark::Done => self.toward_background(self.palette[2]),
            Mark::Current => self.palette[4],
            Mark::Pending => self.toward_background(self.foreground),
            Mark::Wait => self.palette[3],
            Mark::Block => self.palette[1],
            Mark::Info => self.foreground,
        }
    }

    /// The color a pane's attention claim is painted with.
    ///
    /// **Its own table, and not the one for the body marks, even though both come from the same palette.** The
    /// marks' table mutes `done` because what is finished is where nothing is left to do; here `done`
    /// says the opposite —it finished and **nobody has looked at it**—, so reusing it would mute exactly what it
    /// claims. It is the dangerous copy of this milestone and that is why it is said.
    ///
    /// **Unbacked it is the same one dimmed**: the claim is still there —the application does not hide what its
    /// producer said— but nobody is backing it, and that shows before reading anything. The detail
    /// —since when, or what failed— is said by the notice line, which goes right below.
    pub fn attention_color(&self, class: Attention, backed: bool) -> Color32 {
        let color = match class {
            Attention::Wait => self.palette[3],
            Attention::Block => self.palette[1],
            Attention::Done => self.palette[2],
        };
        if backed {
            color
        } else {
            self.toward_background(color)
        }
    }
}

impl Theme {
    /// The screen color that corresponds to a color asked for by a program.
    ///
    /// It does not apply the bold or dim policies: that is what [`Theme::with_attributes`] is for.
    pub fn color(&self, c: TermColor) -> Color32 {
        match c {
            TermColor::Indexed(i) => self.indexed(i),
            TermColor::Rgb { r, g, b } => Color32::from_rgb(r, g, b),
            TermColor::Foreground => self.foreground,
            TermColor::Background => self.background,
            TermColor::Cursor => self.cursor,
            // The dim variant is only produced by the first eight entries. It is masked instead
            // of indexed raw because this function is called by the painter once per cell and an
            // out-of-range index would be a panic in the middle of a frame; the debug
            // assertion is there so that error surfaces in the tests instead of disguising itself as black.
            TermColor::Dim(i) => {
                debug_assert!(i < 8, "the dim variant only covers the first eight entries");
                self.toward_background(self.palette[(i & 0b111) as usize])
            }
            TermColor::BrightForeground => self.palette[15],
            TermColor::DimForeground => self.toward_background(self.foreground),
        }
    }

    /// The color, now with both policies applied.
    ///
    /// **Which of the cell's two colors they apply to is not this place's business.**
    /// With reverse video, the color the program declared as foreground —and which is the one
    /// bold and dim describe— travels in the background field. The painter knows that; the theme
    /// only knows what the two policies consist of.
    pub fn with_attributes(&self, c: TermColor, bold: bool, dim: bool) -> Color32 {
        // Dimming arrives by two paths —as a color of its own and as a cell attribute— and
        // here they meet, so it is first normalized to a single one. Without this, a color that already comes
        // dimmed and also carries the attribute would be muted twice, and bold would not reach it
        // because it would be hidden behind a variant it does not recognize.
        let (base, dim) = match c {
            TermColor::Dim(i) => (TermColor::Indexed(i & 0b111), true),
            TermColor::DimForeground => (TermColor::Foreground, true),
            other => (other, dim),
        };
        let base = if bold { brighten(base) } else { base };
        let resolved = self.color(base);
        if dim {
            self.toward_background(resolved)
        } else {
            resolved
        }
    }

    /// The default background, which is also that of the whole window.
    pub fn background(&self) -> Color32 {
        self.background
    }

    /// The default color of the letters, for the text the application paints and not the program.
    pub fn foreground(&self) -> Color32 {
        self.foreground
    }

    /// The color of the line that separates two consoles.
    pub fn divider(&self) -> Color32 {
        self.divider
    }

    /// The color that marks the console the keyboard goes to.
    pub fn focus(&self) -> Color32 {
        self.focus
    }

    /// The color that marks the console the dragged one would be dropped onto.
    ///
    /// **It comes from the focus color and is not a new color**, because it says the same thing in another verb tense:
    /// "the keyboard is going to go here". A color of its own would force choosing it in both themes and explaining
    /// how it differs from this one, and the answer would be "in nothing".
    ///
    /// Translucent because it is painted **on top of** whatever the console is writing: opaque would cover the
    /// content just when the user is deciding whether to drop there.
    pub fn drop_target(&self) -> Color32 {
        let focus = self.focus;
        Color32::from_rgba_unmultiplied(focus.r(), focus.g(), focus.b(), DROP_TARGET_ALPHA)
    }

    /// Moves a color toward the background. It is what "dimmed" means here.
    fn toward_background(&self, c: Color32) -> Color32 {
        mix(c, self.background, DIM_FACTOR)
    }

    fn indexed(&self, i: u8) -> Color32 {
        match i {
            0..=15 => self.palette[i as usize],
            16..=231 => {
                let n = i - 16;
                Color32::from_rgb(
                    CUBE_LEVELS[(n / 36) as usize],
                    CUBE_LEVELS[((n / 6) % 6) as usize],
                    CUBE_LEVELS[(n % 6) as usize],
                )
            }
            // The twenty-four grays, from almost black to almost white. Neither of the two extremes
            // reaches it: entries 0 and 15 are there for that.
            232..=255 => {
                let v = 8 + (i - 232) * 10;
                Color32::from_rgb(v, v, v)
            }
        }
    }
}

pub(crate) fn project_accents(theme: &Theme) -> Vec<Color32> {
    let mut colors = Vec::new();
    for slot in [4, 2, 5, 6, 3, 1, 12, 10, 13, 14, 11, 9] {
        let original = theme.color(term_core::TermColor::Indexed(slot));
        let mut color = original;
        for step in 0..=10 {
            let amount = step as f32 / 10.0;
            color = mix(original, theme.foreground(), amount);
            if project_ink_contrast(color, theme.panel_background()) >= 3.0 {
                break;
            }
        }
        if project_ink_contrast(color, theme.panel_background()) >= 3.0
            && colors
                .iter()
                .all(|other| project_inks_distinct(color, *other))
        {
            colors.push(color);
        }
    }
    colors
}

pub(crate) fn project_header_colors(background: Color32) -> (Color32, Color32) {
    let foreground = if project_ink_contrast(Color32::BLACK, background)
        >= project_ink_contrast(Color32::WHITE, background)
    {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    let hover = mix(
        background,
        if foreground == Color32::BLACK {
            Color32::WHITE
        } else {
            Color32::BLACK
        },
        0.12,
    );
    // A pure black or white project fill cannot move farther toward its own pole.
    let hover = if hover == background {
        mix(background, foreground, 0.12)
    } else {
        hover
    };
    (foreground, hover)
}

pub(crate) fn project_inks_distinct(a: Color32, b: Color32) -> bool {
    let distance = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
    distance(a.r(), b.r()) + distance(a.g(), b.g()) + distance(a.b(), b.b()) >= 48 * 48
}
pub(crate) fn project_ink_contrast(a: Color32, b: Color32) -> f32 {
    let luminance = |c: Color32| {
        let linear = |n: u8| {
            let x = n as f32 / 255.0;
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// **Bold means bright.**
///
/// It is the convention taken for granted by programs that use bold to stand out, and that makes
/// a color `ls` legible. It only raises the first eight named entries: an exact color or a
/// cube entry is already the color the program wanted, and lightening it would be disobeying it.
fn brighten(c: TermColor) -> TermColor {
    match c {
        TermColor::Indexed(i) if i < 8 => TermColor::Indexed(i + 8),
        TermColor::Foreground => TermColor::BrightForeground,
        other => other,
    }
}

/// Mixes two colors, with `p` as the share the second takes.
fn mix(a: Color32, b: Color32, p: f32) -> Color32 {
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * p).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn project_headers_keep_opaque_contrast_in_factory_catalog_and_overrides() {
        use super::*;
        let custom = Theme::default().with_colors(&crate::settings::Colors {
            panel: Some(Color32::WHITE),
            foreground: Some(Color32::BLACK),
            palette: [Some(Color32::from_rgb(224, 175, 104)); 16],
            ..Default::default()
        });
        for theme in std::iter::once(Theme::default())
            .chain(NamedTheme::ALL.iter().map(|theme| theme.theme()))
            .chain(std::iter::once(custom))
        {
            for background in project_accents(&theme) {
                let (ink, hover) = project_header_colors(background);
                assert!(ink == Color32::BLACK || ink == Color32::WHITE);
                assert!(project_ink_contrast(ink, background) >= 4.5);
                assert!(project_ink_contrast(ink, hover) >= 4.5);
                assert!(project_ink_contrast(ink, hover) >= project_ink_contrast(ink, background));
                assert_ne!(background, hover);
            }
        }
        assert_eq!(
            project_header_colors(Color32::WHITE),
            (Color32::BLACK, Color32::from_gray(224))
        );
        assert_eq!(
            project_header_colors(Color32::BLACK),
            (Color32::WHITE, Color32::from_gray(31))
        );
        assert_eq!(
            project_header_colors(Color32::from_rgb(224, 175, 104)),
            (Color32::BLACK, Color32::from_rgb(228, 185, 122))
        );
    }
    #[test]
    fn bundled_catalog_has_unique_stable_names_and_exact_source_colors() {
        use super::{NamedTheme, catalog};
        use std::collections::HashSet;
        let mut ids = HashSet::new();
        assert_eq!(NamedTheme::ALL.len(), 10);
        for &theme in NamedTheme::ALL {
            assert!(ids.insert(theme.id()));
            assert_eq!(
                NamedTheme::from_written(&theme.id().to_ascii_uppercase()),
                Some(theme)
            );
            assert!(!theme.title().is_empty());
            if let Some((colors, bg, fg, cursor)) = catalog::palette(theme) {
                let actual = theme.theme();
                let rgb = |value: u32| {
                    eframe::egui::Color32::from_rgb(
                        (value >> 16) as u8,
                        (value >> 8) as u8,
                        value as u8,
                    )
                };
                assert_eq!(actual.palette, colors.map(rgb));
                assert_eq!(
                    (actual.background, actual.foreground, actual.cursor),
                    (rgb(bg), rgb(fg), rgb(cursor))
                );
                assert_eq!(actual.focus, rgb(colors[4]));
            }
        }
        assert_eq!(
            NamedTheme::CatppuccinMocha.theme().background,
            eframe::egui::Color32::from_rgb(0x1e, 0x1e, 0x2e)
        );
        assert!(!NamedTheme::RosePineDawn.is_dark());
        assert!(NamedTheme::RosePineMoon.is_dark());
    }

    #[test]
    fn generated_palettes_match_pinned_sources() {
        let fixtures = [
            (
                NamedTheme::CatppuccinMocha,
                include_str!("../../../resources/themes/Catppuccin Mocha.json"),
            ),
            (
                NamedTheme::CatppuccinLatte,
                include_str!("../../../resources/themes/Catppuccin Latte.json"),
            ),
            (
                NamedTheme::Nord,
                include_str!("../../../resources/themes/Nord.json"),
            ),
            (
                NamedTheme::GithubDarkDefault,
                include_str!("../../../resources/themes/GitHub Dark Default.json"),
            ),
            (
                NamedTheme::GithubLightDefault,
                include_str!("../../../resources/themes/GitHub Light Default.json"),
            ),
            (
                NamedTheme::RosePine,
                include_str!("../../../resources/themes/Rose Pine.json"),
            ),
            (
                NamedTheme::RosePineDawn,
                include_str!("../../../resources/themes/Rose Pine Dawn.json"),
            ),
            (
                NamedTheme::RosePineMoon,
                include_str!("../../../resources/themes/Rose Pine Moon.json"),
            ),
        ];
        let keys = [
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "purple",
            "cyan",
            "white",
            "brightBlack",
            "brightRed",
            "brightGreen",
            "brightYellow",
            "brightBlue",
            "brightPurple",
            "brightCyan",
            "brightWhite",
            "background",
            "foreground",
            "cursorColor",
        ];
        for (theme, source) in fixtures {
            let json: serde_json::Value = serde_json::from_str(source).unwrap();
            let (palette, bg, fg, cursor) = catalog::palette(theme).unwrap();
            let actual: Vec<_> = palette.into_iter().chain([bg, fg, cursor]).collect();
            let expected: Vec<_> = keys
                .iter()
                .map(|key| {
                    u32::from_str_radix(json[*key].as_str().unwrap().trim_start_matches('#'), 16)
                        .unwrap()
                })
                .collect();
            assert_eq!(actual, expected, "{}", theme.id());
        }
    }

    use super::*;

    fn theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn the_two_named_themes_have_all_twenty_two_published_colors() {
        let cases = [
            (
                NamedTheme::TokyoNight,
                [
                    0x15161e, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xa9b1d6,
                    0x414868, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0x9d7cd8, 0x7dcfff, 0xc0caf5,
                ],
                [0x1a1b26, 0xc0caf5, 0xc0caf5, 0x414868, 0x7aa2f7, 0x15161e],
            ),
            (
                NamedTheme::GruvboxDark,
                [
                    0x282828, 0xcc241d, 0x98971a, 0xd79921, 0x458588, 0xb16286, 0x689d6a, 0xa89984,
                    0x928374, 0xfb4934, 0xb8bb26, 0xfabd2f, 0x83a598, 0xd3869b, 0x8ec07c, 0xebdbb2,
                ],
                [0x282828, 0xebdbb2, 0xebdbb2, 0x504945, 0x83a598, 0x1d2021],
            ),
        ];
        let code = |color: Color32| {
            (u32::from(color.r()) << 16) | (u32::from(color.g()) << 8) | u32::from(color.b())
        };

        for (name, palette, own_colors) in cases {
            let theme = name.theme();
            assert_eq!(
                theme.palette.map(code),
                palette,
                "ANSI palette for {name:?}"
            );
            assert_eq!(
                [
                    theme.background,
                    theme.foreground,
                    theme.cursor,
                    theme.divider,
                    theme.focus,
                    theme.panel_background,
                ]
                .map(code),
                own_colors,
                "application colors for {name:?}"
            );
        }
    }

    /// **The contract rule that is easy to read backwards.** Its table describes `style` as something "for
    /// lines without `mark`", but two of the nine real files in the repo combine them on purpose: the
    /// mark says what the line **is** and the style says that its color *is* the data. If the mark won,
    /// that color would be lost without anyone noticing.
    #[test]
    fn the_style_rules_over_the_mark_when_both_come() {
        let t = theme();

        assert_eq!(
            t.panel_color(Some(Mark::Info), Some(Style::Ok)),
            t.panel_color(None, Some(Style::Ok)),
            "the color has to come from the style, not from the mark"
        );
        assert_ne!(
            t.panel_color(Some(Mark::Info), Some(Style::Ok)),
            t.panel_color(Some(Mark::Info), None),
            "with style and without it it cannot look the same"
        );
    }

    #[test]
    fn a_line_without_mark_or_style_is_painted_like_normal_text() {
        assert_eq!(theme().panel_color(None, None), theme().foreground);
    }

    /// The thirteen names of the contract, each with its color. Without this, returning the same one for all
    /// would leave the suite green and the panel in a single tone.
    #[test]
    fn the_six_marks_and_the_seven_styles_are_not_all_painted_the_same() {
        let t = theme();
        let marks = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ];
        let styles = [
            Style::Normal,
            Style::Dim,
            Style::Title,
            Style::Accent,
            Style::Ok,
            Style::Warn,
            Style::Error,
        ];

        // The **distinct** colors within each set are counted, and they are not compared across sets:
        // `normal` and `info` are worth the same on purpose —both are "text without adjectives"— and requiring
        // thirteen colors would force inventing a difference that means nothing.
        let from_marks: std::collections::HashSet<_> =
            marks.iter().map(|m| t.by_mark(*m)).collect();
        let from_styles: std::collections::HashSet<_> =
            styles.iter().map(|e| t.by_style(*e)).collect();

        assert_eq!(
            from_marks.len(),
            6,
            "the six marks are distinct from each other"
        );
        assert_eq!(from_styles.len(), 7, "the seven styles are distinct");
        assert_ne!(
            t.by_mark(Mark::Current),
            t.by_mark(Mark::Block),
            "'in progress' and 'blocked' are what has to be told apart at a glance"
        );
    }

    #[test]
    fn the_panel_background_is_told_apart_from_the_console_one() {
        let t = theme();
        assert_ne!(t.panel_background(), t.background());
    }

    #[test]
    fn the_three_default_colors_are_the_project_ones() {
        let t = theme();
        assert_eq!(
            t.color(TermColor::Background),
            Color32::from_rgb(0x1E, 0x1E, 0x1E)
        );
        assert_eq!(
            t.color(TermColor::Foreground),
            Color32::from_rgb(0xA7, 0xA7, 0xA7)
        );
        assert_eq!(
            t.color(TermColor::Cursor),
            Color32::from_rgb(0xA7, 0xA7, 0xA7)
        );
    }

    #[test]
    fn the_divider_and_the_focus_show_and_are_told_apart_from_each_other() {
        let t = theme();
        // Three inequalities, and each covers a distinct way this becomes useless without
        // anything failing: a divider the color of the background is an invisible divider —two consoles next to each other that
        // read as one—, and a highlight equal to the divider leaves the window without saying which of the two
        // the keyboard goes to, which is exactly what the highlight exists to say.
        assert_ne!(t.divider(), t.background(), "the divider would not show");
        assert_ne!(
            t.focus(),
            t.background(),
            "the focus highlight would not show"
        );
        assert_ne!(
            t.focus(),
            t.divider(),
            "the highlight is not told apart from the divider and they go next to each other"
        );
    }

    #[test]
    fn the_border_between_the_named_ones_and_the_cube() {
        let t = theme();
        // 15 is the last named entry; 16 is the first of the cube, and it is pure black —not the
        // theme's background—. Confusing these two is the error this test exists to catch.
        assert_eq!(
            t.color(TermColor::Indexed(15)),
            Color32::from_rgb(0xFF, 0xFF, 0xFF)
        );
        assert_eq!(
            t.color(TermColor::Indexed(16)),
            Color32::from_rgb(0x00, 0x00, 0x00)
        );
    }

    #[test]
    fn the_border_between_the_cube_and_the_grays() {
        let t = theme();
        // 231 is the last of the cube and is pure white; 232 is the first gray and is **not** black.
        assert_eq!(
            t.color(TermColor::Indexed(231)),
            Color32::from_rgb(0xFF, 0xFF, 0xFF)
        );
        assert_eq!(
            t.color(TermColor::Indexed(232)),
            Color32::from_rgb(0x08, 0x08, 0x08)
        );
        assert_eq!(
            t.color(TermColor::Indexed(255)),
            Color32::from_rgb(0xEE, 0xEE, 0xEE)
        );
    }

    #[test]
    fn the_cube_spreads_the_channels_in_the_correct_order() {
        let t = theme();
        // 196 is the pure red of the cube: 196-16 = 180 = 5*36, so red at maximum and the others at
        // zero. If someone inverts the order of the channels, this is the test that goes red.
        assert_eq!(
            t.color(TermColor::Indexed(196)),
            Color32::from_rgb(0xFF, 0x00, 0x00)
        );
        // 21 is pure blue: 21-16 = 5, the last level of the lowest-weight channel.
        assert_eq!(
            t.color(TermColor::Indexed(21)),
            Color32::from_rgb(0x00, 0x00, 0xFF)
        );
    }

    #[test]
    fn the_bright_ones_meet_the_rule_they_were_written_with() {
        let t = theme();
        // The palette doc claims that the bright ones come from moving their normal color a quarter of the way
        // toward white. A claim written and not checked becomes a lie as
        // soon as someone tweaks an entry, so here it is checked.
        // The two exceptions the doc itself declares are left out: bright black and bright white
        // come from the prototype, not from the rule.
        for i in 1..=6u8 {
            let normal = t.color(TermColor::Indexed(i));
            let bright = t.color(TermColor::Indexed(i + 8));
            assert_eq!(
                bright,
                mix(normal, Color32::WHITE, 0.25),
                "the bright entry {} does not come from the rule",
                i + 8
            );
        }
    }

    #[test]
    fn an_exact_color_passes_untouched() {
        let t = theme();
        let c = TermColor::Rgb { r: 1, g: 2, b: 3 };
        assert_eq!(t.color(c), Color32::from_rgb(1, 2, 3));
    }

    #[test]
    fn bold_raises_to_the_bright_variant() {
        let t = theme();
        let red = TermColor::Indexed(1);
        assert_eq!(
            t.with_attributes(red, false, false),
            Color32::from_rgb(0xCF, 0x6A, 0x4C)
        );
        assert_eq!(
            t.with_attributes(red, true, false),
            Color32::from_rgb(0xDB, 0x8F, 0x79)
        );
        assert_eq!(
            t.with_attributes(red, true, false),
            t.color(TermColor::Indexed(9))
        );
    }

    #[test]
    fn bold_does_not_touch_what_the_program_chose_by_hand() {
        let t = theme();
        // Neither an exact color nor a cube entry is lightened: they are already the color that was asked for.
        let exact = TermColor::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        assert_eq!(
            t.with_attributes(exact, true, false),
            Color32::from_rgb(10, 20, 30)
        );
        let from_the_cube = TermColor::Indexed(196);
        assert_eq!(
            t.with_attributes(from_the_cube, true, false),
            Color32::from_rgb(0xFF, 0x00, 0x00)
        );
        // And an entry that is already bright does not rise again: 9 does not become 17.
        assert_eq!(
            t.with_attributes(TermColor::Indexed(9), true, false),
            t.color(TermColor::Indexed(9))
        );
    }

    #[test]
    fn dimming_moves_the_color_toward_the_background() {
        let t = theme();
        // Red halfway to #1E1E1E.
        assert_eq!(
            t.with_attributes(TermColor::Indexed(1), false, true),
            Color32::from_rgb(0x77, 0x44, 0x35)
        );
        // And the dimmed background is still the background: there is nowhere to move toward.
        assert_eq!(
            t.with_attributes(TermColor::Background, false, true),
            t.color(TermColor::Background)
        );
    }

    #[test]
    fn the_librarys_dim_variant_gives_the_same_as_the_attribute() {
        let t = theme();
        // They are the two paths the same thing arrives by: the `Dim(n)` color the engine delivers and
        // the dim attribute on the normal color. If they diverged, the same escape
        // sequence would look two different colors depending on which way it came in.
        for n in 0..8u8 {
            assert_eq!(
                t.color(TermColor::Dim(n)),
                t.with_attributes(TermColor::Indexed(n), false, true),
                "index {n} does not match by the two paths"
            );
        }
    }

    #[test]
    fn an_already_dimmed_color_is_not_muted_twice() {
        let t = theme();
        // Whoever paints is going to call `with_attributes` with the color and the cell attributes just
        // as they come, without looking at whether the color is already one of the dimmed ones. If it were not normalized, the
        // dimming would be applied twice and the color would end up a quarter of the way to the background
        // instead of halfway — more muted for having come in through the side door.
        for n in 0..8u8 {
            assert_eq!(
                t.with_attributes(TermColor::Dim(n), false, true),
                t.color(TermColor::Dim(n)),
                "index {n} is muted twice"
            );
        }
        assert_eq!(
            t.with_attributes(TermColor::DimForeground, false, true),
            t.color(TermColor::DimForeground)
        );
    }

    #[test]
    fn bold_reaches_a_color_that_already_came_dimmed() {
        let t = theme();
        // A dimmed and bold red rises to bright red and then is muted, just as if the
        // dimming had come through the attribute. Without normalizing, bold would not touch it: the
        // dim variant is not one of the ones it knows how to raise.
        assert_eq!(
            t.with_attributes(TermColor::Dim(1), true, false),
            t.with_attributes(TermColor::Indexed(1), true, true)
        );
        assert_eq!(
            t.with_attributes(TermColor::DimForeground, true, false),
            t.with_attributes(TermColor::Foreground, true, true)
        );
    }

    #[test]
    fn the_foreground_has_its_two_variants() {
        let t = theme();
        assert_eq!(
            t.color(TermColor::BrightForeground),
            Color32::from_rgb(0xFF, 0xFF, 0xFF)
        );
        assert_eq!(
            t.color(TermColor::DimForeground),
            Color32::from_rgb(0x63, 0x63, 0x63)
        );
        // Bold on the default foreground leads to its bright variant, not to
        // entry 8 of the palette, which is the muted gray.
        assert_eq!(
            t.with_attributes(TermColor::Foreground, true, false),
            t.color(TermColor::BrightForeground)
        );
    }

    #[test]
    fn bold_and_dim_at_once_lighten_and_then_mute() {
        let t = theme();
        // The order matters and is fixed: first it rises to bright, then it is muted. The other way around
        // would give another color, and two layers that did it in a different order would not match.
        let expected = mix(
            Color32::from_rgb(0xDB, 0x8F, 0x79),
            Color32::from_rgb(0x1E, 0x1E, 0x1E),
            DIM_FACTOR,
        );
        assert_eq!(
            t.with_attributes(TermColor::Indexed(1), true, true),
            expected
        );
    }

    #[test]
    fn no_index_of_the_256_palette_blows_up_the_table() {
        let t = theme();
        for i in 0..=255u8 {
            let _ = t.color(TermColor::Indexed(i));
        }
        // The dim variant only runs over its valid range: asking for it outside the first eight
        // entries is a programming error, and that is why a debug assertion catches it
        // instead of returning any old color.
        for n in 0..8u8 {
            let _ = t.color(TermColor::Dim(n));
        }
    }

    /// The three with a color of their own. Returning the same one for all, the attention row would say "it claims"
    /// and keep silent about **which**, which is the only thing that decides whether one has to go running or not.
    #[test]
    fn the_three_attention_claims_are_not_all_painted_the_same() {
        let t = theme();
        let colors = Attention::ALL.map(|class| t.attention_color(class, true));

        for (i, one) in colors.iter().enumerate() {
            for other in &colors[i + 1..] {
                assert_ne!(one, other, "two attention claims share color: {colors:?}");
            }
        }
    }

    /// **The dangerous copy of this milestone, tied down where it shows.** The body marks' table
    /// mutes `done` because what is finished is where nothing is left to do; here `done` says the
    /// opposite —it finished and **nobody has looked at it**—, so reusing it would mute exactly what it
    /// claims. With `by_mark` in the middle this test falls, and without it the suite would stay green with the
    /// quietest attention claim of the three painted gray.
    #[test]
    fn the_finished_attention_claim_is_not_painted_muted_like_its_body_mark() {
        let t = theme();

        assert_ne!(
            t.attention_color(Attention::Done, true),
            t.panel_color(Some(Mark::Done), None),
            "the finished attention claim is being painted with the body table, which mutes it"
        );
        assert_eq!(
            t.attention_color(Attention::Done, true),
            t.palette[2],
            "backed it is painted with the whole color, undimmed"
        );
    }

    /// **Each class nailed to its color, and not just "the three are distinct".** The painting tests
    /// compare against this very function, so crossing the amber of `wait` with the red of `block` —or
    /// painting `wait` with the gray of normal text, which is to stop alarming— passed the whole suite. It is
    /// the usual disguise: asserting on the place where the value is recorded instead of on the value.
    #[test]
    fn each_attention_claim_carries_the_color_the_decision_assigned_it() {
        let t = theme();

        assert_eq!(t.attention_color(Attention::Wait, true), t.palette[3]);
        assert_eq!(t.attention_color(Attention::Block, true), t.palette[1]);
        assert_eq!(t.attention_color(Attention::Done, true), t.palette[2]);
    }

    /// And unbacked it is **that same color dimmed**, not another: dimming says "nobody backs it",
    /// not "it is something else". With a color of its own for the stale one, the class would stop being read.
    #[test]
    fn an_unbacked_attention_claim_is_its_own_color_dimmed() {
        let t = theme();

        for class in Attention::ALL {
            assert_eq!(
                t.attention_color(class, false),
                t.toward_background(t.attention_color(class, true)),
                "{class:?} unbacked is not its muted color"
            );
        }
    }

    /// Backed and unbacked cannot look the same: dimming **is** what says nobody is
    /// behind it, and with the two colors equal the tab would lie about how fresh the attention claim is.
    #[test]
    fn an_unbacked_attention_claim_looks_different_from_the_same_one_backed() {
        let t = theme();
        for class in Attention::ALL {
            assert_ne!(
                t.attention_color(class, true),
                t.attention_color(class, false),
                "{class:?} looks the same whether someone backs it or not"
            );
        }
    }
}
