//! Dibuja la rejilla del terminal: fondos, glifos y cursor.
//!
//! **Es a propósito el pintor más ingenuo que funciona**: una celda cada vez, sin caché y sin
//! seguimiento de lo que cambió. No es descuido ni un primer borrador — optimizar antes de medir
//! escondería el síntoma que este hito existe para observar, y el modo de fallo de una caché de
//! glifos son celdas fantasma, que es de los que no se ven hasta que ya llevas semanas encima.
//!
//! **La medida de la celda se pregunta, no se inventa.** El ancho sale del glifo de referencia de
//! la fuente y el alto de su altura de línea. Calcularlo a ojo es lo que descuadra las filas.

use crate::fonts::Presentation;
use crate::theme::Theme;
use eframe::egui::{
    Align2, Color32, Context, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, vec2,
};
use term_core::{CellWidth, CursorShape, Size, TermColor, View, ViewCell};

/// El tamaño más grande que se acepta para una rejilla.
///
/// Holgado a propósito: un monitor de 4K con la letra más pequeña que se puede leer no pasa de
/// unas 300 columnas. No es un límite de producto, es el filtro que impide que un rectángulo que
/// no corresponde a ninguna ventana se convierta en una rejilla de cientos de miles de celdas.
const CEILING: Size = Size {
    rows: 300,
    cols: 1000,
};

/// Cuánto ocupa una celda en pantalla.
#[derive(Debug, Clone)]
pub struct CellMetrics {
    /// Privados juntos: el ancho y el alto son medidas **de esta fuente**. Con los tres campos
    /// abiertos, cambiar uno solo deja una métrica que mide una fuente y pinta con otra, y la
    /// rejilla se descuadra sin que nada falle — el mismo motivo por el que la vista de celdas
    /// guarda su tamaño en privado.
    width: f32,
    height: f32,
    font: FontId,
}

impl CellMetrics {
    /// Pregunta a la fuente cuánto mide una celda.
    ///
    /// El ancho se toma de un glifo cualquiera **de ancho fijo**: en una fuente monoespaciada todos
    /// miden lo mismo, así que preguntar por uno vale por todos. Si algún día se admite una fuente
    /// que no lo sea, este es el sitio donde se romperá — y romperse aquí es mejor que descuadrarse
    /// en pantalla sin avisar.
    ///
    /// **Se pregunta en cada pasada, no se guarda.** Al cambiar el aumento del escritorio la fuente
    /// cambia de tamaño, y una medida guardada descuadraría la rejilla entera sin avisar.
    pub fn measure(ctx: &Context, font: FontId) -> Self {
        let (width, height) =
            ctx.fonts_mut(|fonts| (fonts.glyph_width(&font, 'M'), fonts.row_height(&font)));
        Self {
            width,
            height,
            font,
        }
    }

    /// Lo que ocupa una celda de ancho.
    ///
    /// Se puede leer, pero no fijar: quien mide una rejilla necesita este número —para colocar dos
    /// consolas seguidas, o para saber si caben en la ventana— y calcularlo por su cuenta sería
    /// justo lo que esta caja evita, medir con una fuente y pintar con otra.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Lo que ocupa una celda de alto. Se lee por lo mismo que [`CellMetrics::width`].
    pub fn height(&self) -> f32 {
        self.height
    }

    /// La fuente con la que se midió, para quien pinte texto que tenga que cuadrar con esta rejilla.
    ///
    /// Se clona en vez de prestarse porque `Painter::text` la consume, y un `FontId` son dos campos.
    /// Sigue sin poder fijarse: medir con una y pintar con otra es lo que esta caja existe para
    /// impedir.
    pub fn font(&self) -> FontId {
        self.font.clone()
    }

    /// Cuántas celdas enteras caben en el espacio dado.
    ///
    /// **Nunca menos de una fila y una columna, y nunca más de [`CEILING`].** El suelo está porque
    /// `Screen::resize` entra en pánico con cero columnas: sin él, arrastrar el borde de la ventana
    /// hasta hacerla estrecha mataría la aplicación. El techo está porque el espacio disponible no
    /// siempre es el de una ventana de verdad — mientras el escritorio no ha dicho todavía cuánto
    /// mide, egui responde con su rectángulo de reserva, que son diez mil puntos de lado. Sin
    /// techo, eso son cientos de miles de celdas reservadas y un aviso de tamaño absurdo al
    /// programa de dentro.
    pub fn cells_in(&self, space: Vec2) -> Size {
        // Los tamaños imposibles —negativos, o sin número mientras la ventana se crea— caen en
        // cero al convertirse a entero, y el suelo los sube a uno.
        Size {
            rows: ((space.y / self.height) as u16).clamp(1, CEILING.rows),
            cols: ((space.x / self.width) as u16).clamp(1, CEILING.cols),
        }
    }

