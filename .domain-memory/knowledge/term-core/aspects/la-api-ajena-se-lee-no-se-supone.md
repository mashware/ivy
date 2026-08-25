---
id: asp_la-api-ajena-se-lee-no-se-supone_hnvyp8
slug: la-api-ajena-se-lee-no-se-supone
name: >-
  La API ajena se lee, no se supone: en este proyecto suponerla ha costado ya
  dieciséis veces
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-09T20:07:25.783Z'
updated_at: '2026-08-25T11:40:00.000Z'
last_verified: '2026-08-11T19:50:20.710Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/medida.rs
  - crates/app/src/pintor.rs
  - crates/term-core/src/lib.rs
symbols:
  - 'Painter::text'
  - 'IntegrationInfo::cpu_usage'
  - 'RawInput::time'
  - 'Pty::spawn'
  - 'Sense::click'
  - 'Context::run_ui'
  - 'Rect::distance_sq_to_pos'
  - 'Frame::_new_kittest'
content_hashes:
  crates/app/src/lib.rs: 'sha256:c7f2d67be30a71cbe392f9c446013656f0c3443fb6f7d7035e8575837d3f7a1c'
  crates/app/src/medida.rs: 'sha256:b55a4357bfb9ee09793b07a352ac7f74bdcfbbd12c25527fe829182eaadcec3c'
  crates/app/src/pintor.rs: 'sha256:531c8fb585188fd0f14e2bfb6fe0d70a2a2be51c64f5cc5d8848f293ccb18ef8'
  crates/term-core/src/lib.rs: 'sha256:efa66ec48f1e9c4ba5f5d05657492ca563960b543264d613c06cd094b0818c73'
tags:
  - metodo
  - verificacion
  - egui
  - eframe
  - f1.3
  - f1.4
feature_id: feat_term-core_nx0g1r
---

## What it does

**El patrón, con su cuenta.** **Dieciséis** veces en este proyecto una librería de terceros ha hecho algo distinto de lo que su nombre prometía, y **las dieciséis se resolvieron leyendo su código fuente**, nunca razonando sobre él. No es mala suerte: es lo que pasa cuando se construye un terminal sobre un motor de VT, un pseudoterminal y un framework de interfaz de modo inmediato, tres capas cuyas abstracciones fueron diseñadas para otra cosa.

**Los casos, para que la cuenta no sea una impresión:**

1. `App::update` **no existe** en eframe 0.36 — el método es `ui`, y hay un `logic` que corre también con la ventana oculta. Cambió la forma entera del bucle.
2. `Receiver` es `!Sync`: prestarlo no cruza de hilo, moverlo sí. Cambió el contrato de `Pty::spawn`, que pasó a devolver el canal.
3. `RawInput::time` está documentado como «monótono, **relativo a lo que sea**» y los eventos de tecla no traen sello propio. Por eso no existe un número llamado «tecla → píxel».
4. **`Painter::rect_filled` y `Painter::text` no pintan: apilan formas y vuelven.** Teselar ocurre después de la pasada. El banco publicaba que el fotograma gastaba el 14 % de su hueco; gastaba el 33 %.
5. `IntegrationInfo::cpu_usage` cronometra **la pasada entera**, no solo el dibujo. El desglose de latencia llamaba «teselar y dibujar» a un residuo que incluía media docena de cosas más.
6. `viewport_rect` vale **10 000 × 10 000 puntos** cuando la ventana todavía no reporta tamaño. Sin techo, eso son cientos de miles de celdas y un aviso de tamaño absurdo al programa.
7. `max_passes` vale **2**: un fotograma puede correr la pasada dos veces sobre la misma entrada, y la misma tecla se escribiría dos veces al pseudoterminal.
8. `egui-winit` convierte Ctrl+C en un evento de «copiar» **antes** de emitir la tecla. Sin recogerlo por esa puerta, **Ctrl+C no interrumpe**, que es lo más básico de un terminal.
9. `portable-pty` copia el entorno entero del padre al hijo, y no tiene parámetro para evitarlo.
10. Alimentar el parser con un solo carácter repetido recorre su camino rápido y **da un número que miente** — el mismo aprendizaje, dos veces, en dos instrumentos distintos.
11. **`Sense::click()` incluye `FOCUSABLE`** (`sense.rs:60`), o sea que el widget *pide* el foco de teclado de egui. En una aplicación cuyo enrutado de teclado es **geográfico y propio**, ese foco compite con el de la aplicación. El propio fuente lo dice: *«Use `Sense::CLICK` if you don't want the widget to be focusable»* (`sense.rs:58`). Cazado **antes** de escribir, leyendo la fuente.
12. **`Context::run` no existe en 0.36.1: es `run_ui`** (`context.rs:793`), y entrega directamente un `&mut Ui`, sin necesidad de montar un panel. Cazado por el compilador, que fue barato; pero el mismo despiste sobre un método que **sí** existiera con otra semántica habría sido del tipo 4 o 5.
13. **`Rect::distance_sq_to_pos` da cero para un punto *sobre* el borde**, no solo dentro. Así que **dos rectángulos que se tocan reciben los dos el acierto directo**: `hit_test.rs:205-211` los busca con radio 0,0 y `find_closest_within` compara con `<=` (`hit_test.rs:414-433`). Consecuencia real: en las dos columnas frontera del divisor arrastrable, un mismo gesto **arrastra el divisor y da el foco a la consola de al lado** — `hit_test.rs:377-386` devuelve los dos aciertos a propósito. Un comentario del código afirmaba que no ensanchar la zona de agarre evitaba eso; era falso, y lo destapó la revisión leyendo la fuente. El comportamiento se dejó (pulsar en el borde de una consola y darle el foco es lo esperable); lo que se quitó fue la garantía inventada.
14. **`eframe::Frame` sí se puede construir en un test: `Frame::_new_kittest()`** (`epi.rs:720`), público, sin ninguna condición de compilación, y con la documentación diciendo que existe para probar aplicaciones. Y `App::ui` **no toca el `Frame`** — su parámetro se llama `_frame`. Este caso es distinto de los doce primeros y por eso es el más incómodo: **no se supuso lo que hacía una llamada, se supuso que la llamada no existía**, y se escribió en un artefacto de revisión que probar la pasada de interfaz era «una limitación real del framework». Cinco pruebas que tapaban mutilaciones capaces de dejar la ventana muda estuvieron sin escribir por eso.

