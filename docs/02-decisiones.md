# Decisiones

Cada una con su razón y lo que se descartó. Si una decisión cambia, se edita aquí y se dice por qué.

---

## D1 — Multiplataforma real (Linux, macOS, Windows)

**Decidido.** No es una app personal para Linux: hay demasiada gente en Mac y Windows como
para descartarla.

**Consecuencia:** descarta GTK4 + VTE, que era la opción técnicamente más cómoda en Linux
(ver D3).

**Confirmado por el panorama, agosto 2026.** De los cuatro competidores analizados, tres son **solo
macOS** (Xirp, Conductor, y Superset salvo un AppImage experimental). El único que corre en las tres
plataformas es Warp — y es también el único que cumple todas las demás condiciones
([07-panorama.md](07-panorama.md) → E5). Que la multiplataforma sea rara en esta categoría no
debilita D1: la señala como lo que efectivamente distingue al rival más fuerte del resto.

---

## D2 — El motor de terminal se consume como librería

**Decidido:** crate **`alacritty_terminal`** para el parseo VT y el estado, **`portable-pty`**
para el PTY (abstrae Unix y ConPTY de Windows).

**Por qué:** escribir un emulador de terminal es años-persona — parser VT con décadas de casos
borde, scrollback, reflow, shaping, IME. Pero ya no hace falta: **Zed lo consume exactamente
así** y pinta encima con su propio framework.

