//! Quién se entera de que un productor ha publicado, y cómo se lo dice a la ventana.
//!
//! **Un solo vigilante para todas las consolas.** Cada uno crea un hilo, y el aviso trae la ruta que
//! cambió, que es cuanto hace falta para saber a quién despierta. Cuatro divisiones cuestan un hilo.
//!
//! **Se vigila el directorio de cada consola, no su `panel.json`.** Leído en la fuente de `notify`: el
//! seguimiento de una ruta que no es directorio se pone sobre el inodo, se elimina cuando ese inodo
//! desaparece y **no se vuelve a registrar nunca**. Como el contrato recomienda publicar escribiendo a
//! un temporal y renombrándolo encima, vigilar el fichero dejaría el panel congelado en silencio la
//! primera vez que alguien siguiera la recomendación. Vigilando el directorio llega además el aviso
//! del primer fichero, que con la otra forma ni siquiera se puede armar: seguir una ruta que todavía
//! no existe falla.
//!
//! **Y el aviso solo significa «mira otra vez».** El crate no agrupa ni deduplica —una escritura
//! produce varios avisos, y cuántos no es determinable— así que lo que se consume tiene que ser
//! idempotente. Una bandera lo es, y con eso el antirrebote sale gratis: dos avisos y veinte dan
//! exactamente el mismo trabajo, que es una lectura en el fotograma siguiente.
//!
//! **Este módulo no sabe de ventanas.** Recibe cómo despertar, igual que el puente, y por la misma
//! razón: lo que se va a reescribir es el framework de interfaz, y despertar a alguien no necesita
//! ninguno.
//!
//! Dos límites conocidos, escritos aquí porque el que los pise no va a leer el diseño:
//! - **Las rutas se fijan al armarlo.** Con las dos divisiones de este hito basta, pero el día en que
//!   se puedan abrir y cerrar divisiones (Fase 2) hay que rearmarlo, o la nueva no se enteraría nunca.
//!   Rearmar cuesta unos 300 µs, y lo que importa no es eso: **entre soltar el viejo y armar el nuevo
//!   no llega nada**, así que una publicación que caiga justo ahí se pierde para siempre.
//! - **La comparación de rutas es textual**, y eso vale porque el respaldo de Linux guarda la ruta tal
//!   cual se le da. El de macOS entrega rutas resueltas (`/var/…` → `/private/var/…`), así que ahí no
//!   casaría con nada, en silencio. Es cosa del hito que compile por primera vez en macOS.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use notify::event::{AccessKind, AccessMode, ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};
// **El *trait* entra sin nombre**, y hace falta: `backend.watch(..)` es suyo, y su nombre choca con el
// tipo de este módulo desde que dejó de llamarse en español.
use notify::Watcher as _;

/// «El directorio de esta consola ha cambiado: mira otra vez.»
///
/// La comparte el hilo del vigilante, que la levanta, con el fotograma, que la recoge. No lleva el
/// evento ni cuántos hubo a propósito: lo único que la ventana necesita saber es si tiene que releer.
///
/// **El orden de memoria es el relajado y no es un descuido**: por esta bandera no se publica ningún
/// dato. Lo que hay que leer viaja por el sistema de ficheros y lo escribe otro proceso, así que no
/// hay nada que adquirir ni que liberar — pedir un orden más fuerte afirmaría una sincronización que
/// no existe.
#[derive(Clone)]
pub struct Flag(Arc<AtomicBool>);

