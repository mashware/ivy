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
use panel_contract::{Line, Mark, Panel, Problem, Style};

use crate::pintor::Metrica;
use crate::tema::Tema;

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
pub(crate) const ANCHO_DE_FABRICA: usize = 36;

/// Lo menos que se le deja a la consola cuando su panel le quita sitio, en celdas.
///
/// **El panel cede, no la consola.** El panel es margen y la consola es el trabajo: en una ventana
/// estrecha lo que sobra es el estado, no el programa. Con veinte columnas una consola sigue siendo
/// usable; con cinco, no.
///
/// **Y es también el suelo que decide si una división se puede partir en dos**, que es el otro sitio
/// donde hace falta saber cuándo una consola deja de servir. Son la misma pregunta, así que son la misma
/// constante: dos números para «lo menos que necesita una consola» acabarían diciendo cosas distintas.
pub(crate) const CONSOLA_MINIMA: f32 = 20.0;

/// Lo menos que se le deja al panel cuando se arrastra su frontera, en celdas.
///
/// **Con una celda el panel casi no se ve, y eso es lo que se busca: que siempre quede dónde agarrar.**
/// Llegando a cero, la frontera se iría con el panel y el gesto que lo cerró no se podría deshacer con
/// el ratón — un viaje sin billete de vuelta.
///
/// Es el suelo del **gesto**, no del reparto: [`apartar`] sigue pudiendo devolver un panel **más estrecho
/// que esto** —hasta de ancho cero— cuando la división no da para más. Son dos preguntas distintas, lo
/// que el usuario pide y lo que cabe, y confundirlas dejaría a la consola por debajo de su mínimo. (Lo
/// que `apartar` no devuelve nunca es *ningún* panel una vez se le pide un ancho: en el caso extremo
/// devuelve el rectángulo vacío, y su prueba lo dice así.)
///
/// **Y desde que se puede plegar, esto es exactamente la raya entre estrechar y plegar**: pedir menos que
/// esto es lo que el gesto lee como «ciérralo». Antes de que el plegado existiera, este mismo número
/// servía para lo contrario —impedir llegar a cero, porque la frontera se habría ido con el panel— y esa
/// razón ya no es la suya: ahora lo que sostiene el regreso es el asidero que sobrevive al plegado.
pub(crate) const SUELO_AL_ARRASTRAR: usize = 1;

/// Lo que se deja de margen a la izquierda del texto, en celdas.
///
/// Una celda y no cero: pegado al borde, el texto se lee como un error de pintado. A la derecha no
/// hace falta, porque ahí no hay nada — el panel es lo último de la división.
const MARGEN: f32 = 1.0;

