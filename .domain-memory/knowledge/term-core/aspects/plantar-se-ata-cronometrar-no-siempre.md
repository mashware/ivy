---
id: asp_plantar-se-ata-cronometrar-no-siempre_38pyam
slug: plantar-se-ata-cronometrar-no-siempre
name: >-
  Si el trabajo se puede PLANTAR se ata con una prueba; si solo se puede
  cronometrar y cuesta lo mismo que el ruido, el sostén es estructural
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T16:09:00.137Z'
updated_at: '2026-08-17T16:09:00.137Z'
last_verified: '2026-08-17T16:09:00.137Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - rearmar_el_vigilante
  - crear_una_pestaña
  - cerrar_la_pestaña_activa
  - cronometrado_aparte
  - llevarse_lo_de_este_fotograma
content_hashes:
  crates/app/src/lib.rs: 'sha256:5303a764d73851d1199719a27ca85c6efac8142650b896676e8be63ee15d187e'
tags:
  - medida
  - pruebas
  - mutacion
  - F2.2
feature_id: feat_term-core_nx0g1r
---

## What it does

**La regla del proyecto es que todo trabajo no declarado se publica como coste del framework** —el fotograma se reparte por resta— y el arreglo típico es mover la raya del cronómetro para que encierre el trabajo. Lo que este caso enseña es **cuándo ese arreglo se puede probar y cuándo no**, y conviene saberlo antes de escribir la prueba.

**Para probarlo hay que comparar lo declarado con un reloj de pared tomado por FUERA de la operación**, y entre los dos relojes cabe una expropiación del planificador. Medido el 17 de agosto de 2026 con `rearmar_el_vigilante` en abrir y cerrar pestaña: en la suite un rearme cuesta **microsegundos** —las consolas de prueba no tienen directorio que vigilar, y dándoselo sigue siendo del mismo orden—, o sea **lo mismo que el hueco entre los dos relojes**. Señal y ruido del mismo orden no se distinguen.

**Tres formulaciones, las tres descartadas:**

| Formulación | Qué pasó |
|---|---|
| Margen de medio rearme, un intento | **Frágil**: con la máquina cargada el reloj de pared dio 61 ms donde solía dar 9, y la prueba fallaba sin que nada del código estuviera mal |
| Calibrar el rearme con el mínimo de cinco medidas | Las **dos mutaciones** que sacan el rearme del cronómetro seguían en verde |
| Aceptar que cuadre uno de ocho intentos | Igual: las dos mutaciones vivas |

**Se quitó la prueba.** Una prueba que no cae al romper a propósito lo que dice cubrir no cubre nada, y dejarla es peor que no tenerla: da falsa seguridad. Es la segunda vez en el mismo hito —ver [[la-vuelta-de-medida-se-cierra-al-pintar]]— que una prueba que **parecía más fuerte era más débil**.

> **La regla práctica: si el trabajo se puede PLANTAR, se ata con una prueba; si solo se puede CRONOMETRAR y cuesta lo mismo que el ruido, el sostén es estructural y se declara sin prueba.**

**Lo que sí se ata, y por qué funciona:** `cerrar_una_pestaña_no_se_lleva_lo_que_sus_consolas_habían_hecho` **planta** tres duraciones grandes y distintas —cinco, tres y dos segundos— en los tres sitios donde una consola guarda trabajo sin declarar, y afirma que lo declarado supera la suma. Su mutación cae. Los números son grandes **y distintos** a propósito: con el mismo número en los tres, tirar uno seguiría dando de sobra; con cifras del orden de lo que una consola interpreta de verdad, el propio cierre ya llenaría el total.

**Y el sostén estructural, cuando toca:** la raya del cronómetro encierra la llamada, y eso es **legible** —en los cuatro sitios que rearman el vigilante se comprobó uno por uno que el `+=` va después y que ninguno cuenta dos veces—. Se descartó hacerlo cumplir por el tipo (que `rearmar_el_vigilante` devolviera su coste con `#[must_use]`, para que olvidarse fuera un error de compilación) porque los dos sitios que hoy lo encierran tendrían que partirse para no contarlo dos veces — y **contar dos veces abarata el coste del framework**, que es la dirección peligrosa del error.

**Un corolario que salió de la misma fase:** una prueba que saca su número **de la propia constante que prueba** es un espejo, no una prueba. Una prueba del margen de tamaño desviaba la ventana `MARGEN_DEL_TAMAÑO` puntos, así que recortar el margen a cero le recortaba también la desviación —pedía el tamaño exacto— y **seguía en verde con el defecto puesto**. Con el medio punto escrito a mano, cae. Los números de una prueba tienen que ser independientes de lo que la prueba afirma.

## Where it lives

`crates/app/src/lib.rs` — `cronometrado_aparte` y los cuatro sitios que llaman a `rearmar_el_vigilante` (`poner_delante`, `injertar_la_nueva`, `cerrar_la_pestaña_activa`, `cerrar_la_del_foco`); `Division::llevarse_lo_de_este_fotograma`, que es la mitad que sí se planta. Aprendido en el PR #75 de F2.2, con las mutaciones 2 y 3 declaradas vivas en `06-review.md`.
