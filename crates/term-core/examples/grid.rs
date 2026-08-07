//! Lanza un programa, mete su salida en la rejilla y la vuelca como texto.
//!
//! Es la comprobación a mano del hito: se ve lo que el terminal *entiende*, no los bytes que
//! recibe. Con `vim` o `htop` dentro, la rejilla tiene que leerse como su interfaz.
//!
//! ```text
//! cargo run --example grid -p term-core            # zsh -l
//! cargo run --example grid -p term-core -- vim     # vim
//! cargo run --example grid -p term-core -- htop
//! ```
//!
//! A los 2 segundos redimensiona a 30×100 y vuelve a volcar, que es como se comprueba el reflujo.

use std::time::{Duration, Instant};

use term_core::{Pty, Screen, Size};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Sin argumentos, un shell de login. Con argumentos, lo que se pida tal cual: `-l` significa
    // cosas distintas en cada programa (en `vim` es modo lisp), así que no se cuela por detrás.
    let mut cli = std::env::args().skip(1);
    let programa = cli.next().unwrap_or_else(|| "zsh".to_string());
    let args: Vec<String> = match cli.collect::<Vec<_>>() {
        v if v.is_empty() && programa == "zsh" => vec!["-l".to_string()],
        v => v,
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let inicial = Size { rows: 24, cols: 80 };

    let mut pty = Pty::spawn(&programa, &args, inicial)?;
    let mut pantalla = Screen::new(inicial);

    bombear(&pty, &mut pantalla, Duration::from_secs(2));
    println!("── rejilla {}×{} ──", inicial.rows, inicial.cols);
    println!("{}", pantalla.to_text());

    // Las dos mitades del redimensionado: la rejilla refluye, y el programa se entera y repinta.
    // La rejilla va primero a propósito: en cuanto el programa sepa el tamaño nuevo empieza a
    // mandar bytes para él, y si la rejilla siguiera siendo la vieja se interpretarían contra unas
    // dimensiones equivocadas.
    let nuevo = Size {
        rows: 30,
        cols: 100,
    };
    pantalla.resize(nuevo);
    pty.resize(nuevo)?;

    bombear(&pty, &mut pantalla, Duration::from_secs(2));
    println!(
        "\n── rejilla {}×{} tras redimensionar ──",
        nuevo.rows, nuevo.cols
    );
    println!("{}", pantalla.to_text());

    Ok(())
}

/// Pasa lo que haya en el canal a la rejilla durante un rato.
///
/// Este bucle es justo el trabajo que en F1.3 hará el bucle de pintado, y por eso `advance` se
/// llama desde aquí en vez de desde un hilo: quien consume decide cuánto procesa por pasada.
fn bombear(pty: &Pty, pantalla: &mut Screen, durante: Duration) {
    let hasta = Instant::now() + durante;
    while Instant::now() < hasta {
        match pty.output().recv_timeout(Duration::from_millis(50)) {
            Ok(bytes) => pantalla.advance(&bytes),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
        }
    }
}
