//! Mide **lo que cuesta darle al terminal las fuentes de la máquina**, que es lo que decide cómo se
//! hace el resto del hito.
//!
//! ```text
//! cargo run --release --example fonts -p app
//! ```
//!
//! **No toca producción.** Igual que el banco del historial midió el motor por debajo de la fachada de
//! `term-core`, aquí se mide `fontique` y `skrifa` **antes** de que ninguna entre en la aplicación:
//! primero el número, y el número decide qué código hace falta.
//!
//! **En depuración los números salen varias veces peores.** El que vale es el de `--release`.
//!
//! ## Tres cosas que la primera versión de este banco hacía mal, y por qué importan
//!
//! 1. **Publicaba la primera familia que encontraba, y el orden no es estable.** Dos corridas seguidas
//!    dieron `Noto Sans Mono CJK JP` y `Noto Sans Mono CJK HK` para el mismo carácter, y `Noto Color
//!    Emoji` y `DejaVu Sans` para el mismo emoji. Un banco cuyo resultado cambia entre corridas no
//!    mide: **aquí se ordenan las familias por nombre** para que la respuesta sea reproducible.
//! 2. **Una sola muestra por punto no es una medida.** Enumerar dio 13,6 ms y 24,3 ms en dos corridas
//!    del mismo binario. Se toman varias y se publica la **mediana de las calientes**, como hizo el
//!    banco del historial después de que su revisión lo obligara.
//!
//!    **Pero eso no alcanza a la columna fría, y es el límite de este banco.** El frío **ocurre una vez
//!    por corrida** —la muestra 1 llena las cachés y las otras cuatro ya las encuentran llenas—, así que
//!    en frío no hay cinco muestras que promediar: hay **una**. Y es justo la que la vía perezosa paga.
//!    Su dispersión solo se ve **repitiendo la corrida**: tres seguidas con la máquina en reposo movieron
//!    el número que decide de 3,4 a 4,8 ms. Por eso **D24** publica la columna fría como **rango de
//!    corridas** y no como un valor, y por eso quien repita esta medida tiene que correrlo **más de una
//!    vez**.
//! 3. **«Cubre el carácter» no es «lo pinta a color».** `DejaVu Sans` mapea 😀 a un glifo **monocromo**
//!    y gana por orden alfabético. Es exactamente el defecto que Alacritty tiene documentado —coge el
//!    glifo de DejaVu y solo recurre a Noto Emoji si aquél no lo trae—, así que aquí se pregunta
//!    **además** si esa cara tiene tablas de color.
//!
//! ## Las cuatro preguntas que contesta
//!
//! 1. **Cuánto tarda enumerar las fuentes de la máquina.** Decide si el descubrimiento se paga **al
//!    arrancar** —donde ya hay 5,42 ms medidos de restaurar la disposición— o solo cuando aparece un
//!    carácter que nadie cubre.
//! 2. **Cuánto cuesta encontrar quién cubre un carácter**, por las dos vías: recorriéndolo todo y
//!    preguntando por script o por familia genérica.
//! 3. **Cuánto cuesta registrar las caras en el motor de texto, y cuánta memoria se lleva.** Es lo que
//!    se pagaría al abrir la ventana, y va aparte de enumerar porque son dos costes distintos.
//! 4. **Cuánto ocupa un glifo de color y de qué forma viene.** Decide el techo de la caché y si el
//!    emoji se reescala desde un mapa de bits o se dibuja al tamaño que se pida.
//!
//! **Lo que este banco NO contesta, y hay que decirlo:** qué cuesta *pintar* el glifo ya rasterizado.
//! Eso vive en el fotograma y se mide con una tanda, no aquí.

use std::time::{Duration, Instant};

use eframe::egui;
use fontique::{Collection, CollectionOptions, GenericFamily, Script, SourceCache};

