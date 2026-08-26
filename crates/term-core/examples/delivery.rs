//! Mide cuánto entrega el pseudoterminal a un programa que imprime líneas, con y sin el
//! procesamiento de salida del núcleo.
//!
//! ```text
//! cargo run --release --example delivery -p term-core
//! ```
//!
//! Los dos números salen de la **misma** cadena y de la misma máquina: el programa de dentro
//! enciende o apaga el interruptor por su cuenta con `stty`, así que no hace falta ninguna API para
//! medir la diferencia, y lo que se compara no depende de dos ejecuciones distintas.
//!
//! **Lo que cuesta es cada salto de línea, no cada byte**, así que la relación depende de cuántos
//! haya por byte — por eso se mide con dos largos y no con uno. Un solo número parece universal y no
//! lo es.
//!
//! **Este coste se paga a propósito.** Se intentó quitar apagando `OPOST` al abrir el pseudoterminal,
//! y se revirtió: reponer el retorno de carro desde fuera exige saber si el programa lo quiere, y esa
//! información desaparece en cuanto se le miente sobre el estado de su terminal. La autopsia entera
//! está en **D12** de `docs/02-decisiones.md`. El ejemplo se queda porque el coste sigue ahí y
//! conviene poder volver a mirarlo.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use term_core::{Pty, Size};

/// Tamaño del fichero que se vuelca. Doce megas es un `cat` de un log de verdad.
const BYTES: usize = 12 * 1024 * 1024;

/// Margen por lectura. No es el tiempo total: es cuánto se espera a que llegue *algo más* antes de
/// dar por terminada la entrega.
const DEADLINE: Duration = Duration::from_secs(60);

fn main() {
    // Se vuelca un fichero con `cat` y no se generan las líneas con un bucle del shell. Medido: un
    // `while` de `sh` con `printf` entrega 12 MiB/s **con el interruptor puesto de las dos formas**,
    // porque el cuello es el propio bucle. Con eso, la medida daba «1×» y habría enterrado el
    // hallazgo del ticket con un número tranquilizador y falso.
    for (tag, line) in [
        (
            "línea de log (70 B)",
            &b"lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod\n"[..],
        ),
        ("línea corta   (2 B)", &b"y\n"[..]),
    ] {
        let file = seed(line);
        let without_lines = measure(&file, "stty -opost");
        let with_lines = measure(&file, "stty opost onlcr");
        let _ = std::fs::remove_file(&file);

        println!("{tag} — {} líneas", BYTES / line.len());
        report("procesando en el núcleo", &with_lines);
        report("sin procesar en el núcleo", &without_lines);
        println!(
            "  relación: {:.0}×\n",
            with_lines.took.as_secs_f64() / without_lines.took.as_secs_f64()
        );
    }
}

/// Lo que costó volcar el fichero, y cuánto llegó de verdad al otro lado.
struct Delivery {
    took: Duration,
    delivered: usize,
}

/// El caudal se calcula sobre lo que **escribió el programa**, no sobre lo que se entregó.
///
/// No son lo mismo y por eso importa: con el procesamiento encendido llegan más bytes de los que se
/// escribieron —un retorno de carro por salto, y dos si además se traduce aquí—, así que dividir por
/// lo entregado premiaría a la configuración lenta por hacer bulto.
fn report(tag: &str, delivery: &Delivery) {
    let written = BYTES as f64 / (1024.0 * 1024.0);
    let delivered = delivery.delivered as f64 / (1024.0 * 1024.0);
    println!(
        "  {tag:<26}: {:>9.3?}  →  {:>6.1} MiB/s   ({delivered:.1} MiB entregados)",
        delivery.took,
        written / delivery.took.as_secs_f64(),
    );
}

/// Escribe el fichero que se volcará.
fn seed(line: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join("term-core-entrega.txt");
    let mut file = std::fs::File::create(&path).expect("crear el fichero de la medida");
    let mut written = 0;
    while written < BYTES {
        file.write_all(line).expect("escribir el fichero");
        written += line.len();
    }
    file.flush().expect("cerrar el fichero");
    path
}

fn measure(file: &Path, variant: &str) -> Delivery {
    let command = format!("{variant}; cat {}", file.display());
    let (_pty, channel) = Pty::spawn("sh", &["-c", &command], Size::default()).expect("lanzar sh");

    // El reloj arranca con el primer byte, no antes: arrancar el shell y abrir el fichero no es lo
    // que se está midiendo.
    let first = channel
        .recv_timeout(DEADLINE)
        .expect("el programa no escribió");
    let start = Instant::now();
    let mut delivered = first.len();
    while let Ok(chunk) = channel.recv_timeout(DEADLINE) {
        delivered += chunk.len();
    }

    Delivery {
        took: start.elapsed(),
        delivered,
    }
}
