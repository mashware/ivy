//! Dibuja la rejilla del terminal: fondos, glifos y cursor.
//!
//! **Es a propósito el pintor más ingenuo que funciona**: una celda cada vez, sin caché y sin
//! seguimiento de lo que cambió. No es descuido ni un primer borrador — optimizar antes de medir
//! escondería el síntoma que este hito existe para observar, y el modo de fallo de una caché de
//! glifos son celdas fantasma, que es de los que no se ven hasta que ya llevas semanas encima.
//!
//! **La medida de la celda se pregunta, no se inventa.** El ancho sale del glifo de referencia de
//! la fuente y el alto de su altura de línea. Calcularlo a ojo es lo que descuadra las filas.

use crate::tema::Tema;
use eframe::egui::{
    Align2, Color32, Context, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, vec2,
};
use term_core::{CellWidth, CursorShape, Size, TermColor, View, ViewCell};

/// El tamaño más grande que se acepta para una rejilla.
///
/// Holgado a propósito: un monitor de 4K con la letra más pequeña que se puede leer no pasa de
/// unas 300 columnas. No es un límite de producto, es el filtro que impide que un rectángulo que
/// no corresponde a ninguna ventana se convierta en una rejilla de cientos de miles de celdas.
const MAXIMO: Size = Size {
    rows: 300,
    cols: 1000,
};

/// Cuánto ocupa una celda en pantalla.
#[derive(Debug, Clone)]
pub struct Metrica {
    /// Privados juntos: el ancho y el alto son medidas **de esta fuente**. Con los tres campos
    /// abiertos, cambiar uno solo deja una métrica que mide una fuente y pinta con otra, y la
    /// rejilla se descuadra sin que nada falle — el mismo motivo por el que la vista de celdas
    /// guarda su tamaño en privado.
    ancho: f32,
    alto: f32,
    fuente: FontId,
}

impl Metrica {
    /// Pregunta a la fuente cuánto mide una celda.
    ///
    /// El ancho se toma de un glifo cualquiera **de ancho fijo**: en una fuente monoespaciada todos
    /// miden lo mismo, así que preguntar por uno vale por todos. Si algún día se admite una fuente
    /// que no lo sea, este es el sitio donde se romperá — y romperse aquí es mejor que descuadrarse
    /// en pantalla sin avisar.
    ///
    /// **Se pregunta en cada pasada, no se guarda.** Al cambiar el aumento del escritorio la fuente
    /// cambia de tamaño, y una medida guardada descuadraría la rejilla entera sin avisar.
    pub fn medir(ctx: &Context, fuente: FontId) -> Self {
        let (ancho, alto) = ctx.fonts_mut(|fuentes| {
            (
                fuentes.glyph_width(&fuente, 'M'),
                fuentes.row_height(&fuente),
            )
        });
        Self {
            ancho,
            alto,
            fuente,
        }
    }

    /// Lo que ocupa una celda de ancho.
    ///
    /// Se puede leer, pero no fijar: quien mide una rejilla necesita este número —para colocar dos
    /// consolas seguidas, o para saber si caben en la ventana— y calcularlo por su cuenta sería
    /// justo lo que esta caja evita, medir con una fuente y pintar con otra.
    pub fn ancho(&self) -> f32 {
        self.ancho
    }

    /// Lo que ocupa una celda de alto. Se lee por lo mismo que [`Metrica::ancho`].
    pub fn alto(&self) -> f32 {
        self.alto
    }

    /// La fuente con la que se midió, para quien pinte texto que tenga que cuadrar con esta rejilla.
    ///
    /// Se clona en vez de prestarse porque `Painter::text` la consume, y un `FontId` son dos campos.
    /// Sigue sin poder fijarse: medir con una y pintar con otra es lo que esta caja existe para
    /// impedir.
    pub fn fuente(&self) -> FontId {
        self.fuente.clone()
    }

