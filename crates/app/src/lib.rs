//! Ventana, divisiones y pintado.
//!
//! La única caja atada al framework de interfaz. Si se cambia de framework, se reescribe esta y
//! las otras dos siguen tal cual.
//!
//! Es librería además de binario a propósito: el banco de barrido que mide el repintado tiene que
//! llamar al pintor de verdad, y si lo duplicara su número dejaría de ser el del producto.
//!
//! Aquí se juntan las piezas: el puente trae lo que el programa escribe, la rejilla lo interpreta,
//! el pintor lo dibuja y el teclado vuelve al pseudoterminal. Lo que falta por llegar son los
//! cronómetros.
//!
//! **El fotograma está partido en dos mitades que no hacen lo mismo.** `logic` corre siempre,
//! también con la ventana oculta, y es donde viven los efectos que no se pueden repetir: bombear,
//! interpretar y redimensionar. `ui` corre solo cuando hay pase de interfaz de verdad, y es donde
//! se lee el teclado y se pinta. El reparto no es estético: con la ventana oculta, la entrada que
//! `logic` ve es la del último pase mostrado, así que leer el teclado ahí reenviaría la última
//! tecla pulsada miles de veces por segundo durante un volcado.

pub mod medida;
pub mod pintor;
pub mod puente;
pub mod teclado;

pub mod tema;

use eframe::egui;
use medida::Medida;
use pintor::Metrica;
use puente::Puente;
use std::time::Instant;
use tema::Tema;
use term_core::{Pty, Screen, Size};

/// El nombre con el que la ventana se presenta al escritorio.
const NOMBRE: &str = "agent-terminal";

/// El programa que se lanza cuando el entorno no dice cuál.
///
/// Existe en todo sistema donde esto puede correr, que es lo que se le pide a un respaldo.
const SHELL_DE_RESPALDO: &str = "/bin/sh";

/// El cuerpo de la letra, en puntos.
const CUERPO: f32 = 14.0;

/// Cuánto se traga la rejilla como mucho en un fotograma.
///
/// **Se cuenta en bytes y no en tiempo, y eso es deliberado.** Con un plazo, el coste de
/// interpretar valdría por definición lo que se le pusiera y el número devolvería lo que uno mismo
/// metió; con una cantidad fija de bytes el coste de interpretar es constante por construcción,
/// todo lo demás del fotograma es el pintor, y si tragar esto empieza a costar milisegundos **eso
/// es la señal** que el hito busca.
///
/// El número sale de una medida, no de una intuición: la rejilla absorbe 64,5 MiB/s, así que esto
/// son unos 2 ms. Un volcado de 10 MiB tarda unos 80 fotogramas — algo más de un segundo— y no
/// pierde ni un byte por el camino.
const PRESUPUESTO: usize = 128 * 1024;

/// El tamaño con el que arranca la rejilla, antes de que exista una ventana que medir.
///
/// Vive lo que tarda el primer pase de interfaz: en cuanto hay un rectángulo de verdad, la rejilla
/// pasa a medirse desde él.
const REJILLA_INICIAL: Size = Size { rows: 24, cols: 80 };

/// Abre la ventana y no vuelve hasta que se cierra.
pub fn arrancar() -> eframe::Result {
    eframe::run_native(
        NOMBRE,
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(Ventana::nueva(&cc.egui_ctx)?))),
    )
}

