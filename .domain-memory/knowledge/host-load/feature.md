---
id: feat_host-load_t5sdjw
slug: host-load
name: host-load — medir cuánto vacía un terminal
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-08T21:25:53.276Z'
updated_at: '2026-08-08T21:25:53.276Z'
last_verified: '2026-08-08T21:25:53.276Z'
file_paths:
  - crates/host-load/src/lib.rs
  - crates/host-load/src/main.rs
symbols:
  - build_load
  - start_instant
  - pace_delay
  - aggregate
  - Run
content_hashes:
  crates/host-load/src/lib.rs: 'sha256:cb1d4bed2f3afd7e12c233f07ec78cbb3a75875b3776adc4bd988c8958722e7d'
  crates/host-load/src/main.rs: 'sha256:c48abf553ba48a5cdd59d228bc128248579c86bbb7e438565339aeb2721f53fc'
tags:
  - medida
  - rendimiento
  - terminal
  - D3
---

## What it does

La herramienta que convierte en repetible la medida que sostiene **D3**: cuánto tarda un terminal en **vaciar** una carga fija que se le vuelca por la salida estándar. Se lanza a mano dentro de N divisiones visibles a la vez y `report` agrega la tanda.

Existe porque el criterio del anexo «Medido» de D3 —*con N divisiones visibles produciendo a la vez, el agregado tiene que subir con N, no bajar*— no tenía forma de comprobarse desde el repo, y porque dos «Riesgos concretos» de la Fase 1 apoyan en una medida la decisión más cara de la fase (cambiar de framework de interfaz).

Tres cosas la definen, y las tres vienen de errores cometidos midiendo a mano antes de que existiera:

1. **La carga es determinista** — el contenido de la línea `i` depende solo de `i`, sin reloj ni azar. Es lo que hace comparables dos medidas de máquinas o días distintos. Y lleva color, negrita, atenuado y un repintado con movimiento de cursor **a propósito**: un solo carácter repetido recorre el camino rápido del parser y da un número que miente (mismo aprendizaje que `throughput.rs` en `term-core`).
2. **El arranque se alinea al reloj de pared** — N procesos que ni se conocen arrancan juntos sin coordinarse. Con ventana de 10 s una división se descolgó al ciclo siguiente y falseó la tanda; por eso el valor por defecto son 30 s.
3. **`report` se niega a agregar lo que no compitió** — excluye las medidas cuya salida no era un terminal (una tubería tragó 22 MiB a 3.010 MiB/s, tres órdenes de magnitud por encima de cualquier terminal) y no da agregado si las ventanas de ejecución no se solaparon. El agregado divide por el **máximo** y no por la media: la tanda ocupa el terminal hasta que termina la última división.

**El instrumento tiene un límite estructural que decide para qué sirve** — está en el aspecto asociado, y hay que leerlo antes de usar esta herramienta para cerrar ningún criterio.

No depende de `term-core` ni de ninguna otra caja: es un escritor a `stdout`, y el sujeto medido es el terminal que recibe.

## Where it lives

`crates/host-load/`. La lógica comprobable en `src/lib.rs` (`build_load`, `start_instant`, `pace_delay`, `Run`, `aggregate`); el binario y el parseo de argumentos en `src/main.rs`. El protocolo de medida y los números de referencia de WezTerm y Orca están en el `//!` de `lib.rs`. La decisión que lo origina, en `docs/02-decisiones.md`, D3, sección «Medido».
