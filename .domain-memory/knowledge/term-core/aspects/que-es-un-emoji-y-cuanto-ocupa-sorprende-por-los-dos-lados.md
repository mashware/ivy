---
id: asp_que-es-un-emoji-y-cuanto-ocupa-sorprende-por-los-dos-lados_htsvvx
slug: que-es-un-emoji-y-cuanto-ocupa-sorprende-por-los-dos-lados
name: >-
  Qué caracteres son emoji y cuántas celdas ocupan sorprende por los dos lados:
  la almohadilla y los dígitos lo son, y un emoji con selector ocupa una celda
  mientras que uno sin él ocupa dos
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T17:16:47.025Z'
updated_at: '2026-08-25T17:16:47.025Z'
last_verified: '2026-08-25T17:16:47.025Z'
file_paths:
  - crates/app/src/fuentes.rs
  - crates/app/src/pintor.rs
symbols:
  - presentacion_de
  - por_defecto_va_a_color
  - CellWidth
content_hashes:
  crates/app/src/fuentes.rs: 'sha256:d6d2b2e362c00d1b61b89b14f41b98bc0d7c28513c69d568685c156cc1730c74'
  crates/app/src/pintor.rs: 'sha256:109c7e330be5065d3b14b62eafcd3fa4c3b54cd36122350c39c71084337b8a87'
tags:
  - unicode
  - emoji
  - anchura
  - term-core
  - alacritty
feature_id: feat_term-core_nx0g1r
---

## What it does

**Dos suposiciones que parecen seguras y las dos son falsas.** Salieron construyendo F2.9 (agosto de 2026) y las dos se comprobaron **ejecutando**, no leyendo la especificación.

### 1. El ASCII también tiene emojis: once teclas

Unicode le da **`Emoji=Yes` a la almohadilla, al asterisco y a los diez dígitos**. Con el selector de emoji detrás son teclas de pleno derecho: `#️⃣`, `*️⃣`, `0️⃣`…`9️⃣`. **Once caracteres ASCII que van a color.**

Donde muerde: un pintor de terminal recorre miles de celdas por fotograma y la tentación de descartar el ASCII con una comparación —sin consultar las tablas de Unicode— es enorme, porque el ASCII es casi todo lo que pasa por una pantalla. **Ese atajo se come las once teclas.** El atajo correcto descarta el ASCII **sin nada pegado**: un ASCII suelto nunca va a color, uno con selector puede.

> Lo cazó una prueba escrita **para atar el atajo**, que recorre los 128 caracteres con los dos selectores detrás. Y hace falta una prueba **en cada sitio**: mutar el atajo del pintor dejaba en verde la prueba de la regla, porque la regla vive en otro fichero.

### 2. Un emoji con selector ocupa UNA celda; uno sin él, DOS

Comprobado ejecutando el motor: `😀` llega como cabeza de un carácter **ancho** (dos columnas), mientras que `☁️`, `❤️` y `✔️` llegan como **`Single`** —una columna— con el selector pegado. Es lo contrario de lo que sugiere el sentido común: el carácter que **pide** presentación de emoji es el que menos sitio recibe.

**La causa:** la anchura la decide `wcwidth` sobre el carácter base, y `U+2601` es de anchura neutra. Los terminales que son el listado —Ghostty, Kitty— fuerzan a dos la anchura de una secuencia emoji, siguiendo UTS #51; `alacritty_terminal` no lo hace.

**La consecuencia visible, si se respeta la anchura declarada:** `☁️` y `❤️` salen a un **60 % del alto de la línea**, porque el dibujo se acota al ancho de **una** celda para no invadir la vecina. Se ven pequeños, y es lo correcto: pintarlos a la altura de la línea taparía lo que el programa escribió al lado, y quién decide la anchura de un carácter es el motor, no quien pinta.

> **Y de aquí sale una tercera cosa, sobre el reescalado:** el mapa de bits de `Noto Color Emoji` es **más ancho que alto** (136×128), así que escalarlo «a la altura de la celda» —que es como suele redactarse el requisito— lo deja **más ancho que las dos columnas** que el motor le reservó: 18 píxeles para una celda de 17 de alto. El factor tiene que ser **el menor** de los dos, no el de la altura.

## Where it lives

La regla de presentación vive en `presentacion_de` y `por_defecto_va_a_color`, en `crates/app/src/fuentes.rs`; el atajo del ASCII, en el bucle de `pintar` de `crates/app/src/pintor.rs`. La anchura la trae `CellWidth` en la foto de la rejilla, y sale del motor de terceros. Las pruebas que atan las dos mitades: `un_ascii_sin_nada_pegado_no_va_nunca_a_color` y `una_tecla_de_ascii_con_su_selector_si_va_a_color` en `fuentes.rs`, `una_tecla_hecha_con_una_almohadilla_sale_a_color` y `un_emoji_de_una_sola_columna_se_queda_dentro_de_ella` en `pintor.rs` y `fuentes.rs`.
