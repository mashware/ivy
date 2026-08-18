#!/bin/sh
# Toma una tanda de la medida del fotograma **desde dentro de la aplicación**, que es la única
# forma de ver el pintado (ver `el-vaciado-no-ve-el-pintado` y el anexo «Medido» de D3).
#
#   scripts/medir-fotograma.sh <tanda> [mib]
#   scripts/medir-fotograma.sh comparar <informe> <informe>
#
#     <tanda>  Con UNA pestaña y DOS divisiones, que es como se midió hasta F2.1:
#                A = sin panel · B = panel quieto · C = panel publicándose a ritmo
#                D = como C, pero con el fichero temporal fuera del directorio vigilado. Es la tanda
#                que refuta que el sitio del temporal influya, y el anexo de D3 la cita: sin ella no
#                se reproduce lo publicado
#              Con DOS pestañas, las que F2.1 añade — las cuatro con panel a ritmo, o sea el régimen
#              de la C, para que se puedan comparar entre sí y con ella:
#                F = la pestaña de fondo QUIETA: sus dos consolas terminan sin volcar nada. Aísla lo
#                    que cuesta la barra y bombear dos divisiones que no producen
#                G = la pestaña de fondo VOLCANDO: sus dos consolas vuelcan igual que las de delante.
#                    Aísla el trabajo de fondo
#              Con UNA pestaña y CUATRO divisiones, las que F2.2 añade — también en el régimen de la
#              C, que es con quien se comparan:
#                H = las cuatro EN FILA. Cada consola se queda con la cuarta parte del ancho, que es
#                    el peor caso del panel: la pregunta abierta del issue #33
#                I = las cuatro EN CUADRO (dos columnas de dos). Medio ancho cada una, que es la
#                    disposición que alguien usaría de verdad
#              Con UNA pestaña y DOS divisiones **arrastrándose**, la que F2.3 añade — también en el
#              régimen de la C, que es con quien se compara:
#                J = como la C, y además la ventana **se arrastra a sí misma**: agarra una consola, la
#                    lleva sobre la otra y la suelta, una y otra vez, alternando el lado. La diferencia
#                    entre J y C es lo que cuesta el gesto
#
#              NO hay tanda con arrastres de CUATRO divisiones, y no es un olvido: el ciclo del gesto solo
#              devuelve la disposición a la de partida **con dos**. Con más, insertar parte el peso del
#              destino y quitar lo reparte entre las que quedan, así que el peso se concentra: medido con
#              cuatro en fila sobre 1.200 puntos, la primera consola pasa de 298,5 a 119,4 puntos en un
#              solo ciclo. Cada fotograma mediría una ventana distinta y la media no describiría ninguna.
#              Atado por `el_ciclo_de_dos_gestos_vuelve_a_la_forma_de_partida_solo_con_dos_consolas`.
#     [mib]    tamaño de la carga por consola, 44 por defecto
#
#   El modo `comparar` no mide: lee dos informes de la misma tanda y dice si se pueden promediar.
#
# Cómo funciona, y por qué no hace falta nadie delante:
#
#   - El programa de cada consola sale de la variable `SHELL` (`crates/app/src/lib.rs`), así que
#     lanzar la ventana con `SHELL=<este guion en modo consola>` hace que las dos ejecuten la carga
#     solas. `host-load --at 30` las arranca a la vez sin que se conozcan.
#   - Cuando **las dos** consolas terminan, la ventana se cierra sola y publica el informe por su
#     salida estándar. Matarla con una señal NO publica nada: el informe se imprime en `on_exit`.
#
# EL TAMAÑO DE LA VENTANA SE PIDE, Y ANTES NO SE PEDÍA. Sin pedirlo lo decide el escritorio, y eso hacía
# la medida **irreproducible sin que nada lo dijera**: el 17 de agosto de 2026, tres corridas de la tanda A
# en la misma máquina y el mismo día dieron tres rejillas distintas —36×47, 40×66 y 61×57—, y de la rejilla
# cuelgan todas las medidas del proyecto. La causa, encontrada midiendo: `NativeOptions::default()` no lleva
# tamaño, así que GNOME sobre Wayland manda el suyo en el tercer fotograma. La aplicación ahora lo pide y
# **insiste** cuando el compositor lo pisa, y publica en la cabecera el tamaño que tuvo — que es la única
# línea del informe que se puede volver a pedir.
#
# LO QUE INVALIDA UNA CORRIDA, y no se ve en el resultado: la cabecera del informe imprime la rejilla
# **final**, así que si la ventana cambió de tamaño a mitad, el informe sale idéntico al de una
# corrida limpia. **Lo comprueba el guion**, y ya no una regla escrita aquí que había que
# recordar: antes decía «tira la corrida si la cabecera no dice 61×57», y con dos pestañas dice 59×57
# —la barra se lleva una fila y pico—, así que esa regla habría descartado justo las tandas que F2.1
# viene a tomar. Lo que se comprueba ahora no lleva ningún número horneado:
#
#   - que la cabecera declare **el número de pestañas que se pidió**,
#   - que la ventana abriera **con el tamaño que se le pidió**, y
#   - que las dos corridas de la misma tanda publiquen **la misma rejilla** (modo `comparar`). Esta
#     tercera se anunciaba aquí desde F2.1 y **no estaba escrita en ninguna parte**: es la que habría
#     cazado las tres rejillas de aquel día.
#
# Y no toques la máquina mientras: el cronómetro del fotograma es reloj de pared, no de CPU.
#
# El presupuesto por fotograma son 128 KiB **por división**, no por ventana, así que `mib` × 8 es el
# suelo de fotogramas con salida **de cada consola que vuelca**, y no cambia al añadir pestañas: cada
# división tiene su propio presupuesto. Con 44 MiB salen ≥352, del orden de la tanda del 11 de agosto.
# Lo que sí crece con las pestañas es el fotograma, porque todas bombean en el mismo.
#
# CADA TANDA SE CORRE DOS VECES y se publica la media de las dos, que es lo que hace el anexo. No es
# por precisión: dos corridas que no se parecen delatan una contaminada, y una sola no se puede
# auditar. Lo que NO reproduce este guion es la cifra del despertador que el anexo cita: salió de una
# sonda temporal que no está versionada.
set -eu

