---
id: asp_el-vaciado-no-ve-el-pintado_tefp5s
slug: el-vaciado-no-ve-el-pintado
name: El vaciado del PTY no sirve como criterio de nuestro propio terminal
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-08T21:26:20.297Z'
updated_at: '2026-08-08T21:26:20.297Z'
last_verified: '2026-08-08T21:26:20.297Z'
file_paths:
  - docs/05-roadmap.md
  - docs/02-decisiones.md
  - crates/host-load/src/lib.rs
symbols: []
content_hashes:
  docs/05-roadmap.md: 'sha256:fb6628f3b9309687a4d815b927ef4c9ae979a2c270414ca49dbbfb0f06cfa6e1'
  docs/02-decisiones.md: 'sha256:755c58ddfca86a83e18f70795c4477ede05615e490fd9f1e807cd210776d6ded'
  crates/host-load/src/lib.rs: 'sha256:cb1d4bed2f3afd7e12c233f07ec78cbb3a75875b3776adc4bd988c8958722e7d'
tags:
  - D3
  - F1.3
  - medida
  - límite-del-instrumento
feature_id: feat_host-load_t5sdjw
---

## What it does

Medir cuánto tarda un terminal en **vaciar el PTY** es un instrumento válido para **comparar terminales ajenos**, pero **no sirve como criterio de aceptación de nuestra propia app**. La razón no es un matiz: es estructural, y quien vuelva aquí a «cerrar el criterio de D3» con esta herramienta se estaría engañando.

**El instrumento no ve el pintado.** Un terminal que traga rápido y luego descarta frames sale bien en el número y mal a la vista. Y **la mitigación que el roadmap ya planifica para F1.3 es exactamente eso**: «repintar solo cuando el PTY produzca cambios», o sea coalescer frames. Nuestra app aprobaría la prueba **por construcción**, midiendo bien precisamente por hacer lo que la prueba no mira.

**El pintado es inobservable desde el proceso que produce la carga.** Ningún protocolo de terminal reporta un frame presentado; el ida y vuelta de `ESC[6n` (DSR) solo demuestra *consumido y parseado* —quita la holgura del búfer del PTY— y no que se haya dibujado nada. El único sitio donde se puede ver es **dentro de `app`**: bytes procesados por frame, frames presentados, retraso del último frame respecto a la última entrada. Eso es F1.3/F1.4 y no se puede construir antes que el bucle de pintado.

**Contra quién sí decide.** Contra terceros, y con holgura: Orca falló ya en el vaciado — 8,6 MiB/s con una división y 7,1 agregados con cuatro, o sea **escalado negativo**, mientras WezTerm pasaba de 28,4 a 85,7. No hizo falta ver el pintado para descartar ese stack.

**Un segundo límite, más pequeño pero fácil de pisar:** los dos regímenes de la herramienta no se comparan entre sí. Sin `--rate` el MiB/s responde «cuánto da»; con `--rate` el tiempo incluye las esperas, así que responde «¿aguantó el caudal?» — medido, `--rate 10` con consumidor rápido devuelve 10,0 MiB/s clavados y con uno lento cae a 0,3.

## Where it lives

Escrito en tres sitios a propósito, porque es lo que evita la conclusión equivocada: el `//!` de `crates/host-load/src/lib.rs`, el anexo «Medido» de D3 en `docs/02-decisiones.md`, y los «Riesgos concretos» de la Fase 1 en `docs/05-roadmap.md`, de donde sale la frase sobre repintar solo cuando el PTY produzca cambios.
