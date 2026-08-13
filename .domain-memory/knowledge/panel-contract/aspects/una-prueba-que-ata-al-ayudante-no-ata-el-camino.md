---
id: asp_una-prueba-que-ata-al-ayudante-no-ata-el-camino_hts3p9
slug: una-prueba-que-ata-al-ayudante-no-ata-el-camino
name: >-
  Una prueba que ata al ayudante no ata el camino real: ocho veces en un solo
  hito, y las ocho con la suite en verde
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:06:42.827Z'
updated_at: '2026-08-13T13:24:25.350Z'
last_verified: '2026-08-13T13:24:25.350Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/app/src/panel.rs
  - crates/app/src/lib.rs
  - crates/app/src/vigilante.rs
symbols:
  - read
  - leer_acotado
  - pintar
  - color_del_panel
  - pintar_division
  - 'Ventana::nueva'
  - poner_el_despertador
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:024c1d104e63a6ddc80b1dc2596686d28bff05a93cc3f44ef57dc86b59e4ff0a'
  crates/app/src/panel.rs: 'sha256:568e67aba57fd9f60e9ca802161c515776057b5d550f6074bf1af4cf561d98a8'
  crates/app/src/lib.rs: 'sha256:1e1fccb7584128dc015d57b4a85b7afba61479eddebd8cffdbabeba928cc19be'
  crates/app/src/vigilante.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
tags:
  - pruebas
  - mutacion
  - medida
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El patrón, con su cuenta.** En el hito F1.6 este mismo fallo apareció **ocho veces**, en fases distintas y en los cinco PRs, y las ocho veces la suite estaba **verde** y una revisión previa lo había dado por cubierto. No es descuido de nadie: es la forma natural de escribir una prueba cuando la propiedad vive en una función pequeña y bien nombrada. Que reaparezca **después** de escribirla aquí es el dato importante — saber el patrón no basta; hay que mutar.

> **La mutación hay que hacerla en el sitio donde el mecanismo importa, no donde el mecanismo está escrito.** Mutar el ayudante y verlo caer demuestra que el ayudante funciona; no demuestra que alguien lo llame, ni que lo llame bien.

**Los ocho casos:**

1. **El techo de lectura** (validación del PR 1). La prueba llamaba a `leer_acotado` directamente. Sustituyendo dentro de `read` esa llamada por un `std::fs::read` —reservar el fichero entero, justo lo prohibido— **las 41 pruebas seguían verdes**, porque la comprobación posterior devuelve el mismo error.
2. **El color del panel** (revisión del PR 2). Lo probado era `Tema::color_del_panel`. Su único llamante podía **ignorar el estilo**, **pintarlo todo del mismo color** o **no pintar nada**, y nada caía. Cinco mutantes de una sola raíz.
3. **Dónde cae cada fila** (validación del PR 2). Las pruebas afirmaban **contenido y orden** de la cola de formas, descartando la posición. Quitando el avance de la altura, las líneas se pintaban **una encima de otra** y las 122 pasaban.
4. **La continuación de la envoltura** (validación del PR 3). La prueba decía «la continuación cae más a la derecha que el símbolo», y eso lo cumple **cualquier** sangría distinta de cero: con la continuación a una celda del borde —o sea **debajo** del símbolo, justo lo prohibido— las 150 pasaban.
5. **El mapa de marca a símbolo** (validación del PR 3). Se comprobaba el **conjunto** de símbolos, no los pares: el mapa era permutable entero con la suite en verde.
6. **Las rutas que la ventana da al vigilante** (revisión del PR 4). Nada ataba que `Ventana::nueva` armara el vigilante sobre los directorios de sus consolas: pasarle una lista **vacía** lo deja **armado**, así que el respaldo tampoco entra y **ningún panel se mueve en toda la sesión**, con las 164 verdes.
7. **Que la ventana pida fotograma** (validación del PR 4). La única prueba contaba llamadas a un despertador **que fabricaba la propia prueba**; sustituir el de la ventana por un cierre vacío dejaba todo verde con el criterio incumplido. Y su corrección tiene trampa propia: con `Ventana::nueva` el prompt del shell mantiene la ventana despierta y la prueba pasaría igual.
8. **El latido** (revisión del PR 5). La prueba llamaba a `logic` suelto con el panel **ya releído por un ayudante**, y se afirmaba sobre un booleano que satisface cualquier cosa. Reescrita para correr la pasada entera y afirmar sobre el **número** de segundos que la ventana pide esperar.

**Cómo se reconoce antes de que muerda.** Pregúntate qué mutación **de producción** pone la prueba en rojo, y comprueba que esa mutación está en el camino que se ejecuta, no en la función que la prueba nombra. Si la prueba llama a algo que el código real llama a su vez, no ata al intermediario. **Y si la prueba fabrica el doble del colaborador, no ata que el real sea ése.**

**Tres corolarios, los tres aprendidos a costa de alguien:**

- **Hay propiedades que el resultado no distingue.** El techo devuelve el mismo error se reserve un cuarto de mega o dos: hay que **medir el recurso**. Un asignador global bajo `#[cfg(test)]` lo resuelve, con la cuenta **por hilo** —las pruebas corren en paralelo— y con `try_with` en vez de `with`, porque al deshacerse un hilo todavía se libera memoria.
- **Hay propiedades que solo se ven en la posición, no en el contenido.** Un hueco de una línea vacía no encola ninguna forma: lo que demuestra que dejó su sitio es la **distancia** entre sus vecinas.
- **Una aserción puede ser vacía sin parecerlo.** La del respaldo del vigilante comprobaba que se pidiera repintado — pero un contexto de egui recién creado **ya lo pide por su cuenta**, así que la prueba pasaba con el mecanismo quitado.

**La trampa hermana, de la misma familia:** una prueba que se afirma **contra la constante que pretende proteger**. La del margen del panel comparaba contra `MARGEN * ancho`; poniendo el margen a cero, los dos lados cambiaban juntos y la suite seguía verde.

## Where it lives

Repartido por todo el hito F1.6: `crates/panel-contract/src/panel.rs` (el techo de lectura, con su módulo `cuenta` y el asignador global), `crates/app/src/panel.rs` (el color, la posición de las filas, la continuación de la envoltura, el mapa de marcas), `crates/app/src/lib.rs` (el reparto, el despertador de la ventana, el latido) y `crates/app/src/vigilante.rs` (las rutas que la ventana da al vigilante, y el sentido de la bandera).