/// Cuántas veces se repite cada medida dentro de una corrida. Repetir en vez de medir una vez es lo que
/// el banco del historial acabó haciendo después de que su revisión tumbara la muestra única.
///
/// **Cinco muestras dan una mediana caliente de cuatro y un frío de uno.** Subir esta constante no mejora
/// el frío: sigue habiendo una sola muestra fría por corrida, porque la segunda ya encuentra las cachés
/// llenas. Lo que mejora el frío es **repetir la corrida**.
const MUESTRAS: usize = 5;

/// El cuerpo de la fuente con la que este terminal pinta hoy, copiado de la aplicación.
///
/// **La altura de la celda NO se inventa: se le pregunta al motor de texto**, que es la regla que el
/// pintor tiene escrita —«la medida de la celda se pregunta, no se inventa»—. La primera versión de este
/// banco puso un 17 a ojo, y encima lo trataba como píxeles cuando es **puntos**: con el escritorio al
/// 200 % la celda son el doble de píxeles y la caché **cuatro veces** más grande, que es justo el número
/// del que saldría su techo. Lo destapó la revisión.
const CUERPO: f32 = 14.0;

/// Los aumentos de escritorio con los que se publica el tamaño de la caché.
///
/// **Dos y no uno**, porque el techo se dimensiona en píxeles y el escritorio decide cuántos hay por
/// punto. **D10** ya estableció que el zoom no puede cambiar estas cifras en silencio.
const AUMENTOS: [f32; 2] = [1.0, 2.0];

/// Los caracteres con los que se pregunta cobertura, y por qué cada uno.
///
/// **Ninguno es decorativo:** cada uno representa una de las cosas que hoy salen como cuadro.
const MUESTRA: [(char, &str); 5] = [
    ('日', "CJK — el caso de D3"),
    ('한', "hangul — otra cobertura, otra fuente"),
    (
        '\u{e0b0}',
        "Área de Uso Privado — el icono de un `ls` moderno",
    ),
    ('😀', "emoji con presentación de color por defecto"),
    ('☁', "emoji con presentación de TEXTO por defecto"),
];

