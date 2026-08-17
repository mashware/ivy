---
id: asp_el-escritorio-elige-el-tamano-y-la-tanda-no-es-reproducible_cgyn1y
slug: el-escritorio-elige-el-tamano-y-la-tanda-no-es-reproducible
name: >-
  El tamaño de la ventana hay que pedirlo Y insistir: el compositor manda el
  suyo en el tercer fotograma — RESUELTO en F2.2
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T12:02:19.548Z'
updated_at: '2026-08-17T16:07:58.498Z'
last_verified: '2026-08-17T16:07:58.498Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/medida.rs
  - scripts/medir-fotograma.sh
symbols:
  - arrancar
  - tamaño_de_ventana
  - TamañoPedido
  - insistir_con_el_tamaño
  - MARGEN_DEL_TAMAÑO
content_hashes:
  crates/app/src/lib.rs: 'sha256:5303a764d73851d1199719a27ca85c6efac8142650b896676e8be63ee15d187e'
  crates/app/src/medida.rs: 'sha256:c66a2a454bba2d7e6e289ede1b8a1d6e060fcdf40111022e0d4b234291d66daa'
  scripts/medir-fotograma.sh: 'sha256:3827b1223e7110cde05841466d25a38ccbc6d8fe0ceace88acd29fabf84c4e5f'
tags:
  - medida
  - F2.2
  - reproducibilidad
  - resuelto
  - wayland
feature_id: feat_term-core_nx0g1r
---

## What it does

> **Estado (17 de agosto de 2026): RESUELTO** en el PR #75 de F2.2. Se conserva entero porque la causa no era la que parecía y el mecanismo del arreglo **no es evidente**: pedir el tamaño una vez no basta.

**Todas las cifras del anexo «Medido» de D3 dependen de un número que el proyecto no controlaba: la rejilla.** El coste de un fotograma es sobre todo **interpretar** y **encolar el pintado**, y las dos cosas son proporcionales a las celdas que hay. La rejilla sale del tamaño de la ventana, y `arrancar()` abría con `eframe::NativeOptions::default()`, que **nunca pide un tamaño**: lo elegía el escritorio.

> **El síntoma, medido el 17 de agosto de 2026 en la misma máquina y el mismo día: tres corridas de la tanda A dieron tres rejillas.** 61×57 de madrugada, **36×47** en una rama de trabajo y **40×66** en `main` medido cuatro minutos después en un árbol de trabajo aparte con su propio binario en release. La última es la que lo cierra: **no puede ser del código**, porque no tenía dentro ninguna línea del cambio que se estaba validando.

### La causa, sondeada fotograma a fotograma

**Pedir el tamaño al CONSTRUIR la ventana sí se aplica — y dura dos fotogramas.** Con `MEDIR_TAMANO=800x600` sobre GNOME/Wayland:

| Fotograma | Tamaño de la ventana |
|---|---|
| 0 y 1 | **800×600** — lo pedido, aplicado |
| 2 | **1126×661** — el compositor manda su propia configuración |
| 3 y siguientes | **800×600** — tras volver a pedirlo con `ViewportCommand::InnerSize` |

De ahí salen los dos números del síntoma: **36×47 era una ventana de 800×600** (el tamaño por defecto de winit) y **40×66 la del compositor**. No había nada aleatorio; había dos tamaños distintos según quién ganara la última palabra.

**Volver a pedirlo después SÍ se respeta**, y se queda. Eso es todo el arreglo: `TamañoPedido` guarda lo pedido e `insistir_con_el_tamaño` lo vuelve a pedir en cada fotograma en que la ventana no lo tenga.

### Tres cosas del arreglo que no son evidentes

1. **La insistencia se acaba, y el tope cuenta FOTOGRAMAS, no respuestas.** Un redimensionado en Wayland necesita una vuelta de configuración del compositor, o sea al menos un fotograma, y en el arranque los fotogramas van seguidos: con un tope de cuatro, un compositor que tarde cinco en contestar —una máquina más lenta, una sesión más ocupada— **agotaría los intentos antes de la primera respuesta** y la ventana no volvería a pedir nada, tirando todas las tandas de esa máquina. Son **32**. Y el tope tiene que existir: sin él, un compositor que no ceda convierte esto en una pelea por fotograma, y cada intento pide otro fotograma — la medida se mediría a sí misma peleándose.
2. **Los fotogramas que ya tienen el tamaño bueno no gastan intento.** Los dos primeros ya lo tienen, y gastarlos ahí deja a la ventana sin con qué responder cuando llegue la configuración del compositor.
3. **El compositor entrega el tamaño con fracción y NO siempre la misma**: pedidos los mismos 1280×800, dos corridas dieron 1280×800 y una tercera **1279,8125×799,8125**. Así que todas las comparaciones —la de la aplicación para no pelearse, y las dos del guion de medida— llevan **medio punto de margen**, con `<=` en los dos sitios. Una comparación exacta tira corridas idénticas, y como la fracción no es determinista, volver a tomarlas puede fallar otra vez.

### Y el informe publica el tamaño que TUVO

Es la **única línea de la cabecera que se puede volver a pedir**: la rejilla en celdas sale del tamaño **y** de la métrica de la fuente, así que leerla no dice con qué ventana se tomó la tanda. Los anexos anteriores a esto se midieron con ventanas cuyo tamaño nadie apuntó, y por eso **no se pueden reproducir ni comparar con los de después**.

> **La regla, que sigue en pie aunque el caso esté cerrado: un instrumento cuya escala la fija un tercero no es reproducible, aunque publique su número.** Y la variante que este caso añade: **pedirle algo al escritorio no es lo mismo que tenerlo** — hay que comprobar que se cumplió, y publicar lo que hubo.

**Dato de campo que vale conservar:** si se pide más de lo que cabe, este escritorio concede hasta **5120×1357 puntos** — comprobado pidiendo 4000×3000 y viendo qué entrega.

## Where it lives

`crates/app/src/lib.rs` — `VARIABLE_DEL_TAMAÑO` (`MEDIR_TAMANO`), `tamaño_de_ventana`, `TamañoPedido`, `INSISTENCIAS_CON_EL_TAMAÑO`, `MARGEN_DEL_TAMAÑO` e `insistir_con_el_tamaño`; `crates/app/src/medida.rs` — la línea `ventana:` de la cabecera; `scripts/medir-fotograma.sh` — el tamaño de referencia, la guarda por corrida y el modo `comparar`. Descubierto validando el PR #74 y resuelto en el #75, los dos de F2.2.
