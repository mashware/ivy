//! Mide cuánto absorbe la rejilla en un volcado masivo.
//!
//! Es el riesgo que el roadmap asigna a este hito: un `cat` de un fichero grande. La mitigación
//! acordada **no** es poner un tope dentro de la caja, sino saber el número y que la forma de la
//! API permita trocear — el tope lo pondrá quien pinte, en F1.3, cuando exista un frame contra el
//! que medirlo.
//!
//! ```text
//! cargo run --release --example throughput -p term-core
//! ```
//!
//! En depuración el número sale varias veces peor; el que vale es el de `--release`.

use std::time::Instant;

use term_core::{Screen, Size};

/// Tamaño del volcado. Diez megas es un `cat` de un fichero grande de verdad, no un juguete.
const BYTES: usize = 10 * 1024 * 1024;

/// Trozo con el que se alimenta la rejilla, del tamaño del búfer de lectura del PTY.
const CHUNK: usize = 8192;

fn main() {
    // Texto con saltos de línea y algún escape de color: alimentar el parser con un solo carácter
    // repetido mediría el camino rápido y mentiría sobre el caso real.
    let pattern = b"\x1b[32mlorem ipsum dolor sit amet\x1b[0m consectetur adipiscing elit\n";
    let mut dump = Vec::with_capacity(BYTES + pattern.len());
    while dump.len() < BYTES {
        dump.extend_from_slice(pattern);
    }

    let mut screen = Screen::new(Size::default());

    let start = Instant::now();
    for chunk in dump.chunks(CHUNK) {
        screen.advance(chunk);
    }
    let took = start.elapsed();

    let mib = dump.len() as f64 / (1024.0 * 1024.0);
    println!("volcado:  {mib:.1} MiB en trozos de {CHUNK} B");
    println!("tiempo:   {took:.3?}");
    println!("absorbe:  {:.1} MiB/s", mib / took.as_secs_f64());
    println!(
        "por trozo: {:.1} µs de media",
        took.as_secs_f64() * 1e6 / dump.len().div_ceil(CHUNK) as f64
    );
}