RAIZ=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$RAIZ/target/release/app"
CARGA="$RAIZ/target/release/host-load"

# **El tamaño de referencia, en puntos.** Cualquiera sirve mientras sea el mismo en todas las tandas que
# se comparen entre sí; éste se eligió por caber de sobra en la pantalla donde se tomó el anexo de F2.2 y
# por ser un número redondo que se recuerda. Se puede cambiar desde fuera para medir otra ventana, y
# entonces **hay que volver a tomar todas las tandas del anexo**: una tanda de 1280×800 y otra de 800×600
# no se comparan.
TAMANO=${TAMANO:-1280x800}

# **Cuánto puede desviarse un tamaño del otro para ser el mismo, en puntos, y hay un solo sitio con este
# número en el guion.** Es el mismo que usa la aplicación —`MARGEN_DEL_TAMAÑO` en `crates/app/src/lib.rs`—
# y tiene que seguir siéndolo: si el guion fuera más estricto, tiraría corridas que la aplicación da por
# buenas, con un mensaje que además culparía al escritorio.
MARGEN_DEL_TAMANO=0.5

# El tamaño que un informe dice que tuvo la ventana, escrito como lo escribe la variable: con «x».
tamano_de() {
    sed -n 's/^ventana: *\(.*\) puntos$/\1/p' "$1" | sed 's/×/x/' | head -1
}

# Si dos tamaños «<ancho>x<alto>» son el mismo dentro del margen. Vacío o ilegible nunca es parecido.
#
# **`LC_ALL=C` no es adorno**: `awk` convierte los números según la configuración regional, así que en una
# sesión con coma decimal `1279.8125` se leería como `1279` y una corrida buena saldría inválida.
parecido() {
    LC_ALL=C awk -v a="$1" -v b="$2" -v m="$MARGEN_DEL_TAMANO" 'BEGIN {
        split(a, p, "x"); split(b, q, "x");
        if (p[1] == "" || p[2] == "" || q[1] == "" || q[2] == "") exit 1
        exit !(p[1] - q[1] <= m && q[1] - p[1] <= m && p[2] - q[2] <= m && q[2] - p[2] <= m)
    }'
}

