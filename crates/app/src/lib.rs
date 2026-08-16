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

/// La variable de entorno que dice con cuántas pestañas abrir.
///
/// **Existe para poder medir sin nadie delante, y por eso lleva el prefijo del guion de medida** —
/// `MEDIR_MIB` y `MEDIR_TANDA` ya viven en `scripts/medir-fotograma.sh`. Los atajos que crean pestañas
/// entran por eventos del framework, así que un guion no tiene forma de abrir la segunda; sin esta
/// puerta, la tanda de dos pestañas del anexo de D3 no se puede lanzar.
///
/// Se escribe sin eñe a propósito: es lo que un guion de shell tiene que poder teclear sin sorpresas de
/// codificación, y sus dos hermanas ya son ASCII.
const VARIABLE_DE_LAS_PESTAÑAS: &str = "MEDIR_PESTANAS";

/// Cuánto aire deja la barra por encima y por debajo del número de la pestaña.
///
/// Es lo único constante de su alto: el resto sale de la métrica de la fuente, así que la barra crece
/// con el cuerpo de la letra en vez de quedarse corta al subirlo.
const MARGEN_DE_LA_BARRA: f32 = 4.0;

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

/// Lo que un acorde del teclado le pide a la ventana, cuando lo que pide no es de la consola.
///
/// **Sale por el valor de retorno de [`Division::teclear`] y no por un parámetro de salida**, porque es
/// solo salida: el `&mut Modifiers` de esa función sí es parámetro porque **también es entrada** —el
/// acorde puede empezar en un fotograma y usarse en el siguiente—, y una orden no arrastra nada del
/// pase anterior.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Orden {
    /// Cambiar de pestaña, **tantas como diga el número**: negativo hacia atrás, positivo hacia delante.
    ///
    /// **Lleva un número y no una dirección porque el clic en una ficha salta a donde sea**: de la primera
    /// a la tercera son dos. Cada pulsación del teclado, en cambio, vale exactamente uno — no se componen
    /// entre ellas, se aplican en orden, y por qué es así está en [`Division::teclear`].
    Ir(isize),
    /// Abrir una pestaña nueva, con sus dos consolas, y dejarla delante.
    Crear,
    /// Cerrar la que se está viendo. Si es la única, lo que se cierra es la ventana.
    Cerrar,
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

/// Con cuántas pestañas abre la ventana, a partir de lo que dijera la variable.
///
/// **Nunca menos de una**, y ese suelo no es prudencia: la lista de pestañas no puede estar vacía —la
/// ventana siempre enseña una— y un cero dejaría todos los accesos indexando fuera de rango en el primer
/// fotograma. Un valor que no sea un número se ignora igual que si no estuviera: quien mide se entera al
/// ver una sola pestaña, y abortar el arranque por una variable mal escrita sería peor.
///
/// **Recibe el valor y no lo lee, y eso lo decidió una revisión.** Leerlo aquí obligaría a las pruebas a
/// llamar a `std::env::set_var`, cuyo contrato es sobre **todo** el entorno: cualquier lectura concurrente
/// es comportamiento indefinido, no solo la de esta clave. Y en esta suite las hay a mansalva —
/// `Division::nueva` lee `SHELL`, y `portable_pty` recorre `std::env::vars_os()` al construir **cada**
/// consola—, con las pruebas corriendo en hilos a la vez. La única línea que queda sin cubrir es la de
/// [`pestañas_al_arrancar`], por lo mismo que las de `on_exit`.
fn cuantas_pestañas(valor: Option<&str>) -> usize {
    valor
        .and_then(|valor| valor.parse().ok())
        .unwrap_or(1)
        .max(1)
}

/// Lo que la variable de entorno dice, ya resuelto.
fn pestañas_al_arrancar() -> usize {
    cuantas_pestañas(std::env::var(VARIABLE_DE_LAS_PESTAÑAS).ok().as_deref())
}

/// Aparta de arriba el sitio de la barra, y devuelve lo que le queda a las consolas.
///
/// **Con una sola pestaña no hay barra, y eso no es una comodidad estética.** Es lo que deja la rejilla
/// idéntica a la de antes de las pestañas, y esa igualdad es la tanda de control del anexo de D3: sin
/// ella, la barra le quitaría alto a las consolas igual que el panel les quitó columnas, el fotograma
/// saldría **más barato** por pintar menos celdas y el anexo publicaría una mejora que no existe.
///
/// **El corte se acota, porque [`egui::Rect::split_top_bottom_at_y`] no lo hace** — leído en su fuente
/// (`emath-0.35.0/src/rect.rs:671`), donde construye los dos rectángulos con el corte tal cual. Con la
/// ventana más baja que la barra, un corte por debajo del área dejaría el rectángulo de las consolas
/// **invertido**, con su borde de arriba por debajo del de abajo. Es el mismo cuidado que [`repartir`]
/// ya tiene con el ancho, y el mismo caso borde que allí está probado.
fn apartar_la_barra(
    area: egui::Rect,
    cuantas: usize,
    metrica: &Metrica,
) -> (Option<egui::Rect>, egui::Rect) {
    if cuantas < 2 {
        return (None, area);
    }

    let alto = metrica.alto() + 2.0 * MARGEN_DE_LA_BARRA;
    let corte = (area.top() + alto).min(area.bottom());
    let (barra, consolas) = area.split_top_bottom_at_y(corte);

    (Some(barra), consolas)
}

/// Dónde cae la ficha de una pestaña dentro de la barra.
///
/// **Todas del mismo ancho, y sin desplazamiento.** El número se recorta cuando dejan de caber —de ahí
/// que quien la pinte recorte a este rectángulo—, lo que la deja legible hasta unas quince o veinte
/// pestañas. Por encima de eso se navega por atajo, que es como se navega de todas formas en cuanto hay
/// más de tres.
///
/// No hace falta guardarse de `cuantas` a cero: solo hay fichas cuando hay barra, y solo hay barra con
/// dos pestañas o más.
fn ficha(barra: egui::Rect, cual: usize, cuantas: usize) -> egui::Rect {
    let ancho = barra.width() / cuantas as f32;
    let izquierda = barra.left() + ancho * cual as f32;

    egui::Rect::from_min_max(
        egui::pos2(izquierda, barra.top()),
        egui::pos2(izquierda + ancho, barra.bottom()),
    )
}

/// Abre la ventana y no vuelve hasta que se cierra.
///
/// **Aquí se lee el entorno, y en ningún otro sitio.** [`Ventana::con_pestañas`] recibe el número ya
/// resuelto para que ninguna prueba dependa de lo que haya exportado la sesión que la lanza; el precio
/// es que esta línea, como la de [`eframe::App::on_exit`], no la cubre ninguna prueba: cambiar el `1` de
/// [`pestañas_al_arrancar`] por un fijo dejaría la suite entera en verde y la medida sin su puerta.
/// Quien mide se entera en cuanto abre la ventana y ve una sola pestaña.
pub fn arrancar() -> eframe::Result {
    let cuantas = pestañas_al_arrancar();

    eframe::run_native(
        NOMBRE,
        eframe::NativeOptions::default(),
        Box::new(move |cc| Ok(Box::new(Ventana::con_pestañas(&cc.egui_ctx, cuantas)?))),
    )
}

/// Una consola y todo lo que hace falta para sostenerla.
///
/// Cada campo de aquí era un campo de [`Ventana`] cuando solo había una consola. Agruparlos es lo que
/// permite instanciar el conjunto entero más de una vez sin escribir cada operación dos veces.
struct Division {
    /// Cómo se la llama en el informe, y **no cambia aunque se cierren otras**.
    ///
    /// **El índice en la disposición no vale para esto**, por lo mismo que no valía para las pestañas:
    /// en cuanto se pueda cerrar una división, las de al lado corren un puesto y el informe le pondría
    /// a una tabla de números el nombre de otra consola — con los tramos de la que los produjo, y sin
    /// nada en el texto que permita notarlo. Se asigna al abrirla y no se reutiliza.
    ///
    /// **Va aquí dentro y no en una lista paralela de la pestaña.** Dos vectores que hay que mantener
    /// alineados son una invariante que alguien romperá; dentro del tipo no hay nada que alinear.
    nombre: usize,
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
    /// Lo que ha costado releer el `panel.json` de esta consola **sin llegar a pintarla**, a la espera
    /// de que se cierre su vuelta al final de la pasada de interfaz.
    ///
    /// Una consola de una pestaña de fondo relee su panel igual que la que se ve —de que haya panel
    /// depende cuánto ancho le queda—, pero no pasa por el pintado, que es donde ese tramo se declara.
    /// Sin este campo, leer un fichero de disco quedaría dentro del fotograma sin que nadie lo
    /// cronometre y la resta se lo regalaría al framework de interfaz.
    ///
    /// **Vive un suspiro: se escribe al medir y se vacía al cerrar la vuelta, dentro de la misma
    /// pasada.** No hace falta que acumule entre pasadas, y conviene saber por qué: eframe llama a
    /// `logic` **y** a `ui` dentro del mismo cierre que egui repite (`epi_integration.rs:288-303`), así
    /// que una pasada repetida vuelve a bombear y vuelve a dejar vuelta pendiente. Nunca hay una
    /// lectura esperando a una vuelta que no llega.
    panel_sin_pintar: std::time::Duration,
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
    fn nueva(ctx: &egui::Context, nombre: usize) -> term_core::Result<Self> {
        // El shell sale del entorno de quien lanza la aplicación, que es lo que hace un terminal:
        // sin esto se abre un shell sin prompt ni historial, y este hito existe para mirar lo que
        // sale en pantalla. Sigue sin ser una entrada ajena —es la sesión del propio usuario—,
        // pero el día en que el programa venga de configuración o de un `panel.json` de fuera,
        // esta línea pasa a ser superficie sensible y hay que revisarla como tal.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| SHELL_DE_RESPALDO.to_owned());

        Self::lanzando(ctx, &shell, nombre)
    }

    /// Lo mismo, con el programa dicho a mano.
    ///
    /// Existe porque el programa que elige [`Division::nueva`] es el del usuario, y una prueba que
    /// dependa de él no prueba este código: prueba la configuración de quien la corre.
    fn lanzando(ctx: &egui::Context, programa: &str, nombre: usize) -> term_core::Result<Self> {
        // El directorio se crea **antes** de lanzar, porque el programa puede publicar en su primer
        // segundo y la variable que le decimos tiene que apuntar a algo que ya existe. Si lanzar
        // falla después, el directorio se borra solo: el valor muere por el camino de error.
        Self::con_panel(ctx, programa, panel_o_ninguno(PanelDir::create()), nombre)
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
        nombre: usize,
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
            nombre,
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
            panel_sin_pintar: std::time::Duration::ZERO,
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
    ///
    /// **Devuelve las órdenes que los acordes le pidan a la ventana**, en el orden en que llegaron.
    ///
    /// **Son una lista y no una sola, y eso no es generalidad de más.** egui entrega en un mismo lote todo
    /// lo que llegó mientras la ventana dormía —el caso de este hito: dos agentes callados—, y ahí puede
    /// venir más de un acorde. Con una sola habría que elegir cuál sobrevive, y ninguna elección es
    /// buena: quedarse con la última pierde un cierre, sumarlas solo funciona entre desplazamientos, y un
    /// desplazamiento pedido *antes* de crear o cerrar se refiere a una lista de pestañas que ya no
    /// existe. Aplicándolas en orden no hay nada que elegir. Un vector vacío no reserva memoria, así que
    /// el caso normal —ningún acorde— no cuesta nada. El corte de esos
    /// acordes vive aquí dentro, en el mismo recorrido que ya consume el pegado y **antes** de traducir
    /// a bytes, y no en un recorrido propio de [`Ventana`]: el acorde se reconstruye evento a evento, así
    /// que dos recorridos sobre la misma cola serían **dos reconstrucciones que pueden discrepar**, y el
    /// evento hay que consumirlo en uno de los dos — repartirlo deja la pregunta de cuál gana.
    ///
    /// `pestañas_abiertas` es cuántas hay al empezar el lote. Se recibe en vez de deducirse porque una
    /// división no sabe —ni tiene por qué— cuántas hay: lo que aquí se decide es si el acorde se consume
    /// o se le deja pasar al programa, y eso solo puede decidirse **dentro** de este recorrido. Filtrarlo
    /// después, en la ventana, llegaría tarde: la tecla ya se habría tragado. Dentro, la cuenta se lleva
    /// al día evento a evento, porque el propio lote crea y cierra pestañas.
    fn teclear(
        &mut self,
        ctx: &egui::Context,
        empezo_el_fotograma: Instant,
        modificadores: &mut egui::Modifiers,
        pestañas_abiertas: usize,
    ) -> Vec<Orden> {
        self.pulsaciones.clear();
        // Se pregunta una vez y no por evento porque **el modo no puede cambiar a mitad de pase**:
        // lo pone el programa de dentro con lo que escribe, y lo que escribe se interpreta en la otra
        // mitad del fotograma. (Preguntarlo dentro del cierre también compilaría —los dos campos son
        // préstamos disjuntos—, así que esto es una elección, no una pared.)
        let marcado = self.pantalla.pegado_marcado();
        let mut pegado = false;
        let mut ordenes = Vec::new();

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
            let mut pedidas = Vec::new();
            let mut abiertas = pestañas_abiertas;

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

                // **Cambiar de pestaña se corta aquí, antes de traducir a bytes, y no consumirlo sería
                // el fallo.** Con Ctrl+Mayúsculas puestas, la flecha cae en la tabla de siempre de
                // [`teclado::tecla`] —Mayúsculas no cambia el código de control y Ctrl con algo que no
                // es letra se cae a esa tabla— y el programa de dentro recibiría `\x1b[D`: el cursor se
                // movería en el editor de la pestaña que se acaba de dejar.
                //
                // **Con una sola pestaña la flecha no se corta**, y esa guarda no es cosmética: sin ella
                // el acorde se tragaría la tecla para no hacer nada —ir de una pestaña a ella misma— y se
                // la quitaría al programa, que hoy sí la recibe. Crear y cerrar sí valen con una sola.
                //
                // **La cuenta se lleva evento a evento y no se mira una vez por fotograma**, porque el
                // propio lote la cambia: con una pestaña abierta y `[crear, ←]` en el mismo lote, mirarla
                // al principio dejaría la flecha fuera de la guarda y se le colaría al programa, que es
                // justo lo que el párrafo de arriba dice que no puede pasar.
                //
                // **La repetición de mantener pulsada la tecla vale para las flechas y no para crear ni
                // cerrar.** egui marca las repeticiones con `repeat`, pero sigue poniendo `pressed`, así
                // que sin mirarlo un segundo con el acorde de cerrar apretado son unas treinta órdenes de
                // cierre en un lote: **todas las pestañas cerradas, todos sus programas muertos y la
                // ventana con ellos, de un solo gesto**. Ciclar entre pestañas sí es barato y reversible.
                // Aun así el evento **se consume igual** cuando es repetición: devolverlo a la cola
                // mandaría `0x14` al programa por cada repetición.
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    ..
                } = evento
                    && acorde.ctrl
                    && acorde.shift
                {
                    // **Primero si el acorde es nuestro, y solo después qué hacer con él.** Resolver las
                    // dos cosas en el mismo `match` deja un camino que no consume el evento **ni lo
                    // traduce**: la flecha se quedaba en la cola sin llegar nunca al programa.
                    let es_nuestro = match key {
                        egui::Key::ArrowLeft | egui::Key::ArrowRight => abiertas > 1,
                        egui::Key::T | egui::Key::W => true,
                        _ => false,
                    };

                    if es_nuestro {
                        match key {
                            egui::Key::ArrowLeft => pedidas.push(Orden::Ir(-1)),
                            egui::Key::ArrowRight => pedidas.push(Orden::Ir(1)),
                            egui::Key::T if !repeat => {
                                abiertas += 1;
                                pedidas.push(Orden::Crear);
                            }
                            // Cerrar la última no quita nada de la lista: pide cerrar la ventana. Por eso
                            // la cuenta no baja de uno.
                            egui::Key::W if !repeat => {
                                abiertas = (abiertas - 1).max(1);
                                pedidas.push(Orden::Cerrar);
                            }
                            // La repetición de crear o cerrar se traga sin hacer nada, que es el objeto de
                            // la guarda.
                            _ => {}
                        }

                        return false;
                    }
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
            ordenes = pedidas;
        });

        // Una orden sale igual aunque no se haya tecleado nada para el programa, que es el caso normal:
        // el acorde de cambiar de pestaña no manda ningún byte.
        if self.pulsaciones.is_empty() {
            return ordenes;
        }
        if let Err(fallo) = self.pty.write(&self.pulsaciones) {
            // Lo que se acaba de teclear no ha llegado. Una tecla que se pierde en silencio es de
            // los fallos que más tardan en entenderse desde el otro lado de la pantalla.
            eprintln!("no se pudo enviar lo tecleado al programa: {fallo}");
            // Un envío que falló no es una ida rápida: no se anota, o el número mediría el fallo.
            return ordenes;
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

        ordenes
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
    /// Cómo se la llama en el informe, y **no cambia aunque se cierren otras**.
    ///
    /// **Es el índice de la lista lo que no vale para esto.** Al cerrar una pestaña, las de su derecha
    /// corren un puesto: la que era la 2 pasa a ser la 1, y el informe que se publica al salir acabaría
    /// poniéndole a una tabla de números el nombre de otra pestaña — con los tramos de la que los produjo.
    /// Quien lea ese informe no tiene forma de notarlo. Se asigna al abrirla y no se reutiliza.
    nombre: usize,
}

impl Pestaña {
    /// Dos consolas nuevas, con el shell del usuario dentro de cada una.
    ///
    /// **Los nombres de las divisiones llegan de fuera**, igual que el de la pestaña: el contador que
    /// garantiza que no se repitan es de la ventana, porque las divisiones de dos pestañas distintas
    /// también tienen que distinguirse en el informe.
    fn nueva(
        ctx: &egui::Context,
        nombre: usize,
        nombres_de_division: [usize; 2],
    ) -> term_core::Result<Self> {
        let [izquierda, derecha] = nombres_de_division;
        Ok(Self::con(
            [
                Division::nueva(ctx, izquierda)?,
                Division::nueva(ctx, derecha)?,
            ],
            nombre,
        ))
    }

