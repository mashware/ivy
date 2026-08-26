//! Una pantalla de terminal llena de trampas, pintada sin nada vivo detrás.
//!
//! No hay shell, ni teclado, ni hilo que traiga nada: la rejilla se rellena a mano con una cadena
//! fija y se pinta. Existe porque el pintor es la única pieza del hito **sin test posible** —lo que
//! hay que comprobar es cómo se ve— y sin esto sería la única que nadie podría verificar por su
//! cuenta.
//!
//! El contenido no es decorativo. Cada fila está puesta para enseñar uno de los riesgos que el
//! diseño dejó anotados:
//!
//! - **Temblor de espaciado**: la fila de emes pegadas. Si egui ajusta cada glifo al retículo de
//!   píxeles por su cuenta, en un monoespaciado se ve como un espaciado irregular.
//! - **Costura entre fondos**: el bloque de fondo continuo. Con el antialias activo, dos
//!   rectángulos contiguos del mismo color pueden dejar una raya de un píxel entre ellos.
//! - **Desajuste con los caracteres anchos**: el japonés y el emoji. La rejilla les reserva dos
//!   columnas porque lo dice el parser; si la fuente los pinta más anchos o más estrechos, se ve.
//! - Y de paso: colores nombrados, colores de la paleta de 256, negrita, atenuado y vídeo inverso.
//!
//! Se lanza con `cargo run --example screen_demo -p app --release`.

use app::painter::{CellMetrics, paint};
use app::theme::Theme;
use eframe::egui;
use term_core::{Screen, Size};

/// Lo que se escribe en la rejilla. Fijo: dos ejecuciones tienen que dar la misma pantalla.
const GUION: &str = concat!(
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
    // El inverso combinado con intensidad: la negrita y el atenuado describen el color que el
    // programa declaró como primer plano, que aquí ha pasado a ser el fondo. Si se aplicaran al
    // otro, estos dos bloques saldrían idénticos entre sí y al de arriba.
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
    // Un ancho que **no cabe** al final de la fila: se va entero a la siguiente y deja la última
    // columna vacía. Sin esta fila, la rama del pintor que trata ese hueco no la mira nadie.
    "Un ancho que no cabe al final de la fila de 80 columnas, justo aquí: ---------日 sigue\r\n",
    "Texto \x1b[8moculto entre estas dos marcas\x1b[0m — entre las marcas no debe verse nada\r\n",
    "\r\n",
    "Temblor de espaciado (todas las emes tienen que estar a la misma distancia):\r\n",
    "MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM\r\n",
    "iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii\r\n",
    "\r\n",
    "El cursor se posa sobre esta M, que tiene que seguir leyéndose: M\x1b[1D",
);

/// El tamaño de la rejilla de prueba. Fijo, para que la pantalla no dependa de la ventana.
///
/// Treinta y dos filas y no veinticuatro: con las veinticuatro de un terminal clásico el guion se
/// sale por abajo, la rejilla desplaza y **la primera fila deja de verse**. Que una pantalla de
/// prueba esconda parte de lo que hay que mirar es peor que ser poco realista.
const REJILLA: Size = Size { rows: 32, cols: 80 };

fn main() -> eframe::Result {
    let mut pantalla = Screen::new(REJILLA);
    pantalla.advance(GUION.as_bytes());

    eframe::run_native(
        "agent-terminal — pantalla de prueba",
        eframe::NativeOptions::default(),
        Box::new(|_cc| {
            Ok(Box::new(Prueba {
                pantalla,
                tema: Theme::default(),
            }))
        }),
    )
}

struct Prueba {
    pantalla: Screen,
    tema: Theme,
}

impl eframe::App for Prueba {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let metrica = CellMetrics::measure(ui.ctx(), egui::FontId::monospace(14.0));

        let vista = self.pantalla.view();
        let origen = ui.max_rect().left_top();
        // **Un descubridor de usar y tirar, y a propósito.** Este banco mide el pintado con texto que las
        // fuentes de fábrica ya cubren, así que nunca hay nada que descubrir; y como no llama a
        // `resolve`, uno persistente solo acumularía apuntes que nadie mira.
        paint(
            ui.painter(),
            &vista,
            &self.tema,
            &metrica,
            origen,
            &mut Default::default(),
        );
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.tema.background().to_normalized_gamma_f32()
    }
}
