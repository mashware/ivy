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
use medida::{Medida, RestoDelFotograma};
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

/// Cuántos puntos de ancho se lleva la línea que separa las dos consolas.
///
/// Dos y no uno: con uno la línea se ve, pero el hito siguiente le añade el arrastre y una zona de
/// agarre de un punto es imposible de atrapar con el ratón.
const GROSOR_DEL_DIVISOR: f32 = 2.0;

/// Lo que el fotograma necesita saber después de bombear todas las consolas.
struct Bombeo {
    /// A alguna le cortó el presupuesto y le queda salida por tragar, así que hace falta otro
    /// fotograma.
    alguna_dejo_cola: bool,
    /// Todas han muerto y ninguna tiene cola: **solo entonces** se cierra la ventana.
    todas_terminadas: bool,
}

/// Los rectángulos que ocupan las dos consolas y la línea que las separa.
struct Reparto {
    izquierda: egui::Rect,
    divisor: egui::Rect,
    derecha: egui::Rect,
}

/// Reparte el ancho del área entre las dos consolas, mitad y mitad.
///
/// **El grosor del divisor se descuenta primero y se reparte lo que queda.** Si se partiera el área
/// entera y luego se restara la línea, la consola de la que se restase saldría más estrecha que la otra
/// sin que nadie hubiera pedido eso.
///
/// **Reparte a la mitad y no acepta una fracción, a propósito.** El reparto variable llega con el
/// divisor arrastrable, y solo entonces hace falta un suelo por consola: una fracción que solo puede
/// valer un medio no puede dejar a nadie sin sitio, así que el recorte que lo evitaría sería hoy una
/// rama que ninguna llamada puede alcanzar.
///
/// El área no puede quedar más estrecha que el divisor: `max` con cero mantiene los tres rectángulos
/// bien formados incluso con una ventana de un punto de ancho, donde lo que se ve ya no importa pero un
/// rectángulo invertido sí sería un error.
fn repartir(area: egui::Rect) -> Reparto {
    let util = (area.width() - GROSOR_DEL_DIVISOR).max(0.0);
    let corte = area.left() + util / 2.0;
    let tras_el_divisor = corte + GROSOR_DEL_DIVISOR;

    Reparto {
        izquierda: egui::Rect::from_min_max(area.left_top(), egui::pos2(corte, area.bottom())),
        divisor: egui::Rect::from_min_max(
            egui::pos2(corte, area.top()),
            egui::pos2(tras_el_divisor, area.bottom()),
        ),
        derecha: egui::Rect::from_min_max(
            egui::pos2(tras_el_divisor, area.top()),
            egui::pos2(area.right().max(tras_el_divisor), area.bottom()),
        ),
    }
}

/// Abre la ventana y no vuelve hasta que se cierra.
pub fn arrancar() -> eframe::Result {
    eframe::run_native(
        NOMBRE,
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(Ventana::nueva(&cc.egui_ctx)?))),
    )
}

/// Una consola y todo lo que hace falta para sostenerla.
///
/// Cada campo de aquí era un campo de [`Ventana`] cuando solo había una consola. Agruparlos es lo que
/// permite instanciar el conjunto entero más de una vez sin escribir cada operación dos veces.
struct Division {
    /// El pseudoterminal y su cola van juntos a propósito. El hilo del puente vive mientras viva
    /// el pseudoterminal, no mientras viva el puente: separarlos dejaría un hilo y una cola sin
    /// dueño, creciendo.
    pty: Pty,
    puente: Puente,
    pantalla: Screen,
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
    /// Los cronómetros de esta consola, y no de la ventana: así sus tramos siguen comparando con las
    /// tablas de D3 haya una división o cuatro.
    medida: Medida,
    /// Lo que la primera mitad del fotograma ya sabe de la vuelta —desde cuándo esperaba lo
    /// recogido, cuándo se recogió y lo que costó interpretarlo—, a la espera de que la pasada de
    /// interfaz añada lo que cuesta encolar el pintado.
    pendiente_de_medir: Option<(Option<Instant>, Instant, std::time::Duration)>,
}

impl Division {
    /// Lanza el programa de esta división y la deja lista para el primer fotograma.
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
            recibido: Vec::new(),
            pulsaciones: Vec::new(),
            medido: REJILLA_INICIAL,
            medida: Medida::default(),
            pendiente_de_medir: None,
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

