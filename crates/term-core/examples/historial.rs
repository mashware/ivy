//! Mide **lo que cuesta guardar historial**, que es lo que decide su techo.
//!
//! ```text
//! cargo run --release --example historial -p term-core
//! ```
//!
//! **Mide el motor por debajo de esta caja, y es a propósito.** `Screen` fija hoy el historial en
//! cero, así que medir a través de su fachada exigiría abrirle un hueco a la API de producción para
//! un instrumento. Aquí se hace al revés: primero el número, y el número decide qué API hace falta.
//!
//! **En depuración los tres números salen varias veces peores.** El que vale es el de `--release`.
//!
//! Las tres preguntas que contesta, y por qué cada una:
//!
//! 1. **Cuánto ocupa de verdad una línea de historial.** El techo se elige en líneas y se paga en
//!    memoria, así que sin este número el techo es una cuenta sobre el papel — y no hay techo de
//!    divisiones, así que se multiplica por las consolas que haya.
//! 2. **Cuánto cuesta refluir con N líneas guardadas.** Refluir recorre el historial **entero** cada
//!    vez que cambia el número de columnas, y eso ocurre en **cada fotograma** mientras se arrastra
//!    una frontera. Se miden varios puntos: con uno solo, la extrapolación es una recta inventada.
//! 3. **Si guardar historial encarece interpretar un volcado.** Es la pregunta que puede reordenar el
//!    hito: si sube, las tablas del anexo de D3 —con las que este proyecto juzgó a su framework— hay
//!    que volver a tomarlas.

use std::time::Instant;

use alacritty_terminal::Term;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Config;
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::vte::ansi::Processor;

/// Los techos que se comparan. El primero es el de hoy —ninguno— y el último, el que el motor trae
/// por defecto y que esta medida existe para no heredar a ciegas.
///
/// **Sin el 5 000 a propósito**: no distinguía nada del 3 500 por encima del ruido, así que era una
/// fila que se lee como un dato y no lo es.
const TECHOS: [usize; 4] = [0, 1_000, 3_500, 10_000];

/// Cuántas veces se repite cada punto. **Una sola muestra por punto no es una medida**: la primera
/// versión de este banco publicaba tres cifras significativas sobre una dispersión del 20 %, y un
/// escalón de su tabla resultó ser ruido. Se publica la **mediana** y los extremos.
const REPETICIONES: usize = 5;

/// Con cuántas columnas se mide. Es un ancho de consola realista en una ventana de trabajo, y
/// **importa**: lo que ocupa una línea y lo que cuesta refluirla crecen con las columnas.
const COLUMNAS: usize = 180;

/// Cuántas filas tiene la parte visible. Sale de las tandas de medida de la aplicación.
const FILAS: usize = 49;

/// Lo que se vuelca para llenar el historial y para medir el interpretado.
const VOLCADO: usize = 8 * 1024 * 1024;

/// Trozo con el que se alimenta la rejilla, del tamaño del búfer de lectura del pseudoterminal.
const TROZO: usize = 8192;

struct Dims {
    rows: usize,
    cols: usize,
}