impl Flag {
    /// Una bandera ya levantada, que es como nacen.
    ///
    /// **Y no es `Default`**: un `Default` que vale `true` se lee al revés en el sitio donde se usa.
    /// Nacer levantada cubre un hueco real —el vigilante se arma cuando la ventana ya tiene sus
    /// consolas abiertas, así que un productor rapidísimo puede haber publicado antes de que nadie
    /// mirara su directorio— y cuesta una lectura en el primer fotograma.
    pub fn raised() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    /// La levanta. Que ya lo estuviera no es un caso raro: es el normal.
    pub fn raise(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// La recoge: dice si estaba levantada y la deja bajada.
    ///
    /// **Bajarla antes de leer y no después** es lo que hace que un aviso llegado *durante* la lectura
    /// no se pierda: levanta la bandera otra vez y el fotograma siguiente vuelve. Al revés se leería
    /// el fichero a medio escribir y se bajaría la bandera que avisaba de la otra mitad.
    pub fn take(&self) -> bool {
        self.0.swap(false, Ordering::Relaxed)
    }
}

/// Una consola vigilada: dónde publica y a quién hay que avisar.
struct Watched {
    /// El directorio, que es lo que se sigue de verdad. Se guarda para reconocer el aviso de que ha
    /// dejado de existir, que llega con **su** ruta y no con la del fichero.
    dir: PathBuf,
    /// La ruta exacta del `panel.json`. Se guarda hecha porque se compara en cada aviso, y los avisos
    /// llegan de a varios por escritura.
    file: PathBuf,
    flag: Flag,
}

/// El vigilante de la ventana.
pub struct Watcher {
    /// **Se guarda por su vida, no por su interfaz**: soltarlo termina su hilo y deja de llegar nada.
    /// `None` significa que no hay nada que vigilar o que no se pudo armar; cuál de las dos cosas lo
    /// dice [`Watcher::is_blind`], que no es lo mismo.
    _backend: Option<RecommendedWatcher>,
    /// Si el vigilante ha dejado de ser de fiar. Lo comparte con su hilo, porque **casi todas las
    /// formas de quedarse ciego ocurren después de armarlo** y solo el hilo las ve.
    blind: Arc<AtomicBool>,
}

impl Watcher {
    /// Arma el vigilante sobre el directorio de cada consola.
    ///
    /// `wake` es cómo se saca a la ventana del reposo, y llega como función por lo mismo que al
    /// puente: **este módulo no depende del framework de interfaz**, que es justo lo que se va a
    /// reescribir. Se llama desde el hilo del vigilante, así que no puede entrar en pánico.
    ///
    /// **No poder armarlo no impide abrir la ventana**, igual que no poder crear el directorio de una
    /// consola no impide abrirla: se dice por la salida de errores y se sigue en el modo degradado que
    /// describe [`Watcher::is_blind`].
    pub fn new(wake: impl Fn() + Send + 'static, consoles: &[(&Path, Flag)]) -> Self {
        let blind = Arc::new(AtomicBool::new(false));

        // Sin consolas con directorio no hay nada que seguir, y montar una instancia del vigilante
        // del sistema y su hilo para no mirar nada sería pagar por no hacer nada. **Y no es estar
        // ciego**: no hay ningún panel que se pueda quedar viejo.
        if consoles.is_empty() {
            return Self {
                _backend: None,
                blind,
            };
        }

        let watchlist = consoles
            .iter()
            .map(|(dir, flag)| Watched {
                dir: dir.to_path_buf(),
                file: dir.join(panel_contract::PANEL_FILE),
                flag: flag.clone(),
            })
            .collect();

        let handler = handler(wake, watchlist, Arc::clone(&blind));
        let mut backend = match notify::recommended_watcher(handler) {
            Ok(backend) => backend,
            Err(failure) => {
                eprintln!("no se pudo armar el vigilante de los paneles ({failure})");
                blind.store(true, Ordering::Relaxed);
                return Self {
                    _backend: None,
                    blind,
                };
            }
        };

        for (dir, _) in consoles {
            if let Err(failure) = backend.watch(dir, RecursiveMode::NonRecursive) {
                // Se renuncia al vigilante **entero** y no solo a esta consola. Un vigilante a medias
                // sería una ventana en la que un panel se entera y el de al lado no, sin nada que lo
                // distinga al mirarla; el respaldo de releer es correcto para todas.
                eprintln!("no se pudo vigilar {} ({failure})", dir.display());
                blind.store(true, Ordering::Relaxed);
                return Self {
                    _backend: None,
                    blind,
                };
            }
        }

        Self {
            _backend: Some(backend),
            blind,
        }
    }

