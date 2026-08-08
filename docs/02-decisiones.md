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
