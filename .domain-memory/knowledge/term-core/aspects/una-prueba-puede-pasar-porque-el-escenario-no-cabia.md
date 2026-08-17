---
id: asp_una-prueba-puede-pasar-porque-el-escenario-no-cabia_j56vtg
slug: una-prueba-puede-pasar-porque-el-escenario-no-cabia
name: >-
  Un disfraz nuevo de prueba que no ata nada: pasa porque el escenario elegido
  no dejaba ocurrir el defecto, no porque la guarda funcione
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T23:13:30.250Z'
updated_at: '2026-08-16T23:13:30.250Z'
last_verified: '2026-08-16T23:13:30.250Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - cabe_partir
  - 'Division::teclear'
content_hashes:
  crates/app/src/lib.rs: 'sha256:01adf2fc83535e392e34a240d6ed26b7af67123641006bbb510b9f94b9c87f5a'
tags:
  - pruebas
  - mutación
  - F2.2
feature_id: feat_term-core_nx0g1r
---

## What it does

**El caso.** Partir una división lanza un programa, así que el acorde lleva una guarda contra la repetición de la tecla mantenida: sin ella, un segundo apretado son treinta consolas. La prueba mandaba el acorde con tres repeticiones y compraba que solo hubiera una división nueva. **Verde.** Quitando la guarda: **verde también**.

**Por qué.** La ventana de la prueba medía 160 columnas. Tras la primera partición, la división con el foco se quedó en 40, y el suelo geométrico exige 40 **más el grosor del divisor** para volver a partir. O sea: lo que frenó a la segunda partición **no fue la guarda, fue que ya no cabía**. La prueba medía otra cosa y lo decía todo bien.

> **El disfraz, en una frase: el escenario de la prueba impedía por su cuenta el defecto que la prueba decía cubrir.** No es afirmar sobre el campo equivocado ni elegir un número donde defecto y arreglo coinciden —los dos disfraces que este proyecto ya tenía fichados—: aquí la aserción es la correcta y el número también; lo que está mal es **el mundo** donde se afirma.

**Cómo se arregla, y es barato:** ensanchar el escenario hasta que el defecto **pueda** ocurrir, y añadir la mitad que faltaba —la que demuestra que el escenario no era el límite—. Aquí: 200 columnas, y después del acorde mantenido, el mismo acorde **soltado y vuelto a pulsar** sí parte. Sin esa segunda mitad, la prueba vuelve a poder pasar por la razón equivocada en cuanto alguien toque el suelo.

**Cómo se detecta:** solo mutando. Ninguna lectura del código de la prueba lo delata —se lee perfecta—, y la suite en verde es exactamente lo que el defecto produce.

## Where it lives

`crates/app/src/lib.rs` — la guarda `!repeat` en `Division::teclear` y el suelo `cabe_partir`; la prueba `mantener_pulsado_el_acorde_de_partir_no_abre_mas_que_una_consola`. Encontrado en F2.2, PR #73.
