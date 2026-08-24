# Contrato — el documento de panel

Estado actual: **implementado y en uso en la aplicación**, y también en `prototipos/flow-panel` con
`flow` como primer productor. Lo de aquí es la especificación tal como funciona hoy, más lo que queda
pendiente al final.

**Este documento está partido en dos a propósito** ([02-decisiones.md](02-decisiones.md) → D17):

| | |
|---|---|
| **Parte I — El documento** | Qué se publica. Es el contrato: lo que se versiona y lo que nadie puede romper |
| **Parte II — Los transportes** | Cómo llega. Hoy un fichero; mañana puede llegar por más de uno |

Lo segundo no es la definición de lo primero. Un `panel.json` es **un transporte** del documento, no
el documento. La fuente de verdad es el estado que la aplicación tiene por división, y los
transportes lo alimentan.

---

# Parte I — El documento

## Forma

```json
{
  "v": 1,
  "updated_at": "2026-08-06T18:12:00+02:00",
  "phase": "validate",
  "attention": "wait",
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
| `updated_at` | recomendado | ISO 8601. **Se lee y todavía no se pinta** —como `link`, ver Pendiente— y **el aviso de rancio no sale de aquí** (ver reglas) |
| `phase` | no | Lo que se está ejecutando **ahora**. Texto libre, y se pinta literal — **dentro de la línea de identidad**, así que `header: false` también lo esconde |
| `attention` | no | **Si esta división reclama algo, y de qué clase.** `wait` `block` `done`. Ausente = no reclama nada. Ver abajo |
| `header` | no (por defecto `true`) | Si la app añade su línea de identidad arriba: el identificador de la división y su `phase` |
| `stale_after_minutes` | no (por defecto 30) | Cuánto silencio es legítimo antes de avisar |
| `lines` | **sí** | El cuerpo |

### `attention`

El único campo que la aplicación **interpreta**, y está acotado a la única pregunta que necesita
contestar para navegar: *¿tengo que ir ahí?* La decisión completa, con lo descartado, en
[02-decisiones.md](02-decisiones.md) → D18.

| Valor | Qué dice | Ejemplo |
|---|---|---|
| `wait` | **te espera a ti**: una pregunta, un permiso, una decisión | «¿apruebo el diseño?» |
| `block` | atascada por algo que no eres tú | «esperando que el contrato de otro servicio exista» |
| `done` | terminó y nadie lo ha mirado | el ciclo acabó y el diff sigue sin revisar |

- **Se acumula hacia arriba**: la pestaña muestra lo que más reclame de sus divisiones, con `wait`
  ganando a `block` y `block` a `done`. `wait` va primero porque es lo único que se desatasca contigo
  delante ahora mismo.
- **Ausente es ausente.** Sin este campo la división no se marca, y la app **no** lo deduce del
  cuerpo, ni de `phase`, ni de si la consola escribe. Una línea con `mark: block` es contenido del
  resumen, no una señal: un «esperé aprobación el martes» marcaría la pestaña para siempre.
- **No dice qué hace el programa.** Para «trabajando» o «en reposo» no hay campo y no lo va a haber:
  eso o lo deduce la app o lo declara un harness que no conoce la pregunta
  ([02-decisiones.md](02-decisiones.md) → D6).

### `lines`

Cada elemento es una línea. Cadena suelta → estilo normal. Cadena vacía → hueco. Objeto, con los
campos de abajo. **Un elemento que no sea ni cadena ni objeto —un número, un nulo— se descarta**, no
se pinta en blanco: una línea vacía donde había basura sería inventar contenido.

| Campo | Qué hace |
|---|---|
| `text` | El contenido. **Se corta a 512 caracteres al pintarlo**: es lo único que acota lo que cuesta maquetar una sola línea, y una de 260.000 cuesta un fotograma entero por sí sola |
| `mark` | `done` `current` `pending` `wait` `block` `info` — la app elige símbolo, color y alineación |
| `ref` | Etiqueta corta alineada en columna. **No tiene que ser un número**: `#1`, `Ahora`, `Contexto`. **Corta de verdad**: lo que pase del ancho de su columna se recorta, y la columna no puede empujar el texto fuera del panel |
| `style` | `normal` `dim` `title` `accent` `ok` `warn` `error` — pensado para líneas sin `mark`, pero **puede acompañar a una**: entonces la marca pone el símbolo y el estilo el color |
| `link` | URL. **Se lee y todavía no se pinta** — ver Pendiente |
| `indent` | Sangría manual, en celdas (heredada por las líneas de continuación). **Se acota a 64** al leer, y **al pintar también contra el ancho del panel**: una sangría de 40 no se respeta entera en un panel de 36, porque dejaría el texto fuera |

