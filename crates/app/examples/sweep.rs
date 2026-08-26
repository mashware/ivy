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
use app::theme::Tema;
use eframe::egui;
use std::time::{Duration, Instant};
use term_core::{Screen, Size};

/// El tamaño de cada consola simulada. **El mismo para todos los valores de N**: es lo que permite
/// leer el resultado como «coste por consola» y ver si dos cuestan el doble que una, o más.
const REJILLA: Size = Size { rows: 24, cols: 80 };

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
const CALENTAMIENTO: usize = 60;

/// Fotogramas que se miden en cada fase.
///
/// Se cuentan fotogramas y no segundos porque **el ritmo lo pone la pantalla**, no el banco: en un
/// monitor de 120 Hz esto son cinco segundos por valor de N y en uno de 60, diez. Contar por tiempo
/// mediría un número distinto de fotogramas en cada máquina y haría incomparables las tandas.
const MEDIDOS: usize = 600;

/// El cuerpo de la letra.
///
/// El mismo que usa la aplicación, y por ahora escrito aquí igual que en la pantalla de prueba: el
/// producto lo tiene en una constante privada. Si los dos dejaran de coincidir, este número dejaría
/// de ser el del producto sin que nada avise — unificarlo es trabajo de cuando el bucle esté dentro.
const CUERPO: f32 = 14.0;

