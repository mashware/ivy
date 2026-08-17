---
id: asp_el-escritorio-elige-el-tamano-y-la-tanda-no-es-reproducible_cgyn1y
slug: el-escritorio-elige-el-tamano-y-la-tanda-no-es-reproducible
name: >-
  El tamaño de la ventana lo elige el escritorio y nadie lo pide: tres corridas
  de la misma tanda dieron tres rejillas el mismo día
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T12:02:19.548Z'
updated_at: '2026-08-17T12:02:19.548Z'
last_verified: '2026-08-17T12:02:19.548Z'
file_paths:
  - crates/app/src/lib.rs
  - scripts/medir-fotograma.sh
symbols:
  - arrancar
  - 'Metrica::celdas_en'
content_hashes:
  crates/app/src/lib.rs: 'sha256:69e903da2ae25795351e9b8a05001efaad7c4703b59964f4e430a94a6176c041'
  scripts/medir-fotograma.sh: 'sha256:645272985d365203bb70a3d1df416d638ac62d8889b14458c96d8b137fd21be9'
tags:
  - medida
  - F2.2
  - riesgo
  - reproducibilidad
feature_id: feat_term-core_nx0g1r
---

## What it does

**Todas las cifras del anexo «Medido» de D3 dependen de un número que el proyecto no controla: la rejilla.** El coste de un fotograma es sobre todo **interpretar** y **encolar el pintado**, y las dos cosas son proporcionales a las celdas que hay. La rejilla sale del tamaño de la ventana, y **`arrancar()` abre con `eframe::NativeOptions::default()`, que nunca pide un tamaño**: lo elige el escritorio.

> **Medido el 17 de agosto de 2026, en la misma máquina y el mismo día: tres corridas de la tanda A dieron tres rejillas.** 61×57 de madrugada, **36×47** en una rama de trabajo y **40×66** en `main` medido cuatro minutos después en un árbol de trabajo aparte con su propio binario en release. La última es la que lo cierra: **no puede ser del código**, porque no tenía dentro ninguna línea del cambio que se estaba validando.

**Por qué importa, y no es un detalle de laboratorio:**

1. **Dos corridas de la misma tanda no son comparables entre sí**, y el guion de medida tiene precisamente esa comprobación —«las dos corridas de la misma tanda publican la misma rejilla»—, así que las **marcaría inválidas**. La guarda funciona; lo que falla es la premisa de que el tamaño se repite.
2. **Las tandas de días distintos tampoco lo son.** Comparar 5,78 ms con 7,01 ms solo dice algo si las dos midieron la misma cantidad de celdas. Es el mismo error que [[medir-algo-que-cambia-la-rejilla-no-es-una-resta]] señala desde otro ángulo: ahí la rejilla la cambiaba el panel, aquí la cambia el gestor de ventanas.
3. **La rejilla es la única cifra del informe que el guion comprueba de verdad**, así que una corrida con la ventana «equivocada» sale **idéntica en forma** a una limpia: una pestaña declarada, las dos divisiones iguales, el divisor al 50 %. Nada en el texto la delata. La comprobación estructural sigue valiendo —sirve para ver que la disposición no ha cambiado— pero **no autoriza a comparar latencias**.

> **La regla: un instrumento cuya escala la fija un tercero no es reproducible, aunque publique su número.** Antes de comparar dos tandas hay que comprobar que declararon la misma rejilla; y si se quieren tandas comparables, el tamaño hay que **pedirlo** — `NativeOptions` con un tamaño fijo, o fijarlo desde fuera y declararlo en el informe.

**Lo que hoy sí se puede afirmar con una tanda**: que la disposición reparte igual que antes (mismas divisiones, mismo divisor, mismo foco) y que la corrida no se marchó inválida. **Lo que no**: ninguna comparación de milisegundos con el anexo.

## Where it lives

`crates/app/src/lib.rs` — `arrancar()`, que llama a `eframe::run_native` con `NativeOptions::default()`; `scripts/medir-fotograma.sh` — la comprobación de que las dos corridas de una tanda publican la misma rejilla, y `comparar-tandas`. Descubierto validando el PR #74 de F2.2, y pendiente de resolver en el PR que toma las tandas del hito.