/// Cómo queda repartido el rectángulo de una división entre lo que se ve.
///
/// Tiene nombre, y no es una tupla, por lo mismo que lo tiene [`crate::Reparto`], que hace lo propio
/// entre las dos consolas: dos repartos en el mismo fotograma que se leyeran distinto serían dos
/// idiomas para una sola idea.
pub struct Hueco {
    /// Lo que le queda a la consola, que es contra lo que hay que medir su rejilla.
    pub consola: Rect,
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
/// **`celdas_del_panel` lleva las dos cosas en un solo parámetro, y no es por brevedad.** «Si hay algo
/// que enseñar» y «cuánto ancho se lleva» son la misma decisión tomada en el mismo sitio: con dos
/// parámetros existe el estado escribible «no hay panel, pero con treinta y seis celdas», y este
/// módulo ya pagó una vez tener **dos criterios paralelos** para si el panel se ve — uno en el reparto
/// del ancho y otro en el pintado, que se contradecían.
///
/// El nombre dice la unidad a propósito: aquí conviven celdas y puntos, y un mismo nombre para las dos
/// deja al lector sin saber cuál está mirando.
pub fn apartar(division: Rect, ancho_de_celda: f32, celdas_del_panel: Option<usize>) -> Hueco {
    let Some(celdas) = celdas_del_panel else {
        return Hueco {
            consola: division,
            panel: None,
        };
    };

    let en_puntos = celdas as f32 * ancho_de_celda;
    let disponible = (division.width() - CONSOLA_MINIMA * ancho_de_celda).max(0.0);
    let corte = division.right() - en_puntos.min(disponible);

    Hueco {
        consola: Rect::from_min_max(division.left_top(), Pos2::new(corte, division.bottom())),
        panel: Some(Rect::from_min_max(
            Pos2::new(corte, division.top()),
            division.right_bottom(),
        )),
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
pub struct Estado {
    ultimo_bueno: Option<Panel>,
    /// Lo maquetado, y para qué se maquetó.
    ///
    /// **Existe por coste, y el escenario es presente**: mientras una consola vuelca, la ventana
    /// repinta todos los fotogramas, y envolver el panel al ancho se recalcularía sesenta veces por
    /// segundo sin que su fichero haya cambiado.
    maqueta: Option<Maqueta>,
    /// Por qué falló la última lectura, si falló. Es lo que distingue «aquí no hay productor» de «hay
    /// uno y está roto», que se ven igual y no son lo mismo.
    problema: Option<Problem>,
}

/// Una maqueta y las dos cosas de las que depende: si cambia cualquiera, hay que rehacerla.
struct Maqueta {
    ancho: usize,
    /// **La línea que pone la app entra en la clave de la caché.** Un panel que envejece cambia lo que
    /// hay que pintar sin que su fichero se haya tocado, así que una caché que solo mirara el ancho
    /// seguiría enseñando la maqueta de cuando el panel era fresco — y el aviso no aparecería nunca.
    aviso: Option<String>,
    filas: Vec<Fila>,
}

impl Estado {
    /// Vuelve a leer el panel si su fichero ha cambiado.
    ///
    /// **No cronometra nada**: quien mide es la ventana, con un solo reloj alrededor de leer y pintar.
    /// Dos relojes para un tramo que se publica como uno es la forma más fácil de que el número deje de
    /// significar lo que dice.
    pub fn refrescar(&mut self, dir: &std::path::Path) {
        // La fecha con la que se compara es la de la última lectura buena, que es la que el propio
        // contrato devuelve. Guardarla aparte sería tener dos copias del mismo dato.
        let visto = self
            .ultimo_bueno
            .as_ref()
            .and_then(|panel| panel.written_at);

        match panel_contract::read_if_newer(dir, visto) {
            Ok(Some(panel)) => {
                self.ultimo_bueno = Some(panel);
                self.problema = None;
                self.maqueta = None;
            }
            // **Lo que hay sigue vigente, y eso incluye borrar el problema anterior.** Que la lectura
            // llegue hasta aquí significa que se pudo preguntar por el fichero y que su fecha es la del
            // panel que ya se tiene: lo que se enseña **es** lo último, y un fallo de hace un rato ya no
            // describe la realidad. Sin esto, un tropiezo pasajero —quedarse sin descriptores, un
            // montaje de red que parpadea— acusaría al productor para siempre, porque su fichero no va a
            // volver a cambiar de fecha por sí solo.
            Ok(None) => self.problema = None,
            Err(problema) => self.problema = Some(problema),
        }
    }

    /// El último panel que se entendió, si hay alguno.
    ///
    /// **Solo lo usan las pruebas, y por eso está detrás de su bandera.** El pintado no lo necesita —
    /// va por [`Estado::maquetar_si_hace_falta`], que puede componer un panel que es solo la confesión
    /// de la aplicación— y dejarlo como método de producción sería una puerta abierta a que alguien
    /// vuelva a decidir «hay panel» por su cuenta, que es justo lo que [`Estado::se_ve`] centraliza.
    #[cfg(test)]
    pub(crate) fn a_pintar(&self) -> Option<&Panel> {
        self.ultimo_bueno.as_ref()
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
    pub fn se_ve(&self, ahora: SystemTime) -> bool {
        self.ultimo_bueno.is_some() || self.aviso(ahora).is_some()
    }

    /// Lo que la aplicación tiene que decir de su cuenta sobre este panel, si tiene algo que decir.
    ///
    /// **Es la línea de la app, no una línea del productor**, y por eso no depende de `header`: esa
    /// bandera decide si se pinta la identidad de la división, no si la aplicación puede ser honesta.
    /// Un productor que la apagara para esconder que su panel está roto sería justo el caso que esto
    /// viene a impedir.
    fn aviso(&self, ahora: SystemTime) -> Option<String> {
        // **El fichero que todavía no existe es la excepción, y va primero por eso.** El resto se
        // ordena por lo que hay que decir: primero los fallos de lectura, que son lo más reciente que
        // se sabe, y solo si no hay ninguno se mira el reloj.
        match (&self.problema, &self.ultimo_bueno) {
            (Some(cual), _) if es_falta_de_fichero(cual) => self
                .ultimo_bueno
                .as_ref()
                .map(|_| "el fichero del panel ya no está".to_owned()),
            (Some(cual), Some(_)) => Some(format!("lo que se ve no es lo último: {cual}")),
            (Some(cual), None) => Some(format!("sin panel: {cual}")),
            (None, Some(panel)) if panel.is_stale_at(ahora) => Some(sin_noticias(panel, ahora)),
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
    pub fn cuanto_falta_para_avisar(&self, ahora: SystemTime) -> Option<Duration> {
        // Con un problema el aviso ya está puesto y no cambia con el tiempo: no hay a qué volver.
        if self.problema.is_some() {
            return None;
        }
        let panel = self.ultimo_bueno.as_ref()?;
        let silencio = ahora.duration_since(panel.written_at?).ok()?;

        match panel.stale_after.checked_sub(silencio) {
            // Todavía no es viejo: lo próximo que cambia es que lo sea.
            Some(falta) => Some(falta),
            // Ya lo es, y lo que se enseña es un número redondeado: se vuelve cuando salte.
            None => Some(hasta_el_siguiente_salto(silencio)),
        }
    }

    /// La maqueta para este ancho y este aviso, calculándola solo si no vale la que hay.
    fn maquetar_si_hace_falta(
        &mut self,
        id: &str,
        ancho: usize,
        tema: &Tema,
        aviso: Option<String>,
    ) -> Option<&[Fila]> {
        if self.ultimo_bueno.is_none() && aviso.is_none() {
            return None;
        }

        let vigente =
            matches!(&self.maqueta, Some(hecha) if hecha.ancho == ancho && hecha.aviso == aviso);
        if !vigente {
            self.maqueta = Some(Maqueta {
                filas: maquetar(
                    self.ultimo_bueno.as_ref(),
                    id,
                    ancho,
                    tema,
                    aviso.as_deref(),
                ),
                ancho,
                aviso,
            });
        }
        self.maqueta.as_ref().map(|hecha| hecha.filas.as_slice())
    }
}

/// Si el problema es que el fichero no está, que es el único que **no** se enseña por sí solo.
fn es_falta_de_fichero(problema: &Problem) -> bool {
    matches!(problema, Problem::Missing)
}

/// Lo que se dice de un panel que lleva callado más de lo suyo.
///
/// **Sin cuánto cuando el plazo es de menos de un minuto**: el contrato permite un
/// `stale_after_minutes` de cero, y «sin noticias desde hace 0 min» un segundo después de publicar se
/// lee como un fallo de la aplicación, no como una confesión.
fn sin_noticias(panel: &Panel, ahora: SystemTime) -> String {
    match ahora.duration_since(panel.written_at.unwrap_or(ahora)) {
        Ok(silencio) if silencio.as_secs() >= 60 => {
            format!("sin noticias desde hace {}", en_palabras(silencio))
        }
        _ => "sin noticias".to_owned(),
    }
}

/// Un rato, dicho como lo diría alguien: minutos hasta que son demasiados, y entonces horas.
///
/// Sin segundos ni decimales a propósito. Lo que el aviso tiene que transmitir es «hace mucho», y un
/// número que cambia cada segundo en una esquina de la pantalla es ruido, no información — y cada
/// cambio cuesta un fotograma, porque hay que despertar a la ventana para pintarlo.
fn en_palabras(cuanto: Duration) -> String {
    let minutos = cuanto.as_secs() / 60;
    match minutos {
        0..=119 => format!("{minutos} min"),
        _ => format!("{} h", minutos / 60),
    }
}

/// Cuánto falta para que el número de [`en_palabras`] cambie.
///
/// Es lo que hace que el aviso no se pudra en pantalla sin despertar a la ventana más de lo justo:
/// mientras se digan minutos se vuelve al siguiente minuto, y cuando se digan horas, a la siguiente
/// hora.
fn hasta_el_siguiente_salto(silencio: Duration) -> Duration {
    let paso = if silencio.as_secs() < 2 * 60 * 60 {
        60
    } else {
        60 * 60
    };

    Duration::from_secs(paso - silencio.as_secs() % paso)
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
const MAX_CARACTERES: usize = 512;

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
fn sanear(texto: &str) -> String {
    texto
        .chars()
        .take(MAX_CARACTERES)
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
fn simbolo(marca: Mark) -> char {
    match marca {
        Mark::Done => '●',
        Mark::Current => '▶',
        Mark::Pending => '·',
        Mark::Wait => '…',
        Mark::Block => '×',
        Mark::Info => '›',
    }
}

/// Lo menos que se le deja al texto cuando la sangría y la columna se lo comen todo, en celdas.
///
/// **Sin un suelo, la envoltura no progresa**: con el hueco a cero o negativo, cada vuelta del bucle
/// mete cero caracteres y la ventana se cuelga. El prototipo ya lo aprendió, y por eso su envoltura
/// lleva el mismo `max`.
const HUECO_MINIMO: usize = 8;

/// Una fila ya maquetada: en qué columna empieza, qué dice y de qué color.
#[derive(Debug, Clone, PartialEq)]
struct Fila {
    columna: usize,
    texto: String,
    color: Color32,
}

/// Convierte un panel en las filas que se van a pintar, al ancho dado en celdas.
///
/// **Aquí está todo lo que cuesta y nada de lo que dibuja**, y esa separación es lo que permite
/// recordar el resultado entre fotogramas: mientras el fichero y el ancho no cambien, la maqueta es la
/// misma. Sin ella, envolver el panel se recalcularía sesenta veces por segundo mientras la consola de
/// al lado vuelca.
fn maquetar(
    panel: Option<&Panel>,
    id: &str,
    ancho: usize,
    tema: &Tema,
    aviso: Option<&str>,
) -> Vec<Fila> {
    let mut filas = Vec::new();

    if panel.is_none_or(|panel| panel.header) {
        let fase = panel.and_then(|panel| panel.phase.as_deref());
        let identidad = match fase {
            Some(fase) => format!("{id} · {fase}"),
            None => id.to_owned(),
        };
        envolver(
            &mut filas,
            0,
            &sanear(&identidad),
            tema.color_del_panel(None, None),
            ancho,
        );
    }

    // **El aviso va arriba y no al final**, porque abajo es donde el recorte se come lo que no cabe:
    // el aviso de que un panel no es de fiar sería justo lo primero en desaparecer de un panel largo.
    if let Some(aviso) = aviso {
        envolver(
            &mut filas,
            0,
            &sanear(aviso),
            tema.color_del_panel(None, Some(Style::Warn)),
            ancho,
        );
    }

    // La fila en blanco la escribe la maqueta, no el pintado: separada del cuerpo, lo que pone la
    // aplicación se lee como lo que es y no como la primera línea del productor.
    if !filas.is_empty() {
        filas.push(Fila {
            columna: 0,
            texto: String::new(),
            color: Color32::TRANSPARENT,
        });
    }

    let Some(panel) = panel else {
        return filas;
    };

    for bloque in bloques(&panel.lines) {
        // **El ancho de la columna se calcula por bloque, y esa es la regla del contrato.** Con un
        // ancho global, un solo bloque con una referencia larga desplazaría el texto de todos los
        // demás, y lo que el panel gana alineando lo perdería en margen desperdiciado.
        // **Y la columna se acota contra el ancho del panel.** El contrato no le pone techo a la
        // referencia, así que una sola larguísima fijaría un ancho de columna que empuja el texto de
        // todo su bloque fuera del rectángulo. Lo que no cabe se corta, como se corta el texto.
        let columna = bloque
            .iter()
            .filter_map(|linea| linea.reference.as_deref())
            .map(|referencia| sanear(referencia).chars().count())
            .max()
            .unwrap_or(0)
            .min(ancho.saturating_sub(HUECO_MINIMO).saturating_sub(3));

        for linea in bloque {
            filas.extend(maquetar_linea(linea, columna, ancho, tema));
        }
    }

    filas
}

/// Parte las líneas en bloques por las líneas vacías, que es lo que el contrato usa para agruparlas.
///
/// El hueco **pertenece al bloque que cierra**: así se conserva como fila y la maqueta no tiene que
/// acordarse de reponerlo entre bloque y bloque.
fn bloques(lineas: &[Line]) -> Vec<Vec<&Line>> {
    let mut bloques = vec![Vec::new()];
    for linea in lineas {
        let hueco = linea.text.is_empty() && linea.reference.is_none() && linea.mark.is_none();
        bloques
            .last_mut()
            .expect("siempre hay uno abierto")
            .push(linea);
        if hueco {
            bloques.push(Vec::new());
        }
    }
    bloques
}

/// Maqueta una línea: su símbolo, su referencia alineada, su sangría y su texto envuelto.
fn maquetar_linea(linea: &Line, columna: usize, ancho: usize, tema: &Tema) -> Vec<Fila> {
    let color = tema.color_del_panel(linea.mark, linea.style);
    let referencia = linea.reference.as_deref().map(sanear).unwrap_or_default();
    let texto = sanear(&linea.text);

    // Un hueco de verdad no lleva prefijo ni ocupa columna: es solo una fila en blanco.
    if texto.is_empty() && referencia.is_empty() && linea.mark.is_none() {
        return vec![Fila {
            columna: 0,
            texto: String::new(),
            color,
        }];
    }

    // **La cabeza se acota antes de componerse, no después.** Símbolo, referencia y sangría se pintan
    // en la primera fila sin pasar por la envoltura, así que una referencia más larga que el panel
    // —forma legal: el contrato no le pone techo— sacaba esa fila entera fuera del rectángulo y con
    // ella todas sus continuaciones. Lo que sobra de la referencia se corta, igual que se corta el
    // texto: lo que no cabe no se puede leer, y fingir que sí desplaza todo lo demás.
    let tope = ancho.saturating_sub(HUECO_MINIMO);
    // Se corta al ancho de la columna, que ya viene acotado contra el panel: más larga que su columna,
    // la referencia empujaría el texto de su propia línea y rompería la alineación del bloque.
    let referencia: String = referencia.chars().take(columna).collect();

    let mut cabeza = String::new();
    match linea.mark {
        Some(marca) => cabeza.push(simbolo(marca)),
        // El hueco del símbolo se reserva también sin marca: es lo que mantiene el texto de todas las
        // líneas en la misma columna, que es de lo que va alinear.
        None => cabeza.push(' '),
    }
    cabeza.push(' ');
    if columna > 0 {
        cabeza.push_str(&format!("{referencia:columna$} "));
    }
    // La sangría se acota contra lo mismo: un `indent` de 40 celdas es forma legal —el contrato admite
    // hasta 64— y en un panel de 36 dejaría la continuación fuera del rectángulo, invisible y contada
    // como fila visible, comiéndose renglones y falseando el aviso de cuántas faltan.
    cabeza.push_str(
        &" ".repeat(
            linea
                .indent
                .min(tope.saturating_sub(cabeza.chars().count())),
        ),
    );

    let sangria = cabeza.chars().count().min(tope);
    let mut filas = Vec::new();
    envolver(&mut filas, sangria, &texto, color, ancho);

    // La cabeza se pega a la primera fila; las de continuación arrancan ya sangradas, que es lo que
    // alinea la envoltura bajo el texto y no bajo el símbolo.
    match filas.first_mut() {
        Some(primera) => {
            primera.texto = format!("{cabeza}{}", primera.texto);
            // **Y la primera fila arranca en cero, lleve texto o no.** La rama de texto vacío de la
            // envoltura devuelve su sangría —correcto para una continuación, no para la fila que lleva
            // la cabeza pegada—, así que una línea con marca y sin texto salía desplazada por el ancho
            // de su propio símbolo. Es forma legal del contrato.
            primera.columna = 0;
        }
        None => filas.push(Fila {
            columna: 0,
            texto: cabeza,
            color,
        }),
    }
    filas
}

/// Parte un texto en trozos que quepan, y los añade con la sangría de continuación puesta.
///
/// Corta **por palabras** cuando puede y por caracteres cuando una sola palabra no cabe, que es el caso
/// de una ruta larga o de un identificador. Los espacios múltiples se colapsan, como el contrato avisa:
/// no sirven para alinear.
fn envolver(filas: &mut Vec<Fila>, sangria: usize, texto: &str, color: Color32, ancho: usize) {
    let hueco = ancho.saturating_sub(sangria).max(HUECO_MINIMO);
    let mut pendiente: Vec<&str> = texto.split_whitespace().rev().collect();

    if pendiente.is_empty() {
        filas.push(Fila {
            columna: sangria,
            texto: String::new(),
            color,
        });
        return;
    }

    while let Some(palabra) = pendiente.pop() {
        let mut linea = String::new();
        let mut palabra = palabra;
        loop {
            let cabe = hueco - linea.chars().count();
            if palabra.chars().count() > cabe {
                if linea.is_empty() {
                    // Una palabra sola más larga que el hueco: se parte por caracteres, porque
                    // dejarla entera desbordaría el panel y saltar de línea no la haría caber nunca.
                    let corte = palabra
                        .char_indices()
                        .nth(cabe)
                        .map_or(palabra.len(), |(i, _)| i);
                    linea.push_str(&palabra[..corte]);
                    pendiente.push(&palabra[corte..]);
                }
                break;
            }
            linea.push_str(palabra);
            match pendiente.last() {
                Some(siguiente)
                    if linea.chars().count() + 1 + siguiente.chars().count() <= hueco =>
                {
                    linea.push(' ');
                    palabra = pendiente.pop().expect("acabo de mirarla");
                }
                _ => break,
            }
        }
        // La primera fila la coloca quien llama, pegándole la cabeza; las demás llevan su sangría.
        let columna = if filas.is_empty() { 0 } else { sangria };
        filas.push(Fila {
            columna,
            texto: linea,
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
pub fn pintar(
    painter: &Painter,
    estado: &mut Estado,
    id: &str,
    tema: &Tema,
    metrica: &Metrica,
    ahora: SystemTime,
) {
    let rect = painter.clip_rect();
    painter.rect_filled(rect, 0.0, tema.fondo_del_panel());

    // El ancho en celdas se descuenta el margen de los dos lados aunque solo se pinte el de la
    // izquierda: sin el de la derecha, la envoltura llenaría hasta el borde exacto del panel y el texto
    // quedaría pegado a la consola de al lado.
    let ancho = ((rect.width() / metrica.ancho()) as usize).saturating_sub(2 * MARGEN as usize);
    let aviso = estado.aviso(ahora);
    let Some(filas) = estado.maquetar_si_hace_falta(id, ancho, tema, aviso) else {
        return;
    };

    // **Sin un renglón entero no se pinta nada, ni siquiera el aviso.** Con un suelo de uno, un panel
    // de alto cero encolaba «… N más» fuera de su rectángulo: un aviso de que falta contenido, en un
    // sitio donde no cabe ningún contenido.
    let caben = (rect.height() / metrica.alto()) as usize;
    if caben == 0 {
        return;
    }
    let (visibles, sobran) = recortar(filas, caben);

    let izquierda = rect.left() + MARGEN * metrica.ancho();
    for (n, fila) in visibles.iter().enumerate() {
        let x = izquierda + fila.columna as f32 * metrica.ancho();
        let y = rect.top() + n as f32 * metrica.alto();
        if fila.texto.is_empty() {
            continue;
        }
        painter.text(
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            &fila.texto,
            metrica.fuente(),
            fila.color,
        );
    }

    if sobran > 0 {
        let y = rect.top() + (visibles.len() as f32) * metrica.alto();
        painter.text(
            Pos2::new(izquierda, y),
            Align2::LEFT_TOP,
            format!("… {sobran} más"),
            metrica.fuente(),
            tema.color_del_panel(Some(Mark::Pending), None),
        );
    }
}

/// Cuántas filas se pintan y cuántas se quedan fuera.
///
/// **El aviso ocupa su propia fila**, así que cuando algo sobra hay que pintar una menos: sin
/// descontarla, la última línea de contenido quedaría tapada por el aviso que dice que falta contenido.
fn recortar(filas: &[Fila], caben: usize) -> (&[Fila], usize) {
    if filas.len() <= caben {
        return (filas, 0);
    }
    let visibles = caben.saturating_sub(1);
    (&filas[..visibles], filas.len() - visibles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_contract::{FICHERO_DEL_PANEL, PanelDir};

    /// Un panel leído de un fichero de verdad, para probar la maqueta sin pasar por el pintado.
    fn panel_con(fichero: &str) -> Panel {
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(&dir, fichero);
        let mut estado = Estado::default();
        estado.refrescar(dir.path());
        estado.a_pintar().expect("hay panel").clone()
    }

    fn publicar(dir: &PanelDir, contenido: &str) {
        std::fs::write(dir.path().join(FICHERO_DEL_PANEL), contenido).expect("publicar el panel");
    }

    /// Adelanta la fecha del fichero para que la relectura la vea cambiada.
    ///
    /// A mano y no dejándolo al reloj: dos escrituras seguidas caen dentro del mismo tic en algunos
    /// sistemas de ficheros, y entonces la prueba pasaría o fallaría según la máquina.
    fn envejecer(dir: &PanelDir, segundos: u64) {
        let ruta = dir.path().join(FICHERO_DEL_PANEL);
        std::fs::File::open(&ruta)
            .expect("abrir")
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(segundos))
            .expect("mover la fecha");
    }

    /// Lo que mide una celda de ancho en las pruebas del reparto: fijo, para que el suelo salga
    /// redondo y los fallos se lean sin calcular nada.
    const ANCHO_DE_CELDA: f32 = 8.0;

    fn division_de(ancho: f32) -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(ancho, 300.0))
    }

    #[test]
    fn el_panel_se_lleva_su_ancho_y_la_consola_el_resto() {
        let hueco = apartar(division_de(800.0), ANCHO_DE_CELDA, Some(ANCHO_DE_FABRICA));
        let panel = hueco.panel.expect("hay panel");

        assert_eq!(panel.width(), ANCHO_DE_FABRICA as f32 * ANCHO_DE_CELDA);
        assert_eq!(
            hueco.consola.width() + panel.width(),
            800.0,
            "entre los dos tienen que cubrir la división, sin hueco ni solape"
        );
        assert_eq!(
            hueco.consola.right(),
            panel.left(),
            "el panel empieza justo donde acaba la consola"
        );
    }

    /// El panel es margen y la consola es el trabajo, así que en una ventana estrecha lo que se
    /// estrecha es el panel. Con la regla al revés, arrastrar el divisor dejaría a un programa
    /// interactivo sin sitio para escribir mientras su panel conserva sus treinta y seis columnas.
    #[test]
    fn en_una_division_estrecha_el_panel_cede_y_la_consola_conserva_su_suelo() {
        let estrecha = (CONSOLA_MINIMA + 4.0) * ANCHO_DE_CELDA;

        let hueco = apartar(
            division_de(estrecha),
            ANCHO_DE_CELDA,
            Some(ANCHO_DE_FABRICA),
        );

        assert_eq!(
            hueco.panel.expect("hay panel").width(),
            4.0 * ANCHO_DE_CELDA,
            "el panel es el que cede"
        );
        assert_eq!(hueco.consola.width(), CONSOLA_MINIMA * ANCHO_DE_CELDA);
    }

    /// Y el borde del anterior: por debajo del suelo de la consola el panel desaparece del todo antes
    /// que devolver un rectángulo invertido, que es lo que haría una resta sin suelo.
    #[test]
    fn en_una_division_mas_estrecha_que_el_suelo_el_panel_se_queda_sin_ancho() {
        let hueco = apartar(division_de(16.0), ANCHO_DE_CELDA, Some(ANCHO_DE_FABRICA));
        let panel = hueco.panel.expect("el rectángulo existe, aunque vacío");

        assert_eq!(panel.width(), 0.0);
        assert_eq!(hueco.consola.width(), 16.0);
        assert!(hueco.consola.left() <= hueco.consola.right() && panel.left() <= panel.right());
    }

    /// **El ancho son treinta y seis columnas, no doscientos ochenta y ocho puntos**, y con una sola
    /// medida de celda las dos cosas dan el mismo número: la primera prueba de este módulo pasa igual
    /// con un ancho fijo en puntos. La diferencia solo se ve **entre fotogramas de aumentos distintos**
    /// —la fuente cambia de tamaño y la métrica se vuelve a preguntar en cada pasada—, y es justo lo
    /// que la decisión de contarlo en celdas existe para conservar: cuántos caracteres caben.
    #[test]
    fn el_ancho_del_panel_se_cuenta_en_celdas_y_no_en_puntos() {
        // El doble de aumento, que es el salto que hace un escritorio al pasar a una pantalla densa.
        for celda in [ANCHO_DE_CELDA, ANCHO_DE_CELDA * 2.0] {
            let hueco = apartar(division_de(1600.0), celda, Some(ANCHO_DE_FABRICA));

            assert_eq!(
                hueco.panel.expect("hay panel").width() / celda,
                ANCHO_DE_FABRICA as f32,
                "con celdas de {celda} puntos el panel dejó de medir sus columnas"
            );
        }
    }

    /// Sin nada que pintar no se descuenta ancho y no hay segundo
    /// rectángulo: la división se ve exactamente como antes de que existiera el panel.
    #[test]
    fn sin_panel_que_pintar_la_consola_se_queda_con_todo() {
        let entera = division_de(800.0);

        let hueco = apartar(entera, ANCHO_DE_CELDA, None);

        assert_eq!(hueco.consola, entera);
        assert!(hueco.panel.is_none());
    }

    /// El sitio que se le da al panel en las pruebas del pintado: de sobra de alto para que quepa
    /// todo, así que lo que no salga es porque no se pintó y no porque no cupiera.
    const SITIO: Rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 600.0));

    /// Pinta un panel de verdad en el sitio dado y devuelve **todo lo que quedó encolado**.
    ///
    /// Devuelve las formas enteras y no solo su texto porque dos propiedades del pintado no son
    /// texto: **el fondo**, que es lo único que separa el panel de su consola, y **dónde cae** cada
    /// fila.
    fn lo_que_se_encola(fichero: &str, sitio: Rect) -> Vec<eframe::egui::Shape> {
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(&dir, fichero);
        let mut estado = Estado::default();
        estado.refrescar(dir.path());
        assert!(
            estado.a_pintar().is_some(),
            "el fichero de la prueba no se leyó"
        );

        pintando(&mut estado, sitio, SystemTime::now())
    }

    /// Pinta el estado que se le dé, en el momento que se le diga.
    ///
    /// Separado de [`lo_que_se_encola`] porque la confesión depende de **cuándo** se mire: el mismo
    /// panel se pinta distinto si ha pasado su plazo.
    fn pintando(estado: &mut Estado, sitio: Rect, ahora: SystemTime) -> Vec<eframe::egui::Shape> {
        let ctx = eframe::egui::Context::default();
        let mut salida = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrica = Metrica::medir(ui.ctx(), eframe::egui::FontId::monospace(crate::CUERPO));
            pintar(
                &ui.painter_at(sitio),
                estado,
                "a1b2",
                &Tema::default(),
                &metrica,
                ahora,
            );
        });

        // Las formas se sacan **antes** de soltar la salida: una pasada de interfaz que se suelta sin
        // aplicar sus texturas entra en pánico, y aquí no hay pantalla donde aplicarlas.
        let formas = std::mem::take(&mut salida.shapes);
        salida.drop_without_applying_deltas();

        formas
            .into_iter()
            .map(|recortada| recortada.shape)
            .collect()
    }

    /// Un estado con un panel publicado hace lo que se diga.
    ///
    /// La fecha se mueve a mano y no se espera al reloj, por lo mismo que en [`envejecer`]: una prueba
    /// que dependa de cuánto tarda en correr da un resultado distinto en cada máquina.
    fn publicado_hace(fichero: &str, cuanto: Duration) -> Estado {
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(&dir, fichero);
        std::fs::File::open(dir.path().join(FICHERO_DEL_PANEL))
            .expect("abrir")
            .set_modified(SystemTime::now() - cuanto)
            .expect("mover la fecha");

        let mut estado = Estado::default();
        estado.refrescar(dir.path());
        estado
    }

    /// El texto de lo que se pintó, fila a fila.
    fn textos(formas: Vec<eframe::egui::Shape>) -> Vec<String> {
        formas
            .into_iter()
            .filter_map(|forma| match forma {
                eframe::egui::Shape::Text(texto) => Some(texto.galley.text().to_owned()),
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
        let mut estado = publicado_hace(
            r#"{"stale_after_minutes":5,"lines":["lo que decía"]}"#,
            Duration::from_secs(45 * 60 + 30),
        );

        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));

        assert!(
            pintado.iter().any(|fila| fila.contains("sin noticias")),
            "el panel lleva 45 minutos sin tocarse y se enseña como si fuera de ahora: {pintado:?}"
        );
        assert!(
            pintado.iter().any(|fila| fila.contains("45 min")),
            "el aviso no dice cuánto lleva callado: {pintado:?}"
        );
        assert!(
            pintado.iter().any(|fila| fila.contains("lo que decía")),
            "se perdió lo que el productor había publicado: {pintado:?}"
        );

        // **Y va arriba, no al final.** Abajo es donde el recorte se come lo que no cabe: en un panel
        // largo, el aviso de que no es de fiar sería lo primero en desaparecer.
        let donde = |texto: &str| pintado.iter().position(|fila| fila.contains(texto));
        assert!(
            donde("sin noticias") < donde("lo que decía"),
            "el aviso se pinta detrás del cuerpo del productor: {pintado:?}"
        );
    }

    /// Y la otra mitad: **un panel al día no dice nada**. Sin esto, un aviso puesto siempre pasaría por
    /// bueno y el panel tendría una línea de ruido permanente.
    #[test]
    fn un_panel_recien_publicado_no_confiesa_nada() {
        let mut estado = publicado_hace(
            r#"{"stale_after_minutes":5,"lines":["recién puesto"]}"#,
            Duration::from_secs(10),
        );

        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));

        assert!(
            !pintado.iter().any(|fila| fila.contains("sin noticias")),
            "un panel de hace diez segundos se anuncia como viejo: {pintado:?}"
        );
    }

    /// **Conservar el último panel bueno sin decirlo esconde a un productor roto** detrás de un resumen
    /// plausible de hace una hora. Esta es la mitad de AC7 que faltaba: la ventana sigue en pie, se
    /// conserva lo último bueno, **y el panel dice que eso no es lo último**.
    #[test]
    fn una_publicacion_que_no_se_entiende_se_confiesa_sin_perder_lo_bueno() {
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(&dir, r#"{"lines":["lo bueno de antes"]}"#);
        let mut estado = Estado::default();
        estado.refrescar(dir.path());

        publicar(&dir, "esto no es json");
        envejecer(&dir, 1);
        estado.refrescar(dir.path());

        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));
        assert!(
            pintado
                .iter()
                .any(|fila| fila.contains("no es lo último") && fila.contains("no se entiende")),
            "se sigue enseñando lo viejo como si fuera lo último: {pintado:?}"
        );
        assert!(
            pintado
                .iter()
                .any(|fila| fila.contains("lo bueno de antes")),
            "confesar no puede costar lo último que sí se entendió: {pintado:?}"
        );
    }

    /// Y cuando **nunca** hubo nada bueno, el panel es solo la confesión. Sin esto, una consola cuyo
    /// productor publica basura desde el principio se ve igual que una que no tiene productor: como una
    /// consola pelada, sin una palabra que explique por qué no hay panel.
    #[test]
    fn una_consola_sin_nada_bueno_ensena_el_aviso_a_solas() {
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(&dir, "ni esto tampoco");
        let mut estado = Estado::default();
        estado.refrescar(dir.path());

        assert!(
            estado.se_ve(SystemTime::now()),
            "hay un productor publicando algo ilegible y la división se ve como si no lo hubiera"
        );
        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));
        assert!(
            pintado
                .iter()
                .any(|fila| fila.contains("sin panel") && fila.contains("no se entiende")),
            "no se dice por qué no hay panel: {pintado:?}"
        );
    }

    /// **Un fichero que se pasa del techo no se pinta, pero tampoco se calla.** El techo es lo que
    /// impide que un `panel.json` ajeno cueste un fotograma entero en interpretarse; que además dejara
    /// la consola indistinguible de una sin productor sería el fallo que este hito viene a evitar,
    /// solo que provocado por la propia defensa.
    #[test]
    fn un_panel_que_pasa_del_techo_no_se_pinta_pero_se_dice_por_que() {
        let dir = PanelDir::create().expect("crear el directorio");
        let relleno = "x".repeat(panel_contract::MAX_BYTES as usize);
        publicar(&dir, &format!(r#"{{"lines":["{relleno}"]}}"#));
        let mut estado = Estado::default();
        estado.refrescar(dir.path());

        assert!(
            estado.se_ve(SystemTime::now()),
            "hay un productor publicando algo enorme y la división se ve como si no lo hubiera"
        );
        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));
        assert!(
            pintado
                .iter()
                .any(|fila| fila.contains("sin panel") && fila.contains("KiB")),
            "no se dice por qué no hay panel: {pintado:?}"
        );
        assert!(
            !pintado.iter().any(|fila| fila.contains("xxxx")),
            "se pintó contenido de un fichero que pasa del techo: {pintado:?}"
        );
    }

    /// **La raya que sostiene la promesa del hito**: una consola cuyo productor no ha publicado todavía
    /// se ve exactamente como antes de que el panel existiera. Que el fichero no esté es el estado
    /// normal de arranque, no una avería que anunciar.
    #[test]
    fn una_consola_sin_fichero_no_ensena_nada_ni_avisa_de_nada() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut estado = Estado::default();
        estado.refrescar(dir.path());

        assert!(
            !estado.se_ve(SystemTime::now()),
            "una consola sin productor se estrecha para enseñar un panel que no existe"
        );
        assert!(estado.aviso(SystemTime::now()).is_none());
    }

