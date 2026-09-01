//! Lo que quien usa el terminal puede cambiarle, y de dónde se lee.
//!
//! ## Este fichero lo escribe una persona, y eso decide casi todo lo de aquí
//!
//! La disposición de la ventana la escribe la aplicación, así que un documento a medias solo puede venir
//! de un error propio y **D16** decidió no abrirlo a medias: adivinar media disposición es peor que no
//! restaurarla. Aquí es al revés. **Lo teclea una persona que se equivoca**, y para cada línea hay un
//! repuesto exacto —el valor de fábrica—, así que **se tolera por línea**: la que no se entiende se
//! ignora, se dice cuál es, y el resto se aplica.
//!
//! No es una excepción a D16: es el lado que D16 ya contempla cuando contrapone el contrato del panel,
//! *«que lo escribe un tercero a mano»*. Y coincide con lo que hacen los cuatro terminales contra los que
//! este proyecto compite —WezTerm, Kitty, Alacritty y Ghostty avisan y siguen con lo de fábrica—, pero esa
//! coincidencia es una confirmación, no la razón.
//!
//! ## Y por eso el formato es texto plano y no JSON
//!
//! La tolerancia por línea es el requisito, y **en JSON una coma olvidada impide construir el documento
//! entero**. El contrato del panel es JSON y tolera por campo, pero puede permitírselo porque **lo genera
//! un programa**; aquí la coma olvidada es el error más común que existe. En texto plano ese fallo global
//! no puede ocurrir: el peor caso es una línea. Y admite comentarios, sin los cuales un fichero de
//! configuración no se explica a sí mismo.

use std::path::{Path, PathBuf};

use eframe::egui::Color32;

/// El nombre del fichero, al lado de la disposición y no dentro.
pub const FILE: &str = "config";

/// El valor que desata un atajo, **y el único que el terminal publica como palabra**.
///
/// **Vive una vez porque estaba escrito tres**: las dos ramas que lo reconocen y el aviso que le dice a
/// quien se equivoca qué escribir. Cambiar el token en el intérprete y olvidar el aviso deja al terminal
/// recomendando una palabra que él mismo rechaza, y la prueba que cubre ese aviso afirma sobre otra parte
/// de la frase, así que no lo vería.
const OFF: &str = "none";

/// El techo del fichero, aplicado **antes** de interpretar.
///
/// **Es literalmente el de la disposición, no una copia con el mismo número.** Un comentario que diga «el
/// mismo que aquél» junto a una constante propia es una promesa que nada obliga a cumplir: el día que una
/// de las dos cambie, el comentario miente y nadie se entera.
use crate::session::MAX_BYTES;

/// Cuántas líneas malas se dicen antes de callar.
///
/// **El aviso aquí ES el producto, no una nota al margen.** La disposición avisa una vez por clase de
/// problema porque la escribe la aplicación y el aviso solo informa; aquí, con dos colores mal escritos,
/// oír solo uno hace que se arregle la mitad y se vuelva a abrir. Pero un fichero pegado del sitio
/// equivocado tampoco debe llenar la salida de error, así que hay tope **y se dice cuántos quedaron**.
const MAX_WARNINGS: usize = 20;

/// La versión del formato que este lector entiende.
///
/// **Es opcional, al contrario que en la disposición, y la razón es la que D16 da para exigirla allí.**
/// Allí el fichero **congela la forma de un árbol**, así que un lector viejo malinterpretaría un fichero
/// nuevo. Aquí el formato es plano: una clave que un lector no conoce **se ignora y se dice**, y una que
/// falta toma su valor de fábrica. No hay forma que congelar, así que exigir una línea ritual a quien
/// escribe el fichero no protegería de nada.
///
/// Lo que sí hace falta es la mitad que sí protege: **una versión MAYOR que ésta no se aplica**, por si un
/// día el formato cambia de manera incompatible. Sin fichero de por medio no hay nada que decidir.
const VERSION: u64 = 1;

/// Lo que le pasa al fichero **entero**, cuando no hay nada que aplicar.
///
/// Una línea mala no está aquí: eso es un [`Warning`], y el resto del fichero se aplica igual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigProblem {
    /// No hay fichero. **Es el caso normal** y no se avisa de él.
    Absent,
    /// Hay un enlace con ese nombre y no apunta a nada.
    BrokenLink,
    /// Hay algo con ese nombre y no es un fichero.
    NotAFile,
    /// No se pudo leer.
    Unreadable(String),
    /// Se pasa del techo.
    TooBig,
    /// No es texto.
    NotText,
    /// Dice ser de una versión que este terminal no conoce.
    Version(u64),
}

impl ConfigProblem {
    /// Si vale la pena decirlo por la salida de error.
    ///
    /// **Que no haya fichero es el caso normal, no un problema**, y decirlo en cada arranque de quien no
    /// configura nada sería ruido puro. La regla vive **en el tipo** y no en quien llama, igual que en el
    /// módulo hermano: es la única forma de que dos sitios que lo usen no dejen de coincidir.
    pub fn warrants_warning(&self) -> bool {
        !matches!(self, Self::Absent)
    }
}

impl std::fmt::Display for ConfigProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => write!(f, "there is no settings file"),
            Self::BrokenLink => write!(f, "the settings link points nowhere"),
            Self::NotAFile => write!(f, "that is not a file"),
            Self::Unreadable(why) => write!(f, "it could not be read: {why}"),
            Self::TooBig => {
                write!(f, "it is over the {} KiB that get read", MAX_BYTES / 1024)
            }
            Self::NotText => write!(f, "it is not text"),
            Self::Version(which) => write!(
                f,
                "it claims to be version {which} and this terminal understands up to {VERSION}"
            ),
        }
    }
}

/// Lo que le pasa a **una línea**, con dónde está.
///
/// **Lleva el número de línea porque sin él el aviso no sirve**: «un color mal escrito» en un fichero de
/// cuarenta líneas obliga a repasarlas todas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub line: usize,
    pub missing: Missing,
}

/// Qué le falta a una línea para poder aplicarse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// No tiene la forma `key = value`.
    NoEquals,
    /// Tiene el igual pero le falta la clave o el valor.
    Incomplete,
    /// La clave no es ninguna de las que este terminal entiende.
    UnknownKey(String),
    /// El valor no vale para esa clave.
    InvalidValue { key: String, value: String },
    /// Esa clave ya estaba puesta más arriba; gana ésta.
    Repeated(String),
    /// El acorde de esa acción lo decide el escritorio: solo se puede apagar.
    OnlyTurnsOff(String),
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: ", self.line)?;
        match &self.missing {
            Missing::NoEquals => write!(f, "it is not of the form 'key = value'"),
            Missing::Incomplete => write!(f, "the key or the value is missing"),
            Missing::UnknownKey(key) => write!(f, "'{key}' is not a setting this terminal knows"),
            Missing::InvalidValue { key, value } => {
                write!(f, "'{}' is not valid for '{key}'", trimmed(value))
            }
            Missing::Repeated(key) => write!(f, "'{key}' was already set above; this one wins"),
            Missing::OnlyTurnsOff(key) => write!(
                f,
                "the chord for '{key}' is the desktop's and cannot be changed; \
                 the only thing you can do is turn it off with '{OFF}'"
            ),
        }
    }
}

/// Lo que cabe de un valor en un aviso.
///
/// **El valor sale tal cual del fichero, y un fichero puede traer una línea de cien mil caracteres o unos
/// códigos de escape.** No es superficie de ataque —el fichero es de quien usa el terminal— pero las reglas
/// duras de este proyecto contemplan el fichero **de origen ajeno**, que aquí es un `config` copiado de
/// cualquier sitio. Recortar cuesta una línea y hace el aviso legible.
fn trimmed(value: &str) -> String {
    const FITS: usize = 60;
    if value.chars().count() <= FITS {
        return value.escape_debug().to_string();
    }
    let short: String = value.chars().take(FITS).collect();
    format!("{}…", short.escape_debug())
}

/// Los colores que el fichero pide, cada uno **solo si lo pide**.
///
/// Son **veintidós**: las dieciséis de la paleta y los seis propios.
///
/// **Cada campo es opcional a propósito**: lo que el fichero no diga se queda como está de fábrica, que es
/// lo que hace que un fichero con una sola línea funcione.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Colors {
    /// Las dieciséis de la paleta que pide el programa de dentro.
    pub palette: [Option<Color32>; 16],
    pub background: Option<Color32>,
    pub foreground: Option<Color32>,
    pub cursor: Option<Color32>,
    /// La línea que separa dos consolas.
    pub divider: Option<Color32>,
    /// Con qué se señala la consola a la que va el teclado.
    pub focus: Option<Color32>,
    /// El fondo del panel de estado.
    pub panel: Option<Color32>,
}

