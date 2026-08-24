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
pegar, buscar, subir por el historial. Plegar el panel, o darle el ancho que quieras. Cerrar y encontrártelo como lo dejaste.
**Y cuando algo te reclama en una pestaña que no estás mirando, la ficha de esa pestaña te lo dice** —
si te espera a ti, si está atascada, o si terminó y nadie lo ha visto.

**Y lo que un terminal da por hecho, porque es contra terminales contra quien se compite** (**D19**):
que se vean tus fuentes —los iconos de un `ls` y el japonés, no cuadros vacíos—, que puedas cambiarle
la fuente, el tamaño, los colores y los atajos, y que el ratón llegue al programa de dentro para
poder hacer clic en `vim`.

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
problema deja de ser hacerlas y pasa a ser saber a cuál ir.

> **Corregido en agosto de 2026.** Aquí decía «ninguna herramienta del panorama tiene esto: todas
> enseñan las tareas, ninguna las ordena por lo que te reclama», y **dejó de ser cierto**: la barra
> lateral de Herdr acumula el estado de agente a panel, a pestaña y a workspace, y ordena por quién
> necesita atención ([07-panorama.md](07-panorama.md) → E8). Lo que sigue sin tener nadie no es el
> **orden**, es el **cuerpo**: por qué está atascada, en qué paso va y cuánto le queda. La lista de
> esta fase se ordena con `attention` (D18) y se lee con lo que publique el productor.

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

> **Medidos en F1.3, ago 2026: ninguno de los dos aparece.** El repintado completo escala lineal
> —dos consolas cuestan 1,96× una y el fotograma gasta el 36 % de su hueco— y escribir cuesta 36 µs.
> El salto no toca. Lo que queda por comprobar es la carga que estos números **no** cubren: cuatro
> divisiones y los paneles pintando a la vez, o sea F1.4 y F1.6. Los números, con lo que prueban y
> lo que no, están en el anexo «Medido — 9 agosto» de [D3](02-decisiones.md).

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
| **F1.0** ✅ | `git init` y el workspace con las tres cajas vacías | `cargo build` limpio | S |
| **F1.1** ✅ | `term-core`: `portable-pty` lanza el shell, hilo lector, escritura al PTY | Un binario de prueba lanza `zsh -l`, escribe `ls\n`, imprime la salida cruda | S |
| **F1.2** ✅ | `term-core`: los bytes entran en `alacritty_terminal::Term`; volcar la rejilla a texto | `vim` y `htop` producen una rejilla coherente; al redimensionar, refluye | M |
| **F1.3** ✅ | `app`: pintar la rejilla en egui — fuente monoespaciada, colores ANSI, cursor. Teclado → PTY | Se puede usar `ls`, `vim` y **`claude`** dentro de la ventana | L |
| **F1.4** ✅ | Dos divisiones lado a lado, foco por clic, divisor arrastrable | Dos shells vivos a la vez sin que se pisen | M |
| **F1.5** ✅ | `PANEL_ID` / `PANEL_DIR` inyectados al lanzar cada shell; el directorio se crea y se limpia al cerrar | `echo $PANEL_ID` da valores distintos en cada división | S |
| **F1.6** ✅ | `panel-contract` + `notify`: panel a la derecha de cada división, repintado al cambiar el fichero, y aviso cuando lo que se ve es viejo o no se pudo leer | `flow` escribe en una división y solo esa repinta | M |

### Lo que la Fase 1 deja fuera a propósito

Scrollback largo, selección con ratón, copiar, búsqueda, ligaduras de fuente, protocolo
gráfico de Kitty, pestañas, botones, MCP. Todo eso es Fase 2 o posterior. Meterlo aquí convierte
un prototipo de una semana en uno de tres meses y no responde mejor la pregunta.

### Riesgos concretos