fn main() -> eframe::Result {
    let etiqueta = std::env::args().nth(1);

    let mut pantallas = Vec::new();
    for _ in 0..PLAN.iter().copied().max().unwrap_or(1) {
        let mut pantalla = Screen::new(REJILLA);
        pantalla.advance(guion(REJILLA).as_bytes());
        pantallas.push(pantalla);
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
            Ok(Box::new(Barrido {
                pantallas,
                tema: Tema::default(),
                etiqueta,
                fase: 0,
                vistos: 0,
                lecturas: Vec::new(),
                encolados: Vec::new(),
                fotogramas: Vec::new(),
                intervalos: Vec::new(),
                anterior: None,
                resultados: Vec::new(),
                no_cabe: None,
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
fn guion(rejilla: Size) -> String {
    let mut texto = String::new();
    for fila in 0..rejilla.rows {
        // Seis fondos y seis colores de letra, girando a ritmos distintos para que la letra nunca
        // caiga sobre su propio color: con los dos al mismo ritmo, una de cada seis filas salía
        // azul sobre azul. No cambia el número —el glifo se dibuja igual, se lea o no— pero una
        // banda ilegible en la pantalla invita a creer que esas filas no se pintaron.
        let fondo = 41 + (fila % 6);
        let frente = 31 + ((fila + 1) % 6);
        texto.push_str(&format!("\x1b[{frente};{fondo}m"));
        for columna in 0..rejilla.cols {
            // Letras, nunca espacios: el pintor se salta los espacios, y una pantalla de espacios
            // mediría sobre todo el bucle vacío.
            texto.push((b'a' + ((fila + columna) % 26) as u8) as char);
        }
        texto.push_str("\x1b[0m");
        // Sin salto después de la última fila: desplazaría la pantalla y la primera dejaría de
        // verse, con lo que el banco mediría 23 filas creyendo medir 24.
        if fila + 1 < rejilla.rows {
            texto.push_str("\r\n");
        }
    }
    texto
}

/// Cómo se colocan las consolas para que quepan, o nada si no caben de ninguna forma.
///
/// Devuelve el desplazamiento de una consola a la siguiente: en fila o en columna. **No es una
/// comodidad, es lo que hace que el banco se pueda correr**: dos consolas de ochenta columnas
/// necesitan más de 1 300 puntos de ancho, y una pantalla con el aumento del escritorio subido no
/// llega — pero sí le sobra alto. Y no se puede elegir antes de abrir la ventana, porque hasta
/// entonces no se sabe ni lo que mide la letra ni lo que da la pantalla.
fn disposicion(hueco: egui::Vec2, consola: egui::Vec2, cuantas: usize) -> Option<egui::Vec2> {
    let cuantas = cuantas as f32;
    if hueco.x >= consola.x * cuantas && hueco.y >= consola.y {
        Some(egui::vec2(consola.x, 0.0))
    } else if hueco.y >= consola.y * cuantas && hueco.x >= consola.x {
        Some(egui::vec2(0.0, consola.y))
    } else {
        None
    }
}

/// Lo medido en una fase del barrido.
struct Resultado {
    consolas: usize,
    lectura: Resumen,
    encolado: Resumen,
    /// El coste de CPU del fotograma entero, teselado y dibujo incluidos. El número que decide.
    fotograma: Resumen,
    fotogramas_por_segundo: f64,
}

/// La media y el peor de una serie de tiempos. Los dos, porque dicen cosas distintas: la media dice
/// cuánto cuesta, y el peor es el que se ve como un tirón.
struct Resumen {
    media: Duration,
    peor: Duration,
}

impl Resumen {
    fn de(tiempos: &[Duration]) -> Self {
        let total: Duration = tiempos.iter().sum();
        Self {
            media: total / tiempos.len().max(1) as u32,
            peor: tiempos.iter().copied().max().unwrap_or_default(),
        }
    }
}

struct Barrido {
    pantallas: Vec<Screen>,
    tema: Tema,
    etiqueta: Option<String>,
    fase: usize,
    vistos: usize,
    lecturas: Vec<Duration>,
    encolados: Vec<Duration>,
    /// Lo que `eframe` dice que costó de CPU el fotograma **anterior**. Va un fotograma por detrás
    /// de los otros dos, que en régimen continuo da igual y en el cambio de fase lo absorbe el
    /// calentamiento.
    fotogramas: Vec<Duration>,
    intervalos: Vec<Duration>,
    anterior: Option<Instant>,
    resultados: Vec<Resultado>,
    /// Lo que hacía falta y lo que había, si alguna vez las consolas no cupieron en la ventana. Una
    /// sola vez basta para invalidar la tanda: lo que no se ve no se dibuja, y el número saldría
    /// bien por no haber trabajado.
    no_cabe: Option<(egui::Vec2, egui::Vec2)>,
}

impl Barrido {
    /// Cierra la fase actual y pasa a la siguiente. Devuelve si el barrido ha terminado.
    fn cerrar_fase(&mut self) -> bool {
        let intervalo = Resumen::de(&self.intervalos).media.as_secs_f64();
        self.resultados.push(Resultado {
            consolas: PLAN[self.fase],
            lectura: Resumen::de(&self.lecturas),
            encolado: Resumen::de(&self.encolados),
            fotograma: Resumen::de(&self.fotogramas),
            fotogramas_por_segundo: if intervalo > 0.0 {
                1.0 / intervalo
            } else {
                0.0
            },
        });

        self.lecturas.clear();
        self.encolados.clear();
        self.fotogramas.clear();
        self.intervalos.clear();
        self.vistos = 0;
        self.anterior = None;
        self.fase += 1;
        self.fase == PLAN.len()
    }

    fn publicar(&self) {
        if let Some((hacia_falta, habia)) = self.no_cabe {
            println!(
                "SIN NÚMERO: las consolas no cabían enteras en la ventana.\n\
                 Hacían falta {:.0}×{:.0} puntos y había {:.0}×{:.0}.\n\
                 Lo que queda fuera no se dibuja, así que la medida diría que todo va bien\n\
                 precisamente por no haber pintado. Con una pantalla más grande sale el número.",
                hacia_falta.x, hacia_falta.y, habia.x, habia.y
            );
            return;
        }

        let perfil = if cfg!(debug_assertions) {
            "depuración — el número que vale es el de --release"
        } else {
            "release"
        };
        println!("\n── banco de repintado ──");
        println!(
            "rejilla:      {}×{} por consola",
            REJILLA.rows, REJILLA.cols
        );
        println!("letra:        monoespaciada de {CUERPO} puntos");
        println!("renderizador: glow");
        println!("perfil:       {perfil}");
        println!(
            "medida:       {MEDIDOS} fotogramas por valor de N, {CALENTAMIENTO} de calentamiento"
        );
        if let Some(etiqueta) = &self.etiqueta {
            println!("máquina:      {etiqueta}");
        }
        println!();

        for resultado in &self.resultados {
            println!(
                "N = {}  ·  leer {:>9.3?}  ·  encolar {:>9.3?}  ·  CPU del fotograma {:>9.3?} (peor {:>9.3?})  ·  {:.1} fps",
                resultado.consolas,
                resultado.lectura.media,
                resultado.encolado.media,
                resultado.fotograma.media,
                resultado.fotograma.peor,
                resultado.fotogramas_por_segundo,
            );
        }

        // La pregunta del hito no es cuánto cuesta una consola, sino **cómo crece**: si dos cuestan
        // el doble, el modo inmediato escala y el síntoma no aparece; si cuestan bastante más, ese
        // es exactamente el síntoma que el roadmap describe. Se compara con el coste de CPU del
        // fotograma entero y no con el encolado: el encolado no incluye el teselado, que es la mitad
        // cara del trabajo y la que más crece con el número de celdas.
        if let ([una], [dos]) = (&self.resultados[..1], &self.resultados[1..]) {
            let referencia = una.fotograma.media.as_secs_f64().max(f64::MIN_POSITIVE);
            let factor = dos.fotograma.media.as_secs_f64() / referencia;
            println!("\ndos consolas cuestan {factor:.2}× lo que cuesta una");

            // Cuánto del hueco entre fotogramas se está gastando. Es lo que dice si sobra margen o
            // se va justo, que los fotogramas por segundo no pueden decir: están topados por la
            // sincronía vertical, así que solo demuestran que el fotograma cabe.
            let hueco = 1.0 / dos.fotogramas_por_segundo.max(f64::MIN_POSITIVE);
            println!(
                "con dos, el fotograma gasta el {:.0} % de su hueco ({:.2} ms de {:.2} ms)",
                100.0 * dos.fotograma.media.as_secs_f64() / hueco,
                dos.fotograma.media.as_secs_f64() * 1e3,
                hueco * 1e3,
            );
        }
    }
}

impl eframe::App for Barrido {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // El barrido ha terminado y solo queda que la ventana se entere. El cierre no es inmediato
        // —el escritorio lo atiende cuando puede— así que todavía llegan fotogramas por aquí, y sin
        // esta salida el siguiente buscaría una fase que ya no existe.
        if self.fase == PLAN.len() {
            return;
        }

        // Régimen continuo: se pide el siguiente fotograma sin esperar a que nada cambie. Es lo
        // contrario de lo que hace el producto, y es el punto — aquí se mide el techo.
        ui.ctx().request_repaint();

        let ahora = Instant::now();
        let intervalo = self.anterior.replace(ahora).map(|previo| ahora - previo);

        let metrica = CellMetrics::measure(ui.ctx(), egui::FontId::monospace(CUERPO));
        let consolas = PLAN[self.fase];
        let hueco = ui.max_rect();
        let consola = egui::vec2(
            metrica.width() * REJILLA.cols as f32,
            metrica.height() * REJILLA.rows as f32,
        );

        // La disposición se decide **con el mayor N del plan y no con el de esta fase**: si las dos
        // consolas van a acabar apiladas, la de la primera fase tiene que pintarse en el mismo sitio
        // y del mismo modo, o se estarían comparando dos cosas distintas.
        let mayor = PLAN.iter().copied().max().unwrap_or(1);
        let paso = disposicion(hueco.size(), consola, mayor);

        // Se da la tanda por perdida solo en los fotogramas que cuentan: durante el calentamiento la
        // ventana todavía se está maximizando, y darla por pequeña ahí sería tirarla por algo que
        // está a punto de arreglarse solo.
        if paso.is_none() && self.vistos >= CALENTAMIENTO {
            // Se informa de la disposición **más barata de las dos**, no del producto de los dos
            // ejes: apilar necesita el ancho de una consola y el alto de todas, y decir que hacían
            // falta las dos cosas a la vez pediría una pantalla que no hace falta.
            self.no_cabe = Some((
                egui::vec2(consola.x, consola.y * mayor as f32),
                hueco.size(),
            ));
        }
        let paso = paso.unwrap_or_default();

        let mut lectura = Duration::ZERO;
        let mut encolado = Duration::ZERO;
        for (indice, pantalla) in self.pantallas[..consolas].iter().enumerate() {
            let antes = Instant::now();
            let vista = pantalla.view();
            let leido = antes.elapsed();

            let origen = hueco.left_top() + paso * indice as f32;
            let antes = Instant::now();
            // **Un descubridor de usar y tirar, y a propósito.** Este banco mide el pintado con texto que las
            // fuentes de fábrica ya cubren, así que nunca hay nada que descubrir; y como no llama a
            // `resolve`, uno persistente solo acumularía apuntes que nadie mira.
            paint(
                ui.painter(),
                &vista,
                &self.tema,
                &metrica,
                origen,
                &mut Default::default(),
            );

            lectura += leido;
            encolado += antes.elapsed();
        }

        self.vistos += 1;
        if self.vistos <= CALENTAMIENTO {
            return;
        }

        self.lecturas.push(lectura);
        self.encolados.push(encolado);
        // Lo que costó de CPU el fotograma anterior, con su teselado y su dibujo dentro. Es el
        // único de los tres que esta función no puede cronometrar por su cuenta: cuando el teselado
        // ocurre, ya ha terminado.
        if let Some(segundos) = frame.info().cpu_usage {
            self.fotogramas.push(Duration::from_secs_f32(segundos));
        }
        if let Some(intervalo) = intervalo {
            self.intervalos.push(intervalo);
        }

        if self.lecturas.len() >= MEDIDOS && self.cerrar_fase() {
            self.publicar();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.tema.fondo().to_normalized_gamma_f32()
    }
}
