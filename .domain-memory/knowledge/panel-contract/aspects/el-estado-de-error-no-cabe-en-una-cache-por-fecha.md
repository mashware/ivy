---
id: asp_el-estado-de-error-no-cabe-en-una-cache-por-fecha_ntvqc4
slug: el-estado-de-error-no-cabe-en-una-cache-por-fecha
name: >-
  Un fallo pasajero dejaba el panel acusado de por vida: el estado de error no
  cabe en una caché que se salta el trabajo comparando fechas
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:23:16.227Z'
updated_at: '2026-08-13T13:23:16.227Z'
last_verified: '2026-08-13T13:23:16.227Z'
file_paths:
  - crates/app/src/panel.rs
symbols:
  - 'Estado::refrescar'
  - 'Estado::aviso'
  - problema
content_hashes:
  crates/app/src/panel.rs: 'sha256:568e67aba57fd9f60e9ca802161c515776057b5d550f6074bf1af4cf561d98a8'
tags:
  - caché
  - errores
  - EMFILE
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El fallo.** Un fallo de lectura **pasajero** —un `EMFILE`, y un anfitrión de terminales es justo quien se queda sin descriptores— dejaba el panel marcado como no fiable **para siempre**. La razón es la propia optimización: el problema solo se limpiaba con una relectura buena, y **un fichero que nadie vuelve a tocar no cambia de fecha**, así que no hay relectura nunca más. El usuario veía «lo que se ve no es lo último» sobre un panel que en realidad estaba perfectamente al día.

> **La condición que limpia un estado de error es «volvimos a intentarlo y salió bien», y la optimización de una caché por fecha es precisamente no volver a intentarlo.** Las dos cosas no pueden vivir en el mismo sitio. Cada vez que se guarda un fallo junto a un dato cacheado, hay que decir explícitamente qué lo borra.

**El arreglo.** El problema se limpia también cuando la relectura devuelve «no hay nada nuevo que leer»: eso ya es prueba de que el fallo anterior terminó — se pudo mirar la fecha, que es lo que el fallo impedía.

**Y el criterio que se derrumbó con él.** Había **dos** criterios paralelos para «hay algo que enseñar»: el reparto del ancho preguntaba si había panel bueno, y el pintado preguntaba otra cosa. Con un problema y sin panel bueno, uno decía que sí y el otro que no. Unificado en `se_ve()`, que es la única pregunta: ¿hay panel bueno **o** hay algo que confesar?

## Where it lives

`Estado::refrescar` en `crates/app/src/panel.rs`: relee el `panel.json` solo si su fecha cambió, y guarda aparte el último panel bueno y el último problema.
