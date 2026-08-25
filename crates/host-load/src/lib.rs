//! Mide cuánto tarda un terminal en **vaciar** una carga fija que se le vuelca por la salida
//! estándar, y agrega el resultado de varias divisiones que producen a la vez.
//!
//! # Qué mide, y qué no
//!
//! Mide lo que el terminal **saca del pseudoterminal**, no lo que **pinta**. Un terminal que traga
//! rápido y luego descarta frames sale bien en este número y mal a la vista, así que esto es un
//! **suelo**, no un techo.
//!
//! Eso lo hace un buen instrumento para **comparar terminales ajenos** —si uno falla ya en el
//! vaciado, no hace falta ver el pintado para descartarlo— y un **mal criterio para el nuestro**:
//! la mitigación que el roadmap planifica para F1.3 es repintar solo cuando el PTY produzca
//! cambios, o sea coalescer frames, que aprobaría esta prueba por construcción.
//!
//! El pintado es **inobservable desde aquí**: ningún protocolo de terminal reporta un frame
//! presentado, y el ida y vuelta de `ESC[6n` solo demuestra *consumido y parseado*. El instrumento
//! que sí lo vería son contadores dentro de la propia app —bytes procesados por frame, frames
//! presentados, retraso del último—, y eso necesita un bucle de pintado que todavía no existe.
//!
//! # Protocolo
//!
//! ```text
//! # 1. Una sola división: la referencia
//! cargo run --release -p host-load -- run --tag wezterm-1
//!
//! # 2. N divisiones visibles a la vez, una orden por división, dentro de la misma ventana de 30 s
//! cargo run --release -p host-load -- run --tag wezterm-4
//!
//! # 3. El agregado de cada tanda
//! cargo run --release -p host-load -- report --tag wezterm-4
//! ```
//!
//! Lo que decide **no es el MiB/s absoluto**, es si el agregado **sube** con N o baja. Un terminal
//! que reparte el trabajo de verdad multiplica el agregado; uno que se pelea por un hilo común lo
//! mantiene o lo empeora.
//!
//! Las divisiones tienen que estar **visibles a la vez**: un terminal que solo monta la pestaña
//! activa mediría una sola por muy abiertas que estén las demás.
//!
//! ## Deja el ancho de la ventana fuera del experimento
//!
//! Al partir una ventana en N divisiones, cada una se estrecha. Las líneas de la carga miden ~46
//! columnas: por debajo de eso el terminal empieza a **envolver**, que es trabajo extra, y el
//! agregado bajaría con N por una razón que no es la que se quiere medir.
//!
//! Así que **la comparación honesta es a igual ancho**: o cada división mantiene ≥46 columnas, o se
//! mide con la misma anchura por división en las dos fases. La herramienta no puede comprobarlo
//! —desde el proceso productor no se ve el reparto de la ventana— así que queda en manos de quien
//! mide, y por eso está escrito aquí.
//!
//! ## Los dos regímenes se leen distinto, y no se comparan entre sí
//!
//! Sin `--rate` la carga sale tan rápido como el terminal admita, y el MiB/s responde «cuánto da».
//! Con `--rate` responde otra cosa: **si aguantó el caudal pedido**. Al ir a ritmo, el tiempo total
//! incluye las esperas, así que un terminal que va sobrado devuelve exactamente el caudal que se le
//! pidió y ninguno más — medido: `--rate 10` da 10,0 MiB/s clavados. Lo que informa es la caída:
//! con un consumidor que no daba abasto, esa misma orden devolvió 0,3 MiB/s.
//!
//! O sea que en modo caudal la medida está **censurada por arriba**: sirve para detectar al que no
//! aguanta, no para ordenar a los que sí. Mezclar los dos regímenes bajo una etiqueta daría un
//! agregado sin sentido, así que [`aggregate`] **se niega a hacerlo** en vez de fiarlo a la
//! disciplina de quien mide.
//!
//! # Números de referencia
//!
//! Medidos el 8 de agosto de 2026, Wayland, GPU por hardware, 22,7 MiB de carga:
//!
//! ```text
//!             1 división            4 divisiones visibles      caída   agregado
//! WezTerm     0,80 s · 28,4 MiB/s   0,98 / 0,98 / 1,01 / 1,06   ×1,26   85,7 MiB/s
//! Orca        2,63 s ·  8,6 MiB/s   8,44 / 12,50 / 12,72 / 12,64  ×4,4    7,1 MiB/s
//! ```
//!
//! Orca mueve **menos** trabajo total con cuatro divisiones que con una: escala negativo. Es el
//! resultado que sostiene la decisión D3 de no construir sobre Electron y xterm.js.

