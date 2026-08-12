//! Los dos números que este hito tiene que publicar, y sus etiquetas.
//!
//! **Un número mal etiquetado es peor que ningún número**, y aquí eso no es una frase: el hito
//! existe para decidir si egui aguanta, y esa decisión se toma leyendo esto. Por eso cada número
//! dice exactamente qué tramo mide y cuál no.
//!
//! **Lo que se mide de la ida:** desde que empieza la pasada de interfaz que lee la tecla hasta que
//! sus bytes están escritos en el pseudoterminal. **Y lo que no**: el trecho entre pulsar la tecla y
//! esa pasada. No es un descuido — no es medible desde dentro del proceso: los eventos de teclado
//! de egui no traen sello propio, y el reloj de su entrada está documentado como «monótono,
//! relativo a lo que sea». Un número llamado «tecla → píxel» sería un número inventado.
//!
//! **Lo que se mide de la vuelta:** desde que el byte estaba disponible para recogerlo hasta que el
//! fotograma que lo muestra está pagado, en **cinco tramos que suman el total**:
//!
//! 1. **Esperar** — en la cola, hasta que el bucle lo recoge. En un volcado es el tramo que domina,
//!    y con diferencia: el presupuesto por fotograma deja pasar 128 KiB cada vez, así que lo que no
//!    cabe espera su turno.
//! 2. **Interpretar** — meterlo en la rejilla del terminal.
//! 3. **Encolar** — la llamada al pintor. Se llama así porque eso es lo que hace: apila formas y
//!    vuelve.
//! 4. **El panel de estado** — mirar si su fichero cambió, leerlo e interpretarlo cuando cambia, y
//!    encolar su pintado. Va aparte del tramo anterior aunque los dos sean trabajo del mismo
//!    fotograma: **agruparlos daría la misma resta con una etiqueta falsa**, y aquí eso ya costó una
//!    medida entera.
//! 5. **El resto del fotograma** — teselar esas formas —**las de las consolas y las de los
//!    paneles**—, dibujarlas, y todo el armazón que egui monta alrededor: recoger la entrada del
//!    escritorio, abrir y cerrar la pasada, medir la fuente, subir texturas. Es lo que queda del
//!    coste total al quitarle los tramos anteriores, y **se llama «el resto» a propósito**: desde
//!    dentro no hay forma de aislar la teselación, así que ponerle ese nombre sería publicar como
//!    medido algo que no lo está. El tramo del panel cubre **encolar** su pintado, no teselarlo.
//!
//! El total lo publica `eframe`, que es quien puede: cuando se tesela, la pasada ya ha terminado.
//!
//! **Los cuatro primeros tramos son de una división; el quinto es de la ventana.** Esa frontera no es
//! organizativa, la impone el instrumento: los cuatro se cronometran alrededor de
//! *una* consola, y así siguen siendo comparables con las tablas de D3 haya una división o cuatro.
//! El consumo de CPU, en cambio, lo publica el framework **una sola vez por pasada**: no viene
//! separado por división y partirlo sería inventar un reparto que nadie ha medido. Por eso el resto
//! se calcula una vez, restando al total la suma de lo cronometrado por **todas** las divisiones.
//!
//! Los cinco tramos se guardan **del mismo fotograma**. El coste total llega con uno de retraso
//! —`eframe` lo publica del anterior—, así que se retienen los tramos hasta que su total aparece;
//! restar tramos de un fotograma al total de otro daría un número que parece bueno y no lo es.

use std::collections::VecDeque;
use std::fmt;
use std::time::{Duration, Instant};

/// Cuántas muestras se guardan de cada número.
///
/// Un tope, y no todas: una sesión larga acumularía millones de muestras y el informe mediría sobre
/// todo el rato en que no pasaba nada. Con las últimas, lo que se publica describe cómo iba la cosa
/// **mientras se usaba**, que es lo que interesa.
const MUESTRAS: usize = 4096;

/// Una serie de tiempos, con lo que hace falta para resumirla.
#[derive(Default)]
struct Serie {
    muestras: VecDeque<Duration>,
    /// Cuántas se han tomado en total, incluidas las que ya no se guardan. Sin esto, un informe de
    /// 4 096 muestras sobre una sesión de un millón parecería la sesión entera.
    total: usize,
}

