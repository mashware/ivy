//! Que la API pública de `term-core` está hecha de tipos propios.
//!
//! Es el criterio del hito y **se comprueba compilando**, no ejecutando: este fichero recorre toda
//! la superficie pública del lector nombrando únicamente tipos de `term_core`, y hace `match`
//! exhaustivo sobre sus enumeraciones. Si `Screen::view` devolviera los tipos de
//! `alacritty_terminal` en vez de los nuestros, ninguno de estos `match` compilaría: sus variantes
//! no existen allí, y las que existen tienen otros nombres.
//!
//! Vive en `tests/` y no dentro del módulo a propósito: aquí se consume la caja **desde fuera**,
//! igual que la va a consumir quien pinte.
//!
//! **Lo que esto NO demuestra, y conviene saberlo antes de fiarse.** No detecta un tipo ajeno que
//! entre por una vía que no sea el tipo de retorno. La que se coló en la primera versión de este
//! PR fue un `impl From<Color> for TermColor`: los `impl` de trait **no llevan visibilidad**, así
//! que son API pública aunque el tipo de la izquierda sea de la dependencia, y este test pasaba
//! exactamente igual con la fuga puesta. Mirar que no hay `pub use` de la dependencia tampoco la
//! habría visto.
//!
//! La comprobación que sí la ve es generar la documentación y buscar el nombre ajeno:
//!
//! ```text
//! cargo doc -p term-core --no-deps
//! grep -rl "From&#60;Color&#62;\|From&#60;&#38;Cell&#62;" target/doc/term_core/   # tiene que no encontrar nada
//! ```
//!
//! **Sigue cazando las dos fugas, y está medido.** Se sospechó que el renombrado se lo había llevado
//! —el tipo propio pasó a llamarse igual que el del motor— así que se inyectaron las dos fugas, una
//! por cada tipo, y el patrón las encuentra las dos: `rustdoc` escribe el nombre **real** del tipo en
//! la cabecera del `impl`, no el alias con que este crate lo importa. Casi se cambió por
//! `grep alacritty_terminal`, que suena mejor y **no encuentra ninguna de las dos**.
//!
//! Lo que sí perdió es precisión en el otro sentido: un `impl From&#60;&#38;Cell&#62;` legítimo **sobre
//! nuestro propio `Cell`** ahora daría un acierto. Sería un falso positivo, no un silencio — y este
//! fichero prohíbe justo esos `impl` tres párrafos más abajo, así que el caso no se da.
//!
//! Por eso la traducción vive en funciones `pub(crate)` y no en `impl From`: no es estilo, es lo
//! que hace la fuga **imposible** en vez de dejarla en manos de acordarse de mirar.

use term_core::{
    Cell, CellWidth, CursorShape, Half, MouseReporting, Screen, Scroll, Size, TermColor, Underline,
    ViewCell,
};

/// Recorre todas las variantes de color sin nombrar el enum de la librería de debajo.
fn describe(color: TermColor) -> &'static str {
    match color {
        TermColor::Indexed(_) => "entrada de la paleta",
        TermColor::Rgb { .. } => "color exacto",
        TermColor::Foreground => "primer plano del tema",
        TermColor::Background => "fondo del tema",
        TermColor::Cursor => "color del cursor",
        TermColor::Dim(_) => "entrada atenuada",
        TermColor::BrightForeground => "primer plano brillante",
        TermColor::DimForeground => "primer plano atenuado",
    }
}

fn describe_width(width: CellWidth) -> &'static str {
    match width {
        CellWidth::Single => "una columna",
        CellWidth::WideHead => "primera de dos",
        CellWidth::WideTail => "acompañante",
        CellWidth::WrapFiller => "hueco de arrastre",
    }
}

fn describe_underline(style: Underline) -> &'static str {
    match style {
        Underline::None => "sin subrayado",
        Underline::Single => "simple",
        Underline::Double => "doble",
        Underline::Curl => "ondulado",
        Underline::Dotted => "punteado",
        Underline::Dashed => "discontinuo",
    }
}

fn describe_cursor(shape: CursorShape) -> &'static str {
    match shape {
        CursorShape::Block => "bloque",
        CursorShape::Underline => "subrayado",
        CursorShape::Beam => "barra",
        CursorShape::HollowBlock => "bloque hueco",
        CursorShape::Hidden => "apagado",
    }
}

