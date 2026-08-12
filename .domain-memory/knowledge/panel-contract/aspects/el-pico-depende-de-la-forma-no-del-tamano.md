---
id: asp_el-pico-depende-de-la-forma-no-del-tamano_h8y96s
slug: el-pico-depende-de-la-forma-no-del-tamano
name: >-
  El pico al interpretar JSON depende de la forma del fichero, no de su tamaño:
  con los mismos 256 KiB el factor va de 3× a 95×
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:07:12.510Z'
updated_at: '2026-08-12T14:07:12.510Z'
last_verified: '2026-08-12T14:07:12.510Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/panel-contract/examples/lectura.rs
  - docs/04-contrato-panel.md
symbols:
  - MAX_BYTES
  - read
  - interpretar
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:2f88d49d982871a2367457566cc72b098caee9f6217d84e0ed9db26e29c6b7b7'
  crates/panel-contract/examples/lectura.rs: 'sha256:a8b364eb1f469d850d060adb5c8953582075b21cbb4022073e517dbf9726db8a'
  docs/04-contrato-panel.md: 'sha256:5af2cb170fb82f7c94d1f1a491cd2349f599e2bde0740449655e5fcc44a2e75f'
tags:
  - medida
  - memoria
  - fotograma
  - techo
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Medido en `--release`, con los mismos 262 KiB en cuatro formas distintas.** Lo que se reserva es **un nodo por valor**, así que el peor caso es el que mete más valores en los mismos bytes:

| Forma | Líneas | Pico | Factor | Interpretar |
|---|---|---|---|---|
| Una sola línea gigante | 1 | 769 KiB | **3,0×** | 0,1 ms |
| Líneas como las de un panel real | 1.588 | 1,78 MiB | **7,1×** | 2,8 ms |
| Objetos mínimos del contrato | 20.163 | 16,27 MiB | 65,1× | 17,1 ms |
| Objetos de una clave cualquiera | 32.766 | **23,78 MiB** | **95,1×** | **22,3 ms** |

**Un solo número no describe nada aquí, y escribir uno solo ya salió mal.** El comentario del techo decía «256 KiB llegan a unos 10 MiB de pico, unas cuarenta veces»: el caso realista es **5,6 veces más barato** y el peor **2,4 veces más caro**. Ni la cifra ni el factor correspondían a ninguna forma real.

**El caso normal no se acerca ni de lejos.** Los nueve `panel.json` que hay en el repo se leen en unos **10 µs cada uno** —de ellos 2,8 µs de llamadas al sistema— con picos de 5-11 KiB. Releer los cuatro de cuatro divisiones en cada fotograma costaría menos de la cuarta parte del 1 %.

> **La restricción para quien lea desde el bucle de pintado sigue siendo la misma —no releer en cada fotograma— pero su razón era la equivocada.** No es que leer cueste un fotograma: es que un fichero **patológico** cuesta fotograma y medio. Enunciada como «leer es caro» invita a construir una caché de invalidación contra un problema que los ficheros reales no tienen, mientras el problema de verdad —maquetar y pintar 32.766 líneas— sigue entero.

**Cuatro divisiones no multiplican el pico por cuatro**: los intermedios de cada lectura mueren antes de que empiece la siguiente, así que es un pico más tres residentes — 32 MiB en el peor caso, no 95. Y la memoria vuelve al soltar el panel.

**Descartado con su número:** derivar con `serde` en vez del valor genérico. En el caso real son **2,5 µs** de diferencia, el 0,06 % de un fotograma con cuatro divisiones. Y `untagged` no arregla nada —acumula cada elemento en su búfer interno antes de decidir la variante, casi lo mismo que construir el valor genérico—; solo un visitante a mano gana, y es **estricto**, que es justo lo que la tolerancia por campo no admite.

**Cuándo dejaría de valer:** si cambia la versión del intérprete de JSON, si se toca el techo, o si se mide en otra máquina. `cargo run --release --example lectura -p panel-contract` devuelve los números de esa máquina.

## Where it lives

El techo y sus cifras, en `crates/panel-contract/src/panel.rs` (`MAX_BYTES`). El banco que los reejecuta, en `crates/panel-contract/examples/lectura.rs`. El aviso para quien publique, en `docs/04-contrato-panel.md`.
