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
pub const FICHERO: &str = "config";

/// El techo del fichero, aplicado **antes** de interpretar.
///
/// **Es literalmente el de la disposición, no una copia con el mismo número.** Un comentario que diga «el
/// mismo que aquél» junto a una constante propia es una promesa que nada obliga a cumplir: el día que una
/// de las dos cambie, el comentario miente y nadie se entera.
use crate::session::BYTES_MAXIMOS;

/// Cuántas líneas malas se dicen antes de callar.
///
/// **El aviso aquí ES el producto, no una nota al margen.** La disposición avisa una vez por clase de
/// problema porque la escribe la aplicación y el aviso solo informa; aquí, con dos colores mal escritos,
/// oír solo uno hace que se arregle la mitad y se vuelva a abrir. Pero un fichero pegado del sitio
/// equivocado tampoco debe llenar la salida de error, así que hay tope **y se dice cuántos quedaron**.
const AVISOS_MAXIMOS: usize = 20;

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
/// Una línea mala no está aquí: eso es un [`Aviso`], y el resto del fichero se aplica igual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problema {
    /// No hay fichero. **Es el caso normal** y no se avisa de él.
    Ausente,
    /// Hay un enlace con ese nombre y no apunta a nada.
    EnlaceRoto,
    /// Hay algo con ese nombre y no es un fichero.
    NoEsFichero,
    /// No se pudo leer.
    Ilegible(String),
    /// Se pasa del techo.
    DemasiadoGrande,
    /// No es texto.
    NoEsTexto,
    /// Dice ser de una versión que este terminal no conoce.
    Version(u64),
}

impl Problema {
    /// Si vale la pena decirlo por la salida de error.
    ///
    /// **Que no haya fichero es el caso normal, no un problema**, y decirlo en cada arranque de quien no
    /// configura nada sería ruido puro. La regla vive **en el tipo** y no en quien llama, igual que en el
    /// módulo hermano: es la única forma de que dos sitios que lo usen no dejen de coincidir.
    pub fn merece_aviso(&self) -> bool {
        !matches!(self, Self::Ausente)
    }
}

impl std::fmt::Display for Problema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ausente => write!(f, "no hay fichero de configuración"),
            Self::EnlaceRoto => write!(f, "el enlace de la configuración no apunta a ningún sitio"),
            Self::NoEsFichero => write!(f, "eso no es un fichero"),
            Self::Ilegible(porque) => write!(f, "no se pudo leer: {porque}"),
            Self::DemasiadoGrande => {
                write!(f, "se pasa de los {} KiB que se leen", BYTES_MAXIMOS / 1024)
            }
            Self::NoEsTexto => write!(f, "no es texto"),
            Self::Version(cual) => write!(
                f,
                "dice ser de la versión {cual} y este terminal entiende hasta la {VERSION}"
            ),
        }
    }
}

/// Lo que le pasa a **una línea**, con dónde está.
///
/// **Lleva el número de línea porque sin él el aviso no sirve**: «un color mal escrito» en un fichero de
/// cuarenta líneas obliga a repasarlas todas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    pub linea: usize,
    pub falta: Falta,
}

/// Qué le falta a una línea para poder aplicarse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Falta {
    /// No tiene la forma `clave = valor`.
    SinIgual,
    /// Tiene el igual pero le falta la clave o el valor.
    ADmedias,
    /// La clave no es ninguna de las que este terminal entiende.
    ClaveDesconocida(String),
    /// El valor no vale para esa clave.
    ValorInvalido { clave: String, valor: String },
    /// Esa clave ya estaba puesta más arriba; gana ésta.
    Repetida(String),
    /// El acorde de esa acción lo decide el escritorio: solo se puede apagar.
    SoloSeApaga(String),
}

impl std::fmt::Display for Aviso {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "línea {}: ", self.linea)?;
        match &self.falta {
            Falta::SinIgual => write!(f, "no tiene la forma «clave = valor»"),
            Falta::ADmedias => write!(f, "le falta la clave o el valor"),
            Falta::ClaveDesconocida(clave) => write!(f, "«{clave}» no es un ajuste conocido"),
            Falta::ValorInvalido { clave, valor } => {
                write!(f, "«{}» no vale para «{clave}»", recortado(valor))
            }
            Falta::Repetida(clave) => write!(f, "«{clave}» ya estaba puesta antes; gana ésta"),
            Falta::SoloSeApaga(clave) => write!(
                f,
                "el acorde de «{clave}» lo decide el escritorio y no se puede cambiar; \
                 lo único que se puede es apagarlo con «ninguno»"
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
fn recortado(valor: &str) -> String {
    const CABEN: usize = 60;
    if valor.chars().count() <= CABEN {
        return valor.escape_debug().to_string();
    }
    let corto: String = valor.chars().take(CABEN).collect();
    format!("{}…", corto.escape_debug())
}

/// Los colores que el fichero pide, cada uno **solo si lo pide**.
///
/// Son **veintidós**: las dieciséis de la paleta y los seis propios.
///
/// **Cada campo es opcional a propósito**: lo que el fichero no diga se queda como está de fábrica, que es
/// lo que hace que un fichero con una sola línea funcione.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Colores {
    /// Las dieciséis de la paleta que pide el programa de dentro.
    pub paleta: [Option<Color32>; 16],
    pub fondo: Option<Color32>,
    pub texto: Option<Color32>,
    pub cursor: Option<Color32>,
    /// La línea que separa dos consolas.
    pub divisor: Option<Color32>,
    /// Con qué se señala la consola a la que va el teclado.
    pub foco: Option<Color32>,
    /// El fondo del panel de estado.
    pub panel: Option<Color32>,
}

/// Lo que el fichero pide, ya entendido.
// **`Copy` se cae aquí**: los atajos son una lista, y una lista no se copia sola. El tipo se pasa por
// referencia, que es como ya lo pedían sus dos únicos consumidores.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ajustes {
    pub colores: Colores,
    /// El cuerpo de la letra, en puntos. `None` si el fichero no lo pide.
    ///
    /// **Se llama igual que en la ventana y distinto que en el fichero, a propósito.** `tamaño` es la clave
    /// que la gente teclea y es contrato publicado; `cuerpo` es como el resto del código llama a este
    /// número. La traducción ocurre **una vez**, donde se interpreta la clave, y no en cada salto.
    pub cuerpo: Option<f32>,
    /// Los atajos que el fichero cambia: la acción y su acorde nuevo, o `None` para desatarla.
    ///
    /// **Una lista y no una tabla ya resuelta**, porque el orden importa: «gana la última» solo se puede
    /// decidir aplicándolas en el orden en que se escribieron.
    pub atajos: Vec<(crate::keyboard::Action, Option<crate::keyboard::Chord>)>,
    /// Si el fichero apagó el atajo de copiar.
    ///
    /// **Aparte de la lista, porque copiar no tiene acorde que mover.** Su acorde lo resuelve el
    /// escritorio antes de que llegue aquí —y sin decir si había Mayúsculas—, así que lo único que se
    /// puede hacer con él es dejar de atenderlo, que es lo que devuelve `Ctrl+Mayúsculas+C` a `vim`.
    pub copiar: Option<bool>,
}

