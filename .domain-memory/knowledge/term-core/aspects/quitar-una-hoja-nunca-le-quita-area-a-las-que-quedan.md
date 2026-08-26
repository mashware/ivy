---
id: asp_quitar-una-hoja-nunca-le-quita-area-a-las-que-quedan_fy36sq
slug: quitar-una-hoja-nunca-le-quita-area-a-las-que-quedan
name: >-
  Quitar una hoja casi nunca le quita área a las que quedan: absorber les cuesta
  un divisor — y el suelo geométrico se comprueba igual contra el árbol de AHORA
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:07:02.388Z'
updated_at: '2026-08-18T13:58:47.482Z'
last_verified: '2026-08-18T13:58:47.482Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - Nodo
  - quitar
  - colapsar
  - absorber
  - repartir
  - cabe_partir
  - mover
content_hashes:
  crates/app/src/lib.rs: 'sha256:d7688143658df12a548b8eb3bd4e0e7fc0ce9966b7a5e14a6e7d7a8e8835ba9d'
tags:
  - arbol-de-disposicion
  - invariantes
  - geometria
feature_id: feat_term-core_nx0g1r
---

## What it does

**Quitar una hoja del árbol de disposición casi nunca le quita área a las que quedan.** El «casi» se midió en F2.3 y corrige lo que esta ficha afirmaba antes. Las tres operaciones que intervienen, y ninguna es evidente por separado:

- **`quitar`** saca un hijo de la lista de su rama **sin tocar los pesos de los demás**, así que la parte de cada hermana pasa de `wᵢ/Σ` a `wᵢ/(Σ−w)` — estrictamente **mayor**.
- **`colapsar`** sustituye por su única hija la rama que se queda con una, y esa hija se lleva el área **entera** de la rama.
- **`absorber`** sube las nietas a hermanas escalando sus pesos (`w · suyo / Σ`), y eso conserva **el reparto** pero **no la geometría exacta**.

> **La excepción, medida y no razonada: `absorber` le cuesta un divisor a TODA la rama, también a las que no participan.** Subir las hijas mete un divisor más en la rama de fuera, y los divisores se descuentan del área **antes** de repartir: el útil baja un grosor por cada hija que sube de más. Con un área de 1.200 puntos, una consola ajena al gesto que se llevaba la mitad **pasa de 599 a 598**. Atado por `absorber_conserva_la_geometria_de_las_consolas_que_no_se_mueven`, que por eso afirma **proporciones y no puntos** — ver [[una-invariante-geometrica-se-afirma-en-proporciones]].

> **Lo que esta ficha decía antes era la propia doc del código, y también era falsa**: «dándole a cada una `w · suyo / Σ` no se mueve nada en pantalla». Corregida en el código en F2.3 (commit `20872e6`). Es un caso de libro de [[lo-que-rompe-es-lo-que-la-operacion-nueva-vuelve-alcanzable]]: la afirmación llevaba dos hitos escrita y nadie había hecho pasar a `mover` por ese camino.

**Y la conclusión de la ficha SIGUE EN PIE, pero por otra vía.** Comprobar el suelo geométrico contra el árbol de ahora es coherente y suficiente: la formulación intuitiva —«validar contra el árbol posterior a quitar»— **es incoherente**, porque no se puede validar contra un árbol que todavía no existe. Y lo que el suelo protege es **la rama que se parte**, no a las demás: recibir a una consola sube en uno los hijos de la rama de destino, exactamente igual que partirla, que es justo lo que `cabe_partir` existe para comprobar. Que una consola ajena pierda un punto no lo viola, porque **el mínimo de 20 columnas es condición de la operación y no invariante permanente** — encoger la ventana ya lo rompe, y el suelo que `repartir` aplica en cada fotograma es el ancho de **una celda**, no `MINIMUM_CONSOLE`.

## Where it lives

`crates/app/src/lib.rs`: `Nodo::quitar`, `Nodo::colapsar`, `Nodo::absorber`, `Nodo::mover`, `repartir` y `cabe_partir`. Salido del diseño de F2.3 (issue 76), donde la primera redacción decía «validar contra el árbol posterior a quitar» y no significaba nada; la excepción del divisor salió al escribir la prueba del segundo PR de ese mismo hito, midiendo — no leyendo.
