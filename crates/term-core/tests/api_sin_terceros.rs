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
    CellWidth, CursorShape, Desplazamiento, Screen, Size, TermColor, Underline, ViewCell,
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
        } = *celda;

        assert!(!describir(fg).is_empty());
        assert!(!describir(bg).is_empty());
        assert!(!describir_ancho(width).is_empty());
        assert!(!describir_subrayado(underline).is_empty());
        let _ = (c, inverse, bold, dim, italic, strikeout, hidden);
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
