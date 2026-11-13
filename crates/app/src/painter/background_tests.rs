//! Selection background regression: inspect the production painter after epaint's pixel rounding.
use super::*;
use eframe::egui::{Id, LayerId, Order, epaint::Primitive};
use term_core::{Cell, Half, Screen};

fn selected_blanks(size: Size) -> View {
    let mut screen = Screen::new(size);
    screen.advance(b"\x1b[?25l");
    screen.start_selection(Cell {
        row: 0,
        col: 0,
        half: Half::Left,
    });
    screen.extend_selection(Cell {
        row: size.rows - 1,
        col: size.cols - 1,
        half: Half::Right,
    });
    let view = screen.view();
    assert!(
        view.cells()
            .iter()
            .all(|cell| cell.selected && cell.c == ' ')
    );
    assert_eq!(view.cursor().unwrap().shape, CursorShape::Hidden);
    view
}

fn assert_background_coverage(ppp: f32, origin: Pos2, feathering: bool, measured: bool) {
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    ctx.tessellation_options_mut(|options| options.feathering = feathering);
    let size = Size {
        rows: 32,
        cols: 160,
    };
    let view = selected_blanks(size);
    let theme = Theme::default();
    let selected_background = theme.color(view.cells()[0].fg);
    let output = ctx.run_ui(Default::default(), |ui| {
        let metrics = if measured {
            CellMetrics::measure(&ctx, FontId::monospace(14.0))
        } else {
            // The small whole-device-pixel steps reproduce f32 half-pixel ties at both scales.
            let physical = if ppp == 1.75 { 9.0 } else { 7.0 };
            CellMetrics {
                width: physical / ppp,
                height: physical / ppp,
                font: FontId::monospace(14.0),
                per_point: ppp,
            }
        };
        let painter = ui
            .ctx()
            .layer_painter(LayerId::new(Order::Foreground, Id::new("selected blanks")));
        paint(
            &painter,
            &view,
            &theme,
            &metrics,
            origin,
            &mut crate::fonts::Discoverer::default(),
            CursorInk::Filled,
        );
    });
    let vertices: Vec<_> = ctx
        .tessellate(output.shapes.clone(), ppp)
        .into_iter()
        .flat_map(|primitive| match primitive.primitive {
            Primitive::Mesh(mesh) => mesh.vertices,
            Primitive::Callback(_) => panic!("unexpected callback in the blank grid"),
        })
        .collect();
    output.drop_without_applying_deltas();
    // Blanks and a hidden cursor leave exactly one selection background quad per cell.
    let per_cell = if feathering { 8 } else { 4 };
    assert_eq!(
        vertices.len(),
        usize::from(size.rows) * usize::from(size.cols) * per_cell
    );
    let bounds: Vec<_> = vertices
        .chunks_exact(per_cell)
        .map(|cell| {
            let opaque: Vec<_> = cell
                .iter()
                .filter(|vertex| vertex.color.a() == 255)
                .collect();
            assert_eq!(opaque.len(), 4);
            assert!(
                opaque
                    .iter()
                    .all(|vertex| vertex.color == selected_background)
            );
            opaque.into_iter().fold(Rect::NOTHING, |bounds, vertex| {
                bounds.union(Rect::from_min_max(vertex.pos * ppp, vertex.pos * ppp))
            })
        })
        .collect();
    // A feather extends half a pixel beyond each opaque edge. Any larger separation
    // leaves an uncovered pixel when independently rounded edges disagree.
    // Exact adjacency needs no tolerance beyond f32 noise.
    let feather = if feathering { 1.0 } else { 0.0 };
    for row in 0..usize::from(size.rows) {
        for col in 0..usize::from(size.cols) {
            let index = row * usize::from(size.cols) + col;
            if col > 0 {
                let gap = bounds[index].left() - bounds[index - 1].right() - feather;
                assert!(
                    gap.abs() < 0.001,
                    "horizontal gap={gap}px at row={row} col={col}, ppp={ppp} AA={feathering} measured={measured}"
                );
            }
            if row > 0 {
                let gap =
                    bounds[index].top() - bounds[index - usize::from(size.cols)].bottom() - feather;
                assert!(
                    gap.abs() < 0.001,
                    "vertical gap={gap}px at row={row} col={col}, ppp={ppp} AA={feathering} measured={measured}"
                );
            }
        }
    }
}

#[test]
fn selected_backgrounds_tile_after_fractional_scale_tessellation() {
    for feathering in [false, true] {
        for (ppp, origin) in [
            (1.0, pos2(1.0, 1.0)),
            (1.5, pos2(1.0, 1.0)),
            (1.75, pos2(2.0, 2.0)),
        ] {
            assert_background_coverage(ppp, origin, feathering, false);
        }
    }
}

#[test]
fn measured_selected_backgrounds_tile_after_fractional_scale_tessellation() {
    for feathering in [false, true] {
        for ppp in [1.5, 1.75] {
            assert_background_coverage(ppp, pos2(2.0, 1.0), feathering, true);
        }
    }
}
