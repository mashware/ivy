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
//! Por eso la traducción vive en funciones `pub(crate)` y no en `impl From`: no es estilo, es lo
//! que hace la fuga **imposible** en vez de dejarla en manos de acordarse de mirar.

use term_core::{
    Celda, CellWidth, CursorShape, Desplazamiento, Mitad, RatonReclamado, Screen, Size, TermColor,
    Underline, ViewCell,
};

/// Recorre todas las variantes de color sin nombrar el enum de la librería de debajo.
fn describir(color: TermColor) -> &'static str {
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

fn describir_ancho(ancho: CellWidth) -> &'static str {
    match ancho {
        CellWidth::Single => "una columna",
        CellWidth::WideHead => "primera de dos",
        CellWidth::WideTail => "acompañante",
        CellWidth::WrapFiller => "hueco de arrastre",
    }
}

fn describir_subrayado(estilo: Underline) -> &'static str {
    match estilo {
        Underline::None => "sin subrayado",
        Underline::Single => "simple",
        Underline::Double => "doble",
        Underline::Curl => "ondulado",
        Underline::Dotted => "punteado",
        Underline::Dashed => "discontinuo",
    }
}

fn describir_cursor(forma: CursorShape) -> &'static str {
    match forma {
        CursorShape::Block => "bloque",
        CursorShape::Underline => "subrayado",
        CursorShape::Beam => "barra",
        CursorShape::HollowBlock => "bloque hueco",
        CursorShape::Hidden => "apagado",
    }
}

#[test]
fn la_vista_se_consume_entera_con_tipos_propios() {
    let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
    pantalla.advance(b"\x1b[7;1;4mhola\x1b[0m");

    let vista = pantalla.view();

    // Cada campo público de la celda, leído sin importar nada de la dependencia.
    for celda in vista.cells() {
        let ViewCell {
            c,
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
        } = *celda;

        assert!(!describir(fg).is_empty());
        assert!(!describir(bg).is_empty());
        assert!(!describir_ancho(width).is_empty());
        assert!(!describir_subrayado(underline).is_empty());
        let _ = (c, inverse, bold, dim, italic, strikeout, hidden, selected);
    }

    assert!(
        !describir_cursor(
            vista
                .cursor()
                .expect("con la pantalla al día el cursor se ve")
                .shape
        )
        .is_empty()
    );
    assert_eq!(vista.size(), Size { rows: 4, cols: 20 });
    assert_eq!(vista.cells().len(), 80);
    assert_eq!(
        vista
            .cursor()
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
/// `Desplazamiento` fuera el tipo ajeno, sus variantes no se llamarían así.
#[test]
fn el_historial_se_pide_y_se_lee_con_tipos_propios() {
    fn describir(cuanto: Desplazamiento) -> &'static str {
        match cuanto {
            Desplazamiento::Lineas(_) => "unas líneas",
            Desplazamiento::Paginas(_) => "unas pantallas",
            Desplazamiento::AlPrincipio => "hasta lo más viejo que se guarda",
            Desplazamiento::AlFinal => "de vuelta a donde el programa escribe",
        }
    }

    let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
    for n in 0..40 {
        pantalla.advance(format!("linea {n}\r\n").as_bytes());
    }

    for cuanto in [
        Desplazamiento::AlPrincipio,
        Desplazamiento::Lineas(-1),
        Desplazamiento::Paginas(-1),
        Desplazamiento::AlFinal,
    ] {
        assert!(!describir(cuanto).is_empty());
        pantalla.desplazar(cuanto);
    }

    // Y lo que se lee de vuelta es un número nuestro, no la posición interna del motor.
    let cuanto: usize = pantalla.view().desplazamiento();
    assert_eq!(cuanto, 0, "se acabó pidiendo volver al final");
}

/// Que **la selección** también se pide y se lee con tipos propios.
///
/// Es superficie nueva y de la clase que este fichero vigila: el motor guarda la selección como un rango
/// suyo, con su punto y su mitad, y dejar asomar cualquiera de los tres sería la fuga. Lo que sale de aquí
/// son una celda nuestra, una mitad nuestra, un `bool` por celda y una cadena.
#[test]
fn la_seleccion_se_pide_y_se_lee_con_tipos_propios() {
    fn describir(mitad: Mitad) -> &'static str {
        match mitad {
            Mitad::Izquierda => "por la mitad de la izquierda",
            Mitad::Derecha => "por la mitad de la derecha",
        }
    }

    let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
    pantalla.advance(b"hola mundo");

    let desde = Celda {
        fila: 0,
        columna: 0,
        mitad: Mitad::Izquierda,
    };
    let hasta = Celda {
        fila: 0,
        columna: 3,
        mitad: Mitad::Derecha,
    };
    assert!(!describir(desde.mitad).is_empty());
    assert!(!describir(hasta.mitad).is_empty());

    pantalla.empezar_seleccion(desde);
    pantalla.extender_seleccion(hasta);

    let hay: bool = pantalla.hay_seleccion();
    let texto: Option<String> = pantalla.texto_seleccionado();
    assert!(hay);
    assert_eq!(texto.as_deref(), Some("hola"));

    // Y el realce llega a la foto como un booleano por celda, no como el rango.
    let vista = pantalla.view();
    let realzadas = vista.cells().iter().filter(|celda| celda.selected).count();
    assert_eq!(realzadas, 4, "el realce no llegó celda a celda");

    pantalla.quitar_la_seleccion();
    assert!(!pantalla.hay_seleccion());
}

/// Que preguntar por el ratón también se hace con tipos propios.
///
/// **Va aparte de las de arriba porque es superficie nueva y ninguna la custodiaba.** Lo que
/// `raton_reclamado` lee son las banderas del motor, así que devolverlas tal cual es el refactor
/// natural — y con él, toda la caja seguiría compilando y este fichero seguiría pasando. La prueba
/// nombra el tipo propio y **desestructura sus cuatro campos**, que es lo que hace imposible que el
/// tipo del motor pase por aquí: sus banderas no tienen estos nombres.
#[test]
fn el_raton_se_pregunta_y_se_lee_con_tipos_propios() {
    let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
    pantalla.advance(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");

    let RatonReclamado {
        botones,
        arrastre,
        movimiento,
        extendido,
    } = pantalla.raton_reclamado();

    // **Los tres modos se SUSTITUYEN, no se acumulan** —leído en el motor, no supuesto—: pedir el
    // arrastre apaga el de solo clics. Por eso aquí queda encendido el arrastre y no los botones. La
    // codificación sí es aparte y convive con cualquiera de los tres.
    assert!(arrastre && extendido);
    assert!(!botones && !movimiento);
    assert!(pantalla.raton_reclamado().lo_quiere());
    // Y la otra pregunta de la superficie nueva, que devuelve un `bool` y no el modo del motor.
    let _: bool = pantalla.la_rueda_va_como_flechas();
}
