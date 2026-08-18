---
id: asp_una-invariante-geometrica-se-afirma-en-proporciones_3tthws
slug: una-invariante-geometrica-se-afirma-en-proporciones
name: >-
  Una invariante geométrica se afirma en proporciones y no en puntos: la versión
  en puntos falló con el código BUENO, y el margen se elige entre dos cifras
  medidas
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-18T13:59:04.860Z'
updated_at: '2026-08-18T13:59:04.860Z'
last_verified: '2026-08-18T13:59:04.860Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - absorber
  - repartir
  - GROSOR_DEL_DIVISOR
  - colocar
content_hashes:
  crates/app/src/lib.rs: 'sha256:d7688143658df12a548b8eb3bd4e0e7fc0ce9966b7a5e14a6e7d7a8e8835ba9d'
tags:
  - pruebas
  - geometria
  - invariantes
  - mutacion
feature_id: feat_term-core_nx0g1r
---

## What it does

**Un disfraz nuevo, y es el simétrico de los que este proyecto ya tenía fichados.** Los conocidos son pruebas que **pasan sin atar nada** ([[una-prueba-que-ata-al-ayudante-no-ata-el-camino]], [[una-prueba-puede-pasar-porque-el-escenario-no-cabia]]). Éste es una prueba que **falla sin haber defecto**, porque afirma algo **más fuerte de lo que el código promete**.

**El caso (F2.3, segundo PR).** Había que atar que `absorber` —la operación que sube las nietas a hermanas cuando una rama se queda con una sola hija— conserva la geometría. La formulación que el registro de la sesión anterior había dejado escrita era «los rectángulos de las que quedan **no se mueven ni un punto**». Se escribió así y **cayó con el código bueno**: una consola pasaba de 599 a 598 puntos.

> **Se mueven uno, y es inevitable.** Subir las hijas mete un divisor más en la rama de fuera, y los divisores se descuentan del área **antes** de repartir. El divisor tiene que caber en algún sitio. Lo que la invariante promete es que **el reparto entre ellas no cambia**, no que nada se mueva — ver [[quitar-una-hoja-nunca-le-quita-area-a-las-que-quedan]].

**La forma correcta: afirmar la RAZÓN entre los tamaños.** Una consola el doble que otra sigue siendo el doble antes y después, y eso es cierto exactamente, sin depender del grosor del divisor ni del área elegida.

> **Y el margen se elige entre dos cifras medidas, no al tanteo.** El punto del divisor desvía la razón **0,007**; el defecto que la prueba persigue —el escalado sustituido por una copia del peso— la desvía **1,0**. Con un margen de centésimas la prueba pasaba, pero **a 0,007 de fallar sola** en cuanto alguien cambiara el área del escenario. Se puso en `0,05`: dos órdenes de magnitud por encima del ruido conocido y veinte veces por debajo de la señal. Un margen justificado con las dos cifras a los lados es lo contrario del disfraz que este proyecto tiene fichado —elegir un número con el que el defecto y el arreglo dan lo mismo—, y se documenta **en el comentario de la prueba** para que el siguiente no lo lea como arbitrario.

## Where it lives

`crates/app/src/lib.rs`, prueba `absorber_conserva_la_geometria_de_las_consolas_que_no_se_mueven`, y la doc de `Nodo::absorber` que la cita. El escenario reutiliza el ayudante `un_grupo_que_obliga_a_absorber` que la prueba de la invariante de forma ya tenía, y saca la consola por un lado del **mismo sentido** que la rama de fuera para que la inserción solo reparta el otro eje y los anchos comparados no se enteren.
