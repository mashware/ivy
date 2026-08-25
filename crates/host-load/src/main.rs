//! Órdenes de `host-load`. El protocolo de medida, los números de referencia y —sobre todo— el
//! límite del instrumento están en la documentación de la caja (`lib.rs`).
//!
//! ```text
//! host-load run    [--mib N] [--at S] [--rate M] [--tag T] [--results RUTA] [--dump RUTA]
//! host-load report [--tag T] [--results RUTA]
//! ```

use std::fs::OpenOptions;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use host_load::{
    CHUNK, Excluded, Run, aggregate, build_load, build_load_cjk, build_load_color, pace_delay,
    start_instant,
};

type Error = Box<dyn std::error::Error + Send + Sync>;

const MIB_POR_DEFECTO: usize = 22;
const AT_POR_DEFECTO: u64 = 30;
const UN_MIB: usize = 1_048_576;

const USO: &str = "\
host-load — mide cuánto tarda un terminal en vaciar una carga fija

  host-load run    [--mib N] [--at S] [--rate M] [--tag T] [--results RUTA] [--dump RUTA]
  host-load report [--tag T] [--results RUTA]

  --mib N       tamaño de la carga en MiB (por defecto 22)
  --cjk         escribe en japonés, chino y coreano en vez de en ASCII
  --emoji       salpica la carga de emojis, con y sin selector de presentación
  --at S        arrancar en el siguiente múltiplo de S segundos (por defecto 30; 0 = ya)
  --rate M      caudal sostenido objetivo en MiB/s (por defecto, tan rápido como admita)
  --tag T       etiqueta de la tanda; report agrupa por ella
  --results R   fichero de resultados (por defecto, uno fijo en el directorio temporal)
  --dump R      escribe la carga a un fichero y sale, para comparar bytes entre máquinas
";

fn main() -> ExitCode {
    match ejecutar() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("host-load: {e}");
            ExitCode::FAILURE
        }
    }
}

fn ejecutar() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("report") => report(&args[1..]),
        Some("--help" | "-h") | None => {
            print!("{USO}");
            Ok(())
        }
        Some(otro) => Err(format!("orden desconocida «{otro}»\n\n{USO}").into()),
    }
}

/// Argumentos de la línea de órdenes.
///
/// Se parsean a mano y no con una caja de terceros: son seis opciones y esta caja no tiene ninguna
/// dependencia, lo que la hace compilable en cualquier sitio donde haya que medir.
struct Opciones {
    mib: usize,
    /// Si la carga se escribe en CJK. Solo la pide la tanda del descubrimiento de fuentes.
    cjk: bool,
    emoji: bool,
    at: u64,
    rate: Option<f64>,
    tag: String,
    results: std::path::PathBuf,
    dump: Option<std::path::PathBuf>,
}

fn parsear(args: &[String]) -> Result<Opciones, Error> {
    let mut o = Opciones {
        mib: MIB_POR_DEFECTO,
        cjk: false,
        emoji: false,
        at: AT_POR_DEFECTO,
        rate: None,
        tag: "sin-etiqueta".to_string(),
        // Ruta **absoluta** a propósito: las N divisiones pueden estar en directorios distintos y
        // una ruta relativa haría que cada una escribiera en su propio fichero, con lo que el
        // agregado saldría de una sola medida sin que nadie lo notara.
        //
        // Y con el usuario dentro del nombre: el directorio temporal es compartido, así que dos
        // personas midiendo a la vez en la misma máquina mezclarían sus medidas bajo la misma
        // etiqueta sin enterarse.
        results: std::env::temp_dir().join(format!(
            "host-load-{}.tsv",
            std::env::var("USER").unwrap_or_else(|_| "anon".to_string())
        )),
        dump: None,
    };

    let mut i = 0;
    while i < args.len() {
        let valor = |i: usize| -> Result<&String, Error> {
            args.get(i + 1)
                .ok_or_else(|| format!("«{}» necesita un valor", args[i]).into())
        };
        match args[i].as_str() {
            "--mib" => o.mib = valor(i)?.parse()?,
            "--cjk" => o.cjk = true,
            "--emoji" => o.emoji = true,
            "--at" => o.at = valor(i)?.parse()?,
            "--rate" => o.rate = Some(valor(i)?.parse()?),
            "--tag" => o.tag = valor(i)?.clone(),
            "--results" => o.results = valor(i)?.into(),
            "--dump" => o.dump = Some(valor(i)?.into()),
            otro => return Err(format!("opción desconocida «{otro}»\n\n{USO}").into()),
        }
        i += 2;
    }
    Ok(o)
}

/// Ahora, desde la época, **con fracción de segundo**.
///
/// La fracción es lo que hace que N divisiones arranquen de verdad a la vez; truncarla aquí fue el
/// fallo que dejaba pasar por concurrentes dos medidas desfasadas casi un segundo.
fn ahora() -> Result<Duration, Error> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?)
}

