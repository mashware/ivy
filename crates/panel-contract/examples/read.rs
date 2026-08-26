//! Mide lo que cuesta leer un `panel.json`: tiempo por llamada y **pico de memoria al interpretar**.
//!
//! **Ata los números que el código afirma**: el pico y el tiempo que [`panel_contract::MAX_BYTES`]
//! declara para un fichero en el techo, y el coste del caso que va a ocurrir de verdad —los
//! `panel.json` que hay en el repo—. La versión anterior de ese comentario decía «10 MiB, unas
//! cuarenta veces el fichero» y nadie podía volver a comprobarlo: era 7× con líneas realistas y 95×
//! en el peor caso. Un número escrito en el código sin con qué reejecutarlo envejece mal, y este
//! ejemplo existe para que no vuelva a pasar.
//!
//! **Lo que midió y no está aquí**, a propósito: las llamadas al sistema por lectura, el coste de
//! comprobar que los bytes son texto, y la comparación contra lo que `serde` derivaría. Son
//! diagnóstico de una sola vez —sus respuestas ya viven en el comentario de [`panel_contract::read`]
//! y en el diseño—, no números que el código afirme, y la última obligaba a arrastrar tres tipos
//! duplicados del contrato que nadie recordaría actualizar.
//!
//! ```text
//! cargo run --release --example read -p panel-contract
//! ```
//!
//! En depuración los números salen varias veces peores; los que valen son los de `--release`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use panel_contract::{MAX_BYTES, read};

// ─── El contador de memoria ────────────────────────────────────────────────────────────────────
//
// El pico no se puede sacar de fuera del proceso: `/usr/bin/time` da el máximo residente de toda la
// corrida, que aquí incluye los ficheros de prueba. Hace falta contar lo vivo en cada instante.

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct Counter;

unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Relaxed);
            add(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            REALLOCATIONS.fetch_add(1, Relaxed);
            LIVE.fetch_sub(layout.size(), Relaxed);
            add(new_size);
        }
        moved
    }
}

fn add(amount: usize) {
    let live = LIVE.fetch_add(amount, Relaxed) + amount;
    PEAK.fetch_max(live, Relaxed);
}

#[global_allocator]
static ALLOCATOR: Counter = Counter;

/// Lo que costó una llamada, en memoria.
struct Usage {
    peak: usize,
    allocations: usize,
    reallocations: usize,
}

/// Ejecuta midiendo, y **devuelve lo producido vivo**: el pico incluye lo que la llamada deja hecho.
fn while_measuring<T>(work: impl FnOnce() -> T) -> (T, Usage) {
    let baseline = LIVE.load(Relaxed);
    PEAK.store(baseline, Relaxed);
    ALLOCATIONS.store(0, Relaxed);
    REALLOCATIONS.store(0, Relaxed);

    let produced = work();

    let usage = Usage {
        peak: PEAK.load(Relaxed).saturating_sub(baseline),
        allocations: ALLOCATIONS.load(Relaxed),
        reallocations: REALLOCATIONS.load(Relaxed),
    };
    (produced, usage)
}

// ─── Las medidas ───────────────────────────────────────────────────────────────────────────────

fn main() {
    let bench = Bench::new();

    println!("== 1. Los `panel.json` de verdad del repo ==\n");
    real_files(&bench);

    println!("\n== 2. Un fichero justo en el techo, en cuatro formas ==\n");
    println!("Con los mismos bytes, lo que cambia el pico es cuántos valores traen: se reserva un");
    println!("nodo por cada uno.\n");
    for filler in fillers_at_the_cap() {
        at_the_cap(&bench, &filler);
    }

    println!("\n== 3. Lo que reserva la lectura de bytes ==\n");
    allocations_of_a_read(&bench);
}

/// Un directorio temporal donde escribir lo que se va a leer.
///
/// No se usa `PanelDir` a propósito: sortea un nombre y borra al morir, y aquí hace falta un sitio
/// estable donde reescribir el mismo fichero muchas veces.
struct Bench {
    dir: PathBuf,
}

impl Bench {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("panel-lectura-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("crear el banco");
        Self { dir }
    }

    /// Publica un contenido y devuelve el directorio que `read` espera.
    fn publish(&self, contents: &str) -> &Path {
        std::fs::write(self.dir.join("panel.json"), contents).expect("publicar");
        &self.dir
    }