/// La ventana raíz: una consola y todo lo que hace falta para sostenerla.
struct Ventana {
    /// El pseudoterminal y su cola van juntos a propósito. El hilo del puente vive mientras viva
    /// el pseudoterminal, no mientras viva el puente: separarlos dejaría un hilo y una cola sin
    /// dueño, creciendo.
    pty: Pty,
    puente: Puente,
    pantalla: Screen,
    tema: Tema,
    /// Lo que se recoge del puente en un fotograma, reutilizado entre ellos para no reservar
    /// memoria en cada uno. Nunca crece más allá del presupuesto: se pide con tope y se
    /// interpreta entero.
    recibido: Vec<u8>,
    /// Los bytes que las teclas de este pase mandan al programa. Reutilizado por lo mismo.
    pulsaciones: Vec<u8>,
    /// El tamaño que midió el último pase de interfaz.
    ///
    /// Se mide donde hay un rectángulo —`ui`— y se aplica donde viven los efectos —`logic`—, así
    /// que llega siempre con un fotograma de retraso. No se nota: entre dos fotogramas de un
    /// arrastre no da tiempo a ver la rejilla vieja.
    medido: Size,
    medida: Medida,
    /// Lo que la primera mitad del fotograma ya sabe de la vuelta —desde cuándo esperaba lo
    /// recogido, cuándo se recogió y lo que costó interpretarlo—, a la espera de que la pasada de
    /// interfaz añada lo que cuesta encolar el pintado.
    pendiente_de_medir: Option<(Option<Instant>, Instant, std::time::Duration)>,
    /// Cuándo empezó el fotograma en curso, que es donde arranca el cronómetro de la ida.
    empezo_el_fotograma: Instant,
    /// El último fotograma cuyo coste ya se anotó, para no contarlo dos veces si egui repite la
    /// pasada sobre la misma entrada.
    ultimo_fotograma_cerrado: u64,
}

impl Ventana {
    fn nueva(ctx: &egui::Context) -> term_core::Result<Self> {
        // El shell sale del entorno de quien lanza la aplicación, que es lo que hace un terminal:
        // sin esto se abre un shell sin prompt ni historial, y este hito existe para mirar lo que
        // sale en pantalla. Sigue sin ser una entrada ajena —es la sesión del propio usuario—,
        // pero el día en que el programa venga de configuración o de un `panel.json` de fuera,
        // esta línea pasa a ser superficie sensible y hay que revisarla como tal.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| SHELL_DE_RESPALDO.to_owned());
        // Si lo que dice el entorno ya no existe —un perfil borrado, un shell desinstalado—, se
        // cae al de respaldo en vez de no abrir. Quedarse sin ventana teniendo `/bin/sh` a mano, y
        // con un error que nadie va a entender, es el peor de los dos resultados.
        let (pty, salida) = match Pty::spawn(&shell, &[], REJILLA_INICIAL) {
            Ok(abierto) => abierto,
            Err(fallo) => {
                eprintln!("no se pudo lanzar «{shell}» ({fallo}); se abre {SHELL_DE_RESPALDO}");
                Pty::spawn(SHELL_DE_RESPALDO, &[], REJILLA_INICIAL)?
            }
        };

        // Lo único que el puente necesita de la ventana es cómo despertarla. Pedir el repintado no
        // entra en pánico, que es la condición que el puente pone: si lo hiciera, su hilo moriría
        // y publicaría una muerte que no ha ocurrido.
        let despertador = ctx.clone();
        let puente = Puente::nuevo(salida, move || despertador.request_repaint());

        Ok(Self {
            pty,
            puente,
            pantalla: Screen::new(REJILLA_INICIAL),
            tema: Tema::default(),
            recibido: Vec::new(),
            pulsaciones: Vec::new(),
            medido: REJILLA_INICIAL,
            medida: Medida::default(),
            pendiente_de_medir: None,
            empezo_el_fotograma: Instant::now(),
            ultimo_fotograma_cerrado: 0,
        })
    }

    /// Aplica el tamaño que midió el último pase de interfaz.
    ///
    /// **El orden entre las dos mitades no es indiferente: primero la rejilla y después el
    /// programa.** En cuanto el programa sabe el tamaño nuevo empieza a mandar bytes pensados para
    /// él, y si la rejilla siguiera siendo la vieja se interpretarían contra dimensiones
    /// equivocadas.
    fn redimensionar(&mut self) {
        if self.medido == self.pantalla.size() {
            return;
        }

        self.pantalla.resize(self.medido);
        if let Err(fallo) = self.pty.resize(self.medido) {
            // La rejilla ya cambió y el programa no se ha enterado: lo que se ve a partir de aquí
            // está pintado contra un tamaño que no es el suyo. No hay forma de arreglarlo desde
            // aquí, pero callarlo dejaría una pantalla torcida sin ninguna explicación.
            eprintln!("no se pudo avisar al programa del tamaño nuevo: {fallo}");
        }
    }

