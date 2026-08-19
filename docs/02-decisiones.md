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
`cargo run --release --example barrido -p app`, y llama al pintor de verdad, no a una copia.

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

---

## D5 — Tres canales, no uno

**Decidido.** Ver [03-arquitectura.md](03-arquitectura.md).

| Dirección | Canal | Para qué |
|---|---|---|
| Agente → app | **MCP** | Crear divisiones, publicar estado, leer salida |
| Productor → app | **`panel.json`** | Estado que publica cualquiera, agente o no |
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
esos números. Lo que sigue sin medirse es **el coste del propio gesto**: cuánto cuesta un fotograma
mientras alguien arrastra la frontera del panel, y si rehacer la maqueta al cruzar cada celda se nota. Esa
tanda es lo que cierra F2.4.

## D12 — El pseudoterminal se queda con el procesamiento de salida del núcleo, aunque cueste

**Decidido.** El pseudoterminal **no** toca `termios`: `OPOST` se queda encendido y el núcleo sigue
convirtiendo cada salto de línea en retorno de carro más salto. Se paga el coste. **Esta decisión se
tomó al revés primero, se entregó, y se revirtió al descubrir por qué no puede funcionar** — lo que
sigue es esa autopsia, que vale más que la decisión.

**Lo que cuesta, medido en este repo** con `cargo run --release --example entrega -p term-core`,
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
   **en fila** cada consola publica **20 columnas** de rejilla —exactamente `panel::CONSOLA_MINIMA`—, y en
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
