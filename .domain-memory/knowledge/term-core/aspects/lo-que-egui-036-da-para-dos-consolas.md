---
id: asp_lo-que-egui-036-da-para-dos-consolas_673gqh
slug: lo-que-egui-036-da-para-dos-consolas
name: Lo que egui 0.36 da y no da para poner dos consolas en una ventana
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-11T16:28:57.321Z'
updated_at: '2026-08-11T16:28:57.321Z'
last_verified: '2026-08-11T16:28:57.321Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/pintor.rs
symbols:
  - 'Ventana::ui'
  - 'Ventana::atender_clics'
  - repartir
  - 'pintor::pintar'
content_hashes:
  crates/app/src/lib.rs: 'sha256:bcf3ddb3e7b0c70d3d8b306bd58930520777e0baa28afa408bd8eb0b12ccd2ad'
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

Leído en el fuente de egui/eframe **0.36.1** en el registro local de cargo, no supuesto. Cinco cosas, y **cuatro son trampas de nombre**.

**`SidePanel` ya no existe.** Se fundió en `Panel::left/right/top/bottom` (`containers/panel.rs:206-258`). Quien busque el nombre viejo en documentación antigua no lo encuentra.

**Y el panel no sirve para repartir por fracción, que es la trampa que importa.** Su tamaño es `default_outer_size: Option<f32>` y `outer_size_range: Rangef`: **puntos, no fracción**. Con `.resizable(true)` trae el separador arrastrable ya hecho, y por eso parece la opción obvia — pero al redimensionar la ventana el panel conserva su ancho en puntos y **toda la diferencia cae en el otro lado**, así que la proporción se rompe sin que nadie toque el divisor. Para un reparto por fracción hay que calcular los rectángulos a mano, y es lo que se hizo.

**El recorte a un rectángulo existe, es barato y es híbrido.** `Ui::painter_at(rect)` (`ui.rs:619`) es `painter().with_clip_rect(rect)`: un `clone` con intersección de recortes, **cero coste por celda**. La teselación solo hace poda gruesa —y para texto, **por línea entera, no por glifo**—; el corte exacto a píxel lo hace el *scissor test* de la GPU. Importa porque el pintor de este proyecto **no recorta los glifos a su celda a propósito**, así que un carácter ancho al borde invadiría la consola vecina si nadie recorta por encima. Lo único que añade es coste **fijo**: cuatro grupos de recorte por fotograma en vez de uno, que el teselador agrupa en cuatro mallas.

**El «foco» de egui no es geográfico — es la trampa de nombre que más cuesta.** `Memory::request_focus` es foco de teclado **por widget**, para Tab y flechas. **No** sirve para decidir a qué pseudoterminal va una tecla: ese estado lo lleva la aplicación. Y de ahi se deriva la consecuencia práctica: al detectar el clic hay que usar **`Sense::CLICK` y no `Sense::click()`**, porque el segundo incluye `FOCUSABLE` y el widget *pediría* ese foco, compitiendo con el enrutado propio. El propio fuente lo dice (`sense.rs:58`).

**Arrastrar**: `Sense::drag()` más `Response::drag_delta()`, que da el desplazamiento desde el fotograma anterior. **El cursor no cambia solo**: hay que pedirlo a mano — el propio `Panel` lo hace así (`panel.rs:878`). Sin verificar en uso todavía: el divisor arrastrable es el PR que queda del hito.

**Y para probar la interacción sin abrir ventana:** `Context::run_ui(RawInput, |ui| ...)` (`context.rs:793`) entrega directamente un `&mut Ui` — no hace falta montar un panel. **Hacen falta dos pasadas**: egui resuelve la interacción contra lo que existía en la pasada anterior, así que un clic en la primera no encuentra a nadie a quien dárselo. Con eso, el criterio «un clic da el foco» se prueba con egui de verdad en un test normal.

**Cuándo dejaría de valer**: al subir de versión de egui. Todo esto es de la 0.36.1 y esta librería renombra y funde tipos entre versiones — `SidePanel` es la prueba, y `Context::run` → `run_ui` la segunda.

## Where it lives

El reparto a mano es la función `repartir` en `crates/app/src/lib.rs`, y el recorte por consola está en `Ventana::pintar_division`, que llama a `painter_at`. El clic con `Sense::CLICK` vive en `Ventana::atender_clics`, separado de la pasada de pintado precisamente para poder probarlo con `run_ui`. La precondición de que el pintor no recorta glifos está documentada en `crates/app/src/pintor.rs`.
