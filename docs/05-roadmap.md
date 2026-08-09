# Roadmap

En orden. **Cada fase vale por sí sola aunque se abandone la siguiente** — esa es la regla que
decide qué entra en cada una.

Las fases 1 y 2 están desglosadas en hitos porque son las que se van a construir. De la 3 en
adelante son titulares a propósito: detallar lo que aún no se ha tocado es inventar.

---

# Parte I — Qué tienes al final de cada fase

Sin nada técnico. Esta es la parte que hay que releer cuando no se sepa si algo entra en una
fase o en la siguiente: **si no aparece descrito aquí, no entra**.

## Fase 1 — «Esto existe»

**Lo que ves:** una ventana con dos consolas lado a lado. A la derecha de cada una, una franja
estrecha que muestra en qué va esa tarea: la fase, el paso actual, lo que esté bloqueado.

**Lo que puedes hacer:** un agente en la izquierda trabajando en una tarea, otro en la derecha
en otra, y saber de un vistazo cuál avanza y cuál te está esperando sin leer lo que escriben.

**Lo que falta:** todo lo demás. Ni pestañas, ni copiar y pegar, ni historial, ni mover nada de
sitio. Dos consolas fijas.

**Por qué vale igual:** es la única fase que puede decir que no. Si con esto delante la sensación
es «no aporta nada que no me diera tener dos ventanas», el proyecto se para aquí habiendo gastado
poco. Si es «no quiero volver atrás», el resto es trabajo, no apuesta.

## Fase 2 — «Esto es mi terminal»

**Lo que ves:** la app entera, la del mockup. Pestañas arriba, y dentro de cada una las
divisiones que quieras, colocadas como quieras.

**Lo que puedes hacer:** una pestaña por proyecto o por contexto, y dentro tantas tareas en
paralelo como quepan. **Arrastrar una división y soltarla en otro sitio** cuando el trabajo
cambia de forma — lo que no da ningún terminal actual y lo que originó este proyecto. Copiar,
pegar, buscar, subir por el historial. Plegar el panel. Cerrar y encontrártelo como lo dejaste.

**Lo que falta:** el panel sigue siendo de solo lectura. Cuenta cosas, no puedes hacer nada
desde él.

**Por qué es el hito de verdad:** aquí se desinstala WezTerm. Y es la primera versión que se le
puede enseñar a alguien en Mac o Windows.

## Fase 3 — «El panel responde»

**Lo que ves:** lo mismo, pero el panel tiene botones. «Continuar», «lanzar los tests», «abrir
la MR» — lo que la propia tarea declare que tiene sentido en ese momento.

**Lo que puedes hacer:** contestarle al agente sin escribir. Y en la otra dirección, el agente
puede pedirle cosas a la app: abrirse una división para lanzar los tests aparte, sin tocar la
que tiene delante.

**Lo que cambia de fondo:** hasta aquí la app era una pantalla donde mirar; desde aquí es un
sitio donde trabajar. Y el trabajo deja de ser solo el de los agentes: **cualquier herramienta
—un despliegue, un CI, un script de la empresa— puede publicar ahí su estado y sus botones.**

## Fase 4 — «¿Qué necesita algo de mí?»

**Lo que ves:** una pantalla nueva por encima de las pestañas. No consolas: una lista. Arriba lo
que espera una decisión tuya; debajo lo bloqueado; al final lo que avanza solo.

**Lo que puedes hacer:** empezar el día ahí. Con ocho tareas repartidas en cuatro pestañas el
problema deja de ser hacerlas y pasa a ser saber a cuál ir. **Ninguna herramienta del panorama
tiene esto**: todas enseñan las tareas, ninguna las ordena por lo que te reclama.

**Por qué está aparcada:** solo funciona si el estado que llega es fiable, y eso se sabe después
de usar la Fase 3 un tiempo. Antes es maquetar una lista bonita con datos en los que no confías.

## Fase 5 — «Alguien vigila por ti»

**Lo que ves:** la lista de la Fase 4, pero ordenada y anotada sola.

**Lo que puedes hacer:** delegar el mirar. Un agente que no programa — solo observa a los demás
y avisa: «esta lleva veinte minutos parada», «esta ha terminado y nadie la ha revisado», «a esta
le queda poco contexto».

**Por qué está lejos y da igual:** no necesita ninguna pieza nueva, es la Fase 3 usada de otra
manera. Si nunca se hace, no se ha construido nada de más para llegar. Eso es a propósito.

## Dónde está el corte

| | |
|---|---|
| **Fase 1** | Para saber si merece la pena |
| **Fase 2** | Para uso diario — y lo primero enseñable |
| **Fase 3** | Deja de ser una pantalla y pasa a ser un sitio de trabajo |
| **Fase 4** | Lo que nadie más tiene |
| **Fase 5** | Regalo, si llega |

---

# Parte II — Cómo se construye

---

## Fase 0 — Hecho

