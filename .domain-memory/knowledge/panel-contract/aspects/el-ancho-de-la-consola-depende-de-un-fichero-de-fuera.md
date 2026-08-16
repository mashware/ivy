---
id: asp_el-ancho-de-la-consola-depende-de-un-fichero-de-fuera_x62rkj
slug: el-ancho-de-la-consola-depende-de-un-fichero-de-fuera
name: >-
  El ancho que se le anuncia a una consola depende de un fichero de fuera: quien
  calcula rejillas sin releer el `panel.json` se equivoca en 36 columnas
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T16:50:20.249Z'
updated_at: '2026-08-16T16:50:20.249Z'
last_verified: '2026-08-16T16:50:20.249Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/panel.rs
  - crates/app/src/vigilante.rs
symbols:
  - 'Ventana::medir_las_que_no_se_ven'
  - 'Division::refrescar_su_panel'
  - 'Division::repartir_con_su_panel'
  - 'panel::apartar'
  - 'Bandera::recoger'
content_hashes:
  crates/app/src/lib.rs: 'sha256:b27adaa55359fbc81c3b0a62f6993d4aecce7bd71aeff4f8faee1484d59dbe87'
  crates/app/src/panel.rs: 'sha256:bb0c2831a73e9318e084df0e0ac888d48ef188b7bb87d678dea35bc49458c572'
  crates/app/src/vigilante.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
tags:
  - panel
  - rejilla
  - medida
  - F2.1
  - pestanas
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El estado del panel de una división solo se refrescaba al pintarla**, y de ese estado depende **cuánto ancho le queda a su consola**: `panel::apartar` le quita `ANCHO = 36` columnas cuando hay panel que enseñar (`panel.rs:32`). Las dos cosas juntas producen un fallo que no es obvio desde ninguna de las dos por separado.

**El caso concreto (F2.1, PR #64).** Al darle rejilla a las pestañas que no se pintan, la primera versión no releía su `panel.json`: para ella «no hay panel», así que medía la consola con el **rectángulo entero** — 93 columnas en vez de 57. Y no dura un fotograma: dura **todo el tiempo en segundo plano**. O sea el mismo defecto que el arreglo venía a quitar ([[la-pestana-de-fondo-vive-en-otra-ventana]]), con el signo cambiado y **más grande** que el original de 80 columnas.

**La justificación que lo dejó pasar era falsa, y conviene que quede escrita porque suena razonable:** «releer es tocar el disco una vez por división y por fotograma». No lo es. `refrescar_su_panel` sale por `Bandera::recoger`, un **`swap` atómico** (`vigilante.rs:73`), y solo llega al fichero en el fotograma en que el vigilante avisó de que **ese** `panel.json` cambió — que es exactamente cuando hace falta. El vigilante reparte por ruta exacta a todas las vigiladas, incluidas las de las pestañas de fondo.

**El peor caso sí existe y es el modo degradado:** con el vigilante ciego, la mitad lógica levanta las banderas de **todas** las divisiones cada 500 ms. Con 8 pestañas son 16 lecturas concentradas en un fotograma en vez de 2, unos 100-200 µs. Se declara entera, así que no falsea la cifra de D3.

**Resuelto en el PR #64 (`9702da5`):** `Ventana::medir_las_que_no_se_ven` releee el panel de cada división de fondo, reparte el hueco con `repartir_con_su_panel` y **cronometra lo que cuesta**, que viaja en `panel_sin_pintar` hasta la vuelta del bombeo — no como vuelta propia, por lo dicho en [[la-pasada-repetida-rebombea-y-sobrescribe-la-vuelta]].

> **La regla:** cuando el tamaño que se le anuncia a un programa depende de un fichero de fuera, **quien calcula ese tamaño tiene que leer ese fichero**, o estará midiendo contra una foto vieja. Y si lo lee dentro del fotograma, tiene que declarar lo que cuesta: en un instrumento por resta, la lectura no declarada se publica como coste del framework.

**Y una de prueba:** ninguna prueba ataba el hueco del panel — la mutación a `rect.size()` sobrevivía con 261 verdes, porque las divisiones de prueba nacen sin directorio de panel y para ellas las dos expresiones dan lo mismo. El número elegido es lo que decidía si el defecto se veía.

## Where it lives

`crates/app/src/lib.rs` — `Ventana::medir_las_que_no_se_ven`, `Division::refrescar_su_panel`, `Division::repartir_con_su_panel`. `crates/app/src/panel.rs:32` — `panel::apartar` y su `ANCHO`. `crates/app/src/vigilante.rs:73` — `Bandera::recoger`, el `swap` atómico.
