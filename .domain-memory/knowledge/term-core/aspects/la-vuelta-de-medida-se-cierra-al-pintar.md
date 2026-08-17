---
id: asp_la-vuelta-de-medida-se-cierra-al-pintar_szjxfy
slug: la-vuelta-de-medida-se-cierra-al-pintar
name: >-
  La vuelta de medida de una división solo se cierra al pintarla, y lo que lleva
  sin declarar vive en tres sitios — quien mire uno declara cero
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:25:24.471Z'
updated_at: '2026-08-17T12:01:16.017Z'
last_verified: '2026-08-17T12:01:16.017Z'
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
  - 'Division::llevarse_lo_de_este_fotograma'
  - 'Medida::llevarse_lo_pendiente'
content_hashes:
  crates/app/src/lib.rs: 'sha256:69e903da2ae25795351e9b8a05001efaad7c4703b59964f4e430a94a6176c041'
  crates/app/src/medida.rs: 'sha256:627412ac283ad6138a2b514940f63e45eb84d875a0405dd62991852b0442a69c'
tags:
  - medida
  - F2.1
  - F2.2
  - resuelto
feature_id: feat_term-core_nx0g1r
---

## What it does

**El único sitio que cerraba la vuelta de medida de una división era pintarla.** `Division::bombear` deja los tramos en `pendiente_de_medir` y solo `pintar_division` los recogía. Una división que interpreta bytes y **no se pinta** no cerraba su vuelta nunca: `Medida::cerrar_fotograma` devolvía `None`, el recorrido de la ventana la saltaba, y la resta de `RestoDelFotograma` **le regalaba ese tiempo al framework de interfaz** — la cifra con la que D3 decide si egui aguanta. Y la muestra no se doblaba: se **sobrescribía en silencio**.

Es la misma clase de fallo que [[quien-no-declara-su-trabajo-se-lo-atribuye-a-otro]], con un **disparador nuevo**: aquel aparecía al pasar de una consola a dos; éste aparece **la primera vez que existe una división que se bombea y no se pinta**, o sea una pestaña de fondo. Sesgo estimado con una sola pestaña de fondo volcando: el «resto» pasaría de 0,50 ms a ~2,7 — **más de cinco veces**.

> **Y el arreglo no necesitaba serie nueva, que es la trampa de este hallazgo.** `Medida::cerrar_fotograma` solo anota en las cuatro series **dentro del `if let Some(esperado)`**, y aun así devuelve lo interpretado **fuera** de ese `if`. Basta con cerrar la vuelta con `Tramos::de_una_vuelta(None, recogido_en, interpretado, ZERO, ZERO)`: la resta se hace y **ninguna media se ensucia**. Un diseño que añadía una serie propia para esto lo cortó el retador tras leer esas líneas.

**Resuelto en F2.1** con `cerrar_las_vueltas_no_pintadas`, que corre **al final de la pasada de interfaz** — no en la mitad lógica, porque allí todavía no se sabe qué se va a pintar. La mutación que lo demuestra: barrer **antes** de pintar pasaba las 216 pruebas y rompía la medida entera, porque ninguna media recibiría muestra en toda la sesión.

> **Y tenía un hermano que este arreglo no tocaba**, encontrado después al revisar el PR #64: la vuelta no solo hay que cerrarla, hay que **no perderla**. En un fotograma cuya pasada egui repite, el bombeo rearmaba la vuelta y `anotar_vuelta` la **sobrescribía**, con el mismo destino final —el tiempo interpretado publicado como coste del framework— por una puerta distinta. Ver [[la-pasada-repetida-rebombea-y-sobrescribe-la-vuelta]].

## Y lo que la división que se **cierra** lleva sin declarar vive en TRES sitios (F2.2, PR #74)

Cuando una división se suelta hay que llevarse antes lo que ha hecho, o su trabajo se lo queda la resta. **El error no es olvidarse: es mirar un solo sitio.** El orden real del fotograma lo decide, y es contraintuitivo:

1. `logic` **cierra el fotograma anterior**, lo que **vacía** `medida.pendiente`.
2. `logic` **bombea**, lo que llena `pendiente_de_medir` con lo que la consola acaba de interpretar.
3. La pasada de interfaz atiende **el teclado antes del pintado**, así que el acorde de cerrar llega **aquí**.

> **Consecuencia: en el instante del cierre, `Medida::llevarse_lo_pendiente()` vale SIEMPRE cero.** Una primera versión de cerrar una división miraba solo ese sitio y declaraba cero justo en el fotograma en que se cierra una consola **que estaba volcando** — milisegundos de interpretar, publicados como coste del framework.

Los tres sitios son: **lo anotado** (`medida.pendiente`, no vacío solo si egui repitió la pasada y el pintado alcanzó a anotar), **lo que dejó el bombeo** (`pendiente_de_medir`, el caso normal) y **`panel_sin_pintar`** — lo que costó releer su `panel.json` mientras su pestaña estaba de fondo, que `cerrar_las_vueltas_no_pintadas` solo recoge **si además interpretó algo**, así que una consola parada —un agente esperando respuesta— se lo lleva puesto al fotograma siguiente. `Division::llevarse_lo_de_este_fotograma` recoge los tres.

**Y la prueba de esto se escribe mal por defecto**: plantar el valor en `medida.pendiente` y conducir el cierre sin pasar por `logic` afirma sobre un estado que el fotograma real **nunca produce**, así que pasa con el defecto dentro. Hay que plantar en los tres, con **números distintos**, y afirmar sobre la suma — con el mismo número en dos sitios, tirar uno sigue dando de sobra. Es el disfraz de [[una-prueba-que-ata-al-ayudante-no-ata-el-camino]] aplicado a un campo en vez de a una función.

**Sigue vivo en el camino hermano:** `Ventana::cerrar_la_pestaña_activa` tiene el mismo defecto —mira solo lo anotado— desde F2.1.

## Where it lives

`Ventana::cerrar_las_vueltas_no_pintadas`, `Ventana::pintar_division`, `Ventana::cerrar_la_del_foco` y `Division::llevarse_lo_de_este_fotograma` en `crates/app/src/lib.rs`; `Medida::cerrar_fotograma`, `Medida::llevarse_lo_pendiente` y `RestoDelFotograma::anotar` en `crates/app/src/medida.rs`
