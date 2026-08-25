//! Qué fuente pinta cada carácter, y **con qué presentación**.
//!
//! Dos mitades de la misma pregunta, y por eso viven juntas: **qué cara cubre un carácter** y **con qué
//! presentación hay que pintarlo**. Rasterizar el glifo de color llega después, en este mismo módulo.
//!
//! ## El descubrimiento es PEREZOSO, y eso lo decidió una medida
//!
//! El terminal **no enumera las fuentes de la máquina al abrir la ventana**. Las busca la primera vez que
//! aparece un carácter que ninguna cara registrada cubre. Enumerar y registrar al arrancar costaría entre
//! **23,5 y 31,0 ms** —más de cuatro veces el único tramo del arranque que este proyecto tiene medido— y
//! se pagarían **siempre**; buscar cuando hace falta cuesta entre **4,0 y 5,7 ms** y solo se paga si el
//! carácter aparece de verdad. Los números y la resta entera están en **D24**.

use std::collections::BTreeSet;
use std::sync::Arc;

use eframe::egui;
use fontique::{Collection, CollectionOptions, GenericFamily, SourceCache};
use skrifa::MetadataProvider as _;
use unicode_properties::emoji::{
    EmojiStatus, UnicodeEmoji, is_emoji_presentation_selector, is_text_presentation_selector,
};

/// Cómo hay que pintar un carácter.
///
/// **Existe porque `☁` y `☁️` son el mismo carácter.** El segundo lleva detrás el selector de emoji, y
/// pintar los dos igual es incorrecto en un sentido o en el otro: o se pierde el emoji que el programa
/// pidió, o se pinta un dibujo a color donde había un símbolo de texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentacion {
    /// Por el camino de texto de siempre, con el color que diga el tema.
    Texto,
    /// Como un dibujo con sus propios colores, que la fuente trae dentro.
    Color,
}

/// Qué presentación le toca a un carácter, dado lo que lleve pegado.
///
/// **La regla, y es la de Ghostty:** va a color **si y solo si** le sigue el selector de emoji
/// (`U+FE0F`) **y el carácter admite ese selector**, **o** su presentación por defecto ya es de emoji y
/// **no** le sigue el selector de texto (`U+FE0E`). Todo lo demás va monocromo.
///
/// **Lo de «y el carácter admite ese selector» es la mitad que se olvida.** Unicode solo define
/// secuencias de variación para caracteres con `Emoji=Yes`; detrás de cualquier otro, el selector no
/// significa nada y honrarlo manda a la fuente de color una `a` o un espacio en blanco. Y no es un caso
/// rebuscado: un selector suelto —una cadena cortada por la mitad de un emoji— el motor lo pega a la
/// celda que tenga delante, **aunque esté vacía**.
///
/// `pegados` son los caracteres de anchura cero de la celda, tal y como los da la vista. Solo se mira
/// **el primero**: un selector de variación aplica al carácter que tiene justo delante, así que uno que
/// apareciera más adentro de la secuencia no habla de este carácter.
///
/// **Lo que esta regla NO alcanza, y hay que decirlo:** los modificadores de tono de piel
/// (`U+1F3FB`–`U+1F3FF`) son **anchos**, así que el motor les da **su propia celda** en vez de pegarlos.
/// En `☝🏽` la base `U+261D` llega con **nada pegado**, y como su presentación por defecto es de texto,
/// esta regla la manda a monocromo — mientras el modificador, en la celda de al lado, iría a color.
/// Unicode y Ghostty tratan la secuencia entera como presentación de emoji. **Resolverlo exige mirar la
/// celda siguiente, que es información que esta función no tiene por firma**, y hoy no se ve porque nadie
/// pinta color todavía: es trabajo de la entrega que lo traiga. Lo encontró una revisión.
pub fn presentacion_de(c: char, pegados: &[char]) -> Presentacion {
    match pegados.first().copied() {
        Some(selector) if is_emoji_presentation_selector(selector) && c.is_emoji_char() => {
            Presentacion::Color
        }
        Some(selector) if is_text_presentation_selector(selector) => Presentacion::Texto,
        _ if por_defecto_va_a_color(c) => Presentacion::Color,
        _ => Presentacion::Texto,
    }
}

/// Si la presentación **por defecto** del carácter es de emoji, o sea `Emoji_Presentation=Yes`.
///
/// **Se enumeran las variantes en vez de preguntarlo con un método**, porque la caja de propiedades no
/// trae ninguno: expone `Emoji`, `Emoji_Component` y la combinación de las dos, pero
/// `Emoji_Presentation` solo aparece **dentro** de los nombres de su enumerado. Las cuatro de abajo son
/// exactamente aquellas cuya documentación declara `Emoji_Presentation=YES`; se comprobó leyendo su
/// tabla, no deduciéndolo de los nombres. **Dos que el nombre delata mal**: `EmojiModifierBase` y
/// `EmojiOtherAndEmojiComponent` parecen candidatas y no lo son.
///
/// **Su enumerado es abierto (`#[non_exhaustive]`), así que una categoría que añadan mañana cae en el
/// caso de abajo y se pinta monocromo**, sin que la compilación diga nada. Se deja así a sabiendas: ese
/// carácter saldría **como sale hoy**, que es el peor caso aceptable — inventarle color a una categoría
/// que no conocemos sería lo contrario de la regla de que lo que no está no se inventa.
fn por_defecto_va_a_color(c: char) -> bool {
    matches!(
        c.emoji_status(),
        EmojiStatus::EmojiPresentation
            | EmojiStatus::EmojiPresentationAndModifierBase
            | EmojiStatus::EmojiPresentationAndEmojiComponent
            | EmojiStatus::EmojiPresentationAndModifierAndEmojiComponent
    )
}

// ── El descubrimiento de fuentes ─────────────────────────────────────────────────────────────────

/// El carácter con el que se reconoce el cuadro de reemplazo.
///
/// **`U+0378` no está asignado en Unicode y está FUERA de las zonas de uso privado.** Las dos cosas
/// importan: sin asignar, ninguna fuente lo cubre nunca —así que su dibujo **es** el del cuadro—; y fuera
/// del uso privado, se le pregunta siempre a la misma familia que al carácter con el que se compara. Un
/// referente del uso privado se preguntaría a la familia de iconos en cuanto hubiera una, y estaría
/// comparando peras con manzanas. Lo segundo lo señaló una revisión.
const CUADRO_DE_REEMPLAZO: char = '\u{0378}';

/// Cuánto tiempo se dedica a buscar fuentes en una misma vuelta.
///
/// **Se acota el TIEMPO y no el número de búsquedas, porque el tiempo es lo que se nota.** Lo que cuesta
/// una búsqueda depende de lo que pese la fuente ganadora y de cuántas familias haya que abrir antes: el
/// rango medido va de **94 µs a 18 ms** (**D24**). Un techo de «cuatro búsquedas» acota cuatro cosas que
/// pueden costar 0,4 ms o 72; un techo de tiempo acota lo único que importa.
///
/// Dos milisegundos son un tercio del fotograma (5,78 ms, **D3**) y se pagan **solo cuando hay algo nuevo
/// que buscar**. Lo que no cabe espera al fotograma siguiente, que es para lo que existe la cola.
///
/// **La primera versión ponía el techo en la cola de espera y no en el trabajo**, y con eso un `cat` de un
/// binario costaba **6,0–6,6 ms sostenidos en cada fotograma** —más que el fotograma entero—. Medido por
/// una revisión, en release, y confirmado aquí antes de cambiarlo.
const PRESUPUESTO_POR_VUELTA: std::time::Duration = std::time::Duration::from_millis(2);

/// Cuántas búsquedas seguidas sin encontrar nada hacen que se deje de buscar.
///
/// **Es el freno de un programa roto, y sin él no hay ninguno.** Un `cat` de un binario produce caracteres
/// que **ninguna** fuente cubre, así que ninguna búsqueda registra nada y ningún otro techo llega a
/// dispararse: se seguiría buscando mientras siga saliendo basura. Con esto, tras un puñado de fallos
/// seguidos el terminal deja de intentarlo — y esos caracteres salen como cuadro, que es lo que salían
/// igualmente.
///
/// **Se reinicia en cuanto una búsqueda acierta**, así que una sesión normal nunca lo alcanza.
const FALLOS_SEGUIDOS_MAXIMOS: usize = 12;

/// Cuántos caracteres sin resolver se guardan mientras esperan a la vuelta siguiente.
///
/// **El techo es contra un programa roto, no contra un usuario.** Quien escupe basura binaria produce
/// caracteres distintos a cientos por fotograma, y cada uno cuesta una búsqueda entera. Con el techo, una
/// pantalla de basura gasta lo que gastan cuarenta caracteres y el resto sale como cuadro — que es lo que
/// habría salido igualmente. Sin él, la ventana se queda buscando fuentes para caracteres que nadie va a
/// leer.
const PENDIENTES_MAXIMOS: usize = 40;

/// Cuántas familias del sistema se le llegan a registrar al motor de texto.
///
/// **El techo existe porque no hay forma de preguntar qué cubre ya el motor.** `Fonts::has_glyph` no
/// sirve —ver `Descubridor::resolver`—, así que se busca para todo lo no-ASCII y a veces se registra una
/// cara que no hacía falta: va detrás de las de fábrica y no cambia lo que se ve, pero **cada una engorda
/// el atlas de glifos y alarga la cadena de reserva** que se recorre por carácter.
///
/// **Ocho, y no es un número redondo por gusto:** cubre los casos que este hito persigue —CJK, hangul,
/// iconos, emoji— con margen de sobra, y deja el peor caso acotado. Al llegar al techo se deja de
/// registrar: lo que aparezca después sale como cuadro, que es exactamente lo que salía antes del hito.
const FAMILIAS_MAXIMAS: usize = 8;

/// El nombre de la familia por la que se piden los iconos de las zonas de uso privado.
///
/// **Existe porque egui ordena la cadena de reserva por familia entera y no por rango**, así que la única
/// forma de que una fuente gane en unas zonas y no en otras es darle su propia familia y que quien pinta
/// elija cuál pedir. Ver [`Descubridor::entrega_las_caras`].
pub const FAMILIA_DE_ICONOS: &str = "iconos-del-sistema";

/// Si ese carácter vive en una de las tres zonas que Unicode deja **para uso privado**.
///
/// **Ahí es donde viven los iconos de una `Nerd Font` y los separadores de Powerline**, y por eso hace
/// falta distinguirlas: Unicode no le asigna significado a esas zonas, así que **dos fuentes distintas
/// ponen dibujos distintos en el mismo hueco** y no hay forma de decir cuál es «la correcta» mirando el
/// carácter. Solo la puede decir quien instaló la fuente.
fn es_de_uso_privado(c: char) -> bool {
    matches!(c, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}' | '\u{100000}'..='\u{10FFFD}')
}

/// Lo que se le antepone al nombre de una familia del sistema para nombrarla ante el motor de texto.
///
/// **Sin él, una fuente del sistema que se llame igual que una de fábrica la SUSTITUYE.** Las claves de
/// egui son nombres corrientes —`Hack`, `Ubuntu-Light`, `NotoEmoji-Regular`— y `Hack` está instalada en
/// muchas máquinas de desarrollo: registrarla cambiaría **los bytes** de la de fábrica conservando **su
/// posición** al frente de la cadena, así que el terminal cambiaría de fuente entera de golpe, sin que
/// ningún carácter fuera de uso privado y sin pasar siquiera por el reparto de familias. Lo encontró una
/// revisión, buscando caminos en vez de leyendo el feliz.
const PREFIJO_DEL_SISTEMA: &str = "sistema:";

/// Una cara del sistema, y si va en la familia de iconos.
struct CaraDelSistema {
    /// **Ya prefijada**: es el nombre con el que el motor de texto la conoce, no el que le da el sistema.
    familia: String,
    datos: Arc<egui::FontData>,
    /// **Solo las de iconos van delante**, y es una excepción con nombre a la regla de que las de fábrica
    /// mandan. Ver `Descubridor::entrega_las_caras`.
    delante: bool,
}

