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

### 1. Estado — `panel.json` (productor → app)

Un fichero por división, en `$PANEL_DIR/panel.json`. Lo escribe quien hace el trabajo,
machacándolo entero. La app lo observa y repinta al cambiar.

Especificación completa: [04-contrato-panel.md](04-contrato-panel.md).

**Por qué fichero y no socket:** aislamiento de fallos (si el productor muere, el panel sigue
mostrando el último estado bueno), inspeccionable con `cat`, y funciona a través de SSH sin
protocolo propio.

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
| Un agente escribiendo en la división de otro | El servidor MCP acota por `PANEL_ID` |
| Leer el buffer y mandarlo a un modelo | Exfiltra lo que haya en pantalla: tokens, claves, respuestas de API. La selección de qué se envía debe ser explícita, nunca automática |
| Definir `ANTHROPIC_API_KEY` al lanzar un panel | Prohibido: cambiaría la facturación del usuario (ver D4) |
| Secretos en el panel | El panel es texto persistido; no es sitio para credenciales |
