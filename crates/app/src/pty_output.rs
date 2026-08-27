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

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Cuánto se guarda por adelantado antes de dejar de recoger del canal.
///
/// **Es la otra mitad del freno**, y sin ella la de abajo no sirve de nada: el canal de
/// `term-core` tiene techo, pero un hilo que lo vacíe en cuanto llega algo hace que ese techo no
/// se alcance nunca, y entonces nada frena al programa. Medido: acotar solo el canal deja el
/// consumo de memoria exactamente igual que antes.
///
/// Un megabyte son ocho fotogramas de trabajo por delante con el presupuesto actual: suficiente
/// para que el pintado nunca se quede esperando bytes, y poco para que retenerlo dé igual. Lo que
/// no cabe no se pierde: se queda en el programa que lo escribió, que espera.
const QUEUE_CEILING: usize = 1024 * 1024;

/// Cada cuánto se despierta el hilo a comprobar por su cuenta si ya hay sitio.
///
/// La señal de quien recoge es lo que despierta normalmente; este plazo solo existe para que una
/// señal perdida cueste un milisegundo y no un cuelgue.
const RECHECK: Duration = Duration::from_millis(1);

/// Lo que hay en la cola: los bytes y cuándo llegó cada tanda.
#[derive(Default)]
struct Queue {
    bytes: VecDeque<u8>,
    /// Cuándo llegó cada tanda y hasta qué byte llega, contando desde el principio de todo lo que
    /// ha pasado por aquí. Existe para poder responder **cuánto lleva esperando** lo que se recoge:
    /// en un volcado esa espera es el tramo que domina el retraso visual —más de un segundo, frente
    /// a los microsegundos de todo lo demás— y sin sellarla el número diría que la aplicación es
    /// instantánea justo cuando no lo es.
    arrivals: VecDeque<(u64, Instant)>,
    /// Cuántos bytes se han recogido ya en total. Es el origen contra el que se leen las llegadas.
    bytes_picked_up: u64,
}

/// El resultado de una recogida: si queda cola, y desde cuándo esperaba lo que se ha llevado.
pub struct Pickup {
    /// El presupuesto cortó el bombeo y queda cola por tragar.
    pub left_a_queue: bool,
    /// Cuándo llegó el más antiguo de los bytes recogidos, o nada si no se recogió ninguno. Es el
    /// principio del tramo que el usuario ve como retraso.
    pub waiting_since: Option<Instant>,
}

/// Lo que el programa ha escrito y todavía no se ha recogido, más la señal de si sigue vivo.
pub struct PtyOutput {
    /// Una cola por los dos extremos y no un vector: se añade por el final y se saca por el
    /// principio, y sacar del principio de un vector copia todo lo que queda detrás. Con un
    /// volcado en marcha eso serían megabytes copiados en cada fotograma, y ese coste acabaría
    /// dentro del número que este hito existe para medir.
    queue: Arc<Mutex<Queue>>,
    /// Avisa al hilo de que se ha hecho sitio en la cola y puede volver a recoger del canal.
    ///
    /// Va aparte del candado y no dentro de `Queue` porque quien espera y quien avisa son hilos
    /// distintos y ninguno de los dos es dueño del otro.
    room: Arc<Condvar>,
    /// Saca al hilo de la espera por sitio cuando este objeto deja de existir.
    ///
    /// Esperar sitio es lo único que el hilo hace **sin** mirar el canal, así que ahí no se entera
    /// de que el programa ha muerto ni de que ya nadie va a recoger. Sin esta bandera, un puente
    /// destruido con la cola en el techo deja su hilo girando para siempre.
    stop: Arc<AtomicBool>,
    /// No transporta nada: lo que importa es **cuándo se cierra**, que es cuando el hilo termina.
    /// El tipo vacío deja escrito en la firma que por aquí no va a llegar ningún valor.
    death: Receiver<std::convert::Infallible>,
}