    /// Si el programa terminó y no queda nada suyo por mostrar.
    ///
    /// **Se espera a que la cola esté vacía**: morir no borra lo que el programa escribió antes de
    /// hacerlo, y dar la división por terminada con cola pendiente se comería justo el final, que es
    /// la parte que se estaba mirando.
    fn terminada(&self, queda_cola: bool) -> bool {
        !self.puente.sigue_vivo() && !queda_cola
    }
}

/// La ventana raíz: las consolas, y lo que es de la ventana y no de ninguna.
struct Ventana {
    /// Las dos consolas, de izquierda a derecha.
    ///
    /// **Un array de dos y no una lista**: el número está fijado por el hito —el árbol de divisiones
    /// arbitrarias es Fase 2— y decirlo en el tipo es lo que permite indexar sin escribir ramas para un
    /// caso vacío que no existe. Una lista obligaría a preguntar en cada acceso si hay alguien ahí.
    divisiones: [Division; 2],
    /// A cuál va el teclado. Arranca en `0` —la izquierda— porque teclear antes del primer clic ocurre
    /// siempre al abrir la ventana, y con un foco nulo esas teclas se perderían.
    ///
    /// **No es el foco de egui.** El del framework es foco de teclado por widget, para tabular entre
    /// controles; esto es geográfico y lo decide la aplicación. Confundirlos deja el teclado en manos
    /// de quien no lo enruta.
    foco: usize,
    tema: Tema,
    /// El coste de la pasada que nadie cronometró; ver [`RestoDelFotograma`] por qué es de la ventana.
    resto_del_fotograma: RestoDelFotograma,
    /// Cuándo empezó el fotograma en curso, que es donde arranca el cronómetro de la ida.
    empezo_el_fotograma: Instant,
    /// El último fotograma cuyo coste ya se anotó, para no contarlo dos veces si egui repite la
    /// pasada sobre la misma entrada.
    ultimo_fotograma_cerrado: u64,
}

impl Ventana {
    fn nueva(ctx: &egui::Context) -> term_core::Result<Self> {
        Ok(Self {
            divisiones: [Division::nueva(ctx)?, Division::nueva(ctx)?],
            foco: 0,
            tema: Tema::default(),
            resto_del_fotograma: RestoDelFotograma::default(),
            empezo_el_fotograma: Instant::now(),
            ultimo_fotograma_cerrado: 0,
        })
    }

    /// Cierra el fotograma anterior: cada consola cierra sus tramos y la ventana se queda con lo que
    /// ninguna cronometró.
    fn cerrar_fotograma(&mut self, coste_cpu: std::time::Duration) {
        let cronometrado = self
            .divisiones
            .iter_mut()
            .filter_map(|division| division.medida.cerrar_fotograma(coste_cpu))
            .reduce(|una, otra| una + otra);

        // Sin tramos de nadie no hay resto que anotar: el fotograma entero se contaría como resto y la
        // serie acabaría midiendo sobre todo los fotogramas en los que no pasaba nada.
        if let Some(cronometrado) = cronometrado {
            self.resto_del_fotograma.anotar(coste_cpu, cronometrado);
        }
    }

    /// Bombea las dos consolas y las redimensiona, y resume lo que el fotograma tiene que decidir con
    /// eso.
    ///
    /// Vive aparte del bucle del framework para que las dos decisiones que salen de aquí —pedir otro
    /// fotograma y cerrar la ventana— se puedan comprobar sin abrir ninguna. Es la única forma de
    /// probar que **una consola muerta no cierra la ventana**, que es lo que este hito promete y lo que
    /// un fallo aquí se llevaría por delante sin que ningún test se enterara.
    fn bombear_todas(&mut self) -> Bombeo {
        let mut bombeo = Bombeo {
            alguna_dejo_cola: false,
            todas_terminadas: true,
        };

        for division in &mut self.divisiones {
            // Primero se interpreta lo pendiente y después se cambia de tamaño: esos bytes los
            // escribió el programa para las dimensiones que todavía no sabe que han cambiado.
            let queda_cola = division.bombear();
            division.redimensionar();

            bombeo.alguna_dejo_cola |= queda_cola;
            bombeo.todas_terminadas &= division.terminada(queda_cola);
        }

        bombeo
    }

