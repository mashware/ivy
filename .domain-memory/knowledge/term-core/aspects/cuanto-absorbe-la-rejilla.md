---
id: asp_cuanto-absorbe-la-rejilla_4jamm7
slug: cuanto-absorbe-la-rejilla
name: >-
  Cuánto absorbe la rejilla en un volcado masivo, y qué presupuesto deja por
  frame
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-07T19:58:41.554Z'
updated_at: '2026-08-07T19:58:41.554Z'
last_verified: '2026-08-07T19:58:41.554Z'
file_paths:
  - crates/term-core/examples/throughput.rs
  - crates/term-core/src/screen.rs
  - docs/05-roadmap.md
symbols:
  - 'Screen::advance'
content_hashes:
  crates/term-core/examples/throughput.rs: 'sha256:114cac38ded03ca2b308e5cd055560ec9226105303f1aecc74c6bda8430318ca'
  crates/term-core/src/screen.rs: 'sha256:f940fcd9032bb9284759fdadb03ff4f1606d548d260b538bc389d35f44b13f02'
  docs/05-roadmap.md: 'sha256:6fe1601f30d97065dad492557fd89921971490411bcf276336b7c3129a8bd7d4'
tags:
  - rendimiento
  - fase-1
  - medicion
  - rejilla
feature_id: feat_term-core_nx0g1r
---

## What it does

**Medido, no estimado:** `Screen::advance` absorbe **64,5 MiB/s** en `--release` — 10 MiB en 155 ms, alimentada en trozos de 8 KiB (el mismo tamaño que el búfer de lectura del PTY), lo que da **~121 µs por trozo**. En compilación de depuración sale varias veces peor; el número que vale es el de `--release`.

Se midió con un patrón que lleva saltos de línea y escapes de color a propósito: alimentar el parser con un solo carácter repetido recorre su camino rápido y da un número que miente sobre el caso real.

**Para qué sirve el número.** El riesgo del volcado masivo (un `cat` de un fichero grande) se decidió no mitigarlo dentro de la caja: `term-core` no impone ningún tope, y quien consume decide cuánto procesa por pasada. Esa decisión solo es sensata si el tope se elige con un dato. Traducción directa: con un presupuesto de **2 ms por frame** caben ~16 trozos, es decir **~130 KiB por frame**; un `cat` de 10 MiB tardaría ~80 frames, algo más de un segundo a 60 fps, **sin perder ninguno**.

Quien tiene que elegir ese presupuesto es F1.3, cuando exista un bucle de pintado de verdad. Lo que este dato le quita es la ceguera, no la decisión.

**Cuándo dejaría de valer:** si cambia la versión del parser, si el trozo de lectura deja de ser 8 KiB, o si se mide en una máquina distinta. Volver a correr `cargo run --release --example throughput -p term-core` cuesta 155 ms y devuelve el número de esa máquina.

## Where it lives

`crates/term-core/examples/throughput.rs` es la medición, reproducible con `cargo run --release --example throughput -p term-core`. El riesgo que motiva todo esto está declarado en `docs/05-roadmap.md`, tabla de riesgos concretos, asignado a F1.2.
