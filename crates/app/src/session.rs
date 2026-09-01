//! El formato con el que la disposición de la ventana va y vuelve de disco, y lo que se le exige.
//!
//! **Aquí no hay ni una consola.** Este módulo describe formas y las valida; materializarlas —que es
//! bifurcar un proceso por hoja— ocurre en la raíz, que es donde viven los campos de una división. La
//! frontera no es de gusto: mantiene esta mitad **pura y probable sin abrir un pseudoterminal**, y deja
//! el `fork` en un solo sitio.
//!
//! **Y los tipos de aquí son un espejo, no los de la disposición viva.** Cambiarle un campo al árbol de
//! [`crate::Node`] no debe cambiar lo que hay escrito en el disco de nadie; el precio es una conversión
//! de ida y otra de vuelta, y lo que compra es que las dos formas evolucionen por separado. La única
//! excepción, y por su razón, es el peso: ver [`StoredChild::weight`].
//!
//! **La segunda mitad del módulo sí toca el disco**, y también sin abrir ninguna consola: dónde vive el
//! fichero ([`path_from`]), cómo se lee ([`read`]) y cómo se sustituye sin poder dejarlo a medias
//! ([`Persistence::save`]). Está aquí y no en quien arranca porque el nombre del fichero, su techo y su
//! versión **son el formato**: quien los mirara por su cuenta los estaría escribiendo por segunda vez.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Cuántas hojas se aceptan en un documento entero.
///
/// **Existe aunque D14 descartara ponerle techo al número de divisiones, y no la contradice.** Partir
/// exige un gesto humano y la geometría **se niega antes de crear nada**; restaurar bifurca N shells
/// **antes de que exista geometría** —al abrir no se sabe el tamaño de la ventana—, así que el suelo que
/// protege a partir está **aguas abajo del `fork`**. Es techo **del documento**, no de divisiones.
///
/// **Y el número se elige contra el recurso que guarda, no contra lo que cabe en pantalla.** Cada hoja
/// restaurada es un proceso, un pseudoterminal, sus descriptores y ~1,06 MiB de colas: un millar de hojas
/// —lo que deja pasar el techo de bytes— serían más de un gigabyte de colas antes de pintar nada, o sea
/// el techo convertido en el desastre que venía a evitar. 128 son unos 136 MiB, y siguen estando por
/// encima de las del orden de cien consolas que caben en la ventana con la que se mide.
pub const MAX_LEAVES: usize = 128;

/// La versión del formato.
///
/// **Es obligatoria, al contrario que la del contrato del panel, y por la razón contraria.** Allí puede
/// faltar porque hay ficheros anteriores al campo, escritos por terceros; aquí no existe ninguno, porque
/// este fichero lo escribe siempre la aplicación.
///
/// Sube **solo con lo que un lector viejo malinterpretaría** —una clave renombrada, quitada, o con otro
/// significado—; **añadir una clave opcional no la sube**, y que se quede en 2 mucho tiempo es la señal
/// de que funciona. Va **una vez por documento y nunca por nodo**: versiones anidadas dejarían mezclar
/// formas dentro de un mismo fichero.
pub const VERSION: u64 = 2;

/// Lo más grande que se acepta interpretar.
///
/// **Está aquí y no en quien lea el fichero, y eso es la decisión.** D16 compromete este techo, y si
/// viviera en el camino que abre el fichero, quien escriba **otro** camino que interprete un documento
/// tendría que acordarse por su cuenta. En el módulo que posee el formato no hay nada que recordar.
///
/// **64 KiB y no los 256 del contrato del panel**: el documento legítimo más grande que este formato
/// produce son unos 27 KiB —el techo de hojas por lo que ocupa cada una—, así que esto es 2,4 veces eso.
/// Y la razón de que haya techo no es el pico de interpretar: es que **el fichero tiene tamaño
/// arbitrario, y leerlo sin tope es reservar sin tope en el arranque**.
pub const MAX_BYTES: usize = 64 * 1024;

/// Lo que se guarda de una consola, y es todo lo que se guarda de ella.
///
/// De los quince campos de una división viva, dos. Los otros trece no, y cada uno por su razón: el
/// nombre porque el contador que garantiza que no se repitan es de la ventana y un nombre viejo chocaría
/// con él; el directorio del panel y su estado porque una consola restaurada **estrena identidad**; el
/// pseudoterminal y su pantalla porque son un proceso vivo y un historial; y los cronómetros porque son
/// de la corrida.
///
/// **Y ningún campo de texto libre, que es una decisión y no una casualidad.** Ni programa, ni
/// argumentos, ni entorno, ni directorio de trabajo, ni ninguna ruta. El lanzamiento de una consola deja
/// de ser inofensivo en cuanto el programa o sus argumentos **procedan de configuración**, y este fichero
/// es configuración: si el tipo no lo tiene, no hay nada que conectar a un `spawn`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct StoredLeaf {
    /// En celdas, como en la división viva: lo que el panel tiene que conservar al cambiar el aumento
    /// del escritorio es **cuántos caracteres caben**, no cuántos puntos ocupa.
    pub panel_width: usize,
    /// Aparte del ancho a propósito: plegar no pierde el ancho de trabajo, y desplegar vuelve **al que
    /// tenía** en vez de inventarse uno.
    pub folded: bool,
}

/// En qué sentido reparte una rama.
///
/// **Es un tipo propio y no [`crate::Orientation`], a propósito.** Los nombres de este enum son **el
/// formato**: aparecen literalmente en el fichero de todo el que use la aplicación. Reutilizar el tipo
/// vivo ataría el fichero a un nombre que el código puede querer cambiar mañana por razones que no
/// tienen nada que ver con el disco. El precio son cuatro líneas de conversión.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoredOrientation {
    /// Una al lado de otra; se reparte el ancho.
    Columns,
    /// Una encima de otra; se reparte el alto.
    Rows,
}

/// Una consola, o un corte con sus partes.
///
/// **Etiqueta explícita —`leaf` o `branch`— y no «se deduce de qué clave está presente».** La forma sin
/// etiqueta se lee igual de bien y **pierde el motivo del fallo**: quien intente resolver la disyuntiva
/// probando formas devuelve «no encaja en ninguna», y lo que hace falta decir es **qué** clave sobra o
/// falta y **dónde**. La etiqueta la escribe `serde` de serie, así que el motivo sale gratis.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoredNode {
    Leaf(StoredLeaf),
    Branch {
        orientation: StoredOrientation,
        children: Vec<StoredChild>,
    },
}

/// Un hijo de una rama, con lo que le toca del reparto.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StoredChild {
    /// **Un número suelto, y no el tipo del peso vivo, y esto sí es deliberado.** Derivar la lectura
    /// sobre [`crate::Weight`] **se saltaría su constructor**, que es lo único que hace imposible una suma
    /// de pesos igual a cero — el `NaN` en un rectángulo que ese tipo existe para evitar. Así que el
    /// cable lleva `f32` y la validación lo pasa por `crate::Weight::new`: **una sola implementación de la
    /// invariante**, la que ya estaba probada.
    pub weight: f32,
    pub node: StoredNode,
}

/// Una pestaña.
///
/// **Un objeto con una sola clave en vez del nodo suelto**, para poder añadirle algo opcional mañana sin
/// subir la versión. No lleva ni el foco ni el nombre: la ventana abre en la primera pestaña, y los
/// nombres no se reutilizan.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StoredTab {
    pub root: StoredNode,
}

/// El documento entero, tal como se lee del disco: **sin validar todavía**.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StoredDocument {
    /// La versión del formato. La clave es corta porque aparece en todos los ficheros y no dice nada
    /// más de lo que dice.
    pub v: u64,
    pub tabs: Vec<StoredTab>,
}

/// Un documento que **ya pasó por la validación**, y por eso se puede materializar.
///
/// **El campo es privado y no hay más forma de construirlo que [`validate`]**, así que «este árbol está
/// bien formado» es propiedad del tipo y no de que alguien se acuerde de comprobarlo antes. Es la misma
/// doctrina que [`crate::Weight`] un nivel más arriba, y por el mismo motivo: este proyecto tiene contado
/// con números lo que pasa con las invariantes que dependen de la memoria de quien escribe.
#[derive(Clone, PartialEq, Debug)]
pub struct Validated {
    document: StoredDocument,
    /// **La cuenta que hizo la validación, guardada en vez de recalculada.** Recorrer el árbol otra vez
    /// para volver a contar lo mismo tiene dos costes y solo uno es el trabajo: el otro es que **el
    /// número que se comprobó contra el techo y el que lee quien llama podrían dejar de ser el mismo**.
    leaves: usize,
}

impl Validated {
    /// Las pestañas del documento, en orden.
    pub fn tabs(&self) -> &[StoredTab] {
        &self.document.tabs
    }

    /// Cuántas consolas va a haber que abrir. Se sabe **antes** de abrir ninguna.
    pub fn leaves(&self) -> usize {
        self.leaves
    }
}

/// Por qué un documento no sirve.
///
/// **La tolerancia es por documento y no por campo, al contrario que en el contrato del panel.** Aquel
/// lo escribe un agente a mano y perder el panel entero por un campo que nadie mira sería lo peor
/// posible; este lo escribe siempre la aplicación, así que si no se entiende, **abrir de fábrica es una
/// respuesta correcta y barata** y adivinar media disposición es peor que no restaurarla.
///
/// Cada variante lleva **qué** y **dónde**, porque el único canal para decirlo es una línea.
///
/// **Un solo tipo para los fallos del fichero y los de la forma, y eso lo decide el molde.** El contrato
/// del panel reúne los suyos igual —«no está», «no es un fichero», «no se entiende»— en un enum
/// (`crates/panel-contract/src/panel.rs:121`), y por la razón que aquí vale doble: quien lee no puede
/// hacer nada distinto con un fichero ausente y con uno corrupto salvo **abrir de fábrica**, así que
/// partirlos en dos tipos daría dos caminos que acaban en la misma línea de aviso. El techo de bytes ya
/// estaba en este enum antes de que hubiera fichero, que es la misma frontera.
#[derive(Clone, PartialEq, Debug)]
pub enum LayoutProblem {
    /// No hay fichero todavía. **Es el primer arranque de todos, así que no se avisa de él**: la
    /// distinción con «hay algo y no se entiende» es justo lo que hace que el caso normal sea callado.
    Absent,
    /// Hay algo con ese nombre y no es un fichero.
    ///
    /// **Se comprueba antes de abrir**, con los datos que hacía falta pedir de todas formas —son la
    /// pista de reserva de [`read`]—, así que la rama es gratis. Y lo que evita importa: una tubería con
    /// nombre ahí **bloquearía la apertura**, y esto ocurre antes de que la ventana exista, así que no
    /// abriría nunca.
    ///
    /// **Cubre el descuido, no una carrera**, igual que en el molde: pedir los datos y abrir son dos
    /// resoluciones distintas de la misma ruta, y entre las dos cabe una sustitución. Quien pueda hacerla
    /// es el propio usuario en su propio directorio de configuración.
    NotAFile,
    /// No se pudo abrir o leer.
    ///
    /// **Lleva el motivo como texto y no como el error del sistema**, para que este enum siga siendo
    /// comparable consigo mismo: el error de entrada y salida no se puede clonar ni comparar, y las
    /// pruebas de la tabla de fallos afirman sobre el problema entero.
    Unreadable(String),
    /// Se leyó, y los bytes no son texto.
    NotText,
    /// No es JSON, o no encaja en la forma. Trae el motivo tal como lo da el intérprete, que ya nombra
    /// la clave y la posición.
    Unintelligible(String),
    /// La versión no es la que este binario conoce. **Qué hacer con eso no se decide aquí**: una más
    /// nueva no se puede sobrescribir y una más vieja habría que leerla con su lector, y las dos cosas
    /// son del camino que toca el fichero.
    Version { found: u64 },
    /// Un documento sin pestañas no describe ninguna ventana.
    NoTabs,
    /// Una rama con menos de dos hijos. **La invariante que sostiene la razón de ser del árbol**: si una
    /// rama puede tener una sola hija, vuelve el anidamiento que hace que una frontera arrastre un
    /// bloque de dos consolas, que es lo que más se le critica a `tmux`.
    SingleChildBranch { tab: usize },
    /// Dos ramas del mismo sentido, una dentro de otra. La otra mitad de la misma invariante: partir
    /// inserta como hermana justamente para no construir esto.
    NestedBranches { tab: usize },
    /// Un peso que el tipo del peso vivo rechaza —cero, negativo, o no un número— o que se sale del
    /// rango que el reparto puede multiplicar.
    ///
    /// **No lleva el valor, y eso lo decidió una revisión.** Llevándolo, este tipo se comparaba mal
    /// consigo mismo: un peso `NaN` guardado aquí hace que `PesoInvalido != PesoInvalido`, porque `NaN`
    /// no es igual a nada, ni a sí mismo. El síntoma ya estaba en las pruebas —la de los pesos era **la
    /// única** obligada a comprobar solo la variante mientras sus vecinas comparaban el problema
    /// entero—, así que dejaba de atar **cuál** peso se rechazó. El valor va en el mensaje, que es donde
    /// lo lee quien tiene que arreglarlo.
    InvalidWeight { tab: usize },
    /// Los pesos de una rama son cada uno un número **y su suma no lo es**.
    ///
    /// **No es «los pesos tienen que sumar algo concreto», que se descartó a propósito**: son
    /// proporciones, y `{3, 1}` y `{0,5, 1,5}` describen el mismo reparto. Es que el reparto **divide
    /// cada peso por la suma de su rama**, así que con una suma infinita a **todas** les toca cero — dos
    /// pesos de 3·10³⁸ son cada uno finito y positivo, y sumados desbordan. La otra dirección **no hace
    /// falta comprobarla**: dos números positivos no pueden sumar cero.
    ///
    /// **Y es la puerta que sostiene el formato entero desde que no hay rango propio de pesos.** La suma de
    /// una rama es lo único que los gestos **no** hacen crecer —arrastrar la conserva, partir divide en dos
    /// mitades que suman lo mismo, cerrar reparte el peso del grupo entre sus nietas—, así que exigirla
    /// finita aquí es exigir exactamente la invariante que el árbol vivo mantiene. Ni una condición más ni
    /// una menos: eso es lo que hace que la ida y vuelta no pueda perder una disposición.
    NonFiniteSum { tab: usize },
    /// Más consolas de las que se aceptan abrir de golpe.
    TooManyLeaves { count: usize },
    /// Más bytes de los que se aceptan interpretar.
    TooLarge { bytes: u64 },
}

