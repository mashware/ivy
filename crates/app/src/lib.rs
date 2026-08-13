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
pub mod panel;
pub mod pintor;
pub mod puente;
pub mod teclado;

pub mod tema;
pub mod vigilante;

use eframe::egui;
use medida::{Medida, RestoDelFotograma};
use panel_contract::PanelDir;
use pintor::Metrica;
use puente::Puente;
use std::time::{Instant, SystemTime};
use tema::Tema;
use term_core::{Pty, Screen, Size};
use vigilante::{Bandera, Vigilante};

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

/// Hasta dónde se le deja crecer al vector de lo tecleado antes de devolverle la memoria.
///
/// Tecleando nunca se acerca —son unos pocos bytes por fotograma—, así que esto solo actúa después de
/// un pegado. El valor no es crítico: basta con que sea holgado para el uso normal y pequeño frente a
/// un portapapeles de verdad.
const HOLGURA_DE_PULSACIONES: usize = 8 * 1024;

/// El tamaño con el que arranca la rejilla, antes de que exista una ventana que medir.
///
/// Vive lo que tarda el primer pase de interfaz: en cuanto hay un rectángulo de verdad, la rejilla
/// pasa a medirse desde él.
const REJILLA_INICIAL: Size = Size { rows: 24, cols: 80 };

/// Cuántos puntos de ancho se lleva la línea que separa las dos consolas.
///
/// Dos y no uno: con uno la línea se ve, pero también es la zona de agarre del arrastre, y un punto
/// es imposible de atrapar con el ratón.
///
/// **La zona de agarre es exactamente esta línea, sin ensanchar**, y lo que eso compra es que no haya
/// superficie sensible invisible: se agarra donde se ve. Si dos puntos resultan pocos con la ventana
/// delante, lo que se sube es esta constante, y la línea se ve más gruesa en consecuencia.
///
/// **Lo que no compra —y una versión anterior de este comentario decía que sí— es evitar que el mismo
/// gesto cambie el foco.** El borde del divisor *es* el borde de la consola de al lado, y egui cuenta un
/// punto sobre el borde como acierto directo del rectángulo (`Rect::distance_sq_to_pos` da cero ahí), así
/// que en las dos columnas frontera se acierta a los dos widgets: el divisor recibe el arrastre y la
/// consola recibe el clic. No se corrige porque no es un defecto — quien pulsa en el borde izquierdo de la
/// línea ha pulsado en el borde derecho de la consola izquierda, y darle el foco a esa es lo que cabe
/// esperar—; se escribe para que nadie lo lea como una garantía que no existe.
const GROSOR_DEL_DIVISOR: f32 = 2.0;

/// Con qué reparto abre la ventana: las dos consolas iguales.
const FRACCION_INICIAL: f32 = 0.5;

/// Cada cuánto vuelve a mirar la ventana cuando se ha quedado sin quien la avise.
///
/// Medio segundo es lo que se tarda en notar que un panel va con retraso sin que la ventana esté
/// despierta todo el rato. **Solo corre en el modo degradado**: mientras el vigilante funcione, la
/// ventana duerme hasta que alguien publica.
const RESPIRO_SIN_VIGILANTE: std::time::Duration = std::time::Duration::from_millis(500);

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