fn main() {
    println!("── qué cuesta darle al terminal las fuentes de la máquina ──");
    println!(
        "en frío: LA muestra 1, una sola — en caliente: la mediana de las {} restantes",
        MUESTRAS - 1
    );
    println!(
        "el frío ocurre una vez por corrida: para saber cuánto se mueve, repite esta corrida\n"
    );

    // ── 1. Enumerar ────────────────────────────────────────────────────────────────────────────
    //
    // **Se cronometra la construcción entera**, que es lo que un arranque haría: medir por dentro daría
    // un número que nadie paga.
    let mut tiempos = Vec::new();
    let mut cuantas = 0;
    for _ in 0..MUESTRAS {
        let empezo = Instant::now();
        let mut coleccion = Collection::new(CollectionOptions {
            system_fonts: true,
            ..Default::default()
        });
        cuantas = coleccion.family_names().count();
        tiempos.push(empezo.elapsed());
    }
    println!("1. enumerar las fuentes del sistema");
    println!("   {}  ·  {cuantas} familias", resumen(&tiempos));
    if cuantas == 0 {
        println!("   ⚠ NINGUNA familia: o no hay fontconfig en ejecución, o no hay fuentes.");
        println!("     Es el caso que AC9 obliga a aguantar, y este banco acaba de reproducirlo.");
        return;
    }

    let mut coleccion = Collection::new(CollectionOptions {
        system_fonts: true,
        ..Default::default()
    });
    // **Ordenadas por nombre**, que es lo que hace la respuesta reproducible entre corridas.
    let mut familias: Vec<String> = coleccion.family_names().map(str::to_owned).collect();
    familias.sort();
    let mut fuentes = SourceCache::default();

    // ── 2. Quién cubre qué, recorriéndolo todo ─────────────────────────────────────────────────
    println!("\n2. quién cubre qué, abriendo las {cuantas} familias en orden (el peor caso)");
    for (que, porque) in MUESTRA {
        let mut tiempos = Vec::new();
        let mut quien = None;
        for _ in 0..MUESTRAS {
            let empezo = Instant::now();
            quien = quien_cubre(&mut coleccion, &mut fuentes, &familias, que);
            tiempos.push(empezo.elapsed());
        }
        // **Y si la ganadora lo pinta a color, porque ahí estaba el hallazgo.** Sin esta columna, el
        // banco decía «cubierto» y callaba que quien gana el emoji en esta máquina lo pinta en blanco y
        // negro teniendo una fuente de color instalada.
        let cual = quien.as_ref().map_or_else(
            || "(nadie lo cubre)".to_owned(),
            |q| {
                let color = if q.pinta_a_color(que) {
                    " [a color]"
                } else {
                    " [monocromo]"
                };
                format!("{}{color}", q.familia)
            },
        );
        println!("   {que}  {}  {cual}   ({porque})", resumen(&tiempos));
    }

    // ── 3. Las dos vías rápidas ────────────────────────────────────────────────────────────────
    //
    // **Recorrerlo todo es el peor caso a propósito; esto es lo que la aplicación usaría.** La
    // diferencia entre los dos números decide cómo se escribe la entrega que hace que CJK se vea.
    println!("\n3. las vías rápidas: candidatas por script y por familia genérica");
    for (que, script) in [('日', "Hani"), ('한', "Hang")] {
        let mut tiempos = Vec::new();
        let mut candidatas = 0;
        let mut quien = None;
        for _ in 0..MUESTRAS {
            let empezo = Instant::now();
            let ids: Vec<_> = coleccion
                .fallback_families(Script::parse(script).expect("código de script válido"))
                .collect();
            candidatas = ids.len();
            quien = primera_que_cubre(&mut coleccion, &mut fuentes, &ids, que);
            tiempos.push(empezo.elapsed());
        }
        let cual = quien.as_ref().map_or_else(
            || "(ninguna candidata lo cubre)".to_owned(),
            |q| q.familia.clone(),
        );
        println!(
            "   {que}  {}  {cual}   ({candidatas} candidatas por script)",
            resumen(&tiempos)
        );
    }
    // **El emoji NO sale por script**: `Zsym` no lo cubre en esta máquina. La vía que sí es la familia
    // genérica que el sistema declara para eso.
    let mut tiempos = Vec::new();
    let mut candidatas = 0;
    let mut quien = None;
    for _ in 0..MUESTRAS {
        let empezo = Instant::now();
        let ids: Vec<_> = coleccion.generic_families(GenericFamily::Emoji).collect();
        candidatas = ids.len();
        // **Con la regla que la decisión concluye, no con «la primera que cubra».** Y eso fue una
        // corrección de revisión: midiendo solo la cobertura, el número no incluía seguir buscando
        // cuando la primera candidata resulta ser monocroma — que es exactamente el caso que este banco
        // descubrió recorriendo en orden. Aquí funcionaba por suerte, porque en esta máquina la de color
        // está la primera; en otra no lo estaría, y el número publicado sería el de un camino más corto
        // que el real.
        quien = primera_a_color(&mut coleccion, &mut fuentes, &ids, '😀');
        tiempos.push(empezo.elapsed());
    }
    let cual = quien
        .as_ref()
        .map_or_else(|| "(ninguna lo cubre)".to_owned(), |q| q.familia.clone());
    println!(
        "   😀  {}  {cual}   ({candidatas} candidatas por familia genérica)",
        resumen(&tiempos)
    );

    // ── 4. Registrar las caras en el motor de texto ────────────────────────────────────────────
    //
    // **Es lo que se pagaría al abrir la ventana**, y va aparte de enumerar porque son dos costes
    // distintos: uno lo cobra el sistema y el otro el framework.
    println!("\n4. registrar las caras en el motor de texto");
    registrar(&mut coleccion, &mut fuentes, &familias);

    // ── 4bis. Registrar una cara EN CALIENTE ───────────────────────────────────────────────────
    //
    // **Éste es el número que decide entre las dos vías**, y sin él la decisión sería una preferencia.
    // Si el descubrimiento es perezoso, la cara nueva se registra con la ventana ya abierta: el motor
    // de texto se rehace y ese coste **cae dentro de un fotograma**, no antes del primero. Un fotograma
    // son 16,7 ms a 60 Hz, así que lo que salga aquí dice si se nota como un tirón o no.
    println!("\n4bis. registrar UNA cara más con la ventana ya abierta (la vía perezosa)");
    en_caliente(&mut coleccion, &mut fuentes);

    // ── 5. El glifo de color ───────────────────────────────────────────────────────────────────
    println!("\n5. de qué forma viene un glifo de color, y cuánto ocupa");
    match quien {
        Some(cara) => medir_el_color(&cara),
        None => println!("   ninguna familia de emoji cubre 😀 en esta máquina"),
    }

    println!("\n── lo que este banco NO dice ──");
    println!(
        "qué cuesta PINTAR el glifo ya rasterizado: eso vive en el fotograma y lo mide una tanda."
    );
}

