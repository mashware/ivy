//! La tabla de color y las dos políticas de atributos que son gusto y no especificación.
//!
//! Vive aquí y no en el motor del terminal por una razón que no es de estilo: una tabla de colores
//! *es* el tema, y el motor tiene que sobrevivir intacto a una reescritura de la interfaz. Tampoco
//! se puede pedirle la paleta al parser: la suya está vacía salvo lo que el programa fije por
//! secuencias de escape, y una aplicación que resuelva por ahí sale en negro.

use eframe::egui::Color32;
use panel_contract::{Attention, Mark, Style};
use term_core::TermColor;

/// Cuánto se acerca al fondo un color atenuado.
///
/// Es gusto puro: la especificación dice «menos intenso» y no cuánto. La mitad se lee como
/// claramente apagado sin llegar a desaparecer sobre este fondo, que es lo que se pierde al pasar
/// de dos tercios.
const DIM_FACTOR: f32 = 0.5;

/// Los seis niveles por canal del cubo de color de la paleta de 256.
///
/// No son equidistantes, y no es un capricho: es la tabla que fija `xterm` y que todo el ecosistema
/// da por supuesta. Repartirlos a intervalos regulares daría colores que no coinciden con los que
/// el programa espera ver.
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Cuánto tapa el velo que señala la consola sobre la que se soltaría la que se arrastra, de 0 a 255.
///
/// Es gusto, pero acotado por los dos lados: tiene que verse sobre una consola llena de texto y tiene que
/// dejar leer lo que hay debajo, porque lo que hay debajo es lo que el usuario está mirando para decidir
/// si suelta ahí. Una sexta parte cumple las dos.
const DROP_TARGET_ALPHA: u8 = 40;

/// El tema: qué color de pantalla le corresponde a cada color que pide un programa.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Las dieciséis entradas con nombre. De la 0 a la 7 las normales, de la 8 a la 15 las
    /// brillantes.
    palette: [Color32; 16],
    background: Color32,
    foreground: Color32,
    cursor: Color32,
    /// La línea que separa dos consolas.
    divider: Color32,
    /// Con qué se señala la consola a la que va el teclado.
    ///
    /// **Es un color de la aplicación, no de la paleta del terminal**, y esa distinción importa: la
    /// paleta la pide el programa de dentro y describe *su* contenido. Esto describe el estado de la
    /// aplicación, así que un programa no puede reclamarlo ni cambiarlo.
    focus: Color32,
    /// El fondo del panel de estado, que es lo que lo separa visualmente de su consola sin una línea.
    panel_background: Color32,
}

