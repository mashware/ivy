# Arquitectura

## Las piezas

```
┌─ Ventana ──────────────────────────────────────────────────┐
│  Pestañas                                                   │
│  ┌─ División ────────────────┬─ División ─────────────────┐ │
│  │  Terminal    │  Panel     │  Terminal   │  Panel       │ │
│  │  (PTY)       │            │  (PTY)      │              │ │
│  │  PANEL_ID=a3f2           │  PANEL_ID=b7c1              │ │
│  └───────────────────────────┴────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
         ▲                ▲                    ▲
         │ PTY            │ lee panel.json     │ MCP
         │                │                    │
   botones/teclado   $PANEL_DIR/panel.json   servidor MCP
                            ▲
                            │ escribe
                     el agente, flow, un CI, un script…
```

## Los tres canales

### 1. Estado — el documento de panel (productor → app)

Lo publica quien hace el trabajo. **La fuente de verdad es el estado que la app tiene por división**,
y los transportes lo alimentan ([02-decisiones.md](02-decisiones.md) → D17):

| Transporte | Estado | Para quién |
|---|---|---|
| **Fichero** — `$PANEL_DIR/panel.json`, machacado entero, observado por la app | funcionando | todo el mundo: el listón de entrada es cero |
| **MCP** — `panel_set` entrega el documento sin escribir el fichero | Fase 3 | el agente que además pilota la app |
| **OSC por el PTY** | candidato, tras F2.7 | el que está tras un `ssh` o en un contenedor sin volumen |

Especificación completa: [04-contrato-panel.md](04-contrato-panel.md).

**Por qué el fichero es el de serie:** es el único donde el productor más tonto posible publica sin
escribir código —`cat >` desde un script— y el único donde el estado sobrevive a que el productor
muera sin que nadie programe nada. Además se lee con `cat` y cruza un volumen montado sin protocolo
propio. **Lo que no da:** no sabe quién escribe, y no le dice a quien publica que su documento estaba
roto.

### 2. Control — MCP (agente → app)

Servidor MCP que expone herramientas al agente que corre en la división:

| Herramienta | Qué hace |
|---|---|
| `panel_set` | Publicar el estado de su división |
| `split_open` | Abrir una división nueva |
| `split_read` | Leer la salida de una división |
| `split_close` | Cerrarla |

**Regla de seguridad:** por defecto, un agente solo alcanza **su propia división** (la de su
`PANEL_ID`). Ampliar ese alcance es una decisión explícita del usuario, no el comportamiento
por defecto.

### 3. Entrada — PTY (app → agente)

Los botones del panel escriben en el PTY de su división. Es el respaldo universal: funciona con
cualquier programa, hable MCP o no.

**Riesgo asumido:** un fichero que declara comandos y un botón que los ejecuta es ejecución de
código en el shell del usuario. Ver Seguridad, abajo.

---

## Lectura de la salida

Para lo que no habla MCP, hay que leer la pantalla. La pieza que lo hace viable son las
**marcas de shell (OSC 133)**: delimitan dónde empieza el prompt, dónde el comando y dónde su
salida. Sin ellas la app solo ve un río de texto; con ellas puede acotar «la salida del último
comando».

Se activan cargando el script de integración del shell (WezTerm ya trae uno; en la app propia
será equivalente).

**Lo que no se debe hacer:** parsear el pintado de una interfaz de texto para deducir estado.
Se rompe en cada versión del agente. Para eso está el canal 1.

---

## Seguridad

| Riesgo | Mitigación |
|---|---|
| `actions` del panel ejecutan comandos en el shell | El comando debe verse antes de ejecutarse; `send` es texto plano sin interpolación ni plantillas |
| Un `panel.json` de origen no confiable (repo clonado, contenedor) | Tratar el fichero como no confiable; nunca ejecutar nada sin confirmación visible |
| Un agente escribiendo en la división de otro | El servidor MCP acota por `PANEL_ID`. **Por el fichero no hay acotado**: quien pueda escribir en el directorio es el productor (D17) |
| Leer el buffer y mandarlo a un modelo | Exfiltra lo que haya en pantalla: tokens, claves, respuestas de API. La selección de qué se envía debe ser explícita, nunca automática |
| Definir `ANTHROPIC_API_KEY` al lanzar un panel | Prohibido: cambiaría la facturación del usuario (ver D4) |
| Secretos en el panel | El panel es texto persistido; no es sitio para credenciales |
