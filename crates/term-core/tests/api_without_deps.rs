//! That the public API of `term-core` is made of bespoke types.
//!
//! It is the milestone's criterion and **it is checked by compiling**, not by running: this file traverses
//! the whole public surface of the reader naming only types of `term_core`, and does an exhaustive
//! `match` over its enumerations. If `Screen::view` returned the types of
//! `alacritty_terminal` instead of ours, none of these `match` would compile: their variants
//! do not exist there, and the ones that do have other names.
//!
//! It lives in `tests/` and not inside the module on purpose: here the crate is consumed **from outside**,
//! just as whoever paints is going to consume it.
//!
//! **What this does NOT prove, and it is worth knowing before trusting it.** It does not detect a foreign type that
//! enters by a route other than the return type. The one that slipped into the first version of this
//! PR was an `impl From<Color> for TermColor`: trait `impl`s **carry no visibility**, so
//! they are public API even though the type on the left is from the dependency, and this test passed
//! exactly the same with the leak in place. Checking that there is no `pub use` of the dependency would not have
//! seen it either.
//!
//! The check that does see it is generating the documentation and searching for the foreign name:
//!
//! ```text
//! cargo doc -p term-core --no-deps
//! grep -rl "From&#60;Color&#62;\|From&#60;&#38;Cell&#62;" target/doc/term_core/   # has to find nothing
//! ```
//!
//! **It still catches both leaks, and it is measured.** It was suspected that the rename had taken it away
//! —the bespoke type came to be named the same as the engine's— so both leaks were injected, one
//! per type, and the pattern finds them both: `rustdoc` writes the **real** name of the type in
//! the header of the `impl`, not the alias with which this crate imports it. It was almost changed to
//! `grep alacritty_terminal`, which sounds nicer and **finds neither of the two**.
//!
//! What it did lose is precision in the other direction: a legitimate `impl From&#60;&#38;Cell&#62;` **over
//! our own `Cell`** would now give a hit. It would be a false positive, not a silence — and this
//! file forbids exactly those `impl`s three paragraphs further down, so the case does not arise.
//!
//! That is why the translation lives in `pub(crate)` functions and not in `impl From`: it is not style, it is what
//! makes the leak **impossible** instead of leaving it up to remembering to look.

use term_core::{
    Cell, CellWidth, CursorShape, Half, MouseReporting, Screen, Scroll, Size, TermColor, Underline,
    ViewCell,
};

/// Traverses all the color variants without naming the enum of the library underneath.
fn describe(color: TermColor) -> &'static str {
    match color {
        TermColor::Indexed(_) => "palette entry",
        TermColor::Rgb { .. } => "exact color",
        TermColor::Foreground => "theme foreground",
        TermColor::Background => "theme background",
        TermColor::Cursor => "cursor color",
        TermColor::Dim(_) => "dimmed entry",
        TermColor::BrightForeground => "bright foreground",
        TermColor::DimForeground => "dim foreground",
    }
}

fn describe_width(width: CellWidth) -> &'static str {
    match width {
        CellWidth::Single => "one column",
        CellWidth::WideHead => "first of two",
        CellWidth::WideTail => "companion",
        CellWidth::WrapFiller => "wrap gap",
    }
}

fn describe_underline(style: Underline) -> &'static str {
    match style {
        Underline::None => "no underline",
        Underline::Single => "single",
        Underline::Double => "double",
        Underline::Curl => "wavy",
        Underline::Dotted => "dotted",
        Underline::Dashed => "dashed",
    }
}

fn describe_cursor(shape: CursorShape) -> &'static str {
    match shape {
        CursorShape::Block => "block",
        CursorShape::Underline => "underline",
        CursorShape::Beam => "beam",
        CursorShape::HollowBlock => "hollow block",
        CursorShape::Hidden => "off",
    }
}

#[test]
fn the_view_is_consumed_whole_with_bespoke_types() {
    let mut screen = Screen::new(Size { rows: 4, cols: 20 });
    screen.advance(b"\x1b[7;1;4mgood\x1b[0m");

    let view = screen.view();

    // Every public field of the cell, read without importing anything of the dependency.
    for cell in view.cells() {
        let ViewCell {
            c,
            combining,
            fg,
            bg,
            width,
            inverse,
            bold,
            dim,
            italic,
            underline,
            strikeout,
            hidden,
            selected,
            matched,
        } = *cell;

        assert!(!describe(fg).is_empty());
        assert!(!describe(bg).is_empty());
        assert!(!describe_width(width).is_empty());
        assert!(!describe_underline(underline).is_empty());
        // **What is attached is really consumed, not discarded.** It is the field that would be easiest to expose
        // with a type of the dependency —the engine stores it in a `Vec<char>` of its own— and what this
        // test defends is exactly that: it comes out as a slice of `char` from the standard library.
        let attached: &[char] = view.combining_of(cell);
        assert!(
            attached.is_empty(),
            "this screen is ASCII: no cell should bring anything attached"
        );
        let _ = combining;
        let _ = (
            c, inverse, bold, dim, italic, strikeout, hidden, selected, matched,
        );
    }

    assert!(
        !describe_cursor(
            view.cursor()
                .expect("with the screen up to date the cursor is visible")
                .shape
        )
        .is_empty()
    );
    assert_eq!(view.size(), Size { rows: 4, cols: 20 });
    assert_eq!(view.cells().len(), 80);
    assert_eq!(
        view.cursor()
            .expect("with the screen up to date the cursor is visible")
            .row,
        0
    );
}