impl Default for Theme {
    /// La estética del proyecto.
    ///
    /// El prototipo aporta **nueve** colores, y ninguno de ellos es rojo, cian, magenta ni un
    /// brillante: sirve de base estética, no de paleta. Los que faltaban se han escrito para que
    /// pertenezcan a la misma familia —desaturados, de contraste medio— y no para reproducir
    /// ninguna paleta canónica.
    ///
    /// Las brillantes salen de acercar su color normal a blanco una cuarta parte, con dos
    /// excepciones que vienen del prototipo y no de esa regla: el negro brillante es su gris
    /// apagado y el blanco brillante es blanco puro.
    fn default() -> Self {
        Self {
            palette: [
                Color32::from_rgb(0x1E, 0x1E, 0x1E), // 0  negro — el mismo del fondo
                Color32::from_rgb(0xCF, 0x6A, 0x4C), // 1  rojo
                Color32::from_rgb(0x8F, 0x9D, 0x6A), // 2  verde
                Color32::from_rgb(0xF9, 0xEE, 0x98), // 3  amarillo
                Color32::from_rgb(0x75, 0x87, 0xA6), // 4  azul
                Color32::from_rgb(0x9B, 0x85, 0x9D), // 5  magenta
                Color32::from_rgb(0x7F, 0xA5, 0xA0), // 6  cian — escrito, no estaba
                Color32::from_rgb(0xA7, 0xA7, 0xA7), // 7  blanco
                Color32::from_rgb(0x5F, 0x5A, 0x60), // 8  negro brillante — el gris del prototipo
                Color32::from_rgb(0xDB, 0x8F, 0x79), // 9  rojo brillante
                Color32::from_rgb(0xAB, 0xB6, 0x8F), // 10 verde brillante
                Color32::from_rgb(0xFB, 0xF2, 0xB2), // 11 amarillo brillante
                Color32::from_rgb(0x98, 0xA5, 0xBC), // 12 azul brillante
                Color32::from_rgb(0xB4, 0xA4, 0xB6), // 13 magenta brillante
                Color32::from_rgb(0x9F, 0xBC, 0xB8), // 14 cian brillante
                Color32::from_rgb(0xFF, 0xFF, 0xFF), // 15 blanco brillante — blanco puro
            ],
            background: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            foreground: Color32::from_rgb(0xA7, 0xA7, 0xA7),
            cursor: Color32::from_rgb(0xA7, 0xA7, 0xA7),
            // El gris apagado del prototipo, que es el negro brillante de la paleta: se ve contra el
            // fondo sin llamar la atención. Un divisor no es contenido, así que competir con las letras
            // sería exactamente lo que no debe hacer.
            divider: Color32::from_rgb(0x5F, 0x5A, 0x60),
            // El azul de la familia, un paso por encima del divisor en presencia. Tiene que
            // distinguirse **del divisor** a simple vista, porque los dos se pintan pegados y si se
            // parecieran no habría forma de saber qué consola escucha el teclado.
            focus: Color32::from_rgb(0x75, 0x87, 0xA6),
            // Un paso por debajo del fondo de la consola, no por encima: el panel es el margen y la
            // consola es el contenido, así que lo que tiene que retroceder es el panel. Subirlo —hacerlo
            // más claro— convertiría cada división en dos bloques compitiendo, y con dos divisiones
            // serían cuatro.
            panel_background: Color32::from_rgb(0x18, 0x18, 0x18),
        }
    }
}

impl Theme {
    /// El tema de fábrica con los colores que el fichero de configuración pida encima.
    ///
    /// **Lo que el fichero no diga se queda de fábrica**, que es lo que permite cambiar un solo color sin
    /// escribir los veintidós. Y la paleta se sobrescribe hueco a hueco por lo mismo: pedir el color 1 no
    /// debe borrar los otros quince.
    pub fn from_colors(colors: &crate::settings::Colores) -> Self {
        let mut theme = Self::default();
        for (slot, asked_for) in theme.palette.iter_mut().zip(colors.paleta.iter()) {
            if let Some(color) = asked_for {
                *slot = *color;
            }
        }
        for (slot, asked_for) in [
            (&mut theme.background, colors.fondo),
            (&mut theme.foreground, colors.texto),
            (&mut theme.cursor, colors.cursor),
            (&mut theme.divider, colors.divisor),
            (&mut theme.focus, colors.foco),
            (&mut theme.panel_background, colors.panel),
        ] {
            if let Some(color) = asked_for {
                *slot = color;
            }
        }
        theme
    }
}

/// Los colores con los que se pinta el panel de estado.
///
/// **Salen de la paleta del terminal a propósito, no de una segunda familia.** Los dos se ven a la vez
/// y a un palmo el uno del otro; con dos juegos de verdes o de rojos distintos, la pantalla se leería
/// como dos aplicaciones pegadas. Lo que cambia respecto a la consola es *quién* elige el color: aquí
/// lo elige el vocabulario del contrato, no el programa de dentro.
impl Theme {
    /// El fondo del panel de estado.
    pub fn panel_background(&self) -> Color32 {
        self.panel_background
    }

    /// El color de una línea del panel, según lo que el productor haya dicho de ella.
    ///
    /// **`style` manda sobre `mark` cuando vienen los dos**, y esa precedencia es del contrato: dos de
    /// los nueve ficheros reales del repo los combinan a propósito, para decir «esta línea es
    /// información, y su color *es* el dato». Si `mark` ganara, ese color se perdería.
    ///
    /// Sin ninguno de los dos, el color del texto normal de la consola: una línea sin adjetivos no es
    /// una línea apagada.
    pub fn panel_color(&self, mark: Option<Mark>, style: Option<Style>) -> Color32 {
        match (style, mark) {
            (Some(style), _) => self.by_style(style),
            (None, Some(mark)) => self.by_mark(mark),
            (None, None) => self.foreground,
        }
    }

