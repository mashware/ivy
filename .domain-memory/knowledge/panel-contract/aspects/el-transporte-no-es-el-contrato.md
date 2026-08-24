---
id: asp_el-transporte-no-es-el-contrato_0n14aw
slug: el-transporte-no-es-el-contrato
name: >-
  El fichero no puede ser la fuente de verdad: el contrato es el documento y
  `panel.json` es solo uno de sus transportes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T08:26:17.392Z'
updated_at: '2026-08-24T08:26:17.392Z'
last_verified: '2026-08-24T08:26:17.392Z'
file_paths:
  - docs/02-decisiones.md
  - docs/03-arquitectura.md
  - docs/04-contrato-panel.md
symbols: []
content_hashes:
  docs/02-decisiones.md: 'sha256:97753287aa018f84f87473e2872f7347a4aacb199a69cc2007289f3c34ee1475'
  docs/03-arquitectura.md: 'sha256:04750055e66a4260a0e5342bf03a5b365859fca8a888a7e9b61b6826701ff321'
  docs/04-contrato-panel.md: 'sha256:95629fb310672544479a213c3275547f03e137cc6cdad1da3920c8076c83eba9'
tags:
  - contrato
  - transporte
  - d17
  - mcp
  - osc
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Lo que estaba mal, y no era una preferencia de diseño: era un choque ya programado.** El plan tenía **dos escritores del mismo sitio** — el productor por fichero y el agente por `panel_set` de MCP en la Fase 3 (D5) — y nadie que decidiera quién manda. Sin fuente ni orden, gana quien escribió el último, o sea la planificación del sistema operativo. El síntoma no aparece hoy porque solo hay un productor real.

**La decisión (D17), en tres partes:** el contrato es **el documento** (`phase`, `lines`, marcas, estilos, caducidad), que es lo que se versiona; **la fuente de verdad es el estado que la app tiene por división**, no el fichero; y cada entrega trae **fuente y orden** para que dos productores no se decidan por azar.

> **Y una razón de origen que conviene admitir: el medio se eligió con un solo tipo de productor a la vista.** El primero fue un agente que escribe ficheros, y para eso el fichero es imbatible. Las tres razones que lo justificaban —aislamiento de fallos, se lee con `cat`, cruza SSH sin protocolo propio— siguen siendo buenas, pero **ninguna dice por qué tiene que ser el único**.

**El eje que decide entre transportes no es la elegancia: es qué tiene que hacer el productor para publicar.**

| | Fichero | MCP | Socket | OSC por el PTY | API HTTP |
|---|---|---|---|---|---|
| Desde un script | `cat >` | no | necesita cliente | `printf` | `curl` + puerto |
| Cruza contenedor / SSH | solo con volumen | no | no | **gratis** | con red y puerto |
| Sobrevive al productor | **gratis** | si la app lo guarda | si la app lo guarda | si la app lo guarda | igual |
| Sabe quién escribe | **nadie** | por sesión | credenciales del proceso | **por construcción: su PTY** | solo autenticando |
| Avisa al productor de que su documento está roto | **no** | sí | sí | no | sí |

**Lo que se decidió con esa tabla:** el fichero se queda **de serie** —es el único donde el productor más tonto publica sin escribir código, y el único donde el estado sobrevive gratis a que el productor muera—; **MCP entrega el documento sin escribir el fichero** (escribirlo sería que la app se detecte a sí misma por el vigilante, un rodeo con un fotograma de retraso); y **OSC por el PTY queda como candidato tras F2.7**, porque es el único que cruza un `ssh` o un contenedor sin volumen **y** el único donde el acotado por división no se puede falsear.

**Descartados, con su razón:** la **API HTTP** (todo lo que aporta lo cubre otro y convierte «quien escriba en tu directorio» en «cualquiera que alcance tu máquina»); el **socket como principal** (es lo de Herdr: saber qué proceso escribe no compensa perder al contenedor, al CI de otra máquina y al `scp`); **sustituir el fichero en vez de acompañarlo** (cualquier transporte con cliente sube el listón de entrada, y el listón es lo que permite que el formato tenga productores, que es lo único que puede convertirse en foso).

**Dos avisos para quien vuelva aquí:**

- **D6 no se toca.** Esto es por dónde llega el estado, no quién lo decide. Ver [[d6-vuelve-por-el-documento-no-por-el-codigo]].
- **El vigilante no es un argumento contra el fichero.** Se comprobó antes de escribirlo: la app usa **un solo vigilante** para todos los directorios, no uno por división, y los límites de la máquina de desarrollo son 1.024 instancias y 524.288 rutas. Vigilar ficheros no escala mal aquí.

**Lo que queda abierto a propósito:** cómo se expresan la fuente y el orden en el documento. Herdr lo resuelve con un `--source` estable por integración —que da autoridad y se suelta al salir— y un `--seq` estrictamente creciente que descarta lo que llega tarde. Con un solo transporte vivo, fijar la forma ahora sería inventarla antes de tener el segundo caso delante.

## Where it lives

`docs/02-decisiones.md` → **D17** (la decisión entera con la tabla y lo descartado) y **D5** (la tabla de canales, que ya no nombra `panel.json` como si fuera el canal). `docs/04-contrato-panel.md`, partido en **Parte I — El documento** y **Parte II — Los transportes**. `docs/03-arquitectura.md` → «Los tres canales», canal 1.