impl Serie {
    fn anotar(&mut self, cuanto: Duration) {
        self.total += 1;
        // Una cola por los dos extremos, y no un vector: sacar la muestra más vieja del principio de
        // un vector desplaza las otras cuatro mil, **dentro del fotograma que el instrumento está
        // midiendo**. El coste del instrumento acabaría dentro del tramo que el instrumento publica
        // como «el resto», que es justo el que hay que poder explicar.
        if self.muestras.len() == MUESTRAS {
            self.muestras.pop_front();
        }
        self.muestras.push_back(cuanto);
    }

    fn media(&self) -> Duration {
        let suma: Duration = self.muestras.iter().sum();
        suma / self.muestras.len().max(1) as u32
    }

    /// El valor por debajo del cual queda el 95 % de las muestras.
    ///
    /// Se publica junto a la media porque **el peor de todos no es un dato útil aquí**: una sola
    /// pausa del recolector del escritorio o un cambio de contexto lo dispara y tapa la forma de la
    /// distribución. El 95 es el que se nota al usar.
    fn p95(&self) -> Duration {
        if self.muestras.is_empty() {
            return Duration::ZERO;
        }
        let mut ordenadas: Vec<Duration> = self.muestras.iter().copied().collect();
        ordenadas.sort_unstable();
        let indice = (ordenadas.len() * 95 / 100).min(ordenadas.len() - 1);
        ordenadas[indice]
    }

    fn hay(&self) -> bool {
        !self.muestras.is_empty()
    }
}

impl fmt::Display for Serie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.3?} de media, {:.3?} el 95 %",
            self.media(),
            self.p95()
        )
    }
}

/// Los tramos de un fotograma, a la espera de que llegue su coste total.
///
/// **Es público y se construye con nombres a propósito.** Cuando esto se pasaba como tres duraciones
/// posicionales seguidas, intercambiar dos compilaba sin una queja y publicaba un informe mentiroso —
/// justo el fallo que la separación de tramos existe para impedir. Un tramo que se puede confundir con
/// otro en la llamada no está separado de nada.
#[derive(Clone, Copy)]
pub struct Tramos {
    /// Cuánto esperó en la cola el byte más antiguo, o `None` si a esta consola no le llegó nada en
    /// este fotograma.
    ///
    /// **Que sea opcional es lo que separa dos cosas que no son la misma:** si no llegó salida, no hay
    /// *vuelta* que medir —un fotograma en reposo no es una vuelta rapidísima—, pero la consola **sí se
    /// pintó**, y ese trabajo hay que devolverlo para que la ventana lo descuente del total. Cuando esto
    /// era un `Duration` a secas, el fotograma sin salida se descartaba entero y su pintado acababa
    /// publicado como «el resto del fotograma».
    esperado: Option<Duration>,
    interpretado: Duration,
    encolado: Duration,
    /// Lo que costó mirar si el `panel.json` de esta consola cambió y, si cambió, leerlo e
    /// interpretarlo.
    ///
    /// **Va aparte y no sumado a `encolado`, y esa separación es el motivo de que exista el campo.**
    /// Los dos ocurren dentro del fotograma y los dos hay que devolverlos, así que para la resta daría
    /// igual. Pero un tramo llamado «encolando el pintado» que además incluyera leer un fichero de
    /// disco sería un número con la etiqueta cambiada, y aquí eso ya costó una medida entera: el coste
    /// que no se nombra bien acaba atribuido a quien no lo hizo.
    panel: Duration,
}

impl Tramos {
    /// Los tramos de una vuelta, con cada duración puesta por su nombre.
    ///
    /// `esperando_desde` es cuándo llegó el byte más antiguo de los recogidos; sin bytes en este
    /// fotograma no hay nada que medir de la vuelta — un fotograma en reposo no es una vuelta
    /// rapidísima, es una vuelta que no ocurrió.
    pub fn de_una_vuelta(
        esperando_desde: Option<Instant>,
        recogido_en: Instant,
        interpretado: Duration,
        encolado: Duration,
        panel: Duration,
    ) -> Self {
        Self {
            esperado: esperando_desde.map(|desde| recogido_en.saturating_duration_since(desde)),
            interpretado,
            encolado,
            panel,
        }
    }
}