/// Lo que el fichero pide, ya entendido.
// **`Copy` se cae aquí**: los atajos son una lista, y una lista no se copia sola. El tipo se pasa por
// referencia, que es como ya lo pedían sus dos únicos consumidores.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub colors: Colors,
    /// El cuerpo de la letra, en puntos. `None` si el fichero no lo pide.
    ///
    /// **Se llama igual que en la ventana y distinto que en el fichero, a propósito.** `size` es la clave
    /// que la gente teclea y es contrato publicado; `font_size` es como el resto del código llama a este
    /// número. La traducción ocurre **una vez**, donde se interpreta la clave, y no en cada salto.
    pub font_size: Option<f32>,
    /// Los atajos que el fichero cambia: la acción y su acorde nuevo, o `None` para desatarla.
    ///
    /// **Una lista y no una tabla ya resuelta**, porque el orden importa: «gana la última» solo se puede
    /// decidir aplicándolas en el orden en que se escribieron.
    pub shortcuts: Vec<(crate::keyboard::Action, Option<crate::keyboard::Chord>)>,
    /// Si el fichero apagó el atajo de copiar.
    ///
    /// **Aparte de la lista, porque copiar no tiene acorde que mover.** Su acorde lo resuelve el
    /// escritorio antes de que llegue aquí —y sin decir si había Mayúsculas—, así que lo único que se
    /// puede hacer con él es dejar de atenderlo, que es lo que devuelve `Ctrl+Mayúsculas+C` a `vim`.
    pub copy: Option<bool>,
}

/// Lo menos y lo más que se admite como cuerpo de letra.
///
/// **No es una manía: fuera de este rango la ventana deja de ser un terminal.** Con dos puntos no se lee
/// nada y con doscientos caben tres columnas, y el reparto de las divisiones tiene mínimos que dejarían de
/// poder cumplirse. Los extremos son generosos a propósito —hay quien trabaja a 8 y quien presenta a 40—:
/// lo que se rechaza es lo que solo puede ser una errata.
const MIN_FONT_SIZE: f32 = 4.0;
const MAX_FONT_SIZE: f32 = 96.0;

/// Las líneas que no se pudieron aplicar: las que se guardan **y cuántas hubo**.
///
/// **Los dos números, y no solo la lista, porque el techo miente si se calla.** Guardar veinte y no decir
/// que había cuarenta haría creer que el fichero está bien a partir del vigésimo.
///
/// **No imprime: decide.** Quién lo dice por la salida de error es cosa de quien llama, igual que en el
/// módulo hermano — imprimir no se puede observar desde dentro del proceso, así que lo único que una
/// prueba puede atar es la decisión.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Warnings {
    kept: Vec<Warning>,
    total: usize,
}

impl Warnings {
    fn push(&mut self, warning: Warning) {
        self.total += 1;
        if self.kept.len() < MAX_WARNINGS {
            self.kept.push(warning);
        }
    }

    /// Los que se guardaron, que son los primeros [`MAX_WARNINGS`].
    pub fn kept(&self) -> &[Warning] {
        &self.kept
    }

    /// Cuántas líneas dieron problema en total, contando las que no se guardaron.
    pub fn total(&self) -> usize {
        self.total
    }

    /// Cuántas se quedaron sin listar por el techo.
    pub fn unlisted(&self) -> usize {
        self.total - self.kept.len()
    }
}

/// Dónde vive el fichero, leyendo el entorno.
///
/// Envoltura de una línea, con el trabajo en [`path_from`], que es el patrón del módulo hermano y por su
/// misma razón: fijar estas variables desde una prueba es comportamiento indefinido en un proceso con
/// hilos, así que la decisión tiene que poder probarse sin tocarlas.
pub fn path() -> Option<PathBuf> {
    path_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// Dónde vive el fichero, con los dos valores dados.
pub fn path_from(
    config: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    crate::session::directory_from(config, home).map(|directory| directory.join(FILE))
}

/// Lee el fichero de esa ruta. Devuelve lo que se entendió **y** lo que no.
///
/// **El orden es el del molde de la disposición**: pedir los datos, comprobar que es un fichero, **abrir y
/// leer con el tope puesto**, aplicar el techo sobre los bytes leídos, y solo entonces mirar si es texto.
pub fn read(path: &Path) -> Result<(Settings, Warnings), ConfigProblem> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        // **Un enlace roto NO es «no hay fichero», y la diferencia se nota.** Quien creó su configuración y
        // la movió de sitio se merece que se lo digan; el silencio es para quien no ha escrito nada.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(match std::fs::symlink_metadata(path) {
                Ok(_) => ConfigProblem::BrokenLink,
                Err(_) => ConfigProblem::Absent,
            });
        }
        Err(error) => return Err(ConfigProblem::Unreadable(error.to_string())),
    };
    if !metadata.is_file() {
        return Err(ConfigProblem::NotAFile);
    }
    let file = std::fs::File::open(path).map_err(|f| ConfigProblem::Unreadable(f.to_string()))?;
    let raw = crate::session::read_capped(file, metadata.len())
        .map_err(|error| ConfigProblem::Unreadable(error.to_string()))?;
    // **El techo va sobre los bytes LEÍDOS, no sobre los que los datos del fichero declaran.** No son lo
    // mismo: un fichero de `/proc` declara cero y entrega ciento setenta y cinco bytes, comprobado
    // ejecutándolo. Una versión anterior leía entero con `fs::read` y comparaba después, con un comentario
    // que decía «abrir» donde no se abría nada — el techo existía y **no acotaba la reserva**, que es
    // exactamente lo que su propia razón dice evitar. Lo cazó una revisión.
    if raw.len() > MAX_BYTES {
        return Err(ConfigProblem::TooBig);
    }
    let text = String::from_utf8(raw).map_err(|_| ConfigProblem::NotText)?;
    interpret(&text)
}

/// Lo que una línea resulta ser.
///
/// **Se clasifica UNA vez.** La primera versión devolvía «nada» tanto para una línea vacía como para una
/// mal escrita, y quien llamaba tenía que volver a mirarla para saber de cuál de las dos se trataba: la
/// misma decisión tomada dos veces, en dos sitios que podían dejar de coincidir.
enum ConfigLine<'a> {
    /// Vacía, espacios, o un comentario.
    Blank,
    /// Una clave y su valor, ya sin espacios.
    Pair(&'a str, &'a str),
    /// Dice algo, pero no con la forma que hace falta.
    Bad(Missing),
}

/// Interpreta el texto línea a línea.
pub fn interpret(text: &str) -> Result<(Settings, Warnings), ConfigProblem> {
    let mut settings = Settings::default();
    let mut warnings = Warnings::default();
    let mut already_set: Vec<String> = Vec::new();

    // **La marca de orden de bytes se quita, y es el fallo más cruel que este fichero podía tener.** La
    // meten los editores de Windows y algunas páginas al copiar; queda pegada a la primera clave, y el
    // aviso resultante señala algo que **en pantalla se ve exactamente igual** que la clave correcta:
    // ««color.1» no es un ajuste conocido». Comprobado ejecutándolo.

    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    for (index, line_text) in text.lines().enumerate() {
        let line = index + 1;
        let (key, value) = match classify(line_text) {
            ConfigLine::Blank => continue,
            ConfigLine::Bad(missing) => {
                warnings.push(Warning { line, missing });
                continue;
            }
            ConfigLine::Pair(key, value) => (key, value),
        };

        // **La versión se mira aquí y no antes**, porque es una clave más: puede ir en cualquier línea, y
        // que falte es lo normal.
        // **La versión no entra en la contabilidad de repetidas, y no es un olvido.** «Gana ésta» significa
        // que esta línea pisó el valor de otra; la versión **no guarda ningún valor**, es un veto que se
        // comprueba y se olvida. Decir que gana algo sería mentir dos veces cuando además su valor no se
        // entiende, que es lo que hacía la primera versión de este arreglo. Lo cazó una revisión.
        if key == "version" {
            match value.parse::<u64>() {
                Ok(which) if which > VERSION => return Err(ConfigProblem::Version(which)),
                Ok(_) => {}
                Err(_) => warnings.push(Warning {
                    line,
                    missing: Missing::InvalidValue {
                        key: key.to_owned(),
                        value: value.to_owned(),
                    },
                }),
            }
            continue;
        }
        if let Err(missing) = apply(&mut settings, key, value) {
            warnings.push(Warning { line, missing });
            // **Una línea que no se aplica no cuenta como puesta**, así que no se avisa de repetida por
            // ella: decir «gana ésta» de una línea que no gana nada es un aviso que miente, y encima
            // contradice al de al lado. Comprobado ejecutándolo antes de arreglarlo.
            continue;
        }

        // **El aviso de repetida va DESPUÉS de aplicar, y por eso dice la verdad.** Se emite solo cuando
        // esta línea de verdad se ha quedado con el valor.
        if already_set.iter().any(|already| already == key) {
            warnings.push(Warning {
                line,
                missing: Missing::Repeated(key.to_owned()),
            });
        } else {
            already_set.push(key.to_owned());
        }
    }

    Ok((settings, warnings))
}

