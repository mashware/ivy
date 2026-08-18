---
id: asp_un-agente-que-muere-puede-dejar-la-mutacion-puesta_q1bpsn
slug: un-agente-que-muere-puede-dejar-la-mutacion-puesta
name: >-
  Un agente de pruebas que muere a medias puede dejar la mutación APLICADA en el
  árbol de trabajo — y seguir escribiendo sin aparecer en la lista de agentes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-18T13:59:31.515Z'
updated_at: '2026-08-18T13:59:31.515Z'
last_verified: '2026-08-18T13:59:31.515Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - absorber
content_hashes:
  crates/app/src/lib.rs: 'sha256:d7688143658df12a548b8eb3bd4e0e7fc0ce9966b7a5e14a6e7d7a8e8835ba9d'
tags:
  - metodo
  - pruebas
  - mutacion
  - agentes
feature_id: feat_term-core_nx0g1r
---

## What it does

**La regla dura del proyecto es que las pruebas se mutan**: se rompe a propósito lo que la prueba dice cubrir y se comprueba que cae. Cuando eso lo hace un subagente, el defecto vive **en el árbol de trabajo** mientras dura la comprobación, y ahí está el riesgo que no es evidente.

**Lo que pasó (F2.3, segundo PR).** Un agente de pruebas murió sin entregar informe y dejó **tres** mutaciones aplicadas. Siguió parcheando **después** de morir y **sin aparecer ya en la lista de agentes**; hubo que pararlo con la vía dura. Se vio comprobando `git status` antes de cerrar la sesión.

> **Y una de las tres SOBREVIVÍA a la suite entera.** El escalado de pesos de `absorber` sustituido por una copia del peso de dentro pasaba las **383** pruebas del crate. Sin ese `git status` se habría entregado un PR con el defecto **sembrado** y en verde — no un hueco de cobertura, un defecto real dentro del diff.

**Las dos lecciones, y la segunda es la que sorprende:**

1. **Comprobar `git status` antes de dar por buena una sesión no es higiene, es la última red.** Un agente que muta ficheros y muere a medias deja el código peor de lo que lo encontró, y la suite en verde no lo delata — justamente porque la mutación que sobrevive es la que ninguna prueba ata.
2. **La mutación abandonada es un DETECTOR de huecos de cobertura, y de los buenos.** Aquí regaló el hueco más valioso del hito: que nada ataba los pesos de `absorber` — ver [[quitar-una-hoja-nunca-le-quita-area-a-las-que-quedan]] y [[una-invariante-geometrica-se-afirma-en-proporciones]]. Lo que un agente muerto deja puesto merece leerse antes de revertirlo, no después.

**Consecuencia práctica para este proyecto:** una prueba escrita por un agente se **audita y se muta** antes de aceptarla, y la sesión no se cierra sin `git status` limpio ni sin comparar contra una copia de control del fichero. Ya se hizo en la validación del primer PR de este mismo hito, donde el agente escribió tres pruebas buenas pero tampoco entregó informe.

## Where it lives

Ocurrió sobre `crates/app/src/lib.rs` en el segundo PR de F2.3 (issue 76), en la fase de validación. Registrado en `07-validation.md` de ese trabajo con la tabla de las tres mutaciones abandonadas y cuál de ellas sobrevivía.