**Descartado:**
- **VTE** — solo GTK/Linux; además sin ligaduras de fuente ([issue #192](https://gitlab.gnome.org/GNOME/vte/-/issues/192), abierto hace años) ni protocolo gráfico de Kitty.
- **Escribirlo nosotros** — es lo que hicieron Warp y JetBrains, ambos con equipos a sueldo.

---

## D3 — Rust + GPUI para la interfaz

**Decidido:** Rust, con **GPUI** para el producto y **egui** como opción para prototipar rápido.

**Por qué:** es el camino de Zed para este caso exacto (terminal embebido, GPU, muchos paneles,
tres plataformas), y Rust es donde vive el ecosistema de esta pieza concreta.

**Riesgo asumido:** GPUI tiene API inestable y poca documentación fuera de Zed. Si se hace
cuesta arriba, egui o Iced.

**Candidato nuevo, no decidido — `warpui`, agosto 2026.** Warp liberó su cliente el 30 de abril de
2026 y **el reparto de licencias no es uniforme**; textual de su README:

> Warp's UI framework (the `warpui_core` and `warpui` crates) are licensed under the **MIT license**.
> The rest of the code in this repository is licensed under the **AGPL v3**.

Verificado en el `Cargo.toml` de los dos crates (`license = "MIT"`). Eso pone sobre la mesa un
framework de interfaz en Rust, con GPU, licencia permisiva y **ya probado en un terminal
multiplataforma de verdad** — exactamente el hueco que el riesgo de arriba describe para GPUI.

**No cambia la decisión, y no debe cambiarla nadie sin medir.** Esta decisión se toma con números
—así se tomó el descarte de Electron y así se juzgó egui— y aquí no hay ninguno todavía. Dos cosas
hay que comprobar antes de que esto sea siquiera una opción:

1. **Que `warpui` se pueda usar suelto.** Está en el mismo repositorio que el código AGPL y puede
   arrastrar dependencias de esos crates; si las arrastra, la licencia MIT del crate no sirve de nada.
2. **Que del resto no se copie nada.** El resto del repositorio es AGPLv3 y este proyecto no lo es.

Ver [07-panorama.md](07-panorama.md) → E5 para el contexto de producto.

**Descartado — Electron/Tauri + xterm.js.** Era lo más barato en esfuerzo (stack conocido), pero
tiene un techo justo en nuestro caso de uso. Del propio issue de xterm.js
[«Support dozens of terminals on a single page»](https://github.com/xtermjs/xterm.js/issues/4379):

- Los navegadores limitan los contextos WebGL activos (~una docena). Dicen literalmente que los
  multiplexores necesitan decenas de paneles y **el navegador no lo soporta**.
- Los terminales compiten por el hilo principal: **2 instancias → 30 fps, 4 → 15 fps**.

Eso describe exactamente «varios agentes escribiendo a la vez». VS Code no sufre porque ahí
sueles tener uno o dos terminales activos, no ocho.

### Medido — 8 agosto 2026

El issue de xterm.js era razonamiento prestado. Ahora hay medida propia, contra
[Orca](https://github.com/stablyai/orca) (Electron + xterm.js con WebGL, el producto más maduro
de esta categoría) y WezTerm como referencia nativa. Misma máquina, Wayland, GPU por hardware
confirmada en Orca (`--ozone-platform=wayland`, `--render-node-override=/dev/dri/renderD128`, sin
rastro de SwiftShader). Carga: 22,7 MiB de salida tipo TUI —color, negrita y repintado de barra de
estado con movimiento de cursor cada 50 líneas— volcada de golpe. Se mide lo que el terminal tarda
en vaciar el PTY.

| | WezTerm | Orca |
|---|---|---|
| 1 división | 0,80 s · 28,4 MiB/s | 2,63 s · 8,6 MiB/s |
| 4 divisiones visibles | 0,98 / 0,98 / 1,01 / 1,06 s | 8,44 / 12,50 / 12,72 / 12,64 s |
| Caída de 1 → 4 | ×1,26 | ×4,4 |
| Agregado con 4 | 85,7 MiB/s | 7,1 MiB/s |

**Lo que decide no es el 12× de diferencia, es el signo:** Orca con cuatro divisiones mueve *menos*
trabajo total que con una (7,1 frente a 8,6 MiB/s). Escala negativo. WezTerm pasa de 28,4 a 85,7,
o sea ×3. Es la firma del hilo principal compartido.

El método juega a favor: mide el vaciado del PTY, así que un terminal que bufferea y descarta
frames saldría bien aquí y mal a la vista. Orca sale mal ya en el vaciado.

**Lo que esta medida prueba y lo que no.** Prueba que el techo existe y dónde está. **No** prueba
que la carga de un agente lo toque: la prosa de un agente son decenas de bytes por segundo, y un
`cargo build` son unos cientos de KB, que incluso a los 1,8 MiB/s degradados de Orca se vacían en
0,2 s. Vender el 12× como justificación del terminal propio sería tramposo.

**El régimen donde sí muerde es el uso real, no el sintético.** Cinco o seis divisiones visibles:
cuatro con agentes y **una o dos siguiendo logs** de un worktree que se está probando. Un `tail -f`
sobre un servidor de desarrollo es un productor **sostenido**, no una ráfaga, y convive con el
repintado continuo del panel de cada división. Esa combinación —uno o dos productores sostenidos
más N paneles pintando— es la carga que hay que medir, y es justo la que el escalado negativo dice
que este stack no aguanta. Orca no la tiene: sus paneles están quietos mientras el agente piensa.

**Consecuencia para el producto:** Orca esquiva el techo porque su producto es otro — abanica
agentes en worktrees y solo monta los terminales del worktree activo (montaje perezoso
documentado), así que mira uno cada vez. Aquí las divisiones son visibles a la vez por definición,
así que no se puede esquivar igual.

**Criterio de aceptación:** con N divisiones visibles produciendo a la vez, el rendimiento agregado
tiene que **subir** con N, no bajar. La herramienta que lo mide vive en el repo: `crates/host-load`,
y su documentación lleva el protocolo y estos números como referencia.

**Contra quién decide ese criterio, y contra quién no.** Decide **contra terminales ajenos**: Orca
falló ya en el vaciado, así que no hizo falta ver el pintado para descartarlo. **No decide contra el
nuestro**, y la razón es estructural, no un matiz — conviene leerla antes de dar el criterio por
cerrado con una medida de `host-load`:

- El instrumento mide lo que el terminal **vacía del PTY**, no lo que **pinta**.
- La mitigación que esta misma fase planifica para F1.3 es *«repintar solo cuando el PTY produzca
  cambios»* (ver los riesgos de la Fase 1 en [05-roadmap.md](05-roadmap.md)), o sea **coalescer
  frames**. Nuestra app aprobaría la prueba **por construcción**, midiendo bien precisamente por
  hacer lo que la prueba no mira.
- Y el pintado es **inobservable desde el proceso que produce la carga**: ningún protocolo de
  terminal reporta un frame presentado, y el ida y vuelta de `ESC[6n` solo demuestra *consumido y
  parseado*.

**Pendiente, por tanto:** los contadores que sí lo verían —bytes procesados por frame, frames
presentados, retraso del último respecto a la última entrada— viven **dentro de `app`** y no se
pueden construir antes que el bucle de pintado. Es trabajo de F1.3/F1.4, y es lo que cierra de
verdad el segundo síntoma que el roadmap pide medir.

Si con carga real el techo no se toca, la justificación honesta del terminal propio deja de ser el
rendimiento y pasa a ser el control del compositado y el panel integrado.

**También descarta forkear Orca**, por dos motivos independientes: heredaría este techo, y su
extensibilidad va toda hacia el agente (skills, MCP) — no hay API de plugins para su interfaz, que
es justo donde tendría que vivir el panel.

**Descartado — GTK4 + VTE.** Técnicamente el más cómodo en Linux (barra de título, botones,
arrastrar paneles gratis) pero incompatible con D1.

### Medido — 9 agosto 2026: egui bajo carga, desde dentro

Esto **construye** los contadores que la medida anterior dejó pendientes: los que ven el **pintado**
—y no solo el vaciado del pseudoterminal— ya existen dentro de `app`, porque ya existe el bucle de
pintado. No cierra el criterio entero de aquella medida: *«con N divisiones visibles produciendo a
la vez, el agregado tiene que subir con N»* sigue sin comprobarse, porque la Fase 1 todavía no tiene
dos divisiones — llega en F1.4. Todos los números son de la misma máquina (Wayland, pantalla de 120 Hz), en `--release`,
con el pintor **más ingenuo posible**: una celda cada vez, sin caché y sin seguimiento de lo que
cambió.

**El primer síntoma que el roadmap pedía vigilar — el repintado completo con más de una división:**

| Consolas sintéticas de 24×80 | 1 (1 920 celdas) | 2 (3 840 celdas) |
|---|---|---|
| CPU por fotograma | 1,52 ms (peor 2,96) | **2,98 ms** (peor 5,73) |

Dos consolas cuestan **1,96×** lo que cuesta una: escala lineal, no super-lineal. Con dos, el
fotograma gasta el **36 %** de su hueco de 8,4 ms —y su **peor** fotograma, el 68 %—. Medido con
pantallas **llenas** —cada celda con fondo propio, que obliga a dibujar un rectángulo por celda
además del glifo— y en repintado continuo, que es el techo y no el uso normal. El instrumento es
`cargo run --release --example sweep -p app`, y llama al pintor de verdad, no a una copia.

**El tamaño de esas consolas importa y hay que decirlo antes de leer el número:** son de 24×80 cada
una, así que las dos juntas suman 3 840 celdas — **menos que las 6 954 de una sola ventana real** de
esta máquina (61×114), que es donde se tomó la latencia de más abajo. El banco no mide «dos
divisiones»; mide **dos rejillas independientes** de tamaño clásico. Los fotogramas por segundo de
esa tanda (119,6 y 119,2) se omiten a propósito de la tabla: están topados por la sincronía
vertical, así que solo demuestran que el fotograma cabe en su hueco, no cuánto margen sobra — para
eso está el porcentaje.

**El segundo síntoma — la latencia:**

Tomada dentro de la aplicación, en una ventana de 61×114 celdas:

| | uso normal (13 teclas, 32 fotogramas) | volcado de 13,5 MB (111 fotogramas) |
|---|---|---|
| tecla en el fotograma → byte en el programa | **36 µs** (72 el 95 %) | — |
| byte disponible → fotograma pagado | **3,27 ms** (9,8 el 95 %) | **377 ms** (671 el 95 %) |
| … esperando turno | 2,61 ms | 373 ms |
| … interpretar + encolar + el resto | 0,65 ms | 4,1 ms |

La muestra de «uso normal» es pequeña —trece pulsaciones— y sirve para lo que se usa aquí: fijar el
orden de magnitud, que son microsegundos. Para afirmar más haría falta más.

**Lo que estas dos tablas dicen junto es el veredicto: el trabajo de verdad nunca pasa de unos
pocos milisegundos.** Lo que se percibe como retraso es, en los dos regímenes, **esperar turno** — y
son dos esperas distintas, ninguna imputable al framework: en uso normal son los 2,6 ms del hueco
entre fotogramas de una pantalla de 120 Hz, o sea sincronía vertical; en el volcado son los 373 ms
del **presupuesto de 128 KiB por fotograma**, que es el precio elegido a conciencia para que la
ventana no se congele.

**Respuesta a los dos síntomas: ninguno aparece.** El repintado completo escala lineal y deja dos
tercios del fotograma libres con dos rejillas llenas; la latencia de escritura es de microsegundos.
Ninguno de los dos justifica revisar el framework hoy.

Y un dato que confirma que el presupuesto está bien puesto: **interpretar 128 KiB cuesta 1,67 ms
medidos**, contra los ~1,94 ms que predecía la absorción de 64,5 MiB/s calculada en F1.2. La
estimación de entonces era buena.

**La escalera de escape del pintado no se activa.** Estaba escrita como celda a celda → tramos por
fila → malla con el atlas de fuentes, y ejecutable **solo si el número la pedía**. No lo pide: el
peldaño más ingenuo deja el 64 % del fotograma libre con dos consolas. Queda escrita para cuando
haga falta, sin tocar.

**Qué NO prueban estos números, dicho antes de que alguien los cite de más:**

- **Una máquina, una pantalla, un compositor.** Los números son de este equipo con Wayland y 120 Hz.
- **Dos rejillas de 24×80, no dos divisiones de la ventana real.** Es la reserva más importante de
  todas: 3 840 celdas contra las 6 954 de la ventana donde se midió la latencia. En F1.4 dos
  divisiones **se reparten** la ventana en vez de duplicarla, así que el área total no crece — lo
  que crece es el número de rejillas, de lecturas y de llamadas al pintor. Ese caso **no está
  medido**, y es exactamente el que F1.4 tiene que volver a pasar por el banco, con la ventana de
  verdad partida.
- **Cuatro y ocho consolas tampoco están medidas.** El síntoma del roadmap se redacta con dos y ahí
  se midió. Extrapolando la escala lineal, cuatro rondarían los 6 ms de media y su peor fotograma se
  saldría del hueco: **el margen no da para todo**.
- **El peor fotograma con dos rejillas ya gasta el 68 %** del hueco. La media dice que sobra sitio;
  el peor dice cuánto margen hay realmente, y es la mitad de lo que sugiere la media.
- **Nada de esto mide el compositado ni el panel**, que es lo que la Fase 1 no tiene todavía.
- **Hay coste que hoy se atribuye a egui sin ser suyo.** Leer la rejilla materializa todas sus
  celdas en cada pase —288 KiB por pase en una ventana de 60×240, que a 60 fotogramas son 17 MiB/s—
  y eso se lo llevará puesto cualquier framework. Y el aviso de repintado se pide **una vez por cada
  trozo de 8 KiB** que llega del pseudoterminal, unos 34 000 por segundo en un volcado, cada uno
  tomando un candado que el hilo de interfaz también usa; egui además convierte cada petición en
  **dos** pases. Ninguna de las tres cosas se aisló en estas medidas. Si un día el número sale raro,
  este párrafo es el primer sitio donde mirar.

**Dos cosas que sí muerden y no son de rendimiento.** Van aquí porque un veredicto que solo mirase
los milisegundos daría una impresión falsa de «esto está listo»:

1. **egui no usa las fuentes del sistema.** Embarca cuatro y ninguna cubre CJK ni el Área de Uso
   Privado, así que el japonés y los iconos de un `ls` moderno salen como cuadros vacíos, aunque la
   máquina tenga esas fuentes instaladas. El pintor hace lo correcto —reserva el hueco y pide el
   glifo—; quien no lo tiene es la fuente. Registrar fuentes del sistema al arrancar es trabajo
   pendiente, y es lo que separa esto de ser usable a diario.
2. **La cola de salida no tiene techo** ([#20](https://github.com/mashware/agent-terminal/issues/20)):
   un programa que escribe sin parar —`yes`, `cat /dev/urandom`— agota la memoria. El único freno
   real es dejar de leer el pseudoterminal, y eso vive en `term-core`.

**Qué significa para esta decisión.** D3 sigue como está: **egui aguanta de sobra lo que la Fase 1
necesita**, que es exactamente el papel que esta decisión le daba. Lo que estos números añaden es
una pregunta que antes no se podía ni formular con datos —si el margen es este, ¿qué aporta GPUI que
justifique su coste?— y que **no se responde aquí**: se responde cuando F1.4 tenga dos divisiones de
verdad y F1.6 los paneles pintando a la vez, que es la carga que D3 dice que hay que medir. Cambiar
la decisión de fondo en un anexo, con dos consolas sintéticas de una sola máquina, sería justo el
tipo de atajo que esta decisión se tomó para evitar.

### Medido — 11 agosto 2026: dos consolas de verdad, dentro de la aplicación

Esto es lo que el anexo anterior dejó pendiente: **las divisiones ya no son sintéticas**. Son las dos
consolas de F1.4, cada una con su pseudoterminal, su rejilla y su programa, medidas **dentro de la
aplicación** y no en un banco. Misma máquina que las medidas anteriores, `--release`, pantalla de 120 Hz,
rejilla **61×57 por consola** (ventana maximizada, repartida a mitades).

La carga es un volcado tipo TUI —color, negrita y repintado de barra de estado con movimiento de
cursor—. **La tanda que se publica es la de 400.000 líneas por consola**, que da entre **376 y 382
fotogramas con salida** por corrida: dos corridas del caso simétrico. El asimétrico son dos corridas de
120.000 líneas (105-120 fotogramas). Los números son la media de las medias.

| Tramo del fotograma | Una volcando, la otra quieta | **Las dos volcando a la vez** |
|---|---|---|
| Interpretar (las dos consolas) | 1,92 ms | **3,35 ms** |
| Encolar el pintado (las dos) | 0,87 ms | **1,83 ms** |
| El resto del fotograma (teselar, dibujar, armazón) | 0,61 ms | **0,74 ms** |
| **Coste del fotograma** | **3,39 ms** | **5,92 ms** |
| Del hueco de 120 Hz (8,33 ms) | 41 % | **71 %** |
| Del hueco de 60 Hz (16,67 ms) | 20 % | 36 % |
| **Tecla en el fotograma → byte en el programa** | no medido | **3,42 ms de media, 5,00 ms el p95** (28 pulsaciones) |

**Una tanda más corta da peor número, y el porqué importa.** Tres corridas de 120.000 líneas
(105-120 fotogramas) dieron **7,00 ms**, el 84 % del hueco. La diferencia no es ruido: con el triple de
fotogramas, el coste del arranque —abrir la ventana, subir texturas, medir la fuente por primera vez—
pesa un tercio en la media. **El número que vale es el de la tanda larga**, y la corta queda aquí escrita
para que nadie la encuentre después y crea que hay una contradicción.

**Lo que estos números prueban:**

- Con dos consolas de verdad **volcando a la vez**, el fotograma cuesta **5,92 ms de media y cabe en el
  hueco de 120 Hz**, con el 29 % de margen. Es la primera vez que ese número se toma sobre divisiones
  reales y no simuladas.
- **El grueso del fotograma no lo gasta el framework de interfaz: lo gasta interpretar.** 3,35 de los
  5,92 ms son `alacritty_terminal` tragando los 128 KiB por fotograma **de cada consola** — trabajo del
  terminal, y exactamente lo que el presupuesto por fotograma existe para acotar. Lo que queda para
  dibujar son 2,57 ms, el **31 %** del hueco.
- **La ventana sigue respondiendo al teclado mientras dura el volcado doble, y ahora con cifra:** de la
  tecla en el fotograma al byte escrito en el programa, **3,42 ms de media y 5,00 ms el percentil 95**,
  sobre 28 pulsaciones tecleadas a mano durante la caída. En F1.3, sin volcado, ese tramo daba **36 µs**:
  sube casi ×100, y **la causa está identificada, no es un misterio** — el cronómetro arranca al empezar
  el fotograma y por delante de escribir la tecla corre interpretar, que son esos mismos 3,35 ms. La
  tecla espera a que el fotograma acabe de tragar. Sigue siendo imperceptible: 5 ms en el peor caso es
  menos que un fotograma de 60 Hz.
- **El instrumento reparte bien.** El resto del fotograma descuenta también el pintado de una consola que
  no recibió salida en ese fotograma, que era un fallo de atribución encontrado en revisión. Se ve en el
  propio informe: los fotogramas contados en «el resto» (105-120) igualan o superan los de la consola con
  salida (107-115), porque incluyen aquellos en que solo se pintó la quieta.

**Lo que NO prueban. Esta lista es la mitad del valor del anexo, y es larga a propósito:**

- **Este 31 % NO confirma el 36 % del banco del 9 de agosto, porque no miden lo mismo.** Aquel medía
  **pantallas llenas y repintado continuo** —un techo, y así lo decía— sobre 3.840 celdas. Este mide un
  volcado tipo TUI con repintado a demanda sobre **6.954 celdas**, casi el doble, y en un régimen donde
  el pintor se salta la mayoría de las celdas por llevar el fondo por defecto. Que los dos porcentajes
  salgan parecidos es **coincidencia de dos efectos opuestos** —casi el doble de celdas contra un relleno
  mucho más barato—, no una confirmación. El aviso que el propio instrumento lleva escrito
  (`crates/app/src/medida.rs`) dice justamente esto: comparar tramos entre medidas exige mirar la rejilla,
  no solo el tramo.
- **No hay línea base de una consola dentro de la aplicación**, así que **no se puede afirmar cómo escala
  el pintado** de una a dos. Y no es un descuido evitable: desde F1.4 la ventana abre **siempre dos**
  consolas, así que la comparación 1 contra 2 dejó de ser medible desde dentro. El instrumento que la
  daría es el banco con reparto real, que está sin hacer
  ([#33](https://github.com/mashware/agent-terminal/issues/33)).

---

## D11 — Pegar es del terminal, `Ctrl+V` es del programa, y lo pegado va sin controles

**Decidido.** `Ctrl+Mayúsculas+V` pega: el terminal escribe el texto del portapapeles en el programa
de dentro. `Ctrl+V` **no pega** — le entrega al programa el byte de control que le corresponde, y él
decide qué significa. Y a todo lo que se pega **se le quitan los caracteres de control**, menos el
tabulador y los saltos de línea.

**Por qué el reparto de los dos atajos, y no uno solo.** Son capas distintas atendiendo cada tecla, y
eso se comprobó mirando qué hace un terminal de verdad: en Tilix `Ctrl+V` no pega, pero **dentro de
esa misma ventana** Claude sí pega con `Ctrl+V`, porque recibe el byte y ha decidido que para él eso
significa pegar. Vim recibe el mismo byte y entra en bloque visual; un shell lo usa para
*quoted-insert*. **Quién pega con `Ctrl+V` no es decisión del terminal**, y quedárselo rompería el
bloque visual sin que nadie lo pidiera.

**El apaño técnico que hace falta para conseguirlo**, porque no es gratis: el escritorio emite el
**mismo evento** para los dos atajos —comprobado en la fuente de `egui-winit`: la condición es
«mandato + V» y no mira Mayúsculas— y **se traga el evento de tecla**. Distinguirlos exige
reconstruir el estado de las teclas muertas recorriendo los eventos en orden. Leer el campo que el
framework publica **no vale**: es uno solo por pase y describe cómo estaban al **final** del lote, no
cuando llegó el pegado.

**Por qué se filtran los controles, y son dos razones distintas.**

1. **La marca de pegado se puede cerrar desde dentro.** Un portapapeles que traiga su secuencia de
   cierre la cierra por su cuenta, y lo que venga detrás entra como tecleado — **ejecutándose sin
   Enter**. Quitar solo esa secuencia **no basta**: un ataque comprobado la reconstruye aprovechando
   que buscar-y-borrar no vuelve a mirar lo que deja.
2. **Y el envoltorio no protege del núcleo.** Los caracteres de control los procesa la disciplina de
   línea, que ve los bytes **antes** que el programa: un `0x03` interrumpe, un `0x1a` suspende el
   shell, un `0x1c` provoca un volcado, un `0x04` cierra la entrada — y con las dos consolas cerradas,
   la ventana. Marcar el pegado no cambia nada de eso. Es la razón por la que `xterm` y `VTE` filtran
   los controles del pegado desde hace años.

**Un salto de línea sale distinto según haya marca o no, y equivocarse se ve en pantalla.** Sin
marca hay que imitar la tecla, y Enter manda **retorno de carro**. Con marca el salto va **tal cual**:
el programa sabe que es un pegado y decide él. Mandarle un retorno de carro dentro de la marca es
meterle un carácter de control en medio de un texto, y lo inserta literal — sale un cuadro en la orden
y el shell lo imprime al ejecutar. Se descubrió pegando dos líneas y mirando.

**Lo que cuesta, y se acepta.** Un texto con secuencias de color pierde sus escapes y el resto queda
como texto corriente: pegar un volcado coloreado deja `[32mok[0m` en la línea. Es lo mismo que hace
`xterm` por defecto. Desde el terminal **no hay forma de distinguir el control que el usuario quería
del que le colaron**, y uno cuyo pegado puede ejecutar o suspender es peor que uno que pega en texto
plano.

**Lo que se descartó, y por qué.**

- **Que pegar sea también `Ctrl+V`.** Más simple y quita el mecanismo más delicado — pero se lleva por
  delante el bloque visual de vim y el *quoted-insert* del shell, que se usan a diario.
- **Leer los modificadores del campo del framework.** No distingue los dos atajos: ver arriba.
- **Quitar solo la secuencia de cierre.** Se escapa, comprobado.
- **Repetir el borrado hasta que no quede ninguna.** Cerraría *ese* agujero y ninguno de los del
  núcleo.
- **El clic central** (la selección primaria del escritorio). `egui` no la expone: su portapapeles
  solo sabe leer y escribir el normal, y la pieza de debajo que sí la tiene es privada.
- **Copiar.** Arrastra selección con el ratón y, en la práctica, scrollback. Sigue en F2.6.

**Lo que hay que saber para no llevarse un susto.**

- **El programa de dentro puede pedir que lo pegado le llegue marcado**, y el terminal se lo pregunta
  al motor. Ese accesor devuelve un `bool` y no el tipo del motor, para que la caja del terminal
  siga sin dejar asomar sus dependencias.
- **Con el portapapeles vacío, `Ctrl+V` no manda nada.** El escritorio se traga la tecla y no emite
  ningún evento cuando no hay qué pegar, así que el byte de control tampoco sale. Se nota poco —un
  portapapeles vacío es raro— pero significa que el bloque visual de vim no se abre en una sesión
  recién arrancada sin haber copiado nada. No se puede arreglar desde aquí.
- **En macOS los dos atajos se comportan distinto** y esta regla los rompería: allí el «mandato» es ⌘,
  así que ⌘+V daría el byte de control mientras `Ctrl+V` llega como tecla. La primera compilación en
  ese sistema es F2.7 y tendrá que revisarlo.
- **Pegar puede colgar la ventana** mientras el programa de dentro no lea su entrada. No es de pegar:
  es que escribir al pseudoterminal bloquea sin techo
  ([#50](https://github.com/mashware/agent-terminal/issues/50)). Pegar solo lo hace alcanzable.
- **«Encolar» no es coste de egui: es el pintor de este proyecto**, que sigue siendo el más ingenuo
  posible —una celda cada vez, sin caché ni seguimiento de lo que cambió— llamando a `rect_filled` y
  `text`. De los 3,21 ms de dibujo, lo único atribuible al framework sin discusión es **el resto del
  fotograma: 0,74 ms, el 9 % del hueco**. Decir «el framework gasta el 31 %» sería cargarle código propio
  que se reescribiría igual con cualquier otro.
- **El peor caso no se ha remedido con dos divisiones.** Este escenario es exigente pero no es el techo:
  el techo son pantallas llenas y repintado continuo, que es lo que medía el banco. Así que este anexo
  **no cierra** el criterio de aceptación que pedía medir el peor caso con divisiones de verdad.
- **El percentil 95 del caso simétrico se sale del hueco de 120 Hz** (~9,4 ms sumando los p95 de la
  tanda larga; y sumar percentiles de tramos no da el percentil del fotograma, así que ese número es
  indicativo, no una medida). Cabe en el de 60 Hz con holgura. Hay tirones ocasionales durante un volcado
  doble.
- **Nada sobre cuatro consolas.** Multiplicar por dos da 11,8 ms, pero eso **contradice el propio modelo
  de este anexo**: si con cuatro cada rejilla es más estrecha, encolar por consola baja y solo interpretar
  escala lineal, con lo que saldría algo más cerca de 9 ms. Las dos cuentas son aritmética, no medida, y
  por eso el aviso del roadmap sigue en pie.
- **Nada del panel pintando a la vez**, que es F1.6 y la otra mitad de la carga que esta decisión dice
  que hay que medir.
- **La serie «byte disponible → fotograma pagado» solo es interpretable en el caso simétrico.** Incluye
  el fotograma entero —se presenta indivisible— así que en el asimétrico la de la consola tranquila se
  degrada por el volcado de la vecina y no la describe. Ahí valen los tramos propios, no ese total.

**Qué significa para esta decisión.** **D3 no cambia, y sigue sin poder cerrarse.** Lo que este anexo
aporta no es la confirmación que parecía: es un **orden de magnitud tomado sobre divisiones reales** —el
fotograma cabe en el hueco de 120 Hz con dos consolas volcando— y, sobre todo, **un cambio de sospechoso**.
El framework de interfaz se lleva **0,74 ms demostrables de 8,33**; el resto lo gastan el parser del
terminal y un pintor propio que nadie ha optimizado todavía. Antes de reabrir si GPUI compensa su coste, hay dos
cosas que costarían menos y darían más margen: **escribir un pintor que no repinte cada celda cada
fotograma**, y probar los turnos rotatorios del presupuesto que el diseño de F1.4 dejó anotados. Y para
cerrar el criterio de verdad faltan tres medidas que no están: el peor caso con dos divisiones, la línea
base de una sola dentro de la aplicación, y F1.6 con los paneles encima.

### Medido — 13 agosto 2026: con los dos paneles pintando

La tercera de esas tres. Misma máquina y mismo `--release`, rejilla de **61 filas** por consola y
ventana sin tocar; la carga es el volcado determinista de `host-load` —color, negrita y repintado de
barra de estado con movimiento de cursor—, **44 MiB por consola**, arrancadas a la vez con su
alineación al reloj de pared. Cada tanda son **dos corridas** y los números son la media de las
medias, sobre **357-358 fotogramas con salida por consola** en las cuatro. Se toma con
`scripts/medir-fotograma.sh`, que va en el repo: la tanda del 11 de agosto no dejó guion y
reconstruirla costó más que tomarla.

**Tres regímenes, y el primero es el que hace legible a los otros dos:**

| | **A** sin panel | **B** panel quieto | **C** panel publicándose 1×s |
|---|---|---|---|
| Columnas por consola | **57** | **21** | **21** |
| Interpretar (las dos) | 4,35 ms | 4,51 ms | 4,35 ms |
| Encolar el pintado (las dos) | 1,80 ms | 0,75 ms | 0,74 ms |
| **El panel (las dos)** | **0,77 µs** | **12,8 µs** | **13,6 µs** |
| El resto del fotograma | 0,86 ms | 0,50 ms | 0,46 ms |
| **Coste del fotograma** | **7,01 ms** | **5,78 ms** | **5,56 ms** |
| Del hueco de 120 Hz | 84 % | 69 % | 67 % |
| Fotogramas de la ventana | 368 y 369 | 372 y 371 | **472 y 471** |

> **Con los paneles pintando el fotograma sale más barato, y eso NO significa que los paneles salgan
> gratis.** El panel se lleva **36 de las 57 columnas** de su consola, así que la consola pinta un
> 63 % menos de celdas. **El tramo que baja por esa causa es `encolar`** —1,80 a 0,75 ms, un −58 %—,
> y baja porque hay menos celdas que apilar, no porque nada haya mejorado. Lo que la tabla enseña es
> un intercambio —menos consola a cambio de un panel—, no una ganancia. **Leer «−18 % de coste» como
> una mejora es el error que esta tanda de control existe para impedir**, y sin la columna A el anexo
> lo habría cometido.

**Y una que sale al revés de lo esperado: envolver no se nota.** A 57 columnas las líneas de la carga
—~50 caracteres visibles— no envuelven; a 21 envuelven en tres filas. Cabía esperar que interpretar se
encareciera, porque el ancho de envoltura es justamente de lo que depende. **No pasa**: 4,35 / 4,51 /
4,35 ms, plano dentro del ruido entre corridas. El coste de meter bytes en la rejilla lo domina la
cantidad de bytes, no la forma en que se reparten en filas.

**Lo que sí es del panel, aislado.** La resta que sí vale es la del **tramo del panel**, porque su
suelo es el mismo en las tres columnas: en A ese tramo no es cero —el directorio se crea siempre, así
que hay que recoger la bandera, pedir la hora y repartir el ancho— y mide **0,77 µs**. Sobre ese
suelo:

| | Media (las dos) | El 95 %, consola izquierda |
|---|---|---|
| A — sin panel | 0,77 µs | 1,4 µs |
| B — panel quieto | **12,8 µs** | 19,3 µs |
| C — panel publicándose | **13,6 µs** | 21,4 µs |

Leer y pintar dos paneles cuesta **unos 13 µs por fotograma**, el **0,2 % del fotograma** y el
**0,15 % del hueco de 120 Hz**. El panel, como coste por fotograma, **no se nota**.

**Lo que publicar añade no se puede separar del ruido, y decirlo importa.** La tabla enseña 12,8 µs
quieto y 13,6 publicándose, o sea 0,8 de diferencia. Pero **la tanda D es una réplica válida de C**
—una vez demostrado abajo que dónde cae el temporal no influye—, y las cuatro corridas del régimen
«publicándose» se reparten entre **13,5 y 15,3 µs**: la variación **dentro** del mismo régimen dobla
el efecto que se le atribuye a publicar. Con dos corridas parecía separado; con las cuatro
comparables, no lo está. Lo que sí aguanta es lo cualitativo: **publicar una vez por segundo no se
nota**.

**Lo que sí se nota del panel hablador son los fotogramas, no su coste.** Cada consola produce en
357-358 fotogramas en las cuatro tandas, y sin embargo la ventana cuenta **368-369 en A** y **471-472
en C**. La unión crece con los sumandos iguales, así que **hay menos solape**: en A las dos consolas
producen casi siempre en el mismo fotograma —358 + 357 − 368 = **347 compartidos**, el 97 %— y en C
bastante menos: 358 + 357 − 472 = **243**, el 68 %. El mismo trabajo se reparte entre un **28 % más de fotogramas**, cada uno más
barato. Por eso **el «coste del fotograma» de C no se compara con el de B**: son fotogramas que llevan
dentro cantidades distintas de trabajo.

> **La aritmética del solape está en los datos; la causa es interpretación y así hay que leerla.**
> Que sean los despertares del vigilante los que desincronizan las dos consolas **no está medido**: el
> informe no publica ni la duración de la corrida ni cuántas veces despertó, así que no se puede
> contrastar que las publicaciones expliquen los ~104 fotogramas de más. Lo único que apunta en esa
> dirección es que B —con vigilante armado pero sin nadie publicando— se queda en 371.

**Dónde cae el fichero temporal no influye, y no por casualidad.** Parecía la explicación obvia de
esos 100 fotogramas —un temporal escrito dentro del directorio vigilado despertando a la ventana por
su cuenta—, así que se tomó una cuarta tanda idéntica a C pero con el temporal **fuera**: 473 y 466
fotogramas, contra 472 y 471. Y el código explica por qué tenía que salir igual: el vigilante
**filtra por ruta exacta** y solo levanta la bandera cuando el aviso trae `<dir>/panel.json`
(`crates/app/src/vigilante.rs`), así que cualquier otro fichero del directorio le es invisible. La
hipótesis se cayó dos veces, midiendo y leyendo, y queda escrita para que nadie la vuelva a levantar.

**Dos huecos de atribución, los dos con su número:**

1. **Armar el despertador es trabajo del panel que hoy se publica como coste de egui.** Calcular
   cuándo tocará avisar de que un panel está viejo se hace **dos veces por fotograma** y **fuera** de
   los tres relojes de la división, así que cae en «el resto del fotograma». Medido con sonda en dos
   corridas —784 y 860 ns—: **822 ns por llamada, 1,64 µs por fotograma**, el **0,4 %** de ese tramo y
   un **12 % del coste propio del panel**. El sesgo es pequeño pero va en la dirección que más daño
   hace, porque infla justo la cifra sobre la que esta decisión tiene que decidir.
2. **Enterarse no aparece en ninguna parte.** El vigilante de ficheros corre en **su propio hilo**, y
   el cronómetro de `eframe` es un reloj de pared sobre la pasada del hilo de interfaz. Su coste no
   infla «el resto» —o sea que no se le carga a egui, que era el riesgo— pero **tampoco está en el
   número**: lo que aquí se publica es lo que cuesta leer y pintar el panel, no lo que cuesta saber
   que cambió.

**Lo que estos números NO prueban. Como en los dos anexos anteriores, esta lista es la mitad del
valor:**

- **No comparan con el 11 de agosto, y la propia tanda A lo demuestra.** Aquella publicó 5,92 ms sin
  paneles; ésta, en la misma configuración, da **7,01 ms**. Otro día, otra carga, otra máquina en otro
  estado. **La única comparación válida es dentro de esta tabla**, que es para lo que se tomaron las
  tres el mismo día y en la misma ventana.
- **El total de B y C no se resta contra el de A.** Solo la resta del tramo del panel es honesta,
  porque es el único cuyo suelo no cambia con la rejilla.
- **El 95 % del tramo del panel se toma solo sobre fotogramas en que esa consola recibió salida.** Las
  series solo se anotan cuando llegó algo, mientras que el tramo se descuenta del total siempre. Un
  fotograma que el vigilante despierta sin salida de consola no entra en ese percentil.
- **El coste del fotograma es reloj de pared, no CPU**, pese a que el campo del framework se llame
  así. Con la máquina cargada, la contención de otros procesos entra en el número.
- **Toda corrida imprime «el directorio que se vigilaba ha dejado de existir», y no afecta a los
  números.** Los directorios de panel se borran cuando **la aplicación** termina, no cuando muere el
  programa de una consola: la ventana espera a las dos y solo entonces se cierra. Así que ese aviso
  sale durante el apagado, después de medir. Se deja dicho porque quien repita la tanda lo verá y
  pensará que ha roto algo.
- **La consola izquierda cuesta sistemáticamente más que la derecha, y no se sabe por qué.** En las
  ocho corridas, sin excepción: en la tanda del panel hablador su tramo de panel es ~8,4 µs frente a
  ~5,2 —**1,6×**— y encolar el pintado 437 µs frente a 328 —**1,3×**—. La columna «las dos» esconde
  esa asimetría al sumar. Las dos consolas tienen la misma rejilla y la misma carga, así que la explicación
  está en el orden en que se pintan o en el foco, y **no se ha investigado**.
- **Tres cosas del método no se pueden comprobar desde el informe**: los 44 MiB por consola, que las
  dos arrancaran alineadas, y la frecuencia de publicación. El informe no imprime ni volumen, ni
  duración, ni recuento de publicaciones — quien quiera auditarlo tiene que leer
  `scripts/medir-fotograma.sh`, no los ficheros de salida.
- **Nada del peor caso**: esto es un volcado exigente, no el techo de pantallas llenas con repintado
  continuo. Sigue sin cerrarse ese criterio.
- **Nada de cuatro divisiones**, y ahora hay una razón nueva para no extrapolar: si dos consolas dan
  57 columnas cada una, cuatro darían unas **28 antes de descontar el panel** — y el panel se lleva
  **36**. O sea que con cuatro divisiones el panel **no cabe**, y el reparto tendría que hacer algo
  que hoy no hace. El número no se puede multiplicar
  ([#33](https://github.com/mashware/agent-terminal/issues/33)).

**Qué significa para esta decisión.** **D3 no cambia, y el cambio de sospechoso que anunció el anexo
anterior se confirma y se agranda.** Con los dos paneles encima, lo demostrablemente atribuible al
framework de interfaz es **0,50 ms de 8,33**, el **6 % del hueco** —menos que los 0,74 ms de agosto, y
por una razón que no le hace ningún favor a egui: hay menos celdas que teselar—. Los paneles, que eran
la otra mitad de la carga que esta decisión mandaba medir, gastan **13 µs**, cuatro órdenes de
magnitud por debajo del presupuesto. **El fotograma sigue gastándolo interpretar** —4,5 de los 5,8 ms
de la tanda del panel quieto— y
un pintor propio que nadie ha optimizado.

De las tres medidas que faltaban, **ésta queda hecha**. Siguen faltando dos, y las dos dependen del
banco con reparto real: el peor caso con dos divisiones y la línea base de una sola dentro de la
aplicación, que dejó de ser medible desde dentro cuando la ventana pasó a abrir siempre dos.

---

## D4 — Control por consola, no por API

**Decidido.** El agente se controla escribiendo en su PTY y leyendo su salida, no llamando a
la API del proveedor.

**Por qué — es económico, no técnico.** Las suscripciones (Claude Max, etc.) solo valen dentro
del cliente oficial de cada proveedor. Llamar a la API se factura por tokens aparte. Una
herramienta que obligue a pagar API por lo que el usuario ya paga por suscripción no se adopta.

**Confirmado desde fuera, dos veces.** Cline Kanban orquesta lanzando los binarios oficiales de cada
agente en un proceso, no llamando a modelos por API; y Xirp autentica cada agente *«through its
native CLI»* — con miles de ingenieros y factura de empresa detrás, que es justo donde pagar API
aparte habría costado menos justificar. Ver [07-panorama.md](07-panorama.md) → E1 y E3.

**Consecuencia de diseño:** la app **nunca** debe definir `ANTHROPIC_API_KEY` (ni equivalentes)
al lanzar un panel. Esa variable tiene prioridad sobre la sesión de la suscripción y cambiaría
la facturación del usuario sin que se entere.

**Zona gris a vigilar:** el modo no interactivo de Claude Code (`claude -p`) debería usar la
sesión de la suscripción, pero hay issues abiertos reportando facturación de API inesperada
([#43333](https://github.com/anthropics/claude-code/issues/43333),
[#37686](https://github.com/anthropics/claude-code/issues/37686) — 1.800 $ en dos días).
**No construir sobre ese camino sin verificarlo con gasto pequeño y vigilado.**

### Confirmado por la vía dura — Anthropic lo prohibió, y con fecha

Esto era la suposición sobre la que descansaba toda la decisión, y **dejó de ser una suposición en
2026**. La cronología, verificada en agosto de 2026:

| Cuándo | Qué |
|---|---|
| Enero de 2026 | Anthropic **bloquea** los tokens OAuth de suscripción fuera de sus aplicaciones oficiales, sin aviso previo. Marcha atrás tras las protestas |
| Febrero de 2026 | Lo **prohíbe formalmente**: los tokens de Free, Pro y Max no pueden usarse en herramientas de terceros ni en el SDK de agentes |
| **4 de abril de 2026** | **Entra en vigor.** Pro, Max y Team dejan de cubrir el uso a través de herramientas de terceros — **opencode**, OpenClaw, NanoClaw y cualquier otra que enrute peticiones por OAuth. Compensación: un crédito equivalente a un mes de suscripción |

> **Una suscripción de Claude Code no es una credencial de desarrollador.** Lo que se compra es el
> derecho de uso de **dos productos** —Claude Code y `claude.ai`—, no un token que se enchufa a
> cualquier orquestador. Quien integre Claude en su producto necesita clave de API y facturación por
> uso.

**Qué significa para este proyecto, y es la línea que lo separa todo:** **nosotros no usamos el token
de nadie.** Lanzamos el binario oficial en un pseudoterminal y él se autentica con su propia sesión,
igual que si el usuario lo hubiera escrito a mano. Por eso la app **hospeda** agentes en lugar de
llamarlos, y por eso `ANTHROPIC_API_KEY` está prohibida: no es higiene, es la diferencia entre estar
dentro o fuera de la política. La regla dura sigue exactamente igual, con una razón externa detrás.

**Y una consecuencia para la Fase 5 que no estaba escrita.** El supervisor —«un agente que observa a
los demás y avisa»— **no puede ser una llamada nuestra a un modelo**: eso sería la app consumiendo
API, o sea el usuario pagando aparte por lo que ya paga. Tiene que ser **otro agente de consola**, en
su división, hablando por MCP como cualquier otro. El roadmap dice que la Fase 5 «no necesita ninguna
pieza nueva»; sigue siendo cierto, y **esta es la razón por la que tiene que seguir siéndolo**.

**Y sobre opencode en concreto** ([06-referencias.md](06-referencias.md)): admite más de 75
proveedores —es el más agnóstico de todos— pero **con Claude va por API y tokens desde el 4 de abril
de 2026**. Como *productor* de panel es perfecto; como agente hospedado, su usuario de Claude paga
por uso.

Fuentes: [alternativeto.net](https://alternativeto.net/news/2026/2/anthropic-officially-bans-using-subscription-authentication-for-third-party-claude-use) ·
[winbuzzer.com](https://winbuzzer.com/2026/02/19/anthropic-bans-claude-subscription-oauth-in-third-party-apps-xcxwbn/) ·
[yage.ai — «A Claude Code Subscription Is Not a Developer Credential»](https://yage.ai/share/claude-code-subscription-not-a-developer-credential-en-20260321.html)

---

## D5 — Tres canales, no uno

**Decidido.** Ver [03-arquitectura.md](03-arquitectura.md).

| Dirección | Canal | Para qué |
|---|---|---|
| Agente → app | **MCP** | Crear divisiones, publicar estado, leer salida |
| Productor → app | **el documento de panel** | Estado que publica cualquiera, agente o no. Hoy llega por fichero; el transporte no es el contrato (**D17**) |
| App → agente | **PTY** | Botones, y todo lo que no hable MCP |

**Por qué MCP:** lo hablan Claude Code, opencode, Gemini CLI y Codex — se escribe el servidor
una vez y sirven todos. Y **no consume API aparte**: el agente ya corre con su suscripción y MCP
solo le añade herramientas a esa sesión. Es el canal limpio que la API no podía ser (D4).

**El PTY deja de ser el mecanismo principal** y pasa a ser el respaldo universal: limpio con
quien habla MCP, funcional con quien no.

---

## D6 — El estado lo publica quien trabaja

**Decidido.** La app no deduce el estado leyendo la pantalla ni parseando ficheros de terceros.

**Por qué:** aprendido a base de fallar. En el prototipo, intentar deducir el estado de un tren
de merge requests desde los datos crudos produjo **tres errores seguidos** — qué cuenta como
«cerrada», cuál de dos entradas en curso es la real, qué significan las fases acumuladas. Esa
semántica solo la conoce quien hace el trabajo.

**Consecuencia:** el contrato lleva `updated_at` y la app avisa si el resumen se queda rancio.
Un resumen que solo se escribe al terminar un paso miente si el proceso muere a mitad; la marca
de tiempo lo delata.

### Validación externa — Xirp, agosto 2026

Spotify abrió **Xirp** en beta pública el 10 de agosto de 2026: app de escritorio que hospeda
sesiones de Claude Code, Codex y Gemini CLI, salida de más de 36.000 sesiones internas. Es la
confirmación más fuerte que ha tenido esta decisión, y a la vez el aviso más preciso sobre dónde
está de verdad el diferenciador.

**Lo que confirma:** no deducen el estado leyendo la pantalla. Se lo declaran, por *session hooks*
del propio harness — «Session hooks let Xirp detect when an agent is working, idle, or waiting for
input». Quien ha montado esto a escala de miles de ingenieros llegó a lo mismo que D6: el estado se
pide, no se adivina.

**Lo que separa las dos cosas: que el estado sea declarado no basta, importa quién lo declara.**
Hay tres niveles y solo el tercero contesta «por dónde va la tarea»:

| Nivel | Quién declara | Vocabulario | Qué contesta |
|---|---|---|---|
| 1 — inferido | la app, mirando el pintado o el PTY | el que la app se invente | nada fiable: es el error de los tres fallos de arriba |
| 2 — ciclo de vida | el *harness* (Claude Code, Codex), por hooks | fijo: `working` / `idle` / `waiting for input` | si el programa corre o espera |
| 3 — publicado | quien hace el trabajo, en `panel.json` | el suyo, texto libre | en qué paso va, cuánto queda, qué lo bloquea |

**Por qué el nivel 2 no puede crecer hasta el 3.** No es un hueco de implementación que se tape con
más hooks: el harness **no conoce** esa información. Claude Code sabe que ejecutó una herramienta y
que se detuvo; no sabe que un ciclo de trabajo va por el paso `build`, que lleva dos de tres
entregas, ni que está parado esperando que se apruebe un diseño. Con el nivel 2 los siete pasos de
un ciclo se ven idénticos: `Working`. Y un CI, un despliegue o un script propio —que no tienen
harness— no tienen dónde publicar nada.

Fuente: [Get started with Xirp](https://backstage.spotify.com/docs/xirp/getting-started). El resto
de la comparación, en [07-panorama.md](07-panorama.md) → E3.

### El nivel 2 reinterpretado falla igual que adivinar — con datos ajenos

**Superset** ([07-panorama.md](07-panorama.md) → E4) hace lo mismo que Xirp pero con cuatro eventos
en vez de tres: `Start`, `Stop`, `PermissionRequest`, `PendingQuestion`. Y como su código es
consultable, se puede ver el fallo entero en vez de razonarlo.

Su bug [#5259](https://github.com/superset-sh/superset/issues/5259), abierto: la notificación de
«Task complete» salta **en cada turno del modelo**. Una tarea con 63 turnos produjo **63 avisos**. La
causa, literal: el hook `stop` de Cursor Agent significa *«acabé este turno»*, y la app lo leyó como
*«acabé la tarea»*. Ya les había pasado (#1785) y el arreglo por *debounce* (#1786) se cerró sin
fusionar.

> **Un evento declarado no basta si la app le pone encima una semántica que el harness no tenía.**
> Falla exactamente igual que adivinar mirando la pantalla, porque es lo mismo con más pasos.

Y no se arregla con un *debounce*: el dato que distingue «turno» de «tarea» **no está en ninguna
parte del sistema**, porque nadie lo publicó. Solo lo sabe quien hace el trabajo.

**Lo que esto añade a la tabla de arriba:** el nivel 2 no es un escalón intermedio seguro. Usado
para lo que es —«el programa arrancó, paró, pide permiso»— es correcto y útil. Traducido a
progreso de tarea, es el nivel 1 con un disfraz. La frontera no está en si el dato viene declarado,
sino en si **quien lo declaró conocía la pregunta que se le está haciendo.**

---

## D7 — Identidad por división

**Decidido.** Al lanzar el shell de una división, la app inyecta variables de entorno:

```
PANEL_ID=a3f2   PANEL_DIR=/run/user/1000/panels/a3f2
```

**Por qué:** es lo que desata el contrato de cualquier herramienta concreta. Cualquier proceso
que corra ahí hereda la variable, sabe dónde publicar, y la app sabe qué fichero corresponde a
qué división. Sin esto el descubrimiento queda atado a convenciones ajenas (hoy el prototipo
busca en `.claude/work/`, que es específico de `flow`).

**También es el límite de seguridad de MCP:** el servidor da al agente acceso a *su* división y
no a las demás. Sin identidad, cuatro agentes se pisan.

### Construido — 11 agosto: la forma exacta, y en qué se separa del ejemplo de arriba

Implementado en F1.5. La decisión no cambia; lo que sigue concreta lo que el ejemplo dejaba abierto,
y **se separa de él en dos cosas**, cada una con su razón.

```
PANEL_ID=3f2a9c14   PANEL_DIR=/run/user/1000/agent-terminal-3f2a9c14
```

| | Lo elegido | Por qué |
|---|---|---|
| Dónde | `$XDG_RUNTIME_DIR`, y el temporal del sistema cuando esa variable no está | El directorio de ejecución del usuario es suyo y solo suyo, vive en memoria y **el sistema lo vacía al cerrar la sesión**. Falta en macOS y en una sesión de Linux sin gestor, y ahí el temporal es la única alternativa que existe en todas partes |
| **Sin el tramo `panels/`** | Un solo componente nuevo: `agent-terminal-<id>` | Un tramo intermedio hay que crearlo —y es el único trozo de la ruta con **nombre fijo**, o sea el único que otro usuario puede dejar preparado de antemano en un temporal compartido, donde el modo del directorio de la hoja ya no protege de un padre ajeno. Sin él, las dos cosas desaparecen |
| **Ocho dígitos y no cuatro** | 32 bits de `getrandom` | Cuatro dígitos son 65.536 nombres. La creación **falla si el nombre ya existe** —así la unicidad no se comprueba, se obtiene— y a ese tamaño una colisión dejaría a una división sin identidad. Ensancharlo cuesta lo mismo |
| Permisos | `0700` explícito | Medido: sin fijarlo el modo sale de la máscara de la sesión, `0775` en la máquina de referencia. Bajo el directorio de ejecución no se alcanzaría, pero el respaldo puede ser un temporal compartido, y ahí va a vivir un fichero que cuenta en qué anda el usuario |
| Quién borra | El propio valor, al morir | Cubre tres caminos que un barrido al apagar no cubre: el error a medio construir de quien lanza la consola, el **pánico** —donde el framework de interfaz no ejecuta su gancho de apagado, comprobado en su fuente, pero los destructores sí corren— y cerrar una división sin cerrar la ventana, que llega en la Fase 2 |
| Cómo borra | El fichero del contrato y después el directorio; **nunca en recursivo** | El directorio es de la aplicación, pero **su contenido es del productor**. Un borrado recursivo se llevaría lo que hubiera dejado ahí y convertiría cualquier ruta mal construida en pérdida de datos en vez de en un error. Si queda algo que no es del contrato, el directorio sobrevive y se dice por dónde |

**Lo que la app fija gana sobre lo que el hijo hereda, y sin directorio se quitan.** No es un detalle
de implementación: el pseudoterminal copia el entorno del proceso padre, así que **lanzar la aplicación
desde dentro de una de sus propias consolas** —lo que va a pasar en cuanto se use a diario— le pasaría
a la consola hija la identidad de la madre, y las dos escribirían en el mismo sitio.

Con directorio se fijan, y eso ya pisa lo heredado. **Sin directorio hay que quitarlas, que no es lo
mismo que no ponerlas**: lo que no se toca se hereda, así que una consola sin panel se quedaría con la
identidad de quien la lanzó, su productor publicaría en el buzón ajeno y el destructor de la madre
borraría el fichero del hijo. Nunca se lee de vuelta el valor heredado; se fija o se borra.

**Quién pone el techo de lo que se inyecta.** La caja del pseudoterminal acepta una lista abierta de
pares, porque no tiene por qué saber qué es un panel; el techo está en que **lo único que se le pasa
sale del contrato**, en forma de dos pares exactos. Así, añadir una variable al entorno del usuario
exige cambiar la caja del contrato, no un sitio de llamada. Es lo que hace comprobable la regla de
[D4](#d4--el-control-de-agentes-va-por-consola-no-por-api): la lista completa de lo que la aplicación
define son cuatro nombres, y hay una prueba que falla si aparece un quinto.

**Si el directorio no se puede crear, la ventana abre igual y no se inyecta ninguna de las dos
variables.** Mismo criterio que el respaldo del shell, y con más motivo: nada dentro de la aplicación
las consume todavía, así que una consola sin directorio sigue siendo un terminal completo. Y **media
identidad le mentiría** a un productor que lea `PANEL_ID` como «me hospeda la app»: publicaría en un
sitio que nadie mira.

**Lo que este hito deja sin hacer, a propósito:** nadie lee el fichero —eso es F1.6, y hasta entonces
el pendiente «descubrimiento por `PANEL_DIR`» del [contrato](04-contrato-panel.md) sigue abierto— y no
hay barrido de directorios huérfanos: un barrido borra por descubrimiento en vez de por «esto lo hice
yo», que es la variedad peligrosa de borrado.

**Un aviso para la Fase 2:** si `PANEL_ID` pasa a ser el testigo con el que el servidor MCP autoriza,
32 bits **no bastan** y habrá que ensancharlo ahí. Aquí no es un testigo: es un nombre.

---

## D8 — El terminal frena al programa: se deja de leer, no se descarta

**Decidido.** Cuando lo que un programa escribe entra más deprisa de lo que la pantalla lo
interpreta, la aplicación **deja de leer el pseudoterminal**. Eso llena el búfer del núcleo y el
programa se bloquea al escribir, como en cualquier terminal. Lo retenido tiene un techo en dos
sitios, y hacen falta los dos:

| Dónde | Cuánto | Qué pasa al llenarse |
|---|---|---|
| El canal de `Pty::spawn`, en `term-core` | 8 lecturas de 8 KiB (64 KiB) | El hilo lector deja de leer del pseudoterminal |
| La cola del puente, en `app` | 1 MiB | El hilo deja de recoger del canal, y así el canal se llena |

**Por qué:** sin esto, un programa que escribe sin parar —`cat /dev/zero`, `cat /dev/urandom`—
agota la memoria de la máquina. Medido antes del arreglo: **+343 MiB/s de crecimiento lineal**, 512
MiB en 1,6 s, sin el menor asomo de estabilizarse. Con el arreglo, la misma carga se queda en
**2,7 MiB durante 30 s** sin perder velocidad de drenaje.

**Por qué en dos sitios y no en uno.** Está medido: acotar solo el canal de `term-core` **no frena
nada**. Un consumidor que vacía el canal en cuanto llega algo hace que ese techo no se alcance
nunca, y los bytes se acumulan igual, solo que un poco más allá. La regla, que vale para cualquier
cola futura de este proyecto:

> Un techo solo frena si quien consume esa cola deja de vaciarla.

**Descartado — descartar bytes al llegar al tope.** Es la salida fácil y la peor: un terminal que
pierde salida en silencio miente sobre lo que el programa escribió, y este proyecto trata el fallo
silencioso como el peor resultado posible. Con el freno no se pierde nada: lo que no cabe se queda
en el programa que lo escribió, esperando.

**Descartado — dejar que lo resuelva quien pinta.** Un presupuesto por fotograma acota el *trabajo*
de un fotograma, no la memoria, y confundir las dos cosas fue el error que llevó a este fallo. El
freno vive en `term-core`, la caja que sobrevive a un cambio de framework de interfaz (D3), y se
expresa en bytes, nunca en fotogramas.

**Consecuencia sobre el cierre, en los dos hilos.** Un hilo que espera sitio no está bloqueado
leyendo, así que no se entera de nada de lo que normalmente lo despertaría. Cada uno necesita su
bandera de parada, y por motivos distintos:

- En `term-core`, `Drop` la levanta **antes** del `kill`, porque el `join` que viene después espera
  a ese hilo: sin ella, **cerrar la ventana se cuelga**.
- En el puente, `Drop` la levanta porque quien hace sitio es quien recoge, y un puente destruido ya
  no va a recoger nunca: sin ella el hilo **gira para siempre**. No cuelga el cierre —nadie lo
  espera— pero deja una fuga por cada división que se cierre en ese instante, que hoy tapa el que
  el proceso muera entero detrás y que dejará de taparse en cuanto haya más de una división.

Las dos comprobadas, no temidas: quitando cualquiera de las dos, su prueba se pone en rojo.

**Por qué no estaba decidido antes.** No se descartó: no se miró. El canal viene de F1.1, donde la
elección fue *«`mpsc` de la biblioteca estándar frente a `crossbeam-channel` frente a un callback»*
con el criterio de no añadir dependencias; acotado frente a sin acotar no llegó a plantearse, y lo
ilimitado es el valor por defecto de `mpsc::channel()`.

---

## D9 — Los símbolos del panel se eligen midiendo la fuente, no de memoria

**Decisión.** Las seis marcas del contrato se pintan con `●` (`done`), `▶` (`current`), `·`
(`pending`), `…` (`wait`), `×` (`block`) y `›` (`info`). No es el juego del prototipo, y la diferencia
no es estética: **es el único juego de seis que la fuente monoespaciada tiene y que mide lo que mide
una celda**.

**Lo medido**, sobre las fuentes por defecto de egui 0.36.1 y a cuerpo 14, donde una celda son 8,43
puntos:

| Símbolo | Ancho | Veredicto |
|---|---|---|
| `✓` U+2713, `✗` U+2717 | **0,00** | no están en la fuente |
| `⏸` U+23F8 | 14,00 | solo en la de emoji |
| `⚠` U+26A0, `✔` U+2714 | 17,77 | solo en la de emoji |
| `✱` U+2731 | 13,00 | solo en la de emoji |
| `▶` `·` `›` `→` `»` `…` `─` `●` `◆` `■` `▪` `◦` `▸` `∙` `∘` | 8,43 | están, y monoespaciados |

**Por qué el ancho y no `has_glyphs`.** Porque ese predicado **no responde lo que su nombre promete**:
por dentro es `resolve_face(c) != replacement_face_key` (`epaint-0.36.1/src/text/font.rs:719`), y con
la familia monoespaciada **la cara base es la de reemplazo**, así que devuelve `false` para todo lo que
la fuente sí tiene — comprobado con la letra `A` de control. Solo devuelve `true` para lo que cae a las
fuentes de emoji, o sea justo para lo que **no** sirve. La primera medida de este hito se tomó con él y
salió invertida: daba por ausentes tres símbolos que están, y por buenos dos que descuadran la columna.

**Lo que se descartó, y por qué.**

- **Portar el juego del prototipo tal cual.** Dos de sus seis no están y dos vienen de la fuente de
  emoji. Un panel con cuadros en su primera columna es exactamente lo que el veredicto sobre egui no
  puede permitirse, porque los cuadros estarían en cada línea.
- **Cambiar a la familia proporcional.** No tiene el ancho fijo del que depende la alineación de la
  columna, así que arreglaría los glifos rompiendo lo que los glifos sirven para construir.
- **Usar los de emoji igualmente.** Miden entre 1,5 y 2 celdas: la columna se descuadra, y eso se lee
  peor que un cuadro — un cuadro parece un fallo de fuente, una columna torcida parece un panel mal
  hecho.

**La salida si el juego se queda corto** de expresividad al verlo en uso: pintar las marcas como forma
—un rectángulo, un círculo— en vez de como carácter. Está fuera del hito a propósito y es su propio
trabajo.

**Cuándo dejaría de valer.** Si cambia la versión de egui o si algún día se embarca una fuente propia
—que es la otra cosa conocida que muerde, porque CJK y los iconos de un `ls` moderno salen como
cuadros—. La comprobación está atada por una prueba que pregunta a la fuente y falla si algún símbolo
deja de medir una celda, así que el día que cambie se enterará quien corra la suite y no quien mire la
pantalla.

### Lo que F2.8 añadió: los tres del reclamo, medidos igual

**Decisión.** El reclamo de una división —el campo `attention` de **D18**— se pinta con `?` (`wait`),
`⊘` (`block`) y `∎` (`done`), y **ninguno de los tres es uno de los seis del cuerpo**. Medidos con el
mismo criterio y en la misma fuente: los tres miden **8,43 puntos**, que es lo que mide una celda.

| Símbolo | Ancho | Veredicto |
|---|---|---|
| `?` U+003F, `⊘` U+2298, `∎` U+220E | 8,43 | están, y monoespaciados |
| `✓` U+2713, `☑` U+2611, `⏳` U+23F3, `⚠` U+26A0, `⛔` U+26D4, `★` U+2605 | 0,00 o de emoji | fuera, por lo mismo de siempre |

**Y por qué no se repiten los del cuerpo, que también miden bien.** Porque el mismo signo en dos sitios
del mismo panel invita a leer que la aplicación **dedujo el uno del otro** — que es exactamente lo que
D18 prohíbe: un `mark: block` en una línea del resumen no es un reclamo de la división. Con el juego
repetido, un panel que solo cuenta que algo se atascó el martes se vería igual que uno que te reclama
ahora.

**La tabla de color es propia, y no la de las marcas.** La de las marcas **apaga** `done` porque lo
terminado es donde ya no queda nada que hacer; aquí `done` dice lo contrario —terminó y **nadie lo ha
mirado**—, así que reutilizarla apagaría justo lo que reclama. *Descartado:* reutilizarla «porque son
los mismos nombres». Los nombres coinciden y el significado no.

**Un reclamo que nadie sostiene se atenúa; no se apaga.** Si la aplicación tiene algo que confesar sobre
ese panel —lleva callado más de lo suyo, o su fichero dejó de leerse—, el reclamo se sigue pintando con
su color atenuado: reclama, y no hay nadie detrás. **Con una excepción, que está en D18**: lo terminado
no se atenúa por callar, porque callar es lo que hace quien terminó. *Descartado:* esconderlo, que contradice el idioma de
este panel —confesar y no esconder, que es la razón de que un productor rancio siga teniendo panel—; y
pintarlo igual, que haría a la ficha de la pestaña mentir sobre lo fresco que es. El detalle —desde
cuándo, o qué falló— lo sigue diciendo el aviso, en su propia línea y con su propio color.

---

## D10 — El ancho del panel: en celdas, de cada división, y plegable con el mismo gesto

**Decisión.** El panel se cuenta **en celdas de la fuente** —ni una fracción del ancho de la división ni
un número de puntos— y **es de cada división**: nace con 36 y desde ahí lo mueve quien mira la ventana,
arrastrando la frontera que separa el panel de su consola. Cuando no cabe todo, el que cede es el panel:
se estrecha hasta desaparecer y la consola conserva su suelo de veinte columnas.

> **Actualizada en F2.4 (agosto de 2026).** Antes decía que el ancho era fijo y no se podía arrastrar.
> Lo que sigue en pie es lo que de verdad decidía —contarlo en celdas y que el que cede sea el panel—; lo
> que cambia es que el número dejó de ser el ancho y pasó a ser **con cuántas celdas nace** una división
> que nadie ha tocado. Las tres decisiones nuevas están abajo, en «Lo que F2.4 añadió».

### Lo que F2.4 añadió, con su razón y lo descartado

**1. El ancho es de cada división, no de la aplicación.** Cuatro divisiones son cuatro productores
publicando cosas de tamaños distintos, y el sentido de poder estrechar el panel es dejar ancho el que
cuenta algo y estrecho el que no. *Descartado:* un ancho global — tendría que ceder al mínimo de la
división más estrecha o mentir en las demás, y el suelo de la consola es por división.

**2. Plegar es un estado aparte, no «ancho cero».** Plegado, la división deja de **pedir** ancho, así que
su consola se queda con el rectángulo entero por el **mismo camino** que una cuyo productor no ha
publicado nada — no hay dos formas de no tener panel que alguien tenga que mantener de acuerdo.
*Descartado:* guardar un cero en el ancho. Perdería el ancho de trabajo, y desplegar tendría que
inventarse un valor cuando lo que el usuario espera es volver **al suyo**.

**3. El gesto tiene un mínimo, y ese mínimo es la raya entre estrechar y plegar.** Pedir menos que una
celda es lo que el gesto lee como «ciérralo». Y plegado **queda un asidero** pegado al borde derecho de
la división, hacia dentro: sin él, la frontera se iría con el panel y el gesto que lo cerró no se podría
deshacer con el ratón. *Descartado:* que el asidero fuera a caballo del borde, como el de una frontera
con panel a la vista — ahí empieza el divisor que reparte las consolas, y al registrarse después le
ganaría el empate y lo dejaría muerto. *Precio aceptado y declarado:* en una consola plegada, esa banda
del borde derecho agarra su panel y no la consola, así que ahí no se la puede coger para llevársela.

**Y lo que no se pinta, a propósito:** la frontera del panel **no tiene línea**. Pintarla descontaría su
grosor del reparto y movería la rejilla de todas las consolas, con lo que las medidas dejarían de ser
comparables con los anexos anteriores — se perdería la comparación para ganar un adorno. Lo que la hace
encontrable es el cursor, y por eso su banda de agarre es más ancha que la línea de un divisor.

**Por qué en celdas y no en puntos.** La petición fue «ancho fijo, sin plegar», y se cumple en lo que
quería decir —no depende del ancho de la ventana y no se arrastra— pero **medido en celdas**. Un ancho
en puntos se estrecha en columnas al subir el aumento del escritorio, y lo que el panel tiene que
conservar es **cuántos caracteres caben**, que es lo que decide si su texto se lee. Un panel que en una
pantalla enseña las líneas enteras y en otra las corta no es el mismo panel.

> Ojo con probarlo: con una celda de 8 puntos, «36 celdas» y «288 puntos» dan el mismo número. Una
> prueba escrita con ese ancho de celda no distingue las dos implementaciones — hay que variar el
> tamaño de la celda para que la diferencia aparezca.

**Por qué el que cede es el panel.** El panel es margen y la consola es el trabajo: en una ventana
estrecha lo que sobra es el estado, no el programa. Con la regla al revés, arrastrar el divisor dejaría
a un programa interactivo sin sitio donde escribir mientras su panel conserva sus treinta y seis
columnas.

**Por qué ajustarlo no entra hasta F2.4.** Plegar y estrechar son **el mismo gesto sobre la misma
frontera** —plegar es arrastrar hasta el final—, así que hacer uno y volver después por el otro sale más
caro que hacer los dos juntos. Y lo que hace falta para cualquiera de los dos es de Fase 2: una segunda
frontera arrastrable por división —hoy solo existe la que reparte las dos consolas— y guardar ese ancho
por división para restaurarlo al abrir, que es F2.5.

**Lo que ya está resuelto para cuando llegue.** El pintado no supone que el ancho sea 36: `apartar()` ya
devuelve panels más estrechos cuando no cabe, y la maqueta guarda **para qué ancho** se envolvió el
texto y solo la rehace cuando ese ancho cambia — o sea que un panel de ancho variable no reenvuelve en
cada fotograma. Lo que queda por decidir es qué pasa con anchos muy pequeños, donde la columna de `ref`
tiene que seguir sin empujar el texto fuera del panel.

**Lo que se descartó, y por qué.**

- **Una fracción del ancho de la división**, como el reparto entre las dos consolas. Al redimensionar la
  ventana el panel crecería y encogería con ella, y con él el punto donde se corta su texto: líneas que
  se leían enteras dejarían de leerse sin que nadie tocara nada.
- **Ancho en puntos** — arriba: fijo en puntos no es fijo en columnas.
- **Que ceda la consola** — arriba.
- **Hacerlo arrastrable ya en F1.6.** Era trabajo de Fase 2 dentro del hito que tenía que responder si el
  panel merece existir, y la respuesta a eso no dependía de poder estrecharlo. Llegó en F2.4.

**Cuándo dejaría de valer.** Lo de las cuatro divisiones ya está medido y respondido en el anexo de esta
misma decisión, y el issue [#33](https://github.com/mashware/agent-terminal/issues/33) quedó cerrado con
esos números. Y el coste del propio gesto también está medido: ver «Medido — lo que cuesta el gesto del
panel», abajo.

### Medido — lo que cuesta el gesto del panel (19 de agosto de 2026)

**Cuatro corridas con el binario que se entrega**: dos de la tanda **K** —dos divisiones, las dos volcando,
y la frontera del panel de la izquierda arrastrándose sin parar— y dos de la **C**, idéntica pero sin gesto.
`scripts/medir-fotograma.sh K` y `... C`, ventana de 1280×800 puntos.

> **Y son las de después de la revisión, no las primeras.** La revisión de este PR cambió el binario —cuatro
> defectos en el gesto y en lo que el informe publica— así que las cuatro corridas anteriores describían un
> programa que no es el que se entrega. Es la segunda vez que pasa en este proyecto y la lección es la misma:
> **la tanda se corre al final, después de la revisión**.

> **El control está DENTRO de cada corrida, y eso es lo que hace la resta honesta.** El gesto solo agarra la
> frontera de la división **izquierda**; la derecha tiene su panel publicado y no lo toca nadie. Así que las
> dos comparten fotograma, máquina y momento, y la diferencia entre sus tramos de panel es lo que cuesta el
> gesto — sin depender de comparar dos corridas distintas.

| Corrida | Gestos · efectos | Panel, división **con** gesto | Panel, división **sin** gesto | Diferencia |
|---|---|---|---|---|
| K, 1.ª | 958 · 1.918 | **35,562 µs** (107,3 el 95 %) | 12,238 µs (7,2 el 95 %) | 23,3 µs |
| K, 2.ª | 870 · 1.740 | **33,721 µs** (97,3 el 95 %) | 2,821 µs | 30,9 µs |
| C, 1.ª | — | 4,264 µs | 2,615 µs | 1,6 µs |
| C, 2.ª | — | 3,632 µs | 2,190 µs | 1,4 µs |

**Lo que el gesto cuesta: entre 23 y 31 µs por fotograma**, en la división que lo recibe. Sobre el hueco de
60 Hz son **dos décimas del 1 %**; sobre el de 120 Hz, cuatro. Y el tramo cubre las tres cosas del gesto:
atender la frontera, volver a repartir el ancho y **rehacer la maqueta del panel**.

**La horquilla es ancha a propósito, y el motivo está en la tabla.** En la primera K la división **sin**
gesto marca 12,2 µs de media con un 95 % de 7,2: la media por encima de su propio percentil es la firma de
un pico único, no de un régimen. Con un suelo que se mueve así, publicar «el gesto cuesta 30,9 µs» con tres
cifras sería fingir precisión.

**La vecina no paga el gesto.** 2,2 a 2,8 µs en tres de las cuatro corridas, con gesto y sin él; la cuarta
es el pico de arriba. Estrechar un panel no le cuesta al panel de al lado, que es lo que cabía esperar de un
ancho que es de cada división.

**Los efectos son el doble que los gestos, y conviene saber por qué antes de leer el número:** 1.740 para
870 —el doble exacto— y **1.918 para 958, que son dos de más**. **No es un conteo doble** — se sondeó porque
una proporción tan redonda lo parecía, y hay una prueba que fija el número por gesto. Un gesto tiene dos
fotogramas de arrastre y **cada uno empuja la frontera otras ocho celdas**, porque el punto al que va el
puntero se calcula desde donde la frontera está *ahora*: el ancho recorre 36 → 28 → 20 y vuelve con el gesto
siguiente. Así que son **dos repartos y dos maquetados reales por gesto**, y esos 30 µs ya los incluyen.

> **Y los dos que sobran tienen explicación, que es lo que hay que decir cuando un número no cuadra.** El
> informe cuenta los gestos **completos**, o sea los que llegaron a soltar el botón; los efectos se cuentan
> por fotograma. Un gesto que produjo sus dos fotogramas de arrastre y al que la ventana cerró antes de
> soltar deja sus dos efectos contados y su propio gesto sin contar. La corrida de 870 acabó entre dos
> gestos; la de 958, a mitad de uno.

> **Lo que esta tanda NO mide, y hay que decirlo:** el gesto avanza a saltos de ocho celdas, así que **no hay
> ningún fotograma en que la frontera cruce una sola celda**, que es lo que hace un ratón humano moviéndose
> despacio. Lo medido es el arrastre rápido: menos maquetados, cada uno con un salto de ancho grande. Un
> arrastre lento haría más maquetados de menos recorrido cada uno, y esa forma no está en estos números.

> **AVISO, el mismo que lleva el anexo del gesto de F2.3 y por la misma razón: las medias por fotograma de K
> y C NO se comparan entre sí.** Inyectar entrada mantiene la ventana despierta, así que K corrió 3.844 y
> 3.490 fotogramas contra los 468 y 376 de C — de ocho a diez veces más. Se ve en el «resto del fotograma»,
> que en K sale **más barato** (144 y 135 µs) que en C (240 y 232 µs) simplemente porque se reparte entre
> miles de fotogramas sin trabajo de consola dentro. Lo que sí se compara es lo de la tabla: dos divisiones
> del **mismo** fotograma.

**Un punto ciego que esta tanda hace visible.** La rejilla que el informe publica es la del **último**
fotograma, y en una tanda con este gesto el ancho de la consola izquierda está oscilando: las dos corridas
de K acabaron con 47 y 39 columnas para la misma división. No es una anomalía, es el punto ciego del
instrumento que ya estaba declarado — y aquí obliga a algo práctico: **el comparador de corridas rechaza dos
tandas K por rejillas distintas, y hace bien**. Dos K no se promedian; se leen por separado.

**La vuelta completa no se mueve:** 75,1 y 74,9 ms con gesto, 73,7 y 74,0 sin él. Está dominada por la
espera en la cola —72 de los 75— así que el gesto no aparece ahí, ni tendría por qué: treinta microsegundos
sobre setenta y cinco milisegundos son cuatro diezmilésimas.

---

## D12 — El pseudoterminal se queda con el procesamiento de salida del núcleo, aunque cueste

**Decidido.** El pseudoterminal **no** toca `termios`: `OPOST` se queda encendido y el núcleo sigue
convirtiendo cada salto de línea en retorno de carro más salto. Se paga el coste. **Esta decisión se
tomó al revés primero, se entregó, y se revirtió al descubrir por qué no puede funcionar** — lo que
sigue es esa autopsia, que vale más que la decisión.

**Lo que cuesta, medido en este repo** con `cargo run --release --example delivery -p term-core`,
volcando doce megas con `cat` por un pseudoterminal real:

| Línea | Procesando en el núcleo | Sin procesar | Relación |
|---|---|---|---|
| de log, 70 B | 53,3 MiB/s | 208,2 MiB/s | **4×** |
| corta, 2 B | 2,4 MiB/s | 270,0 MiB/s | **112×** |

**El titular del issue —«casi dos órdenes de magnitud»— es cierto y necesita un adverbio.** Lo que se
paga es **cada salto de línea**, no cada byte: con líneas de log son cuatro veces y con líneas de dos
bytes, ciento doce. Un solo número parece universal y no lo es. El ejemplo se queda en el repo aunque
la optimización no: mide un coste real que sigue ahí.

**Por qué no se puede quitar, que es lo que había que averiguar.** Apagar `OPOST` obliga a reponer el
retorno de carro desde fuera, y **eso exige saber si el programa de dentro lo quiere**. El núcleo lo
sabía porque traducía según el `termios` de cada uno; desde fuera hay que deducirlo, y **la
información no está**:

| Estado del terminal | ¿El núcleo traduce? | Quién lo deja así |
|---|---|---|
| `OPOST` on, `ONLCR` on | sí | nadie lo ha tocado |
| `OPOST` off, `ONLCR` on | **no** | `stty raw`, `cfmakeraw`, `tty.setraw`, `enable_raw_mode` de crossterm, `MakeRaw` de Go |
| `OPOST` off, `ONLCR` on | **no** | **y también nosotros, si lo apagáramos** |

Las dos últimas filas son **el mismo estado, bit a bit** (comprobado ejecutándolo). El modo crudo
estándar apaga **solo** `OPOST` y deja `ONLCR` puesto, que es exactamente lo que dejaría un apagado
nuestro. Así que no hay forma de distinguir «nadie ha tocado nada, tradúceme» de «he entrado en modo
crudo, no me traduzcas», y a todo programa que entra en crudo por la vía normal —cualquier TUI de
`ratatui`, de Node o de Go— se le inyectaría un retorno que pidió no tener.

> **La regla que deja, y es general:** en cuanto le mientes al programa sobre el estado de su
> terminal, **pierdes la información de quién puso qué**. No es que falte una señal mejor: es que el
> núcleo la destruye al aceptar la escritura. Por eso ningún emulador de terminal apaga `OPOST`.

**Lo que se probó y no bastó.**

- **Condicionar la traducción a `ONLCR`.** Es la señal que usa `ncurses` —su manual dice que solo usa
  el salto como movimiento después de `nonl()`— y **cubre a `vim`, `htop` y `nano`**, que lo apagan al
  arrancar. No cubre a nada que no sea `ncurses`. Esta fue la versión que se entregó y se revirtió.
- **Comparar el `termios` entero contra el que dejamos.** Falla igual: `stty raw` produce nuestro
  mismo `c_oflag`.
- **Añadir `ICANON` a la condición.** Arregla el modo crudo y rompe a un programa de `ncurses` en
  `cbreak`, que apaga `ICANON` sin apagar `ONLCR`. Cambia un agujero por otro.
- **Encender el modo `LNM` del motor de terminal** (`ESC[20h`), que hace esto en una línea.
  `reset_state()` lo borra —el modo por defecto al que vuelve no lo incluye— así que un `reset` en la
  consola dejaría la escalera **permanente**. Y un modo es estado que el programa puede pisar:
  reponer con una preferencia algo que garantizaba el núcleo.

**Lo que se aprendió y no se tira.**

- **La ventana de emular al leer algo que se hacía al escribir.** Aunque la señal fuera fiable, el
  trozo que ya está en el búfer del núcleo se traduciría con la regla de *ahora*. Cota: los ~20 KiB
  del búfer; y **con contrapresión no son microsegundos sino segundos**, porque el hilo lector está
  parado esperando sitio. Medido: 17 KiB de un log en escalera al abrir `vim` detrás de un `cat`
  grande.
- **Cómo medir esto sin engañarse**: generar las líneas con un `while` de `sh` y `printf` da 12 MiB/s
  con el interruptor puesto de las dos formas, porque el cuello es el bucle del shell. Sale «1×» y
  entierra el hallazgo con un número tranquilizador y falso. Hay que volcar un fichero con `cat`.
- **Y el cuello de esta aplicación, de paso**: el núcleo entrega ~358 MiB/s sin procesar y por el
  canal de `term-core` salían ~170. Si algún día el coste del pseudoterminal importa de verdad, hay
  más que ganar ahí que en `termios`.

**Cuándo reabrir esto.** Si aparece una forma de saber qué quiere el programa **sin** mentirle sobre
el estado de su terminal. Hoy no la hay, y el issue
[#27](https://github.com/mashware/agent-terminal/issues/27) queda con esta autopsia dentro.

## D13 — Las pestañas son una lista de disposiciones, y las que no se ven trabajan igual

**Qué se decidió.** La ventana pasa a ser una **lista de pestañas** más el índice de la que se ve.
Cada pestaña se lleva lo que es de una disposición —sus dos divisiones, cuál tiene el foco y dónde
está el divisor— y la ventana conserva lo que es del proceso: el tema, el vigilante, la medida y los
modificadores del teclado. **Lo que una disposición es cambió en F2.2**, y la frontera que esta decisión
traza aguantó el cambio: donde aquí se leen «dos divisiones y dónde está el divisor», ahora hay un árbol
n-ario con pesos y el foco es un nombre en vez de una posición. Lo que se quedó arriba, en la ventana, no
se movió. Ver **D14**. La mitad lógica del fotograma recorre **todas** las pestañas; la de
interfaz pinta **solo** la que se ve.

**Por qué.** Es el primer hito de la Fase 2 porque **decide la forma del estado** sobre la que
escriben el árbol de divisiones (F2.2), el arrastre (F2.3) y la persistencia (F2.5). Y porque el
problema que el proyecto resuelve —varios agentes a la vez— no cabe en dos consolas: hace falta un
contexto por tarea.

**Lo que se descartó, y por qué:**

- **Esconder la disposición detrás de una frontera de métodos.** Obligaba a meter dentro del pintado
  dos funciones que se extrajeron a propósito para poder probarse **sin abrir ventana**, y cuatro de
  esos métodos existían solo para no indexar. `Pestaña` es un **contenedor transparente**: sus tres
  campos se ven desde la ventana. El beneficio de la frontera se cobraba en F2.2, y adelantar trabajo
  de la fase siguiente es justo lo que las reglas del proyecto prohíben.
- **Bajarle el presupuesto de bombeo a las pestañas que no se ven.** Es incompatible por
  construcción con lo que el hito promete: el margen que el núcleo regala antes de bloquear al
  programa son **20 KiB**, seis veces menos que el propio presupuesto de 128 KiB, así que recortarlo
  no hace que la consola de fondo vaya más despacio — **hace que su programa se pare al escribir**.
  Las de fondo se bombean con el mismo presupuesto y el fotograma se alarga: el reparto lo hace el
  propio fotograma, sin mecanismo, y el precio se paga en fotogramas por segundo de toda la ventana.
- **Invertir el orden con F2.2** (árbol de divisiones antes que pestañas). Un árbol sin pestañas no
  vale por sí solo, y el orden ya estaba justificado en el roadmap.
- **Una serie propia en el informe** para lo que interpretan las consolas que no se pintan, y un
  contador de pestañas cerradas. Los dos se cortaron por no tener consumidor.
- **Cerrar la última pestaña dejando la lista vacía.** Pedir el cierre no cierra en el acto —siguen
  corriendo fotogramas y el informe se publica al salir—, así que una lista vacía dejaría el índice
  de la activa apuntando fuera de rango en cada uno de ellos. La última pestaña no se quita: lo que
  se cierra es la ventana.

**Lo que costó, y conviene que quede escrito.** Los números del informe estaban mal en cinco sitios
distintos en cuanto hubo pestañas, y ninguno se veía desde fuera: el fotograma se cerraba dos veces
cuando egui repetía la pasada —la guarda contaba pasadas y no fotogramas—, la segunda anotación
machacaba la buena y borraba lo interpretado, abrir una pestaña lanzaba dos procesos que nadie
declaraba, cerrarla se llevaba el tiempo que su consola ya había medido, y las etiquetas del informe
se renumeraban al cerrar una pestaña, de modo que una tabla de números podía acabar titulada con el
nombre de otra. **La lección es del método, no del código:** en un instrumento que reparte un total
por resta, cada camino nuevo que aparece —una pestaña que no se pinta, un atajo que crea procesos— es
una fuga posible, y ninguna se ve leyendo el informe, porque el número sale plausible.

### Medido — el fotograma con pestañas (16 de agosto de 2026)

> **Estas cifras son de antes del historial.** Desde que cada consola guarda 3 500 líneas, interpretar
> cuesta el doble y la vuelta completa con ello (**D21**). Las mismas tandas vueltas a tomar con el
> historial encendido están en el anexo «las cuatro tandas de los anexos, vueltas a tomar con historial»
> de **D21**; lo que sigue se conserva porque es lo que se midió aquel día y es lo que sostiene las
> conclusiones de esta decisión.

Tres tandas, **dos corridas cada una**, en release, sin tocar la máquina, con
`scripts/medir-fotograma.sh C|F|G`. Las tres con los cuatro paneles publicándose a ritmo de una vez
por segundo, para que se puedan comparar entre sí. Carga de 44 MiB por consola que vuelca.

| | **C** — 1 pestaña | **F** — 2, la de fondo quieta | **G** — 2, la de fondo volcando |
|---|---|---|---|
| Rejilla de cada consola | 61×21 | 59×21 | 59×21 |
| Consolas volcando | 2 | 2 | **4** |
| Vuelta completa | 76,4 ms · 76,4 | 75,1 · 76,9 | **90,3 · 95,1** |
| — esperando en la cola | 71,0 · 71,0 | 70,1 · 71,3 | **81,0 · 85,4** |
| — interpretando | 2,21 · 2,17 | 1,97 · 2,25 | 2,03 · 2,24 |
| — encolando el pintado | 371 µs · 351 | 360 · 346 | 361 · 369 |
| — el panel de estado | 7,6 µs · 7,0 | 6,5 · 7,4 | 7,1 · 7,6 |
| **el resto del fotograma** | **386 µs · 409** | **466 · 399** | **399 · 403** |

**Qué prueba esto, que es lo que la decisión mandaba medir:**

1. **El coste del framework de interfaz no se mueve al añadir una pestaña.** El «resto del
   fotograma» —teselar, dibujar y el armazón de egui— se queda entre **386 y 466 µs** en las seis
   corridas, y la variación entre las dos corridas de una misma tanda (386 → 409 en C, 466 → 399 en F)
   es del mismo orden que la variación entre tandas. Dicho al revés: **el número con el que D3 juzga a
   egui no empeora por tener pestañas**, ni siquiera con cuatro consolas volcando a la vez.
2. **La barra sale gratis.** F le quita a cada consola dos filas de rejilla frente a C —61 pasa a
   59— y el fotograma no se distingue del de C en ningún tramo. Pintar las fichas y repartir el alto
   no cuesta nada medible.
3. **Lo que sí cuesta es el trabajo de fondo, y se paga en la cola.** En G la vuelta completa sube de
   76 a **93 ms**, y **casi todo el aumento está en esperar** (71 → 83 ms). Es el presupuesto haciendo
   su trabajo: cuatro divisiones tragan 128 KiB cada una por fotograma, así que con el doble de
   consolas volcando cada byte espera más su turno. Interpretar, encolar y el panel **no se mueven**,
   que es lo que confirma que no hay coste nuevo por división.

**Y qué NO prueba, que importa igual:**

- **No dice lo que le cuesta a la consola de fondo interpretar sus 44 MiB.** Su bloque del informe
  sale **vacío**, y es por decisión: una consola que no se pinta declara su vuelta **sin espera** para
  no ensuciar las cuatro medias con muestras de una vuelta que nadie vio. Su tiempo **sí se descuenta**
  del resto —eso es lo que arregló el hito—, pero no hay número publicado de su trabajo. Lo que la
  tanda G mide es el **efecto** del trabajo de fondo sobre las consolas de delante, no su coste
  directo.
- **No es un fotograma de 120 Hz.** Con dos consolas volcando a plena carga el fotograma entero está
  muy por encima del hueco de 8,3 ms; estos números describen el **peor caso sostenido**, no el uso
  normal. La comparación válida es entre tandas, no contra el presupuesto de un refresco.
- **No dice nada de cuatro pestañas.** Todo esto es con una y con dos. Con cuatro divisiones visibles
  el panel se comería la consola entera y habría que volver a medir
  ([#33](https://github.com/mashware/agent-terminal/issues/33)).
- **La rejilla de estas tandas es de 21 columnas por consola**, no de 57: el panel se lleva 36. No son
  comparables con los anexos de D3 anteriores a los paneles.

**Un número de campo que vale la pena conservar:** en la tanda G la cabecera publica la pestaña de
fondo con rejilla **59×21**, la misma que la visible. Antes de este hito habría dicho **24×80** —la
rejilla de arranque— porque el tamaño solo se calculaba al pintar. Es la comprobación en la
aplicación real de que un agente en una pestaña que no miras trabaja con la ventana que de verdad
tiene.

---

## D14 — La disposición de una pestaña es un árbol n-ario que reparte por pesos

**Qué se decidió.** Dentro de una pestaña, las consolas viven en un **árbol**: una hoja es una
división y una rama es una **orientación** más una lista de hijos **con peso**. Cuatro reglas lo
definen, y las cuatro se eligieron por lo que evitan:

1. **N-ario, no binario.** Una rama tiene los hijos que hagan falta, así que tres consolas en fila son
   **tres hermanas** y cada frontera mueve solo a sus dos vecinas.
2. **Partir en el mismo sentido que la rama que te contiene inserta como hermana**, en vez de crear una
   rama nueva. Es lo que mantiene la regla 1 después del segundo corte.
3. **Pesos, no fracciones normalizadas.** El reparto divide cada peso por la suma de los de su rama, así
   que «los pesos suman 1» no es una invariante que mantener: cerrar un hijo es quitarlo de la lista y
   el resto se reparte solo, en proporción a lo que ya tenía.
4. **Cerrar deshace lo que cerrar puede crear.** Quitar una hoja colapsa la rama que se quede con una
   sola hija, y si esa hija reparte **en el mismo sentido** que la rama de fuera, sus hijas **suben a
   hermanas** con el peso escalado. Sin esto, la regla 1 se rompe en tres gestos.

El foco es el **nombre** de una división, no su sitio; y cada frontera se identifica por su **posición**
en la lista del fotograma, como ya hacía el clic. Los atajos son `Ctrl+Mayúsculas+D` (partir al lado),
`+B` (partir debajo) y `+X` (cerrar la división).

**Por qué.** Tres razones, y ninguna es de gusto:

- **El divisor que mueve un bloque.** Es lo que más se le critica a `tmux`: con tres consolas en fila, un
  árbol binario anida, y la frontera de fuera arrastra **dos consolas a la vez**. El n-ario las hace
  hermanas. Está medido en este proyecto, no supuesto: con la forma anidada, una consola que esa frontera
  no debería tocar pasa de **335,6 a 504,0 puntos**.
- **La invariante sale gratis.** Que lo pintado cubra el área **sin huecos ni solapes** lo garantiza la
  estructura; con rectángulos libres pasaría a ser código que mantener.
- **F2.3 sale casi hecho.** Arrastrar una división a otro sitio es «quitar» e «insertar», y este hito
  construye las dos primitivas de todas formas. Con rectángulos libres, recolocar sería un algoritmo de
  empaquetado.

**Lo que se descartó, y por qué:**

- **Una lista de columnas** (`Vec<Division>` con pesos), repitiendo la jugada de `Vec<Pestaña>`. Sería la
  más corta y **no cumple el hito**: solo da una fila, y la Parte I del roadmap pide «colocadas como
  quieras». Habría que rehacerla en F2.3.
- **El árbol binario a secas**, por el divisor que mueve bloques.
- **Una rejilla de rectángulos en fracciones**, que pierde la invariante que el árbol regala.
- **Un techo al número de divisiones.** No se puede justificar con un escenario real: partir exige un
  gesto humano por división, y la ventana se vuelve inusable por geometría —el suelo de columnas se niega
  a partir lo que no puede dar a las dos hijas su mínimo— mucho antes de agotar memoria (~1,06 MiB de
  colas por división) o descriptores. El límite sale de la geometría, no de una constante inventada.
- **Nombre estable para cada rama.** Se justificó diciendo que el identificador de control de egui tiene
  que ser estable entre fotogramas, y el precedente lo contradice: el clic identifica cada división por
  su índice y llevaba un hito funcionando. **Queda un caso conocido**: con el ratón apretado sobre una
  frontera y el acorde de partir con la otra mano, la partición renumera las fronteras y el arrastre en
  vuelo pasa a mover otra. Son dos manos, el efecto es visible y se corrige soltando. Se reabre en F2.3,
  que es donde recolocar vuelve a mover posiciones en caliente.
- **Una guarda «si la suma de los pesos es cero, reparte a partes iguales».** Deja vivo el estado
  inválido y confía en que todos los caminos pasen por la guarda. En su lugar, el tipo `Peso` rechaza lo
  no finito y lo no positivo, así que la suma cero **no se puede construir**; y arrastrar no deja bajar a
  ningún vecino del 1 % de la suma de los dos. Ese suelo es relativo a los dos vecinos y no al área, así
  que no hornea la geometría de hoy en el estado.

  **Corrección, hecha al implementar F2.5, y hace falta escribirla porque el párrafo de arriba se lee como
  si el 1 % fuera un suelo y no lo es.** El uno por ciento es de **la fracción**, así que el peso que deja
  un arrastre es el uno por ciento de lo que suman los dos vecinos — y esa suma **encoge con cada
  arrastre**. Arrastrando todas las fronteras de una fila al tope, los pesos convergen a un punto fijo
  donde cada uno es la centésima parte del siguiente: **con veintidós consolas en fila, el más pequeño
  vale 10⁻⁴¹**, y de ahí el reparto —que multiplica dos pesos— llega a cero en un solo paso. Medido, no
  razonado. Así que **el reescalado lleva suelo propio** (`Peso::escalado`): los tres sitios donde un peso
  sale de otro —arrastrar, partir y el reescalado de cerrar— recortan el resultado al menor peso normal.
  El estado imposible deja de existir donde se produce, que es lo que este proyecto prefiere a una guarda
  aguas abajo. Y la consecuencia que se descubrió con esto: **el formato en disco no puede exigir un
  mínimo mayor**, o rechazaría disposiciones que un gesto produce — ver D16.
- **Copiar los acordes de otro terminal para partir.** No se puede: **no coinciden entre ellos** — lo que
  en uno parte al lado, en otro parte debajo. Se eligieron por lo que significan aquí (*derecha*, *bajo*).
  Para cerrar sí hay precedente que no se contradice —la `x` de `tmux` y de Zellij matan el panel—, así
  que cerrar la división es `X`. **`W` se queda con la pestaña**, que es lo que hace en cualquier
  navegador, y ninguno de los tres toca a `C` ni a `V`, que son del portapapeles. Los tres llevan guarda
  de repetición porque cada uno lanza o mata programas: sin ella, un segundo con el acorde apretado
  cierra las consolas, la pestaña y la ventana.

**Lo que costó, y conviene que quede escrito.** El hito se entregó en siete PRs y **lo que rompió no fue
lo que se construía**: el PR que enseñó a **partir** cerró catorce hallazgos bloqueantes y **trece estaban
en código que ese PR no escribía**, y el que enseñó a **cerrar** cerró tres, todos fuera del suyo. Lo que rompe es **lo que la operación nueva vuelve alcanzable** — tres
divisiones donde el informe indexaba un array de dos lados (pánico al salir, o sea perdiendo todo lo
medido), dos ramas hermanas donde la búsqueda de frontera cortaba por «cambió» en vez de por
«encontrada», una pestaña de una sola consola donde el texto «izquierda» dejó de describir nada. Y en el
instrumento, la misma lección con otro traje: **lo que una consola lleva sin declarar al cerrarse vive en
tres sitios** —lo anotado en su medida, lo que el bombeo dejó interpretado y lo que costó releer su
panel—, y mirar solo el primero declaraba **cero** justo en el fotograma del acorde. Cada camino nuevo es
una fuga posible, y ninguna se ve leyendo el informe: el número sale plausible.

### Medido — el fotograma con cuatro divisiones (17 de agosto de 2026)

> **Estas cifras son de antes del historial.** Desde que cada consola guarda 3 500 líneas, interpretar
> cuesta el doble y la vuelta completa con ello (**D21**). Las mismas tandas vueltas a tomar con el
> historial encendido están en el anexo «las cuatro tandas de los anexos, vueltas a tomar con historial»
> de **D21**; lo que sigue se conserva porque es lo que se midió aquel día y es lo que sostiene las
> conclusiones de esta decisión.

Tres tandas, **dos corridas cada una**, en release, sin tocar la máquina, con
`scripts/medir-fotograma.sh C|H|I`. Las tres con **todos** los paneles publicándose a ritmo de una vez
por segundo y carga de 44 MiB por consola, para que se puedan comparar entre sí. Los tramos son de la
**división 0** de cada corrida.

**La ventana se pide, y eso es nuevo.** Todas las corridas son de **1280×800 puntos**, pedidos con
`MEDIR_TAMANO` y comprobados en el informe. Antes de este hito el tamaño lo decidía el escritorio, y eso
hacía la medida irreproducible sin que nada lo dijera: el mismo día, tres corridas de la misma tanda
dieron **36×47, 40×66 y 61×57** de rejilla. Por eso **estos números no se comparan con los anexos
anteriores** —los suyos se tomaron con ventanas cuyo tamaño nadie apuntó— y sí entre ellos.

| | **C** — 2 divisiones | **H** — 4 en fila | **I** — 4 en cuadro |
|---|---|---|---|
| Rejilla de cada consola | 49×39 | **49×20** | **24×39** |
| Celdas en total | 3 822 | 3 920 | 3 744 |
| Consolas volcando | 2 | **4** | **4** |
| Vuelta completa | 52,0 ms · 51,6 | **100,2 · 101,4** | **97,2 · 99,4** |
| — esperando en la cola | 46,5 · 46,2 | **89,9 · 91,1** | **87,2 · 89,3** |
| — interpretando | 2,12 · 2,13 | 2,18 · 2,07 | 2,20 · 2,20 |
| — encolando el pintado | 458 µs · 464 | 316 · 320 | 259 · 256 |
| — el panel de estado | 6,1 µs · 6,5 | 15,4 · 18,4 | 25,8 · 8,9 |
| **el resto del fotograma** | **444 µs · 402** | **530 · 513** | **503 · 463** |

**Qué prueba esto, que es lo que quedaba por medir:**

1. **El coste del framework de interfaz no empeora al duplicar las divisiones.** El «resto del
   fotograma» —teselar, dibujar y el armazón de egui— se queda entre **402 y 530 µs** en las seis
   corridas, y la variación entre las dos corridas de una misma tanda (444 → 402 en C, 503 → 463 en I) es
   del mismo orden que la variación entre tandas. Es la misma conclusión que el anexo de D13 sacó para
   las pestañas, ahora para las divisiones, y **cierra la pregunta que el issue
   [#33](https://github.com/mashware/agent-terminal/issues/33) dejó abierta**. Las tres tandas tienen casi
   las mismas celdas en total (3 744 a 3 920), que es justamente lo que las hace comparables: lo que
   cambia es en cuántas consolas están repartidas.
2. **Lo que cuesta el trabajo de fondo se paga en la cola.** La vuelta completa pasa de ~52 ms con dos
   consolas volcando a 98 y 101 ms con cuatro, y **casi todo el aumento está en esperar** (46 ms en C, 88 en
   I y 91 en H). Interpretar **no se mueve** (2,07 a 2,20 ms en las seis corridas), que es lo que confirma que no hay
   coste nuevo por división: es el presupuesto de 128 KiB por división haciendo su trabajo, con el doble de
   consolas tragando del mismo fotograma.
3. **Encolar el pintado baja, y baja con las celdas.** 458 µs con 1 911 celdas por consola, 316 con 980,
   259 con 936. No es una mejora: es que cada consola pinta menos.
4. **El panel cede y la consola no, comprobado en el peor caso.** Lo **medido** es esto: con las cuatro
   **en fila** cada consola publica **20 columnas** de rejilla —exactamente `panel::MINIMUM_CONSOLE`—, y en
   **cuadro** publica las **39** de la tanda de dos. Y de ahí se **deduce** el reparto, que la cabecera no
   publica: en la tanda C cada división son 39 columnas de consola más las 36 del panel, o sea 75; en fila
   le toca la mitad de eso, unas 37, así que el panel no cabe y **cede hasta 17** para dejarle a la consola
   su suelo de 20. En cuadro lo que se parte por la mitad es el **alto** —24 filas en vez de 49— y el ancho
   no se toca, así que la disposición que alguien usaría de verdad no le quita ni una columna a la consola.

**Y qué NO prueba, que importa igual:**

- **No dice nada de una ventana grande.** En esta máquina el escritorio deja llegar hasta **5 120×1 357
  puntos** —comprobado pidiendo más y viendo qué concede—, y la tanda mide 1280×800. Con cuatro en fila a
  lo ancho de esa ventana el panel entraría entero y la conclusión 4 sería otra: la interesante es la que
  se midió, porque es la que aprieta.
- **El tramo del panel son microsegundos y su relación entre corridas no significa nada**: 25,8 y 8,9 µs en
  las dos corridas de la misma tanda I. Está publicado por completitud, no para leer tendencias.
- **No mide partir ni cerrar en caliente.** Las tandas abren la disposición y vuelcan; el congelón de
  ~100 ms al cerrar ([#62](https://github.com/mashware/agent-terminal/issues/62)) sigue sin medirse con
  divisiones, y este hito recorre ese camino mucho más a menudo que antes.
- **No es un fotograma de 120 Hz.** Con cuatro consolas volcando a plena carga la vuelta completa está muy
  por encima del hueco de 8,3 ms: describe el **peor caso sostenido**, no el uso normal. La comparación
  válida es entre tandas.

**Una corrida se marcó inválida mientras se afinaba el guion, y conviene que quede escrito por qué.** Una
tanda I publicó una ventana de **1 279,8125×799,8125** puntos habiendo pedido 1 280×800: el compositor
entrega el tamaño con fracción y **no siempre la misma**. La guarda del guion la tiró con una comparación exacta, y ahora
lleva **medio punto de margen** —el mismo que la aplicación usa para no pelearse con el escritorio—,
porque medio punto no mueve ni una celda de la rejilla y sin él la guarda descarta corridas idénticas.

## D15 — Mover una división transforma el árbol por valor, y solo lo sustituye si la transformación llega al final

**Qué se decidió.** Una división se **agarra con el ratón** y se **suelta a un lado de otra**; al soltar,
la disposición se reordena para hacerle sitio. Cinco decisiones lo definen:

1. **La operación se hace por valor y es atómica por construcción.** `Nodo::mover` recibe el árbol
   entero, lo transforma y solo se sustituye el original **si la transformación llega al final**. No hay
   ningún camino en el que una consola quede viva y fuera del árbol.
2. **El destino es una de las cuatro mitades de otra división**, y cuál se elige por la distancia al borde
   **en proporción al lado que le toca** —multiplicando en cruz, no dividiendo—. En puntos, una consola
   ancha y baja solo se dejaría soltar por arriba y por abajo.
3. **Antes de soltar se ve la mitad que la consola va a ocupar**, y no una franja fina al borde: es
   literalmente el sitio que ocupará. Una franja se leería como el gesto de acoplar a la pantalla de los
   gestores de ventanas, que aquí no existe.
4. **El velo no promete lo que se va a rechazar.** El suelo geométrico —el mismo que ya usa partir— se
   consulta **antes** de señalar un destino, así que donde soltar se vaya a negar no se marca nada.
5. **Las divisiones se identifican por su nombre, y el identificador con el que egui sostiene el arrastre
   también.** Las fronteras siguen por posición.

**Por qué.**

- **Componer «cerrar» con «partir» no da atomicidad, y no por descuido de implementación.** `colapsar`
  descarta el peso de la hija superviviente y `absorber` escala los de las nietas, así que **tras quitar
  una hoja la forma anterior ya no se puede reconstruir**: si partir fallara después, no hay a dónde
  volver y la consola se queda fuera. Por valor, el fallo es no haber cambiado nada.
- **Y una operación que muta en el sitio con validación previa también funcionaría — mientras todos se
  acuerden de validar.** Este proyecto tiene contado con números lo que pasa con las invariantes que
  dependen de que alguien se acuerde: 15 de 17 hallazgos bloqueantes de los dos hitos anteriores estaban
  en código que esos PRs no escribían. Transformar por valor la hace **imposible de romper**.
- **El suelo se comprueba contra el árbol de ahora**, y es conservador: quitar una hoja casi nunca le quita
  área a las que quedan —`absorber` les cuesta un divisor, y nada más—. La formulación intuitiva
  —«validar contra el árbol posterior a quitar»— **es incoherente**: no se puede validar contra un árbol
  que todavía no existe.
- **La identidad no se refactorizó, y con esto queda cerrado el caso que D14 dejó abierto para este hito.**
  Las hojas ya tenían nombre estable; lo que faltaba era que el identificador del control lo llevara
  **también**. Con el sitio dentro, cualquier cosa que renumere mientras el botón sigue apretado hace que
  el arrastre en vuelo apunte a otra cosa — y no hacen falta dos manos: agarrar una consola y pulsar
  `Ctrl+Mayúsculas+→` sin soltar **reordenaba dos consolas de la pestaña de destino**. Las fronteras
  siguen por posición porque su arrastre **no sobrevive a nada**: se resuelve dentro del mismo fotograma.

**Lo que se descartó, y por qué:**

- **Componer `cerrar` + `partir`** desde la ventana: no son atómicos, por lo de arriba.
- **Mutar en el sitio con validación previa**: funciona, pero la atomicidad pasa a depender de cada camino
  futuro.
- **Cerrar la consola y volver a abrirla en el destino**: mata el trabajo de dentro, que es el motivo del
  hito. Y está medido: cerrar en caliente congela la ventana **50,72 ms** de mediana, **404 ms** si el
  programa ignora `SIGHUP`, y puede colgarla indefinidamente.
- **`Result<Nodo, Nodo>` para el resultado de mover.** Era el diseño; se cambió a una tupla porque la
  variante de error cargaba el árbol entero y porque `Err` no significaba siempre lo mismo. Una tupla no
  invita a leer «falló» como «no cambió nada».
- **Deshacer un movimiento.** No hace falta: un movimiento que no puede completarse no llega a empezar.
- **Mover una división a otra pestaña.** Fuera de F2.3 por la Parte I del roadmap, no por dificultad.
- **Nombre estable para las fronteras.** Ver arriba: su arrastre no cruza fotogramas.

### Medido — el fotograma mientras se arrastra (18 de agosto de 2026)

Dos tandas, **dos corridas cada una**, en release, sin tocar la máquina, con
`scripts/medir-fotograma.sh C|J`. Las dos con una pestaña de **dos divisiones**, los dos paneles
publicándose a ritmo de una vez por segundo, carga de 44 MiB por consola y ventana de **1280×800 puntos**,
con las dos consolas a 49×39 celdas. La **J** añade una sola cosa: **la ventana se arrastra a sí misma** de
principio a fin —agarra una consola, la lleva sobre la otra y la suelta, alternando el lado— por la misma
puerta por la que entra el ratón de verdad. Los tramos son de la **división 0**.

| | **C** — sin gesto | **J** — arrastrando sin parar |
|---|---|---|
| Gestos inyectados | — | **1 724 · 1 722** |
| De ésos, los que reordenaron | — | **862 · 861** |
| Fotogramas con salida | 358 · 363 | 357 · 357 |
| **Fotogramas en total** | **467 · 532** | **6 898 · 6 888** |
| Vuelta completa | 37,2 ms · 37,1 | **37,3 · 38,5** |
| — esperando en la cola | 35,0 · 35,0 | **35,1 · 35,6** |
| — interpretando | 849 µs · 763 | 813 · 1 058 |
| — encolando el pintado | 205 µs · 189 | 163 · 234 |
| — el panel de estado | 3,6 µs · 3,6 | 2,6 · 4,6 |
| el resto del fotograma | 222 µs · 220 | 112 · 105 |

**Qué prueba esto:**

1. **Arrastrar sin parar no empeora nada de lo que la consola percibe.** La vuelta completa —lo que tarda un
   byte disponible en llegar a un fotograma pagado— es **37,1 a 38,5 ms en las cuatro corridas**, y la
   variación entre las dos corridas de la misma tanda (37,3 → 38,5 en J) es del mismo orden que la variación
   entre tandas. Esperar en la cola tampoco se mueve: **35,0 a 35,6 ms**. Y los fotogramas **con salida** son
   los mismos (357 a 363), o sea que el presupuesto de bombeo hace su trabajo igual.
2. **El gesto declara su coste.** Atender el ratón, pintar el velo, rehacer el reparto al soltar e inyectar
   los eventos están **todos** cronometrados y restados, así que ninguno se publica como coste del framework
   —la cifra con la que D3 juzga a egui—. La revisión encontró una quinta pieza que se había quedado fuera:
   la copia de los rectángulos del último fotograma. No se cronometró: se **quitó**, moviendo el vector en
   vez de copiarlo, que es mejor que declarar trabajo que no hace falta hacer.
3. **El resto del fotograma se queda muy por debajo del presupuesto** incluso arrastrando: 112 y 105 µs de
   media, 258 y 302 µs el 95 %.

**Y qué NO prueba, que importa igual:**

- **Las medias del «resto del fotograma» de C y J NO se comparan entre sí, y la tabla las pone juntas solo
  para que se vea por qué.** Inyectar eventos de puntero mantiene la ventana despierta —igual que la mantiene
  despierta un ratón humano moviéndose—, así que J corre **unas trece veces más fotogramas** que C. El coste
  medio por fotograma baja porque se reparte entre miles de fotogramas que no llevan trabajo de consola
  dentro, no porque el gesto abarate nada. Leer «112 µs contra 222» como una mejora sería exactamente el
  número plausible que este proyecto lleva tres hitos evitando. Lo mismo vale para interpretar, encolar y el
  panel. La conclusión 1 se apoya en la vuelta completa, que es por fotograma **con salida**.
- **La mitad de los gestos se descartan al soltar, y el informe lo dice: 1 724 inyectados y 862 que
  reordenaron.** No es un fallo del gesto: el inyector agarra la consola **por su hueco**, así que tras el
  primer intercambio vuelve a agarrar la misma —que ahora está en el otro hueco— y la suelta encima de sí
  misma, caso que el gesto rechaza a propósito. La tanda mide por tanto una **mezcla** de los dos caminos, y
  los dos son reales al arrastrar. Sondeado con instrumentación en el camino del soltado, no deducido.
- **Publicar solo los gestos inyectados habría dejado pasar una tanda vacía**, y esto lo encontró la
  revisión: en una ventana estrecha el suelo geométrico rechaza **todos** los soltados, la disposición no
  cambia ni una vez, y una corrida así declaraba «1 724 hechos» y se daba por buena. El anexo habría
  publicado que el gesto no cuesta nada. Ahora el guion invalida la corrida si ninguno reordenó.
- **La etiqueta «(izquierda)» o «(derecha)» de cada tabla de tramos describe el hueco en el que la consola
  acabó**, y en la tanda J una consola cambia de hueco a mitad de corrida: la mitad de las muestras se
  tomaron con ella en el otro lado. Los números no salen mal porque las dos consolas se reparten el ancho a
  medias y tienen la misma rejilla, pero la etiqueta afirma una procedencia que la corrida no sostiene, y
  dejaría de ser inocua en una tanda de anchos distintos.
- **No hay tanda con arrastres de cuatro divisiones**, y no es un olvido: el ciclo del gesto solo devuelve la
  disposición a la de partida **con dos**. Con más, insertar parte el peso del destino y quitar lo reparte
  entre las que quedan, así que el peso se concentra — medido con cuatro en fila sobre 1 200 puntos, la
  primera consola pasa de **298,5 a 119,4** puntos en un solo ciclo de ida y vuelta.
- **El número de fotogramas de la tanda J no es estable, y con él tampoco el de gestos.** Tres corridas
  seguidas dieron **6 898, 6 888 y 6 854** fotogramas —muy parejas— pero una cuarta, tomada mientras la
  máquina compilaba, se quedó en **764** con solo 191 gestos. Los fotogramas **con salida** fueron los mismos
  en las cuatro (357 a 359), que es lo que hace que la conclusión 1 aguante: se apoya en la vuelta completa,
  que es por fotograma con salida, y no en nada dividido por el total. Es otra razón para no comparar las
  medias del resto entre corridas, no solo entre tandas.
- **No mide el gesto de una persona.** Mide un gesto sintético que agarra el centro de una consola, cruza en
  dos fotogramas y suelta. Un arrastre humano dura más y se mueve más despacio.
- **No mide partir ni cerrar en caliente**, igual que el anexo anterior: el congelón de ~100 ms al cerrar
  ([#62](https://github.com/mashware/agent-terminal/issues/62)) sigue sin medirse con divisiones.

**Dos cosas que la primera versión de la tanda hizo mal, y quedan escritas porque los números salían
plausibles.** La primera: el gesto pedía repintado en cada fotograma para no dormirse a mitad de un arrastre,
y con eso la tanda dejaba de ser «la de comparación más el gesto» para ser otro régimen —**2 219 y 7 008
fotogramas** con el resto en **163 y 103 µs**, o sea el gesto *abaratando* el fotograma a la mitad—. La
segunda: contar los gestos inyectados como si fueran reordenaciones, que es la que habría dejado pasar la
tanda vacía de más arriba. Las dos las cazó la revisión de este PR, no la medida.

---

## D16 — La disposición se guarda en un fichero propio, y lo que no se entiende abre de fábrica

**Qué se decidió.** Al cerrar la ventana y volver a abrirla vuelven **las pestañas, el árbol de
divisiones con sus pesos, y por cada consola el ancho de su panel y si estaba plegada**. Se guarda en
un fichero JSON propio, `$XDG_CONFIG_HOME/agent-terminal/disposicion.json` —con caída a
`$HOME/.config/…`—, y **nada más**: ni el directorio de trabajo de cada consola, ni el tamaño de la
ventana, ni qué pestaña estaba activa. Nueve decisiones lo definen:

1. **Fichero propio, y uno solo para toda la máquina.** No una disposición por directorio de
   lanzamiento: eso metería «el proyecto» como concepto y ninguna fase lo ha definido.
2. **La ruta tiene que ser absoluta, o no se persiste.** Solo se acepta una variable de entorno **no
   vacía y absoluta**; si ninguna resuelve, **no se lee ni se escribe nada** y la ventana abre de
   fábrica. **No hay caída a un directorio temporal**, al contrario de lo que hace el contrato del
   panel.
3. **Se escribe con retardo de 500 ms, contado desde el primer cambio no escrito**, no desde el último.
   Y hacen falta **dos** referencias, con papeles distintos: el plazo se **arma** cuando la disposición
   difiere de la **última vista** —o sea cuando algo cambia—, y al vencer se **escribe** si difiere de la
   **última escrita con éxito**. Con una sola no funciona ninguno de los dos extremos: comparando
   siempre contra lo último escrito, un directorio no escribible deja la diferencia puesta y la ventana
   no se duerme nunca; y guardando el intento fallido como si hubiera salido bien, **un fallo no se
   reintenta jamás** si la disposición no vuelve a cambiar, y se pierde todo desde la última escritura
   buena en vez de medio segundo.
4. **La escritura es atómica**: se escribe un temporal en el **mismo directorio** que el destino
   —`disposicion.json.<pid>.<intento>`, para que dos instancias no se pisen el temporal, y **creado en
   exclusiva**: si ese nombre ya existe **no se adopta**, porque el temporal es lo que se renombra
   encima del destino y un enlace plantado ahí se escribiría a través— y se renombra encima.
   El número de intento se añadió al implementar, y por un escenario que esta decisión no había visto:
   **el número de proceso se recicla**. Un temporal que sobreviviera a una muerte violenta —entre crearlo
   y renombrarlo— no lo limpia nadie, así que con el nombre dependiendo solo del proceso, **toda sesión
   futura a la que el sistema le diera ese número se quedaría sin poder guardar nada**, para siempre y con
   un solo aviso que quien abre desde un icono no ve. Contando los intentos, un temporal huérfano estorba
   **un intento** y no una sesión; y no se adopta ninguno, que era el punto.
   El destino nunca contiene un documento a medias. Permisos `0600` en el fichero y `0700` en su
   directorio. Si el renombrado falla, se borra el temporal y se avisa **una vez por sesión y por
   motivo**; lo que reintenta es el cambio siguiente.
5. **Una consola restaurada estrena identidad**: `PANEL_ID` y `PANEL_DIR` nuevos, panel vacío hasta que
   alguien publique.
6. **Tolerancia por documento, no por campo.** Lo que no se entiende se descarta **entero** y la
   ventana abre de fábrica —una pestaña, una consola— sin impedir el arranque.
7. **La versión del formato es obligatoria.** Una **mayor** que la conocida **no se sobrescribe en toda
   la sesión**. Una **conocida y más vieja** se lee con el lector de esa versión y se reescribe en la
   nueva al primer guardado — y **retirar el soporte de una versión vieja es una decisión aparte, que se
   escribe**. Sin esa segunda regla, un fichero de la versión anterior caería en «forma inválida» y se
   machacaría, que es el daño exacto que la versión existe para evitar.
8. **Dos techos y ninguno más**: **64 KiB** de fichero —porque tiene tamaño arbitrario, y leerlo sin
   tope es reservar sin tope en el arranque— y **128 hojas** de documento, comprobadas **recorriendo el
   árbol antes del primer `fork`**. Y **si un `fork` falla a mitad, no se instala nada**: la ventana abre
   de fábrica, la misma atomicidad que D15 — media ventana restaurada es un estado que nadie ha
   diseñado. En el documento, **hoja y rama llevan etiqueta explícita** (`hoja` / `rama`), no se deducen
   de qué clave está presente: así el fallo dice qué y dónde.
9. **El fichero guarda geometría y nunca instrucciones.**

**Por qué.** Hoy la ventana abre siempre igual, así que todo el trabajo de F2.1 a F2.4 —pestañas por
contexto, las divisiones que quieras colocadas como quieras, cada panel con su ancho— **se pierde en
cada cierre**, y colocar la ventana es un gesto que hay que repetir cada mañana.

Y las decisiones de arriba no son gustos. Cada una tiene detrás algo ya medido en este proyecto:

- **El retardo son 500 ms, y el número no es al azar**: es el mismo que el respiro con el que la ventana
  relee los paneles cuando el vigilante se ha quedado ciego, que ya está en el código. Con él, un
  arrastre continuo escribe **dos veces por segundo** en vez de sesenta, y lo que se pierde al morir
  —medio segundo de gesto— no se nota.
- **El retardo se cuenta desde el primer cambio y no desde el último porque, contado desde el último,
  no escribe nunca mientras el usuario arrastra.** La disposición cambia **una y otra vez mientras el
  botón sigue apretado**: mover una frontera reparte pesos en cuanto el puntero se mueve, y la del
  panel en cuanto cruza una celda —su ancho se cuenta en celdas enteras, así que no cambia en cada
  fotograma, pero sí muchas veces por gesto—. Rearmar el plazo en cada cambio lo empuja mientras el
  gesto dure: uno de diez segundos se pierde entero. Contado desde el primero, ese mismo gesto escribe
  una vez por retardo.
- **La escritura es atómica porque no existe un cierre limpio garantizado.** La aplicación se cierra
  por el gestor de ventanas o por señal, y **cerrar una consola congela la ventana 50,7 ms de mediana**
  —una pestaña de dos consolas, unos 101 ms; y **404 ms esa misma consola** si el programa de dentro
  ignora la señal de cuelgue— y puede colgarla
  ([#62](https://github.com/mashware/agent-terminal/issues/62)). Guardar solo al salir habría
  perdido la sesión entera en el caso normal.
- **La identidad es nueva porque conservarla sería deducir el estado.** Un panel restaurado con el
  `panel.json` de ayer pintaría **como actual** el último estado de un agente que ya no existe:
  «compilando» de una compilación que murió anoche. La app no deduce el estado (**D6**), y esto sería
  deducirlo por omisión.
- **La tolerancia es por documento porque este fichero lo escribe siempre la aplicación.** Es lo
  contrario de lo que hace el contrato del panel, y a propósito: aquel lo escribe un tercero a mano y
  perder el panel entero por un campo que nadie mira sería lo peor posible; aquí **adivinar media
  disposición es peor que no restaurarla**.
- **La versión entra el primer día porque el fichero congela la forma del árbol en disco.** Sin ella,
  el primer cambio de forma mete al fichero viejo en el mismo cajón que al corrupto y **lo borra en
  silencio**. Sube solo con lo que un lector viejo malinterpretaría; añadir una clave opcional no la
  sube. Y una versión mayor no se sobrescribe porque tener dos binarios en la máquina no es raro, y
  machacar el fichero sería que el viejo se lleve por delante la disposición del nuevo.
- **El techo de hojas existe aunque D14 descartara un techo al número de divisiones, y no la
  contradice.** Partir exige un gesto humano y **la geometría se niega antes de crear nada**; restaurar
  bifurca N shells **antes de que exista geometría** —al abrir no se sabe el tamaño de la ventana—. El
  suelo que protege a partir está **aguas abajo del `fork`**. Es techo **del documento**, no de
  divisiones.
- **Y el techo de hojas no se deriva de una geometría concreta**, porque no la hay: sale de que el
  techo de bytes deja pasar **del orden de un millar de hojas** —entre seiscientas y mil según lo
  compacto que venga el documento—, y **el número se elige contra el recurso que el techo guarda, no
  contra lo que cabe en pantalla**. Cada hoja restaurada es un proceso, un pseudoterminal, sus
  descriptores y ~1,06 MiB de colas: mil hojas serían más de un gigabyte de colas **antes de que exista
  geometría**, o sea el techo convertido en el desastre que venía a evitar. **128 hojas** son unos 136
  MiB, un número acotado y declarado, y siguen estando por encima de las **del orden de cien consolas**
  que caben en la ventana con la que se mide —siete columnas de veinte celdas por quince filas de tres,
  con el cuerpo de 14—, que es la única ventana de la que este proyecto tiene un tamaño escrito. **No se
  publica una cifra exacta de hojas por byte a propósito**: depende de la
  forma del documento, no de su tamaño.
- **El fichero no guarda instrucciones porque ahí está la frontera de lo sensible.** El lanzamiento de
  una consola deja de ser inofensivo «en cuanto el programa o sus argumentos procedan de
  **configuración**», y este fichero es configuración. Guarda geometría; el tipo con el que se lee **no
  tiene ni un campo de texto libre**, y restaurar llama al constructor que lanza el shell del usuario,
  nunca al que acepta un programa por parámetro. El día que algo de eso cambie, hay que revisar el
  lanzamiento como superficie sensible y tratarlo como un `panel.json` ajeno.

**Lo que se descartó, y por qué:**

- **El almacén de persistencia que trae el framework de interfaz.** Cede **la ruta del fichero y la
  cadencia del guardado** —las dos cosas que este hito decide— y mete el estado del producto dentro de
  la pieza que **D3** declara sustituible. Además exigiría **activar una funcionalidad que hoy no está
  pedida** —no viene de serie— y que arrastra al árbol un formato de serialización y un resolutor de
  directorios que nada más necesita.
- **Guardar el registro de gestos que construyó el árbol** en vez del árbol. Tentador porque las
  operaciones ya rechazan lo que no cabe, así que validar saldría gratis. **No cubre la mitad del
  hito**: el ancho y el plegado del panel no nacen de partir ni de mover, viven en la consola
  (**D10**), así que el espejo por consola hace falta igual.
- **Tolerancia por campo**, por lo dicho arriba.
- **Un techo de profundidad del árbol.** El intérprete de JSON ya corta en 128 niveles, y **la opción
  que desactiva ese corte no está pedida en ningún manifiesto del repo**, así que un techo propio
  **pasaría su prueba por construcción** — el disfraz que este proyecto ya tiene fichado. Y con la invariante «una rama tiene
  dos hijos o más», el techo de 128 hojas ya acota profundidad, nodos, pestañas e hijos por rama: **un
  número hace los cuatro trabajos**.
- **Un tope al ancho de panel leído.** El reparto ya lo recorta y hay prueba que lo ata.
- **Una guarda contra el valor sin número en los pesos.** JSON no tiene literal para él, y el tipo del
  peso ya rechaza lo no finito y lo no positivo, así que **la suma cero no se puede construir**.
- **Un mínimo propio para los pesos leídos.** Lo hubo —`1e-6`— y **era un defecto, no una guarda**:
  rechazaba disposiciones **que un gesto produce**. Arrastrar recorta la *fracción* al uno por ciento, no
  el peso, así que lo que deja es una parte de lo que suman los dos vecinos, y esa suma encoge con cada
  arrastre; con una fila larga se baja de `1e-6` sin esfuerzo, y **el suelo de columnas hace que en
  pantalla no se note nada**. O sea: la ventana se veía bien, el fichero se escribía, y al reabrir se
  descartaba entero con un aviso que quien abre desde un icono no ve. La razón por la que ese mínimo
  existía —que el reparto multiplica dos pesos y el producto puede subdesbordar— **es cierta, y su sitio
  es el reescalado**, no el lector: ver la corrección de D14. **El tope por arriba se queda**, porque un
  fichero escrito a mano sí puede traer un peso que ningún gesto produce.
- **Forzar la escritura a disco antes de renombrar.** La garantía que hace falta es «nunca se ve medio
  fichero», y eso lo da el renombrado solo; sobrevivir a un corte de luz no lo pidió nadie, y ese
  empujón dentro del fotograma son milisegundos en un disco ocupado.
- **Borrar el directorio de panel de la sesión anterior.** En la rama normal vive bajo el directorio de
  ejecución del usuario, que **el sistema vacía al cerrar sesión**, así que no hay nada que limpiar y
  construir la limpieza sería un mecanismo sin escenario. **Donde no hay tal directorio** —macOS, o
  Linux sin gestor de sesión— **cae al temporal del sistema, que NO se vacía**: lo dice el propio
  destructor del tipo, que avisa precisamente por eso. Y ese destructor **no corre cuando la aplicación
  se mata o la cierra el gestor de ventanas**, que es el modo de cierre en el que esta decisión se apoya
  más arriba. Así que en esa rama **se acumulan directorios vacíos**, y eso va escrito abajo como límite
  en vez de taparse con una limpieza que la rama normal no necesita.
- **Una variable de entorno propia para que las tandas de medida no toquen el fichero real.** Basta con
  que el guion apunte la variable del directorio de configuración al de la corrida: nada más en el
  código la lee. Una variable menos y un contrato menos.
- **Guardar el tamaño y la posición de la ventana.** Restaurarlos devolvería el tamaño a algo que
  cambia entre sesiones, y de ahí salieron **tres rejillas distintas el mismo día**, que es lo que hace
  incomparables las medidas de las que vive D3. Conviene no confundirlo con lo que ya pasa: **hoy no
  hay tamaño por defecto** —si nadie lo pide, lo elige el escritorio—, y los 1280×800 son de la tanda
  de medida, no del arranque.
- **Guardar el directorio de trabajo de cada consola.** Saber dónde está un proceso hijo se lee del
  sistema de ficheros virtual de Linux y no existe igual en macOS ni Windows: adelantaría trabajo de
  F2.7 o abriría una vía por plataforma. Y sería **una ruta del fichero que acaba en el lanzamiento**,
  o sea la línea de arriba.
- **Guardar qué pestaña estaba activa y qué consola tenía el foco.** Abre en la primera.

**Lo que el hito añade al informe de medida, y por qué se decide aquí y no al implementarlo.** El coste
del fotograma se reparte **por resta**, así que todo trabajo que no se declara se publica como coste del
framework — la cifra exacta con la que D3 juzga a egui. Este hito abre dos caminos dentro del fotograma
y los dos se declaran:

- **Comparar el espejo de la ventana con lo último escrito**, que corre **en cada pasada de interfaz y
  sobre la ventana entera**.
- **Escribir el fichero** cuando vence el plazo. Su cronómetro abarca **desde copiar la disposición hasta
  que el renombrado vuelve** —o hasta que el temporal queda borrado, si falló—: la copia, la validación y
  la serialización van dentro, no solo la llamada de escribir. **Se precisó al implementar**, porque el
  texto anterior decía «desde resolver la ruta» y con cien consolas la copia y el recorrido de validación
  pueden dominar la muestra: un tramo mal delimitado se acaba atribuyendo al disco.
- Y el reparto entre los dos: **lo que no llega a tocar el disco no es una escritura**. Un plazo que vence
  sin nada nuevo que guardar, y una disposición que el formato no admite, se cuentan en el tramo del
  espejo, que es lo que son —comparar y validar—. Así el tramo de la escritura describe **solo** lo que
  cuesta escribir, y el informe puede publicar «tantas escrituras de tantos intentos» sin mezclar.

**Y cada uno necesita DOS cosas, no una, porque restar y publicar son mecanismos distintos.** El resto
del fotograma se calcula restándole a lo que costó entero **lo que los tramos declararon**, y lo que
alimenta esa resta son los tramos de cada división más el acumulador de la ventana. **Una serie solo
guarda muestras para imprimirlas: no resta nada.** Así que un tramo con serie y sin acumulador queda
publicado y **sin descontar** —su coste sigue apareciendo como coste del framework—, y un tramo en el
acumulador y sin serie queda descontado y **sin ninguna fila que auditar**. Los dos caminos nuevos
entran **en el acumulador y además con serie propia**.

**Y el precedente del tramo del panel no se hereda, conviene decirlo:** ese viaja **dentro de los tramos
de una división**, que es un campo por división; el espejo es de **la ventana entera** y no tiene ese
vehículo. Quien lea «como el panel» hará lo que hizo el panel y se quedará sin la resta.

**El del espejo, además, acumula por pasada y anota una sola muestra al cerrar el fotograma**: anotarla
por pasada mete dos muestras en el fotograma que egui repite, y entonces la media describe «por pasada»
y no «por fotograma».

Y **leer el fichero al arrancar no entra en ningún fotograma**, a propósito: se interpreta antes de que
la ventana exista. No hay tramo que declarar porque no hay coste que atribuir.

**Y la tanda tiene que poder delatarse, con el reparto que este proyecto ya usa: el informe declara, el
guion invalida.** El **informe** publica cuántas veces se escribió el fichero —ese número vive dentro
del proceso y el guion no puede saberlo—, y el **guion** invalida la corrida si no se escribió ninguna,
igual que ya invalida la que no publicó ningún panel o la que no completó ningún arrastre: sin ese
número, una serie vacía se lee como «esto no cuesta nada». El guion apunta además el directorio de
configuración **al de la corrida y lo borra al empezar cada una**, o la corrida siguiente arranca con la
disposición que dejó la anterior. Y **lo que la tanda pide explícitamente manda sobre lo que se
restauró**: si la corrida pide dos pestañas y cuatro divisiones, eso es lo que abre, aunque el fichero
diga otra cosa.

**Dos consecuencias de eso, y las dos hay que escribirlas porque si no se descubren tarde:**

- **El plazo del guardado es una razón nueva para que la ventana despierte**, y eso cambia **cuántos
  fotogramas tiene una corrida**. El anexo de D15 ya avisa de que una tanda con más fotogramas **no
  compara sus medias por fotograma**; este hito reintroduce ese mismo confundido, así que se declara en
  el anexo en vez de descubrirse leyéndolo.
- **Con las dos reglas de arriba juntas, ninguna corrida restaura nada** —se borra el directorio, y lo
  que la tanda pide manda—, así que **el camino más caro que añade el hito quedaría excluido de la medida
  por construcción**. Hace falta **una tanda propia** que escriba un fichero de disposición conocido
  **antes** de abrir y lance **sin pedir pestañas ni divisiones**, para que gane el fichero, y cuyo
  informe declare **cuántas consolas se restauraron** y lo que costó restaurarlas. Sin ella, «el coste de
  bifurcar N consolas ya está pagado» es una afirmación que **ninguna corrida sostiene**.

**Seis límites aceptados, escritos porque ninguno se mitiga y todos se descubren usándolo:**

- **Un fallo de escritura solo se reintenta con el cambio siguiente, no con el tiempo.** Es la cara buena
  de las dos referencias —lo que reintenta es el cambio, así que la ventana no se queda despierta
  reintentando ni hace falta inventar una espera—, y su precio: si el disco falla a las diez y el usuario
  **no vuelve a mover nada**, la colocación de ese día no llega nunca al fichero. Lo dice una línea por la
  salida de error, que quien abre desde un icono no ve.

- **Dos instancias abiertas se machacan la disposición**: la última que guarde se lleva el fichero. Es
  consecuencia directa de que la disposición sea una y global, y ponerle un cerrojo entre procesos
  sería un mecanismo mayor que el hito.
- **Restaurar no revalida la geometría.** Un árbol guardado en una pantalla grande y abierto en una
  pequeña deja consolas apretadas por debajo de sus mínimos. Se acepta porque la alternativa
  —revalidar y caer a fábrica— **tiraría la disposición del usuario por abrir el portátil sin el
  monitor**, que es peor producto; y hay atenuante ya construido: **el que cede es el panel** (D10), así
  que la consola conserva su suelo mientras el panel tenga algo que dar.
- **Quien abre la aplicación desde un icono del escritorio no ve la salida de error**, así que para él
  todos los fallos de lectura son silenciosos. Es carencia aceptada: F2.5 no estrena interfaz, y es la
  razón de no ampliar el conjunto de fallos más allá de lo que cabe en una línea.
- **Cada consola vuelve en el directorio desde el que se lanzó la aplicación, no donde estaba.** Es lo
  primero que nota cualquiera: con cuatro consolas en cuatro repositorios vuelve el árbol exacto y los
  anchos exactos, y cuatro shells en el sitio equivocado. Y la razón de dejarlo fuera **no** es que
  adelantaría otro hito: es que **saber dónde está un proceso hijo se lee de una interfaz que solo
  existe en Linux**, así que hacerlo crearía trabajo por plataforma — que es precisamente lo que F2.7
  existe para acotar. Y sería además **una ruta que sale del fichero y acaba en el lanzamiento**, o sea
  la línea de arriba.
- **En una máquina sin directorio de ejecución de sesión** —macOS, o Linux sin gestor— **los directorios
  de panel se acumulan**: ahí el respaldo es el temporal del sistema, que no se vacía, y el destructor
  que los borra no corre cuando la aplicación se mata o la cierra el gestor de ventanas. Con identidad
  nueva por consola restaurada, son tantos directorios vacíos como consolas por arranque. No se mitiga
  aquí porque en la rama normal no hay nada que mitigar, y una limpieza propia sería un mecanismo sin
  escenario en el 99 % de los casos.

### Medido — 24 agosto 2026: lo que cuesta abrir con las consolas que dejaste

**La tanda `L`**, que es la que este hito añade y la única que recorre el arranque restaurado: siembra un
fichero de disposición conocido —una pestaña con **cuatro consolas en fila**— y lanza **sin pedir
pestañas ni divisiones**, para que mande el fichero. **Su control es `H`**, que abre esas mismas cuatro
pidiéndolas, y se tomó **el mismo día y con el mismo binario**: las cifras de `H` del anexo de D14 son de
agosto y de otro binario, así que compararse con ellas mezclaría el coste de restaurar con todo lo que ha
cambiado desde entonces. Las cuatro corridas publican la **misma rejilla** —49×20 por consola— que es lo
que las hace comparables.

| | **L** — restaurada | **L** (2.ª) | **H** — pedida | **H** (2.ª) |
|---|---|---|---|---|
| Consolas restauradas del fichero | **4** | **4** | — | — |
| — leer el fichero, interpretarlo y validarlo | **35,1 µs** | **35,2 µs** | — | — |
| — abrir la ventana con ellas | **5,261 ms** | **5,506 ms** | — | — |
| Vuelta completa (división 0) | 49,4 ms | 47,2 | 57,6 | 45,7 |
| **el resto del fotograma** | **401 µs · 393 fot.** | **379 · 410** | **461 · 403** | **370 · 410** |
| mirar si la disposición cambió | 652 ns | 688 | 920 | 659 |

**Las tres cosas que esto prueba:**

1. **Restaurar cuesta 5,42 ms una sola vez, y ocurre entero antes del primer fotograma.** De ahí,
   **35 µs** son leer el fichero —el **0,65 %**— y el resto es abrir la ventana con esas consolas:
   **1,35 ms por consola**. Ese segundo tramo **no es solo el `fork` y el `exec`**: incluye armar el
   vigilante del directorio de cada consola y describir el árbol entero para el primer espejo, que son
   los otros dos trabajos que crecen con el número de consolas.
2. **No deja ningún rastro en el fotograma, y ésa era la pregunta.** El «resto del fotograma» —la cifra
   con la que D3 juzga a egui— sale **401 y 379 µs** restaurando y **461 y 370 µs** pidiendo: los mismos
   números, con los dos rangos solapados. El tramo del espejo tampoco se mueve (652-920 ns en las
   cuatro). Es lo que el diseño afirmaba sin medirlo, y ahora lo sostiene una corrida.
3. **El informe declara de dónde salieron las consolas**, y el guion **invalida la corrida** si no lo
   dice. Sin esa línea, una tanda en la que el fichero no llegara a mandar publicaría la ventana de
   fábrica bajo etiqueta de restaurada: mismo número de consolas, misma rejilla, y ninguna guarda
   saltando. Es el mismo disfraz que el testigo evita con los paneles y la cuenta de efectos con los
   gestos. **Y el comparador también la mira**, porque `L` y `H` son idénticas en geometría por
   construcción: sin ese dato daba luz verde a promediar una con otra.

**Y tres avisos para quien vuelva a estas cifras:**

- **La primera versión de este anexo publicó 4,45 ms, y estaba mal etiquetada.** El cronómetro se
  paraba cuando los procesos existían, no cuando la ventana estaba construida, así que dejaba fuera el
  vigilante de cada consola y el recorrido del árbol — **dos trabajos que crecen con N**, dentro de un
  número que dice medir justo eso. Lo cazó una revisión, y la diferencia es del **15 %**. Queda escrito
  porque es la regla dura del proyecto vista en directo: lo que no se declara se publica como coste de
  otro.
- **La vuelta completa dispersa mucho entre corridas de la misma tanda** —49,4 contra 47,2 en `L`, 57,6
  contra 45,7 en `H`— y **la dispersión es de la máquina, no del hito**: aparece igual en las dos tandas.
  Lo que sí se puede leer es la **comparación entre tandas de la misma pareja**, que es para lo que están.
- **Una corrida de `H` se descartó por contaminada**, y conviene saber cómo se cazó: publicaba **5,7 s**
  de media en la cola con un percentil 95 de 76 ms —una sola muestra dispara la media— y su informe
  traía una sección de **latencia de teclado** que las otras no tienen. Alguien pulsó una tecla mientras
  corría. Es el aviso de «no toques la máquina» convertido en algo que se puede comprobar después: **si
  el informe habla de teclado, la corrida no vale**.

---

## D17 — El contrato es el documento, no el fichero: la fuente de verdad es el estado de la app

**Qué se decidió.** `panel.json` deja de ser **la** definición del contrato y pasa a ser **un
transporte** de un documento que también puede llegar por otros. Tres partes:

1. **El contrato es el documento de panel** — `phase`, `lines`, marcas, estilos, caducidad. Eso es lo
   que se versiona y lo que nadie puede romper. Dónde vive y cómo viaja es aparte.
2. **La fuente de verdad es el estado que la app tiene por división**, no el fichero. El fichero
   alimenta ese estado; no lo es.
3. **Cada entrega trae una fuente y un orden.** Quién publica (`fichero`, `mcp`, y lo que venga) y un
   número creciente por fuente. Dos productores sobre la misma división dejan de pisarse por azar.

**Por qué, y la razón no es teórica.** El plan ya tenía **dos escritores** del mismo sitio: el
productor por fichero y el agente por `panel_set` de MCP en la Fase 3 (**D5**). Sin fuente ni orden,
quien manda es quien escribió el último, que es la peor forma posible de decidirlo — depende de la
planificación del sistema operativo. El síntoma no aparece hoy porque solo hay un productor real, y
por eso conviene decidirlo ahora y no cuando muerda.

**Y una razón de origen, que conviene admitir.** El primer productor fue un agente que escribe
ficheros, y para eso el fichero es imbatible. Las tres razones que justificaban elegirlo —aislamiento
de fallos, se lee con `cat`, cruza SSH sin protocolo propio— siguen siendo buenas, pero **ninguna
dice por qué tiene que ser el único**. La decisión del medio se tomó con un solo tipo de productor a
la vista.

**Los candidatos, medidos por lo único que decide: qué tiene que hacer el productor para publicar.**

| | Fichero | MCP | Socket | OSC por el PTY | API HTTP |
|---|---|---|---|---|---|
| Publicar desde un script | `cat >` | no | necesita cliente | `printf` | `curl` + puerto |
| Publicar desde un agente | ya lo hace | sí, si habla MCP | no | sí | sí |
| Cruza contenedor / SSH | solo con volumen montado | no | no | **gratis, sin configurar nada** | con red y puerto |
| Sobrevive al productor | **gratis** | si la app lo guarda | si la app lo guarda | si la app lo guarda | si la app lo guarda |
| Sabe quién escribe | **no, nadie** | sí, por sesión | sí, credenciales del proceso | **sí, por construcción: es su PTY** | solo si lo autenticas |
| Avisa al productor de que su documento está roto | **no** | sí | sí | no | sí |
| Superficie de ataque | quien escriba en el directorio | acotado por `PANEL_ID` | quien alcance el socket | **un `cat` de un fichero con secuencias publica lo que quiera** | cualquiera en la red |

**Lo que se decide con esa tabla:**

- **El fichero se queda de serie**, y no por inercia: es el único donde el productor más tonto
  posible publica sin escribir código, y el único donde el estado sobrevive a que el productor muera
  sin que nadie programe nada. Para un contrato cuyo valor depende de que alguien publique, el listón
  de entrada es la propiedad más importante que tiene.
- **MCP entra en la Fase 3 entregando el mismo documento al estado de la app, sin escribir el
  fichero.** Escribirlo sería que la app se detecte a sí misma por el vigilante, un rodeo con un
  fotograma de retraso y sin ninguna ventaja.
- **OSC por el PTY queda como candidato a evaluar después de F2.7**, y es el que tapa el agujero que
  hoy no tapa nadie: un CI dentro de un `ssh` dentro de la división, sin volumen montado ni puerto.
  Es además **el único transporte donde el acotado por división no se puede falsear**, porque llega
  por el PTY de esa división y no por un identificador que el emisor elige. Su límite lo enseña cmux:
  por ahí caben avisos, no cuerpos grandes ([07-panorama.md](07-panorama.md) → E6).

**Lo que se descartó, y por qué:**

- **La API HTTP.** Todo lo que aporta lo cubre otro transporte, y añade puerto, autenticación,
  descubrimiento y una superficie de red que la tabla de seguridad de
  [03-arquitectura.md](03-arquitectura.md) hoy no tiene. Es la única de las cinco que convierte «quien
  escriba en tu directorio» en «cualquiera que alcance tu máquina».
- **El socket local como transporte principal.** Es lo que hace Herdr, y su ventaja real —saber qué
  proceso escribe— no compensa perder al productor que no puede hablarlo: un contenedor, un CI en otra
  máquina, un `scp`. Ahí está la diferencia entre «fuera de banda» y «fuera del proceso»
  ([07-panorama.md](07-panorama.md) → E8).
- **Sustituir el fichero en vez de acompañarlo.** Cualquier transporte que exija un cliente sube el
  listón de entrada, y el listón de entrada es lo que hace que el formato pueda tener productores —
  que es la única cosa que puede convertirse en foso ([01-vision.md](01-vision.md)).
- **Dejar la fuente de verdad en el fichero y que MCP escriba ahí.** Mantiene una sola fuente y se
  sigue leyendo con `cat`, pero le da al agente permiso de escritura sobre el fichero de su división
  por un camino que no pasa por el acotado de `PANEL_ID`, y hace que el estado de la app dependa de
  que el sistema de ficheros avise a tiempo. La inspeccionabilidad se conserva de otra forma: el
  fichero sigue siendo el transporte de serie y sigue estando ahí.

**Lo que esto NO cambia, y conviene decirlo para que nadie lo reabra por error:**

- **D6 sigue intacta.** Esto es sobre **por dónde llega** el estado, no sobre quién lo decide. Lo
  sigue declarando quien trabaja, en su vocabulario.
- **El documento no cambia de forma.** Un `panel.json` escrito hoy vale igual mañana.
- **El vigilante no era el problema.** Se comprobó antes de escribir esto: la app usa **un solo
  vigilante** para todos los directorios, no uno por división, y los límites del sistema en la máquina
  de desarrollo son 1.024 instancias y 524.288 rutas. Vigilar ficheros no escala mal aquí, y ese
  argumento no debe usarse contra el fichero.

**Consecuencias, y una es trabajo pendiente:** [04-contrato-panel.md](04-contrato-panel.md) se parte
en dos —el documento y los transportes—, la tabla de canales de **D5** deja de nombrar `panel.json`
como si fuera el canal, y **queda por decidir cómo se expresan la fuente y el orden en el propio
documento**: Herdr lo resuelve con `--source` y un `--seq` estrictamente creciente, y aquí no hay
todavía ni uno ni otro. No se decide en esta entrada porque con un solo transporte vivo sería
inventar la forma antes de tener el segundo caso delante.

---

## D18 — El productor declara si su división reclama, y la app no lo deduce del cuerpo

**Qué se decidió.** El documento de panel gana un campo opcional, `attention`, con tres valores:
`wait`, `block`, `done`. Ausente significa que esa división no reclama nada. Lo escribe el productor.

| Valor | Qué dice | Ejemplo |
|---|---|---|
| `wait` | **te espera a ti**: una pregunta, un permiso, una decisión | «¿apruebo el diseño?» |
| `block` | atascada por algo que no eres tú | «esperando que el contrato de otro servicio exista» |
| `done` | terminó y nadie lo ha mirado | el ciclo acabó y el diff sigue sin revisar |

**Y la app lo acumula hacia arriba:** división → pestaña, con `wait` ganando a `block` y `block` a
`done`. `wait` va primero porque **es lo único que se desatasca contigo delante ahora mismo**; una
división bloqueada por un tercero puede seguir esperando cinco minutos más.

**Por qué hace falta un campo y no vale el cuerpo que ya existe.** El contrato sabe expresar
«esperando» y «bloqueado» desde el principio — `mark: wait`, `mark: block` — pero eso son **líneas del
cuerpo**, y el cuerpo es libre por diseño. Para pintar el indicador de una pestaña o disparar un aviso
a partir de ahí, la app tendría que **leer el cuerpo y decidir qué significa**: que una línea con
`block` en el medio de un resumen quiere decir que la división entera reclama. Eso es la app
decidiendo qué significa el trabajo de otro, o sea **D6 rota por dentro**, que es la peor forma de
romperla: sin que ningún documento diga que se rompió.

**Por qué esto no contradice la regla del cuerpo libre.** El campo no nombra una métrica ni un
vocabulario de dominio: nombra **la única pregunta que la app necesita responder para navegar** —
¿tengo que ir ahí?—. Sigue sin haber campos de coste, de contexto ni de progreso, y `phase` sigue
siendo texto libre que se pinta literal. La app entiende de navegación; la semántica sigue siendo
del productor.

**Y sin el campo la app no inventa nada.** Un productor que no publique `attention` tiene el mismo
panel que hoy y su pestaña no se marca. No hay valor por defecto deducido del cuerpo, ni de la
`phase`, ni de la actividad de la consola. **Callar es callar**, no «esto no reclama porque no lo
parece».

**Por qué ahora y no en la Fase 4.** Porque desbloquea tres cosas que están escritas como pendientes
en tres documentos distintos y todas esperaban a esta decisión: el aviso al pasar a «esperando»
([04-contrato-panel.md](04-contrato-panel.md) → Pendiente), el indicador de una pestaña que no miras
—hoy lo que corre en una pestaña de fondo no se anuncia de ninguna forma—, y la vista resumen de la
Fase 4, que ordena por esto mismo. Y porque la afirmación de que **nadie más ordena por lo que te
reclama dejó de ser cierta en agosto de 2026**: Herdr acumula agente → panel → pestaña → workspace y
su barra lateral contesta esa pregunta mejor que la nuestra ([07-panorama.md](07-panorama.md) → E8).

**Lo que se descartó, y por qué:**

- **Un enum de estado de trabajo tipo `idle` / `working` / `blocked`** — lo que tienen Herdr, Xirp,
  Superset y Warp. Describe **qué hace el programa**, y eso o lo deduce la app (D6) o lo declara un
  harness que no conoce la pregunta (**D6**, «el nivel 2 reinterpretado falla igual que adivinar»).
  `attention` no pregunta qué hace: pregunta si te reclama, que es lo que la app necesita para
  navegar y lo único que el productor puede contestar sin equivocarse.
- **Deducirlo de la presencia de un `mark: wait` en el cuerpo.** Es lo barato y es lo que rompe D6 por
  dentro. Además sería frágil de la forma más tonta: una línea histórica —«esperé aprobación el
  martes», con `mark: wait`— marcaría la pestaña para siempre.
- **Un campo de urgencia numérica.** Tres valores contestan la pregunta; un número obliga a cada
  productor a calibrar contra los demás, y nadie puede hacer eso porque nadie ve los otros paneles.
- **Que el aviso del sistema lo dispare la app por su cuenta al aparecer un `wait`.** Se deja para
  cuando el campo tenga productores: notificar es interrumpir, y una interrupción falsa cuesta mucho
  más que un indicador falso. El indicador entra ahora; el aviso, cuando se sepa que el campo se
  publica bien.

### Lo que F2.8 añadió: quién sostiene un reclamo, y la excepción de lo terminado

**Un reclamo se ve atenuado cuando la aplicación tiene algo que confesar sobre ese panel** — lleva
callado más de lo suyo, o su fichero dejó de leerse. Dice las dos cosas a la vez: *reclama*, y *no hay
nadie detrás*. La razón de atenuarlo en vez de esconderlo, con lo descartado, está en **D9** → «lo que
F2.8 añadió».

**Excepción, y es de producto: `done` no se atenúa por silencio.** Quien terminó **deja de publicar por
definición**, así que su silencio no desmiente nada: es su estado normal. Sin la excepción, media hora
después de acabar —el plazo de serie— *todo* lo terminado se pintaría apagado, y **encontrar por la
mañana lo que acabó de noche es justo para lo que sirve el campo**. Lo que sí lo atenúa es **no poder
leer su fichero**: ahí lo que se enseña es lo último que llegó, y nada dice que siga siendo verdad.

*Descartado:* tratar las tres clases igual, que es más simple de explicar y apaga el caso central del
producto. *Descartado también:* que `done` no se atenúe **nunca** — eso sería esconder un panel roto
detrás de un resumen plausible, que es lo que este panel existe para no hacer.

**Lo encontró la revisión, no el diseño.** El diseño escribió «el rancio se atenúa» para las tres sin
mirar qué significa el silencio en cada una.

### Medido — 24 agosto 2026: lo que cuesta la barra desde que su ficha acumula

**La ficha de cada pestaña dice lo que más reclaman sus divisiones, así que pintar la barra ahora
recorre.** Ese trabajo **se declara** —tiene su línea propia en la cabecera del informe— y por tanto
**no** se publica como coste del framework, que es la cifra con la que **D3** juzga a egui.

Cuatro corridas, dos de la tanda `F` (la pestaña de fondo quieta) y dos de la `G` (la de fondo volcando
como la de delante), con ventana de 1280×800 y **las cuatro consolas publicando un reclamo** —la mitad
`wait` y la mitad `done`—, con las **dos fichas marcadas** en las cuatro corridas.

| Corrida | Fotogramas con barra | La barra, por fotograma | El resto del fotograma |
|---|---|---|---|
| `F` #1 | 512 | **2,346 µs** | 256,2 µs |
| `F` #2 | 479 | **2,689 µs** | 268,1 µs |
| `G` #1 | 553 | **2,689 µs** | 290,0 µs |
| `G` #2 | 552 | **2,398 µs** | 293,9 µs |

**Entre 2,35 y 2,69 µs**, o sea **una centésima y media del 1 %** del presupuesto de 16,7 ms a 60 Hz —
con dos pestañas y cuatro consolas reclamando. Las dos corridas de cada tanda **se pueden promediar**
según el comparador del guion.

**Qué entra en ese número y qué no.** Entra el recorrido que acumula los reclamos de cada pestaña y el
pintado de sus fichas: el cronómetro vive **dentro** de la función que las pinta, no en quien la llama,
porque desde fuera el tramo abarca la llamada entera se haga lo que se haga dentro — y entonces sacar
trabajo de la región medida no rompe ninguna prueba. **No entra atender el clic de la barra**, que
corre en otro punto de la pasada y **sigue sin declararse desde antes de este hito**: es una fuga
conocida, pequeña y anterior, y arreglarla aquí habría arriesgado contar dos veces lo que el cambio de
pestaña ya declara por su cuenta.

**Y el número describe el camino sano**: productores vivos, publicando dentro de plazo. Con paneles
rancios o rotos, la pregunta que la ficha hace compone además el texto del aviso de cada división, y eso
no está medido — sigue dentro del tramo declarado, así que no se le regala a nadie, pero no es este
número.

**Por qué se mide con dos pestañas y no con una.** Con una sola pestaña **no hay barra**, así que una
tanda de una pestaña afirmaría «el coste está declarado» **con el código quitado**. El guion lo impide
por su cuenta desde este hito: en una tanda de dos pestañas o más, la corrida se marca `.INVALIDA` si el
informe no trae la línea de la barra **o si la barra no llegó a marcar ni una ficha** — porque el
recorrido ocurre haya reclamo o no, y sin esa segunda guarda una corrida cuyos productores no
publicaran el campo mediría la mitad del trabajo y se leería igual que una buena. Es la misma guarda que
ya evita publicar como buenas las tandas sin panel, sin gestos o sin restaurar.

**Lo que este número NO dice.** No se compara con las corridas `F`/`G` del anexo de **D13**: son de otro
día y de otro binario, y ahí el resto del fotograma salió entre 386 y 466 µs. Lo que se sostiene es lo de
dentro de esta tabla — que el tramo existe, que está separado, y que es **dos órdenes de magnitud menor**
que el residuo del que se le habría restado.

---

## D19 — Nos posicionamos contra emuladores de terminal, no contra orquestadores de agentes

**Qué se decidió.** El usuario al que hay que ganarle es **el que hoy tiene WezTerm, Ghostty, Kitty,
iTerm2 o Windows Terminal abierto con agentes dentro** — no el que usa Orca, Superset o Conductor. La
evolución hacia funciones de entorno de desarrollo queda **después**, y condicionada a que esto tenga
usuarios.

**Por qué, y son tres razones independientes:**

- **Contra terminales el panel no tiene competencia.** Ningún emulador tiene estado por división: la
  comparación es «lo que ya tienes, **más** una cosa que no tiene ninguno». Contra orquestadores la
  comparación es una lista de quince funciones donde siempre vas detrás, y **un producto con el 70 %
  de Orca y nada propio es peor que Orca**.
- **La carrera de superficie no se gana.** Orca lleva desde marzo de 2026 con equipo y empujes
  diarios; su navegador Chromium por worktree, su app móvil y su integración con Linear son proyectos
  enteros cada uno ([07-panorama.md](07-panorama.md) → E7).
- **El reloj corre al revés.** Cada mes gastado en paridad es un mes en que a Herdr le puede aparecer
  el campo que le falta para llevarse la única pata que queda (E8). Y hay precedente del año en
  curso: **Herdr salió con mucho menos que tmux en superficie y llegó a 31.888 estrellas en cinco
  meses**, por hacer una cosa que tmux no hacía. La adopción no vino de la paridad.

**El listón que sí hay que pasar**, y sale de esta liga y no de la otra:

| Qué da por hecho un usuario de terminal | Estado |
|---|---|
| Scrollback, selección, copiar, buscar | **F2.6** |
| Pegar | hecho — **D11** |
| Corre en su máquina: macOS y Windows | **F2.7** |
| **Que se vean sus fuentes** — CJK y los iconos de un `ls` moderno | **no tenía hito hasta hoy → F2.9** |
| **Poder configurarlo** — fuente, tamaño, colores, atajos | **no tenía hito hasta hoy → F2.10** |
| Que el ratón llegue al programa de dentro (`vim`, `htop`) | dentro de **F2.6**, ahora explícito |
| Enlaces clicables (OSC 8) | pendiente, no descalificatorio |
| Ligaduras, protocolo gráfico de Kitty | **fuera**, y se declara |
| Latencia y coste por fotograma | medidos y publicados — **D3**, D13, D14, D15 |

**Las tres que no tenían sitio en ninguna fase, y la razón de que aparezcan al cambiar de liga:**

1. **Las fuentes del sistema.** En un entorno de desarrollo es una pega; **en un terminal
   descalifica**. El usuario abre, hace `ls`, ve cuadros vacíos y cierra. Ya estaba escrito en D3 con
   estas palabras —*«es lo que separa esto de ser usable a diario»*— y llevaba desde agosto sin hito.
2. **La configuración.** Cero menciones en el roadmap antes de hoy. Es **la primera cosa que hace
   cualquiera** al instalar un terminal, y sin ella no hay adopción que valga.
3. **El ratón dentro de la TUI.** Los informes SGR/X10 no están implementados, así que hoy no se puede
   hacer clic dentro de un programa de texto. Estaba fichado como materia de F2.6, pero como nota
   dentro de otro hito y no como algo que alguien fuera a comprobar.

**Lo que queda fuera del listón, y con quién lo hace** — está aquí para saberlo, no para hacerlo:

| | Quién lo tiene |
|---|---|
| **Abanico**: el mismo encargo a N agentes en N worktrees, comparar y quedarse con el ganador | Orca |
| Navegador integrado; señalar un elemento de la interfaz y mandárselo al agente | Orca (*Design Mode*), Superset |
| Editor de ficheros dentro; diff con anotaciones; atribución de lo que escribió el agente | Orca, Superset |
| Commit, push y abrir la MR/PR desde la app; cola de revisión | Orca, Conductor, Vibe Kanban |
| App móvil de acompañamiento | Orca |
| Worktrees remotos por SSH o en un VPS | Orca, Superset, Herdr |
| Restaurar sesiones de agente tras reiniciar; hibernar las inactivas | Orca, Herdr |
| Integración con Linear y Jira; registro de habilidades instalables | Orca, Superset |
| Tablero de tareas y cadenas de dependencias | Cline Kanban, Vibe Kanban — ver «Ideas sin fase» en [05-roadmap.md](05-roadmap.md) |

**El criterio para dejar entrar algo de esa lista antes de tiempo: que sirva al contrato, no que lo
tenga el vecino.** A favor, los worktrees remotos: un productor publicando desde otra máquina es
exactamente el caso que el fichero gana y el socket de Herdr pierde (**D17**). En contra, el *Design
Mode*: es magnífico y no le sirve de nada a lo que aquí diferencia.

**Lo que esta decisión NO cambia, y conviene no reabrirlo:** **seguimos sin competir con tmux ni con
Zellij** ([01-vision.md](01-vision.md)). Posicionarse contra emuladores no arrastra lo más caro de la
liga de al lado —que la sesión sobreviva a cerrar la ventana, adjuntar y desacoplar, un servidor de
fondo—, y eso es precisamente lo que hace que este listón sea corto.

**Cuándo se revisa esta decisión:** cuando haya usuarios que no seas tú, o cuando alguno de la lista
de arriba publique un contrato abierto con cuerpo. Lo primero cambia a quién hay que convencer; lo
segundo cambia si hay algo que defender.

---

## D20 — El botón izquierdo dentro de una consola selecciona; mover la división pasa a `Ctrl+Mayúsculas`

**Decisión.** Arrastrar con el botón primario **dentro del cuerpo de una consola selecciona texto**.
Para **mover la división de sitio** —el gesto de **D15**— hay que arrastrar con **`Ctrl+Mayúsculas`**
apretado. Y cuando el programa de dentro pide el ratón, **`Mayúsculas` sola fuerza la selección del
terminal**, que es la convención de `xterm` y de todos los que vinieron después.

**Por qué se toca un gesto ya entregado y medido.** Hasta aquí el primario reclamaba el rectángulo
entero de cada consola y **movía la división**. Eso convertía «seleccionar texto» —lo que se hace
todos los días en un terminal— en algo que había que aprender, y dejaba el gesto libre para lo que se
hace de vez en cuando. El listón de **D19** es un usuario de WezTerm, y ahí arrastrar selecciona: un
terminal donde arrastrar mueve paneles no pasa ese listón, por mucho que mover paneles sea lo que más
nos diferencia.

**Por qué `Ctrl+Mayúsculas` y no otra tecla.**

- Es **el prefijo que la aplicación ya usa para todo lo suyo**: partir, cerrar, cambiar de pestaña,
  pegar. Mover una división es de esa familia; darle otro prefijo sería inventar un segundo idioma.
- Deja **`Mayúsculas` sola libre**, y hace falta: es lo que fuerza la selección cuando el programa
  reclama el ratón. Gastarla en mover haría imposible seleccionar dentro de `vim`.
- Deja **`Ctrl` y `Alt` libres**, que es donde los demás terminales ponen la **selección en bloque**.
- **`Super` queda descartada** aparte: en GNOME y en KDE arrastra la ventana entera.

**Lo que se descartó, y por qué.**

- **Dejar el primario moviendo y cualificar la selección.** Es lo que había, y es lo contrario de lo
  que pide el listón: el gesto diario detrás de una tecla y el raro en el gesto libre.
- **Una cabecera por división, al estilo de Tilix**, que fuera el asidero para arrastrar y llevara el
  botón de cerrar. **Buena idea, y por eso no se descarta del todo: pasa a ser su propio hito** (ver el
  roadmap). Aquí se descarta **como parte de F2.6** por su precio: una cabecera **le quita alto a todas
  las consolas, siempre**, y este proyecto ya tomó esa decisión en sentido contrario cuando dejó de
  pintar la barra de pestañas con una sola pestaña — precisamente para que la rejilla fuera idéntica a
  la de antes y las medidas se pudieran comparar. Con cabecera, **las tandas de control publicadas
  dejan de comparar con nada**. Y no resuelve el problema entero: `Mayúsculas` seguiría haciendo falta
  para forzar la selección dentro de un programa que pide el ratón.
- **Un asidero invisible en el borde superior**, sin fila propia. No cuesta alto, pero hay que
  descubrirlo y no da sitio a un botón de cerrar. Queda como opción para el hito de la cabecera.

**Y los dos caminos conviven.** Elegir el modificador ahora no cierra la puerta a la cabecera después:
Tilix tiene las dos cosas. Lo que este hito no hace es meter una decisión de disposición dentro de un
hito de historial y selección.

**Lo que arrastra, y está en el diseño del hito:**

- **El significado del arrastre se fija al empezar**, leyéndolo del propio evento de pulsación. El
  estado de modificadores de un fotograma describe **el final del lote**, no el instante del clic.
- **La tanda `J` del guion de medida deja de medir lo que dice.** Inyecta arrastres primarios pelados,
  así que con el primario seleccionando mediría selección y declararía cero reordenaciones — y el guion
  la invalidaría sola. Se le añade el acorde al inyector.

---

## D21 — El historial son 3 500 líneas por consola: lo elige el listón, y las medidas descartan el resto

**Decisión.** Cada consola guarda **3 500 líneas** de historial. El número lo pone este proyecto y **no
se hereda** el del motor —10 000—. El **3 500 sale del listón**: es el defecto de WezTerm, que es contra
quien **D19** dice que nos medimos. Lo que las medidas hacen es **descartar los demás** y poner precio
al elegido.

**Lo medido**, con `cargo run --release --example scrollback -p term-core`, rejilla de 49×180, volcado de
8 MiB, **cinco muestras por punto** y publicando la mediana:

| Techo | Techo de memoria por consola | Recolocar: líneas cortas | Recolocar: **líneas que envuelven** | Leer 8 MiB |
|---|---|---|---|---|
| 0 (lo de hoy) | 0 | 7,7 µs | 6,4 µs | **44,1 ms** · 182 MiB/s |
| 1 000 | hasta 4,1 MiB | 72 µs | 148 µs | 46,4 ms · 173 MiB/s **(+5 %)** |
| **3 500** | hasta **14,4 MiB** | 280 µs | **984 µs** | **81,6 ms** · 98 MiB/s **(+85 %)** |
| 10 000 (el del motor) | hasta 41,2 MiB | 1,70 ms | **4,07 ms** | 89,9 ms · 89 MiB/s (+104 %) |

Una celda ocupa **24 B** — preguntado, no derivado sumando campos.

**Dos avisos sobre esa tabla, porque una revisión los tuvo que sacar:**

1. **La columna de memoria es una cuenta, no una medida, y es un TECHO.** El motor **no reserva el
   historial de antemano**: su almacenamiento crece al llenarse. Esas cifras son lo que una consola
   puede llegar a ocupar con el historial lleno, no lo que ocupa al abrirse.
2. **Las dos columnas de recolocar son dos regímenes reales, y la que decide es la de la derecha.** Una
   línea **más corta que la rejilla no se recoloca**: al estrechar, el motor la empuja entera y sale por
   la puerta barata. La primera versión de este banco medía **solo** líneas cortas y publicó un número
   **cuatro veces menor** que el real. Una sesión de terminal tiene de los dos, así que el techo se
   elige contra el caro.

**Por qué se descarta el del motor.** Con 10 000 y contenido que envuelve, recolocar cuesta **4,07 ms
por consola**, y una frontera del árbol mueve **dos**: son **8,1 ms por fotograma** mientras se
arrastra, o sea **la mitad del presupuesto a 60 Hz**, en un gesto que dura lo que dure el botón
apretado. Y hasta 165 MiB con cuatro consolas, sin que nada acote cuántas hay (**D14**).

**El precio de 3 500, dicho entero y aceptado.**

- **Arrastrar una frontera cuesta ~2 ms por fotograma** con dos consolas de contenido que envuelve — un
  **12 %** del presupuesto. Solo se paga **mientras se arrastra**, y solo en los fotogramas que cruzan
  un borde de celda: el reparto sale temprano cuando la rejilla no cambia.
- **Leer un volcado se encarece un 85 %** en el banco: de 182 a 98 MiB/s. **Dentro de la ventana eso se
  traduce en un 39 % más de interpretar y un 1,2 % de vuelta completa** — el banco lee a lo ancho de una
  rejilla de 180 columnas y la ventana traga 128 KiB por fotograma, así que no son el mismo régimen. Ver
  abajo el anexo con las cuatro corridas.
- **Hasta 14,4 MiB por consola** de historial lleno, 58 con cuatro.

Es el precio de que un terminal recuerde lo que salió, y se declara en vez de esconderse.

**Y obliga a volver a tomar las medidas publicadas.** Todas las tablas de los anexos —D3, D13, D14,
D15, D16, D18— se tomaron con el historial **apagado**. En cuanto se encienda, su tramo «interpretando»
describe otro régimen. **Volver a tomarlas es parte del hito**: un anexo que mide un régimen bajo la
etiqueta de otro es exactamente el número mal etiquetado que este proyecto no publica.

**Lo que se descartó, y por qué.**

- **Heredar las 10 000 del motor.** Es lo que habría pasado por no tocar una línea, y cuesta la mitad
  del fotograma en un gesto normal. El defecto de una dependencia no es una decisión de este proyecto.
- **1 000 líneas.** Casi gratis —+5 % al leer, 1,8 % del fotograma al arrastrar— pero son veinte
  pantallas: sirven para mirar atrás, no para buscar algo de hace un rato. No pasa el listón.
- **Que lo elija quien usa la aplicación.** Es lo correcto **a medio plazo** y está en el roadmap
  (**F2.10**, la configuración), pero hoy no hay dónde ponerlo: adelantar media configuración dentro de
  este hito es meter otro hito dentro.

**Cuándo dejaría de valer.** Si cambia la versión del motor —los números son suyos—, si llega la
configuración de F2.10, o si alguien mide que recolocar dejó de crecer con el historial. El banco está
en el repositorio para volver a tomarlos en un minuto.

### Medido — 25 agosto 2026: lo que el historial le hace a la ventana de verdad

**El banco medía el motor; esto mide la aplicación.** Cuatro corridas de la tanda `C` —dos consolas
volcando, con panel—, **las cuatro con el mismo binario y seguidas**: dos compilando con el techo a cero
y dos con las 3 500 líneas puestas. Las dos de cada par **se pueden promediar** según el comparador del
guion, y la tabla publica la media con las dos corridas detrás.

| | sin historial | con 3 500 líneas | |
|---|---|---|---|
| **byte disponible → fotograma pagado** | **74,6 ms** (74,19 · 74,94) | **75,5 ms** (75,62 · 75,40) | **+1,2 %** |
| de eso, esperando en la cola | 71,7 ms (71,65 · 71,80) | 72,0 ms (71,99 · 71,95) | +0,4 % |
| **interpretando** | **1,05 ms** (0,95 · 1,16) | **1,46 ms** (1,51 · 1,41) | **+39 %** |
| encolando el pintado | 249 µs (229 · 269) | 234 µs (237 · 230) | dentro del ruido |
| **el resto del fotograma** (el framework) | 279 µs (257 · 300) | 262 µs (262 · 263) | dentro del ruido |

**Lo que hay que leer aquí: el historial encarece interpretar un 39 %, y la vuelta completa un 1,2 %.**
Cada fotograma traga un presupuesto fijo de 128 KiB por división; interpretarlo cuesta ahora la mitad más,
y eso se paga **dentro** del fotograma, no en la cola. El framework no se mueve, que es lo que **D3**
vigila, y el tramo `recolocar` va declarado aparte desde este hito para que no se le cuele.

**Una versión anterior de este anexo publicó «la vuelta se dobla, 37,1 → 75,4 ms» y era falso. Se retira,
y conviene que quede escrito por qué**, porque el fallo es exactamente el que la regla dura del proyecto
describe:

1. **La columna «sin historial» no era del mismo binario ni del mismo día.** Salía de una corrida del **18
   de agosto**, aunque el texto afirmaba «el mismo binario del día y a media hora de distancia». Nadie
   podía verlo leyendo el anexo.
2. **Y la conclusión no la sostenían los datos.** Hay corridas de la tanda `C` del **19 de agosto**, con el
   historial **apagado**, que ya daban **73,7 y 74,0 ms** de vuelta y 71,5 ms de cola. O sea que el salto
   de 37 a 74 ms ya había ocurrido **antes** de encender el historial: atribuírselo era colgarle un coste
   ajeno.

**Y eso deja una pregunta abierta que no es de este hito:** entre el 18 y el 19 de agosto la vuelta
completa de la misma tanda, con la misma ventana y la misma rejilla, pasó de **37 a 74 ms** con el
historial apagado — **y no es coste de CPU**: interpretar se movió de 763 a 851 µs y el residuo del
framework de 220 a 240. Los fotogramas son parecidos en número y en coste, así que lo que se duplicó es
**lo que cada byte espera**, no lo que cuesta atenderlo. Apunta a cuándo se pide repintado, no a cuánto
cuesta pintar. Queda como issue propio.

**Cuándo se nota el 39 %, y cuándo no.** Solo con un volcado sostenido —un `cat` de un fichero grande, que
es lo que esta tanda hace a propósito—, y ahí se traduce en menos de un 2 % de retraso. Un comando normal
no llena la cola y no espera nada. **El precio caro del historial no está aquí**: está en recolocar el
texto al cambiar de ancho, que es lo que mide el anexo siguiente.

### Medido — 24 agosto 2026: el peor caso de recolocar, con la frontera moviéndose sin parar

**Por qué hace falta una tanda nueva y no valían las que había.** Recolocar el texto se paga **por
consola**, y ninguna de las dos tandas con gestos le cambia el ancho a más de una: la `K` estrecha el
panel de la consola izquierda, y la `J` reordena el árbol al soltar sin cruzar bordes de celda mientras
vuela. La frontera que reparte dos consolas es la única que se las cambia **a las dos a la vez**, así que
es la única que ejercita el techo de 3 500 líneas por su lado caro. Es la tanda **`M`**.

| | tanda `C` (control, quieta) | tanda `M`, corrida 1 | tanda `M`, corrida 2 |
|---|---|---|---|
| fotogramas de la corrida | 429 · 470 | 799 | **3 488** |
| fotogramas con salida | 358 | 357 | 357 |
| gestos inyectados · efectos | — | 159 · 480 | 697 · 2 093 |
| **recolocaciones** | 3 · 3 (las del arranque) | **320** | **1 397** |
| lo que costó cada una | 671 · 524 µs | **654 µs** | 197 µs |
| **el resto del fotograma** (el framework) | 262 · 263 µs | **257 µs** | 163 µs |

**Lo primero, que es lo que la tanda venía a comprobar: el gesto no se le cuela al framework.** Con la
frontera moviéndose sin parar y recolocando dos consolas trescientas veintitantas veces, el residuo se
queda en **257 µs**, dentro de los 262–263 del control quieto. El tramo `recolocar` está declarado y se
resta de su fotograma, así que los 209 ms que costó recolocar en esa corrida **no se publican como coste
de egui** — que es exactamente lo que la regla dura del proyecto persigue.

**Lo segundo: recolocar cuesta lo que el banco dijo, y las dos corridas no dicen lo mismo.** Los 654 µs
por vez de la primera están en el mismo orden que los 984 µs del banco y los 524–671 del control quieto.
La segunda da **197 µs**, tres veces menos, y **por qué difieren no está aislado**. Se publican las dos y
se dice lo que se sabe:

- **No se promedian.** Las dos tienen los mismos 357 fotogramas con salida pero una vivió 3 488 fotogramas
  y la otra 799 —inyectar gestos mantiene la ventana despierta, igual que un ratón humano—. Es el mismo
  aviso que el anexo de **D15** lleva para la tanda `J`, y aquí muerde más porque la diferencia es de
  cuatro veces.
- **La explicación que los datos DESCARTAN** es que la corrida barata midiera el historial a medio llenar:
  recolocar crece con las líneas guardadas, y en la corrida larga el volcado terminó pronto, así que la
  mayoría de sus recolocaciones ocurrieron con **más** historial detrás, no con menos. Predice lo
  contrario de lo que salió.
- **La que queda en pie** es con quién compite cada recolocación: la corta pasó el 45 % de sus fotogramas
  volcando y la larga el 10 %. Es consistente con los números **y no está medida** — aislarla pide que el
  informe publique cuánto historial había detrás de cada recolocación, que es instrumentación que este
  hito no añade. Queda dicho como hipótesis, no como resultado.

**Lo que esta tanda NO mide, y conviene saberlo antes de fiarse: redimensionar la ventana.** Cambiar el
tamaño de la ventana le cambia el ancho a **todas** las consolas de **todas** las pestañas a la vez —las
de fondo se remiden en cada fotograma contra el área nueva—, así que un solo fotograma de ese arrastre
recoloca ocho consolas donde el gesto de la frontera recoloca dos. Con el historial lleno son del orden de
**8 ms por fotograma**, más que el fotograma entero. **Ninguna tanda lo cubre**, y no por olvido: todas
piden un tamaño fijo y el guion invalida la corrida que no lo tuvo, que es justo lo que hace comparables
las medidas del proyecto. El trabajo **sí está declarado** —el tramo suma todas las consolas del fotograma
y se resta de él—, así que no es un número mal etiquetado: es un **camino sin número**. Medirlo pide una
tanda que redimensione, y esa tanda tiene que decidir antes qué hacer con la guarda del tamaño.

**Lo tercero, y es el número que se lleva alguien que arrastre una frontera con una consola volcando:**
dos recolocaciones por gesto, de medio milisegundo largo cada una, dentro de un fotograma que ya paga 76
ms de espera en la cola. El gesto se ve fluido porque lo que cuesta no está en el pintado; lo que se
retrasa es **lo que la consola tenía pendiente de enseñar**.

**Lo que la tanda destapó de camino, y no era de rendimiento.** El gesto inyectado apretaba sobre la línea
del divisor **en el mismo fotograma** en que llevaba el puntero hasta ella, y así perdía el empate contra
la consola de al lado: egui admite como candidatos a los tres —la línea mide dos puntos y las dos consolas
la tocan— y el empate lo gana el último registrado. El resultado era que la tanda **arrastraba una
consola** y publicaba el gesto de **D15** bajo esta etiqueta. Se arregla acercando el puntero en un
fotograma y apretando en el siguiente, que es lo que hace una mano: comprobado que un arrastre humano gana
siempre. **A quien usa la aplicación no le pasa**, y eso está medido y no razonado.

### Medido — 24 agosto 2026: las cuatro tandas de los anexos, vueltas a tomar con historial

**Por qué se vuelven a tomar.** Guardar historial encarece **interpretar**, y las tablas publicadas en los
anexos de **D13** (pestañas) y **D14** (cuatro divisiones) se midieron con el historial apagado: dejarlas
sin más las convertiría en cifras que ya no describen a esta aplicación. Ocho corridas, dos por tanda, el
mismo día y con el mismo binario, y **el comparador del guion dice que las dos de cada par se pueden
promediar**.

| | **F** — 2 pestañas, la de fondo quieta | **G** — 2 pestañas, las cuatro volcando | **H** — 4 en fila | **I** — 4 en cuadro |
|---|---|---|---|---|
| Rejilla de cada consola | 47×39 | 47×39 | **49×20** | **24×39** |
| Consolas volcando | 2 de 4 | **4** | **4** | **4** |
| Vuelta completa | 75,3 ms · 74,8 | 77,7 · 78,8 | 77,1 · 80,0 | 79,2 · 80,0 |
| — esperando en la cola | 71,9 · 71,4 | 72,0 · 72,5 | 71,7 · 73,5 | 73,1 · 73,5 |
| — interpretando | 1,41 ms · 1,42 | 1,31 · 1,44 | 1,21 · 1,45 | 1,51 · 1,48 |
| — encolando el pintado | 203 µs · 215 | 203 · 246 | 146 · 171 | 128 · 135 |
| — el panel de estado | 5,4 µs · 5,1 | 5,1 · 6,6 | 10,9 · 12,4 | 5,3 · 5,5 |
| — la barra de pestañas | 2,78 µs · 2,08 | 2,41 · 2,90 | (no hay) | (no hay) |
| **el resto del fotograma** | **236 µs · 243** | **263 · 298** | **319 · 355** | **274 · 291** |

**Estas cifras NO se comparan con las de los anexos de D13 y D14, y decirlo importa más que la tabla.**
Aquéllas son del 16 y el 17 de agosto, con otro binario y otro día de máquina: la misma tanda `C` daba
allí 52 ms de vuelta completa, el 18 de agosto daba 37 y el 19 —todavía **sin historial**— ya daba 74.
Lo que estas ocho corridas sirven es (a) dejar publicado lo que la aplicación hace **hoy**, y (b)
responder entre ellas, que es la comparación válida porque son del mismo día y el mismo binario.

**Lo que responden entre ellas:**

1. **El residuo del framework sigue sin crecer con nada.** Entre **236 y 355 µs** en las ocho corridas, con
   la variación entre las dos de una misma tanda (319 → 355 en `H`) del mismo orden que la variación entre
   tandas. Es la misma conclusión de los anexos de **D13** y **D14**, ahora con el historial encendido y
   con el tramo `recolocar` declarado aparte para que no se cuele aquí.
2. **Cuántas consolas vuelcan casi no mueve la vuelta completa.** De dos volcando (`F`, 75,0 ms de
   media) a cuatro (`H`, 78,6) hay **menos de un 5 %**, y la espera en la cola se queda clavada entre 71,4
   y 73,5 ms en las **ocho** corridas. En el anexo de D14, en agosto, esa misma diferencia era del
   **doble** (52 → 100 ms) — pero eso es de otro binario y no se cruza con esto: lo que aquí se afirma es
   lo de hoy, y hoy no se mueve.
3. **Interpretar se mueve poco y encolar el pintado baja con las celdas**, igual que antes: 146 µs con las
   980 celdas de `H`, 128 con las 936 de `I`, 203 con las 1 833 de `F`… al revés de lo que la cuenta de
   celdas haría esperar entre las dos primeras y la tercera, y por eso se publica sin conclusión: son
   microsegundos dentro de una vuelta de 75 ms.
4. **Recolocar apenas aparece en estas cuatro**, y es lo correcto: ninguna arrastra nada, así que las
   entre dos y cinco recolocaciones de cada corrida son las del arranque, cuando el compositor entrega el
   tamaño. Lo que cuesta cada una —de 43 µs en cuadro a 976 µs en fila— depende de cuánto texto hay que
   recolocar, y el caso caro tiene su propia tanda: la **`M`** de aquí arriba.

## D22 — Qué es «el texto» que se copia, y las cuatro reglas que la selección hereda del motor

**Qué se decidió.** Copiar devuelve lo que el motor de terminal llama «el texto de la selección», y
**esas reglas se escriben aquí** aunque el código sea de la dependencia. Heredar el código es correcto;
heredar la decisión sin decirla, no — en este proyecto ninguna decisión vive solo en el código, y menos
si vive en el de otro.

**Las cuatro reglas de qué es «el texto»:**

1. **Una línea que envuelve se copia SIN el salto.** Una línea más larga que la rejilla la parte el
   terminal, no el programa. Copiarla con un salto dentro devuelve algo que **nadie escribió**, y pegarlo
   en un shell ejecuta dos órdenes en vez de una. Dos líneas de verdad **sí** conservan el suyo: lo que
   las distingue es quién puso el final.
2. **El relleno del final de cada línea no se copia.** Una rejilla está llena de celdas vacías a la
   derecha de lo escrito; arrastrarlas convierte cualquier pegado en una línea con cola de espacios.
3. **El acompañante de un carácter ancho no se duplica.** Un carácter CJK o un emoji ocupan dos celdas y
   son **uno**; copiar las dos lo devolvería repetido.
4. **El hueco de una tabulación se salta.**

**Las tres formas de perder una selección, y también son suyas:**

- **Cambiar el número de columnas la borra.** El texto se recoloca: lo que estaba marcado deja de estar
  donde estaba, y conservar el rango dejaría realzado un trozo que nadie eligió. **Con las mismas
  columnas no**: redimensionar solo el alto la conserva, porque no se recoloca nada.
- **Saltar a pantalla alterna la borra.** Un programa de pantalla completa —`vim`, `less`, `htop`— cambia
  a la alterna al arrancar, y lo que se marcó no está en la pantalla nueva.
- **Limpiar una región que la cruza la borra.** El programa borró lo que estaba marcado; sobrevivir sería
  realzar celdas vacías, o lo que él escriba ahí después.

> **Cuando `htop` te borre la selección, la razón está aquí.** Ese era el objeto de escribirlas: sin esta
> sección, la explicación vive en el fuente de una dependencia y quien la busque no sabe ni que existe.

**Una quinta regla que faltaba, y que el volcado a texto incumplía: lo que va pegado a un carácter también
es texto.** Un selector de emoji —`U+FE0F`— o una marca combinante **no ocupan columna**, pero el programa
los escribió. Copiar con el ratón siempre los ha conservado, porque eso lo hace el motor; **el volcado a
texto de esta caja los tiraba**, así que un programa que escribiera `☁️` se leía como `☁`. No es un
carácter recortado: es **otro** carácter. Corregido en agosto de 2026, al añadirle a la vista lo que va
pegado a cada celda (**F2.9**), que es cuando el defecto se hizo visible por primera vez.

**Y una regla que este proyecto añade, porque el motor no puede saberla.** El acompañante de un carácter
ancho **se realza con su cabeza**. El motor hace solo el camino contrario —marca la cabeza cuando el
acompañante cae dentro, para que su texto no salga cortado— y le basta porque **su** pintor dibuja el
fondo de un carácter ancho de dos celdas de ancho. El nuestro pinta el fondo de **cada celda por
separado** (**D3**), así que sin añadirla un carácter ancho en el borde de la selección sale con media
letra realzada y media no, justo donde hay CJK o emoji.

**Cómo se ve lo seleccionado: intercambiando sus dos colores.** No hay color de realce en el tema, y no
es por ahorrar: los dos colores de una celda **ya eran legibles el uno sobre el otro**, así que
intercambiarlos no puede dejar texto ilegible. Un color propio sí puede, y lo haría justo donde el
programa ha elegido colores raros — que es donde nadie lo probaría. El intercambio va **después** del
vídeo inverso, así que seleccionar por encima del cursor de `vim` lo enseña como el resto del texto en
vez de dejarlo indistinguible.

**Copiar no filtra caracteres de control, al revés que pegar.** Lo que sale ya es texto de celdas y el
motor no guarda escapes. Pegar filtra porque protege **al programa de dentro** de lo que entra (**D11**);
aquí la dirección es la contraria y no hay a quién proteger.

**Lo que se descartó, y por qué.**

- **Escribir nuestras propias reglas de «el texto».** Son diez reglas finas —envolturas, acompañantes,
  tabulaciones, relleno— que el motor ya sostiene y que hay que mantener al día con él. Reescribirlas es
  garantizar que un día discrepen del realce, que sí sale de sus coordenadas.
- **Sacar el rango de la selección por la API de `term-core`.** Es un tipo del motor y sería la fuga que
  el criterio fundacional de esa caja impide. Lo que quien pinta necesita es **un `bool` por celda**, y
  con eso las dos reglas finas del realce las sigue decidiendo el motor.
- **Filtrar los controles al copiar**, por simetría con el pegado. Ninguno sale: lo que hay son celdas.
  Sería copiar la guarda del pegado sin su razón.
- **Un color de realce en el tema.** Ver arriba: puede dejar texto ilegible, y el intercambio no.

**Cuándo dejaría de valer.** Si el motor cambia sus reglas de conversión a texto —los números y el
comportamiento son suyos—, o si algún día el pintor dibujara el fondo de un carácter ancho de una vez, en
cuyo caso la regla que este proyecto añade sobraría.

### Medido — 25 agosto 2026: lo que cuesta tener texto marcado

**Con su control dentro de la misma corrida.** La tanda **`N`** abre dos consolas volcando lo mismo y
marca texto **solo en la izquierda**: la derecha no se toca. Es la misma forma que la tanda `K` usa para
el gesto del panel.

| encolando el pintado | consola marcada | consola sin tocar | diferencia |
|---|---|---|---|
| tanda `N`, corrida 1 | 277,4 µs | 191,9 µs | **+85,5** |
| tanda `N`, corrida 2 | 297,5 µs | 187,3 µs | **+110,2** |
| tanda `C` (quieta), corrida 1 | 236,7 µs | 204,8 µs | **+31,9** |
| tanda `C` (quieta), corrida 2 | 230,4 µs | 189,6 µs | **+40,8** |

**Y aquí está lo que la primera versión de este anexo publicó mal.** Decía «marcar cuesta ~100 µs» tomando
la diferencia entre las dos consolas de la corrida. Pero **esa diferencia existe sin selección ninguna**:
la tanda quieta ya da 32 y 41 µs entre izquierda y derecha. La consola de al lado **no es un control
limpio** — hay una asimetría de sitio que no es de este gesto.

**Lo que queda, con las dos cuentas que se pueden hacer y su desacuerdo dicho:**

- **Diferencia de diferencias** (98 − 36): **~62 µs por fotograma**.
- **La misma posición entre corridas** (287 de media contra 234, izquierda con y sin marcar): **~53 µs**.

**Las dos no coinciden del todo, y con estos datos no se cierra la diferencia.** La corrida de la tanda
`N` vivió **1 066 y 2 640 fotogramas** contra los **429 y 470** de la quieta: inyectar gestos mantiene la
ventana despierta, así que las dos no comparten régimen — el mismo aviso que el anexo de **D15** lleva para
la tanda `J`. Señal de ello: el control de `N` sale **más barato** que el mismo puesto en la tanda quieta,
que es al revés de lo que cualquier explicación de «marcar cuesta» predice. Lo que se publica es la
horquilla, **50–60 µs por fotograma**, y que **no está aislada**. Aislarla pide una tanda que marque y
desmarque alternando dentro de la misma corrida, contra la misma consola.

**Lo que sí está claro, y es más importante que la cifra: el coste NO crece con lo que hay marcado.**
Mientras haya una selección viva, componer la foto pregunta **por cada celda de la rejilla** si va
realzada — las 1 911 de una consola de 49×39, marcadas dos o dos mil. O sea que:

- marcar una palabra cuesta lo mismo que marcar la pantalla entera;
- el coste **crece con la rejilla y con el número de consolas**, no con el gesto;
- y se paga en **todos** los fotogramas mientras la selección siga viva, no solo mientras se arrastra.

La primera versión de este anexo lo explicaba como «veinte rectángulos de más al pintar», y la aritmética
se contradecía sola: sesenta microsegundos entre veinte rectángulos serían tres cada uno, cuando encolar
las 1 911 celdas enteras cuesta 190. Los rectángulos de más existen —una celda realzada tiene un fondo
distinto del de la ventana, y el pintor se salta las que llevan ese fondo—, pero no son el grueso.

**Hasta dónde llega lo medido, dicho antes de que alguien se fíe de más.** La tanda marca **veinte
celdas de una fila**, que es un gesto de los que se hacen —una ruta, una orden—. Lo que **no** se ha
corrido es el otro extremo: marcar la pantalla entera, que es el gesto de «copiar toda la salida». Ahí, al
barrido por celda se le suma **un rectángulo de fondo por cada celda marcada** —el pintor se salta las que
llevan el fondo de la ventana, y una realzada no lo lleva—, y eso son 1 911 rectángulos por fotograma
contra los veinte de la tanda. Que ese sumando no sea el grueso está **razonado, no corrido**.

**Y dura más de lo que parece: una selección sigue viva —y cobrando— hasta que alguien la tira.** No la
tiran teclear, ni perder el foco, ni cambiar de pestaña, ni mover la división: solo un clic seco o las tres
reglas del motor de aquí arriba. El camino cotidiano que lo paga es marcar una línea, copiarla y seguir
trabajando: el `cargo build` siguiente barre la rejilla en cada fotograma **con la marca ya fuera de
pantalla**. Y **cada consola sostiene la suya**, así que con cuatro divisiones se paga cuatro veces.

**Queda una pregunta abierta, y no es de este hito:** preguntar por celda con la selección viva se puede
acotar mirando primero si la fila entra en el rango. No se hace ahora porque sería optimizar contra una
cifra que este anexo acaba de declarar **no aislada**, y este proyecto no optimiza contra números que no
sostiene.

**Y el gesto se declara con TRES números en la cabecera, no con dos.** A los gestos inyectados y los que
surtieron efecto se les suma **en cuántos fotogramas quedó texto marcado** —fotogramas y no gestos: cada
gesto extiende la selección una vez por fotograma de arrastre, así que sale mayor—. Ese tercero lo leen las
tandas en los dos sentidos: la que mide marcar exige **más de uno por gesto**, y **todas las demás exigen
cero**. La razón es **D20**: desde que el botón primario a secas marca texto, cualquier gesto que pierda su
asidero se convierte en una selección **sin que nada más lo delate**, porque se inyecta entero y declara
efectos igual. Comprobado quitándole el acorde al gesto de mover consolas: la tanda `J` se invalida
diciendo que dejó texto marcado en 183 fotogramas. Y en el otro sentido, la que marca exige **más de uno
por gesto** y no simplemente más de cero: cada gesto extiende la selección una vez por fotograma de
arrastre, así que pedir «al menos uno en toda la corrida» dejaría pasar una tanda en la que solo el primero
marcara.

---

## D23 — El ratón al programa de dentro: quién se queda cada gesto, y en qué se lo cuenta

**Decisión.** Una consola cuyo programa **ha pedido** el ratón recibe los gestos que ocurren sobre ella, en
un informe que **escribe este proyecto** porque la librería del motor no lo trae. El reparto tiene un orden
fijo, se decide **al apretar** y no se relee, y `Mayúsculas` es la salida que devuelve el gesto al terminal
pase lo que pase.

### El orden, y por qué es ése

| Quién se lo queda | Cuándo | Por qué va ahí |
|---|---|---|
| **Mover la división** | `Ctrl+Mayúsculas` apretado | Es de la ventana. Si un programa pudiera quitárselo, abrir `vim` en una consola la dejaría clavada donde está |
| **La selección** | `Mayúsculas` apretado | Sin esta salida, dentro de `htop` o de `vim` no habría **ninguna** forma de sacar una línea de la pantalla. Es lo que hace todo terminal con esa misma tecla |
| **El programa** | Lo ha pedido | Lo normal en cuanto hay una interfaz de pantalla completa dentro |
| **La selección** | Nadie más lo reclamó | El caso de todos los días: un shell no pide el ratón |

**Para la rueda el orden es el mismo con una rama más**, y ésa es la que hace que `less` y `man` se
desplacen sin pedir el ratón: `Mayúsculas` → siempre el historial; el programa lo pide → sus dos números
propios; **pantalla alterna** → flechas; si no → el historial.

**Y el significado se fija al apretar**, exactamente como en **D20**: los gestos entran todos por el mismo
botón, y releer el acorde en cada fotograma haría que soltar una tecla a mitad de un arrastre cambiara lo
que el gesto significa **con el botón todavía apretado**.

### La codificación es nuestra, y dónde se corta

La caja del motor sabe decir **si** el programa quiere el ratón —son sus banderas— pero armar el informe lo
hacía el binario de Alacritty, no su librería. Así que la tabla es de este proyecto:

```
extendida:   \x1b[<{botón};{columna};{fila}M   al apretar
             \x1b[<{botón};{columna};{fila}m   al soltar
antigua:     \x1b[M{32+botón}{32+columna}{32+fila}
rueda:       botón 64 (arriba) y 65 (abajo);  arrastrar suma 32;  Ctrl 16 y Alt 8
```

Columnas y filas **empiezan en uno** y son las de la rejilla visible.

**Lo que queda fuera: la codificación de coordenadas en UTF-8.** La antigua mete cada número en un byte con
32 sumado, así que **satura en la columna 223** — y lo que hace `xterm` sin ese modo es exactamente eso:
dejar de informar. Pasado ese punto **no se manda nada**, nunca un número aproximado: unas coordenadas
equivocadas el programa las entiende como un clic en otro sitio, y eso es peor que no recibir nada. Es el
mismo corte que este proyecto ya declaró para las teclas de función, y por la misma razón: escribir una
tabla sin poder verla funcionando es adivinar.

**Y `Mayúsculas` no viaja en el informe**, aunque la codificación tenga sitio para ella: en esta ventana esa
tecla fuerza la selección, así que un gesto con ella apretada no llega nunca a codificarse. Ponerla sería
una rama muerta que además sugiere lo contrario de lo que la ventana hace.

### Lo que se leyó en el motor y no se supuso

**El desplazamiento alternativo viene ENCENDIDO de fábrica.** Está en el modo por defecto del motor —leído
en su fuente, igual que hace `xterm`—, así que en la práctica lo que decide que la rueda salga como flechas
es **la pantalla alterna**. La bandera se sigue preguntando porque un programa **puede apagarla**: `vim` lo
hace en cuanto quiere el ratón para sí, y darle flechas entonces movería su punto de inserción sin que nadie
tocara una tecla. La primera prueba de esto se escribió suponiendo lo contrario y **falló**: es un caso más
de que la API ajena se lee.

### La medida: serie propia, y una etiqueta que dice exactamente lo que abarca

Lo que cuesta el ratón se anota en **su propia serie**, no en la de la tecla. Es la misma razón por la que
aquélla excluye el pegado: son dos poblaciones distintas bajo una etiqueta que solo describe a una. Un
arrastre mete **decenas de muestras por gesto** —una por celda cruzada— contra la única que mete una tecla,
así que promediarlas juntas dejaría la cifra que **D3** publica describiendo sobre todo al ratón.

**La etiqueta es «codificar el gesto de ratón → byte en el programa», y no la de la tecla.** La primera
versión de este anexo lo publicaba como «del fotograma que ve el gesto al byte», que es lo que mide la serie
de la tecla — y **es falso**: este reloj empieza mucho después, con el recorrido del árbol, el reparto del
puntero y resolver la celda **fuera**, dentro del reloj que ya cronometra atender el ratón entero. Bajo
aquella etiqueta publicaba un tramo estrictamente más corto que el que nombraba, que es el error que este
proyecto castiga por encima de los demás. Corregido en el código, en el informe y aquí. **Los dos números no
se comparan.**

**Anexo — medido.** Tanda `O`, dos corridas del mismo binario, promediables según el comparador. Hace **el
mismo recorrido** que la tanda que marca texto y solo se distingue en que el programa de la consola
izquierda ha pedido el ratón; la derecha es el control dentro de la misma corrida.

| Corrida | Gestos | Informes | Codificar y escribir, media | El 95 % |
|---|---|---|---|---|
| 1 | 1 001 | 3 004 | 5,126 µs | 10,480 µs |
| 2 | 1 387 | 4 164 | 5,286 µs | 10,200 µs |

**Lo que cuesta codificar un gesto y escribírselo al programa: 5,1–5,3 µs de media y unos 10 µs el 95 %.**

**Y los informes salen exactamente a tres por gesto** —apretar, un movimiento, soltar—, que es lo que
confirma que ningún camino informa de más: la primera versión mandaba **cuatro**, con un movimiento
publicado **después** del soltado.

**Y esta cifra sí está aislada, al revés que la de D22.** No sale de restar una consola de la otra: es su
propia serie, cronometrada entre dos puntos del mismo camino, y sus muestras se cuentan **por informe** y no
por fotograma. Lo que **no** se puede hacer con esta tanda es restarle la consola de control para sacar «lo
que cuesta el ratón por fotograma»: la asimetría entre izquierda y derecha existe sin gesto ninguno, y esa
resta ya publicó dos números falsos en este mismo documento.

**Un aviso más, del mismo tipo que el de D15:** las dos corridas **no comparten régimen** —1 001 y 1 387
gestos inyectados—, así que sus medias **por fotograma** no se promedian. Las de esta serie sí, porque su
unidad es el informe.

**Y los efectos se cuentan por GESTO, no por informe.** Un gesto de ratón son el apretón, sus movimientos y
el soltado; contando cada uno, la tanda publicaba **el triple de efectos que de gestos** y su columna dejaba
de ser comparable con las de las demás tandas, donde los efectos nunca pasan de los gestos. Cuenta el
apretón, que abre el gesto, y el giro de rueda, que **es** un gesto entero por sí mismo. Con el código
final la tanda declara **1 001 gestos y 1 002 efectos**.

### Las guardas nuevas, y en los dos sentidos

La cabecera del informe declara ahora **cuatro** números del gesto, no tres: a los inyectados, los que
surtieron efecto y los fotogramas con texto marcado se les suma **cuántos informes de ratón salieron**. Ese
cuarto se lee igual que el tercero:

- **Y las flechas no cuentan.** Lo que sale cuando la rueda se traduce son las mismas teclas que manda el
  teclado; publicarlas bajo «N informes de ratón» metía muestras ajenas en esa serie **y hacía saltar la
  guarda** que exige cero informes en toda tanda que no sea la del ratón — una corrida correcta con un
  `less` desplazándose se marcaba inválida.
- **La tanda del ratón exige más de uno por gesto.** Sin ese número, una corrida en la que el programa nunca
  llegara a pedir el ratón —la secuencia que no sale, un binario viejo— publicaría los tramos de una
  **selección** bajo la etiqueta del ratón, y las dos tandas serían **indistinguibles leyendo el informe**:
  hacen el mismo recorrido.
- **Y exige además que no marcara nada**, porque los dos caminos corriendo a la vez mezclarían sus tramos.
- **Todas las demás exigen cero.** Un informe que se escapara metería una ida al pseudoterminal dentro de la
  media de otra tanda.

**Probadas fallando, que es la única forma de saber que sirven.** Quitándole a la tanda la línea que hace
que su programa pida el ratón, la corrida se invalida diciendo que **dejó texto marcado en 1 990
fotogramas** — el gesto entero se resolvió como una selección, exactamente el escenario contra el que la
guarda existe. Sin ella, ese informe se habría publicado como bueno y sus tramos serían los de la tanda que
marca texto con la etiqueta del ratón encima.

---

## D24 — Las fuentes del sistema se buscan cuando hacen falta, no al arrancar

**Decisión.** El terminal **no enumera las fuentes de la máquina al abrir la ventana**. Las busca la
primera vez que aparece un carácter que ninguna de las caras registradas cubre, y registra entonces la
que lo cubra. **Elegido con los números delante**, igual que el techo del historial en **D21**.

### Los números, medidos aquí

`cargo run --release --example fonts -p app`, cinco muestras por punto y **tres corridas del binario con
la máquina en reposo**. Máquina de desarrollo con **259 familias** instaladas.

**En frío y en caliente van por separado, y ésa es la mitad del anexo.** El banco cachea los ficheros que
abre, así que la primera muestra paga abrirlos y las demás no. **La vía perezosa paga siempre la fría** —
una vez por escritura nueva, que es justo lo que ocurre en producción.

**Y por eso la columna fría se publica como rango de corridas y no como un valor.** El frío **ocurre una
vez por corrida**: dentro de una corrida no hay cinco muestras frías que promediar, hay **una**. Así que
la única forma de saber cuánto se mueve es **repetir la corrida**, y se movió — hasta un **40 %** en el
número que sostiene esta decisión. Publicar el valor de una corrida como si fuera *el* valor es
exactamente lo que la regla dura del proyecto prohíbe, y la primera versión de este anexo lo hacía.

| Qué | En frío (3 corridas) | En caliente |
|---|---|---|
| **Enumerar** las fuentes del sistema | **17,3 – 23,2 ms** | 15,1 ms |
| **Registrar 4 caras** en el motor de texto | **6,2 – 7,8 ms** · 24,5 MiB de fuente en memoria | 1,9 – 2,2 ms |
| **Registrar 1 cara con la ventana andando** | **3,4 – 4,8 ms** — del **21 % al 29 %** de un fotograma a 60 Hz | 1,2 – 1,5 ms |
| Encontrar quién cubre 日 **por script** | **570 – 852 µs** | 140 – 210 ns |
| Encontrar el emoji **a color por familia genérica** | **7,2 – 10,8 µs** | 0,67 – 0,98 µs |
| Encontrar quién lo cubre **abriendo las 259 en orden** | de 94 µs a **18,2 ms** | 94 – 152 µs |

> **La columna caliente sí es estable, y el contraste es el dato.** Enumerar en caliente dio 15,07, 15,10,
> 15,17 y 15,12 ms en cuatro corridas seguidas — cuatro cifras dentro del 1 %—, mientras su frío iba de
> 17,3 a 23,2. Lo que varía es **abrir ficheros**, no recorrer listas. Y sirve de aviso: **la primera
> versión de este anexo publicaba 13,7 ms de caliente, un número que no se ha vuelto a ver en ninguna de
> las cuatro corridas.**

### Por qué perezoso, en una resta

**Al arrancar costarían entre 23,5 y 31,0 ms** —enumerar más registrar, sumando los dos extremos de cada
rango—, y son ms que **hoy no se pagan**.

> **Con qué se compara, dicho con precisión, porque la primera versión de este anexo lo dijo mal.** Decía
> que «multiplica por 5,3 lo que hoy se tarda en llegar al primer fotograma», tomando los **5,42 ms** de
> **D16** como si fueran el arranque entero. **No lo son**: ese número es lo que cuesta **restaurar la
> disposición**, y es **el único tramo del arranque que este proyecto tiene medido**. Abrir la ventana,
> montar el framework y cargar las fuentes de fábrica **no están medidos**, así que no hay un total con el
> que dividir. Lo que sí se puede afirmar: son **entre cuatro y seis veces el único tramo medido**, y se
> pagarían **antes del primer fotograma**.

**Buscar cuando hace falta cuesta entre 4,0 y 5,7 ms** —encontrar la cara más registrarla, sumando otra vez
los extremos— y ocurre **una vez por escritura nueva**, no por carácter ni por fotograma. Son **del 24 % al
34 % de un fotograma**: un tirón medible, puntual, y a cambio de no pagar nada al abrir.

**La decisión aguanta el rango entero, y ésa es la razón de publicarlo.** El peor caso de la vía perezosa
(5,7 ms) contra el **mejor** del arranque (23,5 ms) sigue siendo una distancia de **más de cuatro veces**
— y el arranque se paga **siempre**, mientras que lo perezoso se paga **solo si el carácter aparece**.
Ninguna combinación de los extremos medidos le da la vuelta.

> **Ese porcentaje es cuatro veces lo que decía la primera versión de este anexo, y conviene decir por qué.**
> Publicaba «1,3 ms, un 8 %» por tres errores que la revisión encontró **corriendo el banco**: la mediana
> de cinco muestras publicaba el caso **caliente** cuando la vía perezosa paga el **frío**; las dos mitades
> de la suma estaban medidas **con fuentes distintas** —la búsqueda con una de 19,5 MiB y el registro con
> otra de 3,8—; y los tiempos de búsqueda medían **una copia del fichero de fuente**, no la búsqueda.

> **Y esos 24,5 MiB no son un detalle.** Es el fichero de fuente **entero por cara**, no los glifos que se
> usan. Con la vía perezosa solo se paga la memoria de lo que de verdad aparece en pantalla.

### Lo que la medida encontró y no se buscaba

**Una fuente que «cubre» un emoji puede pintarlo en blanco y negro, y ganar por orden alfabético.**
Recorriendo las familias en orden, quien se lleva `😀` en esta máquina es **DejaVu Sans**, y el banco lo
publica con esas palabras: `DejaVu Sans [monocromo]` — teniendo `Noto Color Emoji` instalada. Es
exactamente el defecto que Alacritty tiene documentado desde hace años.

**Preguntar por la familia genérica de emoji lo arregla, y además es lo más barato de todo el anexo:
7,2 – 10,8 µs.** Así que la regla no es «la primera que cubra», es **«la primera que cubra con la
presentación que toca»** — y el número de arriba ya está medido **con esa regla**, no con la de solo
cobertura.

**Y `Noto Color Emoji` no tiene contornos de color: es un mapa de bits de 136×128.** Confirma por el lado
duro lo que la lectura de la fuente de egui ya decía — su rasterizador solo dibuja contornos, así que
registrar esa fuente **no daría ni la silueta**: no daría nada.

**La dispersión de recorrerlo todo no es «según dónde caiga en el orden», es según lo que pese la
ganadora.** `Adwaita Sans` está pronto y pesa 0,88 MB: 94 – 134 µs. `Noto Sans CJK HK` está más tarde y
pesa 19,5 MB: 12,2 – 18,2 ms. Lo que se paga es abrir el fichero, no recorrer la lista — y la primera
versión de este anexo atribuía el rango a la posición, que es una causa falsa.

### Lo que cuesta guardar los emojis ya dibujados

La celda de este terminal mide **16,3 puntos de alto**, preguntados al motor de texto y no inventados. Un
glifo reescalado a esa celda ocupa:

| Aumento del escritorio | Por glifo | Mil glifos |
|---|---|---|
| 100 % | 1.024 B | **1,0 MiB** |
| 200 % | 4.096 B | **3,9 MiB** |

**El techo se dimensiona en píxeles y el escritorio decide cuántos hay por punto**, así que la cifra
depende del aumento — lo mismo que **D10** estableció para el ancho del panel. La primera versión de este
anexo publicaba una sola cifra con una altura de celda inventada y tratada como píxeles.

**Ojo con el otro número:** los **68 KiB** del mapa de bits son su tamaño **una vez descodificado a RGBA**.
La fuente lo guarda comprimido — el fichero entero de `Noto Color Emoji`, con todos sus glifos, son
10,5 MiB.

### Lo que este banco NO dice, y hay que medir aparte

- **Qué cuesta pintar el glifo ya rasterizado.** Eso vive dentro del fotograma y se mide con una tanda del
  guion, no con un banco. Va en la entrega que traiga el color.
- **Cuánto cuesta el arranque entero.** Solo hay medido un tramo —restaurar la disposición—, así que hoy
  no se puede decir qué fracción del arranque sería cualquier cosa que se le añada.

### Lo que la vía perezosa obliga a recordar, y no es obvio

**Hay que recordar también los caracteres que NADIE cubre.** Un carácter cubierto se busca una vez y su
cara queda registrada, así que el siguiente ya no busca. Pero uno que ninguna fuente de la máquina tenga
**no deja nada registrado**, así que sin recordar el fallo volvería a recorrer las candidatas cada vez que
apareciera — y ése es justo el carácter que un programa roto escupe en bucle. Es un requisito de la
entrega que traiga el descubrimiento, y sale de esta decisión.

### Y dos consecuencias de compilación que se descubrieron aquí

**Descubrir fuentes en Linux se apoya en fontconfig, y enlazarlo exigiría `pkg-config` y sus cabeceras
para compilar** — comprobado: la máquina donde se escribió esto no las tiene y la compilación falla en el
guion de construcción, no en el código. Se carga **en ejecución** en su lugar, que es la misma técnica —y
la misma razón— por la que `libwayland` llega hoy de rebote. El precio está declarado: la ausencia de
fontconfig se descubre **al arrancar** en vez de al compilar, y es el caso que el criterio de aceptación
de este hito obliga a aguantar sin caerse.

**Y una dependencia «solo de desarrollo» no es inocua para el binario que se publica.** Las dependencias
de desarrollo participan en la resolución de versiones: añadir `fontique` movió `windows-sys` de 0.59 a
0.60 para `arboard` —el portapapeles, que **sí** va en el binario— y metió la pila de `windows 0.62`. En
Linux no cambia nada; **en Windows, que es la plataforma de F2.7, ya cambia lo que se compila**. Lo
encontró la revisión leyendo el fichero de bloqueo.

---

## D25 — El emoji a color se pinta como imagen, y todos los glifos viven en un atlas propio

**Decisión.** Un carácter cuya presentación pide color **no se le pide al motor de texto**: se rasteriza
aparte y se pinta como **imagen**. Y todos los glifos rasterizados se pegan en **una sola textura** de
512×512 píxeles, no en una textura cada uno.

Las dos mitades salen de razones distintas, y las dos están medidas.

### Por qué como imagen y no por el motor de texto

**epaint 0.36.1 no sabe rasterizar un glifo a color.** Su rasterizador coge **solo** el contorno
(`outline_glyphs`) y lo pinta con `set_paint(WHITE)`; en todo su módulo de texto no hay una mención a las
tablas de color de una fuente. Comprobado leyendo su fuente en `text/font.rs:241`.

Así que registrar `Noto Color Emoji` en la cadena de reserva —que es lo que **D24** hace para el CJK y los
iconos— **no daría emojis a color**. Daría la silueta. Y en esta máquina no daría ni eso: `Noto Color
Emoji` es un **mapa de bits de 136×128 sin contornos**, así que el motor no tendría nada que dibujar.

**Lo que sí hay en el árbol es la pieza que hace falta**: `skrifa` —que epaint ya usa por debajo— sabe
sacar el mapa de bits de un glifo, y el pintor de este proyecto **pinta celda a celda**, así que el emoji
como imagen es una rama más en un `if` que ya existía, no un cambio de arquitectura.

### Por qué un atlas, y el número que lo decidió

**La primera versión daba una textura por glifo, y la medida la tumbó.** El teselador de epaint no puede
juntar en la misma malla dos formas con identificadores de textura distintos —lo dice él mismo: «Mixing
different `texture_id` in the same…»— así que **cada emoji de la pantalla partía el lote de dibujo**.

**Casi el triple de lo que cuesta dibujar, y por trabajo que no es del framework.** Con atlas, todos los
emojis de la pantalla se dibujan juntos. Es lo que hacen los terminales que son el listón de este
proyecto, y por esta razón.

**Pero el atlas solo no bastaba, y el segundo número lo dice.** El motor junta formas **consecutivas** que
compartan textura, y el pintor recorre la rejilla celda a celda: una letra, otra letra, un emoji, otra
letra… así que el lote se partía igual, ahora entre el atlas de fuentes y el de emojis. La solución es
pintar **en dos pasadas** —todos los textos y después todos los emojis—, que es la misma disciplina con la
que este pintor ya separaba los fondos de los glifos, y por la misma razón.

### La resta entera, con el control corrido con el mismo binario

| El resto del fotograma — la cifra con la que **D3** juzga a egui | Dos consolas volcando emojis |
|---|---|
| Una textura por glifo | **743,4 µs** |
| Atlas, pintado entremezclado | 565,9 µs |
| Atlas y dos pasadas | 370,8 µs |
| **Atlas, dos pasadas y el atajo del ASCII** | **356,3 µs** |
| *Control: las mismas dos consolas en ASCII* | *268,8 µs* |

**Lo que hay que mirar del control no es la diferencia con la tanda, sino que no se ha movido.** 268,8 µs
con este binario contra los 274,3 que publicó el hito anterior: **una pantalla sin emojis no paga nada**
por que el terminal sepa pintarlos, ni siquiera la pasada de más. Eso es lo que hace aceptable el resto de
la tabla.

Con emojis, dibujar cuesta un 33 % más y encolar un 26 % (289,8 → 366,1 µs). Es el precio de **117 emojis
por consola** en pantalla, que es una carga deliberadamente peor que cualquier uso real.

### El atajo del ASCII, y el defecto que casi entra con él

La pasada del color recorre la rejilla entera para pintar unos pocos emojis, así que descarta el ASCII
**sin consultar las tablas de Unicode**. Duplicar ahí una regla que vive en otro sitio solo es aceptable
si algo avisa cuando las dos dejan de decir lo mismo, y ese algo es una prueba que recorre los 128
caracteres con los dos selectores detrás.

**Lo que el atajo ahorra no se distingue del ruido, y se dice así.** 366,1 µs contra 377,5 en el tramo del
pintado: la diferencia entre dos corridas de la misma tanda es del mismo orden. Los 92 µs que cuesta la
pasada de más los paga **recorrer la rejilla otra vez**, y eso el atajo no lo evita. Una versión anterior
de este anexo se los atribuía a él — un número con la etiqueta cambiada, que es justo lo que este proyecto
dice que es peor que no tener número.

**La escribió y la tumbó a la primera.** El atajo descartaba **todo** el ASCII, y Unicode le da `Emoji=Yes`
a la **almohadilla, al asterisco y a los diez dígitos**: `#` seguido del selector es `#️⃣`. Once teclas que
habrían salido en monocromo, en el hito cuyo objeto es que salgan a color. El atajo ahora descarta solo el
ASCII **sin nada pegado**.

### El techo dejó de ser un contador y es mejor así

El diseño pedía «la memoria de los glifos rasterizados tiene techo declarado». Con atlas, el techo es
**una textura de tamaño fijo**: la memoria que la tarjeta gráfica dedica a los emojis es **1 MiB y no
cambia**, aparezcan uno o mil. A la altura de celda de esta máquina caben unos ochocientos glifos —17
píxeles de alto, 18 de ancho—, muchos más de los que una persona ve en una sesión. Cuando se llena **se
dice por la salida de error** y los emojis que no estén ya dentro salen en monocromo.

Un contador habría dado el mismo límite con memoria que crece hasta él. La textura fija lo da desde el
primer emoji.

### Qué queda fuera, dicho con su razón

**Los contornos de color —`COLR`— no se pintan.** Hacerlo exige rasterizar contornos a mano, que es el
rasterizador propio que este hito descartó **a propósito** por invalidar el histórico de medida entero.
En Linux no se nota, porque las fuentes de emoji del sistema son de mapa de bits; **en Windows sí**,
porque `Segoe UI Emoji` es `COLR`. Queda declarado como lo que le falta a **F2.7**, no como algo que aquí
funcione.

**Los modificadores de tono de piel tampoco.** Son caracteres **anchos**, así que el motor de terminal les
da celda propia en vez de pegarlos a la del emoji base, y la regla de presentación no puede mirar la celda
siguiente: no lo tiene en su firma. La base sale a color y el modificador sale por su cuenta.

**Y el panel de estado no pinta color.** Descubre fuentes —eso lo arregló la entrega anterior— pero su
texto lo maqueta el motor de una vez, no celda a celda, así que un emoji en un resumen sale monocromo. El
hito trata de la consola; esto queda dicho para que nadie lo lea como un fallo.

### Lo que se descartó

- **Registrar la fuente de emoji en la cadena de reserva**, como se hace con el CJK. No da color: da la
  silueta, o nada. Es la primera cosa que se probó y la que reordenó el hito entero.
- **Una textura por glifo.** Medido arriba: casi triplica la cifra de **D3**.
- **Reescalar en la tarjeta gráfica** en vez de en la CPU, subiendo el mapa de bits a su tamaño original.
  Serían 68 KiB por emoji en memoria de vídeo en vez de 1,2 KiB, sesenta veces más, y a cambio de un
  filtrado que a esta escala no se distingue.
- **Un rasterizador propio para todo**, que es la opción C del abanico de este hito. Sigue escrita como
  **destino** para cuando este proyecto cambie de framework (**D3**), no como descarte.

---

## D26 — La configuración la escribe una persona, y por eso se tolera por línea y en texto plano

**Decisión.** El fichero que quien usa el terminal escribe para configurarlo es **texto plano
`clave = valor`**, con comentarios de línea completa, y **se tolera por línea**: la que no se entiende se
ignora, se dice **con su número**, y el resto del fichero se aplica.

Vive en `$XDG_CONFIG_HOME/agent-terminal/config`, **al lado** de la disposición y no dentro: aquélla la
escribe la aplicación, ésta la escribe una persona.

### Esto NO reabre D16: es el lado que D16 ya resolvió

**D16 discrimina por autor, con estas palabras**: *«La tolerancia es por documento porque este fichero **lo
escribe siempre la aplicación**. Es lo contrario de lo que hace el contrato del panel, y a propósito:
**aquel lo escribe un tercero a mano** y perder el panel entero por un campo que nadie mira sería lo peor
posible»*.

El fichero de configuración cae de ese segundo lado. Un documento a medias en la disposición solo puede
venir de un error propio, y adivinar media disposición es peor que no restaurarla; aquí hay un **repuesto
exacto para cada línea** —el valor de fábrica— y quien escribe se equivoca de tecla.

> **Una versión anterior de este diseño proponía «acotar D16», y estaba equivocada.** Lo destapó el retador
> del diseño y se comprobó leyendo la decisión. Queda escrito porque el error es instructivo: se iba a
> cambiar una decisión que ya contemplaba el caso, por no haberla releído entera.

### Por qué texto plano y no JSON, que ya está en el árbol

**La tolerancia por línea es el requisito, y un error de sintaxis global la rompe.** En JSON una coma
olvidada impide construir el documento entero — y la coma olvidada es el error más común que existe al
teclear.

Y **la tolerancia por campo no salva de eso**: el contrato del panel es JSON y tolera por campo, pero puede
permitírselo porque **lo genera un programa**. Aquí no.

En texto plano el peor caso es **una línea**. Y admite comentarios, sin los cuales un fichero de
configuración no se explica a sí mismo. Es lo que hacen **Ghostty y Kitty**, que usan este mismo formato —
una confirmación, no la razón.

**Descartado TOML**: resolvería los comentarios pero no el error global, y añade una dependencia al árbol
que la política del manifiesto de la raíz obliga a justificar.

### El comentario es de línea completa, y no es una simplificación

**La almohadilla es a la vez el carácter de comentario y el prefijo de un color.** Un comentario al final
del valor parte `color.1 = #ff0000` justo antes del valor, así que la primera versión del intérprete **se
comía todos los colores**. Lo destapó la suite al primer intento, con siete pruebas en rojo.

### La versión es opcional, al contrario que en la disposición

**Y la razón es la misma que D16 da para exigirla allí.** Aquel fichero **congela la forma de un árbol**,
así que un lector viejo malinterpretaría un fichero nuevo. Éste es plano: una clave desconocida se ignora y
se dice, y una que falta toma su valor de fábrica. **No hay forma que congelar**, así que exigir una línea
ritual no protegería de nada.

Lo que sí se conserva es la mitad que protege: **una versión mayor que la conocida no se aplica**.

### Bajo el banco de medida, el fichero no se lee

**Y esto sostiene más de lo que parece.** El guion de medida ya aísla el directorio de configuración, pero
**quien lanza el binario a mano con las variables de medida —que es como la documentación dice abrir una
disposición concreta— leería la suya**. Su tamaño de letra cambiaría la rejilla, y de la rejilla dependen
**todas** las cifras publicadas de este proyecto.

Hay una segunda consecuencia que lo hace más necesario todavía: el guion **invalida una corrida** si una
tanda que no vuelca en japonés busca fuentes. Con una fuente configurada leída bajo medida, **toda tanda
enumeraría y toda corrida saldría inválida**.

El veto es **del fichero entero** desde el primer día, aunque los colores no cambien la rejilla: «solo estos
ajustes» sería una lista que se olvida de crecer.

### Lo que queda fuera, con su razón

- **Traer un tema desde otro fichero** (`incluir`). Se cortó en el reto al diseño: añadía una ruta, un tope
  de recursión, una regla de ciclo y un convenio de directorio **para cero criterio de aceptación**.
- **Un catálogo de temas con nombre**, que es lo que la gente comparte de verdad —Ghostty trae más de 460—.
  Es **lo primero que entra después** de este hito, no un descarte.
- **Recargar al guardar.** No lo pide ningún criterio, y el vigilante que existe sigue rutas fijadas al
  armarlo, una por consola: no vale tal cual.
- **Elegir el programa que lanza cada consola.** En cuanto el programa o sus argumentos vengan de
  configuración, lanzarlo pasa a ser **superficie sensible** y hay que auditarlo como tal. Merece su propia
  revisión, no un rincón de este hito.

### Copiar no es un acorde reasignable: es un interruptor, y hasta este hito no copiaba

**El escritorio se queda `Ctrl+Mayúsculas+C` antes de que sea una tecla.** Lo entrega ya convertido en una
petición de copiar, y **sin decir si había Mayúsculas**: su comprobación es «mandato más la letra C». Así
que el acorde de copiar de un terminal **no llega nunca como tecla**, y la tabla de atajos no puede
alcanzarlo.

De ahí salen las dos consecuencias de este anexo:

1. **La acción sale de la tabla.** Once acciones reasignables, no doce. Aceptar
   `atajo.copiar = ctrl+shift+e` habría sido publicar un acorde que no se dispara nunca. La clave sigue
   existiendo —es la que este terminal ya publicó— y **solo admite `ninguno`**, que es lo que devuelve el
   acorde a `vim` o a `tmux`. Cualquier otro valor se rechaza diciendo por qué.
2. **Copiar estaba roto desde F2.8, y nadie lo vio porque su prueba sembraba la tecla.** La petición caía
   hasta la tabla del teclado, que traduce `Ctrl+C` a `\x03`: **pedir copiar interrumpía el programa**. Las
   pruebas del hito que lo entregó encolaban un `Event::Key` con la letra `C` —un camino que en la ventana
   de verdad no se recorre—, así que pasaron un hito entero sobre código muerto. Ahora encolan el evento
   que manda el escritorio.

**El arreglo es el mismo que el proyecto ya usaba para pegar**, y estaba escrito como pendiente en el
propio código desde F2.6: *«quien traiga el portapapeles tendrá que decidirlo antes, en el bucle, donde el
estado de los modificadores sí está disponible»*. Con Mayúsculas copia; sin ellas, `Ctrl+C` sigue
interrumpiendo. Apagado, el evento cae hasta la tabla y sale por `\x03`, que es justo lo que espera quien
se lo acaba de devolver a su programa.

**Lo que se pierde, y se dice:** la guarda que impedía que mantener el acorde apretado volviera a copiar en
cada fotograma. El evento del escritorio **no trae la marca de repetición** —`egui-winit` la descarta a
propósito—, así que no hay dónde mirar. Costaba una copia idéntica por fotograma mientras la tecla
estuviera apretada; hasta hoy esa guarda vivía sobre el camino de la tecla, que no se recorre, así que en
la ventana de verdad **nunca estuvo activa**. No se cambia nada por perderla.

### Un acorde sin Ctrl llega dos veces, y consumir una sola lo deja actuar y escribir

`egui` emite el texto de la tecla **junto a la tecla** siempre que sea imprimible y no haya mandato. Los
once acordes de fábrica se libraban por casualidad: los que no llevan Ctrl caen sobre teclas que no
escriben nada. Pero el fichero acepta `shift+e` y el documento lo publica como válido — y así,
`atajo.partir-al-lado = shift+e` partía la consola **y además escribía una `E` en el shell**.

Se consumen los dos: al consumir la tecla como atajo se descarta el texto que viene detrás. La alternativa
—rechazar los acordes sin Ctrl sobre teclas imprimibles— era más barata y le quitaba a quien configura
`shift+F5` y compañía por el mismo precio.

---

## D27 — Todo el repositorio va en inglés: código, documentos, commits y ramas

**El código propio de este proyecto estuvo en español desde el primer commit** —7 de agosto de 2026—:
tipos, funciones, campos, módulos, los nombres de diez ficheros, los comentarios, el rustdoc, los 853
nombres de prueba y los mensajes de sus afirmaciones. También lo que ve quien usa el terminal: las claves
del fichero de configuración y los mensajes por pantalla.

**Y esa decisión no estaba escrita en ningún sitio.** Ésta es la primera vez que aparece en este
documento, dieciocho días después de tomarse. El proyecto tiene como regla dura que *no hay decisiones
que vivan solo en el código*, y esta la incumplió desde el día uno — que es exactamente por lo que creció
sin que nadie la mirara: nunca hubo un párrafo al que volver.

**Lo que había en la práctica era un híbrido sin enunciar.** En inglés, los nombres de las cajas y todo lo
que toca al ecosistema (`Screen`, `Size`, `Point`, `spawn`, lo que viene de `alacritty_terminal`, `egui` o
`serde`); en español, el dominio propio (`Ventana`, `Pestaña`, `Celda`, `Desplazamiento`,
`RatonReclamado`). La frontera era real y funcionaba, pero al no estar dicha producía líneas como
`pub struct Screen` con un `pub struct Celda` al lado.

### La decisión

**Todo lo que vive en el repositorio pasa a inglés**: el código propio, los nombres de fichero, los
comentarios, el rustdoc, las pruebas, los documentos, **los mensajes de commit, los nombres de rama y los
PR**. Y también lo que ve quien usa el terminal: las claves del fichero —`background`, `size`,
`key.split-right`— y los mensajes por pantalla.

**Sin compatibilidad con las claves en español, y no es un descuido**: nadie tiene este terminal
instalado todavía, así que no hay ningún fichero de configuración ahí fuera al que respetarle nada. Una
tabla de alias sería deuda desde el minuto cero, mantenida para un usuario que no existe. Es la única
ventana en la que este cambio sale gratis, y por eso se hace ahora.

**El español se queda solo en la conversación de trabajo**, que no vive en el repositorio.

### Por qué

- **El listón que este proyecto se puso es el de un usuario de WezTerm, Ghostty o Kitty** (**D19**), y
  ninguno de los tres se lee en español. Un terminal que no se puede leer no se puede contribuir, ni
  auditar, ni bifurcar.
- **Cuanto más tarde, más caro.** Son 43.860 líneas propias hoy, y cada hito las aumenta.
- **Pide el árbol limpio**: durante la migración cualquier rama abierta da conflicto en todo. Por eso va
  justo después de mezclarse F2.10 y **por delante de su cuarta entrega**.

### Lo descartado

- **Dejarlo en español y escribir la D que faltaba.** Cierra el incumplimiento por minutos y no cuesta
  nada. Se descarta porque no resuelve el problema de fondo: el código sigue sin poder leerlo nadie de
  fuera, y el precio de arreglarlo sube con cada hito.
- **Inglés solo de aquí en adelante.** Es la peor de las tres: deja dos idiomas mezclados **sin frontera
  que los explique**, que es peor que cualquiera de los dos puros — y es justamente el estado del que este
  hito sale.
- **Traducir los comentarios a máquina.** Este repositorio vale por lo que sus comentarios explican, no
  por sus líneas de código: llevan el porqué de cada decisión con mucho matiz —«miente», «el disfraz
  fichado», «el defecto y el arreglo dan el mismo resultado»— y una traducción mecánica los convierte en
  ruido. Se traducen a mano, y ahí es donde se va el tiempo del hito.
- **Aceptar las claves en los dos idiomas.** Es lo que haría **D26** si hubiera alguien a quien
  respetarle un fichero ya escrito — pero no lo hay: el terminal no está instalado en ninguna parte. Una
  tabla de alias nace como deuda y se mantiene para siempre por un usuario que nunca existió.

### Los commits, las ramas y los PR, desde ya

**No esperan al hito.** Se escriben en inglés desde esta misma decisión — son cosa nueva cada vez, así
que no hay nada que migrar y aplazarlo solo produciría más material que traducir. Los que ya están
escritos se quedan como están: reescribir el histórico cuesta más de lo que aclara.

**Los documentos son la excepción, y solo mientras dure el hito.** Una decisión en inglés dentro de un
`02-decisiones.md` en español se lee peor que las dos cosas puras. Se traducen **enteros**, de una vez,
en su entrega — no se van mezclando fichero a fichero.

### Lo que este hito tiene que decidir antes de escribir código

Queda abierto a propósito, porque decidirlo aquí sería adelantar trabajo del hito:

1. **Dónde queda la frontera** con el ecosistema, enunciada y no solo practicada.
2. **Cómo se entrega.** Un solo cambio de 43.860 líneas no lo revisa nadie.
3. **Qué mecanismo para los mensajes por pantalla**, si es que hace falta alguno más allá de escribirlos
   en inglés.
4. **Qué pasa con el conocimiento ya almacenado**, escrito en español.

### El riesgo que no es de traducción

**El histórico de medida se apoya en nombres que aparecen en informes y en guiones.** El banco de medida
invalida corridas por lo que el informe **dice** —`configuración: vetada`, los recuentos de arrastres, el
testigo de las fuentes—, y esos textos los buscan guardas de `scripts/medir-fotograma.sh` con `grep`.
Renombrar sin mirar no rompe una prueba: **rompe una guarda**, y una guarda rota publica como buena una
corrida mentirosa. Es el mismo modo de fallo que este proyecto ya pagó una vez, cuando la guarda del
guion estaba muerta y su verificación fue falsa.

### El glosario, y las dos fronteras que lo ordenan

Esto es lo que el hito tenía que decidir antes de escribir código, y lo que las nueve entregas
siguientes aplican sin volver a discutir. **No es una tabla de traducción: es una tabla de decisiones**,
y cada fila que no es obvia lleva su razón al lado.

#### La primera frontera: de quién es el nombre

Después de este hito ya no hay dos idiomas, así que la frontera **deja de separar idiomas y pasa a
separar dueños**: cuándo se adopta la palabra que el ecosistema ya usa y cuándo se acuña una propia.
La pregunta sobrevive a la traducción, y por eso hay que contestarla.

> **Un nombre nuevo se decide con tres preguntas, y manda la primera que dé «sí».**
>
> 1. **¿El tipo *es* el de una dependencia** —reexportado, o envoltura que tiene que seguirle la
>    forma? **Se adopta su nombre.** `Pty`, `Screen`, `Size`, `Dims`, `TermColor`, `Underline`,
>    `Watcher`.
> 2. **¿Es una palabra que quien usa el terminal ya tiene?** El listón es su usuario (**D19**), así
>    que el criterio es comprobable y no de gusto: **la palabra aparece en la configuración, la
>    documentación o los nombres de acción de dos o más de {WezTerm, Kitty, Ghostty, tmux, Zellij}**.
>    Si sí, **se adopta aunque el tipo sea nuestro**: `Pane`, `Tab`, `Cell`, `Divider`,
>    `MouseReporting`, `Tiling`.
> 3. **Si no es ninguna de las dos, el nombre es nuestro** y describe **nuestro** mecanismo: `Pump`,
>    `Weight`, `PtyOutput`.
>
> **Y una prohibición que gobierna las tres: no se adopta una palabra que el ecosistema ya usa para
> otra cosa.** Eso descalifica `Window` para la división —es lo que Kitty llama a una división y lo
> que tmux llama a una pestaña— y `Border` para la frontera, que en Kitty y en tmux es el marco *de*
> un panel.
>
> **Y la asimetría que eso deja hay que decirla, porque salta a la vista:** `Window` sí es el nombre
> de la ventana de verdad. No es una excepción a la prohibición, es la prohibición leída bien — lo que
> prohíbe es adoptar una palabra **que significa otra cosa**, y para la ventana del sistema `Window`
> significa exactamente eso, igual que en `winit`. Quien la usa para otra cosa es `egui`, donde una
> «window» es un panel flotante dentro de la aplicación; ése es el nombre del que hay que cuidarse al
> importar, no el concepto.

**Esta regla no es nueva: `term-core` la practica desde F1.1 y la tiene escrita tres veces**, las
tres en el mismo fichero, en los comentarios de la celda, del ratón reclamado y del desplazamiento. La
que la enuncia entera es la del desplazamiento: **«Vocabulario propio, no el del motor, por lo mismo
que el resto de esta caja: quien la use no tiene que conocer los tipos de sus dependencias ni migrar
con ellos.»**

> Y aquí una corrección que este anexo se hace a sí mismo, porque el registro de decisiones no puede
> llevar una cita falsa: la primera versión decía **cuatro** veces e incluía el tipo de los caracteres
> combinantes, que **no lleva ese comentario** —su rustdoc habla de por qué no va dentro de la celda—;
> y la frase que citaba era **un injerto de dos de los tres comentarios**. Son tres, y la cita literal
> es la de arriba.

Lo que faltaba no era la regla, era **enunciarla y decir dónde no se aplica**: su argumento tiene
sujeto —«quien use esta caja»— y **en `app` no hay nadie**, porque no se publica. Ahí manda la
pregunta 2, la del usuario, y no la de la migración que nadie va a hacer.

#### Cuando el nombre correcto choca con un `use`, se le pone alias al ajeno

Dos nombres de este glosario **no compilan tal cual**, y los dos por la misma razón: el módulo donde
vive el tipo propio ya importa un nombre idéntico de una dependencia.

| Nombre propio | Qué lo bloquea |
|---|---|
| `Scroll` (el desplazamiento por el historial) | su fichero importa `Scroll` de `alacritty_terminal::grid` |
| `Watcher` (el vigilante del directorio del panel) | su fichero importa el *trait* `Watcher` de `notify`, y **tiene que estar en ámbito** para poder llamar a `watch` |

> **La solución no es cambiar el nombre propio: es ponerle alias al ajeno.** Un `use … as GridScroll`
> para el tipo, y un `use notify::Watcher as _;` —importación anónima de trait, que es justo para
> esto— para el que solo hace falta en ámbito. El vocabulario del proyecto no cede el sitio al de una
> dependencia que además no debería asomar: eso es lo que esta caja lleva defendiendo desde F1.1.

**Y una clasificación de la primera versión que estaba mal, no solo estorbando:** el vigilante
figuraba como nombre **adoptado** por la pregunta 1, «el tipo *es* el de una dependencia». No lo es.
`notify::Watcher` es un **trait** con `watch` y `unwatch`, y el tipo propio no le sigue la forma:
guarda un vigilante dentro y levanta una bandera. Es un nombre **acuñado** por la pregunta 3, y da la
casualidad de que coincide con el del ecosistema.

**Avisos que hoy no rompen y conviene tener escritos**, porque están a un `use` de distancia: `Cell`
—el fichero hermano ya importa la celda de `alacritty_terminal`—, `Flag` —`notify::event::Flag`
existe, en la misma lista de importación que ese módulo ya usa—, `Row` —`epaint::text::Row` es «una
fila de texto maquetado», exactamente lo que el tipo propio es— y `Theme` —`egui::Theme` existe, misma
palabra y mismo dominio—. Ninguno colisiona hoy; los cuatro colisionarían con un `use` más.

#### La segunda frontera: lo vivo y lo que está en disco

También estaba escrita, y en el sitio correcto: el enumerado que dice en qué sentido reparte una rama
existe **aparte** de su gemelo vivo porque «**los nombres de este enum son el formato**: aparecen
literalmente en el fichero de todo el que use la aplicación; reutilizar el tipo vivo ataría el fichero
a un nombre que el código puede querer cambiar mañana». Lo mismo con el nodo, el hijo y la pestaña,
que existen dos veces por esa razón.

**Si los dos gemelos acaban llamándose igual en inglés, se pierde justo esa distinción.** Así que:

> **Un tipo cuyos nombres son el formato en disco lleva el prefijo `Stored`.** El test es mecánico y
> **sin excepciones**: ¿el tipo deriva `Serialize` o `Deserialize` en ese módulo? Prefijo.

| Vivo | En disco |
|---|---|
| — (`Documento`) | **`StoredDocument`** — la raíz: sus dos campos *son* `{"v":…, "pestañas":[…]}` |
| `Tab` | `StoredTab` |
| `Node` | `StoredNode` |
| `Child` | `StoredChild` |
| — (`Hoja`) | `StoredLeaf` |
| `Orientation` | `StoredOrientation` |
| — (`Sonda`) | **`StoredProbe`** — solo lee la versión, pero la lee *del fichero* |

La primera versión de esta regla **se incumplía a sí misma dos veces**: dejaba fuera la raíz del
formato y el tipo que lee la versión, los dos con `derive` puesto. Un test mecánico con dos
excepciones escritas a mano no es un test mecánico, y por eso ahora no las tiene. Los tipos de ese
módulo que **no** derivan serde —el problema del documento, la política de guardado, el fallo de
escritura, el documento ya validado— no llevan prefijo.

**Y el prefijo avisa del tipo, pero el formato lo hacen los campos *y las variantes*.** Dos enumerados
llevan `rename_all`, así que **los nombres de sus variantes son el fichero**: renombrar la variante de
las filas deja el fichero escribiendo `"rows"` mientras el guion de medida sigue sembrando `"filas"`.
Eso **no lo caza el compilador, ni la suite, ni el prefijo**: solo la guarda de la tanda que restaura,
y solo si alguien la corre. Es el agujero real de este hito, y está aquí escrito para que nadie lo
descubra dos veces.

**`panel-contract` no lleva prefijo**, y es coherente: **la caja entera es el formato**, así que su
nombre ya es la etiqueta.

#### La división es un `Pane`, y el gesto es `split`

Comprobado con fuente y no de memoria, que es lo que este proyecto exige de cualquier API ajena:

| Terminal | Sustantivo | Dónde |
|---|---|---|
| WezTerm | **`pane`** | `trait Pane` en `mux/src/pane.rs`; `ActivatePaneDirection`, `PaneSelect` |
| tmux | **`pane`** | `select-pane`, `pane-border-style` |
| Zellij | **`pane`** | `Action::NewPane`, el bloque `pane {}` de su configuración |
| Ghostty | `split` | `new_split`, `goto_split`, `split-divider-color` |
| Kitty | `window` | una división es un «kitty window» dentro de un tab |
| Alacritty | — | no tiene el concepto; lo delega |

**`Pane` la cosa** —tres de cinco, y `Division` **no lo usa nadie**: es la forma española—, y **`split`
el gesto**, que ya estaba comprometido sin que nadie se diera cuenta: esta misma decisión escribió
`key.split-right` como ejemplo de las claves nuevas, que es exactamente lo que hacen WezTerm y
Ghostty. Las once claves de atajo pasan a `key.*`.

**Un hueco que se declara en vez de rellenarse:** ninguno de los cinco tiene palabra para el **peso**
de un panel en un árbol n-ario. Lo más cercano son el `bias` de Kitty —**un entero de 10 a 90**, y de
una pareja— y el `ratio` de Ghostty —un 0–1, también de una pareja, y **que además no es vocabulario
de usuario**: vive en su árbol de divisiones interno y no en su configuración, así que **no pasa la
propia pregunta 2 de este glosario**—. Aquí los pesos son n y suman lo que sea. Por eso `Weight` es
acuñado y no adoptado: adoptar `bias` o `ratio` **mentiría sobre la aritmética del árbol**.

#### La trampa del calco, que se cobró cinco nombres

**Un calco que coincide con un nombre del ecosistema y no significa lo mismo es peor que una
traducción libre.** La regla se escribió al empezar el glosario y aun así cazó cinco nombres ya
propuestos:

| Tipo | Primer nombre, y por qué cae | Queda |
|---|---|---|
| `Resultado` | `Result` es de `std` y significa otra cosa | **`Outcome`** |
| `Recogida` | `Drain`: `vec::Drain` y `HashMap::Drain` son **iteradores**, y este tipo es un resultado con dos campos | **`Pickup`** |
| `Adjuntos` | `Combining` nombra el **referente**, no el tipo: el tipo es un tramo empaquetado de 27+5 bits que apunta a un vector aparte | **`CombiningSpan`** |
| `Reparto` | `Partition` es de disco y `Iterator::partition` reparte por un predicado; y `Allotment`, para un nativo, es un huerto de alquiler | **`Tiling`** |
| `Gesto` (del ratón) | `MouseEvent` nombra una **entrada**, y el tipo es una **salida**: el informe ya dice «informes al programa» | **`MouseReport`** |
| `Puente` | `Reader` sin implementar `io::Read` miente por convención, y lo que el tipo hace —traer la salida **y la señal de muerte**— es justo lo que un `Reader` no hace | **`PtyOutput`** |

#### El reparto de `Layout`, que es el choque de verdad

Tres tipos y un fichero querían la misma palabra:

| Hoy | Qué es | Queda | Por qué |
|---|---|---|---|
| `Reparto` | los rectángulos de las partes de un área **y las líneas que las separan** | `Tiling` | «tiling» es el término de los gestores de ventanas en mosaico, que es este dominio exacto |
| `Hueco` | el reparto **dentro** de una división, entre consola y panel | `PanelTiling` | **emparejado a propósito**: su comentario dice que tiene nombre «por lo mismo que lo tiene `Reparto`… dos repartos que se leyeran distinto serían **dos idiomas para una sola idea**» |
| `Maqueta` | una maqueta de texto ya calculada | `PanelLayout` | aquí `Layout` sí es lo que es: texto colocado |
| `Disposicion` | con cuántas consolas abre cada pestaña | `OpeningLayout` | es lo que se **pide** al abrir, no lo que resulta |
| `disposicion.json` | el fichero | `layout.json` | el nombre que espera quien abre su carpeta de configuración |

#### Lo que no se traduce nunca

- **Las claves y valores de `panel.json`.** Y la razón buena **no** es que ya estén en inglés: son
  **contrato con productores de fuera** (**D17**), así que la licencia de «sin compatibilidad» que
  esta decisión concede **no las alcanza**. Esa licencia cubre las claves de atajo y el fichero de
  disposición, que solo lee esta máquina. Ojo además con tres que **no se llaman como su campo**: la
  clave es `v`, `stale_after_minutes` y `ref`.
- **Identificadores del sistema y del protocolo:** `OPOST`, `SIGWINCH`, `TIOCSWINSZ`, `SGR`, `CSI`,
  `XDG_CONFIG_HOME`, `SHELL`, `PANEL_ID`, `PANEL_DIR`.
- **Nombres de caja:** `alacritty_terminal`, `egui`, `eframe`, `epaint`, `serde`, `serde_json`,
  `notify`, `skrifa`, `fontique`, `portable-pty`, `winit`, y los propios `term-core`,
  `panel-contract`, `host-load`. (**No `fontdb`**, que la primera versión de esta lista citaba: no
  está en el árbol — la caja de fuentes de este proyecto es `fontique`.)
- **Nombres propios y etiquetas:** `WezTerm`, `Ghostty`, `Kitty`, `tmux`, `vim`, `htop`, `Nerd Font`,
  `COLR`, `D1`–`D27`, `F1.0`–`F2.12`, y **las letras de tanda `A`–`Q`**, que no tienen idioma.
- **Y los literales cuya *longitud* es la carga de prueba.** El ejemplo que pinta la pantalla de
  comprobación coloca un carácter ancho **justo en el borde de la columna 80**, y su comentario dice
  que «sin esta fila, la rama del pintor que trata ese hueco no la mira nadie». **Ese fichero no tiene
  ni una prueba.** Traducir ese texto mueve el carácter de sitio y **desactiva la rama en silencio**.
  Un texto así se mide en columnas, no en palabras.

#### Los homónimos, que no son duplicación

Se creyó que la coexistencia de un `Problem` inglés con dos `Problema` españoles era el precio de no
haber escrito la frontera. **Es falso**: uno es «lo que le pasa al fichero de configuración entero» y
otro «por qué un documento de disposición no sirve». Son tipos distintos en módulos distintos, y el
idioma solo los estaba haciendo parecer parientes. **No se fusiona ninguno** —fusionar cambia
comportamiento bajo suite verde, y eso es otro hito— y el criterio da tres respuestas:

- **Mismo concepto, referente distinto → sustantivo compartido y calificador propio:**
  `ConfigProblem`, `LayoutProblem`, y el `Problem` de `panel-contract` se queda como está porque su
  caja ya lo califica. Igual `Linea` (la del fichero de configuración) → **`ConfigLine`**, frente al `Line` del panel, que se queda.
- **Concepto idéntico duplicado a propósito → nombre idéntico:** los dos `Directorio` de prueba pasan
  **los dos a `TestDir`**, porque su comentario dice «el mismo molde que usa el módulo hermano» y
  calificarlos esconderían que son un patrón. No `TempDir`: es de `tempfile`.
- **Conceptos distintos que compartían nombre por accidente → nombres sin nada en común:** los dos
  `Gesto` no son parientes. El de la ventana es lo que **la tanda de medida se inyecta** →
  `InjectedGesture`; el del ratón es lo que el ratón hizo para codificárselo al programa →
  `MouseReport`. Traducir los dos a `Gesture` convertiría el accidente en intencionado.

#### El resto del glosario

**Núcleo de la ventana:** `Ventana`→`Window` · `Pestaña`→`Tab` · `Division`→`Pane` ·
`Consola`→`Console` · `Nodo`→`Node` · `Hijo`→`Child` · `Peso`→`Weight` · `Frontera`→`Divider` ·
`Orientacion`→`Orientation` con variantes `Columns`/`Rows` —**las variantes nombran el resultado y no
el eje**, que es lo que su comentario defiende— · `Lado`→`Side` · `Cierre`→`Closing` ·
`TamañoPedido`→`RequestedSize` · `ConQueAbrir`→`OpenWith` · `Puerta`→`Entry` · `Bombeo`→`Pump` ·
`Orden`→`WindowRequest` (**no `Command`**: ya está tomado por lo que se lanza).

**La familia de «lo que X deja para el resto del fotograma» son cuatro, no siete**, y comparten
sufijo porque su valor es reconocerse entre sí: `DelRaton`→`MouseOutcome` ·
`DeLaRuedaAlPrograma`→`WheelOutcome` · `DeLaBarra`→`TabBarOutcome` · `DeLaDivision`→`PaneOutcome`.

Y los otros tres **no son de la familia y no llevan el sufijo**: `DeLaRueda`→`WheelInput`, porque es
lo que la rueda **pidió** y no lo que deja; `DelBoton`→`ButtonPress`, que es un evento; y
`DeLaSesion`→`SessionHeader`, que es la cabecera del informe.

**Ratón y teclado:** `RatonReclamado`→`MouseReporting` · `QueHaceElArrastre`→`DragIntent` ·
`QuienSeQuedaLaRueda`→`WheelOwner` · `Donde`→`CellPos` · `Accion`→`Action` · `Acorde`→`Chord` ·
`Atajos`→`Keymap` · `Desplazamiento`→`Scroll` · `Mitad`→`Half`.

**Configuración, sesión y panel:** `Ajustes`→`Settings` · `Colores`→`Colors` · `Aviso`→`Warning` ·
`Avisos`→`Warnings` · `Falta`→`Missing` · `Valido`→`Validated` ·
`Fallo`→`SaveFailure` · `Persistencia`→`Persistence` · `Estado`→`PanelState` ·
`Reclamo`→`AttentionState` (**no `Claim`**: en inglés es «afirmación» o «derecho») · `Fila`→`Row` ·
`Vigilante`→`Watcher` · `Vigilada`→`Watched` · `Bandera`→`Flag` · `Cola`→`Queue` · `Tema`→`Theme` ·
`Metrica`→`CellMetrics`.

**El banco de medida** — su regla ya estaba escrita y se conserva: **«se llama por el acto y no por la
cosa»**, porque un tipo llamado `Configuracion` dentro del módulo de medida se leería como «la
configuración» en vez de como «lo que costó leerla». `Serie`→`Series` · `Tramos`→`Spans` ·
`Medida`→`Timings` · `RestoDelFotograma`→`RestOfFrame` · `Guardado`→`Saving` ·
`Descubrimiento`→`Discovery` · `Rasterizado`→`Rasterizing` · `EnPuntos`→`SizeInPoints` ·
`Reflujo`→`Reflow` · `Barra`→`TabBar` · `Restauracion`→`Restoring` ·
`LecturaDeAjustes`→`ReadingSettings` (**no `SettingsRead`**, que se lee como participio y rompe la
familia en `-ing`) · `Arrastres`→`Drags` · `ArrastresInyectados`→`InjectedDrags` · `Vuelta`→`Round` ·
`CosteDelColor`→`ColorCost` · `Despertares`→`Wakeups` · `Resumen`→`Summary` · `Barrido`→`Sweep` ·
`Prueba`→`TestCase` · `Veredicto`→`Verdict` · `Montaje`→`Harness`.

**Los que la primera versión se dejó fuera**, encontrados cruzando el glosario con los tipos que
existen hoy: `Opciones`→`Options` (los argumentos de la línea de órdenes del medidor de carga) ·
`Entrega`→`Delivery` (lo que costó volcar el fichero) · `Recogida`→`Pickup` (el resultado de una
recogida de la cola, que solo se deducía de la fila del calco) · y los cuatro del contrato del panel,
internos o de ejemplos: `Banco`→`Bench` · `Contador`→`Counter` · `Forma`→`Filler` · `Uso`→`Usage`.

**Fuentes y glifos:** `Cara`→`Face` · `CaraDelSistema`→`SystemFace` · `Descubridor`→`Discoverer` ·
`Presentacion`→`Presentation` · `EnElAtlas`→`InAtlas` · `Glifos`→`Glyphs` · `Celda`→`Cell` ·
`Adjuntos`→`CombiningSpan`.

#### Tres cifras del cuerpo que el inventario deja atrás

D27 se escribió con los números de su día y **el inventario del hito los subió**. No se corrigen
arriba —este documento añade con fecha, no edita hacia atrás— pero quedan aquí para que nadie los
cite viejos: **46.458 líneas** propias y no 43.860, **859 nombres de prueba** y no 853, y **dieciocho
ficheros** con nombre español y no diez. La diferencia no es un error de cuenta: **F2.10 entró en
medio**, y es la demostración literal del «cuanto más tarde, más caro» que la propia decisión usa
como argumento.

#### Qué significa «enteros, de una vez» para los documentos

D27 dice que los documentos «se traducen **enteros**, de una vez, en su entrega — no se van mezclando
fichero a fichero», y la frase admite dos lecturas. **Manda la que su propia razón sostiene**: el
ejemplo que da es «una decisión en inglés dentro de un `02-decisiones.md` en español», o sea **la
mezcla dentro de un fichero**. Así que:

> **«Entero» es cada documento, y «de una vez» es su entrega.** Ningún documento se queda a medias
> —eso es lo prohibido—, y la entrega puede llevar un cambio por documento. Con 3.186 líneas solo en
> este fichero, un único cambio con los ocho documentos dentro no lo revisa nadie, y lo que D27
> protege no es el tamaño del cambio: es que no se lea medio documento en cada idioma.

Queda escrito porque la lectura contraria es defendible leyendo solo la frase, y una decisión que
admite dos lecturas se discute otra vez dentro de tres semanas.

#### Los ficheros son dieciocho, no diez

La decisión contó los diez de `app/src` y se dejó ocho: `ajustes.rs`→`settings.rs` ·
`teclado.rs`→`keyboard.rs` · `sesion.rs`→`session.rs` · `fuentes.rs`→`fonts.rs` ·
`medida.rs`→`measure.rs` · `pintor.rs`→`painter.rs` · `tema.rs`→`theme.rs` ·
`vigilante.rs`→`watcher.rs` · `puente.rs`→`pty_output.rs` · `raton.rs`→`mouse.rs` ·
`barrido.rs`→`sweep.rs` · `fuga.rs`→`leak.rs` · `pantalla.rs`→`screen_demo.rs` ·
`lectura.rs`→`read.rs` · `entrega.rs`→`delivery.rs` · `historial.rs`→`scrollback.rs` ·
`api_sin_terceros.rs`→`api_without_deps.rs`, más el ejemplo de fuentes de `app`.

**Y uno más que no estaba en ninguna lista: el propio `scripts/medir-fotograma.sh`**, que pasa a
`measure-frame.sh`. Su nombre se cita en **quince ficheros** —tres módulos, este documento y diez del
conocimiento almacenado—, así que se renombra en la entrega del banco y **sus citas en prosa se
corrigen en las entregas de prosa**: una cita al nombre viejo dentro de un comentario que todavía está
en español no rompe nada ejecutable, y el alternativo es un cambio que cruza el repositorio entero.

#### Una discrepancia que se declara en vez de resolverse en silencio

Para lo que el programa de dentro **pide** sobre el ratón, el estándar dice *mouse tracking* (DECSET
1000–1003) y los terminales del listón dicen *reporting* (`mouse-reporting` en Ghostty,
`bypass_mouse_reporting_modifiers` en WezTerm). Dos revisiones dieron nombres distintos y **las dos
citas son buenas**. Manda **D19**: el vocabulario de su usuario. Queda `MouseReporting`, y queda
escrito que la otra opción tenía argumento.

#### Lo que se aprendió renombrando `app`, y no estaba previsto

La entrega que renombró la familia de la ventana dejó cuatro cosas escritas, todas encontradas al
hacer el trabajo y ninguna deducible del plan.

**Un renombrado que se le va a otro fichero siempre compila.** Es la propiedad que lo hace peligroso:
el compilador no puede objetar, porque el nombre está cambiado en la declaración *y* en todos los usos.
En esta entrega el mapa se fue a tres ficheros que no le tocaban, y cada uno enseñó algo distinto:

- Al `Gesto` del ratón le puso `InjectedGesture`, que es el nombre que este mismo anexo reserva para el
  **otro** `Gesto`. Lo que aquí arriba se escribió para que dos homónimos no se confundieran acabó
  produciendo la confusión al revés: **el nombre correcto, en el tipo equivocado**.
- Al `Accion` del teclado le tradujo **una** variante de veintiuna. Es la misma forma que la fuga de
  `Lado` de la entrega anterior, y sobrevivió al mismo barrido por la misma razón: **una expresión
  regular de declaraciones no ve nunca una variante de `enum`**. El oráculo que sí la ve es diferenciar
  los **conjuntos** de nombres declarados antes y después, no buscar líneas de declaración.
- A `Hueco` le puso `PanelTiling` **con la razón equivocada**, diciendo que un enlace de rustdoc lo
  obligaba. El enlace nombra un tipo del fichero que sí se estaba renombrando, así que actualizar la
  cita bastaba. Y lo que hace este caso el más instructivo de los tres es que **la conclusión era
  correcta**: el reparto de `Layout`, más arriba, dicta `Hueco`→`PanelTiling` y dice que el
  emparejamiento con `Reparto` es a propósito. Se revirtió creyéndolo fuga y volvió al comprobar el diff
  contra los 84 pares del anexo. Dos lecciones, no una: **una razón plausible no es una razón
  verdadera**, y **una conclusión correcta con razón falsa no se defiende sola** — la primera vez que
  alguien la audite, se cae.

**La lista de exenciones se compara con la caja puesta.** La guarda del glosario lleva una lista de
nombres que siguen en español a propósito porque los declara un fichero de una entrega posterior. Se
comparaba en minúsculas, y así cada entrada cubría **dos cosas distintas**: el tipo ajeno y el local
propio del mismo nombre — `Gesto` y `gesto`, `Nodo` y `nodo`, `Hueco` y `hueco`. Lo segundo es
precisamente lo que la guarda existe para cazar. Rust lo resuelve gratis, porque los tipos van en
`CamelCase` y los locales en `snake_case`.

**Una exención que nombra algo del propio fichero no protege a nadie: esconde trabajo.** Dos entradas
de esa lista —`ArrastresInyectados` y `QueHaceElArrastre`— se declaran en `lib.rs`, o sea en el fichero
que la entrega estaba renombrando. Estaban ahí por descuido y su efecto era exactamente el contrario
del que la lista busca. Regla: **cada entrada de la lista se verifica contra el fichero que declara el
nombre**, no contra la intención de quien la escribió.

**«Terminado» es una afirmación, y se parte en dos cuando no se puede sostener entera.** La guarda tenía
una sola lista, `FINISHED`, y se le añadió la capacidad de declarar terminado **un fichero** para poder
apuntarla a `lib.rs`. Apuntándola se vio que no se puede: además de los nombres que ese fichero solo
*alcanza* —declarados en ficheros de entregas posteriores—, quedan **499 identificadores en español que
el glosario nunca tuvo que decidir**, el `desde` de un rango o el `cual` de un índice, y eso ya sin contar
los 356 nombres de función de prueba, que son prosa. Llamar «terminado» a eso sería un número mal
etiquetado, que es lo que **D13** prohíbe.

Así que hay dos listas, y cada una dice lo que de verdad afirma:

| Lista | Qué afirma | Qué la lee |
|---|---|---|
| `FINISHED` | una caja cuyos identificadores están **todos** en inglés | las tres guardas |
| `CITATIONS_CHECKED` | un fichero cuyas **citas** están comprobadas, aunque sus identificadores no | la guarda de las citas |

**Y la segunda lista lleva dos mitades, no una**: el fichero, y **la caja cuyos nombres puede citar**.
Un comentario de `lib.rs` tiene derecho a nombrar algo que declara un módulo hermano; sin esa segunda
mitad, la guarda denunciaba setenta citas perfectamente vivas. Lo descartado: darle a la guarda el árbol
entero como universo, que la habría vuelto ciega a una cita a un nombre de otra caja.

Lo que esto compra, medido en su primer uso: **ciento cincuenta y ocho comentarios** que nombraban
identificadores que ya no existen, **dos de ellos doblemente muertos** —apuntando a un nombre que un
renombrado *anterior* ya había retirado y nadie había notado—. Un comentario así no lo caza el
compilador, no lo caza la suite, y quien busca el nombre que le dan no encuentra nada.

#### Lo que ninguna guarda de este hito puede ver, dicho antes de que muerda

Tres clases, encontradas las tres renombrando la ventana y ninguna cazable por la guarda.

**Un nombre de tipo que además es una palabra corriente, reemplazado en prosa.** `Hueco` es las dos
cosas, y el reemplazo alcanzó dos comentarios donde era la palabra: «hueco de medida conocido» —la
fórmula con la que este proyecto declara un tramo que **no** cronometra— dejó de decir eso. La guarda de
citas lee lo que va **entre comillas invertidas**, y aquí el nombre iba a pelo en el texto. Lo cazó
barrer los nombres nuevos en `CamelCase` buscándolos **fuera** de comillas, y **eso hay que hacerlo a
mano en cada entrega que renombre un tipo cuyo nombre sea también una palabra del idioma**.

**Un intercambio de nombres entre dos cosas del mismo tipo.** Todos los oráculos de diferencia de
conjuntos —nombres declarados, literales, números— son **invariantes bajo un intercambio**: si `ancho` y
`alto` se cambian el nombre entre sí, el conjunto sale idéntico y el compilador calla. Lo que sí lo ve es
un oráculo **posicional**: emparejar las líneas cambiadas una a una y comprobar, token por token en la
misma posición, que cada identificador cambió a lo que la cadena de renombrados dice. Medido en esta
entrega: **5.693 líneas emparejadas, 0 cambios sin explicar**. Sale barato y hay que hacerlo.

**Un nombre de prueba que cita un método.** `absorber_conserva_la_geometria…` mientras el método ya se
llama `absorb`. No lo ve la resta de nombres declarados —no cambió— ni la guarda de citas —no es un
comentario—. Y no tiene arreglo mecánico: **328 de los 366 nombres de prueba** del fichero grande llevan
alguna palabra que esta entrega renombró, y casi todas son la palabra del idioma dentro de una frase, no
una cita. Van con la prosa, que es una entrega y no un barrido.

#### Y una cifra que nadie había contado: la prosa llevaba muchas entregas mintiendo

Al comprobar qué citas de fuera del código mató este renombrado salió el número de al lado, que es el
que importa: **599 citas de la prosa ya nombraban cosas que el código no tiene, y son de antes de este
hito.** Esta entrega mató 121 más (11 en el registro de decisiones, 1 en el de configuración, 109 en el
conocimiento de dominio); el propio registro de decisiones lleva **152 muertas de antes** y 11 nuevas.

Por eso **no se arreglan aquí**: arreglar 11 de 163 en un fichero sería tapar el 7 % del problema y dejar
el documento pareciendo revisado. Lo que cambia es el encargo de las entregas de la prosa y de los
documentos: **no traducen, reparan** — y ahora saben cuánto.

#### El PR de arreglo: la guarda no veía **ningún** nombre en mayúsculas, y afirmaba lo que no había mirado

Va suelto, entre la quinta entrega y la sexta, y por dos razones que no dan para esconderlo dentro de un
renombrado: **una guarda que afirma en falso es peor que ninguna**, y la ceguera que se arregla aquí no
es deuda de la quinta entrega — lleva puesta desde la tercera, la que declaró terminadas las tres cajas
de fuera.

**Partir un nombre antes de cada mayúscula deja una constante convertida en letras sueltas.**
`ANCHO_DE_FABRICA` salía como catorce piezas de un carácter, y el suelo de dos caracteres que el
comprobador aplica después las tiraba todas: **a ninguna regla se le llegaba a preguntar**. Así que un
nombre en `MAYUSCULAS_CON_GUION` podía llevar cualquier palabra en español y la guarda lo daba por
limpio. La regla nueva es una línea: **una pieza sin ninguna minúscula es una sola palabra**.

Lo que enseñó al arreglarlo, que es la medida que justifica el PR: **tres constantes** en un módulo ya
declarado terminado (`ANCHO_DE_FABRICA`→`DEFAULT_WIDTH` · `CONSOLA_MINIMA`→`MINIMUM_CONSOLE` ·
`SUELO_AL_ARRASTRAR`→`DRAG_FLOOR`) y **ninguna** en las tres cajas de fuera — ésas sí estaban limpias, lo
cual solo se sabe ahora.

**Se escribió primero una regla más fina y se tiró.** Cortar donde *cambia* la caja —`PTYOutput` como
`pty` + `output` en vez de `p`, `t`, `y`, `output`— es más bonito y **no cambia ni un veredicto**: las
letras que ahorra las tira el mismo suelo, y la palabra de detrás se encuentra igual. Ningún nombre del
árbol distingue las dos reglas, y este fichero no se queda mecanismos sin escenario.

**Y la lista de ficheros terminados afirmaba «todos los identificadores en inglés» de cinco ficheros de
los que no era verdad en ninguno.** Sus familias sí están renombradas y no les queda nombre que el
glosario retirase —que es lo que la lista comprueba de verdad—, pero siguen llevando el **local en
minúsculas que el glosario nunca tuvo que decidir**: el `resto` de un trozo, el `ahora` de un reloj, la
`fase` de una cabecera. Eso es exactamente el punto ciego que la guarda declara en su cabecera, y la
lista lo tapaba.

**Contados: 195 identificadores distintos** — ratón 7 · teclado 30 · pintor 27 · fuentes 57 · panel 74.
El método, porque el número sin él no vale: se extraen los identificadores de cada fichero con el mismo
saneo que usa la guarda (fuera cadenas, caracteres y comentarios), **se quitan los nombres de función de
prueba** —que son prosa y van con la prosa—, se deja un ejemplar de cada nombre y se clasifica a mano. La
lista, nombre a nombre, va abajo. Los seis exentos a propósito (`Tema`, `tema`,
`color_del_panel`, `color_del_reclamo`, `fondo_del_panel`, `con_atributos`) no cuentan. Un nombre que
aparece en dos ficheros cuenta en los dos: el inventario es por fichero, que es la unidad de la lista.

**Y el primer recuento dijo 190, que es la clase de fallo que este documento existe para no repetir.**
Faltaban cuatro, y los cuatro por la misma razón: tres eran **locales dentro de una prueba** —`solo_clics`,
`en_otra_distribucion`, `sin_alto`— que están **junto a una prueba cuyo nombre lleva esas mismas palabras**,
así que leyendo el listado los tomé por nombres de prueba y los descarté; el cuarto era `Texto`, un tipo de
otro módulo que el pintor nombra, y que cuenta por la misma regla por la que `fuentes` cuenta en tres
ficheros. Lo destapó una revisión adversaria con un único encargo: **recontar**. La lección no es «contar
mejor»: es que **un número producido a mano necesita que otro lo reproduzca**, y por eso el reparto por
fichero se publica y no solo el total. (El quinto candidato, `A_EMOJI`, se cuenta: su `A_` es la
preposición española, igual que en el `A_TEXTO` de al lado.)

**Cuatro nombres que compilan y se leen mal, encontrados por las revisiones de la quinta entrega.** Los
cuatro son de la misma clase: **dos cosas distintas con el mismo nombre a la vista**, que es justo lo que
el glosario existe para impedir y lo que un renombrado mecánico no puede ver.

| Estaba | Pasa a | Por qué |
|---|---|---|
| `Action::PageUp` · `PageDown` | `ScrollPageUp` · `ScrollPageDown` | En `(historial(Key::PageUp), Action::PageUp)` el mismo nombre son **dos tipos** en la misma expresión: la tecla de egui y lo que la aplicación hace con ella |
| `Action::GoToStart` · `GoToEnd` | `ScrollToTop` · `ScrollToBottom` | Van a `Scroll::ToTop` / `Scroll::ToBottom`, así que ahora **se llaman como acaban**; y «start/end» en un terminal se lee como principio y fin de línea |
| `Action::SplitBeside` | `SplitRight` | La propia D27 escribió `key.split-right` como ejemplo de las claves nuevas: el nombre del código decía otra cosa que la clave que lo configura |
| `Action::SplitBelow` | `SplitDown` | Su pareja, por lo mismo |
| `Glyphs::paste` | `Glyphs::blit` | Dos `paste` sin parentesco en la misma caja: éste **pega una imagen en el atlas**, el otro es el portapapeles |
| `paint_character(width: bool)` | `is_wide` | A dos líneas de `metrics.width: f32`. Un `bool` llamado como una medida |

Las cadenas del fichero de configuración —`"partir-al-lado"`, `"subir-una-pagina"`— **no se tocan**: son
las claves que escribe quien usa el terminal y tienen su propia entrega.

**Y la guarda de citas era ciega a los nombres de módulo, por una razón que no se ve leyéndola.** Esa
guarda pregunta si el nombre citado existe **en alguna parte de la caja**, y **todos** los nombres de
fichero retirados siguen existiendo — como **variable local**: «pantalla» es una `Screen` en dos
ejemplos, «fuentes» es una lista de caras en tres módulos, «sesion» es una cabecera en el módulo de
medida, «vigilante» es un vigilante en el módulo que ahora se llama `watcher.rs`. Así que un comentario
que dice «la regla vive en fuentes» **pasa**: la guarda encuentra el local y da la cita por buena. Y
ensancharla no lo habría encontrado nunca, porque el nombre que busca **está de verdad ahí**.

Lo que hace posible cerrarlo es que los nombres **viejos** se conocen: el anexo escribe cada renombrado
de fichero como una flecha, así que la pregunta deja de ser «¿existe este nombre?» y pasa a ser «¿es
uno que retiramos?», que tiene respuesta exacta. La guarda nueva **deriva los diecisiete tallos de este
mismo documento** —el mismo mecanismo que el glosario: derivar, nunca duplicar— y busca sus citas
`` `así` ``, `` `así.rs` ``, `` `carpeta/así.rs` `` y `` `así::funcion` ``.

Encontró **once** —medido corriendo esta guarda contra el árbol de main, que es la única forma de que el
número sea reproducible—, y **nueve eran de verdad**: dos en el manifiesto de `app`, tres en el pintor, una en
el fichero grande, dos en la propia guarda y una en el guion de medida —`vigilante::repartir`—. Las
otras dos no eran citas de fichero: una era una palabra entre comillas invertidas y se reescribe como
palabra, y la otra —«las demás pruebas llegan hasta `pantalla`»— quería decir un tipo, así que pasa a
nombrarlo (`Screen`). Reescribir una y renombrar la otra no es la misma operación, y el guion de la
guarda dice las dos.

**Lee el lado del código y no el de la prosa**, y esa frontera es una decisión ya tomada: las 599 citas
muertas de antes del hito tienen su entrega, así que `docs/`, los dos `README` y el conocimiento
almacenado son de ellas. Aquí queda el código, sus manifiestos y el guion de medida — donde una cita
muerta **no tiene entrega** y se quedaría.

**Y hay una segunda mitad del mismo hueco que NO se cierra con una guarda, y conviene decir por qué.**
El mismo mecanismo —el universo de nombres es la caja entera— deja pasar una cita muerta a un **campo o
parámetro** en cuanto un módulo hermano tenga un local que se llame igual: `caras` sobrevive en
`fonts.rs` porque el ejemplo de fuentes tiene un local `caras`, `problema` sobrevive en `panel.rs`
porque `session.rs` tiene uno, `pegados` sobrevive porque lo tiene el pintor, `marcado` porque lo tiene
el fichero grande.

Estrechar el universo para una cita en minúsculas —«tiene que existir **en este fichero**»— se probó y
se midió, sobre los diecisiete ficheros de la lista de citas y con el mismo criterio en los dos lados:
**61 avisos sobre main, 43 sobre esta rama**. La diferencia son las citas que este PR repara. Y **de las
43 que quedan no hay ni una de verdad**: son nombres de caja (`eframe`, `app`, `keyboard`), métodos de un
módulo hermano que un comentario tiene derecho a nombrar (`arrancar`, `resolve`), campos del contrato que
llegan de otra caja (`done`, `mark`, `style`, `info`, `current`), letras sueltas (`a`, `c`, `i`, `y`),
cosas de `std` (`next`, `unwrap`, `text`), el valor de una variable de entorno entre comillas invertidas
(`consola`) y claves del fichero de configuración (`tamano`).

**Con 43 de 43 falsos no puede romper la compilación**, y la lista de excepciones que haría falta sería
el mecanismo que se ensancha hasta que deja de fallar. Lo que sí cambia: **los 19 avisos ciertos se
arreglan ahora** —once nombres en diez líneas del fichero grande, tres en fuentes, uno en el panel, uno
en el teclado, dos en los ejemplos y uno en la propia guarda—, y el criterio queda apuntado con la
condición que lo vuelve usable: **cuando el fichero grande, la medida, los ajustes y el tema estén en
inglés**, «existe en este fichero» deja de ser ruido, porque entonces un nombre en minúsculas que no está
en el fichero ya no es un local en español de un hermano. Ésa es la entrega que puede añadir la guarda.

**Y la lista de ficheros con citas comprobadas decía «todos los ficheros de la caja» con dos
excepciones que nadie había escrito.** El binario faltaba y entra. La propia guarda **no**, y ésa es la
respuesta y no un descuido: metiéndola salen **65 citas** tal como esta entrega la deja —y el número
**crece con el fichero**, porque lo que reporta es el vocabulario y las formas de las que ese fichero
habla: nombres retirados (`banderas`, `RatonReclamado`), nombres de ejemplo (`CamelCase`, `like_this`,
`Type`) y palabras corrientes entre comillas invertidas. Una guarda cuyo asunto son los nombres que ya
no están no se puede juzgar por «cita un nombre que el código tiene»: **de las 65, una era de verdad**.

**La que sí era de verdad salió de leerse las 56, y conviene que quede escrito:** el comentario que
explica la guarda de citas nombraba tres identificadores vivos, y uno —`prioridad`— **había
desaparecido del árbol entero**. O sea que la frase que explica la guarda era una de las citas que la
guarda describe. Una de cada 65 no es una guarda, pero es un buen barrido. Lo que **sí**
se le puede envejecer es un nombre de fichero, y la guarda nueva la lee para eso: le encontró dos.

La lista pasa a llamarse por lo que comprueba —**«ficheros sin nombre que el glosario retirase»**— y los
195 van a la entrega del fichero grande, que es la que ya tenía ese encargo para sus vecinos. **Y la
lección se escribe entera**, porque es la de este PR: *una lista que exagera lo que miró se lee una vez,
se cree, y la entrega siguiente la usa para decidir qué está hecho.* Es exactamente así como cinco
ficheros acabaron declarados terminados.

#### Lo que la revisión adversaria le encontró a la guarda nueva, y es la mitad del PR

Un mutador con una sola instrucción —«demuestra que algo de este diff no está cubierto»— devolvió
**seis huecos, cada uno con su mutación superviviente ejecutada**. Ninguno se veía leyendo el código, y
uno de ellos es el disfraz que este proyecto ya tiene fichado.

**1. El lector de citas de fichero no lo ejercitaba ninguna aserción.** `fn cites_the_file(..) -> bool
{ false }` dejaba las ocho pruebas **verdes**, y también romper cada una de sus cuatro formas por
separado. La razón hay que escribirla porque es general: **la única prueba que lo llama busca citas, y
el árbol está limpio — así que romper el buscador deja la suite más verde, no más roja.** Un buscador
necesita su propio oráculo; la ausencia de lo que encuentra no lo es. Entra
`the_reader_of_file_citations_knows_the_four_shapes`, con las cuatro formas y tres negativos.

**2. El suelo de diecisiete flechas contaba, no identificaba.** Devolver los diecisiete tallos con un
sufijo pegado —diecisiete nombres que no existen— dejaba el suelo contento y la prueba pasaba sin mirar
nada. Es exactamente «afirmar sobre el conteo en vez de sobre el efecto». Ahora se piden **cuatro
tallos por su nombre**.

**3. El suelo de dos caracteres no lo sostenía nada.** Subirlo a 3, a 5, o **quitarlo del todo**: las
ocho verdes. Con cinco, el guardián deja de ver `celda`, `hueco`, `serie`, `nodo`, `fila` y `mitad`
dentro de cualquier compuesto — y ese número es el que el comentario nuevo de la partición usa para
explicar el defecto que arregla. Dos casos nuevos, uno por valor: un nombre retirado de tres letras y
otro de cinco. Bajarlo no tiene caso y se dice: quitar un suelo solo endurece.

**4. «Sin minúsculas» no es «todo mayúsculas», y la diferencia es un dígito.** Escrito
`chars().all(char::is_uppercase)` la prueba pasaba igual, y el defecto quedaba **reabierto para toda
constante numerada**: el dígito rompe la condición, la pieza se vuelve a partir en letras sueltas y el
suelo las tira. Caso nuevo con un número dentro.

**5. El manifiesto de la raíz tapaba los de las cajas.** Una sola pregunta «¿hay algún `.toml`?» la
satisfacía él solo, así que quitar los `.toml` del recorrido seguía verde — y **dos de las citas
muertas que esta guarda encontró vivían en el manifiesto de una caja**, no en el de la raíz. Ahora se
piden los dos por separado.

**6. Las dos listas se podían vaciar sin que nada fallara.** Vaciar la de ficheros limpios: verde, porque
las cajas terminadas seguían dando objetivos y la guarda de «esto no tiene ficheros» nunca se le
pregunta a una lista **sin elementos**. Y quitar de la lista de citas cualquier entrada —incluida la que
este PR añade— también: un fichero que sale de la lista simplemente deja de mirarse, y nada echa de
menos lo que no reporta. La lista de citas pasa a **derivarse del árbol** («todos los ficheros de la
caja» se comprueba, no se cree, con este fichero como única excepción escrita) y las dos listas de
nombres llevan suelo de cardinalidad.

**Y derivarla del árbol se cobró dos citas muertas más**: al entrar los cuatro ejemplos de `app` en la
lista salieron `` `resolver` `` en dos de ellos, cuando el método se llama `resolve` desde la entrega
anterior. Más dos nombres ajenos que había que declarar (`yes`, el programa; `Zsym`, un código de
escritura de Unicode).

**Las diez mutaciones supervivientes se volvieron a correr una por una: las diez caen ahora.**

##### Los 195, nombre a nombre

**Ya no están**: los renombró la entrega de los ocho módulos, que es la sección siguiente. La lista se
conserva porque es la medida con la que se decidió partir esa entrega en dos, y porque el reparto por
fichero es lo que permitió a otro reproducir el número.

**`mouse.rs` — 7:** `abajo` · `apretar` · `arriba` · `dentro` · `fuera` · `solo_clics` · `soltar`

**`keyboard.rs` — 30:** `antes` · `codigo` · `cual` · `cuerpo` · `en_otra_distribucion` · `escrita` · `escrito` · `esperada` · `esperado` · `historial` · `hostil` · `letras` · `mando` · `nuevo` · `otro` · `parte` · `pelado` · `pisada` · `publicado` · `queda` · `repetida` · `salida` · `salto` · `sin_publicar` · `soltar` · `suyo` · `tabla` · `veneno` · `venenos` · `viejo`

**`painter.rs` — 27:** `Texto` · `debajo` · `dibujo` · `esquina` · `fondo` · `fondo_apagado` · `fondo_realzado` · `fuentes` · `hubo_color` · `letra` · `letra_realzada` · `malla` · `muchas` · `pasada` · `pegados` · `primera_imagen` · `proporcion` · `que` · `rejilla` · `rejillas` · `salida` · `sin_selector` · `suyo` · `tamano` · `textos_sin` · `ultimo_texto` · `una`

**`fonts.rs` — 57:** `A_EMOJI` · `A_TEXTO` · `Texto` · `a_buscar` · `alfa` · `alto_final` · `ancho_final` · `ancho_maximo` · `antes` · `buscados` · `canal` · `coleccion` · `comun` · `coste` · `crudo` · `cuadrado` · `cuadro` · `cubre` · `datos` · `definiciones` · `delante` · `despues` · `empezo` · `fallidos` · `familia` · `fotograma` · `fuente` · `fuentes` · `gastado` · `genericas` · `glifo` · `grande` · `icono` · `letras` · `mapa` · `mascara` · `nombre` · `origen` · `otro` · `pendientes` · `pequeno` · `pixeles` · `primera` · `registradas` · `resueltos` · `se_rindio` · `segunda` · `selector` · `siguiente` · `sin_fuentes` · `sin_presupuesto` · `sitio` · `sitios` · `suya` · `suyo` · `todas` · `uno`

**`panel.rs` — 74:** `a_pintar` · `ahora` · `anchos` · `bloque` · `cabe` · `cabeza` · `callado` · `caso` · `casos` · `clave` · `contenido` · `corte` · `cual` · `cuando` · `cuerpo` · `dicho` · `disponible` · `entera` · `envejecer` · `estrecha` · `fase` · `fecha` · `fichero` · `fondos` · `frase` · `fresco` · `fuente` · `fuentes` · `identidad` · `izquierda` · `larga` · `largo` · `marcas` · `minutos` · `muchas_enes` · `otro` · `palabra` · `paso` · `pegada` · `pendiente` · `pintado` · `pintando` · `primera` · `publicada` · `publicado_hace` · `publicar` · `recien` · `recortada` · `referencia` · `relleno` · `repetida` · `resto` · `ruta` · `salida` · `saneado` · `sangrada` · `segunda` · `segundos` · `siguiente` · `sin_alto` · `sitio` · `sobran` · `suyo` · `tal_cual` · `textos` · `todos` · `tope` · `tres` · `ultimo_bueno` · `valor` · `viejo` · `vigente` · `visibles` · `visto`

#### La entrega de los ocho módulos: 340 nombres, y por qué la entrega se partió en dos

**El plan decía 419 nombres y eran 638.** La cifra del plan se escribió antes de que nadie contara los
cinco ficheros que la quinta entrega declaró terminados; con sus 195 dentro, la entrega que iba a llevar
«el fichero grande y los tres módulos sin entrega» se convertía en **nueve ficheros y 33.000 líneas de
fuente tocada**, el doble de la cuarta entrega, que ya dejó tres fugas de alcance con siete oráculos
delante. Se parte en dos, y el corte no es por tamaño sino **por producto**:

- **Los ocho módulos de esta entrega**: ratón, teclado, pintor, fuentes, panel, tema, watcher y
  pty_output.
- **El fichero grande a solas**, que es un fichero y 23.142 líneas, y el que más pruebas tiene del
  repositorio.

**340 nombres**, uno por uno: panel 74 · pty_output 60 · fuentes 53 · watcher 47 · theme 43 · teclado 30 ·
pintor 26 · ratón 7. Cuatro de ellos tienen **dos destinos** porque nombraban dos cosas —el `SourceCache`
y la vista de fuentes de egui eran los dos «fuentes», el mapa de bits y el charmap eran los dos «mapa»,
la imagen reducida y un tamaño de prueba eran los dos «pequeño», y el trozo que sale del canal y la
cuenta de bytes que se lleva el fotograma eran los dos «trozo»—, así que el mapa **no es apto para un
`sed`** y eso hay que decirlo antes de escribirlo, no después.

**Y la cuenta a mano volvió a fallar, dos veces, en la misma dirección.** Los tres módulos que la quinta
entrega renombró de fichero pero no de contenido se estimaron en 99 nombres, luego en 123 con un filtro,
y son **150** (theme 43, watcher 47, pty_output 60). Las dos veces el error fue el mismo: **filtrar la
lista de identificadores antes de leerla**. El filtro descartaba lo que ya estaba en el corpus inglés del
repositorio, y con él se iban `con`, `marca`, `estilo`, `base` y una docena más de nombres cortos que
resultan ser palabras de las dos lenguas. La lección, escrita porque ya cuesta tres intentos: **la lista
completa se lee entera o no se lee**; un filtro que ahorra trabajo de lectura ahorra exactamente el
trabajo que encuentra los nombres.

**El método que sí converge, y es el que hay que repetir en la entrega del fichero grande:** aplicar el
mapa, **volver a escanear el fichero**, y lo que quede en español es o un descuido o un nombre del
hermano. Lo destapó theme: al reescanear salió `brillante`, un local de prueba que la lista de contexto
me había mostrado solo en tres líneas de comentario.

**Lo que queda, y de quién es.** De los ocho módulos, siete no conservan **ni un** identificador en
español. El octavo, `theme.rs`, conserva seis: `Colores` y los cinco campos que lee de él
—`paleta`, `texto`, `fondo`, `divisor`, `foco`—, que los declara `settings.rs` y **son de la entrega de
las claves de configuración**. `Colores` entra en la lista de exenciones por eso; los otros cinco no hacen
falta, porque el glosario nunca los decidió y la guarda no los ve.

**Y el encuadre con el que se escribió esta entrega era falso, dicho porque es la falta que este hito
lleva castigando.** «Todo `app` menos su fichero grande» se lee como que la superficie está cerrada, y
**`app/src` tiene doce módulos, no nueve**: además de `lib.rs` siguen enteros en español `measure.rs`
(96 raíces españolas), `session.rs` (93), `settings.rs` (57) y `main.rs`, que son cuatro líneas cuya
única función es la de `lib.rs`. Cada uno tiene su entrega escrita —la medida y el formato en disco, las
claves de configuración, y `main.rs` con el fichero grande—, así que **no falta nada del plan**; lo que
faltaba era la frase. Lo cazó una revisión adversaria a la que se le pidió refutar la afirmación, no
comprobarla, y es la diferencia entre las dos cosas.

**Ocho ficheros en la lista de la guarda, y ni una afirmación nueva.** Los ocho entran en la lista de
«ficheros sin nombre que el glosario retirase», que es lo que la guarda **comprueba**. La afirmación más
fuerte —«todos sus identificadores en inglés»— es cierta hoy de siete de ellos, y **no se convierte en
una lista**: la guarda no puede verificarla, y este hito ya cerró un PR entero por una lista que afirmaba
lo que nadie había mirado. Se escribe aquí, con su método y su residuo, que es donde una medida vive.

**La lista de exenciones baja de 27 entradas a 20.** Salen las ocho que existían porque `theme.rs` y
`watcher.rs` no estaban traducidos (`Tema`, `tema`, `con_atributos`, `color_del_panel`,
`color_del_reclamo`, `fondo_del_panel`, `Bandera`, `Vigilante`) y la de `pty_output.rs` (`Puente`); entra
`Colores`. Cada línea que sale es una línea que alguien tenía que borrar, que es para lo que la lista está
escrita así.

**Tres hallazgos que ningún renombrado mecánico podía dar, y los tres los dio una revisión adversaria
antes de escribir código:**

1. **Un ayudante de prueba sin `#[test]`** en `watcher.rs` —`un_vecino_del_directorio_no_levanta_nada`—
   que la guarda **sí** ve, porque lleva una palabra funcional española, y que habría roto la compilación
   en el instante en que el fichero entrase en la lista. Parece prosa y no lo es.
2. **Dos cosas con el mismo nombre a la vista, las dos ya en inglés.** En `fonts.rs`, un local `attached`
   que cuenta cuadrados pegados al atlas, doscientas líneas de un parámetro `attached` que lleva los
   caracteres de anchura cero de una celda. **Ninguna guarda puede verlo**: las dos son inglesas. Pasa a
   `pasted`.
3. **La documentación de la guarda de citas envejeció otra vez.** Nombraba tres identificadores vivos
   para explicar por qué su universo es una caja y no el árbol; uno se había ido antes del PR anterior, y
   el segundo —`parar`— se va con el módulo que lo declaraba. **Dos veces es un patrón**, así que queda
   escrito en su sitio: *el comentario de una guarda envejece igual que el código que vigila, y no tiene
   guarda*.

**Y un aviso sobre el universo de nombres, que es la otra cara de lo mismo.** Tres citas muertas
aparecieron **sin que este PR las escribiera**: `` `tamano` `` en los ajustes, `` `nuevo` `` en el fichero
grande y `` `parar` `` en la propia guarda dejaron de estar cubiertas en el momento en que el nombre que
las tapaba se fue de un módulo hermano. No son regresiones de esta entrega: son las que **ya estaban
mintiendo** y sobrevivían porque el universo es la caja entera. Cada módulo que se traduce destapa las
suyas, y eso significa que **las entregas que quedan van a encontrar más, no menos**.

**Lo que las cuatro revisiones de la entrega cambiaron, que es la mitad de su valor.** Ninguna encontró
un cambio de comportamiento —el oráculo posicional emparejó **1.667 líneas token a token con cero
desajustes**, cero literales de cadena movidos que no sean captura de formato y cero literales
numéricos—, y aun así el PR salió distinto de como entró:

- **Tres citas muertas que la guarda no veía, y un hueco nuevo del lector de citas.** Una cita escrita
  `` `Tema::con(...)` `` volvía **vacía**: el lector solo quitaba un `()` sin argumentos, así que la
  regla de todo-o-nada la descartaba entera y a `Tema` nunca se le preguntaba — un tipo que ya no existe
  en el árbol. Aceptar la forma `nombre(argumentos)` da **cuatro avisos, tres de verdad**, y los tres
  nombran cosas que una entrega renombró hace dos PR (`Lineas(0)`, `Ir(1)`, `desplazar(AlFinal)`).
  **La versión ancha de la misma idea se midió y se tiró**: partir una cita por los espacios y preguntar
  por cada palabra da **309 avisos y ninguno de verdad**, porque `cargo doc`, `panel.json` y
  `Mayúsculas` son palabras entre comillas invertidas que no citan nada. Cuatro contra 309: la línea
  entra, la otra no.
- **Dos citas que no se leen ni con eso**, y las dos siguen arregladas a mano: `visibles + sobran` y
  `Tema::con(...)` llevan espacios o argumentos, y ese hueco **no es el declarado** en la cabecera de la
  guarda —el de «sobrevive en un módulo hermano»—: aquí el nombre no sobrevive en ninguno, simplemente
  no se lee.
- **Cuatro nombres nuevos que mentían, y salieron de leer lo que nombran.** `to_paint` era el peor: su
  propio comentario dice que **el pintado no lo usa** y que dejarlo en producción sería una puerta
  abierta, así que el nombre afirmaba justo el papel que el comentario prohíbe; pasa a
  `last_understood`.
  `dispatch` no despacha a nadie: levanta banderas y devuelve un booleano, así que pasa a
  `raise_matching`. `answered` se lee como «ya atendido» y el valor es un reclamo que **nadie** ha
  atendido, así que pasa a `answer`. Y `cap`, sin calificar en una función larga, pasa a `head_cap`.
- **Cuatro nombres para dos conceptos en quince líneas.** El tema recibía `faint: bool` mientras quien
  llama le pasa `cell.dim`, y dentro convivían ese `faint`, las variantes `Dim`/`DimForeground` y un
  método `dim()` cuyo comentario dice «acerca un color al fondo». El parámetro pasa a llamarse como el
  campo que recibe (`dim`) y el método a decir lo que hace (`toward_background`).
- **Un hueco de cobertura anterior al hito, cerrado porque esta entrega toca esa función.** De los seis
  colores que el fichero de configuración puede cambiar, **el del cursor no lo comprobaba nada**:
  cambiar su asignación por la del foco dejaba las 718 pruebas en verde. Ahora se afirma **por
  `TermColor::Cursor`**, que es la puerta por la que el pintor pide ese color — afirmar sobre el campo
  comprobaría que el valor está apuntado, no que se usa.

**Y dos homónimos que se aceptan a sabiendas, con su razón escrita.** El tipo `Flag` de este proyecto
convive con `notify::event::Flag`, y el tipo `Watcher` con el *trait* `notify::Watcher`, que por eso
entra sin nombre (`as _`). Los dos nombres los **decide el glosario** (`Bandera`→`Flag`,
`Vigilante`→`Watcher`) y los dos choques se resuelven con idioma corriente de Rust —ruta completa y
`as _`—, así que no se reabre una decisión sin una razón nueva. Lo que **sí** queda dicho es el precio:
un futuro `use notify::Watcher` en ese fichero no compila, y quien lo escriba tiene que saber por qué.

**Un aviso que esta misma sección se ganó al escribirse, y hay que contarlo aquí porque es el precio de
derivar.** La guarda **saca el vocabulario de este documento**, y le basta con encontrar un nombre entre
comillas invertidas, una flecha y otro nombre entre comillas invertidas para meter el primero en el
vocabulario retirado — aunque la frase sea prosa corriente y la flecha, puntuación. Pasó: una
explicación de por qué una función cambiaba de nombre puso el tipo booleano de Rust en el lado español,
y la guarda reportó **101 identificadores** en las tres cajas de fuera, todos ellos ese tipo. Se arregla
**reescribiendo la frase y no debilitando el extractor**: esa sensibilidad *es* el mecanismo, y aflojarla
para que la prosa quepa es exactamente cómo un glosario derivado deja de derivar. La regla de uso queda
dicha: **en este documento, una flecha entre dos nombres es vocabulario, no puntuación.**

**Y pasó dos veces.** La segunda fue escribiendo, dos secciones más abajo, la explicación de por qué la
guarda no protege los 317 renombrados: la frase usaba dos nombres de ejemplo con una flecha en medio
para *enseñar la forma*, y el extractor la leyó como vocabulario igual que la primera. O sea que **ni
conocer la trampa evita caer en ella**, porque la forma que hay que citar es exactamente la que dispara.
La conclusión práctica, y es la única que sirve: **en este anexo la forma de una flecha no se cita nunca
con nombres; se describe con palabras.**

#### La entrega del fichero grande: 317 nombres en 23.143 líneas, y lo que se aprendió midiendo

`crates/app/src/lib.rs` es un fichero de **23.143 líneas** con **1.599 identificadores distintos** y
**356 funciones de prueba**, y el que más pruebas tiene del repositorio. Va solo en su entrega, y el
corte no fue por tamaño sino porque *«todo `app` menos el fichero grande»* y *«el fichero grande»* son
dos productos distintos.

**Se partió en cinco áreas por primera aparición del nombre** —constantes y geometría, la consola, el
árbol y las pestañas, la ventana, y las 16.000 líneas de pruebas— y cada área tuvo su propia propuesta
de mapa. **Las cinco salieron mutuamente consistentes: 317 nombres y cero conflictos**, ni uno con dos
destinos incompatibles. Eso no estaba garantizado con cinco revisiones ciegas unas de otras, y la razón
es la que este anexo lleva repitiendo: reutilizaron el glosario y los precedentes de los ocho módulos
ya hechos en vez de acuñar de nuevo.

**Veintitrés nombres significan dos o tres cosas distintas según dónde estén**, y ése es el trabajo de
verdad de esta entrega. `alto` es una coordenada `y` en los tres ayudantes de gesto y una altura en los
demás. `cuantas` son pesos en el reparto, consolas al abrir y pestañas en la barra. `suyo` tiene **seis**
destinos: un índice, un área, un coste, un tamaño, un resultado y el peso de un grupo. Un solo destino
haría que el nombre **mintiera** en todos los sitios menos uno, que es exactamente lo que la entrega
anterior tuvo que arreglar cuatro veces.

**Así que la herramienta cambió y el método con ella.** El renombrador acepta ahora **un rango de líneas
por entrada**, y los rangos **no se escriben**: se derivan de las cabeceras `fn`, que son la unidad real
de ámbito de un local. Escribirlos a mano desde un informe ya salió mal —los números envejecen en cuanto
alguien edita el fichero— y entra un comprobador cuya única pregunta es la que importa: **¿queda alguna
aparición sin destino?** Un nombre que se queda en español porque su rango no lo cubría **compila igual
de bien**, y ni el compilador ni la guarda lo ven.

**Y la herramienta que mide este hito tenía un defecto, encontrado por una revisión adversaria y no por
mí.** El saneo que separa código de prosa **se comía los saltos de línea** de los comentarios y las
cadenas, así que **todas las líneas posteriores quedaban desplazadas**: 120 posiciones en este fichero.
Un rango calculado sobre eso renombra la cosa equivocada **en silencio**. Arreglado y comprobado en los
31 ficheros del árbol: los saltos que entran son los que salen. Y con él, un segundo punto ciego del
mismo escáner: su patrón de identificador era **ASCII**, así que partía en dos los cuatro nombres con
eñe y no los contaba. Comprobado después: los únicos identificadores no ASCII del árbol están en este
fichero, la medida y la sesión, así que **no escondió nada en los ocho módulos ya mezclados**.

**Del compilador salieron trece errores de 2.561 sustituciones, y los trece de la misma clase:** un
campo o un método que declara la medida, la sesión o los ajustes, renombrado en su sitio de llamada
aquí. Devueltos al nombre que su módulo aún les da. **La guarda cazó otras 22 citas muertas** que la
herramienta no mueve, porque viven en comentarios de línea y no en enlaces de documentación.

**Dos nombres nuevos mentían y se cambiaron a mano**, y los dos son la clase que ninguna guarda ve:
`absorb` tenía **dos `dentro` distintos en doce líneas** —el sentido de la rama y la lista de hijos—, que
un solo destino habría convertido en dos cosas con un nombre a la vista; y `subidas` era una lista de
desplazamientos en una función y las nietas que se suben en otra, donde `scrolled` era falso.

##### Lo que meter el fichero grande en la lista NO compra, medido

**Y aquí va la corrección más importante de esta entrega, porque la encontró una revisión y no yo.**
Meter `app/src/lib.rs` en la lista de ficheros «sin nombre que el glosario retirase» **no protege
ninguno de los 317 renombrados**. La razón está en el mecanismo: el vocabulario español lo **deriva** la
guarda de **las flechas de este anexo**, y **ninguno de los 317 nombres viejos es una flecha** —
son locales, campos y parámetros, y el glosario decide **tipos**. Probado mutando: devolver `mid_y` a
`alto` en los cuatro sitios de una función deja la guarda en **9 de 9, verde**.

Así que la afirmación honesta es exactamente ésta y ni una palabra más: **el fichero no conserva ningún
nombre que el glosario retirase**, y las 28 exenciones hacían falta porque esos 28 **sí** están en las
tablas del anexo. Un regreso a cualquiera de los 317 es **invisible** para la guarda, y decir lo
contrario sería la lista que parece vigilada y no lo está — el disfraz que este documento lleva
fichado desde la cuarta entrega.

**Lo que sí protegió esta entrega, y con números:** el oráculo posicional. 1.855 hunks sin uno
desequilibrado, 2.617 líneas emparejadas una a una, **3.208 sustituciones de identificador en 368 pares
distintos, el 99,5 % coincidiendo exactamente con los mapas**, cero cambios de operador o de
puntuación, cero literales numéricos y **76 cadenas que cambian, todas y solo en capturas de formato**
—sustituido cada `{nombre}` por un hueco, las 76 parejas salen byte a byte idénticas—. Eso es lo que hay
que repetir en las entregas que quedan, y no una lista.

**Y el lector de citas no aprende dos formas más, también medido.** Se le escaparon
`` `division.redimensionar()` `` y `` `rectangulos[0]` ``, y aceptar el acceso con punto y el índice con
corchetes da **45 avisos, de los que 26 son `panel.json`** y el resto `lib.rs`, `.ttc` y
`fs.inotify.max_user_instances`. Dos o tres reales contra cuarenta de ruido: **no entra**. Es la tercera
vez que se mide una ampliación de ese lector y la segunda que se rechaza, y las tres cifras quedan
escritas —4 contra 309, y ahora 45— porque el criterio es la proporción y no la ocurrencia.

##### La lista de exenciones, y el disfraz que se repitió por tercera vez

Meter el fichero grande en esa lista costó **28 exenciones**. Son campos y métodos que declaran la
medida, la sesión y los ajustes y que este fichero **solo usa**: la guarda juzga un fichero por los
identificadores que **aparecen** en él, no por los que declara. Y el número que importa es otro, medido
por una revisión con su propio analizador: **el fichero nombra 55 identificadores españoles ajenos**, y
solo 28 necesitan exención — a los otros 27 no los caza ninguna regla, porque el glosario decide tipos. Están verificadas una a
una contra el fichero que las declara, agrupadas por dueño, y **cada línea la borra la entrega de su
módulo** — la lista es el marcador de progreso que su propio comentario dice que es. Y **las 28 son
portantes, medido de una en una**: quitar cualquiera deja la suite en rojo, siempre por la misma prueba
y siempre señalando este fichero. Cero exenciones decorativas, que son las cuatro que este hito ya
borró.

##### Una clase nueva: el renombrado a mano que se come la prosa

**Y el defecto de verdad de esta entrega fue mío y no del mapa.** Renombrar un parámetro llamado `a` a
`to` sobre un rango de líneas alcanzó **la preposición española «a»** dentro de un literal de cadena y en
once comentarios. El literal es el que el informe imprime cuando la pestaña visible no tiene foco, así que
**salía en cada tanda de medida**: «el teclado no llegaba a ninguna consola» se había convertido en «no
llegaba **to** ninguna consola».

**No lo vio nada.** El guion del banco no tiene patrón para esa línea, y la única prueba que la toca
afirma sobre el prefijo «NO EXISTE», así que la cola corrupta pasaba. Lo encontró una revisión leyendo el
diff.

La clase es nueva y conviene nombrarla, porque la herramienta de este hito existe justamente para
evitarla: **la herramienta distingue código de prosa; un `re.sub` sobre un rango de líneas no.** Todos los
arreglos a mano de esta entrega —y son unos treinta— se hicieron así, y el único que tenía un nombre de
una letra es el único que se comió una palabra. La regla que queda: **un nombre de una o dos letras no se
renombra por rango de líneas**; o va por la herramienta que conoce las regiones, o se mira línea a línea.

**Y mi propia comprobación de eso no valía nada**, que es la mitad más útil del hallazgo: miré el diff
**después** de commitear, así que salió vacío. Dos veces seguidas. Un barrido que se hace sobre el estado
equivocado del árbol es peor que no hacerlo, porque publica un cero. El barrido va contra la rama entera
—`main...HEAD`— y **por el proxy de la herramienta que envuelve `git`**, porque esa envoltura **resume** la
salida del diff y eso es lo que devolvía cero comentarios cambiados cuando había sesenta y cuatro.

**Y una cosa que la entrega NO consiguió, dicha porque el borrador afirmaba lo contrario.** Al fichero
le quedaba **un identificador propio en español**: `en`, la preposición, en cuatro sitios que él mismo
declara —un ayudante de prueba con quince llamadas y tres parámetros de `Pos2`—, más un parámetro `a` en
otros dos. La guarda **no** los ve **por diseño**: el suelo de dos caracteres los tira antes de llegar a
ninguna regla, y eso está escrito en su cabecera desde la primera entrega. Ya estaban así en `main`, así
que no los introdujo este renombrado; los encontró una revisión a la que se le pidió **refutar** la
afirmación, y se arreglan aquí. La lección es la de siempre y van tres: **una afirmación se le da a
alguien para que la rompa, no para que la confirme.**

**Y añadirlas rompió dos casos de la prueba de reglas, que es el hallazgo de esta entrega.** La exención
de `pestañas` —que hace falta, porque la sesión declara ese campo— exentaba de paso el caso `pestanas`,
que es **el único sujeto posible** de la regla del plural con tilde. Y la exención de `gesto` apagaba el
caso que demuestra que la lista no puede exentar el local propio de un fichero. Las dos veces: **verde,
sin comprobar nada**.

Van tres —`color_del_reclamo` en una entrega, `pestanas` y `gesto` en ésta—, así que deja de ser un
arreglo y pasa a ser mecanismo:

1. **La exención compara el nombre entero y tal cual: ni la caja ni las tildes se doblan.** Y la
   distinción no es un parche: **un nombre con tilde y el mismo sin ella son dos identificadores**, y
   este repositorio escribe unos con y otros sin a propósito.
2. **Los pares tipo/local se derivan de la lista** en vez de escribirse, filtrados por lo que el glosario
   retiró de verdad, así que el conjunto se ajusta solo a medida que la lista crece.
3. **Cada caso positivo de esa prueba se comprueba contra la lista, y lo primero**, para que el mensaje
   diga la causa —«alguien exentó este caso»— y no el síntoma, que estaba treinta líneas más abajo.

Tres mutaciones lo atan: exentar un caso positivo, volver a doblar las tildes y volver a doblar la caja.
Las tres caen, y cada una nombra su propia causa.
