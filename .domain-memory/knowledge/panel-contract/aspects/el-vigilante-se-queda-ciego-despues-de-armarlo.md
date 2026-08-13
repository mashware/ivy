---
id: asp_el-vigilante-se-queda-ciego-despues-de-armarlo_mxcghx
slug: el-vigilante-se-queda-ciego-despues-de-armarlo
name: >-
  Un vigilante puede quedarse ciego DESPUÉS de armado y en silencio: sin un
  respaldo, añadirlo empeora lo que había
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:22:29.032Z'
updated_at: '2026-08-13T13:22:29.032Z'
last_verified: '2026-08-13T13:22:29.032Z'
file_paths:
  - crates/app/src/vigilante.rs
  - crates/app/src/lib.rs
symbols:
  - Vigilante
  - esta_ciego
  - RESPIRO_SIN_VIGILANTE
content_hashes:
  crates/app/src/vigilante.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
  crates/app/src/lib.rs: 'sha256:1e1fccb7584128dc015d57b4a85b7afba61479eddebd8cffdbabeba928cc19be'
tags:
  - inotify
  - notify
  - vigilante
  - degradación
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Tres formas distintas de dejar de ser de fiar, y ninguna avisa por sí sola:**

1. **Desbordamiento de la cola del núcleo** (`Q_OVERFLOW`). Llega como `EventKind::Other` con `Flag::Rescan` y **sin ninguna ruta**. Es el único aviso que significa literalmente «te has perdido cosas», y en la primera versión era **el único que se tiraba**, justamente por no traer rutas.
2. **Desaparece el directorio vigilado.** El vigilante moría en silencio.
3. **No se pudo armar de entrada** — el núcleo limita los vigilantes por usuario.

> **La regla: añadir un vigilante sin un camino de degradación EMPEORA lo anterior.** Antes de este PR el panel se releía cuando la consola escribía; con el vigilante ciego y sin respaldo, el panel se queda **congelado y sin decir nada**. Un mecanismo que sustituye a otro tiene que saber, en tiempo de ejecución, si sigue funcionando.

El arreglo es que el vigilante **sabe si ha dejado de ser de fiar**, y la ventana pregunta: si está ciego, relee todas las divisiones con un respiro de 500 ms entre pasadas. Los tres casos comparten ese arreglo.

**Y una razón escrita que se cayó al comprobarla.** La fila que justificaba el respaldo decía que el límite de vigilantes por usuario «se agota en un escritorio cargado». Medido en la máquina de desarrollo: **46 de 1024**. El mecanismo se queda —el escenario de verdad son los otros dos, y ésos sí están medidos— pero la fila se reescribió. Una justificación inventada que sostiene un mecanismo correcto es peor que ninguna: la próxima persona la lee, la cree, y la usa para decidir otra cosa.

**Un dato de diseño que viene de aquí:** se vigila el **directorio** y no el fichero porque un `watch` sobre un fichero va sobre el **inodo** y no se re-registra — al primer renombrado encima, que es justo la publicación que recomienda el contrato, el vigilante se queda mirando un inodo huérfano. Medido antes de diseñar, no supuesto.

## Where it lives

`crates/app/src/vigilante.rs` y el bucle de `crates/app/src/lib.rs`. `Vigilante::esta_ciego()` lo consulta la ventana en cada fotograma; el respaldo levanta todas las banderas y pide repintado con un respiro de 500 ms.
