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
//! 4. **El panel de estado** — mirar si su fichero cambió, leerlo e interpretarlo cuando cambia,
//!    repartir cuánto ancho se lleva, **atender el arrastre de su frontera** y encolar su pintado. Va
//!    aparte del tramo anterior aunque los dos sean trabajo del mismo fotograma: **agruparlos daría la
//!    misma resta con una etiqueta falsa**, y aquí eso ya costó una medida entera.
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
    /// Lo que costó mirar si el `panel.json` de esta consola cambió, leerlo e interpretarlo si
    /// cambió, repartir el ancho, **atender la frontera del panel** y **pintarlo**.
    ///
    /// **Las seis cosas, y decirlo entero importa.** El número lo componen **dos tramos disjuntos** de
    /// `crate::Window::paint_pane`, sumados: uno antes de pintar la consola —mirar la fecha, leer si
    /// cambió, repartir el ancho, atender el arrastre de su frontera y volver a repartir si el ancho
    /// cambió— y otro después —pintar el panel—. Lo que queda **en medio**, que es
    /// encolar el pintado de la consola, tiene su propio campo y no entra aquí. Describir el conjunto
    /// como «leer» dejaría el pintado del panel sin dueño declarado, y un tramo sin dueño se lo lleva
    /// el que se calcula restando — que aquí es el framework de interfaz, o sea justo la cifra sobre
    /// la que D3 tiene que decidir.
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

    /// Junta los tramos de **otra pasada del mismo fotograma**.
    ///
    /// egui puede repetir la pasada sobre la misma entrada, y `eframe` llama a las dos mitades de la
    /// aplicación dentro del cierre que se repite (`epi_integration.rs:288-303`), así que en un
    /// fotograma repetido se vuelve a bombear y a pintar. **Los dos trabajos ocurrieron**, y los dos hay
    /// que devolverlos a la resta del fotograma: sumarlos es lo que dice la verdad.
    ///
    /// **La espera se queda con la primera que hubo, y no se suman.** La espera no es trabajo de la
    /// aplicación —es cuánto llevaba el byte más antiguo en la cola—, así que sumar dos esperas daría un
    /// número que nadie esperó. La de la primera pasada es la del lote que de verdad esperó; la segunda
    /// recoge una cola ya vacía.
    /// Lo que hay que devolverle a la resta del fotograma: todo lo que la aplicación hizo de verdad.
    ///
    /// La espera no entra, y no es un olvido: **no es trabajo**, es cuánto llevaba el byte en la cola.
    fn declarable(&self) -> Duration {
        self.interpretado + self.encolado + self.panel
    }

    fn y_ademas(self, otra: Self) -> Self {
        Self {
            esperado: self.esperado.or(otra.esperado),
            interpretado: self.interpretado + otra.interpretado,
            encolado: self.encolado + otra.encolado,
            panel: self.panel + otra.panel,
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
    /// **Codificar** un gesto de ratón y escribirlo en el programa.
    ///
    /// **La etiqueta dice exactamente lo que el reloj abarca, y eso costó una corrección.** La primera
    /// versión lo publicaba como «del fotograma que ve el gesto al byte», que es lo que mide la serie de
    /// la tecla; este reloj empieza **mucho después** —el recorrido del árbol, el reparto del puntero y
    /// resolver la celda quedan fuera, dentro del reloj que ya cronometra atender el ratón entero— así
    /// que bajo aquella etiqueta publicaba un tramo estrictamente más corto que el que nombraba. **No es
    /// comparable con la ida de la tecla**, y por eso no comparten nombre.
    ///
    /// **Serie propia y no la de la tecla** también por lo que mide: un arrastre mete **decenas de
    /// muestras por gesto** —una por celda recorrida— contra la única que mete una tecla, así que
    /// promediarlas juntas dejaría la cifra que D3 publica describiendo sobre todo al ratón.
    raton: Serie,
    esperar: Serie,
    interpretar: Serie,
    encolar: Serie,
    /// Mirar la fecha del `panel.json`, leerlo e interpretarlo cuando ha cambiado, repartir el ancho,
    /// **atender el arrastre de su frontera** y **pintarlo**.
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

    /// Anota lo que ha tardado codificar un gesto de ratón y escribirlo en el programa.
    pub fn anotar_ida_del_raton(&mut self, cuanto: Duration) {
        self.raton.anotar(cuanto);
    }

    /// Anota los tramos de este fotograma. Quedan pendientes hasta que se sepa lo que costó entero.
    ///
    /// **Se guarda siempre, también sin salida.** Lo que decide si hubo vuelta es que la espera sea
    /// `Some`; el trabajo de pintar ocurrió en los dos casos y la ventana tiene que poder descontarlo.
    ///
    /// **Se SUMA a lo que ya hubiera pendiente, y esto arregla un sesgo que estuvo vivo.** Antes
    /// sustituía, y el fotograma con la pasada repetida —egui la repite cuando algo pide recomponerse—
    /// llegaba aquí dos veces: la segunda anotación, con lo interpretado a casi cero porque la cola ya
    /// estaba vacía, **machacaba la buena**. Lo que la consola había interpretado de verdad se publicaba
    /// como coste del framework de interfaz, que es la cifra con la que D3 decide. Medido antes del
    /// arreglo: **50 ns declarados donde interpretar había costado 20,158 µs**. Ver [`Tramos::y_ademas`]
    /// por qué la espera no se suma.
    ///
    /// **Sumar solo es correcto porque el fotograma se cierra una vez, y las dos cosas van juntas.** Si
    /// el cierre ocurriera por pasada, esto acumularía tramos de la primera pasada dentro del total de la
    /// segunda; y si el cierre fuera por fotograma pero esto sustituyera, se perderían los de la primera.
    /// Lo que hace que encaje: en la mitad lógica del fotograma **N** se cierra con los tramos del
    /// **N-1** y el coste del **N-1** —`eframe` publica el total con un fotograma de retraso—, así que lo
    /// que aquí se suma son siempre las pasadas **de un mismo fotograma**. Quien toque una de las dos
    /// piezas tiene que mirar la otra.
    pub fn anotar_vuelta(&mut self, tramos: Tramos) {
        self.pendiente = Some(match self.pendiente.take() {
            Some(ya_habia) => ya_habia.y_ademas(tramos),
            None => tramos,
        });
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

        Some(tramos.declarable())
    }

    /// Se lleva lo que quedaba pendiente **sin anotar ninguna media**, y devuelve lo que hay que restar.
    ///
    /// **Es para la división que se va con su pestaña.** Sus tramos ocurrieron y hay que declararlos, o el
    /// resto del fotograma se queda con su trabajo. Pero sus medias no las va a publicar nadie —el informe
    /// recorre las pestañas vivas—, y cerrarla por el camino normal exigiría inventarse un coste de
    /// fotograma para la serie de la vuelta: con un cero, una vuelta artificialmente corta; con el de
    /// verdad, una muestra de un fotograma que esta consola ya no va a terminar. Lo honesto es no anotar
    /// nada y quedarse solo con lo que la resta necesita.
    pub fn llevarse_lo_pendiente(&mut self) -> Duration {
        self.pendiente
            .take()
            .map_or(Duration::ZERO, |tramos| tramos.declarable())
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

    /// Si alguna de las cuatro medias tiene muestras.
    ///
    /// Existe por lo mismo que [`Medida::panel_anotado`]: una prueba puede afirmar sobre el camino
    /// real que una consola **que no se pintó** no ensucia ninguna media. Comprobarlo llamando a mano
    /// a la anotación demuestra que la anotación funciona, no que quien la llama le pase lo correcto.
    #[cfg(test)]
    pub fn hay_medias(&self) -> bool {
        self.esperar.hay() || self.interpretar.hay() || self.encolar.hay() || self.panel.hay()
    }

    /// Si esta división llegó a medir algo, para no publicar el informe de una consola en blanco.
    ///
    /// **El ratón cuenta.** Una tanda que solo inyecta gestos y no teclea ni recibe salida tiene sus
    /// muestras aquí y en ningún otro sitio; sin esta condición su informe saldría en blanco y la
    /// corrida se leería como que el gesto no costó nada.
    pub fn hay_algo(&self) -> bool {
        self.ida.hay() || self.raton.hay() || self.vuelta.hay()
    }

    /// Publica los números **de esta división** al destino que se le dé.
    ///
    /// Se publica al cerrar y no durante: la ventana es el terminal, así que no hay sitio dentro
    /// donde enseñar esto sin taparle la pantalla al programa.
    ///
    /// **El destino se recibe, y no es una comodidad: es lo que hace la prueba honesta.** La propiedad
    /// que hay que atar es que el texto publicado **mencione que el tramo del panel incluye pintarlo**,
    /// porque de eso depende que quien copie el número a un documento no se lo atribuya a otro. Con un
    /// `println!` dentro no hay forma de leer lo que sale, y la alternativa —afirmar sobre la constante—
    /// demostraría que la constante está bien redactada, no que alguien la imprima.
    ///
    /// **Y por eso esto no tiene envoltorio.** La primera versión dejaba un `publicar_tramos()` sin
    /// argumentos que llamaba a esta función con la salida estándar, y la prueba entraba por aquí:
    /// vaciar el envoltorio dejaba la suite **verde**, que es la novena vez que este proyecto ata al
    /// ayudante en vez de al camino real. Ahora la puerta de producción y la que se prueba son la
    /// misma, y lo único que aporta quien llama es a dónde escribir.
    pub fn publicar_tramos(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if self.ida.hay() {
            writeln!(
                a,
                "\ntecla en el fotograma → byte en el programa   ({} pulsaciones)",
                self.ida.total
            )?;
            writeln!(a, "  {}", self.ida)?;
            writeln!(
                a,
                "  no incluye el trecho de la pulsación al fotograma: no es medible desde aquí"
            )?;
        }

        if self.raton.hay() {
            writeln!(
                a,
                "\ncodificar el gesto de ratón → byte en el programa   ({} informes de ratón)",
                self.raton.total
            )?;
            writeln!(a, "  {}", self.raton)?;
            writeln!(
                a,
                "  un arrastre mete una muestra por celda recorrida, no una por gesto"
            )?;
            writeln!(
                a,
                "  NO abarca la pasada entera como el tramo de la tecla: repartir el puntero y"
            )?;
            writeln!(
                a,
                "  resolver la celda quedan fuera, dentro del reloj de atender el ratón; los dos"
            )?;
            writeln!(a, "  números no se comparan")?;
        }

        if self.vuelta.hay() {
            writeln!(
                a,
                "\nbyte disponible → fotograma pagado   ({} fotogramas con salida)",
                self.vuelta.total
            )?;
            writeln!(a, "  {}", self.vuelta)?;
            writeln!(a, "  esperando en la cola:   {}", self.esperar)?;
            writeln!(a, "  interpretando:          {}", self.interpretar)?;
            writeln!(a, "  encolando el pintado:   {}", self.encolar)?;
            writeln!(a, "  el panel de estado:     {}", self.panel)?;
            // **La media de este tramo mezcla dos poblaciones y hay que decirlo.** Casi todos los
            // fotogramas solo miran la fecha del fichero y pintan lo ya maquetado; los pocos en que el
            // productor ha publicado cuestan además leer, interpretar y rehacer la maqueta, que es
            // tres órdenes de magnitud más. La media sale pegada al caso barato, así que el número que
            // describe el caso caro es el 95, no ella.
            //
            // **Y el tramo incluye pintar**, no solo leer: el reloj va desde antes de mirar la fecha
            // hasta después de pintar la última fila. Decirlo aquí es lo que impide que quien copie
            // este número a un documento se lo atribuya a otro.
            writeln!(a, "{EXPLICACION_DEL_PANEL}")?;
            writeln!(
                a,
                "  incluye el fotograma entero, también el pintado de las otras consolas: un\n  \
                 fotograma se presenta indivisible. Por eso dos de estos números NO se suman"
            )?;
        }

        Ok(())
    }
}

/// Lo que el informe dice debajo de la línea del tramo del panel.
///
/// **Las dos frases están por razones distintas y ninguna es adorno.** La primera avisa de que la
/// media y el 95 describen poblaciones distintas, así que quien cite «la media» estará citando el
/// caso en que no hubo publicación. La segunda dice **qué abarca el tramo**, y existe porque durante
/// todo el hito F1.6 este número se describió como «leer el panel» cuando el reloj incluye pintarlo:
/// un tramo mal nombrado acaba atribuido a quien no lo hizo, y aquí el que se lleva lo que nadie
/// declara es el framework de interfaz, que es justo la cifra que decide D3.
const EXPLICACION_DEL_PANEL: &str = "\
    \x20   mezcla mirar la fecha y pintar lo ya maquetado (casi todos los fotogramas) con\n\
    \x20   leer, interpretar y rehacer la maqueta (los de publicación): la media describe\n\
    \x20   el primero, el 95 % el segundo\n\
    \x20   son dos relojes, uno antes y otro después del pintado de la consola —que queda\n\
    \x20   fuera—: este tramo INCLUYE pintar el panel, no solo leerlo\n\
    \x20   y también atender el arrastre de su frontera y repartir el ancho otra vez si\n\
    \x20   cambió: el gesto de estrechar el panel se paga aquí, no en el resto del fotograma";

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

    /// Lo que se ha publicado como resto, de media.
    ///
    /// **Solo lo usan las pruebas, y por eso está detrás de su bandera.** Es lo que permite afirmar que
    /// un trabajo declarado **se descuenta de verdad**: sin esto, una prueba solo puede comprobar que
    /// alguien apuntó el número en un campo, que es atar al ayudante y no al camino — y ese hueco ya dejó
    /// pasar una mutación en este mismo trabajo.
    #[cfg(test)]
    pub fn media(&self) -> Duration {
        self.serie.media()
    }

    /// Cuántas veces se ha anotado un resto, o sea **cuántos fotogramas se han cerrado**.
    ///
    /// **Solo lo usan las pruebas, y por eso está detrás de su bandera.** Es el único observable que
    /// distingue un fotograma cerrado una vez de uno cerrado dos: las medias no lo delatan —dos cierres
    /// del mismo fotograma anotan un valor parecido— y el informe tampoco. Es lo que hace falta para
    /// atar la guarda que evita el doble cierre.
    #[cfg(test)]
    pub fn fotogramas_cerrados(&self) -> usize {
        self.serie.total
    }

    pub fn publicar(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self.serie.hay() {
            return Ok(());
        }
        writeln!(
            a,
            "\nel resto del fotograma — de la ventana, común a todas las consolas   ({} fotogramas)",
            self.serie.total
        )?;
        writeln!(a, "  {}", self.serie)?;
        writeln!(
            a,
            "  es teselar, dibujar y el armazón de egui: desde dentro no se pueden separar, y\n  \
             llamarlo «teselado» sería publicar como medido algo que no lo está"
        )?;
        writeln!(
            a,
            "  ya tiene descontado lo que cronometraron todas las consolas de todas las pestañas:\n  \
             también el pintado de una que no recibiera salida, y lo que interpretó una que no se\n  \
             llegó a pintar por estar en una pestaña de fondo"
        )
    }
}

/// Los dos tramos que **la ventana** paga por guardar su disposición: mirar si cambió, y escribirla.
///
/// **Vive aquí y no en la división por lo mismo que el resto del fotograma**: son de la ventana entera, no
/// de una consola, así que no tienen el vehículo que llevan los tramos de una división.
///
/// **Y hacen falta las DOS cosas, que es el hallazgo que costó una revisión entera.** Una serie **solo
/// guarda muestras para imprimirlas**: no descuenta nada. El resto del fotograma se calcula restándole al
/// coste todo lo que se cronometró, y a ese saco hay que entrar aparte. Con serie y sin resta, estos dos
/// tramos se publicarían con su fila **y a la vez** dentro de la cifra que D3 usa para juzgar al framework.
#[derive(Default)]
pub struct Guardado {
    espejo: Serie,
    escritura: Serie,
    /// Cuántas veces se ha escrito el fichero de disposición.
    ///
    /// **Lo publica el informe y no lo puede saber el guion**, que es la misma partición que el proyecto ya
    /// usa con los gestos inyectados: este número vive dentro del proceso. El guion invalida la corrida si es
    /// cero, porque una tanda que dice medir el guardado y no guarda nada publicaría el coste de un camino
    /// que no se recorrió.
    escrituras: usize,
}

impl Guardado {
    /// Anota lo que costó comparar el espejo en **un fotograma entero**.
    ///
    /// Quien llama acumula por pasada y anota **una vez al cerrar**: egui repite la pasada de interfaz, y
    /// anotar por pasada haría que la media describiera «por pasada» en vez de «por fotograma» — el inverso
    /// exacto del sesgo que este instrumento ya se comió una vez.
    pub fn anotar_espejo(&mut self, cuanto: Duration) {
        self.espejo.anotar(cuanto);
    }

    /// Anota lo que costó **un intento** de escritura: desde resolver la ruta hasta que el renombrado
    /// vuelve, o hasta que el temporal queda borrado si falló.
    ///
    /// **El tiempo se anota siempre y la cuenta solo cuando llegó al disco**, y la distinción no es
    /// quisquillosa: el trabajo ocurrió dentro del fotograma en los dos casos y hay que descontarlo, pero
    /// publicar los intentos fallidos como escrituras diría que la disposición está guardada cuando no lo
    /// está. Un disco lleno publicaría «mil escrituras» sin haber escrito ninguna.
    pub fn anotar_escritura(&mut self, cuanto: Duration, llego_al_disco: bool) {
        self.escritura.anotar(cuanto);
        if llego_al_disco {
            self.escrituras += 1;
        }
    }

    /// Cuántas veces se escribió **de verdad**, para que el informe lo declare y el guion pueda invalidar
    /// una corrida que dice medir el guardado y no guardó nada.
    pub fn escrituras(&self) -> usize {
        self.escrituras
    }

    /// Cuántas veces se **intentó** escribir en el disco, para pruebas.
    ///
    /// Existe porque el contador de arriba solo cuenta las que llegaron, así que sin esto una prueba no
    /// puede distinguir «se reintentó y volvió a fallar» de «no se reintentó» — que es exactamente el hueco
    /// que una revisión encontró en la prueba del reintento.
    #[cfg(test)]
    pub fn intentos(&self) -> usize {
        self.escritura.total
    }

    /// Lo que llevan anotado los dos tramos, **solo para pruebas**: es lo que permite afirmar que el camino
    /// real los declara, y no solo que la anotación funciona cuando se la llama a mano.
    #[cfg(test)]
    pub fn hay_algo(&self) -> bool {
        self.espejo.hay() || self.escritura.hay()
    }

    pub fn publicar(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if self.espejo.hay() {
            writeln!(
                a,
                "\nmirar si la disposición cambió — de la ventana   ({} fotogramas con pasada)",
                self.espejo.total
            )?;
            writeln!(a, "  {}", self.espejo)?;
            writeln!(
                a,
                "  recorre el árbol de todas las pestañas y construye su espejo para compararlo: se paga en\n                   todos los fotogramas que pintan, y por eso está descontado del resto. Incluye\n                   también comparar contra lo último escrito al vencer el plazo cuando no hay nada\n                   nuevo que guardar: es el mismo trabajo"
            )?;
        }

        if self.escritura.hay() {
            writeln!(
                a,
                "\nescribir la disposición en disco   ({} escrituras de {} intentos)",
                self.escrituras, self.escritura.total
            )?;
            writeln!(a, "  {}", self.escritura)?;
            writeln!(
                a,
                "  abarca copiar la disposición, VALIDARLA, serializarla, resolver la ruta, crear el directorio\n                   la primera vez, escribir el temporal y renombrarlo encima; y si algo falla, borrar el\n                   temporal y decir la línea del fallo. Solo ocurre cuando vence el plazo, no en cada\n                   fotograma: un intento que no llega al disco es casi todo validación"
            )?;
        }

        Ok(())
    }
}

/// Lo que **la ventana** paga por buscar en la máquina las fuentes que le faltan.
///
/// **Vive aquí y no en la división por la misma razón que el guardado**: las caras se le registran al
/// motor de texto, que es de la ventana entera, no de una consola.
///
/// **Y hace falta la resta, no solo la serie.** El resto del fotograma se calcula restándole al coste todo
/// lo que se cronometró: una serie sola imprimiría su fila **y a la vez** dejaría el número dentro de la
/// cifra con la que **D3** juzga al framework. Es el hallazgo que costó una revisión entera en el tramo
/// del guardado, y aquí muerde igual — este trabajo son **milisegundos**, no microsegundos.
#[derive(Default)]
pub struct Descubrimiento {
    busqueda: Serie,
    /// Cuántos caracteres se han buscado en la máquina, en total.
    ///
    /// **Lo publica el informe y no lo puede saber el guion**, igual que las escrituras de la disposición:
    /// este número vive dentro del proceso. Una tanda que diga medir el descubrimiento y no busque nada
    /// publicaría el coste de un camino que no se recorrió.
    buscados: usize,
    /// Cuántas caras del sistema se le han dado al motor de texto.
    ///
    /// **Va aparte de los buscados y no es redundante**: un carácter que ninguna fuente cubre se busca —y
    /// se paga entero— **sin registrar nada**. Con un solo número, una máquina sin fuentes y una con todo
    /// instalado publicarían informes indistinguibles.
    registradas: usize,
}

impl Descubrimiento {
    /// Anota lo que costó **una vuelta** de descubrimiento: buscar las caras de lo apuntado y
    /// registrarlas.
    pub fn anotar(&mut self, cuanto: Duration, buscados: usize, registradas: usize) {
        self.busqueda.anotar(cuanto);
        self.buscados += buscados;
        self.registradas += registradas;
    }

    /// Si se llegó a buscar algo, **para que una prueba pueda afirmar que el camino real lo anota** y no
    /// solo que la anotación funciona cuando se la llama a mano.
    ///
    /// Los dos contadores los lee el informe directamente; esto es lo único que hacía falta de fuera.
    #[cfg(test)]
    pub fn hay_algo(&self) -> bool {
        self.busqueda.hay()
    }

    pub fn publicar(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self.busqueda.hay() {
            return Ok(());
        }
        writeln!(
            a,
            "\nbuscar en la máquina una fuente que falta — de la ventana   ({} vueltas · {} caracteres buscados · {} caras registradas)",
            self.busqueda.total, self.buscados, self.registradas
        )?;
        writeln!(a, "  {}", self.busqueda)?;
        writeln!(
            a,
            "  NO ocurre en cada fotograma: solo cuando aparece un carácter que ni las caras registradas ni\n                   las de fábrica cubren, y por eso la media de aquí no describe un fotograma corriente.\n                   Abarca preguntarle al motor si el carácter ya se ve —maquetarlo y comparar su dibujo con\n                   el del cuadro—, enumerar las fuentes del sistema la primera vez, abrir las candidatas\n                   hasta dar con la que lo trae, y decirle al motor que se quede con ella. Un carácter que\n                   NADIE cubre se paga igual y no registra nada: por eso los dos contadores van separados"
        )?;
        writeln!(
            a,
            "  LO QUE ESTE TRAMO NO MIDE, dicho aquí porque callarlo sería peor: rehacer el ATLAS de glifos.\n                   Entregarle la configuración al motor solo la guarda; el atlas se reconstruye al empezar\n                   la pasada SIGUIENTE, antes de que ninguna línea de este proyecto tome control, así que no\n                   hay dónde poner el cronómetro. Medido aparte: unos 4,5 ms por registro con una cara CJK\n                   de 20 MB, y caen en «el resto del fotograma». Ocurre tantas veces como diga la columna\n                   de caras registradas, no una por fotograma"
        )?;
        Ok(())
    }
}

/// Lo que cuesta **rasterizar un glifo de color**, que es lo único de este proyecto que hace trabajo caro
/// dentro del pintado.
///
/// **Se llama por lo que mide y no por su tema**: en este proyecto `Color` es un color —lo son `Color32`,
/// `TermColor` y la tabla del tema—, y un tipo con ese nombre que en realidad mide microsegundos se lee
/// mal en cada sitio donde aparece.
///
/// **Va aparte de la búsqueda de fuentes porque ocurre en otro sitio del fotograma, y eso cambia quién lo
/// paga.** Registrar una cara rehace el atlas del motor, así que se hace **fuera** del pintado y su tramo
/// se resta del resto; rasterizar un emoji no invalida nada, así que se hace donde hace falta — dentro de
/// «encolando el pintado». Sin restarlo de ahí, ese tramo dejaría de significar lo que su nombre dice, y
/// este proyecto ya tiene escrito que un número mal etiquetado es peor que ningún número.
#[derive(Default)]
pub struct Rasterizado {
    rasterizado: Serie,
    /// Cuántos glifos de color se han rasterizado de verdad.
    ///
    /// **Lo publica el informe y no lo puede saber el guion**, igual que los caracteres buscados: una
    /// tanda que diga medir el color y no rasterice ninguno estaría publicando el coste de un camino que
    /// no se recorrió.
    rasterizados: usize,
    /// Cuántas veces se pidió un glifo de color que la máquina no pinta así.
    ///
    /// **Va aparte y no es redundante**: un carácter cuya presentación pide color pero que ninguna fuente
    /// instalada trae a color **se paga entero** y no devuelve nada. Con un solo número, una máquina con
    /// fuente de emoji y otra sin ella publicarían informes indistinguibles.
    fallidos: usize,
}

impl Rasterizado {
    /// Anota lo que costó el camino del color **en el pintado de una división**.
    pub fn anotar(&mut self, cuanto: Duration, rasterizados: usize, fallidos: usize) {
        self.rasterizado.anotar(cuanto);
        self.rasterizados += rasterizados;
        self.fallidos += fallidos;
    }

    pub fn publicar(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self.rasterizado.hay() {
            return Ok(());
        }
        writeln!(
            a,
            "\nrasterizar un glifo de color — de la ventana   ({} fotogramas con trabajo · {} glifos rasterizados | {} sin cara de color)",
            self.rasterizado.total, self.rasterizados, self.fallidos
        )?;
        writeln!(a, "  {}", self.rasterizado)?;
        writeln!(
            a,
            "  NO ocurre en cada fotograma: solo la primera vez que un emoji aparece a una altura de celda\n                   dada, así que la media de aquí no describe un fotograma corriente. Abarca buscar en las\n                   familias de emoji del sistema la que lo trae a color, decodificar su mapa de bits —que en\n                   Noto Color Emoji es un PNG indexado de 136x128— reducirlo al tamaño de la celda y subir la\n                   textura. Un carácter que NINGUNA fuente pinta a color se paga igual y no devuelve nada:\n                   por eso los dos contadores van separados"
        )?;
        writeln!(
            a,
            "  ESTÁ RESTADO de «encolando el pintado» de la consola que lo pagó, no sumado encima: corre\n                   DENTRO de ese tramo, al revés que buscar una fuente. Sin la resta, el tramo que dice medir\n                   lo que cuesta encolar mediría además esto"
        )?;
        Ok(())
    }
}

/// Lo grande que es la ventana, en la unidad en la que se le pide que abra.
///
/// **Dos campos con nombre y no una pareja de números**, por lo mismo que los tramos de la tabla llevan
/// el suyo: dos `f32` seguidos se pueden intercambiar sin que el compilador diga nada, y el informe
/// publicaría una ventana con el ancho y el alto cambiados sin que quien lo lea pueda notarlo.
pub struct EnPuntos {
    pub ancho: f32,
    pub alto: f32,
}

/// Lo que ha costado **recolocar el texto** de las consolas a lo largo de la sesión.
///
/// **Es la fuga que el historial abre, y por eso se declara desde el mismo hito que lo enciende.**
/// Recolocar corre en la mitad lógica del fotograma —al aplicar el tamaño que midió el pase anterior—
/// y hasta ahora **no lo cronometraba nadie**: con el historial apagado costaba 7 µs y se perdía en el
/// ruido, así que la fuga era invisible. Con historial recorre **todas** las líneas guardadas: 984 µs
/// por consola medidos con contenido que envuelve (**D21**), y una frontera mueve **dos**.
///
/// Sin esto, ese trabajo se publicaría como coste del framework — la cifra con la que **D3** juzga a
/// egui.
#[derive(Clone, Copy, Default)]
pub struct Reflujo {
    /// En cuántos fotogramas **se llegó a recolocar de verdad**.
    ///
    /// **Casi todos los fotogramas no recolocan nada**: solo los que cruzan un borde de celda mientras
    /// se arrastra una frontera, o los que siguen a un cambio de tamaño de la ventana. Sin este número,
    /// una media repartida entre miles de fotogramas quietos publica «recolocar no cuesta nada», que es
    /// exactamente el disfraz que el resto del informe evita.
    pub fotogramas: usize,
    /// Lo que costaron todos ellos juntos.
    pub total: Duration,
}

impl Reflujo {
    /// Anota lo que costó recolocar en un fotograma. **Solo se llama cuando hubo recolocación**: un
    /// cero apuntado como muestra hunde la media y hace decir a este tramo lo contrario de lo que vale.
    pub fn anotar(&mut self, cuanto: Duration) {
        self.fotogramas += 1;
        self.total += cuanto;
    }

    /// Lo que costó de media cada recolocación, o nada si no hubo ninguna.
    fn por_vez(&self) -> Option<Duration> {
        self.total.checked_div(self.fotogramas as u32)
    }
}

/// Lo que ha costado pintar la barra de pestañas a lo largo de la sesión.
///
/// **Se declara porque el fotograma se reparte por resta**: lo que nadie nombra se publica como coste
/// del framework, que es la cifra con la que D3 juzga a egui. La barra no es de ninguna consola —no
/// tiene dónde caer salvo aquí— y desde que su ficha dice lo que reclaman las divisiones de cada
/// pestaña, además **recorre**: no declararla sería regalarle ese recorrido al framework.
///
/// **El tramo es la barra entera —acumular y pintar—, y decirlo entero importa.** Declarar solo el
/// recorrido dejaría el pintado de las fichas sin dueño, que es la misma frontera a medias que este
/// proyecto ya pagó una vez.
#[derive(Clone, Copy, Default)]
pub struct Barra {
    /// En cuántos fotogramas se llegó a pintar.
    ///
    /// **Cero no es «no costó nada», es «no hubo barra»** —con una sola pestaña no se pinta—, y son dos
    /// cosas que un total a cero no distingue. Es también lo que hace comparable la tanda: sin este
    /// número, una corrida de una sola pestaña publicaría «la barra no cuesta nada» con el código
    /// quitado.
    pub fotogramas: usize,
    /// Lo que costaron todos ellos juntos.
    pub total: Duration,
    /// Lo más que llegaron a marcarse fichas a la vez.
    ///
    /// **Cero significa que la barra recorrió y no marcó nada.** El recorrido ocurre haya reclamo o no,
    /// así que sin este número una tanda cuyos productores no publicaran el campo publicaría el coste de
    /// la barra **sin la mitad que hay que medir**, y se leería igual que una buena.
    pub fichas_marcadas: usize,
}

impl Barra {
    /// Anota lo que costó pintarla **en un fotograma**, ya sumadas sus pasadas.
    ///
    /// Quien llama es el cierre del fotograma, no el pintado: egui repite la pasada sobre la misma
    /// entrada y las dos pintan de verdad, así que anotar por pasada daría una media por pasada bajo
    /// una etiqueta que dice «por fotograma».
    pub fn anotar(&mut self, cuanto: Duration) {
        self.fotogramas += 1;
        self.total += cuanto;
    }

    /// Se queda con lo más que llegaron a marcarse fichas a la vez.
    ///
    /// **El máximo y no lo último**: en el último fotograma pueden haber caducado todas, y lo que hay
    /// que poder decir es si llegó a marcarse alguna en toda la corrida.
    pub fn vio_marcadas(&mut self, cuantas: usize) {
        self.fichas_marcadas = self.fichas_marcadas.max(cuantas);
    }

    /// Lo que costó de media, o nada si no llegó a pintarse.
    fn por_fotograma(&self) -> Option<Duration> {
        self.total.checked_div(self.fotogramas as u32)
    }
}

/// Lo que costó abrir la ventana con la disposición que había guardada.
///
/// **Se publica fuera de la tabla de tramos y no se resta de ningún fotograma**, y eso es lo que hay que
/// entender antes de leer sus números: restaurar ocurre **entero antes del primer fotograma** —el fichero
/// se lee en `run` y las consolas se abren en el constructor—, así que no contamina el reparto por
/// resta con el que este proyecto juzga a su framework. Es coste de **arranque**, que es otro camino y
/// hasta este hito no lo declaraba nadie.
#[derive(Clone, Copy)]
pub struct Restauracion {
    /// Cuántas consolas salieron del fichero. **Es el número que hace comparable la tanda**: sin él, una
    /// corrida en la que el fichero no llegó a mandar publica la ventana de fábrica bajo etiqueta de
    /// restaurada, que es el mismo disfraz que el informe ya evita con los paneles y con los gestos.
    pub consolas: usize,
    /// Leer el fichero del disco, interpretarlo y validarlo.
    ///
    /// **Es opcional, y eso impide publicar un número que nadie tomó.** Con un `Duration` a secas, una
    /// ventana construida con una persistencia que nunca leyó el disco publicaría `0ns en leer y
    /// validar` — indistinguible de una lectura instantánea de verdad, y exactamente la clase de cifra
    /// que este proyecto prohíbe. Sin medida, el informe lo dice con esas palabras.
    pub leer: Option<Duration>,
    /// Abrir la ventana con lo que el fichero pedía: un `fork` y un `exec` por hoja, **y todo lo que
    /// viene detrás por cada consola** — armar el vigilante de su directorio y describir el árbol
    /// entero para el primer espejo.
    ///
    /// **Va hasta que la ventana está construida, no hasta que los procesos existen**, y esa raya la
    /// movió una revisión: parándola antes, dos trabajos que **crecen con N** quedaban fuera de un
    /// número que dice medir justo eso, y se habrían publicado como coste del framework.
    ///
    /// **Es el que crece con N** y por eso va aparte del de arriba: leer un documento de cuatro hojas y
    /// uno de doscientas cuesta casi lo mismo, y abrirlas no.
    pub abrirlas: Duration,
}

/// Lo que costó mirar la configuración de quien usa el terminal, **y si se llegó a mirar**.
///
/// **Se llama por el acto y no por la cosa**, como sus hermanos: `Restauracion`, `Descubrimiento`,
/// `Rasterizado` nombran el trabajo. Un tipo llamado `Configuracion` dentro del módulo de medida se lee
/// como «la configuración de la medida», que es justo lo que no es.
///
/// **Un tramo de una sola vez, como el de restaurar**: ocurre entero antes del primer fotograma, así que no
/// tiene serie ni media. Lo que hace falta es que **esté declarado**, o sus microsegundos caen dentro del
/// arranque sin nombre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LecturaDeAjustes {
    /// No se miró: es una tanda de medida y el fichero está vetado.
    #[default]
    Vetada,
    /// Se miró y no había nada que aplicar — no hay fichero, o no se pudo usar.
    Sin,
    /// Se aplicó lo que el fichero pedía.
    Con {
        /// Lo que costó leerlo e interpretarlo.
        cuanto: Duration,
        /// Cuántas líneas no se pudieron aplicar.
        avisos: usize,
    },
}

