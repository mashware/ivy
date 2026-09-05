---
id: asp_lo-que-se-rompe-sin-que-falle-nada_2ne2h4
slug: lo-que-se-rompe-sin-que-falle-nada
name: >-
  Tres roturas que ni el compilador ni la suite ven: la longitud como carga de
  prueba, las claves que emparejan por texto, y el `assert` de dos mitades que
  se editan juntas
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:13:22.483Z'
updated_at: '2026-09-03T16:13:22.483Z'
last_verified: '2026-09-03T16:13:22.483Z'
file_paths:
  - crates/app/examples/screen_demo.rs
  - crates/app/src/fonts.rs
  - crates/app/src/session.rs
  - crates/app/src/measure.rs
symbols:
  - ICON_FAMILY
  - SYSTEM_PREFIX
  - FILE
content_hashes:
  crates/app/examples/screen_demo.rs: 'sha256:49fe72a81b94223134f9eb25135cbc3032e3bde5ddaee2415b6c0ed57673b422'
  crates/app/src/fonts.rs: 'sha256:e64e2947f2bc20575da8a4a16877b532af56a559637f28262753778f7df63c98'
  crates/app/src/session.rs: 'sha256:b7824b96a3e438e7695ed87ccecbe9d65ed9dd16258b5e74ba3c39c8008f09e3'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
tags:
  - d27
  - f2.12
  - guardas
  - mutacion
  - claves-de-ejecucion
feature_id: feat_migracion-a-ingles_xnx8mc
---

## What it does

**Los tres salieron buscando qué podía romper un renombrado, y ninguno es un renombrado.**

### 1. Literales cuya LONGITUD es la carga de prueba

`crates/app/examples/screen_demo.rs` construye la pantalla con la que se mira el pintor a ojo, y algunas de sus filas están calculadas para que **un carácter ancho caiga justo en el borde de la columna 80**. Su propio comentario lo dice: sin esa fila, la rama del pintor que trata ese hueco no la mira nadie. **Ese fichero tiene cero `#[test]`.**

> Traducir ese texto —o simplemente reescribirlo— mueve el carácter ancho de sitio y **desactiva la rama en silencio**: no falla nada, no hay prueba que caer, y el hueco solo se ve mirando la pantalla. Un texto de prueba así **se mide en columnas, no en palabras**.

Así se trató: la entrega que tradujo `app` declaró ese fichero **isla**, tradujo sus comentarios y **no tocó un literal**. Verificado al revisarla: los literales llegaron idénticos y quedan 78 celdas antes del carácter ancho. Lo que sigue sin existir es la guarda: **nada fija ese ancho**, y lo más barato sería un `#[cfg(test)]` dentro del propio ejemplo.

### 2. Constantes que son claves de tiempo de ejecución

`ICON_FAMILY = "iconos-del-sistema"` y `SYSTEM_PREFIX = "sistema:"` se registran como **texto** en el motor de fuentes y se buscan por ese mismo texto. Renombrar la constante sin el valor —o al revés— rompe el emparejado **al ejecutar, y compila perfectamente**. No hay tipo que medie.

> **Y aquí hay una corrección que conviene que quede escrita, porque el registro del hito llegó a afirmar lo contrario.** Se apuntó que renombrarlas «podría dejar de pintar los iconos sin que nada lo cace», y **es falso**: todos los consumidores pasan por el símbolo, así que un renombrado está comprobado por el compilador, y `fonts.rs:1609` fija `"sistema:Hack"` a mano mientras producción lo construye desde `SYSTEM_PREFIX` — renombrarlo se pone **rojo inmediatamente**. La guarda existe. Aplazarlas por disciplina de alcance es razonable; aplazarlas por ese motivo engaña a quien recoja el trabajo.

### 3. Una prueba que parece la red y es un disfraz

Había un `assert_eq!(FILE, "layout.json")`. Parece atar el nombre del fichero y **no ata nada de lo que importa**: afirma que la constante vale un literal escrito **al lado**, no que valga lo que el guion de medida siembra en disco. Se renombran las dos mitades del `assert`, la suite queda verde, y el guion sigue escribiendo el nombre viejo.

**Cerrado, y el arreglo es el patrón a copiar**: `measure.rs:2879` afirma `SCRIPT.contains(crate::session::FILE)` sobre el texto del guion leído con `include_str!` — una mitad **derivada** de la otra en vez de dos mitades escritas juntas. Su comentario explica el disfraz que sustituye.

### La regla que dejan las tres juntas

> Cuando se busque «qué se rompe sin que falle nada» en este proyecto, no basta con mirar tipos y literales. Hay que preguntar también **quién empareja dos cadenas en tiempo de ejecución** y **qué texto tiene una longitud significativa** — y desconfiar de toda prueba que compare dos mitades que el mismo editor puede cambiar a la vez.

## Where it lives

`crates/app/examples/screen_demo.rs` — las filas cuya longitud es la carga, sin una sola prueba en el fichero.

`crates/app/src/fonts.rs` — `ICON_FAMILY`, `SYSTEM_PREFIX`, la textura de color, y en `:1609` la aserción que fija el prefijo a mano y hace de guarda sin declararse como tal.

`crates/app/src/measure.rs:2879` — el patrón a copiar: la mitad derivada de la otra.

`docs/02-decisiones.md` — **AC14** del hito F2.12 (las claves de tiempo de ejecución siguen emparejando después del renombrado), que sigue sin cerrarse.