/// Los cronómetros **de una división** y lo que llevan medido.
///
/// Todo lo de aquí se cronometra alrededor de una consola concreta. El cuarto tramo no cabe aquí: ver
/// [`RestoDelFotograma`].
///
/// **Comparar estos números con los de una medida anterior exige mirar la rejilla, no solo el tramo.**
/// `encolar` es proporcional al número de celdas y `interpretar` depende de la anchura sobre la que
/// envuelve el texto, así que una consola que ocupa media ventana los da más bajos que una a pantalla
/// completa **sin que nada haya mejorado**. Por eso la cabecera del informe publica la rejilla de cada
/// consola: leer «encolar bajó» como «el framework aguanta mejor» es el error que este aviso existe
/// para evitar. El único tramo cuya definición no cambia con el reparto es `esperar`.
#[derive(Default)]
pub struct Medida {
    ida: Serie,
    esperar: Serie,
    interpretar: Serie,
    encolar: Serie,
    /// Mirar la fecha del `panel.json` y, cuando ha cambiado, leerlo e interpretarlo.
    panel: Serie,
    vuelta: Serie,
    /// Los tramos del fotograma anterior, esperando a que `eframe` publique lo que costó.
    pendiente: Option<Tramos>,
}

impl Medida {
    /// Anota lo que ha tardado la ida: de empezar la pasada de interfaz a escribir en el programa.
    pub fn anotar_ida(&mut self, cuanto: Duration) {
        self.ida.anotar(cuanto);
    }

    /// Anota los tramos de este fotograma. Quedan pendientes hasta que se sepa lo que costó entero.
    ///
    /// **Se guarda siempre, también sin salida.** Lo que decide si hubo vuelta es que la espera sea
    /// `Some`; el trabajo de pintar ocurrió en los dos casos y la ventana tiene que poder descontarlo.
    pub fn anotar_vuelta(&mut self, tramos: Tramos) {
        self.pendiente = Some(tramos);
    }

    /// Cierra el fotograma anterior de esta división con lo que `eframe` dice que costó de CPU.
    ///
    /// Devuelve **lo que esta división tenía cronometrado** —interpretar más encolar—, para que la
    /// ventana pueda restar de una vez lo de todas y sacar el resto. Lo devuelve **haya habido salida o
    /// no**: pintar ocurre en todos los fotogramas, y si esta consola no lo declarase, su pintado
    /// aparecería publicado como «el resto del fotograma», que es el número que decide si el framework
    /// de interfaz aguanta. `None` solo cuando no hubo ni pasada de interfaz que medir.
    ///
    /// **Las cuatro series, en cambio, solo cuentan cuando llegó salida**: un fotograma en reposo no es
    /// una vuelta rapidísima, es una vuelta que no ocurrió, y meterlo en la media diría que la
    /// aplicación responde al instante justamente por no haber hecho nada.
    pub fn cerrar_fotograma(&mut self, coste_cpu: Duration) -> Option<Duration> {
        let tramos = self.pendiente.take()?;

        if let Some(esperado) = tramos.esperado {
            self.esperar.anotar(esperado);
            self.interpretar.anotar(tramos.interpretado);
            self.encolar.anotar(tramos.encolado);
            self.panel.anotar(tramos.panel);
            self.vuelta.anotar(esperado + coste_cpu);
        }

        Some(tramos.interpretado + tramos.encolado + tramos.panel)
    }

    /// Lo que esta división lleva anotado del tramo del panel.
    ///
    /// Existe para que una prueba pueda afirmar que **el camino real declara ese tramo**, y no solo
    /// que la anotación funciona cuando se la llama a mano. Sin esto, dejar el coste del panel en cero
    /// dentro de la pasada de interfaz no rompía nada.
    #[cfg(test)]
    pub fn panel_anotado(&self) -> Option<Duration> {
        self.panel.hay().then(|| self.panel.media())
    }

    /// Si esta división llegó a medir algo, para no publicar el informe de una consola en blanco.
    pub fn hay_algo(&self) -> bool {
        self.ida.hay() || self.vuelta.hay()
    }

