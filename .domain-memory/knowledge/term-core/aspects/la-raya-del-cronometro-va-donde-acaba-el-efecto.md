---
id: asp_la-raya-del-cronometro-va-donde-acaba-el-efecto_2g7jfp
slug: la-raya-del-cronometro-va-donde-acaba-el-efecto
name: >-
  La raya del cronómetro va donde acaba el EFECTO, no donde vuelve la llamada —
  y el nombre de la función miente sobre dónde está esa raya
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T11:01:01.360Z'
updated_at: '2026-08-24T11:01:01.360Z'
last_verified: '2026-08-24T11:01:01.360Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/medida.rs
  - crates/app/src/sesion.rs
  - docs/02-decisiones.md
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:695b2cbd997807c09dca51b084938c643b9c16d416f9dfbc830ffa69ae766479'
  crates/app/src/medida.rs: 'sha256:635e8c4744515c06065172fe15fa48e988d3d1f8d39a906942d9e25212a5cfe7'
  crates/app/src/sesion.rs: 'sha256:e830c75bff3ba304a58835e5647deeb2ef55331849d5092066f7d021d49030c6'
  docs/02-decisiones.md: 'sha256:2d9d5fed62b28f810dee8f7e3130a4e54dea4f09de41cde4dcd5ae464dc835ae'
tags:
  - medida
  - cronometro
  - mutacion
  - pruebas
  - f2.5
feature_id: feat_term-core_nx0g1r
---

## What it does

**El caso, con su número.** El anexo de D16 publicó **4,45 ms** como «lo que cuesta abrir la ventana con las consolas que dejaste». El cronómetro cerraba justo después de `Ventana::materializar`, o sea **cuando los procesos existían**. Detrás quedaban dos trabajos que **crecen con el número de consolas** y que nadie contaba: armar el vigilante del directorio de cada consola y recorrer el árbol entero para el primer espejo. Vueltas a medir con la raya bien puesta: **5,42 ms**. **El 15 % del número estaba fuera del número.**

> **La raya de un cronómetro no va donde vuelve la llamada que da nombre al tramo: va donde el efecto que el tramo promete está completo.** Y el nombre de la función es justo lo que empuja a ponerla mal: `materializar` suena a «aquí acaba materializar», así que la raya se pone ahí sin preguntarse qué más hace falta para que la promesa del tramo —«la ventana abierta con esas consolas»— sea cierta.

**Cómo se cazó, que importa tanto como el hallazgo:** un revisor de código leyendo el diff, no una prueba. **Esto no es atable por tiempo** y la razón está medida: los dos trabajos que faltaban son microsegundos frente a los milisegundos de los `fork`, así que cualquier umbral que distinguiera «paró antes» de «paró después» sería un número injustificable que fallaría en una máquina cargada — la misma conclusión a la que llegó [[plantar-se-ata-cronometrar-no-siempre]] por otro camino. **El sostén de esa raya es el comentario que explica dónde está y por qué, no una prueba.**

**Y un segundo hallazgo del mismo día, sobre cómo se prueba un cronómetro:**

> **`is_some()` sobre un tiempo no ata nada: `Some(0)` también es algo.** La primera versión de la prueba de `Persistencia::tardo_en_cargar` afirmaba que había número; su propia mutación —cambiar el cronómetro por `Duration::ZERO`— **la dejó en verde**. Hay que exigir **`> Duration::ZERO`**, que es lo que el defecto no puede fingir. Es el disfraz que `CLAUDE.md` nombra —«elegir números, un cero, con los que el defecto y el arreglo dan el mismo resultado»— aplicado al instrumento que mide.

**Y una tercera, más fina, sobre dos cronómetros vecinos:** afirmar `> 0` sobre cada uno **no impide cruzarlos**. Cablear el tiempo de leer el fichero al cronómetro de abrir las consolas pasa las dos afirmaciones. Lo que lo ata es la **igualdad** con la fuente (`declarada.leer == persistencia.tardo_en_cargar()`), y el número que lo justifica salió de la propia mutación: leer cuesta ~104 µs y abrir tres consolas ~3,9 ms, **37 veces más**.

**Y una consecuencia de producto que no era de medida:** el veto de escritura de las sesiones de medida miraba solo las dos variables que piden consolas, así que la tanda que **restaura** —que no las pone a propósito, para que mande el fichero— era **la única sesión de medida capaz de pisar la disposición del usuario**, y su propia tanda de control no lo era. Una asimetría entre dos tandas que se comparan fila por fila. Ahora veta cualquiera de las cinco variables de la puerta de medida.

## Where it lives

El cronómetro y su raya: `Ventana::abriendo`, rama `ConQueAbrir::Restaurada`, en `crates/app/src/lib.rs`. El tipo que se publica: `medida::Restauracion` (`consolas`, `leer: Option<Duration>`, `abrirlas`). El tiempo de lectura: `sesion::Persistencia::tardo_en_cargar`. Los números y la autopsia del 15 %, en el anexo «Medido — 24 agosto 2026» de **D16** en `docs/02-decisiones.md`. Ver también [[la-vuelta-de-medida-se-cierra-al-pintar]] y [[una-prueba-que-ata-al-ayudante-no-ata-el-camino]].
