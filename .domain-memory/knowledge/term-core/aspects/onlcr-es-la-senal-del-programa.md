---
id: asp_onlcr-es-la-senal-del-programa_x7zqk9
slug: onlcr-es-la-senal-del-programa
name: >-
  Quitarle al núcleo la traducción del salto de línea obliga a preguntarle a
  ONLCR: es la señal por la que un programa dice si la quiere
type: aspect
status: superseded
superseded_by: asp_onlcr-no-distingue-el-modo-crudo_thhb6n
confidence: 80
created_at: '2026-08-13T18:28:50.597Z'
updated_at: '2026-08-13T18:42:02.621Z'
last_verified: '2026-08-13T18:28:50.597Z'
file_paths:
  - crates/term-core/src/lib.rs
symbols:
  - ProcesadoDeSalida
  - 'Pty::spawn_with_env'
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:2f7280fbd9a155e9d432ba2db3b11f67fbec767928a454bf2643cb0cd575d2e7'
tags:
  - pseudoterminal
  - termios
  - rendimiento
  - ncurses
  - pintado
feature_id: feat_term-core_nx0g1r
---

## What it does

**Apagar `OPOST` obliga a hacerse cargo del salto de línea, y la parte que no se ve venir es que la traducción NO puede ser incondicional.**

El núcleo traducía `\n` a `\r\n` **según el `termios` de cada programa**, y los programas a pantalla completa cuentan con poder apagarla. Medido leyendo el `c_oflag` del **maestro** mientras el programa corre:

| Programa | Mientras corre | Al salir |
|---|---|---|
| `vim` | `ONLCR` **off** | `ONLCR` on |
| `htop` | `ONLCR` **off** | `ONLCR` on |
| `nano` | `ONLCR` **off** | — |
| `less` | **reenciende `OPOST`** | restaura `OPOST` off |

Y `infocmp xterm-256color` —el tipo de terminal que `FIXED_ENV` le declara al hijo— da `cud1=\n` e `ind=\n`: para esos programas un salto desnudo significa **bajar sin mover la columna**. Traducirles descoloca su repintado.

> **La regla correcta es «traducir si y solo si `ONLCR` sigue puesto», y no es una heurística de esta aplicación: es el contrato que los programas ya respetan.** El manual de `ncurses` (`curs_outopts`, `nonl`) dice que solo usa la bajada de línea como movimiento **después de `nonl()`** — que es justo lo que apaga `ONLCR`. Mientras ese bit esté puesto, ningún programa de curses manda un salto desnudo esperando quedarse en su columna.

**Dos propiedades hacen que la regla funcione sola**, y son las que evitan tener que llevar la cuenta de nada: **`ONLCR` sobrevive intacto cuando solo se apaga `OPOST`** —en `stty -a` sigue saliendo sin guion, subflag inerte—, así que queda como declaración de lo que el programa quiere; y **los programas lo restauran al salir**, así que se deja de traducir al entrar en `vim` y se vuelve a traducir al salir.

**Y traducir de más es inocuo**, que es lo que impide que las dos mitades se desincronicen: si algo reenciende `OPOST`, llega `\r\n`, sale `\r\r\n`, y dos retornos dejan el cursor donde uno. Por eso apagarlo puede ser best-effort.

**Lo que cuesta preguntarlo, medido** con un programa en C sobre un pseudoterminal real (200 000 vueltas, tras calentar): **`tcgetattr` son 282 ns**. A 8 KiB por lectura y 170 MiB/s son ~21 800 llamadas por segundo, el **0,61 % de un núcleo** a pleno caudal; y solo se pregunta cuando el trozo trae algún salto, así que un volcado binario no paga ninguna.

**La ventana irreducible, y por qué no se cierra.** El núcleo traducía **al escribir**; emularlo desde fuera obliga a traducir **al leer**, y entre las dos cosas el programa puede haber cambiado los ajustes. Si apaga la señal justo después de escribir, el trozo que ya estaba en el búfer del núcleo se entrega con la regla nueva. Cota: los ~20 KiB del búfer, y solo en el instante de la transición — justo antes de que el programa repinte la pantalla entera.

> **La regla general que deja:** emular desde fuera algo que el núcleo hacía en el momento de escribir tiene una ventana del tamaño de su búfer. Se acota y se declara; cualquier remedio —recordar el estado anterior, retrasar la traducción— es un mecanismo sin escenario demostrable.

**La esquina que queda**: un programa que apague `OPOST` dejando `ONLCR` puesto (`cfmakeraw` solo toca el primero) recibiría traducción donde el núcleo no traducía. Ninguno de los probados lo hace, y por la regla de `ncurses` sería inofensivo.

**Y lo que acota el titular de la ganancia**: `less` reenciende `OPOST`, así que cualquier paginador —`git log`, `man`— no la ve mientras está abierto. Es para programas orientados a línea lanzados desde el shell, que es el caso del proyecto: un agente escupiendo salida. Ver [[el-pseudoterminal-medido]] y [[el-modo-lnm-no-repone-el-retorno]].

## Where it lives

`crates/term-core/src/lib.rs` — el tipo `ProcesadoDeSalida`: `quitarselo_al_nucleo` apaga `OPOST` sobre un duplicado del descriptor del maestro **antes** de lanzar al hijo (un shell lee los ajustes al arrancar, y `less` los fotografía para restaurarlos al salir), y `aplicar` traduce en el hilo lector consultando `lo_quiere_el_programa`. Las seis pruebas del final del fichero lo atraviesan con un `sh` de verdad. Escrito como **D12** en `docs/02-decisiones.md`.