    /// Publica los números **de esta división** por la salida estándar.
    ///
    /// Se imprime al cerrar y no durante: la ventana es el terminal, así que no hay sitio dentro
    /// donde enseñarlos sin taparle la pantalla al programa.
    pub fn publicar_tramos(&self) {
        if self.ida.hay() {
            println!(
                "\ntecla en el fotograma → byte en el programa   ({} pulsaciones)",
                self.ida.total
            );
            println!("  {}", self.ida);
            println!(
                "  no incluye el trecho de la pulsación al fotograma: no es medible desde aquí"
            );
        }

        if self.vuelta.hay() {
            println!(
                "\nbyte disponible → fotograma pagado   ({} fotogramas con salida)",
                self.vuelta.total
            );
            println!("  {}", self.vuelta);
            println!("  esperando en la cola:   {}", self.esperar);
            println!("  interpretando:          {}", self.interpretar);
            println!("  encolando el pintado:   {}", self.encolar);
            println!("  el panel de estado:     {}", self.panel);
            // **La media de este tramo mezcla dos poblaciones y hay que decirlo.** Casi todos los
            // fotogramas solo miran la fecha del fichero —menos de un microsegundo—; los pocos en que
            // el productor ha publicado cuestan leer e interpretar, que es tres órdenes de magnitud
            // más. La media sale pegada al caso barato, así que el número que describe el caso caro es
            // el 95, no ella.
            println!(
                "    mezcla mirar la fecha (casi todos) con leer e interpretar (los de\n    \
                 publicación): la media describe el primero, el 95 % el segundo"
            );
            println!(
                "  incluye el fotograma entero, también el pintado de las otras consolas: un\n  \
                 fotograma se presenta indivisible. Por eso dos de estos números NO se suman"
            );
        }
    }
}

/// El resto del fotograma: lo que costó la pasada y nadie cronometró.
///
/// **Vive en la ventana y no en la división, y no es una elección de estilo.** El framework publica
/// un único consumo de CPU por pasada, sin separar por división; repartirlo entre las divisiones
/// sería inventarse un reparto que nadie ha medido, y anotarlo una vez por división contaría el mismo
/// coste tantas veces como consolas haya abiertas.
#[derive(Default)]
pub struct RestoDelFotograma {
    serie: Serie,
}

impl RestoDelFotograma {
    /// Anota el resto de este fotograma: lo que costó entero menos lo que cronometraron **todas** las
    /// divisiones.
    pub fn anotar(&mut self, coste_cpu: Duration, cronometrado: Duration) {
        // El total puede salir por debajo de los tramos medidos: son de fotogramas contiguos pero no
        // idénticos, y el reloj de `eframe` no es el mismo que el de aquí. Cuando pasa, el resto es
        // cero en vez de un número negativo con pinta de dato.
        self.serie.anotar(coste_cpu.saturating_sub(cronometrado));
    }

    pub fn publicar(&self) {
        if !self.serie.hay() {
            return;
        }
        println!(
            "\nel resto del fotograma — de la ventana, común a las dos consolas   ({} fotogramas)",
            self.serie.total
        );
        println!("  {}", self.serie);
        println!(
            "  es teselar, dibujar y el armazón de egui: desde dentro no se pueden separar, y\n  \
             llamarlo «teselado» sería publicar como medido algo que no lo está"
        );
        println!(
            "  ya tiene descontado lo que cronometraron las dos consolas, también el pintado de una\n  \
             que no recibiera salida en ese fotograma"
        );
    }
}

/// La cabecera del informe: dónde y cómo se midió.
///
/// Las rejillas van una por consola **y con su nombre**, porque el reparto del ancho las deja
/// distintas y ningún tramo se entiende sin saber sobre cuántas celdas se tomó. Es el dato que hace
/// falta para comparar con una medida antigua de una sola consola a pantalla completa: sin él, un
/// «encolar» más bajo se lee como que el framework va más rápido, cuando lo que pasa es que hay la
/// mitad de columnas.
pub fn cabecera(rejillas: [term_core::Size; 2]) {
    let perfil = if cfg!(debug_assertions) {
        "depuración — el número que vale es el de --release"
    } else {
        "release"
    };
    println!("\n── latencia ──");
    for (cual, rejilla) in rejillas.iter().enumerate() {
        println!(
            "{:<13} {}×{}  ({})",
            format!("consola {cual}:"),
            rejilla.rows,
            rejilla.cols,
            NOMBRE_DE_CONSOLA[cual]
        );
    }
    println!("renderizador: glow");
    println!("perfil:       {perfil}");
    println!("máquina:      {}", maquina());
}

