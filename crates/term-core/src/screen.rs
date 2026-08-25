//! El estado del terminal: los bytes se interpretan y quedan en una rejilla.

use alacritty_terminal::Term;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Config;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::Processor;

use crate::Size;
use crate::view::{self, CellWidth, Cursor, View, ViewCell};

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

/// Una celda de la rejilla **visible**, y de qué mitad de ella se habla.
///
/// **Vocabulario propio**, por lo mismo que [`Desplazamiento`]: quien use esta caja no tiene que conocer
/// los tipos del motor.
///
/// **Las coordenadas son las de lo que se ve**, con el desplazamiento del historial ya aplicado — o sea
/// las mismas con las que se indexa [`View`]. Quien pinta cuenta filas y columnas desde arriba a la
/// izquierda de lo que tiene delante, y obligarle a restar el desplazamiento sería pedirle que sepa que
/// existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Celda {
    pub fila: u16,
    pub columna: u16,
    /// De qué mitad de la celda se habla.
    ///
    /// **No es un detalle de precisión: sin esto no se puede seleccionar un solo carácter.** Empezar y
    /// terminar en la misma celda da una selección vacía o de un carácter según por qué lado se entró y
    /// por cuál se sale, que es exactamente lo que hace el ratón en cualquier terminal.
    pub mitad: Mitad,
}

/// De qué mitad de una celda se habla, para empezar o terminar una selección.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mitad {
    Izquierda,
    Derecha,
}

impl Mitad {
    /// Cuál de las dos mitades cae en esa fracción del ancho de la celda.
    ///
    /// **Vive aquí y no en quien pinta** para que la regla —la mitad exacta— esté escrita una vez. Quien
    /// pinta sabe puntos y celdas; dónde cae la frontera entre las dos mitades es de esta caja.
    pub fn de_la_fraccion(dentro: f32) -> Self {
        if dentro < 0.5 {
            Self::Izquierda
        } else {
            Self::Derecha
        }
    }
}

/// Traduce la mitad de la celda al vocabulario del motor.
fn mitad_del_motor(mitad: Mitad) -> Side {
    match mitad {
        Mitad::Izquierda => Side::Left,
        Mitad::Derecha => Side::Right,
    }
}

/// Qué le ha pedido al terminal el programa de dentro sobre el ratón.
///
/// **Vocabulario propio y no las banderas del motor**, por lo mismo que [`Desplazamiento`] y
/// [`Celda`]: quien use esta caja no tiene que conocer sus tipos ni migrar con ellos.
///
/// **Lo que hay aquí es lo que el programa PIDE, no lo que la ventana decide hacer.** El orden de
/// prioridades —cuándo gana el historial, cuándo el programa— es de quien reparte los gestos, no de
/// esta caja: aquí solo se contesta la pregunta.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RatonReclamado {
    /// Quiere que se le avise de apretar y de soltar los botones.
    pub botones: bool,
    /// Quiere que se le avise también mientras se arrastra con un botón apretado.
    pub arrastre: bool,
    /// Quiere que se le avise del movimiento **aunque no haya ningún botón apretado**.
    pub movimiento: bool,
    /// Quiere el informe en la codificación extendida, la que no tiene techo de columna.
    pub extendido: bool,
}

impl RatonReclamado {
    /// Si el programa quiere el ratón para algo.
    ///
    /// **`extendido` no cuenta**: dice *cómo* quiere el informe, no *si* lo quiere. Un programa que
    /// solo encendiera esa bandera no ha pedido nada, y mandarle informes sería hablarle sin que
    /// preguntara.
    pub fn lo_quiere(&self) -> bool {
        self.botones || self.arrastre || self.movimiento
    }
}

/// Cuánto se sube o se baja por el historial de una consola.
///
/// **Vocabulario propio, no el del motor**, por lo mismo que el resto de esta caja: quien la use no
/// tiene que conocer los tipos de sus dependencias ni migrar con ellos.
///
/// Los números son **líneas**, positivos hacia lo viejo — la dirección en la que se sube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desplazamiento {
    /// Tantas líneas hacia lo viejo; en negativo, hacia lo nuevo.
    Lineas(i32),
    /// Tantas pantallas hacia lo viejo; en negativo, hacia lo nuevo. **La pantalla la decide la
    /// rejilla**, no un número fijo: una consola estrecha y alta no tiene la misma página que una
    /// ancha y baja.
    Paginas(i32),
    /// Del todo hacia lo viejo.
    AlPrincipio,
    /// Del todo hacia lo nuevo, que es donde el programa escribe.
    AlFinal,
}

impl Desplazamiento {
    /// Lo traduce a lo que el motor entiende. **Privado a la caja**: el tipo del motor no sale de
    /// aquí.
    fn al_motor(self, filas: u16) -> Scroll {
        match self {
            Self::Lineas(cuantas) => Scroll::Delta(cuantas),
            // Una página es la pantalla entera menos una línea, que es lo que hace cualquier
            // paginador: deja una de contexto para no perder el hilo entre pantalla y pantalla.
            Self::Paginas(cuantas) => Scroll::Delta(cuantas * (filas.saturating_sub(1)) as i32),
            Self::AlPrincipio => Scroll::Top,
            Self::AlFinal => Scroll::Bottom,
        }
    }
}

/// Cuántas líneas recuerda cada consola de lo que salió por arriba.
///
/// **El número lo pone este proyecto y no se hereda del motor**, que trae 10 000. Se eligió midiendo
/// —el banco está en `examples/historial.rs`— y la decisión entera, con la tabla y lo descartado, es
/// **D21** de `docs/02-decisiones.md`. Lo que hay que saber para tocarlo:
///
/// - **Es el defecto de WezTerm**, que es el listón contra el que este proyecto se mide.
/// - **Recolocar el texto al cambiar el ancho recorre el historial entero.** Con estas líneas y
///   contenido que envuelve son ~984 µs por consola; con las 10 000 del motor, 4,07 ms — y una
///   frontera mueve **dos** consolas, así que serían más de ocho milisegundos por fotograma
///   **mientras se arrastra**.
/// - **Guardar historial encarece interpretar un volcado un 85 %**, de 182 a 98 MiB/s. Ese precio
///   está declarado y aceptado en D21; no es un descuido que alguien pueda «arreglar» bajando esto
///   sin leer la decisión.
/// - **Hasta 14,4 MiB por consola** con el historial lleno a 180 columnas, y no hay techo de
///   divisiones (**D14**).
pub const HISTORIAL: usize = 3_500;