**Escribe frases, no columnas medidas.** La app envuelve al ancho, alinea por bloques (separados
por líneas vacías) y recorta al alto. Los espacios múltiples se colapsan: no sirven para alinear.

**Y los caracteres de control no llegan a pintarse**: un salto de línea, un tabulador o una secuencia
de escape dentro de `text` o de `ref` se sustituyen por un espacio. No es censura, es que descuadran
la rejilla del panel — un `\n` haría que una sola línea pintara varias filas encima de las de abajo.
Quien quiera dos líneas, que publique dos elementos en `lines`.

## Reglas del documento

- **Lo que no se entienda se ignora, campo por campo.** Un `stale_after_minutes` mal escrito no tira el
  documento entero: ese campo cae en su valor por omisión y el resto se pinta. Del **contenido**, solo hay
  dos formas de que un documento no valga: que no sea JSON —lo que incluye el fichero a medio escribir y
  el que no es texto UTF-8— o que no traiga `lines`.
- **El cuerpo es libre por diseño: no hay campos de métrica, y no se añaden.** Tokens, coste o
  contexto restante se escriben como una línea más —`{"text": "35.392 tokens · 18% · 0,62 €",
  "mark": "info", "ref": "Contexto"}`— nunca como campos del formato. La razón: en cuanto el
  contrato nombra una métrica, la app pasa a ser dueña de qué merece verse, y el productor pierde lo
  único que lo hace valioso aquí — poder publicar **lo que su tarea necesite**, incluido lo que al
  usuario de esa división se le ocurra pedirle a su agente, sin esperar una versión nueva del
  formato. La app solo entiende de marcas, estilos, alineación y `attention`; el vocabulario es de
  quien trabaja. Las métricas de la IA son un caso de uso del contrato, no su razón de ser
  ([07-panorama.md](07-panorama.md) → Conductor, que las enseña y sigue sin tener este panel).
- **`phase` es el texto que se muestra**; si el trabajo está cerrado, eso lo decide el estado
  real, no `phase`. Un `phase` fuera de cualquier ciclo conocido (`watching`, `desplegando`) es
  perfectamente válido.
- **Publica *antes* de un tramo largo, no después.** Si el proceso muere a mitad, el documento
  dice qué estaba haciendo y la marca de tiempo lo delata. Un resumen escrito solo al terminar
  bien miente por omisión.
- **Aviso de rancio**: si pasa `stale_after_minutes` sin refrescarse, la app lo señala **en una línea
  suya dentro del panel**, y esa línea no depende de `header` — esa bandera esconde la identidad de la
  división, no la honestidad de la aplicación. **Sin excepciones, y eso es deliberado.** El prototipo se
  callaba cuando el trabajo estaba cerrado, pero para saberlo tenía que leer la fase de un `meta.json`
  ajeno y decidir por su cuenta qué significa — justo lo que D6 prohíbe. Quien no quiera el aviso lo
  dice en su propio documento: `stale_after_minutes` muy alto al publicar por última vez. **De dónde
  sale la hora de la última publicación lo define cada transporte**; para el fichero, ver la Parte II.