    fn path(&self) -> PathBuf {
        self.dir.join("panel.json")
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Los nueve ficheros que hay en el repo, uno a uno.
fn real_files(bench: &Bench) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("la raíz del repo")
        .to_path_buf();

    let mut real = Vec::new();
    collect_into(&root.join(".claude/work"), &mut real);
    collect_into(&root.join("prototipos"), &mut real);
    real.sort();

    if real.is_empty() {
        println!("  (no se encontró ningún `panel.json` en el repo)");
        return;
    }

    println!(
        "  {:<46} {:>6} {:>4} {:>8} {:>9} {:>9} {:>7}",
        "fichero", "bytes", "lín", "µs read", "de ellos", "pico", "factor"
    );
    println!(
        "  {:<46} {:>6} {:>4} {:>8} {:>9} {:>9} {:>7}",
        "", "", "", "", "syscalls", "", ""
    );

    let mut total = 0.0;
    for path in &real {
        let contents = std::fs::read_to_string(path).expect("leer el fichero real");
        let dir = bench.publish(&contents);

        // Una pasada en frío antes de medir: la primera lectura paga el caché del directorio.
        let _ = read(dir);

        const ROUNDS: u32 = 5000;
        let start = Instant::now();
        for _ in 0..ROUNDS {
            std::hint::black_box(read(std::hint::black_box(dir)).ok());
        }
        let micros = start.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS);
        total += micros;

        // Lo mismo sin interpretar: metadata + open + read_to_end, que es el suelo irreductible.
        let file = bench.path();
        let start = Instant::now();
        for _ in 0..ROUNDS {
            std::hint::black_box(std::fs::metadata(&file).ok());
            std::hint::black_box(std::fs::read(&file).ok());
        }
        let io = start.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS);

        let (panel, usage) =
            while_measuring(|| read(dir).expect("un fichero real del repo se lee"));

        println!(
            "  {:<46} {:>6} {:>4} {:>8.1} {:>9.1} {:>9} {:>6.1}×",
            short(path, &root),
            contents.len(),
            panel.lines.len(),
            micros,
            io,
            human(usage.peak),
            usage.peak as f64 / contents.len() as f64
        );
    }

    println!(
        "\n  Media: {:.1} µs por lectura. A 60 fotogramas por segundo, releer los nueve en cada uno",
        total / real.len() as f64
    );
    println!(
        "  costaría {:.2} ms de los 16,7 del fotograma.",
        total / 1000.0
    );
}

fn collect_into(from: &Path, en: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_into(&path, en);
        } else if path.file_name().is_some_and(|n| n == "panel.json") {
            en.push(path);
        }
    }
}

