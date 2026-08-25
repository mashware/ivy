---
id: asp_epaint-no-rasteriza-glifos-a-color_291m37
slug: epaint-no-rasteriza-glifos-a-color
name: >-
  epaint 0.36 no sabe rasterizar un glifo a color: solo dibuja contornos, así
  que registrar Noto Color Emoji no daría ni la silueta
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T13:58:00.081Z'
updated_at: '2026-08-25T13:58:00.081Z'
last_verified: '2026-08-25T13:58:00.081Z'
file_paths:
  - crates/app/src/pintor.rs
  - crates/app/Cargo.toml
symbols:
  - 'Painter::image'
  - FontDefinitions
  - 'FontData::from_owned'
  - pintar_glifo
content_hashes:
  crates/app/src/pintor.rs: 'sha256:db9bad5bd4d6b139d467d7b4690dc2b403a89ab5833919cb838db97c1170e0fb'
  crates/app/Cargo.toml: 'sha256:ca1a2443192911db903084df39c10e571f399a47c41b1583296970127d428aac'
tags:
  - egui
  - epaint
  - fuentes
  - emoji
  - F2.9
  - D3
feature_id: feat_term-core_nx0g1r
---

## What it does

**epaint 0.36.1 NO sabe pintar un glifo a color, y eso reordena cualquier hito de emojis.** Leído en su fuente, no supuesto: su rasterizador (`text/font.rs:241`) coge **solo** `outline_glyphs` y pinta el contorno con `set_paint(WHITE)`; el color lo pone después la malla tiñendo el resultado. En todo su módulo de texto no hay **una sola** mención a las tablas de color de una fuente (COLR/CPAL, CBDT/CBLC, sbix).

**La consecuencia práctica, y es la que ahorra una entrega entera:** registrar `Noto Color Emoji` como una cara más **no daría emojis a color**. Daría la silueta — y como su glifo es un **mapa de bits de 136×128 sin contornos** (comprobado por el lado duro), **no daría nada**.

### Lo que sí da egui, y por eso el camino existe

- **Cadena de reserva por familia, ya hecha.** `Font` envuelve varias caras (`text/font.rs:663`) y `resolve_face(c)` busca cuál cubre el carácter, con caché por carácter en `:734`. **CJK y los iconos sí salen** solo con darle caras.
- **El atlas es un `ColorImage` con RGBA premultiplicado de verdad** (`font.rs:283-292`): el sitio donde vive un glifo de color **existe**.
- **`Painter::image`** (`egui/painter.rs:447`) pinta un rectángulo con textura. Y este pintor ya dibuja **celda a celda** con un `painter.text` de un solo carácter, así que pintar un emoji como imagen es **una rama más en un `if` que ya existe**, no un cambio de arquitectura.
- **`skrifa`** (la caja que epaint ya usa por debajo) trae `color/` y `bitmap.rs`: la pieza que falta **ya está en el árbol**.

> **Por eso NO se copió la arquitectura de Alacritty o Ghostty.** Ellos rasterizan todo por su cuenta porque **no tienen un framework de interfaz debajo**; hacerlo aquí sería rehacer lo que egui hace bien y además invalidaría el histórico de medida entero. Lo que se copia es **el resultado**, no el cómo.

### Un aviso que epaint escribe él mismo

`text/font.rs:753-758`: el avance de una cara de **reserva** se calcula con las métricas de la **primaria**. Para **colocar** no afecta a un pintor que fija el rectángulo de cada celda; para el **tamaño** sí, y hay que resolver la cara antes o declararlo y medirlo.

### Y una consecuencia de compilación

**Enlazar fontconfig exige `pkg-config` y sus cabeceras** — comprobado: una máquina sin ellas **no compila el repo**, y falla en el guión de construcción de la dependencia, no en el código. Se carga en **ejecución** en su lugar, la misma técnica —y la misma razón— por la que `libwayland` llega de rebote. El precio declarado: su ausencia se descubre **al arrancar** en vez de al compilar.

**Y ojo con las dependencias «solo de desarrollo»**: añadir `fontique` movió `windows-sys` de 0.59 a 0.60 para el portapapeles, que **sí** va en el binario. En Linux no cambia nada; en Windows ya cambia lo que se compila.

## Where it lives

El pintor celda a celda está en `crates/app/src/pintor.rs` (`pintar_glifo`). Las lecturas de fuente son del registro local de cargo: `epaint 0.36.1` (`src/text/font.rs`, `src/text/fonts.rs`) y `egui 0.36.1` (`src/painter.rs`). El veredicto sobre egui vive en **D3** y la decisión del descubrimiento en **D24**.