    /// El rectángulo que ocupa la celda de una fila y columna, desde el origen dado.
    fn cell(&self, origin: Pos2, row: u16, column: u16) -> Rect {
        let corner = origin + vec2(column as f32 * self.width, row as f32 * self.height);
        Rect::from_min_size(corner, vec2(self.width, self.height))
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
pub fn paint(
    painter: &Painter,
    view: &View,
    theme: &Theme,
    metrics: &CellMetrics,
    origin: Pos2,
    discoverer: &mut crate::fonts::Discoverer,
) {
    let grid_size = view.size();
    // El fondo por defecto es el mismo para toda la pantalla: preguntarlo una vez por celda serían
    // casi dos mil consultas por pasada para obtener siempre lo mismo.
    let window_background = theme.background();

    // Se recorre la rejilla entera en trozos de una fila. La vista es densa por contrato —hay
    // exactamente filas por columnas celdas— así que esto no puede dejarse ninguna fuera, y no hay
    // ningún caso imposible que fingir que se trata.
    let width = grid_size.cols as usize;
    for (row, cells) in view.cells().chunks_exact(width).enumerate() {
        let row = row as u16;
        for (column, cell) in cells.iter().enumerate() {
            let (_, background) = colors_of(cell, theme);
            // El fondo por defecto ya lo pinta la ventana entera: repetirlo celda a celda es
            // trabajo por nada y, con el antialias activo, una costura por cada frontera.
            if background != window_background {
                painter.rect_filled(metrics.cell(origin, row, column as u16), 0.0, background);
            }
        }
    }

    // **Los glifos se pintan en DOS pasadas —primero los de texto, después los de color— y eso no es
    // organización: es lo que decide cuánto cuesta dibujar el fotograma.** El motor solo puede juntar en
    // el mismo lote de dibujo formas que compartan textura; los emojis viven en su atlas y las letras en
    // el de fuentes, así que pintarlos entremezclados en el orden de la rejilla parte el lote **en cada
    // emoji**. Ver **D25**, que trae la medida.
    //
    // **El coste de la pasada de más es recorrer la rejilla otra vez**, unos microsegundos: la del color
    // descarta con una comparación todo lo que no va a color, que es casi todo.
    // **Los glifos se pintan en DOS pasadas —primero los de texto, después los de color— y eso no es
    // organización: es lo que decide cuánto cuesta dibujar el fotograma.** El motor solo puede juntar en
    // el mismo lote de dibujo formas que compartan textura; los emojis viven en su atlas y las letras en
    // el de fuentes, así que pintarlos entremezclados en el orden de la rejilla parte el lote **en cada
    // emoji**. Ver **D25**, que trae la medida.
    //
    // **El coste de la pasada de más es recorrer la rejilla otra vez**, unos microsegundos: la del color
    // descarta con una comparación todo lo que no va a color, que es casi todo.
    // **Y la segunda pasada solo ocurre si la primera vio algún emoji.** Una pantalla sin ninguno —que es
    // la inmensa mayoría— recorrería la rejilla entera para no pintar nada. Con ASCII el atajo de abajo la
    // dejaría en casi nada de todos modos, pero con japonés en pantalla no hay atajo que valga: cada celda
    // consultaría las tablas de Unicode por segunda vez.
    //
    // **Lo que ahorra está por debajo de lo que el banco puede distinguir, y se dice así**: dos corridas de
    // la misma tanda con japonés dan 280 y 301 µs en el tramo del pintado, así que cualquier diferencia
    // menor que esos 21 µs no se puede afirmar. Se hace por la forma —no recorrer la rejilla para no pintar
    // nada— y no porque haya un número detrás.
    let mut there_was_color = false;
    for pass in [Presentation::Text, Presentation::Color] {
        if pass == Presentation::Color && !there_was_color {
            break;
        }
        for (row, cells) in view.cells().chunks_exact(width).enumerate() {
            let row = row as u16;
            for (column, cell) in cells.iter().enumerate() {
                // **Apuntar es lo único que corre por celda, y por eso tiene que ser barato**: un
                // carácter ASCII o ya resuelto se descarta sin tocar nada. Buscar la fuente y registrarla
                // ocurren fuera de esta pasada — ver `Discoverer::resolve`. Va solo en la primera de
                // las dos: apuntar dos veces lo mismo no cambia nada, pero se pagaría dos veces.
                if pass == Presentation::Text {
                    discoverer.note(cell.c);
                }
                // El acompañante de un carácter ancho **no pinta nada**. El glifo entero lo dibuja su
                // primera columna y se sale sobre esta; volver a pintarlo aquí lo duplicaría, medio
                // encima del otro. Y el hueco que deja un ancho que no cabía al final de la fila tampoco
                // es un espacio: es un sitio donde no hay carácter.
                if matches!(cell.width, CellWidth::WideTail | CellWidth::WrapFiller) {
                    continue;
                }
                // Un espacio no tiene nada que dibujar, y son la mayor parte de una pantalla.
                if cell.c == ' ' || cell.hidden {
                    continue;
                }
                let attached = view.combining_of(cell);
                // **Un ASCII SIN NADA PEGADO se descarta de la pasada del color sin consultar las tablas
                // de Unicode.** Es la inmensa mayoría de lo que pasa por una pantalla de terminal.
                //
                // **Lo que este atajo ahorra NO se distingue del ruido entre corridas** —366 µs contra
                // 377 en el tramo del pintado— y queda dicho así en vez de atribuirle un número que no
                // le corresponde: los 92 µs que cuesta la pasada de más los paga **recorrer la rejilla
                // otra vez**, y eso el atajo no lo evita. Lo que sí evita es consultar las tablas de
                // Unicode en casi todas las celdas, que es un argumento de forma y no una medida.
                //
                // **Lo de «sin nada pegado» no es prudencia, es obligatorio**: `#`, `*` y los dígitos
                // tienen `Emoji=Yes`, así que `#` seguido del selector es `#️⃣` y va a color. La primera
                // versión de este atajo descartaba todo el ASCII y se los comía; lo destapó la prueba que
                // recorre los 128 con los dos selectores detrás, escrita justamente porque duplicar aquí
                // una regla que vive en `fonts` solo es aceptable si algo avisa cuando dejan de decir
                // lo mismo.
                if pass == Presentation::Color && cell.c.is_ascii() && attached.is_empty() {
                    continue;
                }
                // Cada celda se pinta en la pasada que le toca por su presentación, y en la otra no.
                if crate::fonts::presentation_of(cell.c, attached) != pass {
                    // Si en la pasada del texto una celda pide color, la del color tiene trabajo.
                    there_was_color |= pass == Presentation::Text;
                    continue;
                }
                paint_character(
                    painter,
                    cell.c,
                    attached,
                    metrics.cell(origin, row, column as u16),
                    cell.width == CellWidth::WideHead,
                    colors_of(cell, theme).0,
                    metrics,
                    discoverer,
                );
            }
        }
    }

    paint_cursor(painter, view, theme, metrics, origin, discoverer);
}

/// Los dos colores de una celda, ya con la intensidad aplicada donde corresponde.
///
/// **La negrita y el atenuado describen el color que el programa declaró como primer plano**, y con
/// vídeo inverso ese color es el que viaja en el fondo — el motor lo entrega intercambiado y marca
/// que lo está. Aplicar la intensidad siempre al primer plano parece lo natural y es el error:
/// atenúa un color que ya no se ve y deja el otro a plena intensidad.
fn colors_of(cell: &ViewCell, theme: &Theme) -> (Color32, Color32) {
    let (foreground, background) = if cell.inverse {
        (
            theme.color(cell.fg),
            theme.with_attributes(cell.bg, cell.bold, cell.dim),
        )
    } else {
        (
            theme.with_attributes(cell.fg, cell.bold, cell.dim),
            theme.color(cell.bg),
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
    if cell.selected {
        (background, foreground)
    } else {
        (foreground, background)
    }
}

/// Pinta un carácter en su hueco: a color si le toca color, y por el camino de texto si no.
///
/// **Existe como función aparte porque el cursor de bloque repinta el carácter que tapa**, y sin esto
/// ese repintado no sabría de emojis: un emoji bajo el cursor saldría en monocromo o no saldría.
#[expect(
    clippy::too_many_arguments,
    reason = "    Son los datos que hacen falta para pintar un carácter, y ninguno se deduce de otro: el carácter, lo \
    que lleva pegado —que decide la presentación—, dónde va, si ocupa dos columnas, de qué color si es \
    texto, con qué tamaño, quién sabe de fuentes y dónde se recuerdan los glifos de color. Agruparlos en \
    un tipo sería una caja que no significa nada fuera de esta llamada."
)]
fn paint_character(
    painter: &Painter,
    character: char,
    attached: &[char],
    slot: Rect,
    is_wide: bool,
    color: Color32,
    metrics: &CellMetrics,
    discoverer: &mut crate::fonts::Discoverer,
) {
    // **Un emoji ancho ocupa las DOS columnas que el motor le reservó**, no una. El hueco que llega es
    // el de su primera columna; el dibujo se centra en el de las dos, que es lo que su anchura declara.
    let its_own = if is_wide {
        Rect::from_min_max(slot.min, slot.max + vec2(metrics.width, 0.0))
    } else {
        slot
    };
    if crate::fonts::presentation_of(character, attached) == crate::fonts::Presentation::Color
        && paint_in_color(painter, character, its_own, discoverer)
    {
        return;
    }
    // **No se recorta el glifo a su celda.** Si la fuente pinta más ancho de lo que el parser
    // reservó, tiene que verse: es el dato que decide si hay que embarcar una fuente propia.
    // Recortarlo escondería el desajuste y lo convertiría en una sorpresa mucho más cara.
    //
    // **La familia se elige por carácter, y ahí está el único sitio donde se puede elegir.** egui ordena
    // su cadena de reserva por familia entera, no por rango: para que la fuente de iconos que el usuario
    // instaló gane en las zonas de uso privado **sin** quitarle las letras a la del terminal, tiene que
    // vivir en su propia familia y pedirse solo donde toca. Quien recorre celda a celda es esto.
    let font = eframe::egui::FontId::new(metrics.font.size, discoverer.family_of(character));
    painter.text(slot.left_top(), Align2::LEFT_TOP, character, font, color);
}

/// Pinta el glifo de color de ese carácter, centrado en su hueco. Dice si llegó a pintar algo.
///
/// **Devolver `false` no es un fallo: es el Contrato 5 del diseño.** Un carácter cuya presentación pide
/// color pero que ninguna fuente de la máquina pinta a color se dibuja con lo que dé el motor —su
/// silueta monocroma, o su cuadro— y no se intenta nada más. El terminal no dibuja glifos que la máquina
/// no tiene.
fn paint_in_color(
    painter: &Painter,
    character: char,
    slot: Rect,
    discoverer: &mut crate::fonts::Discoverer,
) -> bool {
    // **En píxeles FÍSICOS, no en puntos.** La textura se sube al tamaño con el que se va a ver: pedirla
    // en puntos la dejaría a la mitad de resolución en una pantalla al 200 % y el emoji saldría borroso
    // justo donde más se nota.
    let per_point = painter.ctx().pixels_per_point();
    let height = (slot.height() * per_point).round() as u32;
    let width = (slot.width() * per_point).round() as u32;
    let Some((atlas, uv, measurements)) =
        discoverer.color_glyph(painter.ctx(), character, height, width)
    else {
        return false;
    };
    // **Centrado en su hueco, y por eso hace falta el tamaño real de la textura**: el reescalado conserva
    // la proporción del dibujo, así que casi nunca llena el hueco exacto — sobra en un eje. Estirarlo
    // para que llene deformaría el emoji.
    let drawing = Rect::from_center_size(slot.center(), measurements / per_point);
    painter.image(atlas, drawing, uv, Color32::WHITE);
    true
}

fn paint_cursor(
    painter: &Painter,
    view: &View,
    theme: &Theme,
    metrics: &CellMetrics,
    origin: Pos2,
    discoverer: &mut crate::fonts::Discoverer,
) {
    // **Sin cursor no se pinta cursor**, y desde que hay historial ese caso ocurre de verdad: subiendo
    // lo bastante, el cursor se queda en el área activa, por debajo de lo que se mira. La foto lo dice
    // en vez de recortarlo, que es lo que este sitio anunciaba desde F1.3.
    //
    // **Y la guarda de abajo no cubría esto**, aunque su comentario dijera que sí: la fila llegaba
    // recortada, así que siempre estaba dentro de la rejilla y `cell` nunca devolvía nada. Se habría
    // pintado un cursor fantasma en la última fila.
    let Some(cursor) = view.cursor() else {
        return;
    };
    if cursor.shape == CursorShape::Hidden {
        return;
    }
    let Some(under_cursor) = view.cell(cursor.row, cursor.col) else {
        return;
    };
    let mut slot = metrics.cell(origin, cursor.row, cursor.col);
    // Sobre un carácter ancho, el cursor cubre **las dos columnas** que ese carácter ocupa. Cubrir
    // solo una deja media letra tapada y la otra media no, que se lee como un error de dibujo.
    if under_cursor.width == CellWidth::WideHead {
        slot.max.x += metrics.width;
    }
    let color = theme.color(TermColor::Cursor);
    // Sin parpadeo, y no por falta de tiempo: un cursor que parpadea es un temporizador, y un
    // temporizador es trabajo en reposo — justo lo que este hito mide y no puede permitirse
    // contaminar.
    match cursor.shape {
        CursorShape::Block => {
            painter.rect_filled(slot, 0.0, color);
            // El bloque es opaco y va después de los glifos, así que se come el carácter que hay
            // debajo. Se repinta encima, en el color del fondo: es lo que hace un terminal, y sin
            // esto el cursor **borra** la letra sobre la que está en vez de resaltarla.
            if under_cursor.c != ' ' && !under_cursor.hidden {
                // **Por el mismo camino que el pintado normal, y no con un `painter.text` propio.** Sin
                // esto, el repintado no sabría ni de la familia de iconos —un separador de Powerline
                // cambiaba de símbolo al pasarle el cursor por encima, lo encontró una revisión— ni de
                // los emojis a color, que saldrían en monocromo solo bajo el cursor.
                //
                // **Y el emoji se repinta a color tal cual, no teñido del color del fondo.** Un dibujo a
                // color no se puede recolorear a un solo tono sin dejarlo en una mancha plana: lo que se
                // ve encima del bloque es el emoji, que es lo que hacen los demás terminales y lo único
                // que se lee.
                paint_character(
                    painter,
                    under_cursor.c,
                    view.combining_of(under_cursor),
                    metrics.cell(origin, cursor.row, cursor.col),
                    under_cursor.width == CellWidth::WideHead,
                    colors_of(under_cursor, theme).1,
                    metrics,
                    discoverer,
                );
            }
        }
        CursorShape::HollowBlock => {
            painter.rect_stroke(slot, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
        }
        CursorShape::Underline => {
            let height = (metrics.height * 0.1).max(1.0);
            let bar = Rect::from_min_size(
                slot.left_bottom() - vec2(0.0, height),
                vec2(metrics.width, height),
            );
            painter.rect_filled(bar, 0.0, color);
        }
        CursorShape::Beam => {
            let width = (metrics.width * 0.15).max(1.0);
            let bar = Rect::from_min_size(slot.left_top(), vec2(width, metrics.height));
            painter.rect_filled(bar, 0.0, color);
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
    fn metrics(width: f32, height: f32) -> CellMetrics {
        CellMetrics {
            width,
            height,
            font: FontId::monospace(14.0),
        }
    }

    #[test]
    fn lo_seleccionado_se_pinta_con_los_dos_colores_intercambiados() {
        // **El realce se lee siempre porque no inventa un color**: intercambia los dos que la celda ya
        // tenía, que por fuerza eran legibles el uno sobre el otro. Se afirma sobre los **colores que
        // salen**, no sobre el campo de la celda: pintar el realce con el mismo fondo de siempre dejaría
        // la selección invisible y el campo puesto igual.
        let theme = Theme::default();
        let mut cell = ViewCell {
            fg: TermColor::Indexed(1),
            bg: TermColor::Indexed(4),
            ..ViewCell::default()
        };

        let (foreground, background) = colors_of(&cell, &theme);
        cell.selected = true;
        let (selected_foreground, selected_background) = colors_of(&cell, &theme);

        assert_eq!(
            (selected_foreground, selected_background),
            (background, foreground),
            "lo seleccionado no salió con sus dos colores intercambiados"
        );
        assert_ne!(
            selected_background, background,
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
        let theme = Theme::default();
        let cell = ViewCell {
            fg: TermColor::Indexed(1),
            bg: TermColor::Indexed(4),
            inverse: true,
            dim: true,
            ..ViewCell::default()
        };

        let (_, dim_background) = colors_of(&cell, &theme);
        let (selected_foreground, _) = colors_of(
            &ViewCell {
                selected: true,
                ..cell
            },
            &theme,
        );

        assert_eq!(
            selected_foreground, dim_background,
            "la letra de la celda realzada no es el color que iba atenuado: el realce se intercambió \
             antes de aplicar la intensidad"
        );
        assert_ne!(
            selected_foreground,
            theme.color(TermColor::Indexed(4)),
            "la letra salió a plena intensidad: el atenuado que el programa pidió se perdió"
        );
    }

    #[test]
    fn el_espacio_se_reparte_en_celdas_enteras() {
        let grid = metrics(10.0, 20.0).cells_in(vec2(805.0, 419.0));

        // Las celdas que no caben enteras no cuentan: media columna pintada al borde sería medio
        // carácter cortado.
        assert_eq!(grid, Size { rows: 20, cols: 80 });
    }

    #[test]
    fn el_cuerpo_de_letra_mas_grande_que_se_admite_no_deja_una_rejilla_imposible() {
        // **Desde que el cuerpo se configura hay un camino nuevo hasta esta guarda**, y no es el que la
        // motivó: antes solo se llegaba estrechando la ventana, ahora basta con pedir letra grande. Con el
        // máximo que se admite —96 puntos— la celda mide 58×112, así que una división de una ventana
        // pequeña se queda sin sitio para una sola columna.
        //
        // Comprobado ejecutándolo: la rejilla sale **1×1** y no 0×1. Una consola de una celda es inútil
        // pero está viva; una de cero columnas es una división entre cero esperando a ocurrir.
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();

        let mut grids = Vec::new();
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrics = CellMetrics::measure(ui.ctx(), eframe::egui::FontId::monospace(96.0));
            for slot in [vec2(320.0, 200.0), vec2(40.0, 30.0), vec2(0.0, 0.0)] {
                grids.push(metrics.cells_in(slot));
            }
        })
        .drop_without_applying_deltas();

        for grid in grids {
            assert!(
                grid.cols >= 1 && grid.rows >= 1,
                "con el cuerpo máximo la rejilla salió {}x{}: una consola de cero columnas es una \
                 división entre cero esperando a ocurrir",
                grid.cols,
                grid.rows
            );
        }
    }

    #[test]
    fn una_ventana_mas_estrecha_que_una_celda_sigue_dando_una_columna() {
        let grid = metrics(10.0, 20.0).cells_in(vec2(3.0, 4.0));

        assert_eq!(grid, Size { rows: 1, cols: 1 });
    }

    #[test]
    fn el_rectangulo_de_reserva_de_egui_no_produce_una_rejilla_descomunal() {
        // Diez mil puntos de lado es lo que egui responde mientras el escritorio no ha dicho
        // cuánto mide la ventana. Sin techo eso son 743 000 celdas reservadas y un aviso de tamaño
        // absurdo al programa de dentro.
        let grid = metrics(8.4, 16.0).cells_in(vec2(10_000.0, 10_000.0));

        assert_eq!(grid, CEILING);
    }

    #[test]
    fn ningun_espacio_produce_una_rejilla_que_la_pantalla_rechace() {
        // Este es el test que ata las dos piezas: si alguien quita el suelo, revienta aquí y no en
        // la mano del usuario mientras arrastra la ventana.
        let mut screen = Screen::new(Size { rows: 24, cols: 80 });

        for space in [
            vec2(0.0, 0.0),
            vec2(1.0, 1.0),
            vec2(-50.0, -50.0),
            vec2(f32::NAN, f32::NAN),
            vec2(f32::INFINITY, f32::INFINITY),
            vec2(1e9, 1e9),
        ] {
            let grid = metrics(10.0, 20.0).cells_in(space);

            assert!(
                grid.rows >= 1 && grid.cols >= 1,
                "{space:?} produjo una rejilla que hace estallar la pantalla: {grid:?}"
            );
            assert!(
                grid.rows <= CEILING.rows && grid.cols <= CEILING.cols,
                "{space:?} se saltó el techo: {grid:?}"
            );
            screen.resize(grid);
        }
    }

    /// Lo que `paint_cursor` deja dibujado, contado en formas.
    ///
    /// **Se cuenta lo que se PINTA, no lo que la foto dice.** Que la vista devuelva `None` es la mitad
    /// del criterio; la otra —la que se comprueba aquí— es que este pintor no dibuje nada con ese
    /// `None`. Se pinta sobre una capa propia y vacía, así que lo que salga es exactamente lo de este
    /// pintor y nada más.
    /// Cuántas imágenes y cuántos textos encola el pintor para esa pantalla.
    ///
    /// **Es el único oráculo que dice si un emoji salió a color**, y por eso las pruebas de aquí cuentan
    /// formas en vez de mirar campos. Un emoji a color es una **imagen** en la lista de formas; uno
    /// monocromo es un **texto**. Afirmar sobre lo que decidió la regla —o sobre si la caché tiene una
    /// entrada— pasaría igual con el pintor devolviendo `false` siempre.
    fn shapes_of(screen: &Screen) -> (usize, usize) {
        shapes_of_with(screen, &mut crate::fonts::Discoverer::default())
    }

    /// Lo mismo, pero con un descubridor que la prueba ha dejado en el estado que quiere mirar.
    fn shapes_of_with(
        screen: &Screen,
        discoverer: &mut crate::fonts::Discoverer,
    ) -> (usize, usize) {
        let ctx = Context::default();
        // **Una pasada en vacío antes**, porque el motor de texto no existe hasta que la hay: sin esto
        // la primera maquetación ocurre a mitad de la pasada que se está midiendo.
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let view = screen.view();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("la rejilla"),
            ));
            paint(
                &painter,
                &view,
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                discoverer,
            );
        });
        let (mut images, mut texts) = (0, 0);
        for shape in &output.shapes {
            count(&shape.shape, &mut images, &mut texts);
        }
        output.drop_without_applying_deltas();
        (images, texts)
    }

