---
id: asp_lo-que-egui-036-da-para-dos-consolas_673gqh
slug: lo-que-egui-036-da-para-dos-consolas
name: >-
  Lo que egui 0.36 da y no da para poner dos consolas en una ventana y un
  divisor entre ellas
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-11T16:28:57.321Z'
updated_at: '2026-08-11T19:51:22.263Z'
last_verified: '2026-08-11T19:51:22.263Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pintor.rs
symbols:
  - 'Ventana::ui'
  - 'Ventana::atender_el_raton'
  - 'Ventana::arrastrar_el_divisor'
  - repartir
  - 'pintor::pintar'
content_hashes:
  crates/app/src/lib.rs: 'sha256:c7f2d67be30a71cbe392f9c446013656f0c3443fb6f7d7035e8575837d3f7a1c'
  crates/app/src/pintor.rs: 'sha256:531c8fb585188fd0f14e2bfb6fe0d70a2a2be51c64f5cc5d8848f293ccb18ef8'
tags:
  - egui
  - eframe
  - f1.4
  - divisiones
  - api-ajena
feature_id: feat_term-core_nx0g1r
---

## What it does

Leído en el fuente de egui/eframe **0.36.1** en el registro local de cargo, no supuesto, y desde F1.4 **verificado también en uso**.

**`SidePanel` ya no existe.** Se fundió en `Panel::left/right/top/bottom` (`containers/panel.rs:206-258`). Quien busque el nombre viejo en documentación antigua no lo encuentra.

**Y el panel no sirve para repartir por fracción, que es la trampa que importa.** Su tamaño es `default_outer_size: Option<f32>` y `outer_size_range: Rangef`: **puntos, no fracción**. Con `.resizable(true)` trae el separador arrastrable ya hecho, y por eso parece la opción obvia — pero al redimensionar la ventana el panel conserva su ancho en puntos y **toda la diferencia cae en el otro lado**, así que la proporción se rompe sin que nadie toque el divisor. Para un reparto por fracción hay que calcular los rectángulos a mano, y es lo que se hizo.

**El recorte a un rectángulo existe, es barato y es híbrido.** `Ui::painter_at(rect)` (`ui.rs:619`) es `painter().with_clip_rect(rect)`: un `clone` con intersección de recortes, **cero coste por celda**. La teselación solo hace poda gruesa —y para texto, **por línea entera, no por glifo**—; el corte exacto a píxel lo hace el *scissor test* de la GPU. Importa porque el pintor de este proyecto **no recorta los glifos a su celda a propósito**, así que un carácter ancho al borde invadiría la consola vecina si nadie recorta por encima. Lo único que añade es coste **fijo**: cuatro grupos de recorte por fotograma en vez de uno.

**El «foco» de egui no es geográfico — es la trampa de nombre que más cuesta.** `Memory::request_focus` es foco de teclado **por widget**, para Tab y flechas. **No** sirve para decidir a qué pseudoterminal va una tecla: ese estado lo lleva la aplicación. De ahí se deriva que al detectar el clic haya que usar **`Sense::CLICK` y no `Sense::click()`**, porque el segundo incluye `FOCUSABLE` y el widget *pediría* ese foco, compitiendo con el enrutado propio (`sense.rs:58`).

## El arrastre, ya verificado en uso