    /// Cómo se abre una pestaña: el foco a la izquierda y el divisor donde diga [`FRACCION_INICIAL`].
    ///
    /// Existe aparte de [`Pestaña::nueva`] para que las pruebas entren por aquí con sus propias
    /// consolas en vez de armar el struct a mano — si no, el día que abrir una pestaña signifique algo
    /// más, las pruebas seguirían abriendo la de antes.
    fn con(divisiones: [Division; 2], nombre: usize) -> Self {
        Self {
            divisiones,
            foco: 0,
            fraccion: FRACCION_INICIAL,
            nombre,
        }
    }
}

/// La ventana raíz: las pestañas, y lo que es del programa y no de ninguna disposición.
struct Ventana {
    /// Las disposiciones abiertas. **Nunca vacía**: la ventana siempre está enseñando una.
    ///
    /// Se abre con una, o con las que diga la variable de la puerta de medida, y a partir de ahí las crea
    /// y las cierra quien usa la ventana.
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
    /// Lo que este fotograma ha gastado **fuera de las divisiones que siguen vivas**, y que por tanto
    /// ninguna va a declarar.
    ///
    /// Son dos cosas, y las dos inflaban «el resto del fotograma» —o sea, el coste que se le atribuye al
    /// framework de interfaz, que es la cifra con la que D3 decide—:
    ///
    /// 1. **Abrir una pestaña son dos `fork`/`exec`**, y ocurren dentro de la pasada de interfaz porque es
    ///    ahí donde se atiende el acorde. Nadie los cronometraba.
    /// 2. **Cerrar una pestaña se lleva sus divisiones, y con ellas su [`medida::Medida`]** — incluida la
    ///    vuelta que ya tenían anotada de este mismo fotograma. Ese trabajo ocurrió, así que si se va sin
    ///    declararse, el resto se queda con él.
    ///
    /// Se vacía al cerrar el fotograma. **No lleva serie propia en el informe**: lo que hace falta es que
    /// se **reste**, y una serie más para un suceso que pasa cuando el usuario pulsa un atajo diría muy
    /// poco y habría que explicarla entera.
    cronometrado_aparte: std::time::Duration,
    /// El nombre que llevará la próxima pestaña que se abra. Ver [`Pestaña::nombre`].
    ///
    /// Vive en la ventana y no se deduce de la lista **porque la lista se queda corta en cuanto se cierra
    /// algo**: con dos abiertas y la primera cerrada, `pestañas.len()` vale 1 y volvería a repartir un
    /// nombre que ya se usó.
    siguiente_nombre: usize,
    /// El nombre que llevará la próxima división que se abra. Ver [`Division::nombre`].
    ///
    /// **Uno solo para toda la ventana, no uno por pestaña.** El informe publica las divisiones de todas
    /// las pestañas seguidas, así que dos contadores independientes darían dos «división 0» en el mismo
    /// texto — el nombre volvería a no identificar a nadie, que es justo lo que viene a arreglar.
    siguiente_nombre_de_division: usize,
    /// Si alguna pestaña se ha cerrado **a mano**, y por tanto la ventana ya no se cierra sola.
    ///
    /// **Existe por un caso que solo aparece cuando cerrar es posible**: una pestaña cuyos programas ya
    /// terminaron —y cuyo último contenido se está leyendo, que es lo que la regla del cierre existe para
    /// preservar— junto a otra con un agente vivo. Al cerrar a mano la del agente no queda nada vivo, y la
    /// regla de siempre cerraría la ventana **llevándose lo que se estaba leyendo**. Nadie pidió eso: se
    /// pidió cerrar una pestaña.
    ///
    /// No se vuelve a bajar. Quien ha cerrado una pestaña a mano ha dicho que gobierna él las pestañas, y
    /// le quedan la última con su acorde y el botón de la ventana.
    cerrada_alguna_a_mano: bool,
}

impl Ventana {
    /// Una ventana con las pestañas que se le pidan.
    ///
    /// **Cuántas son se le dice desde fuera y no se lee aquí del entorno**, aunque el único sitio que hoy
    /// pide más de una sea [`arrancar`] leyendo [`VARIABLE_DE_LAS_PESTAÑAS`]. Leerlo dentro haría que una
    /// variable exportada en la sesión cambiara lo que abren las pruebas que llaman aquí —que corren en el
    /// mismo proceso y a la vez—, y una prueba que dice «la ventana abre con una sola pestaña» empezaría a
    /// fallar según qué otra estuviera corriendo en ese instante.
    ///
    /// **Pedir cero pestañas es un error de quien llama, y aquí no se corrige.** El suelo lo pone
    /// [`cuantas_pestañas`], que es por donde entra el único valor que no controlamos; una segunda guarda
    /// aquí solo taparía en silencio una llamada que no puede ocurrir. Lo honesto sería un tipo que no
    /// admitiera el cero —`NonZeroUsize`—, y no entra en este cambio porque obliga a un envoltorio en
    /// cada prueba que abre una ventana.
    fn con_pestañas(ctx: &egui::Context, cuantas: usize) -> term_core::Result<Self> {
        // **Si alguna falla no se abre ninguna**, y el `?` sobre el `collect` es lo que lo consigue: el
        // mismo criterio que `panel_o_ninguno`. Media docena de pestañas de las que una está a medias es
        // peor que no arrancar, porque el fallo se descubre al llegar a ella.
        let pestañas: Vec<Pestaña> = (0..cuantas)
            .map(|nombre| Pestaña::nueva(ctx, nombre, [nombre * 2, nombre * 2 + 1]))
            .collect::<term_core::Result<_>>()?;
        let siguiente_nombre = pestañas.len();
        // **Contadas de la lista y no del parámetro**, por lo mismo que se explica abajo para el nombre
        // de pestaña: son dos fuentes para el mismo dato y con dos se puede coger la que no toca.
        let siguiente_nombre_de_division = pestañas
            .iter()
            .map(|pestaña| pestaña.divisiones.len())
            .sum();

        Ok(Self {
            // **Sobre las divisiones de todas las pestañas, no solo la primera.** Una pestaña de fondo
            // sin vigilar no volvería a releer su panel hasta que alguien la mire, que es justamente lo
            // contrario de lo que este hito promete.
            vigilante: Self::vigilar(
                ctx,
                pestañas
                    .iter()
                    .flat_map(|pestaña| pestaña.divisiones.iter()),
            ),
            pestañas,
            activa: 0,
            modificadores: egui::Modifiers::NONE,
            tema: Tema::default(),
            resto_del_fotograma: RestoDelFotograma::default(),
            empezo_el_fotograma: Instant::now(),
            ultimo_fotograma_cerrado: 0,
            cronometrado_aparte: std::time::Duration::ZERO,
            // **Del largo de la lista y no del parámetro `cuantas`, aunque aquí valgan lo mismo.** Son
            // dos fuentes para el mismo dato, y con dos se puede coger la que no toca: poner un cero o
            // el parámetro equivocado hace que la siguiente pestaña que se abra **se llame igual que
            // una que ya existe**, y el informe publique dos bloques de números con la misma etiqueta.
            // Comprobado mutándolo, y no lo caza ninguna prueba: este constructor lanza consolas de
            // verdad leyendo `SHELL`, así que solo se ejercita al arrancar la aplicación. Derivarlo de
            // la lista es lo que hace que no haya nada que confundir.
            siguiente_nombre,
            siguiente_nombre_de_division,
            cerrada_alguna_a_mano: false,
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

    /// Todas las consolas abiertas, se vean o no.
    ///
    /// **La distinción entre esto y [`Ventana::activa`] es la del hito entero**, y no es de estilo:
    /// lo que un programa *hace* corre en todas las pestañas, y lo que se *ve* ocurre solo en una. Un
    /// recorrido puesto del lado equivocado no da error ni sale en pantalla — con una sola pestaña los
    /// dos hacen exactamente lo mismo—, así que la única defensa es que cada sitio diga cuál de las
    /// dos cosas es.
    fn todas_las_divisiones(&self) -> impl Iterator<Item = &Division> {
        self.pestañas
            .iter()
            .flat_map(|pestaña| pestaña.divisiones.iter())
    }

    fn todas_las_divisiones_mut(&mut self) -> impl Iterator<Item = &mut Division> {
        self.pestañas
            .iter_mut()
            .flat_map(|pestaña| pestaña.divisiones.iter_mut())
    }

    /// Arma el vigilante sobre los directorios de las consolas que tienen uno.
    ///
    /// Una consola sin directorio no se vigila porque no hay nada que vigilar: nadie puede publicarle
    /// un panel. Su bandera se queda como esté y su lectura no encuentra dónde mirar.
    fn vigilar<'a>(
        ctx: &egui::Context,
        divisiones: impl IntoIterator<Item = &'a Division>,
    ) -> Vigilante {
        let consolas: Vec<_> = divisiones
            .into_iter()
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
    ///
    /// **Y mira solo la pestaña que se ve**, al revés que el bombeo. La alarma existe para que la
    /// etiqueta de «esto es viejo» **aparezca en pantalla**, y una pestaña que no se pinta no tiene
    /// pantalla: despertar la ventana por ella sería gastar un fotograma en un aviso que nadie va a
    /// ver. No se pierde nada, porque si el panel se ve rancio se decide **al pintarlo**, así que al
    /// volver a esa pestaña el aviso sale en ese mismo fotograma.
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

    /// Escribe el informe de la sesión al destino que se le dé.
    ///
    /// **Recibe el destino en vez de imprimir**, por lo mismo que la tabla de tramos de cada consola:
    /// es la única forma de que una prueba afirme sobre **lo que el informe dice de verdad** y no
    /// sobre las constantes con las que se escribe. Comprobar la constante demuestra que está bien
    /// redactada, no que alguien la imprima — y eso, en este proyecto, ya ha dejado pasar cosas con la
    /// suite en verde.
    fn publicar_informe(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self
            .todas_las_divisiones()
            .any(|division| division.medida.hay_algo())
        {
            return Ok(());
        }

        let rejillas: Vec<_> = self
            .pestañas
            .iter()
            .map(|pestaña| {
                (
                    pestaña.nombre,
                    pestaña
                        .divisiones
                        .iter()
                        .map(|d| (d.nombre, d.pantalla.size()))
                        .collect(),
                )
            })
            .collect();
        medida::cabecera(a, &rejillas, self.activa().nombre)?;

        // Dónde quedó el divisor, y no «mitad y mitad»: desde que se arrastra, los tramos de abajo
        // pueden venir de dos consolas de anchos muy distintos, y una tabla de tramos sin saber con qué
        // reparto se tomó no compara con nada.
        //
        // **Se publica la fracción pedida, no los anchos resultantes.** Los anchos de verdad están
        // arriba, en la rejilla de cada consola, que es lo que se usó para interpretar; repetirlos aquí
        // a partir de la fracción los haría discrepar en el único caso en que el suelo actúa.
        //
        // **Y de la pestaña que se veía, porque es la única cuyo divisor y foco significan algo**: en
        // una que no se pinta ni se arrastra el divisor ni se teclea.
        let visible = self.activa();
        writeln!(
            a,
            "divisor:      al {:.0} % del ancho repartible al salir, y son {GROSOR_DEL_DIVISOR} \
             puntos de ancho. Si se arrastró durante la sesión, los tramos de abajo mezclan repartos",
            visible.fraccion * 100.0
        )?;
        // **Por el nombre de la división, no por su posición.** La posición identifica bien mientras las
        // divisiones sean dos y fijas; el nombre sigue identificando cuando dejen de serlo, y publicar ya
        // el que va a valer es lo que evita que las tandas de antes y las de después se lean distinto.
        writeln!(
            a,
            "foco al salir: división {} ({}) de la pestaña {}",
            visible.divisiones[visible.foco].nombre,
            medida::NOMBRE_DE_CONSOLA[visible.foco],
            visible.nombre
        )?;

        // **Todas las pestañas, no solo la que se veía.** Una consola de fondo cronometra su
        // interpretar igual que las demás, y publicar solo las visibles tiraría justo los números que
        // la tanda de dos pestañas existe para tomar.
        // **Y cada tabla se titula con el nombre de su división.** Es el sitio donde equivocarse sale más
        // caro: quien lea el informe ve una tabla de números bajo una etiqueta y no tiene forma de
        // comprobar que sean de esa consola.
        for pestaña in &self.pestañas {
            let que_pestaña = pestaña.nombre;
            for (cual, division) in pestaña.divisiones.iter().enumerate() {
                writeln!(
                    a,
                    "\n── pestaña {que_pestaña}, división {} ({}) ──",
                    division.nombre,
                    medida::NOMBRE_DE_CONSOLA[cual]
                )?;
                division.medida.publicar_tramos(a)?;
            }
        }

        self.resto_del_fotograma.publicar(a)
    }

    /// Le da a las consolas de las pestañas que no se ven la rejilla que tendrían si se vieran.
    ///
    /// **Sin esto una pestaña de fondo se queda con [`REJILLA_INICIAL`] mientras dure**, porque
    /// `medido` solo se escribe al pintar y a ellas no se las pinta. Y eso no es un detalle interno:
    /// es el tamaño que [`Division::redimensionar`] le anuncia al programa de dentro, así que un
    /// agente que trabaja en una pestaña que no miras formatea su salida para una ventana que no
    /// existe —80 columnas de arranque cuando la suya son 57— y al volver a esa pestaña lo escrito
    /// sigue ajustado a un ancho ajeno hasta que el programa redibuje. La promesa del hito es que la
    /// de fondo siga corriendo; corriendo con la rejilla equivocada corre mal.
    ///
    /// **Cada una se reparte con su propia fracción, no con la de la activa**: el divisor es de la
    /// disposición, y usar el de la pestaña visible mediría estas consolas con un divisor que no es
    /// el suyo — el mismo defecto que ya costó una revisión cuando el clic de la barra se atendía
    /// después de repartir.
    ///
    /// **El panel se relee también aquí, igual que en el pintado, y el primer intento no lo hacía.**
    /// De que haya panel depende cuánto ancho le queda a la consola —el panel se lleva 36 columnas—,
    /// así que una división de fondo cuyo productor publica por primera vez se mediría **36 columnas
    /// de más** mientras dure el segundo plano: el mismo defecto que esta función viene a arreglar,
    /// con el signo cambiado y más grande. Releer no es caro por defecto: `refrescar_su_panel` sale
    /// por un `swap` atómico y solo llega al disco en el fotograma en que el vigilante avisó de que
    /// **ese** `panel.json` cambió, que es exactamente cuando hace falta.
    ///
    /// **Y esa lectura se declara**, que es la otra mitad. Cae dentro de la pasada de interfaz, y aquí
    /// no hay pintado que la cronometre: sin declararla, la resta se la regalaría al framework — la
    /// cifra con la que D3 decide. Se guarda en `panel_sin_pintar` y la recoge
    /// [`Ventana::cerrar_las_vueltas_no_pintadas`], que corre justo después.
    ///
    /// **El cronómetro solo cuenta si esta consola tiene directorio de panel.** Sin él no hay fichero
    /// que leer y el refresco es el `swap` y nada más; publicar esos nanosegundos bajo la etiqueta
    /// «panel» sería un tramo con el nombre cambiado, y en este proyecto eso ya invalidó una medida
    /// entera.
    ///
    /// Con la ventana minimizada u ocluida no hay pasada de interfaz y esto no corre, igual que no
    /// corre el pintado que mide la visible: nadie se queda peor que la que se ve.
    fn medir_las_que_no_se_ven(&mut self, area: egui::Rect, metrica: &Metrica) {
        // Un solo reloj para todas, y aquí sí basta con uno: de la hora solo depende si el panel de
        // una división está rancio, y no se pinta nada después que pudiera desmentirlo.
        let ahora = SystemTime::now();
        let activa = self.activa;

        for (cual, pestaña) in self.pestañas.iter_mut().enumerate() {
            if cual == activa {
                continue;
            }

            let reparto = repartir(area, pestaña.fraccion, metrica.ancho());
            for (division, rect) in pestaña
                .divisiones
                .iter_mut()
                .zip([reparto.izquierda, reparto.derecha])
            {
                let antes_del_panel = Instant::now();
                division.refrescar_su_panel();
                let hueco = division.repartir_con_su_panel(rect, metrica.ancho(), ahora);
                // **La frontera del tramo es la misma que en el pintado**, que cierra su cronómetro
                // también después de repartir: `repartir_con_su_panel` pregunta si el panel se ve, y
                // esa pregunta llega a componer el texto de un aviso cuando el productor está callado
                // o roto. Cerrar antes dejaría ese trabajo fuera, y las dos consolas de un mismo
                // informe medirían el tramo «panel» con fronteras distintas según se las mire o no.
                if division.panel.is_some() {
                    division.panel_sin_pintar = antes_del_panel.elapsed();
                }

                division.medido = metrica.celdas_en(hueco.consola.size());
            }
        }
    }

    /// Cierra la vuelta de las consolas que **interpretaron y no se pintaron**, para que su trabajo no
    /// acabe publicado como coste del framework de interfaz.
    ///
    /// **El sesgo que esto evita es grande y silencioso.** El coste del fotograma se reparte por
    /// resta: cada consola declara lo suyo y lo que sobra se le atribuye al framework. Una consola de
    /// una pestaña que no se ve interpreta bytes igual que las demás, pero no pasa por el pintado, que
    /// es el único sitio que cerraba su vuelta — así que su tiempo se le regalaba al resto, y con una
    /// sola pestaña de fondo volcando lo multiplicaría **varias veces**. Y no dejaba rastro: el bombeo
    /// del fotograma siguiente sobrescribe lo pendiente, así que la muestra no se doblaba, **se
    /// perdía**.
    ///
    /// **El encolado a cero es la verdad, no un relleno**: esa consola no encoló ningún pintado. **El
    /// tramo del panel ya no es cero**, y ésa es la diferencia con la primera versión de este barrido:
    /// una consola de fondo **sí** relee su `panel.json` —de que haya panel depende cuánto ancho le
    /// queda—, y esa lectura la cronometra [`Ventana::medir_las_que_no_se_ven`] y la entrega aquí. Y
    /// **la espera se declara a `None` a propósito**, que es lo que hace que las cuatro medias no se
    /// toquen —ver [`medida::Medida::cerrar_fotograma`]—; lo que se conserva es lo interpretado y lo
    /// que costó el panel, que es justo lo que la resta necesita. Por eso esto **no lleva ninguna serie
    /// nueva**.
    ///
    /// **Va al final de la pasada de interfaz y no tras bombear**, porque hasta que no se ha pintado
    /// no se sabe qué se pintó: un atajo puede cambiar la pestaña visible en ese mismo fotograma.
    ///
    /// Con la ventana minimizada u ocluida no hay pasada de interfaz, así que esto no corre y nadie
    /// declara nada. **No es un agujero**: entonces tampoco declara la consola visible, así que
    /// [`Ventana::cerrar_fotograma`] no encuentra nada que restar y no anota resto — el fotograma se
    /// queda sin medir, que es lo correcto, en vez de medido de menos.
    fn cerrar_las_vueltas_no_pintadas(&mut self) {
        for division in self.todas_las_divisiones_mut() {
            // **El tramo del panel viaja con la vuelta, y no en una anotación propia.** No es una
            // preferencia: [`medida::Medida::anotar_vuelta`] **sobrescribe** lo pendiente, así que una
            // segunda anotación en el mismo fotograma se llevaría por delante la vuelta buena. Aquí no
            // llega a pasar —`bombear` deja siempre algo pendiente y las dos cosas se declaran
            // juntas—, y por eso mismo conviene que quede escrito: la forma segura de añadir un tramo
            // a esta vuelta es meterlo dentro, nunca al lado.
            if let Some((_, recogido_en, interpretado)) = division.pendiente_de_medir.take() {
                division.medida.anotar_vuelta(medida::Tramos::de_una_vuelta(
                    None,
                    recogido_en,
                    interpretado,
                    std::time::Duration::ZERO,
                    std::mem::take(&mut division.panel_sin_pintar),
                ));
            }
        }
    }

    /// Cierra el fotograma anterior: cada consola cierra sus tramos y la ventana se queda con lo que
    /// ninguna cronometró.
    fn cerrar_fotograma(&mut self, coste_cpu: std::time::Duration) {
        let cronometrado = self
            .todas_las_divisiones_mut()
            .filter_map(|division| division.medida.cerrar_fotograma(coste_cpu))
            .reduce(|una, otra| una + otra);

        // **Se vacía siempre, haya resto que anotar o no.** Arrastrarlo al fotograma siguiente sería
        // restarle a un fotograma un trabajo que ocurrió en otro, que es la clase de error que este
        // instrumento persigue. Ver [`Ventana::cronometrado_aparte`] por qué existe.
        let aparte = std::mem::take(&mut self.cronometrado_aparte);

        // Sin tramos de nadie no hay resto que anotar: el fotograma entero se contaría como resto y la
        // serie acabaría midiendo sobre todo los fotogramas en los que no pasaba nada.
        if let Some(cronometrado) = cronometrado {
            self.resto_del_fotograma
                .anotar(coste_cpu, cronometrado + aparte);
        }
    }

    /// Bombea **todas** las consolas de **todas** las pestañas y las redimensiona, y resume lo que el
    /// fotograma tiene que decidir con eso.
    ///
    /// **Las de las pestañas que no se ven se bombean igual, con el mismo presupuesto**, y esto es la
    /// promesa del hito, no una elección de rendimiento. Recortárselo sería dejar de vaciar su
    /// pseudoterminal al ritmo que produce, y el margen que el núcleo regala antes de bloquear al
    /// programa son 20 KiB —seis veces menos que el propio presupuesto—, así que el programa de la
    /// pestaña de fondo **se pararía al escribir**. «Sigue corriendo aunque no la mires» y «consume
    /// menos por no mirarla» son incompatibles por construcción.
    ///
    /// **Lo que sí cuesta, y conviene no esconderlo:** el reparto lo hace el propio fotograma. Cada
    /// consola traga lo suyo y el fotograma se alarga, así que con varias volcando a la vez **todas**
    /// van más lentas, la visible incluida. «Siguen corriendo» no significa «a la misma velocidad que
    /// si las miraras».
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

        for division in self.todas_las_divisiones_mut() {
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
        for (que_consola, rect) in rectangulos.iter().enumerate() {
            // El clic se detecta con `interact` y no mirando la posición del puntero a pelo, porque así
            // respeta capas y orden de pintado. Y con `Sense::CLICK` en vez de `Sense::click()`: el
            // segundo incluye `FOCUSABLE`, o sea que el widget **pediría el foco de teclado de egui** y
            // competiría con el enrutado que hace la aplicación — que es geográfico y no por widget.
            let id = ui.id().with(("division", que_consola));
            if ui.interact(*rect, id, egui::Sense::CLICK).clicked() {
                self.activa_mut().foco = que_consola;
            }
        }
    }

    /// Cambia de pestaña al hacer clic en una ficha.
    ///
    /// Vive aparte del pintado por lo mismo que [`Self::atender_clics`], y usa `Sense::CLICK` por lo
    /// mismo también: `Sense::click()` añade `FOCUSABLE`, o sea que la ficha **pediría el foco de teclado
    /// de egui** y competiría con el enrutado de la aplicación, que es geográfico.
    fn atender_clics_de_la_barra(&mut self, ui: &egui::Ui, barra: egui::Rect) {
        for que_pestaña in 0..self.pestañas.len() {
            let id = ui.id().with(("pestaña", que_pestaña));
            if ui
                .interact(
                    ficha(barra, que_pestaña, self.pestañas.len()),
                    id,
                    egui::Sense::CLICK,
                )
                .clicked()
            {
                // **Por la misma puerta que el teclado, en vez de escribir `activa` a mano.** Son la
                // misma transición, y tenerla en dos sitios ya costó un defecto: el clic no pedía
                // fotograma, así que con dos consolas calladas —el caso de este hito— la pestaña recién
                // mostrada se quedaba con la rejilla de arranque hasta que llegara entrada de otra cosa.
                self.aplicar(
                    Orden::Ir(que_pestaña as isize - self.activa as isize),
                    ui.ctx(),
                );
            }
        }
    }

    /// Pinta la barra: una ficha por pestaña, con su número dentro.
    ///
    /// **Los números que se ven empiezan en uno**, no en cero como los de las consolas del informe: eso
    /// es un instrumento y esto es lo que lee quien tiene la ventana delante.
    ///
    /// El recorte a la ficha es lo que sostiene el reparto a partes iguales sin desplazamiento: cuando el
    /// número deja de caber se corta en su ficha en vez de invadir a la vecina.
    fn pintar_la_barra(&self, ui: &egui::Ui, barra: egui::Rect, metrica: &Metrica) {
        ui.painter_at(barra)
            .rect_filled(barra, 0.0, self.tema.fondo());

        for que_pestaña in 0..self.pestañas.len() {
            let sitio = ficha(barra, que_pestaña, self.pestañas.len());
            let pintor = ui.painter_at(sitio);
            let es_la_activa = que_pestaña == self.activa;

            // La activa se rellena con el mismo color que señala la consola con el foco: las dos dicen
            // «el teclado va aquí», a dos escalas distintas, y darles dos colores diría que son dos
            // estados que no tienen nada que ver.
            if es_la_activa {
                pintor.rect_filled(sitio, 0.0, self.tema.foco());
            }

            pintor.text(
                sitio.center(),
                egui::Align2::CENTER_CENTER,
                // El número se pasa **sin convertir a texto**: `Painter::text` recibe un `impl ToString`
                // y llama a `to_string` por dentro, así que darle un `String` ya hecho lo clona y asigna
                // dos veces por pestaña y por fotograma.
                que_pestaña + 1,
                metrica.fuente(),
                if es_la_activa {
                    self.tema.fondo()
                } else {
                    self.tema.primer_plano()
                },
            );
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
    fn teclear_la_del_foco(&mut self, ctx: &egui::Context) -> Vec<Orden> {
        let empezo = self.empezo_el_fotograma;
        // La semilla se saca y se vuelve a meter porque la división y el campo son dos préstamos
        // del mismo dueño, y el compilador no deja tener los dos a la vez.
        let mut modificadores = self.modificadores;
        let abiertas = self.pestañas.len();
        let pestaña = self.activa_mut();
        let ordenes =
            pestaña.divisiones[pestaña.foco].teclear(ctx, empezo, &mut modificadores, abiertas);
        self.modificadores = modificadores;
        ordenes
    }

    /// Hace lo que el acorde pidió, y **pide fotograma**.
    ///
    /// Lo segundo no es una precaución: hoy solo piden repintado la cola pendiente, el respaldo del
    /// vigilante ciego y el despertador de paneles. Con dos pestañas quietas —dos agentes esperando
    /// respuesta, que es cuando esto se usa— ninguno de los tres tiene nada que pedir, así que sin esta
    /// línea la ventana se dormiría **enseñando la pestaña anterior** y el acorde parecería no haber
    /// funcionado.
    fn aplicar(&mut self, orden: Orden, ctx: &egui::Context) {
        match orden {
            // El ciclo lo da `rem_euclid`, que a diferencia del resto de siempre **no devuelve
            // negativos**: desde la primera hacia atrás se llega a la última, sin una rama aparte para
            // ese caso.
            Orden::Ir(delta) => {
                let cuantas = self.pestañas.len() as isize;
                self.activa = (self.activa as isize + delta).rem_euclid(cuantas) as usize;
            }
            Orden::Crear => self.crear_una_pestaña(ctx),
            Orden::Cerrar => self.cerrar_la_pestaña_activa(ctx),
        }

        ctx.request_repaint();
    }

    /// Abre una pestaña con sus dos consolas y la deja delante.
    ///
    /// **Si alguna consola no se puede abrir, no se crea la pestaña**: se dice por la salida de errores y
    /// la ventana sigue exactamente como estaba. Es el mismo criterio que el directorio de panel —o los
    /// dos, o ninguno—, y por la misma razón: media pestaña se descubre al llegar a ella, que es tarde.
    fn crear_una_pestaña(&mut self, ctx: &egui::Context) {
        // **Abrir la pestaña son dos `fork`/`exec`, y se cronometran.** Ocurren dentro de la pasada de
        // interfaz —es donde se atiende el acorde— y ninguna división los va a declarar: la que los paga
        // es la pestaña que todavía no existe. Sin esto, lanzar dos procesos se publica como coste del
        // framework de interfaz.
        let antes = Instant::now();
        let nombres_de_division = [
            self.siguiente_nombre_de_division,
            self.siguiente_nombre_de_division + 1,
        ];
        let abierta = Pestaña::nueva(ctx, self.siguiente_nombre, nombres_de_division);
        self.cronometrado_aparte += antes.elapsed();
        // **Los nombres se gastan aunque la apertura falle**, para que no se reutilicen nunca: si el
        // intento fallido dejó una línea en la salida de errores, lo que se abra después no debe llamarse
        // igual que lo que no llegó a existir. Vale para la pestaña y para sus dos divisiones por la
        // misma razón, así que se avanzan los dos contadores en el mismo sitio.
        self.siguiente_nombre += 1;
        self.siguiente_nombre_de_division += nombres_de_division.len();

        self.poner_delante(abierta, ctx);
    }

    /// Pone delante la pestaña que se acaba de abrir, o deja la ventana como estaba si no se pudo abrir.
    ///
    /// **Recibe el intento ya hecho en vez de hacerlo, y es por lo mismo que [`panel_o_ninguno`]:** abrir
    /// una consola solo falla cuando el sistema no da más pseudoterminales o descriptores, y eso no se
    /// puede provocar desde una prueba sin tocar el proceso entero. Partirlo aquí es lo que permite
    /// afirmar lo que el caso promete — que un fallo no deja media pestaña, no mueve la que se estaba
    /// viendo y no rearma nada.
    fn poner_delante(&mut self, abierta: term_core::Result<Pestaña>, ctx: &egui::Context) {
        let pestaña = match abierta {
            Ok(pestaña) => pestaña,
            Err(fallo) => {
                eprintln!("no se pudo abrir la pestaña ({fallo})");
                return;
            }
        };

        self.pestañas.push(pestaña);
        self.activa = self.pestañas.len() - 1;
        self.rearmar_el_vigilante(ctx);
    }

    /// Cierra la pestaña que se está viendo. Si es la única, lo que se cierra es la ventana.
    ///
    /// **Rearmar el vigilante es imprescindible**: sin ello se queda siguiendo dos directorios que acaban
    /// de borrarse, ve desaparecer lo que vigilaba y se declara ciego. A partir de ahí la ventana releería
    /// todos los paneles en cada fotograma, para siempre y en el modo degradado, sin que nada en pantalla
    /// lo diga. Hay una prueba que cae si se quita.
    ///
    /// **Se rearma después de soltar la pestaña, y el diseño pedía lo contrario.** Su razón era que el
    /// vigilante viejo no llegase a ver el borrado y se declarara ciego; comprobado invirtiéndolo, **el
    /// resultado observable es el mismo**, porque el vigilante nuevo trae su propia bandera de ceguera y
    /// el viejo se descarta entero con la suya. Sin diferencia que defender, se hace en el orden que no
    /// necesita saber qué pestaña se va.
    fn cerrar_la_pestaña_activa(&mut self, ctx: &egui::Context) {
        // **La última no se quita de la lista, y no es una excepción caprichosa:** pedir el cierre no
        // cierra en el acto —todavía corren fotogramas, y el informe se publica al salir—, así que un
        // vector vacío dejaría a `activa` indexando fuera de rango en cada uno de ellos.
        if self.pestañas.len() == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let que_se_va = self.activa;
        self.cerrada_alguna_a_mano = true;

        // **Lo que sus consolas ya habían declarado de este fotograma se recoge antes de soltarlas.** Se
        // van con su `Medida` dentro, y ahí puede haber una vuelta anotada de este mismo fotograma: bytes
        // que se interpretaron de verdad hace un instante. Si se marcha sin declararse, la resta se lo
        // atribuye al framework de interfaz y el «resto» sale inflado justo en el fotograma en que se
        // cierra una pestaña. El coste del cierre en sí —matar los programas y borrar sus directorios,
        // que pasa al soltar la pestaña— entra en el mismo saco por la misma razón.
        let antes = Instant::now();
        let suyo: std::time::Duration = self.pestañas[que_se_va]
            .divisiones
            .iter_mut()
            .map(|division| division.medida.llevarse_lo_pendiente())
            .sum();

        // Aquí mueren sus dos programas y desaparecen sus dos directorios, por los `Drop` de siempre.
        self.pestañas.remove(que_se_va);
        self.cronometrado_aparte += suyo + antes.elapsed();

        self.rearmar_el_vigilante(ctx);
        // Al quitar la última de la lista el índice se queda fuera; en cualquier otro caso, se queda
        // apuntando a la que ocupó su sitio, que es la de la derecha.
        self.activa = que_se_va.min(self.pestañas.len() - 1);
    }

    /// Vuelve a armar el vigilante sobre las divisiones de todas las pestañas que quedan.
    ///
    /// **Después se levantan todas las banderas, y lo que eso salva no es un hueco de tiempo.** El
    /// vigilante nuevo se construye antes de asignarlo, así que el viejo sigue armado hasta el último
    /// momento y no hay instante sin nadie escuchando — una versión anterior de este comentario decía que
    /// sí, y era falso. Lo que sí se pierde es **lo que el viejo tenía en vuelo**: su hilo puede llevar
    /// avisos recibidos y aún sin procesar, y al soltarlo se van con él. Un productor que publicara en ese
    /// momento se quedaría sin que nadie levantara su bandera, y su panel enseñaría lo de antes hasta la
    /// siguiente publicación — que en un agente que acaba de terminar puede no llegar nunca.
    ///
    /// Releerlos todos una vez es barato y no puede perder nada. **Que las banderas acaben levantadas sí
    /// está atado por prueba; el orden entre armar y levantarlas, no** — provocar una publicación dentro
    /// de esa ventana de microsegundos no se puede hacer de forma fiable desde un test.
    fn rearmar_el_vigilante(&mut self, ctx: &egui::Context) {
        // El vigilante se construye en su propio ámbito para que el préstamo de la lista de pestañas
        // termine antes de asignarlo: el campo y la lista son del mismo dueño.
        let vigilante = Self::vigilar(ctx, self.todas_las_divisiones());

        self.vigilante = vigilante;
        for division in self.todas_las_divisiones() {
            division.por_releer.levantar();
        }
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
        //
        // **Y el contador tiene que ser el de fotogramas, no el de pasadas.** Esto se escribió con
        // `cumulative_pass_nr` y así no filtraba nada, porque ése **sube en cada pasada**: en un
        // fotograma repetido daba dos valores distintos y el segundo cierre entraba igual. Leído en su
        // fuente (`egui-0.36/src/context.rs:1748-1791`): `cumulative_frame_nr` se incrementa **una vez
        // al final de cada `run_ui`**, `cumulative_pass_nr` **una por pasada completada**, y la
        // documentación dice que el primero es siempre menor o igual que el segundo. Los dos se
        // incrementan al terminar, así que dentro de las dos pasadas de un mismo fotograma el de
        // fotogramas vale lo mismo — y eso es lo que hace de guarda. Lo ata
        // `egui_cuenta_los_fotogramas_una_vez_y_las_pasadas_todas`.
        //
        // **Estas cuatro líneas no las cubre ninguna prueba de la ventana, y conviene saberlo**:
        // `eframe::Frame::_new_kittest()` entrega `cpu_usage: None` —comprobado—, así que por este
        // camino `cerrar_fotograma` no llega a llamarse nunca desde una prueba. Lo que sí está probado
        // es el contrato de egui del que depende la guarda, y el cierre en sí entra por
        // [`Ventana::cerrar_fotograma`], que las pruebas llaman directamente. Es la misma frontera
        // declarada de [`Ventana::on_exit`] y [`pestañas_al_arrancar`].
        let fotograma = ctx.cumulative_frame_nr();
        if fotograma != self.ultimo_fotograma_cerrado
            && let Some(segundos) = frame.info().cpu_usage
        {
            self.ultimo_fotograma_cerrado = fotograma;
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
        //
        // **Todas, también las que no se ven**, al revés que el despertador: levantar una bandera es
        // una escritura atómica y lo que compra es que al volver a una pestaña su panel esté al día.
        // Por qué el despertador sí se acota, en [`Ventana::poner_el_despertador`].
        if self.vigilante.esta_ciego() {
            for division in self.todas_las_divisiones() {
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
        // **Salvo que alguna pestaña se haya cerrado a mano**: ver [`Ventana::cerrada_alguna_a_mano`].
        if todas_terminadas && !self.cerrada_alguna_a_mano {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let metrica = Metrica::medir(ui.ctx(), egui::FontId::monospace(CUERPO));

        // El teclado va antes de todo lo demás **y ese orden importa para el número, no para el
        // comportamiento**: el cronómetro de la ida se cierra dentro de `teclear`, así que cualquier
        // trabajo que se cuele por delante —aunque sea leer un rectángulo— se le suma.
        let ordenes = self.teclear_la_del_foco(ui.ctx());

        // **La orden se aplica antes de repartir**, para que lo que se pinte en este mismo fotograma sea
        // ya la pestaña nueva. Aplicarla al final dejaría un fotograma enseñando la anterior, que con la
        // ventana en reposo es el último que se pinta.
        for orden in ordenes {
            self.aplicar(orden, ui.ctx());
        }

        let (barra, area) = apartar_la_barra(ui.max_rect(), self.pestañas.len(), &metrica);

        // **Todo lo que cambia de pestaña va antes de repartir**, y esto es lo que costó un defecto: el
        // reparto sale de la fracción de la pestaña activa, así que atender el clic después dejaba un
        // fotograma pintando **las consolas nuevas con el divisor de la vieja**. Y no solo se veía mal:
        // de esos rectángulos sale `medido`, con lo que los dos pseudoterminales de la pestaña recién
        // abierta recibían una rejilla equivocada y la corregían al fotograma siguiente — un reajuste
        // doble y visible en cualquier programa de pantalla completa.
        if let Some(barra) = barra {
            self.atender_clics_de_la_barra(ui, barra);
        }

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

        // La barra se pinta antes que las consolas por lo mismo que el divisor: lo que se pinte después
        // queda por encima, y el glifo del borde de una consola que se saliera ya lo corta su recorte.
        // Su clic se atendió mucho antes, con el resto de lo que puede cambiar de pestaña.
        if let Some(barra) = barra {
            self.pintar_la_barra(ui, barra, &metrica);
        }

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

        // Las que no se pintan no tienen otro sitio donde enterarse de su tamaño: ésta es la única
        // mitad del fotograma que conoce el área que les tocaría.
        self.medir_las_que_no_se_ven(area, &metrica);

        // Ya se ha pintado, así que lo que siga pendiente es de una consola que no se pintó.
        self.cerrar_las_vueltas_no_pintadas();

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
    /// **Estas dos líneas no las cubre ninguna prueba, y conviene saberlo**: capturar la salida
    /// estándar del propio proceso no se puede hacer desde un test sin maquinaria que no compensa,
    /// así que las pruebas entran por [`Ventana::publicar_informe`]. Vaciar esto dejaría la suite
    /// entera en verde con el informe mudo — el mismo hueco que ya costó una vez en este proyecto.
    /// Lo que lo tapa hoy no es una prueba, es el uso: quien mide se encuentra el fichero de la tanda
    /// vacío en cuanto lo abre. Por eso aquí no puede haber **nada** más que la llamada.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.publicar_informe(&mut std::io::stdout().lock())
            .expect("escribir el informe por la salida estándar");
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
        let mut division = division_con("sleep 30", 0);
        // La ventana guarda esto entre fotogramas; aquí lo hace la variable, que es lo mismo.
        let mut modificadores = egui::Modifiers::NONE;

        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(con_mayusculas()),
                egui::Event::Paste("hola".to_owned()),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
        assert_eq!(division.pulsaciones, b"hola", "con Mayúsculas no se pegó");

        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                egui::Event::Paste("hola".to_owned()),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
        let mut division = division_con("sleep 30", 0);
        let mut modificadores = egui::Modifiers::NONE;

        // Sin que el programa lo pida, el texto va desnudo.
        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(con_mayusculas()),
                egui::Event::Paste("ls\n".to_owned()),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
        let mut division = division_con("sleep 30", 0);
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
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
        let mut division = division_con("sleep 30", 0);
        let mut modificadores = egui::Modifiers::NONE;

        encolar(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(con_mayusculas()),
                egui::Event::WindowFocused(false),
            ],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);

        encolar(
            &ctx,
            vec![egui::Event::Paste("no deberías verme".to_owned())],
        );
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
        let mut division = division_con("sleep 30", 0);
        // La ventana guarda esto entre fotogramas; aquí lo hace la variable, que es lo mismo.
        let mut modificadores = egui::Modifiers::NONE;

        encolar(&ctx, vec![egui::Event::ModifiersChanged(con_mayusculas())]);
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);

        // **Vaciar la cola es lo que convierte esto en dos fotogramas de verdad.** egui rehace la
        // lista de eventos en cada uno, así que el aviso de que se pulsaron las teclas no sigue ahí
        // al siguiente. Sin este vaciado la prueba se aprobaría sola: volvería a leer el mismo aviso
        // que dejó el pase anterior, y no ejercitaría la semilla en absoluto.
        ctx.input_mut(|entrada| entrada.events.clear());

        encolar(&ctx, vec![egui::Event::Paste("tarde".to_owned())]);
        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
        let mut division = division_con("sleep 30", 0);
        // La ventana guarda esto entre fotogramas; aquí lo hace la variable, que es lo mismo.
        let mut modificadores = egui::Modifiers::NONE;

        for acorde in [egui::Modifiers::CTRL, con_mayusculas()] {
            encolar(
                &ctx,
                vec![egui::Event::ModifiersChanged(acorde), egui::Event::Copy],
            );
            division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
            assert_eq!(division.pulsaciones, b"\x03", "dejó de interrumpir");

            encolar(
                &ctx,
                vec![egui::Event::ModifiersChanged(acorde), egui::Event::Cut],
            );
            division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
    fn division_con(programa: &str, nombre: usize) -> Division {
        let (pty, salida) = Pty::spawn("sh", &["-c", programa], REJILLA_INICIAL)
            .expect("no se pudo abrir el pseudoterminal");

        Division {
            nombre,
            pty,
            puente: Puente::nuevo(salida, || {}),
            panel: None,
            estado_del_panel: panel::Estado::default(),
            por_releer: Bandera::levantada(),
            pantalla: Screen::new(REJILLA_INICIAL),
            medida: Medida::default(),
            pendiente_de_medir: None,
            panel_sin_pintar: std::time::Duration::ZERO,
            recibido: Vec::new(),
            pulsaciones: Vec::new(),
            medido: REJILLA_INICIAL,
        }
    }

    /// Monta una ventana con dos consolas, cada una con su programa, sin abrir ninguna ventana de
    /// verdad.
    ///
    /// Lo que aquí se prueba —bombear, decidir el cierre, enrutar el teclado— no toca el escritorio,
    /// así que se arma el struct a mano con las mismas piezas que [`Ventana::con_pestañas`].
    fn ventana_con(izquierda: &str, derecha: &str) -> Ventana {
        ventana_de_pestañas([[izquierda, derecha]])
    }

    /// Monta una ventana con **las pestañas que se le pidan**, cada consola con su programa.
    ///
    /// Existe porque la aplicación **todavía no sabe abrir una segunda pestaña** —crearlas es parte
    /// de F2.1, en un paso posterior— y sin esto no habría forma de comprobar lo que este paso
    /// promete: que una consola de una pestaña que no se ve se sigue vaciando. La primera de la lista
    /// queda activa.
    fn ventana_de_pestañas<'a>(programas: impl IntoIterator<Item = [&'a str; 2]>) -> Ventana {
        let pestañas: Vec<_> = programas
            .into_iter()
            .enumerate()
            .map(|(nombre, [izquierda, derecha])| {
                // Con su nombre, como las abre la aplicación: si todas se llamaran igual, el informe
                // diría «pestaña 0» dos veces y la prueba que lo lee no notaría la diferencia. Y sus
                // divisiones **numeradas seguidas entre pestañas**, también como la aplicación: con un
                // contador por pestaña habría dos «división 0» en el mismo informe.
                Pestaña::con(
                    [
                        division_con(izquierda, nombre * 2),
                        division_con(derecha, nombre * 2 + 1),
                    ],
                    nombre,
                )
            })
            .collect();

        // Sin pestañas, `activa: 0` apunta fuera de la lista y el pánico saldría dentro de la
        // aplicación, lejos de la prueba que lo causó. Se ata donde se comete.
        assert!(
            !pestañas.is_empty(),
            "una ventana necesita al menos una pestaña"
        );

        let siguiente_nombre = pestañas.len();
        let siguiente_nombre_de_division = pestañas
            .iter()
            .map(|pestaña| pestaña.divisiones.len())
            .sum();

        Ventana {
            // Estas consolas no tienen directorio, así que no hay nada que vigilar; el vigilante se
            // arma igual —sobre una lista vacía— para que la ventana de las pruebas no se declare
            // ciega y active el respaldo, que es un camino con su propia prueba.
            //
            // Se le pasan las de todas las pestañas por lo mismo que hace la aplicación al armarlo,
            // aunque hoy dé igual: sin directorio, la lista sale vacía se le pase lo que se le pase.
            vigilante: Ventana::vigilar(
                &egui::Context::default(),
                pestañas
                    .iter()
                    .flat_map(|pestaña| pestaña.divisiones.iter()),
            ),
            pestañas,
            activa: 0,
            modificadores: egui::Modifiers::NONE,
            tema: Tema::default(),
            resto_del_fotograma: RestoDelFotograma::default(),
            empezo_el_fotograma: Instant::now(),
            ultimo_fotograma_cerrado: 0,
            cronometrado_aparte: std::time::Duration::ZERO,
            siguiente_nombre,
            siguiente_nombre_de_division,
            cerrada_alguna_a_mano: false,
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
        let mut division = division_con("printf hola", 0);
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
        let mut division = division_con(
            &format!("head -c {VOLCADO} /dev/zero | tr '\\0' a; printf '\\r\\nFIN'"),
            0,
        );
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
        let mut division = division_con("sleep 30", 0);
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
        let mut division = division_con("while :; do stty size; sleep 0.05; done", 0);

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
        let mut division = division_con("sleep 30", 0);
        let ctx = egui::Context::default();
        let mut modificadores = egui::Modifiers::NONE;
        ctx.input_mut(|entrada| {
            entrada.events.push(egui::Event::Text("a".to_owned()));
            entrada.events.push(egui::Event::WindowFocused(true));
        });

        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
        assert_eq!(division.pulsaciones, b"a", "no se envió lo tecleado");

        division.teclear(&ctx, Instant::now(), &mut modificadores, 2);
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
        // a mano, que se fija el foco ella misma: seguiría verde si `Ventana::con_pestañas` arrancara apuntando
        // a la derecha, o a un índice que no existe. Teclear antes del primer clic ocurre siempre al
        // abrir, y esas teclas no pueden perderse.
        let ventana = Ventana::con_pestañas(&egui::Context::default(), 1)
            .expect("no se pudo abrir la ventana con sus dos consolas");

        assert_eq!(
            ventana.activa().foco,
            0,
            "el foco no arranca en la consola izquierda"
        );
    }

    #[test]
    fn pedir_una_sola_pestaña_abre_exactamente_una_y_apuntando_a_ella() {
        // Las dos afirmaciones de las que cuelga todo lo demás, y ninguna se ve desde `activa()`:
        // ese acceso da la pestaña buena tanto si hay una como si hay siete, así que esconde
        // exactamente lo que aquí se quiere fijar. Sin esto, abrir con dos pestañas —o con el índice
        // apuntando a una que no existe, que es un pánico en el primer fotograma— pasaría inadvertido
        // hasta que alguien mirase la ventana.
        //
        // **Con cuántas abre la aplicación cuando nadie pide nada** ya no se decide aquí sino en
        // `arrancar`, y lo ata `la_variable_de_entorno_dice_cuantas_pestañas_se_abren`.
        let ventana = Ventana::con_pestañas(&egui::Context::default(), 1)
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
        let ventana = Ventana::con_pestañas(&egui::Context::default(), 1)
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
    /// sin atar justo lo que no se puede montar a mano: lo que `Ventana::con_pestañas` le pasa. Cambiar ahí
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
        let ventana = Ventana::con_pestañas(&egui::Context::default(), 1)
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
        let mut division = Division::lanzando(&egui::Context::default(), "sh", 0)
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
        let mut division = Division::con_panel(&egui::Context::default(), "env", None, 0)
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
            let ventana = Ventana::con_pestañas(&egui::Context::default(), 1)
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

    /// Las rutas de los directorios de panel de una pestaña, para poder mirar si siguen ahí después.
    fn directorios_de(pestaña: &Pestaña) -> [std::path::PathBuf; 2] {
        pestaña.divisiones.each_ref().map(|division| {
            division
                .panel
                .as_ref()
                .expect("la consola tiene que arrancar con su directorio")
                .path()
                .to_path_buf()
        })
    }

    #[test]
    fn el_acorde_de_crear_abre_una_pestaña_delante_y_no_le_llega_al_programa() {
        // AC2. Sin consumir el evento el programa recibiría `0x14` —Ctrl+T es un código de control como
        // cualquier otro— y en un shell eso intercambia dos caracteres: el acorde escribiría en el
        // programa de la pestaña que se acaba de dejar.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 1).expect("no se pudo abrir la ventana");

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::T));

        assert_eq!(ventana.pestañas.len(), 2, "no se abrió la pestaña");
        assert_eq!(ventana.activa, 1, "la pestaña nueva no quedó delante");
        assert!(
            ventana.pestañas[0].divisiones[0].pulsaciones.is_empty(),
            "el acorde de crear se le coló al programa"
        );
        // Con su propia identidad: dos consolas que compartieran directorio se pisarían el estado.
        let [de_la_vieja, de_la_nueva] = [0, 1].map(|cual| directorios_de(&ventana.pestañas[cual]));
        for nueva in &de_la_nueva {
            assert!(
                !de_la_vieja.contains(nueva),
                "la pestaña nueva reutilizó el directorio de la vieja"
            );
        }
    }

    #[test]
    fn crear_con_varias_abiertas_pone_la_nueva_al_final_y_delante() {
        // AC2 con la lista ya poblada, que es donde «se abre una pestaña» deja de tener una sola lectura:
        // con una única pestaña, ponerla al final y ponerla junto a la activa son lo mismo. Va al final
        // porque la barra pinta las fichas en el orden de la lista, y meterla en medio movería de sitio a
        // las de la derecha mientras alguien las está mirando.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 3).expect("no se pudo abrir la ventana");
        let de_cada_una: Vec<_> = ventana.pestañas.iter().map(directorios_de).collect();

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::T));

        assert_eq!(ventana.pestañas.len(), 4, "no se abrió la pestaña");
        assert_eq!(
            ventana.activa, 3,
            "la pestaña nueva no quedó al final y delante"
        );
        let siguen: Vec<_> = ventana.pestañas[..3].iter().map(directorios_de).collect();
        assert_eq!(
            siguen, de_cada_una,
            "la pestaña nueva descolocó a las que ya estaban abiertas"
        );
    }

    #[test]
    fn una_pestaña_que_no_se_pudo_abrir_deja_la_ventana_como_estaba() {
        // El otro lado de AC2: media pestaña —una consola abierta y la otra no— se descubriría al llegar
        // a ella, que es tarde, así que o las dos o ninguna. Y «como estaba» incluye **cuál se está
        // viendo**: mover la activa dando por hecho que la pestaña se creó cambiaría por un fallo del
        // sistema lo que la ventana enseña, y si la mueve al sitio que la nueva iba a ocupar, además
        // indexa fuera de la lista.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
        ]);

        ventana.poner_delante(Err("no quedan pseudoterminales".into()), &ctx);

        assert_eq!(
            ventana.pestañas.len(),
            3,
            "se coló media pestaña en la lista"
        );
        assert_eq!(
            ventana.activa, 0,
            "la ventana dejó de enseñar la pestaña que se estaba viendo"
        );
        for (cual, division) in ventana.todas_las_divisiones().enumerate() {
            assert!(
                division.puente.sigue_vivo(),
                "el fallo al abrir se llevó por delante el programa de la consola {cual}"
            );
        }
    }

    #[test]
    fn el_acorde_de_cerrar_mata_sus_programas_y_borra_sus_directorios() {
        // AC3. Lo que se comprueba no es que la pestaña desaparezca de la lista —eso lo diría cualquier
        // `remove`— sino que **lo que colgaba de ella se ha soltado**: sus dos directorios de panel ya no
        // están en disco, y la otra pestaña sigue entera.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 2).expect("no se pudo abrir la ventana");
        ventana.activa = 1;

        let de_la_que_se_va = directorios_de(&ventana.pestañas[1]);
        let de_la_que_queda = directorios_de(&ventana.pestañas[0]);

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::W));

        assert_eq!(ventana.pestañas.len(), 1, "la pestaña no se cerró");
        assert_eq!(ventana.activa, 0, "la activa quedó fuera de la lista");
        for ruta in &de_la_que_se_va {
            assert!(
                !ruta.exists(),
                "quedó el directorio {} de la pestaña cerrada",
                ruta.display()
            );
        }
        for ruta in &de_la_que_queda {
            assert!(
                ruta.exists(),
                "se llevó por delante el directorio {} de la otra pestaña",
                ruta.display()
            );
        }
        assert!(
            ventana.pestañas[0].divisiones[0].pulsaciones.is_empty(),
            "el acorde de cerrar se le coló al programa"
        );
    }

    #[test]
    fn cerrar_la_del_medio_deja_delante_la_de_su_derecha_y_no_descoloca_las_demas() {
        // AC3 con más de dos pestañas, que es donde recolocar la activa dice algo. Con dos, casi cualquier
        // cuenta acierta —quedarse en el sitio, restar uno o irse a la primera dan todas cero—, así que
        // hacen falta cuatro para separar lo correcto de lo que se le parece: al cerrar la segunda queda
        // delante la que estaba a su derecha, y las demás **siguen donde estaban**, que es lo que la barra
        // enseña y lo que las flechas recorren.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 4).expect("no se pudo abrir la ventana");
        let de_cada_una: Vec<_> = ventana.pestañas.iter().map(directorios_de).collect();
        ventana.activa = 1;

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::W));

