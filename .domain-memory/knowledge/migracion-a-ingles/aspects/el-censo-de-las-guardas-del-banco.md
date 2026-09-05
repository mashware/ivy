---
id: asp_el-censo-de-las-guardas-del-banco_k0qaeh
slug: el-censo-de-las-guardas-del-banco
name: >-
  El censo real de las guardas del banco antes de traducirlo: 8 atadas, 1 a
  medias, 3 sin atar y 1 atada EN FALSO por una negación — y el modo `comparar`
  falla en la dirección que no se espera
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:11:59.443Z'
updated_at: '2026-09-03T16:11:59.443Z'
last_verified: '2026-09-03T16:11:59.443Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/measure.rs
  - crates/app/src/lib.rs
symbols:
  - CONSOLE_NAME
  - GESTURE_VARIABLE
content_hashes:
  scripts/measure-frame.sh: 'sha256:ff632681ce0123f34a58a161ee3726107de47669051545e12a59b4968f47bd42'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
  crates/app/src/lib.rs: 'sha256:a97016b4445d86d4aa0f43246bdc3ef5a6690e0adb6e8b801db6ee9be35fcb5a'
tags:
  - d27
  - f2.12
  - guardas
  - banco-de-medida
  - mutacion
feature_id: feat_migracion-a-ingles_xnx8mc
---

## What it does

**Cuántas guardas del banco estaban de verdad atadas por una prueba, contado literal a literal en agosto de 2026, antes de traducirlo.** El acoplamiento entre el guion y el informe que escribe la aplicación vivía **solo en un comentario**, y la impresión de «casi ninguna está atada» era falsa: salía casi al revés.

**28 llamadas a invalidar colgaban de 12 literales del informe** — y el literal es la unidad que un renombrado rompe, no la guarda:

| Estado | Cuántos | Cuáles |
|---|---|---|
| Atados por una prueba que afirma el literal | **8** | ventana · configuración · división · arrastres · recolocar · restauradas · barra · el tamaño |
| Atado **a medias** | 1 | `pestañas:` — la prueba usa `contains` donde el `sed` **ancla con `$`**: un sufijo la deja verde y el `sed` vacío |
| **Sin atar** | 3 | escribir la disposición … intentos · fichas marcadas a la vez como mucho · caracteres buscados |
| **Atado en falso** | 1 | glifos rasterizados |

> **El «atado en falso» es el que enseña algo.** Dos aserciones afirmaban **una negación**: `!report.contains("0 glifos rasterizados · 0 sin fuente")`. Renombrar el literal del productor las deja **verdes por vacío** — la negación de algo que ya no existe siempre se cumple. Es el disfraz de «afirmar sobre el campo en vez de sobre el efecto» con otra cara, y estaba vivo en el árbol con la suite en verde.

### Dos trampas más del mismo acoplamiento

1. **Los nombres de consola entran en la comparación.** `CONSOLE_NAME` acaba dentro de la línea de la división, y el `sed` que extrae la geometría **conserva el resto de la línea** con su `(izquierda)`. Traducirlos en un commit y el `sed` en otro hace que **dos corridas del mismo día salgan «no comparables»**.
2. **Hay dos vocabularios para el gesto y las guardas usan el del entorno.** El guion pasa uno por variable de entorno; el informe publica otro por pantalla. **Siete guardas comparan el del entorno**: traducir el del informe no rompe ninguna, traducir el del entorno rompe siete.

### Y el modo `comparar` falla en la dirección que no se espera

La función que extrae la geometría saca tres literales con `sed`, y antes hay un `grep -q` de la cabecera. Traducir la cabecera **entera** hace que aborte con un mensaje que culpa al informe («es de antes de que la cabecera lo publicara»): ruidoso pero seguro. Traducirla **a medias** deja la extracción devolviendo el mismo vacío para los dos informes: **luz verde para promediar dos corridas cualesquiera**.

> **Y «a medias» es exactamente lo que produce sola una migración fichero a fichero.** Por eso la entrega #2 del hito ató la red **antes** de traducir nada.

### Cómo se cerró

Verificado en septiembre de 2026: existe `measure.rs:2722 every_pattern_the_script_looks_for_in_the_report_still_extracts_something`, que lee el guion con `include_str!`, **re-extrae cada patrón `grep`/`sed` y lo re-aplica** a informes de referencia construidos llamando al `header()` de verdad — no a cadenas escritas a mano — y además falla si alguna línea del guion que lee el informe no aportó patrón. La negación de los glifos ya no está en `lib.rs`. **El censo es histórico; la lección —una prueba ata un acoplamiento solo si DERIVA el patrón, y una negación no ata nada— no lo es.**

## Where it lives

`scripts/measure-frame.sh` — las llamadas a invalidar y los patrones `grep`/`sed` que leen el informe; el modo `comparar` y su extracción de la geometría; y el vocabulario de gestos que exporta por entorno.

`crates/app/src/measure.rs` — el productor del informe, `CONSOLE_NAME`, y en `:2722` la guarda que **deriva** los patrones en vez de copiarlos.

`docs/02-decisiones.md` — **AC8** del hito F2.12, que es este hallazgo convertido en criterio: ninguna guarda del banco queda muerta, y los cuatro que no cubría nada se prueban rompiéndolos a propósito.
