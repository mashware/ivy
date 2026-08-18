---
id: asp_plantar-se-ata-cronometrar-no-siempre_38pyam
slug: plantar-se-ata-cronometrar-no-siempre
name: >-
  Si el trabajo se puede PLANTAR se ata con una prueba; si solo se puede
  cronometrar, lo que decide es DÓNDE se pone el cronómetro
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T16:09:00.137Z'
updated_at: '2026-08-17T20:08:23.772Z'
last_verified: '2026-08-17T20:08:23.772Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - rearmar_el_vigilante
  - cronometrado_aparte
  - llevarse_lo_de_este_fotograma
  - atender_el_raton
  - atender_el_raton_sin_declararse
content_hashes:
  crates/app/src/lib.rs: 'sha256:bd1d056ad4bcd72e1edfbea41ecd1ed51b925068d136aabb02b79da1032b45e3'
tags:
  - medida
  - pruebas
  - mutacion
  - F2.2
  - F2.3
feature_id: feat_term-core_nx0g1r
---

## What it does

**La regla del proyecto es que todo trabajo no declarado se publica como coste del framework** —el fotograma se reparte por resta— y el arreglo típico es mover la raya del cronómetro para que encierre el trabajo. Lo que este caso enseña es **cuándo ese arreglo se puede probar y cómo**, y conviene saberlo antes de escribir la prueba.

**Para probarlo hay que comparar lo declarado con un reloj de pared tomado por FUERA de la operación**, y entre los dos relojes cabe una expropiación del planificador. Medido el 17 de agosto de 2026 con `rearmar_el_vigilante` en abrir y cerrar pestaña: en la suite un rearme cuesta **microsegundos**, o sea **lo mismo que el hueco entre los dos relojes**. Señal y ruido del mismo orden no se distinguen. Tres formulaciones —margen de medio rearme, calibrar con el mínimo de cinco medidas, aceptar uno de ocho intentos— y las tres descartadas: o frágiles, o con las dos mutaciones vivas.

**Se quitó la prueba.** Una prueba que no cae al romper a propósito lo que dice cubrir no cubre nada, y dejarla es peor que no tenerla. Es la segunda vez en el mismo hito —ver [[la-vuelta-de-medida-se-cierra-al-pintar]]— que una prueba que **parecía más fuerte era más débil**.

> **Regla práctica: si el trabajo se puede PLANTAR, se ata con una prueba; si solo se puede CRONOMETRAR y cuesta lo mismo que el ruido, el sostén es estructural.**

**Lo que sí se ata:** `cerrar_una_pestaña_no_se_lleva_lo_que_sus_consolas_habían_hecho` **planta** tres duraciones grandes y distintas —cinco, tres y dos segundos— en los tres sitios donde una consola guarda trabajo sin declarar, y afirma que lo declarado supera la suma. Grandes **y distintas** a propósito: con el mismo número en los tres, tirar uno seguiría dando de sobra.

---

## Ampliación (F2.3, issue 76): no es solo SI se puede probar, es DÓNDE va el cronómetro

Hay **tres sitios** y solo el tercero se ata entero:

| Dónde | Qué caza una prueba |
|---|---|
| **En quien llama** (el estilo que ya había) | **Nada.** Una prueba que llame a la función no toca el contador. Comprobado mutándolo: quitar la línea deja la suite entera en verde |
| **Dentro, con el cuerpo en la misma función** | Que la línea **desaparezca**, sí. Que la región **se encoja**, no: subir la primera lectura del reloj unas líneas publica casi todo el trabajo como coste del framework y la prueba sigue verde, porque «mayor que cero» lo cumple cualquier par de lecturas seguidas. Es el segundo disfraz fichado —un cero con el que el defecto y el arreglo dan lo mismo— y lo cachó la revisión de código, no quien escribió la prueba |
| **Dentro, y el trabajo en OTRA función** | Todo. La región cronometrada es la llamada entera, así que encogerla deja de ser mover una línea y pasa a ser sacar código de una función — visible en cualquier revisión |

> **La prueba ata que la declaración EXISTA; la estructura ata que CUBRA TODO. Ninguna de las dos sola basta.**

**Y ojo con el envoltorio:** partir la función crea justo la forma contra la que avisa [[una-prueba-que-ata-al-ayudante-no-ata-el-camino]]. Aquí no muerde porque **las pruebas entran por la función de fuera**, que es la misma puerta que usa producción. Si alguna entrara por la de dentro, el hueco volvería.

**Un corolario de la misma familia:** una prueba que saca su número **de la propia constante que prueba** es un espejo, no una prueba. Y **un aviso de método que costó rehacer trabajo**: un guion de mutación que revierte con `git checkout --` se lleva por delante lo que no esté comprometido, y **un parche que no llega a casar se lee igual que una mutación que sobrevive** — el guion tiene que comprobar que el fichero cambia antes de dar la mutación por buena. Las dos cosas pasaron.

## Where it lives

`crates/app/src/lib.rs` — `cronometrado_aparte`; los cuatro sitios que llaman a `rearmar_el_vigilante`; `Division::llevarse_lo_de_este_fotograma`, que es la mitad que sí se planta; y desde F2.3, `Ventana::atender_el_raton` (el cronómetro) sobre `atender_el_raton_sin_declararse` (el trabajo). Aprendido en el PR #75 de F2.2 y ampliado en el PR #77 de F2.3.
