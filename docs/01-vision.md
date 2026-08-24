# Visión

## Qué es

Un terminal que hospeda agentes. Tres niveles:

```
Ventana
└── Pestañas
    └── Divisiones          ← cada una es una consola independiente
        ├── Terminal        ← donde corre el agente (o lo que sea)
        └── Panel           ← estado de esa tarea, alimentado desde fuera
```

El panel no lo escribe la app. Lo escribe **quien esté haciendo el trabajo**, mediante un
contrato abierto. La app solo lo pinta. Hoy ese contrato llega por un fichero, pero **el transporte
no es el contrato** ([02-decisiones.md](02-decisiones.md) → D17): el documento es lo que se versiona,
y puede llegar también por MCP o por el propio terminal.

## Para quién

Gente que trabaja con varios agentes de terminal en paralelo y necesita saber, de un vistazo:

- cuál está trabajando y cuál la espera
- por qué paso va cada tarea
- a cuál le queda poco contexto
- cuánto lleva gastado cada una
- qué está bloqueado y por qué

## Contra quién se compite

**Contra emuladores de terminal** —WezTerm, Ghostty, Kitty, iTerm2, Windows Terminal—, no contra
orquestadores de agentes ([02-decisiones.md](02-decisiones.md) → **D19**). El usuario a convencer es
el que ya tiene uno de esos abierto con agentes dentro.

Eso decide el listón: hay que darle lo que da por hecho —scrollback, copiar, buscar, **sus fuentes**,
**poder configurarlo**, el ratón dentro de `vim`, y que corra en su máquina— y **nada de la liga de
al lado**: ni navegador integrado, ni app móvil, ni editor, ni tablero. Contra terminales el panel no
tiene competencia: ninguno tiene estado por división. Contra orquestadores serías el mismo producto
con menos cosas.

Las funciones de entorno de desarrollo son **la evolución, no el precio de entrada**, y llegan si
esto tiene usuarios.

## Qué lo diferencia

Contrastado contra ocho competidores en agosto de 2026
([07-panorama.md](07-panorama.md) → «En qué nos diferenciamos»). Queda **una** cosa, no varias:

**El cuerpo del panel lo decide quien trabaja, y es estado estructurado y persistente.** Los
orquestadores completos —Xirp, Superset, Conductor, Warp, Orca— tienen estado rico, pero con un
vocabulario que elige la app: tres constantes, cuatro eventos, cinco estados, seis indicadores, unas
métricas. cmux sí abre el canal —cualquier proceso le emite OSC 777/99— pero por ahí solo caben un
título y un cuerpo que **se borran al leerse**. **Y Herdr, que es el que más lejos llega, deja
publicar a cualquiera —`--source custom:*`, documentado— pero lo que cabe son cuatro estados suyos y
pares clave-valor «solo para mostrar».** Nadie tiene lo de en medio: una lista con marcas,
referencias y enlaces, escrita en el vocabulario del productor, que sigue ahí cuando vuelves y que
sabe caducar. Eso es lo que permite cuatro divisiones con cuatro productores distintos y una vista
global que los resume.

**Lo que ya NO diferencia, y estaba escrito aquí: preservar las suscripciones.** Lo hacen los ocho
—todos lanzan el binario oficial del agente y ninguno cobra API aparte—. Sigue siendo la decisión
correcta ([02-decisiones.md](02-decisiones.md) → D4) y una condición para que esto se adopte, pero no
es un argumento que distinga nada.

## Qué NO es

- **No es un agente.** No razona, no llama a modelos, no tiene opinión sobre el trabajo.
- **No es un multiplexor de sesiones remotas.** Eso es tmux/Zellij y no compite con ellos.
- **No reescribe el emulador de terminal.** Ese motor se consume como librería.

## Principios

1. **La app no sabe de dominios.** No conoce `flow`, ni git, ni tickets. Solo el contrato.
2. **Quien hace el trabajo publica el estado.** La app nunca deduce ni parsea para adivinar.
3. **Que funcione sin el panel.** Si nadie publica nada, sigue siendo un terminal correcto.
4. **La información que no se puede deducir desde fuera vale más que la que sí.** Por qué paso va la
   tarea, qué la tiene bloqueada, y lo que el usuario de esa división quiera publicar jugando con su
   agente: eso solo lo sabe quien trabaja. Las métricas de la IA —contexto restante, coste— son un
   **caso de uso** de esto, no su definición; ver la regla del cuerpo libre en
   [04-contrato-panel.md](04-contrato-panel.md).

---

## ¿Merece la pena? — valoración de agosto de 2026

Escrita después de analizar ocho competidores. Se guarda porque la pregunta va a volver, y porque
tiene dos mitades con respuestas distintas que conviene no confundir.

### Cuánto tardarían en copiarlo: días

Leer un JSON de un directorio conocido y pintar una lista con marcas no tiene dificultad técnica
ninguna. Cualquiera de los ocho lo tiene funcionando en una tarde y pulido en una semana. **No hay
foso técnico, y creerse lo contrario sería el peor error de este proyecto.**

### Por qué ninguno lo ha hecho ya

