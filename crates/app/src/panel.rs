//! El panel de estado de una división: lo que su productor publica, pintado a la derecha de su
//! consola.
//!
//! **Este módulo no decide qué significa nada.** El vocabulario —las marcas, los estilos, la fase— es
//! del contrato y del productor; aquí solo se traduce a color y a posición. Esa raya es D6, y es la
//! razón de que el panel valga para un agente, un despliegue o un script sin saber nada de ninguno.
//!
//! Lo que este módulo **todavía no hace**, y llega con el hito: alinear la columna de referencias por
//! bloques, envolver al ancho con su sangría de continuación, y pintar el símbolo de cada marca. Hasta
//! entonces cada línea es una fila y lo que no cabe se recorta.

use std::time::{Duration, SystemTime};

use eframe::egui::{Align2, Color32, Painter, Pos2, Rect};
use panel_contract::{Attention, Line, Mark, Panel, Problem, Style};

use crate::painter::CellMetrics;
use crate::theme::Theme;

/// Con cuántas celdas de la fuente nace el panel de una división.
///
/// **En celdas y no en puntos**: un ancho en puntos se estrecha en columnas al subir el aumento del
/// escritorio, y lo que el panel tiene que conservar es **cuántos caracteres caben**, que es lo que
/// decide si su texto se lee. Con treinta y seis caben las líneas de los `panel.json` reales del repo
/// sin que la consola se quede sin sus ochenta columnas de siempre en una ventana normal.
///
/// **De fábrica, no fijo**: desde que se puede arrastrar la frontera del panel, esto es solo el valor
/// con el que empieza una división que nadie ha tocado. El que manda es el de cada división.
///
/// El razonamiento entero —por qué en celdas, por qué el que cede es el panel, que el ancho es de cada
/// división, qué es el mínimo del gesto y por qué plegar es un estado aparte— está en **D10** de
/// `docs/02-decisiones.md`, con lo que se descartó en cada caso.
pub(crate) const DEFAULT_WIDTH: usize = 36;

/// Lo menos que se le deja a la consola cuando su panel le quita sitio, en celdas.
///
/// **El panel cede, no la consola.** El panel es margen y la consola es el trabajo: en una ventana
/// estrecha lo que sobra es el estado, no el programa. Con veinte columnas una consola sigue siendo
/// usable; con cinco, no.
///
/// **Y es también el suelo que decide si una división se puede partir en dos**, que es el otro sitio
/// donde hace falta saber cuándo una consola deja de servir. Son la misma pregunta, así que son la misma
/// constante: dos números para «lo menos que necesita una consola» acabarían diciendo cosas distintas.
pub(crate) const MINIMUM_CONSOLE: f32 = 20.0;

/// Lo menos que se le deja al panel cuando se arrastra su frontera, en celdas.
///
/// **Con una celda el panel casi no se ve, y eso es lo que se busca: que siempre quede dónde agarrar.**
/// Llegando a cero, la frontera se iría con el panel y el gesto que lo cerró no se podría deshacer con
/// el ratón — un viaje sin billete de vuelta.
///
/// Es el suelo del **gesto**, no del reparto: [`set_aside`] sigue pudiendo devolver un panel **más estrecho
/// que esto** —hasta de ancho cero— cuando la división no da para más. Son dos preguntas distintas, lo
/// que el usuario pide y lo que cabe, y confundirlas dejaría a la consola por debajo de su mínimo. (Lo
/// que `set_aside` no devuelve nunca es *ningún* panel una vez se le pide un ancho: en el caso extremo
/// devuelve el rectángulo vacío, y su prueba lo dice así.)
///
/// **Y desde que se puede plegar, esto es exactamente la raya entre estrechar y plegar**: pedir menos que
/// esto es lo que el gesto lee como «ciérralo». Antes de que el plegado existiera, este mismo número
/// servía para lo contrario —impedir llegar a cero, porque la frontera se habría ido con el panel— y esa
/// razón ya no es la suya: ahora lo que sostiene el regreso es el asidero que sobrevive al plegado.
pub(crate) const DRAG_FLOOR: usize = 1;

/// Lo que se deja de margen a la izquierda del texto, en celdas.
///
/// Una celda y no cero: pegado al borde, el texto se lee como un error de pintado. A la derecha no
/// hace falta, porque ahí no hay nada — el panel es lo último de la división.
const MARGIN: f32 = 1.0;

/// Cómo queda repartido el rectángulo de una división entre lo que se ve.
///
/// Tiene nombre, y no es una tupla, por lo mismo que lo tiene [`crate::Tiling`], que hace lo propio
/// entre las dos consolas: dos repartos en el mismo fotograma que se leyeran distinto serían dos
/// idiomas para una sola idea.
pub struct PanelTiling {
    /// Lo que le queda a la consola, que es contra lo que hay que medir su rejilla.
    pub console: Rect,
    /// Dónde va el panel, y **el panel que va ahí**. Van juntos porque no existe el uno sin el otro:
    /// separarlos deja escribible una rama —hueco sin panel— que no puede ocurrir.
    pub panel: Option<Rect>,
}

/// Parte el rectángulo de una división entre su consola y su panel.
///
/// **El descuento va aquí y no después de medir la rejilla**, que es lo que hace que la consola mida
/// el hueco que le queda de verdad: midiendo primero el rectángulo entero, el programa de dentro
/// creería tener columnas que el panel le ha quitado y escribiría debajo de él.
///
/// Sin panel que pintar, la consola se queda con todo y no hay segundo rectángulo: es lo que hace que
/// una división cuyo productor no ha publicado nada se vea exactamente igual que antes de que el panel
/// existiera.
///
/// Con panel, el hueco es lo que sobre después del suelo de la consola, así que en una ventana
/// estrecha el que se estrecha es el panel y puede llegar a desaparecer. Nunca al revés.
///
/// **`panel_cells` lleva las dos cosas en un solo parámetro, y no es por brevedad.** «Si hay algo
/// que enseñar» y «cuánto ancho se lleva» son la misma decisión tomada en el mismo sitio: con dos
/// parámetros existe el estado escribible «no hay panel, pero con treinta y seis celdas», y este
/// módulo ya pagó una vez tener **dos criterios paralelos** para si el panel se ve — uno en el reparto
/// del ancho y otro en el pintado, que se contradecían.
///
/// El nombre dice la unidad a propósito: aquí conviven celdas y puntos, y un mismo nombre para las dos
/// deja al lector sin saber cuál está mirando.
pub fn set_aside(pane: Rect, cell_width: f32, panel_cells: Option<usize>) -> PanelTiling {
    let Some(cells) = panel_cells else {
        return PanelTiling {
            console: pane,
            panel: None,
        };
    };

    let in_points = cells as f32 * cell_width;
    let available = (pane.width() - MINIMUM_CONSOLE * cell_width).max(0.0);
    let cut = pane.right() - in_points.min(available);

    PanelTiling {
        console: Rect::from_min_max(pane.left_top(), Pos2::new(cut, pane.bottom())),
        panel: Some(Rect::from_min_max(
            Pos2::new(cut, pane.top()),
            pane.right_bottom(),
        )),
    }
}

/// Lo que una división reclama ahora mismo.
///
/// **Dos cosas y no una**: *qué* reclama —que lo dice su productor y la aplicación no deduce— y si hay
/// alguien **sosteniéndolo**, es decir si lo que se enseña es de fiar o es lo último que llegó de un
/// productor que ya no contesta. Las dos caben en un solo símbolo, que es lo que hace falta donde no
/// hay sitio para una frase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttentionState {
    /// Cuál de los tres del contrato.
    pub class: Attention,
    /// Si hay alguien detrás de lo que este panel dice.
    ///
    /// Sale del mismo sitio que el aviso que se pinta debajo, y no de un segundo criterio de «esto ya
    /// no vale»: con dos, el día que discrepen el panel diría que el reclamo es de fiar mientras su
    /// propia línea confiesa que no. Cubre las dos formas de quedarse sin nadie detrás —el silencio
    /// largo y el fichero que dejó de leerse—, que en pantalla significan lo mismo.
    ///
    /// **Con una excepción, y está en el registro de decisiones**: lo *terminado* no se queda sin nadie
    /// detrás por callar, porque callar es lo que hace quien terminó.
    pub backed: bool,
}

/// Cuál de dos reclamos gana cuando hay que enseñar **uno solo** — la ficha de una pestaña tiene un
/// símbolo y sus divisiones pueden reclamar cosas distintas.
///
/// **Manda la clase, y a igualdad de clase gana el que alguien sostiene.** Con dos divisiones esperándote
/// y una de ellas viva, la ficha tiene que decir que hay alguien esperando de verdad: pintar el
/// atenuado sería mandarte a la que está muerta.
///
/// **A mano y no derivado**, por lo mismo que el orden de [`Attention`]: un `Ord` derivado ordena por el
/// orden de los campos en el fichero, así que quien los reordenara —o metiera uno en medio— cambiaría
/// cuál gana **en silencio**.
impl Ord for AttentionState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // **Se destructura para que un campo nuevo no compile.** Comparando por `self.campo` a mano, el
        // día que este tipo gane un tercer campo el orden lo ignoraría en silencio y dejaría de ser
        // coherente con el `Eq` derivado — que es justo lo que el orden de [`Attention`] evita con su
        // `match` exhaustivo, y la razón de que aquí no haya un `derive`.
        let Self { class, backed } = self;
        class.cmp(&other.class).then(backed.cmp(&other.backed))
    }
}

impl PartialOrd for AttentionState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Lo que una división sabe del panel de su productor.
///
/// **Guarda el último panel bueno a propósito.** El productor machaca el fichero entero en cada
/// publicación —así lo dice su contrato— así que una lectura puede caer justo en medio y no encontrar
/// JSON válido. Sin esto el panel parpadearía a vacío en cada escritura, que es un fallo mucho más
/// visible que el que evita. **Y por eso mismo hay que confesarlo**: conservar el último bueno sin
/// decirlo esconde a un productor roto detrás de un resumen plausible de hace una hora.
#[derive(Default)]
pub struct PanelState {
    last_good: Option<Panel>,
    /// Lo maquetado, y para qué se maquetó.
    ///
    /// **Existe por coste, y el escenario es presente**: mientras una consola vuelca, la ventana
    /// repinta todos los fotogramas, y envolver el panel al ancho se recalcularía sesenta veces por
    /// segundo sin que su fichero haya cambiado.
    layout: Option<PanelLayout>,
    /// Por qué falló la última lectura, si falló. Es lo que distingue «aquí no hay productor» de «hay
    /// uno y está roto», que se ven igual y no son lo mismo.
    problem: Option<Problem>,
}

/// Una maqueta y las dos cosas de las que depende: si cambia cualquiera, hay que rehacerla.
struct PanelLayout {
    width: usize,
    /// **La línea que pone la app entra en la clave de la caché.** Un panel que envejece cambia lo que
    /// hay que pintar sin que su fichero se haya tocado, así que una caché que solo mirara el ancho
    /// seguiría enseñando la maqueta de cuando el panel era fresco — y el aviso no aparecería nunca.
    warning: Option<String>,
    rows: Vec<Row>,
}

impl PanelState {
    /// Vuelve a leer el panel si su fichero ha cambiado.
    ///
    /// **No cronometra nada**: quien mide es la ventana, con un solo reloj alrededor de leer y pintar.
    /// Dos relojes para un tramo que se publica como uno es la forma más fácil de que el número deje de
    /// significar lo que dice.
    pub fn refresh(&mut self, dir: &std::path::Path) {
        // La fecha con la que se compara es la de la última lectura buena, que es la que el propio
        // contrato devuelve. Guardarla aparte sería tener dos copias del mismo dato.
        let seen = self.last_good.as_ref().and_then(|panel| panel.written_at);

        match panel_contract::read_if_newer(dir, seen) {
            Ok(Some(panel)) => {
                self.last_good = Some(panel);
                self.problem = None;
                self.layout = None;
            }
            // **Lo que hay sigue vigente, y eso incluye borrar el problema anterior.** Que la lectura
            // llegue hasta aquí significa que se pudo preguntar por el fichero y que su fecha es la del
            // panel que ya se tiene: lo que se enseña **es** lo último, y un fallo de hace un rato ya no
            // describe la realidad. Sin esto, un tropiezo pasajero —quedarse sin descriptores, un
            // montaje de red que parpadea— acusaría al productor para siempre, porque su fichero no va a
            // volver a cambiar de fecha por sí solo.
            Ok(None) => self.problem = None,
            Err(problem) => self.problem = Some(problem),
        }
    }

    /// El último panel que se entendió, si hay alguno.
    ///
    /// **Solo lo usan las pruebas, y por eso está detrás de su bandera.** El pintado no lo necesita —
    /// va por [`PanelState::lay_out_if_needed`], que puede componer un panel que es solo la confesión
    /// de la aplicación— y dejarlo como método de producción sería una puerta abierta a que alguien
    /// vuelva a decidir «hay panel» por su cuenta, que es justo lo que [`PanelState::is_visible`] centraliza.
    #[cfg(test)]
    pub(crate) fn last_understood(&self) -> Option<&Panel> {
        self.last_good.as_ref()
    }

    /// Si esta consola tiene algo que enseñar, sea del productor o de la propia aplicación.
    ///
    /// **Sale de lo mismo que decide el pintado, y no de un criterio paralelo.** Con dos predicados que
    /// dicen «hay panel» por su cuenta, el día que discrepen se reserva una franja donde no se pinta
    /// nada, o se pinta fuera de la que se reservó.
    ///
    /// **Que el fichero no exista todavía no cuenta**, y esa es la raya que sostiene la promesa de que
    /// una consola sin productor se ve exactamente como antes de este hito: no hay panel, no se
    /// descuenta ancho, no hay nada.
    pub fn is_visible(&self, now: SystemTime) -> bool {
        self.last_good.is_some() || self.warning(now).is_some()
    }

    /// Qué reclama esta división **ahora**, si reclama algo.
    ///
    /// **Sale del campo que el productor declaró y de nada más.** Ni del cuerpo del resumen, ni de la
    /// fase, ni de si la consola escribe: deducirlo es exactamente lo que el contrato prohíbe, y una
    /// línea con `mark: block` de la semana pasada dejaría la pestaña marcada para siempre.
    ///
    /// **Sin panel bueno no hay reclamo**, aunque la aplicación tenga algo que confesar: lo que se
    /// confiesa entonces es que no hay panel, y un panel que no se ha podido leer nunca no dijo nada
    /// que reclamar.
    pub fn attention(&self, now: SystemTime) -> Option<AttentionState> {
        self.attention_with(self.warning(now).is_none())
    }

