---
id: asp_la-cache-de-la-maqueta-se-invalida-por-celdas-no-por-puntos_0eqcww
slug: la-cache-de-la-maqueta-se-invalida-por-celdas-no-por-puntos
name: >-
  La caché de la maqueta del panel se invalida por CELDAS ENTERAS y no por
  puntos: un arrastre de 300 puntos cuesta 36 maquetados, no 300
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-19T15:31:13.202Z'
updated_at: '2026-08-19T15:31:13.202Z'
last_verified: '2026-08-19T15:31:13.202Z'
file_paths:
  - crates/app/src/panel.rs
symbols:
  - pintar
  - maquetar_si_hace_falta
  - Maqueta
content_hashes:
  crates/app/src/panel.rs: 'sha256:33a221357d004d8d72e2406f45898b7bcf0384b4637af8a486cb30a2eeda0668'
tags:
  - panel
  - rendimiento
  - cache
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**La clave de la caché de la maqueta del panel está en celdas enteras, no en puntos, y eso cambia el tamaño del riesgo de cualquier gesto que mueva su ancho.**

`panel::pintar` calcula el ancho con el que se maqueta así: `((rect.width() / metrica.ancho()) as usize).saturating_sub(2 * MARGEN as usize)`, y ése es el `usize` que `maquetar_si_hace_falta` compara con la maqueta guardada.

> **La intuición natural es falsa, y es la que hay que corregir**: parece que un arrastre continuo —que cambia el ancho en puntos en cada fotograma— invalidaría la caché en **todos** los fotogramas. No lo hace: solo la invalida **al cruzar una frontera de celda**. Un arrastre de 300 puntos con una celda de ~8,4 cuesta unos **36 maquetados, no 300**.

**Por qué importa más de lo que parece.** El peor caso de maquetar está medido y es brutal —una línea de 260.000 caracteres cuesta **65,8 ms**, contra un presupuesto de 16,7 ms por fotograma; ver [[el-peor-caso-de-pintar-es-el-inverso-del-de-leer]]—, así que «la caché se invalida en cada fotograma del gesto» y «se invalida cada ocho puntos» son dos conclusiones muy distintas sobre si un gesto que estrecha el panel es viable. Con la segunda, lo es sin acotar nada.

**Y hay una consecuencia para el ALCANCE, no solo para el coste.** El escenario de «dos anchos en puntos distintos dentro de la misma celda» **no lo produce el gesto**: el ancho se guarda en celdas enteras y el reparto las multiplica, así que el rectángulo del panel es siempre múltiplo exacto de la celda. Sí lo produce **redimensionar la ventana** en una división donde el reparto recorta el panel, porque ahí su ancho deja de ser múltiplo — un camino que existía mucho antes de que la frontera se pudiera arrastrar.

> **La consecuencia para las pruebas es la que muerde.** Un criterio del tipo «el gesto no rehace la maqueta más de una vez por fotograma» **se cumple por construcción** —el panel se pinta una vez por pasada, así que hay un solo ancho y una sola consulta a la caché— y su prueba pasaría sin atar nada. Lo que sí se puede plantar es la propiedad de verdad: **dos anchos en puntos distintos dentro de la misma celda no rehacen la maqueta**.

## Where it lives

`panel::pintar` calcula la clave y `Estado::maquetar_si_hace_falta` la compara, las dos en `crates/app/src/panel.rs`. El ancho que entra ahí lo produce `panel::apartar` a partir del ancho en celdas que guarda cada división.
