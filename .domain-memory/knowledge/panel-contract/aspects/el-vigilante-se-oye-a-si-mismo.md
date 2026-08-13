---
id: asp_el-vigilante-se-oye-a-si-mismo_9chdbq
slug: el-vigilante-se-oye-a-si-mismo
name: >-
  Un vigilante de directorio te devuelve tus propias lecturas: la aplicación se
  oía a sí misma y giraba a 60 fotogramas sin causa visible
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:21:52.516Z'
updated_at: '2026-08-13T13:21:52.516Z'
last_verified: '2026-08-13T13:21:52.516Z'
file_paths:
  - crates/app/src/vigilante.rs
  - crates/app/src/panel.rs
  - docs/04-contrato-panel.md
symbols:
  - Vigilante
  - Bandera
  - no_trae_nada_nuevo
content_hashes:
  crates/app/src/vigilante.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
  crates/app/src/panel.rs: 'sha256:568e67aba57fd9f60e9ca802161c515776057b5d550f6074bf1af4cf561d98a8'
  docs/04-contrato-panel.md: 'sha256:74a34b90ad96f4314965ff99fe2a644715d2ae032f5914bdfe890ef4d0608c4d'
tags:
  - inotify
  - notify
  - vigilante
  - rendimiento
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El fallo, y es de los que no se ven mirando la pantalla.** La máscara con la que `notify` registra un directorio incluye `OPEN`. O sea: **abrir el `panel.json` para leerlo levanta la bandera que dice «hay que leer el `panel.json`»**. Con un fichero válido el bucle se corta solo, porque la comparación de fechas evita reabrirlo; pero **con un fichero que no se entiende no hay fecha buena que guardar**, así que se reabre siempre. Medido: **123 lecturas en dos segundos frente a las 2 esperadas**, la ventana sin dormir nunca, y en pantalla no se mueve absolutamente nada.

> **Antes de armar un vigilante, la pregunta no es «qué eventos me interesan» sino «cuáles de sus eventos los produzco yo».** Un vigilante sobre el directorio del que además lees es un lazo cerrado por construcción, y su síntoma es consumo, no error.

**El arreglo, y por qué no fue el simple.** Se descartan los avisos de acceso **menos cerrar-tras-escribir** (`Access(Close(Write))`). Filtrarlos todos —la propuesta obvia— habría dejado fuera al productor que escribe **sin renombrar**: para ése, cerrar tras escribir es la única señal de que ya terminó, y sin ella el panel se leería a medio escribir.

**El eco del renombrado, medido y descartado.** Publicar como recomienda el contrato —escribir aparte y renombrar encima— costaba **1,88 fotogramas** en vez de uno: el renombre entrega el destino y **acto seguido** un segundo aviso con origen y destino juntos. Verificado en la fuente de `notify` que ese segundo nunca llega sin el primero delante, así que se descarta sin perder nada y baja a **1,00**.

**Lo que se decidió NO arreglar, con su número.** Atender los avisos de «he tocado» significaría leer a mitad de escritura, y un productor que escribe a trozos costaría **~42 fotogramas por publicación**. Quedarse solo con «he terminado» dejaría fuera al productor que mantiene el fichero abierto y reescribe en su sitio. Se queda como está y el contrato publicado gana una viñeta —«publica de una vez»— con la cifra medida, que es la forma honesta de trasladar el coste a quien puede evitarlo.

## Where it lives

`crates/app/src/vigilante.rs` (crate `app`), sobre `notify` 8.2, que en Linux es inotify. El vigilante mira el **directorio** de cada división, no su `panel.json`, y levanta una bandera por consola.