impl PtyOutput {
    /// Arranca el hilo que se queda con la salida del programa.
    ///
    /// `wake` es lo que hay que hacer para que la ventana se entere de que hay algo nuevo; en
    /// la aplicación es pedirle un repintado. Se recibe como una función en vez de recibir la
    /// ventana entera por dos razones: así este módulo no depende del framework de interfaz —que
    /// es lo que se va a reescribir— y así **el despertar se puede comprobar en un test**, que es
    /// la única forma de demostrar la parte del hito que dice que la ventana se despierta sola.
    ///
    /// Pedir el repintado de más no cuesta —se agrupan solos— y pedirlo de menos es una ventana
    /// quieta con trabajo por mostrar.
    ///
    /// **`wake` no puede entrar en pánico.** Si lo hiciera, el hilo moriría, se publicaría una
    /// muerte que no ha ocurrido, y de paso se destruiría el canal de salida: el lector de la caja
    /// de debajo rompería al enviar, dejaría de vaciar el pseudoterminal y el programa acabaría
    /// bloqueado al escribir.
    ///
    /// **El hilo termina con lo que ocurra primero:** que se cierre el canal de salida —o sea, que
    /// muera el programa— o que se destruya este objeto. Lo segundo hace falta porque esperar sitio
    /// es lo único que el hilo hace sin mirar el canal, y ahí no se enteraría de ninguna de las dos
    /// cosas. Aun así conviene tenerlo junto al `Pty`: separarlos deja una cola sin dueño.
    pub fn new(output: Receiver<Vec<u8>>, wake: impl Fn() + Send + 'static) -> Self {
        let queue = Arc::new(Mutex::new(Queue::default()));
        let thread_queue = Arc::clone(&queue);
        let room = Arc::new(Condvar::new());
        let thread_room = Arc::clone(&room);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let (death_sender, death) = channel();

        std::thread::spawn(move || {
            // El emisor viaja al hilo sin usarse: soltarlo **es** la señal de que el programa ha
            // muerto. Se suelta a mano más abajo, y el orden con el aviso final no es un detalle:
            // ver el comentario de ahí.
            let death_sender = death_sender;
            // `for` sobre el canal termina exactamente cuando el otro extremo se cierra, que es
            // cuando el programa muere. La etiqueta es para la otra salida, la de más abajo.
            'pickup: for chunk in output {
                // El sello se toma **antes** de pedir el candado: si el fotograma lo tuviera
                // tomado, esperar por él se contaría como tiempo de llegada y la espera que se
                // publica saldría inflada por la contención, que no es lo que se quiere medir.
                let arrival = Instant::now();
                // El candado se toma y se suelta en cada trozo: el fotograma también lo quiere, y
                // retenerlo mientras se espera al siguiente trozo sería bloquear el pintado
                // justamente durante la espera que este hilo existe para evitar.
                // Si el candado está envenenado, el dato de dentro sigue siendo válido: lo que
                // hubo fue un pánico en otro sitio. Recuperarlo es lo correcto; descartar en
                // silencio dejaría la ventana congelada sin que nada lo explicara.
                let mut queue = thread_queue.lock().unwrap_or_else(PoisonError::into_inner);
                // **Aquí es donde este hilo deja de recoger, y por eso el programa se frena.**
                // Mientras se espera, el canal de la caja de abajo se llena, su hilo lector deja de
                // vaciar el pseudoterminal, el búfer del núcleo se llena y el programa se bloquea al
                // escribir. Se espera con el trozo ya en la mano —recibido pero sin encolar—, que es
                // lo que hace que el canal note el atasco.
                while queue.bytes.len() >= QUEUE_CEILING {
                    // Quien recoge es quien hace sitio, así que si este objeto ya no existe, el
                    // sitio no va a llegar nunca. Se sale por el epílogo de abajo y no con un
                    // `return` para no saltarse el orden entre publicar la muerte y avisar.
                    if thread_stop.load(Ordering::Relaxed) {
                        break 'pickup;
                    }
                    let (reacquired, _) = thread_room
                        .wait_timeout(queue, RECHECK)
                        .unwrap_or_else(PoisonError::into_inner);
                    queue = reacquired;
                }
                queue.bytes.extend(&chunk);
                let up_to = queue.bytes_picked_up + queue.bytes.len() as u64;
                queue.arrivals.push_back((up_to, arrival));
                drop(queue);
                wake();
            }
            // **Primero se publica la muerte y después se avisa.** Al revés hay una carrera con
            // final silencioso: el fotograma que despierta ese último aviso puede llegar antes de
            // que la señal esté publicada, ver el programa como vivo, y no recibir ningún aviso
            // más nunca — la ventana se quedaría enseñando como vivo algo que ya murió, que es
            // justo lo que este módulo existe para impedir.
            drop(death_sender);
            wake();
        });

