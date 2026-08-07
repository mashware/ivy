# Referencias

Lo investigado, con lo que aporta cada caso. Datos a agosto de 2026.

---

## Cómo está construido cada terminal

| Proyecto | Motor | Interfaz | Lección |
|---|---|---|---|
| **Warp** | Propio, Rust | Framework de UI **propio** sobre `wgpu` | Escribieron hasta su sistema de botones. Publicaron [«Why is building a UI in Rust so hard?»](https://www.warp.dev/blog/why-is-building-a-ui-in-rust-so-hard). Pudieron por ser empresa con inversión |
| **Zed** | **`alacritty_terminal`** (crate) | GPUI propio | **El patrón que copiamos**: motor como librería, pintado propio |
| **Ghostty** | `libghostty` propio, Zig | **GTK4** en Linux, Swift/AppKit en Mac | El proyecto más celebrado eligió toolkit nativo por plataforma, no UI propia |
| **PhpStorm / IntelliJ** | **[JediTerm](https://github.com/JetBrains/jediterm)** propio, Java | Swing | JetBrains, con cientos de ingenieros, también escribió el suyo: no había uno multiplataforma que valiera |
| **VS Code** | xterm.js | Electron | El modelo web funciona a escala… con uno o dos terminales activos |
| **Tilix** | VTE | GTK3 | Una persona logró botones y arrastrar paneles **porque no escribió ninguna de las dos piezas** |
| **Ptyxis** | VTE (GTK4, GPU) | GTK4 + libadwaita | Sucesor de GNOME Terminal; Christian Hergert duplicó el rendimiento de VTE antes de construirlo |

**Conclusión:** lo escaso no es el framework de UI multiplataforma — hay de sobra. Es el
**widget de terminal multiplataforma**. Por eso todo el mundo acaba escribiendo el suyo… salvo
Zed, que lo consume como crate. Ver [02-decisiones.md](02-decisiones.md) → D2.

---

## opencode — la referencia más cercana

**Arquitectura:** cliente-servidor. Backend TypeScript/Bun que expone **HTTP + Server-Sent
Events**; encima, varios clientes: TUI (OpenTUI, antes Go/Bubble Tea), app de escritorio Tauri,
extensiones de IDE.

**Su barra lateral derecha** muestra, de arriba abajo:

- Título de la tarea
- **Contexto**: `35.392 tokens · 18% used · $0.62 spent`
- **MCP**: servidores conectados
- **LSP**: analizadores activos
- **Todo**: el plan con `[✓]` / `[ ]` y el paso actual resaltado
- Directorio de trabajo

Más un panel de **SESIONES**: ancladas (curadas por el usuario) y efímeras, incluyendo cualquiera
que esté **ocupada o reintentando en otro sitio**.

**Qué copiamos:** el bloque de contexto y coste (lo único que solo el agente sabe), el indicador
de actividad, la distinción anclado/efímero, y que el panel sea plegable desde el principio.

**Qué no:** duplica el plan en el flujo de mensajes *y* en la barra lateral. El panel existe
precisamente para que el estado no se repita en la conversación.

**Dónde se diferencia — y por qué es nuestro hueco:** su barra lateral solo sabe de opencode.
Y sus peticiones abiertas más votadas son [panel colapsable con multiventana](https://github.com/anomalyco/opencode/issues/19388)
y [pestañas de sesión ancladas](https://github.com/anomalyco/opencode/issues/24451) — la gente
le está pidiendo a un agente que se convierta en gestor de ventanas, porque no existe el
anfitrión que debería hacerlo.

---

## Límites de xterm.js

Del issue [«Support dozens of terminals on a single page»](https://github.com/xtermjs/xterm.js/issues/4379):

- Los navegadores limitan los contextos WebGL activos; **los multiplexores necesitan decenas de
  paneles y el navegador no lo soporta**
- Los terminales compiten por el hilo principal: **2 instancias → 30 fps, 4 → 15 fps**

Aplica solo con varios terminales **produciendo salida a la vez** — que es exactamente nuestro
caso. Base de [02-decisiones.md](02-decisiones.md) → D3.

---

## Suscripciones y facturación

- [Uso de Claude Code con Pro o Max](https://support.claude.com/en/articles/11145838-use-claude-code-with-your-pro-or-max-plan) — si `ANTHROPIC_API_KEY` está definida, tiene prioridad y factura a esa cuenta
- [claude-code#43333](https://github.com/anthropics/claude-code/issues/43333) — `claude -p` con OAuth facturando como API
- [claude-code#37686](https://github.com/anthropics/claude-code/issues/37686) — suscriptor Max con 1.800 $ en dos días

Base de [02-decisiones.md](02-decisiones.md) → D4.

---

## Sobre WezTerm (la base del prototipo)

- Última **estable**: febrero de 2024. El desarrollo sigue vivo, pero solo por nightlies diarias
- **No dibuja decoraciones en Wayland** y GNOME tampoco lo hace por él
  ([wezterm#4962](https://github.com/wezterm/wezterm/issues/4962)) → se ejecuta bajo XWayland
- No hay botones ni menús propios: [wezterm#1485](https://github.com/wezterm/wezterm/issues/1485)
  lleva pedido desde 2022
- El arrastre del divisor de paneles tiene [fallos conocidos](https://github.com/wezterm/wezterm/discussions/4399)

Sirve perfectamente como base del prototipo del panel. No sirve como producto.

---

## Tilix (el punto de partida)

En **modo mantenimiento declarado**: sin funcionalidades nuevas y buscando maintainers. El
problema de fondo no es el abandono, son dos decisiones de 2016: **GTK3** (también en
mantenimiento) y **D** con `gtkd`, que es la razón real de que nadie recoja el testigo.