- [x] Panel de estado funcionando sobre WezTerm (`prototipos/flow-panel`)
- [x] Contrato `panel.json` definido y con un productor real (`flow`)
- [x] Dos vistas: global (todas las tareas) y foco (una tarea, fijada)
- [x] Maqueta visual de la app para juzgar proporciones (`prototipos/mockup-terminal.html`)
- [x] Decisiones de stack y arquitectura ([02-decisiones.md](02-decisiones.md))

---

## Punto de partida

Comprobado en la máquina de desarrollo (2026-08-07):

| | |
|---|---|
| `rustc` / `cargo` | **1.97.1** — actualizado al empezar F1.3 (ver abajo) |
| `git` | 2.53.0 |
| `inotify-tools` | no instalado — y ya no hace falta (ver F1.6) |

> **Corregido el 2026-08-07, al arrancar F1.3.** Esta tabla decía «1.94.0 — listo, no hay nada que
> instalar», y era incompatible con la versión de `eframe` fijada abajo: `eframe 0.36.1` exige
> `rustc 1.95`. Se comprobó con `cargo check` sobre un crate de prueba, y no había forma de escribir
> una línea del crate `app` sin resolverlo. Se subió el toolchain a 1.97.1 —lo que `rustup` ofrecía—
> en vez de bajar `eframe` a 0.35: la versión del framework la elige el proyecto y el toolchain es
> lo barato de mover. Tras la subida, `clippy` y los 8 tests del workspace siguen limpios.

Versiones de los crates resueltas hoy contra crates.io:

```toml
alacritty_terminal = "0.26"   # parser VT + estado del terminal
portable-pty       = "0.9"    # PTY Unix + ConPTY Windows
eframe             = "0.36"   # egui + ventana nativa
notify             = "8.2"    # cambios de fichero, multiplataforma
serde / serde_json = "1"      # el contrato
```

### La UI del prototipo se hace con egui, no con GPUI

D3 dejaba GPUI para el producto y egui como *opción* para prototipar. Para la Fase 1 se elige
**egui**, y conviene dejar escrito por qué, porque es una decisión con fecha de caducidad:

- Lo que la Fase 1 tiene que responder es **si el conjunto se sostiene en las manos** — un
  terminal de verdad con un panel al lado. Eso no depende del framework.
- egui se publica en crates.io con versiones y documentación. GPUI vive dentro del repo de Zed,
  con API inestable y sin más documentación que su propio código.
- Todo lo caro de la Fase 1 —el PTY, la rejilla, el contrato— vive **fuera** de la capa de
  pintado (ver la estructura de abajo). Cambiar de framework después es reescribir el crate
  `app`, no el proyecto.

**Cuándo saltar a GPUI:** cuando egui deje de dar el rendimiento o el control necesario, y no
antes. Los dos síntomas concretos a vigilar están en los riesgos de F1.3.

### Estructura

```
agent-terminal/
├── Cargo.toml              workspace
├── crates/
│   ├── panel-contract/     tipos de panel.json, lectura, validación  ← no depende de nada visual
│   ├── term-core/          una división = PTY + rejilla              ← no depende de nada visual
│   └── app/                ventana, divisiones, pintado              ← la única pieza atada a egui
├── docs/
└── prototipos/
```

Las dos primeras cajas son las que hay que proteger. Si `app` se tira a la basura y se reescribe
en GPUI, `panel-contract` y `term-core` siguen sirviendo tal cual.

---

## Fase 1 — Prototipo mínimo

**Objetivo:** ver si el conjunto se sostiene en las manos. Sin pestañas, sin botones.

**Criterio de éxito:** usarlo **media hora de trabajo real** sin echar de menos WezTerm por algo
que no sea pestañas.

Tamaño: **S** = una sentada · **M** = varias · **L** = va a doler.

| Hito | Qué se construye | Cómo se comprueba | Tamaño |
|---|---|---|---|
| **F1.0** | `git init` y el workspace con las tres cajas vacías | `cargo build` limpio | S |
| **F1.1** | `term-core`: `portable-pty` lanza el shell, hilo lector, escritura al PTY | Un binario de prueba lanza `zsh -l`, escribe `ls\n`, imprime la salida cruda | S |
| **F1.2** | `term-core`: los bytes entran en `alacritty_terminal::Term`; volcar la rejilla a texto | `vim` y `htop` producen una rejilla coherente; al redimensionar, refluye | M |
| **F1.3** | `app`: pintar la rejilla en egui — fuente monoespaciada, colores ANSI, cursor. Teclado → PTY | Se puede usar `ls`, `vim` y **`claude`** dentro de la ventana | L |
| **F1.4** | Dos divisiones lado a lado, foco por clic, divisor arrastrable | Dos shells vivos a la vez sin que se pisen | M |
| **F1.5** | `PANEL_ID` / `PANEL_DIR` inyectados al lanzar cada shell; el directorio se crea y se limpia al cerrar | `echo $PANEL_ID` da valores distintos en cada división | S |
| **F1.6** | `panel-contract` + `notify`: panel a la derecha de cada división, repintado al cambiar el fichero | `flow` escribe en una división y solo esa repinta | M |

