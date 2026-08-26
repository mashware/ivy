//! Cómo se lee la rejilla desde fuera: una vista de celdas y el cursor.
//!
//! Todo lo que hay aquí es **nuestro**. Ningún tipo de `alacritty_terminal` asoma, y no es por
//! gusto: la capa de pintado se va a reescribir —egui ahora, GPUI cuando haga falta— y quien pinte
//! no debería tener que conocer la librería que hay debajo ni migrar con ella.
//!
//! La vista es una **foto**, no una ventana: se construye entera en el momento de pedirla y no
//! guarda ninguna referencia a la pantalla. Eso permite leerla mientras la pantalla sigue
//! avanzando, y es lo que hace que quien pinta pueda quedarse con la suya durante un frame.

use crate::Size;

/// Un color tal y como lo declaró el programa, **sin resolver**.
///
/// Sin resolver a propósito: traducir a píxeles es elegir un tema, y el tema no es asunto de esta
/// caja. Aquí solo se conserva *qué dijo* el programa, para que quien pinte decida qué significa.
///
/// Los dieciséis colores con nombre son, para el terminal, las dieciséis primeras entradas de la
/// paleta, así que se entregan como [`TermColor::Indexed`] en vez de duplicarlos en variantes con
/// nombre. Lo que sí necesita variante propia es lo que **no** es una entrada fija: el primer plano
/// y el fondo del tema, el color del cursor y las formas atenuadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermColor {
    /// Una entrada de la paleta de 256. Las dieciséis primeras son los colores ANSI con nombre.
    Indexed(u8),
    /// Un color exacto pedido por el programa.
    Rgb { r: u8, g: u8, b: u8 },
    /// El primer plano por defecto del tema.
    Foreground,
    /// El fondo por defecto del tema.
    Background,
    /// El color propio del cursor.
    Cursor,
    /// Variante atenuada de una de las ocho primeras entradas de la paleta.
    ///
    /// El terminal distingue «rojo atenuado» de «rojo», y quien pinta decide si son dos colores o
    /// el mismo con menos alfa. Colapsarlos aquí perdería la distinción sin poder recuperarla.
    Dim(u8),
    /// Primer plano brillante del tema.
    BrightForeground,
    /// Primer plano atenuado del tema.
    DimForeground,
}

/// Cuántas columnas ocupa una celda, y cuál de ellas es.
///
/// **Fuente única del ancho.** La librería de debajo lo marca con dos banderas distintas; tenerlo
/// aquí también en un booleano aparte sería el mismo hecho en dos sitios que pueden discrepar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellWidth {
    /// Una columna.
    Single,
    /// Primera de las dos columnas de un carácter ancho. **Es la que lleva el carácter.**
    WideHead,
    /// Segunda columna de un carácter ancho: ocupa su hueco y no tiene carácter propio.
    ///
    /// Va en la vista en vez de omitirse porque la vista es **densa**: la columna `n` de una fila
    /// está siempre en la posición `n`. Saltársela obligaría a quien pinta a llevar la cuenta del
    /// desplazamiento, que es justo el error que descoloca una fila con CJK o emoji.
    WideTail,
    /// Hueco al final de una fila donde un carácter ancho **no cabía**: el carácter se fue entero
    /// a la fila siguiente y esta columna queda vacía a propósito.
    ///
    /// Se distingue de [`CellWidth::Single`] porque no es un espacio que el programa haya escrito,
    /// sino relleno del terminal. Hoy las dos se pintarían igual —el contenido es un espacio— pero
    /// confundirlas al medir columnas o al seleccionar texto daría un resultado que nadie sabría
    /// explicar.
    WrapFiller,
}

/// Estilo de subrayado que pidió el programa.
///
/// Son cinco y no uno, y la diferencia se ve todos los días: el subrayado ondulado es lo que usa
/// un diagnóstico de LSP en `nvim`. Con un booleano, ese error de compilación llegaba a la vista
/// sin ninguna marca y no había forma de recuperarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    /// Ondulado. El de los errores de un LSP.
    Curl,
    Dotted,
    Dashed,
}

