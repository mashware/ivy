---
id: asp_cubrir-un-caracter-no-es-pintarlo-con-su-presentacion_cgxbdq
slug: cubrir-un-caracter-no-es-pintarlo-con-su-presentacion
name: >-
  Una fuente que «cubre» un emoji puede pintarlo en gris y ganar por orden
  alfabético: la regla no es «la primera que cubra», es «la primera que cubra
  con la presentación que toca»
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T13:57:29.445Z'
updated_at: '2026-08-25T13:57:29.445Z'
last_verified: '2026-08-25T13:57:29.445Z'
file_paths:
  - crates/app/examples/fuentes.rs
  - crates/app/src/fuentes.rs
  - docs/02-decisiones.md
symbols:
  - presentacion_de
  - por_defecto_va_a_color
  - quien_cubre
  - primera_a_color
content_hashes:
  docs/02-decisiones.md: 'sha256:0de8ae72f3ac308cb5cc9ba4d96b4b7d71443f38da0b0ca87bafa69318fc44ce'
tags:
  - fuentes
  - emoji
  - unicode
  - F2.9
  - D24
feature_id: feat_term-core_nx0g1r
---

## What it does

**Una fuente que «cubre» un emoji puede pintarlo en blanco y negro, y ganar por orden alfabético.** Medido con `cargo run --release --example fuentes -p app`: recorriendo las 259 familias instaladas en orden, quien se lleva `😀` en esta máquina es **DejaVu Sans**, en **monocromo**, teniendo `Noto Color Emoji` instalada. Es el defecto que Alacritty tiene documentado desde hace años — coge el glifo de DejaVu y solo recurre a Noto Emoji si aquél no lo trae.

**La regla correcta no es «la primera cara que cubra el carácter», es «la primera que lo cubra CON LA PRESENTACIÓN QUE TOCA».** Preguntar por la familia genérica de emoji lo arregla y además es lo más barato de todo el banco: **7,2–10,8 µs en frío** contra los 94 µs–18,2 ms de recorrer las familias en orden. Su primera candidata **es** `Noto Color Emoji`.

> **Un banco que pregunte solo «¿esta cara cubre este carácter?» publica «el emoji está cubierto» y esconde el defecto.** La entrega que traiga el color lo descubriría con el código ya escrito. La primera versión del banco de F2.9 hacía exactamente eso, y ahí estaba escondido el hallazgo.

### La regla de presentación tiene una mitad que se olvida

**«A color si le sigue `U+FE0F`», leído al pie de la letra, manda a la fuente de color cualquier carácter con el selector detrás** — una `a`, y **un espacio en blanco**. Y lo del espacio no es rebuscado: comprobado ejecutando el motor, **un selector suelto se pega a la celda vacía que tenga delante**, y un selector suelto sale de cortar una cadena por la mitad de un emoji.

Unicode solo define secuencias de variación para caracteres con `Emoji=Yes`. La regla completa, que es la de Ghostty:

> A color **si y solo si**: le sigue `U+FE0F` **y el carácter tiene `Emoji=Yes`**, **o** tiene `Emoji_Presentation=Yes` y **no** le sigue `U+FE0E`.

**Y al añadir esa condición hay que atarla por los dos lados**: quitarla manda a color la `a` y el espacio; pasarse de estricta —exigir `Emoji_Presentation` en vez de `Emoji`— deja `☁️` en monocromo, que es el caso que da sentido al hito entero.

### Lo que la regla no alcanza

**Los modificadores de tono de piel (`U+1F3FB`–`U+1F3FF`) son anchos**, así que el motor les da **celda propia** en vez de pegarlos: en `☝🏽` la base llega sin nada pegado y sale en texto mientras el modificador iría a color al lado. Unicode y Ghostty tratan la secuencia entera como emoji. **Resolverlo exige mirar la celda siguiente**, que es información que una regla por celda no tiene.

### Y la caja de propiedades Unicode no lo pone fácil

`unicode-properties 0.1.4` **no trae método** para `Emoji_Presentation`: expone `Emoji` y `Emoji_Component`, y esa propiedad solo aparece **dentro de los nombres** de su enumerado. Hay que enumerar sus cuatro variantes a mano, comprobándolas contra su tabla —**dos que el nombre delata mal**, `EmojiModifierBase` y `EmojiOtherAndEmojiComponent`, parecen candidatas y no lo son—. Y su enumerado es `#[non_exhaustive]`: una categoría futura caería en el brazo por defecto y saldría monocromo **sin aviso del compilador**.

## Where it lives

La regla vive en `crates/app/src/fuentes.rs` (`presentacion_de`, `por_defecto_va_a_color`). El banco que destapó lo del orden alfabético es `crates/app/examples/fuentes.rs`. Los números están publicados en **D24** de `docs/02-decisiones.md`.