        Self {
            queue,
            room,
            stop,
            death,
        }
    }

    /// Se lleva hasta `max` bytes de lo acumulado. Devuelve si ha quedado algo sin recoger.
    ///
    /// **El máximo no es un adorno y llevárselo todo no era una opción.** Un volcado entrega dos
    /// órdenes de magnitud más rápido de lo que la rejilla interpreta, así que vaciar la cola
    /// entera en el búfer de quien llama solo mueve el montón de sitio y lo duplica mientras dura.
    /// Con el tope, el búfer de quien llama no crece nunca más allá de lo que va a interpretar.
    ///
    /// **El máximo no es el techo de la cola**, y conviene no confundirlos: este acota el trabajo
    /// de un fotograma, y el techo ([`QUEUE_CEILING`]) acota la memoria. Recoger es además lo que hace
    /// sitio, así que dejar de llamar a esto acaba frenando al programa de dentro — que es
    /// justamente lo que se quiere cuando el pintado no da abasto.
    ///
    /// Recibe el destino en vez de devolver uno nuevo para que el fotograma pueda reutilizar
    /// siempre el mismo y no reservar memoria una vez por fotograma.
    pub fn pick_up(&self, destination: &mut Vec<u8>, max: usize) -> Pickup {
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);

        // La cola guarda sus bytes en dos tramos —da la vuelta al llegar al final—, así que hay
        // que copiar de los dos. Copiar de las rebanadas y no del iterador es lo que hace que esto
        // cueste lo que se lleva y no lo que se queda.
        let to_take = queue.bytes.len().min(max);
        let (first, second) = queue.bytes.as_slices();
        let from_first = first.len().min(to_take);
        destination.extend_from_slice(&first[..from_first]);
        destination.extend_from_slice(&second[..to_take - from_first]);
        queue.bytes.drain(..to_take);

        // Cuándo llegó el primero de los bytes que se acaban de llevar: la tanda cuyo último byte
        // cae más allá de lo ya recogido. Es el más antiguo de los que van en esta entrega, así que
        // la espera que mide es la peor de las que se resuelven en este fotograma — la que se ve.
        let waiting_since = queue
            .arrivals
            .iter()
            .find(|(up_to, _)| *up_to > queue.bytes_picked_up)
            .map(|(_, when)| *when);

        queue.bytes_picked_up += to_take as u64;
        // Las tandas ya servidas enteras dejan de importar. Sin esta purga la lista crecería para
        // siempre durante un volcado, que es justo cuando no puede permitírselo.
        let bytes_picked_up = queue.bytes_picked_up;
        queue.arrivals.retain(|(up_to, _)| *up_to > bytes_picked_up);

        let pickup = Pickup {
            left_a_queue: !queue.bytes.is_empty(),
            waiting_since,
        };
        // Se suelta el candado **antes** de avisar: si no, el hilo despierta y lo primero que hace
        // es esperar a que lo soltemos.
        drop(queue);
        self.room.notify_one();
        pickup
    }

    /// Si el programa de dentro sigue vivo.
    ///
    /// Deja de serlo en cuanto el hilo termina, que es cuando su canal de salida se cierra.
    pub fn is_alive(&self) -> bool {
        !matches!(self.death.try_recv(), Err(TryRecvError::Disconnected))
    }
}

