---
id: asp_medir-algo-que-cambia-la-rejilla-no-es-una-resta_6sfcd7
slug: medir-algo-que-cambia-la-rejilla-no-es-una-resta
name: >-
  Medir una pieza que se queda con parte del sitio no es una resta: el fotograma
  con paneles sale más barato y eso no es una mejora
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T15:24:41.224Z'
updated_at: '2026-08-13T15:24:41.224Z'
last_verified: '2026-08-13T15:24:41.224Z'
file_paths:
  - docs/02-decisiones.md
  - scripts/medir-fotograma.sh
  - crates/app/src/medida.rs
symbols:
  - 'Medida::publicar_tramos'
  - cabecera
content_hashes:
  docs/02-decisiones.md: 'sha256:315a02dd674468d2ff9049b2b9750cd655d842d9e9b347b4cb6de82ece55c25f'
  scripts/medir-fotograma.sh: 'sha256:3da4da4657cf7b2b100640bc4feb8e2606a9b52079895401e847cdab18618b78'
  crates/app/src/medida.rs: 'sha256:2c620cfa14e9ac593b3293628869a8ddb5d48b5ba3de54e08eb72f775d3ebe17'
tags:
  - medida
  - rendimiento
  - D3
  - metodología
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El número que sale es una trampa, y la trampa es la buena noticia de haberla visto.** Con los dos paneles pintando el fotograma cuesta **5,78 ms**; sin ellos, **7,01**. Más barato con paneles. Y es falso leerlo como una mejora: **el panel se lleva 36 de las 57 columnas de su consola**, que pasa a pintar un **63 % menos de celdas**. Lo que baja es `encolar` —1,80 → 0,75 ms— porque hay menos que apilar.

> **Cuando la pieza que mides se queda con parte del sitio de lo demás, la comparación antes/después no es una resta.** El total puede bajar mientras el coste sube. Lo único restable es el **tramo propio de esa pieza**, y solo si su suelo no cambia — aquí sí: la tanda sin panel aún mide 0,77 µs (recoger la bandera, pedir la hora, repartir el ancho), y **B − A** deja limpio lo que cuesta leer y pintar.

**Sin la tanda de control esto se publica al revés.** El anexo habría dicho «los paneles bajan el coste del fotograma un 18 %». La tanda A no es un lujo metodológico: es lo único que impide la conclusión contraria a la verdad.

**Las cifras que quedan**, dos consolas volcando 44 MiB cada una, 357-358 fotogramas con salida por consola: los dos paneles cuestan **13 µs por fotograma**, el 0,2 %; al framework de interfaz le quedan **0,50 ms demostrables de 8,33**, el 6 % del hueco; y lo que gasta el fotograma es **interpretar**, 4,5 ms.

**Dos cosas que salieron al revés de lo esperado:**

1. **Envolver no se nota.** A 21 columnas las líneas de ~50 caracteres envuelven en tres filas y a 57 no envuelven; cabía esperar que interpretar se encareciera, porque el ancho de envoltura es de lo que depende. **No cambia**: 4,35 / 4,51 / 4,35 ms. Lo domina la cantidad de bytes, no cómo se reparten en filas.
2. **Un panel que se publica sin parar no añade trabajo: añade fotogramas.** Cada consola produce en los mismos 357-358, pero la ventana pasa de 368 a 472 — menos solape, el mismo trabajo repartido entre un 28 % más de fotogramas, cada uno más barato. **Consecuencia dura: el «coste del fotograma» de dos tandas con recuentos de ventana distintos no se compara**, porque son fotogramas que llevan dentro cantidades distintas de trabajo.

## Where it lives

Anexo «Medido — 13 agosto 2026» de D3 en `docs/02-decisiones.md`, tomado con `scripts/medir-fotograma.sh` (tandas A sin panel, B panel quieto, C publicándose, D control del temporal).
