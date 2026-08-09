# Panorama — quién hay y qué robarle

Agosto de 2026. Ordenado por categorías, con lo aprovechable de cada uno.
Los detalles de *cómo está construido* cada terminal están en [06-referencias.md](06-referencias.md);
esto es el mapa del territorio.

---

## A. Terminales con motor y UI propios

Máximo control y rendimiento; te toca dibujar cada botón.

| | Stack | Aprovechable |
|---|---|---|
| **Alacritty** | Rust, GPU | Su motor (`alacritty_terminal`) **es nuestra dependencia** |
| **WezTerm** | Rust, GPU | Multiplexor propio con sesiones desacoplables; config en Lua ejecutable; `wezterm cli` como superficie de control |
| **Kitty** | C + Python | Protocolo gráfico propio (imágenes en el terminal), API de scripting |
| **Ghostty** | Zig (`libghostty`) + UI nativa por plataforma | **El modelo de separación**: núcleo compartido, interfaz nativa en cada sistema |
| **Warp** | Rust + UI propia sobre `wgpu` | **Bloques de comando**: cada comando y su salida son un objeto que seleccionas, pliegas y compartes |
| **Rio**, **Contour**, **Foot** | Rust / C++ / C | Nicho; poco que copiar para este caso |

---

## B. Terminales sobre un toolkit

Heredan botones, menús y arrastrar-y-soltar. A cambio, atados a una plataforma.

| | Stack | Aprovechable |
|---|---|---|
| **Tilix** | VTE + GTK3 | **El modelo de divisiones que queremos**: árbol de paneles, arrastrar terminales para recolocar, sesiones guardables |
| **Ptyxis** | VTE (GTK4, GPU) + libadwaita | Integración por contenedor (Podman/Toolbx/Distrobox) como concepto de "entorno por pestaña" |
| **Konsole** | Qt | Perfiles y divisiones maduras |

---

## C. Terminales-espacio de trabajo

Aquí el terminal deja de ser una rejilla de caracteres y pasa a ser un lienzo. **La categoría
más cercana a lo que queremos.**

| | Stack | Aprovechable |
|---|---|---|
| **Wave Terminal** | Electron | **Bloques**: la ventana se compone de bloques (terminal, editor, previsualización de Markdown/JSON/imagen) y las pestañas se agrupan en *workspaces* desde una barra lateral. Es nuestra estructura con otro nombre |
| **Extraterm** | Electron | **Lo más interesante de toda la lista** (ver abajo) |
| **Tabby** | Electron | Gestor de SSH integrado, ecosistema de plugins, buen soporte en Windows |
| **Hyper** | Electron | Extensiones triviales de escribir; lento y pesado — el aviso de por dónde no ir |
| **VS Code** | Electron + xterm.js | Terminal integrado con divisiones y pestañas a escala de millones de usuarios |

### Extraterm merece párrafo aparte

Envuelve **cada comando en un marco** con su salida dentro, coloreado según el resultado: azul
si funcionó, rojo si falló. Y sobre esos marcos construye cosas que ningún otro terminal tiene:

- La salida de un comando es **editable en el sitio**
- El comando `from` **reutiliza la salida de un comando anterior como entrada** de otro
- Modo cursor para navegar el scrollback como un editor de texto
- `show` para ver ficheros, imágenes y código resaltado dentro del terminal
- Subida y descarga de ficheros que funciona **incluso a través de SSH**

Eso es OSC 133 llevado hasta el final. Y es directamente aplicable: si la app sabe dónde
empieza y acaba cada comando, el panel puede ofrecer «manda esta salida al agente» o
«reintenta este comando» sin que nadie copie y pegue nada.

---

## D. Multiplexores

| | Aprovechable |
|---|---|
| **tmux** | El sustrato sobre el que está construido medio espacio de orquestación (categoría E). Sesiones persistentes, `send-keys`, `capture-pane` |
| **Zellij** | Rust; **su propia interfaz está hecha de plugins WASM**, barra clicable, paneles flotantes movibles con el ratón, arrastrar bordes para redimensionar |

