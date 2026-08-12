---
id: asp_el-panel-cede-y-su-ancho-va-en-celdas_k5w9y5
slug: el-panel-cede-y-su-ancho-va-en-celdas
name: >-
  El ancho del panel se fija en celdas y no en puntos, y en una ventana estrecha
  el que cede es el panel
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T15:18:46.860Z'
updated_at: '2026-08-12T15:18:46.860Z'
last_verified: '2026-08-12T15:18:46.860Z'
file_paths:
  - crates/app/src/panel.rs
symbols:
  - ANCHO
  - CONSOLA_MINIMA
  - apartar
  - Hueco
content_hashes:
  crates/app/src/panel.rs: 'sha256:b229abf065c777de9460a87101cb6343d6f7e56eddac128b0f4f261f5d440247'
tags:
  - producto
  - reparto
  - egui
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Dos decisiones de producto sobre el reparto del ancho, las dos con su razón.**

**1. Fijo, pero en celdas.** La decisión del usuario fue «ancho fijo en puntos, sin plegar», y se cumple en lo que quería decir —no depende del ancho de la ventana y no se puede arrastrar— pero **se cuenta en celdas de la fuente y no en puntos**. Un ancho en puntos se estrecha en columnas al subir el aumento del escritorio, y lo que el panel tiene que conservar es **cuántos caracteres caben**, que es lo que decide si su texto se lee. Un panel que en una pantalla enseña las líneas enteras y en otra las corta no es el mismo panel.

> Ojo con probarlo: con una celda de 8 puntos, «36 celdas» y «288 puntos» dan el mismo número. Una prueba escrita con ese ancho no distingue las dos implementaciones — hay que variar el tamaño de la celda para que la diferencia aparezca.

**2. El panel cede, la consola no.** Cuando no cabe todo, el panel se estrecha hasta desaparecer y la consola conserva un suelo de columnas. **El panel es margen y la consola es el trabajo**: en una ventana estrecha lo que sobra es el estado, no el programa. Con la regla al revés, arrastrar el divisor dejaría a un programa interactivo sin sitio para escribir mientras su panel conserva sus treinta y seis columnas.

**Y el descuento va antes de medir la rejilla**, no después. Midiendo primero el rectángulo entero, el programa de dentro creería tener columnas que el panel le ha quitado y escribiría debajo de él. Es el criterio central del PR que hizo visible el panel.

**El predicado del reparto es «hay algo publicado», no «hay directorio».** Toda consola de la aplicación nace con su directorio y casi ninguna tiene fichero, así que repartir por tener directorio le quitaría el ancho a todas desde el primer fotograma — y es un fallo que una prueba montada sin directorio no ve.

## Where it lives

`crates/app/src/panel.rs`: las dos constantes y la función que reparte, con su tipo `Hueco`. La geometría vive en el módulo del panel y no en el de la ventana, que es quien la llama.
