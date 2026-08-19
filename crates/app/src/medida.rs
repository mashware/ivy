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
    /// **Las cinco cosas, y decirlo entero importa.** El número lo componen **dos tramos disjuntos** de
    /// `Ventana::pintar_division`, sumados: uno antes de pintar la consola —mirar la fecha, leer si
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
    pub fn hay_algo(&self) -> bool {
        self.ida.hay() || self.vuelta.hay()
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

/// Lo grande que es la ventana, en la unidad en la que se le pide que abra.
///
/// **Dos campos con nombre y no una pareja de números**, por lo mismo que los tramos de la tabla llevan
/// el suyo: dos `f32` seguidos se pueden intercambiar sin que el compilador diga nada, y el informe
/// publicaría una ventana con el ancho y el alto cambiados sin que quien lo lea pueda notarlo.
pub struct EnPuntos {
    pub ancho: f32,
    pub alto: f32,
}

/// Los gestos de arrastre que una tanda de medida se inyectó a sí misma, tal como se declaran.
#[derive(Clone, Copy)]
pub struct Arrastres {
    /// Gestos enteros que se inyectaron: los que llegaron a mandar su evento de soltar.
    pub inyectados: usize,
    /// De ésos, los que el árbol **aceptó**, o sea los que reordenaron la disposición de verdad.
    pub reordenaron: usize,
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
pub fn cabecera(
    a: &mut dyn std::io::Write,
    ventana: EnPuntos,
    rejillas: &[(usize, Vec<(usize, term_core::Size)>)],
    visible: usize,
    arrastres: Option<Arrastres>,
) -> std::io::Result<()> {
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
    // los **inyectados** dicen que el gesto ocurrió, y los que **reordenaron** dicen que el árbol los
    // aceptó. Hacen falta los dos porque no son lo mismo: el suelo geométrico rechaza el soltado cuando la
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
            "arrastres:    {} inyectados, {} reordenaron",
            arrastres.inyectados, arrastres.reordenaron
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
        let mut salida = Vec::new();
        cabecera(&mut salida, ventana, rejillas, visible, arrastres)
            .expect("escribir a memoria no falla");
        String::from_utf8(salida).expect("la cabecera es texto")
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
                reordenaron: 1719,
            }),
        );
        assert!(
            con_gesto.contains("arrastres:    1720 inyectados, 1719 reordenaron"),
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
}
