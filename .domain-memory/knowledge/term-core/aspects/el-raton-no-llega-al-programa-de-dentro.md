---
id: asp_el-raton-no-llega-al-programa-de-dentro_x550d9
slug: el-raton-no-llega-al-programa-de-dentro
name: >-
  El ratón YA llega al programa de dentro desde F2.6: la guarda que antes no
  protegía de nada ahora sí tiene escenario
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:06:38.805Z'
updated_at: '2026-08-25T09:22:30.769Z'
last_verified: '2026-08-25T09:22:30.769Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/raton.rs
  - crates/term-core/src/screen.rs
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:6ffd89344e0593f6d7362dccb0c76cc2ac211086a11fe9f50adb258313184450'
  crates/app/src/raton.rs: 'sha256:94147ca1504fb20af9e9cc49d03f7f0c7169613a9eccf1febb9ce98ec557c0a4'
  crates/term-core/src/screen.rs: 'sha256:6af1919efa54066bdf18665edd3cf6316c9ff4c01f3cfbd0b9a94a029dd24ef9'
tags:
  - raton
  - f2.6
  - d23
  - pruebas
feature_id: feat_term-core_nx0g1r
---

## What it does

> **Estado (25 de agosto de 2026): el hecho de esta ficha CAMBIÓ.** F2.6 implementó el reenvío, y esta entrada se conserva porque su aviso final se cumplió al pie de la letra: decía «cuando F2.6 implemente el reenvío del ratón, este camino hay que volver a mirarlo: entonces la guarda sí tendrá escenario». Lo tiene.

**Lo que era verdad hasta F2.6, y por qué se escribió.** Los acordes de teclado se filtran a propósito para que no lleguen al pseudoterminal —**D13**: «ninguno de esos acordes le llega al programa de dentro»— y de ahí salía una intuición que parecía obvia y era **falsa**: que un gesto de ratón necesitaba el mismo filtro. No lo necesitaba, porque **el ratón no llegaba**: en el crate `app` no existía ningún camino que mandara eventos de ratón al pseudoterminal.

> **La consecuencia era la que mordía: un criterio de aceptación del tipo «el programa de dentro no recibe el gesto» se cumplía POR CONSTRUCCIÓN, y su prueba pasaba sin atar nada.** El tercer disfraz fichado del proyecto, con el agravante de que ni siquiera había guarda que probar.

**Lo que es verdad desde F2.6 (D23).** Una consola cuyo programa **ha pedido** el ratón recibe sus gestos como bytes por el pseudoterminal. Y lo que antes era un mecanismo sin escenario ahora tiene **varios**, todos con su defecto real encontrado en revisión:

- **`Mayúsculas` devuelve el gesto a la selección** aunque el programa lo reclame — sin esa salida no habría forma de copiar dentro de `vim` o `htop`.
- **El acorde de mover la división gana siempre**: si un programa pudiera quedárselo, abrir un editor dejaría esa consola clavada donde está.
- **Un apretón sobre las columnas del panel no es del programa**, y cancelar solo ese fotograma no basta — es la versión de ratón del defecto que la selección ya pagó.

**La regla que queda, corregida:** antes de filtrar una entrada «para que no se le cuele al programa», comprobar que esa clase de entrada le llega hoy. **Teclado sí, ratón también desde F2.6.** Y al revés: una prueba que afirme «el programa NO recibió esto» ya no pasa por construcción, así que ahora sí vale — pero solo si la consola de la prueba tiene un programa que **haya pedido** el ratón; si no lo pidió, se vuelve a cumplir por construcción.

## Where it lives

El reparto de quién se queda cada gesto está en `crates/app/src/lib.rs`; la codificación del informe, en `crates/app/src/raton.rs`; qué ha pedido el programa, en `crates/term-core/src/screen.rs`. La decisión con el orden, la tabla y lo que queda fuera es **D23** en `docs/02-decisiones.md`.
