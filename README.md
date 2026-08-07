# agent-terminal

> **Nombre en clave.** Aparcado a propósito: el producto se ha redefinido dos veces mientras se
> escribía esto, y un nombre para algo que aún se mueve suena mal haga lo que haga. Se retoma
> con el prototipo delante. Renombrar es `mv ~/Mashware/agent-terminal ~/Mashware/<nombre>` más
> un buscar y reemplazar en `docs/`.
>
> La marca paraguas ya existe: **Zabri** (`~/Mashware/zabri.io`). Este sería un producto suyo,
> junto a `rule-engine`.

Un terminal **anfitrión de agentes**: pestañas, divisiones, y en cada división una consola
con su propio panel de estado alimentado por un contrato abierto.

## El problema

Trabajar con varios agentes a la vez (Claude Code, opencode, Codex, Gemini CLI) hoy significa
tener N consolas abiertas y no saber, sin mirarlas una a una, cuál avanza, cuál te espera y
cuál se ha quedado sin contexto.

Cada agente ha empezado a resolverlo por su cuenta dentro de su propia interfaz de texto —
opencode ya tiene barra lateral con plan, contexto y coste, y peticiones abiertas para
pestañas y multiventana. Es decir: **cada agente está reimplementando, peor, un gestor de
ventanas**, porque no existe el anfitrión que debería hacer ese trabajo.

## La propuesta

El anfitrión pone la estructura (pestañas, divisiones, paneles) y define **un contrato abierto**
para que cualquiera publique estado en el panel de su división: un agente, un CI, un despliegue,
un script. Cada agente vuelve a ocuparse solo de ser un agente.

## Estado

| | |
|---|---|
| **Funcionando** | `flow-panel` — el panel de estado y su contrato, corriendo sobre WezTerm ([prototipos/](prototipos/)) |
| **Decidido** | Stack, arquitectura de canales, modelo económico ([docs/02-decisiones.md](docs/02-decisiones.md)) |
| **Siguiente** | Fase 1 — prototipo: una ventana, dos divisiones, un panel leyendo el fichero. Desglosada en 7 hitos en [docs/05-roadmap.md](docs/05-roadmap.md) |

## Documentación

| Documento | Contenido |
|---|---|
| [01-vision.md](docs/01-vision.md) | Qué es, para quién, y qué no es |
| [02-decisiones.md](docs/02-decisiones.md) | Decisiones técnicas con su razón y lo que se descartó |
| [03-arquitectura.md](docs/03-arquitectura.md) | Las piezas y los tres canales |
| [04-contrato-panel.md](docs/04-contrato-panel.md) | Especificación de `panel.json` |
| [05-roadmap.md](docs/05-roadmap.md) | Fases, en orden, con lo aparcado |
| [06-referencias.md](docs/06-referencias.md) | Cómo está construido cada terminal, con datos |
| [07-panorama.md](docs/07-panorama.md) | Quién hay en el espacio y qué robarle a cada uno |