/// Una cara concreta de una fuente, con lo que hace falta para preguntarle cosas.
///
/// **Lleva el `Blob` prestado y no una copia de los bytes**, y eso fue un hallazgo de revisión: copiar
/// el fichero de fuente **dentro del cronómetro** hacía que los tiempos midieran el asignador de memoria
/// y no la búsqueda. Con `Noto Sans CJK HK` —19,5 MiB— la copia costaba 5 ms y el número publicado
/// seguía el **tamaño del fichero**, no cuántas familias se habían recorrido. Un `Blob` es un puntero
/// contado, que es lo que la aplicación tendría.
struct Cara {
    familia: String,
    blob: fontique::Blob<u8>,
    indice: u32,
}

impl Cara {
    /// Si esa cara trae el glifo de ese carácter **a color**.
    ///
    /// **Se pregunta FUERA del cronómetro**, por lo mismo que la copia: sondear las tablas de color es
    /// abrir la fuente entera con `skrifa`, y dentro del reloj inflaba todas las medidas de búsqueda.
    fn pinta_a_color(&self, que: char) -> bool {
        use skrifa::MetadataProvider as _;
        skrifa::FontRef::from_index(self.blob.data(), self.indice).is_ok_and(|fuente| {
            let Some(glifo) = fuente.charmap().map(que) else {
                return false;
            };
            let hay_bitmap = fuente
                .bitmap_strikes()
                .glyph_for_size(skrifa::instance::Size::unscaled(), glifo)
                .is_some();
            hay_bitmap || fuente.color_glyphs().get(glifo).is_some()
        })
    }
}

/// La primera familia, en orden, cuya cara cubre ese carácter.
fn quien_cubre(
    coleccion: &mut Collection,
    fuentes: &mut SourceCache,
    familias: &[String],
    que: char,
) -> Option<Cara> {
    let ids: Vec<_> = familias
        .iter()
        .filter_map(|nombre| coleccion.family_id(nombre))
        .collect();
    primera_que_cubre(coleccion, fuentes, &ids, que)
}

/// La primera de esas familias cuya cara cubre el carácter **y lo pinta a color**.
///
/// **Es la regla que la decisión concluye, y por eso se mide así**: «la primera que cubra» se lleva el
/// glifo aunque sea monocroma, que es el defecto que este banco encontró y que Alacritty tiene
/// documentado. Sondear el color cuesta abrir la fuente con `skrifa`, así que este camino es **más caro**
/// que el de solo cobertura — y ése es justo el número que hace falta.
fn primera_a_color(
    coleccion: &mut Collection,
    fuentes: &mut SourceCache,
    ids: &[fontique::FamilyId],
    que: char,
) -> Option<Cara> {
    for &id in ids {
        let Some((nombre, caras)) = coleccion
            .family(id)
            .map(|familia| (familia.name().to_owned(), familia.fonts().to_vec()))
        else {
            continue;
        };
        for cara in caras {
            let Some(blob) = fuentes.get(cara.source()) else {
                continue;
            };
            let candidata = Cara {
                familia: nombre.clone(),
                blob,
                indice: cara.index(),
            };
            if candidata.pinta_a_color(que) {
                return Some(candidata);
            }
        }
    }
    None
}