/// Lo menos y lo más que se admite como cuerpo de letra.
///
/// **No es una manía: fuera de este rango la ventana deja de ser un terminal.** Con dos puntos no se lee
/// nada y con doscientos caben tres columnas, y el reparto de las divisiones tiene mínimos que dejarían de
/// poder cumplirse. Los extremos son generosos a propósito —hay quien trabaja a 8 y quien presenta a 40—:
/// lo que se rechaza es lo que solo puede ser una errata.
const TAMANO_MINIMO: f32 = 4.0;
const TAMANO_MAXIMO: f32 = 96.0;

/// Las líneas que no se pudieron aplicar: las que se guardan **y cuántas hubo**.
///
/// **Los dos números, y no solo la lista, porque el techo miente si se calla.** Guardar veinte y no decir
/// que había cuarenta haría creer que el fichero está bien a partir del vigésimo.
///
/// **No imprime: decide.** Quién lo dice por la salida de error es cosa de quien llama, igual que en el
/// módulo hermano — imprimir no se puede observar desde dentro del proceso, así que lo único que una
/// prueba puede atar es la decisión.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Avisos {
    guardados: Vec<Aviso>,
    cuantos: usize,
}

impl Avisos {
    fn empujar(&mut self, aviso: Aviso) {
        self.cuantos += 1;
        if self.guardados.len() < AVISOS_MAXIMOS {
            self.guardados.push(aviso);
        }
    }

    /// Los que se guardaron, que son los primeros [`AVISOS_MAXIMOS`].
    pub fn guardados(&self) -> &[Aviso] {
        &self.guardados
    }

    /// Cuántas líneas dieron problema en total, contando las que no se guardaron.
    pub fn cuantos(&self) -> usize {
        self.cuantos
    }

    /// Cuántas se quedaron sin listar por el techo.
    pub fn sin_listar(&self) -> usize {
        self.cuantos - self.guardados.len()
    }
}