- **`Sense::DRAG`, no `Sense::drag()`** — la misma trampa que el clic: `drag()` es `DRAG | FOCUSABLE` (`sense.rs:68`).
- **No sensar clic es una ventaja, no una carencia.** Un widget que sensa clic *y* arrastre paga una **ventana de decisión**: egui no sabe si el botón pulsado es un clic o un arrastre hasta que el puntero se mueve (`sense.rs:74-79`, `interaction.rs:196-217`). Siendo solo arrastre se entra en la rama `else` (`interaction.rs:218-221`) y se marca como arrastrado **en el fotograma mismo de la pulsación**. Para un divisor eso es exactamente lo que se quiere.
- **`Response::interact_pointer_pos()` da la posición ACTUAL del puntero, no la de la pulsación.** Verificado en la cadena: `context.rs:1534-1543` lo rellena desde `input.pointer.interact_pos()`, y `input_state/mod.rs:1119-1120` reasigna ese campo en **cada** `PointerMoved`. La posición de la pulsación existe pero es **otro** campo, `press_origin` (`input_state/mod.rs:1031`). Es lo que permite que el divisor persiga al ratón.
- **El arrastre se sigue por identificador, no por rectángulo**, y por eso un divisor que se mueve debajo del puntero no lo pierde: `interaction.rs:134` arranca de `prev_snapshot.dragged` y la búsqueda de candidato nuevo está dentro de `if dragged.is_none()` (`:191`). El propio egui comenta que un widget arrastrado puede estar «in the air» y no pasa nada. Solo lo cortan soltar el botón o Escape.
- **El candidato a arrastrarse sale de dónde estaba el puntero AL PULSAR** (`interaction.rs:169-171`), y `hits` se calcula una vez por pasada. Consecuencia para los tests: **hacen falta tres pasadas** — registrar, pulsar, mover— porque un pulsar-y-mover en la misma pasada busca el candidato en el destino, donde el divisor ya no está.
- **`on_hover_and_drag_cursor` frente a `on_hover_cursor`**: durante el arrastre `hovered()` es true **esté el puntero donde esté** (`interaction.rs:257-261` mete al arrastrado en la lista de «encima» sin mirar la posición), así que la razón que parece obvia —«el puntero se sale de la línea»— es falsa. Lo que sí cubre la variante con `drag` es el fotograma de la **suelta**, donde el arrastre ya terminó.
- **`interact_radius` (5,0, `style.rs:1482`) NO ensancha el agarre.** Solo puebla la lista de candidatos cercanos; los aciertos directos se buscan con radio 0,0, y con el puntero dentro de una consola egui descarta a propósito el arrastre cercano (`hit_test.rs:340-357`).
- **Pero el borde SÍ se comparte**: ver el caso 13 de [[la-api-ajena-se-lee-no-se-supone]]. En las dos columnas frontera del divisor, el gesto arrastra **y** da el foco.
- **Sin verificar, y depende de otro crate**: si el puntero sale de la ventana con el botón pulsado, la fuente deja abierto que el divisor se congele hasta soltar (`interaction.rs:233-236` limpia `potential_drag_id` pero no `dragged`). Comprobado a mano en Linux/winit: **no ocurre**. En otra plataforma habría que volver a mirarlo.

**Y una consecuencia de diseño que no es de egui pero la impone egui:** como la pasada puede correr **dos veces sobre la misma entrada** (`max_passes` = 2), la fracción del divisor **tiene** que calcularse desde la posición absoluta del puntero y no acumulando `drag_delta()`. Con el desplazamiento, el segundo pase lo aplicaría otra vez y el divisor se movió al doble. Es la misma trampa que obligó a **consumir** los eventos de teclado en vez de leerlos.

## Probar la interfaz sin abrir ventana — se puede entera, no a trozos

- `Context::run_ui(RawInput, |ui| ...)` (`context.rs:793`) entrega directamente un `&mut Ui`, sin montar panel. **Hacen falta dos pasadas** para un clic, porque egui resuelve la interacción contra lo que existía en la anterior.
- **Y la pasada de interfaz COMPLETA también se puede correr**, que es lo que se creía imposible: `eframe::Frame::_new_kittest()` (`epi.rs:720`) construye un `Frame` vacío, es público y sin `cfg`, y `App::ui` no lo toca. El área entra por `RawInput::screen_rect`, que es exactamente de donde sale `ui.max_rect()` en la aplicación (`context.rs:801-807` → `input_state/mod.rs:526`). Con eso se prueba **el cableado**, no solo las piezas: que la pasada reparta con la fracción del estado, que llame al teclado y al clic, y que `logic` aplique lo que la pasada midió.
- `run_ui` repite la pasada **solo** si alguien pide descartar (`context.rs:863-865`), así que en un test corre una vez — importa porque los eventos de teclado se consumen y una segunda vuelta no vería la tecla.
- Las fuentes existen desde la primera pasada (`context.rs:1120`), así que `Metrica::medir` se puede llamar **desde fuera** de la pasada para calcular lo esperado, sin correr una pasada extra que ensuciaría el estado de interacción de un arrastre a medias.
- El guion bajo de `_new_kittest` es una advertencia: es un nombre que eframe se reserva y puede desaparecer en una versión menor. Se acepta porque el precio de perderlo es un error de compilación **en las pruebas**, nunca en lo que se entrega.

**Cuándo dejaría de valer**: al subir de versión de egui. Todo esto es de la 0.36.1 y esta librería renombra y funde tipos entre versiones — `SidePanel` es la prueba, y `Context::run` → `run_ui` la segunda.

## Where it lives

El reparto a mano es la función `repartir` en `crates/app/src/lib.rs`; el recorte por consola está en `Ventana::pintar_division`, que llama a `painter_at`. El clic vive en `Ventana::atender_el_raton` —que desde F2.3 atiende también el arrastre de una consola sobre otra; antes se llamaba `atender_clics`— y el arrastre de la frontera en `Ventana::arrastrar_la_frontera`, los dos separados de la pasada de pintado precisamente para poder probarlos con `run_ui`. Las pruebas que corren la pasada **entera** usan el helper `pasada_de_interfaz` del módulo de tests de ese mismo fichero. La precondición de que el pintor no recorta glifos está documentada en `crates/app/src/pintor.rs`.