fn run(args: &[String]) -> Result<(), Error> {
    let o = parsear(args)?;
    // Sin `checked_mul`, un `--mib` disparatado desbordaría en silencio al compilar en release y
    // mediría un tamaño que no es el pedido.
    let bytes = o
        .mib
        .checked_mul(UN_MIB)
        .ok_or_else(|| format!("--mib {} es demasiado grande", o.mib))?;
    // **Las cargas en CJK y con emojis solo las piden las tandas de las fuentes.** La de siempre no se
    // toca, para que las tandas nuevas sigan siendo comparables con los anexos publicados.
    //
    // **Y las dos banderas juntas no significan nada**, así que se rechaza en vez de elegir una en
    // silencio: una tanda lanzada con las dos publicaría el coste de un idioma creyendo medir el otro.
    let carga = match (o.cjk, o.emoji) {
        (true, true) => {
            return Err("--cjk y --emoji miden cosas distintas: hay que elegir una".into());
        }
        (true, false) => build_load_cjk(bytes),
        (false, true) => build_load_color(bytes),
        (false, false) => build_load(bytes),
    };

    if let Some(ruta) = o.dump {
        std::fs::write(&ruta, &carga)?;
        eprintln!(
            "carga de {} bytes escrita en {}",
            carga.len(),
            ruta.display()
        );
        return Ok(());
    }

    // Si la salida no es un terminal, esto no mide un terminal. No se aborta —redirigir es un uso
    // legítimo para comprobar que la herramienta funciona— pero queda marcado en el resultado, que
    // es lo que hace que `report` no lo cuele en el agregado.
    let tty = std::io::stdout().is_terminal();
    if !tty {
        eprintln!("host-load: la salida no es un terminal; esta medida NO mide un terminal");
    }

    let objetivo = start_instant(ahora()?, o.at);
    if let Some(espera) = objetivo.checked_sub(ahora()?)
        && !espera.is_zero()
    {
        eprintln!(
            "host-load: arrancando en {:.1} s (lanza el resto de divisiones ahora)",
            espera.as_secs_f64()
        );
        thread::sleep(espera);
    }

    let caudal = o.rate.map(|mib_s| mib_s * UN_MIB as f64);
    let salida = std::io::stdout();
    let mut salida = salida.lock();

    let comienzo_real = ahora()?.as_secs_f64();
    let reloj = Instant::now();
    let mut escritos: u64 = 0;
    for trozo in carga.chunks(CHUNK) {
        salida.write_all(trozo)?;
        escritos += trozo.len() as u64;
        if let Some(espera) = pace_delay(escritos, reloj.elapsed(), caudal) {
            salida.flush()?;
            thread::sleep(espera);
        }
    }
    salida.flush()?;
    let transcurrido = reloj.elapsed().as_secs_f64();

    let medida = Run {
        tag: o.tag,
        bytes: carga.len() as u64,
        seconds: transcurrido,
        start: comienzo_real,
        at: o.at,
        rate: o.rate,
        tty,
    };

    // El resumen sale por el error estándar: por la salida estándar formaría parte de la carga del
    // siguiente que redirija, y aquí ya no hay nada que medir.
    eprintln!(
        "\n── {:.2} s · {:.1} MiB/s · {:.1} MiB · etiqueta {} ──",
        medida.seconds,
        medida.mib_s(),
        medida.bytes as f64 / UN_MIB as f64,
        medida.tag
    );

    let mut fichero = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&o.results)?;
    // Una sola escritura, salto de línea incluido. `writeln!` emite dos —la línea y el `\n`— y
    // `O_APPEND` solo hace atómica cada llamada, no el par: con N divisiones terminando a la vez,
    // dos líneas se intercalarían y las dos se perderían al releerlas.
    fichero.write_all(format!("{}\n", medida.to_line()).as_bytes())?;
    eprintln!("anotado en {}", o.results.display());
    Ok(())
}

fn report(args: &[String]) -> Result<(), Error> {
    let o = parsear(args)?;
    // Que el fichero no exista todavía es normal: aún no se ha medido nada. Cualquier **otro**
    // fallo de lectura sí se propaga — tratarlos todos como «vacío» convertiría un problema de
    // permisos en un «sin resultados» que se lee como si de verdad no hubiera medidas.
    let contenido = match std::fs::read_to_string(&o.results) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("no se pudo leer {}: {e}", o.results.display()).into()),
    };

    // Las líneas ilegibles se cuentan y se dicen. Descartarlas en silencio hacía indistinguible
    // «solo hubo 3 medidas» de «hubo 4 y una se corrompió», que es justo lo que hay que saber.
    let mut ilegibles = 0usize;
    let mut medidas = Vec::new();
    for linea in contenido.lines().filter(|l| !l.trim().is_empty()) {
        match Run::from_line(linea) {
            Some(r) if r.tag == o.tag => medidas.push(r),
            Some(_) => {}
            None => ilegibles += 1,
        }
    }

    if ilegibles > 0 {
        println!(
            "aviso: {ilegibles} línea(s) del fichero no se pudieron leer (formato antiguo o escritura corrupta)"
        );
    }

    if medidas.is_empty() {
        println!("sin resultados para la etiqueta «{}»", o.tag);
        return Ok(());
    }

    let n = medidas.len();
    let resultado = aggregate(medidas);

    println!("etiqueta «{}» — {n} medida(s)", o.tag);
    for r in &resultado.included {
        println!("  {:>7.2} s · {:>7.1} MiB/s", r.seconds, r.mib_s());
    }
    for (r, por) in &resultado.excluded {
        let motivo = match por {
            Excluded::NoTty => "fuera: la salida no era un terminal",
            Excluded::NoOverlap => "fuera: no coincidió en el tiempo con las demás",
            Excluded::MixedRegime => "fuera: la etiqueta mezcla ráfaga con caudal sostenido",
        };
        println!(
            "  {:>7.2} s · {:>7.1} MiB/s  ← {motivo}",
            r.seconds,
            r.mib_s()
        );
    }

    match resultado.mib_s {
        Some(agregado) => println!(
            "\nagregado: {:.1} MiB/s sobre {} división(es)",
            agregado,
            resultado.included.len()
        ),
        None => println!(
            "\nsin agregado: no hay medidas comparables en esta etiqueta.\n\
             Repite la tanda lanzando todas las divisiones dentro de la misma ventana de --at,\n\
             con el mismo régimen (todas con --rate o ninguna)."
        ),
    }
    Ok(())
}