    /// Da el foco a la consola sobre la que se ha hecho clic.
    ///
    /// Vive aparte del pintado para poder comprobarse con un `Ui` de prueba, sin abrir ventana.
    fn atender_clics(&mut self, ui: &egui::Ui, rectangulos: &[egui::Rect]) {
        for (cual, rect) in rectangulos.iter().enumerate() {
            // El clic se detecta con `interact` y no mirando la posición del puntero a pelo, porque así
            // respeta capas y orden de pintado. Y con `Sense::CLICK` en vez de `Sense::click()`: el
            // segundo incluye `FOCUSABLE`, o sea que el widget **pediría el foco de teclado de egui** y
            // competiría con el enrutado que hace la aplicación — que es geográfico y no por widget.
            let id = ui.id().with(("division", cual));
            if ui.interact(*rect, id, egui::Sense::CLICK).clicked() {
                self.foco = cual;
            }
        }
    }

    /// Manda lo que se ha tecleado en este pase a **una sola** consola, la que tiene el foco.
    ///
    /// Vive aparte del pintado por lo mismo que [`Self::atender_clics`]: mandarlo a las dos escribiría
    /// la misma tecla en dos programas, que es exactamente lo que «sin que se pisen» prohíbe, y dentro
    /// de la pasada de interfaz esa decisión no se puede comprobar sin abrir una ventana.
    fn teclear_la_del_foco(&mut self, ctx: &egui::Context) {
        let empezo = self.empezo_el_fotograma;
        self.divisiones[self.foco].teclear(ctx, empezo);
    }

    /// Pinta una consola recortada a su rectángulo, y devuelve lo que costó encolar el pintado.
    ///
    /// **El recorte no es cosmético.** El pintor no recorta los glifos a propósito —para que un carácter
    /// más ancho que su celda se vea salir— y sin recortar por consola ese glifo del borde derecho
    /// aparecería dentro de la consola vecina, pintado por un programa que no es el suyo.
    fn pintar_division(
        ui: &egui::Ui,
        division: &mut Division,
        rect: egui::Rect,
        tema: &Tema,
        metrica: &Metrica,
    ) {
        division.medido = metrica.celdas_en(rect.size());

        let antes = Instant::now();
        pintor::pintar(
            &ui.painter_at(rect),
            &division.pantalla.view(),
            tema,
            metrica,
            rect.left_top(),
        );
        let encolado = antes.elapsed();

        // El cierre de la vuelta va aquí y no en el bucle de arriba porque **el encolado hay que
        // declararlo siempre**, también cuando a esta consola no le llegó salida: se pintó igual, y su
        // coste tiene que salir del «resto del fotograma» de la ventana.
        if let Some((esperando_desde, recogido_en, interpretado)) =
            division.pendiente_de_medir.take()
        {
            division
                .medida
                .anotar_vuelta(esperando_desde, recogido_en, interpretado, encolado);
        }
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
            self.cerrar_fotograma(std::time::Duration::from_secs_f32(segundos));
        }

        let Bombeo {
            alguna_dejo_cola,
            todas_terminadas,
        } = self.bombear_todas();

        if alguna_dejo_cola {
            // El presupuesto cortó el bombeo. Sin pedir otro fotograma, lo que falta se quedaría
            // esperando a que el programa escriba otra vez —o a que el usuario mueva el ratón— y
            // un volcado se vería a trozos, deteniéndose. **Basta con que una lo pida**: el fotograma
            // es de la ventana, así que el repintado que pide una sirve también a la otra.
            ctx.request_repaint();
        }

