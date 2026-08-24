---
id: asp_la-expectativa-calculada-con-lo-que-se-muta-no-ata_mrkd01
slug: la-expectativa-calculada-con-lo-que-se-muta-no-ata
name: >-
  Tres formas nuevas de que una prueba de pintado pase sin atar nada: la
  expectativa calculada con lo que se muta, el máximo de dos, y el color que
  existe pero no se ve
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T15:24:02.373Z'
updated_at: '2026-08-24T15:24:02.373Z'
last_verified: '2026-08-24T15:24:02.373Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/tema.rs
  - crates/app/src/panel.rs
symbols:
  - 'Tema::color_del_reclamo'
  - 'Pestaña::reclamo'
  - 'Ventana::pintar_la_barra'
  - Reclamo
content_hashes:
  crates/app/src/lib.rs: 'sha256:b6bf814641dc6477356840fc727cffda19e824d7d5e80d702fefcf5161b30cf9'
  crates/app/src/tema.rs: 'sha256:802410da838730679f28ad3ee77a9b373271839c5af59055f591299f621c3f0c'
  crates/app/src/panel.rs: 'sha256:d9844bb19996c434945fd41689580cbaa88ac00d1919dea2597ba2c5346c7319'
tags:
  - pruebas
  - mutacion
  - pintado
  - egui
feature_id: feat_term-core_nx0g1r
---

## What it does

**Tres disfraces nuevos, los tres cazados por un revisor y ninguno visible leyendo el diff.** Salieron del mismo hito (F2.8, agosto de 2026) y ninguno es el disfraz ya fichado de [[una-prueba-que-ata-al-ayudante-no-ata-el-camino]]: en los tres, la prueba **sí entra por el camino real** —el pintado— y aun así no ata nada.

### 1. La expectativa calculada con la misma función que se muta

La prueba del pintado comparaba el color de cada símbolo contra `tema.color_del_reclamo(clase, sostenido)` — **la misma función cuya tabla se quería atar**. Cruzar el ámbar de `wait` con el rojo de `block`, o pintar `wait` con el gris del texto normal, dejaba **las 525 pruebas en verde**: el cambio se aplica a los dos lados de la igualdad y se cancela.

> **Una expectativa que se calcula llamando a lo que se prueba mide que el código es consistente consigo mismo, no que sea correcto.** El valor concreto —`t.paleta[3]`— tiene que estar escrito en la prueba al menos una vez.

Es la misma raíz que el disfraz nº 1 del proyecto (afirmar sobre el campo donde se apunta un valor en vez de sobre su efecto), con la vuelta de tuerca de que **aquí sí se afirma sobre el efecto**: sobre el color que llega a la cola de formas. Lo que falla es el otro lado de la comparación.

### 2. Con dos elementos, gana la posición y no la regla

Dos veces en el mismo hito, con dos mecanismos distintos:

- **El desempate.** «A igualdad de clase gana el reclamo que alguien sostiene» se probaba con el vivo **siempre en la segunda división**. `Iterator::max` devuelve **el último** de los empatados, así que el vivo ganaba **por orden**. Quitando el desempate del `Ord`, las nueve pruebas de la barra seguían verdes.
- **La acumulación.** «La ficha muestra lo que más reclaman **sus** divisiones» se probaba con pestañas de **dos**. Una pareja no distingue «todas» de «las dos primeras»: quedarse con las dos primeras dejaba **once** pruebas en verde.

> **Con dos elementos, media docena de formas de acumular mal dan el mismo resultado.** Tres elementos y **todas** sus ordenaciones; con dos, al menos los dos órdenes.

### 3. Un color puede existir en la cola de formas y ser invisible en pantalla

El símbolo de la pestaña activa se pintaba con su color correcto **sobre el relleno del foco**: contraste medido **1,01:1** el rojo del bloqueo y **1,02:1** el amarillo atenuado — el mismo color, a efectos de leerlo. La prueba afirmaba el color del texto y pasaba con el defecto puesto; el número de al lado ya se invertía por esta misma razón, escrita en su comentario.

> **En una prueba de pintado, afirmar el color de una forma no es afirmar que se vea: se ve o no según lo que haya DEBAJO, y eso es otra forma en la cola.** Donde un color cae sobre un relleno que la propia aplicación elige, la prueba tiene que mirar el par.

## Where it lives

Los tres se descubrieron revisando el PR #99 y el #100 de agent-terminal. El primero lo cazó un auditor de pruebas que mutó de verdad; el segundo, dos veces, un auditor de pruebas y uno de cobertura; el tercero, un revisor de código que **calculó el contraste** en vez de leer el color.

El arreglo de cada uno está en `crates/app/src/lib.rs` (`mod tests`) y en `crates/app/src/tema.rs`: `cada_reclamo_lleva_el_color_que_la_decision_le_asigno` clava cada clase a su entrada de la paleta; `entre_dos_reclamos_iguales_la_ficha_ensena_el_que_alguien_sostiene` recorre los dos órdenes y `con_tres_consolas_la_ficha_ensena_el_reclamo_que_manda` las seis ordenaciones de tres; y `el_simbolo_de_la_ficha_activa_se_pinta_sobre_el_fondo_y_no_sobre_el_realce` mira lo que hay debajo.

Relacionado: [[una-prueba-que-ata-al-ayudante-no-ata-el-camino]] (nueve veces, el disfraz vecino), [[una-prueba-de-arrastre-necesita-dos-pasadas-como-minimo]] (el escenario que no puede ocurrir) y [[plantar-se-ata-cronometrar-no-siempre]] (cuándo una prueba no puede sostener un tramo medido).
