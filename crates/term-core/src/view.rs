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

/// Una celda de la rejilla, ya resuelta para quien la pinte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewCell {
    pub c: char,
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
}

impl Default for ViewCell {
    fn default() -> Self {
        Self {
            c: ' ',
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
    /// tamaño público cualquiera podía escribir `vista.size.cols = 5` sobre una vista de 80. A
    /// partir de ahí `cell(1, 0)` devolvería una celda real pero equivocada en vez de `None`, y
    /// nada fallaría — la comprobación del constructor solo corre en depuración.
    size: Size,
    cells: Vec<ViewCell>,
    cursor: Cursor,
}

impl View {
    pub(crate) fn new(size: Size, cells: Vec<ViewCell>, cursor: Cursor) -> Self {
        debug_assert_eq!(cells.len(), size.rows as usize * size.cols as usize);
        Self {
            size,
            cells,
            cursor,
        }
    }

    /// El tamaño de la rejilla que retrata esta vista.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Dónde está el cursor y qué forma tiene.
    pub fn cursor(&self) -> Cursor {
        self.cursor
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
        let ancho = self.size.cols as usize;
        let desde = row as usize * ancho;
        self.cells.get(desde..desde + ancho)
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

use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape as AlacrittyShape, NamedColor};

pub(crate) fn color_desde(color: Color) -> TermColor {
    match color {
        Color::Spec(rgb) => TermColor::Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        },
        Color::Indexed(i) => TermColor::Indexed(i),
        Color::Named(n) => color_con_nombre(n),
    }
}

fn color_con_nombre(named: NamedColor) -> TermColor {
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

pub(crate) fn forma_desde(shape: AlacrittyShape) -> CursorShape {
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
fn subrayado_desde(banderas: Flags) -> Underline {
    if banderas.contains(Flags::DOUBLE_UNDERLINE) {
        Underline::Double
    } else if banderas.contains(Flags::UNDERCURL) {
        Underline::Curl
    } else if banderas.contains(Flags::DOTTED_UNDERLINE) {
        Underline::Dotted
    } else if banderas.contains(Flags::DASHED_UNDERLINE) {
        Underline::Dashed
    } else if banderas.contains(Flags::UNDERLINE) {
        Underline::Single
    } else {
        Underline::None
    }
}

pub(crate) fn celda_desde(celda: &Cell) -> ViewCell {
    {
        let banderas = celda.flags;

        // Son tres banderas, no dos: además del carácter ancho y su acompañante, el terminal marca
        // el hueco que deja al final de una fila cuando el carácter no cabe y lo baja entera a la
        // siguiente. Tratarlo como celda normal sería perder la única señal de que ese espacio no
        // lo escribió el programa.
        let width = if banderas.contains(Flags::WIDE_CHAR) {
            CellWidth::WideHead
        } else if banderas.contains(Flags::WIDE_CHAR_SPACER) {
            CellWidth::WideTail
        } else if banderas.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
            CellWidth::WrapFiller
        } else {
            CellWidth::Single
        };

        // El vídeo inverso se resuelve aquí, intercambiando los dos colores. Es una regla del
        // terminal, no del tema: dejarla para quien pinta la haría repetirse en cada capa de
        // pintado, y basta con que una se la salte para que el cursor de `vim` se vuelva invisible.
        let (fg, bg) = if banderas.contains(Flags::INVERSE) {
            (color_desde(celda.bg), color_desde(celda.fg))
        } else {
            (color_desde(celda.fg), color_desde(celda.bg))
        };

        ViewCell {
            c: celda.c,
            fg,
            bg,
            width,
            inverse: banderas.contains(Flags::INVERSE),
            bold: banderas.contains(Flags::BOLD),
            dim: banderas.contains(Flags::DIM),
            italic: banderas.contains(Flags::ITALIC),
            underline: subrayado_desde(banderas),
            strikeout: banderas.contains(Flags::STRIKEOUT),
            hidden: banderas.contains(Flags::HIDDEN),
        }
    }
}
