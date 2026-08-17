---
id: asp_lo-que-rompe-es-lo-que-la-operacion-nueva-vuelve-alcanzable_d1y48j
slug: lo-que-rompe-es-lo-que-la-operacion-nueva-vuelve-alcanzable
name: >-
  Lo que rompe al añadir una operación no es su código: son los defectos que ya
  estaban y que ella vuelve alcanzables — 16 de 17 bloqueantes en dos PRs
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T23:13:05.089Z'
updated_at: '2026-08-17T12:00:32.902Z'
last_verified: '2026-08-17T12:00:32.902Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/medida.rs
symbols:
  - 'Nodo::partir'
  - 'Nodo::colapsar'
  - 'Pestaña::mover_en'
  - 'Ventana::publicar_informe'
  - 'Ventana::cerrar_la_del_foco'
  - 'medida::lado'
  - 'Ventana::cerrada_alguna_a_mano'
content_hashes:
  crates/app/src/lib.rs: 'sha256:69e903da2ae25795351e9b8a05001efaad7c4703b59964f4e430a94a6176c041'
  crates/app/src/medida.rs: 'sha256:627412ac283ad6138a2b514940f63e45eb84d875a0405dd62991852b0442a69c'
tags:
  - revisión
  - riesgo
  - árbol-de-divisiones
  - F2.2
feature_id: feat_term-core_nx0g1r
---

## What it does

**El patrón, con su cuenta, y ya son dos PRs seguidos.** El PR que enseñó a **partir** una división cerró **catorce hallazgos bloqueantes** y **trece estaban en código que ese PR no escribe**. El que enseñó a **cerrar** cerró **tres** y **los tres** estaban fuera de su código. Trece más tres son **dieciséis de diecisiete**; el título de esta ficha dijo «15 de 17» durante un día, y lo corrigió el PR que cierra el hito al ir a citarlo en una decisión versionada — la aritmética de una ficha también se comprueba antes de publicarla. Ninguno era un defecto nuevo: eran estados que hasta entonces **no se podían alcanzar desde la aplicación**, y la operación nueva los puso a un gesto de distancia.

**De partir, los tres que más enseñan:**

1. **El informe entraba en pánico con tres divisiones.** El título de cada tabla de tramos indexaba un array de dos lados —«izquierda», «derecha»— por la posición. Con dos divisiones fijas era correcto por construcción; con tres, es un pánico **dentro del informe que se publica al salir**, o sea perdiendo de golpe toda la sesión medida. El mismo defecto ya se había arreglado en la cabecera un PR antes, y siguió vivo en el título porque nadie había podido llegar a tres.

2. **Arrastrar una frontera movía la del subárbol hermano.** La búsqueda de la frontera por su número usaba el booleano de «el peso cambió» como si dijera «la he encontrado». Encontrarla y no moverla —el fotograma en que el puntero se queda quieto— se leía como «no está aquí», la búsqueda seguía por la rama siguiente, y como la cuenta no había avanzado, la del hermano casaba con el mismo número. **Inalcanzable mientras la disposición fue una rama de dos hojas**: hacían falta dos ramas hermanas, o sea dos particiones.

3. **Una premisa del diseño resultó ser falsa.** El diseño descartó dar identidad estable a cada frontera con este argumento: «la posición solo cambia al partir o cerrar, y ninguna de las dos cosas puede ocurrir en medio de un arrastre — es un gesto a la vez». **Son dos manos**: con el ratón apretado sobre un divisor y el acorde de partir con la otra, las fronteras se renumeran y el arrastre en vuelo pasa a mover otra, porque el identificador de control de egui es **posicional**.

**De cerrar, los dos que el patrón predice y nadie vio leyendo el cambio:**

4. **La ventana se cerraba sola llevándose lo que se estaba leyendo.** La regla «la ventana se cierra cuando han muerto todos los programas» tiene una excepción de F2.1: si el usuario ha cerrado algo **a mano**, gobierna él. Esa bandera la levantaba cerrar una pestaña, y cerrar una **división** no la levantaba. Con una consola ya terminada al lado de la del agente, cerrar la del agente dejaba «todo muerto» y **la ventana desaparecía con el contenido que se estaba mirando**. La asimetría estaba a la vista en el código nuevo: cerrar la **única** consola de una pestaña delega en cerrar la pestaña, que sí la levanta — o sea que **el mismo gesto protegía o no según cuántas hermanas tuviera la consola**.

5. **Un texto del informe pasó a mentir sin que nadie lo tocara.** Cuando la pista de lado no aplica, el informe decía «sin lado, que **con más de dos** ya no describe nada». Mientras una pestaña tuvo siempre dos consolas o más —nacía con dos y partir solo añadía— era verdad. **Cerrar hace alcanzable la pestaña de una**, y ahí esa línea afirma lo contrario de lo que pasa. Mismo sitio, mismo mecanismo y mismo PR de origen que el caso 1: **el informe es donde este patrón golpea más veces**, porque está lleno de frases que describen la geometría de entonces.

> **La regla: al añadir una operación, la pregunta no es «¿qué puede fallar en lo que escribo?» sino «¿qué estados pasan a ser alcanzables por primera vez, y quién los estaba dando por imposibles?».** Todo el código que decía «esto no puede pasar» —por el tipo, por el número fijo, por el gesto único— hay que releerlo con la operación nueva delante. Y no solo el código: **las frases**. Un mensaje que explica por qué algo no aplica lleva dentro una suposición sobre cuántos había.

**Y cómo se encuentran, porque no salen leyendo el diff:** de partir, dos de tres salieron **mutando** (cortar el descenso recursivo dejaba doce pruebas verdes) y el tercero de un revisor externo. De cerrar, los dos los trajo **el único revisor de cuatro que entregó informe**, y ninguno lo había visto quien escribió el cambio repasando su propio código. Leer el cambio no basta: el defecto está en el fichero que el cambio **no toca**.

## Where it lives

`crates/app/src/lib.rs` — `Nodo::partir` (el descenso), `Pestaña::mover_en` (la búsqueda de frontera), `Ventana::publicar_informe` (el título de cada tabla y la línea del foco), `Ventana::cerrar_la_del_foco` y la bandera `cerrada_alguna_a_mano`; `crates/app/src/medida.rs` — `lado()`, la regla de cuándo el lado describe algo. Vivido en F2.2, PRs #73 (partir) y #74 (cerrar).