    /// Se lleva lo que el programa ha escrito, hasta el presupuesto, y lo mete en la rejilla.
    ///
    /// Devuelve si el presupuesto cortó el bombeo y quedó cola por tragar.
    fn bombear(&mut self) -> bool {
        self.recibido.clear();
        let recogida = self.puente.recoger(&mut self.recibido, PRESUPUESTO);
        let recogido_en = Instant::now();

        let antes = Instant::now();
        self.pantalla.advance(&self.recibido);
        let interpretado = antes.elapsed();

        // El encolado del pintado se mide en la pasada de interfaz, que viene después; aquí se
        // apunta lo que ya se sabe y allí se completa.
        self.pendiente_de_medir = Some((recogida.esperando_desde, recogido_en, interpretado));
        recogida.queda_cola
    }

    /// Manda al programa lo que se ha tecleado en este pase, y mide cuánto se tarda.
    ///
    /// El cronómetro arranca **al empezar el fotograma**, no al pulsar la tecla: el evento ya estaba
    /// esperando cuando el fotograma empezó y no trae sello propio, así que ese trecho no se puede
    /// ver desde aquí. El número lleva escrita esa frontera.
    fn teclear(&mut self, ctx: &egui::Context, empezo_el_fotograma: Instant) {
        self.pulsaciones.clear();
        // Los eventos que van al programa **se consumen**, no se leen y se dejan. Un fotograma
        // puede correr más de un pase sobre la misma entrada —egui lo hace cuando algo pide
        // recomponerse—, y entonces la misma tecla se escribiría dos veces al pseudoterminal. Los
        // que no se traducen se quedan donde estaban, para quien los quiera después.
        ctx.input_mut(|estado| {
            estado.events.retain(|evento| match teclado::bytes(evento) {
                Some(bytes) => {
                    self.pulsaciones.extend_from_slice(&bytes);
                    false
                }
                None => true,
            });
        });

        if self.pulsaciones.is_empty() {
            return;
        }
        if let Err(fallo) = self.pty.write(&self.pulsaciones) {
            // Lo que se acaba de teclear no ha llegado. Una tecla que se pierde en silencio es de
            // los fallos que más tardan en entenderse desde el otro lado de la pantalla.
            eprintln!("no se pudo enviar lo tecleado al programa: {fallo}");
            // Un envío que falló no es una ida rápida: no se anota, o el número mediría el fallo.
            return;
        }
        self.medida.anotar_ida(empezo_el_fotograma.elapsed());
    }
}

impl eframe::App for Ventana {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // **El cronómetro de la ida arranca aquí y no en la pasada de interfaz**, porque `logic`
        // corre antes dentro del mismo fotograma y lo que hace —tragar y meter en la rejilla— llega
        // a costar milisegundos durante un volcado. La tecla ya estaba esperando desde antes: si el
        // cronómetro empezara después de ese trabajo, el número saldría corto justamente en el
        // momento en que se nota.
        self.empezo_el_fotograma = Instant::now();

        // Lo que costó el fotograma anterior, entero. Llega con un fotograma de retraso porque es
        // lo que tarda en saberse: cuando se tesela y se dibuja, la pasada ya ha terminado. Se
        // cierra **una vez por fotograma y no por pase**: egui puede repetir la pasada sobre la
        // misma entrada, y entonces el segundo cierre emparejaría los tramos de este fotograma con
        // el total del anterior — una muestra inventada, y el total de verdad perdido.
        let pase = ctx.cumulative_pass_nr();
        if pase != self.ultimo_fotograma_cerrado
            && let Some(segundos) = frame.info().cpu_usage
        {
            self.ultimo_fotograma_cerrado = pase;
            self.medida
                .cerrar_fotograma(std::time::Duration::from_secs_f32(segundos));
        }

        // Primero se interpreta lo pendiente y después se cambia de tamaño: esos bytes los escribió
        // el programa para las dimensiones que todavía no sabe que han cambiado.
        let queda_cola = self.bombear();
        self.redimensionar();

