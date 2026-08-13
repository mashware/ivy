//! Mide cuánto entrega el pseudoterminal a un programa que imprime líneas, con y sin el
//! procesamiento de salida del núcleo.
//!
//! ```text
//! cargo run --release --example entrega -p term-core
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
const PLAZO: Duration = Duration::from_secs(60);

fn main() {
    // Se vuelca un fichero con `cat` y no se generan las líneas con un bucle del shell. Medido: un
    // `while` de `sh` con `printf` entrega 12 MiB/s **con el interruptor puesto de las dos formas**,
    // porque el cuello es el propio bucle. Con eso, la medida daba «1×» y habría enterrado el
    // hallazgo del ticket con un número tranquilizador y falso.
    for (etiqueta, linea) in [
        (
            "línea de log (70 B)",
            &b"lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod\n"[..],
        ),
        ("línea corta   (2 B)", &b"y\n"[..]),
    ] {
        let fichero = sembrar(linea);
        let sin = medir(&fichero, "stty -opost");
        let con = medir(&fichero, "stty opost onlcr");
        let _ = std::fs::remove_file(&fichero);

        println!("{etiqueta} — {} líneas", BYTES / linea.len());
        informe("procesando en el núcleo", &con);
        informe("sin procesar en el núcleo", &sin);
        println!(
            "  relación: {:.0}×\n",
            con.tardado.as_secs_f64() / sin.tardado.as_secs_f64()
        );
    }
}

/// Lo que costó volcar el fichero, y cuánto llegó de verdad al otro lado.
struct Entrega {
    tardado: Duration,
    entregado: usize,
}

/// El caudal se calcula sobre lo que **escribió el programa**, no sobre lo que se entregó.
///
/// No son lo mismo y por eso importa: con el procesamiento encendido llegan más bytes de los que se
/// escribieron —un retorno de carro por salto, y dos si además se traduce aquí—, así que dividir por
/// lo entregado premiaría a la configuración lenta por hacer bulto.
fn informe(etiqueta: &str, entrega: &Entrega) {
    let escrito = BYTES as f64 / (1024.0 * 1024.0);
    let entregado = entrega.entregado as f64 / (1024.0 * 1024.0);
    println!(
        "  {etiqueta:<26}: {:>9.3?}  →  {:>6.1} MiB/s   ({entregado:.1} MiB entregados)",
        entrega.tardado,
        escrito / entrega.tardado.as_secs_f64(),
    );
}

/// Escribe el fichero que se volcará.
fn sembrar(linea: &[u8]) -> PathBuf {
    let ruta = std::env::temp_dir().join("term-core-entrega.txt");
    let mut fichero = std::fs::File::create(&ruta).expect("crear el fichero de la medida");
    let mut escrito = 0;
    while escrito < BYTES {
        fichero.write_all(linea).expect("escribir el fichero");
        escrito += linea.len();
    }
    fichero.flush().expect("cerrar el fichero");
    ruta
}

fn medir(fichero: &Path, ajuste: &str) -> Entrega {
    let orden = format!("{ajuste}; cat {}", fichero.display());
    let (_pty, canal) = Pty::spawn("sh", &["-c", &orden], Size::default()).expect("lanzar sh");

    // El reloj arranca con el primer byte, no antes: arrancar el shell y abrir el fichero no es lo
    // que se está midiendo.
    let primero = canal.recv_timeout(PLAZO).expect("el programa no escribió");
    let inicio = Instant::now();
    let mut entregado = primero.len();
    while let Ok(trozo) = canal.recv_timeout(PLAZO) {
        entregado += trozo.len();
    }

    Entrega {
        tardado: inicio.elapsed(),
        entregado,
    }
}
