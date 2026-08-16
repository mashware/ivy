---
id: asp_la-vuelta-de-medida-se-cierra-al-pintar_szjxfy
slug: la-vuelta-de-medida-se-cierra-al-pintar
name: >-
  La vuelta de medida de una división solo se cierra al pintarla, así que quien
  interpreta sin pintarse le regala su tiempo al framework
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:25:24.471Z'
updated_at: '2026-08-15T21:25:24.471Z'
last_verified: '2026-08-15T21:25:24.471Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/medida.rs
symbols:
  - pintar_division
  - cerrar_fotograma
  - anotar_vuelta
  - 'Tramos::de_una_vuelta'
  - 'RestoDelFotograma::anotar'
  - cerrar_las_vueltas_no_pintadas
content_hashes:
  crates/app/src/lib.rs: 'sha256:b3ca4328c99c0b94b314ea9a6c6d1ab4f6afe0dad22ef3415700f289bc9af411'
  crates/app/src/medida.rs: 'sha256:a357d2a7f42908e764852a0cff5ca8c602915de6bb6bb064d62bd866fa4cb211'
tags:
  - medida
  - F2.1
  - resuelto
feature_id: feat_term-core_nx0g1r
---

## What it does

**El único sitio que cerraba la vuelta de medida de una división era pintarla.** `Division::bombear` deja los tramos en `pendiente_de_medir` y solo `pintar_division` los recogía. Una división que interpreta bytes y **no se pinta** no cerraba su vuelta nunca: `Medida::cerrar_fotograma` devolvía `None`, el recorrido de la ventana la saltaba, y la resta de `RestoDelFotograma` **le regalaba ese tiempo al framework de interfaz** — la cifra con la que D3 decide si egui aguanta. Y la muestra no se doblaba: se **sobrescribía en silencio**.

Es la misma clase de fallo que [[quien-no-declara-su-trabajo-se-lo-atribuye-a-otro]], con un **disparador nuevo**: aquel aparecía al pasar de una consola a dos; éste aparece **la primera vez que existe una división que se bombea y no se pinta**, o sea una pestaña de fondo. Sesgo estimado con una sola pestaña de fondo volcando: el «resto» pasaría de 0,50 ms a ~2,7 — **más de cinco veces**.

> **Y el arreglo no necesitaba serie nueva, que es la trampa de este hallazgo.** `Medida::cerrar_fotograma` solo anota en las cuatro series **dentro del `if let Some(esperado)`**, y aun así devuelve lo interpretado **fuera** de ese `if`. Basta con cerrar la vuelta con `Tramos::de_una_vuelta(None, recogido_en, interpretado, ZERO, ZERO)`: la resta se hace y **ninguna media se ensucia**. Un diseño que añadía una serie propia para esto lo cortó el retador tras leer esas líneas.

**Resuelto en F2.1** con `cerrar_las_vueltas_no_pintadas`, que corre **al final de la pasada de interfaz** — no en la mitad lógica, porque allí todavía no se sabe qué se va a pintar. La mutación que lo demuestra: barrer **antes** de pintar pasaba las 216 pruebas y rompía la medida entera, porque ninguna media recibiría muestra en toda la sesión.

> **Y tenía un hermano que este arreglo no tocaba**, encontrado después al revisar el PR #64: la vuelta no solo hay que cerrarla, hay que **no perderla**. En un fotograma cuya pasada egui repite, el bombeo rearmaba la vuelta y `anotar_vuelta` la **sobrescribía**, con el mismo destino final —el tiempo interpretado publicado como coste del framework— por una puerta distinta. Ver [[la-pasada-repetida-rebombea-y-sobrescribe-la-vuelta]].

## Where it lives

`Ventana::cerrar_las_vueltas_no_pintadas` y `Ventana::pintar_division` en `crates/app/src/lib.rs`; `Medida::cerrar_fotograma` y `RestoDelFotograma::anotar` en `crates/app/src/medida.rs`