use std::time::Duration;

/// Tamaño del trozo con el que se alimenta la salida.
///
/// Coincide con el búfer de lectura del PTY en `term-core`, pero **lo que llega al otro lado no es
/// un bloque de este tamaño**: Rust interpone un `LineWriter` en la salida estándar, y medido con
/// `strace` cada trozo sale en dos llamadas —una corta con la cola de línea que quedó pendiente y
/// otra de ~8 KiB hasta el último salto—. El sesgo es constante, así que la comparación entre
/// terminales sobrevive; el MiB/s absoluto lleva ese sesgo dentro.
pub const CHUNK: usize = 8 * 1024;

/// Cada cuántas líneas la carga repinta la barra de estado.
const LINEAS_POR_REPINTADO: usize = 50;

/// Construye la carga.
///
/// **Determinista a propósito**: el contenido de la línea `i` depende solo de `i`, sin reloj, sin
/// azar y sin la ruta de ningún fichero. Dos máquinas distintas generan los mismos bytes, que es
/// lo que hace comparables dos medidas.
///
/// El patrón lleva color, negrita, atenuado y un repintado con movimiento de cursor **a propósito**:
/// alimentar un terminal con un solo carácter repetido recorre el camino rápido de su parser y da
/// un número que miente sobre el caso real.
///
/// Se dejan de añadir líneas al llegar a `bytes_objetivo`, sin cortar la última: truncar a mitad
/// dejaría una secuencia de escape partida, y eso ya no sería la misma carga.
pub fn build_load(bytes_objetivo: usize) -> Vec<u8> {
    let mut carga = Vec::with_capacity(bytes_objetivo + 256);
    let mut i: usize = 1;
    while carga.len() < bytes_objetivo {
        let color = 31 + i % 7;
        let ms = 3 + i % 40;
        carga.extend_from_slice(
            format!(
                "\x1b[{color}m[\x1b[1m{i:06}\x1b[0m\x1b[{color}m]\x1b[0m compilando el módulo {i} … \x1b[32mhecho\x1b[0m \x1b[2m({ms} ms)\x1b[0m\n"
            )
            .as_bytes(),
        );
        if i.is_multiple_of(LINEAS_POR_REPINTADO) {
            // Guardar cursor, saltar al origen, pintar en vídeo inverso, volver: lo que hace una
            // interfaz de texto y lo que más trabajo le da al que pinta.
            carga.extend_from_slice(format!("\x1b7\x1b[H\x1b[7m {i} \x1b[0m\x1b8").as_bytes());
        }
        i += 1;
    }
    carga
}

/// Los caracteres con los que la carga en CJK escribe, y por qué éstos.
///
/// **Cubren los tres bloques que el hito de las fuentes persigue** —kanji, hiragana y hangul— más un
/// icono del Área de Uso Privado, que es el otro caso. Ninguno lo cubren las fuentes de fábrica de egui,
/// así que cada uno **dispara de verdad** el camino de descubrimiento en vez de rozarlo.
const LETRAS_CJK: &[char] = &[
    '日', '本', '語', '漢', '字', '中', '文', 'あ', 'い', 'う', 'え', 'お', '한', '국', '어',
    '\u{f015}',
];

