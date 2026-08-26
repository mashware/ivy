//! Qué bytes manda al programa cada gesto del ratón.
//!
//! Es una función pura de un gesto a los bytes que un terminal enviaría, hermana de [`keyboard`]. No
//! escribe en ningún sitio: quien los mande es el bucle, y por dónde los mande es el puente.
//!
//! **Esta codificación no viene de la librería.** El motor de VT sabe decir si el programa quiere el
//! ratón —eso son sus banderas—, pero armar el informe lo hacía el binario de Alacritty, no su caja.
//! Así que la tabla es de este proyecto y por eso está escrita aquí con su porqué.
//!
//! **Dónde se corta este hito, y por qué.** Queda fuera la codificación de coordenadas en UTF-8. La
//! antigua mete cada número en un byte con 32 sumado, así que **satura en la columna 223**, y lo que
//! `xterm` hace sin ese modo es exactamente eso: dejar de informar. Escribir esa tabla sin poder verla
//! funcionando sería adivinar, y este proyecto ya declaró ese mismo corte para las teclas de función.
//! Lo que queda fuera **devuelve silencio, nunca un byte aproximado**: unas coordenadas equivocadas el
//! programa las entiende como un clic en otro sitio, y eso es peor que no recibir nada.
//!
//! [`keyboard`]: crate::keyboard

use eframe::egui::{Modifiers, PointerButton};
use term_core::MouseReporting;

/// La columna o fila más alta que la codificación antigua puede decir.
///
/// Cada número viaja en **un byte** con 32 sumado, así que el mayor que cabe es `255 - 32`. Pasado ese
/// punto no se informa, que es lo que hace `xterm` sin la codificación en UTF-8.
const LEGACY_ENCODING_CEILING: u16 = 223;

/// Qué ha hecho el ratón, en el vocabulario del informe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseReport {
    /// Se ha apretado un botón.
    Press(PointerButton),
    /// Se ha soltado un botón.
    Release(PointerButton),
    /// Se ha movido el puntero con un botón apretado.
    Drag(PointerButton),
    /// Se ha girado la rueda hacia lo viejo.
    WheelUp,
    /// Se ha girado la rueda hacia lo nuevo.
    WheelDown,
}

/// La celda sobre la que ocurrió el gesto, contada **desde uno** como manda el informe.
///
/// Se construye con [`from_zero`](CellPos::from_zero) para que la conversión ocurra en un solo sitio:
/// el resto del programa cuenta filas y columnas desde cero, y mezclar los dos orígenes deja un informe
/// desplazado una celda que nadie nota hasta que un menú abre la entrada de al lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPos {
    column: u16,
    row: u16,
}

impl CellPos {
    /// La celda que quien pinta llama `(fila, columna)`, contando desde cero.
    pub fn from_zero(row: u16, column: u16) -> Self {
        Self {
            column: column + 1,
            row: row + 1,
        }
    }
}

/// Los bytes que le corresponden a ese gesto, o nada si no hay nada que mandar.
///
/// Devuelve nada cuando el programa **no ha pedido** lo que se le iba a contar —un arrastre a quien
/// solo pidió los clics— y cuando las coordenadas no caben en la codificación que toca.
pub fn encode(
    report: MouseReport,
    at: CellPos,
    reporting: MouseReporting,
    chord: Modifiers,
) -> Option<Vec<u8>> {
    if !wants_this(report, reporting) {
        return None;
    }
    let button = button_number(report)? + chord_bits(chord);
    if reporting.extended {
        return Some(extended_form(report, button, at));
    }
    legacy_form(report, button, at)
}

/// Si el programa ha pedido que se le cuente **esta clase** de gesto.
///
/// **Pedir los clics no es pedir el arrastre**, y son banderas distintas justamente porque un programa
/// que solo quiere saber dónde se pincha no quiere un informe por cada celda que el puntero recorre con
/// el botón apretado.
fn wants_this(report: MouseReport, reporting: MouseReporting) -> bool {
    match report {
        // **La rueda va con los botones.** Un programa que pide los clics recibe los giros: es lo que
        // hace que la rueda mueva la lista de `htop` sin pedir nada más.
        MouseReport::Press(_)
        | MouseReport::Release(_)
        | MouseReport::WheelUp
        | MouseReport::WheelDown => reporting.wants_mouse(),
        MouseReport::Drag(_) => reporting.drag || reporting.motion,
    }
}

