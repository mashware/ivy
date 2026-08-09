//! Qué bytes manda al programa cada tecla.
//!
//! Es una función pura de un evento de la ventana a los bytes que un terminal enviaría. No escribe
//! en ningún sitio: quien los mande es el bucle, y por dónde los mande es el puente.
//!
//! **Dónde se corta este hito, y por qué.** Quedan fuera Alt, las teclas de función y el teclado
//! numérico. Ninguna hace falta para escribir órdenes en un shell ni para moverse por un editor,
//! que es lo que el hito tiene que poder demostrar; y las tres arrastran una decisión que aquí no
//! toca tomar —Alt antepone un escape a lo que envía la tecla, y las otras dos cambian de bytes
//! según el modo en que esté el programa—. Meterlas sin poder verlas funcionando sería escribir
//! tablas a ciegas. Lo que queda fuera devuelve silencio, nunca un byte aproximado: un byte
//! equivocado el programa de dentro lo entiende como otra tecla, y eso es peor que no recibir nada.

use eframe::egui::{Event, Key, Modifiers};
use std::borrow::Cow;

/// Los bytes que le corresponden a un evento, o nada si ese evento no se traduce.
pub fn bytes(evento: &Event) -> Option<Cow<'static, [u8]>> {
    match evento {
        // El texto ya viene resuelto por el sistema: acentos, teclas muertas y métodos de entrada
        // incluidos. Traducirlo desde la tecla física en vez de desde aquí sería rehacer mal el
        // trabajo del escritorio.
        Event::Text(texto) if !texto.is_empty() => Some(Cow::Owned(texto.as_bytes().to_vec())),

        // **Estas tres teclas no llegan como teclas.** La capa que traduce los eventos del sistema
        // operativo mira Ctrl+C, Ctrl+X y Ctrl+V *antes* de emitir nada y las convierte en copiar,
        // cortar y pegar, sin emitir el evento de tecla. En un escritorio normal eso es lo que se
        // espera; en un terminal, Ctrl+C es la forma de interrumpir un programa colgado, así que
        // hay que recogerlas por esta puerta o no interrumpen nada.
        //
        // **Lo que esto cuesta, dicho claro:** mientras el portapapeles no exista, en esta ventana
        // no se puede copiar. Estos eventos no traen los modificadores, así que desde aquí no hay
        // forma de distinguir Ctrl+C de Ctrl+Mayúsculas+C —que es como se copia en un terminal—, y
        // las dos acaban interrumpiendo. Quien traiga el portapapeles tendrá que decidirlo antes,
        // en el bucle, donde el estado de los modificadores sí está disponible.
        Event::Copy => Some(Cow::Borrowed(&[3])),
        Event::Cut => Some(Cow::Borrowed(&[24])),
        // Pegar de verdad es de otro hito. Aquí Ctrl+V vale lo que vale en un terminal: el byte de
        // control que le corresponde, que es además lo que un shell entiende como «lo siguiente,
        // literal». El texto del portapapeles se descarta a propósito.
        Event::Paste(_) => Some(Cow::Borrowed(&[22])),

        // Soltar una tecla no manda nada. La repetición de mantenerla pulsada sí: es lo que hace
        // que dejar apretada una flecha mueva el cursor.
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => tecla(*key, *modifiers),

        _ => None,
    }
}

