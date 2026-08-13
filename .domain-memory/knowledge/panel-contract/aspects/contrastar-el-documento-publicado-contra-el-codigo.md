---
id: asp_contrastar-el-documento-publicado-contra-el-codigo_ek9r8d
slug: contrastar-el-documento-publicado-contra-el-codigo
name: >-
  El revisor que más encuentra es el que contrasta el documento publicado contra
  la fuente: nueve desajustes en un PR, ninguno visible desde el diff
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:23:40.325Z'
updated_at: '2026-08-13T13:23:40.325Z'
last_verified: '2026-08-13T13:23:40.325Z'
file_paths:
  - docs/04-contrato-panel.md
  - docs/05-roadmap.md
  - CLAUDE.md
symbols: []
content_hashes:
  docs/04-contrato-panel.md: 'sha256:74a34b90ad96f4314965ff99fe2a644715d2ae032f5914bdfe890ef4d0608c4d'
  docs/05-roadmap.md: 'sha256:f4fad6e2e1bd05b99530a83799119485719ec743b268d99c9786de393e4b9225'
  CLAUDE.md: 'sha256:45a196c9ea4057500d64d1d1b866c2cb0e2cf95ce7189355f259456702b0ad39'
tags:
  - revisión
  - documentación
  - contrato
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El hallazgo, en una frase:** de tres revisores en el último PR de F1.6, dos a ciegas, **el más valioso con diferencia fue el que solo contrastaba el contrato publicado contra el código fuente**. Encontró **nueve** desajustes; el resto del panel encontró cuatro bloqueantes entre todos.

Los nueve, comprimidos: `updated_at` decía que se pinta y no se pinta · «el derroche es invisible» se contradecía con la viñeta de al lado · «las tres formas» de fallo son cuatro · dos pendientes ya hechos sin tachar · la ubicación del fichero escrita en futuro cuando ya existía · tres recortes silenciosos sin declarar (512 caracteres, la referencia, y la sangría frente al ancho) · `CLAUDE.md` afirmaba que la cola de salida no tiene techo, cuando lo tiene desde D8 · y que hay decisiones D1–D7, cuando hay nueve · el hito sin marcar en el roadmap.

> **La deriva no está dentro del diff: está entre el diff y un fichero que el diff no toca.** Un revisor que lee el diseño y el cambio no puede verla, por bueno que sea. Hace falta alguien cuyo único trabajo sea abrir el documento publicado y preguntarle a la fuente si sigue siendo verdad.

**Por qué pesa tanto aquí y no es manía de orden.** El diseño vive en `.claude/work/`, que está en `.gitignore` y no sobrevive a la sesión. El contrato está versionado y es **lo que abre quien vaya a escribir un productor** — un CI, un script, un servidor MCP, o quien porte el prototipo. Entre los dos, el que gana es el que se puede leer. Ya había pasado una vez con D6 ([[d6-vuelve-por-el-documento-no-por-el-codigo]]): una decisión descartada en el diseño volvía por el documento público.

**Convertido en práctica:** en todo PR que cambie comportamiento descrito en un documento versionado, uno de los revisores va a ciegas con ese único encargo. Cuesta un agente y encuentra lo que nadie más mira.

## Where it lives

`docs/04-contrato-panel.md` (el contrato público de `panel.json`), `docs/05-roadmap.md` y `CLAUDE.md`. Papel de revisor a ciegas usado en la revisión del último PR de F1.6.
