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
//! cargo run --release --example lectura -p panel-contract
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

static VIVOS: AtomicUsize = AtomicUsize::new(0);
static PICO: AtomicUsize = AtomicUsize::new(0);
static RESERVAS: AtomicUsize = AtomicUsize::new(0);
static REASIGNACIONES: AtomicUsize = AtomicUsize::new(0);

struct Contador;

unsafe impl GlobalAlloc for Contador {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let puntero = unsafe { System.alloc(layout) };
        if !puntero.is_null() {
            RESERVAS.fetch_add(1, Relaxed);
            subir(layout.size());
        }
        puntero
    }

    unsafe fn dealloc(&self, puntero: *mut u8, layout: Layout) {
        VIVOS.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(puntero, layout) }
    }

    unsafe fn realloc(&self, puntero: *mut u8, layout: Layout, nuevo: usize) -> *mut u8 {
        let movido = unsafe { System.realloc(puntero, layout, nuevo) };
        if !movido.is_null() {
            REASIGNACIONES.fetch_add(1, Relaxed);
            VIVOS.fetch_sub(layout.size(), Relaxed);
            subir(nuevo);
        }
        movido
    }
}

fn subir(cuanto: usize) {
    let vivos = VIVOS.fetch_add(cuanto, Relaxed) + cuanto;
    PICO.fetch_max(vivos, Relaxed);
}

#[global_allocator]
static ALOCADOR: Contador = Contador;

/// Lo que costó una llamada, en memoria.
struct Uso {
    pico: usize,
    reservas: usize,
    reasignaciones: usize,
}

/// Ejecuta midiendo, y **devuelve lo producido vivo**: el pico incluye lo que la llamada deja hecho.
fn midiendo<T>(trabajo: impl FnOnce() -> T) -> (T, Uso) {
    let base = VIVOS.load(Relaxed);
    PICO.store(base, Relaxed);
    RESERVAS.store(0, Relaxed);
    REASIGNACIONES.store(0, Relaxed);

    let hecho = trabajo();

    let uso = Uso {
        pico: PICO.load(Relaxed).saturating_sub(base),
        reservas: RESERVAS.load(Relaxed),
        reasignaciones: REASIGNACIONES.load(Relaxed),
    };
    (hecho, uso)
}

// ─── Las medidas ───────────────────────────────────────────────────────────────────────────────

fn main() {
    let banco = Banco::nuevo();

    println!("== 1. Los `panel.json` de verdad del repo ==\n");
    caso_normal(&banco);

    println!("\n== 2. Un fichero justo en el techo, en cuatro formas ==\n");
    println!("Con los mismos bytes, lo que cambia el pico es cuántos valores traen: se reserva un");
    println!("nodo por cada uno.\n");
    for forma in formas_en_el_techo() {
        en_el_techo(&banco, &forma);
    }

    println!("\n== 3. Lo que reserva la lectura de bytes ==\n");
    reservas_de_la_lectura(&banco);
}

/// Un directorio temporal donde escribir lo que se va a leer.
///
/// No se usa `PanelDir` a propósito: sortea un nombre y borra al morir, y aquí hace falta un sitio
/// estable donde reescribir el mismo fichero muchas veces.
struct Banco {
    dir: PathBuf,
}