/// Los gestos de arrastre que una tanda de medida se inyectó a sí misma, tal como se declaran.
#[derive(Clone, Copy)]
pub struct Arrastres {
    /// Gestos enteros que se inyectaron: los que llegaron a mandar su evento de soltar.
    pub inyectados: usize,
    /// Cuántas veces el gesto **cambió algo de verdad**: soltados que el árbol aceptó, o fotogramas en que
    /// un arrastre le movió el ancho a un panel.
    ///
    /// **Puede ser MAYOR que los inyectados, y no es un error de cuenta.** Con el gesto del panel, un solo
    /// arrastre cruza varias fronteras de celda y cada cruce cambia el ancho, así que un gesto produce
    /// varios efectos —y con dos divisiones a la vista, los de las dos—. Por eso las dos cifras se publican
    /// **una al lado de la otra y no como una fracción**: la primera dice cuántos gestos ocurrieron, la
    /// segunda cuánto trabajo provocaron. La primera versión las imprimía como «N de M» y publicó
    /// «423 de 211», que es un número imposible de leer bien.
    pub con_efecto: usize,
    /// En cuántos **fotogramas** el gesto dejó texto marcado en alguna consola.
    ///
    /// **Fotogramas y no gestos, y decirlo importa**: un gesto extiende la selección una vez por cada
    /// fotograma de arrastre, así que este número sale **mayor** que el de gestos inyectados. Publicarlo
    /// bajo la palabra «gestos» daba una fila con el tercer número por encima del primero, que es
    /// imposible de leer bien — lo mismo que ya pasó una vez publicando «423 de 211».
    ///
    /// **Las tandas lo leen en los DOS sentidos.** La que mide marcar exige que haya **más de uno por
    /// gesto**: si el gesto entrara por otro sitio —el asidero del panel, el divisor— la corrida se
    /// inyectaría entera y publicaría el coste de otra cosa bajo su etiqueta. Y las que miden **otro**
    /// gesto exigen que sea **cero**: desde **D20** el botón primario a secas dentro de una consola marca
    /// texto, así que un gesto que perdiera su asidero se convertiría en una selección sin que nada más lo
    /// delatara.
    ///
    /// **En la tanda que marca coincide con los efectos, y es correcto**: el efecto de marcar **es** dejar
    /// algo marcado. Los dos se publican igual porque en las demás tandas no coinciden — allí los efectos
    /// son repartos y maquetados, y éste tiene que salir cero.
    pub marcados: usize,
    /// Cuántos **informes de ratón** salieron hacia el programa de dentro durante la corrida.
    ///
    /// **Se lee en los dos sentidos, igual que los marcados.** La tanda que mide el ratón exige que sea
    /// **mayor que cero**: sin ese número, una corrida en la que el programa nunca llegara a pedir el
    /// ratón publicaría los tramos de un gesto que se resolvió como selección, bajo la etiqueta del
    /// ratón. Y las demás exigen que sea **cero**: un informe que se escapara metería una ida al
    /// pseudoterminal dentro de la media de otra tanda.
    pub informados: usize,
    /// Qué gesto se inyectó, tal como se publica. **Va en el informe** porque los tres gestos no cuestan lo
    /// mismo y una tanda no se compara con otra de la otra clase.
    pub gesto: &'static str,
}