    /// **Una imagen encolada es una malla con una textura que NO es el atlas de fuentes.** `Painter::image`
    /// no produce una variante propia: arma una malla de dos triángulos con la textura pedida —leído en
    /// `epaint/shapes/shape.rs:373`—. Distinguirla por su textura es lo que impide contar como imagen
    /// cualquier malla que el marco pinte por su cuenta.
    fn count(shape: &eframe::egui::Shape, images: &mut usize, texts: &mut usize) {
        match shape {
            eframe::egui::Shape::Mesh(mesh)
                if mesh.texture_id != eframe::egui::TextureId::default() =>
            {
                *images += 1;
            }
            eframe::egui::Shape::Text(_) => *texts += 1,
            eframe::egui::Shape::Vec(many) => {
                for one in many {
                    count(one, images, texts);
                }
            }
            _ => {}
        }
    }

    /// Una pantalla de una sola línea con ese texto dentro.
    fn of_text(text: &str) -> Screen {
        let mut screen = Screen::new(Size { rows: 1, cols: 20 });
        screen.advance(text.as_bytes());
        screen
    }

    /// Si esta máquina tiene con qué pintar emojis a color. Sin eso, medio hito no es comprobable aquí.
    fn there_is_color_on_this_machine() -> bool {
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        crate::fonts::Discoverer::default()
            .color_glyph(&ctx, '\u{1F600}', 20, 20)
            .is_some()
    }

