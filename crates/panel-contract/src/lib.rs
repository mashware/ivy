//! Tipos de `panel.json`, lectura y validación, y **el directorio donde ese fichero vive**.
//!
//! No depende de nada visual: si la capa de pintado se reescribe, esta caja sigue sirviendo.
//!
//! El directorio está aquí y no en quien lanza las consolas por lo que dice la línea de arriba:
//! **nada de generar un nombre, crear un directorio y borrarlo tiene que ver con pintar**, así que
//! ponerlo en la caja que se va a reescribir el día que cambie el framework de interfaz obligaría a
//! reescribirlo con ella sin ninguna razón. Y *dónde* se publica el estado es parte del contrato,
//! igual que *qué* se publica.

use std::ffi::OsStr;
use std::fs::DirBuilder;
use std::io;
use std::path::{Path, PathBuf};

mod panel;

pub use panel::{
    Attention, Line, MAX_BYTES, MAX_INDENT, Mark, Panel, Problem, STALE_AFTER, Style, read,
    read_if_newer,
};

/// El único fichero que el contrato pone dentro del directorio. Lo escribe quien hace el trabajo,
/// no esta caja.
///
/// **Público porque hace falta fuera.** El vigilante que llega con F1.6 corre en la aplicación y
/// tiene que descartar los avisos de cualquier otra ruta, así que necesita nombrar este fichero; sin
/// exportarlo, la cadena se volvería a escribir a mano en cada sitio y el contrato dejaría de vivir
/// en uno solo.
pub const FICHERO_DEL_PANEL: &str = "panel.json";

/// Los nombres de las dos variables del contrato, en un solo sitio.
///
/// Están aquí, y no en cada sitio que las escribe, porque hacen falta **también cuando no hay
/// directorio**: entonces hay que quitarlas del entorno del programa, y para quitarlas hay que
/// saber cómo se llaman.
pub const ENV_NAMES: [&str; 2] = ["PANEL_ID", "PANEL_DIR"];

/// El directorio de una división: su identificador y la ruta donde su productor publica el estado.
///
/// Nace con el directorio ya creado y **lo borra al morir**. Esa es la razón de que sea un tipo y no
/// dos funciones sueltas: el `Drop` cubre caminos que una limpieza explícita se dejaría —el error a
/// medio construir de quien lo usa, y el pánico, donde el framework de interfaz no ejecuta su gancho
/// de apagado pero los destructores sí corren.
#[derive(Debug)]
pub struct PanelDir {
    id: String,
    ruta: PathBuf,
}

impl PanelDir {
    /// Genera el identificador, crea su directorio y lo deja listo para que el productor escriba.
    ///
    /// **La creación es la comprobación de unicidad**: el directorio se crea sin recursión, así que
    /// si el nombre ya existiera esto falla en vez de compartir el directorio con otra división —y
    /// compartirlo rompería en silencio el acotado por identificador del que depende el resto.
    /// Por eso también la ruta añade **un solo componente**: no hay ningún tramo intermedio que
    /// hubiera que crear antes, ni que un tercero pueda dejar preparado en un temporal compartido.
    pub fn create() -> io::Result<Self> {
        let id = identificador()?;
        let ruta = base().join(format!("agent-terminal-{id}"));
        crear(&ruta)?;

        Ok(Self { id, ruta })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.ruta
    }