    /// Pero si **había** panel y su fichero desaparece, eso sí se dice: el productor lo borró o alguien
    /// limpió por debajo, y lo que se sigue viendo ya no tiene respaldo en disco.
    #[test]
    fn que_el_fichero_desaparezca_despues_si_se_confiesa() {
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(&dir, r#"{"lines":["estuve aquí"]}"#);
        let mut estado = Estado::default();
        estado.refrescar(dir.path());

        std::fs::remove_file(dir.path().join(FICHERO_DEL_PANEL)).expect("borrar el panel");
        estado.refrescar(dir.path());

        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));
        assert!(
            pintado.iter().any(|fila| fila.contains("ya no está")),
            "el fichero se borró y el panel sigue como si nada: {pintado:?}"
        );
    }

    /// **`header: false` apaga la línea de identidad, no la honestidad de la aplicación.** Si la
    /// apagara, a un productor le bastaría con ponerlo para que su panel roto no lo pareciera.
    #[test]
    fn el_aviso_sale_aunque_el_productor_apague_la_cabecera() {
        let mut estado = publicado_hace(
            r#"{"header":false,"stale_after_minutes":1,"lines":["sin cabecera"]}"#,
            Duration::from_secs(60 * 60),
        );

        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));

        assert!(
            pintado.iter().any(|fila| fila.contains("sin noticias")),
            "apagar la cabecera escondió el aviso de la app: {pintado:?}"
        );
        assert!(
            !pintado.iter().any(|fila| fila.contains("a1b2")),
            "`header: false` tiene que seguir escondiendo la identidad: {pintado:?}"
        );
    }

    /// **El aviso entra en la clave de la maqueta guardada.** Un panel que envejece cambia lo que hay
    /// que pintar sin que su fichero se toque, así que una caché que solo mirase el ancho seguiría
    /// enseñando la maqueta de cuando era fresco y el aviso no aparecería nunca.
    #[test]
    fn envejecer_tira_la_maqueta_aunque_el_fichero_no_cambie() {
        let mut estado = publicado_hace(
            r#"{"stale_after_minutes":10,"lines":["lo mismo de siempre"]}"#,
            Duration::from_secs(60),
        );

        let fresco = textos(pintando(&mut estado, SITIO, SystemTime::now()));
        let viejo = textos(pintando(
            &mut estado,
            SITIO,
            SystemTime::now() + Duration::from_secs(30 * 60),
        ));

        assert!(!fresco.iter().any(|fila| fila.contains("sin noticias")));
        assert!(
            viejo.iter().any(|fila| fila.contains("sin noticias")),
            "el panel envejeció y se siguió pintando la maqueta de cuando era fresco: {viejo:?}"
        );
    }

    /// **Lo que el latido necesita saber**: cuánto queda para que toque avisar. Sin esto la ventana no
    /// sabría cuándo despertarse, y con la consola callada el aviso no llegaría nunca.
    #[test]
    fn se_sabe_cuanto_falta_para_que_toque_avisar() {
        let estado = publicado_hace(
            r#"{"stale_after_minutes":10,"lines":["algo"]}"#,
            Duration::from_secs(4 * 60),
        );

        let falta = estado
            .cuanto_falta_para_avisar(SystemTime::now())
            .expect("todavía no es viejo, así que hay algo que esperar");

        // Seis minutos, con holgura para lo que tarde la propia prueba.
        assert!(
            falta.as_secs() > 5 * 60 && falta.as_secs() <= 6 * 60,
            "faltan {} s para avisar de un panel de hace 4 min con plazo de 10",
            falta.as_secs()
        );
    }

    /// **Y un panel ya viejo sigue pidiendo despertador, porque su aviso lleva un número que avanza.**
    /// Sin esto, el agente que muere a las 10:00 con un plazo de cinco minutos deja en pantalla «sin
    /// noticias desde hace 5 min», y a las 13:00 sigue diciendo cinco: acierta en lo cualitativo y
    /// miente en el dato que sirve para decidir si está muerto o solo lento.
    #[test]
    fn un_panel_ya_viejo_vuelve_a_despertar_cuando_su_numero_cambie() {
        let estado = publicado_hace(
            r#"{"stale_after_minutes":1,"lines":["algo"]}"#,
            Duration::from_secs(45 * 60 + 40),
        );

        let falta = estado
            .cuanto_falta_para_avisar(SystemTime::now())
            .expect("el número del aviso va a cambiar, así que hay a qué volver");

        // Al siguiente minuto, no dentro de un minuto: lo que importa es cuándo salta el número.
        // Veinte segundos de margen entre mover la fecha y preguntar: el más estrecho de la suite
        // era de diez, y es el que caería primero en una máquina saturada.
        assert!(
            falta.as_secs() > 10 && falta.as_secs() <= 20,
            "vuelve en {} s, y al número le faltan 20 para saltar",
            falta.as_secs()
        );
    }

    /// Cuando se dicen horas, se vuelve a la siguiente hora y no cada minuto: el número solo cambia
    /// cuando cambia, y cada cambio cuesta despertar a la ventana.
    #[test]
    fn con_horas_de_silencio_se_despierta_una_vez_por_hora() {
        let estado = publicado_hace(
            r#"{"stale_after_minutes":1,"lines":["algo"]}"#,
            Duration::from_secs(5 * 60 * 60 + 10 * 60),
        );

        let falta = estado
            .cuanto_falta_para_avisar(SystemTime::now())
            .expect("el número sigue avanzando");

        assert!(
            falta.as_secs() > 45 * 60,
            "con cinco horas de silencio se despierta a los {} s, que es una vez por minuto",
            falta.as_secs()
        );
    }

    /// Sin panel no hay nada que esperar.
    #[test]
    fn sin_panel_no_se_pide_despertador() {
        assert!(
            Estado::default()
                .cuanto_falta_para_avisar(SystemTime::now())
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
        publicar(&dir, r#"{"lines":["lo último de verdad"]}"#);
        let mut estado = Estado::default();
        estado.refrescar(dir.path());

        // El fichero se vuelve ilegible un instante y luego se recupera, sin cambiar de fecha.
        let ruta = dir.path().join(FICHERO_DEL_PANEL);
        let fecha = std::fs::metadata(&ruta)
            .expect("mirar la fecha")
            .modified()
            .expect("hay fecha");
        std::fs::write(&ruta, "roto").expect("romperlo");
        std::fs::File::open(&ruta)
            .expect("abrir")
            .set_modified(fecha + Duration::from_secs(1))
            .expect("mover la fecha");
        estado.refrescar(dir.path());
        publicar(&dir, r#"{"lines":["lo último de verdad"]}"#);
        std::fs::File::open(&ruta)
            .expect("abrir")
            .set_modified(fecha)
            .expect("devolver la fecha");

        estado.refrescar(dir.path());

        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));
        assert!(
            !pintado.iter().any(|fila| fila.contains("no es lo último")),
            "el fichero se lee bien y su panel sigue acusado: {pintado:?}"
        );
    }

    /// Con un plazo por debajo del minuto —que el contrato permite— el aviso **no dice un número**: «sin
    /// noticias desde hace 0 min» un segundo después de publicar se lee como un fallo de la aplicación.
    #[test]
    fn con_un_plazo_de_cero_el_aviso_no_dice_cero_minutos() {
        let mut estado = publicado_hace(
            r#"{"stale_after_minutes":0,"lines":["algo"]}"#,
            Duration::from_secs(5),
        );

        let pintado = textos(pintando(&mut estado, SITIO, SystemTime::now()));

        assert!(
            pintado.iter().any(|fila| fila.contains("sin noticias")),
            "con plazo cero el panel es viejo desde el primer segundo: {pintado:?}"
        );
        assert!(
            !pintado.iter().any(|fila| fila.contains("0 min")),
            "el aviso dice «hace 0 min», que se lee como un fallo: {pintado:?}"
        );
    }

    /// Lo que mide una celda con la fuente con la que pinta el panel.
    ///
    /// Hace falta para afirmar sobre posiciones: el margen y el hueco de la identidad se cuentan en
    /// celdas, así que compararlos con un número escrito a mano sería comprobar la fuente de esta
    /// máquina en vez de la aritmética del panel.
    fn celda() -> (f32, f32) {
        let ctx = eframe::egui::Context::default();
        let mut medida = (0.0, 0.0);
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrica = Metrica::medir(ui.ctx(), eframe::egui::FontId::monospace(crate::CUERPO));
            medida = (metrica.ancho(), metrica.alto());
        })
        .drop_without_applying_deltas();
        medida
    }

    /// Pinta un panel de verdad y devuelve **lo que quedó encolado**: el texto de cada fila y su color.
    ///
    /// Sin esto, todo lo que ocurre dentro de `pintar` es invisible para la suite: se puede dejar de
    /// leer el estilo, pintarlo todo del mismo color o **no pintar nada**, y las pruebas de
    /// `Tema::color_del_panel` siguen verdes porque afirman sobre el ayudante, no sobre el camino.
    fn lo_que_se_pinta(fichero: &str) -> Vec<(String, Color32)> {
        lo_que_se_encola(fichero, SITIO)
            .into_iter()
            .filter_map(|forma| match forma {
                eframe::egui::Shape::Text(texto) => {
                    Some((texto.galley.text().to_owned(), texto.fallback_color))
                }
                _ => None,
            })
            .collect()
    }

    /// Dónde cae cada fila de texto, en el orden en que se encolaron.
    fn donde_cae_cada_fila(fichero: &str, sitio: Rect) -> Vec<(Pos2, String)> {
        lo_que_se_encola(fichero, sitio)
            .into_iter()
            .filter_map(|forma| match forma {
                eframe::egui::Shape::Text(texto) => {
                    Some((texto.pos, texto.galley.text().to_owned()))
                }
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
        let (_, alto) = celda();

        let filas = donde_cae_cada_fila(
            r#"{"header":false,"lines":["la primera","","la tercera"]}"#,
            SITIO,
        );

        assert_eq!(filas.len(), 2, "la vacía no encola forma, las otras sí");
        assert_eq!(filas[0].0.y, SITIO.top(), "la primera arranca arriba");
        assert_eq!(
            filas[1].0.y - filas[0].0.y,
            alto * 2.0,
            "entre la primera y la tercera tienen que caber dos renglones: el hueco es uno de ellos"
        );
    }

    #[test]
    fn cada_linea_se_pinta_en_su_orden_y_las_vacias_dejan_su_hueco() {
        let filas = lo_que_se_pinta(r#"{"header":false,"lines":["la primera","","la tercera"]}"#);

        let textos: Vec<&str> = filas.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            textos,
            ["  la primera", "  la tercera"],
            "el hueco no se pinta, pero las dos que hay van en orden"
        );
    }

    /// **El color sale del camino real, no del ayudante.** Con esta prueba, ignorar `linea.style` en
    /// `pintar` cae en rojo; sin ella, la suite seguía verde con el panel entero monocromo.
    #[test]
    fn el_color_de_cada_linea_sale_de_su_estilo_y_de_su_marca() {
        let tema = Tema::default();
        let filas = lo_que_se_pinta(
            r#"{"header":false,"lines":[
                {"text":"un fallo","style":"error"},
                {"text":"bloqueada","mark":"block"},
                {"text":"y su color es el dato","mark":"info","style":"ok"},
                {"text":"sin adjetivos"}
            ]}"#,
        );

        assert_eq!(
            filas,
            vec![
                (
                    "  un fallo".to_owned(),
                    tema.color_del_panel(None, Some(panel_contract::Style::Error))
                ),
                (
                    "× bloqueada".to_owned(),
                    tema.color_del_panel(Some(panel_contract::Mark::Block), None)
                ),
                (
                    "› y su color es el dato".to_owned(),
                    tema.color_del_panel(
                        Some(panel_contract::Mark::Info),
                        Some(panel_contract::Style::Ok)
                    )
                ),
                (
                    "  sin adjetivos".to_owned(),
                    tema.color_del_panel(None, None)
                ),
            ]
        );
    }

    /// La línea de identidad la escribe la aplicación con lo que ya sabe: el identificador de la
    /// división y la fase que el productor publica. Nada de un fichero vecino.
    #[test]
    fn la_cabecera_lleva_la_identidad_de_la_division_y_su_fase() {
        let filas = lo_que_se_pinta(r#"{"phase":"build","lines":["el cuerpo"]}"#);

        let textos: Vec<&str> = filas.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(textos, ["a1b2 · build", "  el cuerpo"]);
    }

    #[test]
    fn con_la_cabecera_quitada_no_se_pinta_la_identidad() {
        let filas = lo_que_se_pinta(r#"{"header":false,"phase":"build","lines":["el cuerpo"]}"#);

        let textos: Vec<&str> = filas.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(textos, ["  el cuerpo"], "la fase va dentro de la identidad");
    }

    /// **El saneo, comprobado sobre lo que llega al pintor y no sobre la función suelta.**
    #[test]
    fn lo_que_llega_al_pintor_no_lleva_saltos_de_linea() {
        let filas = lo_que_se_pinta(r#"{"header":false,"lines":["arriba\nabajo"]}"#);

        assert_eq!(filas[0].0, "  arriba abajo");
    }

    /// **El coste de pintar tiene su propio peor caso, y es el inverso del de interpretar.** Una sola
    /// línea larguísima es barata de leer y costaba 14,6 ms de teselado **en cada fotograma**, porque
    /// epaint descarta texto por fila y no por glifo. Sin este tope, un `panel.json` ajeno deja la
    /// ventana por encima de su presupuesto para siempre.
    #[test]
    fn una_linea_absurdamente_larga_se_corta_antes_de_pintarse() {
        let largo = "A".repeat(60_000);

        let filas = lo_que_se_pinta(&format!(r#"{{"header":false,"lines":["{largo}"]}}"#));

        // **Se cuenta el total, no la primera fila.** Con la envoltura puesta, el tope no se ve en una
        // fila suelta: la línea se reparte en varias, y lo que hay que acotar es cuánto texto se
        // maqueta en total. Contar solo la primera dejaría pasar mil filas de cuarenta caracteres.
        let pintado: usize = filas.iter().map(|(t, _)| t.chars().count()).sum();
        // Los dos de margen son la columna del símbolo, que se reserva aunque la línea no traiga
        // marca: es lo que mantiene el texto de todas alineado.
        assert!(
            pintado <= MAX_CARACTERES + 2,
            "se maquetaron {pintado} caracteres de una línea de 60.000"
        );
        assert!(filas.len() > 1, "y se envolvió en varias filas");
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
        let formas = lo_que_se_encola(r#"{"header":false,"lines":[]}"#, SITIO);

        let fondos: Vec<_> = formas
            .iter()
            .filter_map(|forma| match forma {
                eframe::egui::Shape::Rect(rect) => Some((rect.rect, rect.fill)),
                _ => None,
            })
            .collect();
        assert_eq!(
            fondos,
            vec![(SITIO, Tema::default().fondo_del_panel())],
            "el panel tiene que cubrir su rectángulo con su propio fondo"
        );
        assert!(
            !formas
                .iter()
                .any(|forma| matches!(forma, eframe::egui::Shape::Text(_))),
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
        let sin_alto = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 0.0));
        let del_reves = Rect::from_min_max(Pos2::new(0.0, 100.0), Pos2::new(400.0, 0.0));

        for sitio in [sin_alto, del_reves] {
            let filas = donde_cae_cada_fila(r#"{"header":false,"lines":["una","otra"]}"#, sitio);

            assert!(
                filas.is_empty(),
                "en {sitio:?} no cabe ninguna fila y se encolaron {filas:?}"
            );
        }
    }

    /// **La línea de identidad lleva dentro una cadena del productor**: la fase. El saneo de las líneas
    /// no la alcanza —esta cadena la compone la aplicación—, así que un `\n` en la fase pintaría dos
    /// filas donde va una y taparía la primera del cuerpo.
    #[test]
    fn la_fase_pasa_por_el_mismo_saneo_que_las_lineas() {
        let filas = lo_que_se_pinta(r#"{"phase":"build\nfalsa","lines":["el cuerpo"]}"#);

        let textos: Vec<&str> = filas.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(textos, ["a1b2 · build falsa", "  el cuerpo"]);
    }

    /// Y el tope también: una fase larguísima es el mismo peor caso de teselado que una línea larguísima
    /// —14,6 ms por fotograma mientras siga publicada— y llega por una cadena que el tope de las líneas
    /// no toca.
    ///
    /// De paso, el borde que el contrato declara: con `lines` vacío el panel enseña **solo** su línea de
    /// identidad.
    #[test]
    fn una_fase_absurdamente_larga_no_se_pinta_entera() {
        let larga = "F".repeat(60_000);

        let filas = lo_que_se_pinta(&format!(r#"{{"phase":"{larga}","lines":[]}}"#));

        let pintado: usize = filas.iter().map(|(t, _)| t.chars().count()).sum();
        assert!(
            pintado <= MAX_CARACTERES + "a1b2 · ".len(),
            "se maquetaron {pintado} caracteres de una fase de 60.000"
        );
        assert!(
            filas
                .iter()
                .all(|(t, _)| t.starts_with('F') || t.starts_with("a1b2")),
            "sin líneas, lo único que hay es la identidad envuelta: {filas:?}"
        );
    }

    /// Dos decisiones del pintado que **no se ven ni en el texto ni en el color**, y que hasta aquí no
    /// ataba nada: el texto no va pegado al borde izquierdo —pegado se lee como un error de pintado— y
    /// la identidad deja una fila en blanco antes del cuerpo. Lo segundo importa porque la identidad la
    /// escribe la aplicación y el cuerpo lo escribe el productor: pegadas, la suya se lee como la
    /// primera línea de él.
    #[test]
    fn el_texto_deja_su_margen_y_la_identidad_su_fila_en_blanco() {
        let (ancho, alto) = celda();

        let filas = donde_cae_cada_fila(r#"{"phase":"build","lines":["el cuerpo"]}"#, SITIO);

        let [(identidad, _), (cuerpo, _)] = filas.as_slice() else {
            panic!("tenían que ser dos filas y son {filas:?}");
        };
        // La celda de margen va escrita aquí y no como `MARGEN`: una expectativa que sale de la misma
        // constante que describe se cumple sola, la pongan en el valor que la pongan.
        assert_eq!(
            identidad.x,
            SITIO.left() + ancho,
            "el texto tiene que arrancar a una celda del borde"
        );
        assert_eq!(cuerpo.x, identidad.x, "el cuerpo no arranca donde ella");
        assert_eq!(
            cuerpo.y - identidad.y,
            2.0 * alto,
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
        let marcas = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ];

        let ctx = eframe::egui::Context::default();
        let mut anchos = Vec::new();
        let mut celda = 0.0;
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let fuente = eframe::egui::FontId::monospace(crate::CUERPO);
            ui.ctx().fonts_mut(|fuentes| {
                celda = fuentes.glyph_width(&fuente, 'M');
                anchos = marcas
                    .iter()
                    .map(|m| (simbolo(*m), fuentes.glyph_width(&fuente, simbolo(*m))))
                    .collect();
            });
        })
        .drop_without_applying_deltas();

        for (simbolo, ancho) in anchos {
            assert!(
                (ancho - celda).abs() < 0.01,
                "«{simbolo}» mide {ancho} y una celda mide {celda}: {}",
                if ancho == 0.0 {
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
                simbolo(Mark::Done),
                simbolo(Mark::Current),
                simbolo(Mark::Pending),
                simbolo(Mark::Wait),
                simbolo(Mark::Block),
                simbolo(Mark::Info),
            ],
            ['●', '▶', '·', '…', '×', '›']
        );
    }

    #[test]
    fn ninguna_marca_comparte_simbolo_con_otra() {
        let todos: std::collections::HashSet<char> = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ]
        .iter()
        .map(|m| simbolo(*m))
        .collect();

        assert_eq!(todos.len(), 6);
    }

    /// **La regla del contrato que hace legible un panel: la columna se calcula por bloque.**
    ///
    /// Con un ancho global, el bloque de arriba —cuya referencia es larga— empujaría el texto del de
    /// abajo, y lo que se gana alineando se perdería en margen desperdiciado. Los bloques los separan
    /// las líneas vacías.
    #[test]
    fn la_columna_de_referencias_se_alinea_por_bloques_y_un_bloque_no_ensancha_al_otro() {
        let filas = lo_que_se_pinta(
            r##"{"header":false,"lines":[
                {"ref":"#1","text":"corta"},
                {"ref":"#10-#25","text":"larga"},
                "",
                {"ref":"#2","text":"otro bloque"}
            ]}"##,
        );

        let textos: Vec<&str> = filas.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            textos,
            ["  #1      corta", "  #10-#25 larga", "  #2 otro bloque"],
            "el segundo bloque tiene que volver a su propio ancho"
        );
    }

    /// La continuación se sangra **hasta donde empieza el texto**, no hasta el borde: alineada al borde,
    /// la segunda fila se leería como una línea nueva del productor y la columna dejaría de significar
    /// nada.
    #[test]
    fn una_linea_larga_se_envuelve_con_la_continuacion_bajo_su_texto() {
        let filas = donde_cae_cada_fila(
            r##"{"header":false,"lines":[{"ref":"#1","mark":"done","text":"palabras suficientes para no caber de ninguna manera en el panel"}]}"##,
            SITIO,
        );

        assert!(filas.len() > 1, "tenía que envolverse: {filas:?}");
        let (primera, resto) = filas.split_first().expect("hay filas");

        // **La propiedad es la igualdad, no «más a la derecha».** Con solo `>`, cualquier sangría
        // constante distinta de cero pasa —incluida una celda, o sea debajo del símbolo, que es
        // literalmente lo que el criterio prohíbe—. Lo que hay que afirmar es que la continuación cae
        // donde empieza el **texto** de la primera fila, y ese sitio se calcula de lo que ya se tiene:
        // la x de la primera más lo que ocupa su cabeza.
        let (ancho_celda, _) = celda();
        // En caracteres, no en bytes: el símbolo de la marca ocupa tres bytes y una sola celda.
        let cabeza = primera.1.chars().count()
            - primera
                .1
                .split_once("palabras")
                .expect("la primera fila lleva su cabeza y luego el texto")
                .1
                .chars()
                .count()
            - "palabras".chars().count();
        let bajo_el_texto = primera.0.x + cabeza as f32 * ancho_celda;

        for (donde, texto) in resto {
            assert!(
                (donde.x - bajo_el_texto).abs() < 0.01,
                "«{texto}» arrancó en {} y el texto de la primera empieza en {bajo_el_texto}",
                donde.x
            );
        }
    }

    /// Una palabra sola más larga que el hueco se parte por caracteres. Sin esto la envoltura no
    /// progresa: la palabra no cabe nunca, y saltar de fila no la hace caber.
    #[test]
    fn una_palabra_mas_larga_que_el_hueco_se_parte_en_vez_de_colgar() {
        let filas = lo_que_se_pinta(&format!(
            r#"{{"header":false,"lines":["{}"]}}"#,
            "x".repeat(200)
        ));

        assert!(filas.len() > 1);
        assert!(
            filas.iter().all(|(t, _)| !t.is_empty()),
            "ninguna fila vacía"
        );
    }

    /// **Con la sangría comida entera, la envoltura tiene que seguir avanzando.** Sin suelo, el hueco
    /// sale cero y el bucle no mete ningún carácter por vuelta.
    ///
    /// **Su modo de fallo es colgarse, no ponerse en rojo**, y eso hay que saberlo antes de verlo: con
    /// `HUECO_MINIMO` a cero esta prueba no falla, se queda girando y la suite entera muere por
    /// memoria. Quien la vea colgada está viendo el fallo, no una prueba rota. Es el mismo modo que la
    /// de la tubería con nombre en el crate del contrato.
    #[test]
    fn una_sangria_mayor_que_el_panel_no_cuelga_la_envoltura() {
        let filas = lo_que_se_pinta(
            r#"{"header":false,"lines":[{"indent":64,"text":"esto no cabe de ninguna manera"}]}"#,
        );

        assert!(!filas.is_empty(), "algo se tiene que pintar");
        assert!(filas.iter().all(|(t, _)| !t.is_empty()));
    }

    /// **Lo que no cabe de alto se dice, no se descarta en silencio.** Un panel que se recorta callado
    /// es un resumen que miente por omisión, que es justo lo que el contrato le prohíbe al productor.
    #[test]
    fn cuando_no_caben_todas_la_ultima_fila_dice_cuantas_faltan() {
        let (_, alto) = celda();
        let cuatro_filas =
            Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, alto * 4.0));

        let filas: Vec<String> = donde_cae_cada_fila(
            r#"{"header":false,"lines":["una","dos","tres","cuatro","cinco","seis"]}"#,
            cuatro_filas,
        )
        .into_iter()
        .map(|(_, texto)| texto)
        .collect();

        assert_eq!(filas.len(), 4, "cuatro renglones, cuatro filas");
        assert_eq!(
            filas.last().expect("hay filas"),
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
        publicar(&dir, r#"{"header":false,"lines":["algo que envolver"]}"#);
        let mut estado = Estado::default();
        estado.refrescar(dir.path());
        let tema = Tema::default();

        let primera = estado
            .maquetar_si_hace_falta("a1b2", 40, &tema, None)
            .expect("hay panel")
            .to_vec();
        let repetida = estado
            .maquetar_si_hace_falta("a1b2", 40, &tema, None)
            .expect("hay panel")
            .to_vec();
        assert_eq!(primera, repetida, "el mismo ancho da la misma maqueta");

        let estrecha = estado
            .maquetar_si_hace_falta("a1b2", 12, &tema, None)
            .expect("hay panel")
            .to_vec();
        assert_ne!(
            primera, estrecha,
            "con otro ancho hay que volver a envolver"
        );
    }

    /// Y se tira al publicar: una maqueta que sobrevive a su fichero enseña lo de antes para siempre.
    #[test]
    fn la_maqueta_se_tira_cuando_el_productor_publica() {
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(&dir, r#"{"header":false,"lines":["lo viejo"]}"#);
        let mut estado = Estado::default();
        estado.refrescar(dir.path());
        let tema = Tema::default();
        estado.maquetar_si_hace_falta("a1b2", 40, &tema, None);

        publicar(&dir, r#"{"header":false,"lines":["lo nuevo"]}"#);
        envejecer(&dir, 1);
        estado.refrescar(dir.path());

        let filas = estado
            .maquetar_si_hace_falta("a1b2", 40, &tema, None)
            .expect("hay panel");
        assert!(
            filas.iter().any(|fila| fila.texto.contains("lo nuevo")),
            "la maqueta se quedó con lo viejo: {filas:?}"
        );
    }

    /// La referencia es cadena ajena igual que el texto, y desde este PR se pinta: un `\n` dentro
    /// descuadraría la columna entera.
    #[test]
    fn la_referencia_pasa_por_el_mismo_saneo_que_el_texto() {
        let filas = lo_que_se_pinta(r#"{"header":false,"lines":[{"ref":"a\nb","text":"t"}]}"#);

        assert_eq!(filas[0].0, "  a b t");
    }

    /// **Nada de lo que venga de fuera puede colgar ni reventar la envoltura**, y este es el sitio con
    /// más aritmética de índices de todo el panel: hay un corte por bytes sobre un índice que sale de
    /// recorrer caracteres, y un bucle con dos salidas.
    #[test]
    fn la_envoltura_aguanta_cualquier_cosa_sin_panico_ni_cuelgue() {
        let muchas_enes = "ñ".repeat(300);
        let casos = [
            "",
            " ",
            "                    ",
            "a",
            muchas_enes.as_str(),
            "🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂",
            "e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}e\u{0301}",
            "palabra corta",
            "unapalabramuylargasinespaciosquenocabeenningunhueco",
            "मानक हिन्दी",
            "\u{200b}\u{200b}\u{200b}",
        ];

        for ancho in [0, 1, 2, 7, 8, 9, 40] {
            for caso in casos {
                let mut filas = Vec::new();
                envolver(&mut filas, 0, caso, Color32::WHITE, ancho);
                envolver(
                    &mut filas,
                    ancho.saturating_add(5),
                    caso,
                    Color32::WHITE,
                    ancho,
                );
                assert!(
                    filas.len() < 5_000,
                    "con ancho {ancho} y «{caso}» salieron {} filas: la envoltura no progresa",
                    filas.len()
                );
            }
        }
    }

    /// **Una sangría legal del contrato no puede pintar fuera del panel.** El contrato admite hasta 64
    /// celdas y el panel mide 36: sin acotarla contra el ancho, la continuación se pintaba a 42 celdas
    /// del borde —recortada, o sea invisible— y **`recortar` la contaba como fila visible**, así que se
    /// comía renglones y el «… N más» mentía. Una referencia larga llega al mismo sitio.
    #[test]
    fn una_sangria_legal_del_contrato_no_saca_el_texto_del_panel() {
        let ancho = 36;
        let fichero =
            r#"{"lines":[{"indent":40,"text":"una frase larga que no cabe de ninguna manera"}]}"#;

        let filas = maquetar(
            Some(&panel_con(fichero)),
            "a1b2",
            ancho,
            &Tema::default(),
            None,
        );

        for fila in &filas {
            assert!(
                fila.columna + fila.texto.chars().count() <= ancho,
                "«{}» empieza en {} y mide {}: se sale de un panel de {ancho}",
                fila.texto,
                fila.columna,
                fila.texto.chars().count()
            );
        }
    }

    /// Y por el otro camino: una referencia larguísima empuja la cabeza igual que la sangría.
    #[test]
    fn una_referencia_larguisima_tampoco_saca_el_texto_del_panel() {
        let ancho = 36;
        let larga = "R".repeat(50);
        let fichero = format!(r##"{{"lines":[{{"ref":"{larga}","text":"algo que decir"}}]}}"##);

        let filas = maquetar(
            Some(&panel_con(&fichero)),
            "a1b2",
            ancho,
            &Tema::default(),
            None,
        );

        for fila in &filas {
            assert!(
                fila.columna <= ancho,
                "una continuación arrancó en la columna {} de un panel de {ancho}",
                fila.columna
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
        let filas = maquetar(
            Some(&panel_con(
                r##"{"header":false,"lines":[
                    {"mark":"done","ref":"#1","text":"con texto"},
                    {"mark":"block","ref":"#2","text":""}
                ]}"##,
            )),
            "a1b2",
            36,
            &Tema::default(),
            None,
        );

        assert_eq!(
            filas[0].columna, filas[1].columna,
            "las dos empiezan en la misma columna o la columna no significa nada: {filas:?}"
        );
        assert_eq!(filas[0].columna, 0);
    }

    /// **La cabeza no pasa por la envoltura, así que hay que acotarla aparte.** Una referencia más
    /// larga que el panel —el contrato no le pone techo— sacaba la primera fila entera fuera del
    /// rectángulo, y con ella todas sus continuaciones: se pintaban encima de la consola.
    #[test]
    fn una_referencia_mas_larga_que_el_panel_no_desborda_la_primera_fila() {
        let ancho = 36;
        let larga = "R".repeat(48);

        let filas = maquetar(
            Some(&panel_con(&format!(
                r##"{{"header":false,"lines":[{{"ref":"{larga}","text":"un texto cualquiera"}}]}}"##
            ))),
            "a1b2",
            ancho,
            &Tema::default(),
            None,
        );

        for fila in &filas {
            assert!(
                fila.columna + fila.texto.chars().count() <= ancho,
                "«{}» empieza en {} y mide {}: se sale de un panel de {ancho}",
                fila.texto,
                fila.columna,
                fila.texto.chars().count()
            );
        }
    }

    /// **La sangría del contrato se respeta**, y nada lo ataba: ignorarla del todo dejaba la suite
    /// verde. Es un campo que el productor escribe para agrupar visualmente, así que perderlo en
    /// silencio le quita una herramienta sin decírselo.
    #[test]
    fn la_sangria_que_pide_el_productor_desplaza_su_linea() {
        let filas = maquetar(
            Some(&panel_con(
                r#"{"header":false,"lines":[{"text":"pegada"},{"indent":4,"text":"sangrada"}]}"#,
            )),
            "a1b2",
            36,
            &Tema::default(),
            None,
        );

        let pegada = filas[0].texto.find("pegada").expect("está");
        let sangrada = filas[1].texto.find("sangrada").expect("está");
        assert_eq!(
            sangrada - pegada,
            4,
            "la segunda tiene que arrancar cuatro celdas más a la derecha: {filas:?}"
        );
    }

    /// **Se corta por palabras, no por caracteres.** Cortando siempre por caracteres el panel sigue
    /// cabiendo y ninguna prueba caía, pero las palabras quedan partidas por la mitad y el panel deja
    /// de leerse de un vistazo, que es de lo que va este PR.
    #[test]
    fn la_envoltura_parte_por_palabras_mientras_pueda() {
        let filas = maquetar(
            Some(&panel_con(
                r#"{"header":false,"lines":["alfa beta gamma delta epsilon zeta"]}"#,
            )),
            "a1b2",
            16,
            &Tema::default(),
            None,
        );

        for fila in &filas {
            let texto = fila.texto.trim();
            assert!(
                texto.split_whitespace().all(|palabra| {
                    ["alfa", "beta", "gamma", "delta", "epsilon", "zeta"].contains(&palabra)
                }),
                "«{texto}» trae una palabra partida: {filas:?}"
            );
        }
    }

    /// **El titular del PR, atado.** La prueba de al lado compara dos maquetas iguales, y eso se cumple
    /// igual recalculando desde cero. Lo que hay que afirmar es que **no se recalcula**, y se ve en la
    /// identidad del préstamo: si se remaquetara, la dirección del vector cambiaría.
    #[test]
    fn la_maqueta_no_se_recalcula_cuando_ya_vale() {
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(&dir, r#"{"header":false,"lines":["algo que envolver"]}"#);
        let mut estado = Estado::default();
        estado.refrescar(dir.path());
        let tema = Tema::default();

        let primera = estado
            .maquetar_si_hace_falta("a1b2", 40, &tema, None)
            .expect("hay panel")
            .as_ptr();
        let segunda = estado
            .maquetar_si_hace_falta("a1b2", 40, &tema, None)
            .expect("hay panel")
            .as_ptr();

        assert_eq!(
            primera, segunda,
            "se volvió a maquetar sin que cambiara ni el fichero ni el ancho"
        );
    }

    /// El aviso de lo que falta va **debajo** de lo que sí cabe, no encima de la primera fila.
    #[test]
    fn el_aviso_de_lo_que_falta_va_al_final_y_con_su_margen() {
        let (ancho_celda, alto) = celda();
        let tres = Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, alto * 3.0));

        let filas = donde_cae_cada_fila(
            r#"{"header":false,"lines":["una","dos","tres","cuatro"]}"#,
            tres,
        );

        let (donde, texto) = filas.last().expect("hay filas");
        assert_eq!(texto, "… 2 más");
        assert!(
            donde.y > filas[0].0.y,
            "el aviso se pintó encima de la primera fila"
        );
        assert!(
            (donde.x - (tres.left() + ancho_celda)).abs() < 0.01,
            "el aviso arrancó en {} y el margen deja el texto en {}",
            donde.x,
            tres.left() + ancho_celda
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
        let filas = maquetar(
            Some(&panel_con(
                r##"{"header":false,"lines":[
                    {"mark":"done","ref":"#1","text":"arriba"},
                    {"mark":"block","ref":"#22222"},
                    {"mark":"info","ref":"#3","text":"abajo"}
                ]}"##,
            )),
            "a1b2",
            36,
            &Tema::default(),
            None,
        );

        let columna_de = |fila: &Fila, palabra: &str| {
            fila.texto.chars().count()
                - fila
                    .texto
                    .split_once(palabra)
                    .expect("lleva su texto")
                    .1
                    .chars()
                    .count()
                - palabra.chars().count()
        };
        assert_eq!(
            columna_de(&filas[0], "arriba"),
            columna_de(&filas[2], "abajo"),
            "las tres son un solo bloque: la de en medio no cierra nada, {filas:?}"
        );
        assert_eq!(
            columna_de(&filas[0], "arriba"),
            "● #22222 ".chars().count(),
            "y el ancho lo fija la referencia más larga de las tres"
        );
    }

    #[test]
    fn sin_fichero_no_hay_panel_que_pintar() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut estado = Estado::default();

        estado.refrescar(dir.path());

        assert!(estado.a_pintar().is_none());
    }

    #[test]
    fn lo_publicado_se_lee_y_queda_listo_para_pintar() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut estado = Estado::default();
        publicar(&dir, r#"{"phase":"build","lines":["una","otra"]}"#);

        estado.refrescar(dir.path());

        let panel = estado.a_pintar().expect("hay panel");
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
        let mut estado = Estado::default();
        publicar(&dir, r#"{"lines":["la primera"]}"#);
        estado.refrescar(dir.path());
        let fecha = estado
            .a_pintar()
            .and_then(|panel| panel.written_at)
            .expect("la lectura trae la fecha del fichero");

        publicar(&dir, r#"{"lines":["la segunda"]}"#);
        std::fs::File::open(dir.path().join(FICHERO_DEL_PANEL))
            .expect("abrir")
            .set_modified(fecha)
            .expect("devolver la fecha a donde estaba");
        estado.refrescar(dir.path());

        assert_eq!(
            estado.a_pintar().expect("sigue habiendo panel").lines[0].text,
            "la primera",
            "releyó un fichero cuya fecha no había cambiado"
        );
    }

    #[test]
    fn con_la_fecha_movida_si_se_relee() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut estado = Estado::default();
        publicar(&dir, r#"{"lines":["la primera"]}"#);
        estado.refrescar(dir.path());

        publicar(&dir, r#"{"lines":["la segunda"]}"#);
        envejecer(&dir, 1);
        estado.refrescar(dir.path());

        assert_eq!(
            estado.a_pintar().expect("hay panel").lines[0].text,
            "la segunda"
        );
    }

    /// El productor machaca el fichero entero, así que una lectura puede caer justo en medio. Sin
    /// conservar el último bueno, el panel parpadearía a vacío en **cada** publicación.
    #[test]
    fn una_escritura_a_medias_no_borra_el_ultimo_panel_bueno() {
        let dir = PanelDir::create().expect("crear el directorio");
        let mut estado = Estado::default();
        publicar(&dir, r#"{"lines":["lo bueno"]}"#);
        estado.refrescar(dir.path());

        publicar(&dir, r#"{"lines":[{"text":"a med"#);
        envejecer(&dir, 1);
        estado.refrescar(dir.path());

        assert_eq!(
            estado.a_pintar().expect("el último bueno sigue ahí").lines[0].text,
            "lo bueno"
        );
    }

    /// **Un salto de línea dentro de una cadena ajena pinta varias filas encima de las siguientes.**
    /// Está medido en la fuente de epaint, no supuesto: `break_on_newline` viene puesto por omisión.
    /// Sin sanear, un `panel.json` con un `\n` en su texto tapa el resto del panel.
    #[test]
    fn un_salto_de_linea_en_el_texto_no_puede_pintar_dos_filas() {
        let saneado = sanear("primera\nsegunda");

        assert_eq!(saneado, "primera segunda");
        assert!(!saneado.contains('\n'));
    }

    /// El tabulador es el otro caso: avanza el ancho de varias celdas mientras la medida de anchura lo
    /// cuenta como cero, así que la columna se descuadra sin que nada lo anuncie.
    #[test]
    fn los_tabuladores_y_el_retorno_tampoco_sobreviven() {
        assert_eq!(sanear("uno\tdos\r\ntres"), "uno dos  tres");
        assert_eq!(
            sanear("\u{1b}[31mrojo"),
            " [31mrojo",
            "y las secuencias de escape"
        );
    }

    /// Se sustituyen por un espacio y no se borran: dos palabras separadas por un salto de línea
    /// pegadas se leerían como una sola palabra que el productor nunca escribió.
    #[test]
    fn lo_saneado_no_pega_palabras_que_estaban_separadas() {
        assert_eq!(sanear("hola\nmundo"), "hola mundo");
    }

    #[test]
    fn el_texto_normal_pasa_intacto() {
        let tal_cual = "F1.6: el panel · 35.392 tokens — 18 %";
        assert_eq!(sanear(tal_cual), tal_cual);
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
    /// comprobación de que la cuenta del aviso cuadraba y se quitó al mutarla: `visibles + sobran` es
    /// igual al total con el recorte bueno **y con el roto**, porque las dos versiones reparten las mismas
    /// filas entre los dos números. El recorte lo atan sus propias pruebas; aquí lo que se demuestra es
    /// que el camino se recorre entero sin romperse.
    #[test]
    fn con_el_panel_al_minimo_se_maqueta_y_se_recorta_sin_panico() {
        let tema = Tema::default();
        let dir = PanelDir::create().expect("crear el directorio");
        publicar(
            &dir,
            r##"{"lines":[{"ref":"#12","text":"una linea larga que no cabe de ninguna manera","mark":"current","indent":8},{"text":"otra"},{"text":"y otra"}]}"##,
        );
        let mut estado = Estado::default();
        estado.refrescar(dir.path());

        for ancho in [0usize, 1, 2, 3] {
            let filas = estado
                .maquetar_si_hace_falta("sonda", ancho, &tema, None)
                .expect("hay panel que maquetar");
            assert!(
                !filas.is_empty(),
                "con un panel de {ancho} celdas no se maquetó ninguna fila"
            );
            recortar(filas, 1);
        }
    }
}