impl std::fmt::Display for LayoutProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => write!(f, "no hay ninguna disposición guardada"),
            Self::NotAFile => write!(f, "hay algo ahí que no es un fichero"),
            Self::Unreadable(reason) => write!(f, "no se pudo leer ({reason})"),
            Self::NotText => write!(f, "no es texto UTF-8"),
            Self::Unintelligible(reason) => write!(f, "no se entiende: {reason}"),
            Self::Version { found } => {
                write!(
                    f,
                    "lo escribió la versión {found} del formato y esta conoce la {VERSION}"
                )
            }
            Self::NoTabs => write!(f, "no trae ninguna pestaña"),
            Self::SingleChildBranch { tab } => {
                write!(f, "la pestaña {tab} tiene un corte con un solo hijo")
            }
            Self::NestedBranches { tab } => write!(
                f,
                "la pestaña {tab} anida dos cortes del mismo sentido, que es la forma que hace \
                 que un divisor arrastre dos consolas"
            ),
            Self::InvalidWeight { tab } => write!(
                f,
                "la pestaña {tab} trae un peso que no puede serlo: ni cero, ni negativo, ni infinito"
            ),
            Self::NonFiniteSum { tab } => write!(
                f,
                "la pestaña {tab} tiene un corte cuyos pesos son números y su suma no: repartir \
                 dejaría a todas sus consolas con cero de ancho"
            ),
            Self::TooManyLeaves { count } => write!(
                f,
                "pide {count} consolas y no se abren más de {MAX_LEAVES} de golpe"
            ),
            Self::TooLarge { bytes } => write!(
                f,
                "ocupa {bytes} bytes y no se interpretan más de {MAX_BYTES}"
            ),
        }
    }
}

/// Lo único que se le exige a un documento **antes** de exigirle su forma.
///
/// **Existe porque mirar la versión después de encajar la forma es no tener versión.** La versión sube
/// justo con lo que un lector viejo malinterpretaría —una clave renombrada, quitada, o con otro
/// significado—, o sea **precisamente con lo que hace fallar al intérprete de la forma**. Leyéndola
/// después, un fichero de la versión 2 con una clave nueva no se distingue de uno nuestro estropeado, y el
/// camino que guarda lo sobrescribe: un binario viejo llevándose por delante la disposición del nuevo, que
/// es el daño exacto que la versión existe para evitar.
#[derive(Deserialize)]
struct StoredProbe {
    v: u64,
}

/// Lo que dice un documento, si dice algo que se pueda construir.
pub fn interpret(text: &str) -> Result<Validated, LayoutProblem> {
    // **El techo se aplica ANTES de interpretar, no después.** Interpretar reserva el árbol entero, así
    // que comprobar el tamaño al final es no tener techo: el gasto ya ocurrió.
    if text.len() > MAX_BYTES {
        return Err(LayoutProblem::TooLarge {
            bytes: text.len() as u64,
        });
    }

    // Dos pasadas del intérprete a propósito, y la primera es sobre un molde de un campo: cuesta lo que
    // cuesta recorrer el texto una vez —una vez al arrancar— y es lo que hace que la versión mande sobre
    // la forma en vez de al revés. Que `v` falte o no sea un número entra por aquí como «no se entiende»,
    // que es lo que corresponde: un fichero sin versión no es de una versión que no conozcamos, es un
    // fichero nuestro estropeado, y se sobrescribe.
    let sonda: StoredProbe = serde_json::from_str(text)
        .map_err(|error| LayoutProblem::Unintelligible(error.to_string()))?;
    if sonda.v != VERSION {
        return Err(LayoutProblem::Version { found: sonda.v });
    }

    let document: StoredDocument = serde_json::from_str(text)
        .map_err(|error| LayoutProblem::Unintelligible(error.to_string()))?;
    validate(document)
}

/// Lo que hay que escribir para volver a leer esto, o **nada** si no se pudo.
///
/// **Devuelve `Option` y no una cadena, y eso lo decidió una revisión.** La primera versión devolvía
/// cadena vacía al fallar, y el camino que escribe el fichero **no habría podido distinguirla de un
/// documento**: escribe un temporal y lo renombra encima del bueno, así que un fallo aquí habría
/// **destruido la disposición guardada** y dejado cero bytes en su sitio. Abrir de fábrica es la
/// respuesta a un fichero que no se entiende; **borrar el que sí se entendía no es respuesta a nada.**
/// Con `None`, quien escribe se salta el renombrado y el fichero anterior sobrevive.
pub fn serialize(document: &StoredDocument) -> Option<String> {
    // Sin sangrar: nadie lo edita a mano —lo escribe siempre la aplicación— y una línea por consola
    // hace el fichero varias veces más grande sin que nadie lo agradezca.
    match serde_json::to_string(document) {
        Ok(text) => Some(text),
        Err(error) => {
            // Inalcanzable con los tipos de aquí —no hay claves que no sean texto ni números que no
            // puedan serlo—, así que se dice y no se calla: un fallo silencioso en el camino que guarda
            // es el que se descubre al reabrir y no encontrar nada.
            eprintln!("no se pudo escribir la disposición ({error})");
            None
        }
    }
}

/// El único camino que produce un [`Validated`].
///
/// **Se recorre el árbol entero antes de aceptar nada**, y en particular se cuentan las hojas **antes**
/// de que se abra la primera: comprobar el techo después del `fork` es no tener techo.
pub fn validate(document: StoredDocument) -> Result<Validated, LayoutProblem> {
    if document.v != VERSION {
        return Err(LayoutProblem::Version { found: document.v });
    }
    if document.tabs.is_empty() {
        return Err(LayoutProblem::NoTabs);
    }

    // **Un solo recorrido, que valida y cuenta.** Dos —uno para revisar y otro para contar— visitan cada
    // nodo dos veces en el camino del arranque, y además abren la puerta a que el número que se compara
    // con el techo no sea el que luego se lee.
    let mut leaves = 0;
    for (which, tab) in document.tabs.iter().enumerate() {
        leaves += check(&tab.root, None, which)?;
    }

    if leaves > MAX_LEAVES {
        return Err(LayoutProblem::TooManyLeaves { count: leaves });
    }

    Ok(Validated { document, leaves })
}

/// Las invariantes de forma, rama por rama.
///
/// `from_outside` es el sentido de la rama que contiene a ésta, o nada en la raíz: es lo único que hace
/// falta para cazar el anidamiento, porque partir solo puede crearlo entre una rama y su madre directa.
fn check(
    node: &StoredNode,
    from_outside: Option<StoredOrientation>,
    tab: usize,
) -> Result<usize, LayoutProblem> {
    let StoredNode::Branch {
        orientation,
        children,
    } = node
    else {
        return Ok(1);
    };

    if children.len() < 2 {
        return Err(LayoutProblem::SingleChildBranch { tab });
    }
    if from_outside == Some(*orientation) {
        return Err(LayoutProblem::NestedBranches { tab });
    }

    let mut leaves = 0;
    for child in children {
        // **Una sola puerta por peso, la del tipo vivo, y ninguna propia.** Es la única forma de que no
        // haya dos implementaciones de la misma invariante —la de allí ya está probada— y, sobre todo, de
        // que **lo que el árbol puede tener se pueda volver a leer**: hubo un rango propio, `[1e-6, 1e6]`,
        // y los dos extremos rechazaban estados a los que se llega **arrastrando**. Por abajo, porque el
        // arrastre recorta la *fracción* y no el peso; por arriba, porque lo que conserva es la **suma del
        // par**, así que un peso sube hasta el 99 % de lo que sumaban dos vecinos. Lo que el reparto
        // necesita de verdad no es un peso acotado: es **una suma de rama finita**, y eso se comprueba
        // abajo.
        if crate::Weight::new(child.weight).is_none() {
            return Err(LayoutProblem::InvalidWeight { tab });
        }
        leaves += check(&child.node, Some(*orientation), tab)?;
    }

    // **Y la suma, que es una invariante de la rama y no de cada peso.** Con el rango de arriba esto ya no
    // puede desbordar con dos hijos, pero se deja: el rango puede ampliarse mañana, y cuesta una suma.
    let sum: f32 = children.iter().map(|child| child.weight).sum();
    if !sum.is_finite() {
        return Err(LayoutProblem::NonFiniteSum { tab });
    }

    Ok(leaves)
}

// ── El fichero: dónde vive, cómo se lee y cómo se sustituye ─────────────────────────────────────

/// El directorio propio dentro del de configuración del usuario.
pub const DIRECTORY: &str = "agent-terminal";

/// El nombre del fichero, **fijo**.
///
/// Que no dependa de nada es lo que hace que la disposición sea una, global, como decidió el ticket. Y
/// es también de dónde sale el límite aceptado: dos instancias abiertas escriben el mismo destino y **la
/// última que guarde se lo lleva**.
pub const FILE: &str = "layout.json";