/// Cómo se llama cada consola en el informe, por su sitio en la ventana.
///
/// Un número suelto obliga a adivinar de qué lado es, y el reparto del ancho hace que el lado importe.
pub const NOMBRE_DE_CONSOLA: [&str; 2] = ["izquierda", "derecha"];

/// Cómo se llama esta máquina, o «desconocida» si no hay forma de saberlo.
///
/// **Nunca calla.** El criterio pide la máquina junto al número, y un informe al que le falta esa
/// línea en silencio es un número que mañana no se sabe de dónde salió. `HOSTNAME` es variable de
/// shell y no suele estar exportada, así que apoyarse solo en ella era exactamente ese fallo
/// silencioso: en el primer volcado medido la línea no salió y nadie se enteró.
fn maquina() -> String {
    if let Ok(nombre) = std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")) {
        return nombre;
    }
    // El fichero es de Linux y de los BSD; en otros sistemas se cae al literal, que es la respuesta
    // honesta y no una invención.
    std::fs::read_to_string("/etc/hostname")
        .map(|nombre| nombre.trim().to_owned())
        .ok()
        .filter(|nombre| !nombre.is_empty())
        .unwrap_or_else(|| "desconocida".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn los_cinco_tramos_suman_el_total_del_fotograma() {
        // Es la propiedad que hace que el desglose sirva para atribuir: si los tramos no suman, el
        // veredicto tendría que explicar un hueco, y quien lo lea no sabría a qué culpar.
        //
        // **El quinto tramo es el panel**, y entró aquí en cuanto empezó a pintarse: un trabajo que
        // ocurre dentro del fotograma y no se declara se lo lleva el resto, que es precisamente el
        // número que decide si el framework de interfaz aguanta.
        let mut medida = Medida::default();
        let mut resto = RestoDelFotograma::default();
        let ahora = Instant::now();

        medida.anotar_vuelta(Tramos::de_una_vuelta(
            Some(ahora - ms(100)),
            ahora,
            ms(2),
            ms(3),
            ms(1),
        ));
        let cronometrado = medida
            .cerrar_fotograma(ms(10))
            .expect("había tramos pendientes que cerrar");
        resto.anotar(ms(10), cronometrado);

        assert_eq!(medida.esperar.media(), ms(100));
        assert_eq!(medida.interpretar.media(), ms(2));
        assert_eq!(medida.encolar.media(), ms(3));
        assert_eq!(medida.panel.media(), ms(1));
        assert_eq!(resto.serie.media(), ms(4), "10 menos 2, 3 y 1");
        assert_eq!(medida.vuelta.media(), ms(110), "la espera más el fotograma");
    }

    /// **Lo que el panel le quita al resto, y por qué esta prueba existe aparte.** La de arriba pasaría
    /// igual sumando el tramo del panel dentro de `encolar`: los dos se descuentan y el resto sale el
    /// mismo. Lo que se rompería es la *etiqueta* —«encolando el pintado» incluyendo leer un fichero de
    /// disco—, y una etiqueta cambiada es el error que ya invalidó una medida entera en este proyecto.
    #[test]
    fn el_tramo_del_panel_no_se_le_suma_al_del_pintado() {
        let mut medida = Medida::default();
        let ahora = Instant::now();

        medida.anotar_vuelta(Tramos::de_una_vuelta(
            Some(ahora),
            ahora,
            ms(2),
            ms(3),
            ms(4),
        ));
        medida.cerrar_fotograma(ms(10));

        assert_eq!(
            medida.encolar.media(),
            ms(3),
            "el coste del panel se coló en el tramo del pintado"
        );
        assert_eq!(medida.panel.media(), ms(4));
    }

    #[test]
    fn el_resto_descuenta_lo_cronometrado_por_todas_las_divisiones() {
        // La razón de que el resto viva en la ventana: el consumo de CPU llega una sola vez por
        // pasada. Descontándole solo los tramos de una división, la otra se contaría como resto —y el
        // resto es justo el número que dice si el pintado aguanta.
        let mut izquierda = Medida::default();
        let mut derecha = Medida::default();
        let mut resto = RestoDelFotograma::default();
        let ahora = Instant::now();

        izquierda.anotar_vuelta(Tramos::de_una_vuelta(
            Some(ahora),
            ahora,
            ms(2),
            ms(1),
            Duration::ZERO,
        ));
        derecha.anotar_vuelta(Tramos::de_una_vuelta(
            Some(ahora),
            ahora,
            ms(3),
            ms(1),
            Duration::ZERO,
        ));

        let cronometrado: Duration = [
            izquierda.cerrar_fotograma(ms(10)),
            derecha.cerrar_fotograma(ms(10)),
        ]
        .into_iter()
        .flatten()
        .sum();

        resto.anotar(ms(10), cronometrado);

        assert_eq!(cronometrado, ms(7), "2+1 de una y 3+1 de la otra");
        assert_eq!(resto.serie.media(), ms(3), "10 menos las dos divisiones");
    }

    #[test]
    fn un_fotograma_sin_salida_no_cuenta_como_vuelta_rapidisima() {
        // Sin este descarte, cada fotograma en reposo metería un cero en la serie y la media diría
        // que la aplicación responde al instante justamente por no haber hecho nada.
        let mut medida = Medida::default();

        medida.anotar_vuelta(Tramos::de_una_vuelta(
            None,
            Instant::now(),
            ms(2),
            ms(3),
            ms(1),
        ));
        medida.cerrar_fotograma(ms(10));

        assert!(!medida.vuelta.hay(), "se anotó una vuelta que no ocurrió");
        assert!(!medida.esperar.hay(), "se anotó una espera que no ocurrió");
        assert!(!medida.panel.hay(), "tampoco el tramo del panel");
    }

    #[test]
    fn una_consola_sin_salida_declara_igualmente_lo_que_costo_pintarla() {
        // **El fallo que este test existe para cazar es de atribución, y era invisible.** Con dos
        // consolas, el caso normal es una recibiendo salida y la otra quieta en su prompt. La quieta se
        // pinta igual cada fotograma, así que su encolado es trabajo real; si no lo declarase, la
        // ventana no lo restaría del total y ese tiempo saldría publicado como «el resto del fotograma»
        // — el número que decide si el framework de interfaz aguanta varias consolas.
        let mut medida = Medida::default();
        let mut resto = RestoDelFotograma::default();

        // Y su panel también se mira cada fotograma, aunque su productor esté callado: el `ms(1)` es
        // lo que cuesta preguntar la fecha del fichero, que se paga igual que el pintado.
        medida.anotar_vuelta(Tramos::de_una_vuelta(
            None,
            Instant::now(),
            ms(0),
            ms(4),
            ms(1),
        ));
        let cronometrado = medida
            .cerrar_fotograma(ms(10))
            .expect("una consola pintada tiene que declarar lo que costó pintarla");
        resto.anotar(ms(10), cronometrado);

        assert_eq!(
            cronometrado,
            ms(5),
            "el encolado de la consola quieta más el vistazo a su panel"
        );
        assert_eq!(
            resto.serie.media(),
            ms(5),
            "el resto se quedó con el pintado de la consola quieta"
        );
    }

    #[test]
    fn un_total_menor_que_sus_tramos_no_produce_un_negativo() {
        // Los tramos y el total vienen de fotogramas contiguos y de relojes distintos, así que el
        // total puede quedarse corto. Antes de restar sin más, un resto negativo saldría publicado
        // como un número enorme y con toda la pinta de ser real.
        let mut resto = RestoDelFotograma::default();

        resto.anotar(ms(1), ms(16));

        assert_eq!(resto.serie.media(), Duration::ZERO);
    }

    #[test]
    fn el_95_por_ciento_no_lo_marca_el_peor_de_todos() {
        // Una sola pausa del escritorio no puede decidir el número que se publica.
        let mut serie = Serie::default();
        for _ in 0..99 {
            serie.anotar(ms(1));
        }
        serie.anotar(ms(500));

        assert_eq!(serie.p95(), ms(1), "un valor suelto se llevó el percentil");
    }

    #[test]
    fn la_cuenta_total_sobrevive_al_tope_de_muestras() {
        // El informe dice cuántas medidas hubo, no cuántas se guardaron: con el tope, las dos cifras
        // dejan de coincidir y publicar la segunda haría parecer corta una sesión larga.
        let mut serie = Serie::default();
        for _ in 0..(MUESTRAS + 10) {
            serie.anotar(ms(1));
        }

        assert_eq!(serie.muestras.len(), MUESTRAS);
        assert_eq!(serie.total, MUESTRAS + 10);
    }
}
