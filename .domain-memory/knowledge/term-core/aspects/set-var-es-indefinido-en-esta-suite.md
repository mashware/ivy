---
id: asp_set-var-es-indefinido-en-esta-suite_m3g7dt
slug: set-var-es-indefinido-en-esta-suite
name: >-
  En esta suite `std::env::set_var` es comportamiento indefinido: cada consola
  que se lanza recorre el entorno entero, y las pruebas corren en paralelo
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:24:59.175Z'
updated_at: '2026-08-15T21:24:59.175Z'
last_verified: '2026-08-15T21:24:59.175Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/term-core/src/lib.rs
symbols:
  - set_var
  - 'Division::nueva'
  - CommandBuilder
  - arrancar
content_hashes:
  crates/app/src/lib.rs: 'sha256:b3ca4328c99c0b94b314ea9a6c6d1ab4f6afe0dad22ef3415700f289bc9af411'
  crates/term-core/src/lib.rs: 'sha256:6d8ac25cbaae68a1d230090cc4c349577ba67801bee3f7d5de632814ca3a4289'
tags:
  - pruebas
  - entorno
  - F2.1
feature_id: feat_term-core_nx0g1r
---

## What it does

**El contrato de `std::env::set_var` es sobre TODO el entorno, no sobre la clave que se toca**: con una escritura en vuelo, cualquier **lectura** concurrente es comportamiento indefinido. La justificación intuitiva —«puedo tocar esta variable porque ninguna otra prueba la lee»— **es falsa**, y se escribió en este proyecto antes de que una revisión la tumbara.

**Y esta suite lee el entorno a mansalva y en paralelo:**

- `Division::nueva` lee `SHELL` — y hay decenas de pruebas que abren divisiones.
- `portable_pty` recorre **`std::env::vars_os()` al construir cada `CommandBuilder`**, o sea **una vez por consola lanzada** (`cmdbuilder.rs:75`).
- No hay `--test-threads=1`, ni configuración de nextest, ni `serial_test` en el repo: `cargo test` reparte en hilos.

El síntoma no es un fallo limpio: son **cuelgues o entornos de hijo corruptos, intermitentes y en pruebas que no tienen nada que ver** con la que tocó la variable.

> **La forma de convivir con una variable de entorno en un proyecto que lanza procesos: partir el parseo de la lectura.** Una función pura recibe el valor y se prueba entera; una línea sola lo lee del entorno y **no la cubre ninguna prueba**, dicho en el propio código. Es la misma frontera que ya se aceptó para `on_exit`: hay sitios donde la prueba no puede entrar y lo honesto es escribirlo, no fingir cobertura.

Aplicado en F2.1 con `MEDIR_PESTANAS`, la puerta que abre N pestañas al arrancar: `cuantas_pestañas(Option<&str>)` se prueba, `pestañas_al_arrancar()` no, y quien la lee es `arrancar()`. Lo que tapa esa línea no es una prueba — es el uso: quien mide ve una sola pestaña en cuanto abre la ventana.

## Where it lives

`cuantas_pestañas` / `pestañas_al_arrancar` / `arrancar` en `crates/app/src/lib.rs`; el lector de entorno ajeno está en `portable-pty`, `src/cmdbuilder.rs:75`
