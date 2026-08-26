//! Qué bytes manda al programa cada gesto del ratón.
//!
//! Es una función pura de un gesto a los bytes que un terminal enviaría, hermana de [`teclado`]. No
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
//! [`teclado`]: crate::teclado

use eframe::egui::{Modifiers, PointerButton};
use term_core::MouseReporting;

/// La columna o fila más alta que la codificación antigua puede decir.
///
/// Cada número viaja en **un byte** con 32 sumado, así que el mayor que cabe es `255 - 32`. Pasado ese
/// punto no se informa, que es lo que hace `xterm` sin la codificación en UTF-8.
const TECHO_DE_LA_CODIFICACION_ANTIGUA: u16 = 223;

/// Qué ha hecho el ratón, en el vocabulario del informe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesto {
    /// Se ha apretado un botón.
    Apretar(PointerButton),
    /// Se ha soltado un botón.
    Soltar(PointerButton),
    /// Se ha movido el puntero con un botón apretado.
    Arrastrar(PointerButton),
    /// Se ha girado la rueda hacia lo viejo.
    RuedaArriba,
    /// Se ha girado la rueda hacia lo nuevo.
    RuedaAbajo,
}

/// La celda sobre la que ocurrió el gesto, contada **desde uno** como manda el informe.
///
/// Se construye con [`desde_cero`](Donde::desde_cero) para que la conversión ocurra en un solo sitio:
/// el resto del programa cuenta filas y columnas desde cero, y mezclar los dos orígenes deja un informe
/// desplazado una celda que nadie nota hasta que un menú abre la entrada de al lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Donde {
    columna: u16,
    fila: u16,
}

impl Donde {
    /// La celda que quien pinta llama `(fila, columna)`, contando desde cero.
    pub fn desde_cero(fila: u16, columna: u16) -> Self {
        Self {
            columna: columna + 1,
            fila: fila + 1,
        }
    }
}

/// Los bytes que le corresponden a ese gesto, o nada si no hay nada que mandar.
///
/// Devuelve nada cuando el programa **no ha pedido** lo que se le iba a contar —un arrastre a quien
/// solo pidió los clics— y cuando las coordenadas no caben en la codificación que toca.
pub fn informe(
    gesto: Gesto,
    donde: Donde,
    reclamado: MouseReporting,
    acorde: Modifiers,
) -> Option<Vec<u8>> {
    if !pide_esto(gesto, reclamado) {
        return None;
    }
    let boton = numero_del_boton(gesto)? + suma_del_acorde(acorde);
    if reclamado.extended {
        return Some(extendido(gesto, boton, donde));
    }
    antiguo(gesto, boton, donde)
}

/// Si el programa ha pedido que se le cuente **esta clase** de gesto.
///
/// **Pedir los clics no es pedir el arrastre**, y son banderas distintas justamente porque un programa
/// que solo quiere saber dónde se pincha no quiere un informe por cada celda que el puntero recorre con
/// el botón apretado.
fn pide_esto(gesto: Gesto, reclamado: MouseReporting) -> bool {
    match gesto {
        // **La rueda va con los botones.** Un programa que pide los clics recibe los giros: es lo que
        // hace que la rueda mueva la lista de `htop` sin pedir nada más.
        Gesto::Apretar(_) | Gesto::Soltar(_) | Gesto::RuedaArriba | Gesto::RuedaAbajo => {
            reclamado.wants_mouse()
        }
        Gesto::Arrastrar(_) => reclamado.drag || reclamado.motion,
    }
}

/// El número con el que viaja el botón, antes de sumarle el acorde.
///
/// **Soltar viaja como el botón que se suelta solo en la codificación extendida.** La antigua no tiene
/// forma de decir cuál se soltó y usa un número propio para «se soltó algo», que es el 3.
fn numero_del_boton(gesto: Gesto) -> Option<u8> {
    let del_boton = |boton| match boton {
        PointerButton::Primary => Some(0),
        PointerButton::Middle => Some(1),
        PointerButton::Secondary => Some(2),
        // Los botones laterales no tienen número en esta codificación. Silencio, no un número
        // aproximado: el programa entendería un clic que nadie hizo.
        _ => None,
    };
    match gesto {
        Gesto::Apretar(boton) | Gesto::Soltar(boton) => del_boton(boton),
        // **Arrastrar lleva 32 sumado**: es el bit de «esto es movimiento», que es lo que distingue
        // recorrer una celda con el botón apretado de volver a apretarlo ahí.
        Gesto::Arrastrar(boton) => Some(del_boton(boton)? + 32),
        Gesto::RuedaArriba => Some(64),
        Gesto::RuedaAbajo => Some(65),
    }
}