    /// Cuántas celdas enteras caben en el espacio dado.
    ///
    /// **Nunca menos de una fila y una columna, y nunca más de [`MAXIMO`].** El suelo está porque
    /// `Screen::resize` entra en pánico con cero columnas: sin él, arrastrar el borde de la ventana
    /// hasta hacerla estrecha mataría la aplicación. El techo está porque el espacio disponible no
    /// siempre es el de una ventana de verdad — mientras el escritorio no ha dicho todavía cuánto
    /// mide, egui responde con su rectángulo de reserva, que son diez mil puntos de lado. Sin
    /// techo, eso son cientos de miles de celdas reservadas y un aviso de tamaño absurdo al
    /// programa de dentro.
    pub fn celdas_en(&self, espacio: Vec2) -> Size {
        // Los tamaños imposibles —negativos, o sin número mientras la ventana se crea— caen en
        // cero al convertirse a entero, y el suelo los sube a uno.
        Size {
            rows: ((espacio.y / self.alto) as u16).clamp(1, MAXIMO.rows),
            cols: ((espacio.x / self.ancho) as u16).clamp(1, MAXIMO.cols),
        }
    }

    /// El rectángulo que ocupa la celda de una fila y columna, desde el origen dado.
    fn celda(&self, origen: Pos2, fila: u16, columna: u16) -> Rect {
        let esquina = origen + vec2(columna as f32 * self.ancho, fila as f32 * self.alto);
        Rect::from_min_size(esquina, vec2(self.ancho, self.alto))
    }
}

/// Pinta la vista entera en el sitio indicado.
///
/// **Precondición: la superficie ya tiene que estar pintada con el fondo del tema.** Las celdas
/// que llevan ese mismo fondo no se pintan —serían casi dos mil rectángulos por pasada para no
/// cambiar nada, y con el antialias activo, una frontera por celda donde puede aparecer costura—,
/// así que sobre una superficie sin limpiar saldría el hueco de la ventana entre las letras.
///
/// El orden importa y es siempre el mismo: **primero todos los fondos, después todos los glifos y
/// el cursor al final**. Pintar cada celda entera de una vez dejaría el fondo de una celda encima
/// del glifo de la anterior en cuanto un glifo se saliera de su hueco — que es precisamente lo que
/// aquí no se recorta, para poder verlo.
pub fn pintar(painter: &Painter, vista: &View, tema: &Tema, metrica: &Metrica, origen: Pos2) {
    let tamano = vista.size();
    // El fondo por defecto es el mismo para toda la pantalla: preguntarlo una vez por celda serían
    // casi dos mil consultas por pasada para obtener siempre lo mismo.
    let fondo_de_la_ventana = tema.fondo();

    // Se recorre la rejilla entera en trozos de una fila. La vista es densa por contrato —hay
    // exactamente filas por columnas celdas— así que esto no puede dejarse ninguna fuera, y no hay
    // ningún caso imposible que fingir que se trata.
    let ancho = tamano.cols as usize;
    for (fila, celdas) in vista.cells().chunks_exact(ancho).enumerate() {
        let fila = fila as u16;
        for (columna, celda) in celdas.iter().enumerate() {
            let (_, fondo) = colores_de(celda, tema);
            // El fondo por defecto ya lo pinta la ventana entera: repetirlo celda a celda es
            // trabajo por nada y, con el antialias activo, una costura por cada frontera.
            if fondo != fondo_de_la_ventana {
                painter.rect_filled(metrica.celda(origen, fila, columna as u16), 0.0, fondo);
            }
        }
    }

    for (fila, celdas) in vista.cells().chunks_exact(ancho).enumerate() {
        let fila = fila as u16;
        for (columna, celda) in celdas.iter().enumerate() {
            let hueco = metrica.celda(origen, fila, columna as u16);
            pintar_glifo(painter, celda, tema, metrica, hueco);
        }
    }

    pintar_cursor(painter, vista, tema, metrica, origen);
}