/// La cabecera del informe: dónde y cómo se midió — cuántas pestañas había, cuál se veía y con qué
/// rejilla cada consola.
///
/// **Este comentario llevaba el título dos veces y los dos párrafos pegados sin separación**, de antes de
/// este hito; se arregló al pasar por aquí, porque un doc que se presenta dos veces se lee como si
/// describiera dos cosas.
///
/// Las rejillas van una por consola **y con su nombre**, porque el reparto del ancho las deja
/// distintas y ningún tramo se entiende sin saber sobre cuántas celdas se tomó. Es el dato que hace
/// falta para comparar con una medida antigua de una sola consola a pantalla completa: sin él, un
/// «encolar» más bajo se lee como que el framework va más rápido, cuando lo que pasa es que hay la
/// mitad de columnas.
///
/// **Cada pestaña llega con su nombre, no con su posición en la lista.** Al cerrar una, las de su
/// derecha corren un puesto, así que numerar aquí por el orden del recorrido le pondría a una tabla de
/// números el nombre de otra pestaña — y quien lea el informe no tiene forma de notarlo. `visible` es
/// también un nombre, por lo mismo.
///
/// **Y cada división llega con el suyo, por lo mismo y en cuanto se puedan cerrar.** Hoy son dos y
/// fijas, así que el nombre coincide con la posición; publicar ya el nombre es lo que hace que las
/// tandas tomadas antes y después del árbol de divisiones se lean igual.
///
/// **Una lista por pestaña y no un array de dos**: el número de divisiones deja de estar fijado por el
/// tipo en cuanto llega el árbol, y una firma que siga prometiendo dos obligaría a recortar o rellenar
/// justo aquí, que es donde el informe se convierte en el dato que alguien va a creerse.
/// Lo que la sesión entera tiene que contar de sí misma, además de sus tramos por consola.
///
/// **Es un struct con nombres y no cuatro parámetros seguidos**, por lo mismo que [`Tramos`]: cuando
/// esto eran argumentos posicionales, intercambiar dos compilaba sin una queja y publicaba un informe
/// mentiroso. Y son cuatro cosas que crecen —cada hito que declara un tramo nuevo añade el suyo—, así
/// que la firma tenía los días contados.
#[derive(Clone, Copy, Default)]
pub struct DeLaSesion {
    /// Los gestos que una tanda se inyectó, si es que se inyectó alguno.
    pub arrastres: Option<Arrastres>,
    /// De dónde salieron las consolas, si salieron de un fichero.
    pub restauracion: Option<Restauracion>,
    /// Lo que costó leer la configuración, y si se leyó.
    pub configuracion: LecturaDeAjustes,
    /// Lo que costó pintar la barra de pestañas.
    pub barra: Barra,
    /// Lo que costó recolocar el texto al cambiar el ancho de una consola.
    pub reflujo: Reflujo,
}