---

## E. Orquestadores de agentes — el espacio nuevo

Nacido en 2025-2026. **Todos comparten el mismo patrón: un git worktree por agente, un panel
para ver qué hace cada uno, y revisión de diffs antes de fusionar.**

### E1 — Los que esconden el terminal

Presentan tablero o dashboard; la consola queda debajo.

| | Qué es | Nota |
|---|---|---|
| **Conductor** | App de escritorio: agentes Claude Code / Codex / Cursor en paralelo, cada uno en su worktree, con panel visual y revisión de diffs | **Solo macOS y propietario** |
| **Crystal → Nimbalyst** | Electron; varias sesiones de Claude Code en worktrees, seguimiento de conversación, operaciones git integradas | Crystal es el nombre antiguo |
| **Vibe Kanban** | CLI + web, tablero kanban; cada tarea en su worktree; soporta Claude Code, Codex, Amp, Cursor CLI, Gemini | **La empresa detrás cerró en abril de 2026**; sigue como open source comunitario |
| **Cline Kanban** | Servidor local + tablero en el navegador; cada tarjeta es una tarea con su worktree efímero y su terminal. Agnóstico de agente (Claude Code, Codex, opencode, Cline CLI). Cadenas de dependencias, cron, hooks de aprobación, headless en CI | **El más cercano de toda la lista.** Ver abajo |

### Cline merece párrafo aparte

Son dos productos y solo uno nos toca. **Cline IDE** es un agente dentro del editor: no compite,
compite con Claude Code. Es un *productor* potencial de `panel.json` — y el ejemplo de siempre de
panel cerrado, porque su estado solo lo sabe Cline.

**Cline Kanban** sí es competencia, y llegó a dos de nuestras conclusiones por su cuenta:

- **Diagnostica lo mismo**: que el cuello de botella ya no es el agente sino el humano que no
  sabe por cuál de sus ocho terminales va cada cosa.
- **Confirma D4 desde fuera**: construyeron la orquestación lanzando los binarios oficiales de
  cada agente en un proceso, no llamando a modelos por API. Es la única vía que preserva la
  suscripción, y la tomó una empresa con dinero encima. *(Pendiente de verificar en la política
  oficial: en abril de 2026 las cuotas de suscripción habrían dejado de cubrir herramientas de
  terceros. Si es cierto, refuerza D4 y deja fuera a Cline IDE.)*

Y dejan el hueco por escrito: *«We're not trying to replace your terminal.»* Son una capa encima.
Nosotros somos la capa de abajo. Las tres diferencias que quedan, y que son la razón de ser del
proyecto:

1. **Su terminal está dentro del navegador.** El tablero es un servidor local que abres en una
   pestaña. Sea cual sea el emulador que embeban, no es una consola con divisiones, ratón y
   rendimiento medido: es un visor dentro de una tarjeta.
2. **El estado lo define Cline, no el que trabaja.** Muestran «estado de la tarjeta + última
   salida del agente»: un vocabulario cerrado, uno por runtime soportado. Un CI, un despliegue o
   un script tuyo no tiene dónde publicar «me quedan 12k de contexto» o «estoy bloqueado
   esperando aprobación» — justo lo que no se deduce desde fuera (D6).
3. **Es tarea-céntrico, no sesión-céntrico.** El modelo es «suelta la tarjeta, dale al play». No
   cabe ahí una sesión larga de exploración, ni un shell normal al lado de un agente.

**El riesgo, dicho claro:** si Cline publica un formato abierto para que cualquier proceso escriba
en su tarjeta, se lleva nuestro diferenciador de contrato en una release. Lo que no pueden copiar
barato es el terminal nativo de debajo: el navegador es una decisión estructural suya.

### E2 — Los que mantienen el terminal en el centro

