//! Reproductor del fallo: la salida que no se drena no tiene techo.
//!
//! Recorre la cadena **real** —el pseudoterminal, el hilo puente y la recogida con el presupuesto
//! por fotograma— sin abrir ventana, y publica cuánta memoria residente ocupa el proceso mientras
//! tanto. Sin interfaz de por medio, lo que se ve es solo el almacenamiento; si crece aquí, no lo
//! provoca el pintado.
//!
//! **Tiene tope de seguridad a propósito.** El caso que reproduce agota la memoria de la máquina si
//! se le deja: el pseudoterminal entrega dos órdenes de magnitud más deprisa de lo que se recoge.
//! Al llegar al tope se para y se declara reproducido — dejarlo correr hasta el final no añadiría
//! ninguna información y sí un cuelgue del sistema.
//!
//! Solo Linux: la medida sale de `/proc/self/status`.
//!
//! ```text
//! cargo run --release --example leak -p app -- urandom     # bytes crudos sin fin
//! cargo run --release --example leak -p app -- ceros       # lo mismo, sin coste de generarlos
//! cargo run --release --example leak -p app -- yes         # líneas de dos bytes, sin fin
//! cargo run --release --example leak -p app -- acotado     # ~10 MiB: termina
//! cargo run --release --example leak -p app -- sin-puente  # urandom, consumiendo el canal a pelo
//! ```

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use app::pty_output::PtyOutput;
use term_core::{Pty, Size};

/// Copiado de `app::PUMP_BUDGET`, que es privado. Si allí cambia, aquí deja de medir lo mismo.
const BUDGET: usize = 128 * 1024;

/// El hueco entre recogidas: lo que dura un fotograma a 60 por segundo.
const FRAME: Duration = Duration::from_millis(16);

/// Cuánta memoria residente se tolera antes de declarar el fallo reproducido y parar.
///
/// No es un umbral fino: es un cortafuegos. Con la cadena sana la ocupación se queda en unos pocos
/// megabytes, así que rebasar esto ya solo puede significar que nada frena al productor.
const CEILING: u64 = 512 * 1024;

/// Cuándo rendirse esperando a que crezca, en un escenario que sí debería terminar.
const DEADLINE: Duration = Duration::from_secs(30);

/// Cada cuánto se anota una fila de la tabla.
const SAMPLING: Duration = Duration::from_millis(500);

fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_else(|| "urandom".into());

    let (program, args): (&str, &[&str]) = match scenario.as_str() {
        // Bytes crudos y sin fin, en bloques grandes: la carga que de verdad entrega más deprisa de
        // lo que se recoge. Es el caso del issue.
        "urandom" | "sin-puente" => ("cat", &["/dev/urandom"]),
        // Lo mismo sin el coste de generar azar, por si la máquina lo tuviera caro y el que
        // estuviera midiendo el límite fuese el generador y no la cadena.
        "ceros" => ("cat", &["/dev/zero"]),
        // `yes` escribe sin parar, pero en líneas de dos bytes: cada salto lo procesa la disciplina
        // de línea del pseudoterminal, y eso lo frena muy por debajo de un volcado en bloques. Está
        // aquí porque el issue lo nombra y porque el resultado no es el que se espera.
        "yes" => ("yes", &[]),
        // La misma forma de carga pero acabada: ~10 MiB y fin de fichero. Es el caso que F1.3 midió
        // y aceptó, y está aquí para que la comparación no dependa de recordarlo.
        "acotado" => (
            "sh",
            &[
                "-c",
                "yes 0123456789abcdefghijklmnopqrstuvwxyz | head -n 280000",
            ],
        ),
        other => {
            eprintln!("escenario desconocido: {other}");
            eprintln!("usa: urandom | ceros | yes | acotado | sin-puente");
            std::process::exit(2);
        }
    };

    let resident_at_start = resident_kib().expect("/proc/self/status legible: esto es Linux");
    println!("escenario: {scenario}");
    println!("residente al empezar: {resident_at_start} KiB");
    println!();
    println!("  t (s)   residente (KiB)   crecimiento (KiB)   recogido (MiB)");

    let (pty, output) = Pty::spawn(program, args, Size { rows: 24, cols: 80 })
        .expect("el pseudoterminal se abre y el programa se lanza");

    let verdict = match scenario.as_str() {
        "sin-puente" => pump_direct(output, resident_at_start),
        _ => pump_through_bridge(output, resident_at_start),
    };

    // Destruir el pseudoterminal mata al hijo y cierra el canal. Explícito y antes de imprimir el
    // veredicto: con `yes` dentro, dejarlo para el final del `main` sería seguir alimentando la cola
    // mientras se escribe la conclusión.
    drop(pty);

    println!();
    match verdict {
        Verdict::Reproduced { seconds, growth } => {
            println!(
                "REPRODUCIDO: la memoria residente creció {growth} KiB en {seconds:.1} s y \
                 pasó el tope de {CEILING} KiB sin estabilizarse."
            );
            println!(
                "Nadie frenó al programa de dentro. Sin el tope, esto sigue hasta agotar la máquina."
            );
        }
        Verdict::Terminated {
            seconds,
            growth,
            collected,
        } => {
            println!(
                "TERMINADO: el programa cerró su salida a los {seconds:.1} s tras entregar \
                 {:.1} MiB; el crecimiento máximo fue de {growth} KiB.",
                collected as f64 / (1024.0 * 1024.0)
            );
            println!("Este caso acaba solo: es el que F1.3 midió y aceptó, y no es el del issue.");
        }
        Verdict::Deadline { growth } => {
            println!(
                "SIN VEREDICTO: se agotó el plazo de {} s sin cerrar ni pasar el tope \
                 (crecimiento {growth} KiB).",
                DEADLINE.as_secs()
            );
        }
    }
}

