//! Una división: el PTY y la rejilla del terminal.
//!
//! No depende de nada visual: si la capa de pintado se reescribe, esta caja sigue sirviendo.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use portable_pty::{Child, CommandBuilder, MasterPty, native_pty_system};

mod screen;
mod view;

pub use screen::Screen;
pub use view::{CellWidth, Cursor, CursorShape, TermColor, Underline, View, ViewCell};

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
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    reader: Option<JoinHandle<()>>,
    /// Lo que saca al hilo lector de una espera por sitio en el canal cuando hay que cerrar.
    ///
    /// Sin esto, destruir el `Pty` mientras el canal está lleno **cuelga el cierre**: el hilo espera
    /// un hueco que solo aparecería si alguien recogiera, y el `join` de más abajo espera al hilo.
    parar: Arc<AtomicBool>,
}

/// Cuántas lecturas caben en el canal antes de que el hilo lector deje de leer.
///
/// **Este número es el freno del terminal, no un ajuste de rendimiento.** Mientras el canal tenga
/// sitio, el hilo lector vacía el pseudoterminal; cuando se llena, deja de leer, el búfer del
/// núcleo se llena a su vez —cabe muy poco: unos 20 KiB medidos en Linux— y el programa de dentro
/// **se bloquea al escribir**, que es el único freno que un terminal tiene de verdad.
///
/// Ocho lecturas de 8 KiB son 64 KiB: bastante para que una ráfaga normal no toque el techo, y
/// poco para que lo retenido no importe. Quien consume puede acumular por su cuenta todo lo que
/// quiera, pero si lo hace sin límite anula este freno — el techo solo frena si quien recoge deja
/// de recoger.
const LECTURAS_EN_VUELO: usize = 8;

/// Cada cuánto se vuelve a intentar entregar cuando el canal está lleno.
///
/// Se sondea en vez de esperar bloqueado porque la biblioteca estándar no ofrece un envío con
/// plazo estable, y hace falta poder abandonar la espera para cerrar. Con el canal lleno quien
/// manda es el ritmo de quien recoge —del orden de un fotograma—, así que este intervalo no añade
/// retraso perceptible.
const REINTENTO: Duration = Duration::from_millis(1);

/// Lo que esta caja fija en el entorno del hijo por su cuenta, además de lo que le pasen.
///
/// Está como valor y no repartido en llamadas para que se pueda **leer de un vistazo y afirmar en
/// una prueba**: quien inyecta entorno en el programa de un usuario tiene que poder enseñar la lista
/// completa de lo que inyecta.
///
/// Sin el tipo de terminal y el aviso de color verdadero, un programa a pantalla completa se cree en
/// un terminal tonto y no emite ni color ni las secuencias que la rejilla necesita interpretar.
pub const FIXED_ENV: [(&str, &str); 2] = [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];

impl Pty {
    /// Abre un pseudoterminal, lanza `program` dentro y devuelve el canal de su salida.
    ///
    /// El canal se entrega aparte, no colgado del `Pty`, porque quien lo consume puede querer
    /// hacerlo desde otro hilo — y un `Receiver` prestado no cruza de hilo, aunque movido sí. Que
    /// salga por el constructor evita además que existan dos formas de llegar a él.
    ///
    /// Al hijo se le fija lo que dice [`ENTORNO_FIJADO`].
    ///
    /// **El canal tiene techo, y eso es lo que frena al programa de dentro.** Ver
    /// [`LECTURAS_EN_VUELO`]. Quien recoja del canal sin parar nunca —un `for` sobre él, sin
    /// ritmo ni límite propio— deja el techo sin efecto y devuelve el problema a donde estaba.
    pub fn spawn(program: &str, args: &[&str], size: Size) -> Result<(Self, Receiver<Vec<u8>>)> {
        Self::spawn_with_env(program, args, size, &[])
    }

