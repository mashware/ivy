---
id: asp_un-arrastre-que-dura-varios-fotogramas-en-egui_a1f1w9
slug: un-arrastre-que-dura-varios-fotogramas-en-egui
name: >-
  Un arrastre que dura varios fotogramas: el identificador lleva el NOMBRE,
  Escape lo termina sin soltar, ningún botón está cualificado, y se marca como
  arrastrado SIN MOVERSE
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:07:30.328Z'
updated_at: '2026-08-19T15:29:24.626Z'
last_verified: '2026-08-19T15:29:24.626Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - atender_el_raton
  - arrastrar_la_frontera_de_su_panel
content_hashes:
  crates/app/src/lib.rs: 'sha256:67dfdfc9bad7c0d618f018577eecd1de410625cc5cf9347f593360fff275852e'
tags:
  - egui
  - entrada
  - arrastre
  - identidad
feature_id: feat_term-core_nx0g1r
---

## What it does

**Cuatro cosas de egui 0.36 que su nombre no anuncia. Las tres primeras costaron un defecto real en F2.3; la cuarta, en F2.4 — y la tercera lo costó DOS VECES, ver el aviso del final.**

**1. El identificador del control tiene que llevar el NOMBRE de lo que se arrastra, nunca su SITIO.** egui sostiene el arrastre entre fotogramas por ese identificador. Con la posición dentro, cualquier cosa que renumere **mientras el botón sigue apretado** hace que el arrastre en vuelo apunte a otra cosa — y en esta aplicación los acordes de cambiar de pestaña, partir y cerrar **se aplican al principio de la misma pasada de interfaz**, mucho antes de que se atienda el ratón. El caso que se coló: agarrar una consola, pulsar `Ctrl+Mayús+→` sin soltar, y soltar — se reordenaban dos consolas de **la pestaña de destino**.

> **Esto responde el caso que D14 dejó con nombre y remitió a F2.3, y la respuesta NO fue refactorizar la identidad de las fronteras.** Las hojas ya tenían nombre estable; lo que faltaba era usarlo también en el identificador del control. Las fronteras del árbol siguen por posición porque su arrastre **no sobrevive a nada**: se resuelve dentro del mismo fotograma. **Ese razonamiento no se hereda**: la frontera del panel, que sí dura lo que dure el botón, lleva el nombre de su división.

**2. Escape aborta el arrastre y eso levanta `drag_stopped()` — con el botón TODAVÍA APRETADO.** `interaction.rs` pone `dragged = None` al recibir Escape, y `context.rs` incluye `res.drag_stopped()` en su condición de «se está interactuando», así que `interact_pointer_pos()` sigue devolviendo posición. Un gesto que confunda «el arrastre terminó» con «el usuario soltó» **completa la operación justo al cancelarla**. Lo que distingue es `drag_stopped_by(PointerButton::Primary)`.

**3. Ningún botón está cualificado.** `interaction.rs:162` casa `PointerEvent::Pressed { .. }` **sin mirar qué botón fue**, así que cualquier pulsación pone `potential_drag_id`. En un terminal no es teórico: el botón central es el pegado de X11 y el derecho la selección en casi todos.

**4. Un control que SOLO siente arrastres se marca como arrastrado EN LA PROPIA PULSACIÓN, sin movimiento.** `interaction.rs:219-220`, con su propio comentario: *«This widget is just sensitive to drags, so we can mark it as dragged right away»*. No hay umbral que recorrer.

> **Y `dragged_by` solo tapa el punto 3, no el 4.** `response.rs:438-440` es `dragged() && button_down(button)`: filtra el botón y **no** exige movimiento. Un control de arrastre que **escribe estado** necesita las dos condiciones: el botón correcto **y** que el puntero se haya movido.

**Por qué el 4 es el peligroso, con su caso concreto.** El asidero de la frontera del panel se dibuja donde el reparto **recorta** el panel, que en una división estrecha no es el ancho que el usuario pidió. Sin exigir movimiento, **un clic seco leía ese corte como intención** y machacaba para siempre el ancho pedido con la ventana ancha. Exigir movimiento es lo que separa «he pulsado aquí» de «quiero esto de ancho».

**Y una que sí sale gratis:** `interact_pointer_pos()` **sigue valiendo en el fotograma en que se suelta**, a propósito y con comentario de los autores. Por eso un gesto así **no necesita estado propio** entre fotogramas; un campo con «qué estoy arrastrando» sería una segunda copia de algo ya guardado, y las dos podrían discrepar.

**Bonus:** **`Sense::CLICK_AND_DRAG` no existe**. Solo el método `click_and_drag()`, y ese añade `FOCUSABLE` — la trampa que ya se pagó con el clic y con la frontera. Lo correcto es `Sense::CLICK | Sense::DRAG`.

> **EL AVISO QUE MÁS VALE DE ESTA FICHA, y es sobre la ficha misma.** El punto 3 estaba escrito aquí **antes** de F2.4, con su ejemplo del pegado de X11 y todo — y en F2.4 se volvió a cometer: la frontera del panel se movía con el botón derecho. Tener el conocimiento guardado **no basta** si el camino nuevo se escribe copiando una pieza hermana en vez de repasando esta lista. Ver [[el-molde-del-hermano-trae-sus-decisiones-y-sus-huecos]]. Cada control de arrastre nuevo se revisa contra los cuatro puntos, uno por uno.

## Where it lives

Leído en el fuente de egui 0.36.1 del registro local de cargo (`sense.rs`, `response.rs`, `interaction.rs`, `context.rs`), no supuesto. Implementado en `Ventana::atender_el_raton` y en `Division::arrastrar_la_frontera_de_su_panel` (`crates/app/src/lib.rs`), y cada punto atado con una prueba que cae al revertirlo — comprobado mutándolas una a una.