    #[test]
    fn un_emoji_de_los_de_toda_la_vida_sale_a_color_sin_que_nadie_lo_pida() {
        // **AC3, y es la mitad visible del hito.** `😀` tiene `Emoji_Presentation=Yes`, así que va a
        // color sin selector ninguno.
        //
        // **Se afirma en los DOS escenarios**: donde la máquina tiene fuente de color, el emoji tiene que
        // salir como imagen; donde no la tiene, tiene que seguir saliendo por el camino de texto y la
        // ventana quedar en pie. Saltar la prueba en una máquina pelada dejaría sin cubrir justo el
        // camino donde se cae una implementación.
        let (images, texts) = shapes_of(&of_text("\u{1F600}"));
        if there_is_color_on_this_machine() {
            assert_eq!(
                images, 1,
                "el emoji no se pintó como imagen: salió por el camino de texto, o sea en monocromo"
            );
        } else {
            assert_eq!(
                images, 0,
                "no hay fuente de color en la máquina y aun así se pintó una imagen"
            );
            assert_eq!(
                texts, 1,
                "sin fuente de color el emoji tiene que salir igualmente, en monocromo"
            );
        }
    }

    #[test]
    fn el_simbolo_de_texto_se_queda_en_texto_y_con_su_selector_se_va_a_color() {
        // **AC4, y las dos mitades en la misma prueba a propósito.** `☁` y `☁️` son **el mismo carácter**
        // con presentación distinta: comprobar solo una de las dos pasaría con un pintor que mandara todo
        // a color, o todo a texto.
        let (no_selector, texts_without) = shapes_of(&of_text("\u{2601}"));
        assert_eq!(
            no_selector, 0,
            "una nube sin selector se pintó a color: es un símbolo de texto y así se pierde"
        );
        assert_eq!(texts_without, 1, "la nube no se pintó de ninguna manera");

        let (with_selector, _) = shapes_of(&of_text("\u{2601}\u{FE0F}"));
        if there_is_color_on_this_machine() {
            assert_eq!(
                with_selector, 1,
                "la nube con selector de emoji no salió a color: el selector no está llegando a la regla"
            );
        }
    }