/// Dónde vive el fichero, leyendo el entorno.
///
/// Envoltura de una línea, con el trabajo en [`ruta_de`], que es el patrón del módulo hermano y por su
/// misma razón: fijar estas variables desde una prueba es comportamiento indefinido en un proceso con
/// hilos, así que la decisión tiene que poder probarse sin tocarlas.
pub fn ruta() -> Option<PathBuf> {
    ruta_de(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// Dónde vive el fichero, con los dos valores dados.
pub fn ruta_de(
    config: Option<&std::ffi::OsStr>,
    casa: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    crate::session::directorio_de(config, casa).map(|directorio| directorio.join(FICHERO))
}

/// Lee el fichero de esa ruta. Devuelve lo que se entendió **y** lo que no.
///
/// **El orden es el del molde de la disposición**: pedir los datos, comprobar que es un fichero, **abrir y
/// leer con el tope puesto**, aplicar el techo sobre los bytes leídos, y solo entonces mirar si es texto.
pub fn leer(ruta: &Path) -> Result<(Ajustes, Avisos), Problema> {
    let datos = match std::fs::metadata(ruta) {
        Ok(datos) => datos,
        // **Un enlace roto NO es «no hay fichero», y la diferencia se nota.** Quien creó su configuración y
        // la movió de sitio se merece que se lo digan; el silencio es para quien no ha escrito nada.
        Err(fallo) if fallo.kind() == std::io::ErrorKind::NotFound => {
            return Err(match std::fs::symlink_metadata(ruta) {
                Ok(_) => Problema::EnlaceRoto,
                Err(_) => Problema::Ausente,
            });
        }
        Err(fallo) => return Err(Problema::Ilegible(fallo.to_string())),
    };
    if !datos.is_file() {
        return Err(Problema::NoEsFichero);
    }
    let fichero = std::fs::File::open(ruta).map_err(|f| Problema::Ilegible(f.to_string()))?;
    let crudo = crate::session::leer_acotado(fichero, datos.len())
        .map_err(|fallo| Problema::Ilegible(fallo.to_string()))?;
    // **El techo va sobre los bytes LEÍDOS, no sobre los que los datos del fichero declaran.** No son lo
    // mismo: un fichero de `/proc` declara cero y entrega ciento setenta y cinco bytes, comprobado
    // ejecutándolo. Una versión anterior leía entero con `fs::read` y comparaba después, con un comentario
    // que decía «abrir» donde no se abría nada — el techo existía y **no acotaba la reserva**, que es
    // exactamente lo que su propia razón dice evitar. Lo cazó una revisión.
    if crudo.len() > BYTES_MAXIMOS {
        return Err(Problema::DemasiadoGrande);
    }
    let texto = String::from_utf8(crudo).map_err(|_| Problema::NoEsTexto)?;
    interpretar(&texto)
}

/// Lo que una línea resulta ser.
///
/// **Se clasifica UNA vez.** La primera versión devolvía «nada» tanto para una línea vacía como para una
/// mal escrita, y quien llamaba tenía que volver a mirarla para saber de cuál de las dos se trataba: la
/// misma decisión tomada dos veces, en dos sitios que podían dejar de coincidir.
enum Linea<'a> {
    /// Vacía, espacios, o un comentario.
    Nada,
    /// Una clave y su valor, ya sin espacios.
    Par(&'a str, &'a str),
    /// Dice algo, pero no con la forma que hace falta.
    Mal(Falta),
}

/// Interpreta el texto línea a línea.
pub fn interpretar(texto: &str) -> Result<(Ajustes, Avisos), Problema> {
    let mut ajustes = Ajustes::default();
    let mut avisos = Avisos::default();
    let mut puestas: Vec<String> = Vec::new();

    // **La marca de orden de bytes se quita, y es el fallo más cruel que este fichero podía tener.** La
    // meten los editores de Windows y algunas páginas al copiar; queda pegada a la primera clave, y el
    // aviso resultante señala algo que **en pantalla se ve exactamente igual** que la clave correcta:
    // ««color.1» no es un ajuste conocido». Comprobado ejecutándolo.

    let texto = texto.strip_prefix('\u{FEFF}').unwrap_or(texto);

    for (indice, texto_de_la_linea) in texto.lines().enumerate() {
        let linea = indice + 1;
        let (clave, valor) = match clasificar(texto_de_la_linea) {
            Linea::Nada => continue,
            Linea::Mal(falta) => {
                avisos.empujar(Aviso { linea, falta });
                continue;
            }
            Linea::Par(clave, valor) => (clave, valor),
        };

        // **La versión se mira aquí y no antes**, porque es una clave más: puede ir en cualquier línea, y
        // que falte es lo normal.
        // **La versión no entra en la contabilidad de repetidas, y no es un olvido.** «Gana ésta» significa
        // que esta línea pisó el valor de otra; la versión **no guarda ningún valor**, es un veto que se
        // comprueba y se olvida. Decir que gana algo sería mentir dos veces cuando además su valor no se
        // entiende, que es lo que hacía la primera versión de este arreglo. Lo cazó una revisión.
        if clave == "version" {
            match valor.parse::<u64>() {
                Ok(cual) if cual > VERSION => return Err(Problema::Version(cual)),
                Ok(_) => {}
                Err(_) => avisos.empujar(Aviso {
                    linea,
                    falta: Falta::ValorInvalido {
                        clave: clave.to_owned(),
                        valor: valor.to_owned(),
                    },
                }),
            }
            continue;
        }
        if let Err(falta) = aplicar(&mut ajustes, clave, valor) {
            avisos.empujar(Aviso { linea, falta });
            // **Una línea que no se aplica no cuenta como puesta**, así que no se avisa de repetida por
            // ella: decir «gana ésta» de una línea que no gana nada es un aviso que miente, y encima
            // contradice al de al lado. Comprobado ejecutándolo antes de arreglarlo.
            continue;
        }

        // **El aviso de repetida va DESPUÉS de aplicar, y por eso dice la verdad.** Se emite solo cuando
        // esta línea de verdad se ha quedado con el valor.
        if puestas.iter().any(|ya| ya == clave) {
            avisos.empujar(Aviso {
                linea,
                falta: Falta::Repetida(clave.to_owned()),
            });
        } else {
            puestas.push(clave.to_owned());
        }
    }

    Ok((ajustes, avisos))
}

/// Mete el valor donde le toque, si la clave y el valor valen.
fn aplicar(ajustes: &mut Ajustes, clave: &str, valor: &str) -> Result<(), Falta> {
    if let Some(nombre) = clave.strip_prefix("atajo.") {
        // **Copiar sigue teniendo su clave, y solo acepta `ninguno`.** Es la clave que este terminal ya
        // publicó, y quien la escribió para devolverle el acorde a `vim` no tiene por qué enterarse de
        // que por dentro dejó de ser un acorde. Lo que no se puede es reasignarlo, y eso se dice.
        if nombre.eq_ignore_ascii_case("copiar") {
            if !valor.eq_ignore_ascii_case("ninguno") {
                return Err(Falta::SoloSeApaga(clave.to_owned()));
            }
            ajustes.copiar = Some(false);
            return Ok(());
        }
        let accion = crate::keyboard::Action::from_written(nombre)
            .ok_or_else(|| Falta::ClaveDesconocida(clave.to_owned()))?;
        // **`ninguno` desata la acción, y es la mitad que hace útil todo esto**: es lo que devuelve un
        // acorde al programa de dentro cuando `vim` o `tmux` lo quieren para ellos.
        let acorde = if valor.eq_ignore_ascii_case("ninguno") {
            None
        } else {
            Some(crate::keyboard::Chord::from_written(valor).ok_or_else(|| {
                Falta::ValorInvalido {
                    clave: clave.to_owned(),
                    valor: valor.to_owned(),
                }
            })?)
        };
        ajustes.atajos.push((accion, acorde));
        return Ok(());
    }
    if clave == "tamaño" || clave == "tamano" {
        // **Las dos formas de escribirlo valen, y es deliberado.** Quien teclea la configuración de un
        // terminal no siempre tiene una eñe a mano —un teclado que no es el suyo, una sesión remota— y
        // rechazar `tamano` sería un fallo que quien lo sufre no puede ni ver.
        let cuanto = valor
            .parse::<f32>()
            .ok()
            .filter(|c| c.is_finite() && (TAMANO_MINIMO..=TAMANO_MAXIMO).contains(c))
            .ok_or_else(|| Falta::ValorInvalido {
                clave: clave.to_owned(),
                valor: valor.to_owned(),
            })?;
        ajustes.cuerpo = Some(cuanto);
        return Ok(());
    }
    let colores = &mut ajustes.colores;
    let hueco: &mut Option<Color32> = match clave {
        "fondo" => &mut colores.fondo,
        "texto" => &mut colores.texto,
        "cursor" => &mut colores.cursor,
        "divisor" => &mut colores.divisor,
        "foco" => &mut colores.foco,
        "panel" => &mut colores.panel,
        // **El índice son dígitos y nada más.** Dejarlo en manos del intérprete de números admitía
        // `color.007` y `color.+1` —los dos aplicaban un color— mientras `color.-1` se rechazaba: tres
        // formas de escribir lo mismo con dos resultados distintos, ninguna documentada. Lo cazó una
        // revisión ejecutándolas.
        _ => match clave.strip_prefix("color.") {
            Some(indice)
                if indice.chars().all(|c| c.is_ascii_digit())
                    && indice
                        .parse::<usize>()
                        .is_ok_and(|n| n < colores.paleta.len()) =>
            {
                // **Un índice fuera de la paleta es una clave desconocida, no un valor malo**: lo que
                // está mal escrito es el nombre del ajuste, y decir «el valor no vale» mandaría a mirar
                // el color.
                &mut colores.paleta[indice.parse::<usize>().unwrap_or_default()]
            }
            _ => return Err(Falta::ClaveDesconocida(clave.to_owned())),
        },
    };
    *hueco = Some(color_de(valor).ok_or_else(|| Falta::ValorInvalido {
        clave: clave.to_owned(),
        valor: valor.to_owned(),
    })?);
    Ok(())
}

/// Un color escrito como `#rrggbb`, en mayúsculas o en minúsculas.
fn color_de(valor: &str) -> Option<Color32> {
    let cifras = valor.strip_prefix('#')?;
    if cifras.len() != 6 || !cifras.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let canal = |desde: usize| u8::from_str_radix(&cifras[desde..desde + 2], 16).ok();
    Some(Color32::from_rgb(canal(0)?, canal(2)?, canal(4)?))
}

/// Qué es esta línea.
///
/// **El comentario es de línea completa, y no puede ir al final de un valor.** No es una simplificación:
/// la almohadilla es **a la vez** el carácter de comentario y el prefijo de un color, así que un
/// comentario al final del valor parte `color.1 = #ff0000` por la mitad y deja la clave sin valor.
/// Comprobado escribiéndolo: la primera versión de este intérprete se comía todos los colores.
///
/// Es exactamente lo que hacen Kitty y Ghostty, que usan este mismo formato, y por la misma razón.
fn clasificar(linea: &str) -> Linea<'_> {
    let limpia = linea.trim();
    if limpia.is_empty() || limpia.starts_with('#') {
        return Linea::Nada;
    }
    let Some((clave, valor)) = limpia.split_once('=') else {
        return Linea::Mal(Falta::SinIgual);
    };
    let (clave, valor) = (clave.trim(), valor.trim());
    if clave.is_empty() || valor.is_empty() {
        // **Se distingue de «no tiene igual» a propósito**: quien escribió `fondo =` no se olvidó del
        // igual, se olvidó del color, y decirle lo primero le manda a mirar donde no es.
        return Linea::Mal(Falta::ADmedias);
    }
    Linea::Par(clave, valor)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que devuelve interpretar un texto que se entiende entero.
    fn de(texto: &str) -> (Ajustes, Avisos) {
        interpretar(texto).expect("el texto de la prueba no debería tumbar el fichero entero")
    }

    #[test]
    fn una_linea_mala_no_se_lleva_por_delante_las_buenas() {
        // **Es la razón de ser del formato, y lo que separa este fichero del de la disposición.** Aquel lo
        // escribe la aplicación y no se abre a medias; éste lo teclea una persona, y tumbar los veintidós
        // colores porque uno está mal escrito la obligaría a encontrar el error a ciegas.
        let (ajustes, _) = de("color.1 = #ff0000\ncolor.2 = azul celeste\ncolor.3 = #0000ff");

        assert_eq!(
            ajustes.colores.paleta[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "la línea BUENA de antes de la mala no se aplicó"
        );
        assert_eq!(
            ajustes.colores.paleta[3],
            Some(Color32::from_rgb(0, 0, 0xFF)),
            "la línea buena de DESPUÉS de la mala no se aplicó: el intérprete se paró en el error"
        );
        assert_eq!(
            ajustes.colores.paleta[2], None,
            "el color mal escrito se aplicó igualmente"
        );
    }

    #[test]
    fn la_linea_mala_se_dice_con_su_numero() {
        // **Sin el número, el aviso no sirve**: «un color mal escrito» en un fichero de cuarenta líneas
        // obliga a repasarlas todas. Y sin aviso ninguno, quien escribió la línea creerá que el terminal
        // ignora lo que le pide.
        let (_, avisos) = de("# un comentario\n\ncolor.1 = morado\n");

        assert_eq!(avisos.guardados().len(), 1, "no se avisó de la línea mala");
        assert_eq!(
            avisos.guardados()[0].linea,
            3,
            "el aviso señala la línea equivocada: se cuentan también las vacías y los comentarios, \
             porque el número tiene que coincidir con lo que se ve en el editor"
        );
        assert!(
            avisos.guardados()[0].to_string().contains("morado"),
            "el aviso no dice qué valor no se entendió: «{}»",
            avisos.guardados()[0]
        );
    }

    #[test]
    fn un_fichero_sin_nada_configurado_deja_el_tema_de_fabrica() {
        // **AC5 por su lado más barato**, y el que sostiene que este hito no rompe a nadie: lo que el
        // fichero no diga se queda como está.
        let (ajustes, avisos) = de("# solo comentarios\n\n   \n");

        assert_eq!(
            ajustes,
            Ajustes::default(),
            "un fichero sin ajustes cambió algo"
        );
        assert_eq!(
            avisos.cuantos(),
            0,
            "una línea vacía o de comentario se contó como problema"
        );
        assert_eq!(
            crate::theme::Theme::from_colors(&ajustes.colores).color(term_core::TermColor::Indexed(1)),
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
        let (ajustes, avisos) = de("# el azul de siempre\ncolor.4 = #123456");

        assert_eq!(
            ajustes.colores.paleta[4],
            Some(Color32::from_rgb(0x12, 0x34, 0x56)),
            "el valor de un color se perdió por empezar con almohadilla"
        );
        assert_eq!(
            avisos.cuantos(),
            0,
            "la línea de comentario o la del color se contaron como problema"
        );
    }

    #[test]
    fn una_clave_repetida_gana_la_ultima_y_se_dice() {
        // **Las dos mitades.** Que gane la última es lo que hace que pegar un tema encima de otro funcione
        // como se espera; decirlo es lo que impide que alguien pase media hora cambiando una línea que otra
        // de más abajo está pisando.
        let (ajustes, avisos) = de("color.1 = #ff0000\ncolor.1 = #00ff00");

        assert_eq!(
            ajustes.colores.paleta[1],
            Some(Color32::from_rgb(0, 0xFF, 0)),
            "ganó la primera: pegar un tema encima de otro no haría lo que se espera"
        );
        assert_eq!(avisos.guardados().len(), 1, "la clave repetida no se dijo");
        assert!(
            avisos.guardados()[0].to_string().contains("color.1"),
            "el aviso no dice qué clave estaba repetida: «{}»",
            avisos.guardados()[0]
        );
    }

    #[test]
    fn un_color_de_la_paleta_fuera_de_rango_no_se_traga_ni_desborda() {
        // La paleta tiene dieciséis huecos y el índice lo escribe una persona. Un `color.99` tiene que
        // avisar, no aplicarse en otro sitio ni salirse del vector.
        let (ajustes, avisos) = de("color.16 = #ffffff\ncolor.999 = #ffffff\ncolor.-1 = #ffffff");

        assert_eq!(
            ajustes,
            Ajustes::default(),
            "un índice fuera de la paleta cambió algo"
        );
        assert_eq!(
            avisos.cuantos(),
            3,
            "no se avisó de los tres índices imposibles"
        );
    }

    #[test]
    fn un_color_mal_escrito_no_cuela_por_parecerse() {
        // Los cuatro errores que de verdad se cometen al escribir un color a mano.
        for valor in ["ff0000", "#ff00", "#ffgg00", "#ff00000"] {
            let (ajustes, avisos) = de(&format!("fondo = {valor}"));
            assert_eq!(
                ajustes.colores.fondo, None,
                "«{valor}» se aceptó como color y no lo es"
            );
            assert_eq!(avisos.cuantos(), 1, "«{valor}» no dio aviso");
        }
    }

    #[test]
    fn una_version_mas_nueva_no_se_aplica_a_medias() {
        // **Es la única mitad de la versión que protege de algo**, y por eso es la única que existe: un
        // fichero de un formato futuro podría querer decir con una clave conocida algo distinto, así que
        // aplicar las que se entienden sería peor que no aplicar nada.
        assert_eq!(
            interpretar("version = 99\ncolor.1 = #ff0000"),
            Err(Problema::Version(99)),
            "un fichero de una versión futura se aplicó igualmente"
        );
        let (ajustes, avisos) = de("version = 1\ncolor.1 = #ff0000");
        assert_eq!(
            ajustes.colores.paleta[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "la versión conocida impidió aplicar el resto"
        );
        assert_eq!(
            avisos.cuantos(),
            0,
            "declarar la versión conocida dio un aviso"
        );
    }

    #[test]
    fn sin_declarar_version_el_fichero_se_lee_igual() {
        // **Es lo que separa este formato del de la disposición**: allí la versión es obligatoria porque el
        // fichero congela la forma de un árbol. Aquí es plano, y exigirle una línea ritual a quien lo
        // escribe no protegería de nada. Todos los demás casos de esta suite lo dan por hecho; éste lo ata.
        let (ajustes, avisos) = de("color.1 = #ff0000");

        assert_eq!(
            ajustes.colores.paleta[1],
            Some(Color32::from_rgb(0xFF, 0, 0))
        );
        assert_eq!(
            avisos.cuantos(),
            0,
            "no declarar la versión se contó como problema"
        );
    }

    #[test]
    fn los_avisos_tienen_techo_y_se_dice_cuantos_quedaron_fuera() {
        // **Un fichero pegado del sitio equivocado no debe llenar la salida de error**, pero callar los de
        // más sin decir cuántos eran haría creer que a partir del vigésimo el fichero está bien.
        let texto: String = (0..AVISOS_MAXIMOS + 7)
            .map(|n| format!("desconocida{n} = 1\n"))
            .collect();
        let (_, avisos) = de(&texto);

        assert_eq!(
            avisos.guardados().len(),
            AVISOS_MAXIMOS,
            "se guardaron más avisos que el techo"
        );
        assert_eq!(
            avisos.cuantos(),
            AVISOS_MAXIMOS + 7,
            "la cuenta se quedó en los guardados, así que el resto desaparece sin rastro"
        );
        assert_eq!(
            avisos.sin_listar(),
            7,
            "no se sabe cuántas quedaron sin listar"
        );
    }

    #[test]
    fn una_linea_con_contenido_y_sin_igual_avisa_y_una_vacia_no() {
        let (_, avisos) = de("esto no es un ajuste\n\n   \n# ni esto");

        assert_eq!(
            avisos.cuantos(),
            1,
            "o no se avisó de la línea sin «=», o se avisó de las vacías y los comentarios"
        );
    }

    #[test]
    fn no_se_dice_que_gana_una_linea_que_no_gana_nada() {
        // **El aviso mentía, y lo destapó una revisión ejecutándolo.** Con la clave repetida y el segundo
        // valor mal escrito salían DOS avisos contradictorios en la misma línea: «gana ésta» y, justo
        // debajo, que ese valor no vale. O sea que no gana: se queda la primera.
        let (ajustes, avisos) = de("color.1 = #ff0000\ncolor.1 = morado");

        assert_eq!(
            ajustes.colores.paleta[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "una línea con el valor mal escrito borró el valor bueno de antes"
        );
        assert_eq!(
            avisos.cuantos(),
            1,
            "se avisó dos veces de la misma línea, y una de las dos dice lo contrario que la otra: {:?}",
            avisos.guardados()
        );
        assert!(
            avisos.guardados()[0].to_string().contains("morado"),
            "el aviso que quedó no es el que explica qué pasa: «{}»",
            avisos.guardados()[0]
        );
    }

    #[test]
    fn de_la_version_no_se_dice_que_gane_porque_no_gana_nada() {
        // **«Gana ésta» significa que esta línea pisó el valor de otra, y la versión no guarda ningún
        // valor**: es un veto que se comprueba y se olvida. Un arreglo anterior la metió en esa
        // contabilidad y produjo el aviso más absurdo posible — «gana ésta» sobre una línea cuyo valor
        // tampoco se entendía, o sea que ni ganaba ni se aplicaba. Lo cazó una revisión ejecutándolo.
        let (_, avisos) = de("version = 1\nversion = 1");
        assert_eq!(
            avisos.cuantos(),
            0,
            "se dijo que una versión repetida «gana», y no gana nada: {:?}",
            avisos.guardados()
        );

        let (_, avisos) = de("version = uno\nversion = uno");
        assert_eq!(
            avisos.cuantos(),
            2,
            "cada versión ilegible tiene que avisar una vez y solo una: {:?}",
            avisos.guardados()
        );
        assert!(
            avisos
                .guardados()
                .iter()
                .all(|a| !matches!(a.falta, Falta::Repetida(_))),
            "se dijo «gana ésta» de una línea cuyo valor ni siquiera se entendió: {:?}",
            avisos.guardados()
        );
    }

    #[test]
    fn el_tamano_de_letra_se_lee_y_se_escribe_de_las_dos_formas() {
        // **Las dos grafías valen a propósito**: quien teclea la configuración de un terminal no siempre
        // tiene una eñe a mano —un teclado que no es el suyo, una sesión remota— y rechazar `tamano` sería
        // un fallo que quien lo sufre no puede ni ver en pantalla.
        for clave in ["tamaño", "tamano"] {
            let (ajustes, avisos) = de(&format!("{clave} = 18"));
            assert_eq!(ajustes.cuerpo, Some(18.0), "«{clave}» no se entendió");
            assert_eq!(avisos.cuantos(), 0, "«{clave}» dio aviso");
        }
        let (ajustes, _) = de("tamaño = 12.5");
        assert_eq!(
            ajustes.cuerpo,
            Some(12.5),
            "un tamaño con decimales no se entendió"
        );
    }

    #[test]
    fn un_tamano_imposible_no_deja_la_ventana_sin_rejilla() {
        // **Fuera de rango la ventana deja de ser un terminal**: con dos puntos no se lee nada, con
        // doscientos caben tres columnas, y el reparto de las divisiones tiene mínimos que dejarían de
        // poder cumplirse. Un cero o un negativo son peores: la celda mediría cero y la rejilla saldría
        // de dividir por él.
        for valor in ["0", "-14", "1", "500", "nan", "inf", "grande"] {
            let (ajustes, avisos) = de(&format!("tamaño = {valor}"));
            assert_eq!(
                ajustes.cuerpo, None,
                "«{valor}» se aceptó como cuerpo de letra"
            );
            assert_eq!(avisos.cuantos(), 1, "«{valor}» no dio aviso");
        }
        // Y los extremos que sí valen, para que el rango no se cierre de más: hay quien trabaja a 8 y
        // quien presenta a 40.
        for valor in ["4", "8", "40", "96"] {
            let (ajustes, _) = de(&format!("tamaño = {valor}"));
            assert!(
                ajustes.cuerpo.is_some(),
                "«{valor}» se rechazó y es un tamaño de trabajo"
            );
        }
    }

    #[test]
    fn el_indice_de_la_paleta_son_digitos_y_nada_mas() {
        // **La asimetría que había no se podía explicar**: `color.+1` aplicaba un color y `color.-1` se
        // rechazaba, las dos por dejarle la decisión al intérprete de números. Lo cazó una revisión
        // ejecutándolas.
        for clave in ["color.+1", "color.-1", "color. 1", "color.uno", "color."] {
            let (ajustes, avisos) = de(&format!("{clave} = #ff0000"));
            assert_eq!(
                ajustes,
                Ajustes::default(),
                "«{clave}» aplicó un color, y no es una forma que nadie haya documentado"
            );
            assert_eq!(avisos.cuantos(), 1, "«{clave}» no dio aviso");
        }

        // **Los ceros a la izquierda SÍ valen, y eso es una decisión, no un descuido**: quien escribe
        // `color.07` quiere el siete y no hay otra lectura posible. Rechazarlo sería hostil por gusto.
        let (ajustes, avisos) = de("color.07 = #ff0000");
        assert_eq!(
            ajustes.colores.paleta[7],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "«color.07» no aplicó el color siete"
        );
        assert_eq!(avisos.cuantos(), 0);
    }

    #[test]
    fn un_valor_larguisimo_no_se_vuelca_entero_en_el_aviso() {
        // **El valor sale del fichero tal cual**, y un `config` copiado de cualquier sitio puede traer una
        // línea de cien mil caracteres o unos códigos de escape que le hablen al terminal que lanzó la
        // aplicación. Lo cazó una revisión.
        let largo = "x".repeat(100_000);
        let (_, avisos) = de(&format!("fondo = {largo}"));
        let dicho = avisos.guardados()[0].to_string();
        assert!(
            dicho.chars().count() < 200,
            "el aviso salió con {} caracteres: se está volcando el valor entero",
            dicho.chars().count()
        );

        let (_, avisos) = de("fondo = \u{1b}[31mROJO\u{1b}[0m");
        assert!(
            !avisos.guardados()[0].to_string().contains('\u{1b}'),
            "el aviso deja pasar códigos de escape al terminal que lanzó la aplicación"
        );
    }

    #[test]
    fn a_quien_se_le_olvida_el_valor_se_le_dice_eso_y_no_otra_cosa() {
        // **Decirle «no tiene la forma clave = valor» a quien escribió `fondo =` le manda a mirar donde no
        // es**: el igual está puesto, lo que falta es el color.
        let (_, avisos) = de("fondo =");
        assert_eq!(avisos.cuantos(), 1, "no se avisó de la línea a medias");
        let dicho = avisos.guardados()[0].to_string();
        assert!(
            dicho.contains("falta la clave o el valor"),
            "el aviso describe mal el fallo: «{dicho}»"
        );

        let (_, avisos) = de("esto no lleva igual");
        assert!(
            avisos.guardados()[0]
                .to_string()
                .contains("«clave = valor»"),
            "una línea sin igual tiene que decir eso, y dijo: «{}»",
            avisos.guardados()[0]
        );
    }

    #[test]
    fn la_marca_de_orden_de_bytes_no_se_come_la_primera_linea() {
        // **Es el fallo más cruel que este fichero podía tener**, y lo destapó probar en vez de leer: la
        // marca la meten los editores de Windows y algunas páginas al copiar, se queda pegada a la primera
        // clave, y el aviso señala algo que **en pantalla se ve idéntico** a la clave correcta. Quien lo
        // sufra no tiene forma de ver la diferencia.
        let (ajustes, avisos) = de("\u{FEFF}color.1 = #ff0000\nfondo = #010203");

        assert_eq!(
            ajustes.colores.paleta[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "la marca de orden de bytes se comió la primera línea"
        );
        assert_eq!(
            avisos.cuantos(),
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
        let (ajustes, avisos) = de("# un tema\r\ncolor.1 = #ff0000\r\nfondo = #010203\r\n");

        assert_eq!(
            ajustes.colores.paleta[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "el retorno de carro se quedó pegado al valor"
        );
        assert_eq!(ajustes.colores.fondo, Some(Color32::from_rgb(1, 2, 3)));
        assert_eq!(
            avisos.cuantos(),
            0,
            "un fichero con saltos de Windows dio avisos"
        );
    }

    #[test]
    fn el_color_se_escribe_en_mayusculas_o_en_minusculas() {
        // Se copian de páginas que los escriben de las dos formas, así que rechazar una sería un fallo
        // que quien lo sufre no puede ni ver.
        let (ajustes, avisos) = de("color.1 = #AABBCC\ncolor.2 = #aabbcc");

        assert_eq!(ajustes.colores.paleta[1], ajustes.colores.paleta[2]);
        assert_eq!(avisos.cuantos(), 0, "una de las dos formas se rechazó");
    }

    #[test]
    fn un_valor_con_igual_dentro_no_se_parte_por_el_segundo() {
        // Hoy ningún ajuste lleva un igual en el valor, pero la fuente de la entrega siguiente puede
        // llevarlo en su nombre. Partir por el **primer** igual es lo que hace que eso no muerda después.
        let (_, avisos) = de("fuente = Iosevka Term = Extended");

        assert_eq!(
            avisos.cuantos(),
            1,
            "se avisó más de una vez, así que la línea se partió por más de un igual"
        );
        // **La clave tiene que ser EXACTAMENTE «fuente».** Comprobar solo que el aviso «contiene fuente»
        // no ata nada: partiendo por el último igual, la clave sale como «fuente = Iosevka Term», que
        // también lo contiene. Se comprobó mutándolo.
        assert_eq!(
            avisos.guardados()[0].falta,
            Falta::ClaveDesconocida("fuente".to_owned()),
            "la línea se partió por un igual que no es el primero"
        );
    }

    #[test]
    fn los_atajos_se_leen_del_fichero_en_el_orden_en_que_se_escriben() {
        // **El orden importa y por eso llegan como lista**: «gana la última» solo se puede decidir
        // aplicándolas en el orden en que están escritas.
        let (ajustes, avisos) = de(
            "atajo.partir-al-lado = ctrl+shift+e\n             atajo.cerrar-division = ninguno\n             atajo.pestaña-nueva = ctrl+alt+t",
        );
        assert_eq!(
            avisos.cuantos(),
            0,
            "alguna de las tres líneas no se entendió"
        );
        assert_eq!(ajustes.atajos.len(), 3, "no llegaron los tres atajos");
        assert_eq!(
            ajustes.atajos[0].0,
            crate::keyboard::Action::SplitRight,
            "el orden de lectura no se conserva, y «gana la última» deja de poder decidirse"
        );
        assert_eq!(ajustes.atajos[1].1, None, "«ninguno» no desató la acción");
    }

    #[test]
    fn el_atajo_de_copiar_solo_se_puede_apagar() {
        // **Su acorde lo decide el escritorio**, que se lo queda antes de que llegue aquí y ni siquiera
        // dice si había Mayúsculas. Aceptar `atajo.copiar = ctrl+shift+e` sería prometer un acorde que
        // no se dispararía nunca y que quien lo escribió no tendría forma de depurar.
        let (ajustes, avisos) = de("atajo.copiar = ninguno");
        assert_eq!(avisos.cuantos(), 0, "«ninguno» no se entendió");
        assert_eq!(ajustes.copiar, Some(false), "«ninguno» no apagó el copiar");
        assert!(
            ajustes.atajos.is_empty(),
            "copiar se coló en la lista de acordes, donde no tiene ninguno que mover"
        );

        let (ajustes, avisos) = de("atajo.copiar = ctrl+shift+e");
        assert_eq!(avisos.cuantos(), 1, "reasignarlo no se avisó");
        assert_eq!(ajustes.copiar, None, "reasignarlo tocó el interruptor");
        assert!(
            format!("{}", avisos.guardados()[0]).contains("apagarlo"),
            "el aviso no dice lo único que se puede hacer con ese acorde: {}",
            avisos.guardados()[0]
        );

        // Y **no se apaga sin pedirlo**: un fichero que no lo nombra lo deja como estaba.
        let (ajustes, _) = de("fondo = #101010");
        assert_eq!(
            ajustes.copiar, None,
            "un fichero que no habla de copiar lo dejó dicho igualmente"
        );
    }

    #[test]
    fn un_atajo_mal_escrito_avisa_y_no_se_lleva_a_los_demas() {
        // La misma regla que el resto del fichero, en la superficie que más se teclea a mano.
        for (linea, que) in [
            ("atajo.inventada = ctrl+shift+e", "una acción que no existe"),
            (
                "atajo.cerrar-division = ctrl+shift+noexiste",
                "una tecla que no existe",
            ),
            ("atajo.cerrar-division = e", "un acorde sin tecla de mando"),
            ("atajo.cerrar-division = ", "sin valor"),
        ] {
            let (ajustes, avisos) = de(&format!("color.1 = #ff0000\n{linea}\ncolor.2 = #00ff00"));
            assert!(
                ajustes.atajos.is_empty(),
                "{que} se aplicó igualmente: «{linea}»"
            );
            assert_eq!(avisos.cuantos(), 1, "{que} no dio aviso: «{linea}»");
            assert!(
                ajustes.colores.paleta[1].is_some() && ajustes.colores.paleta[2].is_some(),
                "{que} se llevó por delante las líneas buenas de al lado"
            );
        }
    }

    #[test]
    fn los_veintidos_colores_llegan_al_tema() {
        // **Se afirma sobre el color que el tema devuelve, con el valor literal escrito en la prueba.**
        // Compararlo contra `Theme::from_colors(...)` otra vez mediría que el código es consistente consigo mismo:
        // cruzar dos colores dejaría los dos lados de la igualdad cambiados y la prueba en verde. Este
        // proyecto ya tiene fichado ese disfraz, con 525 pruebas sobreviviendo a un cruce de colores.
        let texto = "color.0 = #010101\ncolor.15 = #0f0f0f\nfondo = #020202\ntexto = #030303\n\
                     cursor = #040404\ndivisor = #050505\nfoco = #060606\npanel = #070707";
        let (ajustes, avisos) = de(texto);
        assert_eq!(
            avisos.cuantos(),
            0,
            "alguna de las ocho líneas no se entendió"
        );

        let tema = crate::theme::Theme::from_colors(&ajustes.colores);

        assert_eq!(
            tema.color(term_core::TermColor::Indexed(0)),
            Color32::from_rgb(1, 1, 1),
            "el color 0 de la paleta no llegó al tema"
        );
        assert_eq!(
            tema.color(term_core::TermColor::Indexed(15)),
            Color32::from_rgb(0x0F, 0x0F, 0x0F),
            "el color 15 no llegó: la paleta se está copiando a medias"
        );
        assert_eq!(
            tema.background(),
            Color32::from_rgb(2, 2, 2),
            "el fondo no llegó"
        );
        assert_eq!(
            tema.foreground(),
            Color32::from_rgb(3, 3, 3),
            "el texto no llegó"
        );
        assert_eq!(
            tema.divider(),
            Color32::from_rgb(5, 5, 5),
            "el divisor no llegó"
        );
        assert_eq!(tema.focus(), Color32::from_rgb(6, 6, 6), "el foco no llegó");
        assert_eq!(
            tema.panel_background(),
            Color32::from_rgb(7, 7, 7),
            "el fondo del panel no llegó"
        );
        // **El cursor se afirma por donde el pintor lo lee, y era el único de los seis que nadie
        // comprobaba.** Medido: cambiar su asignación por la del foco dejaba las 718 pruebas de `app`
        // en verde, así que el defecto y el arreglo daban el mismo resultado. Y va por `TermColor`
        // porque es la puerta por la que el pintor pide ese color, no por un campo: afirmar sobre el
        // campo comprobaría que el valor está apuntado, no que se usa.
        assert_eq!(
            tema.color(term_core::TermColor::Cursor),
            Color32::from_rgb(4, 4, 4),
            "el color del cursor no llegó"
        );
    }

    #[test]
    fn pedir_un_solo_color_no_borra_los_otros_quince() {
        // **Es lo que permite cambiar un color sin escribir los veintidós**, y el modo de fallo es que la
        // paleta se sustituya entera en vez de hueco a hueco: quedarían quince negros.
        let (ajustes, _) = de("color.1 = #ff0000");
        let tema = crate::theme::Theme::from_colors(&ajustes.colores);

        assert_eq!(
            tema.color(term_core::TermColor::Indexed(1)),
            Color32::from_rgb(0xFF, 0, 0),
            "el color pedido no se aplicó"
        );
        assert_eq!(
            tema.color(term_core::TermColor::Indexed(2)),
            crate::theme::Theme::default().color(term_core::TermColor::Indexed(2)),
            "pedir un color cambió otro: la paleta se está sustituyendo entera"
        );
    }

    /// Un directorio propio por prueba, con el número de proceso.
    ///
    /// **La suite corre en paralelo y el nombre del fichero es fijo**, así que dos pruebas que
    /// compartieran directorio se leerían la configuración la una a la otra y fallarían solo a veces. Es
    /// el mismo molde que usa el módulo hermano, y por la misma razón.
    struct Directorio(PathBuf);

    impl Directorio {
        fn nueva(nombre: &str) -> Self {
            let ruta = std::env::temp_dir().join(format!(
                "agent-terminal-ajustes-{}-{nombre}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&ruta);
            std::fs::create_dir_all(&ruta).expect("crear el directorio de la prueba");
            Self(ruta)
        }

        fn con(&self, contenido: &[u8]) -> PathBuf {
            let ruta = self.0.join(FICHERO);
            std::fs::write(&ruta, contenido).expect("escribir el fichero de la prueba");
            ruta
        }
    }

    impl Drop for Directorio {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn un_fichero_que_se_pasa_del_techo_no_se_lee() {
        // **El techo existe porque el fichero tiene tamaño arbitrario**, y leerlo sin tope es reservar sin
        // tope en el arranque. Sin esta prueba, el techo se podía bajar a un byte con la suite entera en
        // verde — comprobado por una revisión con siete mutaciones vivas en este camino.
        let dir = Directorio::nueva("techo");
        let justo = dir.con(&vec![b'#'; BYTES_MAXIMOS]);
        assert!(
            leer(&justo).is_ok(),
            "un fichero de exactamente el techo tiene que leerse"
        );

        let pasado = dir.con(&vec![b'#'; BYTES_MAXIMOS + 1]);
        assert_eq!(
            leer(&pasado),
            Err(Problema::DemasiadoGrande),
            "un fichero de un byte por encima del techo se leyó igualmente"
        );
    }

    #[test]
    fn el_techo_y_el_tope_de_avisos_son_los_que_dice_el_documento() {
        // **Las dos constantes están publicadas en `docs/08-configuracion.md`**, y las pruebas que las usan
        // se parametrizan **con ellas mismas**: mover el número no tumbaba nada mientras el documento
        // seguía prometiendo el viejo. Es el disfraz que este proyecto tiene fichado —elegir un número con
        // el que el defecto y el arreglo dan lo mismo— y hace falta escribirlos literales una vez.
        assert_eq!(BYTES_MAXIMOS, 64 * 1024, "el documento promete 64 KiB");
        assert_eq!(
            AVISOS_MAXIMOS, 20,
            "el documento promete hasta veinte avisos"
        );
    }

    #[test]
    fn lo_que_no_es_un_fichero_no_se_intenta_leer() {
        let dir = Directorio::nueva("no-fichero");
        let como_directorio = dir.0.join(FICHERO);
        std::fs::create_dir_all(&como_directorio)
            .expect("crear el directorio con el nombre del fichero");

        assert_eq!(
            leer(&como_directorio),
            Err(Problema::NoEsFichero),
            "un directorio con el nombre del fichero se intentó leer como configuración"
        );
    }

    #[test]
    fn un_fichero_que_no_es_texto_deja_el_terminal_de_fabrica() {
        // Está prometido por escrito en el documento que se le da a quien escribe el fichero.
        let dir = Directorio::nueva("no-texto");
        let ruta = dir.con(&[0xFF, 0xFE, 0x00, 0x80]);

        assert_eq!(
            leer(&ruta),
            Err(Problema::NoEsTexto),
            "unos bytes que no son texto se interpretaron igualmente"
        );
    }

    #[test]
    fn el_fichero_que_no_esta_se_calla_y_el_enlace_roto_no() {
        // **La diferencia importa y no la veía nadie.** Quien no ha escrito configuración no debe oír nada
        // en cada arranque; quien la escribió y se la movieron **tiene que enterarse**, y antes salía por
        // el mismo camino silencioso. Lo cazó una revisión.
        let dir = Directorio::nueva("ausente");
        let nunca = dir.0.join(FICHERO);
        assert_eq!(leer(&nunca), Err(Problema::Ausente));
        assert!(
            !Problema::Ausente.merece_aviso(),
            "se avisaría en cada arranque a quien no ha configurado nada"
        );

        #[cfg(unix)]
        {
            let enlace = dir.0.join("enlace");
            std::os::unix::fs::symlink(dir.0.join("no-existe"), &enlace).expect("crear el enlace");
            assert_eq!(
                leer(&enlace),
                Err(Problema::EnlaceRoto),
                "un enlace roto se confundió con «no hay fichero»"
            );
            assert!(
                Problema::EnlaceRoto.merece_aviso(),
                "un enlace roto se calla, y quien lo creó no se entera de que su configuración no se aplica"
            );
        }
    }

    #[test]
    fn un_fichero_de_verdad_se_lee_de_verdad() {
        // **Ninguna de las veinte pruebas anteriores llamaba a `leer`**: todas entraban por el intérprete,
        // así que el camino de disco entero se podía borrar con la suite en verde. Lo cazó una revisión con
        // siete mutaciones vivas.
        let dir = Directorio::nueva("de-verdad");
        let ruta = dir.con(
            b"# un tema
color.1 = #ff0000
malo
",
        );

        let (ajustes, avisos) = leer(&ruta).expect("el fichero de la prueba tenía que leerse");
        assert_eq!(
            ajustes.colores.paleta[1],
            Some(Color32::from_rgb(0xFF, 0, 0))
        );
        assert_eq!(
            avisos.cuantos(),
            1,
            "la línea mala no avisó al leer del disco"
        );
    }

    #[test]
    fn el_fichero_se_busca_donde_dice_el_documento() {
        // **`ruta_de` se sacó aparte explícitamente «para poder probarla sin tocar el entorno», y no se
        // probaba**: apuntarla al fichero de la disposición dejaba la suite en verde. Lo cazó una revisión.
        let ruta = ruta_de(Some(std::ffi::OsStr::new("/casa/config")), None)
            .expect("una ruta absoluta tiene que dar sitio");
        assert_eq!(
            ruta,
            PathBuf::from("/casa/config/agent-terminal/config"),
            "el fichero no se busca donde el documento dice"
        );

        assert_eq!(
            ruta_de(None, Some(std::ffi::OsStr::new("/casa"))),
            Some(PathBuf::from("/casa/.config/agent-terminal/config")),
            "sin la variable propia no se cae a «.config» de la casa"
        );
        assert_eq!(
            ruta_de(None, Some(std::ffi::OsStr::new("relativa"))),
            None,
            "una ruta relativa se aceptó: el fichero acabaría dentro del directorio de trabajo"
        );
    }
}
