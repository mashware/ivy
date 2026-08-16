---
id: asp_la-pasada-repetida-rebombea-y-sobrescribe-la-vuelta_95ka2b
slug: la-pasada-repetida-rebombea-y-sobrescribe-la-vuelta
name: >-
  eframe corre `logic` Y `ui` dentro del cierre que egui puede repetir: en una
  pasada repetida el bombeo rearma la vuelta y la buena se pierde en silencio
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T16:49:53.587Z'
updated_at: '2026-08-16T16:49:53.587Z'
last_verified: '2026-08-16T16:49:53.587Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/medida.rs
symbols:
  - 'Ventana::logic'
  - 'Ventana::ui'
  - 'Division::bombear'
  - 'Medida::anotar_vuelta'
  - 'Tramos::y_ademas'
  - 'Ventana::cerrar_fotograma'
content_hashes:
  crates/app/src/lib.rs: 'sha256:b27adaa55359fbc81c3b0a62f6993d4aecce7bd71aeff4f8faee1484d59dbe87'
  crates/app/src/medida.rs: 'sha256:1c09d77a593aaa0cb14679e7508ba91dad93bc864aa84fbb4f4a4378d4f0e069'
tags:
  - medida
  - egui
  - eframe
  - instrumentacion
  - D3
  - F2.1
feature_id: feat_term-core_nx0g1r
---

## What it does

**El hecho de la API, leído y no supuesto:** `eframe` llama a `App::logic` y a `App::ui` **dentro del mismo cierre** que `Context::run_ui` puede repetir — `eframe-0.36.1/src/native/epi_integration.rs:288-303`, con los dos `profiling::scope!` dentro del cierre. La intuición natural, y escrita en este proyecto antes de comprobarla, es que `logic` corre una vez por fotograma y `ui` puede correr dos. **Es falsa: corren los dos por pasada.** Un caso más de [[la-api-ajena-se-lee-no-se-supone]].

**La consecuencia era un sesgo vivo en la medida, no una curiosidad.** En un fotograma con la pasada repetida —egui la repite cuando algo pide recomponerse—: (1) `bombear_todas` corre otra vez y `Division::bombear` **rearma** `pendiente_de_medir` con un interpretado de ~0; (2) `Medida::anotar_vuelta` hacía `self.pendiente = Some(tramos)`, o sea **sobrescribía**; (3) la vuelta buena de la primera pasada se perdía y lo que la consola interpretó de verdad se publicaba como «resto del fotograma», que es coste del framework — la cifra exacta con la que **D3** juzga a egui. Medido durante la revisión: **declaró 50 ns cuando interpretar había costado 20,158 µs**.

Le pasaba igual a la consola **pintada** (`pintar_division` también anota una vez por pasada) y es **anterior a las pestañas**: no lo introdujo ningún PR de F2.1, solo lo hizo visible.

**Arrastraba un compañero:** la guarda que evita cerrar el fotograma dos veces usaba `ctx.cumulative_pass_nr()`, que sube **por pasada** y no por fotograma, así que en un fotograma multipasada no filtraba nada. `cumulative_frame_nr` es el de fotograma.

**Arreglado en el PR #65 (`d9d8fec`), los dos juntos:** `anotar_vuelta` ahora **suma** en vez de asignar, mediante `Tramos::y_ademas`, que acumula los tramos de las dos pasadas y conserva el `esperado` de la primera —la espera es del fotograma, no de la pasada, y sumarla contaría dos veces el mismo hueco—; y la guarda pasó a `cumulative_frame_nr`.

> **La regla que deja:** en un instrumento cuyo tramo pendiente se **asigna** en vez de acumularse, cualquier camino que pueda ejecutarse dos veces por fotograma pierde la muestra buena en silencio. La forma segura de añadir un tramo a una vuelta es meterlo **dentro** de la que ya existe, nunca anotar una vuelta al lado.

**Y una lección de prueba, porque la mutación atravesó tres:** dos afirmaban sobre el campo donde se apunta el coste en vez de sobre el efecto que tiene —apuntarlo sin restarlo del total dejaba la suite verde— y una comparaba milisegundos donde, en modo depuración, encolar el pintado ya satura el margen elegido.

## Where it lives

`crates/app/src/medida.rs` — `Medida::anotar_vuelta`, `Tramos::y_ademas`. `crates/app/src/lib.rs` — `Ventana::logic`, `Ventana::ui`, `Division::bombear`, `Ventana::cerrar_fotograma`. El hecho de la API, en `eframe-0.36.1/src/native/epi_integration.rs:288-303` del registro local de cargo.
