//! El hilo que trae lo que el programa escribe, y la señal de que ha muerto.
//!
//! Existe por una razón concreta: leer del pseudoterminal **bloquea** hasta que hay algo que leer,
//! y una espera bloqueante dentro del fotograma congelaría la ventana. El hilo espera por su
//! cuenta, deja lo leído en una cola y **despierta a la ventana**. Ese despertar es lo único que
//! permite que un agente escriba solo y se le vea avanzar sin tocar el teclado.
//!
//! **Y hay una señal que se rompería en silencio si nadie la sostuviera.** La única forma de saber
//! que el programa ha muerto es que su canal de salida se cierre. Al ceder ese canal a otro hilo,
//! quien pinta deja de verlo. Aquí se retransmite: el hilo se lleva un emisor que no envía nada
//! nunca y lo suelta al terminar, así que el cierre llega al otro lado **por construcción**, no
//! porque alguien se acuerde de levantar una bandera.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex, PoisonError};

/// Lo que el programa ha escrito y todavía no se ha recogido, más la señal de si sigue vivo.
pub struct Puente {
    cola: Arc<Mutex<Vec<u8>>>,
    /// No transporta nada: lo que importa es **cuándo se cierra**, que es cuando el hilo termina.
    /// El tipo vacío deja escrito en la firma que por aquí no va a llegar ningún valor.
    defuncion: Receiver<std::convert::Infallible>,
}

impl Puente {
    /// Arranca el hilo que se queda con la salida del programa.
    ///
    /// `despertar` es lo que hay que hacer para que la ventana se entere de que hay algo nuevo; en
    /// la aplicación es pedirle un repintado. Se recibe como una función en vez de recibir la
    /// ventana entera por dos razones: así este módulo no depende del framework de interfaz —que
    /// es lo que se va a reescribir— y así **el despertar se puede comprobar en un test**, que es
    /// la única forma de demostrar la parte del hito que dice que la ventana se despierta sola.
    ///
    /// Pedir el repintado de más no cuesta —se agrupan solos— y pedirlo de menos es una ventana
    /// quieta con trabajo por mostrar.
    ///
    /// **`despertar` no puede entrar en pánico.** Si lo hiciera, el hilo moriría, se publicaría una
    /// muerte que no ha ocurrido, y de paso se destruiría el canal de salida: el lector de la caja
    /// de debajo rompería al enviar, dejaría de vaciar el pseudoterminal y el programa acabaría
    /// bloqueado al escribir.
    ///
    /// **El hilo vive mientras viva el pseudoterminal, no mientras viva este objeto.** Termina
    /// cuando se cierra el canal de salida, que es cuando se destruye el `Pty`. Destruir el puente
    /// y conservar el `Pty` deja un hilo y una cola sin dueño, creciendo. Quien los tenga, que los
    /// tenga juntos.
    pub fn nuevo(salida: Receiver<Vec<u8>>, despertar: impl Fn() + Send + 'static) -> Self {
        let cola = Arc::new(Mutex::new(Vec::new()));
        let cola_del_hilo = Arc::clone(&cola);
        let (defuncion_emisor, defuncion) = channel();

        std::thread::spawn(move || {
            // El emisor viaja al hilo sin usarse: soltarlo **es** la señal de que el programa ha
            // muerto. Se suelta a mano más abajo, y el orden con el aviso final no es un detalle:
            // ver el comentario de ahí.
            let defuncion_emisor = defuncion_emisor;
            // `for` sobre el canal termina exactamente cuando el otro extremo se cierra, que es
            // cuando el programa muere. No hay condición de parada que mantener aparte.
            for trozo in salida {
                // El candado se toma y se suelta en cada trozo: el fotograma también lo quiere, y
                // retenerlo mientras se espera al siguiente trozo sería bloquear el pintado
                // justamente durante la espera que este hilo existe para evitar.
                // Si el candado está envenenado, el dato de dentro sigue siendo válido: lo que
                // hubo fue un pánico en otro sitio. Recuperarlo es lo correcto; descartar en
                // silencio dejaría la ventana congelada sin que nada lo explicara.
                let mut cola = cola_del_hilo.lock().unwrap_or_else(PoisonError::into_inner);
                cola.extend_from_slice(&trozo);
                drop(cola);
                despertar();
            }
            // **Primero se publica la muerte y después se avisa.** Al revés hay una carrera con
            // final silencioso: el fotograma que despierta ese último aviso puede llegar antes de
            // que la señal esté publicada, ver el programa como vivo, y no recibir ningún aviso
            // más nunca — la ventana se quedaría enseñando como vivo algo que ya murió, que es
            // justo lo que este módulo existe para impedir.
            drop(defuncion_emisor);
            despertar();
        });

        Self { cola, defuncion }
    }