enum Verdict {
    /// Pasó el tope de memoria sin estabilizarse: el fallo está reproducido.
    Reproduced { seconds: f64, growth: u64 },
    /// El programa cerró su salida por su cuenta.
    Terminated {
        seconds: f64,
        growth: u64,
        collected: u64,
    },
    /// Ni una cosa ni la otra dentro del plazo.
    Deadline { growth: u64 },
}

/// La cadena completa: hilo puente encolando, y una recogida por fotograma con el presupuesto.
///
/// Es lo que hace la aplicación de verdad, con el pintado quitado. Que el pintado no esté es
/// justamente lo que hace concluyente la medida: lo que crezca aquí no lo puede haber causado.
fn pump_through_bridge(output: Receiver<Vec<u8>>, resident_at_start: u64) -> Verdict {
    let bridge = PtyOutput::new(output, || {});
    let mut received = Vec::with_capacity(BUDGET);
    let mut collected: u64 = 0;

    let started = Instant::now();
    let mut last_sample = Instant::now();
    let mut max_growth = 0;

    loop {
        received.clear();
        bridge.pick_up(&mut received, BUDGET);
        collected += received.len() as u64;

        let resident = resident_kib().unwrap_or(resident_at_start);
        let growth = resident.saturating_sub(resident_at_start);
        max_growth = max_growth.max(growth);

        if last_sample.elapsed() >= SAMPLING {
            row(started.elapsed(), resident, growth, collected);
            last_sample = Instant::now();
        }

        if growth > CEILING {
            return Verdict::Reproduced {
                seconds: started.elapsed().as_secs_f64(),
                growth,
            };
        }
        // El programa ha muerto y ya no queda nada por recoger: la única salida limpia.
        if !bridge.is_alive() && received.is_empty() {
            return Verdict::Terminated {
                seconds: started.elapsed().as_secs_f64(),
                growth: max_growth,
                collected,
            };
        }
        if started.elapsed() > DEADLINE {
            return Verdict::Deadline { growth: max_growth };
        }

        std::thread::sleep(FRAME);
    }
}

/// Sin puente: se consume el canal del pseudoterminal directamente, al mismo ritmo.
///
/// Sirve para separar las dos colas del camino. Aquí no existe la del puente, así que lo que crezca
/// solo puede ser la del canal que `Pty::spawn` devuelve.
fn pump_direct(output: Receiver<Vec<u8>>, resident_at_start: u64) -> Verdict {
    let mut collected: u64 = 0;
    let started = Instant::now();
    let mut last_sample = Instant::now();
    let mut max_growth = 0;

    loop {
        // El mismo presupuesto que aplica el fotograma, contado en bytes y no en trozos.
        let mut of_this_frame = 0;
        while of_this_frame < BUDGET {
            match output.try_recv() {
                Ok(chunk) => {
                    of_this_frame += chunk.len();
                    collected += chunk.len() as u64;
                }
                Err(_) => break,
            }
        }

        let resident = resident_kib().unwrap_or(resident_at_start);
        let growth = resident.saturating_sub(resident_at_start);
        max_growth = max_growth.max(growth);

        if last_sample.elapsed() >= SAMPLING {
            row(started.elapsed(), resident, growth, collected);
            last_sample = Instant::now();
        }

        if growth > CEILING {
            return Verdict::Reproduced {
                seconds: started.elapsed().as_secs_f64(),
                growth,
            };
        }
        if started.elapsed() > DEADLINE {
            return Verdict::Deadline { growth: max_growth };
        }

        std::thread::sleep(FRAME);
    }
}

fn row(elapsed: Duration, resident: u64, growth: u64, collected: u64) {
    println!(
        "  {:5.1}   {resident:15}   {growth:17}   {:14.1}",
        elapsed.as_secs_f64(),
        collected as f64 / (1024.0 * 1024.0)
    );
}

/// Memoria residente del proceso, en KiB, leída de `/proc/self/status`.
///
/// Es la única de las medidas de memoria que responde a la pregunta del issue: no cuánta se ha
/// reservado, sino cuánta está ocupando la máquina de verdad.
fn resident_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
}
