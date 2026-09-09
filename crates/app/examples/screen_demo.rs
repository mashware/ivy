//! A terminal screen full of traps, painted with nothing alive behind it.
//!
//! There is no shell, no keyboard, no thread bringing anything: the grid is filled by hand with a
//! fixed string and painted. It exists because the painter is the only piece of the milestone **with
//! no possible test** —what has to be checked is how it looks— and without this it would be the only
//! one nobody could verify on their own.
//!
//! The content is not decorative. Each row is placed to show one of the risks the design left noted
//! down:
//!
//! - **Spacing jitter**: the row of packed Ms. If egui snaps each glyph to the pixel grid on its
//!   own, in a monospace it looks like uneven spacing.
//! - **Seam between backgrounds**: the block of continuous background. With antialiasing on, two
//!   adjacent rectangles of the same color can leave a one-pixel line between them.
//! - **Mismatch with wide characters**: the Japanese and the emoji. The grid reserves two columns
//!   for them because the parser says so; if the font paints them wider or narrower, it shows.
//! - And along the way: named colors, colors from the 256 palette, bold, dim and inverse video.
//!
//! It is launched with `cargo run --example screen_demo -p app --release`.

use app::painter::{CellMetrics, paint};
use app::theme::Theme;
use eframe::egui;
use term_core::{Screen, Size};

/// What is written into the grid. Fixed: two executions have to give the same screen.
const SCRIPT: &str = concat!(
    "\x1b[1;33magent-terminal\x1b[0m — pantalla de prueba del pintor\r\n",
    "\r\n",
    "Colores con nombre:  ",
    "\x1b[31mrojo \x1b[32mverde \x1b[33mamarillo \x1b[34mazul \x1b[35mmagenta \x1b[36mcian\x1b[0m\r\n",
    "Los mismos, en negrita: ",
    "\x1b[1;31mrojo \x1b[1;32mverde \x1b[1;33mamarillo \x1b[1;34mazul \x1b[1;35mmagenta \x1b[1;36mcian\x1b[0m\r\n",
    "Y atenuados: ",
    "\x1b[2;31mrojo \x1b[2;32mverde \x1b[2;33mamarillo \x1b[2;34mazul \x1b[2;35mmagenta \x1b[2;36mcian\x1b[0m\r\n",
    "\r\n",
    "De la paleta de 256:  ",
    "\x1b[38;5;196m196 \x1b[38;5;46m46 \x1b[38;5;21m21 \x1b[38;5;226m226 \x1b[38;5;244m244 \x1b[38;5;232m232 \x1b[38;5;255m255\x1b[0m\r\n",
    "Color exacto:         \x1b[38;2;207;106;76mCF6A4C\x1b[0m  ",
    "Vídeo inverso: \x1b[7m seleccionado \x1b[0m\r\n",
    // The inverse combined with intensity: the bold and the dim describe the color the program
    // declared as foreground, which here has become the background. If they were applied to the
    // other, these two blocks would come out identical to each other and to the one above.
    "Inverso + negrita: \x1b[1;31;7m rojo brillante detrás \x1b[0m  ",
    "Inverso + atenuado: \x1b[2;31;7m rojo apagado detrás \x1b[0m\r\n",
    "\r\n",
    "Bloque de fondo continuo — aquí es donde se vería una costura entre celdas:\r\n",
    "\x1b[44m                                                                       \x1b[0m\r\n",
    "\x1b[44m   fondo azul, tres filas seguidas, sin ninguna raya entre ellas       \x1b[0m\r\n",
    "\x1b[44m                                                                       \x1b[0m\r\n",
    "\r\n",
    "Caracteres anchos: 日本語 🎉 — cada uno ocupa dos columnas\r\n",
    "Y entre ellos:     [日]本[語] — los corchetes marcan dónde deberían acabar\r\n",
    // A wide char that **does not fit** at the end of the row: it goes whole to the next one and
    // leaves the last column empty. Without this row, the painter branch that handles that gap is
    // looked at by nobody.
    "Un ancho que no cabe al final de la fila de 80 columnas, justo aquí: ---------日 sigue\r\n",
    "Texto \x1b[8moculto entre estas dos marcas\x1b[0m — entre las marcas no debe verse nada\r\n",
    "\r\n",
    "Temblor de espaciado (todas las emes tienen que estar a la misma distancia):\r\n",
    "MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM\r\n",
    "iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii\r\n",
    "\r\n",
    "El cursor se posa sobre esta M, que tiene que seguir leyéndose: M\x1b[1D",
);

/// The size of the test grid. Fixed, so the screen does not depend on the window.
///
/// Thirty-two rows and not twenty-four: with the twenty-four of a classic terminal the script runs
/// off the bottom, the grid scrolls and **the first row stops being seen**. That a test screen
/// should hide part of what has to be looked at is worse than being unrealistic.
const GRID: Size = Size { rows: 32, cols: 80 };

fn main() -> eframe::Result {
    let mut screen = Screen::new(GRID);
    screen.advance(SCRIPT.as_bytes());

    eframe::run_native(
        "agent-terminal — pantalla de prueba",
        eframe::NativeOptions::default(),
        Box::new(|_cc| {
            Ok(Box::new(Demo {
                screen,
                theme: Theme::default(),
            }))
        }),
    )
}

struct Demo {
    screen: Screen,
    theme: Theme,
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let metrics = CellMetrics::measure(ui.ctx(), egui::FontId::monospace(14.0));

        let view = self.screen.view();
        let origin = ui.max_rect().left_top();
        // **A throwaway discoverer, and on purpose.** This bench measures painting with text that the
        // factory fonts already cover, so there is never anything to discover; and since it does not
        // call `resolve`, a persistent one would only accumulate notes nobody looks at.
        paint(
            ui.painter(),
            &view,
            &self.theme,
            &metrics,
            origin,
            &mut Default::default(),
            app::painter::CursorInk::Filled,
        );
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.theme.background().to_normalized_gamma_f32()
    }
}
