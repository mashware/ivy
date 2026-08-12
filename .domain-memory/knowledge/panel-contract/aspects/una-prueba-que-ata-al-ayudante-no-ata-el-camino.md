---
id: asp_una-prueba-que-ata-al-ayudante-no-ata-el-camino_hts3p9
slug: una-prueba-que-ata-al-ayudante-no-ata-el-camino
name: >-
  Una prueba que ata al ayudante no ata el camino real: tres veces en un solo
  hito, y las tres con la suite en verde
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:06:42.827Z'
updated_at: '2026-08-12T15:17:56.900Z'
last_verified: '2026-08-12T15:17:56.900Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/app/src/panel.rs
  - crates/app/src/lib.rs
symbols:
  - read
  - leer_acotado
  - pintar
  - color_del_panel
  - pintar_division
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:024c1d104e63a6ddc80b1dc2596686d28bff05a93cc3f44ef57dc86b59e4ff0a'
  crates/app/src/panel.rs: 'sha256:b229abf065c777de9460a87101cb6343d6f7e56eddac128b0f4f261f5d440247'
  crates/app/src/lib.rs: 'sha256:fbeb0f243b9e46e62b56b7811dd1150212ae2b53b4cadee3a3bbe2b9c4810b7b'
tags:
  - pruebas
  - mutacion
  - medida
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El patrón, con su cuenta.** En el hito F1.6 este mismo fallo apareció **tres veces**, en tres fases distintas, y las tres veces la suite estaba **verde** y una revisión previa lo había dado por cubierto. No es descuido de nadie: es la forma natural de escribir una prueba cuando la propiedad vive en una función pequeña y bien nombrada.

> **La mutación hay que hacerla en el sitio donde el mecanismo importa, no donde el mecanismo está escrito.** Mutar el ayudante y verlo caer demuestra que el ayudante funciona; no demuestra que alguien lo llame, ni que lo llame bien.

**Los tres casos:**

1. **El techo de lectura** (validación del primer PR). La prueba llamaba a `leer_acotado` directamente. Sustituyendo dentro de `read` esa llamada por un `std::fs::read` —reservar el fichero entero, justo lo prohibido— **las 41 pruebas seguían verdes**, porque la comprobación posterior devuelve el mismo error.
2. **El color del panel** (revisión del segundo PR). Lo probado era `Tema::color_del_panel`. Su único llamante, la función que pinta, podía **ignorar el estilo**, **pintarlo todo del mismo color** o **no pintar nada**, y nada caía. Cinco mutantes de una sola raíz.
3. **Dónde cae cada fila** (validación del segundo PR). Las pruebas de pintado ya existían y afirmaban el **contenido y el orden** de la cola de formas, descartando la posición. Quitando el avance de la altura, las líneas se pintaban **una encima de otra en el mismo renglón** y las 122 pruebas pasaban. Con eso caían dos criterios a la vez.

**Cómo se reconoce antes de que muerda.** Pregúntate qué mutación **de producción** pone la prueba en rojo, y comprueba que esa mutación está en el camino que se ejecuta, no en la función que la prueba nombra. Si la prueba llama a algo que el código real llama a su vez, no ata al intermediario.

**Dos corolarios, los dos aprendidos a costa de alguien:**

- **Hay propiedades que el resultado no distingue.** El techo devuelve el mismo error se reserve un cuarto de mega o dos, así que ninguna aserción sobre el valor separa los casos: hay que **medir el recurso**. Un asignador global bajo `#[cfg(test)]` lo resuelve, con la cuenta **por hilo** —las pruebas corren en paralelo y un contador del proceso da fallos intermitentes— y con `try_with` en vez de `with`, porque al deshacerse un hilo todavía se libera memoria.
- **Y hay propiedades que solo se ven en la posición, no en el contenido.** Un hueco de una línea vacía no encola ninguna forma: lo que demuestra que dejó su sitio es la **distancia** entre sus vecinas.

**La trampa hermana, de la misma familia:** una prueba que se afirma **contra la constante que pretende proteger**. La del margen del panel comparaba contra `MARGEN * ancho`; poniendo el margen a cero, los dos lados de la igualdad cambiaban juntos y la suite seguía verde.

## Where it lives

Los tres casos con su prueba: `crates/panel-contract/src/panel.rs` (`leer_un_fichero_enorme_no_reserva_lo_que_ocupa` y el módulo `cuenta` con su contador), y `crates/app/src/panel.rs` (`el_color_de_cada_linea_sale_de_su_estilo_y_de_su_marca`, `cada_fila_cae_en_su_renglon_y_la_vacia_deja_el_suyo`, y el ayudante `lo_que_se_encola`, que es lo que permite afirmar sobre lo pintado).