- **Y la app dice también cuándo no ha podido leer.** Publicar algo que no se entiende no borra lo
  anterior, pero tampoco pasa desapercibido: el panel enseña lo último bueno con una línea encima que
  dice que eso no es lo último y por qué. Si **nunca** hubo nada bueno, el panel es solo esa línea. La
  única excepción es que no haya llegado nada todavía, que es el estado normal antes de la primera
  publicación: ahí no hay panel ni aviso, y la división se ve como una consola cualquiera.

---

# Parte II — Los transportes

Cuál existe y por qué, con los cinco candidatos comparados y los tres descartados, en
[02-decisiones.md](02-decisiones.md) → D17.

| Transporte | Estado | Para quién es |
|---|---|---|
| **Fichero** (`panel.json`) | **funcionando** | Todo el mundo. El listón de entrada es cero: `cat >` desde un script, una escritura desde un agente |
| **MCP** (`panel_set`) | Fase 3 | El agente que además quiere pilotar la app. Entrega el documento **sin escribir el fichero** |
| **OSC por el PTY** | candidato, después de F2.7 | El que está al otro lado de un `ssh` o dentro de un contenedor sin volumen montado |

**Y cada entrega trae fuente y orden**, para que dos productores sobre la misma división no se
decidan por azar. **Cómo se expresan está sin decidir** — D17 lo deja abierto a propósito, porque con
un solo transporte vivo sería inventar la forma antes de tener el segundo caso delante.

## Transporte 1 — el fichero

| Quién lee | Dónde busca |
|---|---|
| La aplicación | **`$PANEL_DIR/panel.json`**, el directorio que le da a cada consola al lanzarla |
| El prototipo sobre WezTerm | `.claude/work/<trabajo>/panel.json`, por convención de `flow` |

El prototipo descubre el fichero por convención; la app lo hace por identidad de división (ver
[02-decisiones.md](02-decisiones.md) → D7), desde F1.5 para el directorio y F1.6 para la lectura. **El
documento es el mismo**, solo cambia dónde se busca.

### Reglas de este transporte

- **Machaca el fichero entero** en cada escritura. No acumula.
- **Quien pueda, escribe a un fichero temporal en el mismo directorio y lo renombra encima.** Así la
  app nunca lee un fichero a medias. **Es una recomendación, no un requisito**: el primer productor de
  esto es un agente que machaca el fichero y no puede hacer otra cosa, y un requisito que su primer
  productor no puede cumplir es un requisito muerto. La app **no se rompe** con una lectura parcial: se
  queda con el último panel bueno **y dice en el propio panel que lo que se ve no es lo último**, así
  que no parpadea pero tampoco disimula. La recomendación sigue mereciendo la pena para quien tenga
  código propio —un CI, un script, un servidor MCP—: **le ahorra a su panel esos avisos**, que son
  correctos pero no dicen nada bueno de quien publica.
- **Publica de una vez.** Desde F1.6 la app se entera de que has publicado por los avisos del sistema
  de ficheros, y **cada escritura suelta es un aviso**: un productor que construye su `panel.json` a
  trozos —lo natural con `>>` en un bucle de shell— hace que la app lo lea a medio escribir una y otra
  vez. **Medido**: unas **42 lecturas y otros tantos fotogramas por publicación**, contra **uno** si se
  escribe de una vez o con el temporal-y-renombre de arriba. Y lo que se ve mientras tanto es el aviso de «esto no es lo
  último» apareciendo y desapareciendo con cada trozo. **El derroche de fotogramas sí es invisible**, y
  por eso está escrito aquí: nadie lo va a descubrir mirando.
