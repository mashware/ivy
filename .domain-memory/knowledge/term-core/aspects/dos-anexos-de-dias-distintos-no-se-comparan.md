---
id: asp_dos-anexos-de-dias-distintos-no-se-comparan_t7pske
slug: dos-anexos-de-dias-distintos-no-se-comparan
name: >-
  Dos anexos de días distintos no se comparan: el binario cambia debajo. Un
  antes/después alterna la variable sobre EL MISMO binario, compilado a
  propósito
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T22:26:20.874Z'
updated_at: '2026-08-24T22:26:20.874Z'
last_verified: '2026-08-24T22:26:20.874Z'
file_paths:
  - docs/02-decisiones.md
  - scripts/medir-fotograma.sh
  - crates/term-core/src/screen.rs
symbols:
  - HISTORIAL
content_hashes:
  docs/02-decisiones.md: 'sha256:2201996a78171d760ec80e0cf445419050c036975883ef44094d7ec0ca3013f9'
  scripts/medir-fotograma.sh: 'sha256:5164b53754c528c121b0dc462bd6b6ded7dd92b97e504c321ea968fdd7d77d2f'
  crates/term-core/src/screen.rs: 'sha256:53c0f8b50a8a8c82c87d3d9afc31d84ff9f171f5720cf419937e1ac25464f193'
tags:
  - medida
  - metodo
  - d21
  - d3
feature_id: feat_term-core_nx0g1r
---

## What it does

**El caso, y es el peor tipo de error que este proyecto puede cometer: un número mal etiquetado publicado en el registro de decisiones.**

El anexo de **D21** publicó que encender el historial **doblaba la vuelta completa** — «37,1 → 75,4 ms», con el texto afirmando que las dos corridas eran «del mismo binario del día y a media hora de distancia». Las dos cosas eran falsas:

1. **La columna de control era de otro día y otro binario.** Salía de una corrida del **18 de agosto**; la otra, del **24**. Entre medias habían entrado hitos enteros.
2. **Y la conclusión no la sostenían los datos.** Hay corridas de la misma tanda del **19 de agosto**, con el historial **apagado**, que ya daban **73,7 y 74,0 ms**. El salto de 37 a 74 ya había ocurrido **antes** de encender nada: atribuírselo al historial era colgarle un coste ajeno.

**Medido de verdad**, con cuatro corridas seguidas del mismo binario —dos compilando con el techo a cero y dos con las 3 500 líneas—: el historial cuesta **+39 % de interpretar** y **+1,2 % de vuelta completa**, con el residuo del framework quieto. O sea **un orden de magnitud menos** de lo publicado, y en otro tramo.

> **La regla que queda: un antes/después se mide alternando la variable sobre EL MISMO binario, y eso normalmente exige compilar a propósito el estado que ya no existe.** Apagar la constante, compilar, medir, restaurar, compilar, medir. No hay atajo: buscar en `target/medidas/` una corrida vieja «de antes» **parece** el atajo y es la trampa, porque el fichero no dice con qué binario se tomó.

**Por qué es fácil caer, y por qué este almacén ya avisaba de la mitad.** Los informes se acumulan en `target/medidas/` con su fecha en el nombre, así que siempre hay «una de antes» a mano. Ninguna guarda del guion lo caza —comprueban que la corrida midió lo que promete, no con qué código— y el comparador solo mira que **la ventana y la rejilla** coincidan, que es otra cosa. El aviso hermano ya fichado, [[el-escritorio-elige-el-tamano-y-la-tanda-no-es-reproducible]], cubre la variable **tamaño**; ésta cubre la variable **código**, y es la que no se ve en el informe.

**Consecuencia práctica para los anexos publicados:** son **registros fechados**, no cifras vigentes. La misma tanda `C` da 52 ms el 17 de agosto, 37 el 18, 74 el 19 y 75 el 25 — y solo el último par es comparable con el anterior. Un anexo que se relee un mes después hay que leerlo con su fecha delante, y **cruzar dos anexos de fechas distintas para sacar una conclusión es exactamente lo que produjo este error**. Cuando haga falta la comparación, se vuelve a medir.

**Y deja una pregunta abierta que no era del hito:** entre el 18 y el 19 de agosto la vuelta completa de la misma tanda, con la misma ventana y la misma rejilla, pasó de 37 a 74 ms con el historial apagado — **y no es coste de CPU**: interpretar se movió de 763 a 851 µs y el residuo del framework de 220 a 240. Los fotogramas son parecidos en número y en coste, así que lo que se duplicó es **lo que cada byte espera**, no lo que cuesta atenderlo. Apunta a cuándo se pide repintado, no a cuánto cuesta pintar.

## Where it lives

El anexo corregido y la retirada del anterior están en `docs/02-decisiones.md`, decisión **D21**, en «Medido — 25 agosto 2026: lo que el historial le hace a la ventana de verdad». La constante que se apaga para el control es `term_core::HISTORIAL` en `crates/term-core/src/screen.rs`. Las tandas se lanzan con `scripts/medir-fotograma.sh C`, y los informes quedan en `target/medidas/`, que **no está versionado**.
