//! Cuánto cuesta repintar N consolas a la vez, medido.
//!
//! Es el banco que responde la pregunta que decide el framework de interfaz: **egui es de modo
//! inmediato y repinta la pantalla entera**, así que la duda no es si aguanta una consola, sino qué
//! pasa cuando hay más de una produciendo a la vez. El roadmap redacta ese síntoma con dos
//! divisiones, y este hito tiene una sola — de ahí un banco que las simula.
//!
//! ```text
//! cargo run --release --example sweep -p app -- "portátil de trabajo"
//! ```
//!
//! El argumento es la etiqueta de la tanda, opcional, y sigue el mismo criterio que `host-load`:
//! **la máquina la pone quien mide**, porque es el único que la conoce sin inventar dependencias.
//! En depuración el número sale varias veces peor; el que vale es el de `--release`.
//!
//! **Qué mide, y por qué son tres números y no uno.**
//!
//! - **Leer** la rejilla: reservar y rellenar todas sus celdas. Se lo llevará puesto cualquier
//!   framework, así que no juzga a este — pero si se sumara al resto, lo taparía.
//! - **Encolar**: lo que cuesta la llamada al pintor. Y se llama encolar, no pintar, porque eso es
//!   literalmente lo que hace: `Painter::rect_filled` y `Painter::text` **apilan formas en una
//!   lista** y vuelven. Convertirlas en triángulos ocurre después, fuera de la pasada de interfaz,
//!   donde un cronómetro puesto aquí no llega.
//! - **CPU del fotograma**: lo que de verdad cuesta el fotograma entero — la pasada de interfaz
//!   **más el teselado y el dibujo**, sin la espera de la sincronía vertical. Lo mide `eframe`,
//!   que es quien puede: para cuando el teselado ocurre, esta función ya ha terminado.
//!
//! **El tercero es el que decide, y por eso están los tres.** Un banco que cronometrara solo la
//! llamada al pintor publicaría un número optimista con toda la apariencia de ser el bueno: contaría
//! el apilado de formas y no el trabajo de convertirlas. Los fotogramas por segundo tampoco sirven
//! solos: están topados por la sincronía vertical, así que un «119 fps» solo demuestra que el
//! fotograma cabe en su hueco, no cuánto margen sobra.
//!
//! **Tres cosas hacen que el número sea el peor caso, a propósito**:
//!
//! 1. **La pantalla está llena.** El pintor se salta los espacios y las celdas cuyo fondo es el de
//!    la ventana, que en una pantalla de terminal real son la mayoría. Medir con una pantalla
//!    realista mediría sobre todo lo que el pintor **no** hace.
//! 2. **Se repinta en continuo**, pidiendo otro fotograma en cada uno, al ritmo que dé la pantalla
//!    —que en un monitor de 120 Hz es el doble de exigente que en uno de 60, y por eso los
//!    fotogramas por segundo se publican junto al número—. El producto hace justo lo contrario
//!    —repinta solo cuando el programa escribe—, así que esto no es el uso normal: es el techo, que
//!    es lo que hay que conocer antes de decidir.
//! 3. **Llama al pintor de verdad**, el mismo que usa la aplicación. Si lo duplicara, su número
//!    dejaría de ser el del producto.
//!
//! Y una cosa que hace que el número sea honesto: **si las consolas no caben enteras en la ventana,
//! el banco no publica nada**. Lo que no se ve no se tesela, y un número sacado de ahí diría que
//! todo va bien precisamente por no haber dibujado.

use app::painter::{CellMetrics, paint};
use app::theme::Theme;
use eframe::egui;
use std::time::{Duration, Instant};
use term_core::{Screen, Size};

/// El tamaño de cada consola simulada. **El mismo para todos los valores de N**: es lo que permite
/// leer el resultado como «coste por consola» y ver si dos cuestan el doble que una, o más.
const GRID: Size = Size { rows: 24, cols: 80 };

/// Los valores de N que se barren.
///
/// Dos, y no más, a propósito: el síntoma que el roadmap describe habla de dos divisiones, y este
/// hito no adelanta trabajo de los siguientes. Cuatro y ocho llegarán cuando haya una fase que las
/// necesite.
const PLAN: [usize; 2] = [1, 2];

/// Fotogramas que se tiran antes de empezar a contar en cada fase.
///
/// El primer pase construye el atlas de la fuente y reserva las texturas, y sale varias veces más
/// caro que los siguientes: contarlo subiría la media de una fase sin que nada de eso vuelva a
/// pasar en las demás.
const WARMUP: usize = 60;

