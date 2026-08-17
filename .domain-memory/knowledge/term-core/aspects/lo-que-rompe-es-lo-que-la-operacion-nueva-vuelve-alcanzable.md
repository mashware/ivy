---
id: asp_lo-que-rompe-es-lo-que-la-operacion-nueva-vuelve-alcanzable_d1y48j
slug: lo-que-rompe-es-lo-que-la-operacion-nueva-vuelve-alcanzable
name: >-
  Lo que rompe al añadir una operación no es su código: son los defectos que ya
  estaban y que ella vuelve alcanzables — 13 de 14 bloqueantes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T23:13:05.089Z'
updated_at: '2026-08-16T23:13:05.089Z'
last_verified: '2026-08-16T23:13:05.089Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/medida.rs
symbols:
  - 'Nodo::partir'
  - 'Pestaña::mover_en'
  - 'Ventana::publicar_informe'
  - 'medida::lado'
content_hashes:
  crates/app/src/lib.rs: 'sha256:01adf2fc83535e392e34a240d6ed26b7af67123641006bbb510b9f94b9c87f5a'
  crates/app/src/medida.rs: 'sha256:627412ac283ad6138a2b514940f63e45eb84d875a0405dd62991852b0442a69c'
tags:
  - revisión
  - riesgo
  - árbol-de-divisiones
  - F2.2
feature_id: feat_term-core_nx0g1r
---

## What it does

**El patrón, con su cuenta.** El PR que enseñó a partir una división en dos cerró **catorce hallazgos bloqueantes**, y **trece estaban en código que ese PR no escribe**. Ninguno era un defecto nuevo: eran estados que hasta entonces **no se podían alcanzar desde la aplicación**, y la operación nueva los puso a un gesto de distancia.

**Los tres que más enseñan:**

1. **El informe entraba en pánico con tres divisiones.** El título de cada tabla de tramos indexaba un array de dos lados —«izquierda», «derecha»— por la posición. Con dos divisiones fijas era correcto por construcción; con tres, es un pánico **dentro del informe que se publica al salir**, o sea perdiendo de golpe toda la sesión medida. El mismo defecto ya se había arreglado en la cabecera un PR antes, y siguió vivo en el título porque nadie había podido llegar a tres.

2. **Arrastrar una frontera movía la del subárbol hermano.** La búsqueda de la frontera por su número usaba el booleano de «el peso cambió» como si dijera «la he encontrado». Encontrarla y no moverla —el fotograma en que el puntero se queda quieto— se leía como «no está aquí», la búsqueda seguía por la rama siguiente, y como la cuenta no había avanzado, la del hermano casaba con el mismo número. **Inalcanzable mientras la disposición fue una rama de dos hojas**: hacían falta dos ramas hermanas, o sea dos particiones.

3. **Una premisa del diseño resultó ser falsa.** El diseño descartó dar identidad estable a cada frontera con este argumento: «la posición solo cambia al partir o cerrar, y ninguna de las dos cosas puede ocurrir en medio de un arrastre — es un gesto a la vez». **Son dos manos**: con el ratón apretado sobre un divisor y el acorde de partir con la otra, las fronteras se renumeran y el arrastre en vuelo pasa a mover otra, porque el identificador de control de egui es **posicional**.

> **La regla: al añadir una operación, la pregunta no es «¿qué puede fallar en lo que escribo?» sino «¿qué estados pasan a ser alcanzables por primera vez, y quién los estaba dando por imposibles?».** Todo el código que decía «esto no puede pasar» —por el tipo, por el número fijo, por el gesto único— hay que releerlo con la operación nueva delante.

**Y cómo se encuentran, porque no salen leyendo el diff:** dos de los tres salieron **mutando** (cortar el descenso recursivo dejaba doce pruebas verdes) y el tercero de un revisor externo que lo reprodujo con una copia aislada de la función. Leer el cambio no basta: el defecto está en el fichero que el cambio **no toca**.

## Where it lives

`crates/app/src/lib.rs` — `Nodo::partir` (el descenso), `Pestaña::mover_en` (la búsqueda de frontera), `Ventana::publicar_informe` (el título de cada tabla); `crates/app/src/medida.rs` — `lado()`, la regla de cuándo el lado describe algo. Vivido en F2.2, PR #73.
