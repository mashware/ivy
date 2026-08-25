# El fichero de configuración

Dónde va: **`$XDG_CONFIG_HOME/agent-terminal/config`** — o `~/.config/agent-terminal/config` si no tienes
esa variable puesta. Al lado de `disposicion.json`, que es donde el terminal guarda solo cómo dejaste las
ventanas; éste lo escribes tú.

**Si no existe, el terminal abre exactamente como siempre.** No hace falta crearlo.

## Cómo se escribe

```
# Una línea que empieza por almohadilla es un comentario.
# Las líneas vacías no son nada.

color.1 = #cf6a4c
fondo   = #1d1f21
foco    = #6c9ef8
```

Una línea es `clave = valor`. Los espacios alrededor no importan.

**El comentario es de línea completa y no puede ir al final de un valor.** La almohadilla es a la vez el
carácter de comentario y el principio de un color, así que un comentario al final partiría `color.1 =
#cf6a4c` por la mitad. Es lo mismo que hacen Kitty y Ghostty.

## Qué se puede poner hoy

| Clave | Qué es |
|---|---|
| `color.0` … `color.15` | los dieciséis colores que un programa puede pedir. Del 0 al 7 los normales, del 8 al 15 los brillantes |
| `fondo` | el fondo de la consola |
| `texto` | el color con el que se escribe cuando el programa no pide otro |
| `cursor` | el cursor |
| `divisor` | la línea que separa dos consolas |
| `foco` | con qué se señala la consola que tiene el teclado |
| `panel` | el fondo del panel de estado |

Los colores se escriben **`#rrggbb`**, en hexadecimal, y da igual si en mayúsculas o en minúsculas.

**Lo que no pongas se queda como estaba.** Puedes cambiar un solo color y no escribir los demás.

## Qué pasa si te equivocas

**Una línea mal escrita no te tumba el fichero.** Se ignora, se te dice cuál es y qué le pasa por la salida
de error, y **el resto se aplica**:

```
agent-terminal: línea 7: «morado» no vale para «color.3»
```

Se dicen hasta veinte líneas con problemas; si hay más, se te dice cuántas quedaron sin listar.

**Una clave repetida gana la última**, y también se te dice — así pegar un tema encima de otro hace lo que
esperas, y no te pasas media hora cambiando una línea que otra de más abajo está pisando.

**Y si te olvidas del valor** —`fondo =`— se te dice eso mismo, no que falte el igual: el igual está
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

La fuente, su tamaño y los atajos llegan en las siguientes entregas de este mismo hito. Recargar el fichero
sin cerrar el terminal queda para después, y está escrito por qué.