        if queda_cola {
            // El presupuesto cortó el bombeo. Sin pedir otro fotograma, lo que falta se quedaría
            // esperando a que el programa escriba otra vez —o a que el usuario mueva el ratón— y
            // un volcado se vería a trozos, deteniéndose.
            ctx.request_repaint();
        }

        // El programa de dentro ha terminado —un `exit`, o el agente que se cierra— y una consola
        // sin nada corriendo no es una consola. **Se espera a que la cola esté vacía**: morir no
        // borra lo que escribió antes de hacerlo, y cerrar con cola pendiente se comería justo el
        // final, que es la parte que se estaba mirando.
        if !self.puente.sigue_vivo() && !queda_cola {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let metrica = Metrica::medir(ui.ctx(), egui::FontId::monospace(CUERPO));

        self.teclear(ui.ctx(), self.empezo_el_fotograma);

        let superficie = ui.max_rect();
        self.medido = metrica.celdas_en(superficie.size());

        let antes = Instant::now();
        pintor::pintar(
            ui.painter(),
            &self.pantalla.view(),
            &self.tema,
            &metrica,
            superficie.left_top(),
        );
        let encolado = antes.elapsed();

        if let Some((esperando_desde, recogido_en, interpretado)) = self.pendiente_de_medir.take() {
            self.medida
                .anotar_vuelta(esperando_desde, recogido_en, interpretado, encolado);
        }
    }

    /// Los números se publican al apagar, y **aquí y no al ver morir el programa**, por dos razones
    /// que se descubrieron mirando la salida: pedir el cierre no cierra en el acto, así que ese
    /// camino imprimía el informe una vez por cada fotograma que aún llegaba; y cerrar la ventana a
    /// mano —que es como se sale la mitad de las veces— no pasa por ahí, así que la sesión se
    /// perdía entera sin publicar nada.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.medida.publicar(self.pantalla.size());
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // El fondo se pinta aquí y no con un panel dentro de `ui` para que también cubra el
        // primer frame y los huecos de un redimensionado, donde todavía no ha corrido ningún
        // panel: sin esto asoma el gris por defecto de eframe. Y es el mismo fondo que el pintor
        // da por puesto: las celdas que lo llevan no se dibujan.
        self.tema.fondo().to_normalized_gamma_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Monta una ventana con un programa dentro, sin framework de interfaz de por medio.
    ///
    /// Las dos mitades del fotograma que aquí se prueban —bombear y redimensionar— no tocan la
    /// ventana del escritorio, así que no hace falta abrir ninguna: se arma el struct a mano con
    /// las mismas piezas que [`Ventana::nueva`].
    fn ventana_con(programa: &str) -> Ventana {
        let (pty, salida) = Pty::spawn("sh", &["-c", programa], REJILLA_INICIAL)
            .expect("no se pudo abrir el pseudoterminal");

        Ventana {
            pty,
            puente: Puente::nuevo(salida, || {}),
            pantalla: Screen::new(REJILLA_INICIAL),
            tema: Tema::default(),
            medida: Medida::default(),
            pendiente_de_medir: None,
            empezo_el_fotograma: Instant::now(),
            ultimo_fotograma_cerrado: 0,
            recibido: Vec::new(),
            pulsaciones: Vec::new(),
            medido: REJILLA_INICIAL,
        }
    }

