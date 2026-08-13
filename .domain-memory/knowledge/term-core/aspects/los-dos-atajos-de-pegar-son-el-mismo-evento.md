---
id: asp_los-dos-atajos-de-pegar-son-el-mismo-evento_861653
slug: los-dos-atajos-de-pegar-son-el-mismo-evento
name: >-
  `Ctrl+V` y `Ctrl+Mayús+V` llegan como el mismo evento sin modificadores, y el
  campo que egui publica no los distingue
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T16:42:11.261Z'
updated_at: '2026-08-13T16:42:11.261Z'
last_verified: '2026-08-13T16:42:11.261Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/teclado.rs
symbols:
  - 'Division::teclear'
  - 'Ventana::modificadores'
content_hashes:
  crates/app/src/lib.rs: 'sha256:a05a4309b46dd644b5b6a2389c026f7fef94d9e1a7cb1fcde40b75eb39607291'
  crates/app/src/teclado.rs: 'sha256:fac665b7c9d92b5de4f79218612a98ba1d6c97b26242a606fd4149487b51b79c'
tags:
  - egui
  - teclado
  - portapapeles
  - D11
feature_id: feat_term-core_nx0g1r
---

## What it does

**El escritorio resuelve estos atajos antes de que lleguen, y los deja indistinguibles.** `is_paste_command` es `mandato && tecla == V` y **no mira Mayúsculas**; tras emitir el evento de pegar hace `return`, así que **el evento de tecla no llega nunca**. Los dos atajos producen exactamente lo mismo.

> **Y leer `InputState::modifiers` NO los distingue.** Ese campo es **uno solo por pase**: egui recorre el lote y se queda con el último aviso de cambio, así que describe cómo estaban las teclas **al final**, no cuando llegó el pegado. Soltar el acorde antes de que se recoja el fotograma daría «ninguno»; pulsar Mayúsculas después de pegar daría «sí». Un comentario de este proyecto dio por hecho durante dos hitos que bastaba con mirarlo «en el bucle»: acertaba en **dónde** y fallaba en **qué**.

**Lo que sí funciona es reconstruirlos evento a evento**, que es lo que egui hace por dentro: recorrer la cola en orden llevando un acumulador propio, que el aviso de cambio actualiza. Tres cosas que hacen falta y no son obvias:

1. **Sembrarlo con el valor del pase anterior.** Un acorde puede empezar en un fotograma y usarse en el siguiente — egui rehace la lista de eventos en cada uno.
2. **Reiniciarlo al perder el foco.** Es la salvaguarda que egui tiene en su bucle, con su razón escrita: al salir la ventana no llega el aviso de que se soltaron las teclas. Sin ella, irse con un atajo del escritorio y soltar fuera deja Mayúsculas puesto para siempre, y el siguiente `Ctrl+V` vuelca el portapapeles.
3. **Dejar el aviso de cambio en la cola**, porque no es nuestro.

**Por qué merece la pena el lío, y no quedarse el `Ctrl+V` para pegar:** ese byte tiene dueño, y no es el terminal. Es el bloque visual de vim y el *quoted-insert* del shell. Comprobado con Tilix delante: allí `Ctrl+V` no pega, pero **dentro de esa misma ventana** Claude sí pega con `Ctrl+V`, porque recibe el byte y decide que para él significa eso. **Quién pega con `Ctrl+V` es decisión de cada programa.**

**Un borde que no se puede arreglar desde aquí:** con el **portapapeles vacío** el escritorio reconoce el atajo, no encuentra nada y hace `return` **sin emitir ningún evento**. Así que `Ctrl+V` no manda ni el byte de control, y el bloque visual de vim no se abre en una sesión sin nada copiado.

## Where it lives

`Division::teclear` en `crates/app/src/lib.rs` reconstruye los modificadores; la semilla vive en `Ventana` porque son del teclado, no de una consola. Leído en `egui-winit-0.36.1/src/lib.rs:1027-1035,1430` y `egui-0.36.1/src/input_state/mod.rs:396,433,442`.
