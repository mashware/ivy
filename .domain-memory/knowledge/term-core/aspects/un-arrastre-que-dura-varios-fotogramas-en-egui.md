---
id: asp_un-arrastre-que-dura-varios-fotogramas-en-egui_a1f1w9
slug: un-arrastre-que-dura-varios-fotogramas-en-egui
name: >-
  Arrastre de varios fotogramas en egui: el identificador lleva el NOMBRE,
  Escape termina sin soltar, ningún botón cualifica, se marca arrastrado SIN
  MOVERSE, y el empate lo gana el ÚLTIMO
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:07:30.328Z'
updated_at: '2026-08-24T22:29:27.895Z'
last_verified: '2026-08-24T22:29:27.895Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - atender_el_raton
  - arrastrar_la_frontera_de_su_panel
  - arrastrar_la_frontera
  - mover_una_frontera
content_hashes:
  crates/app/src/lib.rs: 'sha256:8f5d9220dadc886213f3207269ffaf9502f3454547843a94c9aa6e24dedbb2d5'
tags:
  - egui
  - entrada
  - arrastre
  - identidad
feature_id: feat_term-core_nx0g1r
---

## What it does

**Cinco cosas de egui 0.36 que su nombre no anuncia. Las tres primeras costaron un defecto real en F2.3; la cuarta, en F2.4; la quinta, en F2.6 — y la tercera lo costó DOS VECES, ver el aviso del final.**

**1. El identificador del control tiene que llevar el NOMBRE de lo que se arrastra, nunca su SITIO.** egui sostiene el arrastre entre fotogramas por ese identificador. Con la posición dentro, cualquier cosa que renumere **mientras el botón sigue apretado** hace que el arrastre en vuelo apunte a otra cosa — y en esta aplicación los acordes de cambiar de pestaña, partir y cerrar **se aplican al principio de la misma pasada de interfaz**, mucho antes de que se atienda el ratón. El caso que se coló: agarrar una consola, pulsar `Ctrl+Mayús+→` sin soltar, y soltar — se reordenaban dos consolas de **la pestaña de destino**.

> **Esto responde el caso que D14 dejó con nombre y remitió a F2.3, y la respuesta NO fue refactorizar la identidad de las fronteras.** Las hojas ya tenían nombre estable; lo que faltaba era usarlo también en el identificador del control. Las fronteras del árbol siguen por posición porque su arrastre **no sobrevive a nada**: se resuelve dentro del mismo fotograma. **Ese razonamiento no se hereda**: la frontera del panel, que sí dura lo que dure el botón, lleva el nombre de su división.

**2. Escape aborta el arrastre y eso levanta `drag_stopped()` — con el botón TODAVÍA APRETADO.** `interaction.rs` pone `dragged = None` al recibir Escape, y `context.rs` incluye `res.drag_stopped()` en su condición de «se está interactuando», así que `interact_pointer_pos()` sigue devolviendo posición. Un gesto que confunda «el arrastre terminó» con «el usuario soltó» **completa la operación justo al cancelarla**. Lo que distingue es `drag_stopped_by(PointerButton::Primary)`.

**3. Ningún botón está cualificado.** `interaction.rs:162` casa `PointerEvent::Pressed { .. }` **sin mirar qué botón fue**, así que cualquier pulsación pone `potential_drag_id`. En un terminal no es teórico: el botón central es el pegado de X11 y el derecho la selección en casi todos.

**4. Un control que SOLO siente arrastres se marca como arrastrado EN LA PROPIA PULSACIÓN, sin movimiento.** `interaction.rs:219-220`, con su propio comentario: *«This widget is just sensitive to drags, so we can mark it as dragged right away»*. No hay umbral que recorrer.

> **Y `dragged_by` solo tapa el punto 3, no el 4.** `response.rs:438-440` es `dragged() && button_down(button)`: filtra el botón y **no** exige movimiento. Un control de arrastre que **escribe estado** necesita las dos condiciones: el botón correcto **y** que el puntero se haya movido.