impl Screen {
    /// Crea una pantalla vacía del tamaño dado.
    pub fn new(size: Size) -> Self {
        let config = Config {
            scrolling_history: HISTORIAL,
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

    /// Sube o baja por lo que salió por arriba.
    ///
    /// **No le llega nada al programa de dentro**: mirar el historial es cosa del terminal, y el
    /// programa ni se entera ni tiene por qué. Es lo que separa esto de mandarle las flechas.
    ///
    /// **Y el desplazamiento se queda donde está aunque siga entrando salida.** Lo sostiene el propio
    /// motor: mientras hay desplazamiento, lo que llega se acumula por debajo sin mover lo que se
    /// mira. Es lo que uno espera al parar a leer algo mientras un programa sigue escribiendo.
    pub fn desplazar(&mut self, cuanto: Desplazamiento) {
        self.term.scroll_display(cuanto.al_motor(self.size().rows));
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

    /// Qué le ha pedido el programa de dentro al terminal sobre el ratón.
    ///
    /// **Devuelve un tipo propio y no las banderas del motor**, por la misma razón que
    /// [`pegado_marcado`](Screen::pegado_marcado): esta caja existe para sobrevivir a que la interfaz
    /// se reescriba, y lo consigue no dejando asomar los tipos de sus dependencias por su API.
    pub fn raton_reclamado(&self) -> RatonReclamado {
        let modo = self.term.mode();
        RatonReclamado {
            botones: modo.contains(TermMode::MOUSE_REPORT_CLICK),
            arrastre: modo.contains(TermMode::MOUSE_DRAG),
            movimiento: modo.contains(TermMode::MOUSE_MOTION),
            extendido: modo.contains(TermMode::SGR_MOUSE),
        }
    }

    /// Si girar la rueda tiene que llegarle al programa **como flechas**.
    ///
    /// Es lo que hace que `less` y `man` se desplacen con la rueda sin pedir el ratón: piden la
    /// pantalla alterna y encienden el desplazamiento alternativo, y el terminal les traduce el giro a
    /// las teclas que ya saben atender.
    ///
    /// **El desplazamiento alternativo viene encendido de fábrica** —está en el modo por defecto del
    /// motor, leído y no supuesto, igual que hace `xterm`—, así que en la práctica lo que decide es la
    /// pantalla alterna. La bandera sigue preguntándose porque un programa **puede apagarla**: `vim` lo
    /// hace en cuanto quiere el ratón para sí, y darle flechas entonces movería su punto de inserción
    /// sin que nadie tocara una tecla.
    pub fn la_rueda_va_como_flechas(&self) -> bool {
        let modo = self.term.mode();
        modo.contains(TermMode::ALT_SCREEN) && modo.contains(TermMode::ALTERNATE_SCROLL)
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
        // Vacío en el caso corriente: solo crece con las celdas que llevan algo pegado.
        let mut adjuntos = Vec::new();

        let contenido = self.term.renderable_content();
        // **El rango de la selección se saca UNA vez y se pregunta por celda**, y quien contesta es el
        // motor: sus dos reglas finas —el acompañante de un carácter ancho va con su cabeza, y el cursor
        // de bloque no se realza en los extremos— no se rehacen aquí.
        let seleccion = contenido.selection;
        let forma_del_cursor = contenido.cursor.shape;
        // Las coordenadas del iterador y las del cursor van en el mismo sistema, que cuenta desde
        // el principio del historial. Sumar el desplazamiento las lleva a coordenadas visibles, que
        // son con las que quien pinta puede indexar. Para las **celdas** la cuenta basta —el
        // iterador solo recorre lo visible—; para el **cursor** no, porque puede quedarse fuera:
        // ver más abajo.
        let desplazamiento = contenido.display_offset as i32;

        // **Si la celda anterior era la cabeza de un carácter ancho Y estaba realzada.** Ver abajo por
        // qué hace falta llevar la cuenta.
        let mut venia_cabeza_ancha_realzada = false;
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
            let seleccionada = seleccion.is_some_and(|rango| {
                rango.contains_cell(&indexada, contenido.cursor.point, forma_del_cursor)
            });
            let mut celda = view::celda_desde(indexada.cell, seleccionada, &mut adjuntos);
            // **El acompañante de un carácter ancho se realza con su cabeza, y esto lo pone ESTA caja.**
            // El motor hace solo el camino contrario —marca la cabeza cuando el acompañante cae dentro,
            // para que su texto no salga cortado— y le basta porque su pintor dibuja el fondo de un
            // carácter ancho de dos celdas de ancho. El nuestro pinta **el fondo de cada celda por
            // separado**, así que sin esto un carácter ancho en el borde de la selección sale con media
            // letra realzada y media no, justo donde hay CJK o emoji.
            //
            // Se lleva con una bandera y no con una segunda pasada porque el recorrido es por filas y el
            // acompañante viene **justo detrás** de su cabeza: una pasada más sobre la rejilla entera se
            // paga en todos los fotogramas para arreglar dos celdas.
            if celda.width == CellWidth::WideTail && venia_cabeza_ancha_realzada {
                celda.selected = true;
            }
            venia_cabeza_ancha_realzada = celda.selected && celda.width == CellWidth::WideHead;
            celdas[fila as usize * columnas + columna] = celda;
        }

        // **El cursor puede no verse, y ahora se dice en vez de recortarse.** Subiendo por el
        // historial el cursor sigue en el área activa, que queda por debajo de lo que se mira:
        // recortarlo a la última fila lo dibujaría en un sitio donde no está, y quien mirase creería
        // que el programa escribe ahí. Esto es exactamente lo que el comentario de este sitio
        // anunciaba desde F1.2, cuando el historial no existía.
        let fila_cursor = contenido.cursor.point.line.0 + desplazamiento;
        let cursor = (0..size.rows as i32)
            .contains(&fila_cursor)
            .then(|| Cursor {
                row: fila_cursor as u16,
                col: (contenido.cursor.point.column.0 as u16).min(size.cols.saturating_sub(1)),
                shape: view::forma_desde(contenido.cursor.shape),
            });

        View::new(size, celdas, adjuntos, cursor, contenido.display_offset)
    }

    /// Empieza una selección en esa celda, **tirando la que hubiera**.
    ///
    /// **El punto es de la rejilla visible** y aquí se convierte al sistema del motor, que cuenta desde
    /// el principio del historial: restar el desplazamiento es lo que hace que seleccionar algo que se
    /// fue por arriba marque **eso** y no lo que hoy ocupa su sitio en pantalla.
    ///
    /// **No se comprueba que la celda exista.** El motor acota el punto por su cuenta al convertirlo a
    /// rango, y una celda fuera de la rejilla no es un error de quien llama: es el ratón saliéndose de la
    /// consola mientras se arrastra, que es el caso normal.
    pub fn empezar_seleccion(&mut self, donde: Celda) {
        self.term.selection = Some(Selection::new(
            SelectionType::Simple,
            self.punto_del_motor(donde),
            mitad_del_motor(donde.mitad),
        ));
    }

    /// Lleva hasta esa celda la selección empezada, si hay alguna.
    ///
    /// **Sin selección empezada no hace nada, y eso no es una guarda por si acaso**: el arrastre se
    /// atiende en cada fotograma mientras el botón está apretado, y el primero de todos es el que la
    /// empieza. Los demás llegan aquí sin que nadie haya soltado nada.
    pub fn extender_seleccion(&mut self, hasta: Celda) {
        let punto = self.punto_del_motor(hasta);
        let mitad = mitad_del_motor(hasta.mitad);
        if let Some(seleccion) = self.term.selection.as_mut() {
            seleccion.update(punto, mitad);
        }
    }

    /// Tira la selección que hubiera. Sin selección, no hace nada.
    pub fn quitar_la_seleccion(&mut self) {
        self.term.selection = None;
    }

    /// Si hay algo seleccionado ahora mismo.
    ///
    /// **No es lo mismo que «se ha empezado una selección»**: arrastrar y volver al punto de partida deja
    /// una selección empezada y **vacía**, y de ahí no se copia nada. Quien pregunte para decidir si
    /// copiar tiene que preguntar esto.
    pub fn hay_seleccion(&self) -> bool {
        self.term
            .selection
            .as_ref()
            .is_some_and(|seleccion| !seleccion.is_empty())
    }

    /// El texto de lo seleccionado, o nada si no hay selección.
    ///
    /// **Las reglas de qué es «el texto» son del motor y están escritas en D22 del registro de decisiones**:
    /// las líneas que envuelven
    /// se unen **sin salto**, el relleno del final de cada línea se **recorta**, el acompañante de un
    /// carácter ancho **no se duplica** y el hueco de una tabulación se salta. Heredar el código es
    /// correcto; heredar la decisión sin decirla, no.
    ///
    /// **No filtra caracteres de control**, al revés que pegar: lo que sale de aquí son caracteres de
    /// celdas y el motor no guarda escapes. Pegar filtra porque protege al programa de dentro de lo que
    /// entra; aquí la dirección es la contraria.
    pub fn texto_seleccionado(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    /// Lleva una celda de lo visible al sistema de coordenadas del motor.
    fn punto_del_motor(&self, celda: Celda) -> Point {
        let desplazamiento = self.term.grid().display_offset() as i32;
        Point::new(
            Line(i32::from(celda.fila) - desplazamiento),
            Column(usize::from(celda.columna)),
        )
    }

    /// Vuelca **el área activa** a texto — donde el programa escribe, no necesariamente lo que se
    /// está mirando.
    ///
    /// **La diferencia importa desde que hay historial, y por eso está dicha así.** Subiendo por lo
    /// que salió por arriba, lo que se ve **no** es el área activa; esto sigue devolviendo el área
    /// activa, que es lo que quiere quien comprueba qué escribió un programa. Quien necesite lo que
    /// se ve tiene [`view`](Screen::view), que sí lleva el desplazamiento aplicado.
    ///
    /// Recorta los espacios finales de cada línea, descarta las líneas vacías del final y une con
    /// saltos de línea, sin salto al final. Una pantalla sin contenido devuelve la cadena vacía.
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
                // **Y lo que va pegado detrás, que también lo escribió el programa.** Sin esto, `☁️`
                // sale como `☁` —el selector de emoji no ocupa columna pero SÍ es texto—, y devolver
                // eso es devolver un carácter **distinto** del que se escribió. Es la misma regla que
                // **D22** fija para copiar: lo que sale de aquí es lo que el programa puso, ni un
                // carácter más ni uno menos.
                linea.extend(celda.zerowidth().into_iter().flatten());
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

    /// Una celda por la que empezar o terminar una selección.
    fn celda(fila: u16, columna: u16, mitad: Mitad) -> Celda {
        Celda {
            fila,
            columna,
            mitad,
        }
    }

    /// Selecciona de la primera celda a la última **incluida**, que es lo que hace un arrastre.
    fn seleccionar(pantalla: &mut Screen, desde: (u16, u16), hasta: (u16, u16)) {
        pantalla.empezar_seleccion(celda(desde.0, desde.1, Mitad::Izquierda));
        pantalla.extender_seleccion(celda(hasta.0, hasta.1, Mitad::Derecha));
    }

    #[test]
    fn una_linea_que_envuelve_se_copia_sin_el_salto_que_no_escribio_nadie() {
        // **La primera de las reglas de qué es «el texto», y es la que más se nota.** Una línea más
        // larga que la rejilla la parte el terminal, no el programa: copiarla con un salto dentro
        // devuelve algo que **nadie escribió**, y pegarlo en un shell ejecuta dos órdenes en vez de una.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 10 });
        pantalla.advance(b"0123456789abcde");

        seleccionar(&mut pantalla, (0, 0), (1, 4));

        assert_eq!(
            pantalla.texto_seleccionado().as_deref(),
            Some("0123456789abcde"),
            "la línea envuelta se copió partida"
        );
    }

    #[test]
    fn dos_lineas_de_verdad_se_copian_con_su_salto() {
        // **El otro lado de la regla, y sin esta prueba la de arriba se cumple quitando todos los
        // saltos.** Lo que distingue una línea envuelta de dos líneas es quién puso el final: el
        // terminal o el programa.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 10 });
        pantalla.advance(b"uno\r\ndos");

        seleccionar(&mut pantalla, (0, 0), (1, 2));

        assert_eq!(
            pantalla.texto_seleccionado().as_deref(),
            Some("uno\ndos"),
            "dos líneas escritas por el programa se pegaron en una"
        );
    }

    #[test]
    fn el_relleno_del_final_de_la_linea_no_se_copia() {
        // **Seleccionar hasta el borde derecho no arrastra los espacios que nadie escribió.** Una
        // rejilla está llena de celdas vacías a la derecha de lo escrito; copiarlas convierte cualquier
        // pegado en una línea con cola de espacios.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"hola");

        seleccionar(&mut pantalla, (0, 0), (0, 19));

        assert_eq!(
            pantalla.texto_seleccionado().as_deref(),
            Some("hola"),
            "se copió el relleno del final de la línea"
        );
    }

    #[test]
    fn el_hueco_de_una_tabulacion_no_se_copia_como_espacios() {
        // **La cuarta regla de qué es «el texto».** Una tabulación deja celdas vacías hasta la siguiente
        // parada; copiarlas como espacios convierte una tabla alineada en una fila de espacios sueltos, y
        // pegarla en cualquier sitio que vuelva a tabular la descoloca.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"a\tb");

        seleccionar(&mut pantalla, (0, 0), (0, 8));

        assert_eq!(
            pantalla.texto_seleccionado().as_deref(),
            Some("a\tb"),
            "el hueco de la tabulación se copió como espacios"
        );
    }

    #[test]
    fn el_acompanante_de_un_caracter_ancho_no_se_duplica_al_copiar() {
        // **Un carácter ancho ocupa dos celdas y es UNO.** La segunda es su acompañante: copiar las dos
        // devolvería el carácter repetido, que es lo que se ve al pegar CJK o un emoji.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance("日本".as_bytes());

        seleccionar(&mut pantalla, (0, 0), (0, 3));

        assert_eq!(
            pantalla.texto_seleccionado().as_deref(),
            Some("日本"),
            "el acompañante del carácter ancho se copió como carácter"
        );
    }

    #[test]
    fn el_realce_de_un_caracter_ancho_cubre_sus_dos_celdas() {
        // **Un carácter ancho ocupa dos celdas y se ve como una cosa**, así que realzar solo la primera
        // lo parte por la mitad: media letra sobre el fondo de la selección y media sobre el de la
        // consola. Quien decide esto es el motor —el acompañante va con su cabeza—, y esta prueba es lo
        // que impide rehacer la regla aquí con una comparación de coordenadas que no la conoce.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance("日x".as_bytes());

        // Solo la CABEZA del carácter ancho: la columna 1 es su acompañante y no se toca.
        pantalla.empezar_seleccion(celda(0, 0, Mitad::Izquierda));
        pantalla.extender_seleccion(celda(0, 0, Mitad::Derecha));

        let vista = pantalla.view();
        let realzadas: Vec<usize> = (0..3)
            .filter(|columna| {
                vista
                    .cell(0, *columna as u16)
                    .expect("la celda está en la rejilla")
                    .selected
            })
            .collect();

        assert_eq!(
            realzadas,
            vec![0, 1],
            "el realce no cubrió las dos celdas del carácter ancho: se ve partido por la mitad"
        );
    }

    #[test]
    fn marcar_el_acompanante_de_un_caracter_ancho_realza_tambien_su_cabeza() {
        // **El camino contrario del de arriba, y lo pone el motor.** Arrastrando desde la derecha se
        // entra por el acompañante antes que por la cabeza: sin esta regla, el primer fotograma del
        // gesto realzaría la mitad vacía del carácter y no la que lleva la letra. Rehacer el realce aquí
        // con una comparación de coordenadas se lleva esta regla por delante sin que nada se queje.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance("日x".as_bytes());

        // Solo el ACOMPAÑANTE: la columna 1.
        pantalla.empezar_seleccion(celda(0, 1, Mitad::Izquierda));
        pantalla.extender_seleccion(celda(0, 1, Mitad::Derecha));

        let vista = pantalla.view();
        assert!(
            vista
                .cell(0, 0)
                .expect("la celda está en la rejilla")
                .selected,
            "se marcó el acompañante y su cabeza —la que lleva la letra— se quedó sin realzar"
        );
    }

    #[test]
    fn arrastrar_y_volver_al_punto_de_partida_no_deja_nada_que_copiar() {
        // **«Hay selección empezada» y «hay algo seleccionado» no son lo mismo**, y quien decide si
        // copiar tiene que preguntar lo segundo: un clic seco deja lo primero puesto, y copiar entonces
        // devolvería la celda de debajo del puntero sin que nadie la haya seleccionado.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"hola mundo");

        pantalla.empezar_seleccion(celda(0, 2, Mitad::Izquierda));
        pantalla.extender_seleccion(celda(0, 2, Mitad::Izquierda));

        assert!(
            !pantalla.hay_seleccion(),
            "un clic sin arrastrar dejó algo seleccionado"
        );
    }

    #[test]
    fn cambiar_de_columnas_borra_la_seleccion_y_conservarlas_la_conserva() {
        // **Las dos mitades de la misma regla heredada del motor, y hacen falta las dos.** Al cambiar el
        // número de columnas el texto se recoloca: lo que estaba marcado deja de estar donde estaba, y
        // conservar el rango dejaría realzado un trozo que nadie eligió. Con las mismas columnas no se
        // recoloca nada, así que la selección **sobrevive** — y sin esta segunda mitad, «borrar siempre»
        // pasaría la prueba.
        let mut pantalla = Screen::new(Size { rows: 6, cols: 20 });
        pantalla.advance(b"hola mundo");
        seleccionar(&mut pantalla, (0, 0), (0, 3));
        assert!(pantalla.hay_seleccion(), "no se llegó a seleccionar nada");

        pantalla.resize(Size { rows: 3, cols: 20 });
        assert!(
            pantalla.hay_seleccion(),
            "cambiar solo las filas borró la selección: no se recoloca nada, así que no hay motivo"
        );

        pantalla.resize(Size { rows: 3, cols: 10 });
        assert!(
            !pantalla.hay_seleccion(),
            "cambiar de columnas dejó la selección puesta, realzando un trozo que nadie eligió"
        );
    }

    #[test]
    fn saltar_a_pantalla_alterna_borra_la_seleccion() {
        // **Lo que se marcó no está en la pantalla nueva.** Un programa de pantalla completa —`vim`,
        // `less`, `htop`— cambia a la alterna al arrancar, y una selección que sobreviviera quedaría
        // realzando celdas de un contenido que ya no existe.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"hola mundo");
        seleccionar(&mut pantalla, (0, 0), (0, 3));
        assert!(pantalla.hay_seleccion(), "no se llegó a seleccionar nada");

        pantalla.advance(b"\x1b[?1049h");

        assert!(
            !pantalla.hay_seleccion(),
            "la selección sobrevivió al salto a pantalla alterna"
        );
    }

    #[test]
    fn limpiar_una_region_que_cruza_la_seleccion_la_borra() {
        // **El programa borró lo que estaba marcado.** Si la selección sobreviviera, seguiría realzando
        // celdas vacías y copiarlas devolvería nada — o peor, lo que el programa escriba después ahí.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"hola mundo");
        seleccionar(&mut pantalla, (0, 0), (0, 3));
        assert!(pantalla.hay_seleccion(), "no se llegó a seleccionar nada");

        // Limpiar la pantalla entera, que es lo que hace `clear`.
        pantalla.advance(b"\x1b[2J");

        assert!(
            !pantalla.hay_seleccion(),
            "se limpió lo que estaba marcado y la selección se quedó ahí"
        );
    }

    #[test]
    fn se_selecciona_lo_que_se_ve_tambien_subido_por_el_historial() {
        // **Las coordenadas de la selección son las de lo que se MIRA**, no las del área activa. Subido
        // por el historial, marcar la primera fila tiene que dar la línea vieja que se está mirando; con
        // las coordenadas sin trasladar daría la que hoy ocupa ese sitio en el área activa.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..40 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }
        pantalla.desplazar(Desplazamiento::AlPrincipio);

        seleccionar(&mut pantalla, (0, 0), (0, 7));

        assert_eq!(
            pantalla.texto_seleccionado().as_deref(),
            Some("linea 0"),
            "se seleccionó por coordenadas del área activa en vez de por lo que se ve"
        );
    }

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

        let cursor = pantalla
            .view()
            .cursor()
            .expect("con la pantalla al día el cursor se ve");

        assert_eq!((cursor.row, cursor.col), (0, 4));
        assert_eq!(cursor.shape, CursorShape::Block);
    }

    #[test]
    fn un_cursor_apagado_se_dice_como_tal() {
        let mut pantalla = Screen::new(Size::default());
        pantalla.advance(b"\x1b[?25l");

        assert_eq!(
            pantalla
                .view()
                .cursor()
                .expect("escondido no es lo mismo que fuera de pantalla")
                .shape,
            CursorShape::Hidden
        );
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

    /// **Lo que sale por arriba se recuerda, y esa es la mitad que no existía.** Con el historial
    /// apagado —como estaba— subir no llevaba a ninguna parte: la primera línea de un volcado se
    /// perdía en cuanto la pantalla se llenaba.
    #[test]
    fn lo_que_sale_por_arriba_se_puede_volver_a_ver() {
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }

        // Sin subir, lo primero ya no está: la pantalla son cuatro filas.
        assert!(!texto_de(&pantalla.view()).contains("linea 0"));

        pantalla.desplazar(Desplazamiento::AlPrincipio);

        assert!(
            texto_de(&pantalla.view()).contains("linea 0"),
            "subiendo del todo tenía que aparecer la primera línea del volcado"
        );
    }

    /// **Y el techo es el techo**: pasadas las líneas que se recuerdan, lo más viejo se cae. Sin esta
    /// prueba, un techo mal puesto —o el del motor, que es tres veces mayor— no lo notaría nadie
    /// hasta ver la memoria.
    #[test]
    fn del_historial_no_se_recuerda_mas_de_lo_que_dice_el_techo() {
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        // Bastante más de lo que cabe, para que el principio tenga que haberse caído.
        for n in 0..(HISTORIAL + 500) {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }

        pantalla.desplazar(Desplazamiento::AlPrincipio);

        // **Subir del todo deja el desplazamiento en lo que se recuerda**, así que este número *es*
        // el techo. Afirmar sobre él —y no sobre qué línea quedó la primera— es lo que hace que la
        // prueba caiga si alguien cambia la constante o hereda la del motor, sin depender de cómo
        // cuente el motor las filas de la pantalla.
        assert_eq!(
            pantalla.view().desplazamiento(),
            HISTORIAL,
            "el historial guardó otra cosa de lo que dice su techo"
        );
        let arriba = texto_de(&pantalla.view());
        assert!(
            !arriba.starts_with("linea 0"),
            "se escribió más de lo que cabe, así que lo más viejo tenía que haberse caído: {}",
            arriba.lines().next().unwrap_or("")
        );
    }

    /// **Subiendo por el historial, el cursor deja de verse — y la foto lo dice en vez de recortarlo.**
    /// Recortarlo lo dibujaba en la última fila, o sea en un sitio donde no está: quien mirase creería
    /// que el programa escribe ahí. Es el caso que el comentario de `view` anunciaba desde F1.2.
    #[test]
    fn subiendo_por_el_historial_el_cursor_deja_de_verse() {
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }
        assert!(
            pantalla.view().cursor().is_some(),
            "sin subir, el cursor tiene que verse"
        );

        pantalla.desplazar(Desplazamiento::AlPrincipio);

        assert_eq!(
            pantalla.view().cursor(),
            None,
            "arriba del todo el cursor está fuera de la pantalla, y decirlo es la única forma honesta"
        );
    }

    /// **La foto dice cuánto se ha subido**, que es lo que separa una pantalla quieta de una que se
    /// quedó atrás mientras el programa seguía escribiendo.
    #[test]
    fn la_foto_dice_cuanto_se_ha_subido() {
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }
        assert_eq!(pantalla.view().desplazamiento(), 0);

        pantalla.desplazar(Desplazamiento::Lineas(3));

        assert_eq!(pantalla.view().desplazamiento(), 3);
    }

