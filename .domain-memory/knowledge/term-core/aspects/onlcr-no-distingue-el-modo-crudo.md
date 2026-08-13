---
id: asp_onlcr-no-distingue-el-modo-crudo_thhb6n
slug: onlcr-no-distingue-el-modo-crudo
name: >-
  No se puede apagar OPOST y reponer el retorno desde fuera: el modo crudo
  estándar deja el terminal en el mismo estado, bit a bit
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T18:41:54.127Z'
updated_at: '2026-08-13T18:41:54.127Z'
last_verified: '2026-08-13T18:41:54.127Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/term-core/examples/entrega.rs
symbols:
  - 'Pty::spawn_with_env'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:6d8ac25cbaae68a1d230090cc4c349577ba67801bee3f7d5de632814ca3a4289'
  crates/term-core/examples/entrega.rs: 'sha256:3dfd5af97ca821e1a62a34e1f80ab47d4567a47f685d0af62a470488a8686e60'
tags:
  - pseudoterminal
  - termios
  - rendimiento
  - ncurses
  - descartado
  - revertido
feature_id: feat_term-core_nx0g1r
---

## What it does

**Se intentó, se entregó y se revirtió.** Apagar `OPOST` en el pseudoterminal da entre 4× y 112× según el largo de línea ([[el-pseudoterminal-medido]]), pero obliga a reponer el retorno de carro desde fuera, y **eso exige saber si el programa de dentro lo quiere**. El núcleo lo sabía porque traducía según el `termios` de cada uno. Desde fuera **la información no está**.

| Estado del terminal | ¿El núcleo traduce? | Quién lo deja así |
|---|---|---|
| `OPOST` on, `ONLCR` on | sí | nadie lo ha tocado |
| `OPOST` off, `ONLCR` on | **no** | `stty raw`, `cfmakeraw`, `tty.setraw`, `enable_raw_mode` de crossterm, `MakeRaw` de Go |
| `OPOST` off, `ONLCR` on | **no** | **y también nosotros, si lo apagáramos** |

**Las dos últimas filas son el mismo estado, bit a bit** — comprobado ejecutándolo, no razonado. El modo crudo estándar apaga **solo** `OPOST` (glibc `cfmakeraw` hace `c_oflag &= ~OPOST` y no toca nada más del `c_oflag`) y deja `ONLCR` puesto, que es exactamente lo que dejaría un apagado nuestro.

> **La regla que deja, y es general:** en cuanto le mientes al programa sobre el estado de su terminal, **pierdes la información de quién puso qué**. No es que falte una señal mejor: es que el núcleo la destruye al aceptar la escritura. Por eso ningún emulador de terminal apaga `OPOST`.

**Lo que se probó y no bastó:**

- **Condicionar a `ONLCR`.** Es la señal que usa `ncurses` —su manual (`curs_outopts`, `nonl`) dice que solo usa el salto como movimiento después de `nonl()`— y **cubre a `vim`, `htop` y `nano`**, que lo apagan al arrancar y lo restauran al salir. No cubre nada que no sea `ncurses`: cualquier TUI de `ratatui`, de Node o de Go entra en crudo sin tocarlo. **Esta fue la versión entregada y revertida.**
- **Comparar el `termios` entero contra el que dejamos.** Falla igual: `stty raw` produce nuestro mismo `c_oflag`.
- **Añadir `ICANON` a la condición.** Arregla el modo crudo y rompe a un programa de `ncurses` en `cbreak`, que apaga `ICANON` sin apagar `ONLCR`. Cambia un agujero por otro.
- **El modo `LNM` del motor de terminal**: ver [[el-modo-lnm-no-repone-el-retorno]].

**Y una trampa aparte, por si alguien lo reintenta:** emular al **leer** algo que el núcleo hacía al **escribir** tiene una ventana del tamaño de su búfer (~20 KiB), y **con contrapresión no son microsegundos sino segundos**, porque el hilo lector está parado esperando sitio. Medido: 17 KiB de un log en escalera al abrir `vim` detrás de un `cat` grande.

**Y cómo se descubrió, que importa tanto como el qué:** el diseño midió `vim`, `htop` y `nano` —los tres que confirman la regla— y dio el contrato de `ncurses` por universal. La esquina quedó escrita en la propia decisión como «ninguno de los probados lo hace». **Probar solo los casos que confirman la regla es la forma de que una esquina declarada sea la que te tumba.**

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn_with_env` **no toca `termios`**, y eso ahora es deliberado. `crates/term-core/examples/entrega.rs` mide el coste que se paga a propósito. La autopsia entera está en **D12** de `docs/02-decisiones.md`.