/// Dónde vive la disposición, leyendo el entorno.
///
/// **La envoltura de una línea, y el trabajo en [`path_from`]**, que es el patrón que el arranque ya sigue
/// cinco veces: `std::env::set_var` tiene contrato sobre todo el proceso y esta suite corre en paralelo
/// lanzando procesos, así que una prueba que quisiera fijar estas variables sería comportamiento
/// indefinido.
pub fn path() -> Option<PathBuf> {
    path_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// La decisión, con los dos valores dados: **el fichero, o nada si no hay sitio de fiar**.
///
/// **Nada significa que no se persiste: ni se lee ni se escribe, y no se cae a un directorio temporal.**
/// La caída existe en la caja hermana (`crates/panel-contract/src/lib.rs:186`) y allí es correcta porque
/// aquel directorio **debe** vaciarse al cerrar la sesión; aquí sería restaurar desde un fichero que el
/// sistema borra, o sea una máquina de fallos silenciosos.
///
/// **Y una variable definida y vacía no es una ruta.** Es la trampa ya pagada de la caja hermana: dejarla
/// pasar da una ruta relativa y el directorio se crea dentro del directorio de trabajo **sin fallar**, que
/// con un nombre de fichero fijo acaba dentro de cualquier repositorio que alguien clone. Aquí lo dice la
/// misma comprobación que rechaza lo relativo, porque la cadena vacía tampoco es absoluta.
pub fn path_from(config: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    Some(directory_from(config, home)?.join(FILE))
}

/// El directorio propio dentro del de configuración, con los dos valores dados.
///
/// **Sale aparte porque este proyecto guarda ahí más de un fichero.** La disposición la escribe la
/// aplicación; los ajustes los escribe quien usa el terminal. Comparten sitio y la decisión de qué sitio
/// es, que es justo lo que esta función encierra; no comparten nada más.
pub fn directory_from(config: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    // El orden es el del estándar del escritorio: la variable propia manda, y donde no hay se usa
    // `.config` de la casa. Que una de las dos no sirva **no descarta la otra**: lo que descarta es que
    // no sirva ninguna.
    let base = absolute(config).or_else(|| absolute(home).map(|home| home.join(".config")))?;
    Some(base.join(DIRECTORY))
}

/// La ruta que trae una variable, si es de fiar.
fn absolute(variable: Option<&OsStr>) -> Option<PathBuf> {
    let path = PathBuf::from(variable?);
    path.is_absolute().then_some(path)
}

/// Lee la disposición guardada en esa ruta.
///
/// **El orden es el del molde y no es casual** (`crates/panel-contract/src/panel.rs:199`): pedir los
/// datos del fichero, comprobar que es un fichero, abrir, leer acotado, aplicar el techo **sobre bytes**
/// y solo entonces mirar si es texto.
pub fn read(path: &Path) -> Result<Validated, LayoutProblem> {
    let data = match std::fs::metadata(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(LayoutProblem::Absent);
        }
        Err(error) => return Err(LayoutProblem::Unreadable(error.to_string())),
    };
    if !data.is_file() {
        return Err(LayoutProblem::NotAFile);
    }

    let file =
        std::fs::File::open(path).map_err(|error| LayoutProblem::Unreadable(error.to_string()))?;
    let bytes = read_capped(file, data.len())
        .map_err(|error| LayoutProblem::Unreadable(error.to_string()))?;

    // **El techo se aplica aquí y no solo dentro de [`interpret`], y esa aparente duplicación es el
    // arreglo de un fallo con nombre en el molde.** La lectura corta un byte por encima del techo, y ese
    // corte **parte un carácter de varios bytes**: sin esta comprobación, un fichero enorme con un acento
    // en el sitio justo saldría como «no es texto» en vez de «se pasa del techo», y el camino del techo
    // casi nunca se alcanzaría. El número es el mismo del formato; lo que se decide dos veces es *cuándo*.
    //
    // **Lo que DECIDE es lo leído, y el número que se dice es el mayor de los dos.** Decidir con el dato
    // del disco sería fiarse de una cifra que ya puede ser vieja —el fichero se sustituye por renombrado, y
    // entre pedir los datos y leer puede haber cambiado— y además trunca en objetivos de 32 bits; lo leído
    // está acotado por definición y no puede ni mentir ni desbordar. Para el mensaje, en cambio, lo leído
    // se queda corto por construcción: diría «ocupa 65537» de un fichero de un mega.
    if bytes.len() > MAX_BYTES {
        return Err(LayoutProblem::TooLarge {
            bytes: data.len().max(bytes.len() as u64),
        });
    }

    let text = String::from_utf8(bytes).map_err(|_| LayoutProblem::NotText)?;
    interpret(&text)
}

/// Lee como mucho un byte más que el techo.
///
/// El byte de más es lo que distingue «ocupa justo el techo» de «se pasa». Y `expected` es **pista de
/// reserva y no comprobación**: nadie decide nada con ese número, solo evita que la lectura crezca
/// duplicando desde treinta y dos bytes. El techo lo sigue poniendo el corte.
pub(crate) fn read_capped(file: std::fs::File, expected: u64) -> std::io::Result<Vec<u8>> {
    let ceiling = MAX_BYTES as u64;
    let mut bytes = Vec::with_capacity(expected.min(ceiling) as usize + 1);
    file.take(ceiling + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Si un problema al cargar merece una línea.
///
/// **La única excepción es el caso normal**: que no haya fichero es el primer arranque de todos, y avisar
/// de eso pondría una línea de error en la primera vez de cada usuario. Está separado de quien la imprime
/// **porque la decisión es lo único que se puede afirmar**: escribir a la salida de error no se observa
/// desde dentro del proceso, así que una prueba sobre el aviso solo puede mirar esto.
fn warrants_warning(problem: &LayoutProblem) -> bool {
    !matches!(problem, LayoutProblem::Absent)
}

/// Por qué no se guardó la disposición.
///
/// **Cada variante es un motivo distinto, y eso tiene una consecuencia visible**: el aviso se suprime
/// **por motivo** ([`Persistence::save`]), así que dos fallos que el usuario no puede distinguir no deben
/// ser dos variantes, y dos que pide arreglar de forma distinta —un directorio que no se puede crear y un
/// renombrado que no se puede hacer— no deben ser una.
#[derive(Debug)]
pub(crate) enum SaveFailure {
    /// No hay sitio de fiar donde ponerlo: ver [`path_from`]. **Lo usa también el camino que carga**, con su
    /// propio verbo, porque la causa es la misma y el usuario no tiene dos cosas que arreglar.
    NoPlace,
    /// En disco hay una disposición de una versión **más nueva**, y no se pisa.
    Vetoed { found: u64 },
    /// El documento no se pudo describir. **Inalcanzable con estos tipos**, y aun así una variante y no
    /// un `unwrap`: lo que hay en disco es lo que el usuario colocó a mano.
    NotSerializable,
    /// No se pudo crear el directorio.
    Directory(std::io::Error),
    /// No se pudo crear el temporal **en exclusiva**. Que ya exista entra por aquí, y es lo que impide
    /// escribir a través de lo que otro haya plantado con ese nombre.
    TempFile(std::io::Error),
    /// No se pudo escribir el temporal.
    Writing(std::io::Error),
    /// No se pudo renombrar el temporal encima del destino. **El destino se queda como estaba.**
    Renaming(std::io::Error),
    /// Esta sesión **no debe pisar** lo que hay en disco, y trae por qué.
    ///
    /// **Existe porque no todas las razones para no escribir son del fichero.** Dos, hoy: que la aplicación
    /// **no pudiera abrir** la disposición guardada —entonces lo que hay en pantalla es una ventana de
    /// fábrica, y escribirla se llevaría por delante la del usuario **para siempre**— y que la sesión la
    /// haya pedido una tanda de medida, que abre lo que la tanda diga y no lo que el usuario dejó.
    WontOverwrite(&'static str),
    /// Lo que hay en pantalla **no cabe en el formato**: hoy, más consolas que las que se aceptan abrir
    /// de golpe.
    ///
    /// **Variante propia y no compartida con otro motivo**, que es lo que el contrato de este enum exige:
    /// el aviso se suprime **por motivo**, así que dos cosas que se arreglan de forma distinta —cerrar
    /// consolas, y que el disco vuelva— no pueden gastar la misma ranura.
    DoesntFit,
}

impl std::fmt::Display for SaveFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPlace => write!(
                f,
                "no hay ningún sitio donde ponerla: ni $XDG_CONFIG_HOME ni $HOME dan una ruta absoluta"
            ),
            Self::Vetoed { found } => write!(
                f,
                "en disco hay una disposición de la versión {found} del formato y esta conoce la \
                 {VERSION}, así que no se sobrescribe"
            ),
            Self::NotSerializable => write!(f, "no se pudo describir la disposición"),
            Self::Directory(error) => write!(f, "no se pudo crear el directorio ({error})"),
            Self::TempFile(error) => write!(f, "no se pudo crear el fichero temporal ({error})"),
            Self::Writing(error) => {
                write!(f, "no se pudo escribir el fichero temporal ({error})")
            }
            Self::Renaming(error) => {
                write!(f, "no se pudo poner el fichero en su sitio ({error})")
            }
            Self::WontOverwrite(reason) => write!(f, "no se pisa lo que hay guardado: {reason}"),
            Self::DoesntFit => write!(f, "la disposición de ahora no cabe en el formato"),
        }
    }
}

/// La política de guardado **de una sesión**: dónde va, si se puede escribir, y de qué ya se avisó.
///
/// **No es un fichero y por eso no se llama así.** No tiene descriptor, no abre nada y sobrevive a todas
/// las lecturas y escrituras; lo que guarda es lo que dura **hasta que la aplicación se cierre**: el veto
/// de una versión más nueva, los motivos de los que ya se ha avisado, y la cuenta de intentos. Con esos
/// datos en los parámetros de una función libre, quien llama tendría que llevarlos —y eso es exactamente
/// la clase de invariante que este módulo se niega a dejar en la memoria de quien escribe.
///
/// **Y de ahí la regla para quien la use: la que carga al arrancar y la que guarda después tienen que ser
/// LA MISMA.** Crear una segunda a mitad de sesión **pierde el veto**, y perder el veto significa
/// sobrescribir la disposición de una versión más nueva, que es el daño exacto que la versión del formato
/// existe para evitar.
pub struct Persistence {
    path: Option<PathBuf>,
    /// La versión hallada en disco que impide sobrescribirlo.
    ///
    /// **Se pone al leer y no se quita.** Un binario viejo abierto sobre la disposición de uno nuevo no
    /// puede restaurarla —no sabe su forma— y tampoco debe machacarla: tener dos versiones instaladas en
    /// la misma máquina no es raro, y el daño sería silencioso y en el sitio que más se nota.
    veto: Option<u64>,
    /// Por qué esta sesión no debe escribir, si hay razón. Ver [`SaveFailure::WontOverwrite`].
    ///
    /// **Aparte del veto de versión a propósito**: aquel trae un número y viene de lo que había en disco;
    /// éste viene de lo que le pasó a esta sesión. Juntarlos en un campo obligaría a inventar un número
    /// para el segundo caso.
    no_overwrite: Option<&'static str>,
    /// De qué motivos ya se avisó. Un disco lleno imprimiría una línea por cada plazo vencido, o sea
    /// dos por segundo mientras la ventana se use.
    warned: HashSet<std::mem::Discriminant<SaveFailure>>,
    /// Cuántas veces se ha intentado guardar. **Va en el nombre del temporal, y eso lo pidió una
    /// revisión.**
    ///
    /// Con el nombre dependiendo solo del número de proceso, un temporal que sobreviviera a una muerte
    /// violenta —entre crearlo y renombrarlo— dejaría **sin guardar a toda sesión futura** a la que el
    /// sistema le diera ese mismo número, porque crear en exclusiva fallaría siempre y adoptar lo que
    /// hay está prohibido con razón. Contando los intentos, un temporal plantado estorba **un intento**
    /// y no una sesión.
    attempts: usize,
    /// Lo que tardó [`Self::load`] la última vez que se la llamó, si se la llamó.
    ///
    /// **Vive aquí y no lo devuelve `load`, y es lo que evita tocar seis firmas para publicar un
    /// número.** Quien lo publica es el informe de medida, que ya recibe esta misma `Persistence` por
    /// otro motivo —el veto de versión se queda con la ventana—, así que el dato viaja con quien ya
    /// viajaba. Devolverlo en el `Option` habría obligado a cambiar el tipo con el que abre la ventana y
    /// las seis pruebas que lo construyen a mano.
    load_time: Option<Duration>,
}