        // **La ventana se cierra solo cuando han muerto todas.** Con una consola trabajando y la otra
        // acabada, cerrar se llevaría por delante el trabajo de la que sigue viva — y la que murió se
        // queda con su último contenido en pantalla, que es lo que uno quiere leer justo después de que
        // un programa termine.
        if todas_terminadas {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let metrica = Metrica::medir(ui.ctx(), egui::FontId::monospace(CUERPO));

        // El teclado va antes de todo lo demás **y ese orden importa para el número, no para el
        // comportamiento**: el cronómetro de la ida se cierra dentro de `teclear`, así que cualquier
        // trabajo que se cuele por delante —aunque sea leer un rectángulo— se le suma.
        self.teclear_la_del_foco(ui.ctx());

        let reparto = repartir(ui.max_rect());
        let rectangulos = [reparto.izquierda, reparto.derecha];

        // El divisor se pinta antes que las consolas: si el glifo del borde de una se saliera, el
        // recorte ya lo corta, y así la línea nunca queda por encima de una letra.
        ui.painter()
            .rect_filled(reparto.divisor, 0.0, self.tema.divisor());

        self.atender_clics(ui, &rectangulos);

        for (division, rect) in self.divisiones.iter_mut().zip(rectangulos) {
            Self::pintar_division(ui, division, rect, &self.tema, &metrica);
        }

        // El realce va al final para que quede por encima del contenido: un borde tapado a medias por
        // un glifo del borde no dice nada. `StrokeKind::Inside` lo mantiene dentro del rectángulo, así
        // que no invade la consola vecina ni el divisor.
        ui.painter().rect_stroke(
            rectangulos[self.foco],
            0.0,
            egui::Stroke::new(1.0, self.tema.foco()),
            egui::StrokeKind::Inside,
        );
    }

    /// Los números se publican al apagar, y **aquí y no al ver morir el programa**, por dos razones
    /// que se descubrieron mirando la salida: pedir el cierre no cierra en el acto, así que ese
    /// camino imprimía el informe una vez por cada fotograma que aún llegaba; y cerrar la ventana a
    /// mano —que es como se sale la mitad de las veces— no pasa por ahí, así que la sesión se
    /// perdía entera sin publicar nada.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self
            .divisiones
            .iter()
            .any(|division| division.medida.hay_algo())
        {
            return;
        }

        medida::cabecera(self.divisiones.each_ref().map(|d| d.pantalla.size()));
        println!("reparto:      mitad y mitad, con {GROSOR_DEL_DIVISOR} puntos de divisor");
        println!(
            "foco al salir: consola {} ({})",
            self.foco,
            medida::NOMBRE_DE_CONSOLA[self.foco]
        );

        for (cual, division) in self.divisiones.iter().enumerate() {
            println!(
                "\n── consola {cual} ({}) ──",
                medida::NOMBRE_DE_CONSOLA[cual]
            );
            division.medida.publicar_tramos();
        }

        self.resto_del_fotograma.publicar();
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

    /// Monta una división con un programa dentro, sin framework de interfaz de por medio.
    ///
    /// Las dos mitades del fotograma que aquí se prueban —bombear y redimensionar— no tocan la
    /// ventana del escritorio, así que no hace falta abrir ninguna: se arma el struct a mano con
    /// las mismas piezas que [`Division::nueva`].
    fn division_con(programa: &str) -> Division {
        let (pty, salida) = Pty::spawn("sh", &["-c", programa], REJILLA_INICIAL)
            .expect("no se pudo abrir el pseudoterminal");

        Division {
            pty,
            puente: Puente::nuevo(salida, || {}),
            pantalla: Screen::new(REJILLA_INICIAL),
            medida: Medida::default(),
            pendiente_de_medir: None,
            recibido: Vec::new(),
            pulsaciones: Vec::new(),
            medido: REJILLA_INICIAL,
        }
    }

    /// Monta una ventana con dos consolas, cada una con su programa, sin abrir ninguna ventana de
    /// verdad.
    ///
    /// Lo que aquí se prueba —bombear, decidir el cierre, enrutar el teclado— no toca el escritorio,
    /// así que se arma el struct a mano con las mismas piezas que [`Ventana::nueva`].
    fn ventana_con(izquierda: &str, derecha: &str) -> Ventana {
        Ventana {
            divisiones: [division_con(izquierda), division_con(derecha)],
            foco: 0,
            tema: Tema::default(),
            resto_del_fotograma: RestoDelFotograma::default(),
            empezo_el_fotograma: Instant::now(),
            ultimo_fotograma_cerrado: 0,
        }
    }