    /// Las dos variables que el proceso de la división hereda.
    ///
    /// **El array es de dos y no una lista, porque esa cifra es lo que se revisa.** Quien lanza el
    /// programa acepta una lista abierta de pares —tiene que aceptarla, porque no sabe qué es un
    /// panel—, así que el techo no está ahí: está en que lo único que se le pasa sale de aquí.
    /// Añadir una tercera variable obliga a cambiar este tipo, en la caja del contrato; con una
    /// lista construida en el sitio de la llamada, inyectar una más —una credencial, por ejemplo—
    /// sería un dato y no un cambio que alguien tuviera que aprobar.
    pub fn env(&self) -> [(&'static str, &OsStr); 2] {
        [
            (ENV_NAMES[0], OsStr::new(&self.id)),
            (ENV_NAMES[1], self.ruta.as_os_str()),
        ]
    }
}

impl Drop for PanelDir {
    /// Borra lo que el contrato pone en el directorio, y después el directorio.
    ///
    /// **Sin recursión, a propósito.** El directorio es de esta caja, pero su contenido es del
    /// productor: un borrado recursivo se llevaría por delante lo que hubiera dejado ahí, y
    /// convertiría cualquier ruta mal construida en pérdida de datos en vez de en un error. Si queda
    /// algo que no es del contrato, el directorio sobrevive y se dice por dónde.
    fn drop(&mut self) {
        // Se avisa porque el respaldo a un temporal del sistema **no** se vacía al cerrar la sesión,
        // al contrario que el directorio de ejecución del usuario: sin esta línea, un resto ahí sería
        // invisible y se iría acumulando en cuanto haya productores que escriban de verdad.
        if let Some(fallo) = borrar(&self.ruta) {
            eprintln!(
                "queda el directorio del panel {} ({fallo})",
                self.ruta.display()
            );
        }
    }
}

/// Borra el fichero del contrato y el directorio, y devuelve **lo que merece un aviso**.
///
/// Que el directorio ya no esté no lo merece: el usuario tiene un shell dentro de su consola y puede
/// haberlo borrado él. Que quede algo que no es del contrato, sí.
///
/// Está separado del destructor para poder comprobarse: un destructor no devuelve nada, y la decisión
/// de qué se calla y qué se dice es justo lo que hay que poder afirmar.
fn borrar(ruta: &Path) -> Option<io::Error> {
    // Que no esté es lo normal: hasta que alguien publique, el directorio está vacío.
    let _ = std::fs::remove_file(ruta.join(FICHERO_DEL_PANEL));

    match std::fs::remove_dir(ruta) {
        Ok(()) => None,
        Err(fallo) if fallo.kind() == io::ErrorKind::NotFound => None,
        Err(fallo) => Some(fallo),
    }
}

/// Crea el directorio, y **falla si ya existe**.
///
/// Sin recursión por las dos razones: no hay tramo intermedio que crear, y el modo de crear que sí
/// admite tramos intermedios tampoco protesta cuando la ruta ya está — adoptaría un directorio que
/// esta caja no hizo.
fn crear(ruta: &Path) -> io::Result<()> {
    let mut como = DirBuilder::new();
    // Sin esto el modo sale de la máscara de la sesión —medido: `0775` con la de esta máquina—, y
    // ahí va a vivir un fichero que cuenta en qué anda el usuario. Bajo el directorio de ejecución
    // del usuario no se alcanzaría, porque ya es suyo y solo suyo; pero el respaldo puede ser un
    // temporal compartido por todos.
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        como.mode(0o700);
    }
    como.create(ruta)
}

/// Ocho dígitos hexadecimales.
///
/// Los cuatro del ejemplo de la decisión que fija esto son 65.536 nombres, y con una creación que
/// falla si el nombre ya existe una colisión deja a una división sin identidad. Treinta y dos bits
/// cuestan lo mismo.
fn identificador() -> io::Result<String> {
    // El error se convierte a mano y no con `?` para no tener que activarle a `getrandom` la feature
    // que trae esa conversión: ya está compilado en el árbol sin ella, y pedirla lo recompilaría.
    let azar = getrandom::u32().map_err(|fallo| io::Error::other(fallo.to_string()))?;
    Ok(formatear(azar))
}

/// El formato, aparte del sorteo, para poder comprobarlo con valores dados.
///
/// El relleno con ceros es lo que hace que sean **siempre** ocho dígitos: sin él, un número pequeño
/// daría un nombre más corto, y una prueba que mire la longitud de un identificador sorteado al azar
/// casi nunca lo notaría.
fn formatear(azar: u32) -> String {
    format!("{azar:08x}")
}

