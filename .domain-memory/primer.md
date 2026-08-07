# Project primer

## What this is

Un terminal **anfitrión de agentes**: ventana → pestañas → divisiones, donde cada división es una
consola independiente **más un panel de estado**. El panel no lo escribe la app: lo escribe *quien
hace el trabajo* (un agente, un CI, un despliegue, un script) mediante un fichero con formato
definido (`panel.json`). La app solo lo pinta.

Resuelve el problema de trabajar con varios agentes de terminal en paralelo sin saber, de un
vistazo, cuál trabaja y cuál espera, por qué paso va cada tarea, a cuál le queda poco contexto,
cuánto lleva gastado y qué está bloqueado.

**Qué NO es:** no es un agente (no razona ni llama a modelos), no es un multiplexor de sesiones
remotas (eso es tmux/Zellij) y no reescribe el emulador de terminal (se consume como librería).

## Users

- **Desarrollador que orquesta varios agentes a la vez** (Claude Code, opencode, Codex, Gemini
  CLI) — necesita la vista global del estado de todas sus tareas en curso.
- **Productor de estado, agente o no** — cualquier proceso que corra dentro de una división y
  quiera publicar en qué va: un CI, un despliegue, un script propio.

## Main modules / domains

- **Terminal** — el emulador por división. Existe porque sin una consola correcta y rápida nada
  de lo demás importa; el motor VT y el PTY son de terceros (D2), aquí solo se hospedan.
- **Panel** — pinta el estado publicado por el productor. Su razón de ser es que la información
  que **no** se puede deducir desde fuera (contexto restante, coste, bloqueos) vale más que la
  que sí, y solo el que trabaja la conoce.
- **Contrato `panel.json`** — el formato abierto que separa a la app de cualquier dominio
  concreto. Es lo que permite cuatro divisiones con cuatro productores distintos y un resumen
  global de todas.
- **Canales (D5)** — tres, deliberadamente: MCP (agente → app: crear divisiones, publicar
  estado, leer salida), `panel.json` (productor → app) y PTY (app → agente: botones y todo lo
  que no hable MCP). El PTY no es el mecanismo principal, es el respaldo universal.
- **Identidad por división (D7)** — `PANEL_ID` / `PANEL_DIR` inyectados al lanzar cada shell.
  Es lo que desata el contrato de cualquier herramienta concreta y, a la vez, el límite de
  seguridad de MCP.

## Invariants / constraints

- **La app nunca define `ANTHROPIC_API_KEY`** ni equivalentes de otros proveedores al lanzar un
  panel: tendría prioridad sobre la sesión de la suscripción y cambiaría la facturación del
  usuario sin que se entere (D4).
- **La app no deduce el estado.** No parsea el pintado de una TUI ni ficheros de terceros para
  inferir en qué va una tarea. Esa semántica solo la conoce quien trabaja — deducirla ya produjo
  tres errores seguidos en el prototipo (D6).
- **El control de agentes va por consola, no por API.** Las suscripciones solo valen dentro del
  cliente oficial de cada proveedor; obligar a pagar API por lo ya pagado mata la adopción (D4).
- Los `actions` del panel son **ejecución de código** en el shell del usuario: el comando se ve
  antes de ejecutarse y el texto que se envía es plano, sin interpolación ni plantillas.
- Un `panel.json` de **origen ajeno** (repo clonado, contenedor) es no confiable: nunca ejecutar
  nada de ahí sin confirmación visible.
- Leer el buffer y mandarlo a un modelo **exfiltra** lo que haya en pantalla (tokens, claves,
  respuestas de API): qué se envía se elige explícitamente, nunca de forma automática.
- **MCP se acota por `PANEL_ID`**: un agente alcanza su división y no las demás.
- **Multiplataforma real** (Linux, macOS, Windows). Descarta de raíz cualquier solución
  solo-Linux, por cómoda que sea (D1).
- **Que funcione sin el panel:** si nadie publica nada, sigue siendo un terminal correcto.
- Cada fase del roadmap vale por sí sola aunque se abandone la siguiente; lo que una fase deja
  fuera está fuera a propósito.

## External stakeholders / integrations

- **MCP (Model Context Protocol)** — el canal agente→app. Elegido porque lo hablan Claude Code,
  opencode, Gemini CLI y Codex: se escribe el servidor una vez y sirven todos, y **no consume
  API aparte** (se añade a la sesión que el agente ya tiene con su suscripción).
- **`alacritty_terminal`** — parseo VT y estado del terminal, como librería.
- **`portable-pty`** — el PTY, abstrayendo Unix y ConPTY de Windows.

## Glossary

- **División** — una consola independiente dentro de una pestaña: terminal + panel. Es la unidad
  de identidad (`PANEL_ID`) y el límite de seguridad de MCP.
- **Panel** — la vista de estado de una división. Nunca la escribe la app.
- **Productor** — quien publica el estado en `panel.json`: un agente, un CI, un despliegue, un
  script. La app no sabe cuál es ni le importa.
- **Contrato abierto** — que el formato del panel no pertenece a ningún producto concreto, a
  diferencia del panel de opencode, que solo sabe de opencode.
- **Rancio (stale)** — un resumen cuyo `updated_at` ha envejecido. Un resumen que solo se escribe
  al terminar un paso miente si el proceso muere a mitad; la marca de tiempo lo delata (D6).

## Stack

Rust 1.94. **GPUI** para el producto y **egui** para prototipar (D3); Electron/Tauri + xterm.js
descartado por su techo con muchos paneles. Las decisiones vivas están en `docs/02-decisiones.md`
(D1–D7) y el plan por fases en `docs/05-roadmap.md`: la Parte I define qué entra en cada fase y
la Parte II cómo se construye.
