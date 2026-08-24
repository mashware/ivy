---
id: asp_attention-lo-declara-el-productor_89mxc6
slug: attention-lo-declara-el-productor
name: >-
  Para saber si una división te reclama hace falta un campo declarado: deducirlo
  de un `mark: block` del cuerpo rompe D6 por dentro
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T08:26:45.811Z'
updated_at: '2026-08-24T08:26:45.811Z'
last_verified: '2026-08-24T08:26:45.811Z'
file_paths:
  - docs/02-decisiones.md
  - docs/04-contrato-panel.md
  - docs/05-roadmap.md
symbols: []
content_hashes:
  docs/02-decisiones.md: 'sha256:97753287aa018f84f87473e2872f7347a4aacb199a69cc2007289f3c34ee1475'
  docs/04-contrato-panel.md: 'sha256:95629fb310672544479a213c3275547f03e137cc6cdad1da3920c8076c83eba9'
  docs/05-roadmap.md: 'sha256:d6b3727577809ebf4b11c0171d1beba0910ec2b86610b88e3ae5a294cb66b583'
tags:
  - contrato
  - attention
  - d18
  - d6
  - navegacion
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El hueco que nadie había visto en tres documentos a la vez.** El contrato sabe expresar «esperando» y «bloqueado» desde el principio — `mark: wait`, `mark: block` — pero eso son **líneas del cuerpo**, y el cuerpo es libre por diseño. Para pintar el indicador de una pestaña o disparar un aviso a partir de ahí, la app tendría que **leer el cuerpo y decidir qué significa**: que una línea con `block` en medio de un resumen quiere decir que la división entera reclama.

> **Eso es D6 rota por dentro, que es la peor forma de romperla: sin que ningún documento diga que se rompió.** Y sería frágil de la forma más tonta — una línea histórica («esperé aprobación el martes», con `mark: wait`) marcaría la pestaña para siempre.

**La decisión (D18):** campo opcional `attention` con tres valores, escrito por el productor. `wait` = **te espera a ti** (pregunta, permiso, decisión); `block` = atascada por algo que no eres tú; `done` = terminó y nadie lo ha mirado. Ausente = no reclama nada. **Se acumula división → pestaña**, con `wait` ganando a `block` y `block` a `done`: `wait` va primero porque es lo único que se desatasca contigo delante ahora mismo.

**Por qué no contradice la regla del cuerpo libre.** No nombra una métrica ni vocabulario de dominio: nombra **la única pregunta que la app necesita para navegar** — ¿tengo que ir ahí? Sigue sin haber campos de coste, contexto ni progreso, y `phase` sigue siendo texto libre.

**Y callar es callar.** Sin el campo, la división no se marca y la app **no** lo deduce del cuerpo, ni de `phase`, ni de si la consola escribe. No hay valor por omisión deducido.

**Lo descartado, y el primero es el importante:** un enum de estado de trabajo tipo `idle`/`working`/`blocked` —lo que tienen Herdr, Xirp, Superset, Warp y Orca— describe **qué hace el programa**, y eso o lo deduce la app o lo declara un harness que no conoce la pregunta (D6, «el nivel 2 reinterpretado falla igual que adivinar»). `attention` no pregunta qué hace: pregunta si te reclama. También descartados: una urgencia numérica (obligaría a cada productor a calibrar contra unos paneles que no ve) y que el aviso del sistema lo dispare la app al aparecer un `wait` — se deja para cuando el campo tenga productores, porque **una interrupción falsa cuesta mucho más que un indicador falso**.

**Y el dato de competencia que lo hizo urgente:** la afirmación de que «ninguna herramienta ordena las tareas por lo que te reclama», escrita como diferenciador de la Fase 4, **dejó de ser cierta en agosto de 2026**. Herdr acumula agente → panel → pestaña → workspace y ordena por quién necesita atención. Lo que sigue sin tener nadie no es el **orden**: es el **cuerpo**.

## Where it lives

`docs/02-decisiones.md` → **D18**. `docs/04-contrato-panel.md` → Parte I, campo `attention` y su tabla. `docs/05-roadmap.md` → hito **F2.8**, adelantado por delante de F2.6 y F2.7, y la corrección de la frase de la Fase 4. Ver también [[el-transporte-no-es-el-contrato]].
