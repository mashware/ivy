---
id: asp_la-consola-de-control-no-se-resta-sin-medir-su-asimetria_2c2aw2
slug: la-consola-de-control-no-se-resta-sin-medir-su-asimetria
name: >-
  Restar la consola de control de la consola con gesto NO da el coste del gesto:
  las dos ya difieren sin gesto ninguno, y esa resta ya publicó dos números
  falsos
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T07:36:53.108Z'
updated_at: '2026-08-25T07:36:53.108Z'
last_verified: '2026-08-25T07:36:53.108Z'
file_paths:
  - scripts/medir-fotograma.sh
  - crates/app/src/medida.rs
  - docs/02-decisiones.md
symbols: []
content_hashes:
  scripts/medir-fotograma.sh: 'sha256:96a1e649a130812d99a0d5a88ec31fa3ecb1403745a9d5a00fd270dc962dc043'
  crates/app/src/medida.rs: 'sha256:5c0a49ba86ceeeb28966cab5ea4899a9e7ddfbcd858788896af1912796621d8b'
  docs/02-decisiones.md: 'sha256:fc611d2106ed7c61fbf2afacb79dd2ed3c5987f805d981a1861a3d90883c58c3'
tags:
  - medida
  - instrumento
  - tandas
  - d22
  - d21
  - f2.6
feature_id: feat_term-core_nx0g1r
---

## What it does

**El patrón del instrumento, en una frase: una consola de control DENTRO de la misma corrida arregla el problema de comparar entre corridas, y no arregla el de comparar entre consolas.**

Varias tandas del guion (`K` el panel, `M` la frontera del árbol, `N` marcar texto) hacen el gesto en la consola izquierda y dejan la derecha quieta como control. Es una mejora real sobre comparar dos corridas distintas —ver [[dos-anexos-de-dias-distintos-no-se-comparan]]—: el binario, la máquina y el momento son los mismos. La tentación que sigue es obvia y es donde está la trampa: **restar la de control de la del gesto y publicar la diferencia como «lo que cuesta el gesto»**.

**No lo es, porque la asimetría existe sin gesto ninguno.** La tanda quieta ya da **32 y 41 µs** entre izquierda y derecha, sin que nadie toque nada. Esa diferencia de base no aparecía en la tabla del anexo, así que el número publicado se la comía entera.

**Ya ha producido dos números falsos publicados en el registro de decisiones:**

1. **D21** — el doblado de la vuelta completa atribuido al historial. Autopsia entera en [[dos-anexos-de-dias-distintos-no-se-comparan]].
2. **D22** — «marcar cuesta ~100 µs», de restar 171 a 273 en la misma corrida. Corregido a una **horquilla de 50–60 µs declarada como NO aislada**, con las **dos** cuentas posibles publicadas (62 y 53 µs) y el hecho de que **no coinciden** dicho en el propio anexo.

> **Y la causa que se le atribuía también era falsa, que es la segunda mitad del hallazgo.** El anexo decía «veinte rectángulos de fondo de más al pintar», o sea un coste proporcional a lo marcado. Lo real es que **con una selección viva se pregunta por cada celda de la rejilla** si va realzada: 1.911 en una consola de 49×39, estén marcadas dos o dos mil. El coste **no crece con lo marcado**: crece con la rejilla y con el número de consolas. Una aritmética que se contradecía sola dentro del propio anexo, y aun así se publicó.

**Lo que haría falta para aislarlo de verdad**, y está escrito en D22 en vez de fingirse: correr la tanda **dos veces con el mismo binario**, una con el gesto y otra sin él, y comparar **la misma consola consigo misma**. Mientras eso no se corra, lo honesto es publicar la horquilla y decir que no está aislada.

**Y hay un tercer sesgo que impide promediar a la ligera**: las corridas no comparten régimen. Las tandas con gesto inyectado corren **muchos más fotogramas** —1.066 y 2.640 contra 429 y 470 de la quieta—, porque inyectar entrada mantiene la ventana despierta igual que un ratón humano. Sus medias por fotograma no se comparan con las de una tanda quieta.

## Where it lives

El reparto por resta está en `crates/app/src/medida.rs`; las tandas con consola de control (`K`, `M`, `N`) y sus guardas, en `scripts/medir-fotograma.sh`. Los dos anexos afectados son los de **D21** y **D22** en `docs/02-decisiones.md`, los dos ya reescritos. Es la misma familia de error que [[quien-no-declara-su-trabajo-se-lo-atribuye-a-otro]] y [[medir-algo-que-cambia-la-rejilla-no-es-una-resta]]: el instrumento reparte por resta, y toda diferencia que no se declara se la queda el sumando que se calcula restando.