pub fn cabecera(
    a: &mut dyn std::io::Write,
    ventana: EnPuntos,
    rejillas: &[(usize, Vec<(usize, term_core::Size)>)],
    visible: usize,
    sesion: DeLaSesion,
) -> std::io::Result<()> {
    let DeLaSesion {
        arrastres,
        restauracion,
        configuracion,
        barra,
        reflujo,
    } = sesion;
    let perfil = if cfg!(debug_assertions) {
        "depuración — el número que vale es el de --release"
    } else {
        "release"
    };
    writeln!(a, "\n── latencia ──")?;
    // **Lo grande que era la ventana, y es lo único de la cabecera que se puede volver a pedir.** La
    // rejilla en celdas de cada consola sale de este tamaño y de la métrica de la fuente, así que leerla
    // no dice con qué ventana se tomó la tanda: los anexos anteriores a esta línea se midieron con
    // ventanas cuyo tamaño nadie apuntó, y por eso no se pueden reproducir. Se publica en puntos porque
    // es la unidad en la que se pide.
    writeln!(a, "ventana:      {}×{} puntos", ventana.ancho, ventana.alto)?;
    // **Cuántas pestañas había, y no solo cuáles se pintaron.** Los tramos de abajo se tomaron con
    // todas ellas bombeando, así que una tanda de dos pestañas y otra de una no se pueden comparar sin
    // este dato — y sin él, el informe de las dos se lee como si fuera de una sola y con el fotograma
    // inexplicablemente más caro.
    writeln!(a, "pestañas:     {}", rejillas.len())?;
    for (que_pestaña, rejillas) in rejillas {
        let mirada = if *que_pestaña == visible {
            "se veía"
        } else {
            "de fondo, se bombeaba sin pintarse"
        };
        writeln!(a, "pestaña {que_pestaña}:    {mirada}")?;
        for (cual, (division, rejilla)) in rejillas.iter().enumerate() {
            writeln!(
                a,
                "{:<15} {}×{}{}",
                format!("  división {division}:"),
                rejilla.rows,
                rejilla.cols,
                donde_cae(rejillas.len(), cual)
            )?;
        }
    }
    // **Los gestos que la tanda se inyectó, y solo si pidió alguno.** Dos números, y los dos tienen señal:
    // los **inyectados** dicen que el gesto ocurrió, y los que **surtieron efecto** dicen que llegó a
    // cambiar algo. Hacen falta los dos porque no son lo mismo: el suelo geométrico rechaza el soltado cuando la
    // consola de destino no puede darle su mínimo a las dos, y en una ventana estrecha eso pasa en todos —
    // la tanda inyecta mil gestos y no reordena ni uno. Con un solo número, esa corrida se leería como «el
    // gesto no cuesta nada» en vez de como «no pasó nada», que es el disfraz de declarar cero que este
    // proyecto ya se ha comido una vez.
    //
    // **Los pedidos NO se publican, y eso lo corrigió la revisión.** La primera versión imprimía «pedidos» y
    // decía que la diferencia con los hechos era el aviso; pero el guion pide un número deliberadamente
    // altísimo para que el gesto dure toda la tanda, así que esa diferencia vale lo mismo en una corrida
    // buena que en una rota. Un número que un lector va a intentar interpretar y no puede es peor que no
    // publicarlo.
    //
    // **La línea no sale cuando nadie pidió arrastres**, y eso es deliberado: las tandas del anexo
    // anteriores a este hito se compararían con informes de forma distinta, y aquí lo que se compara son
    // tandas entre sí.
    if let Some(arrastres) = arrastres {
        writeln!(
            a,
            "arrastres:    {} inyectados · {} efectos · {} fotogramas con texto marcado · {} informes al programa ({})",
            arrastres.inyectados,
            arrastres.con_efecto,
            arrastres.marcados,
            arrastres.informados,
            arrastres.gesto
        )?;
    }
    // **De dónde salieron las consolas, y solo cuando salieron del fichero.** Sin esta línea, una tanda
    // que dice medir el arranque restaurado y otra que abrió de fábrica producen informes **idénticos**:
    // la cabecera publica cuántas consolas hay, no quién las pidió. Y con la misma cuenta de consolas
    // ninguna de las guardas del guion salta — que es exactamente el disfraz que este proyecto ya se comió
    // una vez con los paneles y otra con los gestos.
    //
    // **Los dos tiempos van separados a propósito**: leer no crece con el número de consolas y abrirlas sí,
    // así que sumarlos daría un número que no se puede extrapolar a una disposición más grande.
    if let Some(restauracion) = restauracion {
        let leer = match restauracion.leer {
            Some(cuanto) => format!("{cuanto:?}"),
            None => "sin medir".to_string(),
        };
        writeln!(
            a,
            "restauradas:  {} consolas · {leer} en leer y validar · {:?} en abrirlas (antes del primer \
             fotograma, no se resta de ninguno)",
            restauracion.consolas, restauracion.abrirlas
        )?;
    }
    // **Se declara SIEMPRE, se haya leído o no**, y ésa es la mitad que importa. Una corrida que leyó la
    // configuración de quien mide y otra que corrió de fábrica publican rejillas que pueden no tener nada
    // que ver, y sin esta línea sus informes son indistinguibles. Toda tanda de medida tiene el fichero
    // vetado, así que aquí debe leerse siempre «de fábrica»: si alguna vez dice otra cosa, esa corrida no
    // se compara con ninguna.
    match configuracion {
        // **Los tres estados se distinguen EN EL PRIMER SUSTANTIVO, no en el paréntesis**, y eso es lo
        // que hace que el guion de medida pueda leerlos. Con «de fábrica» para el vetado y para el que
        // miró y no encontró nada, un veto roto en una máquina sin fichero —que es **el caso real del
        // banco**, porque el guion vacía el directorio de configuración— se leería como una corrida buena.
        // Lo cazó una revisión.
        LecturaDeAjustes::Vetada => writeln!(
            a,
            "configuración: vetada (no se lee bajo la puerta de medida: el tamaño de letra de quien mide \
             cambiaría la rejilla, y de la rejilla dependen todas las cifras publicadas)"
        )?,
        // **«Se miró y no había» NO es «del usuario», y confundirlos publica un número con la etiqueta
        // cambiada.** Una máquina sin fichero y otra que aplicó un cuerpo de letra distinto tienen rejillas
        // que pueden no tener nada que ver, y con un solo estado para las dos sus informes salían
        // idénticos. Lo cazó una revisión.
        LecturaDeAjustes::Sin => writeln!(
            a,
            "configuración: de fábrica (se miró y no había nada que aplicar)"
        )?,
        LecturaDeAjustes::Con { cuanto, avisos } => writeln!(
            a,
            "configuración: del usuario · {cuanto:?} en leerla · {avisos} línea(s) que no se pudieron \
             aplicar (antes del primer fotograma, no se resta de ninguno)"
        )?,
    }
    // **Solo cuando la hubo.** Con una sola pestaña no hay barra que pintar, y publicar «0 fotogramas»
    // invita a leer que no cuesta nada.
    if let Some(media) = barra.por_fotograma() {
        writeln!(
            a,
            "barra:        {} fotogramas · {:?} en total · {media:?} por fotograma · {} fichas \
             marcadas a la vez como mucho (acumular los reclamos y pintar las fichas; se resta de su \
             fotograma)",
            barra.fotogramas, barra.total, barra.fichas_marcadas
        )?;
    }
    // **Solo cuando hubo.** En una tanda que no redimensiona nada no se recoloca ni una vez, y
    // publicar «0 veces» invita a leer que no cuesta.
    if let Some(cada) = reflujo.por_vez() {
        writeln!(
            a,
            "recolocar:    {} veces · {:?} en total · {cada:?} cada vez (el texto, al cambiar el ancho \
             de una consola; corre en la mitad lógica y se resta de su fotograma)",
            reflujo.fotogramas, reflujo.total
        )?;
    }
    writeln!(a, "renderizador: glow")?;
    writeln!(a, "perfil:       {perfil}")?;
    writeln!(a, "máquina:      {}", maquina())
}

