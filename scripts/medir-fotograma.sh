#!/bin/sh
# Toma una tanda de la medida del fotograma **desde dentro de la aplicación**, que es la única
# forma de ver el pintado (ver `el-vaciado-no-ve-el-pintado` y el anexo «Medido» de D3).
#
#   scripts/medir-fotograma.sh <tanda> [mib]
#
#     <tanda>  A = sin panel · B = panel quieto · C = panel publicándose a ritmo
#              D = como C, pero con el fichero temporal fuera del directorio vigilado. Es la tanda
#              que refuta que el sitio del temporal influya, y el anexo de D3 la cita: sin ella no se
#              reproduce lo publicado
#     [mib]    tamaño de la carga por consola, 44 por defecto
#
# Cómo funciona, y por qué no hace falta nadie delante:
#
#   - El programa de cada consola sale de la variable `SHELL` (`crates/app/src/lib.rs`), así que
#     lanzar la ventana con `SHELL=<este guion en modo consola>` hace que las dos ejecuten la carga
#     solas. `host-load --at 30` las arranca a la vez sin que se conozcan.
#   - Cuando **las dos** consolas terminan, la ventana se cierra sola y publica el informe por su
#     salida estándar. Matarla con una señal NO publica nada: el informe se imprime en `on_exit`.
#
# LO QUE INVALIDA UNA CORRIDA, y no se ve en el resultado: la cabecera del informe imprime la rejilla
# **final**, así que si la ventana cambió de tamaño a mitad, el informe sale idéntico al de una
# corrida limpia. Medido: la ventana ya nace con la rejilla de referencia (61 filas × 57 columnas sin
# panel), así que la regla es **comprobar la cabecera y tirar la corrida si no dice eso**. Y no toques
# la máquina mientras: el cronómetro del fotograma es reloj de pared, no de CPU.
#
# El presupuesto por fotograma son 128 KiB por consola, así que `mib` × 8 es el suelo de fotogramas
# con salida por consola. Con 44 MiB salen ≥352, del orden de la tanda del 11 de agosto.
#
# CADA TANDA SE CORRE DOS VECES y se publica la media de las dos, que es lo que hace el anexo. No es
# por precisión: dos corridas que no se parecen delatan una contaminada, y una sola no se puede
# auditar. Lo que NO reproduce este guion es la cifra del despertador que el anexo cita: salió de una
# sonda temporal que no está versionada.
set -eu

RAIZ=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$RAIZ/target/release/app"
CARGA="$RAIZ/target/release/host-load"

# ── Modo consola ────────────────────────────────────────────────────────────────
# La aplicación invoca este mismo guion como si fuera el shell del usuario. Vuelca la carga y
# termina; al terminar las dos, la ventana se cierra.
if [ "${MEDIR_COMO_CONSOLA:-}" = "1" ]; then
    exec "$CARGA" run --mib "${MEDIR_MIB:-44}" --at 30 --tag "${MEDIR_TANDA:-x}"
fi

# ── Modo tanda ──────────────────────────────────────────────────────────────────
TANDA=${1:?falta la tanda: A, B, C o D}
MIB=${2:-44}
SALIDA="$RAIZ/target/medidas"
mkdir -p "$SALIDA"
# **El testigo de que alguien llegó a publicar.** La siembra corre en segundo plano, así que si falla
# —los directorios no aparecen, un `mv` se cae— la ventana sigue su curso y publica un informe
# completo: uno con forma de tanda A bajo etiqueta B, C o D, y nada aguas abajo lo delataría. Los
# directorios de panel desaparecen al cerrarse la aplicación, así que la comprobación no puede ser a
# posteriori sobre ellos: tiene que dejar rastro fuera.
TESTIGO="$SALIDA/.sembrado"
rm -f "$TESTIGO"

case "$TANDA" in
    A|B|C|D) ;;
    *) echo "tanda desconocida «$TANDA»: usa A, B, C o D" >&2; exit 2 ;;
esac

# **Los dos binarios, antes de nada.** Sin `host-load` las dos consolas mueren en el acto y la ventana
# se cierra: sale un informe con cabecera y sin tramos, que a simple vista parece una corrida válida.
# Y si la aplicación no puede lanzar este guion como shell, cae al de respaldo del sistema y las dos
# consolas se quedan en un prompt: la ventana no se cierra nunca y no publica nada.
for binario in "$APP" "$CARGA"; do
    [ -x "$binario" ] || { echo "falta $binario — compila con: cargo build --release -p app -p host-load" >&2; exit 1; }
done

