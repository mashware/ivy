//! El estado del terminal: los bytes se interpretan y quedan en una rejilla.

use alacritty_terminal::Term;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Config;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::Processor;

use crate::Size;
use crate::view::{self, Cursor, View, ViewCell};

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

    /// Si el programa de dentro ha pedido que lo pegado le llegue **marcado como pegado**.
    ///
    /// Un shell que lo pide no ejecuta lo que reciba así aunque traiga saltos de línea: lo deja
    /// escrito para que se revise antes de darle a Enter. Sin la pregunta, quien pega varias líneas
    /// las ejecuta una a una sin haberlas visto, que es lo que este accesor existe para evitar.
    ///
    /// **Devuelve `bool` y no el tipo del motor a propósito.** Esta caja existe para sobrevivir a que
    /// la interfaz se reescriba, y lo consigue no dejando asomar los tipos de sus dependencias por su
    /// API pública: quien la use no tiene que conocerlos ni migrar con ellos.
    pub fn pegado_marcado(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Una foto de la rejilla visible: todas las celdas y el cursor.
    ///
    /// Es el lector que usa quien pinta, y **no toca la pantalla**: a diferencia de consultar los
    /// daños, se puede llamar tantas veces como haga falta y siempre devuelve lo mismo mientras no
    /// entren bytes nuevos. Eso es lo que permite pintar dos veces el mismo frame sin que la
    /// segunda salga en blanco.
    ///
    /// Convive con [`to_text`](Screen::to_text) en vez de sustituirlo: son dos contratos distintos
    /// —uno da texto legible, el otro celdas con atributos— y fusionarlos no lo pide nadie.
    pub fn view(&self) -> View {
        let size = self.size();
        let columnas = size.cols as usize;
        let mut celdas = vec![ViewCell::default(); size.rows as usize * columnas];

        let contenido = self.term.renderable_content();
        // Las coordenadas del iterador y las del cursor van en el mismo sistema, que cuenta desde
        // el principio del historial. Sumar el desplazamiento las lleva a coordenadas visibles, que
        // son con las que quien pinta puede indexar. Hoy vale siempre 0 —esta caja se crea sin
        // historial— y para las **celdas** esta cuenta seguirá valiendo cuando lo haya. Para el
        // **cursor** no: ver más abajo.
        let desplazamiento = contenido.display_offset as i32;

        for indexada in contenido.display_iter {
            let fila = indexada.point.line.0 + desplazamiento;
            let columna = indexada.point.column.0;
            // El iterador no sale de lo visible —comprobado contra su fuente—, así que esto no
            // debería dispararse nunca. Se queda como aserción y no como salto silencioso: si una
            // versión futura cambiara el rango, saltaría en los tests en vez de dejar filas en
            // blanco que nadie sabría explicar.
            debug_assert!(
                fila >= 0 && fila < size.rows as i32 && columna < columnas,
                "el iterador salió de lo visible: fila {fila}, columna {columna}"
            );
            if fila < 0 || fila >= size.rows as i32 || columna >= columnas {
                continue;
            }
            celdas[fila as usize * columnas + columna] = view::celda_desde(indexada.cell);
        }

        // Hoy el cursor siempre cae dentro de lo visible, porque esta caja se crea sin historial.
        // El recorte es red de seguridad para que la vista no pueda salir con un índice fuera de
        // rango, pero **no es una solución**: cuando la Fase 2 traiga scrollback y se pueda subir
        // por el historial, el cursor caerá de verdad fuera de la pantalla y recortarlo lo dibujaría
        // en la última fila, que es un sitio donde no está. La aserción es lo que hará que eso salte
        // en un test en vez de aparecer como un cursor fantasma.
        let fila_cursor = contenido.cursor.point.line.0 + desplazamiento;
        debug_assert!(
            (0..size.rows as i32).contains(&fila_cursor),
            "el cursor cayó fuera de lo visible ({fila_cursor}): con historial hay que decir que no se ve, no recortarlo"
        );
        let cursor = Cursor {
            row: fila_cursor.clamp(0, size.rows.saturating_sub(1) as i32) as u16,
            col: (contenido.cursor.point.column.0 as u16).min(size.cols.saturating_sub(1)),
            shape: view::forma_desde(contenido.cursor.shape),
        };

        View::new(size, celdas, cursor)
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
    use crate::{CellWidth, CursorShape, TermColor, Underline};

    #[test]
    fn el_programa_puede_pedir_y_retirar_que_lo_pegado_le_llegue_marcado() {
        // De este modo depende que pegar varias líneas se deje escrito en vez de ejecutarse línea a
        // línea, así que hay que poder preguntarlo — y que la respuesta siga al programa, que lo
        // pone al arrancar un shell interactivo y lo quita al ceder el terminal a otra cosa.
        let mut pantalla = Screen::new(Size::default());
        assert!(
            !pantalla.pegado_marcado(),
            "nadie lo ha pedido y ya está puesto"
        );

        pantalla.advance(b"\x1b[?2004h");
        assert!(pantalla.pegado_marcado(), "se pidió y no se enteró");

        pantalla.advance(b"\x1b[?2004l");
        assert!(!pantalla.pegado_marcado(), "se retiró y siguió puesto");
    }

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
    fn la_vista_es_densa_y_del_tamano_de_la_rejilla() {
        let pantalla = Screen::new(Size { rows: 3, cols: 10 });

        let vista = pantalla.view();

        assert_eq!(vista.size(), Size { rows: 3, cols: 10 });
        assert_eq!(vista.cells().len(), 30);
        assert_eq!(vista.row(2).expect("falta la última fila").len(), 10);
        assert!(vista.row(3).is_none(), "no debería haber una cuarta fila");
        assert!(vista.cell(0, 10).is_none(), "no debería haber columna 10");
    }

    #[test]
    fn un_caracter_ancho_ocupa_dos_columnas_y_la_segunda_guarda_su_hueco() {
        // Es la razón de que la vista sea densa: si el acompañante se omitiera, quien pinta tendría
        // que llevar la cuenta del desplazamiento, y ahí es donde se descoloca una fila con CJK.
        let mut pantalla = Screen::new(Size::default());
        pantalla.advance("漢a".as_bytes());

        let vista = pantalla.view();

        let primera = vista.cell(0, 0).expect("falta la celda 0");
        assert_eq!(primera.c, '漢');
        assert_eq!(primera.width, CellWidth::WideHead);

        let acompanante = vista.cell(0, 1).expect("falta el acompañante");
        assert_eq!(
            acompanante.width,
            CellWidth::WideTail,
            "la segunda columna del carácter ancho tiene que estar en la vista"
        );

        // Y la letra siguiente cae en la columna 2, no en la 1: el ancho no descolocó la fila.
        let siguiente = vista.cell(0, 2).expect("falta la celda 2");
        assert_eq!(siguiente.c, 'a');
        assert_eq!(siguiente.width, CellWidth::Single);
    }

    #[test]
    fn el_hueco_que_deja_un_ancho_que_no_cabe_se_distingue_de_un_espacio() {
        // Cuatro columnas: «abc» llena tres y el carácter ancho necesita dos. No cabe, así que el
        // terminal deja la última en blanco y lo baja entero. Ese blanco no lo escribió el
        // programa, y es la única señal de que la fila no acaba donde parece.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 4 });
        pantalla.advance("abc漢".as_bytes());

        let vista = pantalla.view();

        assert_eq!(
            vista.cell(0, 3).expect("falta la última columna").width,
            CellWidth::WrapFiller,
            "el hueco de arrastre tiene que distinguirse de un espacio normal"
        );
        assert_eq!(vista.cell(1, 0).expect("falta la fila 1").c, '漢');
        assert_eq!(
            vista.cell(1, 0).expect("falta la fila 1").width,
            CellWidth::WideHead
        );
    }

    #[test]
    fn el_video_inverso_llega_ya_resuelto() {
        let mut normal = Screen::new(Size::default());
        normal.advance(b"X");
        let normal = *normal.view().cell(0, 0).expect("falta la celda");

        let mut inversa = Screen::new(Size::default());
        inversa.advance(b"\x1b[7mX");
        let inversa = *inversa.view().cell(0, 0).expect("falta la celda");

        assert_eq!(
            (inversa.fg, inversa.bg),
            (normal.bg, normal.fg),
            "el vídeo inverso tiene que llegar con los colores ya intercambiados"
        );
    }

    #[test]
    fn los_atributos_y_el_color_llegan_sin_resolver() {
        let mut pantalla = Screen::new(Size::default());
        // Rojo con nombre, negrita, atenuado, cursiva, subrayado y tachado a la vez.
        pantalla.advance(b"\x1b[31;1;2;3;4;9mX");

        let celda = *pantalla.view().cell(0, 0).expect("falta la celda");

        assert_eq!(
            celda.fg,
            TermColor::Indexed(1),
            "el rojo con nombre es la entrada 1 de la paleta"
        );
        assert!(celda.bold && celda.dim && celda.italic && celda.strikeout);
        assert_eq!(celda.underline, Underline::Single);
    }

    #[test]
    fn los_cinco_subrayados_se_distinguen() {
        // Con un booleano, un diagnóstico de LSP en `nvim` —que es ondulado— llegaba a la vista sin
        // ninguna marca y no había forma de recuperarlo.
        let casos = [
            (&b"\x1b[4m"[..], Underline::Single),
            // El doble es `4:2`, no el `21` que se lee por ahí: para este parser `21` es cancelar
            // la negrita. Comprobado en su tabla de atributos, no supuesto.
            (&b"\x1b[4:2m"[..], Underline::Double),
            (&b"\x1b[4:3m"[..], Underline::Curl),
            (&b"\x1b[4:4m"[..], Underline::Dotted),
            (&b"\x1b[4:5m"[..], Underline::Dashed),
            (&b"\x1b[24m"[..], Underline::None),
        ];

        for (secuencia, esperado) in casos {
            let mut pantalla = Screen::new(Size::default());
            pantalla.advance(secuencia);
            pantalla.advance(b"X");

            let celda = *pantalla.view().cell(0, 0).expect("falta la celda");
            assert_eq!(
                celda.underline,
                esperado,
                "con la secuencia {:?}",
                String::from_utf8_lossy(secuencia)
            );
        }
    }

    #[test]
    fn con_video_inverso_la_intensidad_se_refiere_al_color_que_pasa_a_ser_fondo() {
        // Fija el filo que hereda quien resuelva el tema: la intensidad describe el primer plano
        // **declarado**, y con vídeo inverso ese color acaba en `bg`. Aquí lo que hay que atenuar es
        // el rojo, no el fondo del tema. Si esto cambia, que cambie con este test delante.
        let mut pantalla = Screen::new(Size::default());
        pantalla.advance(b"\x1b[2;31;7mX");

        let celda = *pantalla.view().cell(0, 0).expect("falta la celda");

        assert_eq!(celda.fg, TermColor::Background);
        assert_eq!(
            celda.bg,
            TermColor::Indexed(1),
            "el rojo pasa a ser el fondo"
        );
        assert!(celda.dim, "y es el rojo lo que está atenuado, no el fg");
        assert!(
            celda.inverse,
            "y hay que decirlo: sin esta marca, quien pinta no puede saber a cuál de los dos \
             colores se refiere la intensidad, y la aplica al que ya no se ve"
        );
    }

    #[test]
    fn el_cursor_sale_en_coordenadas_visibles() {
        let mut pantalla = Screen::new(Size::default());
        pantalla.advance(b"hola");

        let cursor = pantalla.view().cursor();

        assert_eq!((cursor.row, cursor.col), (0, 4));
        assert_eq!(cursor.shape, CursorShape::Block);
    }

    #[test]
    fn un_cursor_apagado_se_dice_como_tal() {
        let mut pantalla = Screen::new(Size::default());
        pantalla.advance(b"\x1b[?25l");

        assert_eq!(pantalla.view().cursor().shape, CursorShape::Hidden);
    }

    #[test]
    fn leer_la_vista_no_gasta_la_pantalla() {
        // A diferencia de consultar los daños, este lector no es destructivo. Si lo fuera, el
        // segundo frame de un repintado saldría en blanco.
        let mut pantalla = Screen::new(Size::default());
        pantalla.advance(b"hola");

        let primera = pantalla.view();
        let segunda = pantalla.view();

        // Comprobar solo que las dos son iguales no bastaría: si el lector devolviera una vista
        // vacía las dos veces, el test pasaría igual. Hay que ver primero que hay algo que perder.
        assert_eq!(primera.cell(0, 0).expect("falta la celda").c, 'h');
        assert_eq!(primera, segunda);
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