/// La misma carga, pero escribiendo en CJK.
///
/// **Existe aparte y `build_load` no se toca, y eso es deliberado**: cambiar la carga de siempre haría
/// que las tandas nuevas no fueran comparables con ninguno de los anexos publicados. Ésta solo la pide la
/// tanda que mide el descubrimiento de fuentes.
///
/// **Determinista igual que la otra**: el contenido de la línea `i` depende solo de `i`.
pub fn build_load_cjk(bytes_objetivo: usize) -> Vec<u8> {
    let mut carga = Vec::with_capacity(bytes_objetivo + 256);
    let mut i: usize = 1;
    while carga.len() < bytes_objetivo {
        let color = 31 + i % 7;
        // **Una palabra distinta por línea, rotando**, para que no sea siempre el mismo carácter: quien
        // pinta cachea por carácter, así que repetir uno solo recorrería el camino rápido y mediría el
        // descubrimiento de **una** fuente en vez del de una pantalla de verdad.
        let palabra: String = (0..4)
            .map(|n| LETRAS_CJK[(i + n) % LETRAS_CJK.len()])
            .collect();
        carga.extend_from_slice(
            format!(
                "\x1b[{color}m[\x1b[1m{i:06}\x1b[0m\x1b[{color}m]\x1b[0m {palabra} — compilando el módulo {i} … \x1b[32m完了\x1b[0m\n"
            )
            .as_bytes(),
        );
        if i.is_multiple_of(LINEAS_POR_REPINTADO) {
            carga.extend_from_slice(format!("\x1b7\x1b[H\x1b[7m {i} \x1b[0m\x1b8").as_bytes());
        }
        i += 1;
    }
    carga
}

/// Primer instante alineado en el que arrancar, contado desde la época.
///
/// **Trabaja en fracciones de segundo, y eso es lo único que hace que sirva.** La primera versión
/// restaba dos épocas ya truncadas a segundo y dormía esa diferencia entera, con lo que la fracción
/// del lanzamiento se arrastraba intacta: dos divisiones lanzadas con medio segundo de diferencia
/// arrancaban con medio segundo de diferencia — medido, 470,314 contra 470,807 cuando las dos
/// debían salir en 470,000. Peor todavía, con `ahora` = 120,5 y `at` = 30 el objetivo truncado caía
/// **en el pasado** y esa división salía disparada mientras otra lanzada 0,5 s después esperaba
/// 29 s. Alinear mal es peor que no alinear: la desalineación queda escondida.
///
/// Con `at` a cero, ahora mismo. Si `ahora` cae exactamente en un múltiplo, es `ahora`.
pub fn start_instant(ahora: Duration, at: u64) -> Duration {
    if at == 0 {
        return ahora;
    }
    let periodo = u128::from(at) * 1_000_000_000;
    match ahora.as_nanos() % periodo {
        0 => ahora,
        resto => ahora + Duration::from_nanos((periodo - resto) as u64),
    }
}

/// Cuánto esperar antes del siguiente trozo para no superar el caudal objetivo.
///
/// `None` cuando no hay caudal objetivo o cuando ya se va por detrás de él: **nunca acelera para
/// alcanzarlo**. Si el terminal no da el caudal pedido, el caudal real sale por debajo, y eso *es*
/// el resultado de la medida.
pub fn pace_delay(
    bytes_escritos: u64,
    transcurrido: Duration,
    caudal: Option<f64>,
) -> Option<Duration> {
    let caudal = caudal?;
    if caudal <= 0.0 {
        return None;
    }
    let deberia_llevar = bytes_escritos as f64 / caudal;
    let sobra = deberia_llevar - transcurrido.as_secs_f64();
    // `Duration::from_secs_f64` entra en pánico con un valor no finito o desbordado, y un caudal
    // absurdamente pequeño (`--rate 1e-25`) lo produce. Comprobarlo aquí es lo que evita que la
    // herramienta reviente en vez de decir que no va a esperar.
    (sobra > 0.0 && sobra.is_finite() && sobra < Duration::MAX.as_secs_f64())
        .then(|| Duration::from_secs_f64(sobra))
}

/// Una medida: lo que una división escribió y cuánto tardó su terminal en tragarlo.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub tag: String,
    pub bytes: u64,
    pub seconds: f64,
    /// Instante real de arranque en segundos desde la época, **con fracción**.
    ///
    /// Guardarlo truncado a segundo es lo que hacía inútil la comprobación de solape: dos medidas
    /// desfasadas 900 ms anotaban el mismo segundo, pasaban por concurrentes y su agregado salía
    /// al doble de lo real.
    pub start: f64,
    /// El `--at` con el que se lanzó. Informativo.
    pub at: u64,
    /// Caudal objetivo en MiB/s, si se pidió. Distingue el régimen: [`aggregate`] no mezcla una
    /// medida de ráfaga con una de caudal, porque responden a preguntas distintas.
    pub rate: Option<f64>,
    /// Si la salida era un terminal de verdad. Si no lo era, la medida no mide un terminal.
    pub tty: bool,
}