/// Cuántos caracteres de anchura cero se conservan pegados a una celda.
///
/// **El motor no tiene ninguno**, y ésa es la razón de que este exista: el `Vec<char>` de
/// `alacritty_terminal 0.26` crece sin tope, así que un programa que escupa marcas combinantes sobre la
/// misma celda lo hace crecer sin límite. Es una entrada de fuera sin acotar — el mismo patrón que **D8**
/// castigó con la cola de salida.
///
/// **Dieciséis, y la razón es acotar, no preservar una secuencia concreta.** La primera versión de este
/// comentario decía que hacían falta seis para no partir una familia de emoji unida por juntadores
/// (`👨‍👩‍👧‍👦`), y **eso es falso con este motor**: se comprobó ejecutándolo contra
/// `alacritty_terminal 0.26`, que solo adjunta caracteres de **anchura cero**, y los cuatro miembros de
/// esa familia son **anchos**. Llegan como **cuatro celdas**, cada una con **un** juntador pegado — no
/// como una celda con seis. Quien de verdad puede apilar muchos es una marca combinante repetida, que no
/// tiene número canónico ninguno.
///
/// Así que este número **no preserva nada concreto**: pone un techo donde el motor no tiene ninguno, y
/// dieciséis es holgado para todo lo que se ha visto llegar. Si algún día algo real se corta aquí, se
/// sube — con la secuencia delante, que es lo que faltaba la primera vez.
const COMBINING_PER_CELL: usize = 16;

// El tramo se empaqueta en un solo entero de 32 bits (ver `CombiningSpan`), así que el techo tiene que caber
// en los cinco bits bajos. Sin esto, subirlo a 32 haría que `count` diera la vuelta **en silencio** y
// todas las celdas declararan que no llevan nada.
const _: () = assert!(COMBINING_PER_CELL <= MAX_COUNT as usize);

/// Los caracteres de anchura cero que van pegados a una celda: **dónde están, no cuáles son**.
///
/// Es opaco a propósito. Los caracteres viven en un vector plano de la [`View`], y esto es el tramo que
/// los señala; se leen con [`View::combining_of`].
///
/// **Por qué no van dentro de la celda.** [`ViewCell`] es `Copy` y la rejilla se reconstruye **entera cada
/// fotograma**, así que un `Vec` por celda sería una reserva de memoria por celda y por fotograma en el
/// camino más caliente del proyecto — y le quitaría el `Copy` a un tipo que lo tiene por eso mismo.
/// Preguntar por una celda que no lleva nada cuesta comparar un entero con cero.
///
/// **Y lo que ese trato cuesta, dicho aquí para que nadie lo descubra a la mala: comparar dos
/// [`ViewCell`] solo significa algo dentro de la MISMA vista.** Esto es dónde están guardados los
/// caracteres, no cuáles son, así que dos celdas que muestran exactamente lo mismo en dos vistas
/// distintas pueden salir **distintas**, y dentro de una misma vista dos celdas sin nada pegado salen
/// iguales solo porque su tramo vacío arranca en el mismo sitio. Hoy no las compara nadie; el día que
/// alguien lo necesite, lo que quiere comparar es el carácter y lo que [`View::combining_of`] devuelva.
/// **Va en UN entero y no en dos campos, y son cuatro bytes por celda.** Medido: con `offset: u32` y
/// `count: u8` como campos de un tipo anidado, la celda pasa de 24 a **32** bytes, porque el tipo trae
/// su propio relleno y el `u8` no puede colarse en el hueco que la celda ya tenía. Empaquetado son **28**.
/// Cuatro bytes por celda, reservados y puestos a cero **cada fotograma**, en el camino que este mismo
/// módulo vigila con una prueba.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CombiningSpan(u32);

/// Cuántos caben en los cinco bits bajos del tramo.
const MAX_COUNT: u32 = 31;
/// Y cuántos caracteres puede haber por delante en el vector, en los veintisiete de arriba.
const MAX_OFFSET: u32 = (1 << 27) - 1;