    /// Espera a que el programa termine, que es el único instante en que se sabe **con certeza**
    /// que todo lo que escribió está ya en la cola: el hilo del puente publica la muerte después de
    /// haber entregado el último trozo. Sin esta espera, cuánto hay recogido depende de en qué
    /// trozos parta el pseudoterminal la salida, y el test daría un número distinto cada vez.
    fn esperar_a_que_muera(ventana: &Ventana) {
        let limite = Instant::now() + Duration::from_secs(5);
        while ventana.puente.sigue_vivo() {
            assert!(Instant::now() < limite, "el programa no terminó");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn lo_que_el_programa_escribe_acaba_en_la_rejilla_una_sola_vez() {
        let mut ventana = ventana_con("printf hola");
        esperar_a_que_muera(&ventana);

        assert!(!ventana.bombear(), "cuatro bytes no agotan el presupuesto");
        assert!(ventana.pantalla.to_text().contains("hola"));

        // Un fotograma sin nada nuevo no puede volver a interpretar lo del anterior. Si el búfer
        // no se vaciara al empezar, la pantalla mostraría dos veces algo que el programa escribió
        // una — y sería el usuario quien lo descubriera.
        ventana.bombear();
        assert_eq!(
            ventana.pantalla.to_text().matches("hola").count(),
            1,
            "lo ya interpretado volvió a entrar en la rejilla"
        );
    }

    #[test]
    fn un_volcado_mayor_que_el_presupuesto_tarda_varios_fotogramas_y_llega_entero() {
        // Tres presupuestos y pico: bastante para que ningún fotograma pueda con todo, y poco para
        // que el test siga tardando milisegundos.
        const VOLCADO: usize = 400_000;
        let mut ventana = ventana_con(&format!(
            "head -c {VOLCADO} /dev/zero | tr '\\0' a; printf '\\r\\nFIN'"
        ));
        esperar_a_que_muera(&ventana);

        let mut queda = ventana.bombear();

        // Este es el presupuesto haciendo su trabajo: sin él, el fotograma se traga los 400 KiB de
        // una vez y la ventana se queda quieta mientras tanto, que es justo lo que no puede pasar.
        assert!(queda, "un volcado de {VOLCADO} bytes cupo en un fotograma");
        assert!(
            ventana.recibido.len() <= PRESUPUESTO,
            "el búfer del fotograma creció por encima del presupuesto: {}",
            ventana.recibido.len()
        );
        // Y esta es la premisa de la que depende cerrar la ventana solo cuando además no queda
        // cola: el programa ya ha muerto y todavía tiene salida sin mostrar. Cerrar aquí se comería
        // el final, que es la parte que se estaba mirando.
        assert!(!ventana.puente.sigue_vivo());

        let mut fotogramas = 1;
        while queda {
            queda = ventana.bombear();
            fotogramas += 1;
            assert!(fotogramas < 100, "el bombeo no avanza");
        }

        assert!(
            fotogramas >= VOLCADO.div_ceil(PRESUPUESTO),
            "el volcado salió en {fotogramas} fotogramas, menos de los que el presupuesto obliga"
        );
        // Lo que el programa escribió al final está en la pantalla: el volcado se tragó entero, sin
        // perder por el camino ni el último trozo ni los cortes entre fotograma y fotograma.
        assert!(
            ventana.pantalla.to_text().contains("FIN"),
            "se perdió el final del volcado"
        );
    }

    #[test]
    fn la_rejilla_toma_el_tamano_que_midio_el_pase_anterior() {
        let mut ventana = ventana_con("sleep 30");
        let nuevo = Size { rows: 10, cols: 40 };

        ventana.medido = nuevo;
        ventana.redimensionar();

        assert_eq!(ventana.pantalla.size(), nuevo);
    }

    #[test]
    fn una_tecla_no_se_manda_dos_veces_aunque_el_fotograma_haga_dos_pases() {
        // egui puede correr la pasada de interfaz dos veces sobre la misma entrada. Los eventos que
        // van al programa se consumen por eso: leerlos y dejarlos escribiría la misma tecla dos
        // veces al pseudoterminal, y el programa de dentro no tiene forma de saber que sobra una.
        let mut ventana = ventana_con("sleep 30");
        let ctx = egui::Context::default();
        ctx.input_mut(|entrada| {
            entrada.events.push(egui::Event::Text("a".to_owned()));
            entrada.events.push(egui::Event::WindowFocused(true));
        });

        ventana.teclear(&ctx, Instant::now());
        assert_eq!(ventana.pulsaciones, b"a", "no se envió lo tecleado");

        ventana.teclear(&ctx, Instant::now());
        assert!(
            ventana.pulsaciones.is_empty(),
            "la misma tecla se envió en el segundo pase"
        );

        // Lo que no se traduce se queda donde estaba: el bucle se lleva sus teclas, no la entrada
        // entera, y hay quien mira esos eventos después.
        ctx.input(|entrada| {
            assert_eq!(
                entrada.events,
                [egui::Event::WindowFocused(true)],
                "el bucle se llevó eventos que no eran suyos"
            );
        });
    }
}
