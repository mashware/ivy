---
id: asp_una-prueba-de-la-ventana-necesita-que-el-panel-se-publique_8gg9pt
slug: una-prueba-de-la-ventana-necesita-que-el-panel-se-publique
name: >-
  La franja del panel no existe para una prueba hasta que alguien publica un
  panel de verdad: fijar la rejilla a mano inventa una franja que el reparto no
  puso ahí
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T09:23:22.822Z'
updated_at: '2026-08-25T09:23:22.822Z'
last_verified: '2026-08-25T09:23:22.822Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/panel.rs
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:6ffd89344e0593f6d7362dccb0c76cc2ac211086a11fe9f50adb258313184450'
  crates/app/src/panel.rs: 'sha256:d9844bb19996c434945fd41689580cbaa88ac00d1919dea2597ba2c5346c7319'
tags:
  - pruebas
  - panel
  - f2.6
  - arnes
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Un panel que nadie ha publicado NO ocupa columnas**: el reparto solo le aparta ancho cuando su estado dice que se ve, y sin lectura buena ni aviso que dar, la consola se queda con la división entera. Es correcto en producción —una consola sin productor no debe perder sitio— y es una trampa en las pruebas.

**La trampa, con sus dos caras.** Una prueba que quiera ejercitar «apretar sobre las columnas del panel» tiene dos formas de montarse y las dos fallan si se hacen a la ligera:

1. **Dejar que la consola tome el ancho de la división entera** —lo que sale de medir el rectángulo sin más—: entonces **la franja del panel no existe** y todo apretón cae «dentro de la rejilla». El escenario no ocurre y la prueba pasa **sin ejercitarlo**: comprobado mutándola, la guarda se podía quitar entera con la suite verde.
2. **Fijar la rejilla a mano más estrecha** —«la mitad de las columnas»—: inventa una franja que el reparto real no puso ahí. La prueba ejercita **algo**, pero no lo que la ventana hace.

**Lo que sí monta el escenario: publicar un `panel.json` de verdad** en un directorio propio, refrescar el estado por la puerta normal, comprobar que el panel **se ve**, y **derivar** la rejilla del mismo reparto que usa la ventana. Tres líneas más y el escenario es el real.

> Esto estaba **declarado como hueco sin cubrir** desde la entrega de la selección —«la prueba fija esa rejilla a mano en vez de derivarla del reparto»— y mordió una entrega después, cuando el mismo defecto apareció en el camino del ratón y la primera prueba que se escribió para cazarlo **no lo cazaba**.

**Dos detalles del arnés que van con esto, porque cuestan lo mismo de descubrir:**

- **El directorio del panel de prueba lleva el nombre de la prueba**, no solo el del proceso. La suite corre en paralelo dentro del mismo proceso: sin ese discriminante, el borrado de una cae entre la escritura y la lectura de otra.
- **Un fotograma de «moverse dentro de la misma celda» tiene que superar el umbral con el que egui decide que un gesto es un arrastre** —unos seis puntos— o ese fotograma no llega al camino que se quiere probar. Con un punto de movimiento, la mutación correspondiente no tumba la prueba.

## Where it lives

El reparto entre consola y panel está en `crates/app/src/panel.rs` y se invoca desde la división en `crates/app/src/lib.rs`; el ayudante de prueba que publica el panel y deriva la rejilla vive en el módulo de pruebas de ese mismo fichero. El hueco declarado que esto cierra estaba escrito en el artefacto de validación de la entrega de la selección.
