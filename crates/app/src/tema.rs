//! La tabla de color y las dos políticas de atributos que son gusto y no especificación.
//!
//! Vive aquí y no en el motor del terminal por una razón que no es de estilo: una tabla de colores
//! *es* el tema, y el motor tiene que sobrevivir intacto a una reescritura de la interfaz. Tampoco
//! se puede pedirle la paleta al parser: la suya está vacía salvo lo que el programa fije por
//! secuencias de escape, y una aplicación que resuelva por ahí sale en negro.

use eframe::egui::Color32;
use term_core::TermColor;

/// Cuánto se acerca al fondo un color atenuado.
///
/// Es gusto puro: la especificación dice «menos intenso» y no cuánto. La mitad se lee como
/// claramente apagado sin llegar a desaparecer sobre este fondo, que es lo que se pierde al pasar
/// de dos tercios.
const ATENUACION: f32 = 0.5;

/// Los seis niveles por canal del cubo de color de la paleta de 256.
///
/// No son equidistantes, y no es un capricho: es la tabla que fija `xterm` y que todo el ecosistema
/// da por supuesta. Repartirlos a intervalos regulares daría colores que no coinciden con los que
/// el programa espera ver.
const NIVELES_CUBO: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// El tema: qué color de pantalla le corresponde a cada color que pide un programa.
#[derive(Debug, Clone, Copy)]
pub struct Tema {
    /// Las dieciséis entradas con nombre. De la 0 a la 7 las normales, de la 8 a la 15 las
    /// brillantes.
    paleta: [Color32; 16],
    fondo: Color32,
    primer_plano: Color32,
    cursor: Color32,
    /// La línea que separa dos consolas.
    divisor: Color32,
    /// Con qué se señala la consola a la que va el teclado.
    ///
    /// **Es un color de la aplicación, no de la paleta del terminal**, y esa distinción importa: la
    /// paleta la pide el programa de dentro y describe *su* contenido. Esto describe el estado de la
    /// aplicación, así que un programa no puede reclamarlo ni cambiarlo.
    foco: Color32,
}

impl Default for Tema {
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
            paleta: [
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
            fondo: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            primer_plano: Color32::from_rgb(0xA7, 0xA7, 0xA7),
            cursor: Color32::from_rgb(0xA7, 0xA7, 0xA7),
            // El gris apagado del prototipo, que es el negro brillante de la paleta: se ve contra el
            // fondo sin llamar la atención. Un divisor no es contenido, así que competir con las letras
            // sería exactamente lo que no debe hacer.
            divisor: Color32::from_rgb(0x5F, 0x5A, 0x60),
            // El azul de la familia, un paso por encima del divisor en presencia. Tiene que
            // distinguirse **del divisor** a simple vista, porque los dos se pintan pegados y si se
            // parecieran no habría forma de saber qué consola escucha el teclado.
            foco: Color32::from_rgb(0x75, 0x87, 0xA6),
        }
    }
}