/// That **looking at the history** is also requested and read with bespoke types.
///
/// It goes apart from the view because it is new surface and **the test above did not guard it**: the engine
/// underneath has its own vocabulary for this —a scroll enum with its variants— and
/// letting it show through would be exactly the leak this file exists to prevent. **All** the
/// variants are traversed with an exhaustive `match`, which is what makes compiling the check: if
/// `Scroll` were the foreign type, its variants would not be named this way.
#[test]
fn the_history_is_requested_and_read_with_bespoke_types() {
    fn describe(amount: Scroll) -> &'static str {
        match amount {
            Scroll::Lines(_) => "a few lines",
            Scroll::Pages(_) => "a few screens",
            Scroll::ToTop => "up to the oldest that is kept",
            Scroll::ToBottom => "back to where the program writes",
        }
    }

    let mut screen = Screen::new(Size { rows: 4, cols: 20 });
    for n in 0..40 {
        screen.advance(format!("entry {n}\r\n").as_bytes());
    }

    for amount in [
        Scroll::ToTop,
        Scroll::Lines(-1),
        Scroll::Pages(-1),
        Scroll::ToBottom,
    ] {
        assert!(!describe(amount).is_empty());
        screen.scroll_by(amount);
    }

    // And what is read back is a number of ours, not the internal position of the engine.
    let amount: usize = screen.view().scroll();
    assert_eq!(amount, 0, "it ended up requesting to return to the end");
}

/// That **the selection** is also requested and read with bespoke types.
///
/// It is new surface and of the kind this file watches: the engine keeps the selection as a range
/// of its own, with its point and its half, and letting any of the three show through would be the leak. What comes out of here
/// are a cell of ours, a half of ours, a `bool` per cell and a string.
#[test]
fn the_selection_is_requested_and_read_with_bespoke_types() {
    fn describe(half: Half) -> &'static str {
        match half {
            Half::Left => "by the left half",
            Half::Right => "by the right half",
        }
    }

    let mut screen = Screen::new(Size { rows: 4, cols: 20 });
    screen.advance(b"good night");

    let from = Cell {
        row: 0,
        col: 0,
        half: Half::Left,
    };
    let to = Cell {
        row: 0,
        col: 3,
        half: Half::Right,
    };
    assert!(!describe(from.half).is_empty());
    assert!(!describe(to.half).is_empty());

    screen.start_selection(from);
    screen.extend_selection(to);

    let has: bool = screen.has_selection();
    let text: Option<String> = screen.selected_text();
    assert!(has);
    assert_eq!(text.as_deref(), Some("good"));

    // And the highlight reaches the snapshot as a boolean per cell, not as the range.
    let view = screen.view();
    let highlighted = view.cells().iter().filter(|cell| cell.selected).count();
    assert_eq!(highlighted, 4, "the highlight did not arrive cell by cell");

    screen.clear_selection();
    assert!(!screen.has_selection());
}

/// That asking about the mouse is also done with bespoke types.
///
/// **It goes apart from those above because it is new surface and none guarded it.** What
/// `mouse_reporting` reads are the engine's flags, so returning them as they are is the natural
/// refactor — and with it, the whole crate would still compile and this file would still pass. The test
/// names the bespoke type and **destructures its four fields**, which is what makes it impossible for the
/// engine's type to pass through here: its flags do not have these names.
#[test]
fn the_mouse_is_asked_and_read_with_bespoke_types() {
    let mut screen = Screen::new(Size { rows: 4, cols: 20 });
    screen.advance(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");

    let MouseReporting {
        buttons,
        drag,
        motion,
        extended,
    } = screen.mouse_reporting();

    // **The three modes are REPLACED, not accumulated** —read in the engine, not assumed—: requesting the
    // drag turns off the clicks-only one. That is why here the drag stays on and not the buttons. The
    // encoding is indeed separate and coexists with any of the three.
    assert!(drag && extended);
    assert!(!buttons && !motion);
    assert!(screen.mouse_reporting().wants_mouse());
    // And the other question of the new surface, which returns a `bool` and not the engine's mode.
    let _: bool = screen.wheel_acts_as_arrows();
}
