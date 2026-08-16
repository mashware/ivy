---
id: asp_no-se-le-baja-el-presupuesto-a-una-de-fondo_ryegvz
slug: no-se-le-baja-el-presupuesto-a-una-de-fondo
name: >-
  «Que una consola de fondo siga corriendo» y «que consuma menos» son
  incompatibles por construcción: recortarle el presupuesto le activa el freno
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:25:49.343Z'
updated_at: '2026-08-15T21:25:49.343Z'
last_verified: '2026-08-15T21:25:49.343Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - PRESUPUESTO
  - 'Division::bombear'
  - bombear_todas
content_hashes:
  crates/app/src/lib.rs: 'sha256:b3ca4328c99c0b94b314ea9a6c6d1ab4f6afe0dad22ef3415700f289bc9af411'
tags:
  - presupuesto
  - pestañas
  - F2.1
  - medido
feature_id: feat_term-core_nx0g1r
---

## What it does

**El presupuesto de bombeo es por división, no de la ventana**, y eso decide lo que se puede hacer con una consola que no se ve: `PRESUPUESTO` se pide entero en cada `Division::bombear`, y `bombear_todas` la llama una vez por división. Con N divisiones el techo real por fotograma es **N×128 KiB**.

> **La consecuencia es una contradicción de diseño, no un ajuste:** «que una consola de fondo siga corriendo» y «que una consola de fondo consuma menos» **son incompatibles por construcción**. Bajarle el presupuesto es dejar de vaciar su pseudoterminal al ritmo que produce, y el margen que regala el núcleo antes de bloquear al programa son **20.480 bytes** ([[el-pseudoterminal-medido]]), **seis veces menos que el propio presupuesto**. O sea que recortarlo **le activa el freno de D8**: el programa se bloquea al escribir. No es que vaya más lenta la pintura — **es que el trabajo se para**.

**Lo que sí reparte, y no hace falta construirlo, es el propio fotograma:** cada división traga sus 128 KiB y el fotograma se alarga, así que el caudal total se reparte solo sin que ninguna se pare. El precio se paga en fotogramas por segundo de **toda** la ventana.

**Con números** (aritmética sobre los 4,35-4,51 ms medidos para dos consolas en el anexo de D3; ~2,18 ms por consola a presupuesto lleno):

| Pestañas volcando | Fotograma | 120 Hz | 60 Hz |
|---|---|---|---|
| 1 (medido) | 7,01 ms | 84 % | 42 % |
| 2 | ~11,3 ms | **136 %** | 68 % |
| 4 | ~20,1 ms | 241 % | **121 %** |

**Dos pestañas volcando a la vez ya se salen del hueco de 120 Hz**, y cuatro se salen también del de 60 Hz. Es el precio explícito de la promesa del hito, aceptado y por publicar — no un defecto. **Pestañas quietas —el caso de diseño, agentes esperando— no cuestan nada**: un `try_recv` vacío por división.

**Y la palanca alternativa tampoco está:** un presupuesto por tiempo en vez de por bytes está descartado por escrito, porque entonces el coste de interpretar vale por definición lo que se le ponga y **el instrumento devuelve lo que uno mismo metió**.

## Where it lives

`PRESUPUESTO`, `Division::bombear` y `Ventana::bombear_todas` en `crates/app/src/lib.rs`
