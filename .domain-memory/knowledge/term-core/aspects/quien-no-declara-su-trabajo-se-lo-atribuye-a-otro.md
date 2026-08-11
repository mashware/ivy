---
id: asp_quien-no-declara-su-trabajo-se-lo-atribuye-a-otro_6bbts6
slug: quien-no-declara-su-trabajo-se-lo-atribuye-a-otro
name: >-
  En un instrumento por partes, quien no declara su trabajo se lo atribuye a
  otro: el fallo aparece al pasar de una a dos
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-11T16:28:21.609Z'
updated_at: '2026-08-11T16:28:21.609Z'
last_verified: '2026-08-11T16:28:21.609Z'
file_paths:
  - crates/app/src/medida.rs
  - crates/app/src/lib.rs
symbols:
  - 'Medida::cerrar_fotograma'
  - 'Medida::anotar_vuelta'
  - 'RestoDelFotograma::anotar'
  - 'Ventana::cerrar_fotograma'
content_hashes:
  crates/app/src/medida.rs: 'sha256:b55a4357bfb9ee09793b07a352ac7f74bdcfbbd12c25527fe829182eaadcec3c'
  crates/app/src/lib.rs: 'sha256:bcf3ddb3e7b0c70d3d8b306bd58930520777e0baa28afa408bd8eb0b12ccd2ad'
tags:
  - medida
  - atribucion
  - f1.4
  - revision
  - d3
feature_id: feat_term-core_nx0g1r
---

## What it does

**El fallo, en una frase:** cuando un instrumento reparte un total entre partes y calcula una de ellas **por resta**, cualquier parte que no declare su trabajo se lo regala a la que se calcula restando. Y el síntoma no aparece con una parte — aparece con dos.

**El caso concreto (F1.4, agosto 2026).** El coste de un fotograma se reparte así: cada consola cronometra lo suyo (interpretar los bytes y encolar el pintado) y «el resto del fotograma» —teselar, dibujar, el armazón del framework— sale de **restar** al total de CPU lo que declararon todas. El framework publica el total **una sola vez por pasada**, así que no hay otra forma de obtenerlo.

El instrumento descartaba los tramos de una consola **cuando no le había llegado salida** en ese fotograma, con un argumento correcto: un fotograma en reposo no es una vuelta rapidísima, es una vuelta que no ocurrió, y meterla en la media diría que la aplicación responde al instante justamente por no haber hecho nada. Pero al descartar los tramos se descartaba también **el encolado, que sí había ocurrido**: la consola quieta se pinta igual todos los fotogramas. Ese tiempo acababa publicado como coste del framework.

**Por qué era invisible.** Con **una** consola el fallo no existía: si no había tramos, el fotograma entero se saltaba y no se anotaba ningún resto. Con **dos** se convierte en el caso normal — lo habitual es una trabajando y la otra en su prompt. Y ninguno de los dos tests que cubrían el reparto lo cazaba, porque en los dos **todas** las partes tenían tramos. Lo encontraron dos revisores por separado, ambos leyendo el camino completo del cálculo.

**La distinción que lo arregla, y que es el aprendizaje transportable:** hay que separar **«qué series anotar»** de **«qué trabajo declarar»**. No son la misma pregunta. Una serie estadística puede y debe rechazar muestras que no representan nada; la declaración del trabajo hecho **no puede rechazar nada**, porque alguien la está restando. En el código eso se ve en que el tramo de espera pasó a ser opcional: `None` significa «no hubo vuelta que medir», y sin embargo el método **sigue devolviendo lo cronometrado**.

**La regla, para la próxima vez que algo se calcule por resta:** antes de añadir una parte nueva al reparto, preguntar *¿puede esta parte hacer trabajo y no declararlo?* Si la respuesta es sí, el número calculado por resta está inflado y **nadie lo va a notar**, porque sigue sumando el total. Es un fallo que no rompe ninguna invariante visible: los números siguen cuadrando entre sí.

**Por qué importa aquí más que en otro sitio.** Ese número calculado por resta es justo el que decide **D3** — si el framework de interfaz aguanta. Inflarlo con trabajo propio empuja hacia cambiar de framework por un coste que no es suyo, que es la decisión más cara de la fase.

## Where it lives

El reparto vive en `crates/app/src/medida.rs`: `Medida` (los tramos de una consola, con `esperado` opcional), `RestoDelFotograma` (la parte que se calcula restando) y `Medida::cerrar_fotograma`, que **siempre** devuelve lo cronometrado aunque no anote series. Quien orquesta es `Ventana::cerrar_fotograma` en `crates/app/src/lib.rs`, que suma lo de todas las consolas antes de restar una única vez. El test que fija el comportamiento es `una_consola_sin_salida_declara_igualmente_lo_que_costo_pintarla`; el número resultante se publica en el anexo «Medido — 11 agosto 2026» de D3, en `docs/02-decisiones.md`.
