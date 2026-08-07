//! Una división: el PTY y la rejilla del terminal.
//!
//! No depende de nada visual: si la capa de pintado se reescribe, esta caja sigue sirviendo.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

use portable_pty::{Child, CommandBuilder, native_pty_system};

/// Error de cualquier operación sobre el pseudoterminal.
///
/// Se usa un error opaco en vez de un enum propio porque todavía no hay variantes que quien llama
/// pueda tratar de forma distinta. Cuando las haya, se sustituye sin romper la firma.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub type Result<T> = std::result::Result<T, Error>;

/// Tamaño del pseudoterminal, en celdas.
///
/// Tipo propio y no el de `portable-pty`: mantiene la dependencia fuera de la API pública.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

impl Default for Size {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

/// Un programa corriendo dentro de un pseudoterminal.
///
/// La salida llega por un canal, alimentado por un hilo lector propio: quien consume no se bloquea
/// al ritmo del programa. Los bytes se entregan **crudos**, sin interpretar ni validar como UTF-8
/// —una secuencia de escape puede llegar partida entre dos mensajes—, porque interpretarlos es
/// trabajo de otra capa.
pub struct Pty {
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    reader: Option<JoinHandle<()>>,
    output: Receiver<Vec<u8>>,
}

impl Pty {
    /// Abre un pseudoterminal y lanza `program` dentro.
    pub fn spawn(program: &str, args: &[&str], size: Size) -> Result<Self> {
        let pair = native_pty_system().openpty(portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        let child = pair.slave.spawn_command(cmd)?;

        // El extremo esclavo solo lo necesita el hijo. Soltarlo aquí es lo que hace que la lectura
        // del maestro llegue a fin de fichero cuando el hijo muere; si se mantuviera abierto, el
        // hilo lector se quedaría bloqueado para siempre.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let (tx, output) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            child,
            writer,
            reader: Some(reader),
            output,
        })
    }

    /// Canal por el que llega la salida cruda del programa.
    ///
    /// Se desconecta cuando el programa termina.
    pub fn output(&self) -> &Receiver<Vec<u8>> {
        &self.output
    }

    /// Escribe bytes al programa, como si se hubieran tecleado.
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // El orden importa: matar al hijo es lo que cierra su extremo del pseudoterminal y provoca
        // el fin de fichero en el hilo lector. Esperar al hilo antes de eso lo colgaría, porque
        // seguiría bloqueado dentro de `read`.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    /// Margen holgado: lo que se comprueba es que el canal acabe cerrándose, no lo rápido que lo
    /// hace. Cualquier espera acotada sirve mientras no sea infinita.
    const PLAZO: Duration = Duration::from_secs(10);

    /// Consume el canal hasta que se cierre y devuelve todo lo recibido.
    ///
    /// Con plazo a propósito: un `recv()` sin plazo convertiría el fallo que estos tests buscan
    /// —un hilo lector que no termina— en un cuelgue eterno en vez de en un test en rojo.
    fn consumir_hasta_el_cierre(pty: &Pty) -> Vec<u8> {
        let mut salida = Vec::new();
        loop {
            match pty.output().recv_timeout(PLAZO) {
                Ok(bytes) => salida.extend_from_slice(&bytes),
                Err(RecvTimeoutError::Disconnected) => return salida,
                Err(RecvTimeoutError::Timeout) => {
                    panic!("el canal seguía abierto tras {PLAZO:?}: el hilo lector no terminó")
                }
            }
        }
    }

    #[test]
    fn el_canal_se_cierra_cuando_el_programa_termina() {
        let pty = Pty::spawn("sh", &["-c", "exit 0"], Size::default()).expect("lanzar sh");

        consumir_hasta_el_cierre(&pty);
    }

    #[test]
    fn un_programa_inexistente_da_error_en_vez_de_panico() {
        let result = Pty::spawn(
            "programa-que-no-existe-en-ningun-sitio",
            &[],
            Size::default(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn lo_que_escribe_el_programa_llega_por_el_canal() {
        let pty = Pty::spawn("sh", &["-c", "printf hola"], Size::default()).expect("lanzar sh");

        let salida = consumir_hasta_el_cierre(&pty);

        assert!(
            salida.windows(4).any(|v| v == b"hola"),
            "no llegó la salida del programa: {salida:?}"
        );
    }
}
