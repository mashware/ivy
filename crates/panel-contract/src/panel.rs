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
/// `cargo run --release --example lectura -p panel-contract`:
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
    /// Cuánto reclama, para poder comparar dos. Números sin más significado que su orden.
    fn prioridad(self) -> u8 {
        match self {
            Self::Done => 0,
            Self::Block => 1,
            Self::Wait => 2,
        }
    }
}

impl Ord for Attention {
    fn cmp(&self, otro: &Self) -> std::cmp::Ordering {
        self.prioridad().cmp(&otro.prioridad())
    }
}

impl PartialOrd for Attention {
    fn partial_cmp(&self, otro: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(otro))
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
            Self::Unreadable(fallo) => write!(f, "no se pudo leer ({fallo})"),
            Self::Invalid(motivo) => write!(f, "no se entiende ({motivo})"),
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
        let Some(escrito) = self.written_at else {
            return false;
        };
        match now.duration_since(escrito) {
            Ok(silencio) => silencio > self.stale_after,
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
pub fn read_if_newer(dir: &Path, visto: Option<SystemTime>) -> Result<Option<Panel>, Problem> {
    match std::fs::metadata(dir.join(crate::FICHERO_DEL_PANEL)) {
        Ok(datos) if datos.modified().is_ok_and(|fecha| Some(fecha) == visto) => Ok(None),
        // **La ausencia se resuelve aquí y no volviendo a [`read`]**, que es el caso más frecuente de
        // todos: una consola cuyo productor no publica nada pasa por aquí en cada fotograma, y
        // dejarla seguir pediría los datos del fichero **dos veces** para llegar a la misma
        // conclusión.
        Err(fallo) if fallo.kind() == std::io::ErrorKind::NotFound => Err(Problem::Missing),
        _ => read(dir).map(Some),
    }
}

/// Lee el panel que hay en un directorio de división.
///
/// El nombre del fichero lo pone esta caja, no quien llama: es parte del contrato y vive en un solo
/// sitio.
pub fn read(dir: &Path) -> Result<Panel, Problem> {
    let ruta = dir.join(crate::FICHERO_DEL_PANEL);

    let datos = match std::fs::metadata(&ruta) {
        Ok(datos) => datos,
        Err(fallo) if fallo.kind() == std::io::ErrorKind::NotFound => return Err(Problem::Missing),
        Err(fallo) => return Err(Problem::Unreadable(fallo)),
    };
    if !datos.is_file() {
        return Err(Problem::NotRegular);
    }

    let fichero = std::fs::File::open(&ruta).map_err(Problem::Unreadable)?;
    let bytes = leer_acotado(fichero, datos.len()).map_err(Problem::Unreadable)?;
    // **El tope y la codificación se deciden por separado, y en este orden.** Leer directamente a
    // texto ataba las dos cosas: el corte del tope parte un carácter de varios bytes —y aquí hay
    // acentos y guiones largos en todos los ficheros reales—, así que un fichero enorme salía como
    // «no se pudo leer» y el camino de «se pasa del techo» casi nunca se alcanzaba.
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Problem::TooBig);
    }
    let texto =
        String::from_utf8(bytes).map_err(|_| Problem::Invalid("no es texto UTF-8".to_owned()))?;