/// Deja la etiqueta en una sola línea y sin tabuladores.
///
/// No es cosmético: un tabulador parte la línea en campos que no son y la medida se pierde en
/// silencio; un salto de línea permite **colar una medida inventada** en el fichero de resultados.
fn etiqueta_en_una_linea(tag: &str) -> String {
    tag.replace(['\t', '\n', '\r'], " ")
}

impl Run {
    /// Fin de la ventana de ejecución, en segundos desde la época.
    fn end(&self) -> f64 {
        self.start + self.seconds
    }

    /// Los dos regímenes no se comparan entre sí; esto es lo que los distingue.
    fn regimen(&self) -> Option<u64> {
        self.rate.map(f64::to_bits)
    }

    pub fn to_line(&self) -> String {
        format!(
            "{}\t{}\t{:.6}\t{:.6}\t{}\t{}\t{}",
            etiqueta_en_una_linea(&self.tag),
            self.bytes,
            self.seconds,
            self.start,
            self.at,
            match self.rate {
                Some(r) => format!("{r}"),
                None => "-".to_string(),
            },
            if self.tty { "si" } else { "no" }
        )
    }

    /// Relee una línea del fichero de resultados.
    ///
    /// Devuelve `None` ante cualquier cosa que no sea una medida válida — incluidos los números
    /// **no finitos**. Un `seconds` a `NaN` no se descartaba solo: `f64::max` y `f64::min` ignoran
    /// los `NaN`, así que la medida no aportaba ni al final más temprano ni a la más lenta, pero
    /// sus bytes sí entraban en la suma e **inflaban el agregado**.
    pub fn from_line(linea: &str) -> Option<Self> {
        let campos: Vec<&str> = linea.split('\t').collect();
        if campos.len() != 7 {
            return None;
        }
        let seconds: f64 = campos[2].parse().ok()?;
        let start: f64 = campos[3].parse().ok()?;
        if !seconds.is_finite() || seconds < 0.0 || !start.is_finite() || start < 0.0 {
            return None;
        }
        let rate = match campos[5] {
            "-" => None,
            otro => {
                let r: f64 = otro.parse().ok()?;
                r.is_finite().then_some(r)?;
                Some(r)
            }
        };
        Some(Self {
            tag: campos[0].to_string(),
            bytes: campos[1].parse().ok()?,
            seconds,
            start,
            at: campos[4].parse().ok()?,
            rate,
            tty: campos[6] == "si",
        })
    }

    pub fn mib_s(&self) -> f64 {
        if self.seconds <= 0.0 {
            return 0.0;
        }
        self.bytes as f64 / self.seconds / 1_048_576.0
    }
}

/// Por qué una medida se queda fuera del agregado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Excluded {
    /// La salida no era un terminal: el número mide una tubería o un fichero, no un terminal.
    NoTty,
    /// Las medidas de la etiqueta no coincidieron en el tiempo, así que nunca compitieron entre sí.
    NoOverlap,
    /// La etiqueta mezcla ráfaga con caudal sostenido, que responden a preguntas distintas.
    MixedRegime,
}

/// El agregado de una etiqueta, y lo que se ha dejado fuera al calcularlo.
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregate {
    pub included: Vec<Run>,
    pub excluded: Vec<(Run, Excluded)>,
    /// `None` cuando no hay nada agregable. No es un error: es no tener respuesta.
    pub mib_s: Option<f64>,
}