/// Fotogramas que se miden en cada fase.
///
/// Se cuentan fotogramas y no segundos porque **el ritmo lo pone la pantalla**, no el banco: en un
/// monitor de 120 Hz esto son cinco segundos por valor de N y en uno de 60, diez. Contar por tiempo
/// mediría un número distinto de fotogramas en cada máquina y haría incomparables las tandas.
const MEASURED: usize = 600;

/// El cuerpo de la letra.
///
/// El mismo que usa la aplicación, y por ahora escrito aquí igual que en la pantalla de prueba: el
/// producto lo tiene en una constante privada. Si los dos dejaran de coincidir, este número dejaría
/// de ser el del producto sin que nada avise — unificarlo es trabajo de cuando el bucle esté dentro.
const BODY: f32 = 14.0;

fn main() -> eframe::Result {
    let label = std::env::args().nth(1);

    let mut screens = Vec::new();
    for _ in 0..PLAN.iter().copied().max().unwrap_or(1) {
        let mut screen = Screen::new(GRID);
        screen.advance(script(GRID).as_bytes());
        screens.push(screen);
    }

    eframe::run_native(
        "agent-terminal — banco de repintado",
        eframe::NativeOptions {
            // Maximizada: el banco necesita todo el sitio que la pantalla dé, y cuánto es eso no se
            // sabe desde aquí. Con un tamaño escogido a ojo, en una pantalla con aumento alto las
            // dos consolas no caben y la tanda se pierde por una constante mal puesta.
            viewport: egui::ViewportBuilder::default().with_maximized(true),
            ..Default::default()
        },
        Box::new(move |_cc| {
            Ok(Box::new(Sweep {
                screens,
                theme: Theme::default(),
                label,
                phase: 0,
                seen: 0,
                reads: Vec::new(),
                enqueues: Vec::new(),
                frames: Vec::new(),
                intervals: Vec::new(),
                previous: None,
                results: Vec::new(),
                doesnt_fit: None,
            }))
        }),
    )
}

/// Lo que se escribe en cada consola simulada.
///
/// **Determinista**: lo que sale en la fila `i` depende solo de `i`, para que dos tandas de dos
/// días o dos máquinas distintas sean comparables. Y **llena de color a propósito**: cada celda
/// lleva fondo propio, que es lo que obliga al pintor a dibujar un rectángulo por celda además del
/// glifo. Una pantalla de terminal de verdad está medio vacía; esta es el techo.
fn script(grid: Size) -> String {
    let mut text = String::new();
    for row in 0..grid.rows {
        // Seis fondos y seis colores de letra, girando a ritmos distintos para que la letra nunca
        // caiga sobre su propio color: con los dos al mismo ritmo, una de cada seis filas salía
        // azul sobre azul. No cambia el número —el glifo se dibuja igual, se lea o no— pero una
        // banda ilegible en la pantalla invita a creer que esas filas no se pintaron.
        let background = 41 + (row % 6);
        let foreground = 31 + ((row + 1) % 6);
        text.push_str(&format!("\x1b[{foreground};{background}m"));
        for col in 0..grid.cols {
            // Letras, nunca espacios: el pintor se salta los espacios, y una pantalla de espacios
            // mediría sobre todo el bucle vacío.
            text.push((b'a' + ((row + col) % 26) as u8) as char);
        }
        text.push_str("\x1b[0m");
        // Sin salto después de la última fila: desplazaría la pantalla y la primera dejaría de
        // verse, con lo que el banco mediría 23 filas creyendo medir 24.
        if row + 1 < grid.rows {
            text.push_str("\r\n");
        }
    }
    text
}

/// Cómo se colocan las consolas para que quepan, o nada si no caben de ninguna forma.
///
/// Devuelve el desplazamiento de una consola a la siguiente: en fila o en columna. **No es una
/// comodidad, es lo que hace que el banco se pueda correr**: dos consolas de ochenta columnas
/// necesitan más de 1 300 puntos de ancho, y una pantalla con el aumento del escritorio subido no
/// llega — pero sí le sobra alto. Y no se puede elegir antes de abrir la ventana, porque hasta
/// entonces no se sabe ni lo que mide la letra ni lo que da la pantalla.
fn layout(available: egui::Vec2, console: egui::Vec2, count: usize) -> Option<egui::Vec2> {
    let count = count as f32;
    if available.x >= console.x * count && available.y >= console.y {
        Some(egui::vec2(console.x, 0.0))
    } else if available.y >= console.y * count && available.x >= console.x {
        Some(egui::vec2(0.0, console.y))
    } else {
        None
    }
}