impl Dimensions for Dims {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// Texto con saltos y algún escape de color: un solo carácter repetido mediría el camino rápido del
/// parser y mentiría sobre el caso real.
///
/// **Y en dos largos, porque de eso depende el número que más pesa.** Una línea más corta que la
/// rejilla **no se recoloca**: al estrechar, el motor la empuja entera y sale por la puerta barata, así
/// que un banco de líneas cortas mide el reflujo **que no ocurre**. La primera versión de esto medía
/// solo el corto y publicó un número **cuatro veces** menor que el real. Los dos regímenes son reales
/// —una sesión de terminal tiene de los dos— así que se publican los dos y el techo se elige contra el
/// caro.
fn volcado(envuelve: bool) -> Vec<u8> {
    let corto = b"\x1b[32mlorem ipsum dolor sit amet\x1b[0m consectetur adipiscing elit\n".to_vec();
    let patron = if envuelve {
        // Metro y medio de rejilla: garantiza que cada línea lógica ocupe dos físicas y que
        // estrechar tenga que recolocarla de verdad.
        let mut largo = Vec::new();
        while largo.len() < COLUMNAS * 3 / 2 {
            largo.extend_from_slice(b"\x1b[32mlorem ipsum dolor\x1b[0m sit amet ");
        }
        largo.push(b'\n');
        largo
    } else {
        corto
    };

    let mut bytes = Vec::with_capacity(VOLCADO + patron.len());
    while bytes.len() < VOLCADO {
        bytes.extend_from_slice(&patron);
    }
    bytes
}

/// La mediana y los extremos de unas cuantas muestras, que es lo que se publica de cada punto.
fn resumen(
    mut muestras: Vec<std::time::Duration>,
) -> (
    std::time::Duration,
    std::time::Duration,
    std::time::Duration,
) {
    muestras.sort();
    (
        muestras[muestras.len() / 2],
        muestras[0],
        muestras[muestras.len() - 1],
    )
}

fn con_techo(techo: usize) -> Term<VoidListener> {
    let config = Config {
        scrolling_history: techo,
        ..Config::default()
    };
    Term::new(
        config,
        &Dims {
            rows: FILAS,
            cols: COLUMNAS,
        },
        VoidListener,
    )
}

fn main() {
    let corto = volcado(false);
    let largo = volcado(true);
    let megas = corto.len() as f64 / (1024.0 * 1024.0);

    println!("rejilla:  {FILAS}×{COLUMNAS} · volcado de {megas:.1} MiB en trozos de {TROZO} B");
    println!(
        "muestras: {REPETICIONES} por punto; se publica la mediana y, entre paréntesis, los extremos\n"
    );

    // ── 1. Lo que ocupa una celda, y lo que eso da de techo ──────────────────────────────────────
    //
    // **El tamaño de la celda se pregunta, no se deduce del layout**: sumar los campos ignora el
    // relleno que el compilador mete entre ellos, y el techo cuelga de este número.
    //
    // **Lo demás de esta sección es una CUENTA, no una medida, y por eso se dice.** El motor no
    // reserva el historial de antemano: su almacenamiento crece al llenarse. Así que estas cifras son
    // **el techo** que una consola puede llegar a ocupar con el historial lleno, no lo que ocupa desde
    // el principio.
    let celda = size_of::<Cell>();
    println!("── lo que ocupa (medido: la celda; el resto es cuenta, y es un TECHO) ──");
    println!("celda:    {celda} B  ← lo único medido aquí");
    println!(
        "línea:    {} B con {COLUMNAS} columnas, sin contar lo que la fila reserva de más",
        celda * COLUMNAS
    );
    for techo in TECHOS {
        let por_consola = celda * COLUMNAS * techo;
        println!(
            "  techo {techo:>6}: hasta {:>6.1} MiB por consola · {:>6.1} MiB con cuatro",
            por_consola as f64 / (1024.0 * 1024.0),
            (por_consola * 4) as f64 / (1024.0 * 1024.0)
        );
    }

    // ── 2. Lo que cuesta recolocar el texto, en los dos regímenes ────────────────────────────────
    //
    // Se llena el historial de verdad antes de medir, y se cambia el número de **columnas**, que es lo
    // que obliga a recolocar. Ida y vuelta para no dejar la rejilla creciendo hacia un lado y para
    // esquivar la salida temprana del motor cuando el tamaño no cambia.
    println!("\n── lo que cuesta recolocar el texto al cambiar el ancho ──");
    for (nombre, bytes) in [("líneas cortas", &corto), ("líneas que envuelven", &largo)] {
        println!("  con {nombre}:");
        for techo in TECHOS {
            let mut muestras = Vec::with_capacity(REPETICIONES);
            for _ in 0..REPETICIONES {
                let mut term = con_techo(techo);
                let mut parser: Processor = Processor::new();
                for trozo in bytes.chunks(TROZO) {
                    parser.advance(&mut term, trozo);
                }

                let vueltas = 20;
                let inicio = Instant::now();
                for i in 0..vueltas {
                    let cols = if i % 2 == 0 { COLUMNAS - 1 } else { COLUMNAS };
                    term.resize(Dims { rows: FILAS, cols });
                }
                muestras.push(inicio.elapsed() / vueltas);
            }
            let (medio, min, max) = resumen(muestras);
            println!(
                "    techo {techo:>6}: {medio:>10.3?} por cambio de ancho  ({min:.3?} … {max:.3?}){}",
                if techo == 0 { "  ← lo de hoy" } else { "" }
            );
        }
    }

    // ── 3. Si guardar historial encarece interpretar ─────────────────────────────────────────────
    //
    // **Es la pregunta que puede reordenar el hito.** Si interpretar se encarece, las tablas de los
    // anexos publicados se tomaron con otro régimen y hay que volver a tomarlas.
    println!("\n── lo que cuesta interpretar el volcado ──");
    for techo in TECHOS {
        let mut muestras = Vec::with_capacity(REPETICIONES);
        for _ in 0..REPETICIONES {
            let mut term = con_techo(techo);
            let mut parser: Processor = Processor::new();
            let inicio = Instant::now();
            for trozo in corto.chunks(TROZO) {
                parser.advance(&mut term, trozo);
            }
            muestras.push(inicio.elapsed());
        }
        let (medio, min, max) = resumen(muestras);
        println!(
            "  techo {techo:>6}: {medio:>10.3?} · {:>6.1} MiB/s  ({min:.3?} … {max:.3?}){}",
            megas / medio.as_secs_f64(),
            if techo == 0 { "  ← lo de hoy" } else { "" }
        );
    }
}