impl Drop for PtyOutput {
    fn drop(&mut self) {
        // No se espera al hilo: puede estar dentro de una lectura del canal que tarde lo que tarde
        // el programa en escribir, y bloquear el cierre de la ventana por eso sería cambiar un
        // cuelgue por otro. Basta con que deje de esperar sitio, que es lo único de lo que no
        // saldría solo.
        self.stop.store(true, Ordering::Relaxed);
        self.room.notify_all();
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
    fn wait_for(mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn lo_que_el_programa_escribe_llega_a_la_cola() {
        let (_pty, output) = Pty::spawn("sh", &["-c", "printf hola"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        let mut received = Vec::new();
        let arrived = wait_for(|| {
            pty_output.pick_up(&mut received, usize::MAX);
            received.windows(4).any(|v| v == b"hola")
        });

        assert!(
            arrived,
            "no llegó lo que el programa escribió: {received:?}"
        );
    }

    #[test]
    fn la_muerte_del_programa_sobrevive_a_haber_cedido_el_canal() {
        // Este es el contrato que se rompe en silencio meses después: ceder el canal a otro hilo
        // se lleva consigo la única señal de que el programa ha muerto. Si este test se pone rojo,
        // la ventana se quedaría abierta y viva delante de un programa que ya no existe.
        let (_pty, output) = Pty::spawn("sh", &["-c", "exit 0"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        assert!(
            wait_for(|| !pty_output.is_alive()),
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
        let wakeups = Arc::new(AtomicUsize::new(0));
        let thread_wakeups = Arc::clone(&wakeups);
        let (_pty, output) = Pty::spawn("sh", &["-c", "exit 0"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, move || {
            thread_wakeups.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(300));
        });

        assert!(
            wait_for(|| wakeups.load(Ordering::SeqCst) > 0),
            "no llegó el aviso de que el programa terminó"
        );
        assert!(
            !pty_output.is_alive(),
            "el aviso final llegó antes de que la muerte estuviera publicada"
        );
    }

    #[test]
    fn mientras_el_programa_vive_el_puente_lo_dice() {
        let (_pty, output) = Pty::spawn("sh", &["-c", "sleep 30"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        // Sin esta comprobación, un puente que dijera «muerto» desde el primer instante pasaría el
        // test de la defunción sin haber retransmitido nada.
        std::thread::sleep(Duration::from_millis(100));
        assert!(pty_output.is_alive(), "el programa sigue corriendo");
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
        let wakeups = Arc::new(AtomicUsize::new(0));
        let thread_wakeups = Arc::clone(&wakeups);
        let (_pty, output) = Pty::spawn(
            "sh",
            &["-c", "printf algo; sleep 30"],
            Size { rows: 24, cols: 80 },
        )
        .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, move || {
            thread_wakeups.fetch_add(1, Ordering::SeqCst);
        });

        let mut received = Vec::new();
        assert!(wait_for(|| {
            pty_output.pick_up(&mut received, usize::MAX);
            !received.is_empty()
        }));
        assert!(
            wakeups.load(Ordering::SeqCst) > 0,
            "llegaron bytes y nadie despertó a la ventana"
        );
    }

    #[test]
    fn recoger_deja_la_cola_vacia() {
        let (_pty, output) = Pty::spawn("sh", &["-c", "printf uno"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        // Se espera a que el programa termine antes de recoger. Con un programa vivo, el
        // pseudoterminal podría partir «uno» en dos entregas y el test fallaría un día de cada
        // muchos por una razón que no tiene que ver con lo que prueba.
        assert!(wait_for(|| !pty_output.is_alive()));

        let mut first = Vec::new();
        pty_output.pick_up(&mut first, usize::MAX);
        assert!(!first.is_empty(), "no llegó nada que recoger");

        // Lo ya recogido no se vuelve a entregar: si se repitiera, la pantalla mostraría el mismo
        // texto dos veces sin que el programa lo hubiera escrito dos veces.
        let mut second = Vec::new();
        pty_output.pick_up(&mut second, usize::MAX);
        assert!(second.is_empty(), "la cola devolvió lo mismo dos veces");
    }

    #[test]
    fn el_maximo_se_respeta_y_lo_que_sobra_se_queda_para_la_proxima() {
        // Sin el máximo, quien recoge se lleva la cola entera: en un volcado eso es duplicar
        // megabytes en el búfer de quien pinta, y el presupuesto por fotograma no acota nada
        // porque el gasto ya se hizo antes de llegar a él.
        let (_pty, output) = Pty::spawn(
            "sh",
            &["-c", "printf 0123456789"],
            Size { rows: 24, cols: 80 },
        )
        .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});
        assert!(wait_for(|| !pty_output.is_alive()));

        let mut first = Vec::new();
        let pickup = pty_output.pick_up(&mut first, 4);
        assert_eq!(first, b"0123", "no se respetó el máximo");
        assert!(pickup.left_a_queue, "quedaban bytes y dijo que no");

        // Y lo que sobró sigue ahí, en su orden, sin haberse perdido ni repetido: es lo que hace
        // que un volcado se vea entero aunque tarde varios fotogramas.
        let mut rest = Vec::new();
        let pickup = pty_output.pick_up(&mut rest, usize::MAX);
        assert_eq!(rest, b"456789", "lo que sobró no salió entero ni en orden");
        assert!(!pickup.left_a_queue, "la cola quedó vacía y dijo que no");
    }

    /// Deja una cola cuyo contenido está **partido en dos tramos**: el anillo da la vuelta y sus
    /// dos rebanadas son las dos mitades de `content`, con `in_first` bytes en la de
    /// delante.
    ///
    /// Se monta a mano, y no llenando el puente por su canal, porque desde fuera no hay forma de
    /// forzarlo: la cola solo da la vuelta si le queda hueco libre detrás de la cabeza, y en cuanto
    /// no le queda crece y se recoloca de una pieza. Un test que lo intentara desde el canal
    /// pasaría casi siempre **sin haber probado nunca el caso**, que es peor que no tenerlo. Por
    /// eso las dos comprobaciones del final: si un día la biblioteca cambia de política y esto deja
    /// de dar la vuelta, el test falla aquí en vez de volverse verde para siempre.
    fn queue_split_in_two(content: &[u8], in_first: usize) -> VecDeque<u8> {
        let mut queue: VecDeque<u8> = VecDeque::with_capacity(content.len() * 2);
        let capacity = queue.capacity();

        // Se llena hasta el borde y se vacía todo menos el primer tramo: la cabeza queda pegada al
        // final del anillo, así que lo siguiente que entre tiene que dar la vuelta.
        let filler = capacity - in_first;
        queue.extend(std::iter::repeat_n(b'.', filler));
        queue.extend(&content[..in_first]);
        queue.drain(..filler);
        queue.extend(&content[in_first..]);

        assert_eq!(queue.capacity(), capacity, "la cola creció y se recolocó");
        assert!(!queue.as_slices().1.is_empty(), "la cola no dio la vuelta");
        queue
    }

    /// Un puente sin programa detrás, con la cola puesta a mano.
    ///
    /// El emisor se devuelve y hay que conservarlo vivo: soltarlo cierra el canal, y eso el puente
    /// lo lee como que el programa ha muerto.
    fn pty_output_with_queue(bytes: VecDeque<u8>) -> (std::sync::mpsc::Sender<Vec<u8>>, PtyOutput) {
        let (sender, output) = channel();
        let pty_output = PtyOutput::new(output, || {});
        let mut queue = pty_output
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Una sola llegada que cubre todo lo puesto a mano: sin ella, recoger no sabría desde
        // cuándo esperaba y devolvería que no hay nada que esperar, que es distinto de cero.
        queue
            .arrivals
            .push_back((bytes.len() as u64, Instant::now()));
        queue.bytes = bytes;
        drop(queue);
        (sender, pty_output)
    }

    #[test]
    fn recoger_junta_los_dos_tramos_cuando_la_cola_ha_dado_la_vuelta() {
        // La cola guarda sus bytes en dos trozos en cuanto da la vuelta. Quien copie solo el
        // primero se deja la mitad del volcado en silencio, y quien los junte al revés escribe en
        // la pantalla lo que el programa mandó después antes que lo que mandó primero.
        let (_sender, pty_output) = pty_output_with_queue(queue_split_in_two(b"0123456789", 6));

        let mut destination = Vec::new();
        let pickup = pty_output.pick_up(&mut destination, usize::MAX);

        assert_eq!(
            destination, b"0123456789",
            "los dos tramos no salieron en orden"
        );
        assert!(
            !pickup.left_a_queue,
            "se llevó la cola entera y dijo que quedaba algo"
        );
    }

    #[test]
    fn el_maximo_que_cae_dentro_del_primer_tramo_no_toca_el_segundo() {
        let (_sender, pty_output) = pty_output_with_queue(queue_split_in_two(b"0123456789", 6));

        let mut destination = Vec::new();
        assert!(
            pty_output.pick_up(&mut destination, 4).left_a_queue,
            "quedaban seis bytes"
        );
        assert_eq!(destination, b"0123", "no se respetó el máximo");

        // Y lo que quedó sigue estando a caballo de la vuelta, entero y en su orden.
        let mut rest = Vec::new();
        assert!(!pty_output.pick_up(&mut rest, usize::MAX).left_a_queue);
        assert_eq!(rest, b"456789");
    }

    #[test]
    fn el_maximo_que_cae_pasada_la_vuelta_se_lleva_de_los_dos_tramos() {
        let (_sender, pty_output) = pty_output_with_queue(queue_split_in_two(b"0123456789", 6));

        let mut destination = Vec::new();
        assert!(
            pty_output.pick_up(&mut destination, 8).left_a_queue,
            "quedaban dos bytes"
        );

        // Seis del primer tramo y dos del segundo: el corte cae dentro del segundo, que es donde
        // una cuenta mal hecha se sale de rango o se deja bytes por el camino.
        assert_eq!(destination, b"01234567");

        let mut rest = Vec::new();
        assert!(!pty_output.pick_up(&mut rest, usize::MAX).left_a_queue);
        assert_eq!(rest, b"89");
    }

    #[test]
    fn lo_recogido_dice_desde_cuando_esperaba() {
        // Es el sello del que cuelga el número que este hito publica como retraso visual. Sin él,
        // la espera —que en un volcado es más de un segundo— no se puede medir, y el resto de
        // tramos son microsegundos: el número diría que la aplicación va instantánea justo cuando
        // se la ve ir por detrás.
        let (_pty, output) = Pty::spawn("sh", &["-c", "printf hola"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});
        assert!(wait_for(|| !pty_output.is_alive()));

        std::thread::sleep(Duration::from_millis(50));
        let mut destination = Vec::new();
        let pickup = pty_output.pick_up(&mut destination, usize::MAX);

        let waiting_since = pickup
            .waiting_since
            .expect("se recogieron bytes y no dijo desde cuándo esperaban");
        assert!(
            waiting_since.elapsed() >= Duration::from_millis(50),
            "la espera salió más corta de lo que el test durmió"
        );
    }

    #[test]
    fn una_recogida_en_vacio_no_inventa_una_espera_de_cero() {
        // Un fotograma sin salida no es una espera instantánea: es que no hubo nada que esperar.
        // Devolver cero metería un cero en la serie y bajaría la media publicada por fotogramas en
        // los que no pasó nada.
        let (_pty, output) = Pty::spawn("sh", &["-c", "sleep 30"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        let mut destination = Vec::new();
        let pickup = pty_output.pick_up(&mut destination, usize::MAX);

        assert!(destination.is_empty());
        assert!(pickup.waiting_since.is_none());
    }

    #[test]
    fn las_llegadas_ya_servidas_no_se_acumulan() {
        // Durante un volcado entran miles de tandas. Si las marcas no se purgaran al servirlas, la
        // lista crecería sin parar y buscar en ella se haría más caro cada fotograma — dentro del
        // candado que el hilo del programa también necesita.
        let (sender, pty_output) = pty_output_with_queue(VecDeque::new());
        for _ in 0..50 {
            sender.send(b"abcd".to_vec()).expect("el puente sigue vivo");
        }

        let mut destination = Vec::new();
        assert!(wait_for(|| {
            pty_output.pick_up(&mut destination, usize::MAX);
            destination.len() == 200
        }));

        let queue = pty_output
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        assert!(
            queue.arrivals.is_empty(),
            "quedaron {} marcas de tandas ya servidas",
            queue.arrivals.len()
        );
    }

    #[test]
    fn el_destino_conserva_lo_que_ya_tenia() {
        // El fotograma reutiliza su búfer y puede llamar con restos de la pasada anterior todavía
        // sin procesar. Si recoger los pisara, se perderían bytes ya leídos del programa.
        let (_pty, output) = Pty::spawn("sh", &["-c", "printf dos"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        let mut destination = b"antes".to_vec();
        assert!(wait_for(|| {
            pty_output.pick_up(&mut destination, usize::MAX);
            destination.len() > b"antes".len()
        }));
        assert!(
            destination.starts_with(b"antes"),
            "se perdió lo que ya había"
        );
    }

    #[test]
    fn la_cola_del_puente_no_crece_sin_limite() {
        // Antes del arreglo, un programa que escribe sin parar y al que nadie recoge hacía
        // crecer esta cola sin freno: con `cat /dev/zero` se midieron cientos de MiB por
        // segundo en el diagnóstico. Aquí se deja el programa escribiendo y **no se llama a
        // `pick_up` en ningún momento** durante la espera: llamarlo aunque fuera una vez le
        // haría sitio al hilo y este seguiría recogiendo, con lo que se mediría el drenaje y no
        // el techo.
        let (pty, output) = Pty::spawn("cat", &["/dev/zero"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        // A los cientos de MiB/s medidos, medio segundo de margen basta de sobra para que la
        // cola alcance el techo si el freno funciona, y para que se dispare muy por encima de
        // la cota si no lo hace.
        std::thread::sleep(Duration::from_millis(500));

        // Se mira el tamaño de la cola con el candado, directamente, y no con `pick_up`:
        // `pick_up(destination, usize::MAX)` se llevaría la cola entera y le haría sitio al hilo,
        // así que solo serviría para medir una vez — y esta comprobación necesita ver la cola
        // mientras el hilo sigue bloqueado esperando, no después de haberla vaciado.
        let accumulated = pty_output
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .bytes
            .len();

        // Cota superior: por encima del techo del puente (1 MiB) hay margen que el freno de
        // aquí no controla directamente, y hay que contar con él para no perseguir un número
        // exacto. De más cerca a más lejos en la cadena: el trozo que este mismo hilo tiene «en
        // la mano» mientras espera, sin encolar todavía (hasta 8 KiB, el tamaño de una lectura
        // de `term-core`); el `sync_channel(8)` de la caja de abajo, con hasta ocho lecturas de
        // 8 KiB en tránsito (64 KiB); el trozo que el hilo lector de `term-core` tiene a su vez
        // en la mano intentando entregarlo (otros 8 KiB); y el búfer del núcleo del
        // pseudoterminal, del orden de 20 KiB según lo medido en la investigación de este fallo.
        // Sumado, algo por encima de 1,1 MiB. Se deja hasta 4 MiB para no depender de
        // temporización fina, y con eso de sobra: a los ritmos medidos en el diagnóstico, sin
        // freno los mismos 500 ms habrían acumulado varias decenas de MiB, no unos pocos.
        const UPPER_BOUND: usize = 4 * 1024 * 1024;

        assert!(
            accumulated >= QUEUE_CEILING,
            "la cola no llegó ni al techo ({QUEUE_CEILING} bytes) en 500 ms: solo {accumulated} bytes; \
             la prueba no está sometiendo el puente a presión de verdad"
        );
        assert!(
            accumulated <= UPPER_BOUND,
            "la cola creció muy por encima del techo y su margen: {accumulated} bytes \
             acumulados en 500 ms, cota {UPPER_BOUND} bytes — el freno que impide que esta \
             cola crezca sin límite no está actuando"
        );

        // Se cierra el pseudoterminal para que el programa deje de escribir, y se sigue recogiendo
        // hasta que el hilo lo note y termine: así la comprobación de la muerte, que es de lo que
        // vive el resto del módulo, se hace sobre un puente que ha salido de la espera por las
        // buenas y no por la parada de emergencia del test siguiente.
        drop(pty);
        let mut rest = Vec::new();
        assert!(wait_for(|| {
            pty_output.pick_up(&mut rest, usize::MAX);
            !pty_output.is_alive()
        }));
    }

    /// El hilo de este módulo tiene **dos** formas de quedarse quieto, y solo una la resuelve el
    /// canal: leyendo, se entera de que el programa ha muerto porque el canal se cierra; esperando
    /// sitio, no mira el canal en absoluto. Como el sitio solo lo hace quien recoge, un puente
    /// destruido con la cola en el techo dejaría su hilo girando para siempre — una fuga por cada
    /// división que se cierre en ese instante, invisible hoy solo porque el proceso muere entero
    /// detrás.
    ///
    /// Se observa con un testigo capturado por el despertador en vez de contando hilos: el
    /// despertador se destruye **con** el hilo, así que su cuenta de referencias bajando a uno
    /// demuestra que el hilo terminó de verdad. Contar hilos del proceso mediría de rebote los del
    /// resto de la suite.
    #[test]
    fn destruir_el_puente_con_la_cola_llena_termina_su_hilo() {
        let witness = Arc::new(());
        let thread_witness = Arc::clone(&witness);

        // El `Pty` se mantiene vivo a propósito durante toda la prueba: si se cerrara, el hilo
        // saldría por el camino de siempre y esto pasaría aunque la parada no existiera.
        let (_pty, output) = Pty::spawn("cat", &["/dev/zero"], Size { rows: 24, cols: 80 })
            .expect("no se pudo abrir el pseudoterminal");
        let pty_output = PtyOutput::new(output, move || {
            let _ = &thread_witness;
        });

        // Sin recoger ni una vez: la cola llega al techo y el hilo se queda esperando sitio.
        assert!(
            wait_for(|| {
                pty_output
                    .queue
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .bytes
                    .len()
                    >= QUEUE_CEILING
            }),
            "la cola no llegó al techo: la prueba no está dejando al hilo donde tiene que estar"
        );

        drop(pty_output);

        assert!(
            wait_for(|| Arc::strong_count(&witness) == 1),
            "el hilo sigue vivo después de destruir el puente: se quedó esperando un sitio que ya \
             nadie le va a hacer"
        );
    }
}
