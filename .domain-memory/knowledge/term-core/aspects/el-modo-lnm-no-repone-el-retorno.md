---
id: asp_el-modo-lnm-no-repone-el-retorno_j05mbs
slug: el-modo-lnm-no-repone-el-retorno
name: >-
  El modo LNM del terminal no sirve para reponer lo que hacía el núcleo: un
  reset lo borra, y cambia una garantía por una preferencia
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T18:29:16.063Z'
updated_at: '2026-08-13T18:43:07.234Z'
last_verified: '2026-08-13T18:43:07.234Z'
file_paths:
  - crates/term-core/src/screen.rs
symbols:
  - 'Screen::advance'
  - 'Screen::new'
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:858f97327f67c34b3921b2beb9f07f3b3ebe6c26c2d6a915d535bb6e1e7a04a0'
tags:
  - alacritty-terminal
  - modos
  - pintado
  - descartado
feature_id: feat_term-core_nx0g1r
---

## What it does

**`alacritty_terminal` ya sabe poner el retorno de carro en cada salto —el modo `LINE_FEED_NEW_LINE`, el `LNM` de ANSI (`ESC[20h`)— y aun así no vale para reponer lo que hacía `ONLCR` en el núcleo.** Es la solución obvia, cabe en una línea, y por eso conviene que quede escrito por qué se descartó.

> **Contexto: la pregunta entera acabó respondida que no.** Reponer el retorno desde fuera resultó imposible de hacer bien por otra razón distinta y más de fondo — ver [[onlcr-no-distingue-el-modo-crudo]]. Esta ficha sigue valiendo como autopsia de **esta** alternativa concreta, que es la primera que se le ocurre a cualquiera.

Cómo funciona: `newline()` (`src/term/mod.rs:1470`) llama a `linefeed()` y, **solo si ese modo está puesto**, también a `carriage_return()`. `linefeed()` (`:1423`) baja de línea y **no toca la columna**: esa es la escalera.

**Por qué no vale:** `reset_state()` (`:1835`) —lo que ejecuta `ESC c`, o sea `reset` y `tput reset`— hace `self.mode &= TermMode::VI; self.mode.insert(TermMode::default())`, y `TermMode::default()` (`:113`) es `SHOW_CURSOR | LINE_WRAP | ALTERNATE_SCROLL | URGENCY_HINTS`: **`LINE_FEED_NEW_LINE` no está**. Un `reset` escrito en la consola borraría el modo y dejaría la escalera **para siempre** en esa división.

Dos razones más, menores: `ESC[20l` lo apaga desde dentro, y el modo **no actúa dentro de una carga `OSC`/`DCS`** mientras que `ONLCR` sí —el núcleo no parsea—, así que el modo tampoco es fiel.

> **La regla que deja:** un modo del terminal es estado que el programa de dentro puede pisar. Reponer con un modo algo que antes garantizaba el núcleo **cambia una garantía por una preferencia**, y basta un `reset` para perderla.

## Where it lives

`crates/term-core/src/screen.rs` — `Screen::new` construye el `Term` y **no fija ningún modo**, que es la decisión; `Screen::advance` es una línea que pasa los bytes al parser sin tocarlos. Las líneas citadas de `alacritty_terminal` son de la 0.26.0 en el registro local de cargo. Escrito como alternativa descartada en **D12** de `docs/02-decisiones.md`.
