//! Lo que dice `panel.json`, y cómo se lee sin que nada de lo que traiga tire la ventana.
//!
//! Los nombres de los tipos y de sus campos **son los del contrato**, en inglés como él: quien compare
//! este fichero con la especificación no tiene que traducir nada por el camino.

use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Lo que se lee de un fichero como mucho.
///
/// No es prudencia: acota **lo que un fichero de fuera puede hacer reservar al interpretarse**, y por
/// eso se aplica **dentro** de la lectura —ver [`read`]—, porque comprobarlo después ya habría
/// reservado.
///
/// **El multiplicador depende de la forma del fichero, no de su tamaño**, y por eso hay tres números y
/// no uno. Con 256 KiB justos, medido en `--release` con
/// `cargo run --release --example read -p panel-contract`:
///
/// | Forma | Pico | Factor | Interpretar |
/// |---|---|---|---|
/// | Líneas de un panel real | 1,8 MiB | 7× | 2,8 ms |
/// | Una sola línea gigante | 769 KiB | 3× | 0,1 ms |
/// | Objetos de una clave, 32.766 de ellos — el peor caso | **24 MiB** | **95×** | **22 ms** |
///
/// **Lo que el techo NO acota es el tiempo.** El caso normal no se acerca: los nueve `panel.json` que
/// hay en el repo cuestan unos **10 µs cada uno**, así que releer los cuatro de cuatro divisiones sería
/// la cuarta parte del 1 % de un fotograma. El que muerde es el patológico —22 ms, un fotograma y
/// medio—, y basta con que sea posible: quien lea esto desde un bucle de pintado **no puede releer en
/// cada fotograma**, tiene que releer solo cuando el fichero haya cambiado de verdad.
///
/// Y de paso acota las líneas y su largo, que es lo que hace innecesario ponerles techo aparte.
pub const MAX_BYTES: u64 = 256 * 1024;

/// Lo más que se sangra una línea.
///
/// Es el ancho del panel más ancho de la maqueta, en celdas: más que eso no cabría de todas formas, y
/// sin techo quien pinte repetiría tantos espacios como diga un fichero de fuera.
pub const MAX_INDENT: u64 = 64;

/// Cuánto silencio es legítimo antes de avisar, cuando el productor no lo dice.
pub const STALE_AFTER: Duration = Duration::from_secs(30 * 60);

/// Lo que el productor publica.
///
/// Todos los campos pueden faltar salvo las líneas: un panel sin líneas no es un panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    /// Versión del contrato. **Falta en siete de los nueve ficheros que hay en el repo**, porque se
    /// añadió después: su ausencia no es un error.
    pub version: Option<u64>,
    /// Lo que el productor dice de cuándo publicó, tal cual, **como texto y sin interpretar**. Se
    /// pinta; no se usa para calcular la antigüedad — ver [`Panel::written_at`].
    pub updated_at: Option<String>,
    /// En qué está el productor. Texto libre suyo, y se pinta literal: traducirlo a un color o a un
    /// icono sería interpretar su vocabulario, y el vocabulario es de quien trabaja.
    pub phase: Option<String>,
    /// Si la aplicación añade su línea de identidad arriba. Por omisión, sí.
    pub header: bool,
    /// Cuánto silencio tolera este productor antes de que su panel se marque como viejo.
    pub stale_after: Duration,
    /// Si esta división **reclama algo**, y de qué clase. Nada = no reclama.
    ///
    /// **Es el único campo del contrato que la aplicación interpreta**, y eso es deliberado: responde
    /// la única pregunta que necesita para navegar —¿tengo que ir ahí?— y de ahí salen el indicador
    /// del panel y el de la ficha de su pestaña. El resto del vocabulario —las marcas, los estilos, la
    /// fase— **se guarda y se pinta**, porque traducirlo sería que la aplicación decida qué significa
    /// el trabajo de otro.
    ///
    /// **No se deduce del cuerpo**: una línea con [`Mark::Block`] es contenido del resumen, no una
    /// señal. Deducirlo de ahí sería frágil de la forma más tonta — un «esperé aprobación el martes»
    /// marcaría la pestaña para siempre.
    pub attention: Option<Attention>,
    /// El cuerpo.
    pub lines: Vec<Line>,
    /// Cuándo se escribió el fichero, **que es de donde sale la antigüedad**.
    ///
    /// No sale de `updated_at`, y la razón es del propio contrato: el productor **machaca el fichero
    /// entero** en cada publicación, así que la fecha del fichero *es* la de su última publicación. Así
    /// hay aviso de rancio incluso cuando el productor no escribe su marca —que es cuando más falta
    /// hace— y esta caja no necesita interpretar fechas ni arrastrar un calendario.
    pub written_at: Option<SystemTime>,
}

/// Una línea del panel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub text: String,
    /// Qué **es** esta línea. La aplicación elige con qué símbolo se pinta y cómo se alinea.
    pub mark: Option<Mark>,
    /// Etiqueta corta que se alinea en columna. No tiene que ser un número.
    pub reference: Option<String>,
    /// Cómo se pinta. **Puede venir junto a una marca**, y entonces el color lo pone el estilo y el
    /// símbolo la marca: dos de los nueve ficheros del repo lo hacen, y a propósito — es como se dice
    /// «esta línea es información, y su color *es* el dato».
    pub style: Option<Style>,
    /// Sangría manual, en celdas.
    pub indent: usize,
    /// La dirección que el productor asocia a esta línea. **Se lee y no se usa todavía**: pintarla
    /// mostraría algo distinto de lo que se copiaría, porque el pintado descarta caracteres que la
    /// cadena sí tiene.
    pub link: Option<String>,
}

/// Qué es una línea. Los seis del contrato, ni uno más.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Done,
    Current,
    Pending,
    Wait,
    Block,
    Info,
}

/// Qué reclama una división, cuando reclama algo.
///
/// **Se ordena por la prioridad que el contrato declara**, no por el sitio en que estén escritas las
/// variantes: cuando una pestaña tiene varias divisiones, «lo que más reclama» es el máximo de sus
/// reclamos. [`Attention::Wait`] es el mayor porque **es lo único que se desatasca contigo delante ahora
/// mismo**; una división atascada por un tercero puede esperar cinco minutos más, y una que terminó y
/// nadie ha mirado, todo el día.
///
/// **El orden se escribe, no se deriva, y la diferencia importa el día que haya un cuarto valor.** Un
/// `Ord` derivado ordena por la posición de las variantes en el fichero: quien añadiera una en medio
/// cambiaría la prioridad **en silencio**, sin que fallara la compilación ni ninguna prueba. Con el
/// `match` de abajo, esa cuarta variante **no compila** hasta que alguien decida qué prioridad tiene —
/// que es exactamente la conversación que hay que tener. Y las variantes pueden entonces declararse en
/// **el orden del contrato**, como las de [`Mark`], en vez de al revés para que el derive salga.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    /// **Te espera a ti**: una pregunta, un permiso, una decisión.
    Wait,
    /// Atascada por algo que no eres tú.
    Block,
    /// Terminó, y nadie lo ha mirado.
    Done,
}

