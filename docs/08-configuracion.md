# El fichero de configuración

Dónde va: **`$XDG_CONFIG_HOME/agent-terminal/config`** — o `~/.config/agent-terminal/config` si no tienes
esa variable puesta. Al lado de `layout.json`, que es donde el terminal guarda solo cómo dejaste las
ventanas; éste lo escribes tú.

**Si no existe, el terminal abre exactamente como siempre.** No hace falta crearlo.

## Cómo se escribe

```
# Una línea que empieza por almohadilla es un comentario.
# Las líneas vacías no son nada.

color.1 = #cf6a4c
background = #1d1f21
focus      = #6c9ef8
```

Una línea es `clave = valor`. Los espacios alrededor no importan.

**El comentario es de línea completa y no puede ir al final de un valor.** La almohadilla es a la vez el
carácter de comentario y el principio de un color, así que un comentario al final partiría `color.1 =
#cf6a4c` por la mitad. Es lo mismo que hacen Kitty y Ghostty.

## Qué se puede poner hoy

| Clave | Qué es |
|---|---|
| `color.0` … `color.15` | los dieciséis colores que un programa puede pedir. Del 0 al 7 los normales, del 8 al 15 los brillantes |
| `background` | el fondo de la consola |
| `foreground` | el color con el que se escribe cuando el programa no pide otro |
| `cursor` | el cursor |
| `divider` | la línea que separa dos consolas |
| `focus` | con qué se señala la consola que tiene el teclado |
| `panel` | el fondo del panel de estado |
| `size` | el cuerpo de la letra, en puntos |

Los colores se escriben **`#rrggbb`**, en hexadecimal, y da igual si en mayúsculas o en minúsculas.

**El tamaño va de 4 a 96 puntos**, con decimales si quieres. El de siempre son **14**. Fuera de ese rango
se te avisa y se usa el de siempre: con dos puntos no se lee nada y con doscientos caben tres columnas.

Cambiar el tamaño **cambia cuántas filas y columnas** te caben en la ventana, así que los programas que
corren dentro lo notan.

**Lo que no pongas se queda como estaba.** Puedes cambiar un solo color y no escribir los demás.

## Los atajos

Los once atajos del terminal se cambian con `key.` y el nombre de lo que hacen:

```
key.split-right = ctrl+shift+e
key.close-pane = none
```

| Nombre | Qué hace | De fábrica |
|---|---|---|
| `split-right` | parte la consola en dos, a lo ancho | `ctrl+shift+d` |
| `split-down` | la parte a lo alto | `ctrl+shift+b` |
| `close-pane` | cierra la consola con el foco | `ctrl+shift+x` |
| `new-tab` | abre una pestaña | `ctrl+shift+t` |
| `close-tab` | cierra la pestaña | `ctrl+shift+w` |
| `previous-tab` · `next-tab` | se mueve entre pestañas | `ctrl+shift+←` · `ctrl+shift+→` |
| `scroll-page-up` · `scroll-page-down` | sube y baja por el historial | `shift+pageup` · `shift+pagedown` |
| `scroll-to-top` · `scroll-to-bottom` | va a los extremos del historial | `shift+home` · `shift+end` |

**Un acorde se escribe con `+`**, en cualquier orden y sin importar mayúsculas: `ctrl+shift+e`,
`shift+prior`, `alt+f4`. Vale `ctrl`, `shift` y `alt`, y **hace falta al menos uno**: si no, te estarías
quitando una letra de escribir.

Las teclas se escriben como las llama el teclado: `e`, `f4`, `space`, `delete`, `home`, `end`, `pageup`,
`pagedown`, `left`, `right`, `up`, `down`. Valen también las abreviaturas de siempre (`prior`, `next`,
`pgup`, `pgdn`, `esc`) y las flechas con su símbolo (`←`, `↑`, `→`, `↓`).

**`Ctrl+C` y `Ctrl+V` no se pueden asignar a nada.** El escritorio se los queda antes de que lleguen
aquí, así que un atajo con esos acordes no se dispararía nunca y no tendrías forma de saber por qué.

### `none` te devuelve el acorde

```
key.close-pane = none
```

A partir de ahí ese acorde **le llega al programa que corre dentro**. Es lo que te permite recuperar
`Ctrl+Mayúsculas+X` para `vim` o para `tmux`.

### Si dos atajos se pisan

**Gana el último que escribas**, y se te dice cuál se ha quedado sin acorde:

```
agent-terminal: 'close-pane' is left without a shortcut: its chord goes to 'split-right', which comes later
```

### Copiar y pegar no se reasignan: copiar solo se apaga

`Ctrl+Mayúsculas+C` y `Ctrl+Mayúsculas+V` **no llegan como teclas**: los resuelve el escritorio antes y
llegan ya convertidos en un copiado y un pegado, **sin decir siquiera si había Mayúsculas**. No hay acorde
que mover, y por eso no están en la tabla.

Lo que sí puedes es **apagar copiar**, que es lo que devuelve `Ctrl+Mayúsculas+C` a `vim` o a `tmux`:

```
key.copy = none
```

Cualquier otro valor se rechaza y se te dice por qué — asignarle otro acorde se aceptaría y no funcionaría
nunca:

```
agent-terminal: line 3: the chord for 'key.copy' is the desktop's and cannot be changed; the only thing you can do is turn it off with 'none'
```

## Qué pasa si te equivocas

**Una línea mal escrita no te tumba el fichero.** Se ignora, se te dice cuál es y qué le pasa por la salida
de error, y **el resto se aplica**:

```
agent-terminal: line 7: 'morado' is not valid for 'color.3'
```

Se dicen hasta veinte líneas con problemas; si hay más, se te dice cuántas quedaron sin listar.

**Una clave repetida gana la última**, y también se te dice — así pegar un tema encima de otro hace lo que
esperas, y no te pasas media hora cambiando una línea que otra de más abajo está pisando.

**Y si te olvidas del valor** —`background =`— se te dice eso mismo, no que falte el igual: el igual está
puesto, lo que falta es el color.

Lo que sí deja el terminal entero de fábrica, avisando de por qué:

- que no se pueda leer, o que con ese nombre haya algo que no sea un fichero;
- que no sea texto;
- que se pase de 64 KiB;
- que declare una versión más nueva que la que este terminal entiende.

Que **no exista** es el único caso que no se dice: es lo normal.

## `version`

Es **opcional**. Si la pones, sirve para una sola cosa: que un terminal más viejo que el formato **no
aplique** un fichero que no entiende, en vez de aplicar la mitad.

```
version = 1
```

No hace falta escribirla. El formato es plano, así que una clave que un terminal no conozca simplemente se
ignora y te lo dice.

Si escribes algo que no es un número, se te avisa de esa línea como de cualquier otra y el fichero sigue
aplicándose.

## Lo que todavía no se puede configurar

La fuente llega en la siguiente entrega de este mismo hito. Recargar el fichero
sin cerrar el terminal queda para después, y está escrito por qué.