15. **Los tres modos de ratón del motor de VT se SUSTITUYEN, no se acumulan.** Pedir el arrastre (`?1002h`) **apaga** el de solo clics (`?1000h`), y un programa que enciende los tres seguidos acaba con uno. La intuición contraria —que son banderas independientes que se suman— se escribió en una prueba de la superficie pública **y la prueba falló**, que es como se descubrió. Importa porque quien lee esas banderas decide con un «¿lo quiere para algo?» que las mira todas: si se acumularan, apagar el arrastre dejaría los clics encendidos y la consola no recuperaría la selección.
16. **`ALTERNATE_SCROLL` viene ENCENDIDA en el modo por defecto del motor** (`TermMode::default()`), igual que hace `xterm`. Así que lo que de verdad decide si la rueda se traduce a flechas es **la pantalla alterna**, no esa bandera. La primera prueba se escribió suponiendo lo contrario —«hacen falta las dos»— y falló en la primera corrida. La bandera se sigue preguntando porque un programa **puede apagarla**, y `vim` lo hace en cuanto quiere el ratón para sí.

**Dónde salta la alarma.** El caso caro no es el que no compila: es aquel en que **la etiqueta de un número describe lo que uno cree que hace la llamada**. Los casos 4 y 5 son el mismo error con signos opuestos — llamar «pintar» a encolar, y llamar «teselar» a un residuo— y los dos publicaban un número optimista con toda la apariencia de ser el bueno, en el hito cuyo producto **es** el número. Ninguno de los dos lo habría detectado un test: los dos números eran internamente coherentes.

**Tres patrones, y el tercero es nuevo:**

- **Constructores de conveniencia que empaquetan una decisión** (casos 11 y 12). `Sense::click()` no es «sensar clics», es «sensar clics *y además* pedir foco». Cuando una API ofrece un constructor corto y otro explícito, **el corto lleva dentro una elección que alguien tomó para otro caso de uso** — mirar qué es antes de aceptarla. Se repitió con `Sense::drag()`, que es `DRAG | FOCUSABLE` (`sense.rs:68`), aunque esa vez ya se sabía qué buscar.
- **Geometría que no se comporta como la intuición** (caso 13). Un borde compartido no es una frontera que separe: para el motor de aciertos pertenece a los dos lados.
- **Suponer que unas banderas se suman** (casos 15 y 16). Un conjunto de modos con nombres parecidos parece un conjunto de interruptores independientes, y a veces es una **máquina de estados**: encender uno apaga otro. Y un modo puede venir **encendido de fábrica**, con lo que la condición que uno cree estar comprobando ya se cumple siempre. Las dos veces lo destapó **una prueba escrita desde la intuición, que falló** — que es la forma barata de descubrirlo, y la razón de escribir la prueba antes de fiarse.
- **Suponer una ausencia, no una semántica** (caso 14). «Esto no se puede hacer» es una afirmación sobre código ajeno igual que «esto hace X», y necesita la misma prueba. Un límite declarado con honestidad vale más que una prueba retorcida — pero solo si antes se ha comprobado que es un límite.

**Qué hacer, en concreto.** Las fuentes están en `~/.cargo/registry/src/index.crates.io-*/<caja>-<versión>/`, y leerlas cuesta una búsqueda. Se leen **antes** de apoyarse en el comportamiento, no después de que el número salga raro, siempre que: un número que se va a publicar dependa de cuándo ocurre algo; un contrato dependa de qué hace exactamente una llamada; se esté a punto de escribir en un comentario «esto hace X» sobre código ajeno; se vaya a usar un **constructor de conveniencia** de una API que también ofrece la versión explícita; o se esté a punto de **declarar que algo no se puede hacer**.

**Y quién lo caza cuando se escapa:** en cinco de los dieciséis casos lo destapó una revisión que fue a leer la fuente en vez de opinar sobre el diff. Es el argumento más fuerte a favor de pedirle a quien revisa que **verifique contra el código de la librería**, no contra el razonamiento del autor.

## Where it lives

Los casos vividos están repartidos por `crates/app/` —`lib.rs` (el bucle, el teclado, el foco por clic, el divisor arrastrable y las pruebas de la pasada de interfaz), `medida.rs` (los cronómetros), `pintor.rs` (la medida de la rejilla)— y `crates/term-core/src/lib.rs` (el pseudoterminal y su canal). Los casos 1 a 10 salieron en F1.3; del 11 al 14 en F1.4, al poner dos consolas con foco por clic y divisor arrastrable. El relato de cada uno, con la línea de fuente que lo demuestra, está en los registros de revisión de esos hitos. Los casos 15 y 16 salieron en F2.6, al preguntarle al motor qué ha pedido el programa sobre el ratón: viven en `crates/term-core/src/screen.rs` y su decisión es **D23**.
