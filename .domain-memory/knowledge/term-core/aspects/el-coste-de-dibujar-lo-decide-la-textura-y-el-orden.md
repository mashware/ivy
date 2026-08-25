---
id: asp_el-coste-de-dibujar-lo-decide-la-textura-y-el-orden_8c264j
slug: el-coste-de-dibujar-lo-decide-la-textura-y-el-orden
name: >-
  Lo que cuesta dibujar un fotograma lo decide qué textura usa cada forma y en
  qué orden se encolan: meter cada emoji en su propia textura casi triplicó la
  cifra con la que se juzga a egui
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T17:16:09.795Z'
updated_at: '2026-08-25T17:16:09.795Z'
last_verified: '2026-08-25T17:16:09.795Z'
file_paths:
  - crates/app/src/fuentes.rs
  - crates/app/src/pintor.rs
  - docs/02-decisiones.md
symbols:
  - Glifos
  - pintar
  - pintar_a_color
  - LADO_DEL_ATLAS
content_hashes:
  crates/app/src/fuentes.rs: 'sha256:d6d2b2e362c00d1b61b89b14f41b98bc0d7c28513c69d568685c156cc1730c74'
  crates/app/src/pintor.rs: 'sha256:109c7e330be5065d3b14b62eafcd3fa4c3b54cd36122350c39c71084337b8a87'
  docs/02-decisiones.md: 'sha256:d4108883d45707cfa55aee07a21e7083a329aba25d304ecd5269a601f8a84e88'
tags:
  - rendimiento
  - egui
  - epaint
  - emoji
  - atlas
  - medida
feature_id: feat_term-core_nx0g1r
---

## What it does

**El teselador de epaint no puede juntar en la misma malla dos formas con `texture_id` distinto** —lo dice él mismo en su fuente: «Mixing different `texture_id` in the same…»—. Así que el coste de dibujar un fotograma no depende solo de cuántas formas se encolan: depende de **cuántas veces cambia la textura entre formas consecutivas**.

**Medido en F2.9 (agosto de 2026)**, dos consolas volcando emojis contra su control en ASCII, mismo binario y mismo día. El tramo es «el resto del fotograma», que es la cifra con la que **D3** juzga al framework:

| | Dos consolas volcando emojis |
|---|---|
| Una textura **por glifo** | **743,4 µs** |
| Atlas único, pintado entremezclado | 565,9 µs |
| Atlas único **y dos pasadas de pintado** | **356,3 µs** |
| *Control: las mismas dos consolas en ASCII* | *268,8 µs* |

### Las dos mitades, y por qué hacen falta las dos

**1. Una textura para todos.** Con una por emoji, cada emoji de la pantalla parte el lote. Un atlas propio —512×512, colocación por filas, `set_partial` para subir solo el rectángulo nuevo— los pone a todos bajo el mismo `texture_id`.

**2. Y pintarlos JUNTOS, que es la mitad que se olvida.** El motor junta formas **consecutivas**. Un pintor de terminal recorre la rejilla celda a celda —letra, letra, emoji, letra— así que con atlas el lote seguía partiéndose, ahora entre el atlas de fuentes y el de emojis. La solución es una pasada por tipo: **todos los textos y después todos los emojis**. Eso es lo que baja de 566 a 371.

### El techo dejó de ser un contador, y es mejor

El criterio pedía «la memoria de los glifos rasterizados tiene techo declarado». Con atlas el techo es **una textura de tamaño fijo**: **1 MiB y no cambia**, aparezca un emoji o mil. Un contador habría dado el mismo límite con memoria creciendo hasta él.

### Lo que hay que mirar de la tabla no es la diferencia, es el control

**268,8 µs con el binario nuevo contra los 274,3 que publicó el hito anterior.** Una pantalla sin emojis **no paga nada** por que el terminal sepa pintarlos, ni siquiera la pasada de más. Ese número es el que hace aceptable el resto de la tabla; sin él, la tanda con emojis sola no dice si hay regresión o no.

### Y una lección sobre los propios números

**Dos corridas de la misma tanda con el mismo binario dan 280 y 301 µs.** Cualquier diferencia menor que esos 21 µs no se puede afirmar. Una versión de este hito publicaba «28 µs medidos» para lo que costaba la pasada de más, y **repetir la corrida lo desmintió**: el número se corrigió a «por debajo de lo que el banco distingue», con el argumento de forma que sí lo sostiene. Es la segunda vez en el mismo hito que un número salió mal etiquetado, y las dos las cazó repetir la medida, no leer el código.

## Where it lives

El atlas y su colocación viven en `crates/app/src/fuentes.rs` (`Glifos`, `LADO_DEL_ATLAS`); las dos pasadas, en el bucle de `pintar` en `crates/app/src/pintor.rs`. La resta entera, con las cuatro filas de la tabla y su control, está en **D25** de `docs/02-decisiones.md`. La tanda que lo mide es la `Q` de `scripts/medir-fotograma.sh`, cuyo control es la `C`.