    /// Como [`Pty::spawn`], y además decide en el hijo las variables de entorno de `env`: un valor
    /// las fija, `None` **las quita**.
    ///
    /// **Quitar tiene que ser posible, y no es lo mismo que no fijar.** `portable-pty` arranca
    /// copiando el entorno de este proceso, así que lo que no se toca **se hereda**: una variable que
    /// quien llama considera suya y esta vez no tiene valor seguiría ahí con el valor del padre. Con
    /// `None` se borra de la copia, y entonces el hijo no la ve venga de donde venga.
    ///
    /// Se aplica **después** de [`FIXED_ENV`], y encima del entorno heredado: fijar sustituye la
    /// entrada copiada, porque el `CommandBuilder` de `portable-pty` las guarda en un mapa por clave.
    ///
    /// Los valores son `OsStr` porque una ruta no está garantizada como UTF-8 y esta caja no tiene
    /// por qué ser la que lo descubra.
    pub fn spawn_with_env(
        program: &str,
        args: &[&str],
        size: Size,
        env: &[(&str, Option<&OsStr>)],
    ) -> Result<(Self, Receiver<Vec<u8>>)> {
        let pair = native_pty_system().openpty(portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        for (nombre, valor) in FIXED_ENV {
            cmd.env(nombre, valor);
        }
        for (nombre, valor) in env {
            match valor {
                Some(valor) => cmd.env(nombre, valor),
                None => cmd.env_remove(nombre),
            }
        }
        let child = pair.slave.spawn_command(cmd)?;

        // El extremo esclavo solo lo necesita el hijo. Soltarlo aquí es lo que hace que la lectura
        // del maestro llegue a fin de fichero cuando el hijo muere; si se mantuviera abierto, el
        // hilo lector se quedaría bloqueado para siempre.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let (tx, output) = sync_channel(LECTURAS_EN_VUELO);
        let parar = Arc::new(AtomicBool::new(false));
        let parar_del_hilo = Arc::clone(&parar);
        let reader = thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if entregar(&tx, buf[..n].to_vec(), &parar_del_hilo).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok((
            Self {
                child,
                // El maestro se conserva para poder redimensionar. Mantenerlo abierto no impide el
                // fin de fichero del hilo lector: ese depende de que nadie tenga abierto el extremo
                // esclavo, y el único que lo tiene es el hijo.
                master: pair.master,
                writer,
                reader: Some(reader),
                parar,
            },
            output,
        ))
    }

    /// Comunica al programa que el terminal ha cambiado de tamaño.
    ///
    /// Es la mitad que hace que el programa de dentro se entere y repinte. La otra mitad —refluir
    /// la rejilla— la hace [`Screen::resize`], y **hacen falta las dos**: solo con esta, la rejilla
    /// se queda con el tamaño viejo; solo con la otra, `vim` sigue pintando como si nada.
    pub fn resize(&mut self, size: Size) -> Result<()> {
        self.master.resize(portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Escribe bytes al programa, como si se hubieran tecleado.
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }
}

/// Deja un trozo en el canal, esperando a que haya sitio, y abandona si hay que cerrar.
///
/// Devolver `Err` significa «no hay a quién entregar»: el canal se cerró o se está cerrando. Quien
/// llama termina el bucle de lectura.
///
/// **La espera es el freno.** Mientras dura, este hilo no lee del pseudoterminal, así que el búfer
/// del núcleo se llena y el programa de dentro se bloquea al escribir. No se descarta nada: los
/// bytes se quedan donde ya estaban, en el programa que los escribió.
fn entregar(
    canal: &SyncSender<Vec<u8>>,
    trozo: Vec<u8>,
    parar: &AtomicBool,
) -> std::result::Result<(), ()> {
    let mut pendiente = trozo;
    loop {
        // Se comprueba antes de intentar y no solo después de fallar: si el cierre llega justo
        // cuando el canal tiene sitio, entregar un trozo más no rompe nada, pero comprobar primero
        // hace que la salida del bucle no dependa del estado del canal.
        if parar.load(Ordering::Relaxed) {
            return Err(());
        }
        match canal.try_send(pendiente) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(devuelto)) => {
                pendiente = devuelto;
                thread::sleep(REINTENTO);
            }
            Err(TrySendError::Disconnected(_)) => return Err(()),
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // Primero la bandera: con el canal lleno, el hilo no está dentro de `read` sino esperando
        // sitio, y ni matar al hijo ni cerrar el pseudoterminal lo sacan de ahí. Sin esto, el
        // `join` de más abajo espera para siempre y **cerrar la ventana se cuelga** — comprobado.
        self.parar.store(true, Ordering::Relaxed);
        // Y después el orden de siempre: matar al hijo es lo que cierra su extremo del
        // pseudoterminal y provoca el fin de fichero en el hilo lector. Esperar al hilo antes de
        // eso lo colgaría, porque seguiría bloqueado dentro de `read`.
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
    fn consumir_hasta_el_cierre(salida_del_pty: &Receiver<Vec<u8>>) -> Vec<u8> {
        let mut salida = Vec::new();
        loop {
            match salida_del_pty.recv_timeout(PLAZO) {
                Ok(bytes) => salida.extend_from_slice(&bytes),
                Err(RecvTimeoutError::Disconnected) => return salida,
                Err(RecvTimeoutError::Timeout) => {
                    panic!("el canal seguía abierto tras {PLAZO:?}: el hilo lector no terminó")
                }
            }
        }
    }

    fn contiene(salida: &[u8], texto: &str) -> bool {
        String::from_utf8_lossy(salida).contains(texto)
    }

    // Ojo con el `_pty` de los tests: no es una variable sobrante. Mientras viva, el hijo vive;
    // sustituirlo por `_` lo suelta en el acto, y `Drop` mata al programa antes de que le dé tiempo
    // a escribir. El test no fallaría siempre — fallaría a veces, que es peor.

    #[test]
    fn el_canal_se_cierra_cuando_el_programa_termina() {
        let (_pty, salida) =
            Pty::spawn("sh", &["-c", "exit 0"], Size::default()).expect("lanzar sh");

        consumir_hasta_el_cierre(&salida);
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
        let (_pty, canal) =
            Pty::spawn("sh", &["-c", "printf hola"], Size::default()).expect("lanzar sh");

        let salida = consumir_hasta_el_cierre(&canal);

        assert!(
            contiene(&salida, "hola"),
            "no llegó la salida del programa: {salida:?}"
        );
    }

    #[test]
    fn el_programa_recibe_el_tipo_de_terminal() {
        // Sin esto, un programa a pantalla completa se cree en un terminal tonto y no emite ni
        // color ni las secuencias que la rejilla necesita interpretar.
        let (_pty, canal) =
            Pty::spawn("sh", &["-c", "printf %s \"$TERM\""], Size::default()).expect("lanzar sh");

        let salida = consumir_hasta_el_cierre(&canal);

        assert!(
            contiene(&salida, "xterm-256color"),
            "el hijo no vio TERM: {:?}",
            String::from_utf8_lossy(&salida)
        );
    }

    /// La mitad que hace que la identidad de una consola **llegue de verdad** al programa de dentro.
    /// Sin este test, una implementación que se guardara los pares y no los fijara dejaría la suite
    /// entera en verde.
    #[test]
    fn el_programa_recibe_las_variables_que_se_le_pasan() {
        let (_pty, canal) = Pty::spawn_with_env(
            "sh",
            &["-c", "printf %s \"$VARIABLE_DE_PRUEBA\""],
            Size::default(),
            &[("VARIABLE_DE_PRUEBA", Some(OsStr::new("3f2a9c14")))],
        )
        .expect("lanzar sh");

        let salida = consumir_hasta_el_cierre(&canal);

        assert!(
            contiene(&salida, "3f2a9c14"),
            "el hijo no vio la variable: {:?}",
            String::from_utf8_lossy(&salida)
        );
    }

    /// La cara B: lo que no se pasa, no aparece.
    ///
    /// El nombre es de este test a propósito. Con uno que alguien pudiera tener definido —`PANEL_ID`,
    /// por ejemplo— el test **hereda** ese valor y se pone rojo por el entorno de quien lo corre, no
    /// por el código: comprobado, pasa al correr la suite dentro de una consola de esta aplicación.
    #[test]
    fn sin_pasar_nada_la_variable_no_esta_definida() {
        let (_pty, canal) = Pty::spawn(
            "sh",
            &["-c", "printf %s \"${VARIABLE_DE_PRUEBA-sin definir}\""],
            Size::default(),
        )
        .expect("lanzar sh");

        let salida = consumir_hasta_el_cierre(&canal);

        assert!(
            contiene(&salida, "sin definir"),
            "la variable estaba definida sin que nadie la pasara: {:?}",
            String::from_utf8_lossy(&salida)
        );
    }

    /// **Quitar no es no fijar**, y esta es la diferencia que sostiene que una consola sin directorio
    /// no se quede con la identidad heredada de la aplicación que la lanzó.
    ///
    /// Se prueba contra `TERM`, que esta caja fija siempre: si `None` no quitara, el hijo la vería con
    /// el valor de [`FIXED_ENV`]. Así el test no necesita que el entorno de quien lo corre tenga nada
    /// en particular.
    #[test]
    fn con_ninguno_por_valor_la_variable_se_quita_aunque_la_caja_la_fije() {
        let (_pty, canal) = Pty::spawn_with_env(
            "sh",
            &["-c", "printf %s \"${TERM-quitada}\""],
            Size::default(),
            &[("TERM", None)],
        )
        .expect("lanzar sh");

        let salida = consumir_hasta_el_cierre(&canal);

        assert!(
            contiene(&salida, "quitada"),
            "TERM siguió definida: {:?}",
            String::from_utf8_lossy(&salida)
        );
    }

    /// Antes de la corrección el canal era `mpsc::channel()`, sin límite: un programa que escribe
    /// sin parar y nadie que recoja hacía crecer lo acumulado sin techo mientras durara la espera.
    /// Este test reproduce justo eso —`cat /dev/zero` sin consumir el canal— y comprueba que lo
    /// que llega a acumularse tiene un techo, no que el techo sea el nominal exacto (eso ya lo
    /// fija [`LECTURAS_EN_VUELO`]).
    ///
    /// El límite de la comprobación no es arbitrario: ocho lecturas de 8 KiB son 64 KiB de canal,
    /// más lo que quepa en el búfer del núcleo del pseudoterminal —~20 KiB medidos en Linux—, más
    /// como mucho una lectura ya sacada del maestro y todavía sin sitio en el canal. Eso da un
    /// techo real del orden de 90-100 KiB. Se deja el margen en 1 MiB —diez veces ese techo, para
    /// no depender de la holgura exacta del búfer del núcleo de esta máquina— y muy por debajo de
    /// lo que un canal sin límite acumularía en la misma espera: `cat /dev/zero` entrega del orden
    /// de cientos de MiB por segundo, así que en 200 ms un canal ilimitado ya habría superado
    /// varias veces ese margen. Es justo esa distancia la que el test tiene que cazar.
    #[test]
    fn sin_consumir_lo_que_llega_esta_acotado() {
        let (_pty, salida) =
            Pty::spawn("cat", &["/dev/zero"], Size::default()).expect("lanzar cat");

        // Sin tocar el canal: da tiempo a que el hilo lector lo llene, se quede esperando sitio y
        // el `cat` de dentro se bloquee al escribir. Es el estado que hay que medir.
        thread::sleep(Duration::from_millis(200));

        let mut recibido = 0usize;
        while let Ok(trozo) = salida.try_recv() {
            recibido += trozo.len();
        }

        const MARGEN_HOLGADO: usize = 1024 * 1024; // 1 MiB — ver comentario de la función.
        assert!(
            recibido > 0,
            "no llegó nada de \"cat /dev/zero\" en 200 ms: el canal no se está alimentando"
        );
        assert!(
            recibido <= MARGEN_HOLGADO,
            "se acumularon {recibido} bytes sin consumir el canal: el freno no está actuando"
        );
    }

    /// El cuelgue que este test cubre está demostrado, no es hipotético: con el canal lleno, el
    /// hilo lector no está dentro de `read` sino esperando sitio en [`entregar`], y `Drop for Pty`
    /// hace `kill` → `wait` → `join`. Sin la bandera `parar` que el `Drop` levanta antes de matar
    /// al hijo, nada saca a ese hilo de su espera —ni el `kill` ni el fin de fichero le llegan
    /// mientras está bloqueado ahí— y el `join` no vuelve nunca: cerrar la ventana se cuelga.
    ///
    /// La comprobación corre en un hilo aparte y usa `recv_timeout` para esperar su aviso de fin,
    /// no un `join` directo del hilo del test: si el cuelgue vuelve, este test tiene que caer en
    /// rojo con un mensaje, no quedarse esperando para siempre junto con él.
    #[test]
    fn cerrar_con_el_canal_lleno_no_se_cuelga() {
        use std::sync::mpsc;

        let (pty, salida) = Pty::spawn("cat", &["/dev/zero"], Size::default()).expect("lanzar cat");

        // Mismo motivo que en el test anterior: dar tiempo a que el canal se llene de verdad. No
        // se toca `salida` a partir de aquí — el canal tiene que seguir lleno mientras se destruye
        // `pty`, que es justo la condición del cuelgue.
        thread::sleep(Duration::from_millis(200));

        let (aviso_tx, aviso_rx) = mpsc::channel();
        let hilo_del_drop = thread::spawn(move || {
            drop(pty);
            // Si el drop se cuelga, este envío no llega nunca; el `recv_timeout` de abajo es lo
            // que convierte eso en un test en rojo en vez de en un cuelgue del propio test.
            let _ = aviso_tx.send(());
        });

        const PLAZO_DE_CIERRE: Duration = Duration::from_secs(5);
        if aviso_rx.recv_timeout(PLAZO_DE_CIERRE).is_err() {
            panic!(
                "destruir el Pty con el canal lleno no terminó en {PLAZO_DE_CIERRE:?}: \
                 el cierre se cuelga"
            );
        }

        // El drop ya terminó, así que este join es inmediato; está aquí por higiene, no porque
        // haga falta para que el test sea correcto.
        let _ = hilo_del_drop.join();
        drop(salida);
    }
}