/// La primera de esas familias cuya cara cubre el carácter.
fn primera_que_cubre(
    coleccion: &mut Collection,
    fuentes: &mut SourceCache,
    ids: &[fontique::FamilyId],
    que: char,
) -> Option<Cara> {
    for &id in ids {
        // **Se clonan las caras antes de seguir**: la familia presta de la colección, y la colección
        // hace falta en la vuelta siguiente.
        let Some((nombre, caras)) = coleccion
            .family(id)
            .map(|familia| (familia.name().to_owned(), familia.fonts().to_vec()))
        else {
            continue;
        };
        for cara in caras {
            let Some(blob) = fuentes.get(cara.source()) else {
                continue;
            };
            let cubre = cara
                .charmap_index()
                .charmap(blob.data())
                .is_some_and(|mapa| mapa.map(que as u32).is_some_and(|glifo| glifo != 0));
            if cubre {
                return Some(Cara {
                    familia: nombre,
                    blob,
                    indice: cara.index(),
                });
            }
        }
    }
    None
}

/// Qué cuesta registrar en el motor de texto las caras que hacen falta, y cuánto pesan.
fn registrar(coleccion: &mut Collection, fuentes: &mut SourceCache, familias: &[String]) {
    let necesarias: Vec<Cara> = MUESTRA
        .iter()
        .filter_map(|(que, _)| quien_cubre(coleccion, fuentes, familias, *que))
        .collect();

    let mut definiciones = egui::FontDefinitions::default();
    let mut bytes_totales = 0usize;
    let mut registradas = 0usize;
    for cara in &necesarias {
        if definiciones.font_data.contains_key(&cara.familia) {
            continue;
        }
        bytes_totales += cara.blob.data().len();
        definiciones.font_data.insert(
            cara.familia.clone(),
            std::sync::Arc::new(egui::FontData::from_owned(cara.blob.data().to_vec())),
        );
        definiciones
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push(cara.familia.clone());
        registradas += 1;
    }

    // **Varias muestras, como todo lo demás.** La primera versión publicaba una sola bajo el cartel de
    // «mediana de cinco», que es la misma regla que este módulo prohíbe en su punto 2 — y se movía un
    // 16 % entre corridas.
    let mut tiempos = Vec::new();
    for _ in 0..MUESTRAS {
        let ctx = egui::Context::default();
        let empezo = Instant::now();
        ctx.set_fonts(definiciones.clone());
        // **Se fuerza una pasada**: `set_fonts` guarda la configuración y el motor de texto se rehace en
        // el fotograma siguiente. Cronometrar solo la llamada publicaría un cero que no paga nadie.
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("日本語 ☁ 😀");
        })
        .drop_without_applying_deltas();
        tiempos.push(empezo.elapsed());
    }
    println!(
        "   {}  ·  {registradas} caras  ·  {:.1} MiB de fuente en memoria",
        resumen(&tiempos),
        bytes_totales as f64 / (1024.0 * 1024.0)
    );
    println!(
        "   y esa memoria es el fichero de fuente ENTERO por cara, no solo los glifos que se usan"
    );
}