    /// Lo mismo, para quien ya tiene el aviso en la mano.
    ///
    /// **Existe para que el aviso se calcule una vez por división y fotograma** y no dos: el camino de
    /// pintado ya lo necesita entero, y volver a pedirlo aquí solo para preguntarle si es `None` cuesta
    /// un formateo de cadena en cada fotograma de una ventana que puede estar repintándose a sesenta.
    fn attention_with(&self, nothing_to_confess: bool) -> Option<AttentionState> {
        let class = self.last_good.as_ref()?.attention?;
        // **Lo terminado no necesita a nadie detrás.** Quien acabó deja de publicar *por definición*,
        // así que el silencio no desmiente su «terminó y nadie lo ha mirado»: es su estado normal. Sin
        // esta excepción, media hora después de acabar todo lo terminado se apagaría — y encontrar por
        // la mañana lo que acabó de noche es justo para lo que sirve.
        // Lo que sí lo desmiente es **no poder leer su fichero**, y eso es lo único que aquí se mira:
        // `problem` es lo mismo que separa las dos clases de aviso en [`PanelState::warning`], de donde sale
        // `nothing_to_confess`.
        let backed = nothing_to_confess || (class == Attention::Done && self.problem.is_none());
        Some(AttentionState { class, backed })
    }

    /// Lo que la aplicación tiene que decir de su cuenta sobre este panel, si tiene algo que decir.
    ///
    /// **Es la línea de la app, no una línea del productor**, y por eso no depende de `header`: esa
    /// bandera decide si se pinta la identidad de la división, no si la aplicación puede ser honesta.
    /// Un productor que la apagara para esconder que su panel está roto sería justo el caso que esto
    /// viene a impedir.
    fn warning(&self, now: SystemTime) -> Option<String> {
        // **El fichero que todavía no existe es la excepción, y va primero por eso.** El resto se
        // ordena por lo que hay que decir: primero los fallos de lectura, que son lo más reciente que
        // se sabe, y solo si no hay ninguno se mira el reloj.
        match (&self.problem, &self.last_good) {
            (Some(problem), _) if is_a_missing_file(problem) => self
                .last_good
                .as_ref()
                .map(|_| "el fichero del panel ya no está".to_owned()),
            (Some(problem), Some(_)) => Some(format!("lo que se ve no es lo último: {problem}")),
            (Some(problem), None) => Some(format!("sin panel: {problem}")),
            (None, Some(panel)) if panel.is_stale_at(now) => Some(no_news(panel, now)),
            (None, _) => None,
        }
    }

    /// Cuánto falta para que **cambie lo que este panel dice de sí mismo**, si es que va a cambiar.
    ///
    /// **La ventana lo necesita para despertarse a tiempo.** Un productor que se muere o se cuelga no
    /// escribe nada, así que no hay evento de fichero que avise: sin este despertador el aviso de
    /// «esto es viejo» no aparecería jamás en una ventana en reposo, que es exactamente cuando hace
    /// falta.
    ///
    /// **Y no acaba cuando el panel se marca como viejo**: el aviso lleva un número que avanza, así que
    /// hay que volver cuando ese número cambie. Sin eso, un panel que se marcó viejo a los cinco
    /// minutos seguiría diciendo «5 min» tres horas después — acertando en lo cualitativo y mintiendo
    /// justo en el dato que sirve para decidir si el agente está muerto o solo lento.
    pub fn how_long_until_warning(&self, now: SystemTime) -> Option<Duration> {
        // Con un problema el aviso ya está puesto y no cambia con el tiempo: no hay a qué volver.
        if self.problem.is_some() {
            return None;
        }
        let panel = self.last_good.as_ref()?;
        let silence = now.duration_since(panel.written_at?).ok()?;

        match panel.stale_after.checked_sub(silence) {
            // Todavía no es viejo: lo próximo que cambia es que lo sea.
            Some(missing) => Some(missing),
            // Ya lo es, y lo que se enseña es un número redondeado: se vuelve cuando salte.
            None => Some(until_the_next_step(silence)),
        }
    }

    /// La maqueta para este ancho, este aviso y este reclamo, calculándola solo si no vale la que hay.
    fn lay_out_if_needed(
        &mut self,
        id: &str,
        width: usize,
        theme: &Theme,
        warning: Option<String>,
        attention: Option<AttentionState>,
    ) -> Option<&[Row]> {
        if self.last_good.is_none() && warning.is_none() {
            return None;
        }

        // **El reclamo no está en la clave, y es a propósito**: se atenúa exactamente cuando aparece
        // el aviso —que sí está— y su clase solo cambia al releer el fichero, que tira la maqueta
        // entera. Meterlo sería un término que ninguna prueba puede tumbar. Quien cambie el criterio de
        // «sostenido» y lo despegue del aviso tiene que añadirlo aquí.
        let still_valid =
            matches!(&self.layout, Some(done) if done.width == width && done.warning == warning);
        if !still_valid {
            self.layout = Some(PanelLayout {
                rows: lay_out(
                    self.last_good.as_ref(),
                    id,
                    width,
                    theme,
                    warning.as_deref(),
                    attention,
                ),
                width,
                warning,
            });
        }
        self.layout.as_ref().map(|done| done.rows.as_slice())
    }
}

/// Si el problema es que el fichero no está, que es el único que **no** se enseña por sí solo.
fn is_a_missing_file(problem: &Problem) -> bool {
    matches!(problem, Problem::Missing)
}

/// Lo que se dice de un panel que lleva callado más de lo suyo.
///
/// **Sin cuánto cuando el plazo es de menos de un minuto**: el contrato permite un
/// `stale_after_minutes` de cero, y «sin noticias desde hace 0 min» un segundo después de publicar se
/// lee como un fallo de la aplicación, no como una confesión.
fn no_news(panel: &Panel, now: SystemTime) -> String {
    match now.duration_since(panel.written_at.unwrap_or(now)) {
        Ok(silence) if silence.as_secs() >= 60 => {
            format!("sin noticias desde hace {}", in_words(silence))
        }
        _ => "sin noticias".to_owned(),
    }
}

/// Un rato, dicho como lo diría alguien: minutos hasta que son demasiados, y entonces horas.
///
/// Sin segundos ni decimales a propósito. Lo que el aviso tiene que transmitir es «hace mucho», y un
/// número que cambia cada segundo en una esquina de la pantalla es ruido, no información — y cada
/// cambio cuesta un fotograma, porque hay que despertar a la ventana para pintarlo.
fn in_words(how_much: Duration) -> String {
    let minutes = how_much.as_secs() / 60;
    match minutes {
        0..=119 => format!("{minutes} min"),
        _ => format!("{} h", minutes / 60),
    }
}

/// Cuánto falta para que el número de [`in_words`] cambie.
///
/// Es lo que hace que el aviso no se pudra en pantalla sin despertar a la ventana más de lo justo:
/// mientras se digan minutos se vuelve al siguiente minuto, y cuando se digan horas, a la siguiente
/// hora.
fn until_the_next_step(silence: Duration) -> Duration {
    let step = if silence.as_secs() < 2 * 60 * 60 {
        60
    } else {
        60 * 60
    };

    Duration::from_secs(step - silence.as_secs() % step)
}

/// Lo más largo que se pinta de una línea, en caracteres.
///
/// **No es cosmético: es lo único que acota el coste de pintar, y está medido.** El techo de tamaño
/// del contrato acota lo que un fichero hace reservar al *interpretarse*, y su peor caso son treinta
/// mil objetos diminutos. Pintar tiene el peor caso **inverso**: una sola línea de 260.000 caracteres
/// —254 KiB, holgadamente por debajo del techo, y de las baratas de interpretar— cuesta **14,6 ms de
/// teselado en cada fotograma** mientras siga publicada, con un millón de vértices por fotograma. El
/// presupuesto entero son 16,7 ms y las dos consolas ya gastan 5,92.
///
/// **Y el recorte al rectángulo no salva**: epaint descarta texto **por fila, no por glifo**
/// (`tessellator.rs:2050`), así que una fila que empieza dentro del panel se copia entera a la malla
/// aunque se salga por la derecha.
///
/// Sigue haciendo falta con la envoltura puesta: envolver reparte el texto en filas, pero **no lo
/// tira**, así que un millón de caracteres serían un millón de caracteres maquetados.
const MAX_CHARACTERS: usize = 512;

/// Quita de una cadena de fuera lo que descuadraría la rejilla del panel, y acota lo que se pinta.
///
/// **Está medido en la fuente de epaint 0.36.1**: `break_on_newline` viene puesto por omisión en los
/// trabajos de maquetado que usa `Painter::text` (`text_layout_types.rs:107`), así que una sola línea
/// con un `\n` dentro pinta varias filas, y la maqueta cuenta una. Un tabulador es el otro caso:
/// avanza el ancho de varias celdas mientras la medida de anchura lo cuenta como cero.
///
/// El `panel.json` puede venir de un repo clonado o de un contenedor, así que esto no es prudencia
/// contra un descuido: es la frontera con lo de fuera. **Se aplica también a la referencia**, que es
/// cadena ajena igual y desde este PR se pinta.
///
/// Se sustituye por un espacio en vez de borrarse, para que dos palabras separadas por un salto de
/// línea no acaben pegadas y se lean como una sola.
fn sanitize(text: &str) -> String {
    text.chars()
        .take(MAX_CHARACTERS)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// El símbolo con el que se pinta cada marca del contrato.
///
/// **Elegidos midiendo la fuente, no de memoria, y el ancho es el criterio.** De los seis del
/// prototipo, `✓` no existe en la monoespaciada —ancho cero, igual que un codepoint inventado— y `⏸` y
/// `⚠` solo están en las fuentes de emoji, donde miden 14 y 17,77 puntos frente a los 8,43 de una
/// celda: **descuadrarían la columna**, que se lee peor que un cuadro porque no parece un fallo de
/// fuente sino un panel mal alineado. Los otros tres del prototipo sí sirven tal cual.
///
/// `done` y `pending` comparten forma a propósito —el círculo lleno y el punto— porque son los dos
/// extremos de lo mismo: lo acabado y lo que ni ha empezado.
///
/// Si algún día el juego se queda corto de expresividad, la salida está escrita en el plan: pintar las
/// marcas como forma en vez de como carácter. Eso es otro hito.
fn symbol(mark: Mark) -> char {
    match mark {
        Mark::Done => '●',
        Mark::Current => '▶',
        Mark::Pending => '·',
        Mark::Wait => '…',
        Mark::Block => '×',
        Mark::Info => '›',
    }
}

/// El símbolo con el que se pinta cada clase de reclamo.
///
/// **Los tres son propios y ninguno se repite con los del cuerpo**, a propósito: el mismo signo en dos
/// sitios invita a leer que la aplicación dedujo el uno del otro, que es justo lo que el contrato
/// prohíbe — un `mark: block` de una línea del resumen **no** es un reclamo de la división.
///
/// **Elegidos midiéndolos contra el ancho de una `M`**, con el mismo criterio y por la misma razón que
/// los de arriba. La prueba que lo comprueba contra la fuente cuenta el porqué entero.
///
/// `?` porque te preguntan a ti, `⊘` porque algo la tiene parada, y `∎` porque ahí ya no queda nada
/// por hacer más que mirarlo.
pub(crate) fn attention_symbol(class: Attention) -> char {
    match class {
        Attention::Wait => '?',
        Attention::Block => '⊘',
        Attention::Done => '∎',
    }
}

/// Lo que dice la fila del reclamo, en palabras.
///
/// **El símbolo no viaja solo**: en el panel hay sitio, y un signo suelto obliga a recordar una tabla.
///
/// Las tres frases son **las del contrato, palabra por palabra**, incluido el «a ti» de la primera —
/// que es justo lo que la separa de la segunda, «por algo que **no** eres tú». Acortarlas por caber es
/// perder la oposición que las distingue; lo que no cabe se envuelve, como todo lo demás del panel.
fn attention_text(class: Attention) -> &'static str {
    match class {
        Attention::Wait => "te espera a ti",
        Attention::Block => "atascada por algo que no eres tú",
        Attention::Done => "terminó y nadie lo ha mirado",
    }
}

/// Lo menos que se le deja al texto cuando la sangría y la columna se lo comen todo, en celdas.
///
/// **Sin un suelo, la envoltura no progresa**: con el hueco a cero o negativo, cada vuelta del bucle
/// mete cero caracteres y la ventana se cuelga. El prototipo ya lo aprendió, y por eso su envoltura
/// lleva el mismo `max`.
const MINIMUM_SLOT: usize = 8;

/// Una fila ya maquetada: en qué columna empieza, qué dice y de qué color.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    column: usize,
    text: String,
    color: Color32,
}

/// Convierte un panel en las filas que se van a pintar, al ancho dado en celdas.
///
/// **Aquí está todo lo que cuesta y nada de lo que dibuja**, y esa separación es lo que permite
/// recordar el resultado entre fotogramas: mientras el fichero y el ancho no cambien, la maqueta es la
/// misma. Sin ella, envolver el panel se recalcularía sesenta veces por segundo mientras la consola de
/// al lado vuelca.
fn lay_out(
    panel: Option<&Panel>,
    id: &str,
    width: usize,
    theme: &Theme,
    warning: Option<&str>,
    attention: Option<AttentionState>,
) -> Vec<Row> {
    let mut rows = Vec::new();

    if panel.is_none_or(|panel| panel.header) {
        let phase = panel.and_then(|panel| panel.phase.as_deref());
        let identity = match phase {
            Some(phase) => format!("{id} · {phase}"),
            None => id.to_owned(),
        };
        wrap(
            &mut rows,
            0,
            &sanitize(&identity),
            theme.panel_color(None, None),
            width,
        );
    }

    // **El reclamo va en la fila de la aplicación y no en la de identidad**, y esa frontera es la
    // razón de ser de este hito: `header: false` esconde la identidad de la división, y la respuesta a
    // «¿tengo que ir ahí?» no puede apagarla el productor con una bandera suya. Es la misma frontera
    // que ya cumple el aviso de abajo.
    if let Some(attention) = attention {
        wrap(
            &mut rows,
            0,
            &format!(
                "{} {}",
                attention_symbol(attention.class),
                attention_text(attention.class)
            ),
            theme.attention_color(attention.class, attention.backed),
            width,
        );
    }

    // **El aviso va arriba y no al final**, porque abajo es donde el recorte se come lo que no cabe:
    // el aviso de que un panel no es de fiar sería justo lo primero en desaparecer de un panel largo.
    if let Some(warning) = warning {
        wrap(
            &mut rows,
            0,
            &sanitize(warning),
            theme.panel_color(None, Some(Style::Warn)),
            width,
        );
    }

    // La fila en blanco la escribe la maqueta, no el pintado: separada del cuerpo, lo que pone la
    // aplicación se lee como lo que es y no como la primera línea del productor.
    if !rows.is_empty() {
        rows.push(Row {
            column: 0,
            text: String::new(),
            color: Color32::TRANSPARENT,
        });
    }

    let Some(panel) = panel else {
        return rows;
    };

    for block in blocks(&panel.lines) {
        // **El ancho de la columna se calcula por bloque, y esa es la regla del contrato.** Con un
        // ancho global, un solo bloque con una referencia larga desplazaría el texto de todos los
        // demás, y lo que el panel gana alineando lo perdería en margen desperdiciado.
        // **Y la columna se acota contra el ancho del panel.** El contrato no le pone techo a la
        // referencia, así que una sola larguísima fijaría un ancho de columna que empuja el texto de
        // todo su bloque fuera del rectángulo. Lo que no cabe se corta, como se corta el texto.
        let column = block
            .iter()
            .filter_map(|line| line.reference.as_deref())
            .map(|reference| sanitize(reference).chars().count())
            .max()
            .unwrap_or(0)
            .min(width.saturating_sub(MINIMUM_SLOT).saturating_sub(3));

        for line in block {
            rows.extend(lay_out_line(line, column, width, theme));
        }
    }

    rows
}