impl CombiningSpan {
    /// El tramo que empieza en `offset` y dura `count`, **o nada si alguno no cabe**.
    ///
    /// **Nada, y no un tramo recortado**: un `offset` que no quepa y se sature apuntaría a caracteres
    /// **reales de otra celda**, que es peor que no tener ninguno. La primera versión saturaba, y una
    /// revisión lo cazó. Es inalcanzable —el vector está acotado por celdas × dieciséis, y aquí caben
    /// ciento treinta y cuatro millones— así que esto no defiende de nada: **elige el modo de fallo**.
    fn new(offset: usize, count: usize) -> Self {
        let (Ok(offset), Ok(count)) = (u32::try_from(offset), u32::try_from(count)) else {
            return Self::default();
        };
        if offset > MAX_OFFSET || count > MAX_COUNT {
            return Self::default();
        }
        Self(offset << 5 | count)
    }

    fn offset(self) -> usize {
        (self.0 >> 5) as usize
    }

    fn count(self) -> usize {
        (self.0 & MAX_COUNT) as usize
    }
}

/// Una celda de la rejilla, ya resuelta para quien la pinte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewCell {
    pub c: char,
    /// Lo que va **pegado** a este carácter y no ocupa columna: selectores de presentación, marcas
    /// combinantes, los juntadores de una secuencia de emoji.
    ///
    /// **Sin esto, `☁` y `☁️` son la misma celda.** Son el mismo carácter con presentación distinta —la
    /// segunda lleva detrás el selector de emoji— y quien pinta no tenía forma de distinguirlos: el
    /// carácter que los separa no ocupa columna, así que se quedaba en el motor.
    ///
    /// Los caracteres se leen con [`View::combining_of`]; esto solo dice dónde están.
    pub combining: CombiningSpan,
    /// Color del carácter. **El vídeo inverso ya está aplicado**: si el programa lo pidió, este es
    /// el fondo declarado. Resolverlo aquí evita que cada capa de pintado repita la misma regla, y
    /// que una de ellas se olvide.
    pub fg: TermColor,
    /// Color del fondo, con el vídeo inverso ya aplicado igual que en `fg`.
    pub bg: TermColor,
    pub width: CellWidth,
    /// Los dos colores **ya vienen intercambiados**; esto dice que lo están.
    ///
    /// No es información redundante ni una invitación a rehacer el intercambio. Hace falta porque
    /// la intensidad —negrita y atenuado— describe el color que el programa declaró como **primer
    /// plano**, y con vídeo inverso ese color es el que ahora viaja en `bg`. Sin este dato, quien
    /// pinta aplica la intensidad al campo equivocado: con `\x1b[2;31;7m` atenúa un color que ya no
    /// se ve y deja el rojo a plena intensidad. Lo comprobó una revisión del pintor, no la teoría.
    pub inverse: bool,
    /// Negrita. **No se resuelve aquí**: «negrita = brillante» es decisión de tema, no de lectura.
    ///
    /// **Ojo con el vídeo inverso.** La intensidad describe el color que el programa declaró como
    /// primer plano; si pidió vídeo inverso, ese color es ahora el campo `bg`. O sea que con
    /// `\x1b[2;31;7m` lo que hay que atenuar es el rojo, que está en `bg`, no el `fg`. Resolverlo
    /// del todo aquí exigiría aplicar el tema, que es de la capa de arriba.
    pub bold: bool,
    /// Atenuado. Mismo aviso sobre el vídeo inverso que en `bold`.
    pub dim: bool,
    pub italic: bool,
    pub underline: Underline,
    pub strikeout: bool,
    /// El programa pidió no mostrar el contenido. El carácter sigue estando: quien pinta decide.
    pub hidden: bool,
    /// Esta celda está dentro de lo que hay seleccionado ahora mismo.
    ///
    /// **Un booleano por celda y no el rango de la selección**, que es el criterio fundacional de esta
    /// caja: el rango es un tipo del motor y sacarlo por aquí lo filtraría. Y quien pinta no necesita el
    /// rango —ya recorre la rejilla celda a celda—, necesita saber de cada una si va realzada.
    ///
    /// **Lo decide el motor y no una comparación de coordenadas aquí**, y la diferencia no es cosmética:
    /// el acompañante de un carácter ancho cuenta como seleccionado cuando lo está su cabeza, y el
    /// cursor de bloque **no** se realza en los extremos de la selección. Rehacer esas dos reglas aquí
    /// sería reescribir lo que la dependencia ya sostiene, y con ellas mal la selección se ve rota
    /// exactamente donde hay emoji.
    pub selected: bool,
}