/// El número con el que viaja el botón, antes de sumarle el acorde.
///
/// **Soltar viaja como el botón que se suelta solo en la codificación extendida.** La antigua no tiene
/// forma de decir cuál se soltó y usa un número propio para «se soltó algo», que es el 3.
fn button_number(report: MouseReport) -> Option<u8> {
    let number_of = |button| match button {
        PointerButton::Primary => Some(0),
        PointerButton::Middle => Some(1),
        PointerButton::Secondary => Some(2),
        // Los botones laterales no tienen número en esta codificación. Silencio, no un número
        // aproximado: el programa entendería un clic que nadie hizo.
        _ => None,
    };
    match report {
        MouseReport::Press(button) | MouseReport::Release(button) => number_of(button),
        // **Arrastrar lleva 32 sumado**: es el bit de «esto es movimiento», que es lo que distingue
        // recorrer una celda con el botón apretado de volver a apretarlo ahí.
        MouseReport::Drag(button) => Some(number_of(button)? + 32),
        MouseReport::WheelUp => Some(64),
        MouseReport::WheelDown => Some(65),
    }
}

/// Lo que el acorde le suma al número del botón.
///
/// **`Mayúsculas` no aparece, y es a propósito**: en esta ventana esa tecla fuerza la selección, así que
/// un gesto con ella apretada no llega nunca hasta aquí. Ponerla igualmente sería escribir una rama
/// muerta que además sugiere lo contrario de lo que la ventana hace.
fn chord_bits(chord: Modifiers) -> u8 {
    let mut bits = 0;
    if chord.alt {
        bits += 8;
    }
    if chord.ctrl {
        bits += 16;
    }
    bits
}

/// El informe en la codificación extendida: números en texto y una letra que dice si se apretó o soltó.
fn extended_form(report: MouseReport, button: u8, at: CellPos) -> Vec<u8> {
    let final_byte = if matches!(report, MouseReport::Release(_)) {
        'm'
    } else {
        'M'
    };
    format!("\x1b[<{button};{};{}{final_byte}", at.column, at.row).into_bytes()
}