/// Cómo se llama cada consola en el informe, por su sitio en la ventana.
///
/// Un número suelto obliga a adivinar de qué lado es, y el reparto del ancho hace que el lado importe.
pub const NOMBRE_DE_CONSOLA: [&str; 2] = ["izquierda", "derecha"];

/// De qué lado cae una división, **si es que el lado significa algo**.
///
/// **«Izquierda» y «derecha» solo describen algo cuando hay exactamente dos**, así que con cualquier
/// otro número esto no devuelve nada en vez de inventarse un lado. No es una guarda por si acaso: hasta
/// que la cabecera dejó de recibir un array de dos, el número estaba fijado por el tipo, y al pasarlo a
/// una lista —que es lo que el árbol de divisiones necesita— la garantía dejó de existir. Indexar el
/// array por la posición se convertía en un pánico **dentro del informe que se publica al salir**, o sea
/// perdiendo de golpe todo lo medido en la sesión.
///
/// Lo usan los tres sitios del informe que nombran una división —la cabecera, el título de su tabla de
/// tramos y la línea del foco—, cada uno con su propio adorno alrededor. Que la regla viva en un solo
/// sitio es lo que evita que uno de los tres se quede indexando por su cuenta el día que partir una
/// división deje de ser una rareza: desde que se puede partir, tres divisiones son un gesto.
pub fn lado(cuantas: usize, cual: usize) -> Option<&'static str> {
    match NOMBRE_DE_CONSOLA.get(cual) {
        Some(donde) if cuantas == NOMBRE_DE_CONSOLA.len() => Some(donde),
        _ => None,
    }
}

