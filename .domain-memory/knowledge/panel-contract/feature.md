---
id: feat_panel-contract_8y2gc0
slug: panel-contract
name: >-
  panel-contract — el contrato de panel.json, su lectura y el directorio donde
  vive
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:06:14.300Z'
updated_at: '2026-08-12T14:06:14.300Z'
last_verified: '2026-08-12T14:06:14.300Z'
file_paths:
  - crates/panel-contract/src/lib.rs
  - crates/panel-contract/src/panel.rs
  - docs/04-contrato-panel.md
symbols:
  - Panel
  - Line
  - Mark
  - Style
  - Problem
  - read
  - PanelDir
  - MAX_BYTES
  - MAX_INDENT
  - STALE_AFTER
  - FICHERO_DEL_PANEL
content_hashes:
  crates/panel-contract/src/lib.rs: 'sha256:eef1e94af37a47b31674f9c0a32cbb016c13a971396af680b7e6c0e74a2de17a'
  crates/panel-contract/src/panel.rs: 'sha256:2f88d49d982871a2367457566cc72b098caee9f6217d84e0ed9db26e29c6b7b7'
  docs/04-contrato-panel.md: 'sha256:5af2cb170fb82f7c94d1f1a491cd2349f599e2bde0740449655e5fcc44a2e75f'
tags:
  - contrato
  - panel
  - entrada-no-confiable
  - D6
---

## What it does

La caja que sabe **qué dice** `panel.json` y cómo leerlo sin que nada de lo que traiga tire la ventana. Y también dónde vive ese fichero: el directorio de cada división, con su identidad.

Existe separada de la capa de pintado por la misma razón económica que `term-core`: la interfaz se va a reescribir (D3), y *qué* se publica y *dónde* son parte del contrato, no del framework de turno. Nada de generar un nombre, crear un directorio y borrarlo tiene que ver con pintar.

**Su entrada es no confiable por definición.** El fichero lo escribe otro proceso —un agente, un CI, un despliegue— y puede venir de un repo clonado o de un contenedor. Eso decide tres cosas del diseño de la lectura: la tolerancia es **por campo y no por documento** (quien publica es un agente escribiendo JSON a mano, y perder el panel entero por un campo que nadie mira sería lo peor que se podría hacer); el techo de tamaño se aplica **leyendo**, no comprobando después; y se exige fichero regular **antes de abrir**, porque una tubería con nombre bloquearía la apertura y leer ocurre dentro del fotograma.

**Y no interpreta el vocabulario del productor** (D6): guarda las seis marcas y los siete estilos con sus nombres literales, y `phase` es texto libre que se pinta tal cual. Traducirlo a un color o a un icono sería que la app decida qué significa el trabajo de otro.

## Where it lives

`crates/panel-contract/`. Los tipos, la lectura y el rancio en `src/panel.rs`; el directorio de la división (`PanelDir`) y las constantes públicas en `src/lib.rs`. La especificación que esta caja implementa está en `docs/04-contrato-panel.md`, y es un documento **público del proyecto**: quien porte el prototipo lee eso, no el diseño. El banco que reejecuta los números que el código afirma, en `examples/lectura.rs`.