/// Lo medido en una fase del barrido.
struct PhaseResult {
    consoles: usize,
    read: Summary,
    enqueue: Summary,
    /// El coste de CPU del fotograma entero, teselado y dibujo incluidos. El número que decide.
    frame: Summary,
    frames_per_second: f64,
}

/// La media y el peor de una serie de tiempos. Los dos, porque dicen cosas distintas: la media dice
/// cuánto cuesta, y el peor es el que se ve como un tirón.
struct Summary {
    mean: Duration,
    worst: Duration,
}

impl Summary {
    fn of(times: &[Duration]) -> Self {
        let total: Duration = times.iter().sum();
        Self {
            mean: total / times.len().max(1) as u32,
            worst: times.iter().copied().max().unwrap_or_default(),
        }
    }
}

struct Sweep {
    screens: Vec<Screen>,
    theme: Theme,
    label: Option<String>,
    phase: usize,
    seen: usize,
    reads: Vec<Duration>,
    enqueues: Vec<Duration>,
    /// Lo que `eframe` dice que costó de CPU el fotograma **anterior**. Va un fotograma por detrás
    /// de los otros dos, que en régimen continuo da igual y en el cambio de fase lo absorbe el
    /// calentamiento.
    frames: Vec<Duration>,
    intervals: Vec<Duration>,
    previous: Option<Instant>,
    results: Vec<PhaseResult>,
    /// Lo que hacía falta y lo que había, si alguna vez las consolas no cupieron en la ventana. Una
    /// sola vez basta para invalidar la tanda: lo que no se ve no se dibuja, y el número saldría
    /// bien por no haber trabajado.
    doesnt_fit: Option<(egui::Vec2, egui::Vec2)>,
}

impl Sweep {
    /// Cierra la fase actual y pasa a la siguiente. Devuelve si el barrido ha terminado.
    fn close_phase(&mut self) -> bool {
        let interval = Summary::of(&self.intervals).mean.as_secs_f64();
        self.results.push(PhaseResult {
            consoles: PLAN[self.phase],
            read: Summary::of(&self.reads),
            enqueue: Summary::of(&self.enqueues),
            frame: Summary::of(&self.frames),
            frames_per_second: if interval > 0.0 { 1.0 / interval } else { 0.0 },
        });

        self.reads.clear();
        self.enqueues.clear();
        self.frames.clear();
        self.intervals.clear();
        self.seen = 0;
        self.previous = None;
        self.phase += 1;
        self.phase == PLAN.len()
    }

    fn publish(&self) {
        if let Some((needed, had)) = self.doesnt_fit {
            println!(
                "SIN NÚMERO: las consolas no cabían enteras en la ventana.\n\
                 Hacían falta {:.0}×{:.0} puntos y había {:.0}×{:.0}.\n\
                 Lo que queda fuera no se dibuja, así que la medida diría que todo va bien\n\
                 precisamente por no haber pintado. Con una pantalla más grande sale el número.",
                needed.x, needed.y, had.x, had.y
            );
            return;
        }

        let profile = if cfg!(debug_assertions) {
            "depuración — el número que vale es el de --release"
        } else {
            "release"
        };
        println!("\n── banco de repintado ──");
        println!("rejilla:      {}×{} por consola", GRID.rows, GRID.cols);
        println!("letra:        monoespaciada de {BODY} puntos");
        println!("renderizador: glow");
        println!("perfil:       {profile}");
        println!("medida:       {MEASURED} fotogramas por valor de N, {WARMUP} de calentamiento");
        if let Some(label) = &self.label {
            println!("máquina:      {label}");
        }
        println!();

        for result in &self.results {
            println!(
                "N = {}  ·  leer {:>9.3?}  ·  encolar {:>9.3?}  ·  CPU del fotograma {:>9.3?} (peor {:>9.3?})  ·  {:.1} fps",
                result.consoles,
                result.read.mean,
                result.enqueue.mean,
                result.frame.mean,
                result.frame.worst,
                result.frames_per_second,
            );
        }

        // La pregunta del hito no es cuánto cuesta una consola, sino **cómo crece**: si dos cuestan
        // el doble, el modo inmediato escala y el síntoma no aparece; si cuestan bastante más, ese
        // es exactamente el síntoma que el roadmap describe. Se compara con el coste de CPU del
        // fotograma entero y no con el encolado: el encolado no incluye el teselado, que es la mitad
        // cara del trabajo y la que más crece con el número de celdas.
        if let ([one], [two]) = (&self.results[..1], &self.results[1..]) {
            let reference = one.frame.mean.as_secs_f64().max(f64::MIN_POSITIVE);
            let factor = two.frame.mean.as_secs_f64() / reference;
            println!("\ndos consolas cuestan {factor:.2}× lo que cuesta una");

            // Cuánto del hueco entre fotogramas se está gastando. Es lo que dice si sobra margen o
            // se va justo, que los fotogramas por segundo no pueden decir: están topados por la
            // sincronía vertical, así que solo demuestran que el fotograma cabe.
            let available = 1.0 / two.frames_per_second.max(f64::MIN_POSITIVE);
            println!(
                "con dos, el fotograma gasta el {:.0} % de su hueco ({:.2} ms de {:.2} ms)",
                100.0 * two.frame.mean.as_secs_f64() / available,
                two.frame.mean.as_secs_f64() * 1e3,
                available * 1e3,
            );
        }
    }
}