Los ocho han tenido la oportunidad. Warp lleva años y su lista de agentes está **compilada en el
binario** como un `enum`. Conductor no tiene ni una vía de vuelta. Orca lee el título del terminal, y
por ahí ya le entra cualquiera, pero solo para decir una palabra suya. No es incapacidad: es que su
producto empuja al contrario.

1. **Un contrato abierto degrada su propuesta.** Los que venden producto —Warp, Xirp, Superset,
   Conductor, Orca— venden lo mismo: *«nosotros entendemos a tu agente»*. Un formato donde cualquiera publica lo que quiera los convierte en **un visor de ficheros
   ajenos**. Lo primero se vende; lo segundo es infraestructura, y la infraestructura la regala quien
   no tiene que monetizarla.
2. **Un contrato es un compromiso, no una feature.** Hay que versionarlo y no romperlo nunca. Un
   equipo que itera cada semana huye de eso — por eso Warp eligió un `enum` en el binario antes que una
   superficie estable, y no es descuido: es elección.
3. **Obliga a renunciar a ser dueño de la semántica.** Si el productor escribe mal, la app se ve mal.
   Ningún equipo de producto acepta que la calidad de su interfaz dependa de terceros, salvo que eso
   sea precisamente su tesis.

**La ventaja no es que no puedan: es que no quieren.** Eso aguanta más que una ventaja técnica, y
mucho menos que una patente.

> **Y hay un contraejemplo que hay que mirar de frente: Herdr.** No vende «entendemos a tu agente»,
> vende ser el sustrato donde corren; regala la infraestructura porque es su tesis, no su coste; y
> **ya abrió el canal a productores que no son agentes, documentado**. Los tres frenos de arriba no
> le aplican a él. Lo que le frena es otra cosa, y es estructural: **vive dentro de un TUI, donde el
> sitio donde pintar un cuerpo es una fila de texto**. Por eso su barra lateral contesta «a qué
> división voy» mejor que la nuestra y no intenta contestar «por dónde va». Mover eso le cuesta el
> producto entero, igual que a Orca le costaría salir de Electron.

### El riesgo real, y no es el copiado

Que la necesidad **se resuelva por otro camino**: que los harnesses publiquen progreso estructurado
de forma nativa. Si Claude Code, Codex y compañía empiezan a emitir paso actual, plan y bloqueo por
sus propios hooks, los demás tendrán el 80 % del contenido gratis por el stream, y este contrato queda
para el 20 % restante — el CI, el despliegue, el script propio.

Ese escenario es **más probable** que «Warp lee `panel.json`». Y la defensa no es técnica: es tener
productores escribiendo antes de que ocurra.

### Las dos preguntas

**Para uso propio: sí, con margen.** Los ocho productos completos no hacen esto —verificado producto
a producto, no intuido—. El argumento de que «tres ni corren en Linux» **ya no vale**: Orca y Herdr
corren en las tres plataformas, son gratis y están muy pulidos. Lo que queda es el cuerpo del panel,
no la disponibilidad. F1.0 a F1.3 están cerrados, el terminal se
usa y el prototipo del panel funciona: queda menos por delante de lo que ya se lleva hecho.

**Como producto: frágil, y hay que decirlo.** Competir con Warp —multiplataforma, open source, 64k
estrellas—, con Orca —MIT, 52k, empujes diarios— y con Herdr —Apache-2.0, 32k en cinco meses—
apoyándose en una decisión de diseño copiable en una tarde no es imposible, pero no es un plan de
negocio. Que la diferenciación tenga que ser evidente en treinta segundos ya estaba escrito en
el panorama antes de esta ronda de análisis; sigue valiendo, y ahora con menos margen.

### Lo único que puede convertirse en foso

**Que el formato tenga productores.** Sin adopción es un fichero JSON; con adopción es un estándar, y
entonces el valor deja de estar en la app — es la jugada del LSP y de Backstage: quien define el
protocolo que otros implementan gana aunque su cliente no sea el mejor.

Aquí ya hay uno real: **`flow` publica hoy**, en el prototipo. Warp tiene cinco agentes integrados a
mano; esto tiene uno que publica por su voluntad. Es poco, pero es de otra naturaleza.

### La prueba que queda pendiente, y es la que decide

No más análisis de competencia — con ocho está contestado. **Usar el panel con contenido real una
semana de trabajo y ver si cambia el día.** No una demostración: uso propio, con `flow` publicando.

- Si lo cambia, la prioridad pasa a que escriban más productores, no a pulir el terminal.
- Si no lo cambia, se ha averiguado con dos hitos de trabajo en vez de con diez, y queda un terminal
  que funciona.

Lo que **no** vale es seguir puliendo el terminal esperando que el diferenciador se aclare solo. No se
va a aclarar más de lo que está hoy, y cada mes que pasa es un mes en que la idea puede aparecérsele a
otro — que es exactamente lo que ha ocurrido **diez** veces en agosto de 2026 con otras piezas de esta
propuesta, la última de ellas —que el estado lo declare quien trabaja en vez de deducirlo la app— con
Herdr.