    /// **Lo que sigue llegando no te devuelve al final.** Es lo que uno espera al parar a leer algo
    /// mientras un programa sigue escribiendo: lo nuevo se acumula por debajo y lo que se mira no se
    /// mueve. Sin esto, subir mientras algo vuelca sería inútil — que es exactamente el caso en el que
    /// hace falta.
    #[test]
    fn lo_que_sigue_llegando_no_devuelve_la_vista_al_final() {
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }
        pantalla.desplazar(Desplazamiento::AlPrincipio);
        let mirando = texto_de(&pantalla.view());

        for n in 20..40 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }

        assert_eq!(
            texto_de(&pantalla.view()),
            mirando,
            "siguió entrando salida y la vista se movió: leer algo mientras un programa escribe sería \
             imposible"
        );
    }

    /// Y **bajando del todo se vuelve a lo último**, que es donde el programa escribe.
    #[test]
    fn bajando_del_todo_se_vuelve_a_donde_el_programa_escribe() {
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }
        let al_dia = texto_de(&pantalla.view());
        pantalla.desplazar(Desplazamiento::AlPrincipio);

        pantalla.desplazar(Desplazamiento::AlFinal);

        assert_eq!(texto_de(&pantalla.view()), al_dia);
        assert!(
            pantalla.view().cursor().is_some(),
            "y el cursor vuelve a verse"
        );
    }

    /// **Una página es la pantalla menos una línea**, como en cualquier paginador: deja una de
    /// contexto para no perder el hilo entre pantalla y pantalla. Con la pantalla entera, la línea de
    /// unión se pierde en cada salto y nadie sabe por dónde iba.
    #[test]
    fn una_pagina_deja_una_linea_de_contexto() {
        let mut pantalla = Screen::new(Size { rows: 10, cols: 20 });
        for n in 0..50 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }

        pantalla.desplazar(Desplazamiento::Paginas(1));

        assert_eq!(
            pantalla.view().desplazamiento(),
            9,
            "una página de diez filas tiene que subir nueve líneas, no diez"
        );
    }

    /// El volcado a texto sigue dando **el área activa** aunque se esté mirando el historial, y su
    /// documentación lo dice así. Es lo que quiere quien comprueba qué escribió un programa; quien
    /// quiera lo que se ve tiene la foto.
    #[test]
    fn el_volcado_a_texto_da_el_area_activa_aunque_se_mire_el_historial() {
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..20 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }
        let activa = pantalla.to_text();

        pantalla.desplazar(Desplazamiento::AlPrincipio);

        assert_eq!(pantalla.to_text(), activa);
        assert!(
            !texto_de(&pantalla.view()).contains("linea 19"),
            "la prueba no está mirando el historial, así que no compara nada"
        );
    }

    #[test]
    fn una_consola_recien_abierta_no_reclama_el_raton() {
        // El caso de todos los días: un shell no pide el ratón, así que el terminal se lo queda entero
        // para seleccionar. Si esto fuera al revés, arrastrar en un `bash` dejaría de marcar texto.
        let pantalla = Screen::new(Size { rows: 4, cols: 20 });

        assert!(
            !pantalla.raton_reclamado().lo_quiere(),
            "un shell sin pedir nada se llevó el ratón"
        );
    }

    #[test]
    fn cada_bandera_del_raton_se_lee_por_separado() {
        // **Las tres se piden con secuencias distintas y significan cosas distintas**: un programa que
        // solo quiere clics no quiere que se le informe de cada píxel que recorre el puntero. Leerlas
        // en bloque mandaría informes de movimiento a `vim`, que pide `1002` y no `1003`.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"\x1b[?1000h");
        assert_eq!(
            pantalla.raton_reclamado(),
            RatonReclamado {
                botones: true,
                ..RatonReclamado::default()
            },
            "pedir los clics encendió algo más"
        );

        pantalla.advance(b"\x1b[?1002h");
        assert!(
            pantalla.raton_reclamado().arrastre,
            "pedir el arrastre no se leyó"
        );
        assert!(
            !pantalla.raton_reclamado().movimiento,
            "pedir el arrastre encendió también el movimiento"
        );
        // **Y los tres se SUSTITUYEN, no se acumulan** —leído en el motor, no supuesto—: pedir el
        // arrastre apaga el de solo clics. Importa porque quien lee estas banderas decide con
        // `lo_quiere()`, que las mira todas: si se acumularan, apagar el arrastre dejaría los clics
        // encendidos y la consola no recuperaría la selección.
        assert!(
            !pantalla.raton_reclamado().botones,
            "pedir el arrastre dejó encendido el modo de solo clics: los tres se sustituyen"
        );

        pantalla.advance(b"\x1b[?1003h");
        assert!(
            pantalla.raton_reclamado().movimiento,
            "pedir el movimiento no se leyó"
        );
    }

    #[test]
    fn pedir_solo_la_codificacion_extendida_no_es_pedir_el_raton() {
        // **`extendido` dice CÓMO se quiere el informe, no SI se quiere.** Contarlo como querer el
        // ratón haría que un programa que enciende la codificación por delante —y muchos la encienden
        // junto con otras cosas al arrancar— se llevara el botón izquierdo sin haberlo pedido, y con
        // él la selección.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"\x1b[?1006h");

        assert!(
            pantalla.raton_reclamado().extendido,
            "la codificación extendida no se leyó"
        );
        assert!(
            !pantalla.raton_reclamado().lo_quiere(),
            "encender solo la codificación se llevó el ratón"
        );
    }

    #[test]
    fn apagar_el_raton_lo_devuelve() {
        // Un programa que termina apaga lo que encendió. Sin esto, salir de `htop` dejaría la consola
        // sin poder seleccionar hasta cerrarla.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"\x1b[?1000h");
        assert!(pantalla.raton_reclamado().lo_quiere());

        pantalla.advance(b"\x1b[?1000l");

        assert!(
            !pantalla.raton_reclamado().lo_quiere(),
            "el programa apagó el ratón y el terminal siguió creyendo que lo quería"
        );
    }

    #[test]
    fn la_rueda_va_como_flechas_solo_en_pantalla_alterna() {
        // **El desplazamiento alternativo viene ENCENDIDO de fábrica** —leído en el motor, no supuesto:
        // su modo por defecto ya lo trae, igual que `xterm`—. Así que lo único que hace falta para que
        // el giro se traduzca es la pantalla alterna, y quien no la pide no recibe flechas: mandárselas
        // a un shell le cambiaría la orden que tiene escrita en cada giro.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        assert!(
            !pantalla.la_rueda_va_como_flechas(),
            "un shell sin pantalla alterna recibió flechas al girar la rueda"
        );

        pantalla.advance(b"\x1b[?1049h");
        assert!(
            pantalla.la_rueda_va_como_flechas(),
            "en pantalla alterna el giro no se tradujo, y `less` no se desplazaría"
        );
    }

    #[test]
    fn un_programa_puede_apagar_el_desplazamiento_alternativo() {
        // La otra mitad de la regla, y la que hace falta porque la bandera viene encendida: un programa
        // de pantalla alterna que la apaga —`vim` lo hace en cuanto quiere el ratón para sí— deja de
        // recibir flechas al girar. Sin esta mitad, girar la rueda dentro de un editor movería el punto
        // de inserción sin que nadie tocara una tecla.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        pantalla.advance(b"\x1b[?1049h");
        assert!(pantalla.la_rueda_va_como_flechas());

        pantalla.advance(b"\x1b[?1007l");

        assert!(
            !pantalla.la_rueda_va_como_flechas(),
            "el programa apagó el desplazamiento alternativo y siguió recibiendo flechas"
        );
    }

    /// Lo que la foto enseña, en texto: es lo que se **ve**, no lo que el programa tiene escrito.
    fn texto_de(vista: &View) -> String {
        (0..vista.size().rows)
            .map(|fila| {
                vista
                    .row(fila)
                    .expect("la fila está dentro")
                    .iter()
                    .map(|celda| celda.c)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    // ── Lo que va pegado a una celda ──────────────────────────────────────────────────────────

    /// Lo pegado a la primera celda de la vista, escrito con las mismas palabras que usa quien pinta.
    fn pegado_a(pantalla: &Screen, columna: u16) -> Vec<char> {
        let vista = pantalla.view();
        let celda = *vista.cell(0, columna).expect("la celda existe");
        vista.adjuntos_de(&celda).to_vec()
    }

    #[test]
    fn el_selector_que_convierte_un_simbolo_en_emoji_llega_a_la_vista() {
        // **Es la razón de ser de este dato.** `☁` y `☁️` son el mismo carácter con presentación
        // distinta, y lo que los separa —el selector U+FE0F— no ocupa columna: se quedaba en el motor,
        // así que las dos formas llegaban a quien pinta **idénticas** y no había forma de distinguirlas.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 10 });
        pantalla.advance("\u{2601}\u{FE0F}".as_bytes());

        assert_eq!(
            pegado_a(&pantalla, 0),
            vec!['\u{FE0F}'],
            "el selector de emoji no llegó a la vista"
        );
    }

    #[test]
    fn el_mismo_simbolo_sin_selector_no_trae_nada_pegado() {
        // **La otra mitad, y sin ella la de arriba se cumple devolviendo siempre el selector.** Lo que
        // se afirma es que la vista distingue los dos casos, no que sepa producir uno.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 10 });
        pantalla.advance("\u{2601}".as_bytes());

        assert!(
            pegado_a(&pantalla, 0).is_empty(),
            "una celda sin nada pegado trajo algo"
        );
    }

    #[test]
    fn cada_celda_se_queda_con_lo_suyo_y_no_con_lo_de_la_vecina() {
        // **El tramo de cada celda apunta a un vector común**, así que dos celdas con cosas pegadas es
        // el caso donde un desplazamiento mal calculado le da a una lo de la otra. Los dos selectores
        // son distintos a propósito: con el mismo, cruzarlos daría el mismo resultado.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 10 });
        pantalla.advance("\u{2601}\u{FE0F}\u{2601}\u{FE0E}".as_bytes());

        assert_eq!(pegado_a(&pantalla, 0), vec!['\u{FE0F}']);
        assert_eq!(pegado_a(&pantalla, 1), vec!['\u{FE0E}']);
    }

    #[test]
    fn lo_pegado_tiene_techo_y_el_motor_no_lo_tiene() {
        // **El `Vec<char>` de la dependencia crece sin tope**, así que un programa que escupa marcas
        // combinantes sobre la misma celda lo hace crecer sin límite: una entrada de fuera sin acotar,
        // el patrón que D8 castigó con la cola de salida. El techo lo pone esta caja al copiar.
        //
        // **La PRIMERA marca es distinta de todas las demás, y ésa es la mitad de la prueba.** Con
        // doscientas marcas iguales, quedarse con las dieciséis **últimas** en vez de con las primeras da
        // el mismo resultado y esta prueba pasa igual — el segundo disfraz que las reglas del proyecto
        // nombran: elegir números con los que el defecto y el arreglo coinciden. Y alternar dos marcas
        // tampoco basta: 200 − 16 = 184 es par, así que el tramo del final arranca con la misma que el
        // del principio y **la prueba vuelve a pasar rota**. Hicieron falta las dos correcciones.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 10 });
        let mut bytes = String::from("a");
        let alternadas: Vec<char> = (0..200)
            .map(|i| match i {
                0 => '\u{0300}',
                i if i % 2 == 0 => '\u{0301}',
                _ => '\u{0308}',
            })
            .collect();
        bytes.extend(alternadas.iter());
        pantalla.advance(bytes.as_bytes());

        let pegados = pegado_a(&pantalla, 0);
        assert_eq!(
            pegados.len(),
            16,
            "lo pegado a una celda pasó del techo de esta caja"
        );
        assert_eq!(
            pegados,
            alternadas[..16].to_vec(),
            "el recorte se quedó con las marcas equivocadas: lo que decide la presentación es lo que va \
             JUSTO detrás del carácter, así que el techo tiene que cortar por el final"
        );
    }
    #[test]
    fn la_celda_no_engorda_sin_que_nadie_se_entere() {
        // **La rejilla se reserva ENTERA cada fotograma** (`vec![ViewCell::default(); filas × columnas]`),
        // así que cada byte que engorde la celda se paga multiplicado por las celdas y por los fotogramas.
        // Este número no es sagrado —lo pegado a una celda ya lo subió de 24 a 28— pero subirlo tiene que
        // ser una decisión que alguien tome mirándola, no un efecto de haber añadido un campo. Y ya ha
        // hecho su trabajo: la primera versión del tramo iba en dos campos anidados y costaba **32**;
        // empaquetarlo en un entero devolvió cuatro bytes por celda, que son cuatro por fotograma también.
        assert_eq!(
            std::mem::size_of::<ViewCell>(),
            28,
            "la celda de la vista cambió de tamaño: mira qué campo lo hizo y si compensa"
        );
    }
    #[test]
    fn el_volcado_a_texto_no_se_deja_lo_que_va_pegado() {
        // **Devolver `☁` donde el programa escribió `☁️` es devolver OTRO carácter.** El selector de
        // emoji no ocupa columna pero es texto igual, y quien lea este volcado —una prueba hoy, un
        // agente leyendo su consola mañana— se lleva algo que nadie escribió. Es la misma regla que
        // **D22** fija para copiar con el ratón, que el motor ya cumple por su cuenta.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 10 });
        pantalla.advance("\u{2601}\u{FE0F}".as_bytes());

        assert_eq!(
            pantalla.to_text(),
            "\u{2601}\u{FE0F}",
            "el volcado a texto se dejó el selector por el camino"
        );
    }

    #[test]
    fn el_volcado_a_texto_de_lo_corriente_no_cambia() {
        // **La otra mitad, y sin ella la de arriba se cumple metiendo caracteres de más.** Lo que el
        // programa no escribió no puede aparecer aquí.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 10 });
        pantalla.advance(b"hola");

        assert_eq!(pantalla.to_text(), "hola");
    }

    #[test]
    fn lo_pegado_va_con_la_cabeza_del_caracter_ancho_y_no_con_su_acompanante() {
        // **Es el caso normal de este hito, no una esquina**: CJK y los emojis son ANCHOS, así que
        // cualquier cosa que se les pegue llega a una celda de dos columnas. Quien pinta recorre celda a
        // celda, y si el acompañante trajera una copia de lo pegado, la mitad derecha de un emoji se
        // pintaría con otra presentación que la izquierda.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 10 });
        pantalla.advance("\u{1F600}\u{FE0F}".as_bytes());

        let vista = pantalla.view();
        let cabeza = *vista.cell(0, 0).expect("la cabeza existe");
        let acompanante = *vista.cell(0, 1).expect("el acompañante existe");

        assert_eq!(cabeza.width, CellWidth::WideHead);
        assert_eq!(acompanante.width, CellWidth::WideTail);
        assert_eq!(
            vista.adjuntos_de(&cabeza),
            &['\u{FE0F}'],
            "la cabeza del carácter ancho perdió lo que llevaba pegado"
        );
        assert!(
            vista.adjuntos_de(&acompanante).is_empty(),
            "el acompañante se quedó con una copia de lo pegado, y no es suyo"
        );
    }

    #[test]
    fn lo_pegado_sigue_ahi_cuando_se_sube_por_el_historial() {
        // **Subir por el historial reconstruye la vista desde otro sitio de la rejilla**, y ahí es donde
        // un dato nuevo se cae sin que nadie lo note: lo que se ve arriba ya no viene del área activa.
        // Si el selector se perdiera al subir, la misma línea se pintaría de una forma mientras se
        // escribe y de otra al volver a mirarla.
        //
        // **Se comprueba primero que la línea NO se ve sin subir**, y eso no es ceremonia: sin ese paso
        // la prueba pasa igual sin desplazarse ni una línea, y entonces no habla del historial. Lo
        // destapó mutarla.
        let mut pantalla = Screen::new(Size { rows: 2, cols: 10 });
        pantalla.advance("\u{2601}\u{FE0F}\r\n".as_bytes());
        for _ in 0..5 {
            pantalla.advance(b"relleno\r\n");
        }

        let sin_subir = pantalla.view();
        assert_ne!(
            sin_subir.cell(0, 0).expect("la fila 0 existe").c,
            '\u{2601}',
            "la línea del selector se ve sin subir: esta prueba no estaría mirando el historial"
        );

        pantalla.desplazar(Desplazamiento::AlPrincipio);
        let vista = pantalla.view();
        assert!(
            vista.desplazamiento() > 0,
            "no se subió por el historial: el desplazamiento sigue en cero"
        );
        let celda = *vista
            .cell(0, 0)
            .expect("la primera línea del historial se ve");
        assert_eq!(celda.c, '\u{2601}');
        assert_eq!(
            vista.adjuntos_de(&celda),
            &['\u{FE0F}'],
            "lo pegado se perdió al subir por el historial"
        );
    }
}
