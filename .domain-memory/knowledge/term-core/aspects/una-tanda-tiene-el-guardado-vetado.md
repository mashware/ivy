---
id: asp_una-tanda-tiene-el-guardado-vetado_hwwhag
slug: una-tanda-tiene-el-guardado-vetado
name: >-
  Una tanda de medida tiene el guardado de la disposición VETADO, así que sus
  escrituras son cero por construcción: las guardas del guion cuentan INTENTOS
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T22:26:53.883Z'
updated_at: '2026-08-24T22:26:53.883Z'
last_verified: '2026-08-24T22:26:53.883Z'
file_paths:
  - scripts/medir-fotograma.sh
  - crates/app/src/lib.rs
  - crates/app/src/sesion.rs
  - crates/app/src/medida.rs
symbols:
  - no_pisar_porque
  - 'Fallo::NoSePisa'
  - guardar
content_hashes:
  scripts/medir-fotograma.sh: 'sha256:5164b53754c528c121b0dc462bd6b6ded7dd92b97e504c321ea968fdd7d77d2f'
  crates/app/src/lib.rs: 'sha256:8f5d9220dadc886213f3207269ffaf9502f3454547843a94c9aa6e24dedbb2d5'
  crates/app/src/sesion.rs: 'sha256:e830c75bff3ba304a58835e5647deeb2ef55331849d5092066f7d021d49030c6'
  crates/app/src/medida.rs: 'sha256:b25ea9e9128a5e7699f1b9ac89d4501e7304d44fc5a3c7635d3284cb64e7bc1d'
tags:
  - medida
  - guardas
  - d16
feature_id: feat_term-core_nx0g1r
---

## What it does

**Dos mecanismos del proyecto que se contradicen, y ninguno de los dos es incorrecto por su cuenta.**

1. **La aplicación veta el guardado en cuanto ve una variable de la puerta de medida** (`MEDIR_PESTANAS`, `MEDIR_DIVISIONES`, `MEDIR_TAMANO`, `MEDIR_ARRASTRES`, `MEDIR_GESTO`). La razón es buena y está escrita: una tanda no puede machacarle la disposición a quien mide, ni abrir con las consolas que él dejó en vez de con las que la tanda pide. El veto se amplió a **todas** las variables al detectar que la tanda que restaura era la única sesión de medida con la escritura viva — dos regímenes distintos comparándose fila por fila en un anexo.
2. **El guion exigía al menos una escritura** en toda tanda con gestos, con el razonamiento habitual del proyecto: si nadie escribió, el coste de escribir se publica en cero y se lee como «guardar no cuesta nada» en vez de como «no se guardó».

> **Juntos, la guarda es imposible de cumplir: las escrituras de una tanda son cero POR CONSTRUCCIÓN.** Toda tanda con gestos quedaba invalidada. No saltó durante meses porque **entre que el veto se amplió y la siguiente tanda con gestos pasaron semanas** — y las corridas guardadas en `target/medidas/` eran anteriores, así que a simple vista la guarda «funcionaba».

**El arreglo, y por qué no es rebajar el listón.** La guarda pasa a contar **intentos**. El trabajo que hay que declarar **sí ocurre y sí se cronometra** — copiar la disposición, validarla, serializarla y llegar hasta la puerta —; lo único que no ocurre es el toque al disco. Lo que la guarda vigila es que **ese camino se recorra**, que es lo que da sentido al tramo publicado.

> **La regla general: cuando un mecanismo de seguridad de la aplicación apaga un efecto, toda guarda que comprobaba ESE efecto pasa a ser incumplible — y el síntoma no aparece hasta la primera corrida que la ejercita.** Al añadir un veto, hay que ir a mirar quién comprobaba lo vetado. El comentario del código que amplió el veto **mencionaba la guarda del guion** y aun así no se ajustó: leer que existe no es lo mismo que comprobar si sigue siendo satisfacible.

**Nota sobre las guardas de las tandas en general**, que es donde este caso encaja: una guarda que **nunca ha fallado** no está probada. La guarda de recolocaciones de la tanda `M` se probó bajando el recorrido del gesto hasta que no cruzara ningún borde de celda: la corrida se inyecta entera —312 gestos, 936 efectos— y **se invalida**. Sin ese ensayo la guarda anterior, que solo miraba que la línea existiera, habría dado la corrida por buena — el arranque ya recoloca dos o tres veces por su cuenta.

## Where it lives

El veto se pone en `arrancar()` de `crates/app/src/lib.rs`, con `no_pisar_porque`, y lo aplica `Sesion::guardar` en `crates/app/src/sesion.rs` devolviendo `Fallo::NoSePisa`. La guarda vive en `scripts/medir-fotograma.sh`, en el bloque que se ejecuta cuando `ARRASTRES` no es cero. La línea que el guion parsea la publica `crates/app/src/medida.rs` como «escribir la disposición en disco (N escrituras de M intentos)».