/// Mete el valor donde le toque, si la clave y el valor valen.
fn apply(settings: &mut Settings, key: &str, value: &str) -> Result<(), Missing> {
    if let Some(name) = key.strip_prefix("key.") {
        // **Copiar sigue teniendo su clave, y solo acepta `none`.** Es la clave que este terminal ya
        // publicó, y quien la escribió para devolverle el acorde a `vim` no tiene por qué enterarse de
        // que por dentro dejó de ser un acorde. Lo que no se puede es reasignarlo, y eso se dice.
        if name.eq_ignore_ascii_case("copy") {
            if !value.eq_ignore_ascii_case(OFF) {
                return Err(Missing::OnlyTurnsOff(key.to_owned()));
            }
            settings.copy = Some(false);
            return Ok(());
        }
        let action = crate::keyboard::Action::from_written(name)
            .ok_or_else(|| Missing::UnknownKey(key.to_owned()))?;
        // **`none` desata la acción, y es la mitad que hace útil todo esto**: es lo que devuelve un
        // acorde al programa de dentro cuando `vim` o `tmux` lo quieren para ellos.
        let chord = if value.eq_ignore_ascii_case(OFF) {
            None
        } else {
            Some(crate::keyboard::Chord::from_written(value).ok_or_else(|| {
                Missing::InvalidValue {
                    key: key.to_owned(),
                    value: value.to_owned(),
                }
            })?)
        };
        settings.shortcuts.push((action, chord));
        return Ok(());
    }
    if key == "size" {
        let amount = value
            .parse::<f32>()
            .ok()
            .filter(|c| c.is_finite() && (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(c))
            .ok_or_else(|| Missing::InvalidValue {
                key: key.to_owned(),
                value: value.to_owned(),
            })?;
        settings.font_size = Some(amount);
        return Ok(());
    }
    let colors = &mut settings.colors;
    let slot: &mut Option<Color32> = match key {
        "background" => &mut colors.background,
        "foreground" => &mut colors.foreground,
        "cursor" => &mut colors.cursor,
        "divider" => &mut colors.divider,
        "focus" => &mut colors.focus,
        "panel" => &mut colors.panel,
        // **El índice son dígitos y nada más.** Dejarlo en manos del intérprete de números admitía
        // `color.007` y `color.+1` —los dos aplicaban un color— mientras `color.-1` se rechazaba: tres
        // formas de escribir lo mismo con dos resultados distintos, ninguna documentada. Lo cazó una
        // revisión ejecutándolas.
        _ => match key.strip_prefix("color.") {
            Some(index)
                if index.chars().all(|c| c.is_ascii_digit())
                    && index
                        .parse::<usize>()
                        .is_ok_and(|n| n < colors.palette.len()) =>
            {
                // **Un índice fuera de la paleta es una clave desconocida, no un valor malo**: lo que
                // está mal escrito es el nombre del ajuste, y decir «el valor no vale» mandaría a mirar
                // el color.
                &mut colors.palette[index.parse::<usize>().unwrap_or_default()]
            }
            _ => return Err(Missing::UnknownKey(key.to_owned())),
        },
    };
    *slot = Some(color_from(value).ok_or_else(|| Missing::InvalidValue {
        key: key.to_owned(),
        value: value.to_owned(),
    })?);
    Ok(())
}

/// Un color escrito como `#rrggbb`, en mayúsculas o en minúsculas.
fn color_from(value: &str) -> Option<Color32> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |from: usize| u8::from_str_radix(&digits[from..from + 2], 16).ok();
    Some(Color32::from_rgb(channel(0)?, channel(2)?, channel(4)?))
}

