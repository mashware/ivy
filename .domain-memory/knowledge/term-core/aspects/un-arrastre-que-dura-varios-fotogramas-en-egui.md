---
id: asp_un-arrastre-que-dura-varios-fotogramas-en-egui_a1f1w9
slug: un-arrastre-que-dura-varios-fotogramas-en-egui
name: >-
  Un arrastre que dura varios fotogramas: el identificador lleva el NOMBRE,
  Escape lo termina sin soltar, y ningún botón está cualificado
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:07:30.328Z'
updated_at: '2026-08-17T20:07:30.328Z'
last_verified: '2026-08-17T20:07:30.328Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - atender_el_raton
  - Sense
  - drag_stopped_by
  - dragged_by
  - interact_pointer_pos
content_hashes:
  crates/app/src/lib.rs: 'sha256:bd1d056ad4bcd72e1edfbea41ecd1ed51b925068d136aabb02b79da1032b45e3'
tags:
  - egui
  - entrada
  - arrastre
  - identidad
feature_id: feat_term-core_nx0g1r
---

## What it does

**Tres cosas de egui 0.36 que su nombre no anuncia, y las tres costaron un defecto real en F2.3. Dos las encontró la revisión de código, no quien lo escribió.**

**1. El identificador del control tiene que llevar el NOMBRE de lo que se arrastra, nunca su SITIO.** egui sostiene el arrastre entre fotogramas por ese identificador. Con la posición dentro, cualquier cosa que renumere **mientras el botón sigue apretado** hace que el arrastre en vuelo apunte a otra cosa — y en esta aplicación los acordes de cambiar de pestaña, partir y cerrar **se aplican al principio de la misma pasada de interfaz**, mucho antes de que se atienda el ratón. El caso que se coló: agarrar una consola, pulsar `Ctrl+Mayús+→` sin soltar, y soltar — se reordenaban dos consolas de **la pestaña de destino**.

> **Esto responde el caso que D14 dejó con nombre y remitió a F2.3, y la respuesta NO fue refactorizar la identidad de las fronteras.** Las hojas ya tenían nombre estable; lo que faltaba era usarlo también en el identificador del control. Las fronteras siguen por posición porque su arrastre **no sobrevive a nada**: se resuelve dentro del mismo fotograma.

**2. Escape aborta el arrastre y eso levanta `drag_stopped()` — con el botón TODAVÍA APRETADO.** `interaction.rs` pone `dragged = None` al recibir Escape, y `context.rs` incluye `res.drag_stopped()` en su condición de «se está interactuando», así que `interact_pointer_pos()` sigue devolviendo posición. Un gesto que confunda «el arrastre terminó» con «el usuario soltó» **completa la operación justo al cancelarla**. Lo que distingue es `drag_stopped_by(PointerButton::Primary)`.

**3. `dragged()` y `drag_stopped()` no miran qué botón es.** egui apunta como candidato a arrastrarse **cualquier** pulsación. En un terminal no es teórico: el botón central es el pegado de X11 y el derecho la selección en casi todos.

**Y una que sí sale gratis:** `interact_pointer_pos()` **sigue valiendo en el fotograma en que se suelta**, a propósito y con comentario de los autores. Por eso un gesto así **no necesita estado propio** entre fotogramas; un campo con «qué estoy arrastrando» sería una segunda copia de algo ya guardado, y las dos podrían discrepar.

**Bonus, tercera vez que muerde lo mismo:** **`Sense::CLICK_AND_DRAG` no existe**. Solo el método `click_and_drag()`, y ese añade `FOCUSABLE` — la trampa que ya se pagó con el clic y con la frontera. Lo correcto es `Sense::CLICK | Sense::DRAG`.

## Where it lives

Leído en el fuente de egui 0.36.1 del registro local de cargo (`sense.rs`, `response.rs`, `interaction.rs`, `context.rs`), no supuesto. Implementado en `Ventana::atender_el_raton` (`crates/app/src/lib.rs`), y cada punto atado con una prueba que cae al revertirlo — comprobado mutándolas una a una.