# **Con una ventana abierta no se mide, y menos aún se borra nada.** Los directorios de panel no
# llevan el pid de quien los creó, así que la limpieza de abajo no puede distinguir los restos de una
# corrida anterior de los de una sesión de trabajo viva: los borraría todos. Y aunque no borrara, una
# segunda ventana compite por la máquina y el cronómetro del fotograma es reloj de pared.
if pgrep -x app > /dev/null 2>&1; then
    echo "hay una ventana de agent-terminal abierta: ciérrala antes de medir" >&2
    exit 1
fi

# Ahora sí: lo que quede es de una corrida anterior. Un panel viejo falsearía la tanda A entera —que
# es la referencia— y en las demás daría un panel que no es el que esta corrida quiere medir.
rm -rf /run/user/"$(id -u)"/agent-terminal-* 2>/dev/null || true

publicar() {  # publicar <directorio> <n>
    # **Dónde cae el temporal NO importa, y la tanda D existe para haberlo comprobado.** La sospecha
    # era que un temporal dentro del directorio vigilado despertase a la ventana por su cuenta al
    # cerrarse, doblando los despertares por publicación. Es falso por construcción: `vigilante::repartir`
    # solo levanta la bandera cuando el aviso trae la ruta exacta `<dir>/panel.json`, así que cualquier
    # otro fichero del directorio es invisible. D lo confirma midiendo, y se conserva para que la
    # hipótesis no se vuelva a levantar.
    if [ "$TANDA" = D ]; then
        tmp="/run/user/$(id -u)/.medida-$(basename "$1").json"
    else
        tmp="$1/.medida.json"
    fi
    cat > "$tmp" <<JSON
{"v":1,"stale_after_minutes":600,"lines":[
  {"text":"Tanda $TANDA · publicación $2","style":"title"},
  "",
  {"ref":"carga","text":"$MIB MiB por consola, volcado tipo TUI","mark":"current"},
  {"ref":"panel","text":"régimen $TANDA — esto es lo que se está midiendo","mark":"info"},
  {"ref":"nota","text":"la maqueta se rehace en cada relectura buena","mark":"info"},
  "",
  {"text":"seis líneas y un aviso: el tamaño de un panel de trabajo real","mark":"done"}
]}
JSON
    mv "$tmp" "$1/panel.json"   # de una vez, como manda el contrato
}

# Los directorios los crea la aplicación al arrancar, así que hay que esperarlos. No se leen de
# PANEL_DIR: esa variable la ven los programas de dentro, no un proceso de fuera.
sembrar() {
    modo=$1
    espera=0
    while [ "$(ls -d /run/user/"$(id -u)"/agent-terminal-* 2>/dev/null | wc -l)" -lt 2 ]; do
        espera=$((espera + 1))
        [ "$espera" -gt 100 ] && { echo "no aparecieron los dos directorios de panel" >&2; return 1; }
        sleep 0.1
    done
    dirs=$(ls -d /run/user/"$(id -u)"/agent-terminal-*)

    if [ "$modo" = quieto ]; then
        n=0
        for d in $dirs; do n=$((n + 1)); publicar "$d" "$n"; done
        : > "$TESTIGO"
        return 0
    fi

    # Hablador: republica mientras la ventana viva. Una vez por segundo — el contrato mide una
    # publicación de una vez en 1 fotograma, así que esto es el caso recomendado repetido, no el
    # patológico. Y solapa con el volcado a propósito: un fotograma de publicación sin salida de
    # consola no entraría en las series del informe.
    n=0
    while pgrep -x app > /dev/null 2>&1; do
        n=$((n + 1))
        for d in $dirs; do publicar "$d" "$n"; done
        : > "$TESTIGO"
        sleep 1
    done
}

case "$TANDA" in
    A) : ;;                       # sin panel: nadie publica nada
    B) sembrar quieto & ;;
    C|D) sembrar hablador & ;;
esac

echo "tanda $TANDA · $MIB MiB por consola · no toques la máquina hasta que cierre" >&2

# Ruta absoluta a propósito: la consola no hereda el directorio desde el que se lanzó la tanda, y
# un `$0` relativo dejaría a la aplicación buscando un shell que no existe.
MEDIR_COMO_CONSOLA=1 MEDIR_MIB="$MIB" MEDIR_TANDA="$TANDA" \
    SHELL="$RAIZ/scripts/medir-fotograma.sh" \
    "$APP" > "$SALIDA/tanda-$TANDA-$(date +%H%M%S).txt"

wait 2>/dev/null || true

INFORME=$(ls -t "$SALIDA"/tanda-*.txt | head -1)

if [ "$TANDA" != A ] && [ ! -f "$TESTIGO" ]; then
    mv "$INFORME" "$INFORME.INVALIDA"
    echo "TANDA INVÁLIDA: nadie llegó a publicar un panel, así que esto es una tanda A con otro" >&2
    echo "nombre. Informe marcado como $INFORME.INVALIDA" >&2
    exit 1
fi

echo "hecho: $(basename "$INFORME")" >&2