# ── Modo comparar ───────────────────────────────────────────────────────────────
# **La guarda que este guion decía tener desde F2.1 y no tenía.** El anexo publica la media de dos
# corridas, y promediar dos corridas con rejillas distintas da un número que no describe ninguna de las
# dos — y sale plausible. No mide nada: solo lee las dos cabeceras.
if [ "${MEDIR_COMO_CONSOLA:-}" != "1" ] && [ "${1:-}" = comparar ]; then
    [ $# -eq 3 ] || { echo "uso: $0 comparar <informe> <informe>" >&2; exit 2; }
    for informe in "$2" "$3"; do
        [ -f "$informe" ] || { echo "no existe $informe" >&2; exit 2; }
        # **Un informe sin la línea de la ventana no se puede comparar con nada**, y callarse sería el peor
        # de los resultados: dos informes viejos —tomados antes de que la cabecera dijera con qué ventana se
        # midió— tienen la misma geometría *ausente*, así que saldrían «comparables» sin que ninguno declare
        # en qué ventana se tomó. Es exactamente la clase de número plausible que esta guarda viene a evitar.
        grep -q '^ventana: ' "$informe" || {
            echo "$informe no dice con qué ventana se midió: es de antes de que la cabecera lo publicara," >&2
            echo "así que no se puede saber si es comparable con nada. Vuelve a tomar la tanda." >&2
            exit 1
        }
    done
    # Las pestañas y la rejilla de cada división: lo que tiene que coincidir **exacto** para que la media
    # signifique algo. Los tramos de abajo son justo lo que se espera que difiera.
    #
    # **El tamaño de ventana NO entra aquí, y eso es a propósito.** Compararlo carácter a carácter tiraba
    # dos corridas perfectamente promediables: el compositor entrega el tamaño con fracción y no siempre la
    # misma —1280×800 en una corrida y 1279,8125×799,8125 en otra—, las dos dentro del margen y **con la
    # misma rejilla**. Y como la fracción no es determinista, volver a tomarlas podía fallar otra vez. El
    # tamaño se compara abajo, con el mismo margen que usa la aplicación.
    # **Y el NOMBRE de cada división tampoco entra, por la misma clase de razón que el tamaño.** Lo que
    # decide si dos corridas se pueden promediar es cuántas consolas hubo y con qué rejilla cada **sitio**,
    # no qué consola ocupaba cada sitio al cerrar. Desde que la ventana se arrastra a sí misma (tanda J) las
    # consolas **cambian de sitio durante la corrida**, así que cuál acaba a la izquierda depende de si el
    # último gesto llegó a completarse — y con el nombre dentro, dos corridas idénticas en ventana, pestañas
    # y rejillas salían «no comparables» por un dato que no cambia nada de lo que se mide. Medido: dos J del
    # 18 de agosto de 2026, las dos 1280×800 con las dos consolas a 49×39, rechazadas solo por eso.
    geometria() {
        sed -n -e 's/^pestañas: */pestañas /p' -e 's/^  división [0-9]*: */división /p' "$1"
    }
    primera=$(geometria "$2")
    segunda=$(geometria "$3")
    if [ "$primera" = "$segunda" ] && parecido "$(tamano_de "$2")" "$(tamano_de "$3")"; then
        echo "las dos corridas describen la misma ventana: se pueden promediar" >&2
        printf 'ventana %s puntos\n%s\n' "$(tamano_de "$2")" "$primera" >&2
        exit 0
    fi
    echo "LAS DOS CORRIDAS NO SON COMPARABLES: describen ventanas distintas, así que su media no" >&2
    echo "describe ninguna de las dos. Vuelve a tomarlas." >&2
    printf '%s:\nventana %s puntos\n%s\n%s:\nventana %s puntos\n%s\n' \
        "$2" "$(tamano_de "$2")" "$primera" "$3" "$(tamano_de "$3")" "$segunda" >&2
    exit 1
fi

# ── Modo consola ────────────────────────────────────────────────────────────────
# La aplicación invoca este mismo guion como si fuera el shell del usuario. Vuelca la carga y
# termina; al terminar todas, la ventana se cierra.
if [ "${MEDIR_COMO_CONSOLA:-}" = "1" ]; then
    # **Qué número de consola soy, sin carreras.** En la tanda F las dos consolas de la pestaña de
    # fondo tienen que terminar SIN volcar, y una consola no puede saberlo por sí misma: su `PANEL_ID`
    # es un sorteo de ocho dígitos, no dice a qué pestaña pertenece. Lo que sí sabemos es el orden en
    # que la aplicación las abre —pestaña por pestaña, izquierda y luego derecha—, así que basta con
    # repartir turnos. `mkdir` es atómico incluso entre procesos, así que la primera que consigue
    # crear un número se lo queda: sin cerrojos y sin fiarse de que arranquen ordenadas.
    turno=0
    while :; do
        turno=$((turno + 1))
        mkdir "$MEDIR_TURNOS/$turno" 2>/dev/null && break
        if [ "$turno" -gt 32 ]; then
            echo "no quedan turnos: ¿se lanzaron más consolas de las previstas?" >&2
            exit 1
        fi
    done

    # Las que pasan del cupo terminan enseguida y sin escribir nada. **Terminar y no dormir**: una
    # consola dormida sigue viva, y la ventana solo se cierra cuando han terminado todas — la tanda no
    # acabaría nunca. Muerta y callada es exactamente lo que «la pestaña de fondo quieta» significa.
    if [ "$turno" -gt "${MEDIR_VUELCAN:-2}" ]; then
        exit 0
    fi

    exec "$CARGA" run --mib "${MEDIR_MIB:-44}" --at 30 --tag "${MEDIR_TANDA:-x}"
fi

# ── Modo tanda ──────────────────────────────────────────────────────────────────
TANDA=${1:?falta la tanda: A, B, C, D, F, G, H, I o J}
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

# Cuántas pestañas abre la ventana, con qué disposición de consolas cada una, y cuántas vuelcan —
# derivado de la letra. La aplicación abre las consolas **pestaña por pestaña y en el orden en que se
# pintan**, así que con dos pestañas las dos primeras son las de delante y las dos últimas las de fondo,
# y con cuatro en cuadro el orden es la columna izquierda de arriba abajo y luego la derecha.
# **Una tanda se define en UN solo sitio, incluidos sus arrastres.** Estuvieron en un `case` aparte y la
# revisión lo tiró: con dos, añadir una tanda pide dos ediciones y la que se olvide cae en el `*)` del
# segundo con cero gestos — o sea que una tanda nueva pensada para arrastrar se quedaría quieta y el guion la
# invalidaría por no declarar arrastres, en vez de simplemente funcionar.
case "$TANDA" in
    A|B|C|D) PESTANAS=1; DIVISIONES=2x1; VUELCAN=2; ARRASTRES=0 ;;
    F)       PESTANAS=2; DIVISIONES=2x1; VUELCAN=2; ARRASTRES=0 ;;   # la de fondo, quieta: no vuelca ninguna
    G)       PESTANAS=2; DIVISIONES=2x1; VUELCAN=4; ARRASTRES=0 ;;   # la de fondo, volcando como la de delante
    H)       PESTANAS=1; DIVISIONES=4x1; VUELCAN=4; ARRASTRES=0 ;;   # cuatro en fila: el peor caso del panel (#33)
    I)       PESTANAS=1; DIVISIONES=2x2; VUELCAN=4; ARRASTRES=0 ;;   # cuatro en cuadro
    J)       PESTANAS=1; DIVISIONES=2x1; VUELCAN=2; ARRASTRES=100000 ;;   # dos, y arrastrándose: el gesto de F2.3
    *) echo "tanda desconocida «$TANDA»: usa A, B, C, D, F, G, H, I o J" >&2; exit 2 ;;