/// Agrega las medidas de una etiqueta.
///
/// El agregado es `Σ bytes / max(segundos)`. Se divide por el **máximo** y no por la media porque
/// la tanda ocupa el terminal hasta que termina la última división: con la media, cuatro divisiones
/// de 8, 12, 12 y 12 s parecerían más rápidas de lo que fueron.
///
/// **Se niega a agregar lo que no compitió**, y las tres comprobaciones vienen de errores reales:
/// una medida cuya salida no era un terminal (una tubería tragó 22 MiB a 3.010 MiB/s), medidas
/// lanzadas en ventanas distintas que nunca se solaparon, y ráfaga mezclada con caudal sostenido
/// bajo la misma etiqueta.
pub fn aggregate(runs: Vec<Run>) -> Aggregate {
    let mut excluded = Vec::new();
    let mut candidatos = Vec::new();
    for run in runs {
        if run.tty {
            candidatos.push(run);
        } else {
            excluded.push((run, Excluded::NoTty));
        }
    }

    if candidatos.is_empty() {
        return sin_agregado(excluded);
    }

    // Régimen: la mayoría manda, y lo que no encaje se declara fuera en vez de contaminar la suma.
    let regimen = candidatos[0].regimen();
    if candidatos.iter().any(|r| r.regimen() != regimen) {
        excluded.extend(candidatos.into_iter().map(|r| (r, Excluded::MixedRegime)));
        return sin_agregado(excluded);
    }

    // Solapan todas si el arranque más tardío cae antes del final más temprano. Con una sola medida
    // no hay nada con lo que solapar, así que la comprobación no aplica — comprobarla igualmente
    // descartaba una medida legítima de duración cero.
    if candidatos.len() > 1 {
        let arranque_mas_tardio = candidatos.iter().map(|r| r.start).fold(f64::MIN, f64::max);
        let final_mas_temprano = candidatos.iter().map(Run::end).fold(f64::MAX, f64::min);
        if arranque_mas_tardio >= final_mas_temprano {
            excluded.extend(candidatos.into_iter().map(|r| (r, Excluded::NoOverlap)));
            return sin_agregado(excluded);
        }
    }

    let bytes: u64 = candidatos.iter().map(|r| r.bytes).sum();
    let mas_lenta = candidatos.iter().map(|r| r.seconds).fold(0.0, f64::max);
    let mib_s = (mas_lenta > 0.0).then(|| bytes as f64 / mas_lenta / 1_048_576.0);

    Aggregate {
        included: candidatos,
        excluded,
        mib_s,
    }
}