/// Lo que el acorde le suma al número del botón.
///
/// **`Mayúsculas` no aparece, y es a propósito**: en esta ventana esa tecla fuerza la selección, así que
/// un gesto con ella apretada no llega nunca hasta aquí. Ponerla igualmente sería escribir una rama
/// muerta que además sugiere lo contrario de lo que la ventana hace.
fn suma_del_acorde(acorde: Modifiers) -> u8 {
    let mut suma = 0;
    if acorde.alt {
        suma += 8;
    }
    if acorde.ctrl {
        suma += 16;
    }
    suma
}

/// El informe en la codificación extendida: números en texto y una letra que dice si se apretó o soltó.
fn extendido(gesto: Gesto, boton: u8, donde: Donde) -> Vec<u8> {
    let final_ = if matches!(gesto, Gesto::Soltar(_)) {
        'm'
    } else {
        'M'
    };
    format!("\x1b[<{boton};{};{}{final_}", donde.columna, donde.fila).into_bytes()
}

/// El informe en la codificación antigua: tres bytes con 32 sumado a cada número.
///
/// Devuelve nada pasada la columna o la fila 223: ver [`TECHO_DE_LA_CODIFICACION_ANTIGUA`].
fn antiguo(gesto: Gesto, boton: u8, donde: Donde) -> Option<Vec<u8>> {
    if donde.columna > TECHO_DE_LA_CODIFICACION_ANTIGUA
        || donde.fila > TECHO_DE_LA_CODIFICACION_ANTIGUA
    {
        return None;
    }
    // **Soltar no dice qué botón se soltó**: esta codificación no tiene sitio para ello y usa el 3 para
    // «se soltó algo». Quien quiera saber cuál pide la extendida.
    let boton = if matches!(gesto, Gesto::Soltar(_)) {
        3 + (boton & !0b11)
    } else {
        boton
    };
    Some(vec![
        0x1b,
        b'[',
        b'M',
        32 + boton,
        32 + donde.columna as u8,
        32 + donde.fila as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un programa que pidió los clics y el arrastre, en la codificación antigua.
    fn como_htop() -> MouseReporting {
        MouseReporting {
            buttons: true,
            drag: true,
            ..MouseReporting::default()
        }
    }

    /// Lo mismo, pero pidiendo la codificación extendida.
    fn con_la_extendida() -> MouseReporting {
        MouseReporting {
            extended: true,
            ..como_htop()
        }
    }

    #[test]
    fn un_programa_que_no_pide_el_raton_no_recibe_nada() {
        // La consola de todos los días. Sin esto, cada clic dentro de un shell le metería basura en la
        // línea de órdenes.
        assert_eq!(
            informe(
                Gesto::Apretar(PointerButton::Primary),
                Donde::desde_cero(0, 0),
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
        let bytes = informe(
            Gesto::Apretar(PointerButton::Primary),
            Donde::desde_cero(0, 0),
            con_la_extendida(),
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
        let apretar = informe(
            Gesto::Apretar(PointerButton::Primary),
            Donde::desde_cero(4, 9),
            con_la_extendida(),
            Modifiers::NONE,
        );
        let soltar = informe(
            Gesto::Soltar(PointerButton::Primary),
            Donde::desde_cero(4, 9),
            con_la_extendida(),
            Modifiers::NONE,
        );

        assert_eq!(apretar.as_deref(), Some(&b"\x1b[<0;10;5M"[..]));
        assert_eq!(soltar.as_deref(), Some(&b"\x1b[<0;10;5m"[..]));
    }

    #[test]
    fn la_codificacion_antigua_no_dice_que_boton_se_solto() {
        // No tiene sitio para decirlo: usa un número propio para «se soltó algo». Mandar el del botón
        // haría que el programa entendiera un clic nuevo del botón central justo al soltar el derecho.
        let bytes = informe(
            Gesto::Soltar(PointerButton::Secondary),
            Donde::desde_cero(0, 0),
            como_htop(),
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
        let dentro = informe(
            Gesto::Apretar(PointerButton::Primary),
            Donde::desde_cero(0, 222),
            como_htop(),
            Modifiers::NONE,
        );
        let fuera = informe(
            Gesto::Apretar(PointerButton::Primary),
            Donde::desde_cero(0, 223),
            como_htop(),
            Modifiers::NONE,
        );

        assert!(dentro.is_some(), "la columna 223 sí cabe y se calló");
        assert_eq!(fuera, None, "la columna 224 no cabe y se mandó igual");
    }

    #[test]
    fn la_codificacion_extendida_no_tiene_ese_techo() {
        // Es la razón de ser de la extendida: sus números van en texto, así que una ventana ancha de
        // verdad sigue informando donde la antigua ya no puede.
        let bytes = informe(
            Gesto::Apretar(PointerButton::Primary),
            Donde::desde_cero(0, 399),
            con_la_extendida(),
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
        let solo_clics = MouseReporting {
            buttons: true,
            ..MouseReporting::default()
        };

        assert_eq!(
            informe(
                Gesto::Arrastrar(PointerButton::Primary),
                Donde::desde_cero(1, 1),
                solo_clics,
                Modifiers::NONE,
            ),
            None
        );
        assert!(
            informe(
                Gesto::Apretar(PointerButton::Primary),
                Donde::desde_cero(1, 1),
                solo_clics,
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
        let bytes = informe(
            Gesto::Arrastrar(PointerButton::Primary),
            Donde::desde_cero(0, 0),
            con_la_extendida(),
            Modifiers::NONE,
        )
        .expect("pidió el arrastre");

        assert_eq!(bytes, b"\x1b[<32;1;1M");
    }

    #[test]
    fn la_rueda_viaja_con_sus_dos_numeros() {
        // 64 y 65, y son lo que hace que la lista de `htop` suba y baje. Intercambiarlos deja la rueda
        // andando al revés, que es un defecto que se nota a diario y no rompe ninguna otra cosa.
        let arriba = informe(
            Gesto::RuedaArriba,
            Donde::desde_cero(0, 0),
            con_la_extendida(),
            Modifiers::NONE,
        );
        let abajo = informe(
            Gesto::RuedaAbajo,
            Donde::desde_cero(0, 0),
            con_la_extendida(),
            Modifiers::NONE,
        );

        assert_eq!(arriba.as_deref(), Some(&b"\x1b[<64;1;1M"[..]));
        assert_eq!(abajo.as_deref(), Some(&b"\x1b[<65;1;1M"[..]));
    }

    #[test]
    fn el_acorde_viaja_sumado_al_boton() {
        // `Ctrl` suma 16 y `Alt` 8, que es como el programa distingue un clic normal de uno con acorde
        // —abrir en otra pestaña, por ejemplo—. **`Mayúsculas` no aparece a propósito**: en esta
        // ventana esa tecla fuerza la selección, así que un gesto con ella no llega hasta aquí.
        let con_control = informe(
            Gesto::Apretar(PointerButton::Primary),
            Donde::desde_cero(0, 0),
            con_la_extendida(),
            Modifiers::CTRL,
        );
        let con_alt = informe(
            Gesto::Apretar(PointerButton::Primary),
            Donde::desde_cero(0, 0),
            con_la_extendida(),
            Modifiers::ALT,
        );

        assert_eq!(con_control.as_deref(), Some(&b"\x1b[<16;1;1M"[..]));
        assert_eq!(con_alt.as_deref(), Some(&b"\x1b[<8;1;1M"[..]));
    }

    #[test]
    fn los_botones_laterales_no_mandan_nada() {
        // No tienen número en esta codificación. Darles uno prestado sería contarle al programa un clic
        // de un botón que nadie tocó.
        assert_eq!(
            informe(
                Gesto::Apretar(PointerButton::Extra1),
                Donde::desde_cero(0, 0),
                con_la_extendida(),
                Modifiers::NONE,
            ),
            None
        );
    }
}