- **Un fichero de más de 256 KiB no se lee entero.** El techo acota lo que un fichero de fuera puede
  hacer reservar al interpretarse, y de paso cuántas líneas y de qué largo tiene sentido publicar.
  **Un aviso para quien publique mucho, con las cifras medidas**: un fichero justo en el techo cuesta
  unos 2,8 ms y 1,8 MiB de pico si sus líneas se parecen a las de un panel de verdad, pero **22 ms y
  24 MiB** —un fotograma y medio— si son treinta mil objetos diminutos. El caso normal ni se acerca: un
  `panel.json` como los que se escriben hoy se lee en **10 µs**. Publicar cerca del techo se nota;
  publicar muchas líneas cortas se nota el doble.
- **Cuatro formas de que no se llegue a leer**, aparte de las dos del documento: que no exista
  todavía; que lo que hay ahí **no sea un fichero normal** —un directorio, una tubería con nombre: se
  rechaza sin abrirlo, porque abrirla congelaría la ventana—; que **no se pueda abrir o leer**, que en
  la práctica es cuestión de permisos; o que pase del techo de tamaño, y ese se lee hasta el techo
  antes de rechazarlo.
- **El aviso de rancio se mide por la fecha del fichero**, no por `updated_at`. Como el productor
  machaca el fichero entero, esa fecha *es* la de su última publicación — y así hay aviso incluso cuando
  el productor no escribe su marca, que es justo cuando más falta hace. `updated_at` se pinta como lo
  que es: lo que el productor dice.

### Lo que este transporte no da

- **No sabe quién escribe.** Cualquiera que pueda escribir en el directorio *es* el productor.
- **No le dice al productor que su documento está roto.** El error se ve en el panel del usuario, no
  lo ve quien publica — y eso es lo que hace caro escribir un productor nuevo, que es justo lo que
  este proyecto necesita que sea barato.

---

## Pendiente

- [ ] `actions` — botones declarados: `{"label": "Lanzar tests", "send": "npm test\n"}` /
      `{"label": "Ver MR", "open": "https://…"}`. Ver riesgos en
      [03-arquitectura.md](03-arquitectura.md) → Seguridad
- [ ] **Cómo se expresan la fuente y el orden de cada entrega** (D17). Herdr lo resuelve con un
      `--source` estable por integración, que da autoridad y se suelta al salir, y un `--seq`
      estrictamente creciente con el que descarta lo que llega tarde
      ([07-panorama.md](07-panorama.md) → E8). Aquí no hay ni uno ni otro, y hará falta en cuanto MCP
      entregue documentos junto al fichero
- [ ] **El aviso del sistema al pasar a `wait`.** El campo `attention` ya dice cuándo una división
      reclama (D18) y el indicador de la pestaña ya lo acumula; falta decidir si además se interrumpe
      al usuario, y eso se deja para cuando el campo tenga productores: una interrupción falsa cuesta
      mucho más que un indicador falso
- [x] **El cambio de estado como algo más que pintura** — **resuelto por D18**: el productor lo
      declara en `attention` en vez de que la app lo interprete del cuerpo, que era la duda que esta
      línea dejaba abierta
- [x] **Descubrimiento por `PANEL_DIR`** en lugar de convención de `flow` — **hecho en F1.6**. La
      identidad de cada división y su directorio existen desde F1.5; F1.6 añadió leer lo que se publique
      ahí, pintarlo, enterarse solo de que ha cambiado y decir cuándo está viejo
- [ ] **`link` y `updated_at`, que hoy se leen y no se pintan.** De `updated_at` falta decidir si vale
      la pena enseñar lo que el productor *dice* teniendo ya el aviso que sale de la fecha real del
      fichero. `link` salió del hito F1.6 entero a propósito: es el único campo que **abriría** algo
      venido de un documento que puede ser de origen ajeno —un repo clonado, un contenedor—, así que su
      pintado y el saneo de la dirección que lo acompaña merecen su propia revisión
- [x] Que `header: false` no dependa de que exista un `meta.json` al lado — **hecho en F1.6**: la
      bandera se lee del propio `panel.json`, su valor por omisión no mira a ningún fichero vecino, y
      quien pinta la respeta. **Lo que no esconde es el aviso de la aplicación**, y eso es deliberado