    /// Si ya no hay quien avise, y por tanto hay que mirar los ficheros como se hacía antes de que
    /// este vigilante existiera.
    ///
    /// **Casi todas las formas de quedarse ciego ocurren con la ventana ya abierta**, no al armarla:
    /// que el usuario borre desde su propia consola el directorio que se estaba vigilando —el
    /// seguimiento se pierde con el inodo y no se re-registra—, o que el vigilante entregue un fallo,
    /// después del cual su estado es desconocido. Al armar solo falla lo que el sistema no da: el
    /// núcleo limita cuántos vigilantes puede tener un usuario a la vez
    /// (`fs.inotify.max_user_instances`), y ahí ya no hay nada que hacer.
    ///
    /// Sin este respaldo el panel se quedaría **congelado para siempre y en silencio**, enseñando algo
    /// plausible de hace una hora, que es exactamente lo que este hito viene a evitar.
    pub fn is_blind(&self) -> bool {
        self.blind.load(Ordering::Relaxed)
    }
}

/// Lo que corre en el hilo del vigilante cada vez que llega un aviso.
///
/// Está aparte y devuelve la función **para poder invocarla desde una prueba con un aviso puesto a
/// mano**: que el aviso llegue de verdad desde el sistema de ficheros se puede probar, pero es lento y
/// depende del núcleo. Lo que aquí importa —a quién despierta cada aviso— no necesita al núcleo.
fn handler(
    wake: impl Fn() + Send + 'static,
    watchlist: Vec<Watched>,
    blind: Arc<AtomicBool>,
) -> impl FnMut(notify::Result<notify::Event>) + Send + 'static {
    move |notice| {
        let event = match notice {
            Ok(event) => event,
            // Después de un fallo **el estado del vigilante es desconocido**, así que se pasa al
            // respaldo en vez de seguir confiando en unos avisos que quizá ya no lleguen.
            Err(failure) => {
                return go_blind(&blind, &wake, &format!("el vigilante falló ({failure})"));
            }
        };

        // El único aviso que significa «te has perdido cosas», y viene **sin rutas**: no hay a quién
        // repartirlo, así que se mira todo. No deja ciego a nadie —la cola desbordada es de un
        // instante, no una avería— pero el aviso que se perdió detrás no vuelve.
        if event.need_rescan() {
            for watched in &watchlist {
                watched.flag.raise();
            }
            wake();
            return;
        }

        // El directorio vigilado ha dejado de existir, y con él el seguimiento: `notify` lo borra de
        // sus mapas y **no lo vuelve a registrar nunca**. Aquí no llega la ruta del `panel.json`, sino
        // la del directorio, así que sin esto el aviso se descartaría y nadie se enteraría de que a
        // partir de ahora no llega nada.
        if lost_a_watched_dir(&event, &watchlist) {
            return go_blind(
                &blind,
                &wake,
                "el directorio que se vigilaba ha dejado de existir",
            );
        }

        if is_nothing_new(&event.kind) {
            return;
        }

        if raise_matching(&event.paths, &watchlist) {
            // **Pedir el fotograma es la pieza central de este hito, no una precaución.** La ventana
            // repinta solo cuando se lo piden, y la única petición que había estaba atada a que el
            // pseudoterminal produjera: con la ventana en reposo la bandera se levantaría y el
            // fotograma siguiente no llegaría nunca.
            wake();
        }
    }
}

/// Pasa al modo degradado, y lo dice **una sola vez**.
///
/// Una vez es lo correcto y no es tacañería: el bucle de lectura del vigilante entrega el fallo y
/// sigue, así que un fallo que persista sería un chorro sin techo de líneas por la salida de errores.
/// Y se escribe sin poder entrar en pánico, porque esto corre en el hilo del vigilante: con la salida
/// de errores por una tubería sin lector, un `eprintln!` mataría el hilo y no llegaría nada más.
fn go_blind(blind: &AtomicBool, wake: &impl Fn(), reason: &str) {
    if !blind.swap(true, Ordering::Relaxed) {
        let _ = writeln!(
            std::io::stderr(),
            "los paneles se releerán en cada fotograma: {reason}"
        );
        wake();
    }
}

/// Si el aviso dice que un directorio vigilado ya no está donde estaba.
///
/// Las dos formas de perderlo —borrarlo y moverlo— llegan con la ruta del propio directorio.
fn lost_a_watched_dir(event: &notify::Event, watchlist: &[Watched]) -> bool {
    let lost = matches!(
        event.kind,
        EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From))
    );

    lost
        && watchlist
            .iter()
            .any(|watched| event.paths.contains(&watched.dir))
}

