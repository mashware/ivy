---
id: asp_el-formato-en-disco-lo-hacen-las-variantes_ypabp4
slug: el-formato-en-disco-lo-hacen-las-variantes
name: >-
  El formato en disco lo hacen los campos Y las variantes: con `rename_all`,
  renombrar una variante reescribe el fichero y no lo caza nada de lo que se
  corre a diario
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:11:25.448Z'
updated_at: '2026-09-03T16:11:25.448Z'
last_verified: '2026-09-03T16:11:25.448Z'
file_paths:
  - crates/app/src/session.rs
  - crates/app/src/measure.rs
  - scripts/measure-frame.sh
symbols:
  - StoredOrientation
  - StoredNode
  - StoredDocument
  - StoredProbe
  - VERSION
content_hashes:
  crates/app/src/session.rs: 'sha256:b7824b96a3e438e7695ed87ccecbe9d65ed9dd16258b5e74ba3c39c8008f09e3'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
  scripts/measure-frame.sh: 'sha256:ff632681ce0123f34a58a161ee3726107de47669051545e12a59b4968f47bd42'
tags:
  - d27
  - f2.12
  - serde
  - formato-en-disco
  - guardas
feature_id: feat_migracion-a-ingles_xnx8mc
---

## What it does

**Las claves de `layout.json` salen de nombres de Rust sin un solo literal, y el agujero peligroso no son los campos: son las VARIANTES.**

En `crates/app/src/session.rs` hay **solo dos atributos de serde en todo el árbol** — los dos `rename_all = "lowercase"` — y **cero `#[serde(rename = …)]`**. Verificado en septiembre de 2026: sigue siendo 2 y 0. Todo lo demás que llega al fichero **es** el nombre del campo o de la variante.

**Los campos ya estaban protegidos por una doctrina escrita en el propio fichero**: `StoredOrientation` es un tipo aparte de `crate::Orientation` **a propósito**, porque los nombres de ese enum *son* el formato — aparecen literalmente en el fichero de quien use la aplicación, y reutilizar el tipo vivo ataría el fichero a un nombre que el código puede querer cambiar mañana. Lo mismo con `StoredNode`, `StoredChild` y `StoredTab`, que existen dos veces por esa razón. Y `StoredProbe` existe porque *mirar la versión después de encajar la forma es no tener versión*, con su regla de cuándo sube: **justo con lo que un lector viejo malinterpretaría — una clave renombrada, quitada, o con otro significado**.

> **Lo que esa doctrina NO cubría, y es donde muerde: con `rename_all`, los nombres de las VARIANTES son el fichero.** `Columns`/`Rows` y `Leaf`/`Branch` viajan a disco en minúsculas. Renombrar `Filas` a `Rows` deja el fichero escribiendo `"rows"` mientras el guion de medida sigue sembrando `"orientacion":"filas"`.

**Y eso no lo caza nada de lo que se corre a diario**: no el compilador (compila), no `cargo test` si ninguna prueba enfrenta el formato con lo que el guion siembra, y no un prefijo en el nombre del tipo. Lo cazaba **solo** la guarda `restored:` de la tanda `L` del banco, y solo si alguien la corría — el mismo modo de fallo de [[una-tanda-tiene-el-guardado-vetado]]: una guarda que nadie ejecuta durante semanas.

### Cómo se cerró, y por eso este hallazgo sigue valiendo

Es la razón de ser de **AC10** del hito: al renombrar campos o variantes, la versión sube en el mismo commit **y el guion siembra el formato nuevo**. Verificado en septiembre de 2026: el guion siembra ya `"orientation":"rows"` y `"columns"`, o sea las dos mitades se movieron juntas. Y la mitad que no se veía está atada por una prueba que **deriva** en vez de copiar: `measure.rs:2879` afirma `SCRIPT.contains(crate::session::FILE)` sobre el texto del guion leído con `include_str!`.

**Regla práctica que deja:** en este proyecto, «¿esto cambia el formato en disco?» se contesta mirando **campos, variantes y `rename_all`** — nunca solo los campos. Y cualquier prueba que quiera atarlo tiene que **derivar** el formato de lo que el guion siembra, no afirmar un literal escrito al lado.

## Where it lives

`crates/app/src/session.rs` — los dos `rename_all` y la doctrina de los tipos duplicados (`StoredOrientation`, `StoredNode`, `StoredChild`, `StoredTab`, `StoredProbe`), más `VERSION`, `DIRECTORY` y `FILE`.

`scripts/measure-frame.sh` — donde el formato se siembra a mano para la tanda que restaura.

`crates/app/src/measure.rs:2879` — `the_script_seeds_the_same_layout_file_the_application_writes`, la guarda derivada que cierra la mitad que no se veía; su propio comentario explica el disfraz que sustituye.

`docs/02-decisiones.md` — **AC10** del hito F2.12, y el anexo de **D27** en el apartado de qué nombres son el formato en disco.