### Lo que la Fase 1 deja fuera a propósito

Scrollback largo, selección con ratón, copiar/pegar, búsqueda, ligaduras de fuente, protocolo
gráfico de Kitty, pestañas, botones, MCP. Todo eso es Fase 2 o posterior. Meterlo aquí convierte
un prototipo de una semana en uno de tres meses y no responde mejor la pregunta.

### Riesgos concretos

| Riesgo | Dónde aparece | Qué hacer |
|---|---|---|
| **Ancho de glifo** — CJK, emoji y caracteres combinantes descolocan la rejilla | F1.3 | **No hay nada que portar** (corregido, ago 2026): el parser ya resuelve el ancho y marca la celda acompañante, y el lector de `term-core` lo expone como `CellWidth`. Lo que le queda a quien pinta es **respetar** las dos columnas, no calcularlas |
| **Repintado completo** — egui es modo inmediato: repinta todo cada frame | F1.3 | Repintar solo cuando el PTY produzca cambios. **Si con dos divisiones activas no aguanta, ese es el síntoma para revisar GPUI** |
| **Latencia de tecla** — un terminal que se nota lento al escribir es inservible | F1.3 | Medir tecla→pixel. **Segundo síntoma para revisar GPUI** |
| **Teclas muertas e IME** | F1.3 | Aceptar que quede regular en el prototipo; es Fase 2 |
| **Rendimiento del volcado masivo** (`cat` de un fichero grande) | F1.2 | Acotar cuántos bytes se procesan por frame |
| **Windows** — ConPTY se comporta distinto | fuera de Fase 1 | No se toca hasta la Fase 2. `portable-pty` lo abstrae, pero abstraer no es probar |

---

## Fase 2 — Anfitrión de verdad

Lo que convierte el prototipo en algo que se usa a diario.

| Hito | Qué |
|---|---|
| **F2.1** | Pestañas |
| **F2.2** | Divisiones arbitrarias — árbol de divisiones, no solo dos |
| **F2.3** | **Arrastrar divisiones con el ratón** para recolocarlas — lo que ningún terminal actual da |
| **F2.4** | Plegar el panel por división |
| **F2.5** | Guardar y restaurar la disposición al abrir |
| **F2.6** | Scrollback, selección, copiar y pegar |
| **F2.7** | Primera compilación y prueba real en **macOS y Windows** |

F2.3 es la razón de ser de la fase: es la carencia que llevó a plantearse esto, y es la que
justifica no haber cogido un multiplexor ya hecho.

F2.7 no se puede retrasar más allá de aquí. Cuanto más tarde se pruebe en las otras dos
plataformas, más caro sale cada supuesto que resultó ser de Linux.

---

## Fase 3 — Bidireccional

- [ ] `actions`: botones declarados en el contrato que escriben en el PTY de su división
- [ ] Servidor **MCP** con `panel_set`, `split_open`, `split_read`, `split_close`
- [ ] Acotado por `PANEL_ID`: cada agente alcanza su división y no las demás

**No empezar sin releer la tabla de seguridad de [03-arquitectura.md](03-arquitectura.md).**
Un fichero que declara comandos y un botón que los ejecuta es ejecución de código en el shell
del usuario; el comando tiene que verse antes de ejecutarse.

---

## Fase 4 — Vista resumen

**Aparcado a propósito.** Depende de que las divisiones y el contrato estén rodados.

La pantalla que responde a *«¿qué requiere algo de mí ahora mismo?»*: todas las tareas de todas
las pestañas, ordenadas por lo que espera una decisión, lo que está bloqueado, y lo que avanza
solo. El germen ya existe — es la vista global de `flow-panel` — pero la versión buena necesita
el indicador de actividad y el estado de todas las divisiones a la vez.

---

## Fase 5 — Supervisor

**Lejos, y está bien que lo esté.** Un agente que observa el estado de todas las divisiones y
dirige a los demás.

Lo importante: **la infraestructura es la misma que la Fase 3**. Los botones y el supervisor usan
el mismo canal. Si nunca se hace, no se ha perdido nada.

---

## Pendientes sueltos

- [ ] Decidir nombre del proyecto — aparcado hasta tener el prototipo delante
- [ ] `flow-note "texto"` — comando corto para escribir en el panel de la tarea de la rama actual
- [ ] Hook que publique el estado automáticamente al final de cada paso, en vez de a mano
- [ ] Cerrar dos cabos del contrato: `actions`, y que `header: false` no dependa de `meta.json`
      (ver [04-contrato-panel.md](04-contrato-panel.md))
- [ ] Revisar términos de uso antes de distribuir a terceros (automatizar sesiones de
      suscripción es zona gris; ver [02-decisiones.md](02-decisiones.md) → D4)

~~Instalar `inotify-tools`~~ — cae con F1.6: el crate `notify` hace lo mismo y además en macOS
y Windows. Mientras el panel siga corriendo sobre WezTerm, el sondeo por `mtime` cada segundo
funciona y no molesta.
