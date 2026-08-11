# Contrato — `panel.json`

Estado actual: **implementado y en uso** en `prototipos/flow-panel`, con `flow` como primer
productor. Lo de aquí es la especificación tal como funciona hoy, más lo pendiente de
generalizar.

## Ubicación

| Hoy (prototipo) | Objetivo |
|---|---|
| `.claude/work/<trabajo>/panel.json` | `$PANEL_DIR/panel.json` |

El prototipo descubre el fichero por convención de `flow`; la app lo hará por identidad de
división (ver [02-decisiones.md](02-decisiones.md) → D7). **El formato no cambia**, solo dónde
se busca.

## Forma

```json
{
  "v": 1,
  "updated_at": "2026-08-06T18:12:00+02:00",
  "phase": "validate",
  "header": true,
  "stale_after_minutes": 30,
  "lines": [
    "una línea suelta",
    {"text": "las dos fuentes de lectura por lote", "mark": "done", "ref": "#1",
     "link": "https://gitlab.com/…/9977"},
    {"text": "el mapeo a la forma del canal", "mark": "current", "ref": "#3"},
    {"text": "35.392 tokens · 18% · 0,62 €", "mark": "info", "ref": "Contexto"},
    "",
    {"text": "mailsuite-mcp espera el contrato", "mark": "block"}
  ]
}
```

## Campos

| Campo | Obligatorio | Qué hace |
|---|---|---|
| `v` | recomendado | Versión del contrato |
| `updated_at` | recomendado | ISO 8601. Sin él no hay aviso de resumen rancio |
| `phase` | no | Lo que se está ejecutando **ahora**. Texto libre |
| `header` | no (por defecto `true`) | Si la app añade su línea de identidad arriba |
| `stale_after_minutes` | no (por defecto 30) | Cuánto silencio es legítimo antes de avisar |
| `lines` | **sí** | El cuerpo |

### `lines`

Cada elemento es una línea. Cadena suelta → estilo normal. Cadena vacía → hueco. Objeto:

| Campo | Qué hace |
|---|---|
| `text` | El contenido |
| `mark` | `done` `current` `pending` `wait` `block` `info` — la app elige símbolo, color y alineación |
| `ref` | Etiqueta corta alineada en columna. **No tiene que ser un número**: `#1`, `Ahora`, `Contexto` |
| `style` | `normal` `dim` `title` `accent` `ok` `warn` `error` — para líneas sin `mark` |
| `link` | URL. La app la pega a la derecha o la cuelga debajo, como enlace corto y clicable |
| `indent` | Sangría manual (heredada por las líneas de continuación) |

**Escribe frases, no columnas medidas.** La app envuelve al ancho, alinea por bloques (separados
por líneas vacías) y recorta al alto. Los espacios múltiples se colapsan: no sirven para alinear.

## Reglas de comportamiento

- **Machaca el fichero entero** en cada escritura. No acumula.
- **Escribe *antes* de un tramo largo, no después.** Si el proceso muere a mitad, el fichero
  dice qué estaba haciendo y la marca de tiempo lo delata. Un resumen escrito solo al terminar
  bien miente por omisión.
- **`phase` es el texto que se muestra**; si el trabajo está cerrado, eso lo decide el estado
  real, no `phase`. Un `phase` fuera de cualquier ciclo conocido (`watching`, `desplegando`) es
  perfectamente válido.
- **Aviso de rancio**: si pasa `stale_after_minutes` sin refrescarse, la app lo señala — salvo
  que el trabajo esté cerrado, donde un resumen viejo es correcto.
- **El cuerpo es libre por diseño: no hay campos de métrica, y no se añaden.** Tokens, coste o
  contexto restante se escriben como una línea más —`{"text": "35.392 tokens · 18% · 0,62 €",
  "mark": "info", "ref": "Contexto"}`— nunca como campos del formato. La razón: en cuanto el
  contrato nombra una métrica, la app pasa a ser dueña de qué merece verse, y el productor pierde lo
  único que lo hace valioso aquí — poder publicar **lo que su tarea necesite**, incluido lo que al
  usuario de esa división se le ocurra pedirle a su agente, sin esperar una versión nueva del
  formato. La app solo entiende de marcas, estilos y alineación; el vocabulario es de quien trabaja.
  Las métricas de la IA son un caso de uso del contrato, no su razón de ser
  ([07-panorama.md](07-panorama.md) → Conductor, que las enseña y sigue sin tener este panel).

## Pendiente

- [ ] `actions` — botones declarados: `{"label": "Lanzar tests", "send": "npm test\n"}` /
      `{"label": "Ver MR", "open": "https://…"}`. Ver riesgos en
      [03-arquitectura.md](03-arquitectura.md) → Seguridad
- [ ] **El cambio de estado como evento, no solo como pintura.** El contrato ya sabe expresar
      «esperando» y «bloqueado» (`mark: wait`, `mark: block`), pero la app se limita a dibujarlos:
      quien no está mirando esa división no se entera. Xirp resuelve «a qué división voy ahora»
      **notificando** al pasar a *esperando entrada*, sin necesidad de tener las cuatro a la vista
      ([07-panorama.md](07-panorama.md) → E3). Falta decidir qué transición dispara aviso —
      la aparición de un `wait`/`block`, o un campo explícito del productor— y que lo decida **el
      productor**, no la app interpretando el cuerpo del resumen
      ([02-decisiones.md](02-decisiones.md) → D6). Sin esto, el panel solo sirve mientras se mira
- [ ] Descubrimiento por `PANEL_DIR` en lugar de convención de `flow`
- [ ] Que `header: false` no dependa de que exista un `meta.json` al lado