impl eframe::App for Sweep {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // El barrido ha terminado y solo queda que la ventana se entere. El cierre no es inmediato
        // —el escritorio lo atiende cuando puede— así que todavía llegan fotogramas por aquí, y sin
        // esta salida el siguiente buscaría una fase que ya no existe.
        if self.phase == PLAN.len() {
            return;
        }

        // Régimen continuo: se pide el siguiente fotograma sin esperar a que nada cambie. Es lo
        // contrario de lo que hace el producto, y es el punto — aquí se mide el techo.
        ui.ctx().request_repaint();

        let now = Instant::now();
        let interval = self.previous.replace(now).map(|prev| now - prev);

        let metrics = CellMetrics::measure(ui.ctx(), egui::FontId::monospace(BODY));
        let consoles = PLAN[self.phase];
        let available = ui.max_rect();
        let console = egui::vec2(
            metrics.width() * GRID.cols as f32,
            metrics.height() * GRID.rows as f32,
        );

        // La disposición se decide **con el mayor N del plan y no con el de esta fase**: si las dos
        // consolas van a acabar apiladas, la de la primera fase tiene que pintarse en el mismo sitio
        // y del mismo modo, o se estarían comparando dos cosas distintas.
        let largest = PLAN.iter().copied().max().unwrap_or(1);
        let step = layout(available.size(), console, largest);

        // Se da la tanda por perdida solo en los fotogramas que cuentan: durante el calentamiento la
        // ventana todavía se está maximizando, y darla por pequeña ahí sería tirarla por algo que
        // está a punto de arreglarse solo.
        if step.is_none() && self.seen >= WARMUP {
            // Se informa de la disposición **más barata de las dos**, no del producto de los dos
            // ejes: apilar necesita el ancho de una consola y el alto de todas, y decir que hacían
            // falta las dos cosas a la vez pediría una pantalla que no hace falta.
            self.doesnt_fit = Some((
                egui::vec2(console.x, console.y * largest as f32),
                available.size(),
            ));
        }
        let step = step.unwrap_or_default();

        let mut read = Duration::ZERO;
        let mut enqueue = Duration::ZERO;
        for (index, screen) in self.screens[..consoles].iter().enumerate() {
            let before = Instant::now();
            let view = screen.view();
            let took = before.elapsed();

            let origin = available.left_top() + step * index as f32;
            let before = Instant::now();
            // **Un descubridor de usar y tirar, y a propósito.** Este banco mide el pintado con texto que las
            // fuentes de fábrica ya cubren, así que nunca hay nada que descubrir; y como no llama a
            // `resolve`, uno persistente solo acumularía apuntes que nadie mira.
            paint(
                ui.painter(),
                &view,
                &self.theme,
                &metrics,
                origin,
                &mut Default::default(),
            );

            read += took;
            enqueue += before.elapsed();
        }

        self.seen += 1;
        if self.seen <= WARMUP {
            return;
        }

        self.reads.push(read);
        self.enqueues.push(enqueue);
        // Lo que costó de CPU el fotograma anterior, con su teselado y su dibujo dentro. Es el
        // único de los tres que esta función no puede cronometrar por su cuenta: cuando el teselado
        // ocurre, ya ha terminado.
        if let Some(usage) = frame.info().cpu_usage {
            self.frames.push(Duration::from_secs_f32(usage));
        }
        if let Some(interval) = interval {
            self.intervals.push(interval);
        }

        if self.reads.len() >= MEASURED && self.close_phase() {
            self.publish();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.theme.background().to_normalized_gamma_f32()
    }
}