    let mut panel = interpretar(&texto)?;
    panel.written_at = datos.modified().ok();
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
/// **`esperado` es una pista de tamaño, no una comprobación**, y la distinción es justo la que la
/// decisión de arriba defiende: nadie decide nada con ese número, solo se reserva sitio de una vez.
/// Sin él, `read_to_end` empieza por 32 bytes y duplica: medido, **nueve llamadas al sistema para un
/// panel de 861 bytes y diecisiete para uno en el techo**, que además deja reservado el doble de lo que
/// ocupa. Con la pista son cuatro y cinco, y ninguna reasignación. Importa poco en un disco local y
/// mucho en uno de red, donde cada ida y vuelta son milisegundos dentro del fotograma. Si el productor
/// machaca el fichero con uno mayor entre el `metadata` y la lectura, la pista se queda corta y
/// `read_to_end` crece solo: el techo lo sigue poniendo el `take`.
fn leer_acotado(fichero: std::fs::File, esperado: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(esperado.min(MAX_BYTES) as usize + 1);
    fichero.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
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
fn interpretar(texto: &str) -> Result<Panel, Problem> {
    let crudo: serde_json::Value =
        serde_json::from_str(texto).map_err(|fallo| Problem::Invalid(fallo.to_string()))?;

    let lineas = crudo
        .get("lines")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Problem::Invalid("no trae «lines»".to_owned()))?;

    Ok(Panel {
        version: crudo.get("v").and_then(serde_json::Value::as_u64),
        updated_at: cadena(&crudo, "updated_at"),
        phase: cadena(&crudo, "phase"),
        header: crudo
            .get("header")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        stale_after: crudo
            .get("stale_after_minutes")
            .and_then(serde_json::Value::as_u64)
            .map_or(STALE_AFTER, |minutos| {
                Duration::from_secs(minutos.saturating_mul(60))
            }),
        attention: cadena_cruda(&crudo, "attention").and_then(atencion),
        lines: lineas.iter().filter_map(linea).collect(),
        written_at: None,
    })
}

/// Convierte un elemento de la lista en una línea, o **la descarta**.
///
/// Un elemento que no es ni cadena ni objeto —un número, un nulo— no es una línea de nada: pintarlo
/// como una línea en blanco sería inventar contenido a partir de basura.
fn linea(crudo: &serde_json::Value) -> Option<Line> {
    // Una cadena suelta es una línea normal, y la cadena vacía es un hueco: los dos casos son lo mismo
    // aquí, y es quien pinta el que sabe que un texto vacío deja un espacio.
    if let Some(texto) = crudo.as_str() {
        return Some(Line {
            text: texto.to_owned(),
            ..Line::default()
        });
    }
    if !crudo.is_object() {
        return None;
    }

    Some(Line {
        text: cadena(crudo, "text").unwrap_or_default(),
        mark: cadena_cruda(crudo, "mark").and_then(marca),
        reference: cadena(crudo, "ref"),
        style: cadena_cruda(crudo, "style").and_then(estilo),
        // Acotada **aquí**, que es la frontera con lo que viene de fuera: quien pinte va a repetir
        // ese número de espacios, y un valor absurdo sería un pánico por reservar memoria. El techo
        // también evita que la conversión trunque en silencio donde el entero de máquina sea menor.
        indent: crudo
            .get("indent")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            .min(MAX_INDENT) as usize,
        link: cadena(crudo, "link"),
    })
}

fn cadena(crudo: &serde_json::Value, campo: &str) -> Option<String> {
    cadena_cruda(crudo, campo).map(str::to_owned)
}

/// Lo mismo, **sin copiar**: para los campos cuyo valor se traduce a un tipo propio y se tira.
///
/// **Existe porque esta caja mide lo que reserva.** Los tres vocabularios cerrados —la marca, el estilo
/// y el reclamo— convierten la cadena en un `enum` que cabe en un byte y sueltan el texto una llamada
/// después; pedir memoria para eso son tres reservas por línea que nadie llega a usar, en una caja con
/// un módulo entero dedicado a contar reservas y una tabla de picos en su documentación.
fn cadena_cruda<'a>(crudo: &'a serde_json::Value, campo: &str) -> Option<&'a str> {
    crudo.get(campo).and_then(serde_json::Value::as_str)
}

fn atencion(nombre: &str) -> Option<Attention> {
    match nombre {
        "wait" => Some(Attention::Wait),
        "block" => Some(Attention::Block),
        "done" => Some(Attention::Done),
        _ => None,
    }
}

fn marca(nombre: &str) -> Option<Mark> {
    match nombre {
        "done" => Some(Mark::Done),
        "current" => Some(Mark::Current),
        "pending" => Some(Mark::Pending),
        "wait" => Some(Mark::Wait),
        "block" => Some(Mark::Block),
        "info" => Some(Mark::Info),
        _ => None,
    }
}

