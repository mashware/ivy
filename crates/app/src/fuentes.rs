//! Qué fuente pinta cada carácter, y **con qué presentación**.
//!
//! Hoy solo lo segundo: decidir, para un carácter y lo que lleve pegado, si le toca ir **a color** o
//! **monocromo**. Descubrir las fuentes de la máquina y rasterizar el glifo de color llegan después, en
//! este mismo módulo — **uno y no dos**, porque elegir la cara y elegir la presentación son la misma
//! pregunta hecha en dos momentos.

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
}
