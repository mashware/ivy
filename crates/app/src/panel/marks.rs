//! The inks of a panel line that carries a mark: one for the mark's glyph, one for its words.
//!
//! **Two inks and not one**, which is what the design kit's step list needs: a step in progress
//! shows its glyph in the accent while its words stay plain text, and a pending one dims its glyph
//! further than its words. With one ink per row, either the accent would cover a whole sentence
//! or the glyph would lose the colour that says where the work is (D97).
//!
//! **A style still rules over a mark**, glyph included: the contract lets a producer combine both
//! to say «this line's colour is the data», and splitting the line into two colours would take
//! half of that data away.

use crate::theme::{Roles, Theme};
use eframe::egui::Color32;
use panel_contract::{Mark, Style};

/// The glyph's ink and the words' ink for a line, in that order. Without the integrated chrome's
/// `roles` both are the line's one historical colour.
pub(super) fn inks(
    mark: Option<Mark>,
    style: Option<Style>,
    theme: &Theme,
    roles: Option<&Roles>,
) -> (Color32, Color32) {
    let Some(roles) = roles.filter(|_| style.is_none() && mark.is_some()) else {
        let ink = theme.panel_color(mark, style);
        return (ink, ink);
    };
    match mark {
        // Finished and quiet: both muted, so what is left to do stands out.
        Some(Mark::Done) => (roles.tx3, roles.tx3),
        // Where the work is: the glyph carries the accent, the words read as ordinary text.
        Some(Mark::Current) => (roles.accent, roles.tx),
        // Not started: a faint glyph and secondary words.
        Some(Mark::Pending) => (roles.tx3, roles.tx2),
        // What waits or is stuck keeps the claim's colour on both, glyph and words.
        Some(Mark::Wait) => (roles.wait, roles.wait),
        Some(Mark::Block) => (roles.block, roles.block),
        Some(Mark::Info) | None => (roles.tx, roles.tx),
    }
}

#[cfg(test)]
mod tests {
    use crate::panel::tests::paint_integrated;
    use crate::panel::{PanelState, Rect};
    use eframe::egui::{Color32, Pos2, Shape, vec2};
    use std::time::SystemTime;

    fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    /// **Each mark as the integrated chrome paints it, with Ivy Night's literal inks**: the glyph and the words are two
    /// shapes, and each carries its own colour. A style keeps both in its own colour.
    #[test]
    fn each_mark_paints_its_glyph_and_its_words_in_the_kits_inks() {
        let mut state = PanelState::default();
        let dir = panel_contract::PanelDir::create().expect("dir");
        crate::panel::tests::publish(
            &dir,
            r#"{"header":false,"lines":[
                {"text":"read the code","mark":"done"},
                {"text":"isolate the fixture","mark":"current"},
                {"text":"run it again","mark":"pending"},
                {"text":"approve the plan","mark":"wait"},
                {"text":"the pipeline","mark":"block"},
                {"text":"a note","mark":"info"},
                {"text":"the data","mark":"done","style":"ok"}
            ]}"#,
        );
        state.refresh(dir.path());
        let texts: Vec<(String, Color32)> = paint_integrated(
            &mut state,
            Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0)),
            SystemTime::now(),
        )
        .into_iter()
        .filter_map(|shape| match shape {
            Shape::Text(t) => Some((t.galley.text().to_owned(), t.fallback_color)),
            _ => None,
        })
        .collect();
        let (tx, tx2, tx3) = (rgb(0xe4e8e2), rgb(0xa5aaa6), rgb(0x8e9390));
        let ok = crate::theme::Theme::default().panel_color(None, Some(panel_contract::Style::Ok));
        let expected = [
            ("  read the code", tx3),
            ("●", tx3),
            ("  isolate the fixture", tx),
            ("▶", rgb(0x3cc384)),
            ("  run it again", tx2),
            ("·", tx3),
            ("  approve the plan", rgb(0xf2b84b)),
            ("…", rgb(0xf2b84b)),
            ("  the pipeline", rgb(0xf27a66)),
            ("×", rgb(0xf27a66)),
            ("  a note", tx),
            ("›", tx),
            ("  the data", ok),
            ("●", ok),
        ];
        assert_eq!(
            texts,
            expected
                .iter()
                .map(|(t, c)| (t.to_string(), *c))
                .collect::<Vec<_>>()
        );
    }
}
