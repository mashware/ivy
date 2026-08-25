---
id: asp_el-frio-ocurre-una-vez-por-corrida_gjzzwd
slug: el-frio-ocurre-una-vez-por-corrida
name: >-
  Un número «en frío» no se promedia dentro de una corrida: hay UNA muestra, y
  su dispersión solo se ve repitiendo la corrida
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T13:58:59.656Z'
updated_at: '2026-08-25T13:58:59.656Z'
last_verified: '2026-08-25T13:58:59.656Z'
file_paths:
  - crates/app/examples/fuentes.rs
  - docs/02-decisiones.md
  - scripts/medir-fotograma.sh
symbols:
  - resumen
  - MUESTRAS
content_hashes:
  docs/02-decisiones.md: 'sha256:0de8ae72f3ac308cb5cc9ba4d96b4b7d71443f38da0b0ca87bafa69318fc44ce'
  scripts/medir-fotograma.sh: 'sha256:b41b8d91a5fb47b233a78a5da664f8d0563c96031a5d9556d93510866c9663ab'
tags:
  - medida
  - bancos
  - F2.9
  - D24
feature_id: feat_term-core_nx0g1r
---

## What it does

**Un banco que toma N muestras por punto NO acota su número en frío, y publicarlo como si lo hiciera es el error que este proyecto castiga.** El frío **ocurre una vez por corrida**: la muestra 1 llena las cachés —de ficheros, de búsqueda por script— y las demás ya las encuentran llenas. Así que dentro de una corrida **no hay nada que promediar en frío**: hay **una** muestra, con toda la varianza del sistema encima.

**Y suele ser justo la que decide.** En **D24** el número que sostiene la decisión —registrar una cara con la ventana andando— se paga **siempre en frío**, una vez por escritura nueva.

### Medido

Tres corridas del mismo binario **con la máquina en reposo** dieron **3,4 / 4,8 / 4,5 ms** para ese punto. La primera versión de D24 publicaba **3,2 ms** como *el* valor: **por debajo de los tres**. Otro número —enumerar en caliente, 13,7 ms— **no se ha vuelto a ver en cuatro corridas** (15,07 · 15,10 · 15,17 · 15,12).

**El contraste es el dato:** la columna **caliente** varía menos del **1 %** entre corridas y la **fría** hasta un **40 %**. Lo que varía es **abrir ficheros**, no recorrer listas.

### La regla que deja

1. **La columna fría se publica como RANGO de corridas**, no como un valor. Y se dice cuántas corridas lo sostienen.
2. **Subir el número de muestras no mejora el frío** — la segunda muestra ya encuentra las cachés llenas. Lo único que lo mejora es **repetir la corrida**.
3. **La cabecera del banco tiene que decir qué es cada columna.** La del banco de fuentes decía «mediana de 5 muestras, con los extremos entre paréntesis» cuando ni el frío era una mediana ni se imprimía ningún extremo. Es la primera línea que lee quien repite la medida.
4. **Y hay que decir si la decisión aguanta el rango entero.** En D24 sí: el peor caso de la vía perezosa (5,7 ms) contra el **mejor** del arranque (23,5 ms) siguen siendo más de cuatro veces, así que ninguna combinación de los extremos le da la vuelta.

### Corolario para cualquier medida de este repositorio

**Una corrida con la máquina compilando en paralelo no sirve.** El mismo punto dio **5,08 ms con carga** contra 3,4–4,8 en reposo. Hay que esperar a que no queden `cargo` ni `rustc` vivos antes de cronometrar nada.

## Where it lives

El banco que lo destapó es `crates/app/examples/fuentes.rs` —su función `resumen` es la que separa la muestra fría de la mediana de las calientes, y tiene sus dos pruebas—. Los rangos publicados están en **D24** de `docs/02-decisiones.md`. Para las tandas del fotograma, el guión es `scripts/medir-fotograma.sh`.
