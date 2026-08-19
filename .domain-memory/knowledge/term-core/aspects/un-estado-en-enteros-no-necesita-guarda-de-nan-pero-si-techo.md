---
id: asp_un-estado-en-enteros-no-necesita-guarda-de-nan-pero-si-techo_6m3eyt
slug: un-estado-en-enteros-no-necesita-guarda-de-nan-pero-si-techo
name: >-
  La guarda la pide el TIPO en que se guarda, el techo lo pide lo que se puede
  HONRAR, y el momento de aplicarlo decide si el estado recuerda lo que el
  usuario pidió
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-19T15:30:48.346Z'
updated_at: '2026-08-19T15:30:48.346Z'
last_verified: '2026-08-19T15:30:48.346Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pintor.rs
symbols:
  - arrastrar_la_frontera_de_su_panel
  - arrastrar_la_frontera
  - celdas_en
content_hashes:
  crates/app/src/lib.rs: 'sha256:67dfdfc9bad7c0d618f018577eecd1de410625cc5cf9347f593360fff275852e'
  crates/app/src/pintor.rs: 'sha256:e02cf632e6186cee7d25e81d2f352b8d74ef3e631e3ce675540369378071905a'
tags:
  - egui
  - arrastre
  - estado
  - rust
feature_id: feat_term-core_nx0g1r
---

## What it does

**Tres cosas sobre guardar el resultado de un gesto, y las tres se aprendieron equivocándose en el mismo método.**

**1. La guarda contra el valor sin número la pide el TIPO, no el origen del número.** Comprobado **ejecutando** con `rustc -O`, no leyendo documentación:

| Expresión | Resultado |
|---|---|
| `f32::NAN as usize` | **0** |
| `(f32::NAN / 8.0) as usize` | **0** |
| `-5.0f32 as usize` | **0** |
| `1.0e9f32 as usize` | **1000000000** |

Las conversiones de coma flotante a entero en Rust son **saturantes**. Así que un estado guardado como entero no necesita la guarda que sí necesita uno guardado como fracción: el arrastre de las fronteras del árbol lleva `if !nueva.is_finite() { return false; }` porque **un `clamp` sobre un `NaN` devuelve `NaN`**, y guardado ahí no se va nunca — la ventana deja de pintar y ni ensancharla la recupera. Copiar esa guarda a un camino que guarda enteros mete un mecanismo sin escenario.

**2. Pero el techo sí hace falta, y NO por lo que parece.** El caso real de este proyecto no es el valor sin número ni el negativo: es que **mientras el escritorio no ha dicho cuánto mide la ventana, egui responde con su rectángulo de reserva de diez mil puntos**, que en celdas son **más de mil** y **pasa entero** por la conversión. No hay saturación que lo salve. Lo tapa `Metrica::celdas_en`, que ya trae su propio tope con ese razonamiento escrito — así que el número de celdas se le pide a ella y no se calcula a mano.

**3. Y el techo tiene que ser lo que se puede HONRAR, no lo que mide el sitio.** Con el techo puesto en «el ancho de la división», arrastrar a tope guardaba un ancho que el reparto nunca iba a dar —porque antes está el suelo de la consola— y **aparecía de golpe al ensanchar la ventana**: un ancho que el usuario nunca vio. Durante un gesto, lo que el usuario pide es lo que ve.

> **Y el momento de aplicarlo es la mitad de la decisión: el techo se aplica AL ESCRIBIR, nunca al leer.** Sin gesto no se pasa por el recorte, así que un ancho grande guardado con la ventana ancha **sobrevive** a una estrecha: se pinta recortado y vuelve entero al ensanchar. Aplicarlo también al leer destruiría la intención del usuario cada vez que la ventana se hace pequeña.

**Y una trampa de prueba que va con esto:** afirmar sobre la **geometría** no ata el techo, porque el reparto ya recorta lo que pinta. El techo protege el **estado**, así que su prueba tiene que afirmar el valor guardado — y el síntoma de que falta no se ve pintando, se ve **al redimensionar**, que es cuando ya nadie lo relaciona con el gesto.

## Where it lives

`Division::arrastrar_la_frontera_de_su_panel` (`crates/app/src/lib.rs`) para las tres reglas; el molde con la guarda que aquí no aplica es `Ventana::arrastrar_la_frontera` en el mismo fichero; el tope de celdas vive en `Metrica::celdas_en` (`crates/app/src/pintor.rs`), que documenta por qué existe. La tabla de conversiones se obtuvo compilando y ejecutando un programa suelto, no leyendo la referencia del lenguaje.
