---
id: asp_una-prueba-que-ata-al-ayudante-no-ata-el-camino_hts3p9
slug: una-prueba-que-ata-al-ayudante-no-ata-el-camino
name: >-
  Una prueba que ata al ayudante no ata el camino real: nueve veces, y la novena
  dentro del arreglo de la octava
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:06:42.827Z'
updated_at: '2026-08-13T15:25:30.209Z'
last_verified: '2026-08-13T15:25:30.209Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/app/src/panel.rs
  - crates/app/src/lib.rs
  - crates/app/src/vigilante.rs
  - crates/app/src/medida.rs
symbols:
  - read
  - leer_acotado
  - pintar
  - color_del_panel
  - pintar_division
  - 'Ventana::nueva'
  - poner_el_despertador
  - publicar_tramos
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:024c1d104e63a6ddc80b1dc2596686d28bff05a93cc3f44ef57dc86b59e4ff0a'
  crates/app/src/panel.rs: 'sha256:bb0c2831a73e9318e084df0e0ac888d48ef188b7bb87d678dea35bc49458c572'
  crates/app/src/lib.rs: 'sha256:68622c95bad38cef50f3eb8b4de8f2407dddf761bf81edc119d00fcaf5a24223'
  crates/app/src/vigilante.rs: 'sha256:d1afca028fafa9587fe2b06defd2f3d8459aacb424b110958b01297266e7d621'
  crates/app/src/medida.rs: 'sha256:2c620cfa14e9ac593b3293628869a8ddb5d48b5ba3de54e08eb72f775d3ebe17'
tags:
  - pruebas
  - mutacion
  - medida
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El patrón, con su cuenta.** **Nueve veces** en este proyecto, en fases distintas y con la suite **verde** las nueve. No es descuido: es la forma natural de escribir una prueba cuando la propiedad vive en una función pequeña y bien nombrada.

> **La mutación hay que hacerla en el sitio donde el mecanismo importa, no donde el mecanismo está escrito.** Mutar el ayudante y verlo caer demuestra que el ayudante funciona; no demuestra que alguien lo llame, ni que lo llame bien.

**La novena es la que enseña más, porque se cometió arreglando la octava.** El trabajo consistía en corregir un tramo del informe mal etiquetado, y la prueba se escribió **a propósito** para atar lo que el informe imprime de verdad en vez de afirmar sobre una constante — con ese razonamiento escrito en el comentario de la función. Pero se dejó un **envoltorio** delante: `publicar_tramos()` sin argumentos, que llamaba a la función interna con la salida estándar, y la prueba entraba por la interna. **Vaciar el envoltorio dejaba las 289 en verde con el informe mudo.**

> **Un envoltorio de una línea entre la prueba y producción es un ayudante disfrazado de camino real.** Si la puerta que producción usa no es exactamente la función que la prueba llama, hay un hueco, por corta que sea la diferencia.

**Y el arreglo importa tanto como el hallazgo: se quitó el envoltorio, no se añadió otra prueba.** `publicar_tramos` pasa a **recibir el destino**, así que la función probada *es* la que producción llama y lo único que aporta quien llama es a dónde escribir. La mutación deja de ser posible en vez de quedar cubierta. Cuando el hueco es estructural, taparlo con otra prueba deja el hueco.

**Los nueve casos:**

1. **El techo de lectura** — la prueba llamaba a `leer_acotado`; sustituir dentro de `read` esa llamada por un `std::fs::read` dejaba 41 verdes.
2. **El color del panel** — se probaba `Tema::color_del_panel`; su único llamante podía ignorar el estilo o no pintar nada.
3. **Dónde cae cada fila** — se afirmaba contenido y orden, no posición: las líneas se pintaban una encima de otra con 122 verdes.
4. **La continuación de la envoltura** — «cae más a la derecha que el símbolo» lo cumple cualquier sangría distinta de cero.
5. **El mapa de marca a símbolo** — se comprobaba el conjunto, no los pares: era permutable entero.
6. **Las rutas que la ventana da al vigilante** — pasarle una lista vacía lo deja armado y ningún panel se mueve en toda la sesión.
7. **Que la ventana pida fotograma** — la prueba contaba llamadas a un despertador **que fabricaba ella misma**.
8. **El latido** — llamaba a `logic` suelto con el panel ya releído por un ayudante, y afirmaba sobre un booleano que satisface cualquier cosa.
9. **El texto del informe** — el envoltorio de arriba.

**Cómo se reconoce antes de que muerda.** Pregúntate qué mutación **de producción** pone la prueba en rojo, y comprueba que esté en el camino que se ejecuta. Tres señales: la prueba llama a algo que el código real llama a su vez; la prueba fabrica el doble del colaborador; **o hay una función intermedia que la prueba se salta**.

**Tres corolarios, los tres a costa de alguien:**

- **Hay propiedades que el resultado no distingue** — el techo devuelve el mismo error se reserve un cuarto de mega o dos: hay que **medir el recurso**, con la cuenta por hilo porque las pruebas corren en paralelo.
- **Hay propiedades que solo se ven en la posición** — una línea vacía no encola ninguna forma; lo que demuestra que dejó su sitio es la **distancia** entre sus vecinas.
- **Una aserción puede ser vacía sin parecerlo** — la del respaldo del vigilante comprobaba que se pidiera repintado, y un contexto de egui recién creado ya lo pide por su cuenta.

**La trampa hermana:** una prueba que se afirma **contra la constante que pretende proteger**. La del margen del panel comparaba contra `MARGEN * ancho`; con el margen a cero, los dos lados cambiaban juntos.

## Where it lives

Repartido por F1.6 entero y por el trabajo que le sigue: `crates/panel-contract/src/panel.rs` (el techo de lectura), `crates/app/src/panel.rs` (el color, la posición de las filas, la continuación, el mapa de marcas), `crates/app/src/lib.rs` (el reparto, el despertador, el latido), `crates/app/src/vigilante.rs` (las rutas y el sentido de la bandera) y `crates/app/src/medida.rs` (el texto del informe).
