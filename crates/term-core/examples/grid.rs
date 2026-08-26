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

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use term_core::{Pty, Screen, Size};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Sin argumentos, un shell de login. Con argumentos, lo que se pida tal cual: `-l` significa
    // cosas distintas en cada programa (en `vim` es modo lisp), así que no se cuela por detrás.
    let mut cli = std::env::args().skip(1);
    let program = cli.next().unwrap_or_else(|| "zsh".to_string());
    let args: Vec<String> = match cli.collect::<Vec<_>>() {
        v if v.is_empty() && program == "zsh" => vec!["-l".to_string()],
        v => v,
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let initial = Size { rows: 24, cols: 80 };

    let (mut pty, pty_output) = Pty::spawn(&program, &args, initial)?;
    let mut screen = Screen::new(initial);

    pump(&pty_output, &mut screen, Duration::from_secs(2));
    println!("── rejilla {}×{} ──", initial.rows, initial.cols);
    println!("{}", screen.to_text());

    // Las dos mitades del redimensionado: la rejilla refluye, y el programa se entera y repinta.
    // La rejilla va primero a propósito: en cuanto el programa sepa el tamaño nuevo empieza a
    // mandar bytes para él, y si la rejilla siguiera siendo la vieja se interpretarían contra unas
    // dimensiones equivocadas.
    let resized = Size {
        rows: 30,
        cols: 100,
    };
    screen.resize(resized);
    pty.resize(resized)?;

    pump(&pty_output, &mut screen, Duration::from_secs(2));
    println!(
        "\n── rejilla {}×{} tras redimensionar ──",
        resized.rows, resized.cols
    );
    println!("{}", screen.to_text());

    Ok(())
}

/// Pasa lo que haya en el canal a la rejilla durante un rato.
///
/// `advance` se llama desde aquí y no desde un hilo: quien consume decide cuánto procesa por
/// pasada. Ojo, esto **no** es ascendible tal cual al bucle de pintado: `recv_timeout` bloquea y se
/// queda con el hilo hasta agotar el plazo, y un frame es una llamada que tiene que devolver el
/// control. Lo que se hereda es la forma, no el código.
fn pump(pty_output: &Receiver<Vec<u8>>, screen: &mut Screen, for_how_long: Duration) {
    let until = Instant::now() + for_how_long;
    while Instant::now() < until {
        match pty_output.recv_timeout(Duration::from_millis(50)) {
            Ok(bytes) => screen.advance(&bytes),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
        }
    }
}