fn tecla(key: Key, modifiers: Modifiers) -> Option<Cow<'static, [u8]>> {
    // Alt y la tecla de comando de macOS quedan fuera del hito, y «fuera» significa silencio: Alt
    // antepone un escape, así que mandar el byte pelado no sería una aproximación, sería otra
    // tecla. Mayúsculas sí pasa, porque no cambia el código de control — es lo que hace `xterm`.
    if modifiers.alt || modifiers.mac_cmd {
        return None;
    }
    if modifiers.ctrl
        && let Some(codigo) = con_control(key)
    {
        return Some(Cow::Owned(vec![codigo]));
    }
    // Si Ctrl acompaña a algo que no es una letra, se cae a la tabla de siempre en vez de callar:
    // Ctrl+Enter manda retorno igual que Enter, y quedarse mudo sería una tecla muerta.
    let salida: &'static [u8] = match key {
        // Retorno de carro, no salto de línea: es lo que envía un terminal, y lo que el programa
        // de dentro traduce según le convenga.
        Key::Enter => b"\r",
        // Con Mayúsculas, Tab es otra tecla: retrocede la tabulación en vez de avanzarla, y tiene
        // su propia secuencia. Mandar el tabulador de siempre movería el foco al revés de como el
        // programa lo pidió.
        Key::Tab if modifiers.shift => b"\x1b[Z",
        Key::Tab => b"\t",
        // Los terminales modernos mandan borrar, no retroceso. Con el otro byte, el shell borra
        // pero muchos editores no.
        Key::Backspace => &[0x7F],
        Key::Escape => &[0x1B],
        // Las cuatro flechas en modo normal. Un programa puede pedir el modo de aplicación, en el
        // que cambian de forma; eso queda fuera de este hito y está escrito arriba.
        Key::ArrowUp => b"\x1b[A",
        Key::ArrowDown => b"\x1b[B",
        Key::ArrowRight => b"\x1b[C",
        Key::ArrowLeft => b"\x1b[D",
        _ => return None,
    };
    Some(Cow::Borrowed(salida))
}