        let quedan: Vec<_> = ventana.pestañas.iter().map(directorios_de).collect();
        let esperadas = vec![
            de_cada_una[0].clone(),
            de_cada_una[2].clone(),
            de_cada_una[3].clone(),
        ];
        assert_eq!(
            quedan, esperadas,
            "cerrar la del medio descolocó a las que quedaban"
        );
        assert_eq!(
            directorios_de(&ventana.pestañas[ventana.activa]),
            de_cada_una[2],
            "delante no quedó la pestaña que estaba a la derecha de la que se cerró"
        );
        for ruta in &de_cada_una[1] {
            assert!(
                !ruta.exists(),
                "quedó el directorio {} de la pestaña cerrada",
                ruta.display()
            );
        }
    }

    #[test]
    fn dos_cierres_en_el_mismo_lote_cierran_dos_pestañas() {
        // Un lote acumulado mientras la ventana dormía puede traer dos acordes de cerrar, y se aplican los
        // dos en orden. Lo que esto ata es que el segundo cae sobre la lista **ya recortada**: cierra la
        // pestaña que el primero dejó delante, no la que ocupaba ese índice antes.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 3).expect("no se pudo abrir la ventana");
        let de_la_primera = directorios_de(&ventana.pestañas[0]);
        ventana.activa = 2;

        let pidio_cerrar = un_fotograma_con(&mut ventana, &ctx, dos_veces_el_acorde(egui::Key::W));

        assert_eq!(
            ventana.pestañas.len(),
            1,
            "los dos cierres del lote no cerraron dos pestañas"
        );
        assert_eq!(
            directorios_de(&ventana.pestañas[0]),
            de_la_primera,
            "el segundo cierre se llevó una pestaña que no era la que quedó delante"
        );
        assert!(
            !pidio_cerrar,
            "cerrar dos de tres pestañas pidió además cerrar la ventana"
        );
    }

