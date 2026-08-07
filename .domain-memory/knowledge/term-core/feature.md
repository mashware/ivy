---
id: feat_term-core_nx0g1r
slug: term-core
name: term-core — el PTY y la rejilla de una división
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-07T19:11:09.799Z'
updated_at: '2026-08-07T19:11:09.799Z'
last_verified: '2026-08-07T19:11:09.799Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/shell.rs
symbols:
  - Pty
  - Size
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:21f7f713225e6dcff89ef43f9b4945f03c264d3071780325d2969d2661de4d18'
  crates/term-core/examples/shell.rs: 'sha256:316b3b2821edca42c7b5b15633c9ad0743a8db9eee90986d65db3203a22cac3d'
tags:
  - fase-1
  - pty
  - arquitectura
---

## What it does

La caja que sabe ejecutar y leer un programa dentro de un pseudoterminal. Existe separada de la capa de pintado por una razón económica, no estética: la interfaz se va a reescribir (egui ahora, GPUI cuando haga falta, ver D3), y cuando eso pase esta caja tiene que seguir sirviendo tal cual. Por eso no depende de nada visual y por eso los tipos de sus dependencias (`portable-pty`) no asoman en su API pública — quien la usa no debería tener que conocerlas ni migrar con ellas.

Entrega los bytes **crudos**: no interpreta, no valida UTF-8, no normaliza saltos de línea. Una secuencia de escape puede llegar partida entre dos mensajes del canal, y eso es correcto. Quien interpreta es la capa de arriba — hasta F1.2, el terminal donde se imprime; a partir de ahí, `alacritty_terminal`.

## Where it lives

`crates/term-core/`. El `example` `shell.rs` es la demostración a mano de que funciona.