    #[test]
    fn una_tecla_hecha_con_una_almohadilla_sale_a_color() {
        // **El caso que el atajo del ASCII se comió, y por eso está aquí y no solo en la regla.** La
        // pasada del color descarta el ASCII sin consultar las tablas de Unicode, porque es casi todo lo
        // que pasa por una pantalla; pero `#`, `*` y los dígitos tienen `Emoji=Yes`, así que con el
        // selector detrás son teclas y van a color.
        //
        // La prueba de la regla, en `fonts`, no cubre esto: el atajo vive **aquí**, y mutarlo para que
        // se comiera todo el ASCII dejaba aquella en verde. Se comprobó mutándolo.
        if !there_is_color_on_this_machine() {
            return;
        }
        let (images, _) = shapes_of(&of_text("#\u{FE0F}\u{20E3}"));
        assert_eq!(
            images, 1,
            "la tecla «#️⃣» salió por el camino de texto: quien pinta está descartando el ASCII sin \
             mirar lo que lleva pegado, y se come las once teclas que Unicode hace con ASCII"
        );
    }

    #[test]
    fn el_selector_de_texto_devuelve_a_monocromo_un_emoji_que_iria_a_color() {
        // La otra mitad de **AC4**, la que nadie escribe: el selector que **quita** el color. Sin ella, un
        // pintor que ignorase `U+FE0E` pasaría las dos pruebas de arriba.
        let (images, texts) = shapes_of(&of_text("\u{1F600}\u{FE0E}"));
        assert_eq!(
            images, 0,
            "el emoji salió a color llevando detrás el selector de TEXTO, que es justo lo que lo prohíbe"
        );
        assert_eq!(
            texts, 1,
            "con el selector de texto el emoji tiene que salir igual, en monocromo"
        );
    }