    /// Se lleva lo acumulado al búfer de quien llama, y deja la cola vacía.
    ///
    /// Recibe el destino en vez de devolver uno nuevo para que el fotograma pueda reutilizar
    /// siempre el mismo y no reservar memoria una vez por fotograma.
    pub fn recoger(&self, destino: &mut Vec<u8>) {
        let mut cola = self.cola.lock().unwrap_or_else(PoisonError::into_inner);
        destino.append(&mut cola);
    }

    /// Si el programa de dentro sigue vivo.
    ///
    /// Deja de serlo en cuanto el hilo termina, que es cuando su canal de salida se cierra.
    pub fn sigue_vivo(&self) -> bool {
        !matches!(self.defuncion.try_recv(), Err(TryRecvError::Disconnected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    use term_core::{Pty, Size};

    /// Espera a que se cumpla algo, con un tope para que un fallo salga como fallo y no como un
    /// test colgado para siempre.
    fn esperar_a(mut que: impl FnMut() -> bool) -> bool {
        let limite = Instant::now() + Duration::from_secs(5);
        while Instant::now() < limite {
            if que() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn lo_que_el_programa_escribe_llega_a_la_cola() {
        let (_pty, salida) = Pty::spawn("sh", &["-c", "printf hola"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let puente = Puente::nuevo(salida, || {});

        let mut recogido = Vec::new();
        let llego = esperar_a(|| {
            puente.recoger(&mut recogido);
            recogido.windows(4).any(|v| v == b"hola")
        });

        assert!(llego, "no llegó lo que el programa escribió: {recogido:?}");
    }

    #[test]
    fn la_muerte_del_programa_sobrevive_a_haber_cedido_el_canal() {
        // Este es el contrato que se rompe en silencio meses después: ceder el canal a otro hilo
        // se lleva consigo la única señal de que el programa ha muerto. Si este test se pone rojo,
        // la ventana se quedaría abierta y viva delante de un programa que ya no existe.
        let (_pty, salida) = Pty::spawn("sh", &["-c", "exit 0"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let puente = Puente::nuevo(salida, || {});

        assert!(
            esperar_a(|| !puente.sigue_vivo()),
            "el programa terminó y el puente sigue diciendo que está vivo"
        );
    }

    #[test]
    fn cuando_llega_el_aviso_final_la_muerte_ya_esta_publicada() {
        // El orden entre publicar la muerte y dar el último aviso decide si el fallo es visible o
        // silencioso: si se avisara primero, el fotograma que ese aviso despierta podría ver el
        // programa como vivo, y después no habría ningún aviso más — la ventana se quedaría
        // enseñando como vivo algo ya muerto, para siempre.
        //
        // Se caza así: el programa no escribe nada, con lo que el único aviso es el final; y el
        // despertador se entretiene dentro, de modo que el test puede mirar el estado **mientras**
        // el hilo sigue dentro de esa llamada. Con el orden equivocado, aquí se leería «vivo».
        let avisado = Arc::new(AtomicUsize::new(0));
        let marca = Arc::clone(&avisado);
        let (_pty, salida) = Pty::spawn("sh", &["-c", "exit 0"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let puente = Puente::nuevo(salida, move || {
            marca.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(300));
        });

        assert!(
            esperar_a(|| avisado.load(Ordering::SeqCst) > 0),
            "no llegó el aviso de que el programa terminó"
        );
        assert!(
            !puente.sigue_vivo(),
            "el aviso final llegó antes de que la muerte estuviera publicada"
        );
    }

    #[test]
    fn mientras_el_programa_vive_el_puente_lo_dice() {
        let (_pty, salida) = Pty::spawn("sh", &["-c", "sleep 30"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let puente = Puente::nuevo(salida, || {});

        // Sin esta comprobación, un puente que dijera «muerto» desde el primer instante pasaría el
        // test de la defunción sin haber retransmitido nada.
        std::thread::sleep(Duration::from_millis(100));
        assert!(puente.sigue_vivo(), "el programa sigue corriendo");
    }

    #[test]
    fn la_llegada_de_bytes_despierta_a_la_ventana() {
        // Es la pieza que hace posible ver a un agente trabajar sin tocar el teclado. Sin este
        // aviso los bytes llegarían a la cola y **la ventana seguiría dormida**, mostrando algo
        // viejo hasta que el usuario moviera el ratón. Que la cola se llene no lo demuestra: hay
        // que comprobar que se pidió el repintado.
        // El programa escribe y **se queda vivo** a propósito: si terminara, el aviso que el hilo
        // manda al despedirse taparía el que aquí se prueba, y el test pasaría aunque nadie
        // despertara al llegar los bytes. Comprobado mutando el código: sin el aviso del bucle,
        // este test se pone rojo.
        let avisos = Arc::new(AtomicUsize::new(0));
        let contador = Arc::clone(&avisos);
        let (_pty, salida) = Pty::spawn(
            "sh",
            &["-c", "printf algo; sleep 30"],
            Size { rows: 24, cols: 80 },
        )
        .expect("no se pudo abrir el pseudoterminal");
        let puente = Puente::nuevo(salida, move || {
            contador.fetch_add(1, Ordering::SeqCst);
        });

        let mut recogido = Vec::new();
        assert!(esperar_a(|| {
            puente.recoger(&mut recogido);
            !recogido.is_empty()
        }));
        assert!(
            avisos.load(Ordering::SeqCst) > 0,
            "llegaron bytes y nadie despertó a la ventana"
        );
    }

    #[test]
    fn recoger_deja_la_cola_vacia() {
        let (_pty, salida) = Pty::spawn("sh", &["-c", "printf uno"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let puente = Puente::nuevo(salida, || {});

        // Se espera a que el programa termine antes de recoger. Con un programa vivo, el
        // pseudoterminal podría partir «uno» en dos entregas y el test fallaría un día de cada
        // muchos por una razón que no tiene que ver con lo que prueba.
        assert!(esperar_a(|| !puente.sigue_vivo()));

        let mut primera = Vec::new();
        puente.recoger(&mut primera);
        assert!(!primera.is_empty(), "no llegó nada que recoger");

        // Lo ya recogido no se vuelve a entregar: si se repitiera, la pantalla mostraría el mismo
        // texto dos veces sin que el programa lo hubiera escrito dos veces.
        let mut segunda = Vec::new();
        puente.recoger(&mut segunda);
        assert!(segunda.is_empty(), "la cola devolvió lo mismo dos veces");
    }

    #[test]
    fn el_destino_conserva_lo_que_ya_tenia() {
        // El fotograma reutiliza su búfer y puede llamar con restos de la pasada anterior todavía
        // sin procesar. Si recoger los pisara, se perderían bytes ya leídos del programa.
        let (_pty, salida) = Pty::spawn("sh", &["-c", "printf dos"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let puente = Puente::nuevo(salida, || {});

        let mut destino = b"antes".to_vec();
        assert!(esperar_a(|| {
            puente.recoger(&mut destino);
            destino.len() > b"antes".len()
        }));
        assert!(destino.starts_with(b"antes"), "se perdió lo que ya había");
    }
}