/// Qué cuesta añadir una cara al motor de texto **con la ventana ya andando**.
fn en_caliente(coleccion: &mut Collection, fuentes: &mut SourceCache) {
    // **La cara que devuelve LA VÍA ELEGIDA, no la del recorrido alfabético**, y ésa fue una corrección
    // de revisión. Las dos vías dan fuentes distintas —`Droid Sans Fallback` de 3,8 MiB contra
    // `Noto Sans CJK JP` de 19,5 MiB— así que sumar «buscar por script» con «registrar lo que encontró
    // el recorrido» era sumar dos mitades de gestos distintos, y la segunda con una fuente cinco veces
    // más pequeña.
    let ids: Vec<_> = coleccion
        .fallback_families(Script::parse("Hani").expect("código de script válido"))
        .collect();
    let Some(cara) = primera_que_cubre(coleccion, fuentes, &ids, '日') else {
        println!("   no hay ninguna cara que cubra 日 por la vía elegida en esta máquina");
        return;
    };

    let mut definiciones = egui::FontDefinitions::default();
    definiciones.font_data.insert(
        cara.familia.clone(),
        std::sync::Arc::new(egui::FontData::from_owned(cara.blob.data().to_vec())),
    );
    definiciones
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push(cara.familia.clone());

    let mut tiempos = Vec::new();
    for _ in 0..MUESTRAS {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("hola");
        })
        .drop_without_applying_deltas();
        let empezo = Instant::now();
        ctx.set_fonts(definiciones.clone());
        // El fotograma en el que el motor de texto se rehace **y** se pinta algo que usa la cara nueva:
        // eso es lo que el usuario vería como un tirón, si lo hubiera.
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("日本語");
        })
        .drop_without_applying_deltas();
        tiempos.push(empezo.elapsed());
    }
    let cuanto = *tiempos.iter().max().expect("hay muestras");

    println!(
        "   {}  ·  {} ({:.1} MiB)",
        resumen(&tiempos),
        cara.familia,
        cara.blob.data().len() as f64 / (1024.0 * 1024.0)
    );
    // **Se juzga contra el PEOR de los tiempos, no contra el mejor.** Lo que decide si se ve como un
    // tirón es el fotograma malo, no el bueno.
    let presupuesto = Duration::from_micros(16_667);
    if cuanto > presupuesto {
        println!(
            "   ⚠ pasa del presupuesto de un fotograma a 60 Hz ({:.1}×): se vería como un tirón",
            cuanto.as_secs_f64() / presupuesto.as_secs_f64()
        );
    } else {
        println!(
            "   cabe en un fotograma a 60 Hz ({:.0} % de los 16,7 ms)",
            100.0 * cuanto.as_secs_f64() / presupuesto.as_secs_f64()
        );
    }
}

/// La altura de una celda, preguntada al motor de texto como hace el pintor.
fn alto_de_celda() -> f32 {
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.label("M");
    })
    .drop_without_applying_deltas();
    ctx.fonts_mut(|fuentes| fuentes.row_height(&egui::FontId::monospace(CUERPO)))
}

/// De qué forma viene el glifo de color y cuánto ocupa.
fn medir_el_color(cara: &Cara) {
    use skrifa::MetadataProvider as _;
    use skrifa::instance::Size;

    println!(
        "   {} · ¿trae color? {}",
        cara.familia,
        si_o_no(cara.pinta_a_color('😀'))
    );
    let Ok(fuente) = skrifa::FontRef::from_index(cara.blob.data(), cara.indice) else {
        println!("   no se pudo leer la fuente");
        return;
    };
    let Some(glifo) = fuente.charmap().map('😀') else {
        return;
    };

    let alto = alto_de_celda();
    let bitmaps = fuente.bitmap_strikes();
    let encontrado = bitmaps
        .glyph_for_size(Size::new(alto), glifo)
        .or_else(|| bitmaps.glyph_for_size(Size::unscaled(), glifo));
    match encontrado {
        Some(mapa) => {
            let pixeles = mapa.width as usize * mapa.height as usize;
            println!(
                "   mapa de bits {}×{} px  ·  {} KiB una vez DESCODIFICADO a RGBA",
                mapa.width,
                mapa.height,
                pixeles * 4 / 1024
            );
            println!(
                "   (la fuente lo guarda comprimido: {:.1} MiB el fichero entero con todos sus glifos)",
                cara.blob.data().len() as f64 / (1024.0 * 1024.0)
            );
            println!("   celda de {alto:.1} pt de alto, preguntada al motor de texto:");
            for aumento in AUMENTOS {
                let lado = (alto * aumento) as usize;
                let uno = lado * lado * 4;
                println!(
                    "     al {:>4.0} %: {lado}×{lado} px · {uno} B por glifo · {} KiB mil glifos",
                    aumento * 100.0,
                    uno * 1000 / 1024
                );
            }
        }
        None => println!(
            "   sin mapa de bits: su color viene por contornos y se dibuja al tamaño que se pida"
        ),
    }
}

