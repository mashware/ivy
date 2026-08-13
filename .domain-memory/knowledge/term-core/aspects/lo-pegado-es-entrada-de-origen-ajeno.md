---
id: asp_lo-pegado-es-entrada-de-origen-ajeno_9zg973
slug: lo-pegado-es-entrada-de-origen-ajeno
name: >-
  Lo pegado es entrada de origen ajeno: la marca de pegado no protege del
  núcleo, y filtrar solo su cierre se puede escapar
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T16:41:41.826Z'
updated_at: '2026-08-13T16:41:41.826Z'
last_verified: '2026-08-13T16:41:41.826Z'
file_paths:
  - crates/app/src/teclado.rs
  - crates/app/src/lib.rs
  - crates/term-core/src/screen.rs
  - docs/02-decisiones.md
symbols:
  - pegar
  - 'Screen::pegado_marcado'
  - 'Division::teclear'
content_hashes:
  crates/app/src/teclado.rs: 'sha256:fac665b7c9d92b5de4f79218612a98ba1d6c97b26242a606fd4149487b51b79c'
  crates/app/src/lib.rs: 'sha256:a05a4309b46dd644b5b6a2389c026f7fef94d9e1a7cb1fcde40b75eb39607291'
  crates/term-core/src/screen.rs: 'sha256:858f97327f67c34b3921b2beb9f07f3b3ebe6c26c2d6a915d535bb6e1e7a04a0'
  docs/02-decisiones.md: 'sha256:da19a4989a8d929cc92409af0e0c4267a1e2e9dbb2b06e341954025e551b9e5e'
tags:
  - portapapeles
  - seguridad
  - terminal
  - D11
feature_id: feat_term-core_nx0g1r
---

## What it does

**Un portapapeles es entrada de origen ajeno, y el envoltorio de pegado protege de mucho menos de lo que parece.**

**Uno: se puede cerrar desde dentro, y filtrar solo su cierre no basta.** Un texto que traiga la secuencia de cierre la cierra por su cuenta y lo que venga detrás entra como tecleado, **ejecutándose sin Enter**. Y quitarla con un buscar-y-borrar **se escapa**: `str::replace` recorre la cadena original y no reescanea lo que deja, así que con `ESC[2` + el cierre + `01~` los trozos de los lados **se pegan y forman un cierre nuevo**. Comprobado ejecutándolo, no razonado.

**Dos, y es peor: el envoltorio no protege del núcleo en absoluto.** Los caracteres de control los procesa **la disciplina de línea**, que ve los bytes *antes* que el programa. Un `0x03` pegado interrumpe el proceso en primer plano, un `0x1a` suspende el shell y deja la división con pinta de muerta, un `0x1c` provoca un volcado y un `0x04` cierra la entrada. Marcar el pegado no cambia nada de eso.

> **Por eso `xterm` y `VTE` filtran los controles del pegado desde hace años, y por eso la regla no es «quita esa secuencia» sino «quita los controles».** Una invariante —en el cuerpo no hay ningún control salvo tabulador y saltos— es comprobable de un vistazo; una lista de secuencias prohibidas siempre tiene un hueco más.

**El coste, aceptado y escrito:** un texto con colores pierde sus escapes y el resto queda como texto corriente. Es lo que hace `xterm` por defecto. Desde el terminal **no hay forma de distinguir el control que el usuario quería del que le colaron**.

**Y un detalle que solo se ve en pantalla: el salto de línea sale distinto según haya marca.** Sin marca hay que **imitar la tecla Enter** —retorno de carro— porque el programa no distingue lo pegado de lo tecleado. **Con marca va tal cual**: meterle un retorno ahí es meterle un control dentro de un texto, y lo inserta literal — sale un cuadro en medio de la orden y el shell lo imprime. Es la regla que usa Alacritty, y aquí se descubrió **pegando dos líneas y mirando**, porque las pruebas afirmaban lo que el código hacía y las dos estaban de acuerdo en lo equivocado.

## Where it lives

`teclado::pegar` en `crates/app/src/teclado.rs`, decidido en **D11** de `docs/02-decisiones.md`. El modo se consulta con `Screen::pegado_marcado()`, que devuelve `bool` para no dejar asomar el tipo del motor.
