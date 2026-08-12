---
id: asp_el-rancio-sale-de-la-fecha-del-fichero_737kmc
slug: el-rancio-sale-de-la-fecha-del-fichero
name: >-
  El aviso de rancio sale de la fecha del fichero y no de updated_at — y por eso
  un panel.json copiado se pinta como fresco
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:08:09.474Z'
updated_at: '2026-08-12T14:08:09.474Z'
last_verified: '2026-08-12T14:08:09.474Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - docs/04-contrato-panel.md
symbols:
  - 'Panel::written_at'
  - 'Panel::is_stale_at'
  - updated_at
  - STALE_AFTER
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:2f88d49d982871a2367457566cc72b098caee9f6217d84e0ed9db26e29c6b7b7'
  docs/04-contrato-panel.md: 'sha256:5af2cb170fb82f7c94d1f1a491cd2349f599e2bde0740449655e5fcc44a2e75f'
tags:
  - contrato
  - rancio
  - entrada-no-confiable
  - desviacion
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**La decisión.** La antigüedad de un panel se mide por la **fecha de escritura del fichero**, no por el `updated_at` que el productor publica. `updated_at` se lee, se guarda como texto y se pinta tal cual, pero no se interpreta: la caja no parsea fechas ni arrastra un calendario.

**Las dos razones, y la segunda es la buena.** La barata: interpretar ISO 8601 con husos exigiría una dependencia en la caja que existe precisamente para sobrevivir a reescrituras. La de peso viene del propio contrato — **el productor machaca el fichero entero en cada publicación**, así que la fecha del fichero *es* la de su última publicación. Y así hay aviso **incluso cuando el productor no escribe su marca**, que es justo cuando más falta hace: un agente que se cuelga a medio publicar no actualiza `updated_at`, pero sí deja el fichero con su fecha.

**La contrapartida, que hay que tener presente antes de fiarse del aviso:** `mtime` **no sobrevive a copiar el fichero**. Un `panel.json` que llega por un `git checkout`, un `rsync`, un repo clonado o un contenedor recién montado tiene fecha de hoy, así que un panel cuyo contenido es de hace seis meses se pintará como fresco. Con `updated_at` no pasaría. Se acepta porque el caso normal —el productor publicando en su propio directorio— pesa más, y porque ese fichero copiado es entrada no confiable de todas formas.

**La resta va con signo, y no es prudencia.** Una fecha en el futuro no es el caso raro: basta un reloj adelantado o un sistema de ficheros ajeno. Con la resta ingenua de tiempos eso es un pánico, y **nada que venga de fuera puede provocar uno** en un camino que corre dentro del fotograma. Ojo con la razón que se escribió primero —«el contrato no exige zona horaria y un productor en otro huso publicaría una marca adelantada»—: al no interpretar `updated_at`, el huso del productor da igual. El mecanismo sigue haciendo falta, pero por los otros dos motivos.

**Y sin excepciones**, que es donde D6 casi vuelve a entrar — ver [[d6-vuelve-por-el-documento-no-por-el-codigo]].

## Where it lives

`crates/panel-contract/src/panel.rs`: el campo `Panel::written_at` con su comentario, y `Panel::is_stale_at`. Escrito también en `docs/04-contrato-panel.md` para que el productor sepa cómo se le mide.
