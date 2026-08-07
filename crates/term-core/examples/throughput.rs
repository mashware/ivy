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
const TROZO: usize = 8192;

fn main() {
    // Texto con saltos de línea y algún escape de color: alimentar el parser con un solo carácter
    // repetido mediría el camino rápido y mentiría sobre el caso real.
    let patron = b"\x1b[32mlorem ipsum dolor sit amet\x1b[0m consectetur adipiscing elit\n";
    let mut volcado = Vec::with_capacity(BYTES + patron.len());
    while volcado.len() < BYTES {
        volcado.extend_from_slice(patron);
    }

    let mut pantalla = Screen::new(Size::default());

    let inicio = Instant::now();
    for trozo in volcado.chunks(TROZO) {
        pantalla.advance(trozo);
    }
    let tardado = inicio.elapsed();

    let megas = volcado.len() as f64 / (1024.0 * 1024.0);
    println!("volcado:  {megas:.1} MiB en trozos de {TROZO} B");
    println!("tiempo:   {tardado:.3?}");
    println!("absorbe:  {:.1} MiB/s", megas / tardado.as_secs_f64());
    println!(
        "por trozo: {:.1} µs de media",
        tardado.as_secs_f64() * 1e6 / volcado.len().div_ceil(TROZO) as f64
    );
}