| Riesgo | Dónde aparece | Qué hacer |
|---|---|---|
| **Ancho de glifo** — CJK, emoji y caracteres combinantes descolocan la rejilla | F1.3 | **No hay nada que portar** (corregido, ago 2026): el parser ya resuelve el ancho y marca la celda acompañante, y el lector de `term-core` lo expone como `CellWidth`. Lo que le queda a quien pinta es **respetar** las dos columnas, no calcularlas |
| **Repintado completo** — egui es modo inmediato: repinta todo cada frame | F1.3 | **RESPONDIDO (ago 2026): el síntoma no aparece.** Dos consolas cuestan 1,96× una —escala lineal— y el fotograma gasta el 36 % de su hueco con el pintor más ingenuo posible. Se repinta solo cuando el PTY produce cambios, como estaba previsto. **Remedido en F1.4 con dos divisiones de verdad (ago 2026), y el sospechoso cambia:** con las dos consolas volcando a la vez el fotograma cuesta 5,92 ms y cabe en el hueco de 120 Hz, pero **de eso solo 0,74 ms son atribuibles al framework** — 3,35 los gasta el parser tragando su presupuesto y el resto un pintor propio sin optimizar. El teclado sigue respondiendo mientras dura: 3,42 ms de la tecla al byte, frente a los 36 µs sin volcado. **No compara con el 36 % del banco**: aquel medía pantallas llenas y repintado continuo, y esto un volcado sobre casi el doble de celdas. **El peor caso con dos divisiones sigue sin medirse**, y el p95 del volcado doble ya se sale de 120 Hz. Números y límites en el anexo «Medido — 11 agosto» de [D3](02-decisiones.md) |
| **Latencia de tecla** — un terminal que se nota lento al escribir es inservible | F1.3 | **RESPONDIDO (ago 2026): 36 µs** de la tecla en el fotograma al byte en el programa. **No existe un número «tecla→píxel»**: los eventos de egui no traen sello propio y el reloj de su entrada es «relativo a lo que sea», así que ese tramo no es medible desde el proceso — publicarlo con ese nombre habría sido inventarlo. Lo que sí se mide, y con desglose, está en [D3](02-decisiones.md) |
| **Teclas muertas e IME** | F1.3 | Aceptar que quede regular en el prototipo; es Fase 2 |
| **Rendimiento del volcado masivo** (`cat` de un fichero grande) | F1.2 | Acotar cuántos bytes se procesan por frame |
| **Windows** — ConPTY se comporta distinto | fuera de Fase 1 | No se toca hasta la Fase 2. `portable-pty` lo abstrae, pero abstraer no es probar |

---

## Fase 2 — Anfitrión de verdad

Lo que convierte el prototipo en algo que se usa a diario.