impl Default for ViewCell {
    fn default() -> Self {
        Self {
            c: ' ',
            combining: CombiningSpan::default(),
            fg: TermColor::Foreground,
            bg: TermColor::Background,
            width: CellWidth::Single,
            inverse: false,
            bold: false,
            dim: false,
            italic: false,
            underline: Underline::None,
            strikeout: false,
            hidden: false,
            selected: false,
        }
    }
}

/// Forma del cursor, tal y como la pidió el programa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Underline,
    Beam,
    HollowBlock,
    /// El programa lo apagó. Quien pinta no debe dibujarlo.
    Hidden,
}

/// Dónde está el cursor y qué forma tiene.
///
/// La fila y la columna son **coordenadas visibles**: la fila 0 es la de arriba de lo que se ve,
/// no la del historial. Quien pinta puede indexar la vista directamente con ellas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    pub shape: CursorShape,
}

/// Una foto de la rejilla visible.
///
/// **Densa**: hay exactamente `size.rows * size.cols` celdas, y la de la fila `f` columna `c` está
/// en la posición `f * size.cols + c`. Sin huecos y sin saltos, incluidos los acompañantes de los
/// caracteres anchos.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// Privado con el resto: la densidad es un invariante entre el tamaño y las celdas, y con el
    /// tamaño público cualquiera podía escribir `view.size.cols = 5` sobre una vista de 80. A
    /// partir de ahí `cell(1, 0)` devolvería una celda real pero equivocada en vez de `None`, y
    /// nada fallaría — la comprobación del constructor solo corre en depuración.
    size: Size,
    cells: Vec<ViewCell>,
    /// Todo lo que va pegado a las celdas, **plano y en un solo sitio**.
    ///
    /// Cada celda que lleve algo señala aquí su tramo; las que no llevan nada —casi todas— no ocupan.
    /// Ver [`CombiningSpan`] para por qué no van dentro de la celda.
    combining: Vec<char>,
    /// Dónde está el cursor, **si es que se ve**.
    ///
    /// **Es opcional desde que hay historial, y no es prudencia**: subiendo por el historial el
    /// cursor sigue estando donde estaba —en el área activa— y eso queda **fuera de la pantalla**.
    /// Recortarlo a la última fila lo dibujaría en un sitio donde no está, que es peor que no
    /// dibujarlo: quien mira creería que el programa escribe ahí.
    cursor: Option<Cursor>,
    /// Cuántas líneas se ha subido por el historial: **cero es «se está mirando lo último»**.
    ///
    /// **Hoy no lo pinta nadie, y decirlo importa**: es lo que separa una pantalla quieta de una que se
    /// ha quedado atrás mientras el programa sigue escribiendo, y enseñarlo —una marca, una barra— es de
    /// su propio hito. Lo que sostiene mientras tanto es que se pueda **comprobar el efecto** de subir
    /// desde fuera de este módulo, que es la única forma de que una prueba del historial afirme sobre lo
    /// que se ve y no sobre el campo donde se apunta.
    scroll: usize,
}

impl View {
    pub(crate) fn new(
        size: Size,
        cells: Vec<ViewCell>,
        combining: Vec<char>,
        cursor: Option<Cursor>,
        scroll: usize,
    ) -> Self {
        debug_assert_eq!(cells.len(), size.rows as usize * size.cols as usize);
        Self {
            size,
            cells,
            combining,
            cursor,
            scroll,
        }
    }

