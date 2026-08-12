---
id: asp_d6-vuelve-por-el-documento-no-por-el-codigo_5wvfyb
slug: d6-vuelve-por-el-documento-no-por-el-codigo
name: >-
  Una decisión descartada en el diseño vuelve por el documento público: D6
  seguía pedida en el contrato meses después de soltarla
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:07:41.498Z'
updated_at: '2026-08-12T14:07:41.498Z'
last_verified: '2026-08-12T14:07:41.498Z'
file_paths:
  - docs/04-contrato-panel.md
  - docs/02-decisiones.md
symbols: []
content_hashes:
  docs/04-contrato-panel.md: 'sha256:5af2cb170fb82f7c94d1f1a491cd2349f599e2bde0740449655e5fcc44a2e75f'
  docs/02-decisiones.md: 'sha256:f46d49ee51def350ecc462a6930c4d7cc8b7941ecc69b6b15f8b2c791dc05955'
tags:
  - D6
  - contrato
  - documentacion
  - prototipo
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Lo que pasó.** El prototipo callaba el aviso de rancio cuando el trabajo estaba cerrado —un resumen viejo de algo terminado es correcto, y la idea es razonable—. Pero para saber que está cerrado hay que **leer la fase de un `meta.json` ajeno y decidir qué significa**, que es exactamente lo que D6 prohibe: la app no deduce el estado. El diseño de F1.6 lo declaró fuera, con su razón, y hasta avisó de que «quien porte el prototipo lo reintroducirá sin darse cuenta».

**Y lo reintrodujo el propio documento.** `docs/04-contrato-panel.md` seguía diciendo, en su lista de reglas de comportamiento, «la app lo señala — salvo que el trabajo esté cerrado, donde un resumen viejo es correcto». Lo cazó un revisor ciego contrastando código contra contrato publicado; nadie lo había visto en el diseño ni en la revisión, porque los dos miraban el diseño.

> **El aviso escrito en el diseño no protege de nada si la petición sigue viva en el documento que la gente lee.** El diseño vive en `.claude/work/`, que está en `.gitignore` y no sobrevive a la sesión. El contrato está versionado y es lo que abre quien vaya a escribir un productor —un CI, un script, un servidor MCP— o a portar el prototipo. Entre los dos, el que gana es el que se puede leer.

**No era el único desajuste, y esa es la parte que generaliza.** El mismo contraste code-contra-contrato sacó cinco: la tabla de campos se contradecía con una regla nueva sobre de dónde sale el rancio; «solo hay dos formas de que un fichero no valga» frente a cinco variantes reales de fallo; un techo aplicado en el código (la sangría, 64) que el contrato no mencionaba, así que un productor que pida `indent: 100` obtiene 64 sin enterarse; y `link` prometido como enlace clicable cuando el código lo lee y lo descarta.

> **Cuando un documento público es entregable de un PR, revisarlo contra el diff es un ángulo propio.** Los revisores que miran código no lo abren, y los que miran diseño tampoco.

**Cómo quedó la regla**: el aviso de rancio no tiene excepciones. Quien no lo quiera lo dice **en su propio fichero**, con un `stale_after_minutes` alto en su última publicación — que es la forma que D6 admite, porque la semántica la sigue poniendo quien trabaja.

## Where it lives

`docs/04-contrato-panel.md`, sección «Reglas de comportamiento». La decisión original, en `docs/02-decisiones.md` → D6.