/// Busca en la máquina la cara que cubre un carácter, y la registra en el motor de texto.
///
/// **Recuerda también los fallos, y no es un detalle.** Un carácter cubierto se busca una vez y su cara
/// queda registrada, así que el siguiente ya no busca. Pero uno que **ninguna** fuente de la máquina
/// tenga **no deja nada registrado**: sin recordar el fallo volvería a recorrer las candidatas cada vez
/// que apareciera — y ése es justo el carácter que un programa roto escupe en bucle. Sale de **D24**.
#[derive(Default)]
pub struct Descubridor {
    /// Los que ya se intentaron: cubiertos **y** no cubiertos.
    resueltos: std::collections::HashSet<char>,
    /// Los que el pintor ha visto y todavía no se han buscado. Ordenado para que dos corridas con la
    /// misma pantalla hagan el mismo trabajo en el mismo orden.
    pendientes: BTreeSet<char>,
    /// Las caras que ya se le dieron al motor, en el orden en que se le dieron.
    ///
    /// **Hace falta guardarlas** porque registrar en egui no es añadir: es entregarle la configuración
    /// **entera** otra vez. Sin esta lista, cada cara nueva borraría la anterior.
    caras: Vec<CaraDelSistema>,
    /// La colección del sistema, **creada la primera vez que hace falta y no antes**. Es lo que hace
    /// perezoso el descubrimiento: mientras solo aparezcan caracteres que las fuentes de fábrica cubren,
    /// esto nunca se construye y no se paga nada.
    coleccion: Option<Collection>,
    fuentes: SourceCache,
    /// Si enumerar ya falló. Sin esto se reintentaría en cada vuelta en una máquina sin fontconfig.
    sin_fuentes: bool,
    /// Si ya se dijo que se llegó al techo de familias. Decirlo una vez informa; decirlo en cada carácter
    /// llena la salida de error de un programa que no ha hecho nada malo.
    dijo_el_techo: bool,
    /// Cuántas búsquedas seguidas han acabado sin encontrar nada. Ver [`FALLOS_SEGUIDOS_MAXIMOS`].
    fallos_seguidos: usize,
    /// Si el motor de texto **ya conoce** la familia de iconos, o sea si hubo una vuelta posterior a la
    /// que la declaró.
    ///
    /// **Va un paso por detrás de `caras` a propósito, y sin eso la aplicación SE CAE.** `set_fonts`
    /// guarda la configuración y el motor se rehace **en la pasada siguiente**; pero el descubrimiento
    /// corre **antes de pintar, en la misma pasada**, así que entre registrar la cara y pintar la celda no
    /// hay ninguna pasada de por medio. Ofrecer la familia ahí es pedirle a epaint una familia con nombre
    /// que todavía no existe, y eso **es un pánico**, no un aviso: «is not bound to any fonts».
    /// Comprobado ejecutándolo. Lo encontró una revisión sondeando ese camino exacto.
    familia_de_iconos_lista: bool,
    /// Los glifos de color ya rasterizados.
    ///
    /// **Va aquí y no en un tipo aparte porque viaja siempre con el descubridor**: quien pinta un
    /// carácter necesita las dos cosas a la vez —qué familia le toca y, si va a color, qué textura— y
    /// separarlas obligaría a pasar dos referencias mutables por todas las firmas del pintor. El estado
    /// nuevo sí queda en su propia caja: [`Glifos`] se prueba y se vacía sola.
    glifos: Glifos,
    /// Lo que se ha gastado rasterizando glifos de color, y cuántos van.
    ///
    /// **Se cronometra porque corre DENTRO del pintado, al revés que el descubrimiento de caras.**
    /// Registrar una cara rehace el atlas y por eso se hace fuera; rasterizar un emoji no invalida nada,
    /// así que se hace donde hace falta. Pero entonces su coste caería dentro del tramo «encolando el
    /// pintado» —que dejaría de significar lo que su nombre dice— y de ahí, por resta, en la cifra con la
    /// que **D3** juzga a egui. Un número mal etiquetado es peor que ningún número.
    color: CosteDelColor,
}

/// Lo que ha costado el camino del color, para que el informe lo declare.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CosteDelColor {
    /// Cuánto tiempo se ha gastado en total.
    pub gastado: std::time::Duration,
    /// Cuántos glifos se han rasterizado de verdad —los que ya estaban recordados no cuentan.
    pub rasterizados: usize,
    /// Cuántas veces se pidió un glifo de color que la máquina no pinta así.
    pub fallidos: usize,
}

/// Lo que costó una vuelta del descubrimiento, para que el informe lo pueda declarar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Vuelta {
    /// Cuántos caracteres se buscaron.
    pub buscados: usize,
    /// Cuántas caras nuevas se le dieron al motor de texto.
    pub registradas: usize,
}

impl Vuelta {
    /// Si esta vuelta hizo algo. Una que no busca nada no cuesta nada y no merece salir en el informe.
    pub fn hubo_trabajo(self) -> bool {
        self.buscados > 0
    }
}

impl Descubridor {
    /// Qué familia hay que pedirle al motor de texto para pintar ese carácter.
    ///
    /// Los caracteres de las zonas de uso privado van por la familia de iconos, donde manda la fuente que
    /// el usuario instaló; todo lo demás por la monoespaciada de siempre, donde mandan las de fábrica.
    ///
    /// **Y se pregunta aquí, no en una función suelta, porque la familia de iconos NO EXISTE hasta que el
    /// motor se rehace**: epaint entra en pánico —«is not bound to any fonts»— si se le pide una familia
    /// con nombre que nadie ha declarado. Ver `familia_de_iconos_lista` por qué no basta con mirar si hay
    /// caras registradas.
    pub fn familia_de(&self, c: char) -> egui::FontFamily {
        if es_de_uso_privado(c) && self.familia_de_iconos_lista {
            egui::FontFamily::Name(FAMILIA_DE_ICONOS.into())
        } else {
            egui::FontFamily::Monospace
        }
    }

    /// La textura del glifo de color de ese carácter, rasterizándolo si hace falta.
    ///
    /// Devuelve `None` cuando la máquina no tiene ninguna fuente que lo pinte a color: quien llama
    /// **tiene que caer al camino de texto**, que es el Contrato 5 del diseño —lo que no está, no se
    /// inventa—.
    pub fn glifo_a_color(
        &mut self,
        ctx: &egui::Context,
        que: char,
        alto: u32,
        ancho_maximo: u32,
    ) -> Option<(egui::TextureId, egui::Rect, egui::Vec2)> {
        // **Se asegura la colección ANTES de partir `self`**, porque construirla toca dos campos más y
        // no se puede hacer con la mitad prestada.
        if !self.asegura_la_coleccion() {
            return None;
        }
        // **Y aquí se parte en préstamos disjuntos**: la caché de glifos, la colección y el caché de
        // ficheros son campos distintos, así que el compilador deja tener los tres a la vez. Es lo que
        // permite que `glifo_de_color` sea una función libre en vez de un método que pediría el todo.
        let Self {
            glifos,
            coleccion,
            fuentes,
            color,
            ..
        } = self;
        let coleccion = coleccion.as_mut()?;
        // **Solo se cronometra el camino que hace trabajo.** Un glifo ya recordado se devuelve con una
        // consulta a un mapa, y meter esa consulta en el tramo lo diluiría hasta que el número dejara de
        // describir lo que cuesta rasterizar — que es lo que el informe tiene que declarar.
        let ya_estaba = glifos.recuerda(que, alto);
        let empezo = (!ya_estaba).then(std::time::Instant::now);
        let medidas = glifos.textura(ctx, coleccion, fuentes, que, alto, ancho_maximo);
        if let Some(empezo) = empezo {
            color.gastado += empezo.elapsed();
            if medidas.is_some() {
                color.rasterizados += 1;
            } else {
                color.fallidos += 1;
            }
        }
        medidas
    }

    /// Lo que ha costado el camino del color desde la última vez que se preguntó, y se pone a cero.
    ///
    /// **Se vacía al leerlo porque el informe cuenta por fotograma**, igual que el resto de sus tramos:
    /// devolver el acumulado de la sesión daría una media que crece sola y no describe ningún fotograma.
    pub fn coste_del_color(&mut self) -> CosteDelColor {
        std::mem::take(&mut self.color)
    }

    /// Cuántos glifos de color hay recordados. Para que el informe de medida lo pueda declarar.
    pub fn glifos_recordados(&self) -> usize {
        self.glifos.cuantos()
    }

    /// Llena el atlas de glifos de color hasta que no cabe uno más, a esa altura de celda.
    ///
    /// **Existe para que el pintor pueda probar qué se ve cuando el atlas se llena**, que es lo que el
    /// Contrato 5 promete: se cae al camino de texto y el emoji sale igual, en monocromo. El atlas y su
    /// altura son de este módulo, así que desde el pintor no hay forma de dejarlo en ese estado — y
    /// llenarlo con emojis de verdad exige que la máquina tenga ochocientos distintos instalados.
    #[cfg(test)]
    pub fn llena_el_atlas(&mut self, ctx: &egui::Context, alto: u32) {
        self.glifos.alto = alto;
        let lado = LADO_DEL_ATLAS / 4;
        while self
            .glifos
            .pegar(
                ctx,
                egui::ColorImage::filled([lado, lado], egui::Color32::RED),
            )
            .is_some()
        {}
    }

    /// Apunta un carácter que quien pinta se ha encontrado.
    ///
    /// **Es lo único que corre por celda, y por eso es lo único que tiene que ser barato**: un carácter ya
    /// resuelto se descarta con una consulta a un conjunto. Buscar y registrar ocurren fuera del pintado.
    pub fn apunta(&mut self, que: char) {
        // **El ASCII no se mira nunca.** Las fuentes de fábrica de egui lo cubren entero, y es la inmensa
        // mayoría de lo que pasa por una pantalla de terminal: preguntarlo sería pagar una consulta por
        // celda para obtener siempre lo mismo.
        if que.is_ascii() || self.resueltos.contains(&que) || self.sin_fuentes {
            return;
        }
        if self.pendientes.len() < PENDIENTES_MAXIMOS {
            self.pendientes.insert(que);
        }
    }

    /// Apunta todos los caracteres de un texto.
    ///
    /// **Existe porque no solo la rejilla de la consola pinta texto.** El panel de estado enseña lo que
    /// publica su productor, que es texto libre y puede venir en cualquier idioma: sin esto, un resumen
    /// en japonés salía en cuadros **para siempre**, porque nada disparaba la búsqueda. Y eso en el hito
    /// cuyo objeto es justamente que se vea. Lo encontró una revisión.
    pub fn apunta_texto(&mut self, texto: &str) {
        for que in texto.chars() {
            self.apunta(que);
        }
    }

    /// Busca las caras de lo apuntado y se las da al motor de texto. Devuelve lo que hizo.
    ///
    /// **Corre FUERA del pintado**, y por dos razones: preguntarle al motor de texto si cubre un carácter
    /// necesita acceso exclusivo al contexto —pedirlo a mitad de una pasada de interfaz es reentrar en lo
    /// que ya está prestado—, y registrar una cara **rehace el atlas de glifos entero**, que son
    /// milisegundos y no microsegundos.
    pub fn resolver(&mut self, ctx: &egui::Context) -> Vuelta {
        // **Lo primero de todo: la familia declarada en la vuelta ANTERIOR ya la conoce el motor.** Entre
        // aquella vuelta y ésta ha pasado un fotograma entero, que es lo que `set_fonts` necesita para
        // surtir efecto. Va aquí y no al final para que no se adelante a sí misma.
        self.familia_de_iconos_lista = self.caras.iter().any(|cara| cara.delante);

        if self.pendientes.is_empty() {
            return Vuelta::default();
        }
        let pendientes = std::mem::take(&mut self.pendientes);

        // **Lo que ya se ve bien se descarta ANTES de tocar el sistema, y eso cuesta 165 ns.**
        //
        // `Fonts::has_glyph` no vale para esto: promete «¿podemos mostrar este glifo?» y lo que hace es
        // `resolve_face(c) != replacement_face_key`, o sea **devuelve `false` para la `'a'`** —comprobado
        // ejecutándolo contra epaint 0.36.1—. La primera versión de este módulo, al quedarse sin oráculo,
        // buscaba para **todo** lo no-ASCII: un acento español costaba 18 ms y registraba una cara que no
        // cambiaba nada, gastando además uno de los pocos huecos de familias. Lo midió una revisión.
        //
        // **El oráculo que sí funciona es el que usan las pruebas**: maquetar el carácter y comparar su
        // dibujo con el del cuadro de reemplazo. Dice la verdad y cuesta **cuatro órdenes de magnitud
        // menos** que recorrer las familias de la máquina.
        let a_buscar = ctx.fonts_mut(|fuentes| {
            let cuadro = dibujo_de(fuentes, CUADRO_DE_REEMPLAZO, egui::FontFamily::Monospace);
            let mut a_buscar = Vec::new();
            for que in pendientes {
                if dibujo_de(fuentes, que, self.familia_de(que)) != cuadro {
                    // Ya se ve: no hay nada que buscar, y no se vuelve a mirar.
                    self.resueltos.insert(que);
                } else {
                    a_buscar.push(que);
                }
            }
            a_buscar
        });

        let mut vuelta = Vuelta::default();
        // **Se gasta presupuesto, no un número de vueltas.** Y se comprueba ANTES de cada búsqueda: una
        // que empiece dentro del presupuesto puede acabar fuera —el peor caso son 18 ms— y eso es
        // aceptable; empezar la siguiente ya fuera, no.
        let empezo = std::time::Instant::now();
        for que in a_buscar {
            let sin_presupuesto = empezo.elapsed() >= PRESUPUESTO_POR_VUELTA;
            let se_rindio = self.fallos_seguidos >= FALLOS_SEGUIDOS_MAXIMOS;
            if se_rindio {
                // La máquina ya ha dicho doce veces seguidas que no tiene con qué. Se dan por resueltos
                // sin buscar: seguir preguntando cuesta hasta 18 ms cada uno para el mismo «no».
                self.resueltos.insert(que);
                continue;
            }
            if sin_presupuesto {
                // **Se quedan SIN marcar como resueltos**, así que el pintor los vuelve a apuntar y se
                // buscan en el fotograma siguiente. Marcarlos aquí los condenaría a salir como cuadro
                // para siempre solo por haber llegado tarde a esta vuelta.
                continue;
            }
            self.resueltos.insert(que);
            vuelta.buscados += 1;
            if self.registra_quien_cubre(que) {
                vuelta.registradas += 1;
                self.fallos_seguidos = 0;
            } else {
                self.fallos_seguidos += 1;
            }
        }
        // **El caché de ficheros se vacía en cuanto la vuelta termina, y no es opcional.** Guarda un
        // `Blob` vivo por **cada** fichero de fuente que la búsqueda llegó a abrir, y una búsqueda fallida
        // los abre **todos** los de la máquina: sin podar, una sesión que vea un carácter raro se queda
        // con cientos de megas residentes para siempre. Lo que de verdad hace falta ya está copiado en
        // `caras`. Lo midió una revisión.
        self.fuentes.prune(0, true);

        if vuelta.registradas > 0 {
            self.entrega_las_caras(ctx);
            // **Sin esto el glifo descubierto puede quedarse en cuadro indefinidamente.** Hacen falta dos
            // pasadas más —una para que `set_fonts` surta efecto y otra para repintar— y nada las provoca:
            // los repintados se piden por salida pendiente o por entrada. `echo 日本語` y parar dejaba el
            // kanji en cuadro **hasta que alguien tocara algo**. Lo encontró una revisión.
            ctx.request_repaint();
        }
        vuelta
    }