fn short(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Una forma de llenar el techo, con su nombre.
struct Filler {
    name: &'static str,
    text: String,
}

/// Tres maneras de gastar 256 KiB, de la más realista a los dos extremos.
fn fillers_at_the_cap() -> Vec<Filler> {
    // La forma que tiene un panel de verdad: objetos con referencia, texto, marca, estilo y enlace.
    let real_line = r##"{"ref": "#12", "text": "extraer Division de la ventana y dejarla sola", "mark": "done", "style": "ok", "link": "https://github.com/mashware/agent-terminal/pull/31"}"##;
    // El peor caso por número de nodos: el objeto más pequeño que sigue siendo una línea.
    let minimal_line = r#"{"text":"x"}"#;

    vec![
        Filler {
            name: "líneas como las de un panel real",
            text: repeat(real_line),
        },
        Filler {
            name: "objetos mínimos del contrato (más nodos por byte)",
            text: repeat(minimal_line),
        },
        // El peor caso de verdad: ni siquiera hace falta que el campo exista en el contrato. Basta
        // con que el objeto tenga **una** clave para que su mapa reserve un nodo entero de árbol.
        Filler {
            name: "objetos de una clave cualquiera (el peor caso)",
            text: repeat(r#"{"a":0}"#),
        },
        Filler {
            name: "una sola línea gigante",
            text: {
                let padding = "x".repeat(MAX_BYTES as usize - 32);
                format!(r#"{{"lines":["{padding}"]}}"#)
            },
        },
    ]
}

/// Repite un elemento hasta rozar el techo por debajo.
fn repeat(item: &str) -> String {
    let mut text = String::from(r#"{"lines":["#);
    let closing = "]}";
    while text.len() + item.len() + 1 + closing.len() < MAX_BYTES as usize {
        text.push_str(item);
        text.push(',');
    }
    text.pop();
    text.push_str(closing);
    text
}

fn at_the_cap(bench: &Bench, filler: &Filler) {
    let dir = bench.publish(&filler.text);
    let _ = read(dir);

    const ROUNDS: u32 = 50;
    let start = Instant::now();
    for _ in 0..ROUNDS {
        std::hint::black_box(read(std::hint::black_box(dir)).ok());
    }
    let millis = start.elapsed().as_secs_f64() * 1e3 / f64::from(ROUNDS);

    let before = LIVE.load(Relaxed);
    let (panel, usage) = while_measuring(|| read(dir).expect("en el techo todavía se lee"));
    // Lo que **queda** vivo con el panel en la mano: el pico es un instante, esto se paga mientras
    // la división exista.
    let resident = LIVE.load(Relaxed).saturating_sub(before);

    // Cuánto de ese tiempo es el disco y no el intérprete.
    let path = bench.path();
    let start = Instant::now();
    for _ in 0..ROUNDS {
        std::hint::black_box(std::fs::read(&path).expect("leer los bytes"));
    }
    let io = start.elapsed().as_secs_f64() * 1e3 / f64::from(ROUNDS);

    println!("  {}", filler.name);
    println!(
        "    {} bytes · {} líneas leídas",
        filler.text.len(),
        panel.lines.len()
    );
    println!(
        "    tiempo   {millis:.2} ms  (de los que {io:.2} ms son leer los bytes → {:.2} ms interpretar)",
        millis - io
    );
    println!(
        "    pico     {}  = {:.1}× el fichero   ({} reservas, {} reasignaciones)",
        human(usage.peak),
        usage.peak as f64 / filler.text.len() as f64,
        usage.allocations,
        usage.reallocations
    );
    println!(
        "    queda    {} vivos con el panel en la mano ({:.0} % del pico)",
        human(resident),
        resident as f64 * 100.0 / usage.peak as f64
    );
    // Cuatro divisiones **una detrás de otra en el mismo hilo**, que es como ocurriría: el pico no
    // se multiplica por cuatro, porque el intermedio de cada lectura muere antes de la siguiente.
    let before = LIVE.load(Relaxed);
    PEAK.store(before, Relaxed);
    let four: Vec<_> = (0..4).map(|_| read(dir).expect("leer")).collect();
    let peak_of_four = PEAK.load(Relaxed).saturating_sub(before);
    std::hint::black_box(&four);
    drop(four);

    println!(
        "    cuatro divisiones seguidas: {} de pico — no {}, porque los intermedios no coinciden\n",
        human(peak_of_four),
        human(usage.peak * 4)
    );
}

/// Las dos formas de acumular los bytes leídos, replicadas y contadas.
///
/// **Es una comparación, no una copia de `read_capped`**: lo que ata es *por qué* esa función dice
/// de antemano cuánto espera. Sin la pista, `read_to_end` lee tanto como quepa en lo reservado
/// —empezando por 32 bytes—, así que cada duplicación del búfer es una llamada al sistema más: siete
/// lecturas para un panel de 861 B y quince para uno en el techo, contadas con `strace`. Con la
/// pista son dos, pase lo que pase, y la capacidad final deja de ser el doble del fichero.
fn allocations_of_a_read(bench: &Bench) {
    println!("  Acumular los bytes sin decir de antemano cuántos se esperan, contra decirlo.");
    println!(
        "\n  {:<24} {:>9} {:>10} {:>16}",
        "fichero", "reservas", "reasign.", "capacidad final"
    );

    for (name, contents) in [
        ("uno real (861 B)", "x".repeat(861)),
        ("16 KiB", "x".repeat(16 * 1024)),
        ("en el techo", "x".repeat(MAX_BYTES as usize)),
    ] {
        bench.publish(&contents);
        let path = bench.path();
        let (bytes, usage) = while_measuring(|| {
            let file = std::fs::File::open(&path).expect("abrir");
            let mut bytes = Vec::new();
            file.take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .expect("leer");
            bytes
        });

        // Lo mismo diciendo de antemano cuánto se espera, acotado al techo. El tamaño no cuesta
        // nada: quien lee acaba de pedir los metadatos para comprobar que es un fichero regular. Y
        // no decide nada — el techo lo siguen aplicando el `take` y la comprobación de después, así
        // que un fichero que crezca entre medias no rompe nada: el `Vec` crece solo.
        let expected = std::fs::metadata(&path).expect("metadatos").len();
        let (with_capacity, usage_with) = while_measuring(|| {
            let file = std::fs::File::open(&path).expect("abrir");
            let mut bytes = Vec::with_capacity(expected.min(MAX_BYTES) as usize + 1);
            file.take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .expect("leer");
            bytes
        });

        println!(
            "  {:<24} {:>9} {:>10} {:>16}   →   con capacidad previa: {} + {}, {}",
            name,
            usage.allocations,
            usage.reallocations,
            human(bytes.capacity()),
            usage_with.allocations,
            usage_with.reallocations,
            human(with_capacity.capacity())
        );
    }
}

fn human(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