**Por qué el 4 es el peligroso, con su caso concreto.** El asidero de la frontera del panel se dibuja donde el reparto **recorta** el panel, que en una división estrecha no es el ancho que el usuario pidió. Sin exigir movimiento, **un clic seco leía ese corte como intención** y machacaba para siempre el ancho pedido con la ventana ancha. Exigir movimiento es lo que separa «he pulsado aquí» de «quiero esto de ancho».

**5. Cuando varios controles se tocan, el empate lo gana el ÚLTIMO registrado — y la prueba de acierto de egui NO es contención exacta.** Su `interact_radius` (5 puntos por defecto) mete en el conjunto de candidatos a todo lo que esté cerca, no solo a lo que contiene el punto. Un divisor del árbol mide **dos puntos** y las dos consolas lo tocan: las tres son candidatas, y las consolas se registran **después** del divisor, así que ganan ellas.

> **El caso, en F2.6, y es de los que se publican como otro número.** El gesto que una tanda de medida se inyecta llevaba el puntero a la línea del divisor y apretaba **en el mismo fotograma**. Resultado: el empate se lo llevaba una consola, el gesto **la arrastraba a ella**, el soltado la recolocaba y el reparto volvía a mitad y mitad. La tanda medía el gesto de **D15** —mover una consola— bajo la etiqueta del gesto de la frontera, con el informe declarando efectos igual, así que ninguna guarda del guion lo cazaba.

> **Lo que lo arregla es hacer lo que hace una mano: llegar, pararse y apretar después.** Acercar el puntero en un fotograma y pulsar en el **siguiente** — comprobado que un arrastre humano gana siempre, y que el inyectado solo ganaba el primero de la corrida. La causa de fondo es la misma que [[una-prueba-de-arrastre-necesita-dos-pasadas-como-minimo]]: egui resuelve la interacción al principio del fotograma con lo registrado en el **anterior**. **A quien usa la aplicación no le pasa**, y eso está medido, no razonado.

**Y una que sí sale gratis:** `interact_pointer_pos()` **sigue valiendo en el fotograma en que se suelta**, a propósito y con comentario de los autores. Por eso un gesto así **no necesita estado propio** entre fotogramas; un campo con «qué estoy arrastrando» sería una segunda copia de algo ya guardado, y las dos podrían discrepar.

**Bonus:** **`Sense::CLICK_AND_DRAG` no existe**. Solo el método `click_and_drag()`, y ese añade `FOCUSABLE` — la trampa que ya se pagó con el clic y con la frontera. Lo correcto es `Sense::CLICK | Sense::DRAG`.

> **EL AVISO QUE MÁS VALE DE ESTA FICHA, y es sobre la ficha misma.** El punto 3 estaba escrito aquí **antes** de F2.4, con su ejemplo del pegado de X11 y todo — y en F2.4 se volvió a cometer: la frontera del panel se movía con el botón derecho. Tener el conocimiento guardado **no basta** si el camino nuevo se escribe copiando una pieza hermana en vez de repasando esta lista. Ver [[el-molde-del-hermano-trae-sus-decisiones-y-sus-huecos]]. Cada control de arrastre nuevo se revisa contra los cinco puntos, uno por uno.

## Where it lives

Leído en el fuente de egui 0.36.1 del registro local de cargo (`sense.rs`, `response.rs`, `interaction.rs`, `context.rs`, `hit_test.rs`), no supuesto. Implementado en `Ventana::atender_el_raton`, `Ventana::arrastrar_la_frontera` y `Division::arrastrar_la_frontera_de_su_panel` (`crates/app/src/lib.rs`); el gesto inyectado del punto 5 vive en `ArrastresInyectados::mover_una_frontera`, en el mismo fichero. Cada punto atado con una prueba que cae al revertirlo — comprobado mutándolas una a una; la del punto 5 es `el_ciclo_del_gesto_de_la_frontera_devuelve_el_reparto_al_de_partida`, que fue **la que lo encontró**.