fn sin_agregado(excluded: Vec<(Run, Excluded)>) -> Aggregate {
    Aggregate {
        included: Vec::new(),
        excluded,
        mib_s: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(bytes: u64, seconds: f64, start: f64, tty: bool) -> Run {
        Run {
            tag: "t".into(),
            bytes,
            seconds,
            start,
            at: 30,
            rate: None,
            tty,
        }
    }

    #[test]
    fn la_carga_es_identica_en_dos_pasadas() {
        assert_eq!(build_load(200_000), build_load(200_000));
    }

    #[test]
    fn la_carga_lleva_el_patron_que_no_toma_el_camino_rapido_del_parser() {
        let carga = build_load(200_000);
        let texto = String::from_utf8_lossy(&carga);

        assert!(texto.contains("\x1b[1m"), "falta la negrita");
        assert!(texto.contains("\x1b[2m"), "falta el atenuado");
        assert!(texto.contains("\x1b[32m"), "falta el color");
        assert!(
            texto.contains("\x1b7") && texto.contains("\x1b8"),
            "falta el repintado que guarda y restaura el cursor"
        );
        assert!(
            texto.contains("\x1b[7m"),
            "falta el vídeo inverso de la barra"
        );
    }

    #[test]
    fn la_carga_llega_al_tamano_pedido_sin_cortar_a_mitad() {
        for objetivo in [1_000, 100_000, 1_000_000] {
            let carga = build_load(objetivo);
            assert!(carga.len() >= objetivo, "se quedó corta con {objetivo}");
            // Toda unidad que se añade acaba en salto de línea o en el `\x1b8` que restaura el
            // cursor: si acabara en otra cosa, se habría cortado una secuencia de escape a mitad.
            let ultimo = *carga.last().expect("carga vacía");
            assert!(
                ultimo == b'\n' || ultimo == b'8',
                "la carga de {objetivo} acabó a mitad, en el byte {ultimo}"
            );
        }
    }

    #[test]
    fn el_instante_alineado_es_el_siguiente_multiplo() {
        let alineado = start_instant(Duration::from_secs(1_000_001), 30);
        assert_eq!(alineado, Duration::from_secs(1_000_020));
    }

    #[test]
    fn si_ya_esta_alineado_no_espera_un_ciclo_de_mas() {
        let alineado = start_instant(Duration::from_secs(1_000_020), 30);
        assert_eq!(alineado, Duration::from_secs(1_000_020));
    }

    #[test]
    fn con_at_cero_arranca_ya() {
        let ahora = Duration::from_millis(1_000_001_500);
        assert_eq!(start_instant(ahora, 0), ahora);
    }

    #[test]
    fn dos_divisiones_lanzadas_con_desfase_arrancan_en_el_mismo_instante() {
        // El fallo que este cálculo existe para evitar: con aritmética de segundos enteros, una
        // lanzada en 1.000.000,05 y otra en 1.000.000,95 arrancaban con 900 ms de diferencia.
        let a = start_instant(Duration::from_micros(1_000_000_050_000), 30);
        let b = start_instant(Duration::from_micros(1_000_000_950_000), 30);
        assert_eq!(a, b, "las dos tienen que salir en el mismo instante");
        assert_eq!(a, Duration::from_secs(1_000_020));
    }

    #[test]
    fn un_instante_fraccionario_ya_pasado_el_multiplo_espera_al_siguiente() {
        // Con truncado a segundo, 1.000.020,5 daba objetivo 1.000.020 —en el pasado— y esa
        // división salía disparada mientras otra lanzada medio segundo después esperaba 29 s.
        let alineado = start_instant(Duration::from_micros(1_000_020_500_000), 30);
        assert_eq!(alineado, Duration::from_secs(1_000_050));
    }

    #[test]
    fn el_caudal_espera_lo_que_le_sobra() {
        // 1 MiB escrito con un objetivo de 1 MiB/s debería llevar 1 s; lleva 0,25 → espera 0,75.
        let espera = pace_delay(1_048_576, Duration::from_millis(250), Some(1_048_576.0))
            .expect("debería esperar");
        assert!(
            (espera.as_secs_f64() - 0.75).abs() < 1e-6,
            "esperó {espera:?}"
        );
    }

    #[test]
    fn el_caudal_nunca_acelera_para_recuperar() {
        // Va por detrás del objetivo: no hay nada que esperar, y tampoco se compensa.
        assert_eq!(
            pace_delay(1_048_576, Duration::from_secs(3), Some(1_048_576.0)),
            None
        );
    }

    #[test]
    fn sin_caudal_objetivo_no_hay_espera() {
        assert_eq!(pace_delay(1_048_576, Duration::from_millis(1), None), None);
    }

    #[test]
    fn un_caudal_absurdo_no_hace_reventar_la_herramienta() {
        // Antes de la comprobación de finitud, esto entraba en pánico dentro de
        // `Duration::from_secs_f64`: la espera calculada se iba a infinito.
        for absurdo in [
            f64::MIN_POSITIVE,
            1e-320,
            1e-25,
            -1.0,
            0.0,
            f64::NAN,
            f64::INFINITY,
        ] {
            let _ = pace_delay(1_048_576, Duration::from_millis(1), Some(absurdo));
        }
    }

    #[test]
    fn la_linea_de_resultados_va_y_vuelve_igual() {
        let mut original = run(22_700_000, 1.234_567, 1_800_000_000.5, true);
        original.rate = Some(10.0);
        let vuelta = Run::from_line(&original.to_line()).expect("no se pudo releer la línea");
        assert_eq!(original.tag, vuelta.tag);
        assert_eq!(original.bytes, vuelta.bytes);
        assert_eq!(original.rate, vuelta.rate);
        assert_eq!(original.tty, vuelta.tty);
        assert!((original.seconds - vuelta.seconds).abs() < 1e-6);
        assert!(
            (original.start - vuelta.start).abs() < 1e-6,
            "se perdió la fracción del instante de arranque"
        );
    }

    #[test]
    fn una_etiqueta_con_tabulador_o_salto_no_parte_la_linea() {
        let mut r = run(1, 1.0, 0.0, true);
        r.tag = "con\ttabulador\ny salto".into();
        let linea = r.to_line();
        assert_eq!(linea.split('\t').count(), 7);
        assert!(
            !linea.contains('\n'),
            "un salto permitiría colar una medida inventada"
        );
    }

    #[test]
    fn una_medida_con_segundos_no_finitos_se_rechaza_al_releerla() {
        // No basta con ignorarla en los máximos y mínimos: sus bytes entrarían en la suma e
        // inflarían el agregado.
        for veneno in ["NaN", "inf", "-1"] {
            let linea = format!("t\t1048576\t{veneno}\t100.0\t30\t-\tsi");
            assert!(
                Run::from_line(&linea).is_none(),
                "debería rechazarse: {linea}"
            );
        }
    }

    #[test]
    fn el_agregado_divide_por_la_division_mas_lenta() {
        // Cuatro divisiones solapadas de 1 MiB: la tanda dura lo que la más lenta, 2 s.
        let mib = 1_048_576;
        let runs = vec![
            run(mib, 1.0, 100.0, true),
            run(mib, 2.0, 100.0, true),
            run(mib, 1.5, 100.0, true),
            run(mib, 1.2, 100.0, true),
        ];
        let a = aggregate(runs);
        assert_eq!(a.included.len(), 4);
        assert!(a.excluded.is_empty());
        assert!((a.mib_s.expect("sin agregado") - 2.0).abs() < 1e-9);
    }

    #[test]
    fn no_agrega_medidas_que_nunca_coincidieron_en_el_tiempo() {
        // La segunda arranca cuando la primera ya terminó: nunca compitieron.
        let runs = vec![
            run(1_048_576, 1.0, 100.0, true),
            run(1_048_576, 1.0, 500.0, true),
        ];
        let a = aggregate(runs);
        assert!(a.mib_s.is_none(), "no debería haber agregado");
        assert!(a.included.is_empty());
        assert_eq!(a.excluded.len(), 2);
        assert!(
            a.excluded
                .iter()
                .all(|(_, por)| *por == Excluded::NoOverlap)
        );
    }

    #[test]
    fn detecta_el_desfase_de_menos_de_un_segundo() {
        // El caso que el instante truncado escondía: 900 ms de desfase con medidas de 0,8 s.
        let runs = vec![
            run(1_048_576, 0.8, 100.05, true),
            run(1_048_576, 0.8, 100.95, true),
        ];
        let a = aggregate(runs);
        assert!(
            a.mib_s.is_none(),
            "no se solaparon: el agregado habría salido al doble"
        );
    }

    #[test]
    fn excluye_del_agregado_lo_que_no_salio_por_un_terminal() {
        let runs = vec![
            run(1_048_576, 1.0, 100.0, true),
            run(1_048_576, 0.001, 100.0, false),
        ];
        let a = aggregate(runs);
        assert_eq!(a.included.len(), 1);
        assert_eq!(
            a.excluded,
            vec![(run(1_048_576, 0.001, 100.0, false), Excluded::NoTty)]
        );
        assert!((a.mib_s.expect("sin agregado") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn no_mezcla_rafaga_con_caudal_sostenido() {
        let rafaga = run(1_048_576, 1.0, 100.0, true);
        let mut sostenida = run(1_048_576, 1.0, 100.0, true);
        sostenida.rate = Some(10.0);
        let a = aggregate(vec![rafaga, sostenida]);
        assert!(a.mib_s.is_none(), "responden a preguntas distintas");
        assert!(
            a.excluded
                .iter()
                .all(|(_, por)| *por == Excluded::MixedRegime)
        );
    }

    #[test]
    fn una_etiqueta_sin_resultados_no_es_un_error() {
        let a = aggregate(Vec::new());
        assert!(a.mib_s.is_none());
        assert!(a.included.is_empty());
        assert!(a.excluded.is_empty());
    }

    #[test]
    fn una_sola_division_no_tiene_con_quien_solapar() {
        // Incluida la de duración cero, que la comprobación de solape descartaba por comparar una
        // medida consigo misma.
        for segundos in [1.0, 0.0] {
            let a = aggregate(vec![run(1_048_576, segundos, 100.0, true)]);
            assert_eq!(a.included.len(), 1, "con seconds = {segundos}");
        }
    }
}