    /// Los siete estilos del contrato.
    fn by_style(&self, style: Style) -> Color32 {
        match style {
            Style::Normal => self.foreground,
            // Apagado es «esto está aquí por si acaso»: el mismo gesto que el atenuado del terminal,
            // que ya significa eso mismo para el programa de dentro.
            Style::Dim => self.toward_background(self.foreground),
            // El título es lo único que se pinta más claro que el texto normal. No lleva color propio:
            // un título de color competiría con las marcas, que sí lo usan para decir algo.
            Style::Title => self.palette[15],
            Style::Accent => self.palette[4],
            Style::Ok => self.palette[2],
            Style::Warn => self.palette[3],
            Style::Error => self.palette[1],
        }
    }

    /// Las seis marcas del contrato.
    ///
    /// **`done` y `pending` van apagadas, y las dos por la misma razón**: lo terminado y lo que ni ha
    /// empezado son los dos estados en los que no hay nada que hacer. Lo que tiene que saltar a la vista
    /// es `current` —dónde va la cosa— y `block` —qué la para—.
    fn by_mark(&self, mark: Mark) -> Color32 {
        match mark {
            Mark::Done => self.toward_background(self.palette[2]),
            Mark::Current => self.palette[4],
            Mark::Pending => self.toward_background(self.foreground),
            Mark::Wait => self.palette[3],
            Mark::Block => self.palette[1],
            Mark::Info => self.foreground,
        }
    }

    /// El color con el que se pinta un reclamo de una división.
    ///
    /// **Tabla propia, y no la de las marcas del cuerpo, aunque las dos vengan de la misma paleta.** La
    /// de las marcas apaga `done` porque lo terminado es donde no queda nada que hacer; aquí `done`
    /// dice lo contrario —terminó y **nadie lo ha mirado**—, así que reutilizarla apagaría justo lo que
    /// reclama. Es la copia peligrosa de este hito y por eso está dicha.
    ///
    /// **Sin sostener es lo mismo atenuado**: el reclamo sigue ahí —la aplicación no esconde lo que su
    /// productor dijo— pero nadie lo está sosteniendo, y eso se ve antes de leer nada. El detalle
    /// —desde cuándo, o qué falló— lo dice la línea del aviso, que va justo debajo.
    pub fn attention_color(&self, class: Attention, backed: bool) -> Color32 {
        let color = match class {
            Attention::Wait => self.palette[3],
            Attention::Block => self.palette[1],
            Attention::Done => self.palette[2],
        };
        if backed { color } else { self.toward_background(color) }
    }
}

impl Theme {
    /// El color de pantalla que le toca a un color pedido por un programa.
    ///
    /// No aplica las políticas de negrita ni de atenuado: para eso está [`Theme::with_attributes`].
    pub fn color(&self, c: TermColor) -> Color32 {
        match c {
            TermColor::Indexed(i) => self.indexed(i),
            TermColor::Rgb { r, g, b } => Color32::from_rgb(r, g, b),
            TermColor::Foreground => self.foreground,
            TermColor::Background => self.background,
            TermColor::Cursor => self.cursor,
            // La variante atenuada solo la producen las ocho primeras entradas. Se enmascara en vez
            // de indexar a pelo porque esta función la llama el pintor una vez por celda y un
            // índice fuera de rango sería un pánico en mitad de un frame; la afirmación de
            // depuración está para que ese error salga en los tests en vez de disfrazarse de negro.
            TermColor::Dim(i) => {
                debug_assert!(
                    i < 8,
                    "la variante atenuada solo cubre las ocho primeras entradas"
                );
                self.toward_background(self.palette[(i & 0b111) as usize])
            }
            TermColor::BrightForeground => self.palette[15],
            TermColor::DimForeground => self.toward_background(self.foreground),
        }
    }