impl Persistence {
    /// El fichero que dice el entorno.
    pub fn from_env() -> Self {
        Self::at(path())
    }

    /// El de una ruta dada, o el de ninguna. **Nada aquí toca el disco todavía.**
    pub fn at(path: Option<PathBuf>) -> Self {
        Self {
            path,
            veto: None,
            no_overwrite: None,
            warned: HashSet::new(),
            attempts: 0,
            load_time: None,
        }
    }

    /// Deja dicho que esta sesión **no puede pisar** lo que hay guardado, y por qué.
    ///
    /// **Es lo que impide la pérdida de datos que una revisión destapó.** Si la aplicación leyó bien el
    /// fichero y **no pudo abrirlo** —un descriptor agotado, un pseudoterminal que no se concede—, lo que
    /// queda en pantalla es una ventana de fábrica; sin esto, el primer cambio del usuario la escribiría
    /// encima y su disposición de veinte consolas desaparecería **para siempre**, cuando antes de guardar
    /// automáticamente el fichero sobrevivía y el arranque siguiente la recuperaba.
    pub fn forbid_overwrite(&mut self, reason: &'static str) {
        self.no_overwrite = Some(reason);
    }

    /// La disposición guardada, si hay alguna que se entienda.
    ///
    /// **Todo lo que falla devuelve nada y deja arrancar**, que es la tolerancia por documento de este
    /// formato: adivinar media disposición es peor que no restaurarla. Se avisa de todo salvo de que no
    /// haya fichero.
    pub fn load(&mut self) -> Option<Validated> {
        let start = std::time::Instant::now();
        let loaded = self.loading();
        self.load_time = Some(start.elapsed());
        loaded
    }

    /// Lo que tardó la última carga, para que el informe de medida lo declare. **Nada si nadie la
    /// llamó.**
    ///
    /// **Y esa distinción es la que impide publicar un número inventado**, que era el hueco que dejó la
    /// primera versión: devolviendo un `Duration` a secas, una ventana construida con una persistencia
    /// que nunca tocó el disco —lo que hacen las pruebas, y lo que podría hacer un camino nuevo—
    /// publicaba `0ns en leer y validar`, indistinguible de una lectura instantánea de verdad. Con
    /// `None` el informe dice «sin medir», que es lo que pasó.
    pub fn load_time(&self) -> Option<Duration> {
        self.load_time
    }

    fn loading(&mut self) -> Option<Validated> {
        let Some(path) = self.path.clone() else {
            // **Con su propio verbo, y sin tocar la lista de motivos ya avisados, y las dos cosas las
            // pidió una revisión.** Decir «no se guardó» antes de haber intentado guardar nada es un
            // mensaje que no corresponde a su camino; y gastar aquí el motivo dejaría **mudo** al primer
            // fallo de guardado de verdad, que es el que el usuario necesita ver.
            eprintln!("no se restauró la disposición: {}", SaveFailure::NoPlace);
            return None;
        };

        match read(&path) {
            Ok(validated) => Some(validated),
            Err(problem) if !warrants_warning(&problem) => None,
            Err(problem) => {
                // **Una versión que no es la nuestra no es un fichero roto, y la regla depende de por qué
                // lado no lo es.** Más nueva: la escribió una versión que sabe algo que ésta no, así que
                // se veta la escritura para toda la sesión. Más vieja **y conocida**: se leería con el
                // lector de esa versión y se reescribiría en la nueva al primer guardado —hoy solo existe
                // la 1, así que no hay caso, pero retirar el soporte de una versión vieja es una decisión
                // aparte y se escribe—. Y lo que no es ninguna de las dos, como un cero, es un fichero
                // nuestro estropeado: de fábrica, y se sobrescribe.
                if let LayoutProblem::Version { found } = problem
                    && found > VERSION
                {
                    self.veto = Some(found);
                }
                eprintln!("no se restauró la disposición: {problem}");
                None
            }
        }
    }

    /// Deja en disco esa disposición, y dice si lo consiguió.
    ///
    /// **Recibe lo ya validado y no un documento cualquiera, y eso lo pidió una revisión.** Sin eso, el
    /// camino que escribe aceptaba exactamente lo que el camino que lee rechazaría: se podría dejar en
    /// disco un fichero que al reabrir se descarta, y el usuario perdería su disposición **sin que nada lo
    /// dijera** —el fallo aparecería un arranque más tarde y sin causa visible—. Pasando por el mismo tipo
    /// que la lectura exige, lo que se escribe se puede volver a leer **por construcción**.
    ///
    /// **Cada fallo se avisa una vez por motivo, y el reintento no se suprime**: quien llama vuelve a
    /// intentarlo con el cambio siguiente, así que un disco que se vacía se recupera solo sin haber
    /// llenado la salida de error mientras estaba lleno.
    pub fn save(&mut self, layout: &Validated) -> bool {
        self.attempts = self.attempts.wrapping_add(1);
        let attempt = if let Some(found) = self.veto {
            Err(SaveFailure::Vetoed { found })
        } else if let Some(reason) = self.no_overwrite {
            Err(SaveFailure::WontOverwrite(reason))
        } else if let Some(path) = &self.path {
            save_to(path, self.attempts, &layout.document)
        } else {
            Err(SaveFailure::NoPlace)
        };

        match attempt {
            Ok(()) => true,
            Err(error) => {
                self.warn(error);
                false
            }
        }
    }

    /// Avisa de que lo que hay en pantalla **no se puede guardar**, una vez por sesión.
    ///
    /// **Existe porque el formato tiene un techo que la ventana no tiene.** No se aceptan más de
    /// [`MAX_LEAVES`] consolas en un documento —el techo acota el `fork` del arranque, así que no puede
    /// quitarse—, y en cambio nadie impide abrir más pestañas de las que caben ahí. Sin este aviso, quien
    /// llegue a ese número perdería su disposición **sin ninguna señal**: se dejaría de guardar y lo notaría
    /// al reabrir, un día después, sin causa visible.
    pub fn warn_cannot_save(&mut self, problem: &LayoutProblem) {
        if self.should_warn(&SaveFailure::DoesntFit) {
            eprintln!("la disposición de ahora no se puede guardar: {problem}");
        }
    }

    /// Dice la línea, si de ese motivo no se ha dicho ya.
    fn warn(&mut self, error: SaveFailure) {
        if self.should_warn(&error) {
            eprintln!("no se guardó la disposición: {error}");
        }
    }

    /// La decisión, aparte de decir la línea: **es lo único que tiene contenido que afirmar**, porque
    /// escribir a la salida de error no se puede observar desde dentro del proceso.
    fn should_warn(&mut self, error: &SaveFailure) -> bool {
        self.warned.insert(std::mem::discriminant(error))
    }
}

/// Escribe el documento en esa ruta **sustituyéndola de golpe**.
fn save_to(path: &Path, attempt: usize, document: &StoredDocument) -> Result<(), SaveFailure> {
    save_described(path, attempt, serialize(document))
}

/// La escritura, recibiendo ya el texto **o la falta de él**.
///
/// **Que reciba `Option` es lo que hace comprobable la promesa central de este camino**: sin texto no se
/// renombra nada, así que lo que ya estaba guardado sobrevive. Con la llamada dentro, la única forma de
/// llegar a ese caso sería un fallo que estos tipos no pueden producir, y la promesa se quedaría escrita
/// en un comentario.
///
/// **Sin `fsync`, y es una decisión con su límite.** Con el sistema en marcha, «nunca se ve medio fichero»
/// lo da el renombrado solo. Tras un corte de corriente **sí** puede quedar un destino truncado, según el
/// sistema de ficheros: eso sale al reabrir como un fichero que no se entiende, o sea abrir de fábrica, que
/// es lo que ya está aceptado para el último plazo perdido. Lo que el renombrado garantiza es que **nadie
/// lo ve a medias mientras la máquina funciona**; no es un diario.
fn save_described(
    path: &Path,
    attempt: usize,
    described: Option<String>,
) -> Result<(), SaveFailure> {
    let text = described.ok_or(SaveFailure::NotSerializable)?;
    // Sin directorio, o sin nombre de fichero, no hay sitio donde poner nada. Solo se llega con una ruta
    // que no salió de `path_from` —la raíz, por ejemplo—, y el motivo es el mismo que el de no tener
    // directorio de configuración: no hay dónde. De ahí que la variante sea una y su línea las cubra.
    let directory = path.parent().ok_or(SaveFailure::NoPlace)?;
    create_directory(directory).map_err(SaveFailure::Directory)?;

    let temp_path = temp_path_for(path, attempt).ok_or(SaveFailure::NoPlace)?;
    let mut file = create_exclusive(&temp_path).map_err(SaveFailure::TempFile)?;

    // A partir de aquí hay un fichero de más en el directorio, y **cada salida tiene que llevárselo**.
    // Dejarlo no rompe el guardado siguiente —el nombre lleva la cuenta de intentos—, pero se acumularía
    // uno por fallo en el directorio de configuración del usuario, que es basura sin techo.
    if let Err(error) = file.write_all(text.as_bytes()) {
        remove(&temp_path);
        return Err(SaveFailure::Writing(error));
    }
    // Cerrado antes de renombrar. No hay nada que vaciar —esto escribe directo, sin memoria intermedia—,
    // así que cerrar es solo devolver el descriptor.
    drop(file);

    if let Err(error) = std::fs::rename(&temp_path, path) {
        remove(&temp_path);
        return Err(SaveFailure::Renaming(error));
    }
    Ok(())
}

/// El temporal al lado del destino, con el número de este proceso y el del intento.
///
/// **En el mismo directorio y no en el temporal del sistema**, porque ése sería otro sistema de ficheros
/// con toda probabilidad, el renombrado fallaría por cruzarlos, y el respaldo —copiar y truncar— ya no
/// sería atómico. La tentación es real y está en el repo: la caja hermana cae ahí a propósito.
///
/// **El número de proceso porque nada impide dos instancias abiertas**: con un nombre fijo las dos
/// escribirían el mismo temporal, se entrelazarían los bytes y se renombraría un documento corrupto encima
/// del bueno. **Y el del intento porque el del proceso se recicla**: un temporal que sobreviva a una muerte
/// violenta no lo limpia nadie, y con el nombre dependiendo solo del proceso dejaría sin guardar, para
/// siempre, a toda sesión a la que el sistema le diera ese número.
fn temp_path_for(path: &Path, attempt: usize) -> Option<PathBuf> {
    let mut name = path.file_name()?.to_os_string();
    name.push(format!(".{}.{attempt}", std::process::id()));
    Some(path.with_file_name(name))
}

/// Crea el directorio si no está, **solo para el usuario**.
///
/// Sin el modo, el permiso lo decide la máscara de la sesión: medida en esta máquina en `0002`, un
/// directorio saldría `0775` —legible y recorrible por todo el mundo— y este fichero cuenta cómo trabaja el
/// usuario: qué tenía abierto y cómo lo tenía colocado.
///
/// **Con tramos intermedios y sin protestar si ya existe**, y las dos cosas hacen falta: el directorio de
/// configuración puede no existir en una cuenta recién hecha, y a partir del segundo guardado el nuestro
/// ya está. El modo solo se aplica a lo que **esta** llamada cree; un directorio que ya estaba conserva
/// sus permisos, y por eso lo único que protege al temporal es crearlo en exclusiva.
fn create_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// Crea el temporal **y falla si ya existe**.
///
/// El nombre es predecible, así que sin esto un enlace plantado ahí se abriría y **se escribiría a
/// través**: el destino está protegido por no abrirse nunca para escribir, y el temporal solo por esto.
fn create_exclusive(path: &Path) -> std::io::Result<std::fs::File> {
    let mut builder = std::fs::OpenOptions::new();
    builder.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        builder.mode(0o600);
    }
    builder.open(path)
}