    /// Busca quién cubre ese carácter y añade su cara a la lista. `true` si añadió una.
    fn registra_quien_cubre(&mut self, que: char) -> bool {
        if self.caras.len() >= FAMILIAS_MAXIMAS {
            // **Se dice, y una sola vez.** Un techo que se traga lo que descarta es indistinguible de un
            // fallo: quien vea un cuadro después de instalar la fuente adecuada no tendría forma de saber
            // que el terminal encontró la fuente y decidió no usarla. Es la misma regla con la que el
            // resto del proyecto declara sus límites en vez de callarlos.
            if !self.dijo_el_techo {
                eprintln!(
                    "agent-terminal: ya hay {FAMILIAS_MAXIMAS} fuentes del sistema registradas, que es el \
                     techo. Los caracteres que las de fábrica no cubran seguirán saliendo como cuadros."
                );
                self.dijo_el_techo = true;
            }
            return false;
        }
        let Some(cara) = self.busca(que) else {
            // **Y aquí acaba el intento: lo que no está, no se inventa.** El carácter se pinta con lo que
            // dé el motor —su cuadro— y no se intenta nada más. Ya está en `resueltos`, así que no se
            // vuelve a buscar.
            return false;
        };
        let nombre = format!("{PREFIJO_DEL_SISTEMA}{}", cara.0);
        if self.caras.iter().any(|suya| suya.familia == nombre) {
            // Otra cara de la misma familia ya está registrada: cubre esto también.
            return false;
        }
        self.caras.push(CaraDelSistema {
            familia: format!("{PREFIJO_DEL_SISTEMA}{}", cara.0),
            datos: cara.1,
            delante: es_de_uso_privado(que),
        });
        true
    }

    /// La primera cara de la máquina que cubre ese carácter, con el nombre de su familia.
    fn busca(&mut self, que: char) -> Option<(String, Arc<egui::FontData>)> {
        // **Se comprueba aquí ADEMÁS de en `apunta`, y no es redundante:** los caracteres que ya estaban
        // en la cola cuando enumerar falló llegan igual hasta aquí, y sin esta línea cada uno volvería a
        // construir la colección entera para volver a encontrarla vacía.
        if !self.asegura_la_coleccion() {
            return None;
        }
        let coleccion = self.coleccion.as_mut()?;
        // **Los dos campos se prestan por separado a propósito**: pedirlos a través de un método que
        // devuelva la colección prestaría `self` entero y el segundo préstamo no compilaría.
        let fuentes = &mut self.fuentes;
        // **Dos vías, y en este orden.** Las familias monoespaciadas que el sistema declara son pocas y
        // son las adecuadas para un terminal, así que se preguntan primero; recorrerlas todas es la red
        // de debajo y el peor caso medido —de 94 µs a 18,2 ms según lo que pese la ganadora, no según
        // dónde caiga en el orden (**D24**)—.
        let genericas: Vec<_> = coleccion
            .generic_families(GenericFamily::Monospace)
            .collect();
        let mut todas: Vec<String> = coleccion.family_names().map(str::to_owned).collect();
        // **Por nombre, para que la respuesta no cambie entre arranques.** El orden en que el sistema las
        // entrega no es estable: dos corridas del banco dieron familias distintas para el mismo carácter
        // hasta que se ordenó.
        todas.sort();
        let del_resto: Vec<_> = todas
            .iter()
            .filter_map(|nombre| coleccion.family_id(nombre))
            .collect();

        for ids in [&genericas, &del_resto] {
            if let Some(cara) = primera_que_cubre(coleccion, fuentes, ids, que) {
                return Some(cara);
            }
        }
        None
    }

    /// Construye la colección del sistema si todavía no existe. Devuelve si hay con qué buscar.
    ///
    /// **Sale de `busca` para que las dos búsquedas —la de cobertura y la de color— compartan una sola
    /// colección.** Enumerar cuesta entre 15,9 y 31,0 ms (**D24**): hacerlo dos veces por tener dos
    /// funciones sería pagar el tramo más caro del módulo por duplicado.
    fn asegura_la_coleccion(&mut self) -> bool {
        if self.sin_fuentes {
            return false;
        }
        if self.coleccion.is_some() {
            return true;
        }
        // **Se construye aquí y no en el arranque: eso es lo que hace perezoso el descubrimiento.**
        // Mientras solo aparezcan caracteres que las fuentes de fábrica cubren, esta línea no corre nunca.
        let mut coleccion = Collection::new(CollectionOptions {
            system_fonts: true,
            ..Default::default()
        });
        // **Una máquina sin fontconfig devuelve una colección vacía en vez de caerse**, y ése es el caso
        // que **AC9** obliga a aguantar. Se dice por la salida de error y se deja de intentar: sin esto se
        // volvería a enumerar en cada vuelta, gastando el coste entero para nada.
        if coleccion.family_names().next().is_none() {
            eprintln!(
                "agent-terminal: no se encontró ninguna fuente del sistema. Los caracteres que las \
                 fuentes de fábrica no cubran seguirán saliendo como cuadros."
            );
            self.sin_fuentes = true;
            return false;
        }
        self.coleccion = Some(coleccion);
        true
    }

    /// Le da al motor de texto **todas** las caras descubiertas hasta ahora.
    ///
    /// **Detrás de las de fábrica, y ése es el Contrato 2 del diseño**: la cadena de reserva se recorre en
    /// orden, así que una cara del sistema puesta delante le robaría glifos a la fuente del terminal —y el
    /// terminal dejaría de verse como se ve hoy en lo que hoy ya funciona—.
    ///
    /// **Y las de iconos van a una FAMILIA APARTE, no delante de las de fábrica.** Esa fue la primera
    /// versión y estaba mal: egui ordena la cadena de reserva **por familia entera**, no por rango de
    /// caracteres, así que poner delante una fuente de iconos —que casi siempre es una `Nerd Font`, y esas
    /// **traen el latino completo**— le quita las letras a la fuente del terminal. Medido antes de
    /// cambiarlo: descubrir un solo icono **le cambiaba el dibujo a tres de cada cinco letras**, que es
    /// exactamente lo que el Contrato 2 existe para impedir.
    ///
    /// Con familia propia, quien pinta pide [`FAMILIA_DE_ICONOS`] **solo para los caracteres de uso
    /// privado** y la monoespaciada para todo lo demás. El orden por carácter que egui no da, lo da el
    /// pintor, que ya recorre celda a celda.
    fn entrega_las_caras(&self, ctx: &egui::Context) {
        let mut definiciones = egui::FontDefinitions::default();
        let de_fabrica = definiciones
            .families
            .get(&egui::FontFamily::Monospace)
            .cloned()
            .unwrap_or_default();

        definiciones
            .families
            .insert(egui::FontFamily::Monospace, self.monoespaciada(&de_fabrica));
        definiciones.families.insert(
            egui::FontFamily::Name(FAMILIA_DE_ICONOS.into()),
            self.de_iconos(&de_fabrica),
        );
        for cara in &self.caras {
            definiciones
                .font_data
                .insert(cara.familia.clone(), Arc::clone(&cara.datos));
        }
        ctx.set_fonts(definiciones);
    }

    /// La familia monoespaciada: **las de fábrica primero y las del sistema detrás**, sin excepciones.
    ///
    /// **Está aparte para poder afirmar sobre ella**, y no es un rodeo: esta lista **es** lo que egui
    /// consume. Afirmar en su lugar sobre el dibujo que sale no distingue los casos — registrar una cara
    /// rehace el atlas y **todos** los glifos cambian de sitio, así que la comparación pasa igual con el
    /// defecto puesto y quitado. Lo destapó mutarla.
    fn monoespaciada(&self, de_fabrica: &[String]) -> Vec<String> {
        de_fabrica
            .iter()
            .cloned()
            .chain(self.caras.iter().map(|c| c.familia.clone()))
            .collect()
    }

    /// La familia de iconos: **las del sistema primero, y las de fábrica detrás como red**.
    ///
    /// Solo entran las caras que se descubrieron **buscando un carácter de uso privado**: una encontrada
    /// por un kanji no se cuela aquí aunque además traiga iconos.
    fn de_iconos(&self, de_fabrica: &[String]) -> Vec<String> {
        self.caras
            .iter()
            .filter(|c| c.delante)
            .map(|c| c.familia.clone())
            .chain(de_fabrica.iter().cloned())
            .collect()
    }
}

/// Lo que mide el glifo de ese carácter en el atlas, o `None` si no llega a haber glifo.
///
/// **Es el oráculo de «¿esto ya se ve?»**: comparado con el del cuadro de reemplazo dice la verdad, al
/// contrario que `Fonts::has_glyph`. Cuesta **165 ns**, medido en release.
fn dibujo_de(
    fuentes: &mut egui::epaint::text::FontsView<'_>,
    que: char,
    familia: egui::FontFamily,
) -> Option<(u16, u16)> {
    let galley = fuentes.layout_no_wrap(
        que.to_string(),
        egui::FontId::new(CUERPO_PARA_PREGUNTAR, familia),
        egui::Color32::WHITE,
    );
    let uv = galley.rows.first()?.glyphs.first()?.uv_rect;
    Some((uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]))
}

/// El cuerpo con el que se le pregunta al motor si un carácter se ve.
///
/// **Da igual cuál sea mientras sea el mismo para los dos lados de la comparación**: lo que se compara es
/// si dos caracteres pintan el mismo dibujo, no cuánto miden.
const CUERPO_PARA_PREGUNTAR: f32 = 14.0;