/// Los dos colores de una celda, ya con la intensidad aplicada donde corresponde.
///
/// **La negrita y el atenuado describen el color que el programa declaró como primer plano**, y con
/// vídeo inverso ese color es el que viaja en el fondo — el motor lo entrega intercambiado y marca
/// que lo está. Aplicar la intensidad siempre al primer plano parece lo natural y es el error:
/// atenúa un color que ya no se ve y deja el otro a plena intensidad.
fn colores_de(celda: &ViewCell, tema: &Tema) -> (Color32, Color32) {
    let (letra, fondo) = if celda.inverse {
        (
            tema.color(celda.fg),
            tema.con_atributos(celda.bg, celda.bold, celda.dim),
        )
    } else {
        (
            tema.con_atributos(celda.fg, celda.bold, celda.dim),
            tema.color(celda.bg),
        )
    };

    // **Lo seleccionado se realza intercambiando sus dos colores, y no con un color nuevo del tema.**
    // Es lo que hace cualquier terminal, y la razón es que **se lee siempre**: los dos colores de una
    // celda ya eran legibles el uno sobre el otro, así que intercambiarlos no puede dejar texto
    // ilegible. Un color de realce propio sí puede — y lo haría justo donde el programa ha elegido
    // colores raros, que es donde nadie lo probaría.
    //
    // **Va después del vídeo inverso y no antes**, así que sobre una celda ya invertida el realce la
    // devuelve a sus colores de origen. Es lo correcto y lo que se espera: seleccionar por encima del
    // cursor de `vim` lo enseña como el resto del texto en vez de dejarlo indistinguible.
    if celda.selected {
        (fondo, letra)
    } else {
        (letra, fondo)
    }
}

fn pintar_glifo(painter: &Painter, celda: &ViewCell, tema: &Tema, metrica: &Metrica, hueco: Rect) {
    // El acompañante de un carácter ancho **no pinta nada**. El glifo entero lo dibuja su primera
    // columna y se sale sobre esta; volver a pintarlo aquí lo duplicaría, medio encima del otro.
    // Y el hueco que deja un ancho que no cabía al final de la fila tampoco es un espacio: es un
    // sitio donde no hay carácter.
    if matches!(celda.width, CellWidth::WideTail | CellWidth::WrapFiller) {
        return;
    }
    // Un espacio no tiene nada que dibujar, y son la mayor parte de una pantalla de terminal.
    if celda.c == ' ' || celda.hidden {
        return;
    }

    let (color, _) = colores_de(celda, tema);
    // **No se recorta el glifo a su celda.** Si la fuente pinta más ancho de lo que el parser
    // reservó, tiene que verse: es el dato que decide si hay que embarcar una fuente propia.
    // Recortarlo escondería el desajuste y lo convertiría en una sorpresa mucho más cara.
    painter.text(
        hueco.left_top(),
        Align2::LEFT_TOP,
        celda.c,
        metrica.fuente.clone(),
        color,
    );
}

