---
id: asp_el-instrumento-solo-describe-el-fotograma-final_czdwv1
slug: el-instrumento-solo-describe-el-fotograma-final
name: >-
  El informe describe el fotograma FINAL: una ventana que cambia de tamaño a
  mitad y vuelve deja una tanda idéntica a una limpia — declarado sin arreglar
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T16:09:26.353Z'
updated_at: '2026-08-17T16:09:26.353Z'
last_verified: '2026-08-17T16:09:26.353Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/medida.rs
  - scripts/medir-fotograma.sh
symbols:
  - tamaño_de_la_ventana
  - insistir_con_el_tamaño
  - cabecera
content_hashes:
  crates/app/src/lib.rs: 'sha256:5303a764d73851d1199719a27ca85c6efac8142650b896676e8be63ee15d187e'
  crates/app/src/medida.rs: 'sha256:c66a2a454bba2d7e6e289ede1b8a1d6e060fcdf40111022e0d4b234291d66daa'
  scripts/medir-fotograma.sh: 'sha256:3827b1223e7110cde05841466d25a38ccbc6d8fe0ceace88acd29fabf84c4e5f'
tags:
  - medida
  - F2.2
  - riesgo
  - declarado-sin-arreglar
feature_id: feat_term-core_nx0g1r
---

## What it does

**Punto ciego del instrumento, y está declarado sin arreglar con dos medidas detrás.** La cabecera del informe publica la rejilla y el tamaño de ventana **finales**: `tamaño_de_la_ventana` se sobrescribe en cada fotograma y el informe publica el último. Así que una ventana que cambia de tamaño a mitad de una tanda y **vuelve a su sitio** deja un informe **idéntico** al de una corrida limpia, con los tramos de los fotogramas de la otra rejilla dentro de las series. Un aviso del escritorio, un monitor que se enchufa o una política del compositor bastan. El guion de medida lo documentaba en su cabecera desde F2.1 **sin taparlo**.

**Se intentó tapar dos veces, y las dos reglas saltaron en la primera tanda REAL:**

1. **«Tuvo el tamaño pedido y lo perdió después»** — falso positivo. En GNOME/Wayland los dos primeros fotogramas **ya tienen** lo pedido y el compositor manda el suyo en el tercero, así que el arranque normal se publicaba como anomalía.
2. **«Solo cuenta si el fotograma aporta muestras al informe»** — también saltó, y eso enseña el dato que faltaba: **el compositor manda su configuración cuando las consolas ya están volcando**. Así que **toda corrida limpia tiene un fotograma con otro tamaño y sus muestras dentro** — uno de unos 358.

**Conclusión, y de ahí la decisión de no construirlo:** distinguir el arranque normal de una anomalía de verdad exige **contar** fotogramas contaminados y poner un **umbral**, y ese número no se puede justificar mientras el escenario —algo que redimensiona la ventana a mitad— **no se haya observado nunca**. Un fotograma de 358 es ruido; lo que haría falta cazar es un cambio que **dure**, y para eso hace falta haberlo visto una vez.

> **Dos reglas generales que deja, y valen fuera de este caso:** cuando un guardado nuevo **salta en la primera corrida real**, el fallo suele estar en la regla y no en el mundo. Y una señal que **no distingue el arranque del defecto** no es una señal.

Queda escrito en la cabecera de `scripts/medir-fotograma.sh` bajo «LO QUE SIGUE SIN COMPROBARSE», con las dos medidas, para que quien vuelva no lo intente por tercera vez sin datos nuevos.

## Where it lives

`crates/app/src/lib.rs` — `tamaño_de_la_ventana`, que se sobrescribe en cada pasada lógica, e `insistir_con_el_tamaño`, que es donde viviría la señal; `crates/app/src/medida.rs` — `cabecera`, que publica el último valor; `scripts/medir-fotograma.sh` — la declaración. Encontrado por el pase de revisión integrado del PR #75 de F2.2.
