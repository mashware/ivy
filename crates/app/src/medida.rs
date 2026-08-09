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
//! fotograma que lo muestra está pagado, en **cuatro tramos que suman el total**:
//!
//! 1. **Esperar** — en la cola, hasta que el bucle lo recoge. En un volcado es el tramo que domina,
//!    y con diferencia: el presupuesto por fotograma deja pasar 128 KiB cada vez, así que lo que no
//!    cabe espera su turno.
//! 2. **Interpretar** — meterlo en la rejilla del terminal.
//! 3. **Encolar** — la llamada al pintor. Se llama así porque eso es lo que hace: apila formas y
//!    vuelve.
//! 4. **El resto del fotograma** — teselar esas formas, dibujarlas, y todo el armazón que egui
//!    monta alrededor: recoger la entrada del escritorio, abrir y cerrar la pasada, medir la
//!    fuente, subir texturas. Es lo que queda del coste total al quitarle los dos tramos
//!    anteriores, y **se llama «el resto» a propósito**: desde dentro no hay forma de aislar la
//!    teselación, así que ponerle ese nombre sería publicar como medido algo que no lo está.
//!
//! El total lo publica `eframe`, que es quien puede: cuando se tesela, la pasada ya ha terminado.
//!
//! Los cuatro tramos se guardan **del mismo fotograma**. El coste total llega con uno de retraso
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
#[derive(Clone, Copy)]
struct Tramos {
    esperado: Duration,
    interpretado: Duration,
    encolado: Duration,
}

/// Los dos cronómetros del hito y lo que llevan medido.
#[derive(Default)]
pub struct Medida {
    ida: Serie,
    esperar: Serie,
    interpretar: Serie,
    encolar: Serie,
    resto_del_fotograma: Serie,
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
    /// `esperando_desde` es cuándo llegó el byte más antiguo de los recogidos; sin bytes en este
    /// fotograma no hay nada que medir de la vuelta, y no se anota nada — un fotograma en reposo no
    /// es una vuelta rapidísima, es una vuelta que no ocurrió.
    pub fn anotar_vuelta(
        &mut self,
        esperando_desde: Option<Instant>,
        recogido_en: Instant,
        interpretado: Duration,
        encolado: Duration,
    ) {
        let Some(desde) = esperando_desde else {
            self.pendiente = None;
            return;
        };
        self.pendiente = Some(Tramos {
            esperado: recogido_en.saturating_duration_since(desde),
            interpretado,
            encolado,
        });
    }

    /// Cierra el fotograma anterior con lo que `eframe` dice que costó de CPU.
    ///
    /// Aquí se completa la vuelta: lo que no estaba cronometrado es lo que queda del total al
    /// quitarle los dos tramos que sí lo estaban.
    pub fn cerrar_fotograma(&mut self, coste_cpu: Duration) {
        let Some(tramos) = self.pendiente.take() else {
            return;
        };
        // El total puede salir por debajo de los tramos medidos: son de fotogramas contiguos pero
        // no idénticos, y el reloj de `eframe` no es el mismo que el de aquí. Cuando pasa, el resto
        // es cero en vez de un número negativo con pinta de dato.
        let resto = coste_cpu.saturating_sub(tramos.interpretado + tramos.encolado);

        self.esperar.anotar(tramos.esperado);
        self.interpretar.anotar(tramos.interpretado);
        self.encolar.anotar(tramos.encolado);
        self.resto_del_fotograma.anotar(resto);
        self.vuelta.anotar(tramos.esperado + coste_cpu);
    }

    /// Publica los dos números por la salida estándar, con lo que hace falta para leerlos.
    ///
    /// Se imprime al cerrar y no durante: la ventana es el terminal, así que no hay sitio dentro
    /// donde enseñarlos sin taparle la pantalla al programa.
    pub fn publicar(&self, rejilla: term_core::Size) {
        if !self.ida.hay() && !self.vuelta.hay() {
            return;
        }

        let perfil = if cfg!(debug_assertions) {
            "depuración — el número que vale es el de --release"
        } else {
            "release"
        };
        println!("\n── latencia ──");
        println!("rejilla:      {}×{}", rejilla.rows, rejilla.cols);
        println!("renderizador: glow");
        println!("perfil:       {perfil}");
        println!("máquina:      {}", maquina());

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
            println!("  el resto del fotograma: {}", self.resto_del_fotograma);
            println!(
                "  «el resto» es teselar, dibujar y el armazón de egui: desde dentro no se pueden\n  \
                 separar, y llamarlo «teselado» sería publicar como medido algo que no lo está"
            );
        }
    }
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

    #[test]
    fn los_cuatro_tramos_suman_el_total_del_fotograma() {
        // Es la propiedad que hace que el desglose sirva para atribuir: si los tramos no suman, el
        // veredicto tendría que explicar un hueco, y quien lo lea no sabría a qué culpar.
        let mut medida = Medida::default();
        let ahora = Instant::now();

        medida.anotar_vuelta(Some(ahora - ms(100)), ahora, ms(2), ms(3));
        medida.cerrar_fotograma(ms(10));

        assert_eq!(medida.esperar.media(), ms(100));
        assert_eq!(medida.interpretar.media(), ms(2));
        assert_eq!(medida.encolar.media(), ms(3));
        assert_eq!(medida.resto_del_fotograma.media(), ms(5), "10 menos 2 y 3");
        assert_eq!(medida.vuelta.media(), ms(110), "la espera más el fotograma");
    }

    #[test]
    fn un_fotograma_sin_salida_no_cuenta_como_vuelta_rapidisima() {
        // Sin este descarte, cada fotograma en reposo metería un cero en la serie y la media diría
        // que la aplicación responde al instante justamente por no haber hecho nada.
        let mut medida = Medida::default();

        medida.anotar_vuelta(None, Instant::now(), ms(2), ms(3));
        medida.cerrar_fotograma(ms(10));

        assert!(!medida.vuelta.hay(), "se anotó una vuelta que no ocurrió");
    }

    #[test]
    fn un_total_menor_que_sus_tramos_no_produce_un_negativo() {
        // Los tramos y el total vienen de fotogramas contiguos y de relojes distintos, así que el
        // total puede quedarse corto. Antes de restar sin más, un resto negativo saldría publicado
        // como un número enorme y con toda la pinta de ser real.
        let mut medida = Medida::default();
        let ahora = Instant::now();

        medida.anotar_vuelta(Some(ahora), ahora, ms(8), ms(8));
        medida.cerrar_fotograma(ms(1));

        assert_eq!(medida.resto_del_fotograma.media(), Duration::ZERO);
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
