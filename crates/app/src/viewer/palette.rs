//! The code view's colours, taken from the person's theme.
use super::highlight::Token;
use crate::theme::{Roles, Theme, project_ink_contrast, project_inks_distinct};
use eframe::egui::Color32;
use term_core::TermColor;

/// Code is text: the contrast a token colour must reach to be chosen, as for any text (4.5:1).
const TEXT_CONTRAST: f32 = 4.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Palette {
    pub(super) text: Color32,
    pub(super) comment: Color32,
    pub(super) keyword: Color32,
    pub(super) string: Color32,
    pub(super) number: Color32,
    pub(super) kind: Color32,
    pub(super) meta: Color32,
    /// The line numbers.
    pub(super) gutter: Color32,
    /// What a selected stretch of text sits on.
    pub(super) selection: Color32,
}

impl Palette {
    /// Each coloured token takes its ANSI colour, or the bright one, whichever first reads on the
    /// document and on a selection **and** differs from the colours already taken; a theme that
    /// offers neither leaves the token in the text colour. Comments and line numbers take the muted
    /// text roles. The text colour itself is the theme's own, never checked here.
    pub(super) fn of(theme: &Theme, roles: &Roles) -> Self {
        let selection = roles.sel;
        let grounds = [roles.base, roles.base.blend(selection)];
        let reads = |ink: Color32| {
            grounds
                .iter()
                .all(|ground| project_ink_contrast(ink, *ground) >= TEXT_CONTRAST)
        };
        let mut taken = vec![roles.tx];
        let mut pick = |normal: u8| {
            let chosen = [normal, normal + 8]
                .map(|index| theme.color(TermColor::Indexed(index)))
                .into_iter()
                .find(|ink| reads(*ink) && taken.iter().all(|t| project_inks_distinct(*t, *ink)))
                .unwrap_or(roles.tx);
            taken.push(chosen);
            chosen
        };
        let keyword = pick(5);
        let string = pick(2);
        let number = pick(3);
        let kind = pick(6);
        let meta = pick(4);
        let muted = [roles.tx3, roles.tx2]
            .into_iter()
            .find(|ink| reads(*ink))
            .unwrap_or(roles.tx);
        Self {
            text: roles.tx,
            comment: muted,
            keyword,
            string,
            number,
            kind,
            meta,
            gutter: muted,
            selection,
        }
    }

    pub(super) fn of_token(&self, token: Token) -> Color32 {
        match token {
            Token::Text => self.text,
            Token::Comment => self.comment,
            Token::Keyword => self.keyword,
            Token::String => self.string,
            Token::Number => self.number,
            Token::Type => self.kind,
            Token::Meta => self.meta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::NamedTheme;

    const TOKENS: [Token; 7] = [
        Token::Text,
        Token::Comment,
        Token::Keyword,
        Token::String,
        Token::Number,
        Token::Type,
        Token::Meta,
    ];

    /// On every theme the catalog ships, each token reads on the document and on a selection.
    #[test]
    fn every_token_reads_on_every_catalog_theme() {
        for &named in NamedTheme::ALL {
            let theme = named.theme();
            let roles = Roles::of(&theme);
            let palette = Palette::of(&theme, &roles);
            // The grounds and the ratio are spelled out, not borrowed from the code under test.
            for ground in [roles.base, roles.base.blend(roles.sel)] {
                for token in TOKENS {
                    let ink = palette.of_token(token);
                    assert!(
                        project_ink_contrast(ink, ground) >= 4.5,
                        "{named:?} {token:?}: {ink:?} on {ground:?}"
                    );
                }
            }
        }
    }

    /// On the default theme comments, strings, numbers and keywords are four different colours,
    /// none of them the text's.
    #[test]
    fn the_default_theme_tells_the_tokens_apart() {
        let theme = Theme::default();
        let palette = Palette::of(&theme, &Roles::of(&theme));
        let inks = [
            palette.text,
            palette.comment,
            palette.string,
            palette.number,
            palette.keyword,
        ];
        for (i, a) in inks.iter().enumerate() {
            for b in &inks[i + 1..] {
                assert!(project_inks_distinct(*a, *b), "{inks:?}");
            }
        }
    }

    /// A theme whose green and bright green are the colour keywords took cannot colour strings
    /// apart from keywords: strings fall back to the text colour rather than take the same one.
    #[test]
    fn a_colour_already_taken_is_not_taken_twice() {
        let base = Theme::default();
        let keyword = Palette::of(&base, &Roles::of(&base)).keyword;
        let mut colors = crate::settings::Colors::default();
        colors.palette[2] = Some(keyword);
        colors.palette[10] = Some(keyword);
        let theme = base.with_colors(&colors);
        let roles = Roles::of(&theme);
        let palette = Palette::of(&theme, &roles);
        assert_eq!(palette.keyword, keyword, "the fixture must keep the keywords' colour");
        assert_ne!(palette.string, palette.keyword);
        assert_eq!(palette.string, roles.tx);
    }
}
