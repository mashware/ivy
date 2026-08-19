---
id: asp_una-prueba-de-arrastre-necesita-dos-pasadas-como-minimo_p8jmdm
slug: una-prueba-de-arrastre-necesita-dos-pasadas-como-minimo
name: >-
  Una prueba de arrastre en egui necesita al menos DOS pasadas: con una sola
  pasa por construcción, y pasa igual con el mecanismo puesto que quitado
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-19T18:25:00.505Z'
updated_at: '2026-08-19T18:25:00.505Z'
last_verified: '2026-08-19T18:25:00.505Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - arrastrar_la_frontera_de_su_panel
content_hashes:
  crates/app/src/lib.rs: 'sha256:33eb0620948d43e72b88537f8c3fcb262d3202ccd13ec8e13f08279329d6867b'
tags:
  - egui
  - pruebas
  - arrastre
  - disfraz
feature_id: feat_term-core_nx0g1r
---

## What it does

**egui resuelve la interacción al principio del fotograma, con los controles registrados en el fotograma ANTERIOR.** Así que un control **recién registrado** nunca está `dragged` en su primera pasada: no importa que en esa misma pasada le metas el evento de pulsar y el de mover, porque cuando egui decidió quién se arrastra ese control **todavía no existía**.

**El caso, y son DOS veces en el mismo PR.** Se escribieron dos pruebas para atar guardas que deciden **si el control se registra o cómo se calcula su valor**. Las dos llamaban al método una vez, dentro de un `run_ui` con los tres eventos juntos. Las dos verdes. Y las dos **verdes también con la guarda quitada**: el control nuevo no podía estar arrastrado ni con guarda ni sin ella, así que el método salía por la puerta del «no me están arrastrando» y **nunca llegaba al código que se quería probar**.

> **Es una variante del tercer disfraz del proyecto —la prueba que pasa porque el escenario no puede ocurrir— con la vuelta de tuerca que la hace peor de detectar: aquí el escenario no lo impide el código propio ni la ventana elegida, lo impide una regla de la LIBRERÍA.** Ver [[una-prueba-puede-pasar-porque-el-escenario-no-cabia]], donde lo que fallaba era el mundo de la prueba; aquí falla el número de fotogramas.

**Y no se ve leyendo la prueba.** Se lee perfecta: monta el escenario, inyecta el gesto, afirma lo correcto. **Se ve mutándola**, y las dos veces fue la mutación la que lo dijo.

**La forma que sí funciona** es la que ya usaban las demás pruebas del gesto sin que nadie hubiera escrito por qué: **una pasada de interfaz por evento** —mover el puntero al sitio, pulsar, mover, soltar— cada una un fotograma completo. Con eso el control ya existe cuando egui decide, y la mutación cae.

**Regla práctica:** si lo que se ata vive **detrás** de un `dragged`/`dragged_by`, la prueba tiene que llegar a un fotograma **posterior** al registro del control. Un solo `run_ui` no vale, y su verde no significa nada.

## Where it lives

Las pruebas del gesto de la frontera del panel, en el módulo de pruebas de `crates/app/src/lib.rs`: las que hacen cuatro pasadas con `un_fotograma_en` funcionan; las dos que se escribieron con un solo `run_ui` hubo que rehacerlas. La regla de egui está en `interaction.rs` de la 0.36.1, que trabaja sobre los controles de la pasada previa.