/// Dónde viven los directorios de las divisiones.
///
/// El directorio de ejecución del usuario es la primera opción por lo que es: suyo y solo suyo, en
/// memoria, y el sistema lo vacía al cerrar la sesión. Cuando no existe —macOS, o una sesión de
/// Linux sin gestor— se cae al temporal del sistema, que es la única alternativa que existe en todas
/// partes.
fn base() -> PathBuf {
    base_de(std::env::var_os("XDG_RUNTIME_DIR").as_deref())
}

/// La decisión, separada de leer el entorno para poder comprobarla sin tocarlo.
///
/// **Una variable definida y vacía no es una ruta**: dejarla pasar daría una ruta relativa, y el
/// directorio se crearía dentro del directorio de trabajo de la aplicación — sin fallar, que es lo
/// peor que podría hacer.
fn base_de(variable: Option<&OsStr>) -> PathBuf {
    match variable {
        Some(ruta) if !ruta.is_empty() => PathBuf::from(ruta),
        _ => std::env::temp_dir(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_directorio_nace_creado_y_se_puede_escribir_dentro() {
        let panel = PanelDir::create().expect("crear el directorio del panel");

        assert!(panel.path().is_dir(), "no es un directorio: {panel:?}");
        std::fs::write(panel.path().join(FICHERO_DEL_PANEL), b"{}")
            .expect("el productor tiene que poder publicar ahí");
    }

    #[test]
    fn la_ruta_termina_en_el_identificador() {
        let panel = PanelDir::create().expect("crear el directorio del panel");

        let ultimo = panel
            .path()
            .file_name()
            .expect("la ruta tiene un último tramo")
            .to_string_lossy()
            .into_owned();

        assert_eq!(ultimo, format!("agent-terminal-{}", panel.id()));
    }

    #[test]
    fn al_soltarlo_el_directorio_desaparece() {
        let ruta = {
            let panel = PanelDir::create().expect("crear el directorio del panel");
            panel.path().to_path_buf()
        };

        assert!(!ruta.exists(), "quedó el directorio: {}", ruta.display());
    }

    /// El fichero del contrato lo escribe el productor, no esta caja — y aun así se lo lleva al
    /// cerrar. Sin este test, el borrado pasaría igual el resto de la suite y dejaría un directorio
    /// por división en cuanto alguien publicara de verdad.
    #[test]
    fn al_soltarlo_se_lleva_el_fichero_del_contrato() {
        let ruta = {
            let panel = PanelDir::create().expect("crear el directorio del panel");
            std::fs::write(panel.path().join(FICHERO_DEL_PANEL), b"{\"lines\":[]}")
                .expect("publicar un panel");
            panel.path().to_path_buf()
        };

        assert!(!ruta.exists(), "quedó el directorio: {}", ruta.display());
    }

    /// La cara B del borrado sin recursión: lo que no es del contrato no se toca, y por eso el
    /// directorio sigue ahí. Es la propiedad que se quiere, no un descuido.
    #[test]
    fn un_fichero_que_no_es_del_contrato_deja_el_directorio_en_pie() {
        let ruta = {
            let panel = PanelDir::create().expect("crear el directorio del panel");
            std::fs::write(panel.path().join("registro.log"), b"trabajo del productor")
                .expect("escribir algo ajeno al contrato");
            panel.path().to_path_buf()
        };

        // Se recoge el hecho y se limpia **antes** de afirmar: con la limpieza detrás del `assert`, un
        // fallo del test dejaría el directorio ahí para siempre.
        let sobrevivio = ruta.join("registro.log").is_file();
        std::fs::remove_dir_all(&ruta).expect("limpiar lo que el test dejó a propósito");

        assert!(sobrevivio, "se borró algo que no era del contrato");
    }

    #[test]
    fn dos_directorios_no_comparten_ni_identificador_ni_ruta() {
        let una = PanelDir::create().expect("crear el primero");
        let otra = PanelDir::create().expect("crear el segundo");

        assert_ne!(una.id(), otra.id());
        assert_ne!(una.path(), otra.path());
    }

    #[test]
    fn las_dos_variables_son_las_del_contrato_y_apuntan_al_directorio() {
        let panel = PanelDir::create().expect("crear el directorio del panel");

        let [
            (nombre_del_id, valor_del_id),
            (nombre_del_dir, valor_del_dir),
        ] = panel.env();

        assert_eq!(nombre_del_id, "PANEL_ID");
        assert_eq!(nombre_del_dir, "PANEL_DIR");
        assert_eq!(valor_del_id, OsStr::new(panel.id()));
        assert_eq!(valor_del_dir, panel.path().as_os_str());
    }

    /// La unicidad no se comprueba: se obtiene de que crear falle. Si esto dejara de fallar, dos
    /// divisiones podrían acabar compartiendo directorio y nada más en la caja lo notaría.
    #[test]
    fn crear_sobre_un_directorio_que_ya_existe_falla() {
        let panel = PanelDir::create().expect("crear el directorio del panel");

        let otra_vez = crear(panel.path());

        assert!(
            otra_vez.is_err(),
            "crear sobre lo que ya existe tiene que fallar"
        );
    }

    #[cfg(unix)]
    #[test]
    fn el_directorio_no_lo_puede_leer_nadie_mas() {
        use std::os::unix::fs::PermissionsExt;

        let panel = PanelDir::create().expect("crear el directorio del panel");

        let modo = panel
            .path()
            .metadata()
            .expect("leer los permisos")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(modo, 0o700, "modo {modo:o}, y ahí va a vivir el panel");
    }

    #[test]
    fn el_identificador_de_verdad_tiene_ocho_digitos_hexadecimales() {
        let id = identificador().expect("generar un identificador");

        assert_eq!(id.len(), 8, "no son ocho dígitos: {id}");
        assert!(
            id.chars()
                .all(|d| d.is_ascii_hexdigit() && !d.is_uppercase()),
            "no es hexadecimal en minúscula: {id}"
        );
    }

    /// Con valores dados, y no solo con uno sorteado: mirar la longitud de un identificador al azar
    /// deja pasar la falta del relleno con ceros, porque solo se nota con números pequeños — uno de
    /// cada dieciséis.
    #[test]
    fn el_formato_rellena_con_ceros_hasta_ocho() {
        assert_eq!(formatear(0), "00000000");
        assert_eq!(formatear(0xff), "000000ff");
        assert_eq!(formatear(0x0abcdef1), "0abcdef1");
        assert_eq!(formatear(u32::MAX), "ffffffff");
    }

    /// El caso que no falla y hace lo incorrecto: con la variable **definida y vacía** la ruta saldría
    /// relativa, y el directorio del panel acabaría dentro del directorio de trabajo.
    #[test]
    fn una_variable_de_entorno_vacia_no_cuenta_como_ruta() {
        assert_eq!(base_de(Some(OsStr::new(""))), std::env::temp_dir());
    }

    #[test]
    fn sin_directorio_de_ejecucion_se_cae_al_temporal_del_sistema() {
        assert_eq!(base_de(None), std::env::temp_dir());
    }

    #[test]
    fn con_directorio_de_ejecucion_se_usa_ese() {
        assert_eq!(
            base_de(Some(OsStr::new("/run/user/1000"))),
            PathBuf::from("/run/user/1000")
        );
    }

    #[test]
    fn borrar_lo_que_ya_no_esta_no_merece_aviso() {
        let ruta = {
            let panel = PanelDir::create().expect("crear el directorio del panel");
            panel.path().to_path_buf()
        };

        assert!(
            borrar(&ruta).is_none(),
            "avisó de un directorio que ya no estaba"
        );
    }

    #[test]
    fn lo_que_queda_sin_borrar_si_merece_aviso() {
        let panel = PanelDir::create().expect("crear el directorio del panel");
        std::fs::write(panel.path().join("registro.log"), b"del productor")
            .expect("escribir algo ajeno al contrato");

        let aviso = borrar(panel.path());
        std::fs::remove_dir_all(panel.path()).expect("limpiar lo que el test dejó a propósito");

        assert!(
            aviso.is_some(),
            "el directorio sobrevivió con algo dentro y nadie lo dijo"
        );
    }
}