/// Parte las líneas en bloques por las líneas vacías, que es lo que el contrato usa para agruparlas.
///
/// El hueco **pertenece al bloque que cierra**: así se conserva como fila y la maqueta no tiene que
/// acordarse de reponerlo entre bloque y bloque.
fn blocks(lines: &[Line]) -> Vec<Vec<&Line>> {
    let mut blocks = vec![Vec::new()];
    for line in lines {
        let slot = line.text.is_empty() && line.reference.is_none() && line.mark.is_none();
        blocks
            .last_mut()
            .expect("siempre hay uno abierto")
            .push(line);
        if slot {
            blocks.push(Vec::new());
        }
    }
    blocks
}

/// Maqueta una línea: su símbolo, su referencia alineada, su sangría y su texto envuelto.
fn lay_out_line(line: &Line, column: usize, width: usize, theme: &Theme) -> Vec<Row> {
    let color = theme.panel_color(line.mark, line.style);
    let reference = line.reference.as_deref().map(sanitize).unwrap_or_default();
    let text = sanitize(&line.text);

    // Un hueco de verdad no lleva prefijo ni ocupa columna: es solo una fila en blanco.
    if text.is_empty() && reference.is_empty() && line.mark.is_none() {
        return vec![Row {
            column: 0,
            text: String::new(),
            color,
        }];
    }

    // **La cabeza se acota antes de componerse, no después.** Símbolo, referencia y sangría se pintan
    // en la primera fila sin pasar por la envoltura, así que una referencia más larga que el panel
    // —forma legal: el contrato no le pone techo— sacaba esa fila entera fuera del rectángulo y con
    // ella todas sus continuaciones. Lo que sobra de la referencia se corta, igual que se corta el
    // texto: lo que no cabe no se puede leer, y fingir que sí desplaza todo lo demás.
    let head_cap = width.saturating_sub(MINIMUM_SLOT);
    // Se corta al ancho de la columna, que ya viene acotado contra el panel: más larga que su columna,
    // la referencia empujaría el texto de su propia línea y rompería la alineación del bloque.
    let reference: String = reference.chars().take(column).collect();

    let mut head = String::new();
    match line.mark {
        Some(mark) => head.push(symbol(mark)),
        // El hueco del símbolo se reserva también sin marca: es lo que mantiene el texto de todas las
        // líneas en la misma columna, que es de lo que va alinear.
        None => head.push(' '),
    }
    head.push(' ');
    if column > 0 {
        head.push_str(&format!("{reference:column$} "));
    }
    // La sangría se acota contra lo mismo: un `indent` de 40 celdas es forma legal —el contrato admite
    // hasta 64— y en un panel de 36 dejaría la continuación fuera del rectángulo, invisible y contada
    // como fila visible, comiéndose renglones y falseando el aviso de cuántas faltan.
    head.push_str(
        &" ".repeat(
            line.indent
                .min(head_cap.saturating_sub(head.chars().count())),
        ),
    );

    let indent = head.chars().count().min(head_cap);
    let mut rows = Vec::new();
    wrap(&mut rows, indent, &text, color, width);

    // La cabeza se pega a la primera fila; las de continuación arrancan ya sangradas, que es lo que
    // alinea la envoltura bajo el texto y no bajo el símbolo.
    match rows.first_mut() {
        Some(first) => {
            first.text = format!("{head}{}", first.text);
            // **Y la primera fila arranca en cero, lleve texto o no.** La rama de texto vacío de la
            // envoltura devuelve su sangría —correcto para una continuación, no para la fila que lleva
            // la cabeza pegada—, así que una línea con marca y sin texto salía desplazada por el ancho
            // de su propio símbolo. Es forma legal del contrato.
            first.column = 0;
        }
        None => rows.push(Row {
            column: 0,
            text: head,
            color,
        }),
    }
    rows
}

/// Parte un texto en trozos que quepan, y los añade con la sangría de continuación puesta.
///
/// Corta **por palabras** cuando puede y por caracteres cuando una sola palabra no cabe, que es el caso
/// de una ruta larga o de un identificador. Los espacios múltiples se colapsan, como el contrato avisa:
/// no sirven para alinear.
fn wrap(rows: &mut Vec<Row>, indent: usize, text: &str, color: Color32, width: usize) {
    let slot = width.saturating_sub(indent).max(MINIMUM_SLOT);
    let mut pending: Vec<&str> = text.split_whitespace().rev().collect();

    if pending.is_empty() {
        rows.push(Row {
            column: indent,
            text: String::new(),
            color,
        });
        return;
    }

    while let Some(word) = pending.pop() {
        let mut line = String::new();
        let mut word = word;
        loop {
            let room = slot - line.chars().count();
            if word.chars().count() > room {
                if line.is_empty() {
                    // Una palabra sola más larga que el hueco: se parte por caracteres, porque
                    // dejarla entera desbordaría el panel y saltar de línea no la haría caber nunca.
                    let cut = word.char_indices().nth(room).map_or(word.len(), |(i, _)| i);
                    line.push_str(&word[..cut]);
                    pending.push(&word[cut..]);
                }
                break;
            }
            line.push_str(word);
            match pending.last() {
                Some(next) if line.chars().count() + 1 + next.chars().count() <= slot => {
                    line.push(' ');
                    word = pending.pop().expect("acabo de mirarla");
                }
                _ => break,
            }
        }
        // La primera fila la coloca quien llama, pegándole la cabeza; las demás llevan su sangría.
        let column = if rows.is_empty() { 0 } else { indent };
        rows.push(Row {
            column,
            text: line,
            color,
        });
    }
}

/// Pinta el panel de una consola en el rectángulo al que su pintor está recortado.
///
/// Toma el estado prestado en exclusiva porque **maqueta si hace falta y se lo guarda**: la alternativa
/// —maquetar fuera y pasar las filas— obligaría a que quien pinta llevara la cuenta de cuándo la
/// maqueta caducó, y esa cuenta es de aquí.
///
/// El identificador es el de la división, y es lo único que la aplicación aporta de su cosecha: la
/// línea de identidad. **Se compone solo con lo que la app ya sabe y con lo que trae el propio
/// fichero**, nunca leyendo un `meta.json` vecino — eso sería deducir el estado de otro.
///
/// El encuadre sale del propio pintor y no de un argumento aparte: con las dos vías, nada impediría
/// que un día dejaran de coincidir.
pub fn paint(
    painter: &Painter,
    state: &mut PanelState,
    id: &str,
    theme: &Theme,
    metrics: &CellMetrics,
    now: SystemTime,
    discoverer: &mut crate::fonts::Discoverer,
) {
    let rect = painter.clip_rect();
    painter.rect_filled(rect, 0.0, theme.panel_background());

    // El ancho en celdas se descuenta el margen de los dos lados aunque solo se pinte el de la
    // izquierda: sin el de la derecha, la envoltura llenaría hasta el borde exacto del panel y el texto
    // quedaría pegado a la consola de al lado.
    let width = ((rect.width() / metrics.width()) as usize).saturating_sub(2 * MARGIN as usize);
    let warning = state.warning(now);
    let attention = state.attention_with(warning.is_none());
    let Some(rows) = state.lay_out_if_needed(id, width, theme, warning, attention) else {
        return;
    };

    // **Sin un renglón entero no se pinta nada, ni siquiera el aviso.** Con un suelo de uno, un panel
    // de alto cero encolaba «… N más» fuera de su rectángulo: un aviso de que falta contenido, en un
    // sitio donde no cabe ningún contenido.
    let fit = (rect.height() / metrics.height()) as usize;
    if fit == 0 {
        return;
    }
    let (visible, leftover) = clip(rows, fit);

    let text_left = rect.left() + MARGIN * metrics.width();
    for (n, row) in visible.iter().enumerate() {
        let x = text_left + row.column as f32 * metrics.width();
        let y = rect.top() + n as f32 * metrics.height();
        if row.text.is_empty() {
            continue;
        }
        // **El panel también descubre fuentes.** Lo que enseña es lo que publica su productor —texto
        // libre, en cualquier idioma—, y sin apuntarlo un resumen en japonés se quedaba en cuadros para
        // siempre porque nada disparaba la búsqueda.
        discoverer.note_text(&row.text);
        painter.text(
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            &row.text,
            metrics.font(),
            row.color,
        );
    }

    if leftover > 0 {
        let y = rect.top() + (visible.len() as f32) * metrics.height();
        painter.text(
            Pos2::new(text_left, y),
            Align2::LEFT_TOP,
            format!("… {leftover} más"),
            metrics.font(),
            theme.panel_color(Some(Mark::Pending), None),
        );
    }
}

