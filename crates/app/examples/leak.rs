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

use app::pty_output::Puente;
use term_core::{Pty, Size};

/// Copiado de `app::PUMP_BUDGET`, que es privado. Si allí cambia, aquí deja de medir lo mismo.
const PRESUPUESTO: usize = 128 * 1024;

/// El hueco entre recogidas: lo que dura un fotograma a 60 por segundo.
const FOTOGRAMA: Duration = Duration::from_millis(16);

/// Cuánta memoria residente se tolera antes de declarar el fallo reproducido y parar.
///
/// No es un umbral fino: es un cortafuegos. Con la cadena sana la ocupación se queda en unos pocos
/// megabytes, así que rebasar esto ya solo puede significar que nada frena al productor.
const TOPE: u64 = 512 * 1024;

/// Cuándo rendirse esperando a que crezca, en un escenario que sí debería terminar.
const PLAZO: Duration = Duration::from_secs(30);

/// Cada cuánto se anota una fila de la tabla.
const MUESTREO: Duration = Duration::from_millis(500);

fn main() {
    let escenario = std::env::args().nth(1).unwrap_or_else(|| "urandom".into());

    let (programa, args): (&str, &[&str]) = match escenario.as_str() {
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
        otro => {
            eprintln!("escenario desconocido: {otro}");
            eprintln!("usa: urandom | ceros | yes | acotado | sin-puente");
            std::process::exit(2);
        }
    };

    let residente_al_empezar = residente_kib().expect("/proc/self/status legible: esto es Linux");
    println!("escenario: {escenario}");
    println!("residente al empezar: {residente_al_empezar} KiB");
    println!();
    println!("  t (s)   residente (KiB)   crecimiento (KiB)   recogido (MiB)");

    let (pty, salida) = Pty::spawn(programa, args, Size { rows: 24, cols: 80 })
        .expect("el pseudoterminal se abre y el programa se lanza");

    let veredicto = match escenario.as_str() {
        "sin-puente" => bombear_a_pelo(salida, residente_al_empezar),
        _ => bombear_por_el_puente(salida, residente_al_empezar),
    };

    // Destruir el pseudoterminal mata al hijo y cierra el canal. Explícito y antes de imprimir el
    // veredicto: con `yes` dentro, dejarlo para el final del `main` sería seguir alimentando la cola
    // mientras se escribe la conclusión.
    drop(pty);

    println!();
    match veredicto {
        Veredicto::Reventado {
            segundos,
            crecimiento,
        } => {
            println!(
                "REPRODUCIDO: la memoria residente creció {crecimiento} KiB en {segundos:.1} s y \
                 pasó el tope de {TOPE} KiB sin estabilizarse."
            );
            println!(
                "Nadie frenó al programa de dentro. Sin el tope, esto sigue hasta agotar la máquina."
            );
        }
        Veredicto::Terminado {
            segundos,
            crecimiento,
            recogido,
        } => {
            println!(
                "TERMINADO: el programa cerró su salida a los {segundos:.1} s tras entregar \
                 {:.1} MiB; el crecimiento máximo fue de {crecimiento} KiB.",
                recogido as f64 / (1024.0 * 1024.0)
            );
            println!("Este caso acaba solo: es el que F1.3 midió y aceptó, y no es el del issue.");
        }
        Veredicto::Plazo { crecimiento } => {
            println!(
                "SIN VEREDICTO: se agotó el plazo de {} s sin cerrar ni pasar el tope \
                 (crecimiento {crecimiento} KiB).",
                PLAZO.as_secs()
            );
        }
    }
}

enum Veredicto {
    /// Pasó el tope de memoria sin estabilizarse: el fallo está reproducido.
    Reventado { segundos: f64, crecimiento: u64 },
    /// El programa cerró su salida por su cuenta.
    Terminado {
        segundos: f64,
        crecimiento: u64,
        recogido: u64,
    },
    /// Ni una cosa ni la otra dentro del plazo.
    Plazo { crecimiento: u64 },
}