    /// Los caracteres de anchura cero pegados a una celda, en el orden en que llegaron.
    ///
    /// Vacío es el caso corriente. **La celda se pasa entera y no su tramo**: `view.combining_of(cell.combining)`
    /// decía dos veces la misma palabra, y esa repetición era la señal de que la costura estaba puesta
    /// donde no se usa.
    pub fn combining_of(&self, cell: &ViewCell) -> &[char] {
        let span = cell.combining;
        let (offset, end) = (span.offset(), span.offset() + span.count());
        // **Un tramo que se saliera es una vista mal construida, no un dato del programa**, así que salta
        // en las pruebas en vez de disfrazarse de «esta celda no llevaba nada». En producción degrada a
        // vacío, que es lo único seguro que se puede hacer con un índice en el que ya no se confía.
        //
        // La primera versión decía esto mismo en un comentario y hacía lo contrario: devolvía vacío **sin
        // saltar**, o sea exactamente el disfraz que decía evitar. Lo cazó una revisión.
        debug_assert!(
            end <= self.combining.len(),
            "un tramo de la vista apunta fuera de su propio vector: {offset}..{end} de {}",
            self.combining.len()
        );
        self.combining.get(offset..end).unwrap_or_default()
    }

    /// El tamaño de la rejilla que retrata esta vista.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Dónde está el cursor y qué forma tiene, **o nada si no se ve**.
    ///
    /// No se ve cuando se ha subido por el historial lo bastante como para dejarlo fuera de la
    /// pantalla. Ver el campo.
    pub fn cursor(&self) -> Option<Cursor> {
        self.cursor
    }

    /// Cuántas líneas se ha subido por el historial. **Cero es «se está mirando lo último»**.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Una celda por fila y columna, o `None` si cae fuera.
    pub fn cell(&self, row: u16, col: u16) -> Option<&ViewCell> {
        if row >= self.size.rows || col >= self.size.cols {
            return None;
        }
        self.cells
            .get(row as usize * self.size.cols as usize + col as usize)
    }

    /// Una fila entera, o `None` si cae fuera. Siempre tiene `size.cols` celdas.
    pub fn row(&self, row: u16) -> Option<&[ViewCell]> {
        if row >= self.size.rows {
            return None;
        }
        let width = self.size.cols as usize;
        let offset = row as usize * width;
        self.cells.get(offset..offset + width)
    }

    /// Todas las celdas, en orden de lectura.
    pub fn cells(&self) -> &[ViewCell] {
        &self.cells
    }
}

// ── Traducción desde el vocabulario de la librería ───────────────────────────────────────────
//
// Vive aquí y no en `screen.rs` para que quede en un solo sitio qué se convierte en qué: si la
// dependencia cambia de forma, este es el fichero que se toca.
//
// **Funciones y no `impl From`, y esto es el criterio del hito, no estilo.** Un `impl` de trait no
// lleva visibilidad: `impl From<Color> for TermColor` es API **pública** aunque `Color` sea de la
// dependencia, y sale publicado en la documentación. Con él puesto, subir `alacritty_terminal`
// rompería la API de esta caja sin que nadie tocara una línea nuestra — que es exactamente lo que
// el criterio existe para impedir. Se comprobó generando la doc y buscando `From<Color>`.

// `Cell` con alias, como `CursorShape` justo debajo: esta caja **exporta su propio `Cell`**
// desde el módulo hermano, y con el ajeno importado a pelo el mismo nombre significaría dos
// cosas distintas según el fichero — un enlace de rustdoc `[`Cell`]` escrito aquí apuntaría
// al de la dependencia y resolvería sin avisar. El alias va al ajeno, que es la regla.
use alacritty_terminal::term::cell::{Cell as AlacrittyCell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape as AlacrittyShape, NamedColor};

pub(crate) fn color_from(color: Color) -> TermColor {
    match color {
        Color::Spec(rgb) => TermColor::Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        },
        Color::Indexed(i) => TermColor::Indexed(i),
        Color::Named(n) => named_color(n),
    }
}