/// Reparte el ancho del área entre las dos consolas según la fracción, sin dejar a ninguna sin sitio.
///
/// **El grosor del divisor se descuenta primero y se reparte lo que queda.** Si se partiera el área
/// entera y luego se restara la línea, la consola de la que se restase saldría más estrecha que la otra
/// sin que nadie hubiera pedido eso.
///
/// **La fracción es de lo repartible, no del área**, y es lo que hace que la proporción sobreviva al
/// redimensionado de la ventana: al cambiar el área se recalculan los dos anchos desde la misma
/// fracción, en vez de que uno conserve sus puntos y el otro se coma toda la diferencia.
///
/// **Lo que compra el suelo de `ancho_minimo` por consola es modesto, y conviene no exagerarlo.** No
/// evita ningún pánico —el de `Screen::resize` con cero columnas lo cubre el mínimo de 1×1 de
/// [`Metrica::celdas_en`]— y tampoco salva la rejilla: con cero píxeles esa misma función ya da una
/// columna, exactamente la que da con el suelo puesto. Y **el asa no es la consola, es el divisor**, que
/// sigue en pantalla y sigue agarrable con la fracción a cero, así que una consola colapsada se recupera
/// con el mismo gesto que la colapsó. Lo que el suelo consigue es que la consola colapsada **siga
/// mostrando su columna** en vez de nada, para que el colapso se lea como algo que se ha hecho y no como
/// un fallo de pintado, y que el asa no acabe pegada al borde de la ventana.
///
/// **El suelo se rebaja a la mitad de lo repartible cuando no cabe dos veces.** Así el divisor termina en
/// el centro y las dos quedan iguales —por debajo del mínimo, que es lo único posible ahí— sin necesidad de
/// una rama aparte. Y no es solo brevedad: `f32::clamp` **entra en pánico si el mínimo supera al máximo**,
/// y rebajar el suelo hace que eso no pueda ocurrir por construcción, en vez de depender de que una guarda
/// cubra todos los casos.
///
/// El `max` con cero sobre `util` mantiene los tres rectángulos bien formados con una ventana más estrecha
/// que el propio divisor. Ahí las dos consolas llegan a cero puntos: no hay reparto posible, y lo que se
/// conserva es que ningún rectángulo salga invertido.
fn repartir(area: egui::Rect, fraccion: f32, ancho_minimo: f32) -> Reparto {
    let util = (area.width() - GROSOR_DEL_DIVISOR).max(0.0);
    let suelo = ancho_minimo.min(util / 2.0);
    let izquierda = (fraccion * util).clamp(suelo, util - suelo);
    let corte = area.left() + izquierda;
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
    /// El directorio donde el programa de esta consola publica su estado, y el identificador con el
    /// que se le conoce. Se borra al soltar la consola porque el tipo lo borra al morir.
    ///
    /// **Es opcional porque puede faltar de verdad**, no por prudencia: un directorio de ejecución
    /// de solo lectura en un contenedor, el disco lleno, o la variable del entorno apuntando a algo
    /// donde no se puede escribir. Cuando falta, el programa de dentro no ve ninguna de las dos
    /// variables, se le quitan las que hubiera heredado, y se comporta como antes de que existiera
    /// este hito.
    ///
    /// Se lee **también en vida** desde F1.6: es de donde sale el panel de estado de esta consola.
    panel: Option<PanelDir>,
    /// Lo que se sabe del panel de esta consola: su último contenido bueno y cuándo se leyó.
    estado_del_panel: panel::Estado,
    /// Si hay que volver a leer el `panel.json` de esta consola. La levanta el vigilante y la recoge
    /// el fotograma.
    ///
    /// **Es de la división y no del vigilante** porque es lo que hace que un aviso despierte a una
    /// consola y no a las dos: el vigilante es uno para todas y lo único que las distingue es a qué
    /// bandera llega su aviso.
    por_releer: Bandera,
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

/// Un directorio que no se pudo crear **no impide abrir la consola**.
///
/// Es la misma decisión que el respaldo a otro shell cuando el del usuario no se puede lanzar, y con
/// más motivo: sin panel, una consola sigue siendo un terminal completo. Está como función para poder
/// afirmarlo — desde una prueba no se puede hacer fallar la creación de verdad sin tocar el entorno
/// del proceso entero.
fn panel_o_ninguno(creado: std::io::Result<PanelDir>) -> Option<PanelDir> {
    match creado {
        Ok(panel) => Some(panel),
        Err(fallo) => {
            eprintln!("esta consola va sin panel: no se pudo crear su directorio ({fallo})");
            None
        }
    }
}

/// Qué se hace con las dos variables del panel al lanzar el programa de una consola.
///
/// **Sin directorio se quitan, y quitarlas no es lo mismo que no ponerlas.** El programa hereda el
/// entorno de esta aplicación, así que si la aplicación corre dentro de una consola de sí misma —lo
/// que pasa en cuanto se usa para trabajar— la consola hija recibiría la identidad de la madre: su
/// productor publicaría en el buzón de la madre y dos divisiones compartirían identificador, que es
/// exactamente lo que la identidad viene a evitar.
///
/// Son siempre dos, con directorio o sin él: es lo que hace imposible el caso de media identidad.
fn identidad_de(panel: Option<&PanelDir>) -> [(&'static str, Option<&std::ffi::OsStr>); 2] {
    match panel {
        Some(panel) => panel.env().map(|(nombre, valor)| (nombre, Some(valor))),
        None => panel_contract::ENV_NAMES.map(|nombre| (nombre, None)),
    }
}

impl Division {
    /// Lanza el shell de esta división y la deja lista para el primer fotograma.
    fn nueva(ctx: &egui::Context) -> term_core::Result<Self> {
        // El shell sale del entorno de quien lanza la aplicación, que es lo que hace un terminal:
        // sin esto se abre un shell sin prompt ni historial, y este hito existe para mirar lo que
        // sale en pantalla. Sigue sin ser una entrada ajena —es la sesión del propio usuario—,
        // pero el día en que el programa venga de configuración o de un `panel.json` de fuera,
        // esta línea pasa a ser superficie sensible y hay que revisarla como tal.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| SHELL_DE_RESPALDO.to_owned());

        Self::lanzando(ctx, &shell)
    }

    /// Lo mismo, con el programa dicho a mano.
    ///
    /// Existe porque el programa que elige [`Division::nueva`] es el del usuario, y una prueba que
    /// dependa de él no prueba este código: prueba la configuración de quien la corre.
    fn lanzando(ctx: &egui::Context, programa: &str) -> term_core::Result<Self> {
        // El directorio se crea **antes** de lanzar, porque el programa puede publicar en su primer
        // segundo y la variable que le decimos tiene que apuntar a algo que ya existe. Si lanzar
        // falla después, el directorio se borra solo: el valor muere por el camino de error.
        Self::con_panel(ctx, programa, panel_o_ninguno(PanelDir::create()))
    }

    /// Y lo mismo con el directorio ya resuelto, incluido el caso de que no haya podido crearse.
    ///
    /// Ese caso no se puede provocar desde una prueba sin tocar el entorno del proceso entero, y es
    /// justo el que tiene que quedar demostrado: que la consola arranca igual y que su programa no se
    /// queda con una identidad heredada.
    fn con_panel(
        ctx: &egui::Context,
        programa: &str,
        panel: Option<PanelDir>,
    ) -> term_core::Result<Self> {
        let identidad = identidad_de(panel.as_ref());

        // Si lo que dice el entorno ya no existe —un perfil borrado, un shell desinstalado—, se
        // cae al de respaldo en vez de no abrir. Quedarse sin ventana teniendo `/bin/sh` a mano, y
        // con un error que nadie va a entender, es el peor de los dos resultados.
        let (pty, salida) = match Pty::spawn_with_env(programa, &[], REJILLA_INICIAL, &identidad) {
            Ok(abierto) => abierto,
            Err(fallo) => {
                eprintln!("no se pudo lanzar «{programa}» ({fallo}); se abre {SHELL_DE_RESPALDO}");
                Pty::spawn_with_env(SHELL_DE_RESPALDO, &[], REJILLA_INICIAL, &identidad)?
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
            panel,
            estado_del_panel: panel::Estado::default(),
            por_releer: Bandera::levantada(),
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

    /// Vuelve a leer el `panel.json` de esta consola **si alguien ha avisado** de que cambió.
    ///
    /// La bandera se recoge siempre, haya directorio o no: dejarla levantada porque esta consola no
    /// tiene dónde leer haría que la recogiera la lectura de otro fotograma, y entonces el aviso que
    /// se guardó no sería de nadie.
    ///
    /// Sin directorio no hay nada que leer: es el caso de la consola que no pudo crear el suyo, que
    /// tiene que seguir viéndose como antes de que existiera el panel.
    fn refrescar_su_panel(&mut self) {
        if !self.por_releer.recoger() {
            return;
        }

        if let Some(dir) = self.panel.as_ref() {
            self.estado_del_panel.refrescar(dir.path());
        }
    }

    /// Parte el rectángulo de esta división entre su consola y su panel.
    ///
    /// **El reloj se pasa desde fuera y es el mismo con el que se pinta**: de la hora depende que un
    /// panel esté viejo, y de que lo esté depende que haya algo que enseñar. Dos relojes distintos en
    /// el mismo fotograma podrían reservar una franja donde luego no se pinta nada.
    fn repartir_con_su_panel(
        &self,
        rect: egui::Rect,
        ancho_de_celda: f32,
        ahora: SystemTime,
    ) -> panel::Hueco {
        panel::apartar(rect, ancho_de_celda, self.estado_del_panel.se_ve(ahora))
    }

    /// Manda al programa lo que se ha tecleado en este pase, y mide cuánto se tarda.
    ///
    /// El cronómetro arranca **al empezar el fotograma**, no al pulsar la tecla: el evento ya estaba
    /// esperando cuando el fotograma empezó y no trae sello propio, así que ese trecho no se puede
    /// ver desde aquí. El número lleva escrita esa frontera.
    fn teclear(
        &mut self,
        ctx: &egui::Context,
        empezo_el_fotograma: Instant,
        modificadores: &mut egui::Modifiers,
    ) {
        self.pulsaciones.clear();
        // Se pregunta una vez y no por evento porque **el modo no puede cambiar a mitad de pase**:
        // lo pone el programa de dentro con lo que escribe, y lo que escribe se interpreta en la otra
        // mitad del fotograma. (Preguntarlo dentro del cierre también compilaría —los dos campos son
        // préstamos disjuntos—, así que esto es una elección, no una pared.)
        let marcado = self.pantalla.pegado_marcado();
        let mut pegado = false;

        // Los eventos que van al programa **se consumen**, no se leen y se dejan. Un fotograma
        // puede correr más de un pase sobre la misma entrada —egui lo hace cuando algo pide
        // recomponerse—, y entonces la misma tecla se escribiría dos veces al pseudoterminal. Los
        // que no se traducen se quedan donde estaban, para quien los quiera después.
        ctx.input_mut(|estado| {
            // **Los modificadores se reconstruyen aquí, evento a evento, y no se leen de
            // `estado.modifiers`.** Ese campo es uno solo por pase: egui recorre el lote y se queda
            // con el último aviso de cambio, así que describe cómo estaban las teclas **al final**,
            // no cuando llegó el pegado. Con el acorde soltado antes de que se recoja el fotograma
            // valdría «ninguno» y se mandaría el byte de control; pulsando Mayúsculas después de
            // pegar, al revés. Reconstruirlos en orden es lo que hace el propio egui por dentro, y
            // es exacto.
            let mut acorde = *modificadores;
            let mut pego = false;

            estado.events.retain(|evento| {
                if let egui::Event::ModifiersChanged(nuevos) = evento {
                    acorde = *nuevos;
                    // Se queda en la cola: no es nuestro, y quien lo mire después lo necesita.
                    return true;
                }

                // **Perder el foco borra el acorde, y esto no es prudencia: es la misma salvaguarda
                // que egui tiene en su propio bucle**, con su razón escrita ahí — al salir la ventana
                // no llega el aviso de que se soltaron las teclas. Sin esto, quien mantenga
                // Ctrl+Mayúsculas, se vaya con un atajo del escritorio y suelte fuera, vuelve con las
                // Mayúsculas pegadas para siempre: el siguiente Ctrl+V pegaría en vez de mandar su
                // byte, que es la confusión que este trabajo existe para evitar, y en la dirección
                // que además vuelca el portapapeles en el programa.
                if matches!(evento, egui::Event::WindowFocused(false)) {
                    acorde = egui::Modifiers::NONE;
                    return true;
                }

                // **El escritorio manda el mismo evento para Ctrl+V y para Ctrl+Mayúsculas+V**, sin
                // decir cuál fue, y se traga la tecla. Ésta es la única puerta donde se pueden
                // separar, y separarlas es lo que hace que Ctrl+V le siga llegando al programa de
                // dentro: para vim es el bloque visual, y para un shell «lo siguiente, literal».
                if let egui::Event::Paste(texto) = evento
                    && acorde.shift
                {
                    self.pulsaciones
                        .extend_from_slice(&teclado::pegar(texto, marcado));
                    pego = true;
                    return false;
                }

                match teclado::bytes(evento) {
                    Some(bytes) => {
                        self.pulsaciones.extend_from_slice(&bytes);
                        false
                    }
                    None => true,
                }
            });

            *modificadores = acorde;
            pegado = pego;
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

        // **Un pegado no entra en esta serie, y no es un detalle de limpieza.** Lo que este número
        // mide es el trecho de una tecla al byte, y D3 lo publica: 36 µs en reposo, 3,42 ms con las
        // dos consolas volcando. Un portapapeles de un mega escrito de una vez añade ahí una muestra
        // de cientos de milisegundos —y sin techo si el programa no lee, ver el issue #50—, que
        // movería el percentil que esa decisión usa para juzgar el framework de interfaz. Es
        // exactamente el fallo del tramo con la etiqueta cambiada que este fichero ya documenta
        // haber pagado una vez.
        if !pegado {
            self.medida.anotar_ida(empezo_el_fotograma.elapsed());
        }

        // **La memoria de un pegado grande no se queda para siempre.** Este vector se reutiliza entre
        // fotogramas y vaciarlo no devuelve su capacidad: sin esto, pegar cincuenta megas los deja
        // reservados mientras viva el proceso, y por división. Es la simetría que el vector hermano
        // —lo que se recoge del programa— ya tenía con su presupuesto.
        if self.pulsaciones.capacity() > HOLGURA_DE_PULSACIONES {
            self.pulsaciones.shrink_to(HOLGURA_DE_PULSACIONES);
        }
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

/// Una disposición de consolas: lo que se ve dentro de una pestaña.
///
/// **Existe para separar lo que es de una disposición de lo que es del proceso.** Los tres campos de
/// aquí eran campos de [`Ventana`] cuando la ventana era una sola disposición; los que se quedaron
/// arriba —el tema, el vigilante, los modificadores, la medida— no describen cómo están repartidas
/// unas consolas, sino cómo está el programa.
///
/// **Es un contenedor transparente a propósito, sin frontera de métodos.** La costura que hace falta
/// la da el propio tipo: cuando las divisiones dejen de ser dos y pasen a ser un árbol, lo que cambia
/// es el interior de esto, y la lista de pestañas no se entera. Esconderlo además detrás de métodos
/// obligaría a meter dentro del pintado a [`Ventana::atender_clics`] y
/// [`Ventana::arrastrar_el_divisor`], que se sacaron justamente para poder comprobarse sin abrir
/// ninguna ventana.
struct Pestaña {
    /// Las dos consolas, de izquierda a derecha.
    ///
    /// **Un array de dos y no una lista**: el número está fijado por el hito —el árbol de divisiones
    /// arbitrarias es F2.2— y decirlo en el tipo es lo que permite indexar sin escribir ramas para un
    /// caso vacío que no existe. Una lista obligaría a preguntar en cada acceso si hay alguien ahí.
    divisiones: [Division; 2],
    /// A cuál va el teclado. Arranca en `0` —la izquierda— porque teclear antes del primer clic ocurre
    /// siempre al abrir la ventana, y con un foco nulo esas teclas se perderían.
    ///
    /// **No es el foco de egui.** El del framework es foco de teclado por widget, para tabular entre
    /// controles; esto es geográfico y lo decide la aplicación. Confundirlos deja el teclado en manos
    /// de quien no lo enruta.
    foco: usize,
    /// Qué parte del ancho repartible se lleva la consola de la izquierda.
    ///
    /// **Una fracción y no dos anchos en puntos**: es lo que hace que el reparto sobreviva al
    /// redimensionado de la ventana. Guardar puntos es lo que hacen los paneles de egui, y por eso se
    /// descartaron en el diseño — al ensanchar la ventana el panel conserva su ancho y toda la diferencia
    /// cae en el otro lado.
    ///
    /// **Se guarda la proporción que se pidió, no la que cupo.** El suelo por consola lo aplica
    /// [`repartir`] contra el área de cada fotograma, y no al guardar: recortarla antes hornearía el ancho
    /// actual de la ventana dentro del estado, así que una consola arrastrada al borde con la ventana
    /// estrecha seguiría pegada al borde después de ensancharla. Es el mismo argumento que descarta los
    /// paneles de egui, aplicado al estado en vez de a la geometría.
    fraccion: f32,
}

impl Pestaña {
    /// Dos consolas nuevas, con el shell del usuario dentro de cada una.
    fn nueva(ctx: &egui::Context) -> term_core::Result<Self> {
        Ok(Self::con([Division::nueva(ctx)?, Division::nueva(ctx)?]))
    }

    /// Cómo se abre una pestaña: el foco a la izquierda y el divisor donde diga [`FRACCION_INICIAL`].
    ///
    /// Existe aparte de [`Pestaña::nueva`] para que las pruebas entren por aquí con sus propias
    /// consolas en vez de armar el struct a mano — si no, el día que abrir una pestaña signifique algo
    /// más, las pruebas seguirían abriendo la de antes.
    fn con(divisiones: [Division; 2]) -> Self {
        Self {
            divisiones,
            foco: 0,
            fraccion: FRACCION_INICIAL,
        }
    }
}

/// La ventana raíz: las pestañas, y lo que es del programa y no de ninguna disposición.
struct Ventana {
    /// Las disposiciones abiertas. **Nunca vacía**: la ventana siempre está enseñando una.
    ///
    /// Hoy se abre con una sola y no hay forma de crear más — la barra y los atajos son de un paso
    /// posterior. Lo que este campo trae ya es la forma del estado sobre la que se apoyan.
    pestañas: Vec<Pestaña>,
    /// Cuál se ve y a cuál va el teclado. **Siempre menor que el número de pestañas.**
    activa: usize,
    /// Cómo estaban las teclas muertas al acabar el pase anterior.
    ///
    /// **Vive aquí y no en la división porque son del teclado, no de una consola.** Hace falta
    /// guardarlas porque un acorde puede empezar en un fotograma y usarse en el siguiente: si se
    /// mantienen Ctrl+Mayúsculas pulsadas y se suelta la V un fotograma después, el aviso de que se
    /// pulsaron llegó en el lote anterior y ya no está en éste. Y guardarlas en la ventana es lo que
    /// hace que cambiar de consola con el acorde puesto no lo pierda.
    modificadores: egui::Modifiers,
    /// Quien avisa de que un productor ha publicado. Vive con la ventana porque sus rutas son las de
    /// las consolas que la ventana tiene abiertas, y es la ventana quien sabe a quién despertar.
    vigilante: Vigilante,
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
        let pestaña = Pestaña::nueva(ctx)?;

        Ok(Self {
            vigilante: Self::vigilar(ctx, &pestaña.divisiones),
            pestañas: vec![pestaña],
            activa: 0,
            modificadores: egui::Modifiers::NONE,
            tema: Tema::default(),
            resto_del_fotograma: RestoDelFotograma::default(),
            empezo_el_fotograma: Instant::now(),
            ultimo_fotograma_cerrado: 0,
        })
    }

    /// La pestaña que se ve y a la que va el teclado.
    ///
    /// Ahorran escribir los dos campos en cada sitio; **no añaden ninguna garantía** — indexan igual
    /// y entran en pánico igual que hacerlo a mano.
    ///
    /// El mutable no vale donde haga falta prestar además otro campo de la ventana —el tema, al
    /// pintar—, porque presta la ventana entera; ahí se indexa la lista directamente. El de solo
    /// lectura no tiene ese problema.
    fn activa(&self) -> &Pestaña {
        &self.pestañas[self.activa]
    }

    fn activa_mut(&mut self) -> &mut Pestaña {
        &mut self.pestañas[self.activa]
    }

    /// Arma el vigilante sobre los directorios de las consolas que tienen uno.
    ///
    /// Una consola sin directorio no se vigila porque no hay nada que vigilar: nadie puede publicarle
    /// un panel. Su bandera se queda como esté y su lectura no encuentra dónde mirar.
    fn vigilar(ctx: &egui::Context, divisiones: &[Division]) -> Vigilante {
        let consolas: Vec<_> = divisiones
            .iter()
            .filter_map(|division| {
                let dir = division.panel.as_ref()?;
                Some((dir.path(), division.por_releer.clone()))
            })
            .collect();

        // Lo único que el vigilante necesita de la ventana es cómo despertarla, igual que el puente:
        // así su módulo no depende del framework de interfaz, que es lo que se va a reescribir. Pedir
        // el repintado no entra en pánico, que es la condición de quien lo llama desde otro hilo.
        let despertador = ctx.clone();

        Vigilante::nuevo(move || despertador.request_repaint(), &consolas)
    }

    /// **El latido: despertar a tiempo de decir que un panel está viejo.**
    ///
    /// El vigilante ve lo que cambia; esto ve lo que **deja** de cambiar, y son cosas distintas — un
    /// productor que se muere o se cuelga no escribe nada, así que no produce ningún evento de fichero.
    /// Sin este despertador el aviso no aparecería jamás en una ventana en reposo, que es justo cuando
    /// hace falta.
    ///
    /// Se pide **solo lo que falta para el primero que vence**: no es un reloj que corre siempre, es
    /// una alarma que se pone cuando hay algo que esperar y no se pone cuando no lo hay.
    ///
    /// **Se llama dos veces por fotograma, y las dos hacen falta.** Los paneles se releen en la pasada
    /// de interfaz, que corre *después* de la mitad lógica: armando la alarma solo en la primera, el
    /// fotograma en que un panel se lee por primera vez la calcularía con el estado de antes de
    /// leerlo —o sea, sin plazo que esperar— y la ventana se dormiría para siempre. Medido: el retardo
    /// que se publicaba en ese fotograma era `Duration::MAX`. Y armarla solo en la segunda dejaría
    /// fuera las vueltas en que la ventana está tapada y solo corre la mitad lógica. Pedir de más no
    /// cuesta: la ventana se queda con el plazo más corto de todos los que se le pidan.
    fn poner_el_despertador(&self, ctx: &egui::Context) {
        let ahora = SystemTime::now();

        if let Some(falta) = self
            .activa()
            .divisiones
            .iter()
            .filter_map(|division| division.estado_del_panel.cuanto_falta_para_avisar(ahora))
            .min()
        {
            ctx.request_repaint_after(falta);
        }
    }

    /// Cierra el fotograma anterior: cada consola cierra sus tramos y la ventana se queda con lo que
    /// ninguna cronometró.
    fn cerrar_fotograma(&mut self, coste_cpu: std::time::Duration) {
        let cronometrado = self
            .activa_mut()
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

        for division in &mut self.activa_mut().divisiones {
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
                self.activa_mut().foco = cual;
            }
        }
    }

    /// Mueve el divisor con el puntero, y devuelve si la fracción ha cambiado en este pase.
    ///
    /// Vive aparte del pintado por lo mismo que [`Self::atender_clics`]: es la decisión que este PR
    /// entrega, y dentro de la pasada de interfaz no se puede comprobar sin abrir una ventana.
    ///
    /// **La fracción sale de la posición absoluta del puntero, no de su desplazamiento.** Con el
    /// desplazamiento, cada trocito que el recorte del suelo descarta se perdería para siempre: el
    /// puntero seguiría avanzando y la fracción se quedaría atrás, así que al volver el divisor no
    /// estaría donde está el ratón. Con la posición absoluta, el divisor persigue al puntero y el
    /// recorte solo decide dónde se planta.
    ///
    /// Y hay una segunda razón que se descubrió después, más grave que la precisión: **la pasada de
    /// interfaz puede correr dos veces sobre la misma entrada**, y con el desplazamiento el segundo pase
    /// lo aplicaría otra vez, moviendo el divisor al doble. Con la posición absoluta el segundo pase
    /// calcula lo mismo y no cambia nada. Es la misma trampa que obligó a consumir los eventos del
    /// teclado en vez de leerlos.
    fn arrastrar_el_divisor(
        &mut self,
        ui: &egui::Ui,
        divisor: egui::Rect,
        area: egui::Rect,
    ) -> bool {
        // `Sense::DRAG` y no `Sense::drag()`: el segundo añade `FOCUSABLE`, o sea que el divisor pediría
        // el foco de teclado de egui y competiría con el enrutado geográfico de la aplicación. Es la
        // misma trampa que ya costó un caso con el clic.
        let id = ui.id().with("divisor");
        let respuesta = ui
            .interact(divisor, id, egui::Sense::DRAG)
            // Se mantiene el cursor **también mientras se arrastra**. No es por que el puntero se salga de
            // la línea —egui da por «encima» al widget que se está arrastrando, esté el puntero donde
            // esté—: es por el fotograma en que se suelta, donde el arrastre ya ha terminado y el puntero
            // puede haber quedado fuera. Con `on_hover_cursor` ahí el cursor parpadea a flecha justo al
            // acabar la maniobra.
            .on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);

        let Some(puntero) = respuesta.interact_pointer_pos() else {
            return false;
        };

        let util = area.width() - GROSOR_DEL_DIVISOR;
        // Sin nada que repartir no hay fracción que elegir, y con `util` **negativo** la división cambia de
        // signo: el divisor iría hacia el lado contrario al ratón. Se deja la fracción como la dejó el
        // último arrastre de verdad, que es lo que el usuario pidió la última vez que la petición tenía
        // sentido.
        if util <= 0.0 {
            return false;
        }

        // El recorte a `0..1` **no es para la geometría** —de eso se encarga [`repartir`] en cada
        // fotograma, con su propio suelo— sino para el estado: sin él, arrastrar más allá del borde
        // guardaría una fracción de 5, que el informe de salida publicaría como «al 500 %» y que obligaría
        // a recorrer el camino de vuelta antes de que el divisor se moviera.
        let nueva = ((puntero.x - area.left()) / util).clamp(0.0, 1.0);
        // La guarda de arriba deja pasar un `util` **sin número**, porque `NaN <= 0.0` es falso, y de ahí
        // sale una fracción NaN que el recorte tampoco atrapa: un `clamp` sobre un NaN devuelve NaN. Y un
        // NaN guardado aquí no se va nunca — la ventana dejaría de pintar y ni ensancharla ni volver a
        // arrastrar la recuperaría. El área sin número no es hipotética: es el caso que
        // [`Metrica::celdas_en`] ya documenta, mientras el escritorio no ha dicho todavía cuánto mide la
        // ventana.
        if !nueva.is_finite() {
            return false;
        }

        let pestaña = self.activa_mut();
        let anterior = pestaña.fraccion;
        pestaña.fraccion = nueva;
        pestaña.fraccion != anterior
    }

    /// Manda lo que se ha tecleado en este pase a **una sola** consola, la que tiene el foco.
    ///
    /// Vive aparte del pintado por lo mismo que [`Self::atender_clics`]: mandarlo a las dos escribiría
    /// la misma tecla en dos programas, que es exactamente lo que «sin que se pisen» prohíbe, y dentro
    /// de la pasada de interfaz esa decisión no se puede comprobar sin abrir una ventana.
    fn teclear_la_del_foco(&mut self, ctx: &egui::Context) {
        let empezo = self.empezo_el_fotograma;
        // La semilla se saca y se vuelve a meter porque la división y el campo son dos préstamos
        // del mismo dueño, y el compilador no deja tener los dos a la vez.
        let mut modificadores = self.modificadores;
        let pestaña = self.activa_mut();
        pestaña.divisiones[pestaña.foco].teclear(ctx, empezo, &mut modificadores);
        self.modificadores = modificadores;
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
        // **El panel se cronometra aparte del encolado de la consola, con dos relojes que se suman:
        // uno alrededor de leer y repartir, y otro alrededor de pintarlo.** Son dos y no uno porque el
        // pintado de la consola queda justo en medio y tiene su propio tramo. Para la resta del
        // fotograma daría igual sumarlo todo —lo que importa ahí es
        // que se declare—, pero un tramo llamado «encolando el pintado» que además incluyera leer un
        // fichero de disco es un número con la etiqueta cambiada, y en este proyecto eso ya invalidó
        // una medida entera.
        //
        // **Releer va antes de repartir**, porque de que haya panel o no depende cuánto ancho le queda
        // a la consola: una consola cuyo productor acaba de publicar por primera vez se estrecha en
        // este mismo fotograma, no en el siguiente.
        let antes_del_panel = Instant::now();
        division.refrescar_su_panel();
        // Un solo reloj para repartir y para pintar: si el panel envejeciera entre las dos preguntas,
        // se reservaría una franja para un aviso que no llega a pintarse.
        let ahora = SystemTime::now();
        let hueco = division.repartir_con_su_panel(rect, metrica.ancho(), ahora);
        let coste_de_leer = antes_del_panel.elapsed();

        division.medido = metrica.celdas_en(hueco.consola.size());

        let antes = Instant::now();
        pintor::pintar(
            &ui.painter_at(hueco.consola),
            &division.pantalla.view(),
            tema,
            metrica,
            hueco.consola.left_top(),
        );
        let encolado = antes.elapsed();

        let antes_de_pintarlo = Instant::now();
        if let (Some(sitio), Some(dir)) = (hueco.panel, division.panel.as_ref()) {
            panel::pintar(
                &ui.painter_at(sitio),
                &mut division.estado_del_panel,
                dir.id(),
                tema,
                metrica,
                ahora,
            );
        }
        let coste_del_panel = coste_de_leer + antes_de_pintarlo.elapsed();

        // El cierre de la vuelta va aquí y no en el bucle de arriba porque **el encolado hay que
        // declararlo siempre**, también cuando a esta consola no le llegó salida: se pintó igual, y su
        // coste tiene que salir del «resto del fotograma» de la ventana.
        if let Some((esperando_desde, recogido_en, interpretado)) =
            division.pendiente_de_medir.take()
        {
            division.medida.anotar_vuelta(medida::Tramos::de_una_vuelta(
                esperando_desde,
                recogido_en,
                interpretado,
                encolado,
                coste_del_panel,
            ));
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

        // **Sin quien avise se vuelve a mirar en cada fotograma**, que es como se hacía antes de que
        // hubiera vigilante. No es prudencia: cuando se pierde el seguimiento —el usuario borra desde
        // su propia consola el directorio que se vigilaba, o el vigilante entrega un fallo— nadie
        // levanta ninguna bandera, y un panel congelado en silencio es peor que uno que se releía de
        // más.
        //
        // **Y hay que pedir el fotograma, o el respaldo no llega a correr**: con la ventana en reposo
        // y la consola callada —un agente esperando respuesta, que es justo cuando su panel dice lo
        // más valioso— no hay ninguna vuelta donde recoger la bandera. Medio segundo de latencia, y
        // solo en el modo degradado.
        if self.vigilante.esta_ciego() {
            for division in &self.activa().divisiones {
                division.por_releer.levantar();
            }
            ctx.request_repaint_after(RESPIRO_SIN_VIGILANTE);
        }

        // El latido va aquí **y también al final de la pasada de interfaz**, y las dos veces hacen
        // falta: ver [`Ventana::poner_el_despertador`].
        self.poner_el_despertador(ctx);

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

        let area = ui.max_rect();
        let mut reparto = repartir(area, self.activa().fraccion, metrica.ancho());

        // El arrastre se atiende **antes de pintar y se vuelve a repartir en el mismo pase**: pintar con
        // el reparto de antes dejaría al divisor un fotograma por detrás del ratón, que es justo lo que se
        // nota en un arrastre.
        if self.arrastrar_el_divisor(ui, reparto.divisor, area) {
            reparto = repartir(area, self.activa().fraccion, metrica.ancho());
        }

        let rectangulos = [reparto.izquierda, reparto.derecha];

        // El divisor se pinta antes que las consolas: si el glifo del borde de una se saliera, el
        // recorte ya lo corta, y así la línea nunca queda por encima de una letra.
        ui.painter()
            .rect_filled(reparto.divisor, 0.0, self.tema.divisor());

        self.atender_clics(ui, &rectangulos);

        // Se indexa a mano en vez de pasar por `activa_mut`, que prestaría la ventana entera: el tema
        // hace falta a la vez y es otro campo.
        for (division, rect) in self.pestañas[self.activa]
            .divisiones
            .iter_mut()
            .zip(rectangulos)
        {
            Self::pintar_division(ui, division, rect, &self.tema, &metrica);
        }

        // El realce va al final para que quede por encima del contenido: un borde tapado a medias por
        // un glifo del borde no dice nada. `StrokeKind::Inside` lo mantiene dentro del rectángulo, así
        // que no invade la consola vecina ni el divisor.
        ui.painter().rect_stroke(
            rectangulos[self.activa().foco],
            0.0,
            egui::Stroke::new(1.0, self.tema.foco()),
            egui::StrokeKind::Inside,
        );

        // **Y ahora sí, con los paneles ya releídos.** Ver [`Ventana::poner_el_despertador`]: la mitad
        // lógica lo pide con el estado de antes de leer, así que el fotograma en que un panel aparece
        // por primera vez se dormiría sin haber programado nada.
        self.poner_el_despertador(ui.ctx());
    }

    /// Los números se publican al apagar, y **aquí y no al ver morir el programa**, por dos razones
    /// que se descubrieron mirando la salida: pedir el cierre no cierra en el acto, así que ese
    /// camino imprimía el informe una vez por cada fotograma que aún llegaba; y cerrar la ventana a
    /// mano —que es como se sale la mitad de las veces— no pasa por ahí, así que la sesión se
    /// perdía entera sin publicar nada.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let pestaña = self.activa();

        if !pestaña
            .divisiones
            .iter()
            .any(|division| division.medida.hay_algo())
        {
            return;
        }

        let mut salida = std::io::stdout().lock();
        medida::cabecera(pestaña.divisiones.each_ref().map(|d| d.pantalla.size()));
        // Dónde quedó el divisor, y no «mitad y mitad»: desde que se arrastra, los tramos de abajo pueden
        // venir de dos consolas de anchos muy distintos, y una tabla de tramos sin saber con qué reparto
        // se tomó no compara con nada.
        //
        // **Se publica la fracción pedida, no los anchos resultantes.** Los anchos de verdad están arriba,
        // en la rejilla de cada consola, que es lo que se usó para interpretar; repetirlos aquí a partir de
        // la fracción los haría discrepar en el único caso en que el suelo actúa.
        println!(
            "divisor:      al {:.0} % del ancho repartible al salir, y son {GROSOR_DEL_DIVISOR} puntos \
             de ancho. Si se arrastró durante la sesión, los tramos de abajo mezclan repartos",
            pestaña.fraccion * 100.0
        );
        println!(
            "foco al salir: consola {} ({})",
            pestaña.foco,
            medida::NOMBRE_DE_CONSOLA[pestaña.foco]
        );

        for (cual, division) in pestaña.divisiones.iter().enumerate() {
            println!(
                "\n── consola {cual} ({}) ──",
                medida::NOMBRE_DE_CONSOLA[cual]
            );
            division
                .medida
                .publicar_tramos(&mut salida)
                .expect("escribir el informe por la salida estándar");
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

    fn encolar(ctx: &egui::Context, eventos: Vec<egui::Event>) {
        ctx.input_mut(|entrada| {
            entrada.events.clear();
            entrada.events.extend(eventos);
        });
    }

    fn con_mayusculas() -> egui::Modifiers {
        egui::Modifiers {
            shift: true,
            ctrl: true,
            ..Default::default()
        }
    }

    #[test]
    fn con_mayusculas_se_pega_el_texto_y_sin_ellas_va_el_byte_de_control() {
        // Los dos atajos llegan como el MISMO evento y sin modificadores: el escritorio los resuelve
        // antes y se traga la tecla. Separarlos aquí es lo que hace que Ctrl+V le siga llegando al
        // programa de dentro, que es quien decide si para él significa pegar (Claude) o bloque
        // visual (vim).
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30");
        // La ventana guarda esto entre fotogramas; aquí lo hace la variable, que es lo mismo.
        let mut modificadores = egui::Modifiers::NONE;

        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(con_mayusculas()),
                egui::Event::Paste("hola".to_owned()),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(division.pulsaciones, b"hola", "con Mayúsculas no se pegó");

        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                egui::Event::Paste("hola".to_owned()),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(
            division.pulsaciones, b"\x16",
            "sin Mayúsculas tiene que llegar el byte de control, no el texto"
        );
    }

    #[test]
    fn el_modo_que_pide_el_programa_llega_hasta_lo_que_se_pega() {
        // **Es el único cable de este trabajo que no estaba atado**, y la lección es la misma que el
        // diseño invoca para los modificadores: probar la función pura demuestra que compone bien la
        // marca, no que alguien le pase el modo de verdad. Poniendo `false` a pelo en el bucle, las
        // 298 seguían verdes y pegar varias líneas volvía a ejecutarlas una a una.
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30");
        let mut modificadores = egui::Modifiers::NONE;

        // Sin que el programa lo pida, el texto va desnudo.
        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(con_mayusculas()),
                egui::Event::Paste("ls\n".to_owned()),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(division.pulsaciones, b"ls\r", "llegó marcado sin pedirlo");

        // Y en cuanto lo pide, la marca aparece — sin tocar nada más que la pantalla.
        division.pantalla.advance(b"\x1b[?2004h");
        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(con_mayusculas()),
                egui::Event::Paste("ls\n".to_owned()),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(
            division.pulsaciones, b"\x1b[200~ls\n\x1b[201~",
            "el programa pidió el pegado marcado y el bucle no se enteró"
        );
    }

    #[test]
    fn los_modificadores_se_leen_del_momento_del_pegado_y_no_del_final_del_lote() {
        // **Ésta es la prueba que separa el mecanismo bueno del que lo parecía.** El campo que egui
        // publica —`InputState::modifiers`— es uno solo por pase: recorre el lote y se queda con el
        // último aviso de cambio. Leerlo describiría cómo estaban las teclas al FINAL, no cuando
        // llegó el pegado.
        //
        // Las dos secuencias de abajo tienen el mismo conjunto de eventos y orden distinto, y con
        // ese campo darían el resultado cambiado en las dos.
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30");
        // La ventana guarda esto entre fotogramas; aquí lo hace la variable, que es lo mismo.
        let mut modificadores = egui::Modifiers::NONE;

        // Se pega y se suelta el acorde antes de que se recoja el fotograma: el final del lote dice
        // «ninguno», pero cuando llegó el pegado sí estaban.
        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(con_mayusculas()),
                egui::Event::Paste("bueno".to_owned()),
                egui::Event::ModifiersChanged(egui::Modifiers::NONE),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(
            division.pulsaciones, b"bueno",
            "soltar el acorde después de pegar impidió el pegado: se leyó el final del lote"
        );

        // Y al revés: se pega sin Mayúsculas y se pulsan después. El final del lote dice que sí.
        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                egui::Event::Paste("malo".to_owned()),
                egui::Event::ModifiersChanged(con_mayusculas()),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(
            division.pulsaciones, b"\x16",
            "pulsar Mayúsculas después del pegado lo convirtió en pegado: se leyó el final del lote"
        );
    }

    #[test]
    fn salir_de_la_ventana_borra_el_acorde_para_que_no_se_quede_pegado() {
        // **El escenario es un atajo del escritorio, y termina volcando el portapapeles.** Con
        // Ctrl+Mayúsculas pulsadas se cambia de ventana con una combinación global; las teclas se
        // sueltan fuera y el aviso de que se soltaron no llega nunca. Sin borrar el acorde al salir,
        // las Mayúsculas se quedan puestas para siempre y el siguiente Ctrl+V —que el usuario espera
        // que entre en bloque visual— vuelca el portapapeles en el programa.
        //
        // Es la misma salvaguarda que egui tiene en su propio bucle de entrada, con su razón escrita
        // ahí; esta reconstrucción la había olvidado.
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30");
        let mut modificadores = egui::Modifiers::NONE;

        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(con_mayusculas()),
                egui::Event::WindowFocused(false),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores);

        encolar(
            &ctx,
            vec![egui::Event::Paste("no deberías verme".to_owned())],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(
            division.pulsaciones, b"\x16",
            "las Mayúsculas se quedaron pegadas al salir de la ventana, y el pegado se coló"
        );
    }

    #[test]
    fn un_acorde_que_empezo_en_el_fotograma_anterior_sigue_valiendo() {
        // Mantener las teclas pulsadas y soltar la V un fotograma después es lo normal al pegar: el
        // aviso de que se pulsaron llegó en el lote anterior y ya no está en éste. Sin recordar cómo
        // se cerró el pase, ese pegado se leería como si no hubiera Mayúsculas.
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30");
        // La ventana guarda esto entre fotogramas; aquí lo hace la variable, que es lo mismo.
        let mut modificadores = egui::Modifiers::NONE;

        encolar(&ctx, vec![egui::Event::ModifiersChanged(con_mayusculas())]);
        division.teclear(&ctx, Instant::now(), &mut modificadores);

        // **Vaciar la cola es lo que convierte esto en dos fotogramas de verdad.** egui rehace la
        // lista de eventos en cada uno, así que el aviso de que se pulsaron las teclas no sigue ahí
        // al siguiente. Sin este vaciado la prueba se aprobaría sola: volvería a leer el mismo aviso
        // que dejó el pase anterior, y no ejercitaría la semilla en absoluto.
        ctx.input_mut(|entrada| entrada.events.clear());

        encolar(&ctx, vec![egui::Event::Paste("tarde".to_owned())]);
        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(
            division.pulsaciones, b"tarde",
            "el acorde del fotograma anterior se perdió"
        );
    }

    #[test]
    fn interrumpir_sigue_interrumpiendo_con_y_sin_mayusculas() {
        // Es lo que más fácil se rompe al traer el portapapeles: copiar en un terminal es
        // Ctrl+Mayúsculas+C, y si esa combinación dejara de interrumpir, un programa colgado se
        // queda colgado.
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30");
        // La ventana guarda esto entre fotogramas; aquí lo hace la variable, que es lo mismo.
        let mut modificadores = egui::Modifiers::NONE;

        for acorde in [egui::Modifiers::CTRL, con_mayusculas()] {
            encolar(
                &ctx,
                vec![egui::Event::ModifiersChanged(acorde), egui::Event::Copy],
            );
            division.teclear(&ctx, Instant::now(), &mut modificadores);
            assert_eq!(division.pulsaciones, b"\x03", "dejó de interrumpir");

            encolar(
                &ctx,
                vec![egui::Event::ModifiersChanged(acorde), egui::Event::Cut],
            );
            division.teclear(&ctx, Instant::now(), &mut modificadores);
            assert_eq!(division.pulsaciones, b"\x18", "cortar cambió de byte");
        }
    }

    /// Monta una división con un programa dentro, sin framework de interfaz de por medio.
    ///
    /// Las dos mitades del fotograma que aquí se prueban —bombear y redimensionar— no tocan la
    /// ventana del escritorio, así que no hace falta abrir ninguna: se arma el struct a mano con
    /// las mismas piezas que [`Division::nueva`].
    /// Deja en la cola de la ventana exactamente estos eventos, en este orden.
    ///
    /// **Vacía lo que hubiera antes, y esa es la mitad del ayudante.** Los avisos de cambio de
    /// modificadores se dejan a propósito en la cola cuando se procesan, así que sin vaciar, una
    /// segunda llamada correría contra los suyos **más los del tramo anterior** — y la prueba estaría
    /// afirmando sobre una secuencia que no es la que escribió. Un fotograma de verdad rehace la
    /// lista entera; esto hace lo mismo.
    fn division_con(programa: &str) -> Division {
        let (pty, salida) = Pty::spawn("sh", &["-c", programa], REJILLA_INICIAL)
            .expect("no se pudo abrir el pseudoterminal");

        Division {
            pty,
            puente: Puente::nuevo(salida, || {}),
            panel: None,
            estado_del_panel: panel::Estado::default(),
            por_releer: Bandera::levantada(),
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
        let pestaña = Pestaña::con([division_con(izquierda), division_con(derecha)]);

        Ventana {
            // Estas consolas no tienen directorio, así que no hay nada que vigilar; el vigilante se
            // arma igual —sobre una lista vacía— para que la ventana de las pruebas no se declare
            // ciega y active el respaldo, que es un camino con su propia prueba.
            vigilante: Ventana::vigilar(&egui::Context::default(), &pestaña.divisiones),
            pestañas: vec![pestaña],
            activa: 0,
            modificadores: egui::Modifiers::NONE,
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
    fn al_redimensionar_el_programa_de_dentro_se_entera_del_tamano_nuevo() {
        // La **otra** mitad de AC5, y la que no estaba probada: el test de arriba solo comprueba la
        // rejilla, así que sustituir `Pty::resize` por un `Ok(())` dejaba la suite entera en verde. Y esa
        // mitad es la que hace que `vim` repinte: sin ella la rejilla mide bien y el programa pinta como si
        // nada, que es el fallo que el arrastre del divisor puede provocar cada vez que se mueve el ratón.
        //
        // Se prueba con un programa de verdad porque no hay otra forma: el tamaño vive en el núcleo, del
        // lado del esclavo, y desde este proceso no se puede leer. Quien puede contarlo es el programa de
        // dentro, y `stty size` es preguntárselo.
        let nuevo = Size { rows: 10, cols: 40 };
        let mut division = division_con("while :; do stty size; sleep 0.05; done");

        // Primero se comprueba que el programa informa del tamaño de arranque. Sin esto, un `stty` que no
        // existiera o que fallara daría el mismo resultado que un redimensionado que no llega: pantalla sin
        // el número esperado, y el test culparía al redimensionado.
        let arranque = format!("{} {}", REJILLA_INICIAL.rows, REJILLA_INICIAL.cols);
        esperar_a_que_la_pantalla_diga(&mut division, &arranque);

        division.medido = nuevo;
        division.redimensionar();

        esperar_a_que_la_pantalla_diga(&mut division, &format!("{} {}", nuevo.rows, nuevo.cols));
    }

    /// Bombea hasta que el programa de dentro haya escrito el texto pedido, o se rinde.
    ///
    /// La rejilla se limpia antes de esperar: lo que el programa escribió **antes** del redimensionado
    /// sigue en pantalla, y sin limpiar, buscar el tamaño nuevo encontraría el viejo de la vuelta anterior
    /// y daría por bueno un redimensionado que no ocurrió.
    fn esperar_a_que_la_pantalla_diga(division: &mut Division, texto: &str) {
        division.pantalla = Screen::new(division.pantalla.size());

        let limite = Instant::now() + Duration::from_secs(5);
        while !division.pantalla.to_text().contains(texto) {
            assert!(
                Instant::now() < limite,
                "el programa no llegó a escribir «{texto}»; en pantalla hay: {:?}",
                division.pantalla.to_text()
            );
            division.bombear();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn una_tecla_no_se_manda_dos_veces_aunque_el_fotograma_haga_dos_pases() {
        // egui puede correr la pasada de interfaz dos veces sobre la misma entrada. Los eventos que
        // van al programa se consumen por eso: leerlos y dejarlos escribiría la misma tecla dos
        // veces al pseudoterminal, y el programa de dentro no tiene forma de saber que sobra una.
        let mut division = division_con("sleep 30");
        let ctx = egui::Context::default();
        let mut modificadores = egui::Modifiers::NONE;
        ctx.input_mut(|entrada| {
            entrada.events.push(egui::Event::Text("a".to_owned()));
            entrada.events.push(egui::Event::WindowFocused(true));
        });

        division.teclear(&ctx, Instant::now(), &mut modificadores);
        assert_eq!(division.pulsaciones, b"a", "no se envió lo tecleado");

        division.teclear(&ctx, Instant::now(), &mut modificadores);
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
        for division in &ventana.activa().divisiones {
            esperar_a_que_muera(division);
        }
        ventana.bombear_todas();

        let [izquierda, derecha] = &ventana.activa().divisiones;
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
            ventana.activa().foco,
            0,
            "el foco no arranca en la consola izquierda"
        );
    }

    #[test]
    fn la_ventana_arranca_con_una_sola_pestaña_y_apuntando_a_ella() {
        // Las dos afirmaciones de las que cuelga todo lo demás, y ninguna se ve desde `activa()`:
        // ese acceso da la pestaña buena tanto si hay una como si hay siete, así que esconde
        // exactamente lo que aquí se quiere fijar. Sin esto, abrir con dos pestañas —o con el índice
        // apuntando a una que no existe, que es un pánico en el primer fotograma— pasaría inadvertido
        // hasta que alguien mirase la ventana.
        let ventana = Ventana::nueva(&egui::Context::default())
            .expect("no se pudo abrir la ventana con sus dos consolas");

        assert_eq!(
            ventana.pestañas.len(),
            1,
            "la ventana no abre con una sola pestaña"
        );
        assert!(
            ventana.activa < ventana.pestañas.len(),
            "la pestaña activa apunta fuera de la lista"
        );
    }

    #[test]
    fn cada_consola_tiene_su_propia_identidad_y_su_propio_directorio() {
        let ventana = Ventana::nueva(&egui::Context::default())
            .expect("no se pudo abrir la ventana con sus dos consolas");

        let [izquierda, derecha] = ventana.activa().divisiones.each_ref().map(|division| {
            division
                .panel
                .as_ref()
                .expect("la consola tiene que arrancar con su directorio")
        });

        assert_ne!(
            izquierda.id(),
            derecha.id(),
            "las dos consolas comparten identificador"
        );
        assert_ne!(
            izquierda.path(),
            derecha.path(),
            "las dos consolas comparten directorio, así que se pisarían el estado"
        );
    }

    /// **Que la ventana de verdad arranque con quien la avise, y sobre los directorios de sus
    /// consolas.** Todo lo demás de este mecanismo se prueba montando el vigilante a mano, y eso deja
    /// sin atar justo lo que no se puede montar a mano: lo que `Ventana::nueva` le pasa. Cambiar ahí
    /// las divisiones por una lista vacía deja el vigilante **armado** —así que el respaldo tampoco
    /// entra— y ningún panel se volvería a mover en toda la sesión, en silencio.
    ///
    /// **Y hay una prueba por sentido, con su ventana cada una**, porque publicar siempre en la misma
    /// consola solo ata la mitad: atar las dos rutas a la bandera de la primera pasaría igual, y en
    /// producto eso es publicar en la derecha y releer la izquierda. Las dos publicaciones no caben en
    /// la misma ventana: una escritura produce varios avisos y el crate no promete cuántos, así que un
    /// aviso rezagado del primer fichero volvería a levantar una bandera ya recogida.
    #[test]
    fn la_ventana_recien_abierta_se_entera_de_lo_que_publica_su_primera_consola() {
        solo_la_consola_que_publica_vuelve_a_mirar(0);
    }

    #[test]
    fn la_ventana_recien_abierta_se_entera_de_lo_que_publica_su_segunda_consola() {
        solo_la_consola_que_publica_vuelve_a_mirar(1);
    }

    fn solo_la_consola_que_publica_vuelve_a_mirar(cual: usize) {
        let ventana = Ventana::nueva(&egui::Context::default())
            .expect("no se pudo abrir la ventana con sus dos consolas");
        assert!(
            !ventana.vigilante.esta_ciego(),
            "la ventana abrió sin nadie que la avise"
        );
        for division in &ventana.activa().divisiones {
            division.por_releer.recoger();
        }

        publicar_en(
            &ventana.activa().divisiones[cual],
            r#"{"lines":["ya estoy"]}"#,
        );

        esperar_la_bandera(
            &ventana.activa().divisiones[cual].por_releer,
            "la ventana no vigila el directorio de esa consola",
        );
        assert!(
            !ventana.activa().divisiones[1 - cual].por_releer.recoger(),
            "publicar en una consola hizo releer a la otra"
        );
    }

    /// **Que a una consola le falte su directorio no puede apagar el vigilante de la que sí lo tiene.**
    /// Sin esto, un contenedor de solo lectura o un disco lleno en **una** consola dejaría a las dos
    /// releyendo en cada fotograma y a la ventana despertándose cada medio segundo para siempre: la
    /// economía entera de este PR perdida, y nada en pantalla que lo explique.
    #[test]
    fn una_consola_sin_directorio_no_deja_ciega_a_la_ventana() {
        let ctx = egui::Context::default();
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.activa_mut().divisiones[0].panel =
            Some(PanelDir::create().expect("crear el directorio"));
        assert!(
            ventana.activa().divisiones[1].panel.is_none(),
            "esta prueba necesita una consola sin directorio"
        );

        ventana.vigilante = Ventana::vigilar(&ctx, &ventana.activa().divisiones);

        assert!(
            !ventana.vigilante.esta_ciego(),
            "una consola sin sitio donde publicar apagó el vigilante de la que sí lo tiene"
        );
        for division in &ventana.activa().divisiones {
            division.por_releer.recoger();
        }
        publicar_en(&ventana.activa().divisiones[0], r#"{"lines":["aquí sí"]}"#);
        esperar_la_bandera(
            &ventana.activa().divisiones[0].por_releer,
            "la consola con directorio dejó de estar vigilada",
        );
    }

    /// El criterio de punta a punta: el programa de dentro **hereda** la variable y **puede escribir**
    /// donde dice. Se teclea al pseudoterminal de verdad en vez de armar el entorno a mano, porque lo
    /// que se prueba es justo la cadena que va del constructor al programa.
    ///
    /// Con `sh` y no con el shell del usuario: se teclea sintaxis POSIX, y con un shell que redirija
    /// de otra forma —nushell, PowerShell— este test fallaría por la configuración de quien lo corre
    /// en vez de por el código.
    #[test]
    fn el_programa_de_dentro_publica_en_el_directorio_que_le_dijeron() {
        let mut division = Division::lanzando(&egui::Context::default(), "sh")
            .expect("no se pudo abrir la consola");

        let publicado = {
            let esperado = division
                .panel
                .as_ref()
                .expect("la consola tiene que arrancar con su directorio")
                .path()
                .join(panel_contract::FICHERO_DEL_PANEL);

            division
                .pty
                .write(b"printf '{}' > \"$PANEL_DIR/panel.json\"\n")
                .expect("escribir al pseudoterminal");

            esperado
        };

        // El shell tarda lo que tarda en arrancar y leer su configuración, así que se sondea con
        // límite en vez de dormir una cifra fija.
        let limite = Instant::now() + Duration::from_secs(10);
        while !publicado.exists() && Instant::now() < limite {
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            publicado.is_file(),
            "el programa de dentro no pudo publicar en {}",
            publicado.display()
        );
    }

    /// La lista completa de lo que la aplicación mete en el entorno del programa de un usuario, en un
    /// solo sitio y con una igualdad, no con una pertenencia. Falla el día que alguien añada una
    /// quinta variable, sea cual sea — que es exactamente lo que se quiere, porque el peligro no es un
    /// nombre concreto, es «una variable más que nadie revisó».
    /// La lista completa de lo que la aplicación mete en el entorno del programa de un usuario,
    /// **mirando lo que el programa ve de verdad** y no las constantes que se encadenan para
    /// construirlo. La diferencia no es teórica: con la versión que comparaba constantes, meter a mano
    /// un `cmd.env("ANTHROPIC_API_KEY", …)` en el lanzador dejaba la suite entera en verde — o sea, el
    /// test no protegía la única regla del proyecto que existe para eso.
    ///
    /// Se lanza `env` y no un shell **a propósito**: un shell añade las suyas —`PWD`, `SHLVL`, `_`— y
    /// la diferencia dejaría de ser la de la aplicación. `SHELL` se descuenta porque la pone
    /// `portable-pty` por su cuenta, leída de la base de datos de usuarios, no esta aplicación.
    #[test]
    fn el_programa_no_ve_ninguna_variable_mas_que_las_cuatro() {
        let panel = PanelDir::create().expect("crear un directorio de panel");
        let (_pty, canal) =
            Pty::spawn_with_env("env", &[], REJILLA_INICIAL, &identidad_de(Some(&panel)))
                .expect("lanzar env");

        let salida = recoger_hasta_el_cierre(&canal);
        let del_hijo: std::collections::HashMap<&str, &str> = salida
            .lines()
            .filter_map(|linea| linea.split_once('='))
            .collect();

        // Las cuatro están, y con su valor.
        assert_eq!(del_hijo.get("TERM"), Some(&"xterm-256color"));
        assert_eq!(del_hijo.get("COLORTERM"), Some(&"truecolor"));
        assert_eq!(del_hijo.get("PANEL_ID"), Some(&panel.id()));
        assert_eq!(
            del_hijo.get("PANEL_DIR").map(std::path::Path::new),
            Some(panel.path())
        );

        // Y ninguna más cambia respecto a lo que este proceso ya tenía. La comparación es por par
        // completo, así que caza tanto una variable nueva como una con el valor pisado.
        let esperadas = ["TERM", "COLORTERM", "SHELL"]
            .into_iter()
            .chain(panel_contract::ENV_NAMES);
        let esperadas: std::collections::HashSet<&str> = esperadas.collect();
        let del_padre: std::collections::HashMap<String, String> = std::env::vars().collect();
        let mut inesperadas: Vec<&str> = del_hijo
            .iter()
            .filter(|(nombre, valor)| del_padre.get(**nombre).map(String::as_str) != Some(**valor))
            .map(|(nombre, _)| *nombre)
            .filter(|nombre| !esperadas.contains(nombre))
            .collect();
        inesperadas.sort();

        assert!(
            inesperadas.is_empty(),
            "la aplicación mete en el entorno del programa algo que nadie ha declarado: {inesperadas:?}"
        );
    }

    /// Recoge la salida de un programa hasta que muere y la devuelve como texto.
    fn recoger_hasta_el_cierre(canal: &std::sync::mpsc::Receiver<Vec<u8>>) -> String {
        let mut bytes = Vec::new();
        while let Ok(trozo) = canal.recv_timeout(Duration::from_secs(10)) {
            bytes.extend_from_slice(&trozo);
        }
        String::from_utf8_lossy(&bytes).replace('\r', "")
    }

    /// Sin directorio las dos variables no se dejan de poner: **se quitan**. La diferencia importa
    /// porque el programa hereda el entorno de esta aplicación, así que al lanzarla dentro de una de
    /// sus propias consolas, «no ponerlas» dejaría a la consola hija con la identidad de la madre.
    #[test]
    fn una_consola_sin_directorio_manda_quitar_las_dos_variables() {
        let identidad = identidad_de(None);

        assert_eq!(
            identidad,
            [("PANEL_ID", None), ("PANEL_DIR", None)],
            "sin directorio hay que quitar las dos, no dejarlas al azar de lo heredado"
        );
    }

    #[test]
    fn un_directorio_que_no_se_pudo_crear_no_impide_abrir_la_consola() {
        let sin_directorio = panel_o_ninguno(Err(std::io::Error::other("disco lleno")));

        assert!(
            sin_directorio.is_none(),
            "un fallo al crear el directorio no puede impedir que la consola abra"
        );
    }

    /// El camino de error entero: sin directorio la consola **arranca igual** —una consola sin panel
    /// sigue siendo un terminal completo— y su programa no ve ninguna de las dos variables.
    ///
    /// Se mira lo que llega por el puente y **no la rejilla**, porque `env` escribe más líneas que las
    /// 24 de la rejilla inicial y lo que se va por arriba no se guarda en ninguna parte. Comprobado
    /// mutando el lanzador para que fijara las dos variables sin directorio: con la aserción sobre la
    /// rejilla el test seguía en verde, porque los dos nombres habían salido de la pantalla.
    #[test]
    fn una_consola_sin_directorio_arranca_igual_y_su_programa_no_ve_la_identidad() {
        let mut division = Division::con_panel(&egui::Context::default(), "env", None)
            .expect("una consola sin panel tiene que abrir igual");
        esperar_a_que_muera(&division);

        let mut recibido = Vec::new();
        loop {
            let queda_cola = division.bombear();
            recibido.extend_from_slice(&division.recibido);
            if !queda_cola {
                break;
            }
        }
        let salida = String::from_utf8_lossy(&recibido);

        for nombre in panel_contract::ENV_NAMES {
            assert!(
                !salida.contains(&format!("{nombre}=")),
                "el programa vio {nombre} sin que su consola tenga directorio:\n{salida}"
            );
        }
    }

    #[test]
    fn al_cerrar_la_ventana_no_quedan_directorios_de_panel() {
        let rutas = {
            let ventana = Ventana::nueva(&egui::Context::default())
                .expect("no se pudo abrir la ventana con sus dos consolas");

            ventana.activa().divisiones.each_ref().map(|division| {
                division
                    .panel
                    .as_ref()
                    .expect("la consola tiene que arrancar con su directorio")
                    .path()
                    .to_path_buf()
            })
        };

        for ruta in rutas {
            assert!(!ruta.exists(), "quedó el directorio {}", ruta.display());
        }
    }

    #[test]
    fn el_teclado_lo_enruta_la_ventana_a_la_del_foco_y_no_a_la_primera() {
        // AC2 sobre el enrutado que hace la aplicación, no sobre el que elige el test. El de arriba
        // indexa él mismo la consola destino, así que seguiría verde con el fallo que de verdad importa:
        // escribir en las dos, o en la izquierda fija. Con el foco en la derecha, los dos salen en rojo.
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.activa_mut().foco = 1;

        let ctx = egui::Context::default();
        ctx.input_mut(|entrada| entrada.events.push(egui::Event::Text("a".to_owned())));
        ventana.teclear_la_del_foco(&ctx);

        assert_eq!(
            ventana.activa().divisiones[1].pulsaciones,
            b"a",
            "la consola con el foco no recibió lo tecleado"
        );
        assert!(
            ventana.activa().divisiones[0].pulsaciones.is_empty(),
            "la tecla acabó también en la consola sin foco"
        );
    }

    #[test]
    fn que_muera_una_consola_no_cierra_la_ventana() {
        // AC8, y es el criterio que da sentido al hito: con dos agentes trabajando, que uno termine no
        // puede llevarse por delante la ventana del otro. La condición ingenua —«si una murió, cierra»—
        // es justo la que este test pone en rojo.
        let mut ventana = ventana_con("printf ya", "sleep 30");
        esperar_a_que_muera(&ventana.activa().divisiones[0]);

        let bombeo = ventana.bombear_todas();

        assert!(
            !ventana.activa().divisiones[0].puente.sigue_vivo(),
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
        for division in &ventana.activa().divisiones {
            esperar_a_que_muera(division);
        }

        let bombeo = ventana.bombear_todas();

        assert!(
            bombeo.todas_terminadas,
            "las dos murieron y la ventana no se cerró"
        );
        assert!(
            ventana.activa().divisiones[0]
                .pantalla
                .to_text()
                .contains("una")
        );
        assert!(
            ventana.activa().divisiones[1]
                .pantalla
                .to_text()
                .contains("otra")
        );
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
        for division in &ventana.activa().divisiones {
            esperar_a_que_muera(division);
        }

        let bombeo = ventana.bombear_todas();

        assert!(
            bombeo.alguna_dejo_cola,
            "el volcado cupo entero en un fotograma"
        );
        // La de al lado entregó lo suyo en ese mismo fotograma, sin esperar a que el volcado acabe.
        assert!(
            ventana.activa().divisiones[1]
                .pantalla
                .to_text()
                .contains("tranquila"),
            "la consola tranquila se quedó esperando detrás del volcado"
        );
        // Y lo retenido sigue acotado por consola: ninguna se pasa del presupuesto.
        for (cual, division) in ventana.activa().divisiones.iter().enumerate() {
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
            ventana.activa().foco,
            1,
            "el clic en la consola derecha no le dio el foco"
        );
    }

    #[test]
    fn arrastrar_el_divisor_mueve_la_fraccion_a_donde_esta_el_puntero() {
        // AC5, la mitad que se puede comprobar sin ventana: el gesto llega hasta la fracción. Se prueba con
        // un `Ui` de verdad porque lo que hay que comprobar es la traducción de un arrastre del escritorio,
        // y eso solo lo sabe egui.
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        let area = area_de(400.0 + GROSOR_DEL_DIVISOR);
        let reparto = repartir(area, FRACCION_INICIAL, ANCHO_DE_CELDA);
        let agarre = reparto.divisor.center();
        // **El puntero se pasa del borde derecho y luego vuelve**, y ese ida y vuelta es lo que hace que la
        // prueba distinga entre seguir al puntero y acumular su desplazamiento. Con un solo salto las dos
        // implementaciones dan casi lo mismo y la prueba pasaría con la que el doc dice haber descartado;
        // con el exceso de por medio, la que acumula pierde para siempre lo que el recorte se comió y acaba
        // en 0,245 en vez de 0,75.
        let fuera = egui::pos2(area.right() + 200.0, agarre.y);
        // Tres cuartos del ancho repartible: un sitio al que la fracción inicial no llega sola.
        let suelta = egui::pos2(area.left() + 300.0, agarre.y);

        let ctx = egui::Context::default();

        // **Tres pasadas para empezar a arrastrar, y las tres hacen falta.** egui resuelve la interacción
        // contra las regiones de la pasada anterior, así que en la primera no hay a quién dar nada. Y el
        // botón tiene que bajar en una pasada distinta del movimiento: el candidato a arrastrarse sale de
        // **dónde estaba el puntero al pulsar** (`interaction.rs:169-171`), así que un pulsar y mover en la
        // misma pasada busca al candidato en el destino, donde el divisor ya no está.
        pasada_de_arrastre(&mut ventana, &ctx, reparto.divisor, area, mover(agarre));
        pasada_de_arrastre(&mut ventana, &ctx, reparto.divisor, area, pulsar(agarre));
        pasada_de_arrastre(&mut ventana, &ctx, reparto.divisor, area, mover(fuera));
        assert_eq!(
            ventana.activa().fraccion,
            1.0,
            "pasarse del borde derecho no dejó la fracción pegada al tope"
        );

        let movio = pasada_de_arrastre(&mut ventana, &ctx, reparto.divisor, area, mover(suelta));

        assert!(movio, "el arrastre no movió la fracción");
        // Exacto y no aproximado: 300 de 400 repartibles es 0,75 sin resto, así que una tolerancia solo
        // serviría para dejar pasar una implementación que casi acierta.
        assert_eq!(
            ventana.activa().fraccion,
            0.75,
            "el divisor no siguió al puntero después de volver de fuera"
        );
    }

    #[test]
    fn un_arrastre_donde_no_hay_nada_que_repartir_deja_la_fraccion_como_estaba() {
        // Los dos bordes de `arrastrar_el_divisor`, que existen porque la fracción es **estado**: una vez
        // guardada, una fracción absurda no se corrige sola. Con el área más estrecha que el divisor, `util`
        // es negativo y la división cambiaría de signo; con el área sin número —el caso que la métrica ya
        // documenta mientras el escritorio no ha dicho cuánto mide la ventana— saldría un no-número, y ese sí
        // no se va nunca: la ventana dejaría de pintar y ni ensancharla la recuperaría.
        for area in [area_de(1.0), area_de(f32::NAN)] {
            let mut ventana = ventana_con("sleep 30", "sleep 30");
            let divisor = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(2.0, 300.0));
            let agarre = divisor.center();
            let ctx = egui::Context::default();

            pasada_de_arrastre(&mut ventana, &ctx, divisor, area, mover(agarre));
            pasada_de_arrastre(&mut ventana, &ctx, divisor, area, pulsar(agarre));
            // Al borde izquierdo, que es donde el numerador también se hace cero: es la única posición que
            // produce el cero partido por cero del que sale el no-número.
            pasada_de_arrastre(
                &mut ventana,
                &ctx,
                divisor,
                area,
                mover(egui::pos2(area.left(), agarre.y)),
            );

            assert_eq!(
                ventana.activa().fraccion,
                FRACCION_INICIAL,
                "con un área de {} puntos el arrastre movió la fracción",
                area.width()
            );
        }
    }

    #[test]
    fn cada_consola_mide_su_rejilla_por_su_propio_rectangulo_y_no_por_el_de_la_otra() {
        // **El eslabón que faltaba de AC5.** Que el arrastre mueva la fracción está probado, y que un cambio
        // de tamaño dispare las dos mitades también, pero entre las dos cosas hay un paso sin el cual el
        // arrastre no redimensionaría nada: es el pintado el que mide cada consola contra **su** rectángulo y
        // deja el resultado para que el fotograma siguiente lo aplique. Borrar esa línea no rompía ninguna
        // prueba, porque las dos mitades solo se probaban poniendo el tamaño a mano.
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        // Un cuarto y tres cuartos: con mitades, una consola midiendo el rectángulo de la otra daría el mismo
        // número y la prueba no vería nada.
        let reparto = repartir(area_de(802.0), 0.25, ANCHO_DE_CELDA);
        let rectangulos = [reparto.izquierda, reparto.derecha];

        let ctx = egui::Context::default();
        let mut esperado = [REJILLA_INICIAL; 2];
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let metrica = Metrica::medir(ui.ctx(), egui::FontId::monospace(CUERPO));
            for (cual, (division, rect)) in ventana
                .activa_mut()
                .divisiones
                .iter_mut()
                .zip(rectangulos)
                .enumerate()
            {
                Ventana::pintar_division(ui, division, rect, &Tema::default(), &metrica);
                esperado[cual] = metrica.celdas_en(rect.size());
            }
        })
        .drop_without_applying_deltas();

        assert_eq!(
            ventana.activa().divisiones[0].medido,
            esperado[0],
            "la consola izquierda no midió su propio rectángulo"
        );
        assert_eq!(
            ventana.activa().divisiones[1].medido,
            esperado[1],
            "la consola derecha no midió su propio rectángulo"
        );
        // Y la comprobación que hace que las dos de arriba no puedan ser ciertas por casualidad: con este
        // reparto los dos anchos **tienen** que salir distintos.
        assert_ne!(
            ventana.activa().divisiones[0].medido.cols,
            ventana.activa().divisiones[1].medido.cols,
            "las dos consolas midieron el mismo ancho con un reparto de un cuarto"
        );
    }

    /// Una pasada de interfaz que solo atiende el arrastre del divisor, y lo que este dijo.
    ///
    /// Es una función y no un cierre a propósito: un cierre que escriba en la ventana la tiene prestada
    /// mientras exista, y entonces no se puede mirar la fracción entre pasada y pasada — que es justo lo que
    /// hace falta para comprobar un arrastre de varios tramos.
    fn pasada_de_arrastre(
        ventana: &mut Ventana,
        ctx: &egui::Context,
        divisor: egui::Rect,
        area: egui::Rect,
        entrada: egui::RawInput,
    ) -> bool {
        let mut movio = false;
        ctx.run_ui(entrada, |ui| {
            movio = ventana.arrastrar_el_divisor(ui, divisor, area);
        })
        .drop_without_applying_deltas();
        movio
    }

    /// Una pasada de interfaz **entera**: la del rasgo de `eframe`, con la ventana del tamaño pedido.
    ///
    /// **`ui` sí se puede correr desde un test, y esto es lo que lo permite.** `eframe` publica un
    /// constructor de `Frame` vacío para sus propias pruebas —`Frame::_new_kittest`,
    /// `eframe-0.36.1/src/epi.rs:720`— y `ui` no toca el `Frame` para nada: quien lo mira es `logic`, por
    /// el consumo de CPU. El área sale de `screen_rect`, que es exactamente lo que `run_ui` le da al `Ui`
    /// raíz como `max_rect` (`egui-0.36.1/src/context.rs:801-807`), así que la pasada recorre el mismo
    /// camino que en la aplicación en vez de una imitación suya.
    ///
    /// **El guion bajo del nombre es una advertencia y se acepta a sabiendas**: es un nombre que `eframe`
    /// se reserva para su propio uso, así que puede desaparecer en una versión menor sin que eso cuente
    /// como ruptura. Se asume porque el precio de perderlo es un error de compilación **en las pruebas**,
    /// nunca en lo que se entrega, y lo que compra es lo único que prueba que las piezas están conectadas:
    /// sin esto, cada decisión estaba probada por separado y la línea que las une, con nada.
    ///
    /// La pasada corre **una sola vez** por llamada: `run_ui` repite solo si alguien pide descartar
    /// (`context.rs:863-865`), y aquí nadie lo pide. Importa porque los eventos del teclado se consumen:
    /// una segunda vuelta no vería la tecla.
    fn pasada_de_interfaz(
        ventana: &mut Ventana,
        ctx: &egui::Context,
        area: egui::Rect,
        mut entrada: egui::RawInput,
    ) {
        entrada.screen_rect = Some(area);
        let mut frame = eframe::Frame::_new_kittest();
        ctx.run_ui(entrada, |ui| eframe::App::ui(ventana, ui, &mut frame))
            .drop_without_applying_deltas();
    }

    /// La entrada de un puntero que se mueve a un sitio.
    fn mover(a: egui::Pos2) -> egui::RawInput {
        egui::RawInput {
            events: vec![egui::Event::PointerMoved(a)],
            ..Default::default()
        }
    }

    /// La entrada de un botón que baja, sin soltarse.
    fn pulsar(en: egui::Pos2) -> egui::RawInput {
        egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: en,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            }],
            ..Default::default()
        }
    }

    /// Lo que mide una celda de ancho en los tests del reparto.
    ///
    /// En la aplicación este número lo da la métrica de la fuente; aquí se fija para que el suelo por
    /// consola sea un número redondo y los fallos se lean sin calcular nada.
    const ANCHO_DE_CELDA: f32 = 8.0;

    /// Un área de la anchura pedida, con la altura que haga falta para que sea un rectángulo válido.
    fn area_de(ancho: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(ancho, 300.0))
    }

    /// Monta una división con su directorio de verdad y un `panel.json` ya publicado.
    ///
    /// Hace falta el directorio real y no un doble: lo que estas pruebas comprueban es la cadena
    /// entera —el fichero en disco, la lectura del contrato y el reparto que sale de ella— y con un
    /// panel puesto a mano se probaría el reparto contra un estado que nadie ha leído nunca.
    fn division_con_panel(contenido: &str) -> Division {
        let mut division = division_con("sleep 30");
        division.panel = Some(PanelDir::create().expect("crear el directorio de la división"));
        publicar_en(&division, contenido);
        division.refrescar_su_panel();
        division
    }

    /// Publica un contenido en el `panel.json` de una división que ya tiene directorio.
    fn publicar_en(division: &Division, contenido: &str) {
        std::fs::write(
            division
                .panel
                .as_ref()
                .expect("la división tiene directorio")
                .path()
                .join(panel_contract::FICHERO_DEL_PANEL),
            contenido,
        )
        .expect("publicar el panel");
    }

    /// Corre una pasada de interfaz que pinta esta división, que es donde ocurre el refresco.
    fn un_fotograma_sobre(division: &mut Division) {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let metrica = Metrica::medir(ui.ctx(), egui::FontId::monospace(CUERPO));
            Ventana::pintar_division(ui, division, area_de(800.0), &Tema::default(), &metrica);
        })
        .drop_without_applying_deltas();
    }

    /// Lo que se está enseñando en el panel de esta división.
    fn lo_que_se_ve(division: &Division) -> &str {
        &division
            .estado_del_panel
            .a_pintar()
            .expect("hay panel que pintar")
            .lines[0]
            .text
    }

    /// Espera a que el aviso del sistema de ficheros llegue de verdad hasta la bandera.
    ///
    /// El límite es generoso —cinco segundos— porque lo que mide no es cuánto tarda el núcleo sino si
    /// llega: en la práctica llega en milisegundos.
    fn esperar_la_bandera(bandera: &Bandera, si_no_llega: &str) {
        let limite = Instant::now() + Duration::from_secs(5);
        while !bandera.recoger() {
            assert!(Instant::now() < limite, "{si_no_llega}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// **Que publicar saque a la ventana del reposo**, que es la mitad del hito que no se ve en ninguna
    /// bandera. El vigilante recibe *cómo* despertar, y quien se lo da es la ventana: cambiar ahí ese
    /// «cómo» por una función que no hace nada deja todas las banderas funcionando —y la suite en
    /// verde— mientras la ventana dormida no vuelve a pintar jamás, que es justo el caso que este PR
    /// viene a resolver.
    ///
    /// Las consolas son `sleep 30` y su puente no despierta a nadie, así que el único que puede sacar
    /// al contexto del reposo es el vigilante.
    #[test]
    fn publicar_saca_a_la_ventana_del_reposo() {
        let ctx = egui::Context::default();
        let mut divisiones = [division_con("sleep 30"), division_con("sleep 30")];
        for division in &mut divisiones {
            division.panel = Some(PanelDir::create().expect("crear el directorio"));
        }
        let _vigilante = Ventana::vigilar(&ctx, &divisiones);
        dormir_la_ventana(&ctx);

        publicar_en(&divisiones[0], r#"{"lines":["despierta"]}"#);

        let limite = Instant::now() + Duration::from_secs(5);
        while !ctx.has_requested_repaint() {
            assert!(
                Instant::now() < limite,
                "se publicó y nadie pidió fotograma: con la ventana en reposo el panel no se movería"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Deja el contexto sin repintados pendientes, que es como está una ventana en reposo.
    ///
    /// Un contexto recién hecho pide fotograma por su cuenta —tiene su propia interfaz que componer—,
    /// así que sin esto la aserción de que el aviso lo despierta pasaría igual sin aviso.
    fn dormir_la_ventana(ctx: &egui::Context) {
        for _ in 0..4 {
            ctx.run_ui(egui::RawInput::default(), |_| {})
                .drop_without_applying_deltas();
        }
        assert!(
            !ctx.has_requested_repaint(),
            "el contexto de la prueba no llega a estar en reposo"
        );
    }

    /// **Que la pasada de interfaz relee de verdad.** Sin esto, quitar la llamada al refresco dejaba la
    /// suite entera en verde: las pruebas del lector afirmaban sobre el lector, y nadie ataba que el
    /// fotograma lo llamara. Un panel publicado después de abrir la consola no aparecería jamás.
    #[test]
    fn el_fotograma_recoge_un_panel_publicado_despues_de_abrir_la_consola() {
        let mut division = division_con("sleep 30");
        division.panel = Some(PanelDir::create().expect("crear el directorio"));
        assert!(
            division.estado_del_panel.a_pintar().is_none(),
            "todavía no hay nada publicado"
        );

        publicar_en(&division, r#"{"lines":["llegó tarde"]}"#);
        un_fotograma_sobre(&mut division);

        assert_eq!(
            lo_que_se_ve(&division),
            "llegó tarde",
            "el fotograma tenía que haberlo recogido"
        );
    }

    /// Lo que la ventana pide esperar antes del próximo fotograma, en la pasada entera.
    ///
    /// **Se afirma sobre el número y no sobre «¿ha pedido repintar?»**, porque ese booleano lo satisface
    /// cualquier cosa —una animación, una fuente que se carga— y no distingue «se puso el despertador de
    /// este panel» de «alguien pidió pintar por otro motivo».
    fn lo_que_la_ventana_espera(ventana: &mut Ventana, ctx: &egui::Context) -> Duration {
        // **`logic` y `ui` dentro de la misma pasada, que es como corren de verdad.** Llamando solo a
        // `logic` se mide el despertador contra el estado de *antes* de releer los paneles, y ese es
        // justo el fallo que esta prueba existe para cazar.
        let mut frame = eframe::Frame::_new_kittest();
        let entrada = egui::RawInput {
            screen_rect: Some(area_de(800.0)),
            ..Default::default()
        };

        eframe::App::logic(ventana, ctx, &mut frame);
        let salida = ctx.run_ui(entrada, |ui| eframe::App::ui(ventana, ui, &mut frame));
        let espera = salida.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        salida.drop_without_applying_deltas();

        espera
    }

    /// **El latido, desde el camino real y con la pasada entera.** El vigilante ve lo que cambia; esto
    /// ve lo que *deja* de cambiar, y son cosas distintas: un productor que se muere no escribe nada,
    /// así que no hay evento de fichero que lo delate. Sin este despertador el aviso de «esto es viejo»
    /// no aparecería jamás en una ventana en reposo, que es justo cuando hace falta.
    ///
    /// **El fotograma en que el panel se lee por primera vez es el caso difícil**, y es el que aquí se
    /// prueba: los paneles se releen en la pasada de interfaz, que corre después de la mitad lógica, así
    /// que un despertador puesto solo en la primera mitad se calcularía con un estado en el que todavía
    /// no hay panel — y la ventana se dormiría para siempre con el productor ya muerto.
    ///
    /// Las dos mitades van juntas a propósito: sin la negativa, la aserción de arriba pasaría también
    /// con un despertador puesto siempre, que sería una ventana que no duerme nunca.
    #[test]
    fn la_ventana_se_despierta_para_avisar_de_que_un_panel_envejece_y_no_si_no_hay_ninguno() {
        let ctx = egui::Context::default();
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        // Con su directorio y sin publicar: es la raya que el hito promete —el fichero que todavía no
        // existe no es un aviso— y aquí además comprueba que no se programa nada por él.
        for division in &mut ventana.activa_mut().divisiones {
            division.panel = Some(PanelDir::create().expect("crear el directorio"));
        }

        // Unas cuantas pasadas primero: un contexto recién hecho pide fotograma inmediato por su
        // cuenta —tiene su propia interfaz que componer— y sin dejarlo asentarse la mitad negativa no
        // mediría nada.
        let mut sin_nada = Duration::ZERO;
        for _ in 0..4 {
            sin_nada = lo_que_la_ventana_espera(&mut ventana, &ctx);
        }
        assert_eq!(
            sin_nada,
            Duration::MAX,
            "sin ningún panel publicado la ventana se pone un despertador que no tiene nada que hacer"
        );

        publicar_en(
            &ventana.activa().divisiones[0],
            r#"{"stale_after_minutes":10,"lines":["algo"]}"#,
        );
        ventana.activa().divisiones[0].por_releer.levantar();
        let con_panel = lo_que_la_ventana_espera(&mut ventana, &ctx);

        assert!(
            con_panel <= Duration::from_secs(10 * 60),
            "el panel envejece en diez minutos y la ventana espera {con_panel:?} antes de mirar"
        );
        assert!(
            con_panel > Duration::from_secs(9 * 60),
            "se despierta a los {con_panel:?} para un panel que aguanta diez minutos"
        );
    }

    /// **El latido también en la mitad lógica, que es la única que corre con la ventana tapada.**
    /// Leído en la fuente: con la ventana minimizada u ocluida el framework llama a su vuelta lógica,
    /// que **empieza poniendo el retardo de repintado en el máximo** y después ejecuta solo la mitad
    /// lógica de la aplicación — no hay pasada de interfaz que rearme nada. Sin el despertador ahí,
    /// cada vuelta de ventana tapada borra la alarma y la ventana se duerme para siempre con el
    /// productor ya muerto. La prueba de al lado no lo ve: corre las dos mitades seguidas, y la
    /// segunda arma la alarma igual.
    #[test]
    fn con_la_ventana_tapada_el_latido_se_sigue_armando() {
        let ctx = egui::Context::default();
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        for division in &mut ventana.activa_mut().divisiones {
            division.panel = Some(PanelDir::create().expect("crear el directorio"));
        }
        publicar_en(
            &ventana.activa().divisiones[0],
            r#"{"stale_after_minutes":10,"lines":["algo"]}"#,
        );
        ventana.activa().divisiones[0].por_releer.levantar();
        // Una pasada entera primero, que es donde el panel se lee.
        lo_que_la_ventana_espera(&mut ventana, &ctx);

        // Vueltas de solo lógica en vacío hasta dejar el contexto en reposo: sin esto, la aserción de
        // abajo la cumpliría cualquier repintado pendiente de antes.
        for _ in 0..4 {
            let _ = ctx.run_logic(&egui::RawInput::default(), |_| {});
        }
        assert!(
            !ctx.has_requested_repaint(),
            "el contexto no llega a estar en reposo, causas: {:?}",
            ctx.repaint_causes()
        );

        let mut frame = eframe::Frame::_new_kittest();
        let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            eframe::App::logic(&mut ventana, ctx, &mut frame);
        });

        assert!(
            ctx.has_requested_repaint(),
            "con la ventana tapada nadie rearma el despertador: el aviso de rancio no llega nunca"
        );
    }

    /// **La economía que este PR entrega, y la mitad que se rompería sin ruido.** Hasta ahora cada
    /// consola le preguntaba al disco por la fecha de su fichero en cada fotograma —sesenta veces por
    /// segundo mientras haya volcado—; a partir de aquí solo mira cuando le avisan. Sin esta prueba,
    /// quitar la bandera de en medio dejaría la suite entera en verde, porque leer de más se ve igual
    /// que leer a tiempo.
    #[test]
    fn sin_aviso_el_fotograma_no_vuelve_a_mirar_el_fichero() {
        let mut division = division_con_panel(r#"{"lines":["lo primero"]}"#);

        publicar_en(&division, r#"{"lines":["lo segundo"]}"#);
        un_fotograma_sobre(&mut division);

        assert_eq!(
            lo_que_se_ve(&division),
            "lo primero",
            "el fotograma leyó el fichero sin que nadie le hubiera avisado"
        );
    }

    /// Y la otra mitad: la bandera es una puerta, no un muro. Con ella levantada el fotograma relee, y
    /// eso es lo que ata el trabajo del vigilante con lo que se acaba viendo.
    #[test]
    fn con_la_bandera_levantada_el_fotograma_vuelve_a_leer() {
        let mut division = division_con_panel(r#"{"lines":["lo primero"]}"#);

        publicar_en(&division, r#"{"lines":["lo segundo"]}"#);
        division.por_releer.levantar();
        un_fotograma_sobre(&mut division);

        assert_eq!(
            lo_que_se_ve(&division),
            "lo segundo",
            "avisaron de que había cambiado y el fotograma siguió enseñando lo viejo"
        );
    }

    /// **El respaldo, y no es prudencia.** Cuando se pierde el seguimiento —el usuario borra el
    /// directorio desde su propia consola, o el vigilante entrega un fallo— nadie levanta ninguna
    /// bandera. Sin esto, este PR *empeoraría* lo que había: hasta ahora el panel se releía de más y
    /// nunca se quedaba atrás; con un vigilante ciego se quedaría clavado para siempre y en silencio.
    ///
    /// La ruta que no se puede vigilar cuelga de un directorio recién creado **a propósito**: una ruta
    /// fija dentro del temporal del sistema sería una apuesta a que nadie la haya creado nunca en esta
    /// máquina, y el día que alguien lo hiciera esta prueba fallaría para siempre señalando al código.
    #[test]
    fn sin_vigilante_la_ventana_vuelve_a_mirar_en_cada_fotograma() {
        let ctx = egui::Context::default();
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        let dir = PanelDir::create().expect("crear el directorio");
        let sin_armar = dir.path().join("no-existe");
        ventana.vigilante = Vigilante::nuevo(|| {}, &[(sin_armar.as_path(), Bandera::levantada())]);
        assert!(ventana.vigilante.esta_ciego(), "el respaldo no está activo");
        for division in &ventana.activa().divisiones {
            division.por_releer.recoger();
        }
        // **Dormirla primero, o la aserción de abajo es vacía**: un contexto recién hecho ya pide
        // repintado por su cuenta, así que sin esto pasaría igual con el respaldo sin pedir nada.
        dormir_la_ventana(&ctx);

        eframe::App::logic(&mut ventana, &ctx, &mut eframe::Frame::_new_kittest());

        for (cual, division) in ventana.activa().divisiones.iter().enumerate() {
            assert!(
                division.por_releer.recoger(),
                "sin nadie que avise, el panel de la consola {cual} se queda congelado"
            );
        }
        assert!(
            ctx.has_requested_repaint(),
            "el respaldo mira «en cada fotograma» y nadie pide ninguno: con la consola callada no \
             habría vuelta donde recogerlo"
        );
    }

    /// **Que el tramo del panel se declare desde el camino real, y no solo cuando se anota a mano.**
    /// Dejarlo en cero dentro de la pasada de interfaz no rompía nada, y ese tiempo acabaría publicado
    /// como coste del framework — el número que decide si egui aguanta.
    #[test]
    fn la_pasada_de_interfaz_declara_lo_que_costo_el_panel() {
        let mut division = division_con_panel(r#"{"lines":["algo publicado"]}"#);
        // Con salida en este fotograma, que es cuando las series se anotan: sin ella hay pintado que
        // declarar pero no hay *vuelta* que medir, y eso ya tiene su propia prueba en la medida.
        let ahora = Instant::now();
        division.pendiente_de_medir = Some((Some(ahora), ahora, Duration::from_millis(1)));

        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let metrica = Metrica::medir(ui.ctx(), egui::FontId::monospace(CUERPO));
            Ventana::pintar_division(
                ui,
                &mut division,
                area_de(800.0),
                &Tema::default(),
                &metrica,
            );
        })
        .drop_without_applying_deltas();
        division.medida.cerrar_fotograma(Duration::from_millis(10));

        let anotado = division
            .medida
            .panel_anotado()
            .expect("el tramo del panel tiene que llegar a la serie");
        assert!(
            anotado > Duration::ZERO,
            "se declaró un cero: el coste del panel se lo queda el framework por resta"
        );
    }

    /// **La garantía de este PR.** Una consola cuyo productor no ha publicado nada se ve exactamente
    /// como antes de que existiera el panel: sin ancho descontado y sin segundo rectángulo.
    #[test]
    fn una_consola_sin_panel_publicado_se_queda_con_todo_su_rectangulo() {
        let division = division_con("sleep 30");
        let entera = area_de(800.0);

        let hueco = division.repartir_con_su_panel(entera, ANCHO_DE_CELDA, SystemTime::now());

        assert_eq!(hueco.consola, entera, "sin panel no se descuenta nada");
        assert!(hueco.panel.is_none());
    }

    /// **El criterio que hace que el descuento sirva de algo.** Que el rectángulo se parta no basta: si
    /// la rejilla se midiera contra la división entera, el programa de dentro creería tener columnas
    /// que el panel le ha quitado y escribiría debajo de él.
    #[test]
    fn la_rejilla_de_una_consola_con_panel_mide_solo_el_hueco_que_le_queda() {
        let mut division = division_con_panel(r#"{"lines":["algo publicado"]}"#);
        let entera = area_de(800.0);

        let ctx = egui::Context::default();
        let (mut con_panel, mut sin_descontar) = (REJILLA_INICIAL, REJILLA_INICIAL);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let metrica = Metrica::medir(ui.ctx(), egui::FontId::monospace(CUERPO));
            Ventana::pintar_division(ui, &mut division, entera, &Tema::default(), &metrica);
            con_panel = division.medido;
            sin_descontar = metrica.celdas_en(entera.size());
        })
        .drop_without_applying_deltas();

        assert!(
            con_panel.cols < sin_descontar.cols,
            "midió {} columnas, las mismas que sin panel",
            con_panel.cols
        );
        assert_eq!(
            con_panel.rows, sin_descontar.rows,
            "el panel quita ancho, no alto"
        );
    }

    /// **El paso que une el reparto con el pintado, y que no ataba nada.** El reparto tiene sus pruebas
    /// y lo que se pinta las suyas, pero entre los dos está pasarle al panel **el rectángulo que le
    /// tocó**: con el de la consola, el panel se pintaría encima del programa —tapándolo con su fondo,
    /// que es opaco— y la suite seguía verde porque el texto y los colores salen iguales.
    #[test]
    fn el_panel_se_pinta_en_su_franja_y_no_encima_de_la_consola() {
        let mut division = division_con_panel(r#"{"header":false,"lines":["algo publicado"]}"#);
        let entera = area_de(800.0);

        let ctx = egui::Context::default();
        let mut corte = 0.0;
        let mut salida = ctx.run_ui(egui::RawInput::default(), |ui| {
            let metrica = Metrica::medir(ui.ctx(), egui::FontId::monospace(CUERPO));
            Ventana::pintar_division(ui, &mut division, entera, &Tema::default(), &metrica);
            corte = division
                .repartir_con_su_panel(entera, metrica.ancho(), SystemTime::now())
                .consola
                .right();
        });
        let formas = std::mem::take(&mut salida.shapes);
        salida.drop_without_applying_deltas();

        let donde = formas
            .iter()
            .find_map(|recortada| match &recortada.shape {
                egui::Shape::Text(texto) if texto.galley.text().contains("algo publicado") => {
                    Some(texto.pos)
                }
                _ => None,
            })
            .expect("la línea del panel tenía que quedar encolada");
        assert!(
            donde.x >= corte,
            "el panel se pintó dentro de la consola: {} está a la izquierda de {corte}",
            donde.x
        );
    }

    /// **El caso normal con dos consolas: una con su productor publicando y la otra esperando en su
    /// prompt** — y la de al lado tiene su directorio hecho, que es como nace toda consola. Ahí está lo
    /// que las otras pruebas no tocan: las que comprueban que sin panel no se descuenta nada usan una
    /// división **sin directorio**, que en la aplicación solo pasa cuando crearlo falló. Lo que decide
    /// si se cede ancho es que el productor **haya publicado**, no que exista el sitio donde publicar;
    /// con lo segundo, toda consola perdería sus treinta y seis columnas desde el arranque.
    #[test]
    fn una_consola_cede_ancho_cuando_su_productor_publica_y_no_por_tener_donde() {
        // La mitad justa para cada una, que es el reparto de arranque: así la única diferencia entre
        // las dos medidas es el panel.
        let area = area_de(1200.0);

        // La referencia es la ventana de antes de este hito: ninguna consola con nada.
        let mut ninguna = ventana_con("sleep 30", "sleep 30");
        pasada_de_interfaz(
            &mut ninguna,
            &egui::Context::default(),
            area,
            egui::RawInput::default(),
        );
        let como_antes = ninguna.activa().divisiones[1].medido;

        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.activa_mut().divisiones[0] = division_con_panel(r#"{"lines":["algo publicado"]}"#);
        ventana.activa_mut().divisiones[1].panel =
            Some(PanelDir::create().expect("crear el directorio"));
        pasada_de_interfaz(
            &mut ventana,
            &egui::Context::default(),
            area,
            egui::RawInput::default(),
        );

        assert!(
            ventana.activa().divisiones[0].medido.cols < como_antes.cols,
            "la consola cuyo productor publicó no cedió ancho: midió {} columnas",
            ventana.activa().divisiones[0].medido.cols
        );
        assert_eq!(
            ventana.activa().divisiones[1].medido,
            como_antes,
            "la consola con el directorio vacío se estrechó sin que nadie hubiera publicado"
        );
    }

    #[test]
    fn el_reparto_conserva_la_proporcion_al_cambiar_el_ancho_de_la_ventana() {
        // AC6. Es la razón por la que el área se reparte a mano en vez de con los paneles
        // redimensionables de egui: aquellos trabajan en puntos, así que al ensanchar la ventana el
        // panel mantendría su ancho y toda la diferencia caería en el otro lado.
        //
        // **Se prueba con un cuarto y no con la mitad**: con la mitad, un reparto que ignorase la
        // fracción por completo daría exactamente el mismo resultado y el test pasaría igual.
        const UN_CUARTO: f32 = 0.25;
        let estrecha = repartir(
            area_de(400.0 + GROSOR_DEL_DIVISOR),
            UN_CUARTO,
            ANCHO_DE_CELDA,
        );
        let ancha = repartir(
            area_de(800.0 + GROSOR_DEL_DIVISOR),
            UN_CUARTO,
            ANCHO_DE_CELDA,
        );

        assert_eq!(estrecha.izquierda.width(), 100.0);
        assert_eq!(estrecha.derecha.width(), 300.0);
        // Al doblar el ancho de la ventana, los dos anchos se doblan: la diferencia se reparte, no cae
        // entera en un lado.
        assert_eq!(ancha.izquierda.width(), 200.0);
        assert_eq!(ancha.derecha.width(), 600.0);
    }

    #[test]
    fn arrastrar_el_divisor_hasta_el_borde_no_deja_a_ninguna_consola_sin_ancho() {
        // El suelo por consola: arrastrado hasta el borde, el lado que se colapsa se queda en un ancho de
        // celda y no en cero, así que sigue mostrando su columna.
        //
        // Lo que esto **no** evita, y conviene no atribuírselo: el pánico de `Screen::resize` con cero
        // columnas —de eso se encarga el mínimo de 1×1 de la métrica— ni que la consola desaparezca del
        // todo, porque el asa es el divisor y sigue en pantalla con la fracción a cero.
        const UTIL: f32 = 500.0 - GROSOR_DEL_DIVISOR;
        // **Los dos anchos exactos y ninguna desigualdad.** Un `>=` contra el suelo lo cumple también el
        // lado ancho, que mide sesenta veces más, así que la aserción pasaría sin que el suelo existiera.
        for (fraccion, izquierda, derecha) in [
            (0.0_f32, ANCHO_DE_CELDA, UTIL - ANCHO_DE_CELDA),
            (1.0_f32, UTIL - ANCHO_DE_CELDA, ANCHO_DE_CELDA),
        ] {
            let reparto = repartir(area_de(500.0), fraccion, ANCHO_DE_CELDA);

            assert_eq!(
                reparto.izquierda.width(),
                izquierda,
                "con la fracción a {fraccion} la consola izquierda no respetó el suelo"
            );
            assert_eq!(
                reparto.derecha.width(),
                derecha,
                "con la fracción a {fraccion} la consola derecha no respetó el suelo"
            );
        }
    }

    #[test]
    fn cuando_no_caben_dos_anchos_minimos_el_divisor_se_planta_en_el_centro() {
        // El caso en que el suelo no se puede respetar, porque no hay sitio para dos. Recortar contra un
        // mínimo que no cabe daría un mínimo mayor que el máximo, o sea un rectángulo invertido. Lo que se
        // conserva es lo único que se puede: que ninguna de las dos desaparezca.
        let util = ANCHO_DE_CELDA * 2.0 - 1.0;
        let reparto = repartir(area_de(util + GROSOR_DEL_DIVISOR), 0.0, ANCHO_DE_CELDA);

        assert_eq!(reparto.izquierda.width(), reparto.derecha.width());
        assert!(reparto.izquierda.width() > 0.0, "la izquierda desapareció");
        assert!(reparto.derecha.width() > 0.0, "la derecha desapareció");
    }

    #[test]
    fn el_divisor_se_descuenta_antes_de_repartir_y_no_deja_huecos() {
        // Las tres piezas tienen que cubrir el área exactamente: si el divisor se restara después de
        // repartir, una consola perdería ancho sin que nadie hubiera movido nada.
        let area = egui::Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(500.0, 300.0));
        let reparto = repartir(area, 0.4, ANCHO_DE_CELDA);

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
        // Y se prueba con las fracciones de los dos extremos además de una del medio: el recorte contra el
        // suelo es justo donde un mínimo mayor que el máximo aparecería.
        for ancho in [0.0_f32, 1.0, 2.0, 3.0, 20.0] {
            for fraccion in [0.0_f32, 0.5, 1.0] {
                let reparto = repartir(area_de(ancho), fraccion, ANCHO_DE_CELDA);
                for (nombre, rect) in [
                    ("izquierda", reparto.izquierda),
                    ("divisor", reparto.divisor),
                    ("derecha", reparto.derecha),
                ] {
                    assert!(
                        rect.width() >= 0.0,
                        "con {ancho} puntos de ancho y la fracción a {fraccion}, \
                         el rectángulo de la {nombre} salió invertido"
                    );
                }
            }
        }
    }

    #[test]
    fn la_pasada_de_interfaz_reparte_la_ventana_con_la_fraccion_del_estado() {
        // **El cableado de la pasada de interfaz**, que es lo que une las piezas y hasta ahora no lo
        // probaba nada: repartir con `FRACCION_INICIAL` en vez de con la fracción del estado —o sea, un
        // arrastre sin ningún efecto visible— dejaba la suite entera en verde. Cada pieza estaba probada
        // por separado; lo que faltaba era la línea que las junta.
        //
        // **Y con esto AC6 se recorre entero.** La prueba de `repartir` con dos anchos comprueba el
        // mecanismo, pero no quién le pasa el área ni con qué fracción; aquí la ventana se ensancha al
        // doble entre dos pasadas de la aplicación de verdad y lo que se mira es la rejilla con la que
        // acabó cada consola, que es lo que el programa de dentro va a recibir.
        const UN_CUARTO: f32 = 0.25;
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.activa_mut().fraccion = UN_CUARTO;
        let ctx = egui::Context::default();

        for ancho in [400.0 + GROSOR_DEL_DIVISOR, 800.0 + GROSOR_DEL_DIVISOR] {
            let area = area_de(ancho);
            pasada_de_interfaz(&mut ventana, &ctx, area, egui::RawInput::default());

            // La métrica se le pregunta a la misma fuente que usó la pasada, y se puede preguntar desde
            // fuera de ella porque las fuentes existen desde la primera (`context.rs:1120`).
            let metrica = Metrica::medir(&ctx, egui::FontId::monospace(CUERPO));
            let reparto = repartir(area, UN_CUARTO, metrica.ancho());

            assert_eq!(
                ventana.activa().divisiones[0].medido,
                metrica.celdas_en(reparto.izquierda.size()),
                "con la ventana a {ancho} puntos, la consola izquierda no midió su cuarto"
            );
            assert_eq!(
                ventana.activa().divisiones[1].medido,
                metrica.celdas_en(reparto.derecha.size()),
                "con la ventana a {ancho} puntos, la consola derecha no midió sus tres cuartos"
            );
            // Un cuarto y no la mitad, por lo mismo que en la prueba de `repartir`: con la mitad, una
            // pasada que ignorase la fracción daría estos mismos dos números.
            assert_ne!(
                ventana.activa().divisiones[0].medido.cols,
                ventana.activa().divisiones[1].medido.cols,
                "las dos consolas midieron el mismo ancho con un reparto de un cuarto"
            );
        }

        // La fracción sigue siendo la que se pidió: **redimensionar la ventana no la toca**. Es la otra
        // mitad de AC6 —«la fracción no se rompe sola»— y es lo que se vería roto si algún día la pasada
        // guardara aquí lo que cupo en vez de lo que se pidió.
        assert_eq!(
            ventana.activa().fraccion,
            UN_CUARTO,
            "ensanchar la ventana movió la fracción"
        );
    }

    #[test]
    fn la_rejilla_de_cada_consola_acaba_siendo_la_que_midio_su_trozo_de_ventana() {
        // **El último eslabón de AC6, y estaba sin probar.** Que la proporción se conserve no sirve de nada
        // si no llega hasta la rejilla contra la que se interpretan los bytes: la pasada de interfaz mide,
        // y es `logic` quien lo aplica en el fotograma siguiente. Esa unión no la miraba nada — las dos
        // mitades del redimensionado se prueban poniendo `medido` a mano, y las pruebas de `bombear_todas`
        // corren con la rejilla de arranque, donde redimensionar es un no-op—, así que borrar
        // `division.redimensionar()` del bucle, o el bucle entero de `logic`, dejaba la suite en verde con
        // las dos consolas clavadas en 24×80 por grande que fuera la ventana.
        const UN_CUARTO: f32 = 0.25;
        let area = area_de(800.0 + GROSOR_DEL_DIVISOR);
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.activa_mut().fraccion = UN_CUARTO;
        let ctx = egui::Context::default();
        // El `Frame` de pruebas de `eframe` viene sin consumo de CPU (`cpu_usage: None`, `epi.rs:931`), así
        // que `logic` se salta el cierre del fotograma anterior: aquí lo que se mira es el bombeo.
        let mut frame = eframe::Frame::_new_kittest();

        pasada_de_interfaz(&mut ventana, &ctx, area, egui::RawInput::default());
        eframe::App::logic(&mut ventana, &ctx, &mut frame);

        let metrica = Metrica::medir(&ctx, egui::FontId::monospace(CUERPO));
        let reparto = repartir(area, UN_CUARTO, metrica.ancho());
        let rectangulos = [reparto.izquierda, reparto.derecha];
        for (cual, (division, rect)) in ventana
            .activa()
            .divisiones
            .iter()
            .zip(rectangulos)
            .enumerate()
        {
            let esperado = metrica.celdas_en(rect.size());
            // Que el tamaño esperado **no** sea el de arranque es lo que hace que la aserción de abajo
            // signifique algo: si coincidieran, pasaría igual con un redimensionado que no ocurrió.
            assert_ne!(
                esperado, REJILLA_INICIAL,
                "el trozo de ventana de la consola {cual} mide justo la rejilla de arranque"
            );
            assert_eq!(
                division.pantalla.size(),
                esperado,
                "la consola {cual} interpreta contra una rejilla que no es la de su trozo de ventana"
            );
        }
    }

    #[test]
    fn la_pasada_de_interfaz_atiende_el_clic_con_los_rectangulos_del_reparto() {
        // La tercera línea del cableado: que la pasada **llame** al clic, y que le pase los rectángulos
        // que salieron del reparto. Borrar esa llamada dejaba la suite en verde —el foco por clic se prueba
        // con rectángulos escritos a mano— y el foco quedaba clavado en la izquierda para siempre.
        //
        // **El punto donde se pulsa es el que hace la prueba exacta**: con la fracción a un cuarto, 300
        // cae en la consola derecha; con la mitad —la fracción que la pasada usaría si ignorase el
        // estado— cae en la izquierda y el foco no se movería. Así una sola aserción cubre las dos cosas.
        const UN_CUARTO: f32 = 0.25;
        let area = area_de(800.0 + GROSOR_DEL_DIVISOR);
        let dentro_de_la_derecha = egui::pos2(area.left() + 300.0, area.center().y);
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.activa_mut().fraccion = UN_CUARTO;
        let ctx = egui::Context::default();

        // Dos pasadas, como en la prueba de `atender_clics`: egui resuelve la interacción contra las
        // regiones de la pasada anterior, así que un clic en la primera no encontraría a quién dárselo.
        pasada_de_interfaz(&mut ventana, &ctx, area, egui::RawInput::default());
        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area,
            egui::RawInput {
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
            },
        );

        assert_eq!(
            ventana.activa().foco,
            1,
            "el clic en la consola derecha no le dio el foco dentro de la pasada de interfaz"
        );
    }

    #[test]
    fn la_pasada_de_interfaz_manda_lo_tecleado_a_la_consola_con_el_foco() {
        // La otra línea del cableado que nadie miraba: que la pasada **llame** al teclado. El enrutado se
        // prueba llamándolo a mano, así que borrar esa llamada de `ui` deja la suite en verde y la ventana
        // muda — no llegaría una sola tecla a ningún programa.
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.activa_mut().foco = 1;
        let ctx = egui::Context::default();

        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(800.0 + GROSOR_DEL_DIVISOR),
            egui::RawInput {
                events: vec![egui::Event::Text("a".to_owned())],
                ..Default::default()
            },
        );

        assert_eq!(
            ventana.activa().divisiones[1].pulsaciones,
            b"a",
            "la pasada de interfaz no mandó la tecla a la consola con el foco"
        );
    }

    #[test]
    fn el_realce_rodea_a_la_consola_con_el_foco_y_no_siempre_a_la_izquierda() {
        // **El único sitio donde el foco decide algo y no se ve en lo que recibe una consola.** Las
        // otras rutas del foco —el teclado, el clic— se comprueban mirando el destino: la consola que
        // no tenía que recibir nada sigue vacía. Ésta solo existe en lo que se encola para pintar, así
        // que un `rectangulos[0]` fijo, o un foco leído de otro sitio que el de la pestaña, deja la
        // suite entera en verde y el realce clavado en la izquierda para siempre: la ventana enseñaría
        // que se teclea en una consola mientras las teclas van a la otra.
        let area = area_de(800.0 + GROSOR_DEL_DIVISOR);
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        ventana.activa_mut().foco = 1;
        let ctx = egui::Context::default();

        // Como en la prueba del panel: la pasada de interfaz de arriba tira lo encolado, y aquí lo
        // encolado **es** lo que se afirma.
        let entrada = egui::RawInput {
            screen_rect: Some(area),
            ..Default::default()
        };
        let mut frame = eframe::Frame::_new_kittest();
        let mut salida = ctx.run_ui(entrada, |ui| eframe::App::ui(&mut ventana, ui, &mut frame));
        let formas = std::mem::take(&mut salida.shapes);
        salida.drop_without_applying_deltas();

        let realce = formas
            .iter()
            .find_map(|recortada| match &recortada.shape {
                egui::Shape::Rect(forma) if forma.stroke.color == Tema::default().foco() => {
                    Some(forma.rect)
                }
                _ => None,
            })
            .expect("el realce del foco tenía que quedar encolado");

        let metrica = Metrica::medir(&ctx, egui::FontId::monospace(CUERPO));
        let reparto = repartir(area, FRACCION_INICIAL, metrica.ancho());
        assert_eq!(
            realce, reparto.derecha,
            "el realce no rodeó a la consola con el foco"
        );
        // Y la comprobación que impide que la de arriba sea cierta por casualidad: las dos mitades del
        // reparto inicial miden lo mismo, así que lo único que las distingue es dónde empiezan.
        assert_ne!(
            reparto.izquierda, reparto.derecha,
            "con este reparto las dos consolas ocupan el mismo rectángulo y la prueba no dice nada"
        );
    }

    #[test]
    fn el_arrastre_redimensiona_las_consolas_en_el_mismo_pase_en_que_mueve_el_divisor() {
        // El reparto se hace **dos veces** en la misma pasada cuando el arrastre movió la fracción, y sin
        // esa segunda vez todo lo que viene después —el divisor pintado y la rejilla que mide cada
        // consola— se queda con el reparto de antes: un fotograma por detrás del ratón, que es justo lo
        // que se nota arrastrando. Quitar ese segundo reparto no rompía ninguna prueba, porque el arrastre
        // se probaba sin pintar y el pintado sin arrastrar.
        let area = area_de(800.0 + GROSOR_DEL_DIVISOR);
        // El centro del divisor con la fracción inicial: la mitad de lo repartible, donde el suelo por
        // consola no llega a morder, así que el agarre no depende del ancho de la fuente.
        let agarre = egui::pos2(
            area.left() + 400.0 + GROSOR_DEL_DIVISOR / 2.0,
            area.center().y,
        );
        // Tres cuartos de lo repartible: un sitio al que la fracción inicial no llega sola.
        let suelta = egui::pos2(area.left() + 600.0, agarre.y);
        let mut ventana = ventana_con("sleep 30", "sleep 30");
        let ctx = egui::Context::default();

        // Las tres pasadas del arrastre, por lo mismo que en la prueba de `arrastrar_el_divisor`: egui
        // resuelve la interacción contra las regiones de la pasada anterior, y el candidato a arrastrarse
        // sale de dónde estaba el puntero **al pulsar**.
        pasada_de_interfaz(&mut ventana, &ctx, area, mover(agarre));
        pasada_de_interfaz(&mut ventana, &ctx, area, pulsar(agarre));
        pasada_de_interfaz(&mut ventana, &ctx, area, mover(suelta));

        // 600 de 800 repartibles es 0,75 sin resto, así que aquí tampoco hace falta tolerancia.
        assert_eq!(
            ventana.activa().fraccion,
            0.75,
            "el arrastre no llevó la fracción hasta el puntero"
        );

        let metrica = Metrica::medir(&ctx, egui::FontId::monospace(CUERPO));
        let reparto = repartir(area, 0.75, metrica.ancho());
        assert_eq!(
            ventana.activa().divisiones[0].medido,
            metrica.celdas_en(reparto.izquierda.size()),
            "la consola izquierda se midió con el reparto de antes del arrastre"
        );
        assert_eq!(
            ventana.activa().divisiones[1].medido,
            metrica.celdas_en(reparto.derecha.size()),
            "la consola derecha se midió con el reparto de antes del arrastre"
        );
    }
}