impl Banco {
    fn nuevo() -> Self {
        let dir = std::env::temp_dir().join(format!("panel-lectura-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("crear el banco");
        Self { dir }
    }

    /// Publica un contenido y devuelve el directorio que `read` espera.
    fn publica(&self, contenido: &str) -> &Path {
        std::fs::write(self.dir.join("panel.json"), contenido).expect("publicar");
        &self.dir
    }

    fn ruta(&self) -> PathBuf {
        self.dir.join("panel.json")
    }
}

impl Drop for Banco {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Los nueve ficheros que hay en el repo, uno a uno.
fn caso_normal(banco: &Banco) {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("la raíz del repo")
        .to_path_buf();

    let mut reales = Vec::new();
    reunir(&raiz.join(".claude/work"), &mut reales);
    reunir(&raiz.join("prototipos"), &mut reales);
    reales.sort();

    if reales.is_empty() {
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
    for ruta in &reales {
        let contenido = std::fs::read_to_string(ruta).expect("leer el fichero real");
        let dir = banco.publica(&contenido);

        // Una pasada en frío antes de medir: la primera lectura paga el caché del directorio.
        let _ = read(dir);

        const VUELTAS: u32 = 5000;
        let inicio = Instant::now();
        for _ in 0..VUELTAS {
            std::hint::black_box(read(std::hint::black_box(dir)).ok());
        }
        let micros = inicio.elapsed().as_secs_f64() * 1e6 / f64::from(VUELTAS);
        total += micros;

        // Lo mismo sin interpretar: metadata + open + read_to_end, que es el suelo irreductible.
        let fichero = banco.ruta();
        let inicio = Instant::now();
        for _ in 0..VUELTAS {
            std::hint::black_box(std::fs::metadata(&fichero).ok());
            std::hint::black_box(std::fs::read(&fichero).ok());
        }
        let io = inicio.elapsed().as_secs_f64() * 1e6 / f64::from(VUELTAS);

        let (panel, uso) = midiendo(|| read(dir).expect("un fichero real del repo se lee"));

        println!(
            "  {:<46} {:>6} {:>4} {:>8.1} {:>9.1} {:>9} {:>6.1}×",
            corto(ruta, &raiz),
            contenido.len(),
            panel.lines.len(),
            micros,
            io,
            humano(uso.pico),
            uso.pico as f64 / contenido.len() as f64
        );
    }

    println!(
        "\n  Media: {:.1} µs por lectura. A 60 fotogramas por segundo, releer los nueve en cada uno",
        total / reales.len() as f64
    );
    println!(
        "  costaría {:.2} ms de los 16,7 del fotograma.",
        total / 1000.0
    );
}

fn reunir(desde: &Path, en: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(desde) else {
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.is_dir() {
            reunir(&ruta, en);
        } else if ruta.file_name().is_some_and(|n| n == "panel.json") {
            en.push(ruta);
        }
    }
}

fn corto(ruta: &Path, raiz: &Path) -> String {
    ruta.strip_prefix(raiz)
        .unwrap_or(ruta)
        .display()
        .to_string()
}

/// Una forma de llenar el techo, con su nombre.
struct Forma {
    nombre: &'static str,
    texto: String,
}

/// Tres maneras de gastar 256 KiB, de la más realista a los dos extremos.
fn formas_en_el_techo() -> Vec<Forma> {
    // La forma que tiene un panel de verdad: objetos con referencia, texto, marca, estilo y enlace.
    let linea_real = r##"{"ref": "#12", "text": "extraer Division de la ventana y dejarla sola", "mark": "done", "style": "ok", "link": "https://github.com/mashware/agent-terminal/pull/31"}"##;
    // El peor caso por número de nodos: el objeto más pequeño que sigue siendo una línea.
    let linea_minima = r#"{"text":"x"}"#;

    vec![
        Forma {
            nombre: "líneas como las de un panel real",
            texto: repetir(linea_real),
        },
        Forma {
            nombre: "objetos mínimos del contrato (más nodos por byte)",
            texto: repetir(linea_minima),
        },
        // El peor caso de verdad: ni siquiera hace falta que el campo exista en el contrato. Basta
        // con que el objeto tenga **una** clave para que su mapa reserve un nodo entero de árbol.
        Forma {
            nombre: "objetos de una clave cualquiera (el peor caso)",
            texto: repetir(r#"{"a":0}"#),
        },
        Forma {
            nombre: "una sola línea gigante",
            texto: {
                let relleno = "x".repeat(MAX_BYTES as usize - 32);
                format!(r#"{{"lines":["{relleno}"]}}"#)
            },
        },
    ]
}

/// Repite un elemento hasta rozar el techo por debajo.
fn repetir(elemento: &str) -> String {
    let mut texto = String::from(r#"{"lines":["#);
    let cierre = "]}";
    while texto.len() + elemento.len() + 1 + cierre.len() < MAX_BYTES as usize {
        texto.push_str(elemento);
        texto.push(',');
    }
    texto.pop();
    texto.push_str(cierre);
    texto
}

fn en_el_techo(banco: &Banco, forma: &Forma) {
    let dir = banco.publica(&forma.texto);
    let _ = read(dir);

    const VUELTAS: u32 = 50;
    let inicio = Instant::now();
    for _ in 0..VUELTAS {
        std::hint::black_box(read(std::hint::black_box(dir)).ok());
    }
    let milis = inicio.elapsed().as_secs_f64() * 1e3 / f64::from(VUELTAS);

    let antes = VIVOS.load(Relaxed);
    let (panel, uso) = midiendo(|| read(dir).expect("en el techo todavía se lee"));
    // Lo que **queda** vivo con el panel en la mano: el pico es un instante, esto se paga mientras
    // la división exista.
    let residente = VIVOS.load(Relaxed).saturating_sub(antes);

    // Cuánto de ese tiempo es el disco y no el intérprete.
    let ruta = banco.ruta();
    let inicio = Instant::now();
    for _ in 0..VUELTAS {
        std::hint::black_box(std::fs::read(&ruta).expect("leer los bytes"));
    }
    let io = inicio.elapsed().as_secs_f64() * 1e3 / f64::from(VUELTAS);

    println!("  {}", forma.nombre);
    println!(
        "    {} bytes · {} líneas leídas",
        forma.texto.len(),
        panel.lines.len()
    );
    println!(
        "    tiempo   {milis:.2} ms  (de los que {io:.2} ms son leer los bytes → {:.2} ms interpretar)",
        milis - io
    );
    println!(
        "    pico     {}  = {:.1}× el fichero   ({} reservas, {} reasignaciones)",
        humano(uso.pico),
        uso.pico as f64 / forma.texto.len() as f64,
        uso.reservas,
        uso.reasignaciones
    );
    println!(
        "    queda    {} vivos con el panel en la mano ({:.0} % del pico)",
        humano(residente),
        residente as f64 * 100.0 / uso.pico as f64
    );
    // Cuatro divisiones **una detrás de otra en el mismo hilo**, que es como ocurriría: el pico no
    // se multiplica por cuatro, porque el intermedio de cada lectura muere antes de la siguiente.
    let antes = VIVOS.load(Relaxed);
    PICO.store(antes, Relaxed);
    let cuatro: Vec<_> = (0..4).map(|_| read(dir).expect("leer")).collect();
    let pico_de_cuatro = PICO.load(Relaxed).saturating_sub(antes);
    std::hint::black_box(&cuatro);
    drop(cuatro);

    println!(
        "    cuatro divisiones seguidas: {} de pico — no {}, porque los intermedios no coinciden\n",
        humano(pico_de_cuatro),
        humano(uso.pico * 4)
    );
}

/// Las dos formas de acumular los bytes leídos, replicadas y contadas.
///
/// **Es una comparación, no una copia de `leer_acotado`**: lo que ata es *por qué* esa función dice
/// de antemano cuánto espera. Sin la pista, `read_to_end` lee tanto como quepa en lo reservado
/// —empezando por 32 bytes—, así que cada duplicación del búfer es una llamada al sistema más: siete
/// lecturas para un panel de 861 B y quince para uno en el techo, contadas con `strace`. Con la
/// pista son dos, pase lo que pase, y la capacidad final deja de ser el doble del fichero.
fn reservas_de_la_lectura(banco: &Banco) {
    println!("  Acumular los bytes sin decir de antemano cuántos se esperan, contra decirlo.");
    println!(
        "\n  {:<24} {:>9} {:>10} {:>16}",
        "fichero", "reservas", "reasign.", "capacidad final"
    );

    for (nombre, contenido) in [
        ("uno real (861 B)", "x".repeat(861)),
        ("16 KiB", "x".repeat(16 * 1024)),
        ("en el techo", "x".repeat(MAX_BYTES as usize)),
    ] {
        banco.publica(&contenido);
        let ruta = banco.ruta();
        let (bytes, uso) = midiendo(|| {
            let fichero = std::fs::File::open(&ruta).expect("abrir");
            let mut bytes = Vec::new();
            fichero
                .take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .expect("leer");
            bytes
        });

        // Lo mismo diciendo de antemano cuánto se espera, acotado al techo. El tamaño no cuesta
        // nada: quien lee acaba de pedir los metadatos para comprobar que es un fichero regular. Y
        // no decide nada — el techo lo siguen aplicando el `take` y la comprobación de después, así
        // que un fichero que crezca entre medias no rompe nada: el `Vec` crece solo.
        let esperado = std::fs::metadata(&ruta).expect("metadatos").len();
        let (con_capacidad, uso_con) = midiendo(|| {
            let fichero = std::fs::File::open(&ruta).expect("abrir");
            let mut bytes = Vec::with_capacity(esperado.min(MAX_BYTES) as usize + 1);
            fichero
                .take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .expect("leer");
            bytes
        });

        println!(
            "  {:<24} {:>9} {:>10} {:>16}   →   con capacidad previa: {} + {}, {}",
            nombre,
            uso.reservas,
            uso.reasignaciones,
            humano(bytes.capacity()),
            uso_con.reservas,
            uso_con.reasignaciones,
            humano(con_capacidad.capacity())
        );
    }
}

fn humano(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
