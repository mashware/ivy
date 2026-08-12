---
id: asp_el-peor-caso-de-pintar-es-el-inverso-del-de-leer_v28vha
slug: el-peor-caso-de-pintar-es-el-inverso-del-de-leer
name: >-
  El peor caso de pintar un panel es el inverso del de interpretarlo, y el techo
  del contrato no acota el de pintar
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T15:18:25.660Z'
updated_at: '2026-08-12T15:18:25.660Z'
last_verified: '2026-08-12T15:18:25.660Z'
file_paths:
  - crates/app/src/panel.rs
  - crates/panel-contract/src/panel.rs
  - docs/04-contrato-panel.md
symbols:
  - MAX_CARACTERES
  - MAX_BYTES
  - sanear
  - pintar
content_hashes:
  crates/app/src/panel.rs: 'sha256:b229abf065c777de9460a87101cb6343d6f7e56eddac128b0f4f261f5d440247'
  crates/panel-contract/src/panel.rs: 'sha256:024c1d104e63a6ddc80b1dc2596686d28bff05a93cc3f44ef57dc86b59e4ff0a'
  docs/04-contrato-panel.md: 'sha256:3db1e8ddfe5ca04a53e88131e5eb07c1b1600c8d00d81b5df051fb5baee44edd'
tags:
  - medida
  - fotograma
  - entrada-no-confiable
  - egui
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Dos costes distintos, con peores casos opuestos, y un solo techo que solo acota uno.**

El techo de 256 KiB del contrato acota lo que un fichero hace reservar **al interpretarse**, y su peor caso son treinta mil objetos diminutos ([[el-pico-depende-de-la-forma-no-del-tamano]]). **Pintar tiene el peor caso al revés**: medido en `--release`, una sola línea de 260.000 caracteres —254 KiB, holgadamente bajo el techo, y de las **baratas** de interpretar (0,1 ms)— cuesta **65,8 ms de maquetado la primera vez y 14,6 ms de teselado en CADA fotograma** mientras siga publicada, con **1,04 millones de vértices por fotograma** hacia la tarjeta. El presupuesto del fotograma son 16,7 ms y las dos consolas ya gastan 5,92. Con cuatro divisiones, por cuatro.

> **Y el recorte al rectángulo no salva.** epaint hace su descarte **por fila, no por glifo** (`tessellator.rs:2050`, con su propio comentario explicando por qué). Una fila que empieza dentro del recorte se copia entera a la malla aunque se salga por la derecha. Recortar el pintor da corrección visual, no coste acotado.

**La regla que deja:** cada vez que se estrena un consumo nuevo de una entrada de fuera —leer, interpretar, maquetar, teselar— hay que preguntarse **cuál es su peor caso**, porque no tiene por qué ser el del consumo anterior. Un techo heredado da una falsa sensación de cobertura: el fichero que el contrato llama inofensivo era exactamente el que congelaba la ventana.

**Y el otro caracter de control que muerde, también medido en la fuente:** `break_on_newline` viene puesto por omisión en los trabajos de maquetado que usa `Painter::text` (`text_layout_types.rs:107`). Una sola cadena con un `\n` **pinta varias filas**, y un bucle que avanza una fila por elemento escribe encima de lo que viene después. Un tabulador es el caso gemelo: avanza el ancho de varias celdas mientras la medida de anchura lo cuenta como cero.

**Cómo se acota, y dónde.** En la frontera, antes de llamar al pintor: sustituir los caracteres de control por un espacio —no borrarlos, o dos palabras separadas por un salto acaban leídas como una— y **cortar por número de caracteres**, holgado para no adelantar el envolvido al ancho. El corte por caracteres es más barato que maquetar con límite y no depende de medir glifos.

## Where it lives

`crates/app/src/panel.rs`: la constante del tope y la función que sanea, con su prueba `una_linea_absurdamente_larga_se_corta_antes_de_pintarse`. El aviso para quien publique, en `docs/04-contrato-panel.md`.