    /// Espera a que el programa termine, que es el único instante en que se sabe **con certeza**
    /// que todo lo que escribió está ya en la cola: el hilo del puente publica la muerte después de
    /// haber entregado el último trozo. Sin esta espera, cuánto hay recogido depende de en qué
    /// trozos parta el pseudoterminal la salida, y el test daría un número distinto cada vez.
    fn esperar_a_que_muera(division: &Division) {
        let limite = Instant::now() + Duration::from_secs(5);
        while division.puente.sigue_vivo() {
            assert!(Instant::now() < limite, "el programa no terminó");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn lo_que_el_programa_escribe_acaba_en_la_rejilla_una_sola_vez() {
        let mut division = division_con("printf hola");
        esperar_a_que_muera(&division);

        assert!(!division.bombear(), "cuatro bytes no agotan el presupuesto");
        assert!(division.pantalla.to_text().contains("hola"));

        // Un fotograma sin nada nuevo no puede volver a interpretar lo del anterior. Si el búfer
        // no se vaciara al empezar, la pantalla mostraría dos veces algo que el programa escribió
        // una — y sería el usuario quien lo descubriera.
        division.bombear();
        assert_eq!(
            division.pantalla.to_text().matches("hola").count(),
            1,
            "lo ya interpretado volvió a entrar en la rejilla"
        );
    }

    #[test]
    fn un_volcado_mayor_que_el_presupuesto_tarda_varios_fotogramas_y_llega_entero() {
        // Tres presupuestos y pico: bastante para que ningún fotograma pueda con todo, y poco para
        // que el test siga tardando milisegundos.
        const VOLCADO: usize = 400_000;
        let mut division = division_con(&format!(
            "head -c {VOLCADO} /dev/zero | tr '\\0' a; printf '\\r\\nFIN'"
        ));
        esperar_a_que_muera(&division);

        let mut queda = division.bombear();

        // Este es el presupuesto haciendo su trabajo: sin él, el fotograma se traga los 400 KiB de
        // una vez y la ventana se queda quieta mientras tanto, que es justo lo que no puede pasar.
        assert!(queda, "un volcado de {VOLCADO} bytes cupo en un fotograma");
        assert!(
            division.recibido.len() <= PRESUPUESTO,
            "el búfer del fotograma creció por encima del presupuesto: {}",
            division.recibido.len()
        );
        // Y esta es la premisa de la que depende cerrar la ventana solo cuando además no queda
        // cola: el programa ya ha muerto y todavía tiene salida sin mostrar. Cerrar aquí se comería
        // el final, que es la parte que se estaba mirando.
        assert!(!division.puente.sigue_vivo());

        let mut fotogramas = 1;
        while queda {
            queda = division.bombear();
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
            division.pantalla.to_text().contains("FIN"),
            "se perdió el final del volcado"
        );
    }

    #[test]
    fn la_rejilla_toma_el_tamano_que_midio_el_pase_anterior() {
        let mut division = division_con("sleep 30");
        let nuevo = Size { rows: 10, cols: 40 };

        division.medido = nuevo;
        division.redimensionar();

        assert_eq!(division.pantalla.size(), nuevo);
    }

    #[test]
    fn una_tecla_no_se_manda_dos_veces_aunque_el_fotograma_haga_dos_pases() {
        // egui puede correr la pasada de interfaz dos veces sobre la misma entrada. Los eventos que
        // van al programa se consumen por eso: leerlos y dejarlos escribiría la misma tecla dos
        // veces al pseudoterminal, y el programa de dentro no tiene forma de saber que sobra una.
        let mut division = division_con("sleep 30");
        let ctx = egui::Context::default();
        ctx.input_mut(|entrada| {
            entrada.events.push(egui::Event::Text("a".to_owned()));
            entrada.events.push(egui::Event::WindowFocused(true));
        });

        division.teclear(&ctx, Instant::now());
        assert_eq!(division.pulsaciones, b"a", "no se envió lo tecleado");

        division.teclear(&ctx, Instant::now());
        assert!(
            division.pulsaciones.is_empty(),
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

    #[test]
    fn la_ventana_abre_dos_consolas_con_su_programa_vivo_cada_una() {
        // AC1. Dos programas distintos a la vez, y cada rejilla con lo suyo: es la comprobación de que
        // no comparten ni pseudoterminal ni pantalla, que es lo que «sin que se pisen» significa.
        let mut ventana = ventana_con("printf izquierda", "printf derecha");
        for division in &ventana.divisiones {
            esperar_a_que_muera(division);
        }
        ventana.bombear_todas();

        let [izquierda, derecha] = &ventana.divisiones;
        assert!(izquierda.pantalla.to_text().contains("izquierda"));
        assert!(derecha.pantalla.to_text().contains("derecha"));
        assert!(
            !izquierda.pantalla.to_text().contains("derecha"),
            "lo que escribió una consola apareció en la otra"
        );
    }

    #[test]
    fn el_foco_arranca_en_la_izquierda_en_una_ventana_recien_abierta() {
        // AC4, contra el constructor de verdad. El test de arriba lo comprueba sobre una ventana armada
        // a mano, que se fija el foco ella misma: seguiría verde si `Ventana::nueva` arrancara apuntando
        // a la derecha, o a un índice que no existe. Teclear antes del primer clic ocurre siempre al
        // abrir, y esas teclas no pueden perderse.
        let ventana = Ventana::nueva(&egui::Context::default())
            .expect("no se pudo abrir la ventana con sus dos consolas");

        assert_eq!(
            ventana.foco, 0,
            "el foco no arranca en la consola izquierda"
        );
    }

    #[test]
    fn el_teclado_lo_enruta_la_ventana_a_la_del_foco_y_no_a_la_primera() {
        // AC2 sobre el enrutado que hace la aplicación, no sobre el que elige el test. El de arriba
        // indexa él mismo la consola destino, así que seguiría verde con el fallo que de verdad importa:
        // escribir en las dos, o en la izquierda fija. Con el foco en la derecha, los dos salen en rojo.
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.foco = 1;

        let ctx = egui::Context::default();
        ctx.input_mut(|entrada| entrada.events.push(egui::Event::Text("a".to_owned())));
        ventana.teclear_la_del_foco(&ctx);

        assert_eq!(
            ventana.divisiones[1].pulsaciones, b"a",
            "la consola con el foco no recibió lo tecleado"
        );
        assert!(
            ventana.divisiones[0].pulsaciones.is_empty(),
            "la tecla acabó también en la consola sin foco"
        );
    }

    #[test]
    fn que_muera_una_consola_no_cierra_la_ventana() {
        // AC8, y es el criterio que da sentido al hito: con dos agentes trabajando, que uno termine no
        // puede llevarse por delante la ventana del otro. La condición ingenua —«si una murió, cierra»—
        // es justo la que este test pone en rojo.
        let mut ventana = ventana_con("printf ya", "sleep 30");
        esperar_a_que_muera(&ventana.divisiones[0]);

        let bombeo = ventana.bombear_todas();

        assert!(
            !ventana.divisiones[0].puente.sigue_vivo(),
            "la primera consola debería haber muerto ya"
        );
        assert!(
            !bombeo.todas_terminadas,
            "una consola muerta y otra viva pidieron cerrar la ventana"
        );
    }

    #[test]
    fn la_ventana_se_cierra_cuando_han_muerto_las_dos_y_no_queda_cola() {
        // AC9. Y el «sin cola» no es un adorno: morir no borra lo que el programa escribió antes de
        // hacerlo, y cerrar con salida pendiente se comería el final, que es la parte que se mira.
        let mut ventana = ventana_con("printf una", "printf otra");
        for division in &ventana.divisiones {
            esperar_a_que_muera(division);
        }

        let bombeo = ventana.bombear_todas();

        assert!(
            bombeo.todas_terminadas,
            "las dos murieron y la ventana no se cerró"
        );
        assert!(ventana.divisiones[0].pantalla.to_text().contains("una"));
        assert!(ventana.divisiones[1].pantalla.to_text().contains("otra"));
    }

    #[test]
    fn un_volcado_en_una_consola_no_frena_ni_ensucia_a_la_otra() {
        // AC10. El freno del terminal es **por consola**: con una volcando, la otra sigue entregando lo
        // suyo en el mismo fotograma. Si el presupuesto fuera común, la parada se quedaría esperando
        // detrás del volcado y la ventana daría la sensación de estar colgada.
        const VOLCADO: usize = 400_000;
        let mut ventana = ventana_con(
            &format!("head -c {VOLCADO} /dev/zero | tr '\\0' a"),
            "printf tranquila",
        );
        for division in &ventana.divisiones {
            esperar_a_que_muera(division);
        }

        let bombeo = ventana.bombear_todas();

        assert!(
            bombeo.alguna_dejo_cola,
            "el volcado cupo entero en un fotograma"
        );
        // La de al lado entregó lo suyo en ese mismo fotograma, sin esperar a que el volcado acabe.
        assert!(
            ventana.divisiones[1]
                .pantalla
                .to_text()
                .contains("tranquila"),
            "la consola tranquila se quedó esperando detrás del volcado"
        );
        // Y lo retenido sigue acotado por consola: ninguna se pasa del presupuesto.
        for (cual, division) in ventana.divisiones.iter().enumerate() {
            assert!(
                division.recibido.len() <= PRESUPUESTO,
                "la consola {cual} se pasó del presupuesto: {}",
                division.recibido.len()
            );
        }
    }

    #[test]
    fn un_clic_en_la_consola_derecha_le_da_el_foco() {
        // AC3, y es el criterio que hace usable el hito: sin poder elegir dónde escribe uno, dos
        // consolas son una curiosidad. Se prueba con un `Ui` de verdad porque lo que hay que comprobar
        // es la traducción de un clic del escritorio a nuestro foco, y eso solo lo sabe egui.
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        let izquierda = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 300.0));
        let derecha = egui::Rect::from_min_size(egui::pos2(202.0, 0.0), egui::vec2(200.0, 300.0));
        let dentro_de_la_derecha = egui::pos2(300.0, 150.0);

        let ctx = egui::Context::default();
        // Dos pasadas: en la primera se registran las regiones, y en la segunda llega el clic. egui
        // resuelve la interacción contra lo que había en la pasada anterior, así que un clic en la
        // primera no encontraría a nadie a quien dárselo.
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ventana.atender_clics(ui, &[izquierda, derecha]);
        })
        .drop_without_applying_deltas();

        let entrada = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(dentro_de_la_derecha),
                egui::Event::PointerButton {
                    pos: dentro_de_la_derecha,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos: dentro_de_la_derecha,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| {
            ventana.atender_clics(ui, &[izquierda, derecha]);
        })
        .drop_without_applying_deltas();

        assert_eq!(
            ventana.foco, 1,
            "el clic en la consola derecha no le dio el foco"
        );
    }

