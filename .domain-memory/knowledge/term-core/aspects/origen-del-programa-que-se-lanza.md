---
id: asp_origen-del-programa-que-se-lanza_jmepbw
slug: origen-del-programa-que-se-lanza
name: El origen del programa que se lanza decide si term-core es superficie sensible
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-07T19:11:28.224Z'
updated_at: '2026-08-09T18:26:56.608Z'
last_verified: '2026-08-09T18:26:56.608Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/lib.rs
  - docs/05-roadmap.md
symbols:
  - 'Pty::spawn'
  - arrancar
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:dbd22bb07beff290b27a27223f089e5bd44d914c5530d09bbd739011e72d03c9'
  crates/app/src/lib.rs: 'sha256:ea51aded92eaf7e48a86b358573c4b5598a7487cdab504d8a5b6302c4a30d2b3'
  docs/05-roadmap.md: 'sha256:27e4b1b7ac4e1d6e3504f73415a2a8dbd76e8521dd81c1f0c4ab6af4a51042de'
tags:
  - seguridad
  - proceso-hijo
  - f1.5
feature_id: feat_term-core_nx0g1r
---

## What it does

`Pty::spawn` **sigue sin ser superficie sensible**, pero desde F1.3 la razón ya no es «el programa está fijado en código» — porque no lo está.

**Lo que cambió (F1.3, el PR del bucle).** `app` lanza el shell que dice `$SHELL`, con `/bin/sh` de respaldo si esa variable falta o el binario no se puede lanzar. Con `/bin/sh` fijo, la pantalla que hay que mirar para juzgar egui es un shell sin prompt ni historial — no representa el uso real, y el hito entero existe para mirar esa pantalla.

**Por qué eso no abre superficie.** La variable viene del entorno de la sesión de quien abre la aplicación, no de configuración de la app ni de un `panel.json` de origen ajeno. Quien controle ese entorno ya controla la sesión: no hay elevación, no hay nada que un atacante alcance que no alcanzara antes. Y `portable-pty` ya copia el entorno entero del padre al hijo, así que el hijo ve esa sesión con `$SHELL` o sin él; lo único que la caja fija a mano es `TERM` y `COLORTERM`, ninguna credencial.

**La condición que invalida esta valoración no ha cambiado: F1.5.** Ese hito inyecta `PANEL_ID`/`PANEL_DIR` y abre la puerta a que el programa venga de la configuración del usuario. En cuanto el programa o sus argumentos procedan de configuración, de un `panel.json` —que puede ser de origen ajeno: repo clonado, contenedor— o de cualquier otra entrada **externa al proceso**, `spawn` pasa a ser superficie sensible y hay que revisarla como tal.

**La frontera, dicha de una vez, porque es la que hay que saber aplicar:** heredar el entorno de la propia sesión no abre superficie; recibir el programa desde un fichero o una configuración que otro pudo escribir, sí. Las otras dos reglas sobre el proceso hijo preguntan lo mismo —qué entra y de dónde viene—: que un `panel.json` ajeno no es confiable, y que la app nunca define `ANTHROPIC_API_KEY` al lanzar un panel (D4).

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn`, que fija `TERM`/`COLORTERM` y hereda el resto del entorno. `crates/app/src/lib.rs` — `Ventana::nueva`, que lee `$SHELL` y cae a `/bin/sh`. La condición de invalidación está escrita en el hito F1.5 de `docs/05-roadmap.md`.