impl Tema {
    /// El color de pantalla que le toca a un color pedido por un programa.
    ///
    /// No aplica las políticas de negrita ni de atenuado: para eso está [`Tema::con_atributos`].
    pub fn color(&self, c: TermColor) -> Color32 {
        match c {
            TermColor::Indexed(i) => self.indexado(i),
            TermColor::Rgb { r, g, b } => Color32::from_rgb(r, g, b),
            TermColor::Foreground => self.primer_plano,
            TermColor::Background => self.fondo,
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
                self.apagar(self.paleta[(i & 0b111) as usize])
            }
            TermColor::BrightForeground => self.paleta[15],
            TermColor::DimForeground => self.apagar(self.primer_plano),
        }
    }

    /// El color, ya con las dos políticas aplicadas.
    ///
    /// **Quién decide a cuál de los dos colores de la celda se le aplican no es asunto de aquí.**
    /// Con vídeo inverso, el color que el programa declaró como primer plano —y que es el que la
    /// negrita y el atenuado describen— viaja en el campo del fondo. Quien pinta sabe eso; el tema
    /// solo sabe en qué consisten las dos políticas.
    pub fn con_atributos(&self, c: TermColor, negrita: bool, atenuado: bool) -> Color32 {
        // La atenuación llega por dos caminos —como color propio y como atributo de la celda— y
        // aquí se juntan, así que primero se normaliza a uno solo. Sin esto, un color que ya viene
        // atenuado y además trae el atributo se apagaría dos veces, y la negrita no lo alcanzaría
        // por quedar escondido tras una variante que ella no reconoce.
        let (base, atenuado) = match c {
            TermColor::Dim(i) => (TermColor::Indexed(i & 0b111), true),
            TermColor::DimForeground => (TermColor::Foreground, true),
            otro => (otro, atenuado),
        };
        let base = if negrita { abrillantar(base) } else { base };
        let resuelto = self.color(base);
        if atenuado {
            self.apagar(resuelto)
        } else {
            resuelto
        }
    }

    /// El fondo por defecto, que es también el de la ventana entera.
    pub fn fondo(&self) -> Color32 {
        self.fondo
    }

    /// El color de la línea que separa dos consolas.
    pub fn divisor(&self) -> Color32 {
        self.divisor
    }

    /// El color con el que se señala la consola a la que va el teclado.
    pub fn foco(&self) -> Color32 {
        self.foco
    }

    /// Acerca un color al fondo. Es lo que aquí significa «atenuado».
    fn apagar(&self, c: Color32) -> Color32 {
        mezclar(c, self.fondo, ATENUACION)
    }

    fn indexado(&self, i: u8) -> Color32 {
        match i {
            0..=15 => self.paleta[i as usize],
            16..=231 => {
                let n = i - 16;
                Color32::from_rgb(
                    NIVELES_CUBO[(n / 36) as usize],
                    NIVELES_CUBO[((n / 6) % 6) as usize],
                    NIVELES_CUBO[(n % 6) as usize],
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
fn abrillantar(c: TermColor) -> TermColor {
    match c {
        TermColor::Indexed(i) if i < 8 => TermColor::Indexed(i + 8),
        TermColor::Foreground => TermColor::BrightForeground,
        otro => otro,
    }
}

/// Mezcla dos colores, con `p` como la parte que se lleva el segundo.
fn mezclar(a: Color32, b: Color32, p: f32) -> Color32 {
    let canal = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * p).round() as u8;
    Color32::from_rgb(
        canal(a.r(), b.r()),
        canal(a.g(), b.g()),
        canal(a.b(), b.b()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tema() -> Tema {
        Tema::default()
    }

    #[test]
    fn los_tres_colores_por_defecto_son_los_del_proyecto() {
        let t = tema();
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
        let t = tema();
        // Tres desigualdades, y cada una cubre una forma distinta de que esto quede inservible sin que
        // nada falle: un divisor del color del fondo es un divisor invisible —dos consolas pegadas que
        // se leen como una—, y un realce igual al divisor deja la ventana sin decir a cuál de las dos
        // va el teclado, que es justo lo que el realce existe para decir.
        assert_ne!(t.divisor(), t.fondo(), "el divisor no se vería");
        assert_ne!(t.foco(), t.fondo(), "el realce del foco no se vería");
        assert_ne!(
            t.foco(),
            t.divisor(),
            "el realce no se distingue del divisor y van pegados"
        );
    }

    #[test]
    fn el_borde_entre_los_nombrados_y_el_cubo() {
        let t = tema();
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
        let t = tema();
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
        let t = tema();
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
        let t = tema();
        // El doc de la paleta afirma que las brillantes salen de acercar su color normal a blanco
        // una cuarta parte. Una afirmación escrita y no comprobada se convierte en mentira en
        // cuanto alguien retoca una entrada, así que aquí se comprueba.
        // Quedan fuera las dos excepciones que el propio doc declara: el negro y el blanco
        // brillantes vienen del prototipo, no de la regla.
        for i in 1..=6u8 {
            let normal = t.color(TermColor::Indexed(i));
            let brillante = t.color(TermColor::Indexed(i + 8));
            assert_eq!(
                brillante,
                mezclar(normal, Color32::WHITE, 0.25),
                "la entrada brillante {} no sale de la regla",
                i + 8
            );
        }
    }

    #[test]
    fn un_color_exacto_pasa_sin_tocarse() {
        let t = tema();
        let c = TermColor::Rgb { r: 1, g: 2, b: 3 };
        assert_eq!(t.color(c), Color32::from_rgb(1, 2, 3));
    }

    #[test]
    fn la_negrita_sube_a_la_variante_brillante() {
        let t = tema();
        let rojo = TermColor::Indexed(1);
        assert_eq!(
            t.con_atributos(rojo, false, false),
            Color32::from_rgb(0xCF, 0x6A, 0x4C)
        );
        assert_eq!(
            t.con_atributos(rojo, true, false),
            Color32::from_rgb(0xDB, 0x8F, 0x79)
        );
        assert_eq!(
            t.con_atributos(rojo, true, false),
            t.color(TermColor::Indexed(9))
        );
    }

    #[test]
    fn la_negrita_no_toca_lo_que_el_programa_eligio_a_mano() {
        let t = tema();
        // Ni un color exacto ni una entrada del cubo se aclaran: ya son el color que se pidió.
        let exacto = TermColor::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        assert_eq!(
            t.con_atributos(exacto, true, false),
            Color32::from_rgb(10, 20, 30)
        );
        let del_cubo = TermColor::Indexed(196);
        assert_eq!(
            t.con_atributos(del_cubo, true, false),
            Color32::from_rgb(0xFF, 0x00, 0x00)
        );
        // Y una entrada que ya es brillante no vuelve a subir: la 9 no se convierte en la 17.
        assert_eq!(
            t.con_atributos(TermColor::Indexed(9), true, false),
            t.color(TermColor::Indexed(9))
        );
    }

    #[test]
    fn el_atenuado_acerca_el_color_al_fondo() {
        let t = tema();
        // El rojo a medio camino de #1E1E1E.
        assert_eq!(
            t.con_atributos(TermColor::Indexed(1), false, true),
            Color32::from_rgb(0x77, 0x44, 0x35)
        );
        // Y el fondo atenuado sigue siendo el fondo: no hay a dónde acercarse.
        assert_eq!(
            t.con_atributos(TermColor::Background, false, true),
            t.color(TermColor::Background)
        );
    }

    #[test]
    fn la_variante_atenuada_de_la_libreria_da_lo_mismo_que_el_atributo() {
        let t = tema();
        // Son los dos caminos por los que llega lo mismo: el color `Dim(n)` que entrega el motor y
        // el atributo de atenuado sobre el color normal. Si divergieran, la misma secuencia de
        // escape se vería de dos colores distintos según por dónde entrara.
        for n in 0..8u8 {
            assert_eq!(
                t.color(TermColor::Dim(n)),
                t.con_atributos(TermColor::Indexed(n), false, true),
                "el índice {n} no coincide por los dos caminos"
            );
        }
    }

    #[test]
    fn un_color_ya_atenuado_no_se_apaga_dos_veces() {
        let t = tema();
        // Quien pinte va a llamar a `con_atributos` con el color y los atributos de la celda tal
        // como vienen, sin mirar si el color ya es de los atenuados. Si no se normalizara, el
        // atenuado se aplicaría dos veces y el color quedaría a un cuarto del recorrido al fondo en
        // vez de a la mitad — más apagado por haber entrado por la puerta de al lado.
        for n in 0..8u8 {
            assert_eq!(
                t.con_atributos(TermColor::Dim(n), false, true),
                t.color(TermColor::Dim(n)),
                "el índice {n} se apaga dos veces"
            );
        }
        assert_eq!(
            t.con_atributos(TermColor::DimForeground, false, true),
            t.color(TermColor::DimForeground)
        );
    }

    #[test]
    fn la_negrita_alcanza_a_un_color_que_ya_venia_atenuado() {
        let t = tema();
        // Un rojo atenuado y en negrita sube a rojo brillante y luego se apaga, igual que si la
        // atenuación hubiera llegado por el atributo. Sin normalizar, la negrita no lo tocaría: la
        // variante atenuada no es una de las que sabe subir.
        assert_eq!(
            t.con_atributos(TermColor::Dim(1), true, false),
            t.con_atributos(TermColor::Indexed(1), true, true)
        );
        assert_eq!(
            t.con_atributos(TermColor::DimForeground, true, false),
            t.con_atributos(TermColor::Foreground, true, true)
        );
    }

    #[test]
    fn el_primer_plano_tiene_sus_dos_variantes() {
        let t = tema();
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
            t.con_atributos(TermColor::Foreground, true, false),
            t.color(TermColor::BrightForeground)
        );
    }

    #[test]
    fn negrita_y_atenuado_a_la_vez_aclaran_y_luego_apagan() {
        let t = tema();
        // El orden importa y está fijado: primero se sube a brillante, luego se apaga. Al revés
        // daría otro color, y dos capas que lo hicieran en distinto orden no coincidirían.
        let esperado = mezclar(
            Color32::from_rgb(0xDB, 0x8F, 0x79),
            Color32::from_rgb(0x1E, 0x1E, 0x1E),
            ATENUACION,
        );
        assert_eq!(t.con_atributos(TermColor::Indexed(1), true, true), esperado);
    }

    #[test]
    fn ningun_indice_de_la_paleta_de_256_hace_estallar_la_tabla() {
        let t = tema();
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
}
