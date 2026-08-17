---
id: asp_colapsar-una-rama-puede-devolver-el-anidamiento-de-tmux_s1bts4
slug: colapsar-una-rama-puede-devolver-el-anidamiento-de-tmux
name: >-
  Colapsar la rama que se queda con una hija no basta: puede anidar dos ramas
  del mismo sentido y devolver la frontera que mueve un bloque de dos consolas
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T12:01:46.999Z'
updated_at: '2026-08-17T12:01:46.999Z'
last_verified: '2026-08-17T12:01:46.999Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - 'Nodo::colapsar'
  - 'Nodo::absorber'
  - 'Nodo::partir'
  - 'Nodo::cerrar'
content_hashes:
  crates/app/src/lib.rs: 'sha256:69e903da2ae25795351e9b8a05001efaad7c4703b59964f4e430a94a6176c041'
tags:
  - árbol-de-divisiones
  - F2.2
  - diseño
  - medido
feature_id: feat_term-core_nx0g1r
---

## What it does

**La razón de ser del árbol n-ario es que cada frontera mueva solo a sus dos vecinas** — con un árbol binario, tres consolas en fila se anidan y la frontera de fuera mueve **un bloque de dos**, que es el comportamiento de `tmux` del que más se queja quien lo usa. El diseño lo consigue con una regla: **partir en el mismo sentido que la rama que ya contiene la hoja inserta una hermana**, no una rama nueva.

**Y cerrar lo deshace por la puerta de atrás.** La regla de cerrar dice que una rama que se queda con **una sola hija** se sustituye por ella. Pero esa hija **puede ser una rama del mismo sentido que su nueva madre**, y entonces queda exactamente el anidamiento que partir tiene prohibido crear.

**Se llega en tres gestos, todos de teclado:** partir por abajo, partir al lado, cerrar la de arriba. La rama de filas se queda con una sola hija —la de columnas— y se sustituye por ella dentro de la rama de columnas raíz.

> **Medido, no razonado:** arrastrando la frontera de fuera, una consola que esa frontera **no debería tocar** pasa de **335,6 a 504,0 puntos** — crece un 50 %. Comprobado con una prueba temporal antes de decidir nada.

**El arreglo es aplanar, y los pesos se escalan.** Al colapsar, si la hija que sobrevive reparte en el mismo sentido que la rama de fuera, **sus hijas suben a ser hermanas** de las de arriba, con `peso del grupo · el suyo / la suma de los suyos` y **en el sitio que ocupaba el grupo**. Así no se mueve nada en pantalla: ni el bloque respecto a sus tías, ni las hijas entre ellas. Copiar los pesos tal cual sí las movería, y en una dirección que depende de cuántas hermanas hubiera.

> **La lección general: una invariante que una operación mantiene por construcción no la mantiene otra.** «Partir no puede producir esta forma» era cierto y se dio por suficiente; la forma la produce **cerrar**, así que le toca a cerrar deshacerla. Cuando dos operaciones comparten una estructura, cada una tiene que responder por la invariante **entera**, no solo por lo que ella crea.

**Y no se ve en pantalla, que es lo que lo hace peligroso.** Una rama de una hija le da su área entera y el reparto no le pone divisor: la disposición **se pinta igual** con el anidamiento y sin él. Lo único que cambia es lo que pasa **después** — al arrastrar y al partir. Por eso la prueba tiene que afirmar sobre el **efecto** (arrastrar y mirar quién se mueve) y no sobre la forma del árbol, que es donde se apunta el defecto.

Criterio de aceptación AC18 de F2.2, escrito en el diseño **durante** el PR que lo encontró.

## Where it lives

`crates/app/src/lib.rs` — `Nodo::colapsar` (la rama de una hija se sustituye por ella), `Nodo::absorber` (las hijas del grupo suben con el peso escalado), llamada desde `Nodo::quitar` tras el descenso; y `Nodo::partir`, que es la operación que mantiene la invariante por el otro lado. F2.2, PR #74.
