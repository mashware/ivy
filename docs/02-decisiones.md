# Decisiones

Cada una con su razón y lo que se descartó. Si una decisión cambia, se edita aquí y se dice por qué.

---

## D1 — Multiplataforma real (Linux, macOS, Windows)

**Decidido.** No es una app personal para Linux: hay demasiada gente en Mac y Windows como
para descartarla.

**Consecuencia:** descarta GTK4 + VTE, que era la opción técnicamente más cómoda en Linux
(ver D3).

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

---

## D4 — Control por consola, no por API

**Decidido.** El agente se controla escribiendo en su PTY y leyendo su salida, no llamando a
la API del proveedor.

**Por qué — es económico, no técnico.** Las suscripciones (Claude Max, etc.) solo valen dentro
del cliente oficial de cada proveedor. Llamar a la API se factura por tokens aparte. Una
herramienta que obligue a pagar API por lo que el usuario ya paga por suscripción no se adopta.

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