impl Attention {
    /// Las tres, para quien tenga que recorrerlas — una tabla de color, una de símbolos, una prueba.
    ///
    /// **Vive aquí, pegada a `priority`, y no en cada sitio que las recorre.** Una lista literal
    /// repetida por ahí admite una cuarta variante **en silencio**, que es justo lo que el `match` de
    /// abajo impide para las tablas de producción: quien añada una no compilará hasta tocar
    /// `priority`, y entonces tiene esta lista tres líneas más arriba.
    pub const ALL: [Self; 3] = [Self::Wait, Self::Block, Self::Done];

    /// Cuánto reclama, para poder comparar dos. Números sin más significado que su orden.
    fn priority(self) -> u8 {
        match self {
            Self::Done => 0,
            Self::Block => 1,
            Self::Wait => 2,
        }
    }
}

impl Ord for Attention {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority().cmp(&other.priority())
    }
}

impl PartialOrd for Attention {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Cómo se pinta una línea. Los siete del contrato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Normal,
    Dim,
    Title,
    Accent,
    Ok,
    Warn,
    Error,
}

/// Por qué no se pudo leer un panel.
///
/// Cada variante es una cosa distinta que decirle al usuario: que su productor no ha publicado todavía
/// no se parece en nada a que haya publicado algo que no se entiende.
#[derive(Debug)]
pub enum Problem {
    /// Todavía no hay fichero. El caso normal antes de la primera publicación.
    Missing,
    /// Hay algo con ese nombre, pero no es un fichero.
    ///
    /// **Importa porque una tubería con nombre bloquearía la apertura**, y como leer ocurre dentro del
    /// fotograma, eso sería la ventana congelada. Se comprueba **antes** de abrir, así que cubre el
    /// descuido —un productor que crea ahí lo que no debe— y no una carrera deliberada: entre la
    /// comprobación y la apertura hay un hueco, y quien pueda escribir en ese directorio es el propio
    /// usuario.
    NotRegular,
    /// Se pasa del techo de [`MAX_BYTES`].
    TooBig,
    /// No se pudo abrir o leer.
    Unreadable(std::io::Error),
    /// Se leyó, pero no es un panel: no es JSON, o no trae líneas.
    Invalid(String),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "sin publicar todavía"),
            Self::NotRegular => write!(f, "hay algo ahí que no es un fichero"),
            Self::TooBig => write!(f, "pasa de {} KiB", MAX_BYTES / 1024),
            Self::Unreadable(failure) => write!(f, "no se pudo leer ({failure})"),
            Self::Invalid(reason) => write!(f, "no se entiende ({reason})"),
        }
    }
}

impl Panel {
    /// Si lo que se ve ya no es de fiar por viejo.
    ///
    /// **La resta va con signo a propósito.** Una fecha en el futuro no es el caso raro: basta con un
    /// reloj adelantado o un sistema de ficheros ajeno. Con la resta ingenua de tiempos eso es un
    /// pánico, y este camino no puede entrar en pánico por nada que venga de fuera.
    pub fn is_stale_at(&self, now: SystemTime) -> bool {
        let Some(written) = self.written_at else {
            return false;
        };
        match now.duration_since(written) {
            Ok(silence) => silence > self.stale_after,
            // El fichero dice que se escribió en el futuro: no hay silencio que medir.
            Err(_) => false,
        }
    }
}

/// Lee el panel de un directorio **solo si su fichero ha cambiado** desde la fecha que se le dice.
///
/// `Ok(None)` significa «lo que tienes sigue vigente», y es la respuesta que hace barata la relectura
/// desde un bucle de pintado: preguntar la fecha cuesta menos de un microsegundo, e interpretar puede
/// costar un fotograma entero — ver [`MAX_BYTES`].
///
/// **Vive aquí y no en quien llama, y esa frontera es la razón de que exista.** Decidir si el fichero
/// cambió exige saber **cómo se llama el fichero**, y ese nombre es del contrato: quien lo mirase por
/// su cuenta lo estaría escribiendo por segunda vez, y pediría los datos del fichero dos veces por
/// lectura. La fecha con la que comparar sale de [`Panel::written_at`] de la lectura anterior.
///
/// Sin fichero se devuelve el problema, no `None`: «desapareció» y «sigue igual» son cosas distintas
/// para quien pinta.
pub fn read_if_newer(dir: &Path, seen: Option<SystemTime>) -> Result<Option<Panel>, Problem> {
    match std::fs::metadata(dir.join(crate::PANEL_FILE)) {
        Ok(metadata)
            if metadata
                .modified()
                .is_ok_and(|modified| Some(modified) == seen) =>
        {
            Ok(None)
        }
        // **La ausencia se resuelve aquí y no volviendo a [`read`]**, que es el caso más frecuente de
        // todos: una consola cuyo productor no publica nada pasa por aquí en cada fotograma, y
        // dejarla seguir pediría los datos del fichero **dos veces** para llegar a la misma
        // conclusión.
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => Err(Problem::Missing),
        _ => read(dir).map(Some),
    }
}

/// Lee el panel que hay en un directorio de división.
///
/// El nombre del fichero lo pone esta caja, no quien llama: es parte del contrato y vive en un solo
/// sitio.
pub fn read(dir: &Path) -> Result<Panel, Problem> {
    let path = dir.join(crate::PANEL_FILE);

    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => {
            return Err(Problem::Missing);
        }
        Err(failure) => return Err(Problem::Unreadable(failure)),
    };
    if !metadata.is_file() {
        return Err(Problem::NotRegular);
    }

    let file = std::fs::File::open(&path).map_err(Problem::Unreadable)?;
    let bytes = read_capped(file, metadata.len()).map_err(Problem::Unreadable)?;
    // **El tope y la codificación se deciden por separado, y en este orden.** Leer directamente a
    // texto ataba las dos cosas: el corte del tope parte un carácter de varios bytes —y aquí hay
    // acentos y guiones largos en todos los ficheros reales—, así que un fichero enorme salía como
    // «no se pudo leer» y el camino de «se pasa del techo» casi nunca se alcanzaba.
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Problem::TooBig);
    }
    let text =
        String::from_utf8(bytes).map_err(|_| Problem::Invalid("no es texto UTF-8".to_owned()))?;

    let mut panel = parse(&text)?;
    panel.written_at = metadata.modified().ok();
    Ok(panel)
}

