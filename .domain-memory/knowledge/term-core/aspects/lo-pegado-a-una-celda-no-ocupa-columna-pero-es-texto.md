---
id: asp_lo-pegado-a-una-celda-no-ocupa-columna-pero-es-texto_w5ycas
slug: lo-pegado-a-una-celda-no-ocupa-columna-pero-es-texto
name: >-
  El motor guarda pegados a cada celda los caracteres de anchura cero, y esta
  caja los tiraba: por eso el símbolo y su versión emoji llegaban idénticos
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T13:58:32.624Z'
updated_at: '2026-08-25T13:58:32.624Z'
last_verified: '2026-08-25T13:58:32.624Z'
file_paths:
  - crates/term-core/src/view.rs
  - crates/term-core/src/screen.rs
  - docs/02-decisiones.md
symbols:
  - Adjuntos
  - ViewCell
  - 'View::adjuntos_de'
  - celda_desde
  - to_text
content_hashes:
  crates/term-core/src/view.rs: 'sha256:8bb69d1e65b99d0bc3c8a4c29da0701affe1d675cbb02b5b782636ae76a99cc9'
  crates/term-core/src/screen.rs: 'sha256:6af1919efa54066bdf18665edd3cf6316c9ff4c01f3cfbd0b9a94a029dd24ef9'
  docs/02-decisiones.md: 'sha256:0de8ae72f3ac308cb5cc9ba4d96b4b7d71443f38da0b0ca87bafa69318fc44ce'
tags:
  - term-core
  - unicode
  - emoji
  - F2.9
  - D8
  - D22
feature_id: feat_term-core_nx0g1r
---

## What it does

**`alacritty_terminal 0.26` guarda los caracteres de anchura cero pegados a su celda** —`Cell::zerowidth()`, un `Vec<char>`— **y hasta agosto de 2026 esta caja los tiraba**: `ViewCell` solo llevaba su carácter. **Consecuencia: `☁` y `☁️` llegaban a quien pinta IDÉNTICOS**, porque lo único que los separa es el selector `U+FE0F`, que no ocupa columna.

### Tres cosas que se aprendieron al exponerlo, y ninguna es evidente

**1. No caben dentro de la celda.** `ViewCell` es `Copy` y la rejilla se reconstruye **entera cada fotograma** (`vec![ViewCell::default(); filas × columnas]`): un `Vec` por celda sería una reserva de memoria **por celda y por fotograma** en el camino más caliente del proyecto, y le quitaría el `Copy` a un tipo que lo tiene por eso mismo. Van en un **vector plano de la vista** y la celda lleva un **tramo**.

> **Y el tramo se empaqueta en UN entero, que son cuatro bytes por celda.** En dos campos de un tipo anidado la celda pasa de 24 a **32** bytes —el tipo trae su propio relleno y el `u8` no cabe en el hueco que la celda ya tenía—; empaquetado en un `u32`, **28**. Lo cazó una revisión compilando las dos formas.

**2. El motor NO tiene techo.** Su `push_zerowidth` es un `Vec::push` sin tope, o sea **una entrada de fuera sin acotar** —el patrón que **D8** ya castigó con la cola de salida—. El techo lo pone esta caja. **Y ese número no preserva ninguna secuencia concreta**: se creyó que hacían falta seis para no partir una familia de emoji (`👨‍👩‍👧‍👦`) y **es falso** — comprobado ejecutando: sus miembros son **anchos**, así que llegan como **cuatro celdas con un juntador cada una**, no como una con seis. El techo **acota**, no preserva.

**3. La cabeza de un carácter ancho lo lleva y su acompañante no** —y es el caso **normal**, porque CJK y los emojis son anchos, no una esquina—. Si el acompañante trajera una copia, la mitad derecha de un emoji se pintaría con otra presentación que la izquierda. **Y un selector suelto se pega a la celda VACÍA que tenga delante**: comprobado ejecutando, y es lo que hace que un espacio en blanco pueda acabar en la fuente de color.

### El defecto que destapó

**`to_text` devolvía `☁` donde el programa escribió `☁️`.** No es un carácter recortado: es **otro** carácter. **Copiar con el ratón nunca lo tuvo**, porque eso lo hace el motor (`term/mod.rs:610`). El defecto era viejo, pero **hasta este hito la caja no tenía forma de saberlo**, así que no era ni detectable ni arreglable. **D22** ganó su quinta regla de qué es «el texto».

### Una contrapartida que hay que saber

**Comparar dos `ViewCell` solo significa algo dentro de la MISMA vista.** El tramo dice **dónde** están guardados los caracteres, no cuáles son: dos celdas que muestran lo mismo en dos vistas distintas pueden salir distintas. Hoy no las compara nadie; el día que haga falta, lo que se quiere comparar es el carácter y lo que `View::adjuntos_de` devuelva.

## Where it lives

El tipo `Adjuntos` y el lector `View::adjuntos_de` viven en `crates/term-core/src/view.rs`; la construcción de la foto y el volcado a texto, en `crates/term-core/src/screen.rs`. Las reglas de qué es «el texto» están en **D22** de `docs/02-decisiones.md`.
