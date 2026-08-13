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
updated_at: '2026-08-13T18:42:42.215Z'
last_verified: '2026-08-13T18:42:42.215Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/entrega.rs
symbols:
  - 'Pty::spawn'
  - 'Pty::spawn_with_env'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:6d8ac25cbaae68a1d230090cc4c349577ba67801bee3f7d5de632814ca3a4289'
  crates/term-core/examples/entrega.rs: 'sha256:3dfd5af97ca821e1a62a34e1f80ab47d4567a47f685d0af62a470488a8686e60'
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

> **Estado (13 de agosto de 2026): `OPOST` sigue encendido, y ya no por descuido sino por decisión.** Se intentó apagarlo, se entregó y se revirtió el mismo día: reponer el retorno de carro desde fuera es imposible de hacer bien. Ver [[onlcr-no-distingue-el-modo-crudo]] y **D12**.

**El búfer del núcleo es pequeño: 20.480 bytes en modo crudo, 19.456 en canónico.** Reproducible en tres corridas. Es el margen exacto entre dejar de leer el maestro y que el programa de dentro se bloquee al escribir, y es **seis veces menor que los 128 KiB del presupuesto por fotograma**. Quien diseñe contrapresión tiene que contar con que el margen que regala el núcleo es corto, no generoso.

**El bloqueo al escribir es real, inmediato y no pierde nada.** 5 MiB escritos con el maestro sin leer y el lector arrancado a los 300 ms: `write()` bloqueado esos mismos 300 ms, **0 bytes perdidos**. Es la premisa sobre la que descansa todo el freno del terminal, y queda comprobada en vez de asumida.

**Lo que cuesta dos órdenes de magnitud NO es el modo canónico ni el eco: es `OPOST`/`ONLCR`**, la traducción de `\n` a `\r\n` en la salida. Aislado flag a flag:

| Configuración | Entrega |
|---|---|
| `ICANON` on, `ECHO` on, `OPOST` on — lo que se usa | **3,8–4,6 MiB/s** |
| igual pero `OPOST` off | **326–329 MiB/s** |
| crudo completo (`cfmakeraw`) | **384 MiB/s** |

**El «×80» necesita un adverbio, y esto se midió después: lo que se paga es cada salto de línea, no cada byte.** Con la cadena entera de `term-core` y doce megas volcados con `cat` (`cargo run --release --example entrega -p term-core`):

| Línea | Procesando en el núcleo | Sin procesar | Relación |
|---|---|---|---|
| de log, 70 B | 53,3 MiB/s | 208,2 MiB/s | **4×** |
| corta, 2 B | 2,4 MiB/s | 270,0 MiB/s | **112×** |

O sea: **el 4 MiB/s de la tabla de arriba se reproduce con líneas de dos bytes, no con líneas de log.** Publicar un solo número —cualquiera de los dos— publica algo que parece universal y no lo es. Eso explica de paso por qué `yes` solo entrega ~4 MiB/s: no porque escriba de dos en dos bytes (`strace` muestra escrituras de 16 KiB), sino porque su línea son dos. Corolario incómodo que sigue en pie: **`yes` no sirve para probar un fallo de contrapresión**, porque se autolimita por debajo de lo que la rejilla drena.

**Y el cuello de esta aplicación, de paso.** Drenando el maestro directamente con lecturas de 8 KiB y sin la cadena de `term-core` por medio: **46,8 → 358,7 MiB/s**. Por el canal de `term-core` salían ~170 en la versión que se revirtió. Si algún día el coste del pseudoterminal importa de verdad, **hay más que ganar en el hilo lector y en las colas que en `termios`**.

**Trampa al volver a medir esto**: generar las líneas con un `while` de `sh` y `printf` da **12 MiB/s con el interruptor puesto de las dos formas**, porque el cuello es el bucle del shell. La medida sale «1×» y entierra el hallazgo con un número tranquilizador y falso. Hay que volcar un fichero con `cat`.

**`portable-pty` no interpone búfer propio**: `try_clone_reader` es un `dup(2)` sobre el descriptor (`unix.rs:352-355`) y `Read`/`Write` llaman directo a `libc::read`/`libc::write` sin `BufReader` (`filedescriptor-0.8.3/src/unix.rs:208-221`). No hay un tercer sitio donde se acumulen bytes, lo que confirma que las colas del camino son exactamente dos.

**Nada de esto traslada a Windows.** Allí `portable-pty` no usa pseudoterminal ni `termios`: crea dos tuberías anónimas con `CreatePipe(..., 0)` —tamaño por defecto del sistema— y se las pasa a `CreatePseudoConsole`. Ni el tamaño del búfer ni el coste de `ONLCR` significan nada en ese camino, así que ningún arreglo debe apoyarse en estos números. macOS comparte el código `unix` pero **no se midió**.

**Cuándo dejaría de valer**: si cambia la versión de `portable-pty`, o si se mide en otro núcleo o en otra máquina.

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn_with_env`, que abre el pseudoterminal y **no toca `termios`**: ahí se decide todo lo de arriba, y desde D12 esa omisión es deliberada. `crates/term-core/examples/entrega.rs` es la medida reejecutable de los dos largos de línea. La fuente de `portable-pty` 0.9.0 y `filedescriptor` 0.8.3 en el registro local de cargo es donde se comprobó que no hay búfer intermedio.