    #[test]
    fn el_reparto_conserva_la_proporcion_al_cambiar_el_ancho_de_la_ventana() {
        // AC6. Es la razón por la que el área se reparte a mano en vez de con los paneles
        // redimensionables de egui: aquellos trabajan en puntos, así que al ensanchar la ventana el
        // panel mantendría su ancho y toda la diferencia caería en el otro lado.
        let estrecha = repartir(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(402.0, 300.0),
        ));
        let ancha = repartir(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(802.0, 300.0),
        ));

        assert_eq!(estrecha.izquierda.width(), 200.0);
        assert_eq!(estrecha.derecha.width(), 200.0);
        assert_eq!(ancha.izquierda.width(), 400.0);
        assert_eq!(ancha.derecha.width(), 400.0);
    }

    #[test]
    fn el_divisor_se_descuenta_antes_de_repartir_y_no_deja_huecos() {
        // Las tres piezas tienen que cubrir el área exactamente: si el divisor se restara después de
        // repartir, una consola perdería ancho sin que nadie hubiera movido nada.
        let area = egui::Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(500.0, 300.0));
        let reparto = repartir(area);

        assert_eq!(reparto.divisor.width(), GROSOR_DEL_DIVISOR);
        assert_eq!(reparto.izquierda.right(), reparto.divisor.left());
        assert_eq!(reparto.divisor.right(), reparto.derecha.left());
        assert_eq!(reparto.izquierda.left(), area.left());
        assert_eq!(reparto.derecha.right(), area.right());
    }

    #[test]
    fn una_ventana_mas_estrecha_que_el_divisor_no_produce_rectangulos_invertidos() {
        // El borde de verdad de esta función. Nada de lo que se vea con dos puntos de ancho importa,
        // pero un rectángulo con el mínimo mayor que el máximo sí: es un valor imposible viajando por
        // el código, y aquí acabaría llegando al pintor y a `celdas_en`. No hay gestor de ventanas que
        // lo produzca a mano, pero eframe no fija ancho mínimo, así que no depende de nosotros.
        for ancho in [0.0_f32, 1.0, 2.0, 3.0, 20.0] {
            let reparto = repartir(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(ancho, 300.0),
            ));
            for (nombre, rect) in [
                ("izquierda", reparto.izquierda),
                ("divisor", reparto.divisor),
                ("derecha", reparto.derecha),
            ] {
                assert!(
                    rect.width() >= 0.0,
                    "con {ancho} puntos de ancho, el rectángulo de la {nombre} salió invertido"
                );
            }
        }
    }
}
