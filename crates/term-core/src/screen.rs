//! El estado del terminal: los bytes se interpretan y quedan en una rejilla.

use alacritty_terminal::Term;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Config;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::Processor;

use crate::Size;

/// Dimensiones en el vocabulario de `alacritty_terminal`.
///
/// Existe para que ese trait no haya que implementarlo sobre [`Size`], que es público: así la
/// dependencia no asoma en la API de este crate. El crate trae un tipo equivalente, pero vive en
/// un módulo que él mismo declara «ayudas para tests», y construir sobre eso es apoyarse en algo
/// que su autor no se ha comprometido a mantener.
struct Dims {
    rows: usize,
    cols: usize,
}

impl Dimensions for Dims {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

impl From<Size> for Dims {
    fn from(size: Size) -> Self {
        Self {
            rows: size.rows as usize,
            cols: size.cols as usize,
        }
    }
}

/// La pantalla de una división: interpreta bytes de terminal y mantiene la rejilla resultante.
///
/// Se alimenta a mano con [`advance`](Screen::advance), en los trozos que quien la usa decida. Esa
/// es la diferencia con dejar que un hilo la actualice sola: quien pinta controla cuánto trabajo
/// hace por pasada, que es lo que permite que un volcado enorme no se lo coma entero de una vez.
pub struct Screen {
    term: Term<VoidListener>,
    parser: Processor,
}

impl Screen {
    /// Crea una pantalla vacía del tamaño dado.
    pub fn new(size: Size) -> Self {
        let config = Config {
            // Sin historial: el scrollback es Fase 2 y `to_text` solo vuelca lo visible. El valor
            // por defecto son 10 000 líneas por división, memoria que aquí nadie va a leer.
            scrolling_history: 0,
            ..Config::default()
        };

        Self {
            term: Term::new(config, &Dims::from(size), VoidListener),
            parser: Processor::new(),
        }
    }

    /// Interpreta un trozo de salida del terminal.
    ///
    /// Acepta cualquier corte: el parser conserva su estado entre llamadas, así que una secuencia
    /// de escape partida en dos entregas produce el mismo resultado que entera. Es lo que hace
    /// correcto que el PTY entregue lecturas parciales.
    pub fn advance(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    /// Cambia el tamaño de la rejilla y refluye su contenido.
    ///
    /// **Solo cambia la rejilla.** El programa que corre dentro del pseudoterminal no se entera por
    /// esta vía: para eso está [`Pty::resize`](crate::Pty::resize), y hay que llamar a las dos —
    /// **esta primero**. En cuanto el programa sepa el tamaño nuevo empieza a mandar bytes para él,
    /// y si la rejilla siguiera siendo la vieja se interpretarían contra dimensiones equivocadas.
    /// Están separadas porque son dos piezas que no se conocen entre sí, y esconder una dentro de
    /// la otra ataría la rejilla a tener siempre un pseudoterminal detrás — que no es el caso en
    /// los tests, ni tiene por qué serlo después.
    pub fn resize(&mut self, size: Size) {
        self.term.resize(Dims::from(size));
    }

    /// El tamaño actual de la rejilla.
    ///
    /// Se lee de la propia rejilla en vez de guardarse aparte: con dos copias del mismo dato,
    /// redimensionar y actualizar solo una las deja divergir sin que nada falle.
    pub fn size(&self) -> Size {
        let grid = self.term.grid();
        Size {
            rows: grid.screen_lines() as u16,
            cols: grid.columns() as u16,
        }
    }

    /// Vuelca la pantalla visible a texto.
    ///
    /// Recorta los espacios finales de cada línea, descarta las líneas vacías del final y une con
    /// saltos de línea, sin salto al final. Una pantalla sin contenido devuelve la cadena vacía.
    /// No incluye historial.
    ///
    /// Se descartan las líneas vacías finales porque una pantalla casi siempre tiene la mitad de
    /// abajo en blanco, y arrastrar esos saltos convierte cualquier comparación en una cuenta de
    /// líneas invisibles. Cuántas filas tiene la rejilla lo dice [`size`](Screen::size), que es
    /// donde esa información no estorba.
    pub fn to_text(&self) -> String {
        let grid = self.term.grid();
        let mut lineas: Vec<String> = Vec::with_capacity(grid.screen_lines());

        for fila in 0..grid.screen_lines() {
            let fila = &grid[Line(fila as i32)];
            let mut linea = String::with_capacity(grid.columns());
            for columna in 0..grid.columns() {
                let celda = &fila[Column(columna)];
                // Un carácter ancho ocupa dos celdas pero es un solo carácter: la segunda es un
                // acompañante sin contenido y duplicaría el glifo si se copiara.
                if celda.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                linea.push(celda.c);
            }
            lineas.push(linea.trim_end().to_string());
        }

        while lineas.last().is_some_and(String::is_empty) {
            lineas.pop();
        }

        lineas.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_pantalla_recien_creada_no_tiene_contenido() {
        let pantalla = Screen::new(Size::default());

        assert_eq!(pantalla.to_text(), "");
        // El tamaño no se pierde por volcar vacío: lo dice `size`, no el texto.
        assert_eq!(pantalla.size(), Size::default());
    }

    #[test]
    fn las_lineas_vacias_del_final_no_se_arrastran() {
        let mut pantalla = Screen::new(Size::default());

        pantalla.advance(b"primera\r\nsegunda");

        // Dos líneas con contenido, no 24 con veintidós en blanco detrás.
        assert_eq!(pantalla.to_text(), "primera\nsegunda");
    }

    #[test]
    fn el_texto_plano_aparece_sin_secuencias_de_escape() {
        let mut pantalla = Screen::new(Size::default());

        pantalla.advance(b"\x1b[31mhola\x1b[0m");

        let texto = pantalla.to_text();
        assert!(texto.starts_with("hola"), "texto inesperado: {texto:?}");
        assert!(
            !texto.contains('\x1b'),
            "el volcado no debe llevar escapes crudos: {texto:?}"
        );
    }

    #[test]
    fn redimensionar_deja_el_tamano_pedido_y_conserva_el_contenido() {
        let mut pantalla = Screen::new(Size::default());
        pantalla.advance(b"hola");

        pantalla.resize(Size {
            rows: 30,
            cols: 100,
        });

        assert_eq!(
            pantalla.size(),
            Size {
                rows: 30,
                cols: 100
            }
        );
        assert!(
            pantalla.to_text().starts_with("hola"),
            "el contenido no sobrevivió al redimensionado: {:?}",
            pantalla.to_text()
        );
    }

    #[test]
    fn una_secuencia_partida_entre_dos_llamadas_da_el_mismo_resultado() {
        // Es el contrato que sostiene todo el diseño: el PTY entrega lecturas parciales, así que
        // un escape puede llegar cortado por la mitad. Si esto no se cumpliera, habría que
        // acumular bytes antes de interpretarlos y el diseño sería otro.
        let mut entera = Screen::new(Size::default());
        entera.advance(b"\x1b[31mrojo");

        let mut partida = Screen::new(Size::default());
        partida.advance(b"\x1b[");
        partida.advance(b"31mrojo");

        assert_eq!(partida.to_text(), entera.to_text());
        assert!(partida.to_text().starts_with("rojo"));
    }
}