esac
# **Por qué la tanda que arrastra pide cien mil gestos**: el número está deliberadamente por encima de los
# que caben, porque lo que hace falta es que el gesto dure **de principio a fin**. Una tanda que se queda sin
# gestos a mitad promedia dos regímenes distintos y publica la media de los dos como si fuera uno. Cuántos
# ocurrieron de verdad lo dice el informe, y **ése** es el número que vale: el informe publica los inyectados
# y los que reordenaron, no los pedidos.

# Las consolas por pestaña salen de la disposición, que es «columnas por filas».
POR_PESTANA=$(( ${DIVISIONES%x*} * ${DIVISIONES#*x} ))
DIRECTORIOS=$((PESTANAS * POR_PESTANA))

# Los turnos de las consolas viven en un directorio limpio por corrida: si quedaran los de la anterior,
# todas las consolas de ésta se pasarían de cupo y no volcaría ninguna.
TURNOS="$SALIDA/.turnos"
rm -rf "$TURNOS"
mkdir -p "$TURNOS"

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
    # **Se esperan TODOS los directorios, y después se comprueba que son exactamente esos.** Antes se
    # esperaba a que hubiera dos y se listaba lo que hubiera: con dos pestañas hay cuatro, así que la
    # espera se satisfacía con los dos primeros y el listado cogía dos, tres o cuatro según cayera la
    # carrera — una tanda con unas consolas con panel y otras sin él, o sea con el régimen mezclado, y
    # publicada como buena porque el testigo se escribía igual.
    while [ "$(ls -d /run/user/"$(id -u)"/agent-terminal-* 2>/dev/null | wc -l)" -lt "$DIRECTORIOS" ]; do
        espera=$((espera + 1))
        [ "$espera" -gt 100 ] && { echo "no aparecieron los $DIRECTORIOS directorios de panel" >&2; return 1; }
        sleep 0.1
    done
    dirs=$(ls -d /run/user/"$(id -u)"/agent-terminal-*)
    cuantos=$(echo "$dirs" | wc -l)
    if [ "$cuantos" -ne "$DIRECTORIOS" ]; then
        echo "aparecieron $cuantos directorios de panel y se esperaban $DIRECTORIOS: no se siembra" >&2
        return 1
    fi

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
    C|D|F|G|H|I|J) sembrar hablador & ;;
esac

echo "tanda $TANDA · $PESTANAS pestaña(s) de $DIVISIONES · $VUELCAN de $DIRECTORIOS consolas volcando $MIB MiB · ventana $TAMANO · no toques la máquina hasta que cierre" >&2

# Ruta absoluta a propósito: la consola no hereda el directorio desde el que se lanzó la tanda, y
# un `$0` relativo dejaría a la aplicación buscando un shell que no existe.
INFORME="$SALIDA/tanda-$TANDA-$(date +%H%M%S).txt"
MEDIR_COMO_CONSOLA=1 MEDIR_MIB="$MIB" MEDIR_TANDA="$TANDA" \
    MEDIR_TURNOS="$TURNOS" MEDIR_VUELCAN="$VUELCAN" \
    MEDIR_PESTANAS="$PESTANAS" MEDIR_DIVISIONES="$DIVISIONES" \
    MEDIR_TAMANO="$TAMANO" MEDIR_ARRASTRES="$ARRASTRES" \
    SHELL="$RAIZ/scripts/medir-fotograma.sh" \
    "$APP" > "$INFORME"

wait 2>/dev/null || true

invalidar() {
    mv "$INFORME" "$INFORME.INVALIDA"
    echo "TANDA INVÁLIDA: $1" >&2
    echo "Informe marcado como $INFORME.INVALIDA" >&2
    exit 1
}

if [ "$TANDA" != A ] && [ ! -f "$TESTIGO" ]; then
    invalidar "nadie llegó a publicar un panel, así que esto es una tanda A con otro nombre"
fi

# **Que la ventana abriera las pestañas que se le pidieron.** Si la variable no llegó —un cambio en el
# nombre, un binario viejo— la aplicación abre una sola y el informe sale con forma de tanda de una
# pestaña bajo etiqueta de dos: exactamente el mismo engaño que el testigo evita con los paneles.
DECLARADAS=$(sed -n 's/^pestañas: *\([0-9]*\)$/\1/p' "$INFORME" | head -1)
if [ "$DECLARADAS" != "$PESTANAS" ]; then
    invalidar "el informe dice «$DECLARADAS» pestañas y se pidieron $PESTANAS"
fi

# **Que los arrastres pedidos llegaran a ocurrir.** Es el mismo engaño que el testigo evita con los
# paneles, y aquí muerde más: sin gesto durante la tanda el informe declara su coste en **cero** y esa
# tanda se lee como «el gesto no cuesta nada» en vez de como «no hubo gesto». Pasa si la variable no llega
# —un cambio de nombre, un binario viejo—, si la disposición tiene una sola consola, o si la ventana se
# cierra antes de completar el primero.
if [ "$ARRASTRES" != 0 ]; then
    INYECTADOS=$(sed -n 's/^arrastres: *\([0-9]*\) inyectados, [0-9]* reordenaron$/\1/p' "$INFORME" | head -1)
    REORDENARON=$(sed -n 's/^arrastres: *[0-9]* inyectados, \([0-9]*\) reordenaron$/\1/p' "$INFORME" | head -1)
    if [ -z "$INYECTADOS" ]; then
        invalidar "se pidieron arrastres y el informe no declara ninguna línea de arrastres"
    fi
    if [ "$INYECTADOS" = 0 ]; then
        invalidar "se pidieron arrastres y el informe dice que no se inyectó ninguno entero"
    fi
    # **Y que alguno haya REORDENADO, que es lo que de verdad mide la tanda.** Mandar el evento de soltar no
    # significa que la disposición cambie: el suelo geométrico lo rechaza cuando la consola de destino no
    # puede darle su mínimo a las dos, y con una ventana estrecha eso pasa en todos. Esa corrida inyecta mil
    # gestos, no reordena ni uno, y sin esta comprobación se publicaría como «el gesto no cuesta nada».
    if [ "$REORDENARON" = 0 ]; then
        invalidar "se inyectaron $INYECTADOS arrastres y ninguno reordenó nada: la ventana es demasiado \
estrecha para que el suelo geométrico acepte el soltado, así que esta tanda no mide el gesto"
    fi
    echo "arrastres durante la tanda: $INYECTADOS inyectados, $REORDENARON reordenaron" >&2
fi

# **Que las consolas fueran las que se pidieron.** Con la variable de la disposición sin llegar, la
# aplicación abre dos columnas y el informe sale con forma de tanda de dos divisiones bajo etiqueta de
# cuatro. Se cuentan las líneas de división, que es lo que el informe publica de verdad.
CONSOLAS=$(grep -c '^  división ' "$INFORME" || true)
if [ "$CONSOLAS" != "$DIRECTORIOS" ]; then
    invalidar "el informe publica $CONSOLAS consolas y se pidieron $DIRECTORIOS ($PESTANAS × $DIVISIONES)"
fi

# **Que la ventana abriera con el tamaño que se le pidió.** Es la comprobación que faltaba y la razón por
# la que tres corridas de la misma tanda pudieron dar tres rejillas: si el escritorio impone el suyo, la
# aplicación lo dice por su salida de errores **y** publica el que tuvo, así que aquí se ve sin adivinar.
#
# **Con margen y no con una comparación exacta**, y el margen es el de `parecido`, que es el de la
# aplicación: el compositor entrega el tamaño con fracción y no siempre la misma, así que una comparación
# exacta tiraría corridas buenas.
TUVO=$(tamano_de "$INFORME")
if ! parecido "$TAMANO" "$TUVO"; then
    invalidar "se pidió una ventana de ${TAMANO%x*}×${TAMANO#*x} puntos y el informe dice que tuvo «$TUVO»"
fi

# **LO QUE SIGUE SIN COMPROBARSE, Y CONVIENE SABERLO: que la ventana no cambiara de tamaño A MITAD.** La
# cabecera publica la rejilla **final**, así que una ventana que cambió y volvió a su sitio da un informe
# idéntico al de una corrida limpia, con los tramos de la otra rejilla dentro de las series. Se intentó
# taparlo en F2.2 y **se revirtió con dos medidas delante**: (1) el compositor manda su configuración
# mientras las consolas **ya están volcando**, así que toda corrida limpia tiene un fotograma con otro
# tamaño y sus muestras dentro; y (2) separar eso de una anomalía de verdad exige **contar** fotogramas
# contaminados y poner un umbral, que es un número que nadie puede justificar hoy porque el escenario —algo
# redimensiona la ventana a mitad— no se ha observado nunca. Queda como su propio issue.

echo "hecho: $(basename "$INFORME")" >&2
# **La primera división, por su nombre y no por su posición.** El informe las nombra con un número que
# no se reutiliza al cerrar, así que «división 0» es siempre la primera consola que abrió la ventana —
# que es la que estas tandas comparan entre corridas.
echo "rejilla: $(grep -m1 '  división 0:' "$INFORME" | tr -s ' ')" >&2
