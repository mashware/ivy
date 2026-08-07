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