/// Lee como mucho un byte más que el techo.
///
/// **El techo se aplica leyendo, y solo aquí.** Mirar el tamaño antes de abrir parece más barato, pero
/// sería un segundo camino que ninguna prueba puede ejercitar —el primero rechazaría siempre antes— y
/// encima dejaría fuera la carrera que importa: el productor machaca el fichero entero, así que entre
/// mirar y leer puede haber escrito uno más grande.
///
/// El byte de más es lo que permite distinguir «ocupa justo el techo» de «se pasa». Y está separado del
/// resto **para poder afirmar cuánto se lee**: mirando solo el resultado de [`read`], un tope que no
/// acotara nada daría el mismo error, porque la comprobación de después lo taparía.
///
/// **`expected` es una pista de tamaño, no una comprobación**, y la distinción es justo la que la
/// decisión de arriba defiende: nadie decide nada con ese número, solo se reserva sitio de una vez.
/// Sin él, `read_to_end` empieza por 32 bytes y duplica: medido, **nueve llamadas al sistema para un
/// panel de 861 bytes y diecisiete para uno en el techo**, que además deja reservado el doble de lo que
/// ocupa. Con la pista son cuatro y cinco, y ninguna reasignación. Importa poco en un disco local y
/// mucho en uno de red, donde cada ida y vuelta son milisegundos dentro del fotograma. Si el productor
/// machaca el fichero con uno mayor entre el `metadata` y la lectura, la pista se queda corta y
/// `read_to_end` crece solo: el techo lo sigue poniendo el `take`.
fn read_capped(file: std::fs::File, expected: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(expected.min(MAX_BYTES) as usize + 1);
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Convierte el texto en un panel, tolerando por campo.
///
/// **La tolerancia es por campo y no por documento**, y eso decide cómo se lee: un campo suelto que no
/// se entiende —un número donde iba texto, un nombre de marca que no existe— se ignora y se usa su
/// valor por omisión, en vez de tirar el fichero entero. Quien publica es un agente escribiendo JSON a
/// mano: pasará, y perder el panel completo por un campo que nadie mira sería lo peor que podría hacer.
///
/// Solo hay dos formas de que un fichero no valga: que no sea JSON, o que no traiga líneas.
fn parse(text: &str) -> Result<Panel, Problem> {
    let raw: serde_json::Value =
        serde_json::from_str(text).map_err(|failure| Problem::Invalid(failure.to_string()))?;

    let lines = raw
        .get("lines")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Problem::Invalid("no trae «lines»".to_owned()))?;

    Ok(Panel {
        version: raw.get("v").and_then(serde_json::Value::as_u64),
        updated_at: string(&raw, "updated_at"),
        phase: string(&raw, "phase"),
        header: raw
            .get("header")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        stale_after: raw
            .get("stale_after_minutes")
            .and_then(serde_json::Value::as_u64)
            .map_or(STALE_AFTER, |minutes| {
                Duration::from_secs(minutes.saturating_mul(60))
            }),
        attention: string_ref(&raw, "attention").and_then(attention),
        lines: lines.iter().filter_map(line).collect(),
        written_at: None,
    })
}

/// Convierte un elemento de la lista en una línea, o **la descarta**.
///
/// Un elemento que no es ni cadena ni objeto —un número, un nulo— no es una línea de nada: pintarlo
/// como una línea en blanco sería inventar contenido a partir de basura.
fn line(raw: &serde_json::Value) -> Option<Line> {
    // Una cadena suelta es una línea normal, y la cadena vacía es un hueco: los dos casos son lo mismo
    // aquí, y es quien pinta el que sabe que un texto vacío deja un espacio.
    if let Some(text) = raw.as_str() {
        return Some(Line {
            text: text.to_owned(),
            ..Line::default()
        });
    }
    if !raw.is_object() {
        return None;
    }

    Some(Line {
        text: string(raw, "text").unwrap_or_default(),
        mark: string_ref(raw, "mark").and_then(mark),
        reference: string(raw, "ref"),
        style: string_ref(raw, "style").and_then(style),
        // Acotada **aquí**, que es la frontera con lo que viene de fuera: quien pinte va a repetir
        // ese número de espacios, y un valor absurdo sería un pánico por reservar memoria. El techo
        // también evita que la conversión trunque en silencio donde el entero de máquina sea menor.
        indent: raw
            .get("indent")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            .min(MAX_INDENT) as usize,
        link: string(raw, "link"),
    })
}

fn string(raw: &serde_json::Value, field: &str) -> Option<String> {
    string_ref(raw, field).map(str::to_owned)
}

/// Lo mismo, **sin copiar**: para los campos cuyo valor se traduce a un tipo propio y se tira.
///
/// **Existe porque esta caja mide lo que reserva.** Los tres vocabularios cerrados —la marca, el estilo
/// y el reclamo— convierten la cadena en un `enum` que cabe en un byte y sueltan el texto una llamada
/// después; pedir memoria para eso son tres reservas por línea que nadie llega a usar, en una caja con
/// un módulo entero dedicado a contar reservas y una tabla de picos en su documentación.
fn string_ref<'a>(raw: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    raw.get(field).and_then(serde_json::Value::as_str)
}

fn attention(name: &str) -> Option<Attention> {
    match name {
        "wait" => Some(Attention::Wait),
        "block" => Some(Attention::Block),
        "done" => Some(Attention::Done),
        _ => None,
    }
}

fn mark(name: &str) -> Option<Mark> {
    match name {
        "done" => Some(Mark::Done),
        "current" => Some(Mark::Current),
        "pending" => Some(Mark::Pending),
        "wait" => Some(Mark::Wait),
        "block" => Some(Mark::Block),
        "info" => Some(Mark::Info),
        _ => None,
    }
}

