---
id: asp_el-panel-cede-y-su-ancho-va-en-celdas_k5w9y5
slug: el-panel-cede-y-su-ancho-va-en-celdas
name: >-
  El ancho del panel se fija en celdas y no en puntos, y en una ventana estrecha
  el que cede es el panel — comprobado con cuatro divisiones
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T15:18:46.860Z'
updated_at: '2026-08-17T16:10:27.076Z'
last_verified: '2026-08-17T16:10:27.076Z'
file_paths:
  - crates/app/src/panel.rs
  - docs/02-decisiones.md
symbols:
  - ANCHO
  - CONSOLA_MINIMA
  - apartar
  - Hueco
content_hashes:
  crates/app/src/panel.rs: 'sha256:6e305e69f27f7fd80801f34a64fa332f571bc0b1924c7d85017fc37faedcd0a1'
  docs/02-decisiones.md: 'sha256:e931de7f9b29b21429b9a259db1cd9648470337c6e11d13deba391055a972ea5'
tags:
  - producto
  - reparto
  - egui
  - medido
  - F2.2
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Dos decisiones de producto sobre el reparto del ancho, las dos con su razón.**

**1. Fijo, pero en celdas.** La decisión del usuario fue «ancho fijo en puntos, sin plegar», y se cumple en lo que quería decir —no depende del ancho de la ventana y no se puede arrastrar— pero **se cuenta en celdas de la fuente y no en puntos**. Un ancho en puntos se estrecha en columnas al subir el aumento del escritorio, y lo que el panel tiene que conservar es **cuántos caracteres caben**, que es lo que decide si su texto se lee. Un panel que en una pantalla enseña las líneas enteras y en otra las corta no es el mismo panel.

> Ojo con probarlo: con una celda de 8 puntos, «36 celdas» y «288 puntos» dan el mismo número. Una prueba escrita con ese ancho no distingue las dos implementaciones — hay que variar el tamaño de la celda para que la diferencia aparezca.

**2. El panel cede, la consola no.** Cuando no cabe todo, el panel se estrecha hasta desaparecer y la consola conserva un suelo de columnas. **El panel es margen y la consola es el trabajo**: en una ventana estrecha lo que sobra es el estado, no el programa. Con la regla al revés, arrastrar el divisor dejaría a un programa interactivo sin sitio para escribir mientras su panel conserva sus treinta y seis columnas.

**Y el descuento va antes de medir la rejilla**, no después. Midiendo primero el rectángulo entero, el programa de dentro creería tener columnas que el panel le ha quitado y escribiría debajo de él. Es el criterio central del PR que hizo visible el panel.

**El predicado del reparto es «hay algo publicado», no «hay directorio».** Toda consola de la aplicación nace con su directorio y casi ninguna tiene fichero, así que repartir por tener directorio le quitaría el ancho a todas desde el primer fotograma — y es un fallo que una prueba montada sin directorio no ve.

### Medido con cuatro divisiones (17 de agosto de 2026) — y cierra el issue #33

La regla «el panel cede, la consola no» **aguanta el peor caso sin tocar nada**, y la pregunta abierta del [#33](https://github.com/mashware/agent-terminal/issues/33) —«con cuatro divisiones el panel se comería la consola entera»— queda respondida con números. Ventana de 1280×800 puntos, panel publicado en las cuatro:

| Disposición | Rejilla de cada consola | Qué pasa con el panel |
|---|---|---|
| Dos columnas | 49×39 | entra entero (36 columnas) |
| **Cuatro en fila** | **49×20** | a cada división le tocan unas 37 columnas, el panel no cabe y **cede hasta 17** para dejarle a la consola su suelo de **20** — exactamente `CONSOLA_MINIMA` |
| **Cuatro en cuadro** | **24×39** | **no cede nada**: lo que se parte por la mitad es el ALTO, no el ancho |

**Así que la disposición que alguien usaría de verdad —dos columnas de dos— no le quita ni una columna a la consola**, y la de cuatro en fila la deja en su suelo en vez de comerse el sitio. Nada que adelantar de F2.4.

Y de paso, lo que el mismo anexo prueba del fotograma: **el coste del framework de interfaz no empeora al duplicar las divisiones** —402 a 530 µs de «resto del fotograma» en seis corridas, con la variación dentro de una tanda del mismo orden que entre tandas—, y lo que sí crece es la **espera en la cola** (46 ms con dos consolas volcando, 88 y 91 con cuatro), porque el presupuesto de 128 KiB es **por división**. Los números enteros, en el anexo «Medido» de **D14**.

## Where it lives

`crates/app/src/panel.rs`: las dos constantes y la función que reparte, con su tipo `Hueco`. La geometría vive en el módulo del panel y no en el de la ventana, que es quien la llama. La comprobación con cuatro divisiones, en el anexo «Medido» de D14 (`docs/02-decisiones.md`) y en las tandas `H` e `I` de `scripts/medir-fotograma.sh`.