fn estilo(nombre: &str) -> Option<Style> {
    match nombre {
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
    const REAL: &str = r##"{
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

    fn panel_en(dir: &PanelDir, contenido: &str) -> Result<Panel, Problem> {
        std::fs::write(dir.path().join(crate::FICHERO_DEL_PANEL), contenido)
            .expect("publicar el panel");
        read(dir.path())
    }

    #[test]
    fn un_panel_de_verdad_se_lee_entero_y_en_orden() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_en(&dir, REAL).expect("leer el panel");

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
        let panel = panel_en(&dir, REAL).expect("leer el panel");

        let ahora = &panel.lines[4];
        assert_eq!(ahora.mark, Some(Mark::Info));
        assert_eq!(ahora.style, Some(Style::Ok));
    }

    #[test]
    fn la_version_puede_faltar() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_en(&dir, REAL).expect("leer el panel");

        assert_eq!(
            panel.version, None,
            "siete de los nueve del repo no la traen"
        );
    }

    #[test]
    fn la_cabecera_se_da_por_puesta_y_se_puede_quitar() {
        let dir = PanelDir::create().expect("crear el directorio");

        let sin_decirlo = panel_en(&dir, r#"{"lines":[]}"#).expect("leer");
        assert!(sin_decirlo.header, "por omisión la pone la aplicación");

        let quitada = panel_en(&dir, r#"{"header":false,"lines":[]}"#).expect("leer");
        assert!(!quitada.header);
    }

    /// **La tolerancia es por campo, no por documento.** Quien publica es un agente escribiendo JSON a
    /// mano: un campo mal puesto pasará, y perder el panel entero por uno que nadie mira sería lo peor
    /// que se podría hacer con él.
    #[test]
    fn un_campo_que_no_se_entiende_no_tira_el_fichero() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_en(
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

        let suyo = panel_en(&dir, r#"{"stale_after_minutes":5,"lines":[]}"#).expect("leer");
        assert_eq!(suyo.stale_after, Duration::from_secs(300));

        let por_omision = panel_en(&dir, r#"{"lines":[]}"#).expect("leer");
        assert_eq!(por_omision.stale_after, STALE_AFTER);
    }

    #[test]
    fn sin_lineas_no_es_un_panel() {
        let dir = PanelDir::create().expect("crear el directorio");

        let fallo = panel_en(&dir, r#"{"phase":"algo"}"#).expect_err("sin líneas no vale");

        assert!(matches!(fallo, Problem::Invalid(_)), "fue {fallo:?}");
    }

    /// Un `lines` que no es una lista **no es un panel vacío**: si se leyera como tal, quien pinta
    /// perdería el último panel bueno y en su sitio dejaría un hueco, que es exactamente lo que la
    /// distinción entre «no se entiende» y «no hay nada» existe para evitar.
    #[test]
    fn unas_lineas_que_no_son_una_lista_no_son_un_panel() {
        let dir = PanelDir::create().expect("crear el directorio");

        for texto in [
            r#"{"lines":"una sola"}"#,
            r#"{"lines":{"1":"una"}}"#,
            r#"{"lines":7}"#,
        ] {
            let fallo = panel_en(&dir, texto).expect_err("«lines» tiene que ser una lista");
            assert!(
                matches!(fallo, Problem::Invalid(_)),
                "con {texto} fue {fallo:?}"
            );
        }
    }

    /// JSON válido pero de otra forma: la raíz tiene que ser un objeto. Sin esto, buscar las líneas en
    /// algo que no las tiene podría acabar tomando la propia raíz por el cuerpo del panel.
    #[test]
    fn un_json_que_no_es_un_objeto_en_la_raiz_no_es_un_panel() {
        let dir = PanelDir::create().expect("crear el directorio");

        for texto in [r#"["una","otra"]"#, r#""solo una cadena""#, "42", "null"] {
            let fallo = panel_en(&dir, texto).expect_err("la raíz tiene que ser un objeto");
            assert!(
                matches!(fallo, Problem::Invalid(_)),
                "con {texto} fue {fallo:?}"
            );
        }
    }

    #[test]
    fn lo_que_no_es_json_se_dice_sin_romper_nada() {
        let dir = PanelDir::create().expect("crear el directorio");

        // Un fichero a medio escribir es exactamente esto: JSON cortado por la mitad.
        let fallo = panel_en(&dir, r#"{"lines":[{"text":"a med"#).expect_err("no es JSON");

        assert!(matches!(fallo, Problem::Invalid(_)), "fue {fallo:?}");
    }

    /// **El fichero de cero bytes es el instante intermedio de cada publicación**: el productor que
    /// machaca abre para truncar y todavía no ha escrito nada. Tiene que salir «no se entiende» y no un
    /// panel vacío, porque un panel vacío borraría el último bueno justo en mitad de cada escritura.
    #[test]
    fn un_fichero_vacio_es_una_escritura_a_medias_y_no_un_panel_sin_lineas() {
        let dir = PanelDir::create().expect("crear el directorio");

        let fallo = panel_en(&dir, "").expect_err("un fichero vacío no es un panel");

        assert!(matches!(fallo, Problem::Invalid(_)), "fue {fallo:?}");
    }

    #[test]
    fn sin_publicar_todavia_no_es_un_error_raro() {
        let dir = PanelDir::create().expect("crear el directorio");

        let fallo = read(dir.path()).expect_err("no hay fichero");

        assert!(matches!(fallo, Problem::Missing), "fue {fallo:?}");
    }

    /// «Todavía no ha publicado» y «hay algo ahí que no puedo leer» son dos cosas distintas que
    /// decirle a quien mira, y solo la primera es normal. Sin permisos es lo único que separa las dos
    /// ramas de fallo, y hay **dos** ramas: la de mirar qué hay y la de abrirlo.
    #[cfg(unix)]
    #[test]
    fn sin_permiso_para_leerlo_no_se_confunde_con_no_haber_publicado() {
        use std::os::unix::fs::PermissionsExt;

        let dir = PanelDir::create().expect("crear el directorio");
        let ruta = dir.path().join(crate::FICHERO_DEL_PANEL);
        std::fs::write(&ruta, r#"{"lines":[]}"#).expect("publicar el panel");
        let modo = |ruta: &Path, modo| {
            std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(modo))
                .expect("cambiar los permisos")
        };

        // Los permisos se devuelven enseguida, **antes** de afirmar nada: con la restauración detrás
        // del `assert`, un fallo del test dejaría un directorio que su destructor ya no puede vaciar.
        modo(&ruta, 0o000);
        // Quien corra la suite como administrador puede leer igualmente, y entonces aquí no hay nada
        // que comprobar. Se pregunta en vez de suponerlo, que sale más barato que traer una
        // biblioteca solo para mirar el usuario efectivo.
        let sin_permiso = std::fs::File::open(&ruta).is_err();
        let del_fichero = read(dir.path());
        modo(&ruta, 0o600);

        modo(dir.path(), 0o000);
        let del_directorio = read(dir.path());
        modo(dir.path(), 0o700);

        if sin_permiso {
            assert!(
                matches!(del_fichero, Err(Problem::Unreadable(_))),
                "el fichero sin permisos salió como {del_fichero:?}"
            );
            assert!(
                matches!(del_directorio, Err(Problem::Unreadable(_))),
                "el directorio sin permisos salió como {del_directorio:?}"
            );
        }
    }

    /// **La misma propiedad que la de abajo, pero sobre el camino de verdad.**
    ///
    /// La de abajo llama a [`leer_acotado`] directamente, así que solo ata al ayudante: sustituyendo
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
        let relleno = "x".repeat(MAX_BYTES as usize * 8);
        let contenido = format!(r#"{{"lines":["{relleno}"]}}"#);
        std::fs::write(dir.path().join(crate::FICHERO_DEL_PANEL), &contenido)
            .expect("publicar uno enorme");
        drop(contenido);

        let (fallo, pico) = midiendo(|| read(dir.path()));

        assert!(matches!(fallo, Err(Problem::TooBig)), "fue {fallo:?}");
        assert!(
            pico < MAX_BYTES as usize * 4,
            "reservó {pico} bytes leyendo un fichero que se rechaza por grande"
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
        let ruta = dir.path().join(crate::FICHERO_DEL_PANEL);
        std::fs::write(&ruta, "x".repeat(MAX_BYTES as usize * 4)).expect("escribir uno enorme");

        let leido = leer_acotado(std::fs::File::open(&ruta).expect("abrir"), 0).expect("leer");

        assert_eq!(
            leido.len() as u64,
            MAX_BYTES + 1,
            "se leyó más que el techo y un byte"
        );
    }

    #[test]
    fn un_fichero_enorme_no_se_lee_entero() {
        let dir = PanelDir::create().expect("crear el directorio");
        let relleno = "x".repeat(MAX_BYTES as usize + 1024);
        let gordo = format!(r#"{{"lines":["{relleno}"]}}"#);

        let fallo = panel_en(&dir, &gordo).expect_err("se pasa del techo");

        assert!(matches!(fallo, Problem::TooBig), "fue {fallo:?}");
    }

    /// **Con acentos, que es lo normal aquí**: todos los ficheros reales del repo llevan tildes y
    /// guiones largos. Leer directamente a texto ataba el tope a la codificación, y el corte partía un
    /// carácter de dos bytes: el fichero enorme salía como «no se pudo leer» y este camino no se
    /// alcanzaba nunca.
    #[test]
    fn un_fichero_enorme_con_acentos_sigue_siendo_demasiado_grande() {
        let dir = PanelDir::create().expect("crear el directorio");
        let relleno = "ñ".repeat(MAX_BYTES as usize);

        let fallo = panel_en(&dir, &format!(r#"{{"lines":["{relleno}"]}}"#))
            .expect_err("se pasa del techo");

        assert!(matches!(fallo, Problem::TooBig), "fue {fallo:?}");
    }

    /// Un fichero que no es texto no es «no se pudo leer» —eso suena a permisos— sino «no se entiende».
    /// Y es un caso real: una escritura cortada justo entre los dos bytes de una letra acentuada.
    #[test]
    fn lo_que_no_es_texto_se_dice_como_lo_que_es() {
        let dir = PanelDir::create().expect("crear el directorio");
        let ruta = dir.path().join(crate::FICHERO_DEL_PANEL);
        std::fs::write(&ruta, b"{\"lines\":[\"caf\xe9\"]}").expect("escribir bytes crudos");

        let fallo = read(dir.path()).expect_err("no es UTF-8");

        assert!(matches!(fallo, Problem::Invalid(_)), "fue {fallo:?}");
    }

    /// Lo que no es ni cadena ni objeto no es una línea: pintarlo en blanco sería inventar contenido a
    /// partir de basura.
    #[test]
    fn lo_que_no_es_una_linea_se_descarta_en_vez_de_pintarse_en_blanco() {
        let dir = PanelDir::create().expect("crear el directorio");

        let panel = panel_en(&dir, r#"{"lines":[null,42,"buena",{"text":"también"}]}"#)
            .expect("las buenas se leen");

        assert_eq!(panel.lines.len(), 2, "el nulo y el número no son líneas");
        assert_eq!(panel.lines[0].text, "buena");
        assert_eq!(panel.lines[1].text, "también");
    }

    /// El tiempo que el productor tolera se multiplica por sesenta: un número absurdo no puede desbordar.
    #[test]
    fn un_tiempo_de_tolerancia_absurdo_no_desborda() {
        let dir = PanelDir::create().expect("crear el directorio");

        let panel = panel_en(
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

        for (escrito, esperado) in [
            ("wait", Attention::Wait),
            ("block", Attention::Block),
            ("done", Attention::Done),
        ] {
            let panel = panel_en(&dir, &format!(r#"{{"attention":"{escrito}","lines":[]}}"#))
                .expect("leer");

            assert_eq!(panel.attention, Some(esperado), "con «{escrito}»");
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
    /// de `prioridad` deja de compilar.
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

        let panel = panel_en(
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

        let panel = panel_en(
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

        let panel = panel_en(
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

        let panel = panel_en(&dir, r#"{"lines":[{"text":"t","indent":-3}]}"#).expect("leer");

        assert_eq!(panel.lines[0].indent, 0);
    }

    /// Lo que hay ahí podría no ser un fichero, y una tubería con nombre bloquearía la apertura: como
    /// leer ocurre dentro del fotograma, eso sería la ventana congelada.
    #[test]
    fn lo_que_no_es_un_fichero_se_rechaza_antes_de_abrirlo() {
        let dir = PanelDir::create().expect("crear el directorio");
        std::fs::create_dir(dir.path().join(crate::FICHERO_DEL_PANEL))
            .expect("poner un directorio donde iba el fichero");

        let fallo = read(dir.path());
        // Se limpia **antes** de afirmar: con la limpieza detrás, un fallo del test dejaría un
        // directorio que el destructor tampoco puede borrar, porque no está vacío.
        std::fs::remove_dir(dir.path().join(crate::FICHERO_DEL_PANEL)).expect("limpiar");

        assert!(matches!(fallo, Err(Problem::NotRegular)), "fue {fallo:?}");
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
        let ruta = dir.path().join(crate::FICHERO_DEL_PANEL);
        let hecha = std::process::Command::new("mkfifo")
            .arg(&ruta)
            .status()
            .is_ok_and(|salida| salida.success());
        assert!(hecha, "no se pudo crear la tubería con nombre");

        let fallo = read(dir.path());
        std::fs::remove_file(&ruta).expect("limpiar la tubería");

        assert!(matches!(fallo, Err(Problem::NotRegular)), "fue {fallo:?}");
    }

    /// **La cara B de exigir un fichero: un enlace a un fichero normal sí vale.** Lo que se rechaza es
    /// lo que bloquearía la apertura, y un enlace no bloquea nada. Y no es un caso rebuscado: el
    /// productor que existe hoy publica en el directorio de su trabajo, así que enlazar el fichero del
    /// contrato ahí es la forma evidente de aparecer en el panel sin cambiar de sitio.
    #[cfg(unix)]
    #[test]
    fn un_enlace_a_un_fichero_normal_se_lee_como_el_fichero_que_apunta() {
        let destino = PanelDir::create().expect("crear el directorio del productor");
        let dir = PanelDir::create().expect("crear el directorio de la división");
        let publicado = destino.path().join(crate::FICHERO_DEL_PANEL);
        std::fs::write(&publicado, r#"{"lines":["desde el otro lado"]}"#).expect("publicar");
        std::os::unix::fs::symlink(&publicado, dir.path().join(crate::FICHERO_DEL_PANEL))
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
        let panel = panel_en(&dir, r#"{"stale_after_minutes":1,"lines":[]}"#).expect("leer");

        let escrito = panel
            .written_at
            .expect("la fecha del fichero tiene que venir puesta");

        assert!(
            !panel.is_stale_at(escrito),
            "recién escrito no puede estar rancio"
        );
        assert!(
            panel.is_stale_at(escrito + Duration::from_secs(120)),
            "dos minutos de silencio con un tope de uno es rancio"
        );
    }

    /// Las seis marcas y los siete estilos, **todos**. Con solo los que trae un fichero de ejemplo,
    /// borrar el brazo de tres marcas y de cinco estilos dejaba la suite verde.
    #[test]
    fn se_leen_las_seis_marcas_y_los_siete_estilos() {
        let dir = PanelDir::create().expect("crear el directorio");
        let panel = panel_en(
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

        let marcas: Vec<Option<Mark>> = panel.lines[..6].iter().map(|l| l.mark).collect();
        assert_eq!(
            marcas,
            [
                Some(Mark::Done),
                Some(Mark::Current),
                Some(Mark::Pending),
                Some(Mark::Wait),
                Some(Mark::Block),
                Some(Mark::Info)
            ]
        );

        let estilos: Vec<Option<Style>> = panel.lines[6..].iter().map(|l| l.style).collect();
        assert_eq!(
            estilos,
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
        let panel = panel_en(
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
        let hueco = r#"{"lines":["ANCHO"]}"#.len() - "ANCHO".len();
        let justo = "x".repeat(MAX_BYTES as usize - hueco);

        let cabe = panel_en(&dir, &format!(r#"{{"lines":["{justo}"]}}"#)).expect("justo cabe");
        assert_eq!(cabe.lines.len(), 1);

        let fallo =
            panel_en(&dir, &format!(r#"{{"lines":["{justo}x"]}}"#)).expect_err("uno más, no");
        assert!(matches!(fallo, Problem::TooBig), "fue {fallo:?}");
    }

    #[test]
    fn el_silencio_justo_igual_al_tope_todavia_no_es_rancio() {
        let panel = Panel {
            stale_after: Duration::from_secs(60),
            written_at: Some(SystemTime::UNIX_EPOCH),
            ..panel_vacio()
        };

        assert!(!panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(60)));
        assert!(panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(61)));
    }

    #[test]
    fn un_panel_callado_desde_hace_rato_esta_rancio() {
        let panel = Panel {
            stale_after: Duration::from_secs(60),
            written_at: Some(SystemTime::UNIX_EPOCH),
            ..panel_vacio()
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
            ..panel_vacio()
        };

        assert!(!panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(60)));
    }

    #[test]
    fn sin_saber_cuando_se_escribio_no_hay_rancio_que_medir() {
        assert!(!panel_vacio().is_stale_at(SystemTime::now()));
    }

    fn panel_vacio() -> Panel {
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
    mod cuenta {
        use std::cell::Cell;

        thread_local! {
            pub static VIVOS: Cell<usize> = const { Cell::new(0) };
            pub static PICO: Cell<usize> = const { Cell::new(0) };
        }
    }

    struct Contador;

    /// Suma o resta lo vivo y se queda con el máximo.
    ///
    /// `try_with` y no `with`: al deshacerse un hilo, sus variables locales ya no están, y ahí todavía
    /// se libera memoria. Con `with` eso sería un pánico dentro del asignador.
    fn anotar(delta: isize) {
        let _ = cuenta::VIVOS.try_with(|vivos| {
            let ahora = vivos.get().saturating_add_signed(delta);
            vivos.set(ahora);
            let _ = cuenta::PICO.try_with(|pico| pico.set(pico.get().max(ahora)));
        });
    }

    unsafe impl std::alloc::GlobalAlloc for Contador {
        unsafe fn alloc(&self, plano: std::alloc::Layout) -> *mut u8 {
            let puntero = unsafe { std::alloc::System.alloc(plano) };
            if !puntero.is_null() {
                anotar(plano.size() as isize);
            }
            puntero
        }

        unsafe fn dealloc(&self, puntero: *mut u8, plano: std::alloc::Layout) {
            anotar(-(plano.size() as isize));
            unsafe { std::alloc::System.dealloc(puntero, plano) }
        }

        unsafe fn realloc(
            &self,
            puntero: *mut u8,
            plano: std::alloc::Layout,
            nuevo: usize,
        ) -> *mut u8 {
            let movido = unsafe { std::alloc::System.realloc(puntero, plano, nuevo) };
            if !movido.is_null() {
                anotar(nuevo as isize - plano.size() as isize);
            }
            movido
        }
    }

    #[global_allocator]
    static ASIGNADOR: Contador = Contador;

    /// Ejecuta el trabajo y devuelve, además de lo que produjo, **cuánto llegó a tener vivo de más**.
    fn midiendo<T>(trabajo: impl FnOnce() -> T) -> (T, usize) {
        let base = cuenta::VIVOS.get();
        cuenta::PICO.set(base);

        let hecho = trabajo();

        (hecho, cuenta::PICO.get().saturating_sub(base))
    }
}