    #[test]
    fn cerrar_dos_veces_la_unica_pestaña_no_hunde_la_cuenta_de_las_abiertas() {
        // Cerrar la última no quita nada de la lista —lo que pide es cerrar la ventana—, así que la cuenta
        // de pestañas que se lleva **dentro** del lote no puede bajar de uno. Restando sin suelo, el
        // segundo acorde de un lote con una sola pestaña abierta deja la cuenta por debajo de cero.
        let ctx = egui::Context::default();
        let mut ventana = ventana_con("sleep 30", "sleep 30");

        let pidio_cerrar = un_fotograma_con(&mut ventana, &ctx, dos_veces_el_acorde(egui::Key::W));

        assert!(
            pidio_cerrar,
            "dos acordes de cerrar sobre la única pestaña no pidieron cerrar la ventana"
        );
        assert_eq!(
            ventana.pestañas.len(),
            1,
            "la última pestaña se quitó de la lista"
        );
    }

    #[test]
    fn cerrar_la_ultima_pestaña_pide_cerrar_la_ventana_sin_vaciar_la_lista() {
        // AC4, y la parte que importa es la segunda mitad. Pedir el cierre **no cierra en el acto**: aún
        // corren fotogramas, y el informe se publica al salir. Si la última se quitara de la lista, cada
        // uno de esos fotogramas indexaría fuera de rango — un pánico al cerrar la ventana, que es el
        // peor momento para tenerlo porque se lleva por delante el informe.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 1).expect("no se pudo abrir la ventana");

        let pidio_cerrar = un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::W));

        // La primera mitad del criterio, y no la daba por buena ninguna prueba: sin esto, el acorde sobre
        // la única pestaña no haría absolutamente nada y la suite seguiría en verde.
        assert!(
            pidio_cerrar,
            "cerrar la única pestaña no pidió cerrar la ventana"
        );
        assert_eq!(
            ventana.pestañas.len(),
            1,
            "la última pestaña se quitó de la lista"
        );
        assert!(
            ventana.activa < ventana.pestañas.len(),
            "la pestaña activa apunta fuera de la lista"
        );

        // Y los fotogramas que aún corren no entran en pánico, que es lo que la invariante compra.
        for _ in 0..3 {
            un_fotograma_con(&mut ventana, &ctx, Vec::new());
        }
    }

    #[test]
    fn mantener_pulsado_el_acorde_de_cerrar_no_cierra_mas_que_una_pestaña() {
        // **De un solo gesto se perdía todo.** egui marca las repeticiones de una tecla mantenida con
        // `repeat`, pero les sigue poniendo `pressed`, así que un segundo con el acorde apretado son unas
        // treinta órdenes de cierre en un mismo lote — y como se aplican todas en orden, se cerraban todas
        // las pestañas, morían todos sus programas y la última cerraba la ventana. Sin vuelta atrás.
        //
        // La repetición **se consume igual**: devolverla a la cola le mandaría al programa el byte de
        // control de cada repetición.
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30", 0);
        let mut modificadores = egui::Modifiers::NONE;

        let mut lote = acorde_con(egui::Key::W);
        for _ in 0..5 {
            lote.push(egui::Event::Key {
                key: egui::Key::W,
                physical_key: None,
                pressed: true,
                repeat: true,
                modifiers: con_mayusculas(),
            });
        }
        encolar(&ctx, lote);

        let ordenes = division.teclear(&ctx, Instant::now(), &mut modificadores, 4);

        assert_eq!(
            ordenes,
            vec![Orden::Cerrar],
            "las repeticiones de la tecla mantenida pidieron cerrar más pestañas"
        );
        assert!(
            division.pulsaciones.is_empty(),
            "las repeticiones se le colaron al programa"
        );
    }

    #[test]
    fn una_flecha_tras_crear_en_el_mismo_lote_no_se_le_cuela_al_programa() {
        // Con una sola pestaña la flecha es del programa, pero **el propio lote puede haber creado otra**.
        // Mirando cuántas hay una sola vez, antes de recorrerlo, la flecha se evaluaba contra la cuenta
        // vieja, caía en la traducción de siempre y le llegaba `\x1b[C` — exactamente lo que el corte
        // existe para impedir.
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30", 0);
        let mut modificadores = egui::Modifiers::NONE;

        let mut lote = acorde_con(egui::Key::T);
        lote.extend(acorde_con(egui::Key::ArrowRight));
        encolar(&ctx, lote);

        let ordenes = division.teclear(&ctx, Instant::now(), &mut modificadores, 1);

        assert_eq!(
            ordenes,
            vec![Orden::Crear, Orden::Ir(1)],
            "la flecha que seguía a crear no se atendió"
        );
        assert!(
            division.pulsaciones.is_empty(),
            "la flecha se le coló al programa tras crear una pestaña en el mismo lote"
        );
    }

    #[test]
    fn una_flecha_tras_cerrar_en_el_mismo_lote_le_llega_al_programa() {
        // El reverso de la de crear, y falla por el mismo sitio. Con dos pestañas la flecha es del acorde;
        // pero si el lote ya ha pedido cerrar una, cuando la flecha se aplique quedará **una sola**, y con
        // una sola la flecha es del programa. Sin llevar la cuenta al día dentro del lote, el acorde se la
        // tragaría para ir de la única pestaña a ella misma.
        let ctx = egui::Context::default();
        let mut division = division_con("sleep 30", 0);
        let mut modificadores = egui::Modifiers::NONE;

        let mut lote = acorde_con(egui::Key::W);
        lote.extend(acorde_con(egui::Key::ArrowRight));
        encolar(&ctx, lote);

        let ordenes = division.teclear(&ctx, Instant::now(), &mut modificadores, 2);

        assert_eq!(
            ordenes,
            vec![Orden::Cerrar],
            "la flecha que seguía a cerrar se pidió como cambio de pestaña"
        );
        assert_eq!(
            division.pulsaciones, b"\x1b[C",
            "la flecha no le llegó al programa quedando una sola pestaña"
        );
    }

    #[test]
    fn cerrar_una_pestaña_a_mano_no_cierra_la_ventana_con_lo_que_quedaba_por_leer() {
        // El caso aparece justo cuando cerrar se vuelve posible: una pestaña cuyos dos programas ya
        // terminaron —y cuyo último contenido se está leyendo— junto a otra con un agente vivo. Al cerrar
        // la del agente no queda nada vivo, y la regla de siempre cerraría la ventana llevándose lo que se
        // estaba mirando. Nadie pidió eso: se pidió cerrar una pestaña.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["printf leeme", "printf leeme"], ["sleep 30", "sleep 30"]]);
        for division in &ventana.pestañas[0].divisiones {
            esperar_a_que_muera(division);
        }
        ventana.activa = 1;

        ventana.aplicar(Orden::Cerrar, &ctx);

        assert_eq!(ventana.pestañas.len(), 1, "no se cerró la pestaña");
        let mut frame = eframe::Frame::_new_kittest();
        let salida = ctx.run_logic(&egui::RawInput::default(), |ctx| {
            eframe::App::logic(&mut ventana, ctx, &mut frame);
        });

        assert!(
            !salida
                .viewport_commands
                .get(&egui::ViewportId::ROOT)
                .is_some_and(|ordenes| ordenes.contains(&egui::ViewportCommand::Close)),
            "la ventana se cerró sola tras un cierre a mano, con lo que quedaba por leer"
        );
    }

    #[test]
    fn tras_crear_y_cerrar_el_vigilante_sigue_viendo_lo_que_queda() {
        // AC9. Es la comprobación que no se ve desde fuera: el vigilante se arma **entero** cada vez, así
        // que si al cerrar se soltara la pestaña antes de rearmarlo, el vigilante viejo vería desaparecer
        // los directorios que estaba siguiendo y se declararía ciego. A partir de ahí la ventana releería
        // todos los paneles en cada fotograma, para siempre, sin que nada en pantalla lo diga.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 2).expect("no se pudo abrir la ventana");
        assert!(
            !ventana.vigilante.esta_ciego(),
            "la ventana abrió ya ciega, así que esta prueba no probaría nada"
        );

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::T));
        assert!(
            !ventana.vigilante.esta_ciego(),
            "crear una pestaña dejó ciego al vigilante"
        );

        ventana.activa = 1;
        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::W));
        assert!(
            !ventana.vigilante.esta_ciego(),
            "cerrar una pestaña dejó ciego al vigilante: se soltó antes de rearmarlo"
        );
    }

    #[test]
    fn crear_una_pestaña_vuelve_a_armar_el_vigilante_que_estaba_ciego() {
        // AC9 en el caso que el diseño da por sentado para **no** llevar reintento propio: armar el
        // vigilante se abandona entero en cuanto un `watch` falla, y a partir de ahí la ventana relee
        // todos los paneles cada 500 ms. Como cada alta lo rearma entero, un fallo pasajero deja de doler
        // en cuanto se abre la siguiente pestaña; sin ese rearme, la ceguera duraría toda la sesión.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 1).expect("no se pudo abrir la ventana");
        // Armarlo sobre una ruta que no existe es la forma de conseguir uno ciego sin agotar los del
        // núcleo, que es lo que lo provoca de verdad.
        ventana.vigilante = Vigilante::nuevo(
            || {},
            &[(
                std::path::Path::new("/no/existe/ningun/directorio/de/panel"),
                Bandera::levantada(),
            )],
        );
        assert!(
            ventana.vigilante.esta_ciego(),
            "no se pudo montar la ceguera que esta prueba necesita"
        );

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::T));

        assert!(
            !ventana.vigilante.esta_ciego(),
            "abrir una pestaña no rearmó el vigilante, así que la ceguera dura el resto de la sesión"
        );
    }

    #[test]
    fn la_ventana_se_entera_de_lo_que_publica_una_pestaña_recien_creada() {
        // AC9, y es el hueco que `esta_ciego()` **no** ve: si al crear no se rearmara el vigilante, el
        // viejo seguiría perfectamente sano —vigilando las pestañas de antes— y no estaría ciego. Lo que
        // no haría es mirar la pestaña nueva, así que su panel no se releería **nunca**: un agente que
        // publique ahí trabajaría con un panel congelado desde el primer fotograma. Comprobado quitando
        // el rearme: sin esta prueba, la suite entera se queda verde.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 1).expect("no se pudo abrir la ventana");

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::T));
        assert_eq!(ventana.pestañas.len(), 2, "no se abrió la pestaña");

        // El rearme levanta todas las banderas; se recogen para que lo que se mire después sea solo lo
        // que provoca la publicación.
        for division in ventana.todas_las_divisiones() {
            division.por_releer.recoger();
        }

        publicar_en(
            &ventana.pestañas[1].divisiones[0],
            r#"{"lines":["ya estoy"]}"#,
        );

        esperar_la_bandera(
            &ventana.pestañas[1].divisiones[0].por_releer,
            "la ventana no vigila el directorio de la pestaña que acaba de crear",
        );
    }

    #[test]
    fn tras_rearmar_el_vigilante_quedan_todas_las_banderas_levantadas() {
        // AC9, la otra mitad. Entre soltar el vigilante viejo y armar el nuevo **no llega ningún aviso**:
        // lo que un productor publique en ese hueco no lo ve nadie. Sin levantar las banderas después de
        // armar, su panel se quedaría con lo de antes hasta la siguiente publicación — que en un agente
        // que acaba de terminar puede no llegar nunca.
        let ctx = egui::Context::default();
        let mut ventana = Ventana::con_pestañas(&ctx, 2).expect("no se pudo abrir la ventana");
        for division in ventana.todas_las_divisiones() {
            division.por_releer.recoger();
        }

        ventana.rearmar_el_vigilante(&ctx);

        for (cual, division) in ventana.todas_las_divisiones().enumerate() {
            assert!(
                division.por_releer.recoger(),
                "la división {cual} quedó sin bandera tras rearmar, así que su panel puede haberse \
                 publicado en el hueco y nadie lo vería"
            );
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
    fn la_consola_de_una_pestaña_que_no_se_ve_se_sigue_vaciando() {
        // **Esto es lo que el hito promete**, y lo que separa este terminal de abrir dos ventanas: un
        // agente trabajando en otra pestaña no se para porque no lo estés mirando.
        //
        // Y no es una cuestión de que su salida tarde en verse: si la ventana dejara de vaciar su
        // pseudoterminal, el freno del terminal entraría en acción —el núcleo da unos 20 KiB de margen
        // antes de bloquear al que escribe— y **el programa se pararía**. Lo observable desde aquí es
        // que lo que escribió acaba en su rejilla sin haber pintado esa pestaña ni una sola vez.
        let mut ventana = ventana_de_pestañas([
            ["sleep 30", "sleep 30"],
            ["printf trabajando-de-fondo", "sleep 30"],
        ]);
        let de_fondo = &ventana.pestañas[1].divisiones[0];
        esperar_a_que_muera(de_fondo);

        ventana.bombear_todas();

        assert_eq!(
            ventana.activa, 0,
            "la prueba no está mirando la otra pestaña"
        );
        assert!(
            ventana.pestañas[1].divisiones[0]
                .pantalla
                .to_text()
                .contains("trabajando-de-fondo"),
            "la consola de la pestaña de fondo no se vació: su programa se habría bloqueado al escribir"
        );
    }

    #[test]
    fn una_consola_viva_en_una_pestaña_de_fondo_no_deja_cerrar_la_ventana() {
        // El mismo criterio que ya vale para las dos consolas de una pestaña, extendido a todas: cerrar
        // con algo vivo detrás se llevaría por delante un trabajo que nadie ha visto terminar. Sin esto,
        // recorrer solo la pestaña visible cerraría la ventana en cuanto sus dos consolas murieran,
        // matando al programa de la otra pestaña sin decir nada.
        let mut ventana =
            ventana_de_pestañas([["printf ya", "printf esta"], ["sleep 30", "sleep 30"]]);
        for division in &ventana.pestañas[0].divisiones {
            esperar_a_que_muera(division);
        }

        let bombeo = ventana.bombear_todas();

        assert!(
            !bombeo.todas_terminadas,
            "la ventana se cierra con una consola viva en una pestaña que no se ve"
        );
    }

    #[test]
    fn la_ventana_se_cierra_cuando_han_muerto_las_de_todas_las_pestañas() {
        // **La mitad positiva del mismo criterio, y sin ella la de arriba se cumple sola.** «Con más de
        // una pestaña, no cerrar nunca» pasa la prueba de al lado y deja una ventana que no se puede
        // cerrar desde dentro: las cuatro consolas muertas y el terminal ahí plantado, esperando a que
        // alguien lo cierre con el ratón. Comprobado mutándolo —dar por terminada la ventana solo si hay
        // una pestaña— y la suite entera seguía verde.
        //
        // La otra prueba positiva que ya existe usa **una** pestaña, donde recorrer todas y recorrer la
        // visible son lo mismo, así que tampoco la ve.
        let mut ventana = ventana_de_pestañas([
            ["printf una", "printf otra"],
            ["printf tres", "printf cuatro"],
        ]);
        for division in ventana.todas_las_divisiones() {
            esperar_a_que_muera(division);
        }

        let bombeo = ventana.bombear_todas();

        assert!(
            bombeo.todas_terminadas,
            "murieron las cuatro consolas y la ventana sigue sin cerrarse"
        );
        // Y lo que escribió la pestaña que nunca se vio está en su rejilla: «terminada» incluye que no
        // deje cola, así que darla por terminada antes de vaciarla sería cerrar comiéndose su final —
        // que en una pestaña de fondo es justamente lo único que uno iba a leer de ella.
        assert!(
            ventana.pestañas[1].divisiones[1]
                .pantalla
                .to_text()
                .contains("cuatro"),
            "se dio por terminada una consola de fondo cuya salida aún no había llegado a la rejilla"
        );
    }

    #[test]
    fn el_despertador_no_madruga_por_un_panel_que_no_se_ve() {
        // La cara opuesta del bombeo, y por eso va aquí al lado: el trabajo corre en todas las
        // pestañas, pero lo que se ve ocurre en una sola. El razonamiento entero está en el doc de
        // [`Ventana::poner_el_despertador`].
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        for division in &mut ventana.pestañas[1].divisiones {
            division.panel = Some(PanelDir::create().expect("crear el directorio"));
        }
        publicar_en(
            &ventana.pestañas[1].divisiones[0],
            r#"{"stale_after_minutes":10,"lines":["algo"]}"#,
        );
        ventana.pestañas[1].divisiones[0].por_releer.levantar();
        ventana.activa = 1;
        // Con la pestaña delante, su panel se lee y la alarma queda armada: es el estado de partida.
        lo_que_la_ventana_espera(&mut ventana, &ctx);
        ventana.activa = 0;

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
            !ctx.has_requested_repaint(),
            "la ventana se despierta por el panel de una pestaña que no se ve, causas: {:?}",
            ctx.repaint_causes()
        );
    }

    #[test]
    fn al_volver_a_la_pestaña_el_aviso_de_rancio_sale_en_ese_mismo_fotograma() {
        // **Esta es la prueba que salva a la de arriba**, y por eso van juntas. Acotar el despertador a
        // la pestaña visible solo sale gratis si la pregunta de si un panel está viejo se contesta **al
        // pintarlo**, con el reloj de ese fotograma. Si dependiera de que alguien hubiera despertado a
        // la ventana a la hora del vencimiento, el panel de una pestaña de fondo volvería con la
        // etiqueta sin poner y nadie se la pondría hasta que su fichero cambiara — que en un productor
        // muerto, que es justo el caso que el aviso existe para delatar, no pasa nunca. Sin esto, «no se
        // pierde nada» es una afirmación de palabra.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let de_fondo = &mut ventana.pestañas[1].divisiones[0];
        de_fondo.panel = Some(PanelDir::create().expect("crear el directorio"));
        // Plazo cero, que el contrato permite: el panel está viejo desde el primer instante, así que la
        // prueba llega al caso sin esperar diez minutos ni tocar ningún reloj.
        publicar_en(de_fondo, r#"{"stale_after_minutes":0,"lines":["algo"]}"#);
        de_fondo.por_releer.levantar();

        // Fotogramas con la otra pestaña delante. La comprobación de cada uno no es un adorno: si la de
        // fondo se estuviera pintando, la aserción de abajo pasaría sin haber vuelto de ninguna parte.
        for _ in 0..3 {
            let formas = pasada_de_interfaz(
                &mut ventana,
                &ctx,
                area_de(1200.0),
                egui::RawInput::default(),
            );
            assert!(
                !se_encolo_el_texto(&formas, "sin noticias"),
                "el panel de la pestaña de fondo se está pintando sin estar delante"
            );
        }

        ventana.activa = 1;
        let al_volver = pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );

        assert!(
            se_encolo_el_texto(&al_volver, "sin noticias"),
            "se vuelve a la pestaña y su panel no dice que lleva callado: el aviso que el despertador \
             no fue a buscar tampoco aparece al mirarlo"
        );
    }

    #[test]
    fn lo_que_interpreta_una_consola_que_no_se_pinta_se_declara_igual() {
        // **El camino real, no el ayudante.** Que `Medida` sepa cerrar una vuelta sin pintado ya lo ata
        // una prueba unitaria; lo que aquí se comprueba es que **alguien la llama** para las consolas
        // que se quedan fuera del pintado, y que la ventana recoge lo declarado. Por qué importa, en
        // el doc de `Ventana::cerrar_las_vueltas_no_pintadas`.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["printf de-fondo", "sleep 30"]]);
        esperar_a_que_muera(&ventana.pestañas[1].divisiones[0]);

        // El bombeo deja pendiente lo que interpretó cada consola; el pintado solo recoge el de la
        // pestaña visible.
        ventana.bombear_todas();
        // Que hubo algo que interpretar de verdad. Preguntar si quedó medida pendiente **no** vale
        // como guardián: el bombeo la deja puesta siempre, haya llegado un byte o no.
        assert!(
            ventana.pestañas[1].divisiones[0]
                .pantalla
                .to_text()
                .contains("de-fondo"),
            "la consola de fondo no llegó a interpretar nada: la prueba no está midiendo lo que cree"
        );

        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );

        assert!(
            ventana.pestañas[1].divisiones[0]
                .pendiente_de_medir
                .is_none(),
            "la vuelta de la consola de fondo se quedó sin cerrar"
        );
        // **Lo primero que hay que afirmar es que declaró algo, no que ya no quede nada pendiente.**
        // Es justo el fallo que hay que impedir: un barrido que se limitara a **tirar** lo pendiente
        // deja la medida vacía —sin nada pendiente, sin medias sucias y sin nada por recoger, o sea
        // verde— mientras el tiempo de esa consola se sigue publicando como coste del framework.
        // Comprobado mutándolo: sin esta afirmación, ese barrido pasaba.
        let declarado = ventana.pestañas[1].divisiones[0]
            .medida
            .cerrar_fotograma(std::time::Duration::from_millis(10))
            .expect("la consola de fondo no declaró nada de lo que interpretó");
        assert!(
            !declarado.is_zero(),
            "declaró cero: lo que interpretó se perdió por el camino"
        );
        assert!(
            !ventana.pestañas[1].divisiones[0].medida.hay_medias(),
            "la consola de fondo ensució alguna media: no hubo vuelta que medir, solo algo que restar"
        );

        // Y la otra mitad, en un segundo fotograma porque el primero ya se consumió arriba: **la
        // ventana tiene que recogerlo**. Declarar sin que nadie reste deja el número igual de mal, y
        // con una sola pestaña no se nota.
        ventana.bombear_todas();
        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );
        ventana.cerrar_fotograma(std::time::Duration::from_millis(10));
        assert!(
            ventana.pestañas[1].divisiones[0]
                .medida
                .cerrar_fotograma(std::time::Duration::from_millis(10))
                .is_none(),
            "lo que declaró la consola de fondo seguía ahí sin recoger"
        );
    }

    #[test]
    fn la_consola_que_no_se_pinta_declara_lo_interpretado_y_ni_un_tramo_mas() {
        // Que declare **algo** ya lo ata la prueba de arriba; lo que falta es que ese algo sea
        // **exactamente lo que interpretó**. Poner cualquier otra cosa donde van los ceros —el
        // encolado, el tramo del panel— sale verde y no se ve por ninguna parte: lo declarado se
        // resta del coste del fotograma, así que un tramo inventado en una consola que no se pintó
        // le baja el resto al framework de interfaz, que es justo la cifra que decide D3.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["printf de-fondo", "sleep 30"]]);
        esperar_a_que_muera(&ventana.pestañas[1].divisiones[0]);

        ventana.bombear_todas();
        let (_, _, interpretado) = ventana.pestañas[1].divisiones[0]
            .pendiente_de_medir
            .expect("el bombeo tiene que dejar pendiente lo que interpretó la consola de fondo");
        // Con un cero aquí la comparación de abajo la cumpliría también un barrido que declarase
        // ceros por todas partes, que es lo contrario de lo que esta prueba dice comprobar.
        assert!(
            !interpretado.is_zero(),
            "interpretar la salida no costó nada medible: la prueba no está midiendo lo que cree"
        );

        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );

        // Se cierra sobre la medida de esa consola y no sobre la ventana **a propósito**: por la
        // ventana lo declarado sale ya sumado con lo de las otras tres y deja de poderse afirmar
        // sobre este número. Que la ventana lo recoge lo ata la prueba de arriba.
        assert_eq!(
            ventana.pestañas[1].divisiones[0]
                .medida
                .cerrar_fotograma(std::time::Duration::from_millis(10)),
            Some(interpretado),
            "la consola de fondo declaró un tramo que no gastó: ese tiempo se le descuenta al \
             framework sin que nadie lo haya medido"
        );
    }

    #[test]
    fn el_barrido_de_lo_no_pintado_va_despues_del_pintado_y_no_antes() {
        // **El orden dentro de la pasada de interfaz, que no es indiferente.** Barrer lo pendiente
        // antes de pintar —recién bombeado, que es donde parece que va— se lleva por delante lo de
        // la consola que **sí** se ve: llega al pintado sin nada que cerrar, así que su vuelta se
        // declara sin espera y **ninguna de las cuatro medias vuelve a recibir una muestra en toda
        // la sesión**, mientras su encolado y su panel se publican como coste del framework. Ni da
        // error ni se nota en pantalla: el informe sale con las series en blanco.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["printf delante", "sleep 30"], ["sleep 30", "sleep 30"]]);
        esperar_a_que_muera(&ventana.pestañas[0].divisiones[0]);

        ventana.bombear_todas();
        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );
        // Las medias se llenan al cerrar el fotograma, no al pintar: preguntar antes daría un vacío
        // que no dice nada.
        ventana.cerrar_fotograma(std::time::Duration::from_millis(10));

        assert!(
            ventana.pestañas[0].divisiones[0].medida.hay_medias(),
            "la consola que se ve se quedó sin una sola muestra: su vuelta se cerró antes de \
             pintarla, así que se declaró como si no le hubiera llegado nada"
        );
    }

    /// El informe de la sesión, tal como lo escribe la aplicación al cerrarse.
    fn informe_de(ventana: &Ventana) -> String {
        let mut salida = Vec::new();
        ventana
            .publicar_informe(&mut salida)
            .expect("escribir a memoria no falla");
        String::from_utf8(salida).expect("el informe es texto")
    }

    #[test]
    fn el_informe_dice_cuantas_pestañas_habia_y_cual_se_veia() {
        // **Sin esto, dos tandas de medida distintas producen informes indistinguibles.** Los tramos
        // de abajo se toman con todas las pestañas bombeando, así que una tanda de dos pestañas y otra
        // de una no se pueden comparar; y quien transcriba la de dos creyendo que es de una publicaría
        // que el fotograma se encareció sin causa. Es el mismo fallo que el aviso de la rejilla ya
        // existe para evitar, con una variable más.
        let mut ventana = ventana_de_pestañas([
            ["printf visible", "sleep 30"],
            ["printf de-fondo", "sleep 30"],
        ]);
        // Solo las que imprimen y terminan; las otras dos siguen vivas a propósito, porque un informe
        // de una sesión real se toma con consolas todavía trabajando.
        esperar_a_que_muera(&ventana.pestañas[0].divisiones[0]);
        esperar_a_que_muera(&ventana.pestañas[1].divisiones[0]);
        ventana.bombear_todas();
        pasada_de_interfaz(
            &mut ventana,
            &egui::Context::default(),
            area_de(1200.0),
            egui::RawInput::default(),
        );
        ventana.cerrar_fotograma(std::time::Duration::from_millis(10));

        let informe = informe_de(&ventana);

        assert!(
            informe.contains("pestañas:     2"),
            "el informe no dice cuántas pestañas había:\n{informe}"
        );
        assert!(
            informe.contains("pestaña 0:    se veía"),
            "el informe no dice cuál se veía:\n{informe}"
        );
        assert!(
            informe.contains("de fondo, se bombeaba sin pintarse"),
            "el informe no distingue la pestaña de fondo:\n{informe}"
        );
    }

    #[test]
    fn el_informe_publica_los_tramos_de_las_consolas_de_todas_las_pestañas() {
        // **Lo que este hito hace medible se perdería justo al publicarlo.** Una consola de fondo
        // cronometra su interpretar igual que las visibles —para eso se arregló el descuento—, así que
        // un informe que solo publique las de la pestaña que se veía tira los números que la tanda de
        // dos pestañas existe para tomar, y lo hace **en silencio**: el informe sale entero y con buena
        // pinta, solo que con la mitad de las consolas.
        let mut ventana = ventana_de_pestañas([
            ["printf visible", "sleep 30"],
            ["printf de-fondo", "sleep 30"],
        ]);
        // Solo las que imprimen y terminan; las otras dos siguen vivas a propósito, porque un informe
        // de una sesión real se toma con consolas todavía trabajando.
        esperar_a_que_muera(&ventana.pestañas[0].divisiones[0]);
        esperar_a_que_muera(&ventana.pestañas[1].divisiones[0]);
        ventana.bombear_todas();
        pasada_de_interfaz(
            &mut ventana,
            &egui::Context::default(),
            area_de(1200.0),
            egui::RawInput::default(),
        );
        ventana.cerrar_fotograma(std::time::Duration::from_millis(10));

        let informe = informe_de(&ventana);

        // **Las divisiones se numeran seguidas entre pestañas**, así que las de la segunda son la 2 y la
        // 3, no otra vez la 0 y la 1. Escrito así, la prueba distingue un contador de toda la ventana de
        // uno por pestaña: con el segundo, el informe publicaría dos «división 0» y esto caería.
        for (que_pestaña, divisiones) in [(0, [0, 1]), (1, [2, 3])] {
            for division in divisiones {
                assert!(
                    informe.contains(&format!("── pestaña {que_pestaña}, división {division} ")),
                    "el informe no publica la división {division} de la pestaña {que_pestaña}:\n{informe}"
                );
            }
        }

        // **Y la cabecera también, que es otra línea y se escribe en otro sitio.** Es la que declara la
        // rejilla de cada consola y la que el guion de medida lee para validar una corrida, así que
        // nombrar ahí por posición publicaría dos «división 0» con rejillas distintas. Comprobado
        // mutándolo: sin estas líneas, la cabecera podía volver a numerar por posición con la suite en
        // verde, porque las de arriba solo miran los títulos de las tablas.
        for division in [0, 1, 2, 3] {
            assert!(
                informe.contains(&format!("  división {division}:")),
                "la cabecera no declara la rejilla de la división {division}:\n{informe}"
            );
        }
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

    /// Corre el cuerpo con la métrica que la ventana usa de verdad, para que lo que estas pruebas llamen
    /// «el alto de la barra» sea el mismo número que se pinta.
    ///
    /// Va dentro de un pase porque las fuentes no existen fuera de uno: medir contra un contexto recién
    /// hecho entra en pánico en el propio egui.
    fn con_metrica<T>(hacer: impl FnOnce(&Metrica) -> T) -> T {
        let ctx = egui::Context::default();
        let mut resultado = None;
        // **El cuerpo se saca al primer pase y no vuelve**, porque `run_ui` recibe un `FnMut`: egui puede
        // repetir la pasada sobre la misma entrada, y un cuerpo que se ejecutara dos veces afirmaría dos
        // veces sobre estado ya tocado. Es la misma trampa que obligó a consumir los eventos del teclado.
        let mut pendiente = Some(hacer);

        ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(hacer) = pendiente.take() {
                resultado = Some(hacer(&Metrica::medir(
                    ui.ctx(),
                    egui::FontId::monospace(CUERPO),
                )));
            }
        })
        .drop_without_applying_deltas();

        resultado.expect("el pase no llegó a correr el cuerpo")
    }

    /// Una tecla con Ctrl y Mayúsculas, tal y como el acorde llega del escritorio.
    fn acorde_con(key: egui::Key) -> Vec<egui::Event> {
        tecla_con(key, con_mayusculas())
    }

    /// El mismo acorde pulsado dos veces, tal y como llegan dos gestos seguidos.
    ///
    /// **La suelta de en medio es lo que los hace dos.** egui marca como repetición cualquier pulsación
    /// de una tecla que ya estaba abajo —lo hace él, mirando las que tiene pulsadas, sin fiarse de lo que
    /// traiga el evento—, y las repeticiones de crear y cerrar se tragan a propósito. Sin la suelta, esto
    /// sería la tecla mantenida, que es el otro caso y ya tiene su prueba.
    fn dos_veces_el_acorde(key: egui::Key) -> Vec<egui::Event> {
        let mut lote = acorde_con(key);
        lote.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: con_mayusculas(),
        });
        lote.extend(acorde_con(key));
        lote
    }

    /// Una tecla pulsada con los modificadores que se digan, y el aviso de cambio que la precede.
    ///
    /// El aviso va delante porque el acorde **se reconstruye evento a evento**: sin él, la flecha llega
    /// con las teclas de la pasada anterior y la prueba mediría otra cosa.
    fn tecla_con(key: egui::Key, modificadores: egui::Modifiers) -> Vec<egui::Event> {
        vec![
            egui::Event::ModifiersChanged(modificadores),
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: modificadores,
            },
        ]
    }

    #[test]
    fn con_una_sola_pestaña_no_hay_barra_y_las_consolas_conservan_sus_celdas() {
        // AC7, y es el criterio del que cuelga el anexo de D3: la tanda de control mide una ventana que
        // tiene que ser **la misma** que antes de las pestañas. Si la barra apareciera con una sola, las
        // consolas perderían filas, el fotograma saldría más barato por pintar menos celdas y el anexo
        // publicaría como mejora lo que solo es menos trabajo.
        let area = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));

        con_metrica(|metrica| {
            let (barra, para_las_consolas) = apartar_la_barra(area, 1, metrica);

            assert!(barra.is_none(), "apareció una barra con una sola pestaña");
            assert_eq!(
                metrica.celdas_en(para_las_consolas.size()),
                metrica.celdas_en(area.size()),
                "con una sola pestaña las consolas no tienen las mismas celdas que sin barra"
            );
        });
    }

    #[test]
    fn con_dos_pestañas_la_barra_le_quita_alto_a_las_consolas() {
        // El otro lado del criterio anterior. Sin esto, `apartar_la_barra` podría devolver siempre el área
        // entera —y la prueba de arriba seguiría verde— con la barra pintada encima de las consolas.
        let area = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));

        con_metrica(|metrica| {
            let (barra, para_las_consolas) = apartar_la_barra(area, 2, metrica);

            let barra = barra.expect("con dos pestañas tiene que haber barra");
            assert!(
                barra.height() >= metrica.alto(),
                "la barra no deja sitio ni para un número"
            );
            assert_eq!(
                barra.bottom(),
                para_las_consolas.top(),
                "entre la barra y las consolas quedó un hueco, o se solapan"
            );
            assert!(
                para_las_consolas.height() < area.height(),
                "la barra no le quitó alto a las consolas, así que se pinta encima"
            );
        });
    }

    #[test]
    fn una_ventana_mas_baja_que_la_barra_no_deja_ningun_rectangulo_invertido() {
        // `split_top_bottom_at_y` **no acota** —leído en `emath-0.35.0/src/rect.rs:671`, que construye
        // los dos rectángulos con el corte tal cual—, así que un corte por debajo del área devuelve el de
        // abajo con su borde superior por debajo del inferior. Es el mismo caso borde que el ancho ya
        // tiene probado.
        let mas_baja_que_la_barra =
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 1.0));

        con_metrica(|metrica| {
            let (barra, para_las_consolas) = apartar_la_barra(mas_baja_que_la_barra, 2, metrica);

            let barra = barra.expect("con dos pestañas tiene que haber barra");
            assert!(
                barra.top() <= barra.bottom(),
                "la barra salió invertida: {barra:?}"
            );
            assert!(
                para_las_consolas.top() <= para_las_consolas.bottom(),
                "el rectángulo de las consolas salió invertido: {para_las_consolas:?}"
            );
        });
    }

    #[test]
    fn las_fichas_se_reparten_la_barra_sin_solaparse_ni_dejar_hueco() {
        let barra = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 20.0));

        let primera = ficha(barra, 0, 3);
        let ultima = ficha(barra, 2, 3);

        assert_eq!(
            primera.left(),
            barra.left(),
            "la primera no empieza al borde"
        );
        assert_eq!(ultima.right(), barra.right(), "la última no llega al borde");
        assert_eq!(
            ficha(barra, 1, 3).left(),
            primera.right(),
            "entre dos fichas quedó hueco, o se solapan"
        );
    }

    #[test]
    fn el_acorde_cambia_de_pestaña_y_no_le_llega_al_programa() {
        // AC8. Sin consumir el evento el programa recibiría `\x1b[C`: con Ctrl y Mayúsculas puestas, la
        // flecha cae en la tabla de siempre de `teclado::tecla` —Mayúsculas no cambia el código de
        // control, y Ctrl con algo que no es letra se cae a esa tabla—, así que el cursor se movería en el
        // editor de la pestaña que se acaba de dejar.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        encolar(&ctx, acorde_con(egui::Key::ArrowRight));
        let ordenes = ventana.teclear_la_del_foco(&ctx);

        assert_eq!(ordenes, vec![Orden::Ir(1)], "el acorde no pidió cambiar");
        assert!(
            ventana.activa().divisiones[0].pulsaciones.is_empty(),
            "el acorde se le coló al programa"
        );

        ventana.aplicar(Orden::Ir(1), &ctx);
        assert_eq!(ventana.activa, 1, "no cambió la pestaña activa");
    }

    #[test]
    fn dos_flechas_en_el_mismo_lote_avanzan_dos_pestañas() {
        // **El caso es el del hito, no uno rebuscado.** Con la ventana dormida —dos agentes callados—
        // egui entrega en un solo lote todo lo que llegó mientras tanto. Con una orden por lote, de dos
        // pulsaciones se atendería una y la otra se perdería: el evento ya se consumió, así que no queda
        // en la cola para el fotograma siguiente.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
        ]);

        let mut lote = acorde_con(egui::Key::ArrowRight);
        lote.extend(acorde_con(egui::Key::ArrowRight));
        encolar(&ctx, lote);

        assert_eq!(
            ventana.teclear_la_del_foco(&ctx),
            vec![Orden::Ir(1), Orden::Ir(1)],
            "el segundo acorde del lote se perdió"
        );
    }

    #[test]
    fn ir_y_volver_en_el_mismo_lote_deja_la_pestaña_donde_estaba() {
        // La otra cara de componer: sustituir dejaría que una izquierda tras una derecha valiera −1 en vez
        // de quedarse quieta.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        let mut lote = acorde_con(egui::Key::ArrowRight);
        lote.extend(acorde_con(egui::Key::ArrowLeft));
        encolar(&ctx, lote);

        assert_eq!(
            ventana.teclear_la_del_foco(&ctx),
            vec![Orden::Ir(1), Orden::Ir(-1)],
            "ir y volver movió de pestaña"
        );
    }

    #[test]
    fn con_una_sola_pestaña_la_flecha_le_llega_al_programa() {
        // Consumir la flecha con una sola pestaña la quitaría de en medio para no hacer nada —ir de la
        // única pestaña a ella misma— y el programa dejaría de recibirla. Antes de que existieran los
        // atajos, `Ctrl+Mayúsculas+←` le llegaba como `\x1b[D`, y con una pestaña le sigue llegando.
        let ctx = egui::Context::default();
        let mut ventana = ventana_con("sleep 30", "sleep 30");

        encolar(&ctx, acorde_con(egui::Key::ArrowLeft));

        assert_eq!(
            ventana.teclear_la_del_foco(&ctx),
            Vec::new(),
            "pidió cambiar de pestaña habiendo una sola"
        );
        assert_eq!(
            ventana.activa().divisiones[0].pulsaciones,
            b"\x1b[D",
            "la flecha se la tragó el acorde en vez de llegar al programa"
        );
    }

    #[test]
    fn cambiar_de_pestaña_pide_fotograma() {
        // **Sin esto la ventana se duerme enseñando la pestaña anterior.** Y hay un segundo efecto, que es
        // el que lo convierte en algo más que un parpadeo: la rejilla de una consola se ajusta en la mitad
        // lógica del fotograma **siguiente**, así que sin fotograma siguiente los dos pseudoterminales de
        // la pestaña recién mostrada se quedan con el tamaño de arranque —nunca se pintó, así que nunca se
        // midió— hasta que llegue entrada de otra cosa.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        // **Hay que llevar el contexto a reposo antes de mirar, y sin esto la prueba no ata nada.** Un
        // contexto recién hecho pide repintado por su cuenta durante los primeros pases, así que la
        // afirmación de abajo se cumplía igual con la línea que la provoca **borrada** — comprobado
        // mutándola. Es el mismo arranque que usan las pruebas del despertador.
        for _ in 0..4 {
            let _ = ctx.run_logic(&egui::RawInput::default(), |_| {});
        }
        assert!(
            !ctx.has_requested_repaint(),
            "el contexto no llega a estar en reposo, causas: {:?}",
            ctx.repaint_causes()
        );

        ventana.aplicar(Orden::Ir(1), &ctx);

        assert!(
            ctx.has_requested_repaint(),
            "cambiar de pestaña no pidió fotograma"
        );
    }

    /// Corre el fotograma entero —`logic` y `ui` en la misma pasada, como de verdad— con los eventos
    /// dados, y devuelve si la ventana pidió cerrarse en él.
    ///
    /// Es la variante con entrada de [`lo_que_la_ventana_espera`], y hace falta porque lo que estas
    /// pruebas comprueban **es el orden dentro de `ui`**: qué se calcula antes de qué. Llamando a las
    /// piezas por separado en el orden que uno cree que tienen, la prueba afirmaría sobre su propia
    /// composición y no sobre la del producto — que es el hueco por el que se coló el defecto.
    ///
    /// **La invariante de los índices se comprueba aquí y no en una prueba suya**, porque no es de un
    /// escenario: vale para *todo* fotograma, también para los que corren **después** de pedir el cierre.
    /// Puesta en el ayudante, cualquier prueba que corra fotogramas la comprueba de paso.
    ///
    /// **No es lo que caza que se rompa, y conviene no creerlo:** una activa fuera de la lista revienta
    /// el propio fotograma al indexar, antes de llegar aquí. Lo que esto añade es decir **cuál** era el
    /// índice y cuántas pestañas había, en vez de un pánico a secas desde dentro del pintado.
    fn un_fotograma_con(
        ventana: &mut Ventana,
        ctx: &egui::Context,
        eventos: Vec<egui::Event>,
    ) -> bool {
        let mut frame = eframe::Frame::_new_kittest();
        let entrada = egui::RawInput {
            screen_rect: Some(area_de(800.0)),
            events: eventos,
            ..Default::default()
        };

        eframe::App::logic(ventana, ctx, &mut frame);
        let salida = ctx.run_ui(entrada, |ui| eframe::App::ui(ventana, ui, &mut frame));
        let pidio_cerrar = salida.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Close);
        salida.drop_without_applying_deltas();

        assert!(
            ventana.activa < ventana.pestañas.len(),
            "la pestaña activa ({}) quedó fuera de una lista de {}",
            ventana.activa,
            ventana.pestañas.len()
        );

        pidio_cerrar
    }

    #[test]
    fn egui_cuenta_los_fotogramas_una_vez_y_las_pasadas_todas() {
        // **La premisa de la que depende la guarda del doble cierre, comprobada en vez de supuesta.**
        // El coste del fotograma se anota una sola vez, y para saber si ya se anotó se compara un
        // contador de egui contra el último visto. Con el contador equivocado —el de pasadas— la guarda
        // no filtra nada: en un fotograma repetido da dos valores distintos, el segundo cierre entra, y
        // empareja los tramos de este fotograma con el total del anterior. Eso estuvo así, y esta prueba
        // es lo que impide que vuelva sin que nadie se entere.
        //
        // Es también la clase de prueba que este proyecto ha aprendido a escribir a base de sustos: la
        // API ajena se lee, no se supone, y aquí además se **ata**, para que una versión nueva de egui
        // que cambie el criterio se note aquí y no en un anexo con números falsos.
        let ctx = egui::Context::default();
        let mut fotogramas = Vec::new();
        let mut pasadas = Vec::new();

        let salida = ctx.run_ui(egui::RawInput::default(), |ui| {
            fotogramas.push(ui.ctx().cumulative_frame_nr());
            pasadas.push(ui.ctx().cumulative_pass_nr());
            // Solo en la primera: pedir descarte en todas dejaría a egui repitiendo hasta su tope.
            if pasadas.len() == 1 {
                ui.ctx()
                    .request_discard("la prueba necesita un fotograma de dos pasadas");
            }
        });
        salida.drop_without_applying_deltas();

        assert_eq!(
            pasadas.len(),
            2,
            "egui no repitió la pasada, así que la prueba no está mirando lo que cree"
        );
        assert_eq!(
            fotogramas[0], fotogramas[1],
            "el contador de fotogramas cambió entre dos pasadas del mismo fotograma: ya no sirve de \
             guarda para cerrarlo una sola vez"
        );
        assert_ne!(
            pasadas[0], pasadas[1],
            "el contador de pasadas no cambió entre dos pasadas: entonces el defecto que la guarda \
             arregla no se puede reproducir y hay que revisar por qué"
        );
    }

    /// Corre un fotograma **como lo corre `eframe`**: las dos mitades de la aplicación dentro del mismo
    /// cierre que egui puede repetir, y no `logic` por fuera y `ui` por dentro.
    ///
    /// **La diferencia no es de estilo, y ya dejó pasar un defecto.** `eframe` mete las dos llamadas
    /// dentro del cierre de `run_ui` (`epi_integration.rs:288-303`), así que una pasada repetida vuelve a
    /// bombear **y** vuelve a pintar. Un ayudante que llame a `logic` una sola vez modela un fotograma
    /// que no existe, y una prueba escrita sobre él certifica una garantía que el sistema real no da.
    ///
    /// Con `repitiendo` en `true` pide descarte en la primera pasada, que es la forma de provocar el
    /// fotograma de dos pasadas. Devuelve cuántas hubo, para que quien la use pueda afirmar que de verdad
    /// se repitió en vez de dar por hecho que egui obedeció.
    fn un_fotograma_como_eframe(
        ventana: &mut Ventana,
        ctx: &egui::Context,
        repitiendo: bool,
    ) -> usize {
        let mut frame = eframe::Frame::_new_kittest();
        let entrada = egui::RawInput {
            screen_rect: Some(area_de(1200.0)),
            ..Default::default()
        };
        let mut pasadas = 0;

        let salida = ctx.run_ui(entrada, |ui| {
            pasadas += 1;
            eframe::App::logic(ventana, ui.ctx(), &mut frame);
            eframe::App::ui(ventana, ui, &mut frame);
            if repitiendo && pasadas == 1 {
                ui.ctx()
                    .request_discard("la prueba necesita repetir la pasada");
            }
        });
        salida.drop_without_applying_deltas();

        pasadas
    }

    #[test]
    fn cerrar_una_pestaña_no_renombra_a_las_que_quedan_en_el_informe() {
        // **El informe se publica al salir, con los números de toda la sesión, y las etiquetas se
        // escribían con la posición en la lista.** Al cerrar una pestaña las de su derecha corren un
        // puesto, así que una tabla de tramos acababa titulada con el nombre de otra pestaña. No hay
        // forma de notarlo leyendo el informe: los números son plausibles y la etiqueta es firme.
        //
        // Se cierra la **primera** a propósito: es el único caso en que las demás se mueven.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["printf de-la-primera", "sleep 30"],
            ["printf de-la-segunda", "sleep 30"],
        ]);
        esperar_a_que_muera(&ventana.pestañas[1].divisiones[0]);

        // **La segunda tiene que estar delante mientras se mide**, o su bloque no se publica: una
        // pestaña de fondo declara su vuelta sin espera —para no ensuciar las medias— y sin espera no se
        // anota ninguna serie, así que el informe la daría por muda.
        ventana.activa = 1;
        ventana.bombear_todas();
        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );
        ventana.cerrar_fotograma(std::time::Duration::from_millis(10));

        ventana.activa = 0;
        ventana.aplicar(Orden::Cerrar, &ctx);
        assert_eq!(ventana.pestañas.len(), 1, "no se cerró la primera pestaña");

        let informe = informe_de(&ventana);
        assert!(
            informe.contains("── pestaña 1, división 2"),
            "la pestaña que quedó no conserva su nombre, así que sus números se publican con la \
             etiqueta de otra:\n{informe}"
        );
        assert!(
            !informe.contains("── pestaña 0, división 0"),
            "la pestaña que quedó se publicó como la 0, que es la que se cerró:\n{informe}"
        );
    }

    #[test]
    fn dos_pestañas_abiertas_seguidas_no_se_llaman_igual() {
        // El nombre sale de un contador de la ventana, y **si no avanza, dos pestañas distintas se
        // llaman igual**: el informe publicaría dos bloques de números bajo la misma etiqueta, que es
        // otra forma del mismo engaño que arregla la prueba de arriba. No se afirma sobre valores
        // concretos sino sobre que no haya repetidos: lo que importa es que cada una sea distinguible,
        // no en qué número empieza la cuenta.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([["sleep 30", "sleep 30"]]);

        // Con la suelta en medio, que es lo que las hace dos pulsaciones y no una tecla mantenida — las
        // repeticiones de crear se tragan a propósito.
        un_fotograma_con(&mut ventana, &ctx, dos_veces_el_acorde(egui::Key::T));
        assert_eq!(ventana.pestañas.len(), 3, "no se abrieron las dos pestañas");

        let nombres: Vec<usize> = ventana.pestañas.iter().map(|p| p.nombre).collect();
        let mut sin_repetir = nombres.clone();
        sin_repetir.sort_unstable();
        sin_repetir.dedup();
        assert_eq!(
            sin_repetir.len(),
            nombres.len(),
            "hay pestañas que se llaman igual: {nombres:?}"
        );
    }

    #[test]
    fn el_nombre_de_una_division_cerrada_no_se_le_da_a_otra() {
        // **El caso que este nombre existe para cubrir.** Cerrar una pestaña se lleva sus dos divisiones,
        // y si el contador se dedujera de las que quedan vivas —en vez de no retroceder nunca— la pestaña
        // que se abriera después reutilizaría sus números. El informe publicaría entonces dos tablas de
        // números bajo la misma etiqueta, y **quien lo lea no tiene forma de notarlo**: son dos consolas
        // distintas con el mismo nombre, no un hueco que se vea.
        //
        // Se comprueba sobre los nombres vivos y sobre los ya gastados, porque el defecto no deja rastro
        // en el número de divisiones ni en el de pestañas: los dos salen iguales con el contador bueno y
        // con el malo.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        // **El contador de partida no se toca aquí a propósito**: lo calcula quien monta la ventana, y
        // dejar que lo haga es lo que pone ese cálculo dentro de lo que la prueba cubre. Fijándolo a mano
        // —comprobado mutándolo— arrancarlo en cero pasaba desapercibido, y arrancar en cero significa
        // repartirle a la primera pestaña que se abra los nombres de las que ya estaban.

        // La primera se cierra, así que sus divisiones 0 y 1 quedan gastadas para siempre.
        ventana.activa = 0;
        ventana.aplicar(Orden::Cerrar, &ctx);
        assert_eq!(ventana.pestañas.len(), 1, "no se cerró la primera pestaña");

        // **Dos, y no una.** Con una sola no se distingue un contador que no retrocede de uno que se
        // deduce de las divisiones vivas: los nombres se toman **antes** de tocar el contador, así que la
        // primera que se abre sale bien con las dos versiones. El defecto solo asoma en la siguiente, que
        // reutilizaría los nombres que acaba de repartir. Comprobado mutándolo: con una sola creación la
        // prueba pasaba con el contador roto.
        ventana.crear_una_pestaña(&ctx);
        ventana.crear_una_pestaña(&ctx);
        assert_eq!(
            ventana.pestañas.len(),
            3,
            "no se abrieron las pestañas nuevas"
        );

        let vivos: Vec<usize> = ventana
            .todas_las_divisiones()
            .map(|division| division.nombre)
            .collect();
        let mut sin_repetir = vivos.clone();
        sin_repetir.sort_unstable();
        sin_repetir.dedup();
        assert_eq!(
            sin_repetir.len(),
            vivos.len(),
            "hay divisiones vivas que se llaman igual: {vivos:?}"
        );
        for gastado in [0, 1] {
            assert!(
                !vivos.contains(&gastado),
                "la división {gastado} se cerró y su nombre se le dio a otra: {vivos:?}"
            );
        }
    }

    #[test]
    fn abrir_una_pestaña_declara_lo_que_cuesta_lanzar_sus_dos_programas() {
        // Abrir una pestaña son **dos `fork`/`exec`**, y ocurren dentro de la pasada de interfaz porque
        // es donde se atiende el acorde. Nadie los declaraba: el «resto del fotograma» se los quedaba, y
        // ese número es el que dice cuánto cuesta el framework de interfaz — la cifra con la que D3
        // decide si egui aguanta. Lanzar dos procesos no es coste de egui.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([["sleep 30", "sleep 30"]]);

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::T));

        assert_eq!(ventana.pestañas.len(), 2, "no se abrió la pestaña");
        assert!(
            !ventana.cronometrado_aparte.is_zero(),
            "abrir una pestaña no declaró nada: los dos fork/exec se publican como coste del framework"
        );
    }

    #[test]
    fn cerrar_una_pestaña_no_se_lleva_lo_que_sus_consolas_ya_habian_declarado() {
        // Al cerrar una pestaña se sueltan sus divisiones **con su `Medida` dentro**, y ahí puede haber
        // una vuelta anotada de este mismo fotograma: bytes interpretados hace un instante. Si se va sin
        // declararse, la resta se la atribuye al framework y el «resto» sale inflado justo en el
        // fotograma del cierre.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["printf de-la-que-se-va", "sleep 30"],
            ["sleep 30", "sleep 30"],
        ]);
        esperar_a_que_muera(&ventana.pestañas[0].divisiones[0]);

        // El bombeo deja anotado lo que interpretó; la pasada de interfaz cierra su vuelta.
        ventana.bombear_todas();
        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );
        assert!(
            ventana.pestañas[0].divisiones[0]
                .pantalla
                .to_text()
                .contains("de-la-que-se-va"),
            "la consola no interpretó nada: la prueba no está midiendo lo que cree"
        );

        // **Un tramo enorme y a mano encima de lo que ya hay, y no es artificio: es lo que hace que la
        // afirmación signifique algo.** Con solo lo que la consola interpretó de verdad —microsegundos—,
        // recoger su vuelta y **tirarla** deja el campo lleno igualmente, porque el coste de cerrar la
        // pestaña también se suma ahí: comprobado mutándolo, la prueba pasaba con lo recogido descartado.
        // Cinco segundos no los puede producir el cierre, así que si aparecen es porque llegaron de la
        // consola que se fue.
        const SUYO: std::time::Duration = std::time::Duration::from_secs(5);
        ventana.pestañas[0].divisiones[0]
            .medida
            .anotar_vuelta(medida::Tramos::de_una_vuelta(
                None,
                Instant::now(),
                SUYO,
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
            ));

        ventana.aplicar(Orden::Cerrar, &ctx);

        assert_eq!(ventana.pestañas.len(), 1, "no se cerró la pestaña");
        assert!(
            ventana.cronometrado_aparte >= SUYO,
            "declaró {:?} y solo la vuelta de la consola que se fue valía {SUYO:?}: lo que la pestaña \
             cerrada había declarado se fue con ella, así que el resto del fotograma se queda con su \
             trabajo",
            ventana.cronometrado_aparte
        );
    }

    #[test]
    fn lo_declarado_aparte_se_descuenta_del_resto_y_no_se_arrastra_al_fotograma_siguiente() {
        // **Las dos pruebas de arriba no bastan, y esto lo demostró una mutación:** apuntar el coste en
        // el campo y no restarlo del total dejaba las 272 en verde. Comprobar que alguien anotó un número
        // es atar al ayudante; lo que hay que atar es que el resto **baje**, porque el resto es lo que el
        // informe publica como coste del framework de interfaz.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([["printf algo", "sleep 30"]]);
        esperar_a_que_muera(&ventana.pestañas[0].divisiones[0]);

        // Hace falta que alguna división declare algo: sin tramos de nadie no se anota resto.
        ventana.bombear_todas();
        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );

        // **Los números son absurdos para un fotograma, y a propósito.** En modo depuración, encolar el
        // pintado de una rejilla cuesta milisegundos, así que con un coste de 30 ms lo que declaran las
        // divisiones se come el margen y la resta satura a cero: la prueba pasaba con el defecto puesto.
        // Con diez segundos de fotograma y cuatro declarados aparte, todo lo que tarde la máquina es
        // ruido y la afirmación mide lo que dice medir. Comprobado mutándolo.
        const FOTOGRAMA: std::time::Duration = std::time::Duration::from_secs(10);
        ventana.cronometrado_aparte = std::time::Duration::from_secs(4);
        ventana.cerrar_fotograma(FOTOGRAMA);

        let resto = ventana.resto_del_fotograma.media();
        assert!(
            resto < std::time::Duration::from_secs(7),
            "el resto salió {resto:?} de 10s: los 4s declarados aparte no se descontaron"
        );

        // Y no se arrastra: un fotograma después, sin nada declarado aparte, el resto vuelve a ser el
        // total. Sin vaciar el campo se le restaría a un fotograma un trabajo que ocurrió en otro, y la
        // media se quedaría abajo.
        ventana.bombear_todas();
        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );
        ventana.cerrar_fotograma(FOTOGRAMA);
        let media = ventana.resto_del_fotograma.media();
        assert!(
            media > std::time::Duration::from_secs(7),
            "la media quedó en {media:?}: el descuento se aplicó dos veces, así que el campo no se vació \
             al cerrar el fotograma"
        );
    }

    #[test]
    fn un_fotograma_de_dos_pasadas_no_pierde_lo_que_interpreto_la_consola() {
        // **El sesgo, en una frase:** la segunda pasada vuelve a bombear, no encuentra nada —la cola se
        // vació en la primera— y anota una vuelta con lo interpretado a casi cero. Cuando eso
        // **sustituía** lo anotado, el trabajo de verdad de la consola desaparecía de lo declarado y la
        // resta se lo regalaba al framework de interfaz, que es la cifra con la que D3 decide. Medido
        // antes del arreglo: 50 ns declarados donde interpretar había costado 20 µs.
        //
        // Se corre por el fotograma de verdad, con las dos mitades dentro del cierre que egui repite:
        // por el otro camino este defecto es invisible.
        let ctx = egui::Context::default();
        let mut ventana = ventana_con("printf de-verdad", "sleep 30");
        esperar_a_que_muera(&ventana.activa().divisiones[0]);

        let pasadas = un_fotograma_como_eframe(&mut ventana, &ctx, true);
        assert_eq!(
            pasadas, 2,
            "egui no repitió la pasada, así que la prueba no está mirando lo que cree"
        );
        assert!(
            ventana.activa().divisiones[0]
                .pantalla
                .to_text()
                .contains("de-verdad"),
            "la consola no llegó a interpretar nada: la prueba no está midiendo lo que cree"
        );

        // El cierre se llama a mano porque `eframe::Frame::_new_kittest()` entrega `cpu_usage: None`, así
        // que por el camino de la aplicación no llega a ocurrir dentro de una prueba.
        ventana.activa_mut().divisiones[0]
            .medida
            .cerrar_fotograma(std::time::Duration::from_millis(50))
            .expect("la consola no declaró nada de lo que hizo");

        // **La afirmación es binaria a propósito, y no una comparación de duraciones.** Solo se anotan
        // medias cuando la vuelta trae espera, y la espera solo la tiene la pasada que de verdad recogió
        // bytes: la primera. Si la segunda pasada machaca esa vuelta, la que queda no tiene espera y las
        // cuatro series se quedan **vacías** — la sesión entera acabaría publicando un informe en blanco
        // para esta consola. Comparar tiempos aquí no valía: con el defecto puesto, lo que la segunda
        // pasada declara por encolar el pintado ya supera cualquier umbral pequeño, así que la prueba
        // pasaba igual. Comprobado mutándolo.
        assert!(
            ventana.activa().divisiones[0].medida.hay_medias(),
            "la vuelta buena se perdió: la que quedó no traía espera, así que las series del informe se \
             quedan sin una sola muestra de esta consola"
        );
    }

    /// Los tres eventos de un clic del ratón en un punto.
    fn clic_en(donde: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(donde),
            egui::Event::PointerButton {
                pos: donde,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
            egui::Event::PointerButton {
                pos: donde,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            },
        ]
    }

    #[test]
    fn al_llegar_a_una_pestaña_por_clic_sus_consolas_se_miden_con_el_divisor_de_ella() {
        // **El defecto que esto ata costó una revisión entera.** El reparto del ancho sale de la fracción
        // de la pestaña activa, y se calculaba **antes** de atender el clic de la barra: el fotograma del
        // clic pintaba las consolas de la pestaña nueva dentro de los rectángulos de la vieja. Y no era
        // solo verse mal — de esos rectángulos sale `medido`, que es el tamaño que se le anuncia al
        // programa de dentro, así que los dos pseudoterminales recibían una rejilla equivocada y la
        // corregían al fotograma siguiente: un reajuste doble y visible en cualquier programa a pantalla
        // completa.
        //
        // Se corre el fotograma **entero** a propósito: lo que falla aquí es el orden dentro de `ui`, y
        // llamando a las piezas por separado la prueba pasaría con el defecto puesto.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        // Divisores muy distintos: con dos repartos parecidos las dos rejillas darían el mismo número de
        // columnas y la prueba no vería nada.
        ventana.pestañas[0].fraccion = 0.2;
        ventana.pestañas[1].fraccion = 0.8;

        // Primera pasada: se registran las regiones. egui resuelve la interacción contra la anterior, así
        // que un clic en la primera no encontraría a nadie a quien dárselo.
        un_fotograma_con(&mut ventana, &ctx, Vec::new());
        let con_el_divisor_de_la_primera = ventana.pestañas[0].divisiones[0].medido.cols;

        // La ficha de la segunda pestaña ocupa la mitad derecha de la barra.
        un_fotograma_con(&mut ventana, &ctx, clic_en(egui::pos2(600.0, 8.0)));

        assert_eq!(ventana.activa, 1, "el clic no cambió de pestaña");
        let medida_de_la_segunda = ventana.pestañas[1].divisiones[0].medido.cols;
        assert_ne!(
            medida_de_la_segunda, con_el_divisor_de_la_primera,
            "la consola de la pestaña nueva se midió con el divisor de la anterior"
        );
        assert!(
            medida_de_la_segunda > con_el_divisor_de_la_primera,
            "la izquierda de la segunda pestaña se lleva el 80 % y salió más estrecha que el 20 % de la primera"
        );
    }

    #[test]
    fn una_pestaña_que_no_se_ve_recibe_la_rejilla_de_la_ventana_y_no_la_de_arranque() {
        // El programa de una pestaña de fondo formatea su salida para el tamaño que se le anunció. Si
        // nadie se lo cuenta se queda con el de arranque —24×80— y escribe para una ventana que no
        // existe: al volver a esa pestaña, lo escrito está ajustado a un ancho ajeno.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        un_fotograma_con(&mut ventana, &ctx, Vec::new());

        let de_fondo = ventana.pestañas[1].divisiones[0].medido;
        assert_ne!(
            de_fondo, REJILLA_INICIAL,
            "la pestaña de fondo se quedó con la rejilla de arranque"
        );
        // Las dos pestañas nacen con la misma fracción y comparten el área, así que la de fondo tiene
        // que medir exactamente lo mismo que la que se está pintando.
        assert_eq!(
            de_fondo, ventana.pestañas[0].divisiones[0].medido,
            "la de fondo no mide lo mismo que la visible teniendo el mismo divisor y la misma área"
        );
    }

    #[test]
    fn la_de_fondo_se_mide_con_su_divisor_y_no_con_el_de_la_visible() {
        // El divisor es de la disposición, así que cada pestaña reparte su área con el suyo. Medir las
        // de fondo con el de la pestaña visible les daría una rejilla que cambia sola cada vez que
        // arrastras un divisor que no es el de ellas.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        // Muy distintos a propósito: con dos repartos parecidos las dos rejillas darían el mismo
        // número de columnas y la prueba no vería nada.
        ventana.pestañas[0].fraccion = 0.2;
        ventana.pestañas[1].fraccion = 0.8;

        un_fotograma_con(&mut ventana, &ctx, Vec::new());
        let desde_el_fondo = [
            ventana.pestañas[1].divisiones[0].medido,
            ventana.pestañas[1].divisiones[1].medido,
        ];

        // Y ahora la misma pestaña, pero pintándose: lo que se mide desde el fondo tiene que ser lo
        // que se mide al verla. Comparar contra la otra pestaña solo diría que salieron distintas.
        ventana.activa = 1;
        un_fotograma_con(&mut ventana, &ctx, Vec::new());

        assert_eq!(
            desde_el_fondo,
            [
                ventana.pestañas[1].divisiones[0].medido,
                ventana.pestañas[1].divisiones[1].medido,
            ],
            "la pestaña de fondo se midió con un divisor que no era el suyo"
        );
    }

    #[test]
    fn la_rejilla_de_una_pestaña_de_fondo_llega_hasta_su_rejilla_de_verdad() {
        // Escribir `medido` no le sirve de nada a nadie si el campo se queda ahí: quien lo aplica es el
        // bombeo del fotograma siguiente. Sin este paso, la prueba de arriba pasaría con la rejilla de
        // la consola —y con la del programa de dentro— todavía en 24×80.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        un_fotograma_con(&mut ventana, &ctx, Vec::new());
        un_fotograma_con(&mut ventana, &ctx, Vec::new());

        let de_fondo = &ventana.pestañas[1].divisiones[0];
        assert_eq!(
            de_fondo.pantalla.size(),
            de_fondo.medido,
            "la rejilla de la consola de fondo no llegó a cambiar de tamaño"
        );
        assert_ne!(
            de_fondo.pantalla.size(),
            REJILLA_INICIAL,
            "la consola de fondo sigue interpretando contra la rejilla de arranque"
        );
    }

    #[test]
    fn el_panel_que_aparece_en_una_pestaña_de_fondo_le_estrecha_la_consola() {
        // **De que haya panel depende cuánto ancho le queda a la consola** —se lleva 36 columnas—, y
        // el estado del panel solo se refrescaba al pintar. Una división de fondo cuyo productor
        // publica mientras no la miras se medía con el rectángulo entero: 36 columnas de más durante
        // todo el segundo plano, o sea el mismo defecto que esta función arregla, con el signo
        // cambiado y más grande. Es además el caso central del producto: un agente que trabaja en la
        // pestaña que no estás mirando y publica su estado.
        let ctx = egui::Context::default();
        let area = area_de(1200.0);
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        ventana.pestañas[1].divisiones[0].panel =
            Some(PanelDir::create().expect("crear el directorio"));

        // Con el directorio vacío no hay nada que enseñar, así que la consola ocupa su rectángulo.
        pasada_de_interfaz(&mut ventana, &ctx, area, egui::RawInput::default());
        let sin_panel = ventana.pestañas[1].divisiones[0].medido;

        // Y ahora el productor publica, con su pestaña todavía de fondo.
        publicar_en(
            &ventana.pestañas[1].divisiones[0],
            r#"{"lines":["algo publicado"]}"#,
        );
        ventana.pestañas[1].divisiones[0].por_releer.levantar();
        pasada_de_interfaz(&mut ventana, &ctx, area, egui::RawInput::default());

        let de_fondo = ventana.pestañas[1].divisiones[0].medido;
        assert!(
            de_fondo.cols < sin_panel.cols,
            "la consola de fondo no cedió ancho a un panel ya publicado: midió {} columnas, las \
             mismas que sin panel",
            de_fondo.cols
        );

        // Y lo definitivo: mide lo mismo que mediría pintándose. Comparar solo contra «más estrecha»
        // dejaría pasar cualquier ancho equivocado que fuera menor.
        ventana.activa = 1;
        pasada_de_interfaz(&mut ventana, &ctx, area, egui::RawInput::default());
        assert_eq!(
            de_fondo, ventana.pestañas[1].divisiones[0].medido,
            "la consola se mide distinto según se la mire o no"
        );
    }

    #[test]
    fn lo_que_cuesta_releer_el_panel_de_una_de_fondo_se_declara() {
        // Releer el `panel.json` de una consola de fondo es tocar el disco **dentro** de la pasada de
        // interfaz, y ahí no hay pintado que lo cronometre. Sin declararlo, la resta se lo regala al
        // framework de interfaz — la cifra con la que D3 decide si egui aguanta, y el error que este
        // proyecto ya cometió tres veces.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        ventana.pestañas[1].divisiones[0].panel =
            Some(PanelDir::create().expect("crear el directorio"));
        publicar_en(
            &ventana.pestañas[1].divisiones[0],
            r#"{"lines":["algo publicado"]}"#,
        );
        ventana.pestañas[1].divisiones[0].por_releer.levantar();

        // El tramo del panel viaja con la vuelta que deja el bombeo, así que hay que bombear. Lo que
        // interpretó se guarda aparte para poder afirmar sobre **el panel** y no sobre la suma: sin
        // esa resta, la prueba la cumpliría cualquier tramo que declarase algo.
        ventana.bombear_todas();
        let (_, _, interpretado) = ventana.pestañas[1].divisiones[0]
            .pendiente_de_medir
            .expect("el bombeo tiene que dejar pendiente la vuelta de la consola de fondo");

        pasada_de_interfaz(
            &mut ventana,
            &ctx,
            area_de(1200.0),
            egui::RawInput::default(),
        );

        let declarado = ventana.pestañas[1].divisiones[0]
            .medida
            .cerrar_fotograma(std::time::Duration::from_millis(10))
            .expect("la consola de fondo no declaró nada");
        assert!(
            declarado > interpretado,
            "declaró {declarado:?} y solo interpretar ya costó {interpretado:?}: leer el panel se \
             publicaría como coste del framework"
        );
        assert!(
            !ventana.pestañas[1].divisiones[0].medida.hay_medias(),
            "leer el panel de una consola que no se pinta ensució una media: no hubo vuelta que \
             medir, solo algo que restar"
        );
    }

    #[test]
    fn con_tres_pestañas_abiertas_se_miden_todas_las_de_fondo_y_no_solo_la_primera() {
        // **Medido, no supuesto:** con el bucle cortado después de la primera de fondo —un `break` al
        // final de la vuelta— las 263 pruebas de antes seguían verdes, porque todas abrían dos
        // pestañas y con dos la primera de fondo es la única que hay. A partir de la tercera, la que
        // se queda sin medir corre con la rejilla de arranque durante todo el segundo plano, que es
        // exactamente lo que este hito promete que no pasa. Y tres es lo normal aquí: el producto son
        // varios agentes a la vez, no dos.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
        ]);
        // Tres divisores distintos: con fracciones parecidas las tres pestañas darían el mismo número
        // de columnas y medir una por otra no se vería.
        ventana.pestañas[0].fraccion = 0.2;
        ventana.pestañas[1].fraccion = 0.5;
        ventana.pestañas[2].fraccion = 0.8;

        un_fotograma_con(&mut ventana, &ctx, Vec::new());
        let desde_el_fondo: Vec<[Size; 2]> = ventana.pestañas[1..]
            .iter()
            .map(|pestaña| [pestaña.divisiones[0].medido, pestaña.divisiones[1].medido])
            .collect();

        for (cual, medidas) in desde_el_fondo.iter().enumerate() {
            assert!(
                !medidas.contains(&REJILLA_INICIAL),
                "la pestaña {} se quedó con la rejilla de arranque: midió {medidas:?}",
                cual + 1
            );
        }

        // Y cada una tiene que medir desde el fondo lo que mide al verse. Comparar solo contra la
        // rejilla de arranque dejaría pasar cualquier tamaño equivocado que no fuera 24×80.
        for (cual, medidas) in desde_el_fondo.iter().enumerate() {
            let pestaña = cual + 1;
            ventana.activa = pestaña;
            un_fotograma_con(&mut ventana, &ctx, Vec::new());
            assert_eq!(
                *medidas,
                [
                    ventana.pestañas[pestaña].divisiones[0].medido,
                    ventana.pestañas[pestaña].divisiones[1].medido,
                ],
                "la pestaña {pestaña} no midió desde el fondo lo que mide al verse"
            );
        }
    }

    #[test]
    fn al_aparecer_la_barra_la_pestaña_que_pasa_a_fondo_pierde_el_alto_que_la_barra_ocupa() {
        // El fotograma en que se abre la segunda pestaña es el único en que el área de las consolas
        // encoge: hasta ahí no había barra que apartar. La que se queda detrás no se pinta en ese
        // fotograma, así que si se la mide contra el área de antes se queda con **una fila de más**
        // hasta que algo más la vuelva a medir, y su programa escribe una línea que no cabe.
        //
        // **Medido:** medir contra el área de antes de apartar la barra —el rectángulo entero de la
        // ventana— tira ésta y otras tres. Antes de que existiera, ninguna prueba abría una pestaña y
        // miraba después lo que midió la que se quedó detrás.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([["sleep 30", "sleep 30"]]);

        // Con una sola pestaña no hay barra, así que las consolas se llevan el alto entero.
        un_fotograma_con(&mut ventana, &ctx, Vec::new());
        let sin_barra = ventana.pestañas[0].divisiones[0].medido;

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::T));

        assert_eq!(ventana.pestañas.len(), 2, "no se abrió la pestaña");
        let paso_a_fondo = ventana.pestañas[0].divisiones[0].medido;
        assert!(
            paso_a_fondo.rows < sin_barra.rows,
            "la que se quedó detrás midió {} filas, las mismas que cuando no había barra",
            paso_a_fondo.rows
        );
        // Se comparan las filas y no la rejilla entera: lo que la barra quita es alto, y las columnas
        // de la pestaña nueva dependen además de su panel, que la de antes no tiene.
        assert_eq!(
            paso_a_fondo.rows, ventana.pestañas[1].divisiones[0].medido.rows,
            "la que se quedó detrás no tiene el alto de la que ahora se ve"
        );
    }

    #[test]
    fn el_programa_de_una_pestaña_de_fondo_se_entera_del_tamaño_que_se_le_midio() {
        // La última cláusula del criterio que este cambio cubre: que el tamaño **llegue hasta el
        // programa de dentro**. Las demás pruebas llegan hasta `pantalla`, que es esta misma
        // aplicación contándose las cosas a sí misma.
        //
        // Lo que añade sobre `al_redimensionar_el_programa_de_dentro_se_entera_del_tamano_nuevo` es el
        // **camino entero**: aquella llama a `redimensionar` a mano sobre una división suelta, así que
        // demuestra que el `ioctl` funciona, no que alguien lo dispare por una pestaña que nunca se
        // pinta. Ésta pasa por el fotograma completo, que es donde el defecto vivía. Las dos caen si
        // se quita el `ioctl`, y eso está bien: se cubren dos tramos distintos del mismo camino.
        //
        // Se pregunta con `stty size` porque no hay otra forma: el tamaño vive en el núcleo, del lado
        // del esclavo, y desde este proceso no se puede leer.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["sleep 30", "sleep 30"],
            ["while :; do stty size; sleep 0.05; done", "sleep 30"],
        ]);

        // Dos fotogramas: el primero mide —la pasada de interfaz escribe `medido`— y el segundo
        // aplica, porque `redimensionar` corre en la mitad lógica, antes de medir.
        un_fotograma_con(&mut ventana, &ctx, Vec::new());
        un_fotograma_con(&mut ventana, &ctx, Vec::new());

        let de_fondo = &mut ventana.pestañas[1].divisiones[0];
        assert_ne!(
            de_fondo.medido, REJILLA_INICIAL,
            "la de fondo se quedó con la rejilla de arranque y la espera de abajo no probaría nada"
        );
        let suyo = format!("{} {}", de_fondo.medido.rows, de_fondo.medido.cols);
        esperar_a_que_la_pantalla_diga(de_fondo, &suyo);
    }

    #[test]
    fn desde_la_primera_hacia_atras_se_llega_a_la_ultima() {
        // El ciclo, que es lo que hace navegable una barra sin bordes: con el resto de siempre en vez de
        // `rem_euclid` esto sería un índice negativo y un pánico en el primer fotograma.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
        ]);

        ventana.aplicar(Orden::Ir(-1), &ctx);
        assert_eq!(
            ventana.activa, 2,
            "hacia atrás desde la primera no dio la vuelta"
        );

        ventana.aplicar(Orden::Ir(1), &ctx);
        assert_eq!(
            ventana.activa, 0,
            "hacia delante desde la última no dio la vuelta"
        );
    }

    #[test]
    fn tras_cambiar_de_pestaña_el_teclado_va_a_la_consola_con_el_foco_de_esa() {
        // AC8, la mitad que se olvida: cambiar de pestaña sin llevarse el foco escribiría en la consola
        // que tenía el foco en la pestaña **anterior**, que es la forma más silenciosa de escribir en el
        // programa equivocado.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        // La segunda pestaña escucha por su consola derecha; la primera, por la izquierda.
        ventana.pestañas[1].foco = 1;

        ventana.aplicar(Orden::Ir(1), &ctx);
        encolar(&ctx, vec![egui::Event::Text("x".to_owned())]);
        ventana.teclear_la_del_foco(&ctx);

        assert_eq!(
            ventana.pestañas[1].divisiones[1].pulsaciones, b"x",
            "la tecla no fue a la consola con el foco de la pestaña nueva"
        );
        assert!(
            ventana.pestañas[1].divisiones[0].pulsaciones.is_empty(),
            "la tecla fue a la consola que tenía el foco en la pestaña anterior"
        );
    }

    #[test]
    fn un_clic_en_una_ficha_cambia_de_pestaña() {
        // AC8 por la otra puerta. Se prueba con un `Ui` de verdad por lo mismo que el clic en una consola:
        // lo que hay que comprobar es la traducción de un clic del escritorio, y eso solo lo sabe egui.
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let barra = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 20.0));
        let dentro_de_la_segunda = egui::pos2(300.0, 10.0);

        let ctx = egui::Context::default();
        // Dos pasadas: en la primera se registran las regiones, y en la segunda llega el clic. egui
        // resuelve la interacción contra lo que había en la pasada anterior.
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ventana.atender_clics_de_la_barra(ui, barra);
        })
        .drop_without_applying_deltas();

        let entrada = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(dentro_de_la_segunda),
                egui::Event::PointerButton {
                    pos: dentro_de_la_segunda,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos: dentro_de_la_segunda,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| {
            ventana.atender_clics_de_la_barra(ui, barra);
        })
        .drop_without_applying_deltas();

        assert_eq!(
            ventana.activa, 1,
            "el clic en la segunda ficha no cambió de pestaña"
        );
    }

    #[test]
    fn el_acorde_cambia_de_pestaña_dentro_del_fotograma_entero() {
        // **AC8 por el camino real, y el hueco que esto tapa era literal**: lo que había ataba el acorde
        // hasta `teclear_la_del_foco` y luego llamaba a `aplicar` **desde la prueba**. Borrar en `ui` las
        // tres líneas que aplican la orden dejaba la suite entera en verde con el acorde muerto —
        // comprobado mutándolo. Es el ayudante disfrazado de camino real que este proyecto ya ha pagado.
        //
        // El clic tiene su versión de esto; el teclado, hasta ahora, no.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        un_fotograma_con(&mut ventana, &ctx, acorde_con(egui::Key::ArrowRight));

        assert_eq!(
            ventana.activa, 1,
            "el fotograma no llegó a aplicar lo que pidió el acorde"
        );
        assert!(
            ventana.pestañas[0].divisiones[0].pulsaciones.is_empty(),
            "el acorde se le coló al programa de la pestaña que se dejaba"
        );
    }

    #[test]
    fn sin_las_dos_teclas_del_acorde_la_flecha_es_del_programa() {
        // **Con Ctrl sola o con Mayúsculas solas la flecha sigue siendo del programa**, y las dos son
        // teclas que se usan: `Ctrl+→` salta de palabra en un shell y `Mayúsculas+→` selecciona en medio
        // editor. Quitar cualquiera de las dos guardas dejaba la suite en verde —comprobado mutando las
        // dos por separado— y le habría quitado esas teclas a todo el mundo con dos pestañas abiertas.
        for (teclas, como) in [
            (
                egui::Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
                "con Ctrl sola",
            ),
            (
                egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                "con Mayúsculas solas",
            ),
        ] {
            let ctx = egui::Context::default();
            let mut ventana =
                ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

            encolar(&ctx, tecla_con(egui::Key::ArrowRight, teclas));

            assert_eq!(
                ventana.teclear_la_del_foco(&ctx),
                Vec::new(),
                "{como} se pidió cambiar de pestaña"
            );
            assert_eq!(
                ventana.activa().divisiones[0].pulsaciones,
                b"\x1b[C",
                "{como} la flecha se la tragó el acorde en vez de llegar al programa"
            );
        }
    }

    #[test]
    fn soltar_la_flecha_del_acorde_no_cuenta_como_otra_pestaña() {
        // **Un teclado real manda dos eventos por tecla, y los dos caen en el mismo lote.** Sin exigir
        // que esté pulsada, el que avisa de que se suelta cuenta otra vez y una sola pulsación saltaría
        // **dos** pestañas — comprobado mutando la guarda, que sobrevivía porque ninguna prueba soltaba
        // la tecla.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
        ]);

        let mut lote = acorde_con(egui::Key::ArrowRight);
        lote.push(egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: con_mayusculas(),
        });
        encolar(&ctx, lote);

        assert_eq!(
            ventana.teclear_la_del_foco(&ctx),
            vec![Orden::Ir(1)],
            "soltar la flecha contó como una pulsación más"
        );
    }

    #[test]
    fn un_clic_lleva_a_la_ficha_que_se_pulso_y_no_a_la_de_al_lado() {
        // **El clic va a la ficha que se pulsa, y eso no es lo mismo que «cambia de pestaña».** Con dos
        // pestañas las dos frases se cumplen a la vez, y por eso sustituir el desplazamiento calculado por
        // un `Ir(1)` fijo dejaba la suite en verde. Con tres se separan: saltar a la tercera desde la
        // primera son dos, y volver a pulsar la ficha en la que ya se está son **cero**.
        let ctx = egui::Context::default();
        let mut ventana = ventana_de_pestañas([
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
        ]);

        // Primera pasada: se registran las regiones. egui resuelve la interacción contra la anterior.
        un_fotograma_con(&mut ventana, &ctx, Vec::new());

        // La tercera de tres fichas ocupa el último tercio de una barra de 800 puntos.
        let en_la_tercera = egui::pos2(700.0, 8.0);
        un_fotograma_con(&mut ventana, &ctx, clic_en(en_la_tercera));
        assert_eq!(
            ventana.activa, 2,
            "el clic en la tercera ficha llevó a otra pestaña"
        );

        un_fotograma_con(&mut ventana, &ctx, clic_en(en_la_tercera));
        assert_eq!(
            ventana.activa, 2,
            "volver a pulsar la ficha activa movió de pestaña"
        );
    }

    #[test]
    fn la_barra_pinta_el_numero_de_cada_pestaña_y_realza_la_activa() {
        // **Vaciar `pintar_la_barra` dejaba la suite entera en verde con la barra invisible**: lo que
        // había ataba dónde cae la barra y qué se puede pulsar en ella, pero nada de lo que se ve. Una
        // barra que aparta alto y no pinta nada es una franja negra por la que uno cambia de pestaña a
        // ciegas.
        //
        // Se mira **lo que la pasada encoló**, no el estado que lo produce, y se reconoce cada forma por
        // el trozo al que va recortada: cada ficha se pinta con su propio `painter_at`, así que el
        // recorte dice a qué pestaña pertenece lo que se pintó.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        ventana.activa = 1;

        let area = area_de(800.0);
        let formas = pasada_de_interfaz(&mut ventana, &ctx, area, egui::RawInput::default());
        let barra = con_metrica(|metrica| {
            apartar_la_barra(area, 2, metrica)
                .0
                .expect("con dos pestañas tiene que haber barra")
        });

        // El color entra en la afirmación porque es la mitad de que un número **se lea**: intercambiar
        // el de la activa con el de las demás pinta cada número del color de su propio fondo y deja la
        // barra en blanco sin que ninguna forma desaparezca.
        let numeros: Vec<(egui::Rect, String, egui::Color32)> = formas
            .iter()
            .filter_map(|recortada| match &recortada.shape {
                egui::Shape::Text(texto) if barra.contains_rect(recortada.clip_rect) => Some((
                    recortada.clip_rect,
                    texto.galley.text().to_owned(),
                    texto.fallback_color,
                )),
                _ => None,
            })
            .collect();
        assert_eq!(
            numeros,
            vec![
                (
                    ficha(barra, 0, 2),
                    "1".to_owned(),
                    ventana.tema.primer_plano()
                ),
                (ficha(barra, 1, 2), "2".to_owned(), ventana.tema.fondo()),
            ],
            // Empiezan en uno: los ceros son del informe, que es un instrumento, y esto es lo que lee
            // quien tiene la ventana delante.
            "la barra no pintó un número legible por ficha, empezando en uno y en su sitio"
        );

        let realzadas: Vec<egui::Rect> = formas
            .iter()
            .filter_map(|recortada| match &recortada.shape {
                egui::Shape::Rect(pintada)
                    if pintada.fill == ventana.tema.foco()
                        && barra.contains_rect(recortada.clip_rect) =>
                {
                    Some(recortada.clip_rect)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            realzadas,
            vec![ficha(barra, 1, 2)],
            "el realce de la barra no señala exactamente a la pestaña activa"
        );
    }

    #[test]
    fn la_variable_de_entorno_dice_cuantas_pestañas_se_abren() {
        // AC13. Sin esta puerta la tanda de dos pestañas del anexo no se puede lanzar: los atajos entran
        // por eventos del framework, así que un guion no tiene forma de abrir la segunda.
        //
        // **No se toca el entorno de verdad, y eso lo decidió una revisión.** `std::env::set_var` tiene
        // contrato sobre **todo** el entorno: con él puesto, cualquier lectura concurrente es
        // comportamiento indefinido, no solo la de esta misma clave. Y esta suite las hace a mansalva y en
        // paralelo — `Division::nueva` lee `SHELL`, y `portable_pty` recorre `std::env::vars_os()` al
        // construir **cada** consola. La primera versión de esta prueba sembraba fallos intermitentes en
        // toda la suite. Lo que queda sin cubrir es la línea que lee la variable, y está dicho donde vive.
        assert_eq!(cuantas_pestañas(Some("3")), 3, "no se leyó el número");

        // Un cero abriría una ventana sin pestañas, y el primer acceso a la activa indexaría fuera de la
        // lista: la invariante es que nunca está vacía.
        assert_eq!(
            cuantas_pestañas(Some("0")),
            1,
            "un cero dejó la ventana sin pestañas"
        );

        // Un valor mal escrito no aborta el arranque: se abre como si no estuviera, y quien mide lo ve en
        // cuanto mira la ventana. Un negativo tampoco es un número de pestañas.
        assert_eq!(
            cuantas_pestañas(Some("dos")),
            1,
            "un valor sin número no se ignoró"
        );
        assert_eq!(cuantas_pestañas(Some("-2")), 1, "un negativo no se ignoró");
        assert_eq!(
            cuantas_pestañas(None),
            1,
            "sin variable no abre con una sola"
        );
    }

    #[test]
    fn la_variable_que_se_lee_es_la_del_guion_de_medida() {
        // El nombre **es** el contrato con `scripts/medir-fotograma.sh`, que ya exporta `MEDIR_MIB` y
        // `MEDIR_TANDA`. Cambiarlo aquí dejaría la puerta cerrada sin que nada se pusiera rojo: el guion
        // seguiría midiendo una sola pestaña creyendo que mide dos.
        assert_eq!(VARIABLE_DE_LAS_PESTAÑAS, "MEDIR_PESTANAS");
    }

    #[test]
    fn una_ventana_de_varias_pestañas_las_abre_todas_vivas_y_vigiladas() {
        // La otra mitad de AC13: que el número llegue a ser pestañas de verdad. Y **el vigilante sobre las
        // de todas**, que es lo que no se ve al mirar la ventana — armarlo solo sobre la primera dejaría
        // los paneles de las de fondo congelados en silencio, que es justo lo contrario del hito.
        let ventana = Ventana::con_pestañas(&egui::Context::default(), 3)
            .expect("no se pudieron abrir las tres pestañas");

        assert_eq!(ventana.pestañas.len(), 3, "no abrió las tres pestañas");
        assert!(
            ventana.activa < ventana.pestañas.len(),
            "la pestaña activa apunta fuera de la lista"
        );

        let directorios: std::collections::HashSet<_> = ventana
            .todas_las_divisiones()
            .map(|division| {
                division
                    .panel
                    .as_ref()
                    .expect("cada consola arranca con su directorio")
                    .path()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            directorios.len(),
            6,
            "dos consolas comparten directorio, así que se pisarían el estado"
        );
        assert!(
            !ventana.vigilante.esta_ciego(),
            "la ventana abrió sin nadie que la avise"
        );
    }

    #[test]
    fn la_ventana_se_entera_de_lo_que_publica_una_consola_de_la_ultima_pestaña() {
        // **`esta_ciego` no distingue vigilar una pestaña de vigilarlas todas.** Armar el vigilante solo
        // sobre las divisiones de la primera dejaba la suite entera en verde —comprobado mutándolo— y los
        // paneles de las de fondo congelados hasta que alguien mirase esa pestaña, que es exactamente lo
        // contrario de lo que promete el hito. Se publica en la **última** porque es la que se queda fuera
        // de cualquier atajo que se tome al armar.
        let ventana = Ventana::con_pestañas(&egui::Context::default(), 3)
            .expect("no se pudieron abrir las tres pestañas");
        let de_fondo = &ventana.pestañas[2].divisiones[1];
        for division in ventana.todas_las_divisiones() {
            division.por_releer.recoger();
        }

        publicar_en(de_fondo, r#"{"lines":["desde el fondo"]}"#);

        esperar_la_bandera(
            &de_fondo.por_releer,
            "la ventana no vigila el directorio de una consola de la última pestaña",
        );
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
    ///
    /// **Devuelve lo que la pasada encoló**, para quien necesite afirmar sobre lo que se ve y no sobre
    /// el estado que lo produce. La mayoría de las pruebas de aquí lo ignoran a propósito: miran el
    /// estado porque es lo que están atando.
    fn pasada_de_interfaz(
        ventana: &mut Ventana,
        ctx: &egui::Context,
        area: egui::Rect,
        mut entrada: egui::RawInput,
    ) -> Vec<egui::epaint::ClippedShape> {
        entrada.screen_rect = Some(area);
        let mut frame = eframe::Frame::_new_kittest();
        let mut salida = ctx.run_ui(entrada, |ui| eframe::App::ui(ventana, ui, &mut frame));
        let formas = std::mem::take(&mut salida.shapes);
        salida.drop_without_applying_deltas();
        formas
    }

    /// Si entre lo que la pasada encoló a pintar hay un texto que dice esto.
    ///
    /// **Se mira lo encolado y no el estado del panel**, porque lo que se promete es que el aviso *se
    /// ve*: un estado impecable que nadie llega a pintar se lee igual de bien desde el test y no se ve
    /// en la pantalla.
    fn se_encolo_el_texto(formas: &[egui::epaint::ClippedShape], dice: &str) -> bool {
        formas.iter().any(|recortada| {
            matches!(&recortada.shape, egui::Shape::Text(texto) if texto.galley.text().contains(dice))
        })
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
        let mut division = division_con("sleep 30", 0);
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
        let mut divisiones = [division_con("sleep 30", 0), division_con("sleep 30", 1)];
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
        let mut division = division_con("sleep 30", 0);
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

    #[test]
    fn sin_vigilante_el_respaldo_alcanza_tambien_a_las_pestañas_que_no_se_ven() {
        // El respaldo va en el sentido contrario al despertador, y por eso hay que atarlo aparte.
        //
        // Con una sola pestaña esa distinción no se ve, así que sin esta prueba dejar el respaldo
        // mirando solo la visible pasa inadvertido: comprobado, la suite entera sigue verde.
        let ctx = egui::Context::default();
        let mut ventana =
            ventana_de_pestañas([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let dir = PanelDir::create().expect("crear el directorio");
        let sin_armar = dir.path().join("no-existe");
        ventana.vigilante = Vigilante::nuevo(|| {}, &[(sin_armar.as_path(), Bandera::levantada())]);
        assert!(ventana.vigilante.esta_ciego(), "el respaldo no está activo");
        for division in ventana.todas_las_divisiones() {
            division.por_releer.recoger();
        }
        dormir_la_ventana(&ctx);

        eframe::App::logic(&mut ventana, &ctx, &mut eframe::Frame::_new_kittest());

        for (cual, division) in ventana.pestañas[1].divisiones.iter().enumerate() {
            assert!(
                division.por_releer.recoger(),
                "el panel de la consola {cual} de la pestaña de fondo se queda congelado hasta que \
                 alguien vuelva a mirarla"
            );
        }
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
        let division = division_con("sleep 30", 0);
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