    /// El color, ya con las dos políticas aplicadas.
    ///
    /// **Quién decide a cuál de los dos colores de la celda se le aplican no es asunto de aquí.**
    /// Con vídeo inverso, el color que el programa declaró como primer plano —y que es el que la
    /// negrita y el atenuado describen— viaja en el campo del fondo. Quien pinta sabe eso; el tema
    /// solo sabe en qué consisten las dos políticas.
    pub fn with_attributes(&self, c: TermColor, bold: bool, dim: bool) -> Color32 {
        // La atenuación llega por dos caminos —como color propio y como atributo de la celda— y
        // aquí se juntan, así que primero se normaliza a uno solo. Sin esto, un color que ya viene
        // atenuado y además trae el atributo se apagaría dos veces, y la negrita no lo alcanzaría
        // por quedar escondido tras una variante que ella no reconoce.
        let (base, dim) = match c {
            TermColor::Dim(i) => (TermColor::Indexed(i & 0b111), true),
            TermColor::DimForeground => (TermColor::Foreground, true),
            other => (other, dim),
        };
        let base = if bold { brighten(base) } else { base };
        let resolved = self.color(base);
        if dim {
            self.toward_background(resolved)
        } else {
            resolved
        }
    }

    /// El fondo por defecto, que es también el de la ventana entera.
    pub fn background(&self) -> Color32 {
        self.background
    }

    /// El color de las letras por defecto, para el texto que pinta la aplicación y no el programa.
    pub fn foreground(&self) -> Color32 {
        self.foreground
    }

    /// El color de la línea que separa dos consolas.
    pub fn divider(&self) -> Color32 {
        self.divider
    }

    /// El color con el que se señala la consola a la que va el teclado.
    pub fn focus(&self) -> Color32 {
        self.focus
    }

    /// El color con el que se señala la consola sobre la que se soltaría la que se arrastra.
    ///
    /// **Sale del color del foco y no es un color nuevo**, porque dice lo mismo en otro tiempo verbal:
    /// «aquí va a ir el teclado». Un color propio obligaría a elegirlo en los dos temas y a explicar en
    /// qué se diferencia de éste, y la respuesta sería «en nada».
    ///
    /// Traslúcido porque se pinta **encima** de lo que la consola esté escribiendo: opaco taparía el
    /// contenido justo cuando el usuario está decidiendo si soltar ahí.
    pub fn drop_target(&self) -> Color32 {
        let focus = self.focus;
        Color32::from_rgba_unmultiplied(focus.r(), focus.g(), focus.b(), DROP_TARGET_ALPHA)
    }

    /// Acerca un color al fondo. Es lo que aquí significa «atenuado».
    fn toward_background(&self, c: Color32) -> Color32 {
        mix(c, self.background, DIM_FACTOR)
    }

    fn indexed(&self, i: u8) -> Color32 {
        match i {
            0..=15 => self.palette[i as usize],
            16..=231 => {
                let n = i - 16;
                Color32::from_rgb(
                    CUBE_LEVELS[(n / 36) as usize],
                    CUBE_LEVELS[((n / 6) % 6) as usize],
                    CUBE_LEVELS[(n % 6) as usize],
                )
            }
            // Los veinticuatro grises, de casi negro a casi blanco. Ninguno de los dos extremos
            // llega a serlo: para eso están las entradas 0 y 15.
            232..=255 => {
                let v = 8 + (i - 232) * 10;
                Color32::from_rgb(v, v, v)
            }
        }
    }
}

/// **La negrita significa brillante.**
///
/// Es la convención que dan por supuesta los programas que usan negrita para destacar, y que hace
/// legible un `ls` en color. Solo sube las ocho primeras entradas con nombre: un color exacto o una
/// entrada del cubo ya es el color que el programa quiso, y aclararlo sería desobedecerle.
fn brighten(c: TermColor) -> TermColor {
    match c {
        TermColor::Indexed(i) if i < 8 => TermColor::Indexed(i + 8),
        TermColor::Foreground => TermColor::BrightForeground,
        other => other,
    }
}