#[test]
fn la_vista_se_consume_entera_con_tipos_propios() {
    let mut screen = Screen::new(Size { rows: 4, cols: 20 });
    screen.advance(b"\x1b[7;1;4mhola\x1b[0m");

    let view = screen.view();

    // Cada campo público de la celda, leído sin importar nada de la dependencia.
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
        } = *cell;

        assert!(!describe(fg).is_empty());
        assert!(!describe(bg).is_empty());
        assert!(!describe_width(width).is_empty());
        assert!(!describe_underline(underline).is_empty());
        // **Lo pegado se consume de verdad, no se descarta.** Es el campo que más fácil sería sacar
        // con un tipo de la dependencia —el motor lo guarda en un `Vec<char>` suyo— y lo que esta
        // prueba defiende es justo eso: sale como un tramo de `char` del estándar.
        let attached: &[char] = view.combining_of(cell);
        assert!(
            attached.is_empty(),
            "esta pantalla es ASCII: ninguna celda debería traer nada pegado"
        );
        let _ = combining;
        let _ = (c, inverse, bold, dim, italic, strikeout, hidden, selected);
    }

    assert!(
        !describe_cursor(
            view.cursor()
                .expect("con la pantalla al día el cursor se ve")
                .shape
        )
        .is_empty()
    );
    assert_eq!(view.size(), Size { rows: 4, cols: 20 });
    assert_eq!(view.cells().len(), 80);
    assert_eq!(
        view.cursor()
            .expect("con la pantalla al día el cursor se ve")
            .row,
        0
    );
}

/// Que **mirar el historial** también se pide y se lee con tipos propios.
///
/// Va aparte de la vista porque es superficie nueva y **la prueba de arriba no la custodiaba**: el motor
/// de debajo tiene su propio vocabulario para esto —un enum de desplazamiento con sus variantes— y
/// dejarlo asomar sería exactamente la fuga que este fichero existe para impedir. Se recorren **todas**
/// las variantes con un `match` exhaustivo, que es lo que hace que compilar sea la comprobación: si
/// `Scroll` fuera el tipo ajeno, sus variantes no se llamarían así.
#[test]
fn el_historial_se_pide_y_se_lee_con_tipos_propios() {
    fn describe(amount: Scroll) -> &'static str {
        match amount {
            Scroll::Lines(_) => "unas líneas",
            Scroll::Pages(_) => "unas pantallas",
            Scroll::ToTop => "hasta lo más viejo que se guarda",
            Scroll::ToBottom => "de vuelta a donde el programa escribe",
        }
    }

    let mut screen = Screen::new(Size { rows: 4, cols: 20 });
    for n in 0..40 {
        screen.advance(format!("linea {n}\r\n").as_bytes());
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

    // Y lo que se lee de vuelta es un número nuestro, no la posición interna del motor.
    let amount: usize = screen.view().scroll();
    assert_eq!(amount, 0, "se acabó pidiendo volver al final");
}

/// Que **la selección** también se pide y se lee con tipos propios.
///
/// Es superficie nueva y de la clase que este fichero vigila: el motor guarda la selección como un rango
/// suyo, con su punto y su mitad, y dejar asomar cualquiera de los tres sería la fuga. Lo que sale de aquí
/// son una celda nuestra, una mitad nuestra, un `bool` por celda y una cadena.
#[test]
fn la_seleccion_se_pide_y_se_lee_con_tipos_propios() {
    fn describe(half: Half) -> &'static str {
        match half {
            Half::Left => "por la mitad de la izquierda",
            Half::Right => "por la mitad de la derecha",
        }
    }

    let mut screen = Screen::new(Size { rows: 4, cols: 20 });
    screen.advance(b"hola mundo");

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
    assert_eq!(text.as_deref(), Some("hola"));

    // Y el realce llega a la foto como un booleano por celda, no como el rango.
    let view = screen.view();
    let highlighted = view.cells().iter().filter(|cell| cell.selected).count();
    assert_eq!(highlighted, 4, "el realce no llegó celda a celda");

    screen.clear_selection();
    assert!(!screen.has_selection());
}

/// Que preguntar por el ratón también se hace con tipos propios.
///
/// **Va aparte de las de arriba porque es superficie nueva y ninguna la custodiaba.** Lo que
/// `mouse_reporting` lee son las banderas del motor, así que devolverlas tal cual es el refactor
/// natural — y con él, toda la caja seguiría compilando y este fichero seguiría pasando. La prueba
/// nombra el tipo propio y **desestructura sus cuatro campos**, que es lo que hace imposible que el
/// tipo del motor pase por aquí: sus banderas no tienen estos nombres.
#[test]
fn el_raton_se_pregunta_y_se_lee_con_tipos_propios() {
    let mut screen = Screen::new(Size { rows: 4, cols: 20 });
    screen.advance(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");

    let MouseReporting {
        buttons,
        drag,
        motion,
        extended,
    } = screen.mouse_reporting();

    // **Los tres modos se SUSTITUYEN, no se acumulan** —leído en el motor, no supuesto—: pedir el
    // arrastre apaga el de solo clics. Por eso aquí queda encendido el arrastre y no los botones. La
    // codificación sí es aparte y convive con cualquiera de los tres.
    assert!(drag && extended);
    assert!(!buttons && !motion);
    assert!(screen.mouse_reporting().wants_mouse());
    // Y la otra pregunta de la superficie nueva, que devuelve un `bool` y no el modo del motor.
    let _: bool = screen.wheel_acts_as_arrows();
}