/// Ctrl con una letra da los códigos de control del 1 al 26.
///
/// Es la regla de siempre —la letra menos sesenta y cuatro— y es la que hace que Ctrl+D cierre la
/// entrada y Ctrl+Z suspenda.
///
/// Las veintiséis van escritas una a una, y no sacadas del nombre de la tecla, aunque ese nombre
/// resulte ser hoy la propia letra: ese texto está pensado para enseñárselo a una persona en un
/// menú de atajos, no es un contrato. Si mañana cambia, un byte de control cambiaría con él sin
/// que nada dejara de compilar.
fn con_control(key: Key) -> Option<u8> {
    let codigo = match key {
        Key::A => 1,
        Key::B => 2,
        Key::C => 3,
        Key::D => 4,
        Key::E => 5,
        Key::F => 6,
        Key::G => 7,
        Key::H => 8,
        Key::I => 9,
        Key::J => 10,
        Key::K => 11,
        Key::L => 12,
        Key::M => 13,
        Key::N => 14,
        Key::O => 15,
        Key::P => 16,
        Key::Q => 17,
        Key::R => 18,
        Key::S => 19,
        Key::T => 20,
        Key::U => 21,
        Key::V => 22,
        Key::W => 23,
        Key::X => 24,
        Key::Y => 25,
        Key::Z => 26,
        _ => return None,
    };
    Some(codigo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn con(key: Key, modifiers: Modifiers) -> Option<Vec<u8>> {
        bytes(&Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        })
        .map(|b| b.into_owned())
    }

    fn pulsar(key: Key) -> Option<Vec<u8>> {
        con(key, Modifiers::NONE)
    }

    fn pulsar_con_control(key: Key) -> Option<Vec<u8>> {
        con(key, Modifiers::CTRL)
    }

    fn texto(s: &str) -> Option<Vec<u8>> {
        bytes(&Event::Text(s.into())).map(|b| b.into_owned())
    }

    #[test]
    fn el_texto_viaja_tal_cual_en_utf8() {
        assert_eq!(texto("ls"), Some(b"ls".to_vec()));
        // Un carácter que no es ASCII ocupa varios bytes y tiene que salir entero.
        assert_eq!(texto("ñ"), Some(vec![0xC3, 0xB1]), "la eñe son dos bytes");
    }

    #[test]
    fn el_texto_vacio_es_nada_que_mandar_y_no_una_lista_vacia() {
        // Devolver algo vacío obligaría a quien llame a distinguir «no hay nada que mandar» de «no
        // se traduce», que son la misma decisión para él.
        assert_eq!(texto(""), None);
    }

    #[test]
    fn las_cuatro_teclas_de_control_basicas() {
        // Retorno de carro, no salto de línea.
        assert_eq!(pulsar(Key::Enter), Some(vec![0x0D]));
        assert_eq!(pulsar(Key::Tab), Some(vec![0x09]));
        // Borrar, no retroceso: con 0x08 el shell borra pero muchos editores no.
        assert_eq!(pulsar(Key::Backspace), Some(vec![0x7F]));
        assert_eq!(pulsar(Key::Escape), Some(vec![0x1B]));
    }

    #[test]
    fn las_flechas_van_como_secuencia_de_escape() {
        assert_eq!(pulsar(Key::ArrowUp), Some(b"\x1b[A".to_vec()));
        assert_eq!(pulsar(Key::ArrowDown), Some(b"\x1b[B".to_vec()));
        assert_eq!(pulsar(Key::ArrowRight), Some(b"\x1b[C".to_vec()));
        assert_eq!(pulsar(Key::ArrowLeft), Some(b"\x1b[D".to_vec()));
    }

    #[test]
    fn las_veintiseis_letras_caen_en_su_sitio_y_ninguna_se_sale() {
        let letras = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        for (i, key) in letras.iter().enumerate() {
            let esperado = (i + 1) as u8;
            assert_eq!(
                pulsar_con_control(*key),
                Some(vec![esperado]),
                "la letra en la posición {i} no da su código de control"
            );
        }
        // Los dos que se usan a diario, nombrados para que se lea qué está en juego.
        assert_eq!(
            pulsar_con_control(Key::D),
            Some(vec![4]),
            "cierra la entrada"
        );
        assert_eq!(pulsar_con_control(Key::Z), Some(vec![26]), "suspende");
    }

    #[test]
    fn interrumpir_cortar_y_pegar_llegan_por_su_propia_puerta() {
        // La capa de debajo se queda con Ctrl+C, Ctrl+X y Ctrl+V y las convierte en copiar, cortar
        // y pegar antes de emitir tecla ninguna. Si este caso desapareciera, **Ctrl+C dejaría de
        // interrumpir** y ningún test de teclas lo notaría.
        assert_eq!(
            bytes(&Event::Copy).map(|b| b.into_owned()),
            Some(vec![3]),
            "Ctrl+C tiene que seguir interrumpiendo"
        );
        assert_eq!(bytes(&Event::Cut).map(|b| b.into_owned()), Some(vec![24]));
        assert_eq!(
            bytes(&Event::Paste("lo que hubiera".into())).map(|b| b.into_owned()),
            Some(vec![22]),
            "pegar de verdad es de otro hito; aquí vale lo que vale en un terminal"
        );
    }

    #[test]
    fn una_tecla_de_letra_por_si_sola_no_manda_nada() {
        // La capa de debajo emite **dos** eventos por cada pulsación normal: la tecla y el texto
        // que produce. Si la tabla de teclas contestara también a las letras, cada carácter que
        // escribiera el usuario llegaría por duplicado al programa de dentro. El texto es el que
        // manda, y la tecla tiene que callar.
        assert_eq!(pulsar(Key::A), None);
        assert_eq!(pulsar(Key::Z), None);
        assert_eq!(pulsar(Key::Space), None);
        assert_eq!(pulsar(Key::Num0), None);
        // Y con mayúsculas igual: la «A» también viaja como texto.
        assert_eq!(con(Key::A, Modifiers::SHIFT), None);
    }

    #[test]
    fn el_control_de_linux_y_windows_viaja_ademas_en_command() {
        // Fuera de macOS, Ctrl llega marcado en los dos sitios a la vez. Confundir ese `command`
        // con la tecla de comando de macOS **dejaría a Linux y Windows sin ningún Ctrl**.
        //
        // Se prueba con la D y no con la C, la X o la V: esas tres nunca llegan como tecla porque
        // la capa de debajo las convierte antes en copiar, cortar y pegar.
        let ctrl_de_linux = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(con(Key::D, ctrl_de_linux), Some(vec![4]));
    }

    #[test]
    fn la_tecla_de_comando_de_macos_no_manda_bytes_de_control() {
        // En macOS ⌘ llega sin `ctrl` y con `mac_cmd`. Si se tratara como un Ctrl cualquiera,
        // ⌘+Q pediría salir de la aplicación y además mandaría el byte que cuelga la salida del
        // terminal.
        let comando_de_macos = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(con(Key::Q, comando_de_macos), None);
        assert_eq!(con(Key::Enter, comando_de_macos), None);
    }

    #[test]
    fn mayusculas_con_enter_sigue_siendo_retorno() {
        // Mayúsculas sola no pasa por la tabla de códigos de control, así que necesita su propia
        // prueba: si se filtrara como se filtra Alt, **Mayúsculas+Enter sería una tecla muerta**.
        assert_eq!(con(Key::Enter, Modifiers::SHIFT), Some(vec![0x0D]));
    }

    #[test]
    fn mayusculas_con_tabulador_retrocede_en_vez_de_avanzar() {
        // Es la única tecla a la que Mayúsculas le cambia el significado: con ella el programa
        // retrocede el foco en vez de avanzarlo. Si esto mandara el tabulador de siempre,
        // **moverse hacia atrás por un formulario iría hacia delante**.
        assert_eq!(con(Key::Tab, Modifiers::SHIFT), Some(b"\x1b[Z".to_vec()));
        assert_eq!(
            pulsar(Key::Tab),
            Some(vec![0x09]),
            "y sin Mayúsculas, avanza"
        );
    }

    #[test]
    fn el_byte_lo_decide_la_tecla_logica_y_no_la_fisica() {
        // La capa de debajo manda las dos: la que dice la distribución del teclado y la posición
        // física. En una distribución que no sea la inglesa no coinciden, y hacer caso a la
        // posición mandaría el byte de control **de otra letra**.
        let en_otra_distribucion = Event::Key {
            key: Key::D,
            physical_key: Some(Key::H),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        };
        assert_eq!(
            bytes(&en_otra_distribucion).map(|b| b.into_owned()),
            Some(vec![4]),
            "cierra la entrada, no borra"
        );
    }

    #[test]
    fn mayusculas_no_cambia_el_codigo_de_control() {
        // Es lo que hace un terminal de verdad: Ctrl+Shift+C manda lo mismo que Ctrl+C.
        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(con(Key::D, ctrl_shift), Some(vec![4]));
    }

    #[test]
    fn alt_no_manda_un_byte_aproximado() {
        // Alt antepone un escape a lo que envía la tecla. Mandar el byte pelado no sería una
        // aproximación: el programa de dentro lo entendería como otra pulsación.
        let ctrl_alt = Modifiers {
            ctrl: true,
            alt: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(con(Key::D, ctrl_alt), None);
        assert_eq!(con(Key::Enter, Modifiers::ALT), None);
    }

    #[test]
    fn control_con_una_tecla_que_no_es_letra_cae_en_la_tabla_de_siempre() {
        // Quedarse mudo aquí convertiría a Ctrl+Enter en una tecla muerta, cuando lo que manda un
        // terminal es exactamente lo mismo que sin Ctrl.
        assert_eq!(pulsar_con_control(Key::Enter), Some(vec![0x0D]));
        assert_eq!(pulsar_con_control(Key::Backspace), Some(vec![0x7F]));
        assert_eq!(pulsar_con_control(Key::ArrowUp), Some(b"\x1b[A".to_vec()));
        // Y lo que no está en ninguna de las dos tablas sigue sin mandar nada.
        assert_eq!(pulsar_con_control(Key::F1), None);
    }

    #[test]
    fn soltar_una_tecla_no_manda_nada() {
        let soltar = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(bytes(&soltar).is_none());
    }

    #[test]
    fn mantener_una_tecla_pulsada_sigue_mandando() {
        // La repetición es lo que hace que dejar apretada una flecha mueva el cursor. Filtrarla
        // sería romper algo que se nota a la primera.
        let repetida = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            bytes(&repetida).map(|b| b.into_owned()),
            Some(b"\x1b[B".to_vec())
        );
    }

    #[test]
    fn las_teclas_que_este_hito_deja_fuera_no_inventan_bytes() {
        // Están fuera a propósito, y lo que importa es que se note como silencio y no como un byte
        // equivocado que el programa de dentro interpretaría como otra cosa.
        assert_eq!(pulsar(Key::F1), None);
        assert_eq!(pulsar(Key::Home), None);
        assert_eq!(pulsar(Key::PageUp), None);
        assert_eq!(pulsar(Key::Delete), None);
    }

    #[test]
    fn un_evento_que_no_es_de_teclado_se_ignora() {
        assert!(bytes(&Event::WindowFocused(true)).is_none());
    }
}