fn named_color(named: NamedColor) -> TermColor {
    {
        use NamedColor::*;
        match named {
            // Los dieciséis con nombre son las dieciséis primeras entradas de la paleta.
            Black => TermColor::Indexed(0),
            Red => TermColor::Indexed(1),
            Green => TermColor::Indexed(2),
            Yellow => TermColor::Indexed(3),
            Blue => TermColor::Indexed(4),
            Magenta => TermColor::Indexed(5),
            Cyan => TermColor::Indexed(6),
            White => TermColor::Indexed(7),
            BrightBlack => TermColor::Indexed(8),
            BrightRed => TermColor::Indexed(9),
            BrightGreen => TermColor::Indexed(10),
            BrightYellow => TermColor::Indexed(11),
            BrightBlue => TermColor::Indexed(12),
            BrightMagenta => TermColor::Indexed(13),
            BrightCyan => TermColor::Indexed(14),
            BrightWhite => TermColor::Indexed(15),

            DimBlack => TermColor::Dim(0),
            DimRed => TermColor::Dim(1),
            DimGreen => TermColor::Dim(2),
            DimYellow => TermColor::Dim(3),
            DimBlue => TermColor::Dim(4),
            DimMagenta => TermColor::Dim(5),
            DimCyan => TermColor::Dim(6),
            DimWhite => TermColor::Dim(7),

            Foreground => TermColor::Foreground,
            Background => TermColor::Background,
            Cursor => TermColor::Cursor,
            BrightForeground => TermColor::BrightForeground,
            DimForeground => TermColor::DimForeground,
        }
    }
}

pub(crate) fn shape_from(shape: AlacrittyShape) -> CursorShape {
    match shape {
        AlacrittyShape::Block => CursorShape::Block,
        AlacrittyShape::Underline => CursorShape::Underline,
        AlacrittyShape::Beam => CursorShape::Beam,
        AlacrittyShape::HollowBlock => CursorShape::HollowBlock,
        AlacrittyShape::Hidden => CursorShape::Hidden,
    }
}

/// El estilo de subrayado, que son cinco banderas mutuamente excluyentes.
///
/// La librería las retira todas antes de poner la que toca, así que como mucho hay una puesta.
fn underline_from(flags: Flags) -> Underline {
    if flags.contains(Flags::DOUBLE_UNDERLINE) {
        Underline::Double
    } else if flags.contains(Flags::UNDERCURL) {
        Underline::Curl
    } else if flags.contains(Flags::DOTTED_UNDERLINE) {
        Underline::Dotted
    } else if flags.contains(Flags::DASHED_UNDERLINE) {
        Underline::Dashed
    } else if flags.contains(Flags::UNDERLINE) {
        Underline::Single
    } else {
        Underline::None
    }
}