fn si_o_no(que: bool) -> &'static str {
    if que { "sí" } else { "no" }
}

/// **La primera muestra aparte de la mediana de las demás**, y ésa fue una corrección de revisión.
///
/// `SourceCache` guarda los ficheros que abre y la búsqueda por script tiene su propia caché, así que la
/// muestra 1 es **fría** y las demás **calientes**. Publicar la mediana de las cinco daba el número
/// caliente bajo un cartel que decía «mediana de cinco», y **la vía perezosa paga la fría** — una vez por
/// escritura nueva, que es exactamente lo que ocurre en producción. Medido: 875 µs en caliente contra
/// 5,2 ms en frío para el mismo carácter. Publicar solo el primero subestimaba el número que sostiene la
/// decisión **por cuatro**.
fn resumen(tiempos: &[Duration]) -> String {
    let fria = tiempos[0];
    let mut calientes: Vec<Duration> = tiempos[1..].to_vec();
    calientes.sort_unstable();
    let mediana = calientes[calientes.len() / 2];
    format!("{fria:>11.3?} en frío · {mediana:>11.3?} en caliente")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(cuanto: u64) -> Duration {
        Duration::from_millis(cuanto)
    }

    /// Cinco muestras como salen de una máquina cargada: **la fría no es la más lenta** —hay una
    /// caliente que la dobla— y **no llegan ordenadas**. Con muestras ordenadas, o con la fría siendo
    /// siempre el máximo, casi cualquier forma equivocada de resumirlas da el mismo número que la
    /// correcta y la prueba no ata nada.
    ///
    /// Fría = 3 ms (la primera). Calientes = 12, 8, 5, 1 → ordenadas 1, 5, 8, 12 → mediana **8 ms**.
    fn muestras() -> [Duration; 5] {
        [ms(3), ms(12), ms(8), ms(5), ms(1)]
    }

    /// Los dos números que el banco publica, ya separados y sin el relleno del formato.
    fn publicado(tiempos: &[Duration]) -> (String, String) {
        let linea = resumen(tiempos);
        let (frio, caliente) = linea
            .split_once(" · ")
            .expect("el resumen publica los DOS números, el frío y el caliente");
        (frio.trim().to_owned(), caliente.trim().to_owned())
    }

    #[test]
    fn la_muestra_en_frio_es_la_primera_y_no_la_mas_lenta() {
        // **Frío es una posición, no una magnitud.** La caché de ficheros y la de búsqueda por script se
        // llenan en la muestra 1, así que la fría es ésa por construcción; en una máquina cargada
        // cualquier muestra caliente puede salir la más lenta de las cinco. Publicar «la peor» bajo el
        // cartel de «en frío» le colgaría a la vía perezosa un coste que no paga, y ése es el número con
        // el que **D24** decide entre descubrir al arrancar o descubrir cuando haga falta.
        let (frio, _) = publicado(&muestras());

        assert_eq!(
            frio, "3.000ms en frío",
            "lo publicado en frío no es la primera muestra"
        );
    }

    #[test]
    fn la_mediana_caliente_ordena_las_muestras_y_deja_fuera_la_fria() {
        // **Dos mitades, y las dos publican un número distinto si se caen.** Sin ordenar, «mediana» es la
        // muestra que cayó en medio del vector —5 ms aquí— y el número se mueve con el orden de llegada,
        // que es lo que la corrección 2 de este banco prohíbe. Y metiendo la fría en el montón, la
        // mediana se contamina con la única muestra que paga las cachés vacías: los 5,2 ms contra 875 µs
        // que este banco midió para el mismo carácter son la razón de que vayan separadas.
        let (_, caliente) = publicado(&muestras());

        assert_eq!(
            caliente, "8.000ms en caliente",
            "la mediana caliente no salió de ordenar las cuatro muestras que quedan al quitar la fría"
        );
    }
}
