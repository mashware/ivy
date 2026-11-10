//! Where the section headers and the cards go: a pure function of how many cards each section holds
//! and of the width, so painting, scrolling to the selection and arrow keys read one geometry.
use eframe::egui::{Rect, pos2, vec2};

pub(super) const GAP: f32 = 10.0;
/// Below this a card stops being readable, so the grid drops a column instead.
const MIN_CARD_WIDTH: f32 = 260.0;
/// Name, claim or «No published state», phase, warning and three body lines. Fixed, so no document can grow its card over
/// a neighbour (D68's bounded rows, carried over to cards).
pub(super) const CARD_SLOTS: usize = 7;
pub(super) const CARD_PADDING: f32 = 8.0;

pub(super) struct Card {
    pub(super) rect: Rect,
    /// What has to be on screen for the card to be seen in context: the card, and its section's
    /// header when the card sits on the section's first row.
    pub(super) context: Rect,
    /// Row of the whole board, counted across sections, which is what ↑/↓ move along.
    pub(super) row: usize,
    pub(super) column: usize,
}

/// One painted card as navigation reads it, all from the same painting so the three never disagree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Slot {
    pub(super) pane: usize,
    pub(super) row: usize,
    pub(super) column: usize,
}

pub(super) struct Layout {
    pub(super) headers: Vec<Rect>,
    /// One per card, in the order the sections and their entries are given.
    pub(super) cards: Vec<Card>,
    pub(super) height: f32,
}

pub(super) fn layout(counts: &[usize], width: f32, line: f32) -> Layout {
    let width = width.max(1.0);
    let columns = (((width + GAP) / (MIN_CARD_WIDTH + GAP)).floor() as usize).max(1);
    let card_width = ((width - GAP * (columns - 1) as f32) / columns as f32).max(1.0);
    let card_height = line * CARD_SLOTS as f32 + 2.0 * CARD_PADDING;
    let header_height = line + GAP;
    let mut headers = Vec::with_capacity(counts.len());
    let mut cards = Vec::with_capacity(counts.iter().sum());
    let mut y = 0.0;
    let mut first_row = 0;
    for &count in counts {
        let header = Rect::from_min_size(pos2(0.0, y), vec2(width, header_height));
        headers.push(header);
        y += header_height + GAP / 2.0;
        let rows = count.div_ceil(columns);
        for index in 0..count {
            let (row, column) = (index / columns, index % columns);
            let rect = Rect::from_min_size(
                pos2(
                    column as f32 * (card_width + GAP),
                    y + row as f32 * (card_height + GAP),
                ),
                vec2(card_width, card_height),
            );
            cards.push(Card {
                rect,
                context: if row == 0 { rect.union(header) } else { rect },
                row: first_row + row,
                column,
            });
        }
        y += rows as f32 * (card_height + GAP) + GAP;
        first_row += rows;
    }
    Layout {
        headers,
        cards,
        height: y,
    }
}

/// The card one row up or down in the same column, clamped to the last card of a shorter row; the
/// same card when there is no such row.
pub(super) fn vertical(cards: &[Slot], from: usize, down: bool) -> usize {
    let Some(&Slot { row, column, .. }) = cards.get(from) else {
        return from;
    };
    let Some(target) = (if down {
        row.checked_add(1)
    } else {
        row.checked_sub(1)
    }) else {
        return from;
    };
    let in_row = || {
        cards
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.row == target)
    };
    in_row()
        .rfind(|(_, slot)| slot.column <= column)
        .or_else(|| in_row().next())
        .map_or(from, |(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_never_overlap_and_stay_inside_the_width() {
        for width in [100.0, 640.0, 1280.0, 2000.0] {
            let layout = layout(&[3, 1, 5], width, 17.0);
            assert_eq!(layout.cards.len(), 9);
            assert_eq!(layout.headers.len(), 3);
            for (n, card) in layout.cards.iter().enumerate() {
                assert!(card.rect.right() <= width + 0.01, "card {n} at {width}");
                assert!(card.rect.bottom() <= layout.height);
                for header in &layout.headers {
                    assert!(!card.rect.intersects(*header), "card {n} under a header");
                }
                for other in &layout.cards[n + 1..] {
                    assert!(
                        !card.rect.intersects(other.rect),
                        "card {n} overlaps at {width}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_narrow_board_is_one_column_and_a_wide_one_fills_its_rows() {
        let narrow = layout(&[3], 300.0, 17.0);
        assert!(narrow.cards.iter().all(|c| c.column == 0));
        assert_eq!(
            narrow.cards.iter().map(|c| c.row).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        let wide = layout(&[3, 2], 1000.0, 17.0);
        assert_eq!(
            wide.cards
                .iter()
                .map(|c| (c.row, c.column))
                .collect::<Vec<_>>(),
            [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1)],
            "a section starts a row of its own"
        );
        assert!(wide.headers[1].top() > wide.cards[2].rect.bottom());
        assert!(
            wide.cards[3].context.contains_rect(wide.headers[1]),
            "a first-row card brings its header"
        );
        let tall = layout(&[4], 300.0, 17.0);
        assert_eq!(
            tall.cards[1].context, tall.cards[1].rect,
            "a later row brings only itself"
        );
    }

    #[test]
    fn arrows_move_one_grid_row_in_the_same_column_and_clamp_to_a_shorter_row() {
        // Section A: three cards on row 0; section B: two cards on row 1.
        let grid = [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1)].map(|(row, column)| Slot {
            pane: 0,
            row,
            column,
        });
        assert_eq!(vertical(&grid, 1, true), 4);
        assert_eq!(
            vertical(&grid, 2, true),
            4,
            "clamped to the last card of row 1"
        );
        assert_eq!(vertical(&grid, 4, false), 1);
        assert_eq!(vertical(&grid, 0, false), 0, "no row above");
        assert_eq!(vertical(&grid, 3, true), 3, "no row below");
    }
}