/// Se lleva el temporal, y se calla si no puede.
///
/// Lo que se está reportando es el fallo de antes; un segundo aviso por no haber podido limpiar diría dos
/// cosas de las que solo una se puede arreglar.
fn remove(temp_path: &Path) {
    let _ = std::fs::remove_file(temp_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(width: usize) -> StoredNode {
        StoredNode::Leaf(StoredLeaf {
            panel_width: width,
            folded: false,
        })
    }

    /// Una rama con esos pesos y una hoja por cada uno.
    fn branch(orientation: StoredOrientation, weights: &[f32]) -> StoredNode {
        StoredNode::Branch {
            orientation,
            children: weights
                .iter()
                .map(|weight| StoredChild {
                    weight: *weight,
                    node: leaf(36),
                })
                .collect(),
        }
    }

    fn document(raices: Vec<StoredNode>) -> StoredDocument {
        StoredDocument {
            v: VERSION,
            tabs: raices.into_iter().map(|root| StoredTab { root }).collect(),
        }
    }

    #[test]
    fn hoja_y_rama_se_distinguen_por_una_etiqueta_explicita() {
        // **Esto es el contrato, así que se afirma sobre el texto literal y no sobre el tipo.** Si un día
        // alguien cambia la representación del enum, la ida y vuelta seguiría funcionando y **el fichero
        // de todo el que ya use la aplicación dejaría de leerse**: es justo el fallo que la versión del
        // formato existe para no tener que arreglar por sorpresa.
        let written = serialize(&document(vec![branch(
            StoredOrientation::Columns,
            &[3.0, 1.0],
        )]))
        .expect("un documento de dos hojas se escribe");

        assert_eq!(
            written,
            r#"{"v":2,"tabs":[{"root":{"branch":{"orientation":"columns","children":[{"weight":3.0,"node":{"leaf":{"panel_width":36,"folded":false}}},{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}}]}}}]}"#
        );
    }

    #[test]
    fn lo_escrito_se_vuelve_a_leer_igual() {
        let start = document(vec![
            StoredNode::Branch {
                orientation: StoredOrientation::Columns,
                children: vec![
                    StoredChild {
                        weight: 3.0,
                        node: leaf(52),
                    },
                    StoredChild {
                        weight: 1.0,
                        node: StoredNode::Branch {
                            orientation: StoredOrientation::Rows,
                            children: vec![
                                StoredChild {
                                    weight: 2.0,
                                    node: StoredNode::Leaf(StoredLeaf {
                                        panel_width: 36,
                                        folded: true,
                                    }),
                                },
                                StoredChild {
                                    weight: 1.0,
                                    node: leaf(24),
                                },
                            ],
                        },
                    },
                ],
            },
            leaf(36),
        ]);

        let written = serialize(&start).expect("un documento válido se escribe");
        let after = interpret(&written).expect("lo que se acaba de escribir se lee");

        assert_eq!(after.tabs(), start.tabs.as_slice());
    }

    #[test]
    fn los_pesos_no_tienen_que_sumar_nada_concreto() {
        // Los pesos son **proporciones**, y el reparto normaliza por la suma de su rama: `{3, 1}` y
        // `{0,5, 1,5}` describen exactamente el mismo reparto. Exigir que sumaran uno o cien sería una
        // guarda sin escenario **y rompería la ida y vuelta** de cualquier árbol construido arrastrando,
        // que es de donde salen los pesos de verdad.
        let middle = document(vec![branch(StoredOrientation::Rows, &[0.5, 1.5])]);
        let whole = document(vec![branch(StoredOrientation::Rows, &[3.0, 9.0])]);

        assert!(validate(middle).is_ok());
        assert!(validate(whole).is_ok());
    }

    #[test]
    fn una_rama_con_un_solo_hijo_se_rechaza() {
        // **La invariante que sostiene la razón de ser del árbol.** Si una rama puede tener una sola hija,
        // vuelve el anidamiento que hace que una frontera arrastre un bloque de dos consolas — lo que más
        // se le critica a `tmux` y lo que D14 existe para evitar.
        let one = document(vec![branch(StoredOrientation::Columns, &[1.0])]);

        assert_eq!(
            validate(one),
            Err(LayoutProblem::SingleChildBranch { tab: 0 })
        );
    }

    #[test]
    fn dos_ramas_del_mismo_sentido_anidadas_se_rechazan() {
        let nested = document(vec![StoredNode::Branch {
            orientation: StoredOrientation::Columns,
            children: vec![
                StoredChild {
                    weight: 1.0,
                    node: leaf(36),
                },
                StoredChild {
                    weight: 1.0,
                    node: branch(StoredOrientation::Columns, &[1.0, 1.0]),
                },
            ],
        }]);

        assert_eq!(
            validate(nested),
            Err(LayoutProblem::NestedBranches { tab: 0 })
        );
    }

    #[test]
    fn dos_ramas_de_sentidos_distintos_anidadas_se_aceptan() {
        // **El hermano de la prueba anterior, y hace falta.** Sin él, «rechaza el anidamiento» y «rechaza
        // cualquier rama dentro de otra» pasan las mismas pruebas, y la segunda haría imposible la
        // disposición en cuadro que F2.2 entrega.
        let grid = document(vec![StoredNode::Branch {
            orientation: StoredOrientation::Columns,
            children: vec![
                StoredChild {
                    weight: 1.0,
                    node: branch(StoredOrientation::Rows, &[1.0, 1.0]),
                },
                StoredChild {
                    weight: 1.0,
                    node: branch(StoredOrientation::Rows, &[1.0, 1.0]),
                },
            ],
        }]);

        assert!(validate(grid).is_ok());
    }

    #[test]
    fn un_peso_que_el_tipo_vivo_rechaza_se_rechaza_aqui() {
        // Cero y negativo llegan por JSON; `NaN` e infinito no tienen literal, así que la única forma de
        // que entren es un documento armado a mano — y aun así el constructor los para.
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let document = document(vec![branch(StoredOrientation::Columns, &[1.0, bad])]);

            assert!(
                matches!(validate(document), Err(LayoutProblem::InvalidWeight { .. })),
                "un peso de {bad} tenía que caer"
            );
        }
    }

    #[test]
    fn el_techo_acepta_las_hojas_justas_y_rechaza_una_mas() {
        // **Las dos mitades, y la de abajo es la que importa.** Un techo probado solo por arriba no
        // distingue «rechaza más de 128» de «rechaza más de 1»: con la prueba de rechazo sola, poner el
        // techo en cualquier número más pequeño seguiría dando verde.
        let weights: Vec<f32> = vec![1.0; MAX_LEAVES];
        let justas = document(vec![branch(StoredOrientation::Columns, &weights)]);
        assert!(
            validate(justas).is_ok(),
            "{MAX_LEAVES} hojas tienen que caber"
        );

        let weights: Vec<f32> = vec![1.0; MAX_LEAVES + 1];
        let one_more = document(vec![branch(StoredOrientation::Columns, &weights)]);
        assert_eq!(
            validate(one_more),
            Err(LayoutProblem::TooManyLeaves {
                count: MAX_LEAVES + 1
            })
        );
    }

    #[test]
    fn el_techo_cuenta_las_hojas_de_todo_el_documento_y_no_de_una_pestaña() {
        // Si contara por pestaña, dos pestañas de 128 abrirían 256 consolas y el techo no habría servido
        // de nada: lo que se acota es cuántos procesos se bifurcan **al abrir**, y eso es de la ventana.
        let half: Vec<f32> = vec![1.0; MAX_LEAVES / 2 + 1];
        let dos = document(vec![
            branch(StoredOrientation::Columns, &half),
            branch(StoredOrientation::Columns, &half),
        ]);

        assert!(matches!(
            validate(dos),
            Err(LayoutProblem::TooManyLeaves { .. })
        ));
    }

    #[test]
    fn una_version_que_no_es_la_conocida_se_rechaza_diciendo_cual_es() {
        let of_the_future = StoredDocument {
            v: VERSION + 1,
            tabs: vec![StoredTab { root: leaf(36) }],
        };

        assert_eq!(
            validate(of_the_future),
            Err(LayoutProblem::Version { found: VERSION + 1 })
        );
    }

    #[test]
    fn un_documento_sin_pestañas_se_rechaza() {
        let empty = StoredDocument {
            v: VERSION,
            tabs: Vec::new(),
        };

        assert_eq!(validate(empty), Err(LayoutProblem::NoTabs));
    }

    #[test]
    fn lo_que_no_es_json_se_rechaza_y_se_dice_por_que() {
        // **Se afirma que el motivo llega, no solo que falla.** El único canal para contarlo es una línea,
        // así que un `LayoutProblem` sin texto dejaría al usuario con «no se pudo» y nada más.
        let Err(LayoutProblem::Unintelligible(reason)) = interpret("{esto no es json") else {
            panic!("un documento roto tiene que caer por no entenderse");
        };

        assert!(!reason.is_empty(), "el motivo tiene que decir algo");
    }

    #[test]
    fn un_nodo_sin_etiqueta_no_se_lee() {
        // La forma que se descartó: hoja o rama deducidas de qué clave está presente. Si esto se leyera,
        // el fichero tendría dos formas válidas y una de ellas no sería la del contrato.
        let unlabeled = r#"{"v":2,"tabs":[{"root":{"panel_width":36,"folded":false}}]}"#;

        assert!(matches!(
            interpret(unlabeled),
            Err(LayoutProblem::Unintelligible(_))
        ));
    }
}

#[cfg(test)]
mod ceiling_tests {
    use super::*;

    fn with_weights(weights: &[f32]) -> StoredDocument {
        StoredDocument {
            v: VERSION,
            tabs: vec![StoredTab {
                root: StoredNode::Branch {
                    orientation: StoredOrientation::Columns,
                    children: weights
                        .iter()
                        .map(|weight| StoredChild {
                            weight: *weight,
                            node: StoredNode::Leaf(StoredLeaf {
                                panel_width: 36,
                                folded: false,
                            }),
                        })
                        .collect(),
                },
            }],
        }
    }

    #[test]
    fn dos_pesos_cuyo_producto_seria_cero_se_aceptan_porque_el_cero_lo_impide_quien_reescala() {
        // **Esta prueba afirmaba lo contrario, y estaba mal.** El agujero es real —al cerrar una división
        // las nietas suben con el peso escalado, `w · suyo / suma`, y ese producto **subdesborda**:
        // comprobado ejecutándolo, `1e-40 · 1e-40 / 2e-40` da **cero**— pero cerrarlo aquí, rechazando el
        // documento, era cerrarlo en el sitio equivocado: **a estos números se llega arrastrando**, así que
        // lo que este rechazo tiraba era la disposición de quien había usado la aplicación. La respuesta
        // vive ahora donde el producto ocurre, en `crate::Weight::scaled`, y por eso aquí se aceptan.
        let tiny = 1.0e-40_f32;
        assert!(
            crate::Weight::new(tiny).is_some(),
            "cada peso por separado tiene que ser válido, o esta prueba no prueba nada"
        );
        assert_eq!(
            tiny * tiny / (tiny + tiny),
            0.0,
            "y su escalado tiene que dar cero, o tampoco"
        );

        assert!(validate(with_weights(&[tiny, tiny])).is_ok());
    }

