---
id: asp_el-pseudoterminal-medido_61rp32
slug: el-pseudoterminal-medido
name: >-
  El pseudoterminal de Linux, medido: 20 KiB de margen antes de bloquear, y
  OPOST como culpable real de la lentitud
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-09T22:15:41.845Z'
updated_at: '2026-08-09T22:15:41.845Z'
last_verified: '2026-08-09T22:15:41.845Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - 'Pty::spawn'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:efa66ec48f1e9c4ba5f5d05657492ca563960b543264d613c06cd094b0818c73'
tags:
  - pseudoterminal
  - termios
  - rendimiento
  - contrapresion
  - portabilidad
feature_id: feat_term-core_nx0g1r
---

## What it does

Medido con programas en C sobre un pseudoterminal real (Linux 7.0.0-29, `portable-pty` 0.9.0), **no supuesto** — aplicando la regla del proyecto de leer la API ajena en vez de razonar sobre ella.

**El búfer del núcleo es pequeño: 20.480 bytes en modo crudo, 19.456 en canónico.** Reproducible en tres corridas. Es el margen exacto entre dejar de leer el maestro y que el programa de dentro se bloquee al escribir, y es **seis veces menor que los 128 KiB del presupuesto por fotograma**. Quien diseñe contrapresión tiene que contar con que el margen que regala el núcleo es corto, no generoso.

**El bloqueo al escribir es real, inmediato y no pierde nada.** 5 MiB escritos con el maestro sin leer y el lector arrancado a los 300 ms: `write()` bloqueado esos mismos 300 ms, **0 bytes perdidos**. Es la premisa sobre la que descansa todo el freno del terminal, y queda comprobada en vez de asumida.

**Lo que cuesta dos órdenes de magnitud NO es el modo canónico ni el eco: es `OPOST`/`ONLCR`**, la traducción de `\n` a `\r\n` en la salida. Aislado flag a flag:

| Configuración | Entrega |
|---|---|
| `ICANON` on, `ECHO` on, `OPOST` on — lo que se usa hoy | **3,8–4,6 MiB/s** |
| igual pero `OPOST` off | **326–329 MiB/s** |
| crudo completo (`cfmakeraw`) | **384 MiB/s** |

`Pty::spawn` nunca toca `termios`, así que corre con el valor por defecto del núcleo, o sea `OPOST` encendido. **Consecuencia práctica: apagarlo daría ~×80 en cualquier programa que imprima líneas, que son todos.** Y explica por qué `yes` solo entrega ~4 MiB/s — no porque escriba de dos en dos bytes (`strace` muestra escrituras de 16 KiB), sino por el coste de cada `\n` de dentro. Corolario incómodo: **`yes` no sirve para probar un fallo de contrapresión**, porque se autolimita por debajo de lo que la rejilla drena.

**`portable-pty` no interpone búfer propio**: `try_clone_reader` es un `dup(2)` sobre el descriptor (`unix.rs:352-355`) y `Read`/`Write` llaman directo a `libc::read`/`libc::write` sin `BufReader` (`filedescriptor-0.8.3/src/unix.rs:208-221`). No hay un tercer sitio donde se acumulen bytes, lo que confirma que las colas del camino son exactamente dos.

**Nada de esto traslada a Windows.** Allí `portable-pty` no usa pseudoterminal ni `termios`: crea dos tuberías anónimas con `CreatePipe(..., 0)` —tamaño por defecto del sistema— y se las pasa a `CreatePseudoConsole`. Ni el tamaño del búfer ni el coste de `ONLCR` significan nada en ese camino, así que ningún arreglo debe apoyarse en estos números. macOS comparte el código `unix` pero **no se midió**.

**Cuándo dejaría de valer**: si alguien configura `termios` en `Pty::spawn`, si cambia la versión de `portable-pty`, o si se mide en otro núcleo.

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn`, que abre el pseudoterminal y **no toca `termios`**: ahí se decide, por omisión, todo lo de arriba. La fuente de `portable-pty` 0.9.0 y `filedescriptor` 0.8.3 en el registro local de cargo es donde se comprobó que no hay búfer intermedio.