/// Mezcla dos colores, con `p` como la parte que se lleva el segundo.
fn mix(a: Color32, b: Color32, p: f32) -> Color32 {
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * p).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::default()
    }

    /// **La regla del contrato que es fácil leer al revés.** Su tabla describe `style` como algo «para
    /// líneas sin `mark`», pero dos de los nueve ficheros reales del repo los combinan a propósito: la
    /// marca dice qué **es** la línea y el estilo dice que su color *es* el dato. Si la marca ganara,
    /// ese color se perdería sin que nadie se enterase.
    #[test]
    fn el_estilo_manda_sobre_la_marca_cuando_vienen_los_dos() {
        let t = theme();

        assert_eq!(
            t.panel_color(Some(Mark::Info), Some(Style::Ok)),
            t.panel_color(None, Some(Style::Ok)),
            "el color tiene que salir del estilo, no de la marca"
        );
        assert_ne!(
            t.panel_color(Some(Mark::Info), Some(Style::Ok)),
            t.panel_color(Some(Mark::Info), None),
            "con estilo y sin él no puede verse igual"
        );
    }

    #[test]
    fn una_linea_sin_marca_ni_estilo_se_pinta_como_el_texto_normal() {
        assert_eq!(theme().panel_color(None, None), theme().foreground);
    }

    /// Los trece nombres del contrato, cada uno con su color. Sin esto, devolver el mismo para todos
    /// dejaría la suite verde y el panel en un solo tono.
    #[test]
    fn las_seis_marcas_y_los_siete_estilos_no_se_pintan_todos_igual() {
        let t = theme();
        let marks = [
            Mark::Done,
            Mark::Current,
            Mark::Pending,
            Mark::Wait,
            Mark::Block,
            Mark::Info,
        ];
        let styles = [
            Style::Normal,
            Style::Dim,
            Style::Title,
            Style::Accent,
            Style::Ok,
            Style::Warn,
            Style::Error,
        ];

        // Se cuentan los colores **distintos** dentro de cada juego, y no se comparan entre juegos:
        // `normal` y `info` valen el mismo a propósito —los dos son «texto sin adjetivos»— y exigir
        // trece colores obligaría a inventar una diferencia que no significa nada.
        let from_marks: std::collections::HashSet<_> =
            marks.iter().map(|m| t.by_mark(*m)).collect();
        let from_styles: std::collections::HashSet<_> =
            styles.iter().map(|e| t.by_style(*e)).collect();

        assert_eq!(from_marks.len(), 6, "las seis marcas son distintas entre sí");
        assert_eq!(from_styles.len(), 7, "los siete estilos son distintos");
        assert_ne!(
            t.by_mark(Mark::Current),
            t.by_mark(Mark::Block),
            "«en marcha» y «bloqueado» son lo que hay que distinguir de un vistazo"
        );
    }

    #[test]
    fn el_fondo_del_panel_se_distingue_del_de_la_consola() {
        let t = theme();
        assert_ne!(t.panel_background(), t.background());
    }

    #[test]
    fn los_tres_colores_por_defecto_son_los_del_proyecto() {
        let t = theme();
        assert_eq!(
            t.color(TermColor::Background),
            Color32::from_rgb(0x1E, 0x1E, 0x1E)
        );
        assert_eq!(
            t.color(TermColor::Foreground),
            Color32::from_rgb(0xA7, 0xA7, 0xA7)
        );
        assert_eq!(
            t.color(TermColor::Cursor),
            Color32::from_rgb(0xA7, 0xA7, 0xA7)
        );
    }

    #[test]
    fn el_divisor_y_el_foco_se_ven_y_se_distinguen_entre_si() {
        let t = theme();
        // Tres desigualdades, y cada una cubre una forma distinta de que esto quede inservible sin que
        // nada falle: un divisor del color del fondo es un divisor invisible —dos consolas pegadas que
        // se leen como una—, y un realce igual al divisor deja la ventana sin decir a cuál de las dos
        // va el teclado, que es justo lo que el realce existe para decir.
        assert_ne!(t.divider(), t.background(), "el divisor no se vería");
        assert_ne!(t.focus(), t.background(), "el realce del foco no se vería");
        assert_ne!(
            t.focus(),
            t.divider(),
            "el realce no se distingue del divisor y van pegados"
        );
    }

    #[test]
    fn el_borde_entre_los_nombrados_y_el_cubo() {
        let t = theme();
        // 15 es la última entrada con nombre; 16 es la primera del cubo, y es negro puro —no el
        // fondo del tema—. Confundir estos dos es el error que este test existe para cazar.
        assert_eq!(
            t.color(TermColor::Indexed(15)),
            Color32::from_rgb(0xFF, 0xFF, 0xFF)
        );
        assert_eq!(
            t.color(TermColor::Indexed(16)),
            Color32::from_rgb(0x00, 0x00, 0x00)
        );
    }

    #[test]
    fn el_borde_entre_el_cubo_y_los_grises() {
        let t = theme();
        // 231 es la última del cubo y es blanco puro; 232 es el primer gris y **no** es negro.
        assert_eq!(
            t.color(TermColor::Indexed(231)),
            Color32::from_rgb(0xFF, 0xFF, 0xFF)
        );
        assert_eq!(
            t.color(TermColor::Indexed(232)),
            Color32::from_rgb(0x08, 0x08, 0x08)
        );
        assert_eq!(
            t.color(TermColor::Indexed(255)),
            Color32::from_rgb(0xEE, 0xEE, 0xEE)
        );
    }

    #[test]
    fn el_cubo_reparte_los_canales_en_el_orden_correcto() {
        let t = theme();
        // 196 es el rojo puro del cubo: 196-16 = 180 = 5*36, así que rojo al máximo y los otros a
        // cero. Si alguien invierte el orden de los canales, este es el test que se pone rojo.
        assert_eq!(
            t.color(TermColor::Indexed(196)),
            Color32::from_rgb(0xFF, 0x00, 0x00)
        );
        // 21 es azul puro: 21-16 = 5, el último nivel del canal de menor peso.
        assert_eq!(
            t.color(TermColor::Indexed(21)),
            Color32::from_rgb(0x00, 0x00, 0xFF)
        );
    }

    #[test]
    fn las_brillantes_cumplen_la_regla_con_la_que_se_escribieron() {
        let t = theme();
        // El doc de la paleta afirma que las brillantes salen de acercar su color normal a blanco
        // una cuarta parte. Una afirmación escrita y no comprobada se convierte en mentira en
        // cuanto alguien retoca una entrada, así que aquí se comprueba.
        // Quedan fuera las dos excepciones que el propio doc declara: el negro y el blanco
        // brillantes vienen del prototipo, no de la regla.
        for i in 1..=6u8 {
            let normal = t.color(TermColor::Indexed(i));
            let bright = t.color(TermColor::Indexed(i + 8));
            assert_eq!(
                bright,
                mix(normal, Color32::WHITE, 0.25),
                "la entrada brillante {} no sale de la regla",
                i + 8
            );
        }
    }

    #[test]
    fn un_color_exacto_pasa_sin_tocarse() {
        let t = theme();
        let c = TermColor::Rgb { r: 1, g: 2, b: 3 };
        assert_eq!(t.color(c), Color32::from_rgb(1, 2, 3));
    }

    #[test]
    fn la_negrita_sube_a_la_variante_brillante() {
        let t = theme();
        let red = TermColor::Indexed(1);
        assert_eq!(
            t.with_attributes(red, false, false),
            Color32::from_rgb(0xCF, 0x6A, 0x4C)
        );
        assert_eq!(
            t.with_attributes(red, true, false),
            Color32::from_rgb(0xDB, 0x8F, 0x79)
        );
        assert_eq!(
            t.with_attributes(red, true, false),
            t.color(TermColor::Indexed(9))
        );
    }

    #[test]
    fn la_negrita_no_toca_lo_que_el_programa_eligio_a_mano() {
        let t = theme();
        // Ni un color exacto ni una entrada del cubo se aclaran: ya son el color que se pidió.
        let exact = TermColor::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        assert_eq!(
            t.with_attributes(exact, true, false),
            Color32::from_rgb(10, 20, 30)
        );
        let from_the_cube = TermColor::Indexed(196);
        assert_eq!(
            t.with_attributes(from_the_cube, true, false),
            Color32::from_rgb(0xFF, 0x00, 0x00)
        );
        // Y una entrada que ya es brillante no vuelve a subir: la 9 no se convierte en la 17.
        assert_eq!(
            t.with_attributes(TermColor::Indexed(9), true, false),
            t.color(TermColor::Indexed(9))
        );
    }

    #[test]
    fn el_atenuado_acerca_el_color_al_fondo() {
        let t = theme();
        // El rojo a medio camino de #1E1E1E.
        assert_eq!(
            t.with_attributes(TermColor::Indexed(1), false, true),
            Color32::from_rgb(0x77, 0x44, 0x35)
        );
        // Y el fondo atenuado sigue siendo el fondo: no hay a dónde acercarse.
        assert_eq!(
            t.with_attributes(TermColor::Background, false, true),
            t.color(TermColor::Background)
        );
    }

    #[test]
    fn la_variante_atenuada_de_la_libreria_da_lo_mismo_que_el_atributo() {
        let t = theme();
        // Son los dos caminos por los que llega lo mismo: el color `Dim(n)` que entrega el motor y
        // el atributo de atenuado sobre el color normal. Si divergieran, la misma secuencia de
        // escape se vería de dos colores distintos según por dónde entrara.
        for n in 0..8u8 {
            assert_eq!(
                t.color(TermColor::Dim(n)),
                t.with_attributes(TermColor::Indexed(n), false, true),
                "el índice {n} no coincide por los dos caminos"
            );
        }
    }

    #[test]
    fn un_color_ya_atenuado_no_se_apaga_dos_veces() {
        let t = theme();
        // Quien pinte va a llamar a `with_attributes` con el color y los atributos de la celda tal
        // como vienen, sin mirar si el color ya es de los atenuados. Si no se normalizara, el
        // atenuado se aplicaría dos veces y el color quedaría a un cuarto del recorrido al fondo en
        // vez de a la mitad — más apagado por haber entrado por la puerta de al lado.
        for n in 0..8u8 {
            assert_eq!(
                t.with_attributes(TermColor::Dim(n), false, true),
                t.color(TermColor::Dim(n)),
                "el índice {n} se apaga dos veces"
            );
        }
        assert_eq!(
            t.with_attributes(TermColor::DimForeground, false, true),
            t.color(TermColor::DimForeground)
        );
    }

    #[test]
    fn la_negrita_alcanza_a_un_color_que_ya_venia_atenuado() {
        let t = theme();
        // Un rojo atenuado y en negrita sube a rojo brillante y luego se apaga, igual que si la
        // atenuación hubiera llegado por el atributo. Sin normalizar, la negrita no lo tocaría: la
        // variante atenuada no es una de las que sabe subir.
        assert_eq!(
            t.with_attributes(TermColor::Dim(1), true, false),
            t.with_attributes(TermColor::Indexed(1), true, true)
        );
        assert_eq!(
            t.with_attributes(TermColor::DimForeground, true, false),
            t.with_attributes(TermColor::Foreground, true, true)
        );
    }

    #[test]
    fn el_primer_plano_tiene_sus_dos_variantes() {
        let t = theme();
        assert_eq!(
            t.color(TermColor::BrightForeground),
            Color32::from_rgb(0xFF, 0xFF, 0xFF)
        );
        assert_eq!(
            t.color(TermColor::DimForeground),
            Color32::from_rgb(0x63, 0x63, 0x63)
        );
        // La negrita sobre el primer plano por defecto lleva a su variante brillante, no a la
        // entrada 8 de la paleta, que es el gris apagado.
        assert_eq!(
            t.with_attributes(TermColor::Foreground, true, false),
            t.color(TermColor::BrightForeground)
        );
    }

    #[test]
    fn negrita_y_atenuado_a_la_vez_aclaran_y_luego_apagan() {
        let t = theme();
        // El orden importa y está fijado: primero se sube a brillante, luego se apaga. Al revés
        // daría otro color, y dos capas que lo hicieran en distinto orden no coincidirían.
        let expected = mix(
            Color32::from_rgb(0xDB, 0x8F, 0x79),
            Color32::from_rgb(0x1E, 0x1E, 0x1E),
            DIM_FACTOR,
        );
        assert_eq!(t.with_attributes(TermColor::Indexed(1), true, true), expected);
    }

    #[test]
    fn ningun_indice_de_la_paleta_de_256_hace_estallar_la_tabla() {
        let t = theme();
        for i in 0..=255u8 {
            let _ = t.color(TermColor::Indexed(i));
        }
        // La variante atenuada solo recorre su rango válido: pedirla fuera de las ocho primeras
        // entradas es un error de programación, y por eso lo caza una afirmación de depuración en
        // vez de devolver un color cualquiera.
        for n in 0..8u8 {
            let _ = t.color(TermColor::Dim(n));
        }
    }

    /// Los tres con color propio. Devolviendo el mismo para todos, la fila del reclamo diría «reclama»
    /// y callaría **cuál**, que es lo único que decide si hay que ir corriendo o no.
    #[test]
    fn los_tres_reclamos_no_se_pintan_todos_igual() {
        let t = theme();
        let colors = Attention::ALL.map(|class| t.attention_color(class, true));

        for (i, one) in colors.iter().enumerate() {
            for other in &colors[i + 1..] {
                assert_ne!(one, other, "dos reclamos comparten color: {colors:?}");
            }
        }
    }

    /// **La copia peligrosa de este hito, atada donde se nota.** La tabla de las marcas del cuerpo
    /// apaga `done` porque lo terminado es donde ya no queda nada que hacer; aquí `done` dice lo
    /// contrario —terminó y **nadie lo ha mirado**—, así que reutilizarla apagaría justo lo que
    /// reclama. Con `by_mark` de por medio esta prueba cae, y sin ella la suite seguiría verde con el
    /// reclamo más silencioso de los tres pintado de gris.
    #[test]
    fn el_reclamo_terminado_no_se_pinta_apagado_como_su_marca_del_cuerpo() {
        let t = theme();

        assert_ne!(
            t.attention_color(Attention::Done, true),
            t.panel_color(Some(Mark::Done), None),
            "el reclamo terminado se está pintando con la tabla del cuerpo, que lo apaga"
        );
        assert_eq!(
            t.attention_color(Attention::Done, true),
            t.palette[2],
            "sostenido se pinta con el color entero, sin atenuar"
        );
    }

    /// **Cada clase clavada a su color, y no solo «los tres son distintos».** Las pruebas del pintado
    /// comparan contra esta misma función, así que cruzar el ámbar de `wait` con el rojo de `block` —o
    /// pintar `wait` con el gris del texto normal, que es dejar de alarmar— pasaba entera la suite. Es
    /// el disfraz de siempre: afirmar sobre el sitio donde se apunta el valor en vez de sobre el valor.
    #[test]
    fn cada_reclamo_lleva_el_color_que_la_decision_le_asigno() {
        let t = theme();

        assert_eq!(t.attention_color(Attention::Wait, true), t.palette[3]);
        assert_eq!(t.attention_color(Attention::Block, true), t.palette[1]);
        assert_eq!(t.attention_color(Attention::Done, true), t.palette[2]);
    }

    /// Y sin sostener es **ese mismo color atenuado**, no otro: el atenuado dice «nadie lo sostiene»,
    /// no «es otra cosa». Con un color propio para el rancio, la clase dejaría de leerse.
    #[test]
    fn un_reclamo_sin_sostener_es_su_propio_color_atenuado() {
        let t = theme();

        for class in Attention::ALL {
            assert_eq!(
                t.attention_color(class, false),
                t.toward_background(t.attention_color(class, true)),
                "{class:?} sin sostener no es su color apagado"
            );
        }
    }

    /// Sostenido y sin sostener no pueden verse igual: el atenuado **es** lo que dice que no hay nadie
    /// detrás, y con los dos colores iguales la ficha mentiría sobre lo fresco que es el reclamo.
    #[test]
    fn un_reclamo_sin_sostener_se_ve_distinto_del_mismo_sostenido() {
        let t = theme();
        for class in Attention::ALL {
            assert_ne!(
                t.attention_color(class, true),
                t.attention_color(class, false),
                "{class:?} se ve igual lo sostenga alguien o no"
            );
        }
    }
}
