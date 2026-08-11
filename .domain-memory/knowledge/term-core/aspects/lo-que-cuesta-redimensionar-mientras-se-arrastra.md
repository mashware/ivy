---
id: asp_lo-que-cuesta-redimensionar-mientras-se-arrastra_04ss52
slug: lo-que-cuesta-redimensionar-mientras-se-arrastra
name: >-
  Lo que cuesta redimensionar mientras se arrastra: el reflujo de la rejilla es
  barato, y lo que crece con las columnas es el repintado del programa de dentro
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-11T19:52:18.687Z'
updated_at: '2026-08-11T19:52:18.687Z'
last_verified: '2026-08-11T19:52:18.687Z'
file_paths:
  - crates/term-core/src/screen.rs
  - crates/term-core/src/lib.rs
  - crates/app/src/lib.rs
symbols:
  - 'Screen::resize'
  - 'Pty::resize'
  - 'Division::redimensionar'
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:b87919af4bad5a80f53f83baa106bce231d0ab3d35d6284beaf4be188e766002'
  crates/term-core/src/lib.rs: 'sha256:efa66ec48f1e9c4ba5f5d05657492ca563960b543264d613c06cd094b0818c73'
  crates/app/src/lib.rs: 'sha256:c7f2d67be30a71cbe392f9c446013656f0c3443fb6f7d7035e8575837d3f7a1c'
tags:
  - rendimiento
  - medido
  - f1.4
  - f2.2
  - d3
  - divisiones
feature_id: feat_term-core_nx0g1r
---

## What it does

**Medido en `--release`, no estimado**, con una rejilla de 61×57 y contenido de verdad —color, texto y líneas envueltas— para que el reflujo tenga algo que mover:

| | Coste |
|---|---|
| `Screen::resize`, ±1 columna | **11,2 µs** |
| `Screen::resize`, ±20 columnas | 11,9 µs |
| `Pty::resize` (el `ioctl` `TIOCSWINSZ`) | **289 ns** |

**El reflujo hace todo lo que uno teme, y aun así no importa.** `Screen::resize` llama a `Term::resize`, que refluye **las dos rejillas** —la activa y la alterna— y cuyo `grow_columns`/`shrink_columns` reserva un vector de filas nuevo, hace `take_all()` de la rejilla y reasigna fila a fila. Es proporcional a las celdas **y** reserva memoria. Pero a este tamaño son 11 µs, y **el salto de veinte columnas cuesta lo mismo que el de una**: lo que domina es recorrer las filas, no cuánto se mueve.

**El techo real por fotograma es dos reflujos y dos `ioctl` = 23 µs**, o sea el **0,28 % del hueco de 120 Hz** y el 0,39 % de los 5,92 ms que ya se gastan con las dos consolas volcando. Un arrastre que cruce la ventana en un segundo cruza unas 114 fronteras de columna por consola —con la fuente a 14 puntos la celda mide ~8,4— así que son ~1,3 ms de reflujo repartidos en 120 fotogramas. **No hay nada que cachear aquí**, y una caché sería un mecanismo sin escenario que lo justifique.

Lo que acota el número de reflujos no es el ratón, es la guarda de `Division::redimensionar`: se llama una vez por consola y por fotograma, y sale sin hacer nada si la rejilla ya mide lo pedido. Un arrastre **sub-celda** —tres puntos que no cambian el número de columnas— no dispara ninguna de las dos mitades, y eso es correcto: el pseudoterminal y la rejilla se miden en celdas, así que ambas llamadas serían un no-op.

## El término que SÍ crece con las columnas, y no es código propio

Cada `Pty::resize` manda **`SIGWINCH`**, y el programa de dentro responde **repintando su pantalla entera**. Esos bytes vuelven por el parser, que ya es 3,35 ms del fotograma. Un repintado de 61×57 desde `vim` son unos pocos KiB → **~50-90 µs** con los 121 µs por 8 KiB ya medidos; como mucho uno por fotograma, alrededor del **1 % del hueco**, y el presupuesto de 128 KiB lo absorbe sin cortar.

**Consecuencia para F2.2, y es el motivo de escribir esto:** cuando haya que volver a medir con cuatro divisiones, lo que hay que mirar es **este** término, no el reflujo. El reflujo escala con las filas de una consola y las consolas se hacen más pequeñas al haber más; el repintado tras el aviso de tamaño escala con **cuántos programas hay que avisar**, y un arrastre que mueva un divisor entre cuatro puede avisar a dos a la vez.

## Un aviso de error que parecía poder ser por fotograma, y no puede

Cuando `Pty::resize` falla se imprime un aviso por la salida de error. La pregunta razonable es si un arrastre puede convertir eso en una escritura de E/S por fotograma. **No**, y la razón es el orden de las dos mitades: la rejilla se redimensiona **antes** que el pseudoterminal, así que tras un fallo la rejilla ya mide lo pedido y el fotograma siguiente sale por la guarda sin volver a intentarlo. El techo son los cambios de tamaño **distintos**, no los fotogramas. Y el disparador es un `ioctl` que en Linux no falla sobre un maestro abierto, ni aunque el hijo haya muerto.

## Un desfase de un fotograma que es gratis, para que nadie lo lea como fallo

El tamaño se **mide** en la pasada de interfaz y se **aplica** en la mitad lógica del fotograma siguiente. Así que durante un arrastre cada consola pinta un fotograma con la rejilla vieja dentro del rectángulo nuevo. El divisor sí va al día. A 120 Hz no se ve, y arreglarlo exigiría medir y aplicar en la misma mitad, que es justo el reparto que la separación entre las dos mitades del fotograma existe para evitar.

**Cuándo dejarían de valer estos números**: en otra máquina, con otra versión del motor de VT, o con rejillas de otro orden de magnitud. El banco era temporal y se borró; rehacerlo cuesta unos minutos y lo que mide es `Screen::resize` sobre una rejilla con contenido, nunca sobre una vacía — una rejilla vacía no tiene nada que refluir y daría un número que miente, el mismo error que ya costó dos veces al alimentar el parser con un solo carácter repetido.

## Where it lives

El reflujo es `Screen::resize` en `crates/term-core/src/screen.rs`, que delega en `Term::resize` de alacritty_terminal. El aviso al programa es `Pty::resize` en `crates/term-core/src/lib.rs`. Las dos las encadena `Division::redimensionar` en `crates/app/src/lib.rs`, con su guarda de «si ya mide lo pedido, no hagas nada» y con el orden —rejilla primero, programa después— que este aspecto explica. Lo llama `Ventana::bombear_todas`, una vez por consola y por fotograma.
