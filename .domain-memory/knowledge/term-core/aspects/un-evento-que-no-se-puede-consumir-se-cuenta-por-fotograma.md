---
id: asp_un-evento-que-no-se-puede-consumir-se-cuenta-por-fotograma_e1crwv
slug: un-evento-que-no-se-puede-consumir-se-cuenta-por-fotograma
name: >-
  Un evento de egui sobre el que se ACTÚA pero que no se puede consumir exige
  llevar la cuenta del fotograma: sin ella, la pasada repetida manda el clic dos
  veces
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T09:23:01.083Z'
updated_at: '2026-08-25T09:23:01.083Z'
last_verified: '2026-08-25T09:23:01.083Z'
file_paths:
  - crates/app/src/lib.rs
symbols: []
content_hashes:
  crates/app/src/lib.rs: 'sha256:6ffd89344e0593f6d7362dccb0c76cc2ac211086a11fe9f50adb258313184450'
tags:
  - egui
  - raton
  - f2.6
  - eventos
  - pasada-repetida
feature_id: feat_term-core_nx0g1r
---

## What it does

**La regla, y sale de juntar dos cosas que este proyecto ya tenía fichadas por separado.** egui puede correr la pasada de interfaz **dos veces sobre la misma entrada**, y por eso el teclado y la rueda **consumen** sus eventos con `retain`: leerlos y dejarlos escribiría la misma tecla dos veces al pseudoterminal.

**Pero hay eventos que no se pueden consumir.** Los de botón del puntero son **lo que egui necesita** para decidir quién se arrastra: quién se los quite deja el gesto sin empezar nunca. El proyecto ya lo sabía para **mirar** —el acorde del apretón se lee sin consumir— y esa nota decía «aquí solo se mira».

> **En cuanto alguien ACTÚA sobre uno de esos eventos, la nota deja de bastar.** F2.6 reparte los apretones y soltados para mandárselos al programa de dentro, y en un fotograma con la pasada repetida el programa recibía **dos apretones donde el usuario dio uno**: un doble clic que nadie hizo, que dentro de un editor selecciona una palabra o abre lo que haya bajo el puntero.

**La salida es la que ya usa el inyector de gestos de las tandas: llevar la cuenta del fotograma.** Y el número correcto importa —`cumulative_frame_nr` sube **una vez por fotograma**; `cumulative_pass_nr` sube **por pasada** y no filtraría nada, que es el error que este proyecto ya cometió una vez en el inyector y dejó documentado en el propio código.

**El resumen como criterio, para no volver a razonarlo:**

| Sobre el evento | Qué hace falta |
|---|---|
| Solo mirarlo | Nada: leerlo dos veces da lo mismo |
| Actuar y **se puede** consumir | Consumirlo (`retain`), como el teclado y la rueda |
| Actuar y **no se puede** consumir | Llevar la cuenta del **fotograma**, no de la pasada |

**Y hay un efecto lateral que conviene conocer**: una guarda de «no repetir lo ya contado» —como la que evita informar dos veces la misma celda mientras se arrastra— **tapa este problema en su camino sin proponérselo**, porque en la pasada repetida el dato es el mismo. Eso es lo que hace que el defecto salga en un camino y no en el de al lado, y por tanto lo que hace que se busque en el sitio equivocado.

## Where it lives

El reparto de los botones y su cuenta de fotograma están en `crates/app/src/lib.rs`, en el atendido del ratón. Los dos caminos que sí consumen —teclado y rueda— están en el mismo fichero. El precedente del número equivocado (`pass` en vez de `frame`) está comentado en el inyector de gestos de las tandas de medida.
