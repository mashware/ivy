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
updated_at: '2026-08-13T18:28:16.759Z'
last_verified: '2026-08-13T18:28:16.759Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/entrega.rs
symbols:
  - 'Pty::spawn'
  - 'Pty::spawn_with_env'
  - ProcesadoDeSalida
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:2f7280fbd9a155e9d432ba2db3b11f67fbec767928a454bf2643cb0cd575d2e7'
  crates/term-core/examples/entrega.rs: 'sha256:dee77a44c96109d1dfd1212fb2921d7a61ab4ee3af30d6dbd3cbbd239311eb79'
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

> **AVISO DE ESTADO (13 de agosto de 2026): la condición de caducidad de esta ficha se ha cumplido.** Decía «dejaría de valer si alguien configura `termios` en `Pty::spawn`», y eso es exactamente lo que hizo **D12**: hoy el pseudoterminal se abre **con `OPOST` apagado**. Los números de abajo siguen siendo válidos como caracterización del coste del flag; lo que ya no describen es la configuración que corre. Ver [[onlcr-es-la-senal-del-programa]].

**El búfer del núcleo es pequeño: 20.480 bytes en modo crudo, 19.456 en canónico.** Reproducible en tres corridas. Es el margen exacto entre dejar de leer el maestro y que el programa de dentro se bloquee al escribir, y es **seis veces menor que los 128 KiB del presupuesto por fotograma**. Quien diseñe contrapresión tiene que contar con que el margen que regala el núcleo es corto, no generoso.

**El bloqueo al escribir es real, inmediato y no pierde nada.** 5 MiB escritos con el maestro sin leer y el lector arrancado a los 300 ms: `write()` bloqueado esos mismos 300 ms, **0 bytes perdidos**. Es la premisa sobre la que descansa todo el freno del terminal, y queda comprobada en vez de asumida.

**Lo que cuesta dos órdenes de magnitud NO es el modo canónico ni el eco: es `OPOST`/`ONLCR`**, la traducción de `\n` a `\r\n` en la salida. Aislado flag a flag:

| Configuración | Entrega |
|---|---|
| `ICANON` on, `ECHO` on, `OPOST` on — lo que se usaba hasta D12 | **3,8–4,6 MiB/s** |
| igual pero `OPOST` off | **326–329 MiB/s** |
| crudo completo (`cfmakeraw`) | **384 MiB/s** |

**El «×80» necesita un adverbio, y esto se midió después: lo que se paga es cada salto de línea, no cada byte.** Con la cadena entera de `term-core` y doce megas volcados con `cat` (`cargo run --release --example entrega -p term-core`):

| Línea | Procesando en el núcleo | Sin procesar | Relación |
|---|---|---|---|
| de log, 70 B | 53,8 MiB/s | 162,9 MiB/s | **3×** |
| corta, 2 B | 2,6 MiB/s | 170,2 MiB/s | **65×** |

O sea: **el 4 MiB/s de la tabla de arriba se reproduce con líneas de dos bytes, no con líneas de log.** Publicar un solo número —cualquiera de los dos— publica algo que parece universal y no lo es. Eso explica de paso por qué `yes` solo entrega ~4 MiB/s: no porque escriba de dos en dos bytes (`strace` muestra escrituras de 16 KiB), sino porque su línea son dos. Corolario incómodo que sigue en pie: **`yes` no sirve para probar un fallo de contrapresión**, porque se autolimita por debajo de lo que la rejilla drena.

**Y el cuello pasa a ser la aplicación, no el núcleo.** Medido aparte, drenando el maestro directamente con lecturas de 8 KiB y sin la cadena de `term-core` por medio: **46,8 → 358,7 MiB/s**. Por el canal de `term-core` salen ~170. Lo que falta se lo llevan el hilo lector, la traducción de saltos y la cola con techo de D8.

**Trampa al volver a medir esto**: generar las líneas con un `while` de `sh` y `printf` da **12 MiB/s con el interruptor puesto de las dos formas**, porque el cuello es el bucle del shell. La medida sale «1×» y entierra el hallazgo con un número tranquilizador y falso. Hay que volcar un fichero con `cat`.

**`portable-pty` no interpone búfer propio**: `try_clone_reader` es un `dup(2)` sobre el descriptor (`unix.rs:352-355`) y `Read`/`Write` llaman directo a `libc::read`/`libc::write` sin `BufReader` (`filedescriptor-0.8.3/src/unix.rs:208-221`). No hay un tercer sitio donde se acumulen bytes, lo que confirma que las colas del camino son exactamente dos.

**Nada de esto traslada a Windows.** Allí `portable-pty` no usa pseudoterminal ni `termios`: crea dos tuberías anónimas con `CreatePipe(..., 0)` —tamaño por defecto del sistema— y se las pasa a `CreatePseudoConsole`. Ni el tamaño del búfer ni el coste de `ONLCR` significan nada en ese camino, así que ningún arreglo debe apoyarse en estos números. macOS comparte el código `unix` pero **no se midió**.

**Cuándo dejaría de valer el resto**: si cambia la versión de `portable-pty`, o si se mide en otro núcleo o en otra máquina.

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn_with_env`, que abre el pseudoterminal y **desde D12 sí toca `termios`** (apaga `OPOST`), y el tipo `ProcesadoDeSalida`, que hace en el hilo lector el trabajo que el núcleo dejó de hacer. `crates/term-core/examples/entrega.rs` es la medida reejecutable de los dos largos de línea. La fuente de `portable-pty` 0.9.0 y `filedescriptor` 0.8.3 en el registro local de cargo es donde se comprobó que no hay búfer intermedio.
