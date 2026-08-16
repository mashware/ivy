---
id: asp_la-pestana-de-fondo-vive-en-otra-ventana_329695
slug: la-pestana-de-fondo-vive-en-otra-ventana
name: >-
  Una pestaña que nunca se pinta conservaba la rejilla de arranque y el programa
  de dentro formateaba para una ventana que no existe — arreglado en F2.1
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:24:33.386Z'
updated_at: '2026-08-16T16:56:18.193Z'
last_verified: '2026-08-16T16:56:18.193Z'
file_paths:
  - crates/app/src/lib.rs
  - scripts/medir-fotograma.sh
symbols:
  - 'Ventana::medir_las_que_no_se_ven'
  - medido
  - redimensionar
  - pintar_division
  - REJILLA_INICIAL
content_hashes:
  crates/app/src/lib.rs: 'sha256:ca5c117b121ef422e01d80872e0ebf3a1c92fb2c22b23317c8778bbb2fa86650'
  scripts/medir-fotograma.sh: 'sha256:c2df1b6a447aabec1f099c5994634febb7aa0d0c0db959270f484726d7d22e46'
tags:
  - medida
  - pestañas
  - F2.1
  - verificado-en-la-app-real
  - arreglado
feature_id: feat_term-core_nx0g1r
---

## What it does

> **Estado (16 de agosto de 2026): ARREGLADO** en el PR #64 (`9702da5`). Se conserva escrito porque explica por qué existe `Ventana::medir_las_que_no_se_ven`, y porque la trampa que trajo el arreglo resultó **peor que el defecto**: ver [[el-ancho-de-la-consola-depende-de-un-fichero-de-fuera]].

**El defecto era este: el tamaño que se le anuncia al programa de dentro solo se calculaba al pintar.** `medido` se escribía en `pintar_division`, que solo corre para la pestaña activa, y `redimensionar()` lo lee en la mitad lógica del fotograma siguiente. Una pestaña de fondo **nunca se pinta**, así que conservaba `REJILLA_INICIAL` —**24×80**— mientras la visible tenía 59×57.

Verificado **contra el binario de release**, no leído: con `MEDIR_PESTANAS=3` el informe publicaba `pestaña 0: se veía → 59×57` y las dos de fondo a `24×80`.

**Dos consecuencias, y ninguna era cosmética:**

1. **De producto.** Un agente en segundo plano formateaba su salida a **80 columnas** cuando la ventana le daba 57. La promesa del hito es que la de fondo «sigue corriendo»; corría, pero **creyendo que vivía en otra ventana**, y al volver a esa pestaña lo escrito estaba ajustado a un ancho que no era el suyo hasta que el programa redibujara.
2. **De medida.** La tanda de dos pestañas con la de fondo volcando habría medido esa consola interpretando **1.920 celdas en vez de 3.363**, casi la mitad de trabajo, y el anexo habría publicado un coste **optimista sin decirlo**.

> **Y el sesgo no se podía estimar con lo que ya se sabía.** El anexo de D3 demostró que el **ancho** de envoltura no mueve el coste de interpretar (4,35 / 4,51 / 4,35 ms a 57 y a 21 columnas), pero **el alto no se ha medido nunca**. Con 24 filas en vez de 59 hay unas 2,5 veces más desplazamientos de pantalla por línea volcada: **la dirección del sesgo era desconocida**, que es peor que conocerla y grande. Esa es la razón por la que se decidió arreglarlo antes de medir, en vez de medir sobre el 24×80 declarando el sesgo.

Es la misma familia que [[medir-algo-que-cambia-la-rejilla-no-es-una-resta]] —una pieza que se queda con parte del sitio de lo demás— pero por la vía contraria: aquí no era que la pieza robase espacio, es que **a la parte medida nunca se le dijo cuánto tenía**.

**Lo bueno, mientras duró:** el informe publica la rejilla de cada consola de cada pestaña, así que quien lo leyera veía los 24×80 y no se lo tragaba.

## Cómo se arregló

`Ventana::medir_las_que_no_se_ven` recorre al final de `ui` las divisiones de las pestañas que no se pintan, reparte el rectángulo real de la ventana y escribe `medido`. El camino aguas abajo ya existía: `bombear_todas` recorre todas las divisiones de todas las pestañas y llama a `redimensionar()`, que aplica `medido` a la rejilla y al pseudoterminal.

**Confirmado de campo, no solo en pruebas:** la cabecera de la tanda G del anexo «Medido — el fotograma con pestañas» de **D13** publica la pestaña de fondo a **59×21**, donde antes del hito habría dicho 24×80. Y el criterio AC14 se probó hasta su cláusula más lejana —que el tamaño llega al programa de dentro— preguntándoselo con `stty size`.

## Where it lives

`Ventana::medir_las_que_no_se_ven` (el arreglo), `Ventana::pintar_division` y `Division::redimensionar` en `crates/app/src/lib.rs`; el síntoma se veía en la cabecera del informe de `on_exit` y afectaba a las tandas de `scripts/medir-fotograma.sh`. El anexo que lo confirma, en `docs/02-decisiones.md`, bajo **D13**.
