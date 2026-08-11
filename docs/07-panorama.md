# Panorama — quién hay y qué robarle

Agosto de 2026. Ordenado por categorías, con lo aprovechable de cada uno.
Los detalles de *cómo está construido* cada terminal están en [06-referencias.md](06-referencias.md);
esto es el mapa del territorio.

**Si solo vienes a por una cosa, ve a [«En qué nos diferenciamos»](#en-qué-nos-diferenciamos--agosto-2026):**
el resumen de los seis análisis de este mes, con lo que ya no nos diferencia y las tres cosas que
quedan.

---

## A. Terminales con motor y UI propios

Máximo control y rendimiento; te toca dibujar cada botón.

| | Stack | Aprovechable |
|---|---|---|
| **Alacritty** | Rust, GPU | Su motor (`alacritty_terminal`) **es nuestra dependencia** |
| **WezTerm** | Rust, GPU | Multiplexor propio con sesiones desacoplables; config en Lua ejecutable; `wezterm cli` como superficie de control |
| **Kitty** | C + Python | Protocolo gráfico propio (imágenes en el terminal), API de scripting |
| **Ghostty** | Zig (`libghostty`) + UI nativa por plataforma | **El modelo de separación**: núcleo compartido, interfaz nativa en cada sistema |
| **Warp** | Rust + UI propia sobre `wgpu` | **Bloques de comando**: cada comando y su salida son un objeto que seleccionas, pliegas y compartes. **Desde 2026 es también orquestador de agentes y open source — ver E5, es el competidor más serio de la lista** |
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
| **Conductor** | App de escritorio: agentes Claude Code / Codex / Cursor en paralelo, cada uno en su worktree, con panel visual y revisión de diffs | **Solo macOS y propietario.** Ver abajo |
| **Crystal → Nimbalyst** | Electron; varias sesiones de Claude Code en worktrees, seguimiento de conversación, operaciones git integradas | Crystal es el nombre antiguo |
| **Vibe Kanban** | CLI + web, tablero kanban; cada tarea en su worktree; soporta Claude Code, Codex, Amp, Cursor CLI, Gemini | **La empresa detrás cerró en abril de 2026**; sigue como open source comunitario |
| **Cline Kanban** | Servidor local + tablero en el navegador; cada tarjeta es una tarea con su worktree efímero y su terminal. Agnóstico de agente (Claude Code, Codex, opencode, Cline CLI). Cadenas de dependencias, cron, hooks de aprobación, headless en CI | **El más cercano de toda la lista.** Ver abajo |

### Conductor merece párrafo aparte, porque es el que obliga a afinar la tesis

Tres paneles: barra lateral de workspaces (cada uno con nombre de ciudad), **chat en el centro**, y a
la derecha el diff en vivo más el terminal integrado. El chat no es el TUI real del agente: es
interfaz propia, así que consumen el **stream** del harness en vez de hospedar su pintado. De ahí que
esté en E1 — el terminal es un panel secundario.

**Lo que enseña, y por qué importa.** Según reseñas de terceros muestra **recuento de tokens y coste
debajo de cada respuesta, y un aviso cuando el agente se acerca al límite de contexto**. *(No
verificado: no aparece en su documentación oficial, sale de Grokipedia y reseñas, y es macOS, así que
tampoco se ha podido comprobar ejecutándolo.)* Si es cierto, es el único de la lista que enseña esos
datos concretos.

**Y aun así no es el panel que queremos, y aquí está la distinción que hay que tener clara.** El eje
no es **cuánto** dato se enseña, es **quién decide qué se ve**. Conductor elige tres métricas y las
pinta; aunque eligiera cuarenta, las seguiría eligiendo él. Nuestro panel es texto libre que escribe
el productor: en qué paso va la tarea, qué la tiene bloqueada, y **lo que al usuario de esa división
le dé la gana poner** — pidiéndoselo a su propio agente, sin que nadie haya tenido que diseñar antes
ese campo. Los tokens y el coste son un **extra** del contrato, no su razón de ser.

Las tres consecuencias de haberlo construido leyendo el stream:

1. **Solo vale para Claude Code, Codex y Cursor.** Un CI, un despliegue o un script propio no
   aparecen en ningún stream.
2. **No hay ninguna vía de vuelta.** Su referencia de scripts no documenta **ningún** mecanismo para
   que un proceso del workspace informe a la interfaz: ni fichero, ni comando, ni endpoint. Es el
   único de los tres recién analizados que no tiene ni el equivalente a `superset-notify`.
3. **No expone MCP propio:** *«Conductor does not define a separate MCP config format. It uses the
   MCP configuration that Claude Code and Codex load for the session.»* Solo pasa la configuración
   del agente, así que no existe el canal agente → app de **D5**.

**Sus hooks son de worktree, no de agente** — `pre-setup`, `post-setup`, `pre-run`, `post-run`,
`pre-archive`, `post-archive`, en `.conductor/settings.toml`. Preparar, arrancar y limpiar; nadie
publica estado ahí. E inyectan `CONDUCTOR_WORKSPACE_PATH`, `CONDUCTOR_ROOT_PATH`,
`CONDUCTOR_WORKSPACE_NAME` y `CONDUCTOR_PORT`: hay identidad por división, pero para nombrar recursos
y repartir puertos, no para publicar nada. **D7** confirmado en la forma, no en el propósito.

### E5 — Warp: el que cumple las cinco condiciones

Estaba en la categoría A por sus bloques de comando, y en 2026 se mudó de categoría con dos cambios:

- **14 de abril de 2026** — soporte universal de agentes: Claude Code, Codex, Gemini CLI y opencode
  integrados **como ciudadanos de primera clase**, no como procesos de shell cualesquiera.
- **30 de abril de 2026** — **open source**. 64.132 estrellas y movimiento diario.

Y tiene un **Agent Management Panel**: el estado de todos los agentes de todas las sesiones, con cinco
valores (`Working`, `Blocked`, `Canceled`, `Failed/Errored`, `Success`), cancelar en marcha, ver
cuáles esperan entrada o han fallado, y **saltar al pane asociado**.

**Lo que el punto 3 de las conclusiones llamaba «la casilla libre» era: abierto + multiplataforma +
el terminal como ciudadano de primera. Warp cumple las tres, y es el único de la lista que lo hace:**

| | |
|---|---|
| Terminal nativo propio, GPU, no navegador | ✅ |
| macOS, Linux y Windows — nuestro **D1** | ✅ |
| Hospeda Claude Code, Codex, Gemini CLI, opencode | ✅ |
| Panel con el estado de todos y salto a la división | ✅ |
| Open source | ✅ |

**Lo que sigue faltándoles, verificado en su código y no en su web:**

1. **La lista de productores es un `enum` compilado en la app.** Hay un `trait ThirdPartyHarness` con
   una implementación por agente —`harness/claude_code.rs`, `harness/codex.rs`,
   `harness/gemini.rs`— y un `enum Harness` cerrado. Mismo patrón que Superset, pero dentro del
   binario: un CI, un despliegue o un script propio no entran sin un *pull request* a Warp.
2. **Cinco estados y ninguna vía de entrada.** Su documentación no describe fichero, comando ni
   endpoint por el que un proceso arbitrario publique en ese panel.
3. **Es un panel de gestión, no un panel por división.** Vive arriba a la derecha y es una lista de
   sesiones con su semáforo, para decidir a cuál vas. No es un lienzo por división cuyo contenido
   escriba quien trabaja.

**Warp Agent CLI es otro producto, y no compite con nosotros.** Es el agente propio de Warp extraído
a un binario que corre **en cualquier terminal**, incluso por SSH sin instalar nada en el remoto, con
su propio multiplexor de PTYs «parecido a tmux», enrutado automático de modelos y subagentes locales
y en la nube. Se factura **inferencia a coste de API, sin margen**. Compite con Claude Code — y es un
**productor potencial de `panel.json`**, igual que Cline IDE.

**La oportunidad que trae, y que toca D3.** Lo liberado no es todo igual; textual de su README:

> Warp's UI framework (the `warpui_core` and `warpui` crates) are licensed under the **MIT license**.
> The rest of the code in this repository is licensed under the **AGPL v3**.

Verificado en el `Cargo.toml` de los dos crates: `license = "MIT"`. Existe por tanto **un framework de
UI en Rust, con GPU, licencia permisiva y probado en un terminal multiplataforma de verdad**. Es un
tercer candidato para [02-decisiones.md](02-decisiones.md) → D3, anotado allí con sus dos cautelas:
no está verificado que `warpui` sea usable de forma independiente —puede arrastrar dependencias de
crates AGPL del mismo repositorio—, y del resto del código no se copia nada a un proyecto que no sea
AGPL.

**El riesgo, y es el más alto de todo este panorama.** Con Cline y con Xirp la frase era «si publican
un formato abierto, se llevan el diferenciador». Con Warp es peor por dos motivos: **ya tienen todo lo
demás**, y **ya son open source con 64k estrellas**, así que no hace falta que la idea se les ocurra a
ellos — se la puede proponer cualquiera en un *pull request*. Lo que nos queda es una sola cosa, y
conviene tenerla clarísima: **que el contenido del panel de cada división lo decida quien trabaja, y
que quien trabaja pueda ser cualquiera.**

### E6 — cmux: el único con el canal abierto, y el que enseña dónde está el límite

Terminal nativo de macOS en **Swift + AppKit**, usando **`libghostty` como librería** — no un fork.
Es nuestro **D2** aplicado y el modelo de separación de Ghostty que la categoría A ya elogiaba.
**GPL-3.0-or-later**, gratis, solo macOS (iOS en TestFlight). Pestañas verticales, divisiones en las
dos direcciones, navegador integrado, y una barra lateral que muestra por pestaña la rama de git, el
PR enlazado, el directorio, **los puertos a la escucha** y el texto de la última notificación.

**Dos cosas lo separan de todo lo demás de esta categoría:**

1. **No tiene adaptadores.** *«cmux es un terminal, así que cualquier agente que corra en un terminal
   funciona»* — Claude Code, Codex, opencode, Gemini CLI, Kiro, Aider, Goose, Amp, Cline, Cursor
   Agent. Ni el `enum Harness` de Warp ni los diez wrappers de Superset. Y cuando un agente lanza
   subagentes, los convierte en **panes y divisiones nativas** en vez de procesos ocultos.
2. **Tiene canal abierto de verdad.** Las notificaciones entran por **OSC 777** (título y cuerpo),
   **OSC 99** (el de Kitty: además subtítulo e identificadores), el comando `cmux notify` o hooks de
   agente. **Cualquier proceso puede emitirlas**: un CI, un script, un cron. Es lo más cerca que
   nadie ha estado de este contrato.

Y su **socket API** hace lo que nuestro canal MCP: crear workspaces, abrir divisiones, mandar
entrada, **leer el contenido de la pantalla**, capturas y pilotar el navegador.

**Por qué aun así no es este panel — y el límite es estructural, no de madurez:**

- **El payload es una línea de texto, y es efímero.** Título, subtítulo, cuerpo. Sin progreso, sin
  pasos, sin listas, sin bloqueos. Y el ciclo de vida es *recibida → sin leer → leída → borrada*:
  **desaparece al leerla**. La barra lateral guarda «la última notificación», una línea. Eso contesta
  «pasó algo», no «por dónde va esto»; a las dos horas no contesta nada.
- **OSC viaja dentro del flujo del terminal.** Solo lo emite lo que esté dentro de ese PTY, mezclado
  con la salida del programa. `panel.json` es **fuera de banda**: lo escribe un proceso que no vive
  en esa división —un CI en otra máquina, un cron, un servicio— sin ensuciar el scrollback ni competir
  con el pintado de una TUI.
- **Una notificación no puede decir «sigo vivo».** El `updated_at` con `stale_after_minutes` delata
  al productor que murió a mitad; OSC no tiene dónde poner eso.

**Lo que hay que robarles, y es de las mejores ideas del panorama: entender OSC 777 y 99 además del
contrato propio.** Son estándares que ya emiten Kitty, rxvt y media docena de herramientas, así que
soportarlos da compatibilidad gratis con todo lo que hoy notifica, sin pedirle a nadie que adopte
`panel.json`. El contrato para quien quiera un panel; los OSC para que el resto funcione sin cambiar
nada.

### Los cinco recién analizados, en fila

| | Xirp (E3) | Superset (E4) | Conductor | **Warp (E5)** | **cmux (E6)** |
|---|---|---|---|---|---|
| El terminal es | de primera clase | dentro del navegador | un panel secundario | **de primera clase** | **de primera clase** |
| Plataformas | macOS | macOS + AppImage | macOS | **las tres** | macOS |
| Abierto | no | ELv2 | no | **AGPL + UI en MIT** | **GPL-3.0+** |
| Agentes que acepta | 3, con adaptador | 10, con adaptador | 3, con adaptador | 5, `enum` compilado | **cualquiera, sin adaptador** |
| Quién decide qué se ve | la app (3 constantes) | la app (4 eventos) | la app (sus métricas) | la app (5 estados) | **el productor (texto libre)** |
| Vía para que publique un tercero | hooks del harness | hooks + `superset-notify` | **ninguna** | **ninguna** | **OSC 777/99, `cmux notify`** |
| Canal para pilotar la app | MCP | MCP | **no** | sin verificar | socket API |
| Lo publicado es | un estado | un evento | métricas | un estado | **una notificación de una línea** |
| Persiste cuando vuelves | como estado | no | sí | como estado | **no: se borra al leerla** |
| **Estado estructurado que elige quien trabaja** | **no** | **no** | **no** | **no** | **no** |

La última fila es el proyecto, y las dos de encima explican por qué. Los cuatro primeros tienen estado
pero con vocabulario que elige la app; **cmux tiene el canal abierto de verdad y por eso enseña dónde
está el límite**: lo que cabe por ahí es un aviso de una línea que se borra al leerlo. Nadie tiene lo
de en medio — un cuerpo estructurado, persistente y escrito en el vocabulario del productor
(**D6**).

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

### E3 — Xirp (Spotify): la casilla de al lado

Beta pública el **10 de agosto de 2026**, salido de **+36.000 sesiones internas** y miles de
ingenieros. App de escritorio con **sesiones de terminal persistentes** —se cambia de una a otra sin
perder estado—, vista de rejilla, un worktree por tarea, y Claude Code, Codex y Gemini CLI
autenticados **por su CLI nativa**. No es E1, porque el terminal no está escondido; no es E2, porque
no es una TUI sobre tmux. **Es lo más cerca que nadie ha estado de nuestra casilla**, y conviene
decirlo sin adornos.

Confirma tres decisiones de golpe:

| | |
|---|---|
| **D4** | Autentica por la CLI oficial de cada agente, no por API — con factura de empresa detrás, que es donde pagar API aparte habría costado menos justificar |
| **D5** | MCP como canal para meterle contexto a la sesión: *«A session launched from a catalog entity or Portal Workspace can receive relevant context through MCP»* |
| **D6** | El estado se lo declaran por hooks del harness; no lo deducen del pintado |

Y lo que **no** hace, que es donde seguimos solos:

1. **Tres estados y se acabó.** `Working`, `Idle`, `Waiting for input`, más el nombre de la rama. Ni
   el paso de la tarea, ni el bloqueo, ni nada que el productor decida poner por su cuenta. Es el
   nivel 2 del anexo de D6, y no puede crecer al 3 con más hooks: el harness no conoce esa semántica.
2. **No hay contrato.** El productor de estado tiene que ser un harness que Xirp ya soporte. Un CI,
   un despliegue o un script propio no tienen dónde escribir.
3. **macOS solo, cerrado y con cuenta de Spotify obligatoria.**
4. **Su eje es otro.** El valor gordo es el *contexto organizativo* de Portal/Backstage —catálogo de
   servicios, ownership, decisiones, documentación autogenerada que realimenta sesiones futuras—.
   Es una jugada de plataforma de empresa, y buena parte se apaga sin Portal.

**El riesgo, y es el más serio de toda esta lista.** Spotify ya hizo esta jugada exacta: abrió
Backstage en 2020 y se convirtió en el estándar de facto de los portales de desarrollo. Si repiten
—abrir Xirp y publicar un formato para que cualquier proceso publique estado— se llevan el
diferenciador de contrato **y** además tienen el terminal nativo, que es lo que Cline no puede
copiar barato. Lo que hoy nos deja sitio no es tecnología: es que su producto está atado a Portal y
a macOS, y que su modelo de estado son tres constantes.

**Fiabilidad de esta ficha.** Los tres estados y los hooks están en sus docs
([getting-started](https://backstage.spotify.com/docs/xirp/getting-started)); el detalle de interfaz
viene de prensa secundaria y de documentación de beta escueta. **Nadie del proyecto ha ejecutado la
app** — es macOS y pide cuenta. Si alguien puede instalarla, el dato que hay que traer es si existe
algún campo de estado más allá de esos tres.

### E4 — Superset: el producto más completo, sobre el suelo que descartamos

App de escritorio, **Electron + React + `@xterm/xterm`**, source-available bajo **Elastic License
2.0** y gratis. Es el más avanzado en producto de toda esta lista: worktree por tarea, revisión de
diffs con comentarios y edición en línea, terminales persistentes que sobreviven al reinicio,
divisiones y pestañas, navegador integrado con detección de puertos **por workspace**, agentes
lanzados **en cron**, servidor MCP para pilotarlo desde otro agente, y workspaces remotos. Soporta
diez harnesses: Claude Code, Codex, opencode, Cursor Agent, Copilot, Grok, Amp, Gemini, Pi y Mistral
Vibe.

**Confirma cuatro decisiones, y una de ellas al pie de la letra.** Todo por CLI del agente sin tocar
API (**D4**); servidor MCP como canal agente → app (**D5**); mecanismo para que el agente declare
estado en vez de adivinarlo (**D6**); y **inyecta `SUPERSET_TERMINAL_ID` en el entorno del terminal**,
con los hooks saliendo antes si no está — que es **D7** con otro nombre y la misma función.

**Dónde no llega:**

1. **Cuatro eventos, y están en su código:** `eventType: "Start" | "Stop" | "PermissionRequest" |
   "PendingQuestion"`. El payload que viaja es `{terminalId, eventType, agent:{agentId, sessionId}}`.
   No hay hueco para el paso de la tarea, el bloqueo, ni nada que el productor quiera añadir.
2. **No hay panel, hay semáforo con campanita.** Indicador naranja en la barra lateral, distintivo en
   el dock, sonido al terminar y notificación del sistema. Resuelve «a qué división voy», no «por
   dónde va». Lo más cerca de un contrato es `superset-notify review "…"`: un estado de cuatro más una
   frase libre, que viaja como texto de notificación y no como contenido de la división.
3. **Un adaptador por harness, no un contrato.** Instalan un wrapper distinto por agente —
   `~/.cursor/hooks.json`, un plugin en `~/.config/opencode/plugin/`, un
   `.github/hooks/superset-notify.json` dentro del worktree que además añaden a `.git/info/exclude`.
   Diez agentes, diez integraciones que mantener; y un CI o un script propio no entra por esa puerta.
   Ese mantenimiento es exactamente lo que un contrato abierto se ahorra.
4. **El suelo es el que ya descartamos.** Electron + xterm.js es el stack de Orca, con medida propia
   en el anexo «Medido — 8 agosto» de [02-decisiones.md](02-decisiones.md) → D3: escalado negativo,
   7,1 MiB/s agregados con cuatro divisiones frente a 8,6 con una.

**Y aquí está la evidencia empírica ajena de D6**, que es lo más valioso de esta ficha. Dos
hallazgos, los dos en su repositorio:

- **El bug [#5259](https://github.com/superset-sh/superset/issues/5259), abierto.** La notificación
  de «Task complete» salta **en cada turno del modelo**: una tarea con 63 turnos produjo 63 avisos.
  La causa es literal — el hook `stop` de Cursor Agent significa *«acabé este turno»* y Superset lo
  leyó como *«acabé la tarea»*. Ya les pasó (#1785) y el arreglo por *debounce* (#1786) se cerró sin
  fusionar. **Un evento declarado no basta si la app le pone encima una semántica que el harness no
  tenía**: falla igual que adivinando. Y no se arregla con un *debounce*, porque el dato que falta no
  está en ninguna parte del sistema.
- **Su propio `HOOKS_INVESTIGATION.md`** documenta que el camino de compatibilidad v1 se salta el
  acotado por identidad: basta que el `session_id` venga del payload del propio agente para que el
  guard pase, y entonces una sesión ajena hace `curl` al servidor local de notificaciones con los
  identificadores de panel y terminal vacíos. Es el fallo que **D7** nombra como su razón de ser,
  documentado por ellos.

**El riesgo, comparado con Xirp: distinto y menor.** Xirp preocupa por quién está detrás y porque
tiene el terminal nativo; Superset, por lo lejos que ha llegado en producto. Pero la ELv2 les cierra
la puerta a volverse estándar como fue Backstage, y su emulador vive en el navegador — la misma
decisión estructural que ya señalamos en Cline, y que no se cambia sin rehacer el producto.

---

## Lo que este panorama dice del proyecto

**1. El patrón «un worktree por agente» ya está estandarizado.** Todo el mundo lo hace, y `flow`
ya lo hace. No hay que inventarlo: hay que apoyarse en él. La identidad de división (D7) encaja
exactamente ahí — un worktree, una división, un `PANEL_ID`.

**2. Nuestra casilla tiene vecinos, y uno de ellos es Spotify.** Los de E1 esconden el terminal; los
de E2 lo mantienen pero lo construyen *sobre tmux*, heredando lo que tmux no da (ratón decente,
botones, arrastrar paneles). Eso dejaba la casilla vacía hasta que aparecieron dos, por lados
distintos: **Xirp** (E3) es app nativa con terminal de primera clase y sesiones persistentes, y
**Superset** (E4) llega mucho más lejos en producto —divisiones, pestañas, diffs, puertos, cron—
pero con el emulador dentro del navegador. Y **Warp** (E5), que ya no es un vecino: cumple las cinco
condiciones —terminal nativo, las tres plataformas, cinco harnesses hospedados, panel de estado con
salto a la división, y open source con 64k estrellas—. Ya no se puede decir «nadie hospeda agentes
como terminal de primera clase», ni «nadie lo hace abierto y multiplataforma». Y **cmux** (E6) tumba
media frase más: **sí hay uno con el canal abierto**, porque cualquier proceso puede emitirle OSC
777/99. Lo que queda en pie hay que decirlo con precisión, porque la versión corta ya no es cierta:
**nadie define un contrato abierto para publicar estado estructurado y persistente.** Los cuatro
primeros tienen estado con vocabulario que elige la app —tres constantes, cuatro eventos, unas
métricas, cinco estados—; cmux deja elegir el texto pero solo caben un título y un cuerpo que se
borran al leerse. Ninguno tiene un panel por división cuyo cuerpo escriba quien trabaja.

**3. El hueco de Conductor ya no existe: lo cerró Warp.** Era el más pulido del grupo y es **macOS y
propietario**, así que multiplataforma y abierto era espacio libre. Cline Kanban lo tapó por un lado
—abierto, multiplataforma, agnóstico de agente—, Xirp por el otro —terminal de primera clase, con el
respaldo de Spotify, aunque macOS y cerrado— y **Warp lo cerró entero**: abierto, las tres
plataformas y el terminal como ciudadano de primera, las tres cosas a la vez. Lo que queda sin ocupar
**ya no es una combinación de plataforma y licencia**, porque esa combinación existe. Es **el contrato
abierto**, y nada más. Conviene no engañarse con esto: cualquier argumento de venta que no sea el
contrato, hoy lo cumple Warp mejor que nosotros.

**4. Aviso serio: el espacio está lleno y con rotación.** Hay comparativas de doce herramientas,
Crystal ha tenido que renombrarse, y la empresa de Vibe Kanban cerró. Que haya demanda no
significa que sea fácil monetizar. Entra sabiéndolo: para uso propio esto es inversión segura;
como producto, la diferenciación tiene que ser evidente en treinta segundos.

**5. La diferenciación en una frase.** «El anfitrión donde cualquiera publica estado» ya no se
distingue en treinta segundos de «tablero agnóstico de agentes» —Cline Kanban dice algo parecido—, ni
por hospedar terminales, que hace Xirp, ni por ser un terminal nativo abierto y multiplataforma que
hospeda cinco agentes, que es Warp. Se distingue por el *cualquiera*, y por **qué** se publica. La
distinción que aguanta:

**Ellos te dicen si el agente trabaja o espera. Nosotros dejamos que quien trabaja diga por dónde
va — y quien trabaja no tiene que ser un agente.**

Debajo siguen valiendo las patas de siempre: contrato abierto para cualquier productor, terminal de
primera clase, multiplataforma, y sin pagar API sobre lo que ya pagas por suscripción. Lo que ha
cambiado es el peso: **la primera ha pasado de ser una de tres a ser la que sostiene a las demás.**

**Y una precisión que Conductor obliga a hacer, porque es fácil equivocarse aquí.** El eje **no es
cuánto dato se enseña**. Conductor ya enseña tokens, coste y aviso de límite de contexto, y eso no le
da nuestro panel: los eligió él. El eje es **quién decide qué se ve**. Nuestro panel es texto libre
del productor —el paso de la tarea, lo que la bloquea, y lo que al usuario de esa división le
interese poner pidiéndoselo a su propio agente— sin que nadie haya tenido que diseñar antes ese
campo. Las métricas de la IA son un **extra** del contrato; confundirlas con el objetivo nos
llevaría a competir en un terreno donde ya hay cuatro productos por delante.

**6. Y la carrera real, dicha sin adornos — con dos rivales de distinta clase.** Superset (E4) es el
más completo de los orquestadores y está sobre Electron + xterm.js, el stack que D3 midió y descartó:
**tiene que cambiar de suelo para alcanzarnos, y eso obliga a rehacer la app.** Con **Warp (E5) esa
ventaja no existe**: su suelo es Rust con GPU propia, igual de bueno que el nuestro, y encima ya
funciona en las tres plataformas. Frente a Superset la carrera es «nosotros construimos producto
mientras ellos rehacen los cimientos». Frente a Warp no hay carrera de cimientos: **solo nos separa
haber decidido que la app no es la dueña de la semántica.** Es una decisión de diseño, no una ventaja
técnica, y una decisión se copia en una release — más aún siendo ellos open source, donde la idea
puede llegar en un *pull request* de cualquiera. El reloj corre más rápido de lo que parecía.

---

## En qué nos diferenciamos — agosto 2026

El resumen de los seis análisis de este mes. Se escribe aquí porque la respuesta cambió: hace un mes
eran tres patas y hoy es una, y quien vuelva a esta pregunta dentro de tres meses necesita la versión
de hoy, no la de la primera semana.

### Lo que ya NO nos diferencia

Esto es lo importante de asumir, porque estaba en el discurso y hoy tiene dueño:

| | Quién lo tiene ya |
|---|---|
| Terminal nativo, no navegador | Warp, cmux, Xirp |
| Multiplataforma real | **Warp** |
| Open source | Warp, cmux, Superset |
| Un worktree por agente | **todos** |
| Hospedar cualquier agente CLI | **cmux** (sin adaptadores), Warp (cinco) |
| Avisar cuando un agente espera | **todos** |
| Coste, tokens, contexto restante | Conductor |
| Canal para pilotar la app desde el agente | Superset y Xirp (MCP), cmux (socket) |
| Que cualquier proceso pueda publicar algo | **cmux** (OSC 777/99) |

Nueve cosas. Ninguna sirve ya para explicar el proyecto en treinta segundos.

### Lo que sí queda, y son tres

**1. El cuerpo del panel lo decide quien trabaja, y es estructurado.** Los cuatro primeros tienen
estado rico con vocabulario que elige la app. cmux abre el canal pero solo transporta una línea.
Nadie tiene lo de en medio: **una lista con marcas** (`done`, `current`, `pending`, `wait`, `block`,
`info`), referencias, enlaces y una `phase` en texto libre, escrita por el productor en su propio
vocabulario, sin que nadie haya tenido que diseñar antes ese campo.

**2. El productor puede ser cualquiera, y por un canal fuera de banda.** Ni un `enum` compilado
(Warp), ni un wrapper por agente (Superset), ni un harness soportado (Xirp, Conductor), ni «lo que
viva dentro de ese PTY» (cmux, por OSC). Un fichero que escribe quien sea desde donde sea: un agente,
un CI en otra máquina, un despliegue, un cron, un script propio.

**3. Es estado persistente, no un evento.** Sigue ahí cuando vuelves, y **sabe caducar**
(`updated_at` + `stale_after_minutes`). La diferencia práctica: los seis te dicen *a qué división ir
ahora*; este te dice *por dónde va cada una* cuando llegas.

### Dicho en una frase

**Ellos te dicen si el agente trabaja o espera. Nosotros dejamos que quien trabaja diga por dónde va
— y quien trabaja no tiene que ser un agente.**

### La posición real en el mapa

Los dos rivales serios tienen mitades distintas de esta casilla, y ninguno las dos:

| | Tiene | Le falta |
|---|---|---|
| **Warp** (E5) | nativo + multiplataforma + abierto | **el contrato** |
| **cmux** (E6) | nativo + abierto + canal abierto de una línea | **multiplataforma** y el contrato estructurado |

La combinación completa —nativo, multiplataforma, abierto **y** contrato estructurado— sigue vacía.
Pero es un hueco **de una decisión de diseño de ancho, no de tecnología**, y los dos que lo rodean son
open source, así que la idea puede llegarles en un *pull request* de cualquiera.

**Consecuencia para priorizar, y es la razón de que este resumen esté escrito:** el contrato no es la
última pata que implementar cuando el terminal esté pulido. Es **la única que sostiene el proyecto**,
y todo lo que se haga antes que ella se hace sabiendo que no diferencia.

---

## Cosas concretas para robar

| De | Qué |
|---|---|
| **Extraterm** | Marcos por comando, coloreados por resultado; reutilizar la salida de un comando como entrada |
| **Warp** | Bloques de comando plegables y compartibles. Y del panel global de agentes: **saltar desde la lista al pane concreto** — el resumen global no solo informa, es navegación (ver E5) |
| **Wave** | Composición por bloques heterogéneos (terminal, editor, previsualización) y workspaces en barra lateral |
| **Tilix** | Árbol de divisiones y arrastrar terminales para recolocarlos |
| **Zellij** | Interfaz hecha de plugins; paneles flotantes movibles con el ratón |
| **WezTerm** | `wezterm cli` como superficie de control externa; sesiones desacoplables |
| **Ghostty** | Separar núcleo de interfaz para poder ir a Mac y Windows sin rehacer el motor |
| **cmux** | **Entender OSC 777 y 99 además del contrato propio** — son estándares que ya emiten Kitty, rxvt y media docena de herramientas, así que soportarlos da compatibilidad gratis con todo lo que hoy notifica, sin pedirle a nadie que adopte `panel.json`. Y convertir los subagentes en **panes nativos** en vez de dejarlos como procesos ocultos |
| **opencode** | Bloque de contexto y coste; indicador de sesión ocupada en otro sitio |
| **Conductor / Vibe Kanban** | Revisión de diffs antes de fusionar como parte del flujo, no como paso aparte |
| **Conductor** | **Reservar un rango de puertos por división** (`CONDUCTOR_PORT` a `+9`) en vez de detectar en qué puerto ha levantado algo. Encaja con `PANEL_ID`/`PANEL_DIR` de D7 y le quita a la app un trabajo de deducción, que es justo lo que D6 prohíbe |
| **Cline Kanban** | Cadenas de dependencias entre tareas: al terminar una arranca sola la siguiente, sin escribir un script (ver «Ideas sin fase» en [05-roadmap.md](05-roadmap.md)) |
| **Xirp** | **Notificar** al pasar a «esperando entrada», no solo pintarlo: resuelve «a qué división voy ahora» sin tener las cuatro a la vista (ver «Pendiente» en [04-contrato-panel.md](04-contrato-panel.md)) |
| **Superset** | Navegador integrado con **detección de puertos por división** — y el productor ya sabe en qué puerto levantó su servidor, así que puede publicarlo. Agentes lanzados en cron: un `panel.json` con productor programado no pide nada nuevo al contrato. Y **distinguir dos esperas**: un permiso que se concede en un segundo (`PermissionRequest`) no es una pregunta que exige pensar (`PendingQuestion`); nuestro contrato tiene un solo `wait` |
