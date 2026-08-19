---
id: asp_el-molde-del-hermano-trae-sus-decisiones-y-sus-huecos_4vvrc2
slug: el-molde-del-hermano-trae-sus-decisiones-y-sus-huecos
name: >-
  Copiar la pieza hermana importa sus decisiones Y SUS HUECOS: seis de catorce
  hallazgos de una revisión, con el conocimiento ya fichado y sin aplicar
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-19T15:30:17.829Z'
updated_at: '2026-08-19T15:30:17.829Z'
last_verified: '2026-08-19T15:30:17.829Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - arrastrar_la_frontera_de_su_panel
  - arrastrar_la_frontera
content_hashes:
  crates/app/src/lib.rs: 'sha256:67dfdfc9bad7c0d618f018577eecd1de410625cc5cf9347f593360fff275852e'
tags:
  - metodo
  - revision
  - codigo-prestado
feature_id: feat_term-core_nx0g1r
---

## What it does

**Seis de los catorce hallazgos de una sola revisión salían de haber copiado la pieza hermana. Y el agravante: dos de ellos estaban YA FICHADOS en este almacén.**

En F2.4 la frontera del panel se escribió tomando como molde el arrastre de las fronteras del árbol, que llevaba meses funcionando. De ese molde vinieron **las cuatro guardas buenas** —no pedir el foco de teclado, mantener el cursor también al soltar, salir si no hay nada que repartir, no acumular el desplazamiento— y **también dos ausencias**:

- **No filtrar el botón del ratón.** En el molde es un defecto preexistente que nadie había notado; en el camino nuevo habría sido un defecto nuevo. **Estaba escrito** en [[un-arrastre-que-dura-varios-fotogramas-en-egui]], punto 3, con su ejemplo del pegado de X11.
- **Una guarda que no protege de nada.** La del valor sin número es necesaria en el molde porque su estado es una fracción; en el camino nuevo, que guarda enteros, es un mecanismo sin escenario — ver [[un-estado-en-enteros-no-necesita-guarda-de-nan-pero-si-techo]].

> **Lo que falla no es desconocer la regla.** La regla del proyecto ya dice que el código prestado lleva su razón y hay que nombrar qué lo hace aplicable a tu caso. Falla porque **un molde que funciona se lee como validado**, y la ausencia de una guarda en él se lee como «aquí no hace falta» en vez de como «nadie lo ha mirado». Copiar no es heredar decisiones: es heredar también los huecos, y los huecos no se ven porque no hay nada escrito donde debería haberlo.

**Y tener la ficha guardada tampoco basta.** El punto del botón llevaba meses en el almacén y se volvió a cometer, porque quien escribe el camino nuevo mira **la pieza hermana**, no la lista de trampas de la librería. La contramedida es de proceso y es concreta: **cada control de arrastre nuevo se repasa contra los cuatro puntos de esa ficha, uno por uno**, antes de darlo por hecho.

**Lo que sí funcionó para cazarlo:** un revisor **independiente** que fue al fuente de la librería a comprobar la semántica en vez de razonar sobre los nombres. Ninguna de las dos ausencias se ve leyendo el diff, porque el diff **se parece** a código bueno — es código bueno, copiado a un sitio donde una de sus decisiones ya no aplica y otra falta.

**Y el corolario para el defecto preexistente que aparece de paso:** se declara y se tiquetea aparte, nunca se arrastra al hito que lo descubrió ni se usa para bendecirlo. «**Allí tampoco se filtra**» no es un argumento — es la constatación de que hay dos defectos en vez de uno.

## Where it lives

El caso vive en `Division::arrastrar_la_frontera_de_su_panel` y en `Ventana::arrastrar_la_frontera` (`crates/app/src/lib.rs`): la primera es la copia, la segunda el molde. El defecto preexistente del molde —que sigue sin filtrar el botón— está declarado en el informe de revisión de F2.4 y tiene ticket propio.
