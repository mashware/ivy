---
id: asp_el-raton-no-llega-al-programa-de-dentro_x550d9
slug: el-raton-no-llega-al-programa-de-dentro
name: >-
  El ratón no se reenvía al programa de dentro: una guarda para que «no se le
  cuele» protege de nada, y su prueba pasa por construcción
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:06:38.805Z'
updated_at: '2026-08-17T20:06:38.805Z'
last_verified: '2026-08-17T20:06:38.805Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - atender_el_raton
  - arrastrar_la_frontera
content_hashes:
  crates/app/src/lib.rs: 'sha256:bd1d056ad4bcd72e1edfbea41ecd1ed51b925068d136aabb02b79da1032b45e3'
tags:
  - egui
  - entrada
  - pruebas
  - yagni
feature_id: feat_term-core_nx0g1r
---

## What it does

**Los acordes de teclado se filtran a propósito para que no lleguen al pseudoterminal** —D13 lo dice así: «ninguno de esos acordes le llega al programa de dentro»— y de ahí sale una intuición que parece obvia y es **falsa**: que un gesto de ratón necesita el mismo filtro.

**No lo necesita, porque el ratón no llega.** En el crate `app` no existe ningún camino que mande eventos de ratón al pseudoterminal. El reenvío que esperan las TUI —los informes SGR/X10— **no está implementado**, y es materia de F2.6.

> **La consecuencia es la que muerde: un criterio de aceptación del tipo «el programa de dentro no recibe el gesto» se cumple POR CONSTRUCCIÓN, y su prueba pasaría sin atar nada.** Es el tercer disfraz fichado del proyecto —la prueba que pasa porque el escenario no puede ocurrir, no porque la guarda funcione— con el agravante de que aquí ni siquiera hay guarda que probar: es un mecanismo entero, con su criterio de aceptación, defendiendo un escenario inexistente.

**La regla que queda:** antes de filtrar una entrada «para que no se le cuele al programa», comprobar que esa clase de entrada le llega hoy. **Teclado sí, ratón no.** Y cuando F2.6 implemente el reenvío del ratón, este camino hay que volver a mirarlo: entonces la guarda sí tendrá escenario.

## Where it lives

Detectado en el diseño de F2.3 (issue 76) al retar el propio diseño y comprobarlo en el código en vez de razonarlo; el mecanismo y su criterio se cortaron antes de escribir una línea. Evidencia: búsqueda de `PointerButton`, `pointer_button` y `mouse` en `crates/app/src/lib.rs` sobre `origin/main` — todas las apariciones de `egui::Event::PointerButton` caen dentro del módulo de pruebas, construyendo clics sintéticos.