    #[test]
    fn dos_pesos_finitos_cuya_suma_no_es_un_numero_se_rechazan() {
        let at_limit = 3.0e38_f32;
        assert!(
            crate::Weight::new(at_limit).is_some(),
            "cada peso por separado tiene que ser válido"
        );
        assert!(
            !(at_limit + at_limit).is_finite(),
            "y su suma tiene que desbordar"
        );

        // **Y cae por la suma, no por el peso.** Desde que no hay rango propio, ésta es la puerta que
        // sostiene el formato: se afirma la variante exacta porque confundirlas sería no saber cuál de las
        // dos comprobaciones está trabajando.
        assert_eq!(
            validate(with_weights(&[at_limit, at_limit])),
            Err(LayoutProblem::NonFiniteSum { tab: 0 })
        );
    }

    #[test]
    fn se_acepta_cualquier_peso_que_el_arbol_vivo_pueda_tener() {
        // **Y esto arregla un defecto en vez de evitarlo.** Hubo un rango propio, `[1e-6, 1e6]`, y los dos
        // extremos rechazaban disposiciones a las que se llega **arrastrando**: por abajo el arrastre recorta
        // la fracción y no el peso, y por arriba lo que conserva es la suma del par, así que un peso sube
        // hasta el 99 % de lo que sumaban dos vecinos. Cualquiera de los dos rechazos perdía la colocación
        // del usuario al reabrir, sin que nada se hubiera visto en pantalla.
        for weights in [
            [1.0, 1e-30],
            [1.0, f32::MIN_POSITIVE],
            [1.0, 1e30],
            [0.5, 1.5],
            [100.0, 0.01],
        ] {
            assert!(
                validate(with_weights(&weights)).is_ok(),
                "{weights:?} tenía que valer"
            );
        }
    }

    #[test]
    fn lo_que_no_es_un_peso_se_sigue_rechazando() {
        // La puerta que queda, y es la del tipo vivo: ni cero, ni negativo, ni infinito.
        for bad in [0.0, -1.0, f32::INFINITY] {
            assert_eq!(
                validate(with_weights(&[1.0, bad])),
                Err(LayoutProblem::InvalidWeight { tab: 0 }),
                "un peso de {bad:e} tenía que caer"
            );
        }
    }

    #[test]
    fn un_documento_mas_grande_que_el_techo_se_rechaza_sin_interpretarlo() {
        // **Y se rechaza por el tamaño, no por la sintaxis.** El texto es basura: si el techo se
        // comprobara después de interpretar, el motivo sería «no se entiende» y el gasto ya habría
        // ocurrido. Que el motivo sea el tamaño **es** la prueba de que el orden es el correcto.
        let huge = "x".repeat(MAX_BYTES + 1);

        assert_eq!(
            interpret(&huge),
            Err(LayoutProblem::TooLarge {
                bytes: MAX_BYTES as u64 + 1
            })
        );
    }

    #[test]
    fn un_documento_justo_del_tamaño_del_techo_se_interpreta() {
        // La otra mitad: el techo no puede estar en cero disfrazado. Se rellena con espacios, que JSON
        // ignora, hasta ocupar exactamente el techo.
        let minimum = r#"{"v":2,"tabs":[{"root":{"leaf":{"panel_width":36,"folded":false}}}]}"#;
        let padding = " ".repeat(MAX_BYTES - minimum.len());
        let exactly = format!("{minimum}{padding}");
        assert_eq!(exactly.len(), MAX_BYTES);

        assert!(interpret(&exactly).is_ok());
    }

    /// **Lo que la tanda de medida siembra tiene que seguir entendiéndose, y nadie lo afirmaba.**
    ///
    /// `scripts/medir-fotograma.sh` escribe este documento **desde `sh`**, con las etiquetas del formato
    /// puestas a mano en un `printf`. Así que el día en que este enum renombre `leaf`, `branch`, `weight` o
    /// `orientation`, el guion seguiría escribiendo el nombre viejo y **la aplicación abriría de fábrica**:
    /// la tanda que dice medir el arranque restaurado mediría el de siempre. La corrida lo cazaría —el
    /// informe no declararía consolas restauradas y el guion la invalida— pero eso son quince minutos de
    /// medida tirados, y esta prueba lo caza en la suite.
    #[test]
    fn el_documento_que_siembra_la_tanda_de_medida_se_sigue_entendiendo() {
        let seeded = r#"{"v":2,"tabs":[{"root":{"branch":{"orientation":"columns","children":[{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}},{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}},{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}},{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}}]}}}]}"#;

        let validated =
            interpret(seeded).expect("la tanda siembra un documento que ya no se entiende");

        assert_eq!(validated.leaves(), 4, "las cuatro consolas de la tanda L");

        // **Y dónde lo deja, que es la otra mitad y se escapaba.** El guion escribe en
        // `<config>/agent-terminal/layout.json` con esos dos nombres puestos a mano; renombrar
        // cualquiera de las dos constantes dejaría la suite en verde mientras **cada corrida de la tanda
        // abriría de fábrica**, porque la aplicación buscaría su fichero en otro sitio y no encontraría
        // nada.
        assert_eq!(
            DIRECTORY, "agent-terminal",
            "el guion siembra en este directorio"
        );
        assert_eq!(FILE, "layout.json", "y con este nombre de fichero");
    }

    #[test]
    fn la_cuenta_de_hojas_que_se_publica_es_la_que_se_comparo_con_el_techo() {
        // Si `leaves()` recalculara en vez de leer lo guardado, podrían dejar de ser el mismo número.
        let two_and_three = StoredDocument {
            v: VERSION,
            tabs: vec![
                StoredTab {
                    root: StoredNode::Branch {
                        orientation: StoredOrientation::Columns,
                        children: vec![
                            StoredChild {
                                weight: 1.0,
                                node: StoredNode::Leaf(StoredLeaf {
                                    panel_width: 36,
                                    folded: false,
                                }),
                            },
                            StoredChild {
                                weight: 1.0,
                                node: StoredNode::Leaf(StoredLeaf {
                                    panel_width: 36,
                                    folded: false,
                                }),
                            },
                        ],
                    },
                },
                StoredTab {
                    root: StoredNode::Leaf(StoredLeaf {
                        panel_width: 36,
                        folded: false,
                    }),
                },
            ],
        };

        let validated = validate(two_and_three).expect("el documento es válido");
        assert_eq!(validated.leaves(), 3);
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    /// El directorio de una prueba, que se lleva al acabar lo que la prueba escribió.
    ///
    /// **Uno por prueba y con el número de proceso**, porque la suite corre en paralelo y el nombre del
    /// fichero es fijo: dos pruebas que compartieran directorio se leerían la disposición la una a la
    /// otra y fallarían solo a veces.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "agent-terminal-prueba-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("crear el directorio de la prueba");
            Self(path)
        }