/// La primera de esas familias que trae el glifo de ese carácter.
fn primera_que_cubre(
    coleccion: &mut Collection,
    fuentes: &mut SourceCache,
    ids: &[fontique::FamilyId],
    que: char,
) -> Option<(String, Arc<egui::FontData>)> {
    for &id in ids {
        // **Se copian el nombre y las caras antes de seguir**: la familia presta de la colección, y la
        // colección hace falta en la vuelta siguiente.
        let Some((nombre, caras)) = coleccion
            .family(id)
            .map(|familia| (familia.name().to_owned(), familia.fonts().to_vec()))
        else {
            continue;
        };
        // **Se prefiere la cara Regular, no la primera que enumere el sistema.** Medido por una revisión:
        // `日` salía de la cara **índice 5** de su familia, elegida porque le tocó ir primero. En una
        // familia cuya Negrita o Fina se enumere antes, **todo el CJK del terminal saldría en negrita**,
        // permanentemente y sin forma de corregirlo. El desempate por índice deja el orden estable, así
        // que la respuesta tampoco cambia entre arranques.
        let mut caras = caras;
        caras.sort_by_key(|cara| {
            (
                cara.style() != fontique::FontStyle::Normal,
                (cara.weight().value() - fontique::FontWeight::NORMAL.value()).abs() as i32,
                ((cara.width().ratio() - fontique::FontWidth::NORMAL.ratio()).abs() * 100.0) as i32,
                cara.index(),
            )
        });
        for cara in caras {
            let Some(blob) = fuentes.get(cara.source()) else {
                continue;
            };
            let cubre = cara
                .charmap_index()
                .charmap(blob.data())
                .is_some_and(|mapa| mapa.map(que as u32).is_some_and(|glifo| glifo != 0));
            if cubre {
                // **Y se comprueba que el motor de texto va a poder abrirla, ANTES de dársela.** El
                // único modo de fallo que epaint tiene para unos bytes que no entiende es
                // `panic!("Error parsing … TTF/OTF font file")`, y ocurre **dentro de su pasada**: un
                // fichero cuyo `cmap` se lee pero cuyo directorio de tablas no —una actualización de
                // paquete a medias, un fichero truncado, unos desplazamientos viejos de fontconfig—
                // **mataría el proceso** en el fotograma siguiente. Y son ficheros de fuera: no se
                // controla lo que traen. Lo señaló una revisión.
                //
                // Se valida con la misma caja que epaint usa por debajo, así que lo que aquí se abra,
                // allí se abre.
                if skrifa::FontRef::from_index(blob.data(), cara.index()).is_err() {
                    continue;
                }
                // **Con SU índice de cara, no con el cero.** Un fichero de fuente puede llevar varias
                // caras dentro —una colección `.ttc`— y `FontData::from_owned` se queda con la primera:
                // registrar el fichero sin decir cuál se buscaba deja al motor con una cara que **no**
                // cubre el carácter, y el terminal sigue pintando el cuadro después de haber pagado la
                // búsqueda entera. Comprobado: es lo que pasaba con los iconos del Área de Uso Privado.
                let mut datos = egui::FontData::from_owned(blob.data().to_vec());
                datos.index = cara.index();
                return Some((nombre, Arc::new(datos)));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// El glifo de color
// ---------------------------------------------------------------------------------------------

/// Lo que mide de lado el atlas de glifos de color, en píxeles.
///
/// **El techo de este hito es una textura de tamaño FIJO, no un contador**, y eso es mejor de lo que el
/// diseño pedía: la memoria que la tarjeta gráfica dedica a los emojis es **1 MiB y no cambia**, aparezcan
/// uno o mil. A la altura de celda de esta máquina caben unos ochocientos glifos —17 px de alto, 18 de
/// ancho—, muchos más de los que una persona ve en una sesión; cuando se llena se sigue pintando, pero en
/// monocromo por el camino de texto de siempre.
///
/// Es el mismo criterio con el que **D8** acotó la cola de salida y **D21** el historial: una entrada que
/// llega de fuera y no la controla nadie de dentro lleva techo, y el techo se dice.
const LADO_DEL_ATLAS: usize = 512;

/// El cuerpo con el que se le pide el mapa de bits a la fuente.
///
/// **No es la altura de la celda a propósito.** `Noto Color Emoji` trae **una sola tirada** de 136×128
/// píxeles —comprobado ejecutándolo, no leyendo la especificación— así que pedirle un tamaño u otro
/// devuelve exactamente la misma imagen. Pedir el mayor de los razonables es lo único que cambia algo en
/// una fuente que sí traiga varias: se coge la más grande y se reduce, que pierde menos que ampliar una
/// pequeña.
const CUERPO_PARA_RASTERIZAR: f32 = 128.0;

/// Dónde está un glifo dentro del atlas y cuánto mide.
#[derive(Debug, Clone, Copy)]
struct EnElAtlas {
    /// El trozo del atlas que ocupa, en coordenadas de 0 a 1 — que es como se pide al pintar.
    uv: egui::Rect,
    /// Lo que mide de verdad, en píxeles. Quien pinta lo necesita para centrarlo sin deformarlo.
    medidas: egui::Vec2,
}

/// Los glifos de color ya rasterizados, **todos dentro de una sola textura**.
///
/// **Una textura y no una por glifo, y la diferencia se mide.** Con una textura por emoji, el motor no
/// puede juntar dos en el mismo lote de dibujo —lo dice su teselador: mezclar identificadores de textura
/// en la misma malla es un error— así que cada emoji de la pantalla parte el lote. Medido con las dos
/// consolas volcando emojis contra su control en ASCII: **el resto del fotograma pasaba de 275 µs a
/// 743 µs**, es decir la cifra con
/// la que **D3** juzga al framework casi triplicada, y por trabajo que no es del framework. Con atlas,
/// todos los emojis de la pantalla se dibujan juntos. Es lo que hacen los terminales que son el listón.
///
/// **La altura es un campo y no parte de la clave, y eso es una decisión con consecuencia.** Toda la
/// ventana comparte una sola métrica —se mide una vez por pasada, con el mismo cuerpo para todas las
/// divisiones—, así que dos alturas distintas no coexisten nunca: solo se suceden, cuando cambia el zoom
/// del escritorio. Guardar la altura en la clave dejaría las entradas viejas ocupando sitio para siempre;
/// tenerla aquí permite **vaciar entero** el atlas en cuanto cambia, que es lo que el contrato del diseño
/// exige y lo que impide pintar un emoji del tamaño anterior en una celda del nuevo.
#[derive(Default)]
struct Glifos {
    /// La textura donde viven todos. **No existe hasta que hace falta el primero**: una sesión sin
    /// emojis no le pide ni un byte a la tarjeta gráfica.
    atlas: Option<egui::TextureHandle>,
    /// Lo recordado, incluidos los fallos: un `None` es «éste no lo pinta a color nadie de esta máquina».
    ///
    /// **Recordar el fallo no es un detalle.** Sin él, un carácter que ninguna fuente de color cubre
    /// recorrería las familias de emoji **en cada fotograma** — y es justo el carácter que un programa
    /// roto escupe en bucle. Es la misma razón por la que [`Descubridor`] recuerda los suyos.
    guardados: std::collections::HashMap<char, Option<EnElAtlas>>,
    /// Dónde empieza el hueco libre de la fila que se está llenando.
    siguiente: [usize; 2],
    /// Lo que mide de alto la fila que se está llenando. Al saltar de fila se avanza esto.
    alto_de_la_fila: usize,
    /// La altura de celda a la que están rasterizados los de arriba, en píxeles físicos.
    alto: u32,
    /// Si ya se dijo que el atlas se llenó.
    dijo_el_techo: bool,
}

impl Glifos {
    /// Dónde está el glifo de color de ese carácter dentro del atlas, rasterizándolo si hace falta.
    ///
    /// Devuelve `None` cuando la máquina no tiene ninguna fuente que lo pinte a color, o cuando el atlas
    /// se llenó: quien llama **tiene que caer al camino de texto**, que es el Contrato 5 del diseño —lo
    /// que no está, no se inventa—.
    fn textura(
        &mut self,
        ctx: &egui::Context,
        coleccion: &mut Collection,
        fuentes: &mut SourceCache,
        que: char,
        alto: u32,
        ancho_maximo: u32,
    ) -> Option<(egui::TextureId, egui::Rect, egui::Vec2)> {
        if alto == 0 || ancho_maximo == 0 {
            return None;
        }
        // **Cambió la altura de la celda: lo recordado ya no vale.** Sin esto, subir el zoom del
        // escritorio dejaría los emojis del tamaño anterior pintados en celdas del nuevo — y como el
        // atlas nunca se vacía, para el resto de la vida del proceso.
        if self.alto != alto {
            self.guardados.clear();
            self.siguiente = [0, 0];
            self.alto_de_la_fila = 0;
            self.alto = alto;
            self.dijo_el_techo = false;
        }
        let sitio = if let Some(guardado) = self.guardados.get(&que) {
            *guardado
        } else {
            let sitio = glifo_de_color(coleccion, fuentes, que, alto, ancho_maximo)
                .and_then(|imagen| self.pegar(ctx, imagen));
            self.guardados.insert(que, sitio);
            sitio
        };
        // **El atlas existe siempre que haya un sitio guardado**: se crea al pegar el primer glifo, y lo
        // que se guarda es justamente el resultado de haberlo pegado.
        let sitio = sitio?;
        Some((self.atlas.as_ref()?.id(), sitio.uv, sitio.medidas))
    }

    /// Mete una imagen en el primer hueco libre del atlas. `None` si ya no cabe.
    fn pegar(&mut self, ctx: &egui::Context, imagen: egui::ColorImage) -> Option<EnElAtlas> {
        let [ancho, alto] = [imagen.width(), imagen.height()];
        if ancho > LADO_DEL_ATLAS || alto > LADO_DEL_ATLAS {
            return None;
        }
        // Colocación por filas: se llena la fila de izquierda a derecha y se salta a la siguiente cuando
        // no cabe. **Deja huecos y da igual**: los glifos de una sesión miden todos casi lo mismo —los
        // rasteriza la misma altura de celda— así que las filas salen parejas.
        if self.siguiente[0] + ancho > LADO_DEL_ATLAS {
            self.siguiente = [0, self.siguiente[1] + self.alto_de_la_fila];
            self.alto_de_la_fila = 0;
        }
        if self.siguiente[1] + alto > LADO_DEL_ATLAS {
            // **Se dice, y una sola vez.** Un techo callado es indistinguible de un fallo: quien vea sus
            // emojis en monocromo de golpe no tendría forma de saber que el terminal sabe pintarlos y se
            // ha quedado sin sitio donde guardarlos.
            if !self.dijo_el_techo {
                eprintln!(
                    "agent-terminal: el atlas de glifos de color ({LADO_DEL_ATLAS}x{LADO_DEL_ATLAS} \
                     píxeles) se ha llenado. Los emojis que no estén ya en él seguirán saliendo en \
                     monocromo."
                );
                self.dijo_el_techo = true;
            }
            return None;
        }
        let sitio = self.siguiente;
        let atlas = self.atlas.get_or_insert_with(|| {
            ctx.load_texture(
                "glifos-de-color",
                egui::ColorImage::filled(
                    [LADO_DEL_ATLAS, LADO_DEL_ATLAS],
                    egui::Color32::TRANSPARENT,
                ),
                // **Lineal y no `NEAREST`**: el mapa de bits viene a 136×128 y la celda mide una fracción
                // de eso, así que la reducción la hace el remuestreo de la CPU; lo que la tarjeta hace
                // aquí es solo el ajuste final a píxeles físicos. Con `NEAREST` ese ajuste se ve como
                // dientes de sierra en el borde del dibujo.
                egui::TextureOptions::LINEAR,
            )
        });
        // **Solo el rectángulo del glifo nuevo, no el atlas entero.** Re-subir el megabyte completo por
        // cada emoji descubierto sería medio giga de tráfico a la tarjeta en una sesión que llegue al
        // techo, para cambiar unos cientos de bytes.
        atlas.set_partial(sitio, imagen, egui::TextureOptions::LINEAR);
        self.siguiente[0] = sitio[0] + ancho;
        self.alto_de_la_fila = self.alto_de_la_fila.max(alto);
        let lado = LADO_DEL_ATLAS as f32;
        Some(EnElAtlas {
            uv: egui::Rect::from_min_max(
                egui::pos2(sitio[0] as f32 / lado, sitio[1] as f32 / lado),
                egui::pos2(
                    (sitio[0] + ancho) as f32 / lado,
                    (sitio[1] + alto) as f32 / lado,
                ),
            ),
            medidas: egui::vec2(ancho as f32, alto as f32),
        })
    }

    /// Si ese carácter ya está recordado —incluido el recuerdo de que no se pinta a color.
    ///
    /// **Mira también la altura**, porque un cambio de altura vacía el atlas entero: sin eso, el primer
    /// glifo tras un cambio de zoom se rasterizaría **sin** que su coste entrara en el tramo.
    fn recuerda(&self, que: char, alto: u32) -> bool {
        self.alto == alto && self.guardados.contains_key(&que)
    }

    /// Cuántos glifos hay recordados.
    fn cuantos(&self) -> usize {
        self.guardados.len()
    }
}

/// La imagen a color de ese carácter, ya reescalada, si alguna fuente de la máquina lo pinta así.
///
/// **Es una función libre y no un método, y eso resuelve un problema real.** Necesita la colección del
/// sistema y el caché de ficheros, que son **dos campos** del descubridor; como método pediría prestado el
/// descubridor entero, y quien la llama —la caché de glifos, que vive **dentro** de él— ya tiene prestada
/// su parte. La primera versión lo salvaba sacando la caché con `mem::take` y devolviéndola después, lo
/// que dejaba una trampa: una segunda entrada por ese camino habría visto una caché vacía y la primera le
/// habría sobrescrito al volver lo que la segunda guardara. Nadie reentra hoy, y nada lo impedía.
///
/// **Devuelve la imagen y no la cara**: quien llama guarda texturas, no fuentes. Prestarle la cara abierta
/// le obligaría a cargar con los veinte megas del fichero mientras la usa, y a hablar el modelo de fuente
/// de una caja de terceros para nada.
fn glifo_de_color(
    coleccion: &mut Collection,
    fuentes: &mut SourceCache,
    que: char,
    alto: u32,
    ancho_maximo: u32,
) -> Option<egui::ColorImage> {
    // **Solo las familias que el sistema declara de emoji, y ahí acaba la búsqueda.** No se cae a
    // recorrerlas todas como hace `busca`: una familia que no es de emoji y aun así cubre un emoji lo
    // pinta **en monocromo** —en esta máquina `DejaVu Sans` se lleva el 😀 teniendo `Noto Color Emoji`
    // instalada—, así que la red de debajo daría exactamente lo que esta función existe para evitar.
    let genericas: Vec<_> = coleccion.generic_families(GenericFamily::Emoji).collect();
    for id in genericas {
        let Some(caras) = coleccion.family(id).map(|familia| familia.fonts().to_vec()) else {
            continue;
        };
        for cara in caras {
            let Some(blob) = fuentes.get(cara.source()) else {
                continue;
            };
            let Ok(fuente) = skrifa::FontRef::from_index(blob.data(), cara.index()) else {
                continue;
            };
            let Some(glifo) = fuente.charmap().map(que) else {
                continue;
            };
            // **Que la cubra no basta: tiene que traerla a color.** Las familias que el sistema
            // declara de emoji en esta máquina son decenas —casi todas las `Noto` de símbolos— y la
            // mayoría son monocromas. Sin esta comprobación ganaría la primera que enumere el
            // sistema, que es el defecto que este proyecto tiene documentado desde **D24** y el que
            // Alacritty arrastra.
            if let Some(imagen) = rasteriza(&fuente, glifo, alto, ancho_maximo) {
                return Some(imagen);
            }
        }
    }
    None
}

/// Los píxeles de un mapa de bits, en RGBA y con **el alfa ya premultiplicado**. `None` si lo que trae
/// no es un dibujo a color.
///
/// **Está aparte de `rasteriza` para poder afirmar sobre ella, y no es un rodeo.** Cuál de los tres
/// caminos recorre un emoji **no lo decide este proyecto: lo decide la fuente que la máquina tenga
/// instalada**. En ésta manda `Noto Color Emoji`, que trae PNG, así que los otros dos se entregarían sin
/// que ninguna prueba los hubiera ejecutado nunca — y son los dos que llegan por delante en **F2.7**.
/// Comprobado mutándolos con el cuerpo dentro de `rasteriza`: quitar la vuelta de BGRA a RGBA, que pinta
/// de azul lo que el emoji tiene de rojo, dejaba las 647 pruebas en verde.
fn pixeles_de(
    mapa: &skrifa::bitmap::BitmapData<'_>,
    ancho: u32,
    alto: u32,
) -> Option<image::RgbaImage> {
    match mapa {
        skrifa::bitmap::BitmapData::Png(bytes) => {
            let mut pixeles = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .ok()?
                .into_rgba8();
            // **Se premultiplica ANTES de reescalar, y ésa es toda la diferencia entre un emoji limpio y
            // uno con halo.** Promediar píxeles transparentes con opacos sin premultiplicar mezcla el
            // color de los transparentes en el borde del dibujo. Es el defecto clásico del reescalado con
            // alfa, y aquí ocurriría en todos: un emoji es un dibujo redondo dentro de un cuadro vacío.
            for pixel in pixeles.pixels_mut() {
                let alfa = u32::from(pixel.0[3]);
                for canal in &mut pixel.0[..3] {
                    *canal = u8::try_from(u32::from(*canal) * alfa / 255).unwrap_or(*canal);
                }
            }
            Some(pixeles)
        }
        skrifa::bitmap::BitmapData::Bgra(bytes) => {
            let mut crudo = bytes.to_vec();
            // De BGRA a RGBA. Ya viene con el alfa premultiplicado, que es lo que hace falta arriba.
            for pixel in crudo.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            image::RgbaImage::from_raw(ancho, alto, crudo)
        }
        // Una máscara de alfa es un dibujo **monocromo**: no es un glifo de color y no se pinta por aquí.
        skrifa::bitmap::BitmapData::Mask(_) => None,
    }
}

/// El mapa de bits de color de un glifo, reescalado para que quepa en el hueco que se le da.
///
/// Devuelve `None` cuando esa cara **no** trae el glifo a color: o no tiene tabla de mapas de bits, o la
/// tiene y para ese glifo guarda una **máscara de alfa**, que es un dibujo monocromo y por tanto no es
/// esto. Quien llame cae al camino de texto.
///
/// **Los contornos de color —`COLR`— quedan fuera, y no por olvido.** Pintarlos exige rasterizar
/// contornos a mano, que es el rasterizador propio que este hito descartó a propósito por invalidar el
/// histórico de medida entero. En Linux no se nota —las fuentes de emoji del sistema son de mapa de
/// bits— pero en Windows sí, porque `Segoe UI Emoji` es `COLR`: queda declarado como lo que le falta a
/// **F2.7**, no como algo que aquí funcione.
fn rasteriza(
    fuente: &skrifa::FontRef<'_>,
    glifo: skrifa::GlyphId,
    alto: u32,
    ancho_maximo: u32,
) -> Option<egui::ColorImage> {
    let mapa = skrifa::bitmap::BitmapStrikes::new(fuente)
        .glyph_for_size(skrifa::instance::Size::new(CUERPO_PARA_RASTERIZAR), glifo)?;
    let pixeles = pixeles_de(&mapa.data, mapa.width, mapa.height)?;
    if pixeles.width() == 0 || pixeles.height() == 0 {
        return None;
    }
    // **El factor es el MENOR de los dos y no el de la altura, aunque el contrato hable de la altura.**
    // Los dos a la vez —«a la altura de la celda» y «sin invadir la de al lado»— solo se cumplen así: el
    // mapa de bits de `Noto Color Emoji` es más ancho que alto (136×128), así que escalarlo por la altura
    // lo deja **más ancho que las dos columnas** que el motor le reservó, y el emoji se comería el borde
    // de su vecina. Medido: 18 px de ancho para una celda de 17 px de alto.
    let factor =
        (alto as f32 / pixeles.height() as f32).min(ancho_maximo as f32 / pixeles.width() as f32);
    let ancho_final = ((pixeles.width() as f32 * factor).round() as u32).clamp(1, ancho_maximo);
    let alto_final = ((pixeles.height() as f32 * factor).round() as u32).clamp(1, alto);
    let pequeno = image::imageops::resize(
        &pixeles,
        ancho_final,
        alto_final,
        // **Triangular y no Lanczos.** Se reduce de 136 px a diecisiete: a esa escala los dos dan un
        // resultado que no se distingue mirándolo, y el triangular cuesta la mitad. El coste se paga una
        // vez por carácter, pero se paga **dentro de un fotograma**.
        image::imageops::FilterType::Triangle,
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [ancho_final as usize, alto_final as usize],
        pequeno.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El selector de emoji: `U+FE0F`.
    const A_EMOJI: char = '\u{FE0F}';
    /// El selector de texto: `U+FE0E`.
    const A_TEXTO: char = '\u{FE0E}';

    #[test]
    fn un_emoji_de_los_de_toda_la_vida_va_a_color_sin_que_nadie_lo_pida() {
        // `Emoji_Presentation=Yes`. Exigir el selector aquí dejaría en gris la inmensa mayoría de los
        // emojis que un programa escribe, que es como se ven hoy.
        assert_eq!(presentacion_de('😀', &[]), Presentacion::Color);
    }

    #[test]
    fn un_simbolo_de_texto_se_queda_en_texto_aunque_exista_su_version_emoji() {
        // `☁` U+2601 tiene `Emoji=Yes` pero `Emoji_Presentation=No`. Mirar solo «es emoji» lo pintaría
        // a color, y es exactamente el caso que separa esta regla de la ingenua.
        assert_eq!(presentacion_de('☁', &[]), Presentacion::Texto);
    }

    #[test]
    fn el_selector_de_emoji_lleva_a_color_un_simbolo_que_por_defecto_es_texto() {
        // Es `☁️` — la razón de ser de toda esta entrega.
        assert_eq!(presentacion_de('☁', &[A_EMOJI]), Presentacion::Color);
    }

    #[test]
    fn el_selector_de_texto_devuelve_a_monocromo_un_emoji_que_por_defecto_va_a_color() {
        // La mitad que se olvida: la regla tiene que funcionar **en los dos sentidos**. Con solo la
        // mitad de arriba, este caso pasaría igual y la prueba no ataría nada.
        assert_eq!(presentacion_de('😀', &[A_TEXTO]), Presentacion::Texto);
    }

    #[test]
    fn un_selector_que_no_va_el_primero_no_habla_de_este_caracter() {
        // Un selector de variación aplica al carácter que tiene justo delante. Si aparece más adentro
        // de la secuencia, se refiere a otro — mirarlo sería atribuirle a esta celda una presentación
        // que se pidió para otra cosa.
        assert_eq!(
            presentacion_de('☁', &['\u{200D}', A_EMOJI]),
            Presentacion::Texto
        );
    }

    #[test]
    fn una_letra_corriente_no_es_emoji_por_mucho_que_lleve_cosas_pegadas() {
        // Una `a` con una tilde combinante pegada. Sin este caso, una regla que devolviera Color en
        // cuanto hubiera algo pegado pasaría todas las demás pruebas.
        assert_eq!(presentacion_de('a', &['\u{0301}']), Presentacion::Texto);
    }
    #[test]
    fn el_selector_de_emoji_sobre_un_emoji_lo_deja_a_color() {
        // **El caso donde un orden de ramas equivocado se cae.** El resultado coincide con el que daría
        // ignorar el selector, así que sin este caso la tabla del contrato queda con un hueco justo donde
        // las dos condiciones se solapan.
        assert_eq!(presentacion_de('😀', &[A_EMOJI]), Presentacion::Color);
    }

    #[test]
    fn el_selector_de_texto_sobre_un_simbolo_de_texto_lo_deja_en_texto() {
        // La otra esquina del solape, por la misma razón: las dos condiciones apuntan al mismo lado.
        assert_eq!(presentacion_de('☁', &[A_TEXTO]), Presentacion::Texto);
    }
    #[test]
    fn un_caracter_que_no_es_emoji_no_se_va_a_color_por_llevar_el_selector_detras() {
        // **Un selector de emoji detrás de una `a` no hace de la `a` un emoji.** Unicode solo define
        // secuencias de variación para caracteres con `Emoji=Yes`; detrás de cualquier otro, el
        // selector no significa nada. Y `a\u{FE0F}` no es un caso rebuscado: sale de cualquier
        // programa que corte una cadena por la mitad de un emoji, y hasta de un salto de línea —el
        // motor pega un selector suelto a la celda VACÍA que tenga delante—, así que sin esta regla
        // un espacio en blanco se manda a la fuente de color.
        assert_eq!(presentacion_de('a', &[A_EMOJI]), Presentacion::Texto);
        assert_eq!(presentacion_de(' ', &[A_EMOJI]), Presentacion::Texto);
    }
    // ── El descubrimiento ────────────────────────────────────────────────────────────────────────

    use eframe::egui;

    /// Un contexto de egui con su motor de texto ya montado.
    fn contexto() -> egui::Context {
        let ctx = egui::Context::default();
        pasada(&ctx);
        ctx
    }

    /// **`set_fonts` no surte efecto hasta la pasada siguiente**: guarda la configuración y el motor de
    /// texto se rehace después. Sin esto, cualquier comprobación mira el estado de antes.
    fn pasada(ctx: &egui::Context) {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }

    /// Lo que mide el glifo de ese carácter en el atlas, en píxeles.
    ///
    /// **Es el único oráculo fiable que hay, y por eso las pruebas de aquí lo usan en vez de preguntar.**
    /// `Fonts::has_glyph` devuelve `false` para la `'a'` —ver `Descubridor::resolver`—, así que afirmar
    /// sobre él sería afirmar sobre un dato falso. El dibujo que sale al atlas no miente.
    fn tamano_del_glifo(ctx: &egui::Context, c: char) -> Option<(u16, u16)> {
        ctx.fonts_mut(|f| {
            let galley = f.layout_no_wrap(
                c.to_string(),
                egui::FontId::monospace(14.0),
                egui::Color32::WHITE,
            );
            let uv = galley.rows.first()?.glyphs.first()?.uv_rect;
            Some((uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]))
        })
    }

    /// Si ese carácter se está pintando con **el cuadro de reemplazo**.
    ///
    /// Se compara contra `U+10FFFD`, que vive en un plano de uso privado que ninguna fuente normal cubre.
    /// **Medido en el mismo estado que el carácter**: el atlas se rehace al registrar una cara, así que un
    /// tamaño de antes no es comparable con uno de después.
    /// Lo que mide el glifo **pidiendo la familia que el pintor pediría** para ese carácter.
    fn con_su_familia(
        ctx: &egui::Context,
        descubridor: &Descubridor,
        c: char,
    ) -> Option<(u16, u16)> {
        ctx.fonts_mut(|f| {
            let id = egui::FontId::new(14.0, descubridor.familia_de(c));
            let uv = f
                .layout_no_wrap(c.to_string(), id, egui::Color32::WHITE)
                .rows
                .first()?
                .glyphs
                .first()?
                .uv_rect;
            Some((uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]))
        })
    }

    fn sale_como_cuadro(ctx: &egui::Context, c: char) -> bool {
        let suyo = tamano_del_glifo(ctx, c);
        suyo.is_some() && suyo == tamano_del_glifo(ctx, '\u{10FFFD}')
    }

    #[test]
    fn el_ascii_no_se_apunta_nunca() {
        // **Es lo único que corre por celda**, así que descartar el caso corriente sin tocar nada es la
        // mitad de que el hito no cueste. Las fuentes de fábrica cubren el ASCII entero.
        let mut descubridor = Descubridor::default();
        for c in ' '..='~' {
            descubridor.apunta(c);
        }

        assert_eq!(
            descubridor.pendientes.len(),
            0,
            "el ASCII acabó en la cola de lo que hay que buscar"
        );
    }

    #[test]
    fn una_pantalla_de_basura_no_se_come_el_fotograma_buscando_fuentes() {
        // **Ésta es la prueba que faltaba, y su primera versión no probaba nada.** Afirmaba sobre
        // `pendientes.len()` —el campo donde se apunta el valor— y pasaba igual tanto si el trabajo estaba
        // acotado como si no, que es exactamente el disfraz que las reglas del proyecto nombran. Lo que
        // hay que acotar es **el tiempo que se gasta buscando**, y es sobre eso que se afirma aquí.
        //
        // El escenario es real: un `cat` de un binario produce **decenas de miles** de caracteres
        // distintos, ninguno cubierto por nada. Medido antes de acotarlo: **6,0–6,6 ms sostenidos en cada
        // fotograma**, más que el fotograma entero.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();

        // **Se afirma sobre las BÚSQUEDAS, no sobre milisegundos, y eso no es rodear el problema.** Las
        // pruebas corren en depuración, donde todo cuesta un orden de magnitud más que en el binario que
        // se publica: un techo en milisegundos mediría el perfil de compilación tanto como el código, y
        // sería verde o rojo según dónde se corriera. Lo que el presupuesto acota de verdad es **cuántas
        // búsquedas caben en una vuelta**, y eso sí es el mismo número aquí y en release.
        //
        // Diez fotogramas con **dos mil caracteres nuevos cada uno**, como los que produce un volcado.
        let mut mas_buscados = 0;
        for fotograma in 0..10u32 {
            for i in 0..2000u32 {
                if let Some(c) = char::from_u32(0xE000 + fotograma * 2000 + i) {
                    descubridor.apunta(c);
                }
            }
            mas_buscados = mas_buscados.max(descubridor.resolver(&ctx).buscados);
            pasada(&ctx);
        }

        // **La clave es que NO crece con la cola.** Sin frenos se buscaría la cola entera —cuarenta por
        // fotograma, indefinidamente—, que es lo que costaba 6,0–6,6 ms sostenidos.
        //
        // **Y hacen falta los dos frenos: quitar uno solo no tumba esta prueba, quitar los dos sí.**
        // Comprobado mutándolos. No es redundancia: protegen escenarios distintos. El presupuesto acota
        // el caso en que las búsquedas **aciertan** y son caras —una fuente CJK pesa 20 MB—; el freno de
        // fallos acota el caso en que **ninguna** acierta, donde el presupuesto se repone cada fotograma
        // y nunca llegaría a cortar nada.
        assert!(
            mas_buscados < PENDIENTES_MAXIMOS,
            "se buscaron {mas_buscados} caracteres en una sola vuelta, que es la cola entera: el trabajo \
             de buscar fuentes no está acotado y crece con lo que escupa el programa"
        );
    }

    #[test]
    fn un_caracter_que_ya_se_busco_no_se_vuelve_a_buscar() {
        // **Hay que recordar también los FALLOS**, y ésa es la mitad que se olvida: un carácter cubierto
        // deja su cara registrada, así que el siguiente ya no busca; pero uno que NADIE cubre no deja
        // nada, y sin recordarlo se recorrerían las candidatas cada vez que apareciera — que es justo lo
        // que un programa roto escupe en bucle. Sale de **D24**.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        // Un plano de uso privado que ninguna fuente normal cubre: la búsqueda va a fallar seguro.
        descubridor.apunta('\u{10FFFD}');
        let primera = descubridor.resolver(&ctx);

        descubridor.apunta('\u{10FFFD}');
        let segunda = descubridor.resolver(&ctx);

        assert_eq!(primera.buscados, 1, "la primera vuelta no llegó a buscar");
        assert_eq!(
            primera.registradas, 0,
            "registró una cara para algo que nadie cubre"
        );
        assert_eq!(
            segunda.buscados, 0,
            "volvió a buscar un carácter que ya había dado por imposible"
        );
    }

    #[test]
    fn un_caracter_que_nadie_cubre_deja_la_ventana_en_pie_y_sale_como_cuadro() {
        // **AC6.** Lo que no está, no se inventa: se pinta con lo que dé el motor y no se intenta nada
        // más. Lo que esta prueba defiende es que ese camino **no se cae** ni deja el motor a medias.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        descubridor.apunta('\u{10FFFD}');
        descubridor.resolver(&ctx);
        pasada(&ctx);

        assert!(
            sale_como_cuadro(&ctx, '\u{10FFFD}'),
            "un carácter que nadie cubre dejó de salir como cuadro, así que algo se lo inventó"
        );
        assert!(
            !sale_como_cuadro(&ctx, 'a'),
            "buscar un carácter imposible se llevó por delante el resto de la fuente"
        );
    }
    #[test]
    fn el_japones_deja_de_salir_como_cuadro_cuando_la_maquina_tiene_con_que() {
        // **AC1, y es el motivo del hito entero.** Un terminal que enseña una fila de cuadros donde hay
        // japonés está descalificado frente a los que son el listón (**D19**).
        //
        // **La prueba afirma en los DOS escenarios y por eso no se salta sola**: si la máquina tiene una
        // fuente CJK, el kanji tiene que dejar de ser el cuadro; si no la tiene, tiene que seguir siéndolo
        // y la ventana tiene que quedar en pie — que es **AC6** y **AC9**. Saltarla en una máquina pelada
        // dejaría el camino de la escasez sin cubrir, que es justo donde se cae una implementación.
        let ctx = contexto();
        assert!(
            sale_como_cuadro(&ctx, '日'),
            "el japonés ya se veía antes de descubrir nada: esta prueba no estaría probando el hito"
        );

        let mut descubridor = Descubridor::default();
        descubridor.apunta('日');
        let vuelta = descubridor.resolver(&ctx);
        pasada(&ctx);

        if vuelta.registradas > 0 {
            assert!(
                !sale_como_cuadro(&ctx, '日'),
                "se registró una cara para el japonés y sigue saliendo el cuadro"
            );
        } else {
            assert!(
                sale_como_cuadro(&ctx, '日'),
                "no se registró ninguna cara y aun así el japonés cambió: algo se lo inventó"
            );
        }
        assert_eq!(vuelta.buscados, 1, "no llegó a buscarlo");
    }

    #[test]
    fn la_fuente_del_sistema_no_le_roba_el_glifo_a_la_del_terminal() {
        // **Contrato 2, y es lo que impide que este hito estropee lo que ya funcionaba.** La cadena de
        // reserva se recorre en orden: una cara del sistema puesta delante le quitaría las letras a la
        // fuente del terminal, y el terminal cambiaría de aspecto por descubrir un kanji.
        let ctx = contexto();
        let antes = tamano_del_glifo(&ctx, 'a');

        let mut descubridor = Descubridor::default();
        descubridor.apunta('日');
        descubridor.resolver(&ctx);
        pasada(&ctx);

        assert_eq!(
            tamano_del_glifo(&ctx, 'a'),
            antes,
            "registrar una cara del sistema le cambió el dibujo a una letra corriente"
        );
    }

    #[test]
    fn un_icono_del_area_de_uso_privado_lo_gana_la_fuente_instalada() {
        // **AC2, y la excepción al contrato de arriba.** egui embarca de fábrica una fuente de iconos que
        // ocupa esas zonas, así que sin la excepción **no sale un cuadro: sale el icono equivocado** —y la
        // fuente que el usuario instaló no llega a usarse nunca—. Unicode no le da significado a esas
        // zonas: quién tiene razón ahí lo decide quien instaló la fuente.
        //
        // **Se afirma sobre el dibujo, no sobre el orden de una lista.** Afirmar que la cara quedó la
        // primera pasaría igual aunque el motor la ignorara después.
        let ctx = contexto();
        let icono = '\u{F015}';
        let antes = tamano_del_glifo(&ctx, icono);

        let mut descubridor = Descubridor::default();
        descubridor.apunta(icono);
        let vuelta = descubridor.resolver(&ctx);
        pasada(&ctx);

        let de_fabrica = ["Hack".to_owned(), "emoji-icon-font".to_owned()];

        if vuelta.registradas > 0 {
            assert!(
                antes.is_some() && antes == tamano_del_glifo(&ctx, '\u{10FFFD}'),
                "el icono ya se veía antes: esta prueba no estaría probando nada"
            );
            assert_eq!(
                descubridor
                    .de_iconos(&de_fabrica)
                    .first()
                    .map(String::as_str),
                Some(descubridor.caras[0].familia.as_str()),
                "la cara descubierta para un icono no quedó la primera de la familia de iconos, así que \
                 la fuente de fábrica le seguirá ganando el glifo"
            );
            // **Y la monoespaciada NO se toca: es la otra mitad y sin ella el arreglo no vale.**
            assert_eq!(
                descubridor.monoespaciada(&de_fabrica)[..de_fabrica.len()],
                de_fabrica,
                "la cara de iconos se coló en la familia monoespaciada, donde le robaría las letras a la \
                 fuente del terminal"
            );
        } else {
            assert_eq!(
                descubridor.de_iconos(&de_fabrica),
                de_fabrica,
                "no se registró ninguna cara y aun así la familia de iconos cambió"
            );
        }
    }

    #[test]
    fn una_cara_que_no_es_de_iconos_se_queda_detras_de_las_de_fabrica() {
        // **La otra mitad de la excepción, y sin ella «poner todo delante» pasaría igual.** Es el error
        // simétrico y el más caro: una cara encontrada buscando un kanji, puesta delante, le quitaría las
        // letras a la fuente del terminal.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        descubridor.apunta('日');
        let vuelta = descubridor.resolver(&ctx);

        let de_fabrica = ["Hack".to_owned()];
        if vuelta.registradas > 0 {
            let mono = descubridor.monoespaciada(&de_fabrica);
            assert_eq!(
                mono.first().map(String::as_str),
                Some("Hack"),
                "una cara descubierta por un kanji se coló delante de la fuente del terminal"
            );
            assert_eq!(mono.len(), 2, "no se añadió la cara descubierta");
            assert_eq!(
                descubridor.de_iconos(&de_fabrica),
                de_fabrica,
                "una cara descubierta por un kanji se coló en la familia de iconos"
            );
        }
    }
    #[test]
    fn sin_fuentes_en_la_maquina_no_se_reintenta_enumerar_por_cada_caracter() {
        // **AC9.** Enumerar es lo más caro de todo el camino —entre 17 y 23 ms, **D24**—, así que en una
        // máquina sin fontconfig reintentarlo por cada carácter raro que pase por pantalla congelaría la
        // ventana a cambio de nada. Se dice una vez por la salida de error y se deja de intentar.
        //
        // **La prueba fuerza el estado en vez de desinstalar fontconfig**, y se dice: no hay forma de
        // quitarle las fuentes a la máquina desde dentro de una prueba. Lo que sí se ata es que, una vez
        // en ese estado, no se vuelve a tocar el sistema.
        let ctx = contexto();
        let mut descubridor = Descubridor {
            sin_fuentes: true,
            ..Default::default()
        };
        descubridor.pendientes.insert('日');

        let vuelta = descubridor.resolver(&ctx);

        assert_eq!(
            vuelta.registradas, 0,
            "con la máquina dada por vacía se registró una cara"
        );
        assert!(
            descubridor.coleccion.is_none(),
            "se volvió a enumerar las fuentes del sistema después de saber que no hay ninguna"
        );
    }
    #[test]
    fn descubrir_un_icono_no_le_cambia_el_dibujo_a_las_letras() {
        // **Ésta es la prueba que faltaba, y su ausencia dejó pasar un defecto grave.** La primera versión
        // colaba la cara de iconos DELANTE de las de fábrica en la familia monoespaciada, y como una
        // `Nerd Font` trae el latino completo, **le cambiaba el dibujo a tres de cada cinco letras**: el
        // terminal cambiaba de aspecto por haber visto un icono. La prueba que había solo miraba el caso
        // del kanji, que va detrás y nunca lo habría destapado.
        //
        // **Se mide con la familia que el pintor pediría para cada carácter**, que es lo único que
        // describe lo que se ve: preguntar siempre por la monoespaciada escondería precisamente el
        // mecanismo que se está probando.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        let letras: Vec<(char, Option<(u16, u16)>)> = "aWgñ0"
            .chars()
            .map(|c| (c, con_su_familia(&ctx, &descubridor, c)))
            .collect();

        descubridor.apunta('\u{F015}');
        descubridor.resolver(&ctx);
        pasada(&ctx);

        for (c, antes) in letras {
            assert_eq!(
                con_su_familia(&ctx, &descubridor, c),
                antes,
                "descubrir un icono le cambió el dibujo a {c:?}: la fuente de iconos se coló en la \
                 familia monoespaciada y le está robando las letras a la fuente del terminal"
            );
        }
    }

    #[test]
    fn la_familia_de_iconos_no_se_pide_hasta_que_existe() {
        // **Pedirle a epaint una familia con nombre que nadie ha declarado es un PÁNICO**, no un aviso:
        // «is not bound to any fonts». Y quien pinta pide familia en cada celda desde el primer fotograma,
        // mucho antes de que se haya descubierto nada. Comprobado: caía.
        let descubridor = Descubridor::default();

        assert_eq!(
            descubridor.familia_de('\u{F015}'),
            egui::FontFamily::Monospace,
            "se pidió la familia de iconos sin haber registrado ninguna cara de iconos"
        );
    }

    #[test]
    fn la_familia_de_iconos_tampoco_se_pide_en_el_fotograma_que_la_registra() {
        // **Y ésta es la mitad que se cae de verdad, porque es el camino que la aplicación recorre.**
        // `set_fonts` guarda la configuración y el motor se rehace **en la pasada siguiente**; pero el
        // descubrimiento corre **antes de pintar, en la misma pasada**, así que entre registrar la cara y
        // pintar la celda no hay ninguna pasada de por medio. Pedir ahí la familia **hace caer la
        // aplicación**. Comprobado ejecutándolo: `is not bound to any fonts`.
        //
        // La prueba de arriba no lo cubre: aquella mira el estado inicial, y este defecto solo aparece
        // **después** de un registro con éxito.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        descubridor.apunta('\u{F015}');
        let vuelta = descubridor.resolver(&ctx);

        if vuelta.registradas > 0 {
            assert_eq!(
                descubridor.familia_de('\u{F015}'),
                egui::FontFamily::Monospace,
                "se ofreció la familia de iconos en el mismo fotograma que la declaró, cuando el motor \
                 todavía no la conoce: pedirla ahí es un pánico"
            );
            // Y en la vuelta siguiente sí, que es la otra mitad: sin ella, «nunca ofrecerla» pasaría.
            descubridor.resolver(&ctx);
            assert_eq!(
                descubridor.familia_de('\u{F015}'),
                egui::FontFamily::Name(FAMILIA_DE_ICONOS.into()),
                "la familia de iconos no se llegó a ofrecer nunca"
            );
        }
    }
    #[test]
    fn una_fuente_del_sistema_que_se_llame_como_una_de_fabrica_no_la_sustituye() {
        // **Las claves de egui son nombres corrientes** —`Hack`, `Ubuntu-Light`, `NotoEmoji-Regular`— y
        // `Hack` está instalada en muchísimas máquinas de desarrollo. Sin prefijo, registrarla cambiaría
        // **los bytes** de la de fábrica conservando **su posición al frente de la cadena**: el terminal
        // cambiaría de fuente entera de golpe, sin que ningún carácter fuera de uso privado y sin pasar
        // siquiera por el reparto de familias.
        //
        // **Se fabrica la colisión en vez de esperar a tenerla**: esta máquina no tiene ninguna familia
        // llamada `Hack`, así que una prueba que dependiera de lo instalado no probaría nada aquí y
        // fallaría en la máquina de otro.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        descubridor.caras.push(CaraDelSistema {
            familia: format!("{PREFIJO_DEL_SISTEMA}Hack"),
            datos: Arc::new(egui::FontData::from_static(&[])),
            delante: false,
        });

        let de_fabrica = ["Hack".to_owned(), "emoji-icon-font".to_owned()];
        let mono = descubridor.monoespaciada(&de_fabrica);

        assert_eq!(
            mono.iter().filter(|n| *n == "Hack").count(),
            1,
            "la fuente del sistema llamada Hack ocupó el sitio de la Hack de fábrica"
        );
        assert_eq!(
            mono.first().map(String::as_str),
            Some("Hack"),
            "la de fábrica dejó de ser la primera de la cadena"
        );
        assert!(
            mono.iter().any(|n| n == "sistema:Hack"),
            "la del sistema no llegó a añadirse con su nombre propio"
        );
        let _ = ctx;
    }
    /// Si esta máquina tiene con qué pintar emojis a color.
    fn hay_color(ctx: &egui::Context) -> bool {
        Descubridor::default()
            .glifo_a_color(ctx, '\u{1F600}', 20, 20)
            .is_some()
    }

    #[test]
    fn un_ascii_sin_nada_pegado_no_va_nunca_a_color() {
        // **Ata un atajo que vive en el pintor**: la pasada del color descarta el ASCII suelto con dos
        // comparaciones, sin consultar estas tablas, porque es la inmensa mayoría de lo que pasa por una
        // pantalla de terminal. Ese atajo duplica una regla
        // que vive aquí, y duplicar una regla solo es aceptable si algo avisa cuando las dos dejan de
        // decir lo mismo. Esto es ese algo.
        for n in 0..=127u8 {
            assert_eq!(
                presentacion_de(char::from(n), &[]),
                Presentacion::Texto,
                "el carácter ASCII U+{n:04X} suelto pide color, y quien pinta lo descarta sin \
                 preguntar: no saldría"
            );
        }
    }

    #[test]
    fn una_tecla_de_ascii_con_su_selector_si_va_a_color() {
        // **La otra mitad, y la que destapó un defecto de verdad.** La primera versión del atajo del
        // pintor descartaba **todo** el ASCII de la pasada del color, y se comía `#️⃣` `*️⃣` y `0️⃣`–`9️⃣`:
        // Unicode le da `Emoji=Yes` a la almohadilla, al asterisco y a los diez dígitos, así que con el
        // selector detrás son emojis de pleno derecho. Lo encontró la prueba de arriba al correrla.
        for c in "#*0123456789".chars() {
            assert_eq!(
                presentacion_de(c, &['\u{FE0F}']),
                Presentacion::Color,
                "«{c}» con el selector de emoji detrás se quedó en texto: es la base de una tecla y \
                 tiene que salir a color"
            );
        }
    }

    #[test]
    fn el_glifo_de_color_se_rasteriza_una_vez_y_luego_se_recuerda() {
        // **La caché no es una optimización opcional: sin ella se rasteriza en CADA fotograma.** Son 45 a
        // 80 µs por emoji y por fotograma —medido—, o sea que una pantalla con veinte emojis se comería
        // más de un milisegundo en repetir un trabajo idéntico sesenta veces por segundo.
        //
        // **Se afirma sobre el CONTADOR de rasterizados y no sobre el tamaño del mapa**, porque el mapa
        // guarda también los fallos: una implementación que buscara siempre y no guardara nada dejaría el
        // mapa igual de grande.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();

        descubridor.glifo_a_color(&ctx, '\u{1F600}', 20, 20);
        let primera = descubridor.coste_del_color();
        assert_eq!(
            primera.rasterizados + primera.fallidos,
            1,
            "la primera vez no llegó a intentar rasterizar nada"
        );

        for _ in 0..10 {
            descubridor.glifo_a_color(&ctx, '\u{1F600}', 20, 20);
        }
        let despues = descubridor.coste_del_color();
        assert_eq!(
            (despues.rasterizados, despues.fallidos),
            (0, 0),
            "diez peticiones más del mismo emoji volvieron a hacer trabajo: no se está recordando nada"
        );
        assert_eq!(
            despues.gastado,
            std::time::Duration::ZERO,
            "se contó tiempo por consultar algo ya recordado: el tramo del informe dejaría de describir \
             lo que cuesta rasterizar"
        );
    }

    #[test]
    fn cambiar_la_altura_de_la_celda_tira_lo_recordado() {
        // **Sin esto, subir el zoom del escritorio deja los emojis del tamaño anterior pintados en celdas
        // del nuevo** — y como la caché no se vacía nunca, para el resto de la vida del proceso.
        //
        // **Se afirma sobre el TAMAÑO de la textura devuelta, no sobre que se haya vaciado el mapa.** Una
        // caché que se vacíe y vuelva a rasterizar al tamaño viejo cumpliría lo segundo y seguiría
        // pintando mal, que es el defecto que esto existe para impedir.
        let ctx = contexto();
        if !hay_color(&ctx) {
            return;
        }
        let mut descubridor = Descubridor::default();
        let (_, _, pequeno) = descubridor
            .glifo_a_color(&ctx, '\u{1F600}', 20, 20)
            .expect("la máquina tiene fuente de color y no dio glifo");
        let (_, _, grande) = descubridor
            .glifo_a_color(&ctx, '\u{1F600}', 40, 40)
            .expect("no dio glifo a la altura nueva");

        assert!(
            grande.y > pequeno.y,
            "con la celda al doble de alto el emoji salió de {pequeno:?}, el tamaño de la celda \
             anterior: la caché está devolviendo lo viejo"
        );
        assert_eq!(
            descubridor.glifos_recordados(),
            1,
            "quedaron guardadas las dos alturas: la caché no se vació y la memoria crece con cada zoom"
        );
    }

    #[test]
    fn al_llenarse_el_atlas_se_deja_de_recordar_y_se_dice() {
        // **AC8.** El techo de este hito es una textura de tamaño fijo: la memoria que la tarjeta gráfica
        // dedica a los emojis es 1 MiB y no cambia, aparezcan uno o mil. Quién decide cuántos caracteres
        // distintos aparecen es el programa de dentro —un `cat` de un fichero de emojis los recorrería
        // todos—, así que sin techo la textura tendría que crecer sin límite.
        //
        // **Se atan las DOS mitades**: que deje de aceptar al llenarse, y que lo diga. Un techo callado es
        // indistinguible de un fallo — quien vea sus emojis en monocromo de golpe no tendría forma de
        // saber que el terminal sabe pintarlos y se ha quedado sin sitio.
        //
        // **Se llena con imágenes sintéticas y no con emojis de verdad**, y se dice: llenarlo de verdad
        // exige que la máquina tenga ochocientos emojis distintos instalados, que es una condición de la
        // máquina y no del código.
        let ctx = contexto();
        let mut glifos = Glifos::default();
        let lado = LADO_DEL_ATLAS / 8;
        let cuadrado =
            || egui::ColorImage::filled([lado, lado], egui::Color32::from_rgb(10, 20, 30));

        let mut pegados = 0;
        while glifos.pegar(&ctx, cuadrado()).is_some() {
            pegados += 1;
            assert!(pegados <= 8 * 8, "el atlas aceptó más de lo que cabe en él");
        }
        assert_eq!(
            pegados,
            8 * 8,
            "el atlas no aprovechó su superficie: caben {} cuadrados de {lado} píxeles y entraron {pegados}",
            8 * 8
        );
        assert!(
            glifos.dijo_el_techo,
            "el atlas se llenó y no se dijo por la salida de error: un techo callado se lee como un fallo"
        );
    }

    #[test]
    fn dos_glifos_distintos_no_se_pisan_dentro_del_atlas() {
        // **Es el modo de fallo propio de meterlos todos en una textura**, y no lo cubre ninguna prueba de
        // arriba: colocar mal uno lo pinta encima de otro, y entonces dos emojis distintos salen iguales.
        // Con una textura por glifo esto no podía pasar; con atlas, sí.
        //
        // **Se comprueban todos los pares de una tanda larga y no solo los dos primeros**, porque el sitio
        // donde se pisan es el **salto de fila**: dos glifos seguidos caen bien casi con cualquier cuenta,
        // y el que estrena fila es el que se coloca encima del de arriba.
        let ctx = contexto();
        let mut glifos = Glifos::default();
        // Anchos distintos a propósito: con todos iguales, una cuenta que ignorase el ancho real y
        // avanzara un paso fijo colocaría bien igualmente.
        let sitios: Vec<_> = (0..80)
            .filter_map(|n| {
                let ancho = 15 + n % 7;
                glifos.pegar(
                    &ctx,
                    egui::ColorImage::filled([ancho, 17], egui::Color32::RED),
                )
            })
            .collect();
        assert!(
            sitios.len() > 60,
            "solo cupieron {} glifos de 17 píxeles de alto en un atlas de {LADO_DEL_ATLAS}: la prueba no \
             llega a saltar de fila varias veces",
            sitios.len()
        );

        let lado = LADO_DEL_ATLAS as f32;
        for (n, uno) in sitios.iter().enumerate() {
            for otro in &sitios[n + 1..] {
                let comun = uno.uv.intersect(otro.uv);
                // **Menos de medio píxel de solape, no cero exacto**: dos glifos pegados comparten su
                // borde, y en coma flotante ese borde no da una anchura de cero limpia.
                assert!(
                    comun.width() * lado < 0.5 || comun.height() * lado < 0.5,
                    "dos glifos se solapan dentro del atlas ({:?} y {:?}): uno se pinta encima del otro \
                     y los dos emojis salen iguales",
                    uno.uv,
                    otro.uv
                );
            }
        }
    }

    #[test]
    fn un_dibujo_monocromo_no_se_pinta_por_el_camino_del_color() {
        // **Es el hallazgo de la primera entrega, y aquí es donde se resuelve.** Una fuente puede *cubrir*
        // un emoji y pintarlo en blanco y negro —en esta máquina `DejaVu Sans` se lleva el 😀 teniendo
        // `Noto Color Emoji` instalada—, así que «la primera que lo cubra» es la regla equivocada.
        //
        // Lo que ata esta prueba es la mitad comprobable sin depender de qué fuentes haya: **una máscara
        // de alfa nunca sale por aquí**, y una máscara de alfa es exactamente un dibujo monocromo.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        // La 'a' no es emoji y ninguna fuente de emoji la trae a color: tiene que salir sin glifo.
        assert!(
            descubridor.glifo_a_color(&ctx, 'a', 20, 20).is_none(),
            "una letra corriente salió por el camino del color"
        );
        let coste = descubridor.coste_del_color();
        assert_eq!(
            coste.rasterizados, 0,
            "se rasterizó una letra corriente como si fuera un emoji"
        );
        assert_eq!(
            coste.fallidos, 1,
            "el intento fallido no se contó, así que el informe no lo declara"
        );
    }

    #[test]
    fn un_caracter_sin_glifo_de_color_no_se_vuelve_a_buscar() {
        // **Recordar el fallo no es un detalle.** Sin él, un carácter que ninguna fuente de color cubre
        // recorrería las familias de emoji **en cada fotograma** — y es justo el carácter que un programa
        // roto escupe en bucle.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        // **Un carácter que NINGUNA fuente pinta a color, y por eso es el sin asignar de Unicode.** La
        // primera versión de esta prueba usaba un muñeco de nieve y no probaba nada: `Noto Color Emoji`
        // lo trae, así que se recordaba por el camino del éxito y quitar el recuerdo del **fallo** la
        // dejaba pasando igual. Lo destapó mutarla.
        assert!(
            descubridor
                .glifo_a_color(&ctx, CUADRO_DE_REEMPLAZO, 20, 20)
                .is_none(),
            "el carácter sin asignar salió a color: la prueba no está midiendo el camino del fallo"
        );
        let primera = descubridor.coste_del_color();
        assert_eq!(primera.fallidos, 1, "el intento fallido no se contó");

        for _ in 0..5 {
            descubridor.glifo_a_color(&ctx, CUADRO_DE_REEMPLAZO, 20, 20);
        }
        let despues = descubridor.coste_del_color();
        assert_eq!(
            despues.rasterizados + despues.fallidos,
            0,
            "se volvió a buscar un carácter que ya se había buscado: se paga el recorrido entero por \
             fotograma mientras esté en pantalla"
        );
    }

    #[test]
    fn un_emoji_de_una_sola_columna_se_queda_dentro_de_ella() {
        // **El caso que el criterio obliga y que se ve raro, así que conviene que esté escrito.** El motor
        // de terminal le da **una sola celda** a `☁️`, `❤️` y `✔️` —comprobado ejecutándolo: llegan como
        // `Single` con el selector pegado, mientras que `😀` llega como cabeza de un carácter ancho—, y el
        // criterio dice que un emoji no invade a su vecina más allá de lo que su anchura declara.
        //
        // **Consecuencia, dicha con su número**: en una celda de 13×20 puntos, que es la de esta máquina
        // con dos divisiones, ese emoji sale de 13×12 — un 60 % del alto de la línea, centrado. Se ve
        // pequeño y **es lo correcto**: pintarlo a la altura de la línea taparía lo que el programa
        // escribió en la celda de al lado, y quién decide la anchura de un carácter es el motor, no este
        // proyecto.
        let ctx = contexto();
        if !hay_color(&ctx) {
            return;
        }
        let mut descubridor = Descubridor::default();
        let (_, _, medidas) = descubridor
            .glifo_a_color(&ctx, '\u{2601}', 20, 13)
            .expect("la máquina tiene fuente de color y no dio glifo");
        assert!(
            medidas.x <= 13.0,
            "el emoji de una columna salió de {medidas:?} en una celda de 13 de ancho: se come la vecina \
             y tapa lo que el programa escribió ahí"
        );
        assert!(
            medidas.x as u32 == 13,
            "el emoji de una columna salió de {medidas:?} sin llenar los 13 de ancho que tiene: se ve \
             más pequeño de lo que le cabe"
        );
    }

    #[test]
    fn el_emoji_se_reescala_para_caber_en_el_hueco_que_se_le_da() {
        // **AC5, por el lado de quien rasteriza.** El pintor comprueba el rectángulo que sale a pantalla;
        // aquí se comprueba que la imagen que se le entrega ya viene acotada, que es lo que hace que ese
        // rectángulo quepa.
        let ctx = contexto();
        if !hay_color(&ctx) {
            return;
        }
        let mut descubridor = Descubridor::default();
        for (alto, ancho) in [(20u32, 40u32), (40, 20), (17, 17)] {
            let (_, _, medidas) = descubridor
                .glifo_a_color(&ctx, '\u{1F600}', alto, ancho)
                .expect("la máquina tiene fuente de color y no dio glifo");
            assert!(
                medidas.y <= alto as f32 && medidas.x <= ancho as f32,
                "para un hueco de {ancho}x{alto} el emoji salió de {medidas:?}: se sale de su sitio"
            );
            // Y que quepa no basta: tiene que **llenar** uno de los dos ejes, o estaría desperdiciando
            // sitio y saldría más pequeño de lo que la celda da.
            assert!(
                medidas.y as u32 == alto || medidas.x as u32 == ancho,
                "para un hueco de {ancho}x{alto} el emoji salió de {medidas:?}: no llena ningún eje, \
                 así que se ve más pequeño de lo que cabe"
            );
        }
    }

    #[test]
    fn al_llegar_al_techo_de_familias_se_deja_de_registrar_y_se_dice() {
        // **Un techo que se traga lo que descarta es indistinguible de un fallo.** Quien vea un cuadro
        // después de instalar la fuente adecuada no tendría forma de saber que el terminal la encontró y
        // decidió no usarla. Y el caso ocurre: sin oráculo perfecto se registran caras de más, y una
        // sesión que gaste sus huecos en caras redundantes dejará en cuadros un idioma de verdad.
        //
        // **Se fuerza el estado en vez de esperar a llenarlo de verdad**, y se dice: llenar ocho huecos
        // con fuentes reales exige que la máquina tenga ocho familias que cubran ocho scripts distintos,
        // que es una condición de la máquina y no del código.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        for n in 0..FAMILIAS_MAXIMAS {
            descubridor.caras.push(CaraDelSistema {
                familia: format!("{PREFIJO_DEL_SISTEMA}relleno{n}"),
                datos: Arc::new(egui::FontData::from_static(&[])),
                delante: false,
            });
        }

        descubridor.apunta('日');
        let vuelta = descubridor.resolver(&ctx);

        assert_eq!(
            vuelta.buscados, 1,
            "con el techo lleno ni siquiera se llegó a mirar el carácter"
        );
        assert_eq!(
            vuelta.registradas, 0,
            "se registró una cara por encima del techo"
        );
        assert_eq!(
            descubridor.caras.len(),
            FAMILIAS_MAXIMAS,
            "la lista de caras pasó de su techo"
        );
        assert!(
            descubridor.dijo_el_techo,
            "se llegó al techo y no se dijo por la salida de error: un techo callado se lee como un fallo"
        );
    }

    #[test]
    fn un_mapa_de_bits_que_es_una_mascara_monocroma_no_sale_por_el_camino_del_color() {
        // **Una máscara de alfa es un dibujo en blanco y negro, y por el camino del color saldría teñido
        // de lo que hubiera en el canal.** Es el mismo hallazgo que la entrega ya tiene escrito —una
        // fuente puede *cubrir* un emoji sin traerlo a color— pero por el otro camino: aquí la fuente sí
        // es de emoji y sí trae mapa de bits para ese glifo, solo que monocromo. Quien lo pida tiene que
        // caer al camino de texto, que es el Contrato 5.
        //
        // **Se construye la máscara a mano y se dice por qué**: en esta máquina manda `Noto Color Emoji`,
        // que trae PNG, así que ninguna fuente instalada recorre esta rama. Se comprobó mutándola:
        // convertir la máscara en imagen dejaba las 647 pruebas en verde.
        let mascara = skrifa::bitmap::BitmapData::Mask(skrifa::bitmap::MaskData {
            bpp: 8,
            is_packed: false,
            data: &[0xFF, 0x80, 0x40, 0x00],
        });

        assert!(
            pixeles_de(&mascara, 2, 2).is_none(),
            "un dibujo monocromo salió por el camino del color: se pinta como imagen y sale teñido de lo \
             que traiga el canal, en vez de con el color que el programa pidió para esa celda"
        );
    }

    #[test]
    fn un_mapa_de_bits_en_bgra_conserva_los_colores_que_el_emoji_tiene() {
        // **La fuente entrega los canales al revés y hay que darles la vuelta.** Sin eso un emoji rojo
        // sale azul y uno azul sale rojo — un defecto que **no se ve en esta máquina**, porque aquí gana
        // una fuente que trae PNG, y que sí se vería con las que entregan el mapa de bits sin comprimir.
        // Se comprobó mutándolo: quitar el intercambio no rompía ninguna prueba.
        //
        // Un píxel rojo opaco, escrito como lo escribe la fuente: azul, verde, rojo, alfa.
        let bgra = skrifa::bitmap::BitmapData::Bgra(&[0, 0, 255, 255]);

        let pixeles = pixeles_de(&bgra, 1, 1).expect("un píxel en BGRA no dio imagen");

        assert_eq!(
            pixeles.get_pixel(0, 0).0,
            [255, 0, 0, 255],
            "el emoji salió con el rojo y el azul cambiados"
        );
    }

    #[test]
    fn un_pixel_transparente_no_se_lleva_su_color_al_borde_del_emoji() {
        // **Es el halo, y es el defecto clásico de reducir un dibujo con alfa.** Un emoji es una figura
        // dentro de un cuadro vacío, y el vacío de un PNG lleva color aunque no se vea: promediado sin
        // premultiplicar, ese color invisible se mezcla en el borde de la figura y le deja una orla. Se
        // vería en **todos** los emojis, no en un caso raro, porque todos se reducen de 128 píxeles a
        // diecisiete.
        //
        // **Se afirma sobre el píxel transparente y no sobre el borde ya reducido**, porque es donde el
        // defecto se ve sin depender de qué dibujo traiga la fuente instalada: premultiplicado, lo que no
        // se ve no tiene color que aportar. Se comprobó mutándolo: sin premultiplicar, la suite pasaba
        // entera.
        let mut origen = image::RgbaImage::new(2, 1);
        origen.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        // Blanco del todo transparente: no se ve, pero su color está escrito en el fichero.
        origen.put_pixel(1, 0, image::Rgba([255, 255, 255, 0]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(origen)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("no se pudo escribir el PNG de la prueba");

        let pixeles = pixeles_de(&skrifa::bitmap::BitmapData::Png(&png), 2, 1)
            .expect("el PNG de la prueba no dio imagen");

        assert_eq!(
            pixeles.get_pixel(1, 0).0,
            [0, 0, 0, 0],
            "un píxel invisible conservó su color: al reducir el emoji ese color se promedia con el del \
             dibujo y le deja un halo alrededor"
        );
        assert_eq!(
            pixeles.get_pixel(0, 0).0,
            [255, 0, 0, 255],
            "el píxel opaco perdió color al premultiplicar: el emoji entero saldría apagado"
        );
    }

    #[test]
    fn una_celda_sin_alto_o_sin_ancho_no_tumba_la_ventana() {
        // **Es un pánico, no un dibujo raro.** El reescalado acota el tamaño final entre uno y lo que la
        // celda da, y con una celda de cero ese acotado pide un mínimo mayor que su máximo — la
        // biblioteca estándar entra en pánico ahí, así que se cae la ventana entera. Y la celda llega a
        // valer cero: el alto y el ancho salen de multiplicar su rectángulo por los píxeles físicos y
        // redondear, y el de una consola aplastada contra el borde mide una fracción de punto.
        //
        // Se comprobó mutándola: sin la guarda, esta prueba no falla — revienta.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();

        assert!(
            descubridor
                .glifo_a_color(&ctx, '\u{1F600}', 0, 20)
                .is_none(),
            "una celda sin alto dio glifo"
        );
        assert!(
            descubridor
                .glifo_a_color(&ctx, '\u{1F600}', 20, 0)
                .is_none(),
            "una celda sin ancho dio glifo"
        );
    }

    #[test]
    fn con_el_atlas_lleno_no_se_inventa_ninguna_textura() {
        // **AC8 por el lado de quien pide, que es el que decide lo que se ve.** Que el atlas deje de
        // aceptar ya está probado arriba; lo que aquí se ata es que un emoji pedido con el atlas lleno
        // **no devuelve nada** en vez de un trozo cualquiera de la textura. Sin esto, la forma natural de
        // «arreglar» el techo —devolver el último sitio, o un rectángulo por defecto— pinta en cada emoji
        // nuevo el dibujo de otro, y quien mire verá la cara equivocada.
        //
        // **Se llena con imágenes sintéticas y se le fija antes la altura**, porque una altura distinta de
        // la que el atlas tiene dentro lo vacía entero: sin fijarla, la petición de abajo encontraría
        // sitio de sobra y esto no probaría nada.
        let ctx = contexto();
        let mut descubridor = Descubridor::default();
        descubridor.glifos.alto = 20;
        let lado = LADO_DEL_ATLAS / 4;
        while descubridor
            .glifos
            .pegar(
                &ctx,
                egui::ColorImage::filled([lado, lado], egui::Color32::RED),
            )
            .is_some()
        {}

        assert!(
            descubridor
                .glifo_a_color(&ctx, '\u{1F600}', 20, 20)
                .is_none(),
            "con el atlas lleno se devolvió una textura igualmente: es el trozo que ocupa otro glifo, así \
             que el emoji sale con el dibujo de otro"
        );
    }
}