/// El informe en la codificación antigua: tres bytes con 32 sumado a cada número.
///
/// Devuelve nada pasada la columna o la fila 223: ver [`LEGACY_ENCODING_CEILING`].
fn legacy_form(report: MouseReport, button: u8, at: CellPos) -> Option<Vec<u8>> {
    if at.column > LEGACY_ENCODING_CEILING || at.row > LEGACY_ENCODING_CEILING {
        return None;
    }
    // **Soltar no dice qué botón se soltó**: esta codificación no tiene sitio para ello y usa el 3 para
    // «se soltó algo». Quien quiera saber cuál pide la extendida.
    let button = if matches!(report, MouseReport::Release(_)) {
        3 + (button & !0b11)
    } else {
        button
    };
    Some(vec![
        0x1b,
        b'[',
        b'M',
        32 + button,
        32 + at.column as u8,
        32 + at.row as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un programa que pidió los clics y el arrastre, en la codificación antigua.
    fn like_htop() -> MouseReporting {
        MouseReporting {
            buttons: true,
            drag: true,
            ..MouseReporting::default()
        }
    }

    /// Lo mismo, pero pidiendo la codificación extendida.
    fn with_the_extended() -> MouseReporting {
        MouseReporting {
            extended: true,
            ..like_htop()
        }
    }

    #[test]
    fn un_programa_que_no_pide_el_raton_no_recibe_nada() {
        // La consola de todos los días. Sin esto, cada clic dentro de un shell le metería basura en la
        // línea de órdenes.
        assert_eq!(
            encode(
                MouseReport::Press(PointerButton::Primary),
                CellPos::from_zero(0, 0),
                MouseReporting::default(),
                Modifiers::NONE,
            ),
            None
        );
    }

    #[test]
    fn la_celda_del_informe_empieza_en_uno() {
        // **La esquina de arriba a la izquierda es la 1;1 y no la 0;0.** Quien pinta cuenta desde cero,
        // el informe desde uno, y mezclar los dos orígenes deja todo desplazado una celda: un menú abre
        // la entrada de al lado y una barra se agarra por el sitio equivocado.
        let bytes = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::NONE,
        )
        .expect("el programa pidió los clics");

        assert_eq!(bytes, b"\x1b[<0;1;1M");
    }

    #[test]
    fn soltar_y_apretar_se_distinguen_en_la_codificacion_extendida() {
        // **La letra final es lo único que los separa**, y es lo que un editor usa para saber que
        // terminaste de arrastrar la selección. Mandar siempre la misma deja el gesto abierto para
        // siempre desde el punto de vista del programa.
        let press = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(4, 9),
            with_the_extended(),
            Modifiers::NONE,
        );
        let release = encode(
            MouseReport::Release(PointerButton::Primary),
            CellPos::from_zero(4, 9),
            with_the_extended(),
            Modifiers::NONE,
        );

        assert_eq!(press.as_deref(), Some(&b"\x1b[<0;10;5M"[..]));
        assert_eq!(release.as_deref(), Some(&b"\x1b[<0;10;5m"[..]));
    }

    #[test]
    fn la_codificacion_antigua_no_dice_que_boton_se_solto() {
        // No tiene sitio para decirlo: usa un número propio para «se soltó algo». Mandar el del botón
        // haría que el programa entendiera un clic nuevo del botón central justo al soltar el derecho.
        let bytes = encode(
            MouseReport::Release(PointerButton::Secondary),
            CellPos::from_zero(0, 0),
            like_htop(),
            Modifiers::NONE,
        )
        .expect("el programa pidió los clics");

        assert_eq!(bytes, vec![0x1b, b'[', b'M', 32 + 3, 33, 33]);
    }

    #[test]
    fn la_codificacion_antigua_calla_pasada_la_columna_223() {
        // **Silencio, no un byte aproximado.** Cada número viaja en un byte con 32 sumado, así que la
        // 224 no cabe: saturarla en 223 le diría al programa que se pinchó en un sitio donde no se
        // pinchó, y volver a empezar por el 0 sería peor todavía.
        let inside = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 222),
            like_htop(),
            Modifiers::NONE,
        );
        let outside = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 223),
            like_htop(),
            Modifiers::NONE,
        );

        assert!(inside.is_some(), "la columna 223 sí cabe y se calló");
        assert_eq!(outside, None, "la columna 224 no cabe y se mandó igual");
    }

    #[test]
    fn la_codificacion_extendida_no_tiene_ese_techo() {
        // Es la razón de ser de la extendida: sus números van en texto, así que una ventana ancha de
        // verdad sigue informando donde la antigua ya no puede.
        let bytes = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 399),
            with_the_extended(),
            Modifiers::NONE,
        )
        .expect("la extendida no satura");

        assert_eq!(bytes, b"\x1b[<0;400;1M");
    }

    #[test]
    fn pedir_los_clics_no_es_pedir_el_arrastre() {
        // **Son banderas distintas porque significan cosas distintas.** Un programa que solo quiere
        // saber dónde se pincha recibiría, si no, un informe por cada celda que el puntero recorre con
        // el botón apretado — decenas por gesto, y ninguno pedido.
        let clicks_only = MouseReporting {
            buttons: true,
            ..MouseReporting::default()
        };

        assert_eq!(
            encode(
                MouseReport::Drag(PointerButton::Primary),
                CellPos::from_zero(1, 1),
                clicks_only,
                Modifiers::NONE,
            ),
            None
        );
        assert!(
            encode(
                MouseReport::Press(PointerButton::Primary),
                CellPos::from_zero(1, 1),
                clicks_only,
                Modifiers::NONE,
            )
            .is_some(),
            "pidió los clics y no recibió el suyo"
        );
    }

    #[test]
    fn arrastrar_lleva_el_bit_de_movimiento() {
        // Los 32 sumados son lo que distingue «voy recorriendo celdas con el botón apretado» de «he
        // vuelto a apretar aquí». Sin ellos, arrastrar una barra de `htop` se lee como una ráfaga de
        // clics.
        let bytes = encode(
            MouseReport::Drag(PointerButton::Primary),
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::NONE,
        )
        .expect("pidió el arrastre");

        assert_eq!(bytes, b"\x1b[<32;1;1M");
    }

    #[test]
    fn la_rueda_viaja_con_sus_dos_numeros() {
        // 64 y 65, y son lo que hace que la lista de `htop` suba y baje. Intercambiarlos deja la rueda
        // andando al revés, que es un defecto que se nota a diario y no rompe ninguna otra cosa.
        let wheel_up = encode(
            MouseReport::WheelUp,
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::NONE,
        );
        let wheel_down = encode(
            MouseReport::WheelDown,
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::NONE,
        );

        assert_eq!(wheel_up.as_deref(), Some(&b"\x1b[<64;1;1M"[..]));
        assert_eq!(wheel_down.as_deref(), Some(&b"\x1b[<65;1;1M"[..]));
    }

    #[test]
    fn el_acorde_viaja_sumado_al_boton() {
        // `Ctrl` suma 16 y `Alt` 8, que es como el programa distingue un clic normal de uno con acorde
        // —abrir en otra pestaña, por ejemplo—. **`Mayúsculas` no aparece a propósito**: en esta
        // ventana esa tecla fuerza la selección, así que un gesto con ella no llega hasta aquí.
        let with_ctrl = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::CTRL,
        );
        let with_alt = encode(
            MouseReport::Press(PointerButton::Primary),
            CellPos::from_zero(0, 0),
            with_the_extended(),
            Modifiers::ALT,
        );

        assert_eq!(with_ctrl.as_deref(), Some(&b"\x1b[<16;1;1M"[..]));
        assert_eq!(with_alt.as_deref(), Some(&b"\x1b[<8;1;1M"[..]));
    }

    #[test]
    fn los_botones_laterales_no_mandan_nada() {
        // No tienen número en esta codificación. Darles uno prestado sería contarle al programa un clic
        // de un botón que nadie tocó.
        assert_eq!(
            encode(
                MouseReport::Press(PointerButton::Extra1),
                CellPos::from_zero(0, 0),
                with_the_extended(),
                Modifiers::NONE,
            ),
            None
        );
    }
}
