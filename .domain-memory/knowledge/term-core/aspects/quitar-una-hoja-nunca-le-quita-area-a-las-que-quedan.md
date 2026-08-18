---
id: asp_quitar-una-hoja-nunca-le-quita-area-a-las-que-quedan_fy36sq
slug: quitar-una-hoja-nunca-le-quita-area-a-las-que-quedan
name: >-
  Quitar una hoja nunca le quita área a las que quedan, y por eso el suelo
  geométrico se comprueba contra el árbol de AHORA
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:07:02.388Z'
updated_at: '2026-08-17T20:07:02.388Z'
last_verified: '2026-08-17T20:07:02.388Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - Nodo
  - quitar
  - colapsar
  - absorber
  - repartir
  - cabe_partir
content_hashes:
  crates/app/src/lib.rs: 'sha256:bd1d056ad4bcd72e1edfbea41ecd1ed51b925068d136aabb02b79da1032b45e3'
tags:
  - arbol-de-disposicion
  - invariantes
  - geometria
feature_id: feat_term-core_nx0g1r
---

## What it does

**Quitar una hoja del árbol de disposición NUNCA le quita área a ninguna de las que quedan.** Se demuestra con las tres operaciones que intervienen, y ninguna es evidente por separado:

- **`quitar`** saca un hijo de la lista de su rama **sin tocar los pesos de los demás**, así que la parte de cada hermana pasa de `wᵢ/Σ` a `wᵢ/(Σ−w)` — estrictamente **mayor**.
- **`colapsar`** sustituye por su única hija la rama que se queda con una, y esa hija se lleva el área **entera** de la rama.
- **`absorber`** sube las nietas a hermanas escalando sus pesos (`w · suyo / Σ`) y **conserva la geometría exactamente**: su propio comentario dice que así «no se mueve nada en pantalla, ni el bloque respecto a sus tías, ni las hijas entre ellas».

> **Para qué sirve saberlo: hace CONSERVADOR comprobar el suelo geométrico contra el árbol de ahora**, que es la única forma coherente de validarlo. La formulación intuitiva —«hay que validar contra el árbol posterior a quitar»— **es incoherente**: no se puede validar contra un árbol que todavía no existe. Con esta propiedad no hace falta: si el destino cabe **antes** de quitar, cabe **después**.

**Y el suelo sigue haciendo falta por otra razón, que también se confunde:** no es «para que no se vea mal» —de la geometría ya se encarga `repartir` en cada fotograma con su propio suelo, como dice el comentario de `arrastrar_la_frontera`—, sino porque **mover una hoja aumenta en uno el número de hijos de la rama de destino, exactamente igual que partir**, que es justo lo que `cabe_partir` existe para comprobar.

## Where it lives

`crates/app/src/lib.rs`: `Nodo::quitar`, `Nodo::colapsar`, `Nodo::absorber`, `repartir` y `cabe_partir`. Salido del diseño de F2.3 (issue 76), donde la primera redacción decía «validar contra el árbol posterior a quitar» y no significaba nada; la demostración es aritmética sobre el reparto por pesos, no una observación de pantalla.