fn style(name: &str) -> Option<Style> {
    match name {
        "normal" => Some(Style::Normal),
        "dim" => Some(Style::Dim),
        "title" => Some(Style::Title),
        "accent" => Some(Style::Accent),
        "ok" => Some(Style::Ok),
        "warn" => Some(Style::Warn),
        "error" => Some(Style::Error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PanelDir;

    /// Compuesto con las rarezas de varios `panel.json` de verdad del repo —no es la copia de uno—: **estilo y marca en la
    /// misma línea**, la versión ausente, y referencias de anchos muy distintos con un guion largo.
    const ACTUAL: &str = r##"{
      "updated_at": "2026-08-11T22:00:00+02:00",
      "phase": "done",
      "header": true,
      "lines": [
        {"text": "F1.4: dos divisiones lado a lado", "style": "title"},
        "",
        {"ref": "#1", "text": "extraer Division de la ventana", "mark": "done",
         "link": "https://github.com/mashware/agent-terminal/pull/31"},
        {"ref": "#1–#11", "text": "el conocimiento", "mark": "wait"},
        {"ref": "Ahora", "text": "hito cerrado", "mark": "info", "style": "ok"},
        "una línea suelta"
      ]
    }"##;

    fn panel_in(dir: &PanelDir, contents: &str) -> Result<Panel, Problem> {
        std::fs::write(dir.path().join(crate::PANEL_FILE), contents).expect("publicar el panel");
        read(dir.path())
    }

    #[test]
    fn un_panel_de_verdad_se_lee_entero_y_en_orden() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_in(&dir, ACTUAL).expect("leer el panel");

        assert_eq!(panel.lines.len(), 6);
        assert_eq!(panel.lines[0].style, Some(Style::Title));
        assert_eq!(panel.lines[1].text, "", "la cadena vacía es un hueco");
        assert_eq!(panel.lines[2].mark, Some(Mark::Done));
        assert_eq!(panel.lines[2].reference.as_deref(), Some("#1"));
        assert_eq!(panel.lines[3].reference.as_deref(), Some("#1–#11"));
        assert_eq!(panel.lines[5].text, "una línea suelta");
        assert_eq!(panel.phase.as_deref(), Some("done"));
    }

    /// El contrato describe el estilo como algo «para líneas sin marca», pero **dos de los nueve
    /// ficheros del repo los combinan**, y a propósito: la marca dice qué es la línea y el estilo dice
    /// que su color es el dato. Si esto se leyera como excluyente, se perdería uno de los dos.
    #[test]
    fn una_linea_puede_traer_marca_y_estilo_a_la_vez() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_in(&dir, ACTUAL).expect("leer el panel");

        let now_line = &panel.lines[4];
        assert_eq!(now_line.mark, Some(Mark::Info));
        assert_eq!(now_line.style, Some(Style::Ok));
    }

    #[test]
    fn la_version_puede_faltar() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_in(&dir, ACTUAL).expect("leer el panel");

        assert_eq!(
            panel.version, None,
            "siete de los nueve del repo no la traen"
        );
    }

    #[test]
    fn la_cabecera_se_da_por_puesta_y_se_puede_quitar() {
        let dir = PanelDir::create().expect("crear el directorio");

        let unstated = panel_in(&dir, r#"{"lines":[]}"#).expect("leer");
        assert!(unstated.header, "por omisión la pone la aplicación");

        let removed = panel_in(&dir, r#"{"header":false,"lines":[]}"#).expect("leer");
        assert!(!removed.header);
    }

    /// **La tolerancia es por campo, no por documento.** Quien publica es un agente escribiendo JSON a
    /// mano: un campo mal puesto pasará, y perder el panel entero por uno que nadie mira sería lo peor
    /// que se podría hacer con él.
    #[test]
    fn un_campo_que_no_se_entiende_no_tira_el_fichero() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_in(
            &dir,
            r#"{"v":"uno","header":"sí","stale_after_minutes":-5,"attention":3,
                "lines":[{"text":"vale","mark":"inventada","style":42,"indent":"mucha"}]}"#,
        )
        .expect("un campo malo no invalida el panel");

        assert_eq!(panel.version, None);
        assert_eq!(
            panel.attention, None,
            "un reclamo que no es ni siquiera una cadena tiene que caer en «no reclama», no explotar"
        );
        assert!(
            panel.header,
            "un valor que no es booleano cae en el de omisión"
        );
        assert_eq!(panel.stale_after, STALE_AFTER);
        assert_eq!(panel.lines.len(), 1);
        assert_eq!(panel.lines[0].text, "vale", "la línea se conserva");
        assert_eq!(
            panel.lines[0].mark, None,
            "una marca que no existe se ignora"
        );
        assert_eq!(panel.lines[0].style, None);
        assert_eq!(panel.lines[0].indent, 0);
    }

    #[test]
    fn el_silencio_tolerado_lo_dice_el_productor() {
        let dir = PanelDir::create().expect("crear el directorio");

        let its_own = panel_in(&dir, r#"{"stale_after_minutes":5,"lines":[]}"#).expect("leer");
        assert_eq!(its_own.stale_after, Duration::from_secs(300));

        let by_default = panel_in(&dir, r#"{"lines":[]}"#).expect("leer");
        assert_eq!(by_default.stale_after, STALE_AFTER);
    }

    #[test]
    fn sin_lineas_no_es_un_panel() {
        let dir = PanelDir::create().expect("crear el directorio");

        let failure = panel_in(&dir, r#"{"phase":"algo"}"#).expect_err("sin líneas no vale");

        assert!(matches!(failure, Problem::Invalid(_)), "fue {failure:?}");
    }

    /// Un `lines` que no es una lista **no es un panel vacío**: si se leyera como tal, quien pinta
    /// perdería el último panel bueno y en su sitio dejaría un hueco, que es exactamente lo que la
    /// distinción entre «no se entiende» y «no hay nada» existe para evitar.
    #[test]
    fn unas_lineas_que_no_son_una_lista_no_son_un_panel() {
        let dir = PanelDir::create().expect("crear el directorio");

        for text in [
            r#"{"lines":"una sola"}"#,
            r#"{"lines":{"1":"una"}}"#,
            r#"{"lines":7}"#,
        ] {
            let failure = panel_in(&dir, text).expect_err("«lines» tiene que ser una lista");
            assert!(
                matches!(failure, Problem::Invalid(_)),
                "con {text} fue {failure:?}"
            );
        }
    }

    /// JSON válido pero de otra forma: la raíz tiene que ser un objeto. Sin esto, buscar las líneas en
    /// algo que no las tiene podría acabar tomando la propia raíz por el cuerpo del panel.
    #[test]
    fn un_json_que_no_es_un_objeto_en_la_raiz_no_es_un_panel() {
        let dir = PanelDir::create().expect("crear el directorio");

        for text in [r#"["una","otra"]"#, r#""solo una cadena""#, "42", "null"] {
            let failure = panel_in(&dir, text).expect_err("la raíz tiene que ser un objeto");
            assert!(
                matches!(failure, Problem::Invalid(_)),
                "con {text} fue {failure:?}"
            );
        }
    }

    #[test]
    fn lo_que_no_es_json_se_dice_sin_romper_nada() {
        let dir = PanelDir::create().expect("crear el directorio");

        // Un fichero a medio escribir es exactamente esto: JSON cortado por la mitad.
        let failure = panel_in(&dir, r#"{"lines":[{"text":"a med"#).expect_err("no es JSON");

        assert!(matches!(failure, Problem::Invalid(_)), "fue {failure:?}");
    }

    /// **El fichero de cero bytes es el instante intermedio de cada publicación**: el productor que
    /// machaca abre para truncar y todavía no ha escrito nada. Tiene que salir «no se entiende» y no un
    /// panel vacío, porque un panel vacío borraría el último bueno justo en mitad de cada escritura.
    #[test]
    fn un_fichero_vacio_es_una_escritura_a_medias_y_no_un_panel_sin_lineas() {
        let dir = PanelDir::create().expect("crear el directorio");

        let failure = panel_in(&dir, "").expect_err("un fichero vacío no es un panel");

        assert!(matches!(failure, Problem::Invalid(_)), "fue {failure:?}");
    }

    #[test]
    fn sin_publicar_todavia_no_es_un_error_raro() {
        let dir = PanelDir::create().expect("crear el directorio");

        let failure = read(dir.path()).expect_err("no hay fichero");

        assert!(matches!(failure, Problem::Missing), "fue {failure:?}");
    }

    /// «Todavía no ha publicado» y «hay algo ahí que no puedo leer» son dos cosas distintas que
    /// decirle a quien mira, y solo la primera es normal. Sin permisos es lo único que separa las dos
    /// ramas de fallo, y hay **dos** ramas: la de mirar qué hay y la de abrirlo.
    #[cfg(unix)]
    #[test]
    fn sin_permiso_para_leerlo_no_se_confunde_con_no_haber_publicado() {
        use std::os::unix::fs::PermissionsExt;

        let dir = PanelDir::create().expect("crear el directorio");
        let path = dir.path().join(crate::PANEL_FILE);
        std::fs::write(&path, r#"{"lines":[]}"#).expect("publicar el panel");
        let mode = |path: &Path, mode| {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
                .expect("cambiar los permisos")
        };

        // Los permisos se devuelven enseguida, **antes** de afirmar nada: con la restauración detrás
        // del `assert`, un fallo del test dejaría un directorio que su destructor ya no puede vaciar.
        mode(&path, 0o000);
        // Quien corra la suite como administrador puede leer igualmente, y entonces aquí no hay nada
        // que comprobar. Se pregunta en vez de suponerlo, que sale más barato que traer una
        // biblioteca solo para mirar el usuario efectivo.
        let unreadable = std::fs::File::open(&path).is_err();
        let from_file = read(dir.path());
        mode(&path, 0o600);

        mode(dir.path(), 0o000);
        let from_dir = read(dir.path());
        mode(dir.path(), 0o700);

        if unreadable {
            assert!(
                matches!(from_file, Err(Problem::Unreadable(_))),
                "el fichero sin permisos salió como {from_file:?}"
            );
            assert!(
                matches!(from_dir, Err(Problem::Unreadable(_))),
                "el directorio sin permisos salió como {from_dir:?}"
            );
        }
    }

    /// **La misma propiedad que la de abajo, pero sobre el camino de verdad.**
    ///
    /// La de abajo llama a [`read_capped`] directamente, así que solo ata al ayudante: sustituyendo
    /// dentro de [`read`] esa llamada por un `std::fs::read` —reservar el fichero entero, que es justo
    /// lo que el techo existe para impedir— **la suite seguía verde**, porque la comprobación de
    /// después devuelve [`Problem::TooBig`] igual. Medido: el error no distingue los dos caminos, solo
    /// la memoria lo hace.
    ///
    /// El fichero mide ocho veces el techo, así que leerlo entero reservaría 2 MiB y el margen contra
    /// el que se afirma es de 1 MiB: entre lo que cuesta el camino honesto —unos 512 KiB, lo que la
    /// duplicación de `read_to_end` deja de holgura sobre 256 KiB— y lo que costaría el deshonesto hay
    /// un factor de cuatro, que es sitio de sobra para que esto no dependa de la máquina.
    #[test]
    fn leer_un_fichero_enorme_no_reserva_lo_que_ocupa() {
        let dir = PanelDir::create().expect("crear el directorio");
        let padding = "x".repeat(MAX_BYTES as usize * 8);
        let contents = format!(r#"{{"lines":["{padding}"]}}"#);
        std::fs::write(dir.path().join(crate::PANEL_FILE), &contents).expect("publicar uno enorme");
        drop(contents);

        let (failure, peak) = while_measuring(|| read(dir.path()));

        assert!(matches!(failure, Err(Problem::TooBig)), "fue {failure:?}");
        assert!(
            peak < MAX_BYTES as usize * 4,
            "reservó {peak} bytes leyendo un fichero que se rechaza por grande"
        );
    }

    /// La propiedad es **cuánto se lee**, no qué error sale: mirando solo el error de `read`, un tope
    /// que no acotara nada daría el mismo resultado porque la comprobación de después lo taparía.
    ///
    /// **Y la pista de tamaño no puede convertirse en el tope.** Se le pasa la mentira más grande que
    /// cabe —que el fichero no ocupa nada—: si algún día alguien reserva por la pista en vez de leer
    /// hasta el `take`, este caso devuelve un byte y cae aquí.
    #[test]
    fn de_un_fichero_enorme_solo_se_leen_los_primeros_bytes() {
        let dir = PanelDir::create().expect("crear el directorio");
        let path = dir.path().join(crate::PANEL_FILE);
        std::fs::write(&path, "x".repeat(MAX_BYTES as usize * 4)).expect("escribir uno enorme");

        let read_back = read_capped(std::fs::File::open(&path).expect("abrir"), 0).expect("leer");

        assert_eq!(
            read_back.len() as u64,
            MAX_BYTES + 1,
            "se leyó más que el techo y un byte"
        );
    }

    #[test]
    fn un_fichero_enorme_no_se_lee_entero() {
        let dir = PanelDir::create().expect("crear el directorio");
        let padding = "x".repeat(MAX_BYTES as usize + 1024);
        let too_big = format!(r#"{{"lines":["{padding}"]}}"#);

        let failure = panel_in(&dir, &too_big).expect_err("se pasa del techo");

        assert!(matches!(failure, Problem::TooBig), "fue {failure:?}");
    }

    /// **Con acentos, que es lo normal aquí**: todos los ficheros reales del repo llevan tildes y
    /// guiones largos. Leer directamente a texto ataba el tope a la codificación, y el corte partía un
    /// carácter de dos bytes: el fichero enorme salía como «no se pudo leer» y este camino no se
    /// alcanzaba nunca.
    #[test]
    fn un_fichero_enorme_con_acentos_sigue_siendo_demasiado_grande() {
        let dir = PanelDir::create().expect("crear el directorio");
        let padding = "ñ".repeat(MAX_BYTES as usize);

        let failure = panel_in(&dir, &format!(r#"{{"lines":["{padding}"]}}"#))
            .expect_err("se pasa del techo");

        assert!(matches!(failure, Problem::TooBig), "fue {failure:?}");
    }

    /// Un fichero que no es texto no es «no se pudo leer» —eso suena a permisos— sino «no se entiende».
    /// Y es un caso real: una escritura cortada justo entre los dos bytes de una letra acentuada.
    #[test]
    fn lo_que_no_es_texto_se_dice_como_lo_que_es() {
        let dir = PanelDir::create().expect("crear el directorio");
        let path = dir.path().join(crate::PANEL_FILE);
        std::fs::write(&path, b"{\"lines\":[\"caf\xe9\"]}").expect("escribir bytes crudos");

        let failure = read(dir.path()).expect_err("no es UTF-8");

        assert!(matches!(failure, Problem::Invalid(_)), "fue {failure:?}");
    }

    /// Lo que no es ni cadena ni objeto no es una línea: pintarlo en blanco sería inventar contenido a
    /// partir de basura.
    #[test]
    fn lo_que_no_es_una_linea_se_descarta_en_vez_de_pintarse_en_blanco() {
        let dir = PanelDir::create().expect("crear el directorio");

        let panel = panel_in(&dir, r#"{"lines":[null,42,"buena",{"text":"también"}]}"#)
            .expect("las buenas se leen");

        assert_eq!(panel.lines.len(), 2, "el nulo y el número no son líneas");
        assert_eq!(panel.lines[0].text, "buena");
        assert_eq!(panel.lines[1].text, "también");
    }

    /// El tiempo que el productor tolera se multiplica por sesenta: un número absurdo no puede desbordar.
    #[test]
    fn un_tiempo_de_tolerancia_absurdo_no_desborda() {
        let dir = PanelDir::create().expect("crear el directorio");

        let panel = panel_in(
            &dir,
            r#"{"stale_after_minutes":18446744073709551615,"lines":[]}"#,
        )
        .expect("leer");

        assert_eq!(panel.stale_after, Duration::from_secs(u64::MAX));
    }

    /// Las tres palabras con las que un productor dice que su división reclama algo.
    #[test]
    fn las_tres_clases_de_reclamo_se_leen() {
        let dir = PanelDir::create().expect("crear el directorio");

        for (written, expected) in [
            ("wait", Attention::Wait),
            ("block", Attention::Block),
            ("done", Attention::Done),
        ] {
            let panel = panel_in(&dir, &format!(r#"{{"attention":"{written}","lines":[]}}"#))
                .expect("leer");

            assert_eq!(panel.attention, Some(expected), "con «{written}»");
        }
    }

    /// **La prioridad es el orden del tipo, y de eso depende lo que enseñe una ficha de pestaña.**
    ///
    /// Se afirma sobre el **máximo de una lista**, que es como la va a usar quien acumule, y no sobre
    /// comparaciones sueltas: con `a < b` a pares, un orden mal declarado puede seguir pasando si las
    /// parejas que se prueban no son las que mandan.
    ///
    /// **Lo que esta prueba NO puede cubrir, y por eso la prioridad se escribe a mano**: una cuarta
    /// clase de reclamo que alguien añadiera mañana no aparecería en ninguna de estas listas, así que la
    /// prueba seguiría en verde con la prioridad nueva sin decidir. Lo que lo impide es que el `match`
    /// de `priority` deja de compilar.
    #[test]
    fn lo_que_mas_reclama_es_el_maximo() {
        use Attention::{Block, Done, Wait};

        assert_eq!([Done, Block, Wait].into_iter().max(), Some(Wait));
        assert_eq!([Block, Wait].into_iter().max(), Some(Wait));
        assert_eq!([Done, Block].into_iter().max(), Some(Block));
        assert_eq!([Done].into_iter().max(), Some(Done));
    }

    /// Lo escribe un tercero a mano, así que una palabra que no existe es cuestión de tiempo — y el
    /// resto de su panel no tiene la culpa.
    #[test]
    fn un_reclamo_que_no_existe_se_lee_como_si_no_hubiera_dicho_nada() {
        let dir = PanelDir::create().expect("crear el directorio");

        let panel = panel_in(
            &dir,
            r#"{"attention":"waiting","phase":"validate","lines":[{"text":"algo"}]}"#,
        )
        .expect("leer");

        assert_eq!(panel.attention, None);
        assert_eq!(
            panel.phase.as_deref(),
            Some("validate"),
            "un campo mal escrito se llevó por delante el resto del panel"
        );
        assert_eq!(panel.lines.len(), 1, "y también el cuerpo");
    }

    /// **El cuerpo no es una señal, y esto es la mitad del hito que vive en esta caja.** Bastaría un
    /// «esperé aprobación el martes» para dejar una pestaña marcada para siempre.
    ///
    /// **Es una prueba de regresión, no de cobertura, y conviene saber qué la hace caer**: hoy no hay
    /// ningún camino que derive el reclamo del cuerpo, así que la afirmación se cumple por ausencia. Cae
    /// **cuando alguien escribe ese camino** — comprobado añadiendo a la lectura un `or_else` que
    /// buscara la primera línea con marca de espera o de bloqueo. Ése es exactamente el atajo que este
    /// hito existe para que nadie tome.
    #[test]
    fn una_marca_del_cuerpo_no_es_un_reclamo_de_la_division() {
        let dir = PanelDir::create().expect("crear el directorio");

        let panel = panel_in(
            &dir,
            r#"{"lines":[{"text":"esperé aprobación el martes","mark":"wait"},{"text":"y esto","mark":"block"}]}"#,
        )
        .expect("leer");

        assert_eq!(
            panel.lines[0].mark,
            Some(Mark::Wait),
            "el cuerpo sí la trae"
        );
        assert_eq!(
            panel.attention, None,
            "el cuerpo produjo un reclamo de la división entera"
        );
    }

    /// La sangría viene de fuera y quien pinta repetirá esos espacios.
    #[test]
    fn una_sangria_absurda_se_acota() {
        let dir = PanelDir::create().expect("crear el directorio");

        let panel = panel_in(
            &dir,
            r#"{"lines":[{"text":"t","indent":18446744073709551615}]}"#,
        )
        .expect("leer");

        assert_eq!(panel.lines[0].indent, MAX_INDENT as usize);
    }

    /// Una sangría negativa no es «mucha sangría»: es un campo mal escrito y vale su valor por omisión.
    /// El caso existe porque el número entra sin signo, y leerlo con signo para después convertirlo
    /// daría la vuelta al valor y sangraría hasta el tope justo cuando el productor pedía lo contrario.
    #[test]
    fn una_sangria_negativa_no_da_la_vuelta() {
        let dir = PanelDir::create().expect("crear el directorio");

        let panel = panel_in(&dir, r#"{"lines":[{"text":"t","indent":-3}]}"#).expect("leer");

        assert_eq!(panel.lines[0].indent, 0);
    }

    /// Lo que hay ahí podría no ser un fichero, y una tubería con nombre bloquearía la apertura: como
    /// leer ocurre dentro del fotograma, eso sería la ventana congelada.
    #[test]
    fn lo_que_no_es_un_fichero_se_rechaza_antes_de_abrirlo() {
        let dir = PanelDir::create().expect("crear el directorio");
        std::fs::create_dir(dir.path().join(crate::PANEL_FILE))
            .expect("poner un directorio donde iba el fichero");

        let failure = read(dir.path());
        // Se limpia **antes** de afirmar: con la limpieza detrás, un fallo del test dejaría un
        // directorio que el destructor tampoco puede borrar, porque no está vacío.
        std::fs::remove_dir(dir.path().join(crate::PANEL_FILE)).expect("limpiar");

        assert!(
            matches!(failure, Err(Problem::NotRegular)),
            "fue {failure:?}"
        );
    }

    /// **El caso para el que existe el mecanismo, y hasta ahora no estaba probado.** El test de arriba
    /// usa un directorio, que es precisamente el que **no** bloquea: abrirlo falla y ya. Una tubería con
    /// nombre sí se queda esperando a que alguien escriba al otro lado, y como leer ocurre dentro del
    /// fotograma, eso es la ventana congelada — para siempre, no un rato.
    ///
    /// Si esta prueba llegara a colgarse en vez de fallar, **ese es el fallo**: significa que la
    /// comprobación de fichero regular desapareció.
    #[cfg(unix)]
    #[test]
    fn una_tuberia_con_nombre_se_rechaza_en_vez_de_colgar_la_ventana() {
        let dir = PanelDir::create().expect("crear el directorio");
        let path = dir.path().join(crate::PANEL_FILE);
        let mkfifo = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .is_ok_and(|status| status.success());
        assert!(mkfifo, "no se pudo crear la tubería con nombre");

        let failure = read(dir.path());
        std::fs::remove_file(&path).expect("limpiar la tubería");

        assert!(
            matches!(failure, Err(Problem::NotRegular)),
            "fue {failure:?}"
        );
    }

    /// **La cara B de exigir un fichero: un enlace a un fichero normal sí vale.** Lo que se rechaza es
    /// lo que bloquearía la apertura, y un enlace no bloquea nada. Y no es un caso rebuscado: el
    /// productor que existe hoy publica en el directorio de su trabajo, así que enlazar el fichero del
    /// contrato ahí es la forma evidente de aparecer en el panel sin cambiar de sitio.
    #[cfg(unix)]
    #[test]
    fn un_enlace_a_un_fichero_normal_se_lee_como_el_fichero_que_apunta() {
        let producer_dir = PanelDir::create().expect("crear el directorio del productor");
        let dir = PanelDir::create().expect("crear el directorio de la división");
        let published = producer_dir.path().join(crate::PANEL_FILE);
        std::fs::write(&published, r#"{"lines":["desde el otro lado"]}"#).expect("publicar");
        std::os::unix::fs::symlink(&published, dir.path().join(crate::PANEL_FILE))
            .expect("enlazar el fichero del contrato");

        let panel = read(dir.path()).expect("un enlace a un fichero normal se lee");

        assert_eq!(panel.lines[0].text, "desde el otro lado");
    }

    /// **La cadena entera de la antigüedad, de punta a punta.** Los dos tests de abajo construyen el
    /// panel a mano, así que ninguno ata la decisión que importa: que la antigüedad sale de la fecha del
    /// fichero. Sin este, olvidarse de guardarla dejaría la suite verde y **ningún panel se marcaría
    /// viejo jamás**.
    #[test]
    fn la_antiguedad_sale_de_la_fecha_del_fichero_leido() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_in(&dir, r#"{"stale_after_minutes":1,"lines":[]}"#).expect("leer");

        let written = panel
            .written_at
            .expect("la fecha del fichero tiene que venir puesta");

        assert!(
            !panel.is_stale_at(written),
            "recién escrito no puede estar rancio"
        );
        assert!(
            panel.is_stale_at(written + Duration::from_secs(120)),
            "dos minutos de silencio con un tope de uno es rancio"
        );
    }

    /// Las seis marcas y los siete estilos, **todos**. Con solo los que trae un fichero de ejemplo,
    /// borrar el brazo de tres marcas y de cinco estilos dejaba la suite verde.
    #[test]
    fn se_leen_las_seis_marcas_y_los_siete_estilos() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_in(
            &dir,
            r#"{"lines":[
                {"text":"a","mark":"done"},{"text":"b","mark":"current"},
                {"text":"c","mark":"pending"},{"text":"d","mark":"wait"},
                {"text":"e","mark":"block"},{"text":"f","mark":"info"},
                {"text":"g","style":"normal"},{"text":"h","style":"dim"},
                {"text":"i","style":"title"},{"text":"j","style":"accent"},
                {"text":"k","style":"ok"},{"text":"l","style":"warn"},
                {"text":"m","style":"error"}
            ]}"#,
        )
        .expect("leer");

        let marks: Vec<Option<Mark>> = panel.lines[..6].iter().map(|l| l.mark).collect();
        assert_eq!(
            marks,
            [
                Some(Mark::Done),
                Some(Mark::Current),
                Some(Mark::Pending),
                Some(Mark::Wait),
                Some(Mark::Block),
                Some(Mark::Info)
            ]
        );

        let styles: Vec<Option<Style>> = panel.lines[6..].iter().map(|l| l.style).collect();
        assert_eq!(
            styles,
            [
                Some(Style::Normal),
                Some(Style::Dim),
                Some(Style::Title),
                Some(Style::Accent),
                Some(Style::Ok),
                Some(Style::Warn),
                Some(Style::Error)
            ]
        );
    }

    /// Los tres campos que solo se guardan para que alguien los use después. Sin esto, «no se lee» y
    /// «viene vacío» son indistinguibles.
    #[test]
    fn se_leen_la_version_la_marca_de_tiempo_del_productor_y_el_enlace() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_in(
            &dir,
            r#"{"v":1,"updated_at":"2026-08-06T18:12:00+02:00",
                "lines":[{"text":"t","link":"https://example.test/1","indent":3}]}"#,
        )
        .expect("leer");

        assert_eq!(panel.version, Some(1));
        assert_eq!(
            panel.updated_at.as_deref(),
            Some("2026-08-06T18:12:00+02:00")
        );
        assert_eq!(
            panel.lines[0].link.as_deref(),
            Some("https://example.test/1")
        );
        assert_eq!(panel.lines[0].indent, 3, "una sangría válida se conserva");
    }

    /// Las dos fronteras exactas, que es donde un `>` de más o de menos no se nota.
    #[test]
    fn justo_en_el_techo_se_acepta_y_un_byte_mas_no() {
        let dir = PanelDir::create().expect("crear el directorio");
        let overhead = r#"{"lines":["ANCHO"]}"#.len() - "ANCHO".len();
        let exact_fit = "x".repeat(MAX_BYTES as usize - overhead);

        let fits = panel_in(&dir, &format!(r#"{{"lines":["{exact_fit}"]}}"#)).expect("justo cabe");
        assert_eq!(fits.lines.len(), 1);

        let failure =
            panel_in(&dir, &format!(r#"{{"lines":["{exact_fit}x"]}}"#)).expect_err("uno más, no");
        assert!(matches!(failure, Problem::TooBig), "fue {failure:?}");
    }

    #[test]
    fn el_silencio_justo_igual_al_tope_todavia_no_es_rancio() {
        let panel = Panel {
            stale_after: Duration::from_secs(60),
            written_at: Some(SystemTime::UNIX_EPOCH),
            ..empty_panel()
        };

        assert!(!panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(60)));
        assert!(panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(61)));
    }

    #[test]
    fn un_panel_callado_desde_hace_rato_esta_rancio() {
        let panel = Panel {
            stale_after: Duration::from_secs(60),
            written_at: Some(SystemTime::UNIX_EPOCH),
            ..empty_panel()
        };

        assert!(panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(120)));
        assert!(!panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(30)));
    }

    /// Una fecha en el futuro no es el caso raro: basta un reloj adelantado. Con la resta de tiempos
    /// ingenua esto sería un pánico, y nada que venga de fuera puede provocar uno.
    #[test]
    fn una_fecha_en_el_futuro_no_entra_en_panico_ni_marca_rancio() {
        let panel = Panel {
            stale_after: Duration::from_secs(60),
            written_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(7200)),
            ..empty_panel()
        };

        assert!(!panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(60)));
    }

    #[test]
    fn sin_saber_cuando_se_escribio_no_hay_rancio_que_medir() {
        assert!(!empty_panel().is_stale_at(SystemTime::now()));
    }

    fn empty_panel() -> Panel {
        Panel {
            version: None,
            updated_at: None,
            phase: None,
            header: true,
            stale_after: STALE_AFTER,
            attention: None,
            lines: Vec::new(),
            written_at: None,
        }
    }

    // ─── Contar lo que se reserva ──────────────────────────────────────────────────────────────
    //
    // Existe por una sola prueba, y no había forma más barata: la propiedad «no se lee el fichero
    // entero» **no se ve en el resultado** —el error es el mismo se reserve un cuarto de mega o diez—,
    // así que o se mide la memoria o no está atada. Desde fuera del proceso tampoco vale: el máximo
    // residente que da el sistema es el de toda la corrida, y aquí corren decenas de pruebas a la vez.
    //
    // **La cuenta es por hilo, a propósito.** Las pruebas corren en paralelo, y un contador compartido
    // haría que lo que reserva una se le achacara a otra: el fallo sería intermitente, que es la peor
    // clase de prueba. Con un hilo por prueba, cada una mide lo suyo.

    /// Lo vivo y el máximo que llegó a estarlo, **en este hilo**.
    ///
    /// Se inicializan sin reservar —de ahí el `const`—: un valor perezoso reservaría memoria dentro del
    /// asignador la primera vez que se mira, que es una llamada a sí mismo.
    mod counting {
        use std::cell::Cell;

        thread_local! {
            pub static LIVE: Cell<usize> = const { Cell::new(0) };
            pub static PEAK: Cell<usize> = const { Cell::new(0) };
        }
    }

    struct Counter;

    /// Suma o resta lo vivo y se queda con el máximo.
    ///
    /// `try_with` y no `with`: al deshacerse un hilo, sus variables locales ya no están, y ahí todavía
    /// se libera memoria. Con `with` eso sería un pánico dentro del asignador.
    fn record(delta: isize) {
        let _ = counting::LIVE.try_with(|live| {
            let current = live.get().saturating_add_signed(delta);
            live.set(current);
            let _ = counting::PEAK.try_with(|peak| peak.set(peak.get().max(current)));
        });
    }

    unsafe impl std::alloc::GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let pointer = unsafe { std::alloc::System.alloc(layout) };
            if !pointer.is_null() {
                record(layout.size() as isize);
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
            record(-(layout.size() as isize));
            unsafe { std::alloc::System.dealloc(pointer, layout) }
        }

        unsafe fn realloc(
            &self,
            pointer: *mut u8,
            layout: std::alloc::Layout,
            new_size: usize,
        ) -> *mut u8 {
            let moved = unsafe { std::alloc::System.realloc(pointer, layout, new_size) };
            if !moved.is_null() {
                record(new_size as isize - layout.size() as isize);
            }
            moved
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counter = Counter;

    /// Ejecuta el trabajo y devuelve, además de lo que produjo, **cuánto llegó a tener vivo de más**.
    fn while_measuring<T>(work: impl FnOnce() -> T) -> (T, usize) {
        let baseline = counting::LIVE.get();
        counting::PEAK.set(baseline);

        let produced = work();

        (produced, counting::PEAK.get().saturating_sub(baseline))
    }
}