| Hito | Qué |
|---|---|
| **F2.1** ✅ | Pestañas — **cerrado el 16 de agosto de 2026**. Ocho entregas; el fotograma medido con una y dos pestañas en el anexo de [D13](02-decisiones.md) |
| **F2.2** ✅ | Divisiones arbitrarias — **cerrado el 17 de agosto de 2026**. Siete entregas; el árbol n-ario con pesos y los tres acordes en [D14](02-decisiones.md), y el fotograma medido con dos y con cuatro divisiones en su anexo |
| **F2.3** ✅ | **Arrastrar divisiones con el ratón** para recolocarlas — **cerrado el 18 de agosto de 2026**. Tres entregas; el gesto, su atomicidad y la identidad del arrastre en [D15](02-decisiones.md), y el fotograma medido arrastrando sin parar en su anexo |
| **F2.4** ✅ | Plegar el panel por división y **estrechar su ancho arrastrando** — **cerrado el 19 de agosto de 2026**. Tres entregas; el ancho por división, el mínimo del gesto y el plegado como estado aparte en [D10](02-decisiones.md), y lo que cuesta el gesto medido en su anexo |
| **F2.5** ✅ | Guardar y restaurar la disposición al abrir — **cerrado el 24 de agosto de 2026**. Ocho entregas; el fichero propio, su versión y lo que no se entiende en [D16](02-decisiones.md), y en su anexo lo que cuesta abrir con las consolas que dejaste: **5,42 ms una sola vez y antes del primer fotograma**, con la tanda `L` midiéndolo y `H` de control |
| **F2.6** | Scrollback, selección y **copiar** — pegar se adelantó fuera de fase ([#46](https://github.com/mashware/agent-terminal/issues/46)), porque no necesita nada de lo demás y su ausencia hacía que la aplicación se sintiera rota. Ver D11. **Y el reenvío del ratón al programa de dentro**: hoy no llega, así que no se puede hacer clic dentro de `vim` ni de `htop` |
| **F2.7** | Primera compilación y prueba real en **macOS y Windows** |
| **F2.8** ✅ | **El panel dice cuándo te reclama, y la pestaña lo acumula** — **cerrado el 24 de agosto de 2026**. Tres entregas; el campo `attention` y quién lo declara en [D18](02-decisiones.md), los tres símbolos medidos y la tabla de color propia en el anexo de [D9](02-decisiones.md), y en el anexo de D18 lo que cuesta la barra desde que su ficha acumula: **entre 2,35 y 2,69 µs por fotograma**, medido con dos pestañas y los cuatro productores reclamando. El aviso del sistema se quedó fuera a propósito, hasta que el campo tenga productores |
| **F2.9** | **Las fuentes del sistema** — registrar al arrancar las que tenga la máquina, para que CJK y los iconos de un `ls` moderno dejen de salir como cuadros vacíos. Escrito en **D3** desde agosto como «lo que separa esto de ser usable a diario», y sin hito hasta **D19** |
| **F2.10** | **Configuración** — fuente, tamaño, colores y atajos. Es la primera cosa que hace cualquiera al instalar un terminal, y hasta **D19** no aparecía en este roadmap |
| **F2.11** | **Una cabecera por división** — al estilo de Tilix: el asidero para arrastrarla, su botón de cerrar, y de un vistazo cuál tiene el foco. Sale de **D20**, donde se descartó **como parte de F2.6** y no como idea: cuesta alto a **todas** las consolas siempre, y eso deja de comparar con nada las tandas de control ya publicadas. Su hito tiene que empezar por decidir qué se hace con esas medidas |

**De dónde sale esta lista, desde D19.** El listón que hay que pasar antes de publicar es el de un
usuario de WezTerm, Ghostty o Kitty — **no** el de un usuario de Orca. Eso es lo que mete a F2.9 y
F2.10 en la fase y lo que deja fuera el navegador integrado, la app móvil, el editor y el abanico de
agentes: la lista entera de lo que queda fuera, con quién lo tiene, está en **D19**.

**F2.8 se hace antes que F2.6 y F2.7, y el número no se recicla.** Los identificadores de hito viven
en commits, decisiones y en el almacén de conocimiento, así que renumerar sale más caro que aceptar
que la tabla no esté en orden. La razón de adelantarlo: es lo único de la Fase 2 que toca **la pata
que diferencia**, y el panorama de agosto dejó a dos competidores a un solo campo de distancia
([07-panorama.md](07-panorama.md) → E7 y E8). Scrollback y portabilidad son terminal, y el terminal
ya no es lo que hay que defender.

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

> **Y ahora hay una razón externa para que siga siendo así.** El supervisor **no puede ser una
> llamada nuestra a un modelo**: desde el **4 de abril de 2026** las suscripciones de Claude no
> cubren las herramientas de terceros, así que un agente que la app llamara por API lo pagaría el
> usuario aparte — justo lo que **D4** existe para evitar. El supervisor tiene que ser **otro agente
> de consola**, en su división, hablando por MCP como cualquier otro. Ver
> [02-decisiones.md](02-decisiones.md) → D4.

---

## Pendientes sueltos

- [ ] Decidir nombre del proyecto — aparcado hasta tener el prototipo delante
- [ ] `flow-note "texto"` — comando corto para escribir en el panel de la tarea de la rama actual
- [ ] Hook que publique el estado automáticamente al final de cada paso, en vez de a mano
- [ ] Cerrar dos cabos del contrato: `actions`, y que `header: false` no dependa de `meta.json`
      (ver [04-contrato-panel.md](04-contrato-panel.md))
- [ ] Revisar términos de uso antes de distribuir a terceros (automatizar sesiones de
      suscripción es zona gris; ver [02-decisiones.md](02-decisiones.md) → D4). **Más agudo desde
      abril de 2026**: hospedar el binario oficial está claramente dentro —él usa su propia sesión y
      nosotros no tocamos su token— pero **automatizarlo** desde fuera no está resuelto por escrito
- [ ] **Un puente entre opencode y `panel.json`** — su backend expone HTTP con eventos y su barra
      lateral ya calcula lo que el panel quiere pintar. Sería el segundo productor del contrato sin
      tocarle una línea a su código (ver [06-referencias.md](06-referencias.md))

~~Instalar `inotify-tools`~~ — cae con F1.6: el crate `notify` hace lo mismo y además en macOS
y Windows. Mientras el panel siga corriendo sobre WezTerm, el sondeo por `mtime` cada segundo
funciona y no molesta.

---

## Ideas sin fase

Cosas que gustan y **no tienen fase asignada**. No son deuda ni compromiso: están escritas para no
perderlas y para que nadie las cuele en una fase «ya que estamos». Si alguna entra algún día, será
por la puerta de delante — con su hueco en la Parte I y su decisión en `02-decisiones.md`.

### Tareas y tablero

**La idea.** Que una tarea sea una cosa con nombre propio, no solo «lo que hay corriendo en esa
división»: se crea antes de tener consola, se encola, se enlaza con otra («cuando termine esta,
arranca esa»), y se ve en un tablero por columnas. Es lo que hace Cline Kanban, y la parte de las
**cadenas de dependencias** es genuinamente buena — resuelve encadenar trabajo sin escribir un
script.

**Por qué no entra ahora.** Choca de frente con la forma de las fases 1–3, y el choque es de
fondo, no de calendario:

- Aquí la unidad es **la división** — una consola viva, con su `PANEL_ID` y su `panel.json`
  (D7). Una tarea encolada todavía no tiene consola, así que no tiene división, así que no tiene
  dónde publicar. Meter tareas obliga a inventar una segunda unidad por encima de la división, y
  eso es un modelo de datos nuevo, no una pantalla nueva.
- «Cuando termine esta, arranca esa» exige que la app **sepa** que algo terminó. Eso solo puede
  venir del contrato, publicado por quien trabaja — nunca deducido (D6). Antes de la Fase 3
  rodada no hay estado en el que confiar para disparar nada, y disparar procesos con una señal
  poco fiable es peor que no disparar.
- La Fase 4 ya cubre el 80% del valor sin nada de esto: **ordenar por lo que te reclama** es más
  útil que un tablero, y no necesita modelo de tareas — solo el estado que ya llega.

**Qué la haría entrar.** Que después de usar la Fase 4 un tiempo siga faltando algo, y ese algo
sea concreto: encolar trabajo que aún no tiene consola, o encadenar sin script. Si lo que falta se
puede resolver ordenando mejor la lista de la Fase 4, no hace falta un tablero.