    #[test]
    fn el_emoji_no_invade_la_celda_de_al_lado() {
        // **AC5.** El mapa de bits de `Noto Color Emoji` es **más ancho que alto** —136×128, comprobado
        // ejecutándolo— así que escalarlo por la altura de la celda lo deja más ancho que las dos columnas
        // que el motor le reservó, y se come el borde de su vecina.
        //
        // **Se afirma sobre el rectángulo que se encoló**, que es lo que se ve, y no sobre el tamaño de la
        // textura: una textura del tamaño correcto pintada en un rectángulo estirado sale igual de mal.
        if !there_is_color_on_this_machine() {
            return;
        }
        let screen = of_text("\u{1F600}");
        let view = screen.view();
        let metrics = metrics(10.0, 20.0);
        assert_eq!(
            view.cell(0, 0).map(|cell| cell.width),
            Some(CellWidth::WideHead),
            "el motor no le dio dos columnas al emoji: la prueba no está midiendo lo que dice"
        );

        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("un emoji"),
            ));
            paint(
                &painter,
                &view,
                &Theme::default(),
                &metrics,
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
            );
        });
        let mut drawing = None;
        for shape in &output.shapes {
            if let eframe::egui::Shape::Mesh(mesh) = &shape.shape
                && mesh.texture_id != eframe::egui::TextureId::default()
            {
                drawing = Some(mesh.calc_bounds());
            }
        }
        output.drop_without_applying_deltas();
        let drawing = drawing.expect("el emoji no se pintó como imagen");

        // Las dos columnas que su anchura declara, y ni un punto más.
        let its_own = Rect::from_min_size(Pos2::ZERO, vec2(metrics.width * 2.0, metrics.height));
        assert!(
            its_own.contains_rect(drawing),
            "el emoji ocupa {drawing:?} y sus dos columnas son {its_own:?}: se sale sobre la celda vecina"
        );
        // **Y que no se haya deformado para llenarlas.** El margen es ceñido a propósito: con uno de una
        // décima, estirar el emoji al hueco entero —proporción 1,000 contra la suya, 1,062— pasaba esta
        // prueba. Se comprobó mutándolo, y es el disfraz que este proyecto tiene fichado: un margen
        // elegido con el que el defecto y el arreglo dan el mismo resultado.
        let ratio = drawing.width() / drawing.height();
        assert!(
            (ratio - 136.0 / 128.0).abs() < 0.03,
            "el emoji se pintó con proporción {ratio:.3} en vez de la suya (1,062): está estirado"
        );
        // **Y la otra cara de lo mismo, que es la que se ve**: conservar la proporción dentro de un hueco
        // que no la tiene obliga a que sobre sitio en uno de los dos ejes. Un dibujo que llena el hueco
        // exacto está estirado por definición.
        assert!(
            drawing.width() < its_own.width() || drawing.height() < its_own.height(),
            "el emoji llena su hueco exacto ({drawing:?} contra {its_own:?}): está estirado para llenarlo"
        );
    }

    #[test]
    fn bajo_el_cursor_el_emoji_se_sigue_viendo_a_color() {
        // **El cursor de bloque tapa el carácter y lo repinta encima.** Ese repintado tenía su propia
        // llamada al motor de texto, así que un emoji bajo el cursor salía en monocromo —solo ahí, y solo
        // mientras el cursor estuviera encima—. Es el mismo defecto que una revisión encontró con los
        // iconos de Powerline, en el mismo sitio.
        if !there_is_color_on_this_machine() {
            return;
        }
        let mut screen = Screen::new(Size { rows: 1, cols: 20 });
        screen.advance("\u{1F600}".as_bytes());
        screen.advance(b"\x1b[1;1H");
        assert_eq!(
            screen.view().cursor().map(|c| (c.row, c.col)),
            Some((0, 0)),
            "el cursor no quedó sobre el emoji: la prueba no está midiendo lo que dice"
        );
        let (images, _) = shapes_of(&screen);
        assert_eq!(
            images, 2,
            "con el cursor encima tiene que haber dos imágenes —la del pintado y la del repintado del \
             cursor—: si hay una, el cursor está tapando el emoji y repintándolo en monocromo"
        );
    }

    fn cursor_shapes(view: &View) -> usize {
        let ctx = Context::default();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("solo el cursor"),
            ));
            paint_cursor(
                &painter,
                view,
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
            );
        });
        let shapes = output.shapes.len();
        output.drop_without_applying_deltas();
        shapes
    }

    #[test]
    fn subido_el_historial_no_se_pinta_ningun_cursor() {
        // **La otra mitad de «el cursor no se ve», y no la ataba nadie.** Que la foto devuelva `None`
        // se comprueba en `term-core`; que este pintor no dibuje igualmente un bloque en la última
        // fila, no se comprobaba en ninguna parte. La guarda de más abajo —la que mira si la celda
        // existe— no cubría esto aunque su comentario dijera que sí: la fila llegaba recortada, así que
        // siempre caía dentro de la rejilla y se habría pintado un **cursor fantasma** en un sitio
        // donde el programa no escribe.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..40 {
            screen.advance(format!("linea {n}\r\n").as_bytes());
        }

        // Al día, el cursor se ve y se pinta: sin esto, un pintor que no dibujara nunca nada pasaría.
        assert!(
            cursor_shapes(&screen.view()) > 0,
            "con la pantalla al día no se pintó ningún cursor, así que la comprobación de abajo no \
             distingue nada"
        );

        screen.scroll_by(term_core::Scroll::ToTop);
        assert_eq!(
            screen.view().cursor(),
            None,
            "la prueba no llegó a dejar el cursor fuera de lo que se mira"
        );

        assert_eq!(
            cursor_shapes(&screen.view()),
            0,
            "se pintó un cursor con la foto diciendo que no se ve: es un cursor fantasma en la última \
             fila, y quien mire creerá que el programa escribe ahí"
        );
    }

    /// En qué orden encoló el pintor sus glifos: `"texto"` o `"imagen"`, uno por forma.
    fn order_of(screen: &Screen) -> Vec<&'static str> {
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let view = screen.view();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("el orden"),
            ));
            paint(
                &painter,
                &view,
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
            );
        });
        let mut order = Vec::new();
        for shape in &output.shapes {
            note_down(&shape.shape, &mut order);
        }
        output.drop_without_applying_deltas();
        order
    }

    fn note_down(shape: &eframe::egui::Shape, order: &mut Vec<&'static str>) {
        match shape {
            eframe::egui::Shape::Mesh(mesh)
                if mesh.texture_id != eframe::egui::TextureId::default() =>
            {
                order.push("imagen");
            }
            eframe::egui::Shape::Text(_) => order.push("texto"),
            eframe::egui::Shape::Vec(many) => {
                for one in many {
                    note_down(one, order);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn los_emojis_se_encolan_todos_juntos_y_no_entre_las_letras() {
        // **Es la razón de ser del atlas, y hasta aquí no la ataba nadie.** El motor solo junta en el
        // mismo lote de dibujo formas que compartan textura: los emojis viven en su atlas y las letras en
        // el de fuentes, así que encolarlos en el orden de la rejilla parte el lote **en cada emoji**.
        // Medido en **D25** con las dos consolas volcando emojis: el resto del fotograma pasaba de 275 a
        // 743 µs, o sea casi el triple de la cifra con la que **D3** juzga al framework, y por trabajo que
        // no es suyo.
        //
        // **Se afirma sobre el orden de lo encolado, que es lo único que decide el reparto en lotes.**
        // Contar cuántas imágenes salen —que es lo que hacen las demás pruebas de aquí— pasa igual con las
        // dos pasadas fundidas en una: se comprobó fundiéndolas, y las 647 seguían en verde.
        if !there_is_color_on_this_machine() {
            return;
        }
        let order = order_of(&of_text("a\u{1F600}b\u{1F600}c"));

        assert_eq!(
            order
                .iter()
                .filter(|character| **character == "imagen")
                .count(),
            2,
            "no salieron los dos emojis como imagen ({order:?}): la prueba no está mirando lo que dice"
        );
        assert!(
            order
                .iter()
                .filter(|character| **character == "texto")
                .count()
                >= 3,
            "no salieron las tres letras ({order:?}): la prueba no está mirando lo que dice"
        );
        let first_image = order.iter().position(|what| *what == "imagen");
        let last_text = order.iter().rposition(|what| *what == "texto");
        assert!(
            first_image > last_text,
            "los emojis se encolaron entre las letras ({order:?}): el motor parte el lote de dibujo en \
             cada uno, que es exactamente lo que el atlas existe para evitar"
        );
    }

    #[test]
    fn con_el_atlas_lleno_el_emoji_se_sigue_viendo_aunque_sea_en_monocromo() {
        // **El Contrato 5 por el camino que esta máquina no recorre nunca.** Cuando el atlas se llena,
        // pedir el glifo de color devuelve nada y quien pinta **tiene que caer al camino de texto**: el
        // emoji sale en monocromo, que es como salía antes del hito, en vez de desaparecer de la pantalla.
        //
        // **En una máquina con fuente de emoji esa caída no la ejercita ninguna otra prueba**, porque
        // aquí el color siempre está: se comprobó mutando la caída para que no pintara nada, y las 647
        // seguían en verde. La otra mitad —que con el atlas lleno no se devuelva una textura cualquiera—
        // se ata en `fonts`.
        if !there_is_color_on_this_machine() {
            return;
        }
        let mut discoverer = crate::fonts::Discoverer::default();
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        // La celda de esta prueba mide 20 puntos de alto, y los píxeles físicos aquí van uno a uno.
        discoverer.fill_the_atlas(&ctx, 20);

        let (images, texts) = shapes_of_with(&of_text("\u{1F600}"), &mut discoverer);

        assert_eq!(
            images, 0,
            "con el atlas lleno se pintó una imagen igualmente: sale el dibujo de otro glifo"
        );
        assert_eq!(
            texts, 1,
            "con el atlas lleno el emoji no se pintó de ninguna manera: desaparece de la pantalla en vez \
             de salir en monocromo, y quien lo escribió no sabrá que estaba"
        );
    }
}