/// Qué es esta línea.
///
/// **El comentario es de línea completa, y no puede ir al final de un valor.** No es una simplificación:
/// la almohadilla es **a la vez** el carácter de comentario y el prefijo de un color, así que un
/// comentario al final del valor parte `color.1 = #ff0000` por la mitad y deja la clave sin valor.
/// Comprobado escribiéndolo: la primera versión de este intérprete se comía todos los colores.
///
/// Es exactamente lo que hacen Kitty y Ghostty, que usan este mismo formato, y por la misma razón.
fn classify(line: &str) -> ConfigLine<'_> {
    let trimmed_line = line.trim();
    if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
        return ConfigLine::Blank;
    }
    let Some((key, value)) = trimmed_line.split_once('=') else {
        return ConfigLine::Bad(Missing::NoEquals);
    };
    let (key, value) = (key.trim(), value.trim());
    if key.is_empty() || value.is_empty() {
        // **Se distingue de «no tiene igual» a propósito**: quien escribió `background =` no se olvidó del
        // igual, se olvidó del color, y decirle lo primero le manda a mirar donde no es.
        return ConfigLine::Bad(Missing::Incomplete);
    }
    ConfigLine::Pair(key, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que devuelve interpretar un texto que se entiende entero.
    fn parsed(text: &str) -> (Settings, Warnings) {
        interpret(text).expect("el texto de la prueba no debería tumbar el fichero entero")
    }

    #[test]
    fn una_linea_mala_no_se_lleva_por_delante_las_buenas() {
        // **Es la razón de ser del formato, y lo que separa este fichero del de la disposición.** Aquel lo
        // escribe la aplicación y no se abre a medias; éste lo teclea una persona, y tumbar los veintidós
        // colores porque uno está mal escrito la obligaría a encontrar el error a ciegas.
        let (settings, _) = parsed("color.1 = #ff0000\ncolor.2 = azul celeste\ncolor.3 = #0000ff");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "la línea BUENA de antes de la mala no se aplicó"
        );
        assert_eq!(
            settings.colors.palette[3],
            Some(Color32::from_rgb(0, 0, 0xFF)),
            "la línea buena de DESPUÉS de la mala no se aplicó: el intérprete se paró en el error"
        );
        assert_eq!(
            settings.colors.palette[2], None,
            "el color mal escrito se aplicó igualmente"
        );
    }

    #[test]
    fn la_linea_mala_se_dice_con_su_numero() {
        // **Sin el número, el aviso no sirve**: «un color mal escrito» en un fichero de cuarenta líneas
        // obliga a repasarlas todas. Y sin aviso ninguno, quien escribió la línea creerá que el terminal
        // ignora lo que le pide.
        let (_, warnings) = parsed("# un comentario\n\ncolor.1 = morado\n");

        assert_eq!(warnings.kept().len(), 1, "no se avisó de la línea mala");
        assert_eq!(
            warnings.kept()[0].line,
            3,
            "el aviso señala la línea equivocada: se cuentan también las vacías y los comentarios, \
             porque el número tiene que coincidir con lo que se ve en el editor"
        );
        assert!(
            warnings.kept()[0].to_string().contains("morado"),
            "el aviso no dice qué valor no se entendió: «{}»",
            warnings.kept()[0]
        );
    }

    #[test]
    fn un_fichero_sin_nada_configurado_deja_el_tema_de_fabrica() {
        // **AC5 por su lado más barato**, y el que sostiene que este hito no rompe a nadie: lo que el
        // fichero no diga se queda como está.
        let (settings, warnings) = parsed("# solo comentarios\n\n   \n");

        assert_eq!(
            settings,
            Settings::default(),
            "un fichero sin ajustes cambió algo"
        );
        assert_eq!(
            warnings.total(),
            0,
            "una línea vacía o de comentario se contó como problema"
        );
        assert_eq!(
            crate::theme::Theme::from_colors(&settings.colors)
                .color(term_core::TermColor::Indexed(1)),
            crate::theme::Theme::default().color(term_core::TermColor::Indexed(1)),
            "sin nada configurado el tema dejó de ser el de fábrica"
        );
    }

    #[test]
    fn un_color_no_se_confunde_con_un_comentario() {
        // **La almohadilla es a la vez el comentario y el prefijo de un color**, y la primera versión de
        // este intérprete admitía comentarios al final de la línea: se comía TODOS los colores, porque
        // cortaba `color.4 = #123456` justo antes del valor. Lo destapó esta suite al primer intento.
        //
        // Por eso el comentario es de línea completa, como en Kitty y en Ghostty.
        let (settings, warnings) = parsed("# el azul de siempre\ncolor.4 = #123456");

        assert_eq!(
            settings.colors.palette[4],
            Some(Color32::from_rgb(0x12, 0x34, 0x56)),
            "el valor de un color se perdió por empezar con almohadilla"
        );
        assert_eq!(
            warnings.total(),
            0,
            "la línea de comentario o la del color se contaron como problema"
        );
    }

    #[test]
    fn una_clave_repetida_gana_la_ultima_y_se_dice() {
        // **Las dos mitades.** Que gane la última es lo que hace que pegar un tema encima de otro funcione
        // como se espera; decirlo es lo que impide que alguien pase media hora cambiando una línea que otra
        // de más abajo está pisando.
        let (settings, warnings) = parsed("color.1 = #ff0000\ncolor.1 = #00ff00");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0, 0xFF, 0)),
            "ganó la primera: pegar un tema encima de otro no haría lo que se espera"
        );
        assert_eq!(warnings.kept().len(), 1, "la clave repetida no se dijo");
        assert!(
            warnings.kept()[0].to_string().contains("color.1"),
            "el aviso no dice qué clave estaba repetida: «{}»",
            warnings.kept()[0]
        );
    }

    #[test]
    fn un_color_de_la_paleta_fuera_de_rango_no_se_traga_ni_desborda() {
        // La paleta tiene dieciséis huecos y el índice lo escribe una persona. Un `color.99` tiene que
        // avisar, no aplicarse en otro sitio ni salirse del vector.
        let (settings, warnings) =
            parsed("color.16 = #ffffff\ncolor.999 = #ffffff\ncolor.-1 = #ffffff");

        assert_eq!(
            settings,
            Settings::default(),
            "un índice fuera de la paleta cambió algo"
        );
        assert_eq!(
            warnings.total(),
            3,
            "no se avisó de los tres índices imposibles"
        );
    }

    #[test]
    fn un_color_mal_escrito_no_cuela_por_parecerse() {
        // Los cuatro errores que de verdad se cometen al escribir un color a mano.
        for value in ["ff0000", "#ff00", "#ffgg00", "#ff00000"] {
            let (settings, warnings) = parsed(&format!("background = {value}"));
            assert_eq!(
                settings.colors.background, None,
                "«{value}» se aceptó como color y no lo es"
            );
            assert_eq!(warnings.total(), 1, "«{value}» no dio aviso");
        }
    }

    #[test]
    fn una_version_mas_nueva_no_se_aplica_a_medias() {
        // **Es la única mitad de la versión que protege de algo**, y por eso es la única que existe: un
        // fichero de un formato futuro podría querer decir con una clave conocida algo distinto, así que
        // aplicar las que se entienden sería peor que no aplicar nada.
        assert_eq!(
            interpret("version = 99\ncolor.1 = #ff0000"),
            Err(ConfigProblem::Version(99)),
            "un fichero de una versión futura se aplicó igualmente"
        );
        let (settings, warnings) = parsed("version = 1\ncolor.1 = #ff0000");
        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "la versión conocida impidió aplicar el resto"
        );
        assert_eq!(
            warnings.total(),
            0,
            "declarar la versión conocida dio un aviso"
        );
    }

    #[test]
    fn sin_declarar_version_el_fichero_se_lee_igual() {
        // **Es lo que separa este formato del de la disposición**: allí la versión es obligatoria porque el
        // fichero congela la forma de un árbol. Aquí es plano, y exigirle una línea ritual a quien lo
        // escribe no protegería de nada. Todos los demás casos de esta suite lo dan por hecho; éste lo ata.
        let (settings, warnings) = parsed("color.1 = #ff0000");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0))
        );
        assert_eq!(
            warnings.total(),
            0,
            "no declarar la versión se contó como problema"
        );
    }

    #[test]
    fn los_avisos_tienen_techo_y_se_dice_cuantos_quedaron_fuera() {
        // **Un fichero pegado del sitio equivocado no debe llenar la salida de error**, pero callar los de
        // más sin decir cuántos eran haría creer que a partir del vigésimo el fichero está bien.
        let text: String = (0..MAX_WARNINGS + 7)
            .map(|n| format!("desconocida{n} = 1\n"))
            .collect();
        let (_, warnings) = parsed(&text);

        assert_eq!(
            warnings.kept().len(),
            MAX_WARNINGS,
            "se guardaron más avisos que el techo"
        );
        assert_eq!(
            warnings.total(),
            MAX_WARNINGS + 7,
            "la cuenta se quedó en los guardados, así que el resto desaparece sin rastro"
        );
        assert_eq!(
            warnings.unlisted(),
            7,
            "no se sabe cuántas quedaron sin listar"
        );
    }

    #[test]
    fn una_linea_con_contenido_y_sin_igual_avisa_y_una_vacia_no() {
        let (_, warnings) = parsed("esto no es un ajuste\n\n   \n# ni esto");

        assert_eq!(
            warnings.total(),
            1,
            "o no se avisó de la línea sin «=», o se avisó de las vacías y los comentarios"
        );
    }

    #[test]
    fn no_se_dice_que_gana_una_linea_que_no_gana_nada() {
        // **El aviso mentía, y lo destapó una revisión ejecutándolo.** Con la clave repetida y el segundo
        // valor mal escrito salían DOS avisos contradictorios en la misma línea: «gana ésta» y, justo
        // debajo, que ese valor no vale. O sea que no gana: se queda la primera.
        let (settings, warnings) = parsed("color.1 = #ff0000\ncolor.1 = morado");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "una línea con el valor mal escrito borró el valor bueno de antes"
        );
        assert_eq!(
            warnings.total(),
            1,
            "se avisó dos veces de la misma línea, y una de las dos dice lo contrario que la otra: {:?}",
            warnings.kept()
        );
        assert!(
            warnings.kept()[0].to_string().contains("morado"),
            "el aviso que quedó no es el que explica qué pasa: «{}»",
            warnings.kept()[0]
        );
    }

    #[test]
    fn de_la_version_no_se_dice_que_gane_porque_no_gana_nada() {
        // **«Gana ésta» significa que esta línea pisó el valor de otra, y la versión no guarda ningún
        // valor**: es un veto que se comprueba y se olvida. Un arreglo anterior la metió en esa
        // contabilidad y produjo el aviso más absurdo posible — «gana ésta» sobre una línea cuyo valor
        // tampoco se entendía, o sea que ni ganaba ni se aplicaba. Lo cazó una revisión ejecutándolo.
        let (_, warnings) = parsed("version = 1\nversion = 1");
        assert_eq!(
            warnings.total(),
            0,
            "se dijo que una versión repetida «gana», y no gana nada: {:?}",
            warnings.kept()
        );

        let (_, warnings) = parsed("version = uno\nversion = uno");
        assert_eq!(
            warnings.total(),
            2,
            "cada versión ilegible tiene que avisar una vez y solo una: {:?}",
            warnings.kept()
        );
        assert!(
            warnings
                .kept()
                .iter()
                .all(|a| !matches!(a.missing, Missing::Repeated(_))),
            "se dijo «gana ésta» de una línea cuyo valor ni siquiera se entendió: {:?}",
            warnings.kept()
        );
    }

    #[test]
    fn el_tamano_de_letra_se_lee_y_la_clave_vieja_ya_no_vale() {
        let (settings, warnings) = parsed("size = 18");
        assert_eq!(settings.font_size, Some(18.0), "«size» no se entendió");
        assert_eq!(warnings.total(), 0, "«size» dio aviso");

        let (settings, _) = parsed("size = 12.5");
        assert_eq!(
            settings.font_size,
            Some(12.5),
            "un tamaño con decimales no se entendió"
        );

        // **No hay tabla de alias, y esto es lo único que lo ata.** D27 decide que las claves pasan a
        // inglés sin compatibilidad porque nadie tiene el terminal instalado; sin esta afirmación, dejar
        // colada una rama que siga aceptando la clave vieja no lo nota nadie, y la deuda que la decisión
        // se ahorró vuelve por la puerta de atrás.
        for old in ["tamaño", "tamano"] {
            let (settings, warnings) = parsed(&format!("{old} = 18"));
            assert_eq!(
                settings.font_size, None,
                "«{old}» se sigue aceptando: hay tabla de alias donde D27 dice que no la hay"
            );
            assert_eq!(
                warnings.total(),
                1,
                "«{old}» no dio aviso de clave desconocida"
            );
        }
    }

    #[test]
    fn un_tamano_imposible_no_deja_la_ventana_sin_rejilla() {
        // **Fuera de rango la ventana deja de ser un terminal**: con dos puntos no se lee nada, con
        // doscientos caben tres columnas, y el reparto de las divisiones tiene mínimos que dejarían de
        // poder cumplirse. Un cero o un negativo son peores: la celda mediría cero y la rejilla saldría
        // de dividir por él.
        for value in ["0", "-14", "1", "500", "nan", "inf", "grande"] {
            let (settings, warnings) = parsed(&format!("size = {value}"));
            assert_eq!(
                settings.font_size, None,
                "«{value}» se aceptó como cuerpo de letra"
            );
            assert_eq!(warnings.total(), 1, "«{value}» no dio aviso");
        }
        // Y los extremos que sí valen, para que el rango no se cierre de más: hay quien trabaja a 8 y
        // quien presenta a 40.
        for value in ["4", "8", "40", "96"] {
            let (settings, _) = parsed(&format!("size = {value}"));
            assert!(
                settings.font_size.is_some(),
                "«{value}» se rechazó y es un tamaño de trabajo"
            );
        }
    }

    #[test]
    fn el_indice_de_la_paleta_son_digitos_y_nada_mas() {
        // **La asimetría que había no se podía explicar**: `color.+1` aplicaba un color y `color.-1` se
        // rechazaba, las dos por dejarle la decisión al intérprete de números. Lo cazó una revisión
        // ejecutándolas.
        for key in ["color.+1", "color.-1", "color. 1", "color.uno", "color."] {
            let (settings, warnings) = parsed(&format!("{key} = #ff0000"));
            assert_eq!(
                settings,
                Settings::default(),
                "«{key}» aplicó un color, y no es una forma que nadie haya documentado"
            );
            assert_eq!(warnings.total(), 1, "«{key}» no dio aviso");
        }

        // **Los ceros a la izquierda SÍ valen, y eso es una decisión, no un descuido**: quien escribe
        // `color.07` quiere el siete y no hay otra lectura posible. Rechazarlo sería hostil por gusto.
        let (settings, warnings) = parsed("color.07 = #ff0000");
        assert_eq!(
            settings.colors.palette[7],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "«color.07» no aplicó el color siete"
        );
        assert_eq!(warnings.total(), 0);
    }

    #[test]
    fn un_valor_larguisimo_no_se_vuelca_entero_en_el_aviso() {
        // **El valor sale del fichero tal cual**, y un `config` copiado de cualquier sitio puede traer una
        // línea de cien mil caracteres o unos códigos de escape que le hablen al terminal que lanzó la
        // aplicación. Lo cazó una revisión.
        let long = "x".repeat(100_000);
        let (_, warnings) = parsed(&format!("background = {long}"));
        let said = warnings.kept()[0].to_string();
        assert!(
            said.chars().count() < 200,
            "el aviso salió con {} caracteres: se está volcando el valor entero",
            said.chars().count()
        );

        let (_, warnings) = parsed("background = \u{1b}[31mRED\u{1b}[0m");
        assert!(
            !warnings.kept()[0].to_string().contains('\u{1b}'),
            "el aviso deja pasar códigos de escape al terminal que lanzó la aplicación"
        );
    }

    #[test]
    fn a_quien_se_le_olvida_el_valor_se_le_dice_eso_y_no_otra_cosa() {
        // **Decirle «no tiene la forma clave = valor» a quien escribió `background =` le manda a mirar donde no
        // es**: el igual está puesto, lo que falta es el color.
        let (_, warnings) = parsed("background =");
        assert_eq!(warnings.total(), 1, "no se avisó de la línea a medias");
        let said = warnings.kept()[0].to_string();
        assert!(
            said.contains("the key or the value is missing"),
            "el aviso describe mal el fallo: «{said}»"
        );

        let (_, warnings) = parsed("esto no lleva igual");
        assert!(
            warnings.kept()[0].to_string().contains("'key = value'"),
            "una línea sin igual tiene que decir eso, y dijo: «{}»",
            warnings.kept()[0]
        );
    }

    #[test]
    fn la_marca_de_orden_de_bytes_no_se_come_la_primera_linea() {
        // **Es el fallo más cruel que este fichero podía tener**, y lo destapó probar en vez de leer: la
        // marca la meten los editores de Windows y algunas páginas al copiar, se queda pegada a la primera
        // clave, y el aviso señala algo que **en pantalla se ve idéntico** a la clave correcta. Quien lo
        // sufra no tiene forma de ver la diferencia.
        let (settings, warnings) = parsed("\u{FEFF}color.1 = #ff0000\nbackground = #010203");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "la marca de orden de bytes se comió la primera línea"
        );
        assert_eq!(
            warnings.total(),
            0,
            "se avisó de una línea que está bien escrita"
        );
    }

    #[test]
    fn un_fichero_de_windows_se_lee_igual() {
        // **Los saltos de línea de Windows llegan solos**: un tema copiado de una página web, un fichero
        // editado en otra máquina. Si el retorno de carro se quedara pegado al valor, ningún color
        // funcionaría y el aviso hablaría de un valor que en pantalla se ve bien escrito.
        //
        // **Quien lo resuelve es `str::lines`, no este módulo, y se dice porque cambia lo que la prueba
        // ata**: mutar el recorte de espacios de este fichero **no** la tumba. Lo que ata es el contrato
        // con esa función — el día que alguien parta el texto de otra manera, esto cae.
        let (settings, warnings) =
            parsed("# un tema\r\ncolor.1 = #ff0000\r\nbackground = #010203\r\n");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "el retorno de carro se quedó pegado al valor"
        );
        assert_eq!(settings.colors.background, Some(Color32::from_rgb(1, 2, 3)));
        assert_eq!(
            warnings.total(),
            0,
            "un fichero con saltos de Windows dio avisos"
        );
    }

    #[test]
    fn el_color_se_escribe_en_mayusculas_o_en_minusculas() {
        // Se copian de páginas que los escriben de las dos formas, así que rechazar una sería un fallo
        // que quien lo sufre no puede ni ver.
        let (settings, warnings) = parsed("color.1 = #AABBCC\ncolor.2 = #aabbcc");

        assert_eq!(settings.colors.palette[1], settings.colors.palette[2]);
        assert_eq!(warnings.total(), 0, "una de las dos formas se rechazó");
    }

    #[test]
    fn un_valor_con_igual_dentro_no_se_parte_por_el_segundo() {
        // Hoy ningún ajuste lleva un igual en el valor, pero la fuente de la entrega siguiente puede
        // llevarlo en su nombre. Partir por el **primer** igual es lo que hace que eso no muerda después.
        let (_, warnings) = parsed("font = Iosevka Term = Extended");

        assert_eq!(
            warnings.total(),
            1,
            "se avisó más de una vez, así que la línea se partió por más de un igual"
        );
        // **La clave tiene que ser EXACTAMENTE «font».** Comprobar solo que el aviso «contiene font»
        // no ata nada: partiendo por el último igual, la clave sale como «font = Iosevka Term», que
        // también lo contiene. Se comprobó mutándolo.
        assert_eq!(
            warnings.kept()[0].missing,
            Missing::UnknownKey("font".to_owned()),
            "la línea se partió por un igual que no es el primero"
        );
    }

    #[test]
    fn los_atajos_se_leen_del_fichero_en_el_orden_en_que_se_escriben() {
        // **El orden importa y por eso llegan como lista**: «gana la última» solo se puede decidir
        // aplicándolas en el orden en que están escritas.
        let (settings, warnings) = parsed(
            "key.split-right = ctrl+shift+e\n             key.close-pane = none\n             key.new-tab = ctrl+alt+t",
        );
        assert_eq!(
            warnings.total(),
            0,
            "alguna de las tres líneas no se entendió"
        );
        assert_eq!(settings.shortcuts.len(), 3, "no llegaron los tres atajos");
        assert_eq!(
            settings.shortcuts[0].0,
            crate::keyboard::Action::SplitRight,
            "el orden de lectura no se conserva, y «gana la última» deja de poder decidirse"
        );
        assert_eq!(settings.shortcuts[1].1, None, "«none» no desató la acción");
    }

    #[test]
    fn el_atajo_de_copiar_solo_se_puede_apagar() {
        // **Su acorde lo decide el escritorio**, que se lo queda antes de que llegue aquí y ni siquiera
        // dice si había Mayúsculas. Aceptar `key.copy = ctrl+shift+e` sería prometer un acorde que
        // no se dispararía nunca y que quien lo escribió no tendría forma de depurar.
        let (settings, warnings) = parsed("key.copy = none");
        assert_eq!(warnings.total(), 0, "«none» no se entendió");
        assert_eq!(settings.copy, Some(false), "«none» no apagó el copiar");
        assert!(
            settings.shortcuts.is_empty(),
            "copiar se coló en la lista de acordes, donde no tiene ninguno que mover"
        );

        let (settings, warnings) = parsed("key.copy = ctrl+shift+e");
        assert_eq!(warnings.total(), 1, "reasignarlo no se avisó");
        assert_eq!(settings.copy, None, "reasignarlo tocó el interruptor");
        assert!(
            format!("{}", warnings.kept()[0]).contains("turn it off"),
            "el aviso no dice lo único que se puede hacer con ese acorde: {}",
            warnings.kept()[0]
        );

        // Y **no se apaga sin pedirlo**: un fichero que no lo nombra lo deja como estaba.
        let (settings, _) = parsed("background = #101010");
        assert_eq!(
            settings.copy, None,
            "un fichero que no habla de copiar lo dejó dicho igualmente"
        );
    }

    #[test]
    fn un_atajo_mal_escrito_avisa_y_no_se_lleva_a_los_demas() {
        // La misma regla que el resto del fichero, en la superficie que más se teclea a mano.
        for (line, what) in [
            ("key.made-up = ctrl+shift+e", "una acción que no existe"),
            (
                "key.close-pane = ctrl+shift+doesnotexist",
                "una tecla que no existe",
            ),
            ("key.close-pane = e", "un acorde sin tecla de mando"),
            ("key.close-pane = ", "sin valor"),
        ] {
            let (settings, warnings) =
                parsed(&format!("color.1 = #ff0000\n{line}\ncolor.2 = #00ff00"));
            assert!(
                settings.shortcuts.is_empty(),
                "{what} se aplicó igualmente: «{line}»"
            );
            assert_eq!(warnings.total(), 1, "{what} no dio aviso: «{line}»");
            assert!(
                settings.colors.palette[1].is_some() && settings.colors.palette[2].is_some(),
                "{what} se llevó por delante las líneas buenas de al lado"
            );
        }
    }

    #[test]
    fn los_veintidos_colores_llegan_al_tema() {
        // **Se afirma sobre el color que el tema devuelve, con el valor literal escrito en la prueba.**
        // Compararlo contra `Theme::from_colors(...)` otra vez mediría que el código es consistente consigo mismo:
        // cruzar dos colores dejaría los dos lados de la igualdad cambiados y la prueba en verde. Este
        // proyecto ya tiene fichado ese disfraz, con 525 pruebas sobreviviendo a un cruce de colores.
        let text = "color.0 = #010101\ncolor.15 = #0f0f0f\nbackground = #020202\nforeground = #030303\n\
                     cursor = #040404\ndivider = #050505\nfocus = #060606\npanel = #070707";
        let (settings, warnings) = parsed(text);
        assert_eq!(
            warnings.total(),
            0,
            "alguna de las ocho líneas no se entendió"
        );

        let theme = crate::theme::Theme::from_colors(&settings.colors);

        assert_eq!(
            theme.color(term_core::TermColor::Indexed(0)),
            Color32::from_rgb(1, 1, 1),
            "el color 0 de la paleta no llegó al tema"
        );
        assert_eq!(
            theme.color(term_core::TermColor::Indexed(15)),
            Color32::from_rgb(0x0F, 0x0F, 0x0F),
            "el color 15 no llegó: la paleta se está copiando a medias"
        );
        assert_eq!(
            theme.background(),
            Color32::from_rgb(2, 2, 2),
            "el fondo no llegó"
        );
        assert_eq!(
            theme.foreground(),
            Color32::from_rgb(3, 3, 3),
            "el texto no llegó"
        );
        assert_eq!(
            theme.divider(),
            Color32::from_rgb(5, 5, 5),
            "el divisor no llegó"
        );
        assert_eq!(
            theme.focus(),
            Color32::from_rgb(6, 6, 6),
            "el foco no llegó"
        );
        assert_eq!(
            theme.panel_background(),
            Color32::from_rgb(7, 7, 7),
            "el fondo del panel no llegó"
        );
        // **El cursor se afirma por donde el pintor lo lee, y era el único de los seis que nadie
        // comprobaba.** Medido: cambiar su asignación por la del foco dejaba las 718 pruebas de `app`
        // en verde, así que el defecto y el arreglo daban el mismo resultado. Y va por `TermColor`
        // porque es la puerta por la que el pintor pide ese color, no por un campo: afirmar sobre el
        // campo comprobaría que el valor está apuntado, no que se usa.
        assert_eq!(
            theme.color(term_core::TermColor::Cursor),
            Color32::from_rgb(4, 4, 4),
            "el color del cursor no llegó"
        );
    }

    #[test]
    fn pedir_un_solo_color_no_borra_los_otros_quince() {
        // **Es lo que permite cambiar un color sin escribir los veintidós**, y el modo de fallo es que la
        // paleta se sustituya entera en vez de hueco a hueco: quedarían quince negros.
        let (settings, _) = parsed("color.1 = #ff0000");
        let theme = crate::theme::Theme::from_colors(&settings.colors);

        assert_eq!(
            theme.color(term_core::TermColor::Indexed(1)),
            Color32::from_rgb(0xFF, 0, 0),
            "el color pedido no se aplicó"
        );
        assert_eq!(
            theme.color(term_core::TermColor::Indexed(2)),
            crate::theme::Theme::default().color(term_core::TermColor::Indexed(2)),
            "pedir un color cambió otro: la paleta se está sustituyendo entera"
        );
    }

    /// Un directorio propio por prueba, con el número de proceso.
    ///
    /// **La suite corre en paralelo y el nombre del fichero es fijo**, así que dos pruebas que
    /// compartieran directorio se leerían la configuración la una a la otra y fallarían solo a veces. Es
    /// el mismo molde que usa el módulo hermano, y por la misma razón.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "agent-terminal-settings-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("crear el directorio de la prueba");
            Self(path)
        }

        fn with(&self, contents: &[u8]) -> PathBuf {
            let path = self.0.join(FILE);
            std::fs::write(&path, contents).expect("escribir el fichero de la prueba");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn un_fichero_que_se_pasa_del_techo_no_se_lee() {
        // **El techo existe porque el fichero tiene tamaño arbitrario**, y leerlo sin tope es reservar sin
        // tope en el arranque. Sin esta prueba, el techo se podía bajar a un byte con la suite entera en
        // verde — comprobado por una revisión con siete mutaciones vivas en este camino.
        let dir = TestDir::new("techo");
        let exact = dir.with(&vec![b'#'; MAX_BYTES]);
        assert!(
            read(&exact).is_ok(),
            "un fichero de exactamente el techo tiene que leerse"
        );

        let over = dir.with(&vec![b'#'; MAX_BYTES + 1]);
        assert_eq!(
            read(&over),
            Err(ConfigProblem::TooBig),
            "un fichero de un byte por encima del techo se leyó igualmente"
        );
    }

    #[test]
    fn el_techo_y_el_tope_de_avisos_son_los_que_dice_el_documento() {
        // **Las dos constantes están publicadas en `docs/08-configuracion.md`**, y las pruebas que las usan
        // se parametrizan **con ellas mismas**: mover el número no tumbaba nada mientras el documento
        // seguía prometiendo el viejo. Es el disfraz que este proyecto tiene fichado —elegir un número con
        // el que el defecto y el arreglo dan lo mismo— y hace falta escribirlos literales una vez.
        assert_eq!(MAX_BYTES, 64 * 1024, "el documento promete 64 KiB");
        assert_eq!(MAX_WARNINGS, 20, "el documento promete hasta veinte avisos");
    }

    /// **Las claves de color y de tamaño viven dos veces**: en el `match` de [`apply`] y en la tabla del
    /// documento, escritas a mano en los dos sitios y sin nada que las enfrentara. Los once atajos ya
    /// tenían sus dos guardas; estas ocho no tenían ninguna, así que renombrarlas en un lado dejaba
    /// publicado un fichero de configuración que la aplicación rechaza — que es exactamente el fallo que
    /// el documento del contrato describe para los atajos, en la lista de al lado.
    ///
    /// **Se lee el documento, no se copia la tabla.** Una prueba que repitiera las ocho claves se arregla
    /// sola cuando alguien las traduce en el código y en la prueba y se olvida del documento, que es el
    /// caso que hay que cazar. La dirección contraria —una clave que el código acepta y el documento
    /// calla— no se puede derivar: el `match` no se puede recorrer, y copiarlo aquí sería volver a tener
    /// la lista dos veces.
    #[test]
    fn el_documento_publica_claves_que_la_aplicacion_acepta() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        let published: Vec<&str> = DOCUMENT
            .lines()
            .skip_while(|line| !line.starts_with("| Clave |"))
            .skip(2)
            .take_while(|line| line.starts_with('|'))
            .filter_map(|line| line.split('`').nth(1))
            .collect();
        // **Sin suelo esta prueba pasa por estar vacía**, que es como muere una guarda que lee otro
        // fichero: alguien le cambia el encabezado a la tabla, el recorrido no engancha, y no queda nada
        // que comprobar. Ocho es lo que la tabla publica hoy.
        assert_eq!(
            published.len(),
            8,
            "la tabla de claves del documento dio {} filas: o el documento cambió de forma, o este \
             recorrido dejó de encontrarla y la prueba no está mirando nada — {published:?}",
            published.len()
        );

        for key in published {
            // Cada clave se prueba con los dos valores que el documento admite, porque la forma del valor
            // depende de la clave y lo que se afirma es solo que la clave existe.
            let known = ["#ff0000", "14"].iter().any(|value| {
                !matches!(
                    apply(&mut Settings::default(), key, value),
                    Err(Missing::UnknownKey(_))
                )
            });
            assert!(
                known,
                "el documento publica «{key}» y la aplicación la rechaza por desconocida: quien siga el \
                 documento escribiría una línea que no hace nada"
            );
        }
    }

    /// **Y los ejemplos que el documento da a copiar se interpretan de verdad**, no solo sus claves.
    ///
    /// La guarda de al lado lee **la tabla**, así que deja fuera lo que más se copia: los bloques de
    /// ejemplo. Medido antes de escribir esto: cambiarle al documento el prefijo `key.` por otro, o el
    /// valor `none` por el que fuese, dejaba las 873 pruebas en verde — y son las dos mitades que esta
    /// entrega acaba de renombrar. Aquí se le pasa cada línea `clave = valor` de cada bloque **al
    /// intérprete de verdad**, que es lo que hará el terminal con el fichero que alguien pegue.
    ///
    /// **Va por bloques de ejemplo y no por comillas invertidas, y eso evita una lista de excepciones.**
    /// La prosa nombra `clave = valor` como forma, que no es ninguna clave; un bloque de ejemplo, en
    /// cambio, es literalmente lo que alguien copia y pega, así que ahí no hay marcadores de posición
    /// que exentar.
    #[test]
    fn los_ejemplos_del_documento_se_interpretan_sin_un_solo_aviso() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");

        let mut inside = false;
        let examples: Vec<&str> = DOCUMENT
            .lines()
            .filter(|line| {
                if line.starts_with("```") {
                    inside = !inside;
                    return false;
                }
                // Las líneas de los bloques que no son `clave = valor` —los mensajes por pantalla— no
                // son ejemplos de configuración y no se le pasan al intérprete.
                inside && line.contains(" = ") && !line.starts_with("agent-terminal:")
            })
            .collect();
        // Mismo suelo y por la misma razón que su guarda hermana: si el recorrido deja de enganchar los
        // bloques, esta prueba pasa por estar vacía.
        assert_eq!(
            examples.len(),
            8,
            "los bloques de ejemplo del documento dieron {} líneas de configuración: o el documento \
             cambió de forma, o este recorrido dejó de encontrarlas — {examples:?}",
            examples.len()
        );

        for example in examples {
            let (settings, warnings) =
                interpret(example).expect("una línea suelta no puede tumbar el fichero entero");
            assert_eq!(
                warnings.total(),
                0,
                "el documento da «{example}» como ejemplo y el terminal lo rechaza: {}",
                warnings.kept()[0]
            );
            // **Y se afirma que hizo algo**, no solo que no se quejó: una clave desconocida que dejara
            // de avisar pasaría esta prueba sin aplicar nada, que es como una guarda se queda en verde
            // sin comprobar nada. La excepción es `version`, que **no guarda ningún valor** a propósito:
            // es un veto que se comprueba y se olvida, y por eso su ejemplo no puede cambiar nada.
            if !example.starts_with("version") {
                assert_ne!(
                    settings,
                    Settings::default(),
                    "el documento da «{example}» como ejemplo y no cambia ningún ajuste"
                );
            }
        }
    }

    /// Palabras de función españolas, para juzgar una frase que se ve por pantalla.
    ///
    /// **Es el mismo criterio con el que la guarda del glosario elige las suyas**, y por su misma
    /// razón: son cortas, la lista es cerrada, y ninguna es además una palabra inglesa. Por eso quedan
    /// fuera «no» y «sin», que lo son. Se comparan como palabra entera, así que `none` no es «no».
    const SPANISH_FUNCTION_WORDS: &[&str] = &[
        "de", "del", "la", "el", "los", "las", "lo", "que", "con", "por", "para", "una", "uno",
        "al", "mas", "ya", "se", "es", "hay",
    ];

    /// La primera palabra española de ese texto, si la hay.
    fn spanish_word_in(text: &str) -> Option<&str> {
        text.split(|c: char| !c.is_ascii_alphabetic())
            .find(|word| SPANISH_FUNCTION_WORDS.contains(&word.to_ascii_lowercase().as_str()))
    }

    /// **El documento publica tres mensajes por pantalla, palabra por palabra, y nada los enfrentaba
    /// con el código.** Son lo que alguien pega en un buscador cuando su fichero no hace lo que
    /// esperaba, así que traducirlos en el código y no en el documento —o al revés— deja publicada una
    /// frase que el terminal no dice nunca. Es el mismo acoplamiento que el de la tabla de claves de
    /// más arriba, en la mitad que aquélla deja fuera **a propósito**: su recorrido descarta las líneas
    /// que empiezan por `agent-terminal:`, y hasta aquí nadie las recogía.
    ///
    /// **Los dos avisos de línea se reproducen por el intérprete de verdad**, escribiendo en la línea
    /// que el documento señala lo que el documento cuenta que pasó ahí. El tercero no lo produce este
    /// módulo sino la ventana, y como imprimir no se puede observar desde dentro del proceso, lo único
    /// que se puede enfrentar sin volver a copiarlo es **su plantilla en el fuente**.
    #[test]
    fn los_mensajes_que_el_documento_publica_son_los_que_el_terminal_escribe() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuracion.md");
        const WINDOW: &str = include_str!("lib.rs");

        let mut inside = false;
        let published: Vec<&str> = DOCUMENT
            .lines()
            .filter(|line| {
                if line.starts_with("```") {
                    inside = !inside;
                    return false;
                }
                inside && line.starts_with("agent-terminal: ")
            })
            .collect();
        // Mismo suelo y por la misma razón que las dos guardas hermanas de arriba: sin él, un documento
        // que dejara de publicar mensajes deja esta prueba pasando por no mirar nada.
        assert_eq!(
            published.len(),
            3,
            "el documento publica {} mensajes por pantalla: o cambió de forma, o este recorrido dejó \
             de encontrarlos y la prueba no está mirando nada — {published:?}",
            published.len()
        );

        // **Y las tres tienen que estar en inglés, aquí y no solo en el código.** Sin esto, devolver al
        // español el mensaje Y su línea en el documento pasa las dos guardas: las frases seguirían
        // coincidiendo entre sí, que es lo único que el resto de esta prueba comprueba.
        for line in &published {
            assert!(
                line.is_ascii(),
                "el documento publica «{line}» con un carácter que no es ASCII: las comillas \
                 angulares y los acentos solo salen de la versión española de ese mensaje"
            );
            assert_eq!(
                spanish_word_in(line),
                None,
                "el documento publica «{line}», que sigue en español"
            );
        }

        // Un aviso de línea se reproduce poniendo su línea donde el documento dice que estaba: el
        // número forma parte del mensaje, así que sembrarlo en otra línea ya no lo reproduce.
        let written_by_the_terminal = |at: usize, text: &str| {
            let file = format!("{}{text}", "\n".repeat(at - 1));
            let (_, warnings) = parsed(&file);
            assert_eq!(
                warnings.kept().len(),
                1,
                "«{text}» tenía que dar exactamente un aviso y dio {}",
                warnings.kept().len()
            );
            format!("agent-terminal: {}", warnings.kept()[0])
        };
        let ours = [
            written_by_the_terminal(3, "key.copy = ctrl+shift+e"),
            written_by_the_terminal(7, "color.3 = morado"),
        ];

        for said in &ours {
            assert!(
                published.contains(&said.as_str()),
                "el terminal escribe «{said}» y el documento no publica esa frase: quien la busque \
                 tal como sale por pantalla no encontrará el documento — {published:?}"
            );
        }
        // **Y la dirección contraria**, que es la que caza un mensaje nuevo publicado sin guarda: todo
        // aviso de línea que el documento enseñe tiene que ser uno de los que se acaban de reproducir.
        for line in published.iter().filter(|line| line.contains(": line ")) {
            assert!(
                ours.iter().any(|said| said == line),
                "el documento publica «{line}» y ningún aviso del terminal sale así"
            );
        }

        let clash = published
            .iter()
            .find(|line| !line.contains(": line "))
            .expect("el documento publica el aviso de los dos atajos que se pisan");
        // Las dos mitades que el aviso interpola son nombres de acción, y tienen que serlo de verdad.
        for name in clash.split('\'').skip(1).step_by(2) {
            assert!(
                crate::keyboard::Action::from_written(name).is_some(),
                "el documento enseña «{name}» en ese aviso y no es ninguna acción que se pueda atar"
            );
        }
        // De vuelta a la plantilla: lo entrecomillado es lo que la ventana interpola, y el resto es la
        // frase fija que tiene que estar escrita igual en los dos sitios.
        let template = clash
            .split('\'')
            .enumerate()
            .map(|(nth, piece)| {
                if nth % 2 == 1 {
                    "{}".to_owned()
                } else {
                    piece.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("'");
        // **Una cadena partida en el fuente lleva `\` al final de la línea**, que se traga el salto y la
        // sangría de la siguiente. Deshacerlo aquí es lo que hace que esta guarda sobreviva a que
        // `cargo fmt` reajuste esa línea, en vez de caerse por ello.
        let mut source = String::with_capacity(WINDOW.len());
        let mut rest = WINDOW;
        while let Some(cut) = rest.find("\\\n") {
            source.push_str(&rest[..cut]);
            rest = rest[cut + 2..].trim_start();
        }
        source.push_str(rest);
        assert!(
            source.contains(&template),
            "el documento publica «{clash}» y la ventana no escribe esa frase: se buscó la plantilla \
             «{template}»"
        );
    }

    /// **La frase fija de cada mensaje es la mitad de AC6 que ninguna prueba miraba.** Las guardas de
    /// esta suite afirman sobre lo que el aviso **interpola** —la clave, el valor, el número de
    /// línea—, y donde citan la frase citan un trozo. Devolver al español los trece mensajes de los
    /// dos tipos dejaba la suite entera en verde: comprobado antes de escribir esto.
    ///
    /// **Palabras de función y no un léxico ancho**, que es el mismo criterio con el que la guarda del
    /// glosario elige las suyas: son cortas, la lista es cerrada y ninguna de ellas es además una
    /// palabra inglesa. Por eso quedan fuera «no» y «sin», que lo son, aunque las trece frases españolas
    /// las llevaran.
    ///
    /// El oráculo no-ASCII va al lado porque las comillas angulares y los acentos son gratis de
    /// comprobar, no porque baste: cuatro de los mensajes españoles eran ASCII puro.
    #[test]
    fn ninguna_frase_de_las_que_se_ven_por_pantalla_se_quedo_en_espanol() {
        let problems = [
            ConfigProblem::Absent,
            ConfigProblem::BrokenLink,
            ConfigProblem::NotAFile,
            ConfigProblem::Unreadable("permission denied".to_owned()),
            ConfigProblem::TooBig,
            ConfigProblem::NotText,
            ConfigProblem::Version(9),
        ];
        let faults = [
            Missing::NoEquals,
            Missing::Incomplete,
            Missing::UnknownKey("size".to_owned()),
            Missing::InvalidValue {
                key: "size".to_owned(),
                value: "abc".to_owned(),
            },
            Missing::Repeated("size".to_owned()),
            Missing::OnlyTurnsOff("key.copy".to_owned()),
        ];
        // **Los dos `match` son exhaustivos y sin comodín, y no sobran.** Son lo único que obliga a que
        // una variante nueva pase por esta guarda: las dos listas de arriba son literales, así que sin
        // ellos añadir un mensaje se le escaparía en silencio y la prueba seguiría verde.
        for problem in &problems {
            match problem {
                ConfigProblem::Absent
                | ConfigProblem::BrokenLink
                | ConfigProblem::NotAFile
                | ConfigProblem::Unreadable(_)
                | ConfigProblem::TooBig
                | ConfigProblem::NotText
                | ConfigProblem::Version(_) => {}
            }
        }
        for fault in &faults {
            match fault {
                Missing::NoEquals
                | Missing::Incomplete
                | Missing::UnknownKey(_)
                | Missing::InvalidValue { .. }
                | Missing::Repeated(_)
                | Missing::OnlyTurnsOff(_) => {}
            }
        }

        let said: Vec<String> = problems
            .iter()
            .map(ToString::to_string)
            .chain(
                faults
                    .into_iter()
                    .map(|missing| Warning { line: 7, missing }.to_string()),
            )
            .collect();
        assert_eq!(
            said.len(),
            13,
            "no se recorrieron los trece mensajes que se ven por pantalla: {said:?}"
        );

        for message in said {
            assert!(
                message.is_ascii(),
                "«{message}» lleva un carácter que no es ASCII: las comillas angulares y los acentos \
                 solo salen de la versión española de este mensaje"
            );
            assert_eq!(
                spanish_word_in(&message),
                None,
                "el mensaje «{message}» sigue en español, y es lo que se ve por pantalla"
            );
        }
    }

    #[test]
    fn lo_que_no_es_un_fichero_no_se_intenta_leer() {
        let dir = TestDir::new("no-fichero");
        let as_directory = dir.0.join(FILE);
        std::fs::create_dir_all(&as_directory)
            .expect("crear el directorio con el nombre del fichero");

        assert_eq!(
            read(&as_directory),
            Err(ConfigProblem::NotAFile),
            "un directorio con el nombre del fichero se intentó leer como configuración"
        );
    }

    #[test]
    fn un_fichero_que_no_es_texto_deja_el_terminal_de_fabrica() {
        // Está prometido por escrito en el documento que se le da a quien escribe el fichero.
        let dir = TestDir::new("no-texto");
        let path = dir.with(&[0xFF, 0xFE, 0x00, 0x80]);

        assert_eq!(
            read(&path),
            Err(ConfigProblem::NotText),
            "unos bytes que no son texto se interpretaron igualmente"
        );
    }

    #[test]
    fn el_fichero_que_no_esta_se_calla_y_el_enlace_roto_no() {
        // **La diferencia importa y no la veía nadie.** Quien no ha escrito configuración no debe oír nada
        // en cada arranque; quien la escribió y se la movieron **tiene que enterarse**, y antes salía por
        // el mismo camino silencioso. Lo cazó una revisión.
        let dir = TestDir::new("ausente");
        let never = dir.0.join(FILE);
        assert_eq!(read(&never), Err(ConfigProblem::Absent));
        assert!(
            !ConfigProblem::Absent.warrants_warning(),
            "se avisaría en cada arranque a quien no ha configurado nada"
        );

        #[cfg(unix)]
        {
            let link = dir.0.join("enlace");
            std::os::unix::fs::symlink(dir.0.join("no-existe"), &link).expect("crear el enlace");
            assert_eq!(
                read(&link),
                Err(ConfigProblem::BrokenLink),
                "un enlace roto se confundió con «no hay fichero»"
            );
            assert!(
                ConfigProblem::BrokenLink.warrants_warning(),
                "un enlace roto se calla, y quien lo creó no se entera de que su configuración no se aplica"
            );
        }
    }

    #[test]
    fn un_fichero_de_verdad_se_lee_de_verdad() {
        // **Ninguna de las veinte pruebas anteriores llamaba a `read`**: todas entraban por el intérprete,
        // así que el camino de disco entero se podía borrar con la suite en verde. Lo cazó una revisión con
        // siete mutaciones vivas.
        let dir = TestDir::new("de-verdad");
        let path = dir.with(
            b"# un tema
color.1 = #ff0000
malo
",
        );

        let (settings, warnings) = read(&path).expect("el fichero de la prueba tenía que leerse");
        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0))
        );
        assert_eq!(
            warnings.total(),
            1,
            "la línea mala no avisó al leer del disco"
        );
    }

    #[test]
    fn el_fichero_se_busca_donde_dice_el_documento() {
        // **`path_from` se sacó aparte explícitamente «para poder probarla sin tocar el entorno», y no se
        // probaba**: apuntarla al fichero de la disposición dejaba la suite en verde. Lo cazó una revisión.
        let path = path_from(Some(std::ffi::OsStr::new("/casa/config")), None)
            .expect("una ruta absoluta tiene que dar sitio");
        assert_eq!(
            path,
            PathBuf::from("/casa/config/agent-terminal/config"),
            "el fichero no se busca donde el documento dice"
        );

        assert_eq!(
            path_from(None, Some(std::ffi::OsStr::new("/casa"))),
            Some(PathBuf::from("/casa/.config/agent-terminal/config")),
            "sin la variable propia no se cae a «.config» de la casa"
        );
        assert_eq!(
            path_from(None, Some(std::ffi::OsStr::new("relativa"))),
            None,
            "una ruta relativa se aceptó: el fichero acabaría dentro del directorio de trabajo"
        );
    }
}