/// La cadena completa: hilo puente encolando, y una recogida por fotograma con el presupuesto.
///
/// Es lo que hace la aplicación de verdad, con el pintado quitado. Que el pintado no esté es
/// justamente lo que hace concluyente la medida: lo que crezca aquí no lo puede haber causado.
fn bombear_por_el_puente(salida: Receiver<Vec<u8>>, residente_al_empezar: u64) -> Veredicto {
    let puente = Puente::nuevo(salida, || {});
    let mut recibido = Vec::with_capacity(PRESUPUESTO);
    let mut recogido: u64 = 0;

    let empezado = Instant::now();
    let mut ultima_muestra = Instant::now();
    let mut crecimiento_maximo = 0;

    loop {
        recibido.clear();
        puente.recoger(&mut recibido, PRESUPUESTO);
        recogido += recibido.len() as u64;

        let residente = residente_kib().unwrap_or(residente_al_empezar);
        let crecimiento = residente.saturating_sub(residente_al_empezar);
        crecimiento_maximo = crecimiento_maximo.max(crecimiento);

        if ultima_muestra.elapsed() >= MUESTREO {
            fila(empezado.elapsed(), residente, crecimiento, recogido);
            ultima_muestra = Instant::now();
        }

        if crecimiento > TOPE {
            return Veredicto::Reventado {
                segundos: empezado.elapsed().as_secs_f64(),
                crecimiento,
            };
        }
        // El programa ha muerto y ya no queda nada por recoger: la única salida limpia.
        if !puente.sigue_vivo() && recibido.is_empty() {
            return Veredicto::Terminado {
                segundos: empezado.elapsed().as_secs_f64(),
                crecimiento: crecimiento_maximo,
                recogido,
            };
        }
        if empezado.elapsed() > PLAZO {
            return Veredicto::Plazo {
                crecimiento: crecimiento_maximo,
            };
        }

        std::thread::sleep(FOTOGRAMA);
    }
}

/// Sin puente: se consume el canal del pseudoterminal directamente, al mismo ritmo.
///
/// Sirve para separar las dos colas del camino. Aquí no existe la del puente, así que lo que crezca
/// solo puede ser la del canal que `Pty::spawn` devuelve.
fn bombear_a_pelo(salida: Receiver<Vec<u8>>, residente_al_empezar: u64) -> Veredicto {
    let mut recogido: u64 = 0;
    let empezado = Instant::now();
    let mut ultima_muestra = Instant::now();
    let mut crecimiento_maximo = 0;

    loop {
        // El mismo presupuesto que aplica el fotograma, contado en bytes y no en trozos.
        let mut de_este_fotograma = 0;
        while de_este_fotograma < PRESUPUESTO {
            match salida.try_recv() {
                Ok(trozo) => {
                    de_este_fotograma += trozo.len();
                    recogido += trozo.len() as u64;
                }
                Err(_) => break,
            }
        }

        let residente = residente_kib().unwrap_or(residente_al_empezar);
        let crecimiento = residente.saturating_sub(residente_al_empezar);
        crecimiento_maximo = crecimiento_maximo.max(crecimiento);

        if ultima_muestra.elapsed() >= MUESTREO {
            fila(empezado.elapsed(), residente, crecimiento, recogido);
            ultima_muestra = Instant::now();
        }

        if crecimiento > TOPE {
            return Veredicto::Reventado {
                segundos: empezado.elapsed().as_secs_f64(),
                crecimiento,
            };
        }
        if empezado.elapsed() > PLAZO {
            return Veredicto::Plazo {
                crecimiento: crecimiento_maximo,
            };
        }

        std::thread::sleep(FOTOGRAMA);
    }
}

fn fila(transcurrido: Duration, residente: u64, crecimiento: u64, recogido: u64) {
    println!(
        "  {:5.1}   {residente:15}   {crecimiento:17}   {:14.1}",
        transcurrido.as_secs_f64(),
        recogido as f64 / (1024.0 * 1024.0)
    );
}

/// Memoria residente del proceso, en KiB, leída de `/proc/self/status`.
///
/// Es la única de las medidas de memoria que responde a la pregunta del issue: no cuánta se ha
/// reservado, sino cuánta está ocupando la máquina de verdad.
fn residente_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|linea| linea.strip_prefix("VmRSS:"))
        .and_then(|resto| resto.split_whitespace().next())
        .and_then(|numero| numero.parse().ok())
}
