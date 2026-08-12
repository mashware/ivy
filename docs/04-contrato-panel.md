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
| `updated_at` | recomendado | ISO 8601. Se pinta tal cual; **el aviso de rancio no sale de aquí** (ver reglas) |
| `phase` | no | Lo que se está ejecutando **ahora**. Texto libre, y se pinta literal — **dentro de la línea de identidad**, así que `header: false` también lo esconde |
| `header` | no (por defecto `true`) | Si la app añade su línea de identidad arriba: el identificador de la división y su `phase` |
| `stale_after_minutes` | no (por defecto 30) | Cuánto silencio es legítimo antes de avisar |
| `lines` | **sí** | El cuerpo |

### `lines`

Cada elemento es una línea. Cadena suelta → estilo normal. Cadena vacía → hueco. Objeto, con los
campos de abajo. **Un elemento que no sea ni cadena ni objeto —un número, un nulo— se descarta**, no
se pinta en blanco: una línea vacía donde había basura sería inventar contenido.

| Campo | Qué hace |
|---|---|
| `text` | El contenido |
| `mark` | `done` `current` `pending` `wait` `block` `info` — la app elige símbolo, color y alineación |
| `ref` | Etiqueta corta alineada en columna. **No tiene que ser un número**: `#1`, `Ahora`, `Contexto` |
| `style` | `normal` `dim` `title` `accent` `ok` `warn` `error` — pensado para líneas sin `mark`, pero **puede acompañar a una**: entonces la marca pone el símbolo y el estilo el color |
| `link` | URL. **Se lee y todavía no se pinta** — ver Pendiente |
| `indent` | Sangría manual, en celdas (heredada por las líneas de continuación). **Se acota a 64**: más no cabría en ningún panel, y quien pinta repetiría tantos espacios como dijera el fichero |

**Escribe frases, no columnas medidas.** La app envuelve al ancho, alinea por bloques (separados
por líneas vacías) y recorta al alto. Los espacios múltiples se colapsan: no sirven para alinear.

**Y los caracteres de control no llegan a pintarse**: un salto de línea, un tabulador o una secuencia
de escape dentro de `text` o de `ref` se sustituyen por un espacio. No es censura, es que descuadran
la rejilla del panel — un `\n` haría que una sola línea pintara varias filas encima de las de abajo.
Quien quiera dos líneas, que publique dos elementos en `lines`.

## Reglas de comportamiento

- **Machaca el fichero entero** en cada escritura. No acumula.
- **Quien pueda, escribe a un fichero temporal en el mismo directorio y lo renombra encima.** Así la
  app nunca lee un fichero a medias. **Es una recomendación, no un requisito**: el primer productor de
  esto es un agente que machaca el fichero y no puede hacer otra cosa, y un requisito que su primer
  productor no puede cumplir es un requisito muerto. La app **no se rompe** con una lectura parcial: la
  descarta y lo dice. Quedarse con el último panel bueno mientras el fichero no se entienda es lo que
  falta por hacer — hasta entonces lo que se ve puede parpadear, y esta recomendación es lo que se lo
  evita a quien tenga código propio: un CI, un script, un servidor MCP.
- **Un fichero de más de 256 KiB no se lee entero.** El techo acota lo que un fichero de fuera puede
  hacer reservar al interpretarse, y de paso cuántas líneas y de qué largo tiene sentido publicar.
  **Un aviso para quien publique mucho, con las cifras medidas**: un fichero justo en el techo cuesta
  unos 2,8 ms y 1,8 MiB de pico si sus líneas se parecen a las de un panel de verdad, pero **22 ms y
  24 MiB** —un fotograma y medio— si son treinta mil objetos diminutos. El caso normal ni se acerca: un
  `panel.json` como los que se escriben hoy se lee en **10 µs**. Publicar cerca del techo se nota;
  publicar muchas líneas cortas se nota el doble.
- **Lo que no se entienda se ignora, campo por campo.** Un `stale_after_minutes` mal escrito no tira el
  fichero entero: ese campo cae en su valor por omisión y el resto se pinta. Del **contenido**, solo hay
  dos formas de que un fichero no valga: que no sea JSON —lo que incluye el fichero a medio escribir y
  el que no es texto UTF-8— o que no traiga `lines`. Aparte quedan las tres formas de que ni siquiera se
  llegue a leer: que no exista todavía, que lo que hay ahí **no sea un fichero normal** —un directorio,
  una tubería con nombre: se rechaza sin abrirlo, porque abrirla congelaría la ventana— o que pase del
  techo de tamaño.
- **El aviso de rancio se mide por la fecha del fichero**, no por `updated_at`. Como el productor
  machaca el fichero entero, esa fecha *es* la de su última publicación — y así hay aviso incluso cuando
  el productor no escribe su marca, que es justo cuando más falta hace. `updated_at` se pinta como lo
  que es: lo que el productor dice.
- **Escribe *antes* de un tramo largo, no después.** Si el proceso muere a mitad, el fichero
  dice qué estaba haciendo y la marca de tiempo lo delata. Un resumen escrito solo al terminar
  bien miente por omisión.
- **`phase` es el texto que se muestra**; si el trabajo está cerrado, eso lo decide el estado
  real, no `phase`. Un `phase` fuera de cualquier ciclo conocido (`watching`, `desplegando`) es
  perfectamente válido.
- **Aviso de rancio**: si pasa `stale_after_minutes` sin refrescarse, la app lo señala. **Sin
  excepciones, y eso es deliberado.** El prototipo se callaba cuando el trabajo estaba cerrado, pero
  para saberlo tenía que leer la fase de un `meta.json` ajeno y decidir por su cuenta qué significa —
  justo lo que D6 prohíbe. Quien no quiera el aviso lo dice en su propio fichero: `stale_after_minutes`
  muy alto al publicar por última vez.
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
- [~] Descubrimiento por `PANEL_DIR` en lugar de convención de `flow` — **en marcha en F1.6**. La
      identidad de cada división y su directorio ya existen desde F1.5; lo que este hito añade es leer
      lo que se publique ahí y pintarlo
- [ ] **`link` clicable.** Hoy se lee y se guarda, y no se pinta. Sale del hito F1.6 entero a
      propósito: es el único campo que abriría algo venido de un fichero que puede ser de origen ajeno
      —un repo clonado, un contenedor—, así que su pintado y el saneo de la dirección que lo acompaña
      merecen su propia revisión
- [~] Que `header: false` no dependa de que exista un `meta.json` al lado — **resuelto en la lectura**
      desde F1.6: la bandera se lee del propio `panel.json` y su valor por omisión no mira a ningún
      fichero vecino. Queda que quien pinte la línea de identidad la respete
