---
id: feat_migracion-a-ingles_xnx8mc
slug: migracion-a-ingles
name: >-
  F2.12 — el repositorio pasa a inglés (D27): sus guardas, y lo que ninguna de
  ellas ve
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:06:33.222Z'
updated_at: '2026-09-03T16:06:33.222Z'
last_verified: '2026-09-03T16:06:33.222Z'
file_paths:
  - docs/02-decisiones.md
  - crates/app/tests/glossary.rs
  - crates/app/tests/prose.rs
  - docs/05-roadmap.md
symbols:
  - FINISHED_CRATES
  - GLOSSARY_CLEAN_FILES
  - NOT_OURS
  - SPANISH_FUNCTION_WORDS
content_hashes:
  docs/02-decisiones.md: 'sha256:4eb23bb4f11a5f60d8d236644be773469b8f5a617d1657294e0b9d39d4c9872a'
  crates/app/tests/glossary.rs: 'sha256:bb3266feceb40eb1c3c4847deb16921f3432c141ca48f81cb8d8ab367e00fa32'
  crates/app/tests/prose.rs: 'sha256:b3b194d61079eaeef5626f13b7b1d59f308fe619c376a967181913fc0c47b441'
  docs/05-roadmap.md: 'sha256:053371b4421327ea70ca30a39835b3b60f8726fe9944622dcf4cc402c50cfb0c'
tags:
  - d27
  - f2.12
  - migracion
  - guardas
  - traduccion
---

## What it does

El código propio de este proyecto está en español **desde el primer commit** y esa decisión no estaba escrita en ninguna parte, que es lo que la dejó crecer hasta 43.860 líneas. **D27** la revierte: todo el repositorio pasa a inglés — nombres, prosa, documentos y el conocimiento almacenado — **sin tabla de alias en ninguna clave**, porque nadie lo tiene instalado y es la única ventana en la que sale gratis.

El hito va en **16 entregas** con un glosario propio que decide, término a término, cuándo se adopta la palabra que ya usan WezTerm, Ghostty o tmux y cuándo se acuña una — y **prohíbe adoptar una palabra que el proyecto ya gastó en otra cosa**. Esa prohibición es la que más se ha incumplido en la práctica, porque una palabra inglesa mal elegida no se parece a un error.

Lo que esta funcionalidad recoge no es cómo se traduce, sino **qué se equivoca al traducir a escala y cuál de las guardas lo caza** — porque la respuesta ha sido, más de una vez, ninguna: el guardián del glosario caza nombres **españoles retirados**, y hasta septiembre de 2026 declaraba en su propia cabecera que comentarios, rustdoc y literales quedaban **fuera de su alcance**.

## Where it lives

**La decisión y el glosario**: `docs/02-decisiones.md`, anexo de **D27** — la tabla término a término, lo que no se traduce nunca (las letras de tanda `A`–`Q`, entre otras) y qué nombres son el formato en disco, que es el único sitio donde renombrar rompe algo sin que nada falle.

**El calendario**: `docs/05-roadmap.md`, Parte II.

**Las guardas, y su alcance real declarado**:
- `crates/app/tests/glossary.rs` — identificadores. `FINISHED_CRATES` es la afirmación fuerte («ningún identificador español») y a septiembre de 2026 solo lleva `panel-contract`, `host-load` y `term-core`; `GLOSSARY_CLEAN_FILES` es la débil («ningún nombre que el glosario retirase») y es la que cubre los ficheros de `app`. `NOT_OURS` es su lista de exenciones, y su tamaño es una señal: cada entrada silencia una clase.
- `crates/app/tests/prose.rs` — la prosa, desde la entrega #12. Antes de ella **nada impedía devolver español a un fichero ya traducido**.
- `crates/app/src/settings.rs` — dos recorridos sobre las frases que se ven por pantalla, con `SPANISH_FUNCTION_WORDS` como oráculo: lista **cerrada**, y ninguna de sus palabras es también inglesa, que es por lo que «no» y «sin» no pueden entrar.