/// Los dos avisos que **no** cuentan como una publicación nueva.
///
/// Descartarlos no se nota en lo que se ve; se nota en cuántos fotogramas cuesta publicar, que es lo
/// que este hito paga por enterarse.
fn is_nothing_new(kind: &EventKind) -> bool {
    match kind {
        // **Cerrar tras escribir sí es publicar**: no puede ser nuestro, porque aquí solo se lee, y es
        // la única señal que llega cuando el productor ya ha terminado de escribir.
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => false,
        // **Abrir el fichero para leerlo también produce aviso**, y quien lo abre es esta misma
        // aplicación: sin filtrarlo, cada lectura pide un fotograma que vuelve a leer. No es teórico —
        // con un `panel.json` que no se entiende, y que por eso se reabre en cada intento, la ventana
        // no se dormiría nunca: **medido, 123 lecturas en dos segundos frente a 2**.
        EventKind::Access(_) => true,
        // **El eco de un renombrado.** Un renombre encima —lo que el contrato recomienda— entrega el
        // destino y **acto seguido** un segundo aviso con origen y destino juntos, que trae la misma
        // ruta y no dice nada que el primero no dijera. Leído en la fuente: ese segundo nunca llega sin
        // el primero delante. Descartarlo baja de **1,88 a 1 los fotogramas por publicación** en el
        // camino que el contrato recomienda.
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => true,
        _ => false,
    }
}