Interfaz de texto sobre tmux. **Más cerca de nuestra idea**, pero heredan los límites de tmux.

| | Qué es |
|---|---|
| **Claude Squad** | TUI para varias sesiones de agente en paralelo, sobre tmux + worktrees. Claude Code, Codex, opencode, Amp |
| **amux** (dos proyectos distintos con el mismo nombre) | Uno es un multiplexor de agentes sobre tmux; el otro, un plano de control con dashboard web y móvil para decenas de sesiones |
| **dmux** | Agentes en paralelo con tmux + worktrees, con hooks en todo el ciclo de vida del worktree |

---

## Lo que este panorama dice del proyecto

**1. El patrón «un worktree por agente» ya está estandarizado.** Todo el mundo lo hace, y `flow`
ya lo hace. No hay que inventarlo: hay que apoyarse en él. La identidad de división (D7) encaja
exactamente ahí — un worktree, una división, un `PANEL_ID`.

**2. Nadie ocupa nuestra casilla.** Los de E1 esconden el terminal; los de E2 lo mantienen pero
lo construyen *sobre tmux*, heredando lo que tmux no da (ratón decente, botones, arrastrar
paneles). Ninguno es el anfitrión que hospeda agentes **como terminal de primera clase**, ni
define un contrato abierto para que cualquiera publique estado.

**3. El hueco de Conductor sigue ahí, pero más estrecho.** Era el más pulido del grupo y es
**macOS y propietario**, así que multiplataforma y abierto era un espacio libre. Cline Kanban lo
ha tapado en parte: es abierto, multiplataforma y agnóstico de agente. Lo que queda sin ocupar no
es «abierto y multiplataforma» — es **el terminal como ciudadano de primera**.

**4. Aviso serio: el espacio está lleno y con rotación.** Hay comparativas de doce herramientas,
Crystal ha tenido que renombrarse, y la empresa de Vibe Kanban cerró. Que haya demanda no
significa que sea fácil monetizar. Entra sabiéndolo: para uso propio esto es inversión segura;
como producto, la diferenciación tiene que ser evidente en treinta segundos.

**5. La diferenciación en una frase.** «El anfitrión donde cualquiera publica estado» ya no se
distingue en treinta segundos de «tablero agnóstico de agentes» — Cline Kanban dice algo
parecido. La distinción que sí aguanta es más dura:

**Ellos organizan tareas por encima de tu terminal. Nosotros somos el terminal — y el estado lo
escribe quien hace el trabajo, no nosotros.**

Debajo de esa frase siguen valiendo las tres patas de siempre: contrato abierto para cualquier
productor, terminal de primera clase, y sin pagar API sobre lo que ya pagas por suscripción.

---

## Cosas concretas para robar

| De | Qué |
|---|---|
| **Extraterm** | Marcos por comando, coloreados por resultado; reutilizar la salida de un comando como entrada |
| **Warp** | Bloques de comando plegables y compartibles |
| **Wave** | Composición por bloques heterogéneos (terminal, editor, previsualización) y workspaces en barra lateral |
| **Tilix** | Árbol de divisiones y arrastrar terminales para recolocarlos |
| **Zellij** | Interfaz hecha de plugins; paneles flotantes movibles con el ratón |
| **WezTerm** | `wezterm cli` como superficie de control externa; sesiones desacoplables |
| **Ghostty** | Separar núcleo de interfaz para poder ir a Mac y Windows sin rehacer el motor |
| **opencode** | Bloque de contexto y coste; indicador de sesión ocupada en otro sitio |
| **Conductor / Vibe Kanban** | Revisión de diffs antes de fusionar como parte del flujo, no como paso aparte |
| **Cline Kanban** | Cadenas de dependencias entre tareas: al terminar una arranca sola la siguiente, sin escribir un script (ver «Ideas sin fase» en [05-roadmap.md](05-roadmap.md)) |
