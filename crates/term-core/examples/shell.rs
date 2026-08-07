//! Lanza un shell en un pseudoterminal, le manda `ls` e imprime la salida cruda.
//!
//! Es la comprobación a mano del hito: `cargo run --example shell -p term-core`. La salida sale
//! tal cual la escribe el shell, secuencias de escape incluidas — interpretarlas es trabajo de
//! otra capa, y verlas en crudo aquí es justamente la prueba de que llegan enteras.

use std::io::{self, Write};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use term_core::{Pty, Size};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut pty, salida_del_pty) = Pty::spawn("zsh", &["-l"], Size::default())?;
    pty.write(b"ls\n")?;

    // El shell sigue vivo esperando más órdenes, así que el canal no se va a cerrar solo: se lee
    // durante un rato y se sale.
    let hasta = Instant::now() + Duration::from_secs(2);
    let salida = io::stdout();
    let mut salida = salida.lock();
    while Instant::now() < hasta {
        match salida_del_pty.recv_timeout(Duration::from_millis(100)) {
            Ok(bytes) => {
                salida.write_all(&bytes)?;
                salida.flush()?;
            }
            // El shell murió por su cuenta (un `exit`, un fallo al arrancar): no hay nada más que
            // leer y seguir esperando al plazo solo alargaría la salida sin motivo.
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => continue,
        }
    }

    // Al soltar `pty` se mata el shell y se espera al hilo lector: no queda ningún proceso huérfano.
    Ok(())
}