/// La pista de sitio tal y como la lleva la cabecera: entre paréntesis y con su sangría, o nada.
fn donde_cae(cuantas: usize, cual: usize) -> String {
    lado(cuantas, cual).map_or_else(String::new, |donde| format!("  ({donde})"))
}

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

    /// Lo que el informe publica de verdad, leído del mismo camino que escribe por pantalla.
    fn informe_de(medida: &Medida) -> String {
        let mut salida = Vec::new();
        medida
            .publicar_tramos(&mut salida)
            .expect("escribir a memoria no falla");
        String::from_utf8(salida).expect("el informe es texto")
    }

    /// La cabecera que se publica de verdad, por el mismo camino que la escribe al salir.
    ///
    /// El tamaño de ventana va fijo porque ninguna de las pruebas de aquí habla de él; la que sí lo hace
    /// lo pasa por su cuenta.
    fn cabecera_de(rejillas: &[(usize, Vec<(usize, term_core::Size)>)], visible: usize) -> String {
        cabecera_con(
            EnPuntos {
                ancho: 800.0,
                alto: 600.0,
            },
            rejillas,
            visible,
        )
    }

    fn cabecera_con(
        ventana: EnPuntos,
        rejillas: &[(usize, Vec<(usize, term_core::Size)>)],
        visible: usize,
    ) -> String {
        cabecera_de_una_tanda(ventana, rejillas, visible, None)
    }

    fn cabecera_de_una_tanda(
        ventana: EnPuntos,
        rejillas: &[(usize, Vec<(usize, term_core::Size)>)],
        visible: usize,
        arrastres: Option<Arrastres>,
    ) -> String {
        cabecera_completa(ventana, rejillas, visible, arrastres, None)
    }

    fn cabecera_completa(
        ventana: EnPuntos,
        rejillas: &[(usize, Vec<(usize, term_core::Size)>)],
        visible: usize,
        arrastres: Option<Arrastres>,
        restauracion: Option<Restauracion>,
    ) -> String {
        con_barra(
            ventana,
            rejillas,
            visible,
            arrastres,
            restauracion,
            Barra::default(),
        )
    }

    /// La cabecera con un estado de configuración concreto, que es lo que el guion de medida lee.
    fn con_configuracion(configuracion: LecturaDeAjustes) -> String {
        let mut salida = Vec::new();
        cabecera(
            &mut salida,
            EnPuntos {
                ancho: 1280.0,
                alto: 800.0,
            },
            &[(0, vec![(0, rejilla(49, 39))])],
            0,
            DeLaSesion {
                arrastres: None,
                restauracion: None,
                configuracion,
                barra: Barra::default(),
                reflujo: Reflujo::default(),
            },
        )
        .expect("escribir a memoria no falla");
        String::from_utf8(salida).expect("el informe es texto")
    }

    #[test]
    fn los_tres_estados_de_la_configuracion_se_distinguen_donde_el_guion_mira() {
        // **El guion invalida una corrida leyendo el PRIMER SUSTANTIVO de esta línea**, así que su forma
        // es tan contrato como el nombre de un fichero. Cambiar el texto dejaba la suite entera en verde
        // y **toda tanda de medida saldría inválida** — o, peor, pasaría por buena una que rompió el veto.
        //
        // **Y los tres tienen que empezar distinto.** El guion vacía el directorio de configuración, así
        // que un veto roto en el banco produce «se miró y no había»: si eso empezara igual que el vetado,
        // el veto roto se leería como una corrida buena. Lo cazó una revisión, después de que otra hubiera
        // pedido separar los estados sin ver esta consecuencia.
        let vetada = con_configuracion(LecturaDeAjustes::Vetada);
        let linea = vetada
            .lines()
            .find(|l| l.starts_with("configuración:"))
            .expect("no hay línea de configuración que el guion pueda leer");
        assert!(
            linea.starts_with("configuración: vetada"),
            "la línea de la configuración vetada ya no tiene la forma que el guion sabe leer: «{linea}»"
        );

        let sin = con_configuracion(LecturaDeAjustes::Sin);
        let linea = sin
            .lines()
            .find(|l| l.starts_with("configuración:"))
            .expect("no hay línea de configuración");
        assert!(
            !linea.starts_with("configuración: vetada"),
            "«se miró y no había» empieza igual que «vetada», así que un veto roto en el banco —donde no \
             hay fichero por construcción— pasaría por una corrida buena: «{linea}»"
        );

        let con = con_configuracion(LecturaDeAjustes::Con {
            cuanto: Duration::from_micros(25),
            avisos: 0,
        });
        let linea = con
            .lines()
            .find(|l| l.starts_with("configuración:"))
            .expect("no hay línea de configuración");
        assert!(
            linea.starts_with("configuración: del usuario"),
            "la línea de la configuración aplicada cambió de forma: «{linea}»"
        );
    }

    fn con_barra(
        ventana: EnPuntos,
        rejillas: &[(usize, Vec<(usize, term_core::Size)>)],
        visible: usize,
        arrastres: Option<Arrastres>,
        restauracion: Option<Restauracion>,
        barra: Barra,
    ) -> String {
        let mut salida = Vec::new();
        cabecera(
            &mut salida,
            ventana,
            rejillas,
            visible,
            DeLaSesion {
                arrastres,
                restauracion,
                configuracion: LecturaDeAjustes::default(),
                barra,
                reflujo: Reflujo::default(),
            },
        )
        .expect("escribir a memoria no falla");
        String::from_utf8(salida).expect("la cabecera es texto")
    }

    #[test]
    fn la_cabecera_declara_de_donde_salieron_las_consolas_solo_cuando_salieron_del_fichero() {
        // **Es la línea que distingue la tanda que restaura de la que pide.** Sin ella, una corrida que
        // abrió de fábrica con cuatro consolas y otra que las sacó del fichero publican **el mismo
        // informe**: la cabecera dice cuántas hay, no quién las pidió. Y el guion invalida la corrida
        // leyendo justo esta línea, así que si deja de salir, la tanda L pasa a medir el arranque de
        // siempre sin que nada lo diga.
        let restaurada = cabecera_completa(
            EnPuntos {
                ancho: 1280.0,
                alto: 800.0,
            },
            &[(0, vec![(0, rejilla(49, 39)), (1, rejilla(49, 39))])],
            0,
            None,
            Some(Restauracion {
                consolas: 4,
                leer: Some(Duration::from_micros(310)),
                abrirlas: Duration::from_millis(12),
            }),
        );
        assert!(
            restaurada.contains("restauradas:  4 consolas"),
            "la cabecera no declaró las consolas que salieron del fichero:\n{restaurada}"
        );
        // **Los dos tiempos, y por separado.** Publicar solo la suma daría un número que no se puede
        // extrapolar: leer no crece con el número de consolas y abrirlas sí.
        assert!(
            restaurada.contains("310µs") && restaurada.contains("12ms"),
            "la cabecera no declaró lo que costó leer y lo que costó abrirlas:\n{restaurada}"
        );

        let de_fabrica = cabecera_completa(
            EnPuntos {
                ancho: 1280.0,
                alto: 800.0,
            },
            &[(0, vec![(0, rejilla(49, 39)), (1, rejilla(49, 39))])],
            0,
            None,
            None,
        );
        assert!(
            !de_fabrica.contains("restauradas"),
            "una ventana abierta de fábrica dice haber restaurado, que es el disfraz que esta línea \
             existe para quitar:\n{de_fabrica}"
        );
    }

    #[test]
    fn una_lectura_que_nadie_tomo_se_publica_con_esas_palabras_y_no_como_un_cero() {
        // **La rama que hace honesto al `Option`, y no la ejercía ninguna prueba.** El campo se volvió
        // opcional justamente para que una ventana levantada sin tocar el disco no publicara `0ns en leer
        // y validar` —indistinguible de una lectura instantánea de verdad—, y sin embargo el único caso
        // que se pasaba por aquí traía número: cambiar el texto de la rama vacía por un cero, o por la
        // suma de los dos tiempos, dejaba el fichero entero en verde.
        //
        // **Lo que se afirma es lo que se lee, no que haya un `Option` dentro.** La cabecera es lo único
        // que llega a quien interpreta la tanda, y ahí la diferencia entre «sin medir» y «0ns» es la
        // diferencia entre saber que nadie midió y creerse un número.
        let sin_medir = cabecera_completa(
            EnPuntos {
                ancho: 1280.0,
                alto: 800.0,
            },
            &[(0, vec![(0, rejilla(49, 39)), (1, rejilla(49, 39))])],
            0,
            None,
            Some(Restauracion {
                consolas: 2,
                leer: None,
                abrirlas: Duration::from_millis(7),
            }),
        );

        assert!(
            sin_medir.contains("sin medir en leer y validar"),
            "una lectura que nadie tomó no se declara como tal:\n{sin_medir}"
        );
        assert!(
            sin_medir.contains("7ms en abrirlas"),
            "la línea perdió lo que sí se midió:\n{sin_medir}"
        );
        // **Y con la forma que el guion sabe leer, incluso sin número de lectura.** La guarda de la tanda
        // L saca la cuenta de consolas con `s/^restauradas: *\([0-9]*\) consolas .*$/\1/p`, y ese `.*`
        // detrás de «consolas» **exige que siga habiendo algo**: una línea que se quedara en
        // `restauradas:  2 consolas` no casaría, el guion no vería ninguna consola restaurada e
        // invalidaría una corrida buena diciendo que la ventana abrió de fábrica.
        let linea = sin_medir
            .lines()
            .find(|linea| linea.starts_with("restauradas:"))
            .expect("no hay línea de restauradas que el guion pueda leer");
        assert!(
            linea.starts_with("restauradas:  2 consolas ·"),
            "la línea de restauradas ya no tiene la forma que el guion de medida sabe leer: «{linea}»"
        );
    }

    #[test]
    fn la_cabecera_declara_los_arrastres_solo_cuando_la_tanda_los_tuvo() {
        // **Esta prueba existe porque el ayudante de aquí nunca llegaba a pasar `Some`.** La línea de
        // arrastres solo la cubría una prueba de `lib.rs` que entra por el informe entero, así que vaciar
        // este bloque dejaba **todas** las pruebas de este fichero en verde. Lo pilló la revisión.
        //
        // Los dos números son distintos a propósito: iguales no distinguirían publicar uno de publicar el
        // otro.
        let con_gesto = cabecera_de_una_tanda(
            EnPuntos {
                ancho: 1280.0,
                alto: 800.0,
            },
            &[(0, vec![(0, rejilla(49, 39)), (1, rejilla(49, 39))])],
            0,
            Some(Arrastres {
                inyectados: 1720,
                con_efecto: 1719,
                marcados: 3,
                informados: 7,
                gesto: "moviendo consolas",
            }),
        );
        assert!(
            con_gesto.contains(
                "arrastres:    1720 inyectados · 1719 efectos · 3 fotogramas con texto marcado · \
                 7 informes al programa (moviendo consolas)"
            ),
            "la cabecera no declaró los arrastres de la tanda:\n{con_gesto}"
        );

        let sin_gesto = cabecera_de_una_tanda(
            EnPuntos {
                ancho: 1280.0,
                alto: 800.0,
            },
            &[(0, vec![(0, rejilla(49, 39)), (1, rejilla(49, 39))])],
            0,
            None,
        );
        assert!(
            !sin_gesto.contains("arrastres"),
            "la cabecera habla de arrastres en una tanda que no los tuvo, así que las tandas de los anexos \
             anteriores dejan de tener la misma forma y no se pueden comparar:\n{sin_gesto}"
        );
    }

    fn rejilla(rows: u16, cols: u16) -> term_core::Size {
        term_core::Size { rows, cols }
    }

    #[test]
    fn el_tamaño_de_la_ventana_sale_en_una_linea_entera_y_sin_redondear() {
        // **Esta línea es un contrato con `scripts/medir-fotograma.sh`**, que la lee con una expresión
        // anclada por los dos extremos —`^ventana: *\(.*\) puntos$`— para comprobar que la ventana abrió con
        // el tamaño que se le pidió y para decidir si dos corridas se pueden promediar. Cualquier cosa
        // añadida **detrás** de «puntos» rompe el ancla del final sin romper el `contains` de la prueba de
        // `lib.rs`, y lo que el guion se encuentra entonces no es un error: es un tamaño **vacío**, o sea la
        // clase de número plausible por la que existen sus guardas. Se afirma la línea completa por eso.
        //
        // **Y con fracción, que es lo que un compositor entrega de verdad** (1279,8125 puntos, medido):
        // publicarla redondeada daría un número que no es el que hubo, y el guion compara con el mismo
        // margen de la aplicación precisamente porque la fracción llega. Los dos ejes son distintos para que
        // intercambiarlos no publique lo mismo.
        let texto = cabecera_con(
            EnPuntos {
                ancho: 1279.8125,
                alto: 800.0,
            },
            &[(0, vec![(0, rejilla(49, 39))])],
            0,
        );

        assert!(
            texto
                .lines()
                .any(|linea| linea == "ventana:      1279.8125×800 puntos"),
            "la línea del tamaño no es la que el guion de medida sabe leer:\n{texto}"
        );
    }

    #[test]
    fn con_dos_divisiones_la_cabecera_dice_de_qué_lado_cae_cada_una() {
        let texto = cabecera_de(&[(0, vec![(0, rejilla(59, 57)), (1, rejilla(59, 21))])], 0);

        assert!(
            texto.contains("  división 0:   59×57  (izquierda)"),
            "no publica la primera con su lado:\n{texto}"
        );
        assert!(
            texto.contains("  división 1:   59×21  (derecha)"),
            "no publica la segunda con su lado:\n{texto}"
        );
    }

    #[test]
    fn con_tres_divisiones_la_cabecera_no_se_inventa_un_lado_ni_se_cae() {
        // **El tipo ya no impide que lleguen tres.** Hasta este cambio la cabecera recibía un array de
        // dos y el número estaba fijado por él; al pasarlo a una lista —lo que el árbol de divisiones
        // necesita— indexar el array de lados por la posición pasó a ser un pánico posible **dentro del
        // informe que se publica al salir**, o sea perdiendo de golpe todo lo medido en la sesión.
        //
        // Y no basta con que no reviente: «izquierda» y «derecha» solo describen algo cuando hay dos, así
        // que con tres la pista tiene que **callarse**, no repartirse a las dos primeras. La aserción
        // mira las dos cosas porque un `get` que devuelva `None` solo para la tercera pasaría la primera
        // mitad y seguiría publicando una geometría falsa para las otras dos.
        let texto = cabecera_de(
            &[(
                0,
                vec![
                    (0, rejilla(59, 30)),
                    (1, rejilla(59, 30)),
                    (2, rejilla(59, 30)),
                ],
            )],
            0,
        );

        for division in [0, 1, 2] {
            assert!(
                texto.contains(&format!("  división {division}:")),
                "no publica la división {division}:\n{texto}"
            );
        }
        for lado in NOMBRE_DE_CONSOLA {
            assert!(
                !texto.contains(lado),
                "con tres divisiones sigue diciendo «{lado}», que ya no describe nada:\n{texto}"
            );
        }
    }

    /// Una medida con un fotograma anotado, que es lo mínimo para que el informe hable.
    fn con_un_fotograma() -> Medida {
        let mut medida = Medida::default();
        medida.anotar_vuelta(Tramos::de_una_vuelta(
            Some(Instant::now()),
            Instant::now(),
            ms(2),
            ms(1),
            ms(3),
        ));
        medida.cerrar_fotograma(ms(7));
        medida
    }

    #[test]
    fn dos_pasadas_del_mismo_fotograma_suman_su_trabajo_en_vez_de_pisarse() {
        // **El sesgo que esto arregla estuvo vivo y medido:** con `anotar_vuelta` sustituyendo, la
        // segunda pasada de un fotograma —egui la repite cuando algo pide recomponerse— machacaba la
        // vuelta buena de la primera. Y la segunda llega con lo interpretado a casi cero, porque la cola
        // ya se vació en la primera, así que lo que la consola interpretó de verdad desaparecía de lo
        // declarado y se publicaba como coste del framework de interfaz.
        let mut medida = Medida::default();
        let recogido = Instant::now();

        // La primera pasada: la que trae la espera y el trabajo de verdad.
        medida.anotar_vuelta(Tramos::de_una_vuelta(
            Some(recogido - ms(5)),
            recogido,
            ms(20),
            ms(2),
            ms(1),
        ));
        // La segunda: sin espera, porque no recogió nada, y con lo que costó repetir el trabajo.
        //
        // **Los tres tramos de la segunda van distintos de cero a propósito.** En producción apenas
        // interpreta —la cola se vació en la primera—, pero un cero aquí deja de distinguir sumar de no
        // sumar: comprobado mutándolo, con un cero en lo interpretado esta prueba pasaba **con la suma
        // de ese tramo quitada**. Los tres tienen que poder fallar por separado.
        medida.anotar_vuelta(Tramos::de_una_vuelta(None, recogido, ms(3), ms(2), ms(1)));

        assert_eq!(
            medida.cerrar_fotograma(ms(60)),
            Some(ms(29)),
            "lo declarado no es la suma de lo que hicieron las dos pasadas: 20+2+1 de la primera y \
             3+2+1 de la segunda"
        );
    }

    #[test]
    fn de_dos_pasadas_la_espera_es_la_de_la_primera_y_no_la_suma() {
        // La espera **no es trabajo de la aplicación**: es cuánto llevaba en la cola el byte más
        // antiguo. Sumar las dos daría un número que nadie esperó, y ese número va a la media de
        // «tecla → pantalla» que el informe publica. Se queda con la de la pasada que de verdad recogió.
        let mut medida = Medida::default();
        let recogido = Instant::now();

        medida.anotar_vuelta(Tramos::de_una_vuelta(
            Some(recogido - ms(5)),
            recogido,
            ms(1),
            ms(0),
            ms(0),
        ));
        medida.anotar_vuelta(Tramos::de_una_vuelta(
            Some(recogido - ms(9)),
            recogido,
            ms(1),
            ms(0),
            ms(0),
        ));
        medida.cerrar_fotograma(ms(10));

        // Se lee del informe, que es lo que un humano acaba viendo: la espera publicada tiene que ser
        // la de la primera pasada —5 ms— y no los 14 de sumarlas.
        let informe = informe_de(&medida);
        assert!(
            informe.contains("esperando en la cola:   5.000ms de media"),
            "la espera declarada no es la de la primera pasada; el informe dice: {informe}"
        );
        assert!(
            !informe.contains("14.000ms"),
            "la espera de las dos pasadas se sumó, y eso es un número que nadie esperó: {informe}"
        );
    }

    #[test]
    fn el_informe_dice_que_el_tramo_del_panel_incluye_pintarlo() {
        // **Durante todo el hito F1.6 este tramo se describió como «leer el panel»** —en sus dos
        // campos y en el texto que el informe imprime— cuando el reloj que lo produce abarca desde
        // antes de mirar la fecha hasta después de pintar la última fila. El anexo «Medido» de D3 se
        // apoya en este número, así que un lector que lo copie creyendo que solo mide leer le
        // atribuiría el pintado a quien se calcula restando: el framework de interfaz, o sea la cifra
        // sobre la que D3 decide.
        //
        // **Se afirma sobre lo que el informe escribe, no sobre la constante.** Comprobar la
        // constante demostraría que la constante está bien redactada, no que alguien la imprima —
        // que es el fallo que en este proyecto ha dejado pasar ocho cosas con la suite en verde.
        let informe = informe_de(&con_un_fotograma());

        assert!(
            informe.contains("el panel de estado:"),
            "el informe no publica el tramo del panel:\n{informe}"
        );
        assert!(
            informe.contains("atender el arrastre de su frontera"),
            "el informe publica el tramo del panel sin decir que ahí dentro se paga el gesto de \
             estrecharlo: quien lea la tanda creerá que ese coste está en el resto del fotograma, o sea \
             atribuido al framework de interfaz:\n{informe}"
        );
        assert!(
            informe.contains("INCLUYE pintar el panel"),
            "el informe publica el tramo del panel sin decir que incluye pintarlo, que es lo que \
             hace que el número se pueda copiar mal:\n{informe}"
        );
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
    fn una_consola_que_interpreta_y_no_se_pinta_declara_lo_interpretado_sin_tocar_las_medias() {
        // **El caso nuevo, y el más caro de los tres**: una consola que interpreta y no se pinta.
        // Aquí se ata lo que hace que no haga falta ninguna serie para ello — **las cuatro medias no
        // se mueven**, porque meter un cero de encolado en su media diría que pintar sale más barato
        // justamente en los fotogramas en los que no se pintó nada— y que lo interpretado **sí** se
        // devuelve para restarlo.
        let mut medida = Medida::default();
        let mut resto = RestoDelFotograma::default();

        medida.anotar_vuelta(Tramos::de_una_vuelta(
            None,
            Instant::now(),
            ms(2),
            Duration::ZERO,
            Duration::ZERO,
        ));
        let cronometrado = medida
            .cerrar_fotograma(ms(10))
            .expect("lo interpretado tiene que declararse aunque no se haya pintado");
        resto.anotar(ms(10), cronometrado);

        assert_eq!(cronometrado, ms(2), "lo que interpretó, y nada más");
        assert_eq!(
            resto.serie.media(),
            ms(8),
            "el resto se quedó con lo que interpretó una consola que no se pintó"
        );
        assert!(!medida.encolar.hay(), "se ensució la media de encolar");
        assert!(
            !medida.interpretar.hay(),
            "se ensució la media de interpretar"
        );
        assert!(!medida.vuelta.hay(), "se anotó una vuelta que no ocurrió");
        assert!(!medida.panel.hay(), "se ensució el tramo del panel");
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

    /// **La línea que dice lo que cuesta la barra, y que solo sale cuando la hubo.** Sin ella, lo que
    /// cuesta pintar las fichas —y desde este hito, acumular lo que reclaman sus divisiones— solo se
    /// podría averiguar restando dos tandas distintas, que es como se publican los números que no se
    /// pueden defender. Y el guion invalida la corrida leyendo justo esta línea.
    #[test]
    fn la_cabecera_dice_lo_que_costo_la_barra_solo_cuando_hubo_barra() {
        let ventana = EnPuntos {
            ancho: 1280.0,
            alto: 800.0,
        };
        let rejillas = &[
            (0, vec![(0, rejilla(49, 39)), (1, rejilla(49, 39))]),
            (1, vec![(2, rejilla(49, 39)), (3, rejilla(49, 39))]),
        ];

        let con = con_barra(
            ventana,
            rejillas,
            0,
            None,
            None,
            Barra {
                fotogramas: 100,
                total: Duration::from_micros(3_600),
                fichas_marcadas: 2,
            },
        );
        assert!(
            con.contains("barra:        100 fotogramas") && con.contains("36µs por fotograma"),
            "la cabecera no declaró lo que costó la barra ni su media:\n{con}"
        );

        let sin = con_barra(
            EnPuntos {
                ancho: 1280.0,
                alto: 800.0,
            },
            rejillas,
            0,
            None,
            None,
            Barra::default(),
        );
        assert!(
            !sin.contains("barra:"),
            "sin barra que pintar, la cabecera publicó un tramo que nadie midió:\n{sin}"
        );
    }

    fn con_reflujo(reflujo: Reflujo) -> String {
        let mut salida = Vec::new();
        cabecera(
            &mut salida,
            EnPuntos {
                ancho: 1280.0,
                alto: 800.0,
            },
            &[(0, vec![(0, rejilla(49, 39)), (1, rejilla(49, 39))])],
            0,
            DeLaSesion {
                reflujo,
                ..Default::default()
            },
        )
        .expect("escribir a memoria no falla");
        String::from_utf8(salida).expect("la cabecera es texto")
    }

    /// **La línea que dice lo que cuesta recolocar el texto, y que solo sale cuando se recolocó.**
    /// Recolocar corre en la mitad lógica del fotograma, donde no hay pintado que lo envuelva: lo que
    /// no se nombre aquí se publica como coste del framework, que es la cifra con la que **D3** juzga a
    /// egui. Con el historial encendido son casi mil microsegundos **por consola** y una frontera mueve
    /// dos.
    ///
    /// **Y la forma de la línea es un contrato con `scripts/medir-fotograma.sh`**, que la lee para
    /// invalidar una corrida de la tanda M. Su guarda saca el número con
    /// `s/^recolocar: *\([0-9]*\) veces .*$/\1/p` y compara contra los gestos inyectados: si la línea
    /// cambia de forma, el guion no ve **ninguna** recolocación e invalida corridas buenas — y si la
    /// línea desaparece del todo, la guarda que dice cazar «la frontera se movió sin cruzar bordes de
    /// celda» deja de cazar nada.
    #[test]
    fn la_cabecera_dice_lo_que_costo_recolocar_solo_cuando_se_recoloco() {
        // Los números de una corrida publicada: 320 recolocaciones a los ~984 µs que cuesta recorrer el
        // historial de una consola.
        let con = con_reflujo(Reflujo {
            fotogramas: 320,
            total: Duration::from_micros(320 * 984),
        });

        assert!(
            con.contains("314.88ms en total") && con.contains("984µs cada vez"),
            "la cabecera no declaró lo que costó recolocar ni lo que cuesta cada vez:\n{con}"
        );

        // **La forma exacta que el guion sabe leer.** El `.*` que sigue a «veces» exige que detrás haya
        // algo, igual que en la línea de restauradas: una línea recortada no casaría y el guion
        // invalidaría una corrida buena.
        let linea = con
            .lines()
            .find(|linea| linea.starts_with("recolocar:"))
            .expect("no hay línea de recolocar que el guion pueda leer");
        let (etiqueta, resto) = linea
            .split_once(" veces ")
            .expect("la línea de recolocar perdió las palabras que el guion busca");
        assert!(
            etiqueta
                .strip_prefix("recolocar:")
                .expect("la línea empieza por su etiqueta")
                .trim_start()
                .parse::<usize>()
                == Ok(320),
            "el guion no sacaría 320 recolocaciones de esta línea: «{linea}»"
        );
        assert!(
            !resto.is_empty(),
            "detrás de «veces» no queda nada y el guion exige que quede algo: «{linea}»"
        );

        // **Y en una tanda que no redimensiona nada, la línea no sale.** Publicar «0 veces» se lee como
        // «recolocar no cuesta nada» en vez de como «no se recolocó», y además le daría a la guarda del
        // guion un número que pasaría sus dos comprobaciones sin que nada se hubiera recolocado.
        let sin = con_reflujo(Reflujo::default());
        assert!(
            !sin.contains("recolocar:"),
            "sin recolocar nada, la cabecera publicó un tramo que nadie midió:\n{sin}"
        );
    }

    // ── El acoplamiento con el guion de medida, atado ───────────────────────────────────────────
    //
    // **El guion invalida corridas por lo que este informe DICE**, y hasta aquí ese acoplamiento
    // vivía en un comentario. Cuatro de sus patrones no los cubría ninguna prueba y uno estaba
    // atado con una negación, que se cumple por vacío en cuanto el literal cambie de nombre.
    //
    // **Lo que hace fuerte a lo que sigue es que no copia los patrones: los saca del guion y los
    // ejecuta.** Una prueba que repitiera el literal a mano se arregla sola cuando alguien traduce
    // el guion —cambia el `sed`, cambia la prueba, verde— y no habría atado nada.

    /// El guion de medida, leído en tiempo de compilación para que no se pueda desincronizar.
    const GUION: &str = include_str!("../../../scripts/medir-fotograma.sh");

    /// Un patrón que el guion aplica al informe, con la línea donde está para poder señalarlo.
    struct DelGuion {
        linea: usize,
        orden: &'static str,
        patron: String,
    }

    /// Los patrones que el guion busca **en el informe**, sacados de sus `sed` y `grep`.
    ///
    /// Se queda solo con los que llevan un literal del informe: los anclados (`^`…`$`) y los dos
    /// recuentos que el guion busca sin anclar. Un `grep` sobre otra cosa —el estado de salida, un
    /// fichero testigo— no describe el informe y no tiene nada que atar aquí.
    ///
    /// **Y queda uno fuera que sí describe el informe**, dicho para no describir mal lo excluido: el
    /// `grep -m1 '  división 0:'` con el que el guion compone su línea de rejilla al terminar. No
    /// lleva ancla ni palabra clave, así que este criterio no lo ve. **No invalida ninguna corrida**
    /// —solo alimenta un mensaje— y por eso se deja fuera en vez de ensanchar el criterio hasta que
    /// empiece a recoger ruido; si algún día pasa a decidir algo, entra.
    fn patrones_del_guion() -> Vec<DelGuion> {
        let mut encontrados = Vec::new();
        // **La orden se arrastra entre líneas a propósito.** El guion parte sus `sed` largos con
        // barra al final y pone cada `-e` en su propia línea, así que la línea que lleva el patrón
        // muchas veces **no** contiene la palabra `sed`. La primera versión de este extractor no lo
        // tuvo en cuenta y encontró 2 de 20 — lo cazó la guarda del recuento, que está justo debajo
        // para esto.
        let mut orden_en_curso = "";
        let mut continuacion = false;
        for (numero, linea) in GUION.lines().enumerate() {
            if !continuacion {
                orden_en_curso = "";
            }
            // **Un comentario no es una orden, y contarlo infló el recuento de 21 a 24.** El guion
            // explica sus propias guardas citando los patrones que aplica, así que su prosa está
            // llena de `grep` y de literales entrecomillados. Con los comentarios dentro, la guarda
            // que existe para que esta prueba no pase por vacío **se satisfacía con prosa**.
            if linea.trim_start().starts_with('#') {
                continue;
            }
            if linea.contains("sed ") {
                orden_en_curso = "sed";
            } else if linea.contains("grep ") {
                orden_en_curso = "grep";
            }
            continuacion = linea.trim_end().ends_with('\\');
            if orden_en_curso.is_empty() {
                continue;
            }
            // Los patrones del guion van entre comillas, y usa **las dos**: simples para los `sed`
            // y dobles para los dos recuentos que busca sin anclar.
            for comilla in ['\'', '"'] {
                let mut resto = linea;
                while let Some(abre) = resto.find(comilla) {
                    let tras = &resto[abre + 1..];
                    let Some(cierra) = tras.find(comilla) else {
                        break;
                    };
                    let dentro = &tras[..cierra];
                    resto = &tras[cierra + 1..];
                    // **Se busca el ancla en cualquier sitio, no al principio.** Los patrones de
                    // `sed` vienen envueltos en `s/…/…/p`, así que el `^` va dentro y no delante:
                    // pedirlo al principio encontraba 8 de 20.
                    //
                    // **Y tiene que llevar alguna letra.** Un patrón hecho solo de anclas y clases
                    // de caracteres —`^[0-9]+`, el segundo `grep` de una tubería— no lee el informe
                    // sino la salida del anterior, y sobre todo **no tiene texto que pueda cambiar
                    // de idioma**: no hay acoplamiento que atar ahí. Es un criterio y no una lista
                    // de excepciones, así que sigue valiendo cuando el guion crezca.
                    let describe_el_informe = (dentro.contains('^')
                        || dentro.contains("caracteres buscados")
                        || dentro.contains("glifos rasterizados"))
                        && dentro.chars().any(|c| c.is_alphabetic());
                    if describe_el_informe {
                        encontrados.push(DelGuion {
                            linea: numero + 1,
                            orden: orden_en_curso,
                            patron: dentro.to_string(),
                        });
                    }
                }
            }
        }
        encontrados
    }

    /// Lo que sale de aplicarle un patrón del guion a un texto.
    ///
    /// **Vacío y roto no son lo mismo, y por eso son dos variantes.** Los dos dan salida vacía, y
    /// confundirlos hace que la prueba culpe al sitio equivocado: «el informe cambió de texto»
    /// cuando lo que pasa es que el extractor de patrones cortó mal el que iba a aplicar. Uno señala
    /// un defecto del proyecto, el otro un defecto de esta prueba.
    ///
    /// **Y el punto ciego que esto NO cubre, dicho en vez de tapado.** La distinción se hace con el
    /// estado de salida, así que solo separa los patrones que la orden **rechaza**. Un corte que deja
    /// un patrón todavía válido pero que no imprime —quitarle la `p` final a un `sed`, por ejemplo—
    /// sale con 0 y es **indistinguible** de «el informe cambió». Contra eso lo único que hay es la
    /// guarda del recuento mínimo de ahí abajo: si el extractor se rompe de verdad, encuentra menos
    /// patrones y la prueba lo dice. Comprobado con las dos mutaciones.
    enum Salida {
        /// El patrón se aplicó y extrajo algo.
        Extrajo,
        /// El patrón se aplicó bien y no casó con nada.
        NoCaso,
        /// La orden no pudo con el patrón: no es un fallo del informe, es de esta prueba.
        PatronInvalido(i32),
    }

    /// Aplica un patrón del guion a un texto **con la orden de verdad**, no con una imitación.
    ///
    /// Reimplementar `sed` en Rust sería meter una segunda fuente de divergencia justo donde se
    /// quiere quitar la primera.
    ///
    /// **El precio, declarado: esto hace que `cargo test` dependa de `sed` y `grep`.** Aquí se acepta
    /// porque lo que esta prueba protege es un guion de shell, así que donde no haya `sed` tampoco hay
    /// nada que proteger. Si no están, revienta con su mensaje en vez de saltarse sola — un
    /// `#[cfg(unix)]` la haría desaparecer en silencio, que es peor. **F2.7 lleva Windows**, y ahí
    /// habrá que decidir: acotarla al sistema o dar por bueno que el banco no corre en esa
    /// plataforma.
    fn extrae(p: &DelGuion, informe: &str) -> Salida {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let mut orden = Command::new(p.orden);
        match p.orden {
            "sed" => {
                orden.arg("-n").arg(&p.patron);
            }
            _ => {
                orden.arg("-E").arg(&p.patron);
            }
        }
        let mut hijo = orden
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| {
                panic!(
                    "no se pudo lanzar «{}»: {e}. El banco tampoco funcionaría sin él",
                    p.orden
                )
            });
        hijo.stdin
            .as_mut()
            .expect("la entrada está abierta")
            .write_all(informe.as_bytes())
            .expect("escribir a una tubería de memoria no falla");
        let salida = hijo.wait_with_output().expect("recoger la salida");
        let codigo = salida.status.code().unwrap_or(-1);
        // **Cada orden dice «no he encontrado nada» de una forma distinta.** `grep` sale con 1
        // cuando no casa y con 2 o más cuando el patrón no le sirve; `sed` sale con 0 aunque no
        // sustituya nada, y solo se queja si el patrón está mal escrito. Sin esta distinción, un
        // patrón mal extraído por esta prueba se publicaría como un acoplamiento roto del proyecto.
        let patron_invalido = match p.orden {
            "grep" => codigo >= 2,
            _ => codigo != 0,
        };
        if patron_invalido {
            return Salida::PatronInvalido(codigo);
        }
        if String::from_utf8_lossy(&salida.stdout).trim().is_empty() {
            Salida::NoCaso
        } else {
            Salida::Extrajo
        }
    }

    /// Los informes que el productor sabe escribir, uno por cada variante que el guion distingue.
    ///
    /// **Son varios y no uno porque los tres estados de la configuración son excluyentes**: un
    /// informe no puede decir a la vez que la configuración está vetada y que se leyó la del
    /// usuario. Cada patrón tiene que extraer de **alguno**; ninguno tiene que extraer de todos.
    fn informes_de_referencia() -> Vec<(&'static str, String)> {
        let rejillas = vec![
            (0, vec![(0, rejilla(49, 39)), (1, rejilla(49, 21))]),
            (1, vec![(0, rejilla(49, 39))]),
        ];
        let ventana = EnPuntos {
            ancho: 1280.0,
            alto: 800.0,
        };

        let mut barra = Barra::default();
        barra.anotar(ms(7));
        barra.vio_marcadas(3);

        let mut reflujo = Reflujo::default();
        reflujo.anotar(ms(2));

        let arrastres = Arrastres {
            inyectados: 12,
            con_efecto: 6,
            marcados: 4,
            informados: 2,
            gesto: "moviendo consolas",
        };

        let restauracion = Restauracion {
            consolas: 4,
            leer: Some(ms(1)),
            abrirlas: ms(5),
        };

        let cabecera_con_todo = |configuracion| {
            let mut salida = Vec::new();
            cabecera(
                &mut salida,
                EnPuntos {
                    ancho: ventana.ancho,
                    alto: ventana.alto,
                },
                &rejillas,
                0,
                DeLaSesion {
                    arrastres: Some(arrastres),
                    restauracion: Some(restauracion),
                    configuracion,
                    barra,
                    reflujo,
                },
            )
            .expect("escribir a memoria no falla");
            String::from_utf8(salida).expect("el informe es texto")
        };

        // Los tramos de la ventana que el guion también lee, cada uno poblado.
        let mut guardado = Guardado::default();
        guardado.anotar_espejo(ms(1));
        guardado.anotar_escritura(ms(3), true);
        guardado.anotar_escritura(ms(3), false);

        let mut descubrimiento = Descubrimiento::default();
        descubrimiento.anotar(ms(4), 9, 2);

        let mut rasterizado = Rasterizado::default();
        rasterizado.anotar(ms(1), 15, 1);

        let mut tramos_de_la_ventana = Vec::new();
        guardado
            .publicar(&mut tramos_de_la_ventana)
            .expect("escribir a memoria no falla");
        descubrimiento
            .publicar(&mut tramos_de_la_ventana)
            .expect("escribir a memoria no falla");
        rasterizado
            .publicar(&mut tramos_de_la_ventana)
            .expect("escribir a memoria no falla");

        vec![
            (
                "cabecera con la configuración vetada",
                cabecera_con_todo(LecturaDeAjustes::Vetada),
            ),
            (
                "cabecera sin configuración que aplicar",
                cabecera_con_todo(LecturaDeAjustes::Sin),
            ),
            (
                "cabecera con la configuración del usuario",
                cabecera_con_todo(LecturaDeAjustes::Con {
                    cuanto: ms(1),
                    avisos: 2,
                }),
            ),
            (
                "los tramos de la ventana",
                String::from_utf8(tramos_de_la_ventana).expect("el informe es texto"),
            ),
        ]
    }

    #[test]
    fn cada_patron_que_el_guion_busca_en_el_informe_sigue_extrayendo_algo() {
        let patrones = patrones_del_guion();

        // **La guarda cuenta POR FORMA y no un total, y la diferencia la encontró una revisión
        // mutándola.** Con un total —«al menos 20», habiendo 21— la guarda toleraba perder
        // exactamente uno, y los dos que se reconocen por palabra clave son justo los que pueden
        // caerse de uno en uno: traduciendo **solo** el de las fuentes, el conjunto bajaba a 20, la
        // guarda pasaba, y su tanda quedaba invalidada para siempre sin que nada lo dijera. Era el
        // segundo disfraz que este proyecto tiene fichado: un número con el que el defecto y el
        // arreglo dan el mismo resultado.
        //
        // Contando las dos familias por separado, perder **uno solo** de los dos por palabra clave ya
        // cae. Y sigue siendo un criterio y no una lista, así que aguanta que el guion crezca.
        let anclados = patrones.iter().filter(|p| p.patron.contains('^')).count();
        let por_palabra_clave = patrones.len() - anclados;
        assert!(
            anclados >= 19 && por_palabra_clave >= 2,
            "el extractor ha encontrado {anclados} patrones anclados y {por_palabra_clave} por \
             palabra clave, y hay al menos 19 y 2: ha dejado de reconocer la forma de alguno, así \
             que esta prueba estaría pasando por vacío justo donde menos se nota"
        );

        // **Y la guarda que de verdad sobrevive a que el guion crezca: cobertura POR USO.** Las dos
        // de arriba son de forma —cuántos patrones y de qué tipo— y las dos se pueden satisfacer
        // mientras se pierde una guarda real: basta con que el guion gane una línea que lea el
        // informe con una forma que este extractor no reconozca. Se midió, y se perdía una guarda
        // entera con las 23 pruebas en verde.
        //
        // Así que se cuenta al revés: **cada línea del guion que le aplique `grep` o `sed` al
        // informe tiene que haber aportado al menos un patrón**. Lo que quede exento se nombra aquí
        // una por una, con su razón — una lista corta y explícita envejece mejor que un criterio que
        // se ensancha hasta dejar de fallar.
        let mut sin_aportar = Vec::new();
        for (numero, linea) in GUION.lines().enumerate() {
            let numero = numero + 1;
            if linea.trim_start().starts_with('#') || !linea.contains("$INFORME") {
                continue;
            }
            let aplica_un_patron = linea.contains("grep") || linea.contains("sed");
            if !aplica_un_patron {
                continue;
            }
            // **Y su patrón tiene que llevar texto, por lo mismo que arriba.** El guion tiene una
            // línea que cuenta las del fichero de errores con un patrón vacío: aplica un `grep` al
            // informe y **no lee de él ninguna palabra**, así que no hay nada que se pueda quedar
            // atrás al traducir. Es el mismo criterio que decide qué patrón se extrae, aplicado a
            // qué línea tiene que aportar uno — y por ser el mismo, las dos mitades no se pueden
            // desincronizar.
            let algun_patron_con_texto =
                linea
                    .split(['\'', '"'])
                    .skip(1)
                    .step_by(2)
                    .any(|entre_comillas| {
                        !entre_comillas.starts_with('$')
                            && entre_comillas.chars().any(|c| c.is_alphabetic())
                    });
            if !algun_patron_con_texto {
                continue;
            }
            // La única exenta, y por eso va con su número: compone el mensaje de la rejilla al
            // terminar y **no invalida ninguna corrida**. Su literal lo cubre de rebote el recuento
            // de divisiones. Si algún día decide algo, se quita de aquí y tiene que aportar.
            const SOLO_COMPONE_UN_MENSAJE: usize = 933;
            if numero == SOLO_COMPONE_UN_MENSAJE {
                continue;
            }
            if !patrones.iter().any(|p| p.linea == numero) {
                sin_aportar.push(format!("  medir-fotograma.sh:{numero}: {}", linea.trim()));
            }
        }
        assert!(
            sin_aportar.is_empty(),
            "{} línea(s) del guion leen el informe con `grep` o `sed` y esta prueba no ha sacado \
             ningún patrón de ellas, así que **no las está atando**. O el extractor no reconoce su \
             forma, o son guardas nuevas que hay que cubrir — y mientras tanto pueden romperse en \
             silencio:\n{}",
            sin_aportar.len(),
            sin_aportar.join("\n")
        );

        let informes = informes_de_referencia();
        let mut huerfanos = Vec::new();
        let mut invalidos = Vec::new();
        for p in &patrones {
            let mut extrajo_de_alguno = false;
            for (_, informe) in &informes {
                match extrae(p, informe) {
                    Salida::Extrajo => extrajo_de_alguno = true,
                    Salida::NoCaso => {}
                    // Una entrada por patrón y no por informe: el mismo corte mal hecho se veía
                    // cuatro veces, una por informe de referencia.
                    Salida::PatronInvalido(codigo) => {
                        let entrada = format!(
                            "  medir-fotograma.sh:{} ({}) {:?} → salió con {codigo}",
                            p.linea, p.orden, p.patron
                        );
                        if !invalidos.contains(&entrada) {
                            invalidos.push(entrada);
                        }
                    }
                }
            }
            if !extrajo_de_alguno {
                huerfanos.push(format!(
                    "  medir-fotograma.sh:{} ({}) {:?}",
                    p.linea, p.orden, p.patron
                ));
            }
        }
        // **Esto se comprueba antes, y culpando a esta prueba y no al proyecto.** Un patrón que la
        // orden rechaza no dice nada del informe: dice que el extractor de arriba lo cortó mal.
        assert!(
            invalidos.is_empty(),
            "el extractor de esta prueba ha sacado {} patrones que su propia orden rechaza, así que \
             los ha cortado mal. **No es un fallo del informe ni del guion, es de esta prueba** — \
             arréglala antes de creerte lo que dice del resto:\n{}",
            invalidos.len(),
            invalidos.join("\n")
        );
        assert!(
            huerfanos.is_empty(),
            "{} de {} patrones del guion no extraen nada de ningún informe que el productor sepa \
             escribir. O el informe cambió de texto sin el guion, o al revés — y en cuanto eso \
             pasa el guion deja de invalidar corridas malas en vez de fallar:\n{}",
            huerfanos.len(),
            patrones.len(),
            huerfanos.join("\n")
        );
    }

    #[test]
    fn el_guion_siembra_el_mismo_fichero_de_disposicion_que_la_aplicacion_escribe() {
        // **La otra mitad del acoplamiento, y la que no se ve.** El guion escribe una disposición a
        // mano para la tanda que restaura, con la ruta y las claves puestas en su propio texto. La
        // prueba que había afirmaba que la constante del nombre valía un literal escrito **al
        // lado**: se renombran las dos mitades a la vez, queda verde, y el guion sigue sembrando el
        // nombre viejo.
        assert!(
            GUION.contains(crate::session::FICHERO),
            "el guion de medida no siembra «{}», que es el fichero que la aplicación lee: la tanda \
             que restaura abriría de fábrica y su corrida se publicaría como buena",
            crate::session::FICHERO
        );
    }
}