fn pintar_cursor(painter: &Painter, vista: &View, tema: &Tema, metrica: &Metrica, origen: Pos2) {
    // **Sin cursor no se pinta cursor**, y desde que hay historial ese caso ocurre de verdad: subiendo
    // lo bastante, el cursor se queda en el área activa, por debajo de lo que se mira. La foto lo dice
    // en vez de recortarlo, que es lo que este sitio anunciaba desde F1.3.
    //
    // **Y la guarda de abajo no cubría esto**, aunque su comentario dijera que sí: la fila llegaba
    // recortada, así que siempre estaba dentro de la rejilla y `cell` nunca devolvía nada. Se habría
    // pintado un cursor fantasma en la última fila.
    let Some(cursor) = vista.cursor() else {
        return;
    };
    if cursor.shape == CursorShape::Hidden {
        return;
    }
    let Some(debajo) = vista.cell(cursor.row, cursor.col) else {
        return;
    };
    let mut hueco = metrica.celda(origen, cursor.row, cursor.col);
    // Sobre un carácter ancho, el cursor cubre **las dos columnas** que ese carácter ocupa. Cubrir
    // solo una deja media letra tapada y la otra media no, que se lee como un error de dibujo.
    if debajo.width == CellWidth::WideHead {
        hueco.max.x += metrica.ancho;
    }
    let color = tema.color(TermColor::Cursor);
    // Sin parpadeo, y no por falta de tiempo: un cursor que parpadea es un temporizador, y un
    // temporizador es trabajo en reposo — justo lo que este hito mide y no puede permitirse
    // contaminar.
    match cursor.shape {
        CursorShape::Block => {
            painter.rect_filled(hueco, 0.0, color);
            // El bloque es opaco y va después de los glifos, así que se come el carácter que hay
            // debajo. Se repinta encima, en el color del fondo: es lo que hace un terminal, y sin
            // esto el cursor **borra** la letra sobre la que está en vez de resaltarla.
            if debajo.c != ' ' && !debajo.hidden {
                painter.text(
                    hueco.left_top(),
                    Align2::LEFT_TOP,
                    debajo.c,
                    metrica.fuente.clone(),
                    colores_de(debajo, tema).1,
                );
            }
        }
        CursorShape::HollowBlock => {
            painter.rect_stroke(hueco, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
        }
        CursorShape::Underline => {
            let alto = (metrica.alto * 0.1).max(1.0);
            let barra = Rect::from_min_size(
                hueco.left_bottom() - vec2(0.0, alto),
                vec2(metrica.ancho, alto),
            );
            painter.rect_filled(barra, 0.0, color);
        }
        CursorShape::Beam => {
            let ancho = (metrica.ancho * 0.15).max(1.0);
            let barra = Rect::from_min_size(hueco.left_top(), vec2(ancho, metrica.alto));
            painter.rect_filled(barra, 0.0, color);
        }
        CursorShape::Hidden => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use term_core::Screen;

    /// Una métrica de medidas redondas, para que las cuentas del test se lean solas. No pasa por
    /// la fuente a propósito: preguntarle exige una ventana, y lo que aquí se prueba es la cuenta.
    fn metrica(ancho: f32, alto: f32) -> Metrica {
        Metrica {
            ancho,
            alto,
            fuente: FontId::monospace(14.0),
        }
    }

    #[test]
    fn lo_seleccionado_se_pinta_con_los_dos_colores_intercambiados() {
        // **El realce se lee siempre porque no inventa un color**: intercambia los dos que la celda ya
        // tenía, que por fuerza eran legibles el uno sobre el otro. Se afirma sobre los **colores que
        // salen**, no sobre el campo de la celda: pintar el realce con el mismo fondo de siempre dejaría
        // la selección invisible y el campo puesto igual.
        let tema = Tema::default();
        let mut celda = ViewCell {
            fg: TermColor::Indexed(1),
            bg: TermColor::Indexed(4),
            ..ViewCell::default()
        };

        let (letra, fondo) = colores_de(&celda, &tema);
        celda.selected = true;
        let (letra_realzada, fondo_realzado) = colores_de(&celda, &tema);

        assert_eq!(
            (letra_realzada, fondo_realzado),
            (fondo, letra),
            "lo seleccionado no salió con sus dos colores intercambiados"
        );
        assert_ne!(
            fondo_realzado, fondo,
            "el fondo de la celda realzada es el mismo de antes: la selección no se vería"
        );
    }

    #[test]
    fn el_realce_intercambia_los_colores_ya_atenuados_y_no_al_reves() {
        // **La intensidad se aplica primero y el realce se intercambia después**, y el orden se nota
        // justo donde el programa pidió atenuado: es el color que él declaró como primer plano el que va
        // apagado, y con vídeo inverso ese color viaja en el fondo. Intercambiando antes de aplicar la
        // intensidad, lo apagado sería el color equivocado — el mismo defecto que una revisión ya
        // encontró en este pintor sin la selección de por medio.
        let tema = Tema::default();
        let celda = ViewCell {
            fg: TermColor::Indexed(1),
            bg: TermColor::Indexed(4),
            inverse: true,
            dim: true,
            ..ViewCell::default()
        };

        let (_, fondo_apagado) = colores_de(&celda, &tema);
        let (letra_realzada, _) = colores_de(
            &ViewCell {
                selected: true,
                ..celda
            },
            &tema,
        );

        assert_eq!(
            letra_realzada, fondo_apagado,
            "la letra de la celda realzada no es el color que iba atenuado: el realce se intercambió \
             antes de aplicar la intensidad"
        );
        assert_ne!(
            letra_realzada,
            tema.color(TermColor::Indexed(4)),
            "la letra salió a plena intensidad: el atenuado que el programa pidió se perdió"
        );
    }

    #[test]
    fn el_espacio_se_reparte_en_celdas_enteras() {
        let rejilla = metrica(10.0, 20.0).celdas_en(vec2(805.0, 419.0));

        // Las celdas que no caben enteras no cuentan: media columna pintada al borde sería medio
        // carácter cortado.
        assert_eq!(rejilla, Size { rows: 20, cols: 80 });
    }

    #[test]
    fn una_ventana_mas_estrecha_que_una_celda_sigue_dando_una_columna() {
        let rejilla = metrica(10.0, 20.0).celdas_en(vec2(3.0, 4.0));

        assert_eq!(rejilla, Size { rows: 1, cols: 1 });
    }

    #[test]
    fn el_rectangulo_de_reserva_de_egui_no_produce_una_rejilla_descomunal() {
        // Diez mil puntos de lado es lo que egui responde mientras el escritorio no ha dicho
        // cuánto mide la ventana. Sin techo eso son 743 000 celdas reservadas y un aviso de tamaño
        // absurdo al programa de dentro.
        let rejilla = metrica(8.4, 16.0).celdas_en(vec2(10_000.0, 10_000.0));

        assert_eq!(rejilla, MAXIMO);
    }

    #[test]
    fn ningun_espacio_produce_una_rejilla_que_la_pantalla_rechace() {
        // Este es el test que ata las dos piezas: si alguien quita el suelo, revienta aquí y no en
        // la mano del usuario mientras arrastra la ventana.
        let mut pantalla = Screen::new(Size { rows: 24, cols: 80 });

        for espacio in [
            vec2(0.0, 0.0),
            vec2(1.0, 1.0),
            vec2(-50.0, -50.0),
            vec2(f32::NAN, f32::NAN),
            vec2(f32::INFINITY, f32::INFINITY),
            vec2(1e9, 1e9),
        ] {
            let rejilla = metrica(10.0, 20.0).celdas_en(espacio);

            assert!(
                rejilla.rows >= 1 && rejilla.cols >= 1,
                "{espacio:?} produjo una rejilla que hace estallar la pantalla: {rejilla:?}"
            );
            assert!(
                rejilla.rows <= MAXIMO.rows && rejilla.cols <= MAXIMO.cols,
                "{espacio:?} se saltó el techo: {rejilla:?}"
            );
            pantalla.resize(rejilla);
        }
    }

    /// Lo que `pintar_cursor` deja dibujado, contado en formas.
    ///
    /// **Se cuenta lo que se PINTA, no lo que la foto dice.** Que la vista devuelva `None` es la mitad
    /// del criterio; la otra —la que se comprueba aquí— es que este pintor no dibuje nada con ese
    /// `None`. Se pinta sobre una capa propia y vacía, así que lo que salga es exactamente lo de este
    /// pintor y nada más.
    fn formas_del_cursor(vista: &View) -> usize {
        let ctx = Context::default();
        let salida = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("solo el cursor"),
            ));
            pintar_cursor(
                &painter,
                vista,
                &Tema::default(),
                &metrica(10.0, 20.0),
                Pos2::ZERO,
            );
        });
        let formas = salida.shapes.len();
        salida.drop_without_applying_deltas();
        formas
    }

    #[test]
    fn subido_el_historial_no_se_pinta_ningun_cursor() {
        // **La otra mitad de «el cursor no se ve», y no la ataba nadie.** Que la foto devuelva `None`
        // se comprueba en `term-core`; que este pintor no dibuje igualmente un bloque en la última
        // fila, no se comprobaba en ninguna parte. La guarda de más abajo —la que mira si la celda
        // existe— no cubría esto aunque su comentario dijera que sí: la fila llegaba recortada, así que
        // siempre caía dentro de la rejilla y se habría pintado un **cursor fantasma** en un sitio
        // donde el programa no escribe.
        let mut pantalla = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..40 {
            pantalla.advance(format!("linea {n}\r\n").as_bytes());
        }

        // Al día, el cursor se ve y se pinta: sin esto, un pintor que no dibujara nunca nada pasaría.
        assert!(
            formas_del_cursor(&pantalla.view()) > 0,
            "con la pantalla al día no se pintó ningún cursor, así que la comprobación de abajo no \
             distingue nada"
        );

        pantalla.desplazar(term_core::Desplazamiento::AlPrincipio);
        assert_eq!(
            pantalla.view().cursor(),
            None,
            "la prueba no llegó a dejar el cursor fuera de lo que se mira"
        );

        assert_eq!(
            formas_del_cursor(&pantalla.view()),
            0,
            "se pintó un cursor con la foto diciendo que no se ve: es un cursor fantasma en la última \
             fila, y quien mire creerá que el programa escribe ahí"
        );
    }
}