/// Cuántas filas se pintan y cuántas se quedan fuera.
///
/// **El aviso ocupa su propia fila**, así que cuando algo sobra hay que pintar una menos: sin
/// descontarla, la última línea de contenido quedaría tapada por el aviso que dice que falta contenido.
fn clip(rows: &[Row], fit: usize) -> (&[Row], usize) {
    if rows.len() <= fit {
        return (rows, 0);
    }
    let visible = fit.saturating_sub(1);
    (&rows[..visible], rows.len() - visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_contract::{PANEL_FILE, PanelDir};

    /// Un panel leído de un fichero de verdad, para probar la maqueta sin pasar por el pintado.
    fn panel_with(file: &str) -> Panel {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, file);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        state.last_understood().expect("hay panel").clone()
    }

    fn publish(dir: &PanelDir, content: &str) {
        std::fs::write(dir.path().join(PANEL_FILE), content).expect("publicar el panel");
    }

    /// Adelanta la fecha del fichero para que la relectura la vea cambiada.
    ///
    /// A mano y no dejándolo al reloj: dos escrituras seguidas caen dentro del mismo tic en algunos
    /// sistemas de ficheros, y entonces la prueba pasaría o fallaría según la máquina.
    fn age_the_file(dir: &PanelDir, seconds: u64) {
        let path = dir.path().join(PANEL_FILE);
        std::fs::File::open(&path)
            .expect("abrir")
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(seconds))
            .expect("mover la fecha");
    }

    /// Lo que mide una celda de ancho en las pruebas del reparto: fijo, para que el suelo salga
    /// redondo y los fallos se lean sin calcular nada.
    const CELL_WIDTH: f32 = 8.0;

    fn pane_of(width: f32) -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(width, 300.0))
    }

    #[test]
    fn el_panel_se_lleva_su_ancho_y_la_consola_el_resto() {
        let slot = set_aside(pane_of(800.0), CELL_WIDTH, Some(DEFAULT_WIDTH));
        let panel = slot.panel.expect("hay panel");

        assert_eq!(panel.width(), DEFAULT_WIDTH as f32 * CELL_WIDTH);
        assert_eq!(
            slot.console.width() + panel.width(),
            800.0,
            "entre los dos tienen que cubrir la división, sin hueco ni solape"
        );
        assert_eq!(
            slot.console.right(),
            panel.left(),
            "el panel empieza justo donde acaba la consola"
        );
    }

    /// El panel es margen y la consola es el trabajo, así que en una ventana estrecha lo que se
    /// estrecha es el panel. Con la regla al revés, arrastrar el divisor dejaría a un programa
    /// interactivo sin sitio para escribir mientras su panel conserva sus treinta y seis columnas.
    #[test]
    fn en_una_division_estrecha_el_panel_cede_y_la_consola_conserva_su_suelo() {
        let narrow = (MINIMUM_CONSOLE + 4.0) * CELL_WIDTH;

        let slot = set_aside(pane_of(narrow), CELL_WIDTH, Some(DEFAULT_WIDTH));

        assert_eq!(
            slot.panel.expect("hay panel").width(),
            4.0 * CELL_WIDTH,
            "el panel es el que cede"
        );
        assert_eq!(slot.console.width(), MINIMUM_CONSOLE * CELL_WIDTH);
    }

    /// Y el borde del anterior: por debajo del suelo de la consola el panel desaparece del todo antes
    /// que devolver un rectángulo invertido, que es lo que haría una resta sin suelo.
    #[test]
    fn en_una_division_mas_estrecha_que_el_suelo_el_panel_se_queda_sin_ancho() {
        let slot = set_aside(pane_of(16.0), CELL_WIDTH, Some(DEFAULT_WIDTH));
        let panel = slot.panel.expect("el rectángulo existe, aunque vacío");

        assert_eq!(panel.width(), 0.0);
        assert_eq!(slot.console.width(), 16.0);
        assert!(slot.console.left() <= slot.console.right() && panel.left() <= panel.right());
    }

    /// **El ancho son treinta y seis columnas, no doscientos ochenta y ocho puntos**, y con una sola
    /// medida de celda las dos cosas dan el mismo número: la primera prueba de este módulo pasa igual
    /// con un ancho fijo en puntos. La diferencia solo se ve **entre fotogramas de aumentos distintos**
    /// —la fuente cambia de tamaño y la métrica se vuelve a preguntar en cada pasada—, y es justo lo
    /// que la decisión de contarlo en celdas existe para conservar: cuántos caracteres caben.
    #[test]
    fn el_ancho_del_panel_se_cuenta_en_celdas_y_no_en_puntos() {
        // El doble de aumento, que es el salto que hace un escritorio al pasar a una pantalla densa.
        for cell in [CELL_WIDTH, CELL_WIDTH * 2.0] {
            let slot = set_aside(pane_of(1600.0), cell, Some(DEFAULT_WIDTH));

            assert_eq!(
                slot.panel.expect("hay panel").width() / cell,
                DEFAULT_WIDTH as f32,
                "con celdas de {cell} puntos el panel dejó de medir sus columnas"
            );
        }
    }

    /// Sin nada que pintar no se descuenta ancho y no hay segundo
    /// rectángulo: la división se ve exactamente como antes de que existiera el panel.
    #[test]
    fn sin_panel_que_pintar_la_consola_se_queda_con_todo() {
        let whole = pane_of(800.0);

        let slot = set_aside(whole, CELL_WIDTH, None);

        assert_eq!(slot.console, whole);
        assert!(slot.panel.is_none());
    }

    /// El sitio que se le da al panel en las pruebas del pintado: de sobra de alto para que quepa
    /// todo, así que lo que no salga es porque no se pintó y no porque no cupiera.
    const PLACE: Rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 600.0));

    /// Pinta un panel de verdad en el sitio dado y devuelve **todo lo que quedó encolado**.
    ///
    /// Devuelve las formas enteras y no solo su texto porque dos propiedades del pintado no son
    /// texto: **el fondo**, que es lo único que separa el panel de su consola, y **dónde cae** cada
    /// fila.
    fn what_gets_queued(file: &str, place: Rect) -> Vec<eframe::egui::Shape> {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, file);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        assert!(
            state.last_understood().is_some(),
            "el fichero de la prueba no se leyó"
        );

        paint_the_state(&mut state, place, SystemTime::now())
    }

    /// Pinta el estado que se le dé, en el momento que se le diga.
    ///
    /// Separado de [`what_gets_queued`] porque la confesión depende de **cuándo** se mire: el mismo
    /// panel se pinta distinto si ha pasado su plazo.
    fn paint_the_state(
        state: &mut PanelState,
        place: Rect,
        now: SystemTime,
    ) -> Vec<eframe::egui::Shape> {
        let ctx = eframe::egui::Context::default();
        let mut output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrics = CellMetrics::measure(
                ui.ctx(),
                eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE),
            );
            paint(
                &ui.painter_at(place),
                state,
                "a1b2",
                &Theme::default(),
                &metrics,
                now,
                &mut crate::fonts::Discoverer::default(),
            );
        });

        // Las formas se sacan **antes** de soltar la salida: una pasada de interfaz que se suelta sin
        // aplicar sus texturas entra en pánico, y aquí no hay pantalla donde aplicarlas.
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();

        shapes.into_iter().map(|clipped| clipped.shape).collect()
    }

    /// Un estado con un panel publicado hace lo que se diga.
    ///
    /// La fecha se mueve a mano y no se espera al reloj, por lo mismo que en [`age_the_file`]: una prueba
    /// que dependa de cuánto tarda en correr da un resultado distinto en cada máquina.
    fn published_ago(file: &str, how_much: Duration) -> PanelState {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, file);
        std::fs::File::open(dir.path().join(PANEL_FILE))
            .expect("abrir")
            .set_modified(SystemTime::now() - how_much)
            .expect("mover la fecha");

        let mut state = PanelState::default();
        state.refresh(dir.path());
        state
    }

    /// El texto de lo que se pintó, fila a fila.
    fn texts(shapes: Vec<eframe::egui::Shape>) -> Vec<String> {
        shapes
            .into_iter()
            .filter_map(|shape| match shape {
                eframe::egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    }

    /// **El productor que se muere no avisa de que se ha muerto.** Es el caso que este aviso existe
    /// para cubrir: no escribe nada, así que no hay evento de fichero que lo delate — solo el tiempo.
    #[test]
    fn un_panel_que_lleva_callado_mas_de_lo_suyo_lo_confiesa() {
        // 45 min **y medio**: el silencio se trunca a minutos enteros, y con la fecha justo en la
        // frontera un sistema de ficheros que redondeara al alza diría «44 min» y la prueba señalaría
        // al código sin que el código tenga nada.
        let mut state = published_ago(
            r#"{"stale_after_minutes":5,"lines":["lo que decía"]}"#,
            Duration::from_secs(45 * 60 + 30),
        );

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert!(
            painted.iter().any(|row| row.contains("sin noticias")),
            "el panel lleva 45 minutos sin tocarse y se enseña como si fuera de ahora: {painted:?}"
        );
        assert!(
            painted.iter().any(|row| row.contains("45 min")),
            "el aviso no dice cuánto lleva callado: {painted:?}"
        );
        assert!(
            painted.iter().any(|row| row.contains("lo que decía")),
            "se perdió lo que el productor había publicado: {painted:?}"
        );

        // **Y va arriba, no al final.** Abajo es donde el recorte se come lo que no cabe: en un panel
        // largo, el aviso de que no es de fiar sería lo primero en desaparecer.
        let where_at = |text: &str| painted.iter().position(|row| row.contains(text));
        assert!(
            where_at("sin noticias") < where_at("lo que decía"),
            "el aviso se pinta detrás del cuerpo del productor: {painted:?}"
        );
    }

    /// Y la otra mitad: **un panel al día no dice nada**. Sin esto, un aviso puesto siempre pasaría por
    /// bueno y el panel tendría una línea de ruido permanente.
    #[test]
    fn un_panel_recien_publicado_no_confiesa_nada() {
        let mut state = published_ago(
            r#"{"stale_after_minutes":5,"lines":["recién puesto"]}"#,
            Duration::from_secs(10),
        );

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert!(
            !painted.iter().any(|row| row.contains("sin noticias")),
            "un panel de hace diez segundos se anuncia como viejo: {painted:?}"
        );
    }

    /// **Conservar el último panel bueno sin decirlo esconde a un productor roto** detrás de un resumen
    /// plausible de hace una hora. Esta es la mitad de AC7 que faltaba: la ventana sigue en pie, se
    /// conserva lo último bueno, **y el panel dice que eso no es lo último**.
    #[test]
    fn una_publicacion_que_no_se_entiende_se_confiesa_sin_perder_lo_bueno() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, r#"{"lines":["lo bueno de antes"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());

        publish(&dir, "esto no es json");
        age_the_file(&dir, 1);
        state.refresh(dir.path());

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        assert!(
            painted
                .iter()
                .any(|row| row.contains("no es lo último") && row.contains("no se entiende")),
            "se sigue enseñando lo viejo como si fuera lo último: {painted:?}"
        );
        assert!(
            painted.iter().any(|row| row.contains("lo bueno de antes")),
            "confesar no puede costar lo último que sí se entendió: {painted:?}"
        );
    }

    /// Y cuando **nunca** hubo nada bueno, el panel es solo la confesión. Sin esto, una consola cuyo
    /// productor publica basura desde el principio se ve igual que una que no tiene productor: como una
    /// consola pelada, sin una palabra que explique por qué no hay panel.
    #[test]
    fn una_consola_sin_nada_bueno_ensena_el_aviso_a_solas() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, "ni esto tampoco");
        let mut state = PanelState::default();
        state.refresh(dir.path());

        assert!(
            state.is_visible(SystemTime::now()),
            "hay un productor publicando algo ilegible y la división se ve como si no lo hubiera"
        );
        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        assert!(
            painted
                .iter()
                .any(|row| row.contains("sin panel") && row.contains("no se entiende")),
            "no se dice por qué no hay panel: {painted:?}"
        );
    }

    /// **Un fichero que se pasa del techo no se pinta, pero tampoco se calla.** El techo es lo que
    /// impide que un `panel.json` ajeno cueste un fotograma entero en interpretarse; que además dejara
    /// la consola indistinguible de una sin productor sería el fallo que este hito viene a evitar,
    /// solo que provocado por la propia defensa.
    #[test]
    fn un_panel_que_pasa_del_techo_no_se_pinta_pero_se_dice_por_que() {
        let dir = PanelDir::create().expect("crear el directorio");
        let filler = "x".repeat(panel_contract::MAX_BYTES as usize);
        publish(&dir, &format!(r#"{{"lines":["{filler}"]}}"#));
        let mut state = PanelState::default();
        state.refresh(dir.path());

        assert!(
            state.is_visible(SystemTime::now()),
            "hay un productor publicando algo enorme y la división se ve como si no lo hubiera"
        );
        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        assert!(
            painted
                .iter()
                .any(|row| row.contains("sin panel") && row.contains("KiB")),
            "no se dice por qué no hay panel: {painted:?}"
        );
        assert!(
            !painted.iter().any(|row| row.contains("xxxx")),
            "se pintó contenido de un fichero que pasa del techo: {painted:?}"
        );
    }

    /// **La raya que sostiene la promesa del hito**: una consola cuyo productor no ha publicado todavía
    /// se ve exactamente como antes de que el panel existiera. Que el fichero no esté es el estado
    /// normal de arranque, no una avería que anunciar.
    #[test]
    fn una_consola_sin_fichero_no_ensena_nada_ni_avisa_de_nada() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut state = PanelState::default();
        state.refresh(dir.path());

        assert!(
            !state.is_visible(SystemTime::now()),
            "una consola sin productor se estrecha para enseñar un panel que no existe"
        );
        assert!(state.warning(SystemTime::now()).is_none());
    }

    /// Pero si **había** panel y su fichero desaparece, eso sí se dice: el productor lo borró o alguien
    /// limpió por debajo, y lo que se sigue viendo ya no tiene respaldo en disco.
    #[test]
    fn que_el_fichero_desaparezca_despues_si_se_confiesa() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, r#"{"lines":["estuve aquí"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());

        std::fs::remove_file(dir.path().join(PANEL_FILE)).expect("borrar el panel");
        state.refresh(dir.path());

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        assert!(
            painted.iter().any(|row| row.contains("ya no está")),
            "el fichero se borró y el panel sigue como si nada: {painted:?}"
        );
    }

    /// **`header: false` apaga la línea de identidad, no la honestidad de la aplicación.** Si la
    /// apagara, a un productor le bastaría con ponerlo para que su panel roto no lo pareciera.
    #[test]
    fn el_aviso_sale_aunque_el_productor_apague_la_cabecera() {
        let mut state = published_ago(
            r#"{"header":false,"stale_after_minutes":1,"lines":["sin cabecera"]}"#,
            Duration::from_secs(60 * 60),
        );

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert!(
            painted.iter().any(|row| row.contains("sin noticias")),
            "apagar la cabecera escondió el aviso de la app: {painted:?}"
        );
        assert!(
            !painted.iter().any(|row| row.contains("a1b2")),
            "`header: false` tiene que seguir escondiendo la identidad: {painted:?}"
        );
    }

    /// **El aviso entra en la clave de la maqueta guardada.** Un panel que envejece cambia lo que hay
    /// que pintar sin que su fichero se toque, así que una caché que solo mirase el ancho seguiría
    /// enseñando la maqueta de cuando era fresco y el aviso no aparecería nunca.
    #[test]
    fn envejecer_tira_la_maqueta_aunque_el_fichero_no_cambie() {
        let mut state = published_ago(
            r#"{"stale_after_minutes":10,"lines":["lo mismo de siempre"]}"#,
            Duration::from_secs(60),
        );

        let fresh = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        let stale = texts(paint_the_state(
            &mut state,
            PLACE,
            SystemTime::now() + Duration::from_secs(30 * 60),
        ));

        assert!(!fresh.iter().any(|row| row.contains("sin noticias")));
        assert!(
            stale.iter().any(|row| row.contains("sin noticias")),
            "el panel envejeció y se siguió pintando la maqueta de cuando era fresco: {stale:?}"
        );
    }

    /// **Lo que el latido necesita saber**: cuánto queda para que toque avisar. Sin esto la ventana no
    /// sabría cuándo despertarse, y con la consola callada el aviso no llegaría nunca.
    #[test]
    fn se_sabe_cuanto_falta_para_que_toque_avisar() {
        let state = published_ago(
            r#"{"stale_after_minutes":10,"lines":["algo"]}"#,
            Duration::from_secs(4 * 60),
        );

        let missing = state
            .how_long_until_warning(SystemTime::now())
            .expect("todavía no es viejo, así que hay algo que esperar");

        // Seis minutos, con holgura para lo que tarde la propia prueba.
        assert!(
            missing.as_secs() > 5 * 60 && missing.as_secs() <= 6 * 60,
            "faltan {} s para avisar de un panel de hace 4 min con plazo de 10",
            missing.as_secs()
        );
    }

    /// **Y un panel ya viejo sigue pidiendo despertador, porque su aviso lleva un número que avanza.**
    /// Sin esto, el agente que muere a las 10:00 con un plazo de cinco minutos deja en pantalla «sin
    /// noticias desde hace 5 min», y a las 13:00 sigue diciendo cinco: acierta en lo cualitativo y
    /// miente en el dato que sirve para decidir si está muerto o solo lento.
    #[test]
    fn un_panel_ya_viejo_vuelve_a_despertar_cuando_su_numero_cambie() {
        let state = published_ago(
            r#"{"stale_after_minutes":1,"lines":["algo"]}"#,
            Duration::from_secs(45 * 60 + 40),
        );

        let missing = state
            .how_long_until_warning(SystemTime::now())
            .expect("el número del aviso va a cambiar, así que hay a qué volver");

        // Al siguiente minuto, no dentro de un minuto: lo que importa es cuándo salta el número.
        // Veinte segundos de margen entre mover la fecha y preguntar: el más estrecho de la suite
        // era de diez, y es el que caería primero en una máquina saturada.
        assert!(
            missing.as_secs() > 10 && missing.as_secs() <= 20,
            "vuelve en {} s, y al número le faltan 20 para saltar",
            missing.as_secs()
        );
    }

    /// Cuando se dicen horas, se vuelve a la siguiente hora y no cada minuto: el número solo cambia
    /// cuando cambia, y cada cambio cuesta despertar a la ventana.
    #[test]
    fn con_horas_de_silencio_se_despierta_una_vez_por_hora() {
        let state = published_ago(
            r#"{"stale_after_minutes":1,"lines":["algo"]}"#,
            Duration::from_secs(5 * 60 * 60 + 10 * 60),
        );

        let missing = state
            .how_long_until_warning(SystemTime::now())
            .expect("el número sigue avanzando");

        assert!(
            missing.as_secs() > 45 * 60,
            "con cinco horas de silencio se despierta a los {} s, que es una vez por minuto",
            missing.as_secs()
        );
    }

    /// Sin panel no hay nada que esperar.
    #[test]
    fn sin_panel_no_se_pide_despertador() {
        assert!(
            PanelState::default()
                .how_long_until_warning(SystemTime::now())
                .is_none()
        );
    }

    /// **Un fallo pasajero no puede acusar al productor para siempre.** Que la relectura diga «lo que
    /// tienes sigue vigente» significa que se pudo preguntar por el fichero y que su fecha es la del
    /// panel bueno que ya se tiene: el tropiezo de hace un rato —quedarse sin descriptores, un montaje
    /// de red que parpadea— ya no describe nada. Sin limpiarlo, el aviso se quedaba pegado de por vida,
    /// porque un fichero que nadie vuelve a tocar no cambia de fecha nunca.
    #[test]
    fn un_fallo_pasajero_no_deja_al_panel_acusado_para_siempre() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, r#"{"lines":["lo último de verdad"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());

        // El fichero se vuelve ilegible un instante y luego se recupera, sin cambiar de fecha.
        let path = dir.path().join(PANEL_FILE);
        let date = std::fs::metadata(&path)
            .expect("mirar la fecha")
            .modified()
            .expect("hay fecha");
        std::fs::write(&path, "roto").expect("romperlo");
        std::fs::File::open(&path)
            .expect("abrir")
            .set_modified(date + Duration::from_secs(1))
            .expect("mover la fecha");
        state.refresh(dir.path());
        publish(&dir, r#"{"lines":["lo último de verdad"]}"#);
        std::fs::File::open(&path)
            .expect("abrir")
            .set_modified(date)
            .expect("devolver la fecha");

        state.refresh(dir.path());

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));
        assert!(
            !painted.iter().any(|row| row.contains("no es lo último")),
            "el fichero se lee bien y su panel sigue acusado: {painted:?}"
        );
    }

    /// Con un plazo por debajo del minuto —que el contrato permite— el aviso **no dice un número**: «sin
    /// noticias desde hace 0 min» un segundo después de publicar se lee como un fallo de la aplicación.
    #[test]
    fn con_un_plazo_de_cero_el_aviso_no_dice_cero_minutos() {
        let mut state = published_ago(
            r#"{"stale_after_minutes":0,"lines":["algo"]}"#,
            Duration::from_secs(5),
        );

        let painted = texts(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert!(
            painted.iter().any(|row| row.contains("sin noticias")),
            "con plazo cero el panel es viejo desde el primer segundo: {painted:?}"
        );
        assert!(
            !painted.iter().any(|row| row.contains("0 min")),
            "el aviso dice «hace 0 min», que se lee como un fallo: {painted:?}"
        );
    }

    /// Lo que mide una celda con la fuente con la que pinta el panel.
    ///
    /// Hace falta para afirmar sobre posiciones: el margen y el hueco de la identidad se cuentan en
    /// celdas, así que compararlos con un número escrito a mano sería comprobar la fuente de esta
    /// máquina en vez de la aritmética del panel.
    fn cell() -> (f32, f32) {
        let ctx = eframe::egui::Context::default();
        let mut measured = (0.0, 0.0);
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrics = CellMetrics::measure(
                ui.ctx(),
                eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE),
            );
            measured = (metrics.width(), metrics.height());
        })
        .drop_without_applying_deltas();
        measured
    }

    /// Pinta un panel de verdad y devuelve **lo que quedó encolado**: el texto de cada fila y su color.
    ///
    /// Sin esto, todo lo que ocurre dentro de `paint` es invisible para la suite: se puede dejar de
    /// leer el estilo, pintarlo todo del mismo color o **no pintar nada**, y las pruebas de
    /// `Theme::panel_color` siguen verdes porque afirman sobre el ayudante, no sobre el camino.
    fn what_gets_painted(file: &str) -> Vec<(String, Color32)> {
        texts_and_colors(what_gets_queued(file, PLACE))
    }

    /// Lo mismo, para cuando la prueba necesita pintar en un momento concreto y no puede pasar por el
    /// fichero.
    fn texts_and_colors(shapes: Vec<eframe::egui::Shape>) -> Vec<(String, Color32)> {
        shapes
            .into_iter()
            .filter_map(|shape| match shape {
                eframe::egui::Shape::Text(text) => {
                    Some((text.galley.text().to_owned(), text.fallback_color))
                }
                _ => None,
            })
            .collect()
    }

    /// Dónde cae cada fila de texto, en el orden en que se encolaron.
    fn where_each_row_falls(file: &str, place: Rect) -> Vec<(Pos2, String)> {
        what_gets_queued(file, place)
            .into_iter()
            .filter_map(|shape| match shape {
                eframe::egui::Shape::Text(text) => Some((text.pos, text.galley.text().to_owned())),
                _ => None,
            })
            .collect()
    }

    /// **Cada fila en su renglón, y el hueco de la cadena vacía contado.**
    ///
    /// La prueba de abajo mira el contenido y el orden de la cola de formas, y eso **no ata dónde caen**:
    /// quitando el avance de la altura, las tres líneas se pintan una encima de otra en el mismo
    /// renglón y la suite seguía verde. Es la consecuencia que AC18 nombra —«no pinta encima de las
    /// líneas siguientes»— y la mitad de AC2 que dice «en orden».
    ///
    /// El hueco se comprueba **por la distancia**, que es la única forma de verlo: una cadena vacía no
    /// encola ninguna forma, así que lo que demuestra que dejó su sitio es que entre sus vecinas haya
    /// dos renglones y no uno.
    #[test]
    fn cada_fila_cae_en_su_renglon_y_la_vacia_deja_el_suyo() {
        let (_, height) = cell();

        let rows = where_each_row_falls(
            r#"{"header":false,"lines":["la primera","","la tercera"]}"#,
            PLACE,
        );

        assert_eq!(rows.len(), 2, "la vacía no encola forma, las otras sí");
        assert_eq!(rows[0].0.y, PLACE.top(), "la primera arranca arriba");
        assert_eq!(
            rows[1].0.y - rows[0].0.y,
            height * 2.0,
            "entre la primera y la tercera tienen que caber dos renglones: el hueco es uno de ellos"
        );
    }

    #[test]
    fn cada_linea_se_pinta_en_su_orden_y_las_vacias_dejan_su_hueco() {
        let rows = what_gets_painted(r#"{"header":false,"lines":["la primera","","la tercera"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            ["  la primera", "  la tercera"],
            "el hueco no se pinta, pero las dos que hay van en orden"
        );
    }

    /// **El color sale del camino real, no del ayudante.** Con esta prueba, ignorar `linea.style` en
    /// `paint` cae en rojo; sin ella, la suite seguía verde con el panel entero monocromo.
    #[test]
    fn el_color_de_cada_linea_sale_de_su_estilo_y_de_su_marca() {
        let theme = Theme::default();
        let rows = what_gets_painted(
            r#"{"header":false,"lines":[
                {"text":"un fallo","style":"error"},
                {"text":"bloqueada","mark":"block"},
                {"text":"y su color es el dato","mark":"info","style":"ok"},
                {"text":"sin adjetivos"}
            ]}"#,
        );

        assert_eq!(
            rows,
            vec![
                (
                    "  un fallo".to_owned(),
                    theme.panel_color(None, Some(panel_contract::Style::Error))
                ),
                (
                    "× bloqueada".to_owned(),
                    theme.panel_color(Some(panel_contract::Mark::Block), None)
                ),
                (
                    "› y su color es el dato".to_owned(),
                    theme.panel_color(
                        Some(panel_contract::Mark::Info),
                        Some(panel_contract::Style::Ok)
                    )
                ),
                ("  sin adjetivos".to_owned(), theme.panel_color(None, None)),
            ]
        );
    }

    /// La línea de identidad la escribe la aplicación con lo que ya sabe: el identificador de la
    /// división y la fase que el productor publica. Nada de un fichero vecino.
    #[test]
    fn la_cabecera_lleva_la_identidad_de_la_division_y_su_fase() {
        let rows = what_gets_painted(r#"{"phase":"build","lines":["el cuerpo"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["a1b2 · build", "  el cuerpo"]);
    }

    #[test]
    fn con_la_cabecera_quitada_no_se_pinta_la_identidad() {
        let rows = what_gets_painted(r#"{"header":false,"phase":"build","lines":["el cuerpo"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["  el cuerpo"], "la fase va dentro de la identidad");
    }

    /// **El saneo, comprobado sobre lo que llega al pintor y no sobre la función suelta.**
    #[test]
    fn lo_que_llega_al_pintor_no_lleva_saltos_de_linea() {
        let rows = what_gets_painted(r#"{"header":false,"lines":["arriba\nabajo"]}"#);

        assert_eq!(rows[0].0, "  arriba abajo");
    }

    /// **El coste de pintar tiene su propio peor caso, y es el inverso del de interpretar.** Una sola
    /// línea larguísima es barata de leer y costaba 14,6 ms de teselado **en cada fotograma**, porque
    /// epaint descarta texto por fila y no por glifo. Sin este tope, un `panel.json` ajeno deja la
    /// ventana por encima de su presupuesto para siempre.
    #[test]
    fn una_linea_absurdamente_larga_se_corta_antes_de_pintarse() {
        let long = "A".repeat(60_000);

        let rows = what_gets_painted(&format!(r#"{{"header":false,"lines":["{long}"]}}"#));

        // **Se cuenta el total, no la primera fila.** Con la envoltura puesta, el tope no se ve en una
        // fila suelta: la línea se reparte en varias, y lo que hay que acotar es cuánto texto se
        // maqueta en total. Contar solo la primera dejaría pasar mil filas de cuarenta caracteres.
        let painted: usize = rows.iter().map(|(t, _)| t.chars().count()).sum();
        // Los dos de margen son la columna del símbolo, que se reserva aunque la línea no traiga
        // marca: es lo que mantiene el texto de todas alineado.
        assert!(
            painted <= MAX_CHARACTERS + 2,
            "se maquetaron {painted} caracteres de una línea de 60.000"
        );
        assert!(rows.len() > 1, "y se envolvió en varias filas");
    }

    /// **El fondo se pinta aunque no haya nada que decir, y es lo único que separa el panel de su
    /// consola** — no hay línea ni borde entre los dos. Sin él, donde debería verse el panel se ve el
    /// fondo de la consola, con la consola ya estrechada: un hueco negro a la derecha que parece un
    /// fallo de pintado en vez de un productor callado.
    ///
    /// Y el caso es forma legal del contrato: `lines` vacío con la cabecera quitada deja un panel en el
    /// que el fondo es literalmente todo lo que hay.
    #[test]
    fn un_panel_sin_lineas_ni_cabecera_pinta_su_fondo_y_nada_mas() {
        let shapes = what_gets_queued(r#"{"header":false,"lines":[]}"#, PLACE);

        let backgrounds: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match shape {
                eframe::egui::Shape::Rect(rect) => Some((rect.rect, rect.fill)),
                _ => None,
            })
            .collect();
        assert_eq!(
            backgrounds,
            vec![(PLACE, Theme::default().panel_background())],
            "el panel tiene que cubrir su rectángulo con su propio fondo"
        );
        assert!(
            !shapes
                .iter()
                .any(|shape| matches!(shape, eframe::egui::Shape::Text(_))),
            "sin líneas y sin cabecera no hay texto que pintar"
        );
    }

    /// Una división puede quedarse sin alto: la ventana aplastada contra el borde de la pantalla, o el
    /// rectángulo de reserva mientras el escritorio todavía no ha dicho cuánto mide. Entonces no cabe
    /// ni una fila, y lo que se comprueba aquí es que **el bucle para**: sin el corte por lo alto se
    /// siguen encolando filas fuera del panel, y son formas que hay que teselar en cada fotograma para
    /// no verse. Un rectángulo del revés —el alto negativo— entra por el mismo camino y tampoco puede
    /// entrar en pánico.
    #[test]
    fn en_un_panel_sin_alto_no_se_encola_ninguna_fila() {
        let no_height = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 0.0));
        let the_other_way_round = Rect::from_min_max(Pos2::new(0.0, 100.0), Pos2::new(400.0, 0.0));

        for place in [no_height, the_other_way_round] {
            let rows = where_each_row_falls(r#"{"header":false,"lines":["una","otra"]}"#, place);

            assert!(
                rows.is_empty(),
                "en {place:?} no cabe ninguna fila y se encolaron {rows:?}"
            );
        }
    }

    /// **La línea de identidad lleva dentro una cadena del productor**: la fase. El saneo de las líneas
    /// no la alcanza —esta cadena la compone la aplicación—, así que un `\n` en la fase pintaría dos
    /// filas donde va una y taparía la primera del cuerpo.
    #[test]
    fn la_fase_pasa_por_el_mismo_saneo_que_las_lineas() {
        let rows = what_gets_painted(r#"{"phase":"build\nfalsa","lines":["el cuerpo"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["a1b2 · build falsa", "  el cuerpo"]);
    }

    /// Y el tope también: una fase larguísima es el mismo peor caso de teselado que una línea larguísima
    /// —14,6 ms por fotograma mientras siga publicada— y llega por una cadena que el tope de las líneas
    /// no toca.
    ///
    /// De paso, el borde que el contrato declara: con `lines` vacío el panel enseña **solo** su línea de
    /// identidad.
    #[test]
    fn una_fase_absurdamente_larga_no_se_pinta_entera() {
        let long = "F".repeat(60_000);

        let rows = what_gets_painted(&format!(r#"{{"phase":"{long}","lines":[]}}"#));

        let painted: usize = rows.iter().map(|(t, _)| t.chars().count()).sum();
        assert!(
            painted <= MAX_CHARACTERS + "a1b2 · ".len(),
            "se maquetaron {painted} caracteres de una fase de 60.000"
        );
        assert!(
            rows.iter()
                .all(|(t, _)| t.starts_with('F') || t.starts_with("a1b2")),
            "sin líneas, lo único que hay es la identidad envuelta: {rows:?}"
        );
    }

    /// Dos decisiones del pintado que **no se ven ni en el texto ni en el color**, y que hasta aquí no
    /// ataba nada: el texto no va pegado al borde izquierdo —pegado se lee como un error de pintado— y
    /// la identidad deja una fila en blanco antes del cuerpo. Lo segundo importa porque la identidad la
    /// escribe la aplicación y el cuerpo lo escribe el productor: pegadas, la suya se lee como la
    /// primera línea de él.
    #[test]
    fn el_texto_deja_su_margen_y_la_identidad_su_fila_en_blanco() {
        let (width, height) = cell();

        let rows = where_each_row_falls(r#"{"phase":"build","lines":["el cuerpo"]}"#, PLACE);

        let [(identity, _), (body, _)] = rows.as_slice() else {
            panic!("tenían que ser dos filas y son {rows:?}");
        };
        // La celda de margen va escrita aquí y no como `MARGIN`: una expectativa que sale de la misma
        // constante que describe se cumple sola, la pongan en el valor que la pongan.
        assert_eq!(
            identity.x,
            PLACE.left() + width,
            "el texto tiene que arrancar a una celda del borde"
        );
        assert_eq!(body.x, identity.x, "el cuerpo no arranca donde ella");
        assert_eq!(
            body.y - identity.y,
            2.0 * height,
            "entre la identidad y el cuerpo tiene que quedar una fila en blanco"
        );
    }

    /// **Los seis símbolos, preguntándoselo a la fuente y no a la memoria.**
    ///
    /// El criterio es **el ancho**, no `has_glyphs`: ese predicado compara la cara resuelta contra la
    /// de reemplazo, y con la monoespaciada la cara base *es* esa, así que responde «no» hasta para la
    /// letra `A` — la tabla del diseño se tomó con él y salió invertida. Un glifo que la cara
    /// monoespaciada tiene mide lo que mide la `M`; uno que falta mide cero; y uno que viene de las
    /// fuentes de emoji mide más, que es lo que **descuadra la columna**.
    #[test]
    fn los_seis_simbolos_estan_en_la_fuente_y_miden_una_celda() {
        let marks = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ];

        let ctx = eframe::egui::Context::default();
        let mut widths = Vec::new();
        let mut cell = 0.0;
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let font = eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE);
            ui.ctx().fonts_mut(|fonts| {
                cell = fonts.glyph_width(&font, 'M');
                widths = marks
                    .iter()
                    .map(|m| (symbol(*m), fonts.glyph_width(&font, symbol(*m))))
                    .collect();
            });
        })
        .drop_without_applying_deltas();

        for (symbol, width) in widths {
            assert!(
                (width - cell).abs() < 0.01,
                "«{symbol}» mide {width} y una celda mide {cell}: {}",
                if width == 0.0 {
                    "no está en la fuente"
                } else {
                    "viene de la de emoji y descuadraría la columna"
                }
            );
        }
    }

    /// Los seis distintos entre sí: con dos marcas compartiendo símbolo, el panel diría menos de lo que
    /// el productor escribió y nadie se enteraría.
    /// **Cada marca con el suyo, no solo «los seis existen y son distintos».** Intercambiando el de
    /// `done` con el de `wait` la suite seguía verde: un panel que pintara el punto suspensivo en lo
    /// terminado y el círculo en lo que espera diría **lo contrario** de lo que el productor escribió, y
    /// contradiría la tabla que el registro de decisiones publica.
    #[test]
    fn cada_marca_lleva_el_simbolo_que_la_decision_le_asigno() {
        assert_eq!(
            [
                symbol(Mark::Done),
                symbol(Mark::Current),
                symbol(Mark::Pending),
                symbol(Mark::Wait),
                symbol(Mark::Block),
                symbol(Mark::Info),
            ],
            ['●', '▶', '·', '…', '×', '›']
        );
    }

    #[test]
    fn ninguna_marca_comparte_simbolo_con_otra() {
        let all: std::collections::HashSet<char> = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ]
        .iter()
        .map(|m| symbol(*m))
        .collect();

        assert_eq!(all.len(), 6);
    }

    /// **La regla del contrato que hace legible un panel: la columna se calcula por bloque.**
    ///
    /// Con un ancho global, el bloque de arriba —cuya referencia es larga— empujaría el texto del de
    /// abajo, y lo que se gana alineando se perdería en margen desperdiciado. Los bloques los separan
    /// las líneas vacías.
    #[test]
    fn la_columna_de_referencias_se_alinea_por_bloques_y_un_bloque_no_ensancha_al_otro() {
        let rows = what_gets_painted(
            r##"{"header":false,"lines":[
                {"ref":"#1","text":"corta"},
                {"ref":"#10-#25","text":"larga"},
                "",
                {"ref":"#2","text":"otro bloque"}
            ]}"##,
        );

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            ["  #1      corta", "  #10-#25 larga", "  #2 otro bloque"],
            "el segundo bloque tiene que volver a su propio ancho"
        );
    }

    /// La continuación se sangra **hasta donde empieza el texto**, no hasta el borde: alineada al borde,
    /// la segunda fila se leería como una línea nueva del productor y la columna dejaría de significar
    /// nada.
    #[test]
    fn una_linea_larga_se_envuelve_con_la_continuacion_bajo_su_texto() {
        let rows = where_each_row_falls(
            r##"{"header":false,"lines":[{"ref":"#1","mark":"done","text":"palabras suficientes para no caber de ninguna manera en el panel"}]}"##,
            PLACE,
        );

        assert!(rows.len() > 1, "tenía que envolverse: {rows:?}");
        let (first, rest) = rows.split_first().expect("hay filas");

        // **La propiedad es la igualdad, no «más a la derecha».** Con solo `>`, cualquier sangría
        // constante distinta de cero pasa —incluida una celda, o sea debajo del símbolo, que es
        // literalmente lo que el criterio prohíbe—. Lo que hay que afirmar es que la continuación cae
        // donde empieza el **texto** de la primera fila, y ese sitio se calcula de lo que ya se tiene:
        // la x de la primera más lo que ocupa su cabeza.
        let (cell_width, _) = cell();
        // En caracteres, no en bytes: el símbolo de la marca ocupa tres bytes y una sola celda.
        let head = first.1.chars().count()
            - first
                .1
                .split_once("palabras")
                .expect("la primera fila lleva su cabeza y luego el texto")
                .1
                .chars()
                .count()
            - "palabras".chars().count();
        let under_the_text = first.0.x + head as f32 * cell_width;

        for (where_at, text) in rest {
            assert!(
                (where_at.x - under_the_text).abs() < 0.01,
                "«{text}» arrancó en {} y el texto de la primera empieza en {under_the_text}",
                where_at.x
            );
        }
    }

    /// Una palabra sola más larga que el hueco se parte por caracteres. Sin esto la envoltura no
    /// progresa: la palabra no cabe nunca, y saltar de fila no la hace caber.
    #[test]
    fn una_palabra_mas_larga_que_el_hueco_se_parte_en_vez_de_colgar() {
        let rows = what_gets_painted(&format!(
            r#"{{"header":false,"lines":["{}"]}}"#,
            "x".repeat(200)
        ));

        assert!(rows.len() > 1);
        assert!(
            rows.iter().all(|(t, _)| !t.is_empty()),
            "ninguna fila vacía"
        );
    }

    /// **Con la sangría comida entera, la envoltura tiene que seguir avanzando.** Sin suelo, el hueco
    /// sale cero y el bucle no mete ningún carácter por vuelta.
    ///
    /// **Su modo de fallo es colgarse, no ponerse en rojo**, y eso hay que saberlo antes de verlo: con
    /// `MINIMUM_SLOT` a cero esta prueba no falla, se queda girando y la suite entera muere por
    /// memoria. Quien la vea colgada está viendo el fallo, no una prueba rota. Es el mismo modo que la
    /// de la tubería con nombre en el crate del contrato.
    #[test]
    fn una_sangria_mayor_que_el_panel_no_cuelga_la_envoltura() {
        let rows = what_gets_painted(
            r#"{"header":false,"lines":[{"indent":64,"text":"esto no cabe de ninguna manera"}]}"#,
        );

        assert!(!rows.is_empty(), "algo se tiene que pintar");
        assert!(rows.iter().all(|(t, _)| !t.is_empty()));
    }

    /// **Lo que no cabe de alto se dice, no se descarta en silencio.** Un panel que se recorta callado
    /// es un resumen que miente por omisión, que es justo lo que el contrato le prohíbe al productor.
    #[test]
    fn cuando_no_caben_todas_la_ultima_fila_dice_cuantas_faltan() {
        let (_, height) = cell();
        let four_rows =
            Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, height * 4.0));

        let rows: Vec<String> = where_each_row_falls(
            r#"{"header":false,"lines":["una","dos","tres","cuatro","cinco","seis"]}"#,
            four_rows,
        )
        .into_iter()
        .map(|(_, text)| text)
        .collect();

        assert_eq!(rows.len(), 4, "cuatro renglones, cuatro filas");
        assert_eq!(
            rows.last().expect("hay filas"),
            "… 3 más",
            "y la última las cuenta, incluida la que el propio aviso desplaza"
        );
    }

    /// La maqueta se recuerda mientras el fichero y el ancho no cambien: mientras la consola de al lado
    /// vuelca, la ventana repinta sesenta veces por segundo y envolver el panel no puede pagarse cada
    /// vez.
    #[test]
    fn la_maqueta_se_recuerda_y_se_tira_cuando_cambia_el_ancho() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, r#"{"header":false,"lines":["algo que envolver"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let first = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("hay panel")
            .to_vec();
        let repeated = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("hay panel")
            .to_vec();
        assert_eq!(first, repeated, "el mismo ancho da la misma maqueta");

        let narrow = state
            .lay_out_if_needed("a1b2", 12, &theme, None, None)
            .expect("hay panel")
            .to_vec();
        assert_ne!(first, narrow, "con otro ancho hay que volver a envolver");
    }

    /// Y se tira al publicar: una maqueta que sobrevive a su fichero enseña lo de antes para siempre.
    #[test]
    fn la_maqueta_se_tira_cuando_el_productor_publica() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, r#"{"header":false,"lines":["lo viejo"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();
        state.lay_out_if_needed("a1b2", 40, &theme, None, None);

        publish(&dir, r#"{"header":false,"lines":["lo nuevo"]}"#);
        age_the_file(&dir, 1);
        state.refresh(dir.path());

        let rows = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("hay panel");
        assert!(
            rows.iter().any(|row| row.text.contains("lo nuevo")),
            "la maqueta se quedó con lo viejo: {rows:?}"
        );
    }

    /// La referencia es cadena ajena igual que el texto, y desde este PR se pinta: un `\n` dentro
    /// descuadraría la columna entera.
    #[test]
    fn la_referencia_pasa_por_el_mismo_saneo_que_el_texto() {
        let rows = what_gets_painted(r#"{"header":false,"lines":[{"ref":"a\nb","text":"t"}]}"#);

        assert_eq!(rows[0].0, "  a b t");
    }

    /// **Nada de lo que venga de fuera puede colgar ni reventar la envoltura**, y este es el sitio con
    /// más aritmética de índices de todo el panel: hay un corte por bytes sobre un índice que sale de
    /// recorrer caracteres, y un bucle con dos salidas.
    #[test]
    fn la_envoltura_aguanta_cualquier_cosa_sin_panico_ni_cuelgue() {
        let many_tildes = "ñ".repeat(300);
        let cases = [
            "",
            " ",
            "                    ",
            "a",
            many_tildes.as_str(),
            "🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂",
            "e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}",
            "palabra corta",
            "unapalabramuylargasinespaciosquenocabeenningunhueco",
            "मानक हिन्दी",
            "\u{200b}\u{200b}\u{200b}",
        ];

        for width in [0, 1, 2, 7, 8, 9, 40] {
            for case in cases {
                let mut rows = Vec::new();
                wrap(&mut rows, 0, case, Color32::WHITE, width);
                wrap(
                    &mut rows,
                    width.saturating_add(5),
                    case,
                    Color32::WHITE,
                    width,
                );
                assert!(
                    rows.len() < 5_000,
                    "con ancho {width} y «{case}» salieron {} filas: la envoltura no progresa",
                    rows.len()
                );
            }
        }
    }

    /// **Una sangría legal del contrato no puede pintar fuera del panel.** El contrato admite hasta 64
    /// celdas y el panel mide 36: sin acotarla contra el ancho, la continuación se pintaba a 42 celdas
    /// del borde —recortada, o sea invisible— y **`clip` la contaba como fila visible**, así que se
    /// comía renglones y el «… N más» mentía. Una referencia larga llega al mismo sitio.
    #[test]
    fn una_sangria_legal_del_contrato_no_saca_el_texto_del_panel() {
        let width = 36;
        let file =
            r#"{"lines":[{"indent":40,"text":"una frase larga que no cabe de ninguna manera"}]}"#;

        let rows = lay_out(
            Some(&panel_with(file)),
            "a1b2",
            width,
            &Theme::default(),
            None,
            None,
        );

        for row in &rows {
            assert!(
                row.column + row.text.chars().count() <= width,
                "«{}» empieza en {} y mide {}: se sale de un panel de {width}",
                row.text,
                row.column,
                row.text.chars().count()
            );
        }
    }

    /// Y por el otro camino: una referencia larguísima empuja la cabeza igual que la sangría.
    #[test]
    fn una_referencia_larguisima_tampoco_saca_el_texto_del_panel() {
        let width = 36;
        let long = "R".repeat(50);
        let file = format!(r##"{{"lines":[{{"ref":"{long}","text":"algo que decir"}}]}}"##);

        let rows = lay_out(
            Some(&panel_with(&file)),
            "a1b2",
            width,
            &Theme::default(),
            None,
            None,
        );

        for row in &rows {
            assert!(
                row.column <= width,
                "una continuación arrancó en la columna {} de un panel de {width}",
                row.column
            );
        }
    }

    /// **Una línea con marca y sin texto es forma legal, y salía desplazada por su propio símbolo.**
    /// La rama de texto vacío de la envoltura devuelve su sangría —correcto para una continuación, no
    /// para la fila que lleva la cabeza pegada—, así que el símbolo y la referencia aparecían cinco
    /// celdas a la derecha de los de la línea de arriba, en el mismo bloque. Rompía la columna, que es
    /// justo lo que este PR entrega.
    #[test]
    fn una_linea_con_marca_y_sin_texto_no_se_desplaza() {
        let rows = lay_out(
            Some(&panel_with(
                r##"{"header":false,"lines":[
                    {"mark":"done","ref":"#1","text":"con texto"},
                    {"mark":"block","ref":"#2","text":""}
                ]}"##,
            )),
            "a1b2",
            36,
            &Theme::default(),
            None,
            None,
        );

        assert_eq!(
            rows[0].column, rows[1].column,
            "las dos empiezan en la misma columna o la columna no significa nada: {rows:?}"
        );
        assert_eq!(rows[0].column, 0);
    }

    /// **La cabeza no pasa por la envoltura, así que hay que acotarla aparte.** Una referencia más
    /// larga que el panel —el contrato no le pone techo— sacaba la primera fila entera fuera del
    /// rectángulo, y con ella todas sus continuaciones: se pintaban encima de la consola.
    #[test]
    fn una_referencia_mas_larga_que_el_panel_no_desborda_la_primera_fila() {
        let width = 36;
        let long = "R".repeat(48);

        let rows = lay_out(
            Some(&panel_with(&format!(
                r##"{{"header":false,"lines":[{{"ref":"{long}","text":"un texto cualquiera"}}]}}"##
            ))),
            "a1b2",
            width,
            &Theme::default(),
            None,
            None,
        );

        for row in &rows {
            assert!(
                row.column + row.text.chars().count() <= width,
                "«{}» empieza en {} y mide {}: se sale de un panel de {width}",
                row.text,
                row.column,
                row.text.chars().count()
            );
        }
    }

    /// **La sangría del contrato se respeta**, y nada lo ataba: ignorarla del todo dejaba la suite
    /// verde. Es un campo que el productor escribe para agrupar visualmente, así que perderlo en
    /// silencio le quita una herramienta sin decírselo.
    #[test]
    fn la_sangria_que_pide_el_productor_desplaza_su_linea() {
        let rows = lay_out(
            Some(&panel_with(
                r#"{"header":false,"lines":[{"text":"pegada"},{"indent":4,"text":"sangrada"}]}"#,
            )),
            "a1b2",
            36,
            &Theme::default(),
            None,
            None,
        );

        let flush = rows[0].text.find("pegada").expect("está");
        let indented = rows[1].text.find("sangrada").expect("está");
        assert_eq!(
            indented - flush,
            4,
            "la segunda tiene que arrancar cuatro celdas más a la derecha: {rows:?}"
        );
    }

    /// **Se corta por palabras, no por caracteres.** Cortando siempre por caracteres el panel sigue
    /// cabiendo y ninguna prueba caía, pero las palabras quedan partidas por la mitad y el panel deja
    /// de leerse de un vistazo, que es de lo que va este PR.
    #[test]
    fn la_envoltura_parte_por_palabras_mientras_pueda() {
        let rows = lay_out(
            Some(&panel_with(
                r#"{"header":false,"lines":["alfa beta gamma delta epsilon zeta"]}"#,
            )),
            "a1b2",
            16,
            &Theme::default(),
            None,
            None,
        );

        for row in &rows {
            let text = row.text.trim();
            assert!(
                text.split_whitespace().all(|word| {
                    ["alfa", "beta", "gamma", "delta", "epsilon", "zeta"].contains(&word)
                }),
                "«{text}» trae una palabra partida: {rows:?}"
            );
        }
    }

    /// **El titular del PR, atado.** La prueba de al lado compara dos maquetas iguales, y eso se cumple
    /// igual recalculando desde cero. Lo que hay que afirmar es que **no se recalcula**, y se ve en la
    /// identidad del préstamo: si se remaquetara, la dirección del vector cambiaría.
    #[test]
    fn la_maqueta_no_se_recalcula_cuando_ya_vale() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, r#"{"header":false,"lines":["algo que envolver"]}"#);
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let first = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("hay panel")
            .as_ptr();
        let second = state
            .lay_out_if_needed("a1b2", 40, &theme, None, None)
            .expect("hay panel")
            .as_ptr();

        assert_eq!(
            first, second,
            "se volvió a maquetar sin que cambiara ni el fichero ni el ancho"
        );
    }

    /// El aviso de lo que falta va **debajo** de lo que sí cabe, no encima de la primera fila.
    #[test]
    fn el_aviso_de_lo_que_falta_va_al_final_y_con_su_margen() {
        let (cell_width, height) = cell();
        let three_rows =
            Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, height * 3.0));

        let rows = where_each_row_falls(
            r#"{"header":false,"lines":["una","dos","tres","cuatro"]}"#,
            three_rows,
        );

        let (where_at, text) = rows.last().expect("hay filas");
        assert_eq!(text, "… 2 más");
        assert!(
            where_at.y > rows[0].0.y,
            "el aviso se pintó encima de la primera fila"
        );
        assert!(
            (where_at.x - (three_rows.left() + cell_width)).abs() < 0.01,
            "el aviso arrancó en {} y el margen deja el texto en {}",
            where_at.x,
            three_rows.left() + cell_width
        );
    }

    /// **Lo que cierra un bloque es un hueco de verdad, no «una línea sin texto».** Una línea con marca
    /// y referencia pero sin texto es forma legal, y tratarla como hueco parte el bloque en dos: su
    /// referencia deja de contar para el ancho de la columna, y la de arriba se queda con una columna
    /// más estrecha de la que necesita su vecina. Rompe justo lo que este PR entrega.
    #[test]
    fn una_linea_con_marca_y_sin_texto_no_cierra_el_bloque() {
        // Hacen falta **tres** líneas para que se note: la de sin texto entra en el bloque antes de
        // cerrarlo, así que con dos el ancho sale igual de las dos maneras. Con una tercera detrás, si
        // la de en medio cerrara bloque, su referencia larga no llegaría a la primera.
        let rows = lay_out(
            Some(&panel_with(
                r##"{"header":false,"lines":[
                    {"mark":"done","ref":"#1","text":"arriba"},
                    {"mark":"block","ref":"#22222"},
                    {"mark":"info","ref":"#3","text":"abajo"}
                ]}"##,
            )),
            "a1b2",
            36,
            &Theme::default(),
            None,
            None,
        );

        let column_of = |row: &Row, word: &str| {
            row.text.chars().count()
                - row
                    .text
                    .split_once(word)
                    .expect("lleva su texto")
                    .1
                    .chars()
                    .count()
                - word.chars().count()
        };
        assert_eq!(
            column_of(&rows[0], "arriba"),
            column_of(&rows[2], "abajo"),
            "las tres son un solo bloque: la de en medio no cierra nada, {rows:?}"
        );
        assert_eq!(
            column_of(&rows[0], "arriba"),
            "● #22222 ".chars().count(),
            "y el ancho lo fija la referencia más larga de las tres"
        );
    }

    #[test]
    fn sin_fichero_no_hay_panel_que_pintar() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut state = PanelState::default();

        state.refresh(dir.path());

        assert!(state.last_understood().is_none());
    }

    #[test]
    fn lo_publicado_se_lee_y_queda_listo_para_pintar() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut state = PanelState::default();
        publish(&dir, r#"{"phase":"build","lines":["una","otra"]}"#);

        state.refresh(dir.path());

        let panel = state.last_understood().expect("hay panel");
        assert_eq!(panel.lines.len(), 2);
        assert_eq!(panel.phase.as_deref(), Some("build"));
    }

    /// **El freno, comprobado por el efecto y no por un contador.** Se publica algo nuevo dejando la
    /// fecha del fichero donde estaba: si se releyera igual, el panel cambiaría. Como no se relee, se
    /// queda el de antes — que es lo que se pide, porque la señal de que hay algo nuevo es la fecha y
    /// esa no se movió.
    #[test]
    fn no_se_vuelve_a_leer_si_la_fecha_del_fichero_no_ha_cambiado() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut state = PanelState::default();
        publish(&dir, r#"{"lines":["la primera"]}"#);
        state.refresh(dir.path());
        let date = state
            .last_understood()
            .and_then(|panel| panel.written_at)
            .expect("la lectura trae la fecha del fichero");

        publish(&dir, r#"{"lines":["la segunda"]}"#);
        std::fs::File::open(dir.path().join(PANEL_FILE))
            .expect("abrir")
            .set_modified(date)
            .expect("devolver la fecha a donde estaba");
        state.refresh(dir.path());

        assert_eq!(
            state.last_understood().expect("sigue habiendo panel").lines[0].text,
            "la primera",
            "releyó un fichero cuya fecha no había cambiado"
        );
    }

    #[test]
    fn con_la_fecha_movida_si_se_relee() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut state = PanelState::default();
        publish(&dir, r#"{"lines":["la primera"]}"#);
        state.refresh(dir.path());

        publish(&dir, r#"{"lines":["la segunda"]}"#);
        age_the_file(&dir, 1);
        state.refresh(dir.path());

        assert_eq!(
            state.last_understood().expect("hay panel").lines[0].text,
            "la segunda"
        );
    }

    /// El productor machaca el fichero entero, así que una lectura puede caer justo en medio. Sin
    /// conservar el último bueno, el panel parpadearía a vacío en **cada** publicación.
    #[test]
    fn una_escritura_a_medias_no_borra_el_ultimo_panel_bueno() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut state = PanelState::default();
        publish(&dir, r#"{"lines":["lo bueno"]}"#);
        state.refresh(dir.path());

        publish(&dir, r#"{"lines":[{"text":"a med"#);
        age_the_file(&dir, 1);
        state.refresh(dir.path());

        assert_eq!(
            state
                .last_understood()
                .expect("el último bueno sigue ahí")
                .lines[0]
                .text,
            "lo bueno"
        );
    }

    /// **Un salto de línea dentro de una cadena ajena pinta varias filas encima de las siguientes.**
    /// Está medido en la fuente de epaint, no supuesto: `break_on_newline` viene puesto por omisión.
    /// Sin sanear, un `panel.json` con un `\n` en su texto tapa el resto del panel.
    #[test]
    fn un_salto_de_linea_en_el_texto_no_puede_pintar_dos_filas() {
        let sanitized = sanitize("primera\nsegunda");

        assert_eq!(sanitized, "primera segunda");
        assert!(!sanitized.contains('\n'));
    }

    /// El tabulador es el otro caso: avanza el ancho de varias celdas mientras la medida de anchura lo
    /// cuenta como cero, así que la columna se descuadra sin que nada lo anuncie.
    #[test]
    fn los_tabuladores_y_el_retorno_tampoco_sobreviven() {
        assert_eq!(sanitize("uno\tdos\r\ntres"), "uno dos  tres");
        assert_eq!(
            sanitize("\u{1b}[31mrojo"),
            " [31mrojo",
            "y las secuencias de escape"
        );
    }

    /// Se sustituyen por un espacio y no se borran: dos palabras separadas por un salto de línea
    /// pegadas se leerían como una sola palabra que el productor nunca escribió.
    #[test]
    fn lo_saneado_no_pega_palabras_que_estaban_separadas() {
        assert_eq!(sanitize("hola\nmundo"), "hola mundo");
    }

    #[test]
    fn el_texto_normal_pasa_intacto() {
        let as_is = "F1.6: el panel · 35.392 tokens — 18 %";
        assert_eq!(sanitize(as_is), as_is);
    }

    /// **Lo que el gesto de estrechar el panel vuelve alcanzable.** Antes de que su frontera se pudiera
    /// arrastrar, un panel de una o dos celdas solo aparecía en una ventana absurdamente estrecha; ahora
    /// se llega ahí con un gesto, y el suelo del arrastre deja exactamente ese caso.
    ///
    /// **Medido, y el número es el que decide:** la cabeza de una fila —el hueco del símbolo, la columna
    /// de la referencia y su separador— ocupa **siete columnas que no se pueden comprimir**, así que con
    /// el panel por debajo de eso no hay texto legible que quepa. Lo que esta prueba ata es que ese caso
    /// **no rompe nada**: se maqueta y se recorta sin pánico. Que además no se pinte fuera del panel lo
    /// garantiza el recortador del pintor, que es de quien llama.
    ///
    /// Lo que **no** se arregla aquí, y queda dicho: entre una y siete celdas el panel es una franja sin
    /// información. Es la pregunta que D10 dejó abierta —qué se deja de pintar en un panel muy estrecho—
    /// y su respuesta cambia con el plegado, no con esto.
    ///
    /// **Y lo que esta prueba NO afirma, para que nadie la lea como más fuerte de lo que es.** Llevaba una
    /// comprobación de que la cuenta del aviso cuadraba y se quitó al mutarla: `visible + leftover` es
    /// igual al total con el recorte bueno **y con el roto**, porque las dos versiones reparten las mismas
    /// filas entre los dos números. El recorte lo atan sus propias pruebas; aquí lo que se demuestra es
    /// que el camino se recorre entero sin romperse.
    #[test]
    fn con_el_panel_al_minimo_se_maqueta_y_se_recorta_sin_panico() {
        let theme = Theme::default();
        let dir = PanelDir::create().expect("crear el directorio");
        publish(
            &dir,
            r##"{"lines":[{"ref":"#12","text":"una linea larga que no cabe de ninguna manera","mark":"current","indent":8},{"text":"otra"},{"text":"y otra"}]}"##,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());

        for width in [0usize, 1, 2, 3] {
            let rows = state
                .lay_out_if_needed("sonda", width, &theme, None, None)
                .expect("hay panel que maquetar");
            assert!(
                !rows.is_empty(),
                "con un panel de {width} celdas no se maquetó ninguna fila"
            );
            clip(rows, 1);
        }
    }

    /// **Los tres símbolos del reclamo, preguntándoselos a la fuente y no a la memoria**, con el mismo
    /// criterio que los del cuerpo: lo que mide una `M`. Un glifo que la cara monoespaciada no tiene
    /// mide cero y sale como un cuadro; uno que viene de las fuentes de emoji mide casi el doble y
    /// **descuadra la columna**, que se lee peor todavía porque no parece un fallo de fuente sino un
    /// panel mal alineado. Los candidatos obvios de este vocabulario ya cayeron por ahí.
    #[test]
    fn los_tres_simbolos_del_reclamo_miden_una_celda() {
        let ctx = eframe::egui::Context::default();
        let mut widths = Vec::new();
        let mut cell = 0.0;
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let font = eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE);
            ui.ctx().fonts_mut(|fonts| {
                cell = fonts.glyph_width(&font, 'M');
                widths = Attention::ALL
                    .iter()
                    .map(|class| {
                        let symbol = attention_symbol(*class);
                        (symbol, fonts.glyph_width(&font, symbol))
                    })
                    .collect();
            });
        })
        .drop_without_applying_deltas();

        for (symbol, width) in widths {
            assert!(
                (width - cell).abs() < 0.01,
                "«{symbol}» mide {width} y una celda mide {cell}: {}",
                if width == 0.0 {
                    "no está en la fuente"
                } else {
                    "viene de la de emoji y descuadraría la columna"
                }
            );
        }
    }

    /// **Ninguno de los tres es el de una marca del cuerpo**, y no es cosmética: el mismo signo en los
    /// dos sitios invita a leer que la aplicación dedujo el uno del otro, que es exactamente lo que el
    /// contrato le prohíbe. Con el juego repetido, un panel con `mark: block` en una línea y sin
    /// reclamo se vería igual que uno que reclama.
    #[test]
    fn cada_reclamo_lleva_su_simbolo_y_ninguno_se_repite_con_los_del_cuerpo() {
        assert_eq!(
            [
                attention_symbol(Attention::Wait),
                attention_symbol(Attention::Block),
                attention_symbol(Attention::Done),
            ],
            ['?', '⊘', '∎']
        );

        let of_the_body = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ]
        .map(symbol);
        for class in Attention::ALL {
            let its_own = attention_symbol(class);
            assert!(
                !of_the_body.contains(&its_own),
                "«{its_own}» ya lo usa una marca del cuerpo: el panel diría dos cosas con un solo signo"
            );
        }
    }

    /// **La bandera del productor no esconde el reclamo.** `header: false` quita la línea de identidad
    /// —la que es suya— y el reclamo sigue, porque la respuesta a «¿tengo que ir ahí?» es de la
    /// aplicación. Pintándolo en la línea de identidad, un productor la apagaría y la división
    /// desaparecería del radar sin que nadie lo notara.
    #[test]
    fn el_reclamo_se_pinta_aunque_el_productor_apague_su_cabecera() {
        let theme = Theme::default();

        let rows =
            what_gets_painted(r#"{"header":false,"attention":"wait","lines":["lo que sea"]}"#);

        assert_eq!(
            rows,
            vec![
                (
                    "? te espera a ti".to_owned(),
                    theme.attention_color(Attention::Wait, true)
                ),
                ("  lo que sea".to_owned(), theme.panel_color(None, None)),
            ]
        );
    }

    /// Y con la cabecera puesta, el reclamo va **debajo de la identidad y encima del cuerpo**: arriba,
    /// que es donde el recorte no llega.
    #[test]
    fn el_reclamo_va_arriba_del_todo_con_la_identidad_puesta() {
        let rows =
            what_gets_painted(r#"{"phase":"build","attention":"done","lines":["el cuerpo"]}"#);

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            [
                "a1b2 · build",
                "∎ terminó y nadie lo ha mirado",
                "  el cuerpo"
            ]
        );
    }

    /// **Una marca del cuerpo no es un reclamo de la división**, y esta prueba entra por el pintado
    /// para decirlo donde se ve: un `mark: block` es contenido del resumen —«esperé aprobación el
    /// martes»— y deducir de él que la división reclama la dejaría marcada para siempre.
    #[test]
    fn una_marca_del_cuerpo_no_pinta_reclamo_en_la_fila_de_la_aplicacion() {
        let rows = what_gets_painted(
            r#"{"header":false,"lines":[{"text":"esperé aprobación el martes","mark":"block"}]}"#,
        );

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["× esperé aprobación el martes"]);
    }

    /// **Un reclamo que nadie sostiene se atenúa, y no se apaga.** El idioma de este panel es confesar
    /// y no esconder —por eso un productor rancio sigue teniendo panel—, así que su reclamo sigue
    /// pintándose: lo que cambia es el color, que dice «reclama, y no hay nadie detrás». El detalle
    /// —desde cuándo— lo sigue diciendo el aviso, en su propia línea.
    #[test]
    fn un_reclamo_que_nadie_sostiene_se_atenua_y_el_panel_lo_confiesa_aparte() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(
            &dir,
            r#"{"header":false,"attention":"block","stale_after_minutes":1,"lines":["lo que sea"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let rows = texts_and_colors(paint_the_state(
            &mut state,
            PLACE,
            SystemTime::now() + Duration::from_secs(60 * 60),
        ));

        assert_eq!(
            rows[0],
            (
                "⊘ atascada por algo que no eres tú".to_owned(),
                theme.attention_color(Attention::Block, false)
            )
        );
        assert_ne!(
            rows[0].1,
            theme.attention_color(Attention::Block, true),
            "sin nadie que lo sostenga tiene que verse distinto, o el atenuado no dice nada"
        );
        assert!(
            rows[1].0.contains("sin noticias"),
            "el reclamo no sustituye a la confesión, van los dos: {rows:?}"
        );
    }

    /// Y mientras el productor lo sostiene, el mismo panel se pinta con el color entero: sin esta
    /// mitad, atenuarlo siempre pasaría por bueno.
    #[test]
    fn el_mismo_reclamo_se_pinta_entero_mientras_su_productor_lo_sostiene() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(
            &dir,
            r#"{"header":false,"attention":"block","stale_after_minutes":1,"lines":["lo que sea"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let rows = texts_and_colors(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert_eq!(
            rows[0].1,
            theme.attention_color(Attention::Block, true),
            "recién publicado, nadie tiene nada que confesar: {rows:?}"
        );
    }

    /// **Sin panel bueno no hay reclamo**, aunque la aplicación tenga mucho que confesar: lo que
    /// confiesa entonces es que no hay panel. Un documento que nunca se pudo leer no dijo nada que
    /// reclamar, y marcar la división por él sería inventarse la señal.
    #[test]
    fn un_panel_que_nunca_se_pudo_leer_no_reclama_nada() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(&dir, "esto no es json");
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let now = SystemTime::now();

        assert!(
            state.warning(now).is_some(),
            "la aplicación tenía que tener algo que confesar aquí"
        );
        assert_eq!(state.attention(now), None);
    }

    /// El reclamo se va cuando su productor lo quita, y no se queda pegado a la maqueta de antes.
    #[test]
    fn el_reclamo_desaparece_cuando_el_productor_deja_de_reclamar() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(
            &dir,
            r#"{"header":false,"attention":"wait","lines":["lo que sea"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        paint_the_state(&mut state, PLACE, SystemTime::now());

        publish(&dir, r#"{"header":false,"lines":["lo que sea"]}"#);
        age_the_file(&dir, 1);
        state.refresh(dir.path());
        let rows = texts_and_colors(paint_the_state(&mut state, PLACE, SystemTime::now()));

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["  lo que sea"]);
    }

    /// **La fila del reclamo se envuelve como cualquier otra.** Sus frases son las del contrato, y la
    /// del bloqueo mide treinta y tres celdas: en un panel estrecho —que es un ancho que el usuario
    /// elige arrastrando— tiene que partirse, no salirse. Sin esta prueba, pasarle a su envoltura un
    /// ancho equivocado dejaba la suite entera en verde, porque el sitio de las demás pruebas es ancho.
    #[test]
    fn la_fila_del_reclamo_no_se_sale_de_un_panel_estrecho() {
        let width = 12;

        let rows = lay_out(
            Some(&panel_with(r#"{"header":false,"lines":["lo que sea"]}"#)),
            "a1b2",
            width,
            &Theme::default(),
            None,
            Some(AttentionState {
                class: Attention::Block,
                backed: true,
            }),
        );

        assert!(
            rows.len() > 2,
            "con este ancho la frase tenía que partirse en varias filas: {rows:?}"
        );
        for row in &rows {
            assert!(
                row.column + row.text.chars().count() <= width,
                "«{}» empieza en {} y mide {}: se sale de un panel de {width}",
                row.text,
                row.column,
                row.text.chars().count()
            );
        }
    }

    /// **Cuando solo cabe una fila, la que sobrevive es la del reclamo.** De las dos que pone la
    /// aplicación, la del reclamo es la que contesta «¿tengo que ir ahí?» y el aviso es el que matiza
    /// «y no es fresco», así que el orden es ése y no el contrario. Queda atado aquí porque el recorte
    /// es silencioso: se lleva las últimas filas sin que nada falle.
    #[test]
    fn en_un_panel_de_un_solo_renglon_lo_que_queda_es_el_reclamo() {
        let (_, height) = cell();
        let dir = PanelDir::create().expect("crear el directorio");
        publish(
            &dir,
            r#"{"header":false,"attention":"wait","stale_after_minutes":1,"lines":["lo que sea"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());

        let rows = texts_and_colors(paint_the_state(
            &mut state,
            Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, 2.0 * height)),
            SystemTime::now() + Duration::from_secs(60 * 60),
        ));

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            ["? te espera a ti", "… 3 más"],
            "el reclamo tiene que ser lo primero que se pinta, y lo que falta tiene que decirse"
        );
    }

    /// **Lo terminado no se apaga por callar.** Quien acabó deja de publicar por definición, así que el
    /// silencio no desmiente su «terminó y nadie lo ha mirado»: es su estado normal. Sin esta
    /// excepción, media hora después de acabar —el plazo de serie— todo lo terminado se pintaría
    /// apagado, y encontrar por la mañana lo que acabó de noche es justo para lo que sirve.
    ///
    /// **Y la aplicación sigue confesando el silencio en su línea**: lo que la excepción cambia es el
    /// color del reclamo, no lo que el panel cuenta.
    #[test]
    fn lo_terminado_se_sigue_viendo_entero_por_mucho_que_calle() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(
            &dir,
            r#"{"header":false,"attention":"done","stale_after_minutes":1,"lines":["lo que sea"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        let rows = texts_and_colors(paint_the_state(
            &mut state,
            PLACE,
            SystemTime::now() + Duration::from_secs(60 * 60),
        ));

        assert_eq!(
            rows[0].1,
            theme.attention_color(Attention::Done, true),
            "una hora callado, y lo terminado sigue entero: {rows:?}"
        );
        assert!(
            rows[1].0.contains("sin noticias"),
            "el silencio se sigue confesando, que es otra cosa: {rows:?}"
        );
    }

    /// **Pero sí se apaga cuando su fichero deja de leerse**, que es lo único que de verdad desmiente
    /// lo que se está enseñando: sin poder leerlo, lo que hay en pantalla es lo último que llegó y nada
    /// dice que siga siendo verdad. Sin esta mitad, la excepción de arriba sería «lo terminado no se
    /// apaga nunca», que es esconder.
    #[test]
    fn lo_terminado_si_se_apaga_cuando_su_fichero_deja_de_leerse() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(
            &dir,
            r#"{"header":false,"attention":"done","lines":["lo que sea"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();

        publish(&dir, "esto ya no es json");
        age_the_file(&dir, 1);
        state.refresh(dir.path());
        let rows = texts_and_colors(paint_the_state(&mut state, PLACE, SystemTime::now()));

        assert_eq!(
            rows[0].1,
            theme.attention_color(Attention::Done, false),
            "sin poder leer su fichero, nadie sostiene lo que se ve: {rows:?}"
        );
    }

    /// Y la excepción es **solo de lo terminado**: lo que te espera y lo que está atascado siguen
    /// apagándose al quedarse sin nadie detrás, porque ahí el silencio sí desmiente lo que dicen.
    #[test]
    fn la_excepcion_de_lo_terminado_no_alcanza_a_las_otras_dos_clases() {
        let theme = Theme::default();

        for (class, value) in [(Attention::Wait, "wait"), (Attention::Block, "block")] {
            let dir = PanelDir::create().expect("crear el directorio");
            publish(
                &dir,
                &format!(
                    r#"{{"header":false,"attention":"{value}","stale_after_minutes":1,"lines":["x"]}}"#
                ),
            );
            let mut state = PanelState::default();
            state.refresh(dir.path());

            let rows = texts_and_colors(paint_the_state(
                &mut state,
                PLACE,
                SystemTime::now() + Duration::from_secs(60 * 60),
            ));

            assert_eq!(
                rows[0].1,
                theme.attention_color(class, false),
                "{class:?} tenía que apagarse tras una hora callada: {rows:?}"
            );
        }
    }

    /// **Una palabra que no existe no marca nada, y no se lleva por delante el resto del panel.** El
    /// campo lo escribe un tercero a mano, así que `waiting` por `wait` es cuestión de tiempo: la
    /// aplicación no puede adivinar cuál de los tres quiso decir —adivinarlo sería deducir— ni
    /// castigar al resto del documento por una palabra suelta.
    ///
    /// **Entra por el pintado porque es ahí donde se ve.** La caja del contrato ya afirma que ese campo
    /// se lee vacío, pero no que la identidad y el cuerpo sigan pintándose: son dos cosas distintas y
    /// esta es la de este hito.
    #[test]
    fn un_reclamo_que_no_existe_no_marca_y_el_resto_del_panel_se_pinta_entero() {
        let rows = what_gets_painted(
            r#"{"attention":"waiting","phase":"validate","lines":["el cuerpo"]}"#,
        );

        let texts: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["a1b2 · validate", "  el cuerpo"]);
    }

    /// **Lo que [`PanelState::attention`] contesta desde fuera es lo mismo que el panel pinta.** Son dos
    /// caminos: el pintado ya tiene el aviso en la mano y pregunta por dentro; el de fuera —la puerta
    /// por la que preguntará quien acumule los reclamos de una pestaña— tiene que calcularlo él.
    ///
    /// **Sin esta prueba el de fuera no lo mira nadie**: comprobado invirtiendo ahí la pregunta del
    /// aviso —dar por sostenido justo lo que no lo está y al revés— con la suite entera en verde. Lo
    /// que se afirma es el valor, no que las dos puertas coincidan, porque coincidir en lo falso
    /// también coincide.
    #[test]
    fn lo_que_reclamo_contesta_desde_fuera_es_lo_que_el_panel_pinta() {
        let dir = PanelDir::create().expect("crear el directorio");
        publish(
            &dir,
            r#"{"header":false,"attention":"wait","stale_after_minutes":1,"lines":["lo que sea"]}"#,
        );
        let mut state = PanelState::default();
        state.refresh(dir.path());
        let theme = Theme::default();
        let just_now = SystemTime::now();
        let gone_quiet = just_now + Duration::from_secs(60 * 60);

        assert_eq!(
            state.attention(just_now),
            Some(AttentionState {
                class: Attention::Wait,
                backed: true
            }),
            "recién publicado no hay nada que confesar de este panel"
        );
        assert_eq!(
            state.attention(gone_quiet),
            Some(AttentionState {
                class: Attention::Wait,
                backed: false
            }),
            "tras una hora callado ya no hay nadie detrás de lo que se enseña"
        );

        for when in [just_now, gone_quiet] {
            let answer = state.attention(when).expect("reclama");
            let rows = texts_and_colors(paint_the_state(&mut state, PLACE, when));
            assert_eq!(
                rows[0].1,
                theme.attention_color(answer.class, answer.backed),
                "lo que se pinta no es lo que se contesta desde fuera: {rows:?}"
            );
        }
    }

    /// **Las tres frases viven dos veces**: en el código que las pinta y en el documento del
    /// contrato, palabra por palabra en los dos sitios. Su propio comentario dice que son «las del
    /// contrato, palabra por palabra» — y nada lo comprobaba.
    ///
    /// Es la misma clase de acoplamiento que el del guion de medida con el informe: dos ficheros que
    /// tienen que decir lo mismo y ningún sitio donde eso se afirme.
    #[test]
    fn las_tres_frases_del_reclamo_son_las_que_el_contrato_publica() {
        const CONTRACT: &str = include_str!("../../../docs/04-contrato-panel.md");

        // **Se busca en la fila de SU clave, no en el fichero.** Un `contains` sobre el documento
        // entero pasa con las tres frases **rotadas entre clases** —comprobado midiéndolo— y con
        // cualquier trozo suelto: «a ti» está dentro de «te espera a ti». Eso es exactamente lo que
        // el mensaje de abajo dice impedir, así que el `contains` suelto no cubría su propia
        // promesa. La fila la identifica la clave del contrato, que va en la primera columna.
        for (class, key) in [
            (Attention::Wait, "wait"),
            (Attention::Block, "block"),
            (Attention::Done, "done"),
        ] {
            let phrase = attention_text(class);
            let row = CONTRACT
                .lines()
                .find(|line| line.starts_with(&format!("| `{key}`")))
                .unwrap_or_else(|| {
                    panic!(
                        "el documento del contrato no tiene fila para «{key}»: o cambió de forma, \
                         o dejó de publicar un valor que la aplicación sí pinta"
                    )
                });
            // **Y se compara la frase ENTERA, no si está dentro.** Un `contains` sobre la fila pasa
            // con la frase truncada —«te espera» está dentro de «te espera a ti»—, y la frase larga
            // es justo lo que su comentario defiende: el «a ti» es lo que la separa de la segunda.
            // Así que se saca lo que la fila publica como frase y se comparan por igualdad.
            let published = row
                .split('|')
                .nth(2)
                .expect("la fila del contrato tiene tres columnas")
                .trim()
                .replace("**", "");
            let published = published.split(':').next().unwrap_or(&published).trim();
            assert_eq!(
                published, phrase,
                "el panel pinta «{phrase}» para {class:?} y la fila de «{key}» del contrato publica \
                 «{published}»: el productor que lea el contrato espera un texto y ve otro"
            );
        }
    }
}