/// Traduce una celda del motor a la nuestra, diciendo aparte si va dentro de la selección.
///
/// **La selección llega como parámetro y no se lee de la celda**, porque no es suya: el motor la guarda
/// como un rango de la rejilla, no como una bandera por celda. Quien recorre la rejilla ya sabe de cada
/// celda si el rango la incluye, y preguntárselo aquí obligaría a pasarle el rango entero —o sea a
/// filtrar un tipo del motor hasta este módulo.
///
/// **`combining` es el vector de toda la vista, y esta función escribe en él.** Lo que va pegado a la celda
/// no cabe dentro de ella —ver [`CombiningSpan`]—, así que se copia al final del vector común y la celda se
/// queda con el tramo.
pub(crate) fn cell_from(
    cell: &AlacrittyCell,
    selected: bool,
    combining: &mut Vec<char>,
) -> ViewCell {
    {
        let flags = cell.flags;

        let attached = cell.zerowidth().unwrap_or_default();
        let count = attached.len().min(COMBINING_PER_CELL);
        // **El tramo se toma antes de copiar**, que es cuando `offset` vale el final actual del vector.
        let span = CombiningSpan::new(combining.len(), count);
        // **Los PRIMEROS, no los últimos.** Lo que decide la presentación es el carácter que va justo
        // detrás del de la celda, así que recortar por el otro extremo se llevaría por delante justo el
        // que importa.
        combining.extend_from_slice(&attached[..count]);

        // Son tres banderas, no dos: además del carácter ancho y su acompañante, el terminal marca
        // el hueco que deja al final de una fila cuando el carácter no cabe y lo baja entera a la
        // siguiente. Tratarlo como celda normal sería perder la única señal de que ese espacio no
        // lo escribió el programa.
        let width = if flags.contains(Flags::WIDE_CHAR) {
            CellWidth::WideHead
        } else if flags.contains(Flags::WIDE_CHAR_SPACER) {
            CellWidth::WideTail
        } else if flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
            CellWidth::WrapFiller
        } else {
            CellWidth::Single
        };

        // El vídeo inverso se resuelve aquí, intercambiando los dos colores. Es una regla del
        // terminal, no del tema: dejarla para quien pinta la haría repetirse en cada capa de
        // pintado, y basta con que una se la salte para que el cursor de `vim` se vuelva invisible.
        let (fg, bg) = if flags.contains(Flags::INVERSE) {
            (color_from(cell.bg), color_from(cell.fg))
        } else {
            (color_from(cell.fg), color_from(cell.bg))
        };

        ViewCell {
            c: cell.c,
            combining: span,
            fg,
            bg,
            width,
            inverse: flags.contains(Flags::INVERSE),
            bold: flags.contains(Flags::BOLD),
            dim: flags.contains(Flags::DIM),
            italic: flags.contains(Flags::ITALIC),
            underline: underline_from(flags),
            strikeout: flags.contains(Flags::STRIKEOUT),
            hidden: flags.contains(Flags::HIDDEN),
            selected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una vista de una sola celda con ese vector de caracteres pegados detrás.
    fn view_with(combining: Vec<char>) -> View {
        View::new(
            Size { rows: 1, cols: 1 },
            vec![ViewCell::default()],
            combining,
            None,
            0,
        )
    }

    /// Una celda que señala ese tramo.
    fn cell_pointing_to(span: CombiningSpan) -> ViewCell {
        ViewCell {
            combining: span,
            ..ViewCell::default()
        }
    }

    #[test]
    fn un_tramo_que_no_cabe_en_el_entero_deja_la_celda_sin_nada_en_vez_de_recortarse() {
        // **Recortar al máximo que cabe sería enseñar lo de la celda de al lado como si fuera de
        // ésta.** Los caracteres de todas las celdas viven en un vector común, así que un `count`
        // saturado a los treinta y uno que caben en el entero se lleva por delante los de quien venga
        // detrás — y el que mira no tiene forma de saber que lo que ve no lo escribió ahí nadie.
        // Rendirse deja la celda sin nada pegado, que es exactamente como se veía antes de esta
        // entrega.
        //
        // Los cuarenta caracteres del vector son los que un `count` saturado devolvería: si se
        // recortara, esta celda se quedaría con treinta y uno de ellos.
        let view = view_with(vec!['\u{0301}'; 40]);
        let cell = cell_pointing_to(CombiningSpan::new(0, 40));

        assert!(
            view.combining_of(&cell).is_empty(),
            "un tramo que no cabe se recortó en vez de rendirse, y lo recortado son caracteres de otra celda"
        );
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "apunta fuera de su propio vector")]
    fn un_tramo_que_se_sale_del_vector_salta_en_vez_de_pasar_por_una_celda_sin_nada() {
        // **Devolver vacío sin más convierte una vista mal construida en «esta celda no llevaba
        // nada».** Son dos cosas distintas y se ven igual, así que el fallo viajaría hasta el pintado
        // y se manifestaría como un emoji que perdió su selector — a mil líneas de donde se rompió.
        // En depuración salta aquí; en producción degrada a vacío, que es lo único seguro que se
        // puede hacer con un índice del que ya no te fías.
        let view = view_with(vec!['\u{FE0F}']);
        let cell = cell_pointing_to(CombiningSpan::new(0, 2));

        let _ = view.combining_of(&cell);
    }
}