/// Levanta la bandera de cada consola cuyo `panel.json` aparece en el aviso, y dice si alguna se
/// levantó.
///
/// Un aviso trae varias rutas y una sola escritura provoca varios avisos. Lo que **no** se mira es qué
/// clase de evento es: crear, escribir y renombrar encima acaban todos en la misma lectura.
fn raise_matching(paths: &[PathBuf], watchlist: &[Watched]) -> bool {
    let mut any_raised = false;

    for watched in watchlist {
        if paths.contains(&watched.file) {
            watched.flag.raise();
            any_raised = true;
        }
    }

    any_raised
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::Event;
    use notify::event::{CreateKind, DataChange, RemoveKind};
    use std::sync::atomic::AtomicUsize;

    /// Cuántas veces se ha despertado a la ventana.
    ///
    /// **Contar es más fuerte que preguntarle al framework si tiene un repintado pendiente**, que es
    /// lo que hacía la primera versión de estas pruebas: aquello obligaba a dejar el contexto en
    /// reposo primero —cuatro pasadas en vacío— y afirmaba sobre la contabilidad interna de egui en
    /// vez de sobre lo que este módulo hace.
    #[derive(Clone, Default)]
    struct Wakeups(Arc<AtomicUsize>);

    impl Wakeups {
        fn count(&self) -> usize {
            self.0.load(Ordering::Relaxed)
        }

        fn wake_fn(&self) -> impl Fn() + Send + 'static {
            let count = Arc::clone(&self.0);
            move || {
                count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Dos consolas con su directorio, y el manejador de verdad montado sobre ellas.
    ///
    /// Se devuelve el manejador y no las piezas sueltas **a propósito**: lo que hay que atar es lo que
    /// corre en el hilo del vigilante. Una prueba sobre el repartidor a solas demostraría que el
    /// repartidor funciona, no que alguien lo llame ni que se despierte a nadie.
    struct Harness {
        handler: Box<dyn FnMut(notify::Result<Event>)>,
        consoles: [Console; 2],
        wakeups: Wakeups,
        blind: Arc<AtomicBool>,
    }

    struct Console {
        dir: PathBuf,
        flag: Flag,
    }

    impl Console {
        fn panel(&self) -> PathBuf {
            self.dir.join(panel_contract::PANEL_FILE)
        }
    }

    fn two_consoles() -> Harness {
        let consoles = [
            Console {
                dir: PathBuf::from("/no-importa/una"),
                flag: Flag::raised(),
            },
            Console {
                dir: PathBuf::from("/no-importa/otra"),
                flag: Flag::raised(),
            },
        ];

        let wakeups = Wakeups::default();
        let blind = Arc::new(AtomicBool::new(false));
        let handler = handler(
            wakeups.wake_fn(),
            consoles
                .iter()
                .map(|console| Watched {
                    dir: console.dir.clone(),
                    file: console.panel(),
                    flag: console.flag.clone(),
                })
                .collect(),
            Arc::clone(&blind),
        );

        // Las banderas nacen levantadas para que el primer fotograma mire; lo que estas pruebas miden
        // es lo que pasa **a partir de ahí**, así que se recogen antes de empezar.
        for console in &consoles {
            console.flag.take();
        }

        Harness {
            handler: Box::new(handler),
            consoles,
            wakeups,
            blind,
        }
    }

    /// El aviso de que se ha escrito algo en una ruta.
    fn written_to(path: PathBuf) -> notify::Result<Event> {
        Ok(Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Any))).add_path(path))
    }

    #[test]
    fn un_aviso_levanta_la_bandera_de_su_consola_y_no_la_de_la_otra() {
        let mut harness = two_consoles();

        (harness.handler)(written_to(harness.consoles[0].panel()));

        assert!(
            harness.consoles[0].flag.take(),
            "la consola cuyo panel cambió no se ha enterado"
        );
        assert!(
            !harness.consoles[1].flag.take(),
            "la consola de al lado releerá su fichero sin que nadie lo haya tocado"
        );
    }

    #[test]
    fn un_aviso_despierta_a_la_ventana() {
        let mut harness = two_consoles();

        (harness.handler)(written_to(harness.consoles[0].panel()));

        assert_eq!(
            harness.wakeups.count(),
            1,
            "se levantó la bandera y nadie despertó a la ventana: el panel no se movería"
        );
    }

    /// **El fichero temporal del propio contrato es el caso a filtrar, no una rareza.** Quien publica
    /// como el contrato recomienda escribe `panel.json.tmp` en ese mismo directorio y lo renombra
    /// encima: sin filtrar, cada publicación provocaría una lectura de más del fichero que todavía no
    /// es el bueno.
    fn a_neighbor_raises_nothing(neighbor: &str) {
        let mut harness = two_consoles();

        (harness.handler)(written_to(harness.consoles[0].dir.join(neighbor)));

        assert!(
            !harness.consoles[0].flag.take(),
            "«{neighbor}» hizo releer el panel de su consola"
        );
        assert!(
            !harness.consoles[1].flag.take(),
            "«{neighbor}» despertó a la de al lado"
        );
        assert_eq!(
            harness.wakeups.count(),
            0,
            "«{neighbor}» sacó a la ventana del reposo para no hacer nada"
        );
    }

    #[test]
    fn el_temporal_de_la_escritura_atomica_no_levanta_nada() {
        a_neighbor_raises_nothing("panel.json.tmp");
    }

    #[test]
    fn un_fichero_cualquiera_del_directorio_no_levanta_nada() {
        a_neighbor_raises_nothing("notas.txt");
    }

    /// **Un aviso trae varias rutas**, y el crate no promete cuáles: si solo se mirara la primera, una
    /// publicación se perdería según en qué orden llegue.
    #[test]
    fn un_aviso_con_varias_rutas_mira_todas() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(Event::new(EventKind::Create(CreateKind::File))
            .add_path(harness.consoles[0].dir.join("panel.json.tmp"))
            .add_path(harness.consoles[0].panel())));

        assert!(
            harness.consoles[0].flag.take(),
            "el panel.json venía en el aviso y nadie lo vio"
        );
    }

    /// **Publicar bien no puede costar dos fotogramas.** Un renombre encima —lo que el contrato
    /// recomienda— entrega el destino y **acto seguido** un segundo aviso con origen y destino juntos:
    /// la misma ruta otra vez, y nada nuevo que decir. Sin descartarlo, cada publicación bien hecha
    /// pide **1,88 fotogramas** en vez de uno, medido en la aplicación de verdad.
    #[test]
    fn el_eco_de_un_renombrado_no_pide_un_segundo_fotograma() {
        let mut harness = two_consoles();
        let (temp, panel) = (
            harness.consoles[0].dir.join("panel.json.tmp"),
            harness.consoles[0].panel(),
        );

        // Los dos avisos del renombre, en el orden en que llegan.
        (harness.handler)(Ok(Event::new(EventKind::Modify(ModifyKind::Name(
            RenameMode::To,
        )))
        .add_path(panel.clone())));
        (harness.handler)(Ok(Event::new(EventKind::Modify(ModifyKind::Name(
            RenameMode::Both,
        )))
        .add_path(temp)
        .add_path(panel)));

        assert!(
            harness.consoles[0].flag.take(),
            "el renombre encima es una publicación y no se ha visto"
        );
        assert_eq!(
            harness.wakeups.count(),
            1,
            "una sola publicación pidió dos fotogramas"
        );
    }

    /// **La aplicación no puede oírse a sí misma leer.** El vigilante del sistema avisa también de que
    /// alguien ha abierto el fichero, y quien lo abre es esta misma aplicación: sin filtrarlo, leer
    /// levanta la bandera, la bandera pide un fotograma y el fotograma vuelve a leer. Con un
    /// `panel.json` que no se entiende —que por eso se reabre cada vez— eso es una ventana que no se
    /// duerme nunca, con su lectura de disco por fotograma y sin nada en pantalla que lo explique.
    #[test]
    fn abrir_el_fichero_para_leerlo_no_levanta_ninguna_bandera() {
        let mut harness = two_consoles();

        for ours in [
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            EventKind::Access(AccessKind::Read),
            EventKind::Access(AccessKind::Close(AccessMode::Read)),
        ] {
            (harness.handler)(Ok(Event::new(ours).add_path(harness.consoles[0].panel())));

            assert!(
                !harness.consoles[0].flag.take(),
                "{ours:?}: leer el fichero se cuenta como una publicación y la lectura se realimenta"
            );
        }

        assert_eq!(harness.wakeups.count(), 0);
    }

    /// Y la otra mitad, que es la que impide arreglar lo de arriba de más: **cerrar tras escribir sí
    /// es publicar**. Es la única señal que llega cuando el productor que no renombra ya ha terminado,
    /// y sin ella el panel se quedaría con lo que se leyó a medio escribir.
    #[test]
    fn cerrar_tras_escribir_si_levanta_la_bandera() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(Event::new(EventKind::Access(AccessKind::Close(
            AccessMode::Write,
        )))
        .add_path(harness.consoles[0].panel())));

        assert!(
            harness.consoles[0].flag.take(),
            "el productor terminó de escribir y el panel no se entera"
        );
    }

    /// **El único aviso que significa «te has perdido cosas» venía sin rutas y se tiraba.** Es lo que
    /// manda el sistema cuando desborda su cola de avisos: detrás de él va un cambio real que ya nunca
    /// llegará, así que hay que volver a mirarlo todo.
    #[test]
    fn el_aviso_de_que_se_han_perdido_avisos_hace_mirar_a_todas() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(
            Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan)
        ));

        assert!(
            harness.consoles[0].flag.take() && harness.consoles[1].flag.take(),
            "se perdieron avisos y ninguna consola vuelve a mirar su fichero"
        );
        assert_eq!(harness.wakeups.count(), 1, "y hay que ir a mirarlos");
        assert!(
            !harness.blind.load(Ordering::Relaxed),
            "una cola desbordada es de un instante, no una avería: no hay que degradarse por ella"
        );
    }

    /// **Que el usuario borre el directorio desde su propia consola es un escenario del proyecto**, no
    /// una hipótesis: tiene un shell dentro. El seguimiento se pierde con el inodo y no se re-registra
    /// nunca, y el aviso llega con la ruta del directorio, no con la del panel — así que sin mirarlo
    /// se descartaría y nadie sabría que a partir de ahí no llega nada.
    #[test]
    fn perder_el_directorio_vigilado_deja_ciego_al_vigilante() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(Event::new(EventKind::Remove(RemoveKind::Folder))
            .add_path(harness.consoles[0].dir.clone())));

        assert!(
            harness.blind.load(Ordering::Relaxed),
            "se perdió el seguimiento y la ventana seguirá creyendo que alguien la avisa"
        );
        assert_eq!(
            harness.wakeups.count(),
            1,
            "hay que despertar para que el respaldo arranque, o no arranca hasta que pase otra cosa"
        );
    }

    /// Borrar el `panel.json` **no** deja ciego a nadie: el directorio sigue vigilado y el productor
    /// puede volver a publicar. Es la mitad que distingue el hallazgo de arriba de tirar la toalla.
    #[test]
    fn borrar_el_panel_no_deja_ciego_pero_si_hace_mirar() {
        let mut harness = two_consoles();

        (harness.handler)(Ok(
            Event::new(EventKind::Remove(RemoveKind::File)).add_path(harness.consoles[0].panel())
        ));

        assert!(
            !harness.blind.load(Ordering::Relaxed),
            "el directorio sigue ahí y el vigilante se ha dado por muerto"
        );
        assert!(
            harness.consoles[0].flag.take(),
            "el panel desapareció y su consola no vuelve a mirar"
        );
    }

    /// Un fallo del vigilante **no puede tumbar su hilo**, y deja su estado en desconocido: a partir de
    /// ahí no se puede seguir confiando en unos avisos que quizá ya no lleguen.
    #[test]
    fn un_fallo_deja_ciego_al_vigilante_sin_entrar_en_panico() {
        let mut harness = two_consoles();

        (harness.handler)(Err(notify::Error::generic("se acabaron los avisos")));

        assert!(
            harness.blind.load(Ordering::Relaxed),
            "el vigilante falló y la ventana seguirá esperando avisos suyos"
        );
    }

    /// Y el fallo que se repite **no puede convertirse en un chorro por la salida de errores**: el
    /// bucle de lectura del vigilante entrega el fallo y sigue, así que uno persistente llegaría en
    /// cada vuelta.
    #[test]
    fn un_fallo_que_se_repite_solo_despierta_una_vez() {
        let mut harness = two_consoles();

        for _ in 0..10 {
            (harness.handler)(Err(notify::Error::generic("y van diez")));
        }

        assert_eq!(
            harness.wakeups.count(),
            1,
            "un fallo que se repite despierta a la ventana en cada vuelta"
        );
    }

    /// **Sin vigilante, la ventana tiene que saberlo.** Es lo que dispara el respaldo de releer en cada
    /// fotograma, y sin eso el hito empeoraría lo que había en vez de mejorarlo.
    #[test]
    fn un_vigilante_armado_no_esta_ciego() {
        let dir = panel_contract::PanelDir::create().expect("crear el directorio");

        let watcher = Watcher::new(|| {}, &[(dir.path(), Flag::raised())]);

        assert!(
            !watcher.is_blind(),
            "el vigilante se armó sobre un directorio que existe y se declara ciego"
        );
    }

    /// Y al revés: un directorio que no existe **no** se puede vigilar, y eso no es una excusa para
    /// dejar de decir que se está ciego.
    ///
    /// La ruta que no existe cuelga de un directorio recién creado **a propósito**: una ruta fija
    /// dentro del temporal del sistema sería una apuesta a que nadie la haya creado nunca en esta
    /// máquina, y el día que alguien lo hiciera esta prueba fallaría para siempre señalando al código.
    #[test]
    fn un_directorio_que_no_existe_deja_al_vigilante_ciego() {
        let dir = panel_contract::PanelDir::create().expect("crear el directorio");
        let missing = dir.path().join("no-existe");

        let watcher = Watcher::new(|| {}, &[(missing.as_path(), Flag::raised())]);

        assert!(
            watcher.is_blind(),
            "no se pudo vigilar nada y la ventana creerá que alguien la avisa"
        );
    }

    /// **Sin consolas con directorio no hay nada que vigilar, y eso no es estar ciego.** Armar el
    /// vigilante del sistema y su hilo para no mirar nada sería pagar por no hacer nada, y declararse
    /// ciego haría que la ventana releyera en cada fotograma unos ficheros que no existen.
    #[test]
    fn sin_consolas_no_se_arma_nada_y_no_se_esta_ciego() {
        let watcher = Watcher::new(|| {}, &[]);

        assert!(
            watcher._backend.is_none(),
            "se armó para no mirar nada"
        );
        assert!(
            !watcher.is_blind(),
            "no hay ningún panel que se quede viejo"
        );
    }
}
