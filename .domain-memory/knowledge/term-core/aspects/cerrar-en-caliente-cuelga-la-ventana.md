---
id: asp_cerrar-en-caliente-cuelga-la-ventana_s1ca6d
slug: cerrar-en-caliente-cuelga-la-ventana
name: >-
  Cerrar un pseudoterminal en caliente congela la ventana 101 ms y puede
  colgarla para siempre: la señal va al pid y el nieto se queda el esclavo
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:24:06.689Z'
updated_at: '2026-08-15T21:24:06.689Z'
last_verified: '2026-08-15T21:24:06.689Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/lib.rs
symbols:
  - Pty
  - Puente
  - Drop
  - parar
  - try_clone_reader
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:6d8ac25cbaae68a1d230090cc4c349577ba67801bee3f7d5de632814ca3a4289'
  crates/app/src/lib.rs: 'sha256:b3ca4328c99c0b94b314ea9a6c6d1ab4f6afe0dad22ef3415700f289bc9af411'
tags:
  - medido
  - cierre
  - hilos
  - pseudoterminal
  - F2.1
feature_id: feat_term-core_nx0g1r
---

## What it does

**Medido en esta máquina contra `term-core`, no razonado.** `Drop for Pty` hace tres cosas bloqueantes y **ninguna tiene plazo**: `child.kill()`, `child.wait()` y `reader.join()`. Mientras eso solo corría al cerrar la aplicación, un cuelgue se confundía con «tarda en cerrarse». Desde F2.1 se puede cerrar una pestaña **en caliente**, y ese mismo camino corre **dentro de la pasada de interfaz**.

**Congela la ventana ~101 ms por pestaña cerrada.** `child.kill()` de `portable-pty` **no manda SIGKILL: manda SIGHUP**, y después concede gracia sondeando con esperas de 50 ms. El primer sondeo va justo tras la señal, así que en el caso normal —el programa sí muere, pero no le da tiempo antes del primer sondeo— se paga un ciclo entero. Diez muestras con un shell vivo dentro: mediana **50,72 ms**, desviación **por debajo del milisegundo** — no es ruido, es el `sleep` fijo. Con el programa ya muerto, **0,23 ms**. Dos consolas por pestaña en serie → ~101 ms, unos 17 fotogramas del presupuesto que D3 publica. Con un programa que ignore SIGHUP, **404 ms**.

**Y puede colgarla indefinidamente.** Tres hechos que por separado están bien:

1. La señal va **al pid del hijo**, no a su grupo de procesos: lo que el shell haya lanzado y le sobreviva no recibe nada.
2. Ese nieto **hereda los descriptores del extremo esclavo**. Mientras alguien lo tenga abierto, `read()` no llega nunca a fin de fichero. El comentario que dice «el único que lo tiene es el hijo» **deja de ser cierto en cuanto el shell lanza algo que le sobreviva — que es lo que hace un shell**.
3. La bandera `parar` **solo se consulta al intentar poner algo en el canal**. Un hilo bloqueado leyendo bytes que no llegan no la mira jamás.

Resultado: `reader.join()` no vuelve y **la ventana entera queda congelada, sin repintar ni teclado**. Medido con `sh -c "setsid sleep 60 & exec sleep 60"` —lo que hace cualquier cosa lanzada con `&`, un `nohup` o un servidor de desarrollo—: **no terminó en 8 segundos**. Igual con un programa que ignora la señal, donde el SIGKILL de gracia mata al shell y deja al nieto huérfano con el pseudoterminal abierto.

> **Lo predijo [[cada-espera-anade-un-camino-de-cierre]] días antes**, con estas palabras: «se convierte en una fuga por cada división que se cierre en ese instante **en cuanto cerrar una división deje de cerrar el proceso**». Eso ocurrió en F2.1, y el síntoma resultó ser peor que una fuga.

**Por dónde se arregla**, por orden de lo que más resuelve: señalar al **grupo de procesos** (`killpg`) en vez de al pid —el hijo ya es líder de sesión porque `portable-pty` hace `setsid()` en `pre_exec`—, que lo resuelve en su origen; dar al hilo lector forma de abandonar **sin fin de fichero**, o no unirse a él sin plazo (cerrar el maestro no basta: el hilo posee un descriptor propio de `try_clone_reader()`); y no pagar el sondeo de la biblioteca dentro del fotograma.

**No hay zombis:** `child.wait()` cosecha al hijo directo, y a los nietos huérfanos los adopta `init`.

## Where it lives

`Drop for Pty` en `crates/term-core/src/lib.rs`, invocado al soltar una `Pestaña` desde `Ventana::cerrar_la_pestaña_activa` en `crates/app/src/lib.rs`, dentro de `eframe::App::ui`. Ticket abierto con las mediciones: https://github.com/mashware/agent-terminal/issues/62