        /// El fichero donde la aplicación lo pondría.
        ///
        /// **Con el directorio de configuración sin crear, y no solo el propio de la aplicación**: en una
        /// cuenta recién hecha tampoco existe, así que cada prueba que guarda ejercita la creación de los
        /// dos tramos. Con el de arriba ya hecho, una creación que no admitiera tramos intermedios pasaría
        /// estas pruebas y fallaría en la máquina de quien estrena la aplicación.
        fn file(&self) -> PathBuf {
            path_from(Some(self.0.join("config").as_os_str()), None)
                .expect("la ruta de la prueba es absoluta")
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Un documento con dos consolas de anchos distintos, una plegada y con pesos desiguales.
    ///
    /// **Nada de un documento mínimo**: con una sola consola de ancho de fábrica, una ida y vuelta que
    /// perdiera los anchos, los pesos o la orientación seguiría pasando.
    /// Lo mismo, ya validado: es lo que el camino que guarda exige.
    fn test_layout() -> Validated {
        validate(test_document()).expect("el documento de prueba es válido")
    }

    fn test_document() -> StoredDocument {
        StoredDocument {
            v: VERSION,
            tabs: vec![StoredTab {
                root: StoredNode::Branch {
                    orientation: StoredOrientation::Rows,
                    children: vec![
                        StoredChild {
                            weight: 3.0,
                            node: StoredNode::Leaf(StoredLeaf {
                                panel_width: 52,
                                folded: false,
                            }),
                        },
                        StoredChild {
                            weight: 1.0,
                            node: StoredNode::Leaf(StoredLeaf {
                                panel_width: 24,
                                folded: true,
                            }),
                        },
                    ],
                },
            }],
        }
    }

    /// **Lo que se publica como tiempo de lectura tiene que salir de una lectura**, y nada lo afirmaba.
    ///
    /// El informe de medida imprime este número. Sin prueba, cambiar el cronómetro por un cero dejaba la
    /// suite entera en verde mientras el informe publicaba un tiempo que nadie tomó — el disfraz que
    /// este proyecto nombra: elegir un cero con el que el defecto y el arreglo dan lo mismo.
    #[test]
    fn el_tiempo_de_carga_solo_existe_cuando_hubo_carga() {
        let home = TestDir::new("tiempo-de-carga");
        let file = home.file();
        std::fs::create_dir_all(file.parent().expect("el fichero tiene directorio"))
            .expect("crear el directorio de configuración");
        std::fs::write(
            &file,
            r#"{"v":2,"tabs":[{"root":{"leaf":{"panel_width":36,"folded":false}}}]}"#,
        )
        .expect("sembrar la disposición");

        let mut persistence = Persistence::at(Some(file));
        assert!(
            persistence.load_time().is_none(),
            "dice haber medido una carga que todavía no ha ocurrido"
        );

        persistence.load().expect("el documento se entiende");

        // **Mayor que cero, y no solo «hay número».** Afirmando `is_some()` a secas, cambiar el
        // cronómetro por un cero deja la prueba en verde: `Some(0)` también es algo. Y un cero es
        // exactamente lo que este proyecto tiene fichado como el número con el que el defecto y el
        // arreglo dan el mismo resultado. Leer un fichero del disco no tarda cero.
        let took = persistence
            .load_time()
            .expect("leyó el fichero y no dejó dicho lo que tardó: el informe diría «sin medir»");
        assert!(
            took > Duration::ZERO,
            "publica un cero como tiempo de lectura, que es un número que nadie tomó"
        );
    }

    #[test]
    fn la_ruta_sale_del_directorio_de_configuracion() {
        assert_eq!(
            path_from(Some(OsStr::new("/casa/config")), Some(OsStr::new("/casa"))),
            Some(PathBuf::from("/casa/config/agent-terminal/layout.json"))
        );
    }

    #[test]
    fn sin_directorio_de_configuracion_se_cae_a_la_casa() {
        assert_eq!(
            path_from(None, Some(OsStr::new("/casa"))),
            Some(PathBuf::from("/casa/.config/agent-terminal/layout.json"))
        );
    }

    #[test]
    fn una_variable_vacia_o_relativa_no_es_una_ruta_y_no_descarta_la_otra() {
        // La casa es distinta de las dos variables de arriba a propósito: si la vacía o la relativa se
        // colaran, la ruta no sería ésta.
        for variable in ["", "config", "./config", "~/config"] {
            assert_eq!(
                path_from(Some(OsStr::new(variable)), Some(OsStr::new("/casa"))),
                Some(PathBuf::from("/casa/.config/agent-terminal/layout.json")),
                "con {variable:?} no se cayó a la casa"
            );
        }
    }

    #[test]
    fn sin_ninguna_ruta_absoluta_no_se_persiste() {
        assert_eq!(path_from(None, None), None);
        assert_eq!(path_from(Some(OsStr::new("")), Some(OsStr::new(""))), None);
        assert_eq!(
            path_from(Some(OsStr::new("config")), Some(OsStr::new("casa"))),
            None
        );
    }

    #[test]
    fn lo_guardado_se_vuelve_a_encontrar() {
        let directory = TestDir::new("ida-y-vuelta");
        let path = directory.file();

        assert!(
            Persistence::at(Some(path.clone())).save(&test_layout()),
            "no se guardó"
        );

        let read_back = read(&path).expect("lo que se acaba de guardar se lee");
        assert_eq!(read_back.tabs(), test_document().tabs.as_slice());
    }

    #[cfg(unix)]
    #[test]
    fn el_destino_se_sustituye_entero_y_nunca_se_escribe_encima() {
        use std::os::unix::fs::MetadataExt;

        // **La promesa central del hito, y solo esto la ata.** Copiar el texto encima del destino dejaría
        // las mismas tres pruebas de resultado final en verde —el fichero acaba con el contenido correcto
        // y sin temporales al lado— y sin embargo habría un instante en que lo que hay en disco es medio
        // documento. Lo que distingue sustituir de escribir encima es que **el destino es otro fichero**:
        // con renombrado cambia de nodo, copiando no.
        let directory = TestDir::new("sustitucion");
        let path = directory.file();
        let mut persistence = Persistence::at(Some(path.clone()));

        assert!(persistence.save(&test_layout()));
        let first = std::fs::metadata(&path).expect("existe").ino();
        assert!(persistence.save(&test_layout()));
        let second = std::fs::metadata(&path).expect("existe").ino();

        assert_ne!(
            first, second,
            "el destino se escribió encima en vez de sustituirse"
        );
    }

    #[test]
    fn el_guardado_no_deja_ningun_temporal_detras() {
        let directory = TestDir::new("sin-restos");
        let path = directory.file();
        let mut file = Persistence::at(Some(path.clone()));

        // Dos veces, porque el segundo guardado es el que encuentra el directorio y el destino ya
        // hechos: es el camino que se recorre de verdad, dos veces por segundo mientras se arrastra.
        assert!(file.save(&test_layout()));
        assert!(file.save(&test_layout()));

        let inside: Vec<_> = std::fs::read_dir(path.parent().expect("el directorio propio"))
            .expect("leer el directorio")
            .map(|entry| entry.expect("una entrada").file_name())
            .collect();
        assert_eq!(inside, vec![std::ffi::OsString::from(FILE)]);
    }

    #[cfg(unix)]
    #[test]
    fn el_directorio_y_el_fichero_nacen_solo_para_el_usuario() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new("permisos");
        let path = directory.file();
        assert!(Persistence::at(Some(path.clone())).save(&test_layout()));

        let modo = |path: &Path| {
            std::fs::metadata(path)
                .expect("existe")
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(
            modo(path.parent().expect("el directorio propio")),
            0o700,
            "el directorio queda abierto a la máscara de la sesión"
        );
        assert_eq!(modo(&path), 0o600, "el fichero queda legible por otros");
    }

    #[test]
    fn un_fichero_ausente_no_restaura_nada_y_deja_arrancar() {
        let directory = TestDir::new("ausente");
        let path = directory.file();

        assert_eq!(read(&path), Err(LayoutProblem::Absent));
        assert!(Persistence::at(Some(path)).load().is_none());
    }

    #[test]
    fn lo_que_no_es_un_fichero_se_rechaza_antes_de_abrirlo() {
        let directory = TestDir::new("no-es-fichero");
        let path = directory.file();
        std::fs::create_dir_all(&path).expect("plantar un directorio con el nombre del fichero");

        assert_eq!(read(&path), Err(LayoutProblem::NotAFile));
    }

    #[test]
    fn un_fichero_que_se_pasa_del_techo_se_rechaza_por_su_tamaño_y_no_por_su_codificacion() {
        let directory = TestDir::new("demasiado-grande");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        // **Acentos, y no letras sueltas, y ahí está el defecto que esto ata.** La lectura corta un byte
        // por encima del techo; con caracteres de dos bytes ese corte parte uno por la mitad, así que sin
        // la comprobación del tamaño esto saldría como «no es texto» y el techo no se alcanzaría nunca.
        let huge = "á".repeat(MAX_BYTES);
        std::fs::write(&path, &huge).expect("escribir el fichero gordo");

        assert_eq!(
            read(&path),
            Err(LayoutProblem::TooLarge {
                bytes: huge.len() as u64
            })
        );
    }

    #[test]
    fn justo_en_el_techo_se_lee_y_un_byte_mas_no() {
        let directory = TestDir::new("en-el-techo");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        let document = serialize(&test_document()).expect("se describe");

        // El relleno son espacios, que el intérprete acepta al final: así el documento sigue siendo
        // válido y lo único que cambia entre las dos mitades de esta prueba es **un byte de tamaño**.
        // Sin la mitad de abajo, un techo que no acotara nada pasaría igual.
        let at_ceiling = format!("{document}{}", " ".repeat(MAX_BYTES - document.len()));
        std::fs::write(&path, &at_ceiling).expect("escribir");
        assert!(
            read(&path).is_ok(),
            "lo que ocupa justo el techo no se leyó"
        );

        std::fs::write(&path, format!("{at_ceiling} ")).expect("escribir uno más");
        assert_eq!(
            read(&path),
            Err(LayoutProblem::TooLarge {
                bytes: MAX_BYTES as u64 + 1
            })
        );
    }

    #[test]
    fn lo_que_no_es_texto_se_rechaza() {
        let directory = TestDir::new("no-es-texto");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        std::fs::write(&path, [0xff, 0xfe, b'{', b'}']).expect("escribir bytes que no son texto");

        assert_eq!(read(&path), Err(LayoutProblem::NotText));
    }

    #[test]
    fn sin_texto_que_escribir_no_se_renombra_nada() {
        // **Ésta es la razón de que la escritura reciba el texto ya descrito.** Si describir fallara y el
        // camino renombrara igual, el destino quedaría con cero bytes: la disposición que el usuario
        // colocó a mano, borrada por un fallo al guardar.
        let directory = TestDir::new("sin-texto");
        let path = directory.file();
        let mut file = Persistence::at(Some(path.clone()));
        assert!(file.save(&test_layout()));
        let start = std::fs::read_to_string(&path).expect("hay algo guardado");

        let error = save_described(&path, 1, None);

        assert!(
            matches!(error, Err(SaveFailure::NotSerializable)),
            "{error:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("sigue estando"),
            start,
            "se perdió la disposición que había guardada"
        );
        let inside = std::fs::read_dir(path.parent().expect("el directorio propio"))
            .expect("leer el directorio")
            .count();
        assert_eq!(inside, 1, "quedó algo más en el directorio");
    }

    #[test]
    fn un_temporal_que_ya_existe_no_se_adopta() {
        let directory = TestDir::new("temporal-plantado");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        let temp_path = temp_path_for(&path, 1).expect("el temporal tiene nombre");
        std::fs::write(&temp_path, "de otro").expect("plantar el temporal");

        let error = save_to(&path, 1, &test_document());

        assert!(matches!(error, Err(SaveFailure::TempFile(_))), "{error:?}");
        assert!(!path.exists(), "se escribió el destino de todas formas");
        assert_eq!(
            std::fs::read_to_string(&temp_path).expect("sigue estando"),
            "de otro",
            "se escribió a través de lo que había plantado"
        );
    }

    #[test]
    fn si_no_se_puede_poner_en_su_sitio_no_queda_el_temporal() {
        // El destino es un directorio con algo dentro, que es una forma de que el renombrado falle sin
        // tocar los permisos de nadie. Lo que se ata es la limpieza: un temporal que sobreviva a un fallo
        // **deja a esta misma sesión sin poder crear el suyo en exclusiva**, así que el primer fallo
        // sería el último intento.
        let directory = TestDir::new("renombrado-imposible");
        let path = directory.file();
        std::fs::create_dir_all(path.join("dentro"))
            .expect("plantar un directorio con algo dentro");

        let error = save_to(&path, 1, &test_document());

        assert!(matches!(error, Err(SaveFailure::Renaming(_))), "{error:?}");
        assert!(
            !temp_path_for(&path, 1)
                .expect("el temporal tiene nombre")
                .exists(),
            "quedó el temporal, y con él la sesión se queda sin poder guardar"
        );
    }

    #[test]
    fn una_version_mas_nueva_no_se_toca_en_toda_la_sesion() {
        let directory = TestDir::new("version-mas-nueva");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        // **La forma es la que una versión 3 tendría de verdad: una clave renombrada.** Y eso es el
        // corazón de esta prueba, no un detalle. La versión sube **justo** con lo que un lector viejo
        // malinterpretaría —una clave renombrada, quitada, o con otro significado—, o sea con lo que hace
        // fallar al intérprete de la forma. Con un fichero de la versión 3 que conservara la forma de la 2,
        // esta prueba pasa incluso mirando la versión **después** de exigir la forma, que es exactamente el
        // defecto que se colaba: el fichero del futuro se leía como «no se entiende» y se sobrescribía.
        let of_the_future = r#"{"v":3,"solapas":[{"raiz":{"hoja":{"ancho":36,"plegado":false}}}]}"#;
        std::fs::write(&path, of_the_future).expect("escribir el fichero del futuro");
        let mut file = Persistence::at(Some(path.clone()));

        assert!(
            file.load().is_none(),
            "se restauró una forma que no se conoce"
        );
        // Dos intentos, porque el veto es de la sesión y no del intento: con uno, un veto que solo
        // valiera para el primero pasaría igual.
        assert!(!file.save(&test_layout()));
        assert!(!file.save(&test_layout()));
        assert_eq!(
            std::fs::read_to_string(&path).expect("sigue estando"),
            of_the_future,
            "un binario viejo se llevó por delante la disposición del nuevo"
        );
    }

    #[test]
    fn un_fichero_sin_version_es_uno_nuestro_estropeado_y_se_sobrescribe() {
        // La otra mitad del veto, y la que distingue «no sé qué versión es esto» de «esto lo escribió algo
        // que sabe más que yo». Un fichero **sin** versión no puede ser de una versión futura —todas la
        // llevan—, así que es nuestro y está roto: se abre de fábrica y se sustituye.
        let directory = TestDir::new("sin-version");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        std::fs::write(
            &path,
            r#"{"tabs":[{"root":{"leaf":{"panel_width":36,"folded":false}}}]}"#,
        )
        .expect("escribir");
        let mut persistence = Persistence::at(Some(path.clone()));

        assert!(persistence.load().is_none());
        assert!(
            persistence.save(&test_layout()),
            "un fichero sin versión se trató como del futuro"
        );
        assert_eq!(
            read(&path).expect("se lee").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn una_version_que_no_se_conoce_y_no_es_mas_nueva_se_sobrescribe() {
        let directory = TestDir::new("version-cero");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        std::fs::write(&path, r#"{"v":0,"tabs":[]}"#).expect("escribir");
        let mut file = Persistence::at(Some(path.clone()));

        assert!(file.load().is_none());
        assert!(
            file.save(&test_layout()),
            "un fichero nuestro estropeado no se sustituye"
        );
        assert_eq!(
            read(&path).expect("se lee").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn sin_sitio_donde_ponerlo_no_se_lee_ni_se_escribe() {
        let mut file = Persistence::at(None);

        assert!(file.load().is_none());
        assert!(!file.save(&test_layout()));
    }

    #[test]
    fn de_cada_motivo_se_avisa_una_vez_y_de_los_demas_igual() {
        let mut file = Persistence::at(None);
        let other = || std::io::Error::other("otro");

        assert!(file.should_warn(&SaveFailure::Renaming(other())));
        assert!(
            !file.should_warn(&SaveFailure::Renaming(std::io::Error::other("distinto"))),
            "el mismo motivo con otro error del sistema se avisó dos veces"
        );
        assert!(
            file.should_warn(&SaveFailure::Directory(other())),
            "un motivo nuevo se calló porque ya se había avisado de otro"
        );
    }

    #[test]
    fn del_unico_fallo_que_es_normal_no_se_avisa_y_de_los_demas_si() {
        // El aviso es una línea a la salida de error, que no se puede observar desde dentro del proceso:
        // lo que se puede afirmar es **la decisión**, y por eso vive aparte de quien la imprime.
        assert!(
            !warrants_warning(&LayoutProblem::Absent),
            "el primer arranque de cada usuario saldría con una línea de error"
        );
        for problem in [
            LayoutProblem::NotAFile,
            LayoutProblem::NotText,
            LayoutProblem::Unreadable("cualquiera".to_owned()),
            LayoutProblem::Version { found: 3 },
            LayoutProblem::TooLarge { bytes: 1 },
            LayoutProblem::NoTabs,
        ] {
            assert!(warrants_warning(&problem), "se calla: {problem}");
        }
    }

    #[test]
    fn el_temporal_lleva_el_proceso_y_el_intento() {
        // **Una afirmación sobre el nombre, y no sobre el efecto, porque el efecto no cabe en un proceso**:
        // lo que el nombre evita es que dos instancias compartan temporal y que un temporal huérfano de una
        // muerte violenta deje sin guardar a la sesión que reciba ese mismo número de proceso.
        let path = PathBuf::from("/casa/.config/agent-terminal/layout.json");
        let pid = std::process::id();

        let first = temp_path_for(&path, 1).expect("tiene nombre");
        let second = temp_path_for(&path, 2).expect("tiene nombre");

        assert_eq!(
            first,
            PathBuf::from(format!("/casa/.config/agent-terminal/layout.json.{pid}.1"))
        );
        assert_ne!(first, second, "dos intentos comparten temporal");
        assert_eq!(
            first.parent(),
            path.parent(),
            "el temporal no está al lado del destino, así que el renombrado puede cruzar volúmenes"
        );
    }

    #[test]
    fn un_temporal_huerfano_estorba_un_intento_y_no_la_sesion() {
        // El escenario: la aplicación murió entre crear el temporal y renombrarlo, así que quedó uno en
        // disco que nadie limpia; y esta sesión recibió el mismo número de proceso. Adoptarlo está
        // prohibido —se escribiría a través de lo que otro plantó—, así que lo que tiene que ocurrir es que
        // el intento siguiente use otro nombre. Sin eso, esta sesión **no guardaría nunca nada**.
        let directory = TestDir::new("huerfano");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        std::fs::write(
            temp_path_for(&path, 1).expect("tiene nombre"),
            "el que dejó el muerto",
        )
        .expect("plantar el huérfano");
        let mut persistence = Persistence::at(Some(path.clone()));

        assert!(
            !persistence.save(&test_layout()),
            "se adoptó el temporal que había"
        );
        assert!(
            persistence.save(&test_layout()),
            "un huérfano dejó a la sesión entera sin poder guardar"
        );
        assert_eq!(
            read(&path).expect("se lee").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn leer_no_reserva_lo_que_el_fichero_dice_ocupar() {
        // **El techo existe para acotar la reserva, no solo el intérprete**, y eso es lo único que esta
        // prueba mira: la capacidad. Con el tamaño del fichero como reserva y sin acotarlo, un fichero que
        // dice ocupar diez gigas los pide al arrancar, y el techo del formato no llega a mirarse.
        let directory = TestDir::new("reserva");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        let file = std::fs::File::create(&path).expect("crear");
        // Un agujero, no cuatro megas de ceros escritos: lo que importa es lo que dicen los datos del
        // fichero, que es de donde sale la reserva.
        file.set_len(64 * MAX_BYTES as u64)
            .expect("agrandarlo sin escribir");
        drop(file);

        let bytes = read_capped(
            std::fs::File::open(&path).expect("abrir"),
            std::fs::metadata(&path).expect("datos").len(),
        )
        .expect("leer");

        assert!(
            bytes.capacity() <= MAX_BYTES + 1,
            "reservó {} bytes para un techo de {MAX_BYTES}",
            bytes.capacity()
        );
    }

    #[cfg(unix)]
    #[test]
    fn una_tuberia_con_nombre_se_rechaza_sin_quedarse_esperando() {
        // **Es el caso que la comprobación de «es un fichero» existe para evitar, y el único que un
        // directorio plantado no enseña**: abrir una tubería sin nadie al otro lado no falla, **espera**, y
        // esto ocurre antes de que la ventana exista. El plazo es el testigo: sin la comprobación la lectura
        // no vuelve nunca, y lo que esta prueba puede contar es que no volvió.
        let directory = TestDir::new("tuberia");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        let planted = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("lanzar mkfifo");
        assert!(planted.success(), "no se pudo plantar la tubería");

        // En otro hilo a propósito: si la apertura se queda esperando, el que espera es él. Se queda
        // colgado y se lo lleva el final del proceso.
        let (dice, listening) = std::sync::mpsc::channel();
        let its_own = path.clone();
        std::thread::spawn(move || dice.send(read(&its_own)));

        match listening.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(result) => assert_eq!(result, Err(LayoutProblem::NotAFile)),
            Err(_) => panic!("abrir la tubería se quedó esperando: la ventana no llegaría a abrir"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn un_fichero_que_no_se_deja_leer_no_impide_arrancar_ni_veta_la_escritura() {
        use std::os::unix::fs::PermissionsExt;

        // El único fallo del sistema que la lectura puede dar y que nadie ejercita. Y lo que importa no es
        // la variante: es que **no se confunde con una versión del futuro**, así que la sesión sigue
        // pudiendo guardar. Un permiso mal puesto no debe dejar a nadie sin escribir el resto de la sesión.
        //
        // **Esta prueba supone que la suite no corre como administrador**, que se salta los permisos: ahí
        // el fichero se leería y la prueba fallaría al no encontrar el fallo. Se deja fallar en vez de
        // saltarse sola, porque una prueba que se desactiva según quién la corre no se distingue de una
        // que pasa.
        let directory = TestDir::new("ilegible");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        std::fs::write(&path, r#"{"v":2,"tabs":[]}"#).expect("escribir");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("quitarle todos los permisos");

        let problem = read(&path);
        assert!(
            matches!(problem, Err(LayoutProblem::Unreadable(_))),
            "{problem:?}"
        );

        let mut persistence = Persistence::at(Some(path.clone()));
        assert!(persistence.load().is_none());
        assert!(
            persistence.save(&test_layout()),
            "un fichero que no se pudo leer se trató como uno del futuro"
        );
        assert_eq!(
            read(&path).expect("se lee").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn una_forma_invalida_de_la_version_conocida_no_impide_arrancar_y_se_sustituye() {
        // **La versión es la nuestra y la forma no vale**, que es lo que distingue este camino de los otros
        // dos ficheros rotos: ahí falla el intérprete, y aquí encaja todo y lo que no se sostiene es el
        // árbol. Sin esto, un lector que se olvidara de validar lo que interpreta pasaría igual: las
        // pruebas de la forma llaman a `validate` a mano, así que ninguna ata que el camino del disco pase
        // por ahí.
        let directory = TestDir::new("forma-invalida");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        // Un corte con un solo hijo: la forma que hace que una frontera arrastre un bloque de dos.
        std::fs::write(
            &path,
            r#"{"v":2,"tabs":[{"root":{"branch":{"orientation":"columns","children":[
                 {"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}}]}}}]}"#,
        )
        .expect("escribir");
        let mut persistence = Persistence::at(Some(path.clone()));

        assert_eq!(
            read(&path),
            Err(LayoutProblem::SingleChildBranch { tab: 0 })
        );
        assert!(
            persistence.load().is_none(),
            "se restauró un árbol que la validación rechaza"
        );
        assert!(
            persistence.save(&test_layout()),
            "un fichero nuestro con una forma que no vale se trató como uno del futuro"
        );
        assert_eq!(
            read(&path).expect("se lee").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn si_el_directorio_no_se_puede_crear_se_dice_por_ese_motivo_y_no_por_otro() {
        // **La variante importa porque el aviso se suprime por motivo**: un directorio que no se puede
        // crear y un temporal que no se puede crear se arreglan en sitios distintos, así que llegar aquí
        // como lo segundo dejaría al usuario mirando donde no es.
        let directory = TestDir::new("directorio-imposible");
        std::fs::write(directory.0.join("config"), "no soy un directorio")
            .expect("plantar un fichero donde iría el directorio de configuración");
        let path = directory.file();

        let error = save_to(&path, 1, &test_document());

        assert!(matches!(error, Err(SaveFailure::Directory(_))), "{error:?}");
        assert!(
            !Persistence::at(Some(path.clone())).save(&test_layout()),
            "se dio por guardado sin haber dónde ponerlo"
        );
        assert!(!path.exists(), "algo acabó en el sitio del fichero");
    }

    #[test]
    fn una_ruta_que_no_nombra_un_fichero_no_tumba_el_guardado() {
        // Ninguna de las dos sale de `path_from`, así que solo se llega colocándolas a mano. Lo que se ata es
        // que este camino no tiene ningún desenvolver que tumbe la ventana en vez de decir que no pudo:
        // guardar corre dos veces por segundo mientras se arrastra una frontera.
        let without_directory = save_to(Path::new("/"), 1, &test_document());
        assert!(
            matches!(without_directory, Err(SaveFailure::NoPlace)),
            "{without_directory:?}"
        );
        assert!(!Persistence::at(Some(PathBuf::from("/"))).save(&test_layout()));

        let directory = TestDir::new("sin-nombre");
        let subiendo = directory
            .file()
            .parent()
            .expect("el directorio propio")
            .join("..");
        let without_name = save_to(&subiendo, 1, &test_document());
        assert!(
            matches!(without_name, Err(SaveFailure::NoPlace)),
            "{without_name:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn un_enlace_plantado_en_el_destino_no_se_escribe_a_traves() {
        // El destino se protege **no abriéndolo nunca para escribir**, y esto es lo que eso compra: un
        // enlace con su nombre no lleva la escritura a donde apunte. Que el destino cambie de nodo lo ata
        // otra prueba; ésta ata que el fichero de fuera no se toca.
        let directory = TestDir::new("enlace-en-el-destino");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("el directorio propio")).expect("crearlo");
        let victim = directory.0.join("victima");
        std::fs::write(&victim, "lo que había fuera").expect("escribir la víctima");
        std::os::unix::fs::symlink(&victim, &path).expect("plantar el enlace");

        assert!(Persistence::at(Some(path.clone())).save(&test_layout()));

        assert_eq!(
            std::fs::read_to_string(&victim).expect("sigue estando"),
            "lo que había fuera",
            "se escribió a través del enlace, o sea fuera del directorio de configuración"
        );
        assert!(
            !std::fs::symlink_metadata(&path)
                .expect("existe")
                .is_symlink(),
            "el enlace sigue en el sitio del fichero"
        );
    }

    #[test]
    fn un_guardado_que_falla_gasta_el_aviso_de_su_motivo() {
        // **Ata que la supresión la aplica el camino que guarda**, y no solo que la lista de motivos
        // funcione. Con el aviso impreso sin consultarla, un disco lleno diría dos líneas por segundo
        // mientras la ventana se use, que es lo que la lista existe para evitar.
        let mut file = Persistence::at(None);

        assert!(!file.save(&test_layout()));

        assert!(
            !file.should_warn(&SaveFailure::NoPlace),
            "el fallo del guardado no pasó por la supresión"
        );
        assert!(
            file.should_warn(&SaveFailure::Directory(std::io::Error::other("otro"))),
            "gastó también el aviso de motivos que no han fallado"
        );
    }

    #[test]
    fn cargar_sin_sitio_no_gasta_el_aviso_del_guardado() {
        // Los dos caminos comparten el motivo y no el mensaje. Si cargar gastara el aviso, el primer fallo
        // de guardado de verdad —el que el usuario necesita ver— saldría mudo.
        let mut file = Persistence::at(None);

        assert!(file.load().is_none());

        assert!(
            file.should_warn(&SaveFailure::NoPlace),
            "cargar gastó el aviso y el primer fallo al guardar se quedaría sin decirse"
        );
    }
}
