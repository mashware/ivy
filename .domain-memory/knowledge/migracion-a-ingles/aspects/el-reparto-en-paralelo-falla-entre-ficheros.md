---
id: asp_el-reparto-en-paralelo-falla-entre-ficheros_tekr4a
slug: el-reparto-en-paralelo-falla-entre-ficheros
name: >-
  Repartir una traducción entre agentes en paralelo no falla dentro de un
  fichero: falla ENTRE ficheros, y el guardián del glosario no puede verlo
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:07:08.972Z'
updated_at: '2026-09-03T16:07:08.972Z'
last_verified: '2026-09-03T16:07:08.972Z'
file_paths:
  - crates/app/src/panel.rs
  - crates/app/src/measure.rs
  - crates/app/src/session.rs
  - crates/app/tests/glossary.rs
  - crates/app/tests/prose.rs
  - docs/02-decisiones.md
symbols:
  - set_aside
  - PanelTiling
  - Tiling
  - 'Action::SplitRight'
  - GLOSSARY_CLEAN_FILES
content_hashes:
  crates/app/src/panel.rs: 'sha256:0cd7f08563db761ba4ba5eaa9a622b5f190737996094a503ebb2f0914785c966'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
  crates/app/src/session.rs: 'sha256:b7824b96a3e438e7695ed87ccecbe9d65ed9dd16258b5e74ba3c39c8008f09e3'
  crates/app/tests/glossary.rs: 'sha256:bb3266feceb40eb1c3c4847deb16921f3432c141ca48f81cb8d8ab367e00fa32'
  crates/app/tests/prose.rs: 'sha256:b3b194d61079eaeef5626f13b7b1d59f308fe619c376a967181913fc0c47b441'
  docs/02-decisiones.md: 'sha256:4eb23bb4f11a5f60d8d236644be773469b8f5a617d1657294e0b9d39d4c9872a'
tags:
  - d27
  - f2.12
  - traduccion
  - glosario
  - guardas
  - fan-out
feature_id: feat_migracion-a-ingles_xnx8mc
---

## What it does

**Cada traductor fue coherente consigo mismo y ninguno lo fue con los demás.** Entrega #12 (prosa de `app` menos `lib.rs`, septiembre de 2026), siete traductores en paralelo. D27 decide que **la división es un `Pane` y `split` es el gesto**, y `split` estaba ya gastado en el proyecto (`Action::SplitRight`, `key.split-right`, `close-pane`):

| Fichero | «pane» (correcto) | «split» como sustantivo (violación) |
|---|---|---|
| `measure.rs` | 34 | **0** |
| `panel.rs` | **0** | **27** |
| otros nueve | mezcla | 14 |

Un revisor que abra `measure.rs` lo encuentra impecable. Uno que abra `panel.rs` lo encuentra roto de arriba abajo. **Comprobar dentro de un fichero no prueba nada sobre el reparto.**

Lo mismo con `reparto`, que el glosario mapea a `Tiling`: salió como «split» en 5 sitios y como **«division»** en 6 — justo la palabra que D27 retiró para la cosa. **El reparto resucitó el término retirado con otro significado**, que es peor que dejarlo en español: en español la guarda lo habría visto.

### Se delata en líneas contiguas, y es la única señal a simple vista

- `panel.rs:85` — *«Splits a split's rectangle»*, encima de `pub fn set_aside(pane: Rect, …)`. **El parámetro ya se llamaba `pane`.**
- `panel.rs:42` — *«the floor that decides whether a split can be split in two»*: la misma palabra para la cosa y para el gesto, en una oración.
- `panel.rs:53` — *«It is the floor of the **gesture**, not of the split»*: una frase cuyo único propósito es separar gesto de cosa, llamando a la cosa por el nombre del gesto.

### Por qué ninguna guarda lo caza, y esta es la mitad que importa

`glossary.rs` caza **nombres españoles retirados**. Una palabra **inglesa mal elegida** le es invisible por construcción: `split` es inglés, está en el glosario y es un identificador vivo. **La guarda estaba verde con 41 violaciones dentro.** Y la prosa le era invisible además por declaración — su cabecera decía que comentarios, rustdoc y literales quedan fuera de su alcance — así que hasta que se escribió `prose.rs` nada impedía devolver español a un fichero ya traducido.

> **La consecuencia operativa: después de un reparto en paralelo, el barrido va POR TÉRMINO Y ATRAVESANDO TODOS LOS FICHEROS, contando y no leyendo.**

### Y contar tiene su propia trampa: el sustantivo no es el verbo

El primer recuento dijo **60** sitios porque contaba «splits the batch», que es el **verbo** y es correcto. Eran **41**. Hay ~65 usos legítimos del verbo o del gerundio en los mismos ficheros — «splits the text», «can be split in two», «splitting requires a human gesture» — así que **un `sed` de `split`→`pane` corrompe más de lo que arregla**. Comprobado: un barrido anclado al determinante se coló igualmente en «the effects are panes and layouts», donde el original decía `repartos`, o sea «tilings».

### Corolario de método, del mismo caso

Lo que una entrega de traducción **afirma sobre sí misma** se verifica contando, no leyendo su brief. La #12 declaró «los identificadores ya eran ingleses» y había renombrado cuatro funciones de prueba; declaró **tres** islas de español y había **cinco** — las dos que faltaban eran correctas y útiles, y precisamente por no estar declaradas la entrega siguiente las habría roto sin aviso.

## Where it lives

**El glosario que decide**: `docs/02-decisiones.md`, anexo de D27 — `:3307` y `:3320` («la división es un `Pane`, y el gesto es `split`», con Ghostty citado como el terminal que usa `split` para la cosa, o sea la opción que la decisión contó y descartó), `:3343` y `:3353` (`Reparto`→`Tiling`), `:3401` (`Division`→`Pane`), `:3373` (las letras de tanda no se traducen).

**Los sitios que se delatan**: `crates/app/src/panel.rs:42`, `:53`, `:85`.

**Las guardas y su alcance**: `crates/app/tests/glossary.rs` (identificadores; su cabecera declara la prosa fuera) y `crates/app/tests/prose.rs` (la prosa, desde la #12).

**Las dos islas que no estaban declaradas**: `crates/app/src/keyboard.rs:1255-1258`, los nombres de acción españoles retirados que una guarda afirma que el terminal **rechaza**; y `crates/app/src/settings.rs:1668-1670`, fixtures del parser de comentarios.
