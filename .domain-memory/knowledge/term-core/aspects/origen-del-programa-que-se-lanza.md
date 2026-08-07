---
id: asp_origen-del-programa-que-se-lanza_jmepbw
slug: origen-del-programa-que-se-lanza
name: El origen del programa que se lanza decide si term-core es superficie sensible
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-07T19:11:28.224Z'
updated_at: '2026-08-07T19:11:28.224Z'
last_verified: '2026-08-07T19:11:28.224Z'
file_paths:
  - crates/term-core/src/lib.rs
  - docs/05-roadmap.md
symbols:
  - 'Pty::spawn'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:21f7f713225e6dcff89ef43f9b4945f03c264d3071780325d2969d2661de4d18'
  docs/05-roadmap.md: 'sha256:6fe1601f30d97065dad492557fd89921971490411bcf276336b7c3129a8bd7d4'
tags:
  - seguridad
  - pty
  - fase-1
  - revision
feature_id: feat_term-core_nx0g1r
---

## What it does

Hoy `Pty::spawn` **no** es superficie sensible: el programa y sus argumentos los fija quien llama, y todos los llamantes viven dentro del workspace (el `example`, y desde F1.3 el crate `app`). Nada entra desde fuera del proceso, así que no hay vector de inyección y la revisión puede correr al nivel que corresponda al tamaño del cambio, sin panel de seguridad.

**La condición que invalida esa valoración es concreta: F1.5.** Ese hito inyecta `PANEL_ID`/`PANEL_DIR` y abre la puerta a que el shell venga de la configuración del usuario en vez de estar fijado en código. En cuanto el programa o sus argumentos procedan de configuración, de un `panel.json` —que puede ser de origen ajeno: repo clonado, contenedor— o de cualquier entrada externa, `spawn` pasa a ser superficie sensible y hay que revisarla como tal.

Mira esto desde el mismo ángulo que las otras dos reglas sobre el entorno del proceso hijo: que un `panel.json` ajeno no es confiable, y que la app nunca define `ANTHROPIC_API_KEY` al lanzar un panel (D4). Las tres preguntan lo mismo — qué entra en el proceso hijo y de dónde viene.

## Where it lives

`crates/term-core/src/lib.rs` (`Pty::spawn`). El hito que invalida la valoración está en `docs/05-roadmap.md`, Fase 1, F1.5; las reglas hermanas, en `docs/02-decisiones.md` (D4) y `docs/03-arquitectura.md` (tabla de seguridad).
