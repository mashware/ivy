#!/bin/sh
# Takes a batch of the frame measurement **from inside the application**, which is the only way to
# see the painting (see `el-vaciado-no-ve-el-pintado` and the «Medido» annex of D3).
#
#   scripts/measure-frame.sh <batch> [mib]
#   scripts/measure-frame.sh compare <report> <report>
#
#     <batch>  With ONE tab and TWO panes, which is how it was measured up to F2.1:
#                A = no panel · B = panel sitting still · C = panel publishing at a rhythm
#                D = like C, but with the temporary file outside the watched directory. It is the
#                    batch that refutes the temporary's location mattering, and D3's annex cites it:
#                    without it what was published cannot be reproduced
#              With TWO tabs, the ones F2.1 adds — all four with a panel at a rhythm, that is C's
#              regime, so they can be compared with each other and with it:
#                F = the background tab SITTING STILL: its two consoles finish without dumping
#                    anything. It isolates what the bar costs and what pumping two panes that
#                    produce nothing costs
#                G = the background tab DUMPING: its two consoles dump just like the front ones.
#                    It isolates the background work
#              With ONE tab and FOUR panes, the ones F2.2 adds — also in C's regime, which is what
#              they are compared against:
#                H = the four IN A ROW. Each console keeps a quarter of the width, which is the
#                    panel's worst case: the open question of issue #33
#                I = the four IN A SQUARE (two columns of two). Half the width each, which is the
#                    layout somebody would actually use
#              With ONE tab and TWO panes **being dragged**, the one F2.3 adds — also in C's
#              regime, which is what it is compared against:
#                J = like C, and on top of that the window **drags itself**: it grabs a console,
#                    carries it over the other one and drops it, again and again, alternating the
#                    side. The difference between J and C is what the gesture costs
#                P = the two consoles dumping in JAPANESE. It is the ONLY batch that walks the path
#                    of searching the machine for fonts, because it is the only one whose load
#                    carries characters the factory fonts do not cover. Its control is C: same
#                    consoles, same load in size, and the only difference is the language. It
#                    invalidates the run if no font was ever searched for — an empty discovery
#                    section reads as «it costs nothing» — and the other way round too, if a batch
#                    that is not this one searched for any.
#
#                T = the two consoles with a STRIP over each of them. It is the ONLY batch that
#                    paints one: every other run has it switched off by the measurement gate, which
#                    is what keeps the eighteen published batches comparable with each other. Its
#                    control is C — same consoles, same load, and the only difference is the band.
#                    **Its grid is not the grid of any other batch**: a strip takes a row off every
#                    console, so T's own report is the only place that number can be read, and the
#                    guard below throws the run away in both directions — a batch that asks for
#                    strips and painted none, and any batch that painted one without asking.
#
#                V = the two consoles publishing over the CHANNEL and then READING THE DOCUMENT
#                    BACK, once a second. It is the ONLY batch that answers a read, and its control
#                    is **S** — same consoles, same load, same interval: V publishes exactly what S
#                    publishes and then asks for it back, so it is S **plus a read** rather than a
#                    different load under its name. **S is workload parity, not a latency control**:
#                    the read is serialised and answered on the connection thread, outside every
#                    frame span, so no S/V subtraction measures it. V proves only that the off-frame
#                    path executes under the ordinary dumping load; read latency comes from D49's
#                    separate release-mode benchmark.
#                    **Writing a document out is the one thing this channel does whose cost follows
#                    the SIZE of what somebody published** rather than the number of requests, which
#                    is why its report declares the largest document it wrote and why the guard below
#                    throws the run away in both directions.
#
#                X = S with a document NEAR THE CONTRACT'S CEILING: the same seven lines plus
#                    enough keyed filler lines to weigh at least 7/8 of the 256 KiB, published once a
#                    second over the channel. It patches nothing: it is the control of Y, and it
#                    exists because S publishes seven lines, so Y − S would be the large document
#                    and not the patch.
#                Y = X, and then PATCHING ONE LINE of that document with U's ops, every turn. It
#                    is Y − X that says what a patch costs on a long panel, where U − S says it on
#                    seven lines: the clone, the key index over every keyed line and the write-out
#                    that weighs the result land in the CHANNEL's span, and the whole-panel
#                    re-layout the patch forces lands in the PANEL's. Its report must declare a
#                    written-out size at or over the floor, and a check before anything is published
#                    withholds the witness — so the run is invalidated — when a lower bound of the
#                    document's compact form does not reach it. X feeds no size of its own, so that check is the only
#                    proof of X's.
#
#              There is NO drag batch with FOUR panes, and that is not an oversight: the gesture's
#              cycle only returns the layout to where it started **with two**. With more, inserting
#              splits the destination's weight and removing spreads it among those left, so weight
#              concentrates: measured with four in a row over 1,200 points, the first console goes
#              from 298.5 to 119.4 points in a single cycle. Every frame would be measuring a
#              different window and the mean would describe none of them. Held by
#              `the_cycle_of_two_gestures_returns_to_the_starting_shape_only_with_two_consoles`.
#              With ONE tab and TWO panes, and the PANEL'S DIVIDER being dragged, the one F2.4 adds
#              — also in C's regime:
#                K = like C, and on top of that the window grabs the panel divider of the left
#                    console and carries it in and out without stopping, alternating the side so the
#                    width comes back. The RIGHT console is not touched: it is the control batch
#                    **inside the same run**, and the difference between the two panels' spans is
#                    what the gesture costs
#
#              **Two K runs are not averaged**, and the comparator rejects them rightly: the grid
#              the report publishes is the last frame's, and in this batch the left console's width
#              is oscillating, so two runs end with a different number of columns. They are read
#              separately.
#              With the layout COMING OUT OF THE FILE, the one F2.5 adds:
#                L = **asks for neither tabs nor panes**: it seeds a known layout file — one tab
#                    with FOUR consoles in a row — and lets it decide. It is the only batch that
#                    walks the restored startup, which is the most expensive path F2.5 adds and the
#                    one that until it was **excluded from the measurement by construction**: the
#                    script wipes the configuration directory at the start of every run, and what
#                    the batch asks for wins over the file.
#                    Its control is **H**, which opens the same four in a row by asking for them:
#                    the difference between the two is what restoring costs, and L's report declares
#                    it apart — `restored:` — because it happens **before the first frame** and is
#                    subtracted from none of them.
#              With ONE tab and TWO panes, and TEXT BEING MARKED, the other one F2.6 adds — also in
#              C's regime:
#                N = like C, and on top of that the window grabs inside the LEFT console and drags
#                    to mark text, again and again, alternating how much it covers so the piece
#                    grows and shrinks. The RIGHT console is not touched: it is the control batch
#                    **inside the same run**, just as in K, so the difference between the two
#                    consoles' spans is what marking costs, measured against a control that shared
#                    machine, frame and minute.
#                    **It invalidates the run if the report declares no gesture with marked text**:
#                    since D20 the primary button on its own marks, but if the gesture lost its
#                    place — the panel's handle, the divider — the run would inject itself whole and
#                    publish the cost of something else.
#                O = THE SAME walk as N, with a single difference: the program in the LEFT console
#                    **has asked for the mouse**, so the gesture marks nothing — one report comes
#                    out per cell it crosses. The RIGHT one is still the control inside the same
#                    run. That the walk is identical is what makes the two batches comparable: the
#                    difference between them is what sending the mouse costs instead of marking.
#                    **It invalidates the run if at least one report per gesture did not come out**
#                    — the program never asked for it and what was published would be a selection
#                    under another label — **and also if it left text marked**, which would mean
#                    both paths ran at once.
#              With ONE tab and TWO panes, and the TREE'S DIVIDER being dragged, the one F2.6 adds
#              — also in C's regime:
#                M = like C, and on top of that the window grabs the divider that shares out the two
#                    consoles and carries it to one side and the other without stopping,
#                    alternating so the share comes back. It is the ONLY batch that changes the
#                    width of **two** consoles at once, and that is why it is the one that measures
#                    the worst case of reflowing the text — the span that the scrollback made more
#                    expensive, from 7 µs to almost a millisecond **per console**. Neither K nor J
#                    exercises it: K narrows the panel of ONE, and J reorders the tree without
#                    crossing cell boundaries.
#                    Its control is **C**, which opens the same two without touching them.
#
#                    **Two M runs are not averaged**, for the same reason as K's: the grid the
#                    report publishes is the last frame's and here both consoles are oscillating, so
#                    two runs end with a different number of columns. They are read separately.
#     [mib]    size of the load per console, 44 by default
#
#   The `compare` mode does not measure: it reads two reports of the same batch and says whether
#   they can be averaged.
#
# How it works, and why nobody needs to be sitting in front of it:
#
#   - Each console's program comes out of the `SHELL` variable (`crates/app/src/lib.rs`), so
#     launching the window with `SHELL=<this script in console mode>` makes both of them run the
#     load on their own. `host-load --at 30` starts them at the same time without their knowing
#     about each other.
#   - When **both** consoles finish, the window closes itself and publishes the report on its
#     standard output. Killing it with a signal publishes NOTHING: the report is printed in
#     `on_exit`.
#   - **The run does not touch the configuration of whoever is measuring**: it launches with
#     `XDG_CONFIG_HOME` pointing at a directory of its own inside `target/measurements`, and that
#     directory **is wiped at the start of every run**. It is needed from the moment the application
#     saves the layout: without it, a batch would clobber the user's window
#     and, worse, **would open with the consoles they left instead of the ones the batch asks
#     for** — and with the same console count none of the guards below fires. And the
#     application's error output goes to `<report>.err` instead of to the terminal, because
#     writing to the terminal happens **inside a measured frame**.
#
# THE WINDOW SIZE IS ASKED FOR, AND IT USED NOT TO BE. Without asking, the desktop decides it, and
# that made the measurement **irreproducible with nothing saying so**: on 17 August 2026, three runs
# of batch A on the same machine on the same day gave three different grids — 36×47, 40×66 and
# 61×57 — and every measurement in the project hangs off the grid. The cause, found by measuring:
# `NativeOptions::default()` carries no size, so GNOME on Wayland sends its own on the third frame.
# The application now asks for it and **insists** when the compositor overrides it, and publishes in
# the header the size it had — which is the only line of the report that can be asked for again.
#
# WHAT INVALIDATES A RUN, and cannot be seen in the result: the report header prints the **final**
# grid, so if the window changed size half way through, the report comes out identical to a clean
# run's. **The script checks it**, and no longer a rule written up here that had to be remembered:
# it used to say «throw the run away if the header does not say 61×57», and with two tabs it says
# 59×57 — the bar takes a row and a bit — so that rule would have discarded exactly the batches F2.1
# comes to take. What is checked now carries no number baked in:
#
#   - that the header declares **the number of tabs that was asked for**,
#   - that the window opened **with the size it was asked for**, and
#   - that the two runs of the same batch publish **the same grid** (`compare` mode). This third
#     one was announced up here from F2.1 and **was written nowhere**: it is the one that would have
#     caught the three grids of that day.
#
# And do not touch the machine meanwhile: the frame's stopwatch is a wall clock, not a CPU one.
#
# The budget per frame is 128 KiB **per pane**, not per window, so `mib` × 8 is the floor of frames
# with output **for each console that dumps**, and it does not change when tabs are added: every
# pane has its own budget. With 44 MiB that gives ≥352, of the order of the 11 August batch. What
# does grow with the tabs is the frame, because they all pump into the same one.
#
# EVERY BATCH IS RUN TWICE and the mean of the two is published, which is what the annex does. It is
# not for precision: two runs that do not look alike give away a contaminated one, and a single run
# cannot be audited. What this script does NOT reproduce is the wake-up figure the annex cites: that
# came out of a temporary probe which is not versioned.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}
# Consoles can start in another directory, so preserve a relative override's original base.
case "$CARGO_TARGET_DIR" in
    /*) ;;
    *) CARGO_TARGET_DIR="$PWD/$CARGO_TARGET_DIR" ;;
esac
export CARGO_TARGET_DIR
APP="$CARGO_TARGET_DIR/release/app"
LOAD="$CARGO_TARGET_DIR/release/host-load"
PUBLISHER="$CARGO_TARGET_DIR/release/examples/publish"

# **The reference size, in points.** Any of them will do as long as it is the same across every
# batch compared with each other; this one was chosen for fitting comfortably on the screen where
# F2.2's annex was taken and for being a round number that is easy to remember. It can be changed
# from outside to measure another window, and then **every batch of the annex has to be taken
# again**: a 1280×800 batch and an 800×600 one are not compared.
WINDOW_SIZE=${WINDOW_SIZE:-1280x800}

# **How far one size may drift from the other and still be the same, in points, and there is a
# single place in the script with this number.** It is the same one the application uses —
# `SIZE_TOLERANCE` in `crates/app/src/lib.rs` — and it has to stay that way: if the script were
# stricter, it would throw away runs the application considers good, with a message that would
# moreover blame the desktop.
SIZE_MARGIN=0.5

# The size a report says the window had, written the way the variable writes it: with «x».
size_of() {
    sed -n 's/^window: *\(.*\) points$/\1/p' "$1" | sed 's/×/x/' | head -1
}

# Whether two «<width>x<height>» sizes are the same within the margin. Empty or unreadable is never
# alike.
#
# **`LC_ALL=C` is not decoration**: `awk` converts numbers according to the locale, so in a session
# with a decimal comma `1279.8125` would be read as `1279` and a good run would come out invalid.
alike() {
    LC_ALL=C awk -v a="$1" -v b="$2" -v m="$SIZE_MARGIN" 'BEGIN {
        split(a, p, "x"); split(b, q, "x");
        if (p[1] == "" || p[2] == "" || q[1] == "" || q[2] == "") exit 1
        exit !(p[1] - q[1] <= m && q[1] - p[1] <= m && p[2] - q[2] <= m && q[2] - p[2] <= m)
    }'
}

# ── Compare mode ────────────────────────────────────────────────────────────────
# **The guard this script claimed to have from F2.1 and did not.** The annex publishes the mean of
# two runs, and averaging two runs with different grids gives a number that describes neither of
# them — and it comes out plausible. It measures nothing: it only reads the two headers.
if [ "${MEASURE_AS_CONSOLE:-}" != "1" ] && [ "${1:-}" = compare ]; then
    [ $# -eq 3 ] || { echo "usage: $0 compare <report> <report>" >&2; exit 2; }
    for report in "$2" "$3"; do
        [ -f "$report" ] || { echo "$report does not exist" >&2; exit 2; }
        # **A report without the window line cannot be compared with anything**, and staying quiet
        # would be the worst of the outcomes: two old reports — taken before the header said which
        # window it was measured in — have the same *absent* geometry, so they would come out
        # «comparable» without either of them declaring what window it was taken in. That is exactly
        # the kind of plausible number this guard comes to prevent.
        grep -q '^window: ' "$report" || {
            echo "$report does not say which window it was measured in: it predates the header" >&2
            echo "publishing it, so there is no telling whether it compares with anything." >&2
            echo "Take the batch again." >&2
            exit 1
        }
    done
    # The tabs and each pane's grid: what has to match **exactly** for the mean to mean anything. The
    # spans below are precisely what is expected to differ.
    #
    # **The window size does NOT come in here, and that is on purpose.** Comparing it character by
    # character threw away two perfectly averageable runs: the compositor hands the size over with a
    # fraction and not always the same one — 1280×800 in one run and 1279.8125×799.8125 in another —
    # both within the margin and **with the same grid**. And since the fraction is not
    # deterministic, taking them again could fail again. The size is compared below, with the same
    # margin the application uses.
    # **And each pane's NAME does not come in either, for the same kind of reason as the size.** What
    # decides whether two runs can be averaged is how many consoles there were and with what grid
    # each **place**, not which console was occupying each place at closing time. Ever since the
    # window drags itself (batch J) the consoles **change places during the run**, so which one ends
    # up on the left depends on whether the last gesture got to complete — and with the name in,
    # two runs identical in window, tabs and grids came out «not comparable» over a datum that
    # changes nothing of what is measured. Measured: two J runs from 18 August 2026, both 1280×800
    # with both consoles at 49×39, rejected over that alone.
    #
    # **And where the consoles came from, which is not geometry but decides whether two runs are the
    # same thing.** A restored run and an asked-for one are **identical in window, tabs and grids by
    # construction** — that is the whole reason the restoring batch and its control exist — so
    # without this line the comparator gave the go-ahead to averaging one with the other: exactly
    # the disguise the report's `restored:` line exists to remove. Its presence **and** its number
    # are compared.
    geometry() {
        sed -n \
            -e 's/^tabs: */tabs /p' \
            -e 's/^  pane [0-9]*: */pane /p' \
            -e 's/^restored: *\([0-9]*\) consoles .*$/restored \1/p' \
            "$1"
    }
    first=$(geometry "$2")
    second=$(geometry "$3")
    if [ "$first" = "$second" ] && alike "$(size_of "$2")" "$(size_of "$3")"; then
        echo "both runs describe the same window: they can be averaged" >&2
        printf 'window %s points\n%s\n' "$(size_of "$2")" "$first" >&2
        exit 0
    fi
    echo "THE TWO RUNS ARE NOT COMPARABLE: they describe different windows, so their mean" >&2
    echo "describes neither of them. Take them again." >&2
    printf '%s:\nwindow %s points\n%s\n%s:\nwindow %s points\n%s\n' \
        "$2" "$(size_of "$2")" "$first" "$3" "$(size_of "$3")" "$second" >&2
    exit 1
fi

# ── Console mode ────────────────────────────────────────────────────────────────
# The application invokes this very script as if it were the user's shell. It dumps the load and
# finishes; when all of them finish, the window closes.
if [ "${MEASURE_AS_CONSOLE:-}" = "1" ]; then
    # **Which console number I am, without races.** In batch F the two consoles of the background
    # tab have to finish WITHOUT dumping, and a console cannot know that by itself: its `PANEL_ID`
    # is an eight-digit draw, it does not say which tab it belongs to. What we do know is the order
    # in which the application opens them — tab by tab, left and then right — so handing out turns
    # is enough. `mkdir` is atomic even across processes, so the first one that manages to create a
    # number keeps it: no locks and no trusting them to start in order.
    turn=0
    while :; do
        turn=$((turn + 1))
        mkdir "$MEASURE_RUNS/$turn" 2>/dev/null && break
        if [ "$turn" -gt 32 ]; then
            echo "no turns left: were more consoles launched than expected?" >&2
            exit 1
        fi
    done

    # The ones over the quota finish straight away and without writing anything. **Finish and do not
    # sleep**: a sleeping console is still alive, and the window only closes when all of them have
    # finished — the batch would never end. Dead and quiet is exactly what «the background tab
    # sitting still» means.
    if [ "$turn" -gt "${MEASURE_TRIES:-2}" ]; then
        exit 0
    fi

    # **In the mouse batch, the FIRST console asks for it and the others do not.** It is what turns
    # the same walk into reports to the program instead of into a selection, and what leaves the
    # second one as a control inside the same run — just like K with the panel and N with the marked
    # text. It is asked for before starting the load so the window has read it by the time the
    # gestures start arriving.
    if [ "${MEASURE_GESTURE:-}" = mouse ] && [ "$turn" = 1 ]; then
        printf '\033[?1000h\033[?1002h\033[?1006h'
    fi

    # **Batch W writes both terminal link sources on the top two rows and keeps them out of the
    # scrolling.** The ordinary load runs below them: the scroll region starts at the third row, so
    # the dump never carries the links away and a point the injector found is still a link when it
    # clicks — which is what lets the guard demand an activation for every gesture. They start at the
    # twenty-first column, after a space of their own, because the load paints its own counter from
    # the top-left corner and a stray character glued to «https» would end the plain link. The
    # region is asserted again on every rewrite, inside the saved cursor, so anything that resets the
    # margins costs at most one rewrite's interval.
    if [ "${MEASURE_LINKS:-}" = 1 ]; then
        printf '\033[3;r\033[3;1H'
        "$LOAD" run --mib "${MEASURE_MIB:-44}" --at 30 --tag "${MEASURE_BATCH:-x}" &
        load_pid=$!
        while kill -0 "$load_pid" 2>/dev/null; do
            printf '\0337\033[3;r\033[1;20H https://example.test/plain\033[K\033[2;20H \033]8;;https://example.test/osc\033\\OSC-link\033]8;;\033\\\033[K\0338'
            sleep 0.02
        done
        wait "$load_pid"
        exit $?
    fi

    # **Only batch P asks for the CJK load.** The usual one is bit for bit the same as in every
    # published annex: changing it would leave a new batch unable to compare with any of them.
    if [ "${MEASURE_CJK:-}" = 1 ]; then
        exec "$LOAD" run --mib "${MEASURE_MIB:-44}" --at 30 --tag "${MEASURE_BATCH:-x}" --cjk
    fi
    # **And only batch Q asks for the emoji load**, for the same reason: it is the only one that
    # walks the path of rasterising a colour glyph.
    if [ "${MEASURE_EMOJI:-}" = 1 ]; then
        exec "$LOAD" run --mib "${MEASURE_MIB:-44}" --at 30 --tag "${MEASURE_BATCH:-x}" --emoji
    fi
    exec "$LOAD" run --mib "${MEASURE_MIB:-44}" --at 30 --tag "${MEASURE_BATCH:-x}"
fi

# ── Batch mode ──────────────────────────────────────────────────────────────────
BATCH=${1:?the batch is missing: A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y or Z}
MIB=${2:-44}
OUTPUT="$ROOT/target/measurements"
mkdir -p "$OUTPUT"
# **The witness that somebody got to publish.** The seeding runs in the background, so if it fails —
# the directories do not show up, a `mv` falls over — the window carries on and publishes a complete
# report: one shaped like batch A under label B, C or D, and nothing downstream would give it away.
# The panel directories disappear when the application closes, so the check cannot be after the fact
# over them: it has to leave a trace outside.
WITNESS="$OUTPUT/.seeded"
rm -f "$WITNESS"

# How many tabs the window opens, with what console layout each, and how many dump — derived from
# the letter. The application opens the consoles **tab by tab and in the order they are painted**,
# so with two tabs the first two are the front ones and the last two the background ones, and with
# four in a square the order is the left column top to bottom and then the right one.
# **A batch is defined in ONE single place, its drags included.** They lived in a separate `case`
# and the review threw that away: with two, adding a batch takes two edits and the one that gets
# forgotten falls into the second one's `*)` with zero gestures — that is, a new batch meant to drag
# would sit still and the script would invalidate it for declaring no drags, instead of simply
# working.
# **And a batch may ask for the FILE to decide instead of the request.** Not by default: `RESTORES=0`
# is up here and not repeated in every letter so that adding a batch is still editing **one line**,
# which is the rule this `case` already had. What changes with `RESTORES=1` is where the consoles
# come from, not how many: `TABS` and `PANES` still describe the batch's layout, and they are the
# ones seeded into the file and the ones the report is checked against.
RESTORES=0
# **Every knob a row of the table can set is cleared here and not left to the caller**, because the
# seeding and the guards read them from this shell: a knob exported by whoever measures reaches every
# batch, and the guards read the same variable, so they judge the leaked load as the batch's own. A
# `PATCHES=1` in the environment runs X as Y and publishes it under X's name; a `LARGE=1` pads S and
# the file batches to the ceiling, and hides U and V from the leak guard that only fires without it.
ACTIONS= CONTROL= HEADER= READS= PATCHES= LINKS= SEARCH= LARGE= CJK= EMOJI= PANEL_EMOJI=
case "$BATCH" in
    A|B|C|D) TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console ;;
    E)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=search; SEARCH=1 ;; # two, and SEARCHING the left console's scrollback while both dump: its control is C, LEFT against LEFT, because the two consoles of one run already differ with no gesture (D61)
    F)       TABS=2; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console ;;   # the background one, still: none of them dumps
    G)       TABS=2; PANES=2x1; DUMPING=4; DRAGS=0; GESTURE=console ;;   # the background one, dumping like the front one
    H)       TABS=1; PANES=4x1; DUMPING=4; DRAGS=0; GESTURE=console ;;   # four in a row: the panel's worst case (#33)
    I)       TABS=1; PANES=2x2; DUMPING=4; DRAGS=0; GESTURE=console ;;   # four in a square
    J)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=console ;;   # two, and dragging: F2.3's gesture
    K)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=panel ;;     # two, and narrowing the panel without stopping: F2.4's gesture
    L)       TABS=1; PANES=4x1; DUMPING=4; DRAGS=0; GESTURE=console; RESTORES=1 ;;  # four in a row, but out of the file: F2.5's startup
    M)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=divider ;;       # two, and moving the tree's divider: the worst case of reflowing
    N)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=mark ;;         # two, and marking text in the left one: the right one is the control
    O)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=mouse ;;           # two, and the SAME walk, but the left program asked for the mouse
    P)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; CJK=1 ;;       # two, and both dumping in JAPANESE: the only one that discovers fonts
    Q)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; EMOJI=1 ;;     # two, and both dumping EMOJI: the only one whose CONSOLES rasterise colour glyphs (Z does it from its panels)
    R)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=button; ACTIONS=1 ;;   # two, and pressing a button on a panel: its control is C
    S)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; CONTROL=1 ;;       # two, and both published over the CHANNEL instead of by file: its control is C
    T)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; HEADER=1 ;;        # two, and the only one that PAINTS THE STRIP over each console: its control is C
    U)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; CONTROL=1; PATCHES=1 ;; # two, and the only one that PATCHES the document over the channel: its control is S (and S, U and V publish one field the file batches do not — see `document_of`)
    V)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; CONTROL=1; READS=1 ;;  # two, and the only one that PROVES THE OFF-FRAME READ PATH RAN: its control is S, but their frame delta is not read latency
    X)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; CONTROL=1; LARGE=1 ;;            # S, with a document near the ceiling: the control of Y
    Y)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; CONTROL=1; LARGE=1; PATCHES=1 ;; # X, and patching it with U's ops: its control is X
    Z)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; PANEL_EMOJI=1 ;;  # two, and the only one whose PANELS carry emoji: the consoles dump ASCII, so its control is C
    W)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=link; LINKS=1 ;; # two, and OPENING links of all three sources with the desktop hand-off suppressed: its nearest is C, but its consoles also rewrite two link rows and its panel paints a link label, so W − C is not the gesture alone
    *) echo "unknown batch «$BATCH»: use A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y or Z" >&2; exit 2 ;;
esac
# **Why the dragging batch asks for a hundred thousand gestures**: the number is deliberately above
# what fits, because what is needed is for the gesture to last **from start to finish**. A batch that
# runs out of gestures half way averages two different regimes and publishes the mean of both as if
# it were one. How many actually happened is what the report says, and **that** is the number that
# counts: the report publishes the injected ones and the effects they caused, not the requested ones.

# The consoles per tab come out of the layout, which is «columns by rows».
PER_TAB=$(( ${PANES%x*} * ${PANES#*x} ))
DIRECTORIES=$((TABS * PER_TAB))

# The consoles' turns live in a directory that is clean per run: if the previous run's were left,
# every console of this one would be over quota and none would dump.
RUNS="$OUTPUT/.turns"
rm -rf "$RUNS"
mkdir -p "$RUNS"


# **Every binary the chosen batch needs, before anything else.** Without `host-load` the two consoles
# die on the spot and the window closes: out comes a report with a header and no spans, which at a
# glance looks like a valid run. And if the application cannot launch this script as a shell, it falls
# back to the system's and the two consoles sit at a prompt: the window never closes and publishes
# nothing.
#
# **The publisher is on this list and the build line names it**, because a missing example is not an
# error anybody sees: backgrounding it succeeds, the job exits, and the batch burns a whole window
# before invalidating over something else entirely.
set -- "$APP" "$LOAD"
BUILD_WITH="cargo build --release -p app -p host-load"
if [ "${CONTROL:-}" = 1 ]; then
    set -- "$@" "$PUBLISHER"
    # **`--example` is target selection, not an addition to the packages named.** Spelled on one
    # line as `-p app -p host-load --example publish -p panel-control`, cargo builds the example and
    # none of the packages' default binaries: whoever runs the printed line on a clean tree gets the
    # publisher and the next preflight fails on `$APP`. Two invocations, so both halves happen.
    BUILD_WITH="$BUILD_WITH && cargo build --release -p panel-control --example publish"
fi
for binary do
    [ -x "$binary" ] || { echo "$binary is missing — build with: $BUILD_WITH" >&2; exit 1; }
done

# **With a window open nothing is measured, and even less is anything deleted.** The panel
# directories do not carry the pid of whoever created them, so the cleanup below cannot tell the
# leftovers of a previous run from those of a live working session: it would delete them all. And
# even if it deleted nothing, a second window competes for the machine and the frame's stopwatch is
# a wall clock.
if pgrep -x app > /dev/null 2>&1; then
    echo "there is an agent-terminal window open: close it before measuring" >&2
    exit 1
fi

# Now yes: whatever is left is from a previous run. An old panel would falsify the whole of batch A
# — which is the reference — and in the others would give a panel that is not the one this run wants
# to measure.
rm -rf /run/user/"$(id -u)"/agent-terminal-* 2>/dev/null || true

# **And the run has its own configuration directory, for two different reasons.** It goes down here
# and not next to the turns one **on purpose**: this deletes, and the script deletes nothing until
# it has checked that there is no window open. (The turns one is deleted before that check; it comes
# earlier than this line and is not touched here.) The first is out of respect: the moment the
# application saves and restores the layout, a batch using the user's directory **would clobber
# theirs** and would moreover open with the consoles they left, not the ones the batch asks for. The
# second is this script's usual one: **clean per run**, because otherwise the second run of a batch
# would start with what the first one left — same console count, so no guard fires — and would be
# measuring a layout nobody asked it for.
#
# **What it isolates is this project's layout file** — the one D16 puts in
# `$XDG_CONFIG_HOME/agent-terminal/` — and not «the configuration» in general. Said that way because
# the persistence `eframe` brings out of the box resolves another variable, `XDG_DATA_HOME`
# (`eframe/src/native/file_storage.rs`), and **it is deliberately not covered**: D16 discarded that
# store, so covering it would be a mechanism with no scenario. If it were ever used, **this line
# would stop isolating without saying so** and one has to come back here.
#
# **This crate does read this variable, and the sentence that used to sit here said it did not.**
# `crates/app/src/settings.rs` resolves the settings file through `XDG_CONFIG_HOME`, and
# `crates/panel-contract` reads `XDG_RUNTIME_DIR`. The claim was true when it was written and stopped
# being true the day the terminal became configurable, which is the way a comment about present
# behaviour rots: nothing failed, nobody noticed, and it went on reassuring a reader for two
# deliveries. **What isolating the directory is worth has not changed** — under the measurement gate
# the settings file is not read at all, so the isolation protects the layout rather than the
# settings — and the process still reads the variable through system libraries that load themselves:
# `strace` on the binary gives four accesses, to libX11's `XCompose`, to `dconf/user` and to
# fontconfig's two. It makes no difference to what matters here, and that is **measured, not
# reasoned**: four runs of batch A, two with this line and two without it, gave the same window and
# the same grid.
CONFIG="$OUTPUT/.config"
rm -rf "$CONFIG"
mkdir -p "$CONFIG"

# **The seeded layout, and only in the batches that restore.** It is written **before** opening the
# window and in the run's isolated directory, so what the application finds at startup is exactly
# this and not whatever anybody left. The shape is D16's contract: a document with its version, a
# list of tabs, and every tab with its tree of branches and leaves.
#
# **The weights are all 1 on purpose.** The share-out normalises by its branch's sum, so four ones
# give four quarters: the same grid batch H gets by asking for four panes, which is what makes H
# usable as a control. With different weights, the difference between the two batches would include
# the share-out and would no longer be only what restoring costs.
#
# **The panel width is the factory one**, for the same reason: a batch seeding another width would
# measure consoles narrower than H's and would publish that difference as startup cost.
seed_layout() {
    columns=${PANES%x*}
    rows=${PANES#*x}
    leaf='{"leaf":{"panel_width":36,"folded":false}}'

    if [ "$rows" = 1 ]; then
        column_branch="$leaf"
    else
        children=""
        n=0
        while [ "$n" -lt "$rows" ]; do
            [ -z "$children" ] || children="$children,"
            children="$children{\"weight\":1.0,\"node\":$leaf}"
            n=$((n + 1))
        done
        column_branch="{\"branch\":{\"orientation\":\"rows\",\"children\":[$children]}}"
    fi

    # **With a single column, the root IS that column: it is not wrapped.** A branch with a single
    # child is a document the validator **rejects** — `SingleChildBranch` — so `1x2` would seed a
    # file that is not understood, the window would open with the factory layout and the batch would
    # be measuring something else. The guard at the end catches it, but fifteen minutes later.
    # Checked by generating the document and running it through the interpreter.
    if [ "$columns" = 1 ]; then
        root="$column_branch"
    else
        children=""
        n=0
        while [ "$n" -lt "$columns" ]; do
            [ -z "$children" ] || children="$children,"
            children="$children{\"weight\":1.0,\"node\":$column_branch}"
            n=$((n + 1))
        done
        root="{\"branch\":{\"orientation\":\"columns\",\"children\":[$children]}}"
    fi

    list=""
    n=0
    while [ "$n" -lt "$TABS" ]; do
        [ -z "$list" ] || list="$list,"
        list="$list{\"root\":$root}"
        n=$((n + 1))
    done

    mkdir -p "$CONFIG/agent-terminal"
    printf '{"v":2,"tabs":[%s]}\n' "$list" > "$CONFIG/agent-terminal/layout.json"
}

if [ "$RESTORES" = 1 ]; then
    seed_layout
fi

# **The document every batch publishes, written once.** The file-seeded path fills the turn in
# directly; the channel's publisher gets the literal `{n}` and substitutes the turn it is on (see
# `crates/panel-control/examples/publish.rs`). One body because a channel batch with a document of
# its own would be measuring a different panel from its control — `C` is `S`'s control — and nothing
# but this one function keeps two copies from drifting.
document_body() {  # document_body <which of the consoles> <turn> [the key on the note line]
    # **An address on one line, and only where it is asked for.** A key is never painted and the
    # parse only stores it, so this changes nothing about what a panel costs to draw — but it does
    # change the bytes, and a batch compared against another has to publish the same ones. Only the
    # channel batches ask for it, which keeps every file-seeded document exactly as it was.
    # **An `if` and not `[ … ] && …`**, because this function's body runs inside a command
    # substitution: with `set -e` on, a bare test that comes out false is the last command of
    # its list and would end the subshell, handing the caller half a document and no error.
    note_key=''
    if [ -n "${3:-}" ]; then
        note_key="\"key\":\"$3\","
    fi
    # **The attention goes into what is seeded now that the tab shows it.** Without it, the bar
    # would be painted with nobody asking for anything: it would walk the same — the walk does not
    # depend on there being attention — but it would paint not a single symbol, and the batch would
    # publish what the bar costs **without the half this milestone added**. It is shared out per
    # console so that different classes live together in one tab, which is what forces the tab to
    # compare: the one on an odd turn waits, the even one is taken as finished.
    if [ $(( $1 % 2 )) = 0 ]; then attention=done; else attention=wait; fi
    # **Only batch R declares buttons**, for the same reason only P dumps Japanese and only Q dumps
    # emoji: they are the one path that lays out a button's rows, walks them to work out what a press
    # may reach, and composes what one sends. Any other batch painting one would be mixing foreign
    # work into the spans, and the report's `buttons:` line is read in both directions to say so.
    #
    # **Eight of them, which is the ceiling the contract accepts**, so what the batch measures is the
    # worst case a producer can declare rather than a convenient one.
    # **Empty means the field is not written at all**, not an empty list: the composed document puts
    # this straight in front of `"lines"`, so a value with no trailing comma would leave every other
    # batch publishing a panel that is not JSON — which is how the first version of this was found.
    actions=''
    panel_link=''
    # **Read as $ACTIONS and not as $MEASURE_ACTIONS**: this seeding runs in the batch's own shell,
    # where the batch table set it, while the MEASURE_ prefixed ones are what the window and its
    # consoles see. Reading the child's name here left the document with no buttons and the batch
    # invalidated itself, which is the guard doing its job on its author.
    if [ "${ACTIONS:-}" = 1 ]; then
        actions='"actions":[
  {"label":"Run the tests","send":"echo batch"},
  {"label":"Continue","send":"echo continue"},
  {"label":"Open the request","send":"echo request"},
  {"label":"Publish the report","send":"echo report"},
  {"label":"Take the layout back","send":"echo layout"},
  {"label":"Narrow the panel","send":"echo panel"},
  {"label":"Close the console","send":"echo console"},
  {"label":"Say what it cost","send":"echo cost"}],'
    fi
    if [ "${LINKS:-}" = 1 ]; then
        panel_link=',"link":"https://example.test/panel"'
    fi
    # **Only X and Y carry the filler**, read from the batch's own shell like $ACTIONS. It goes
    # **after** the seven lines so the rows on screen are the ones every other channel batch paints,
    # and it is many lines of a working panel's length and never one long one: a single line near the
    # ceiling measures the painter's layout (65.8 ms the first time) instead of the patch.
    filler=''
    if [ "${LARGE:-}" = 1 ]; then
        filler=$(large_filler)
    fi
    # **Only batch Z puts emoji in its panels**, one at the head of each of the first four text lines,
    # read from the batch's own shell like $ACTIONS. They go in front of the words that were already
    # there rather than replacing any, so every row the control paints is still painted. **And the
    # same number of rows**: each emoji is two characters more on its line, which could wrap a line
    # that fit in C, and at the width a batch window gives the panel — thirty-four cells of text — both
    # documents lay out into thirteen rows, measured by laying them out with the panel's own code. Four
    # different ones, so the atlas rasterizes four glyphs once and then only draws them — the steady
    # state of a panel somebody published with emoji in it.
    #
    # **None of them carries a selector, and that is not an omission.** The panel notes every
    # character of its text for font discovery, `U+FE0F` included, and no face egui ships covers the
    # selector: a `☁️` here sends the window off to search the machine, which is batch P's work, and
    # the guard that only P may search invalidates the run.
    e1='' e2='' e3='' e4=''
    if [ "${PANEL_EMOJI:-}" = 1 ]; then
        e1='🔥 ' e2='✅ ' e3='📦 ' e4='🚀 '
    fi
    cat <<JSON
{"v":1,"stale_after_minutes":600,"attention":"$attention",$actions"lines":[
  {"text":"${e1}Batch $BATCH · publication $2","style":"title"},
  "",
  {"ref":"load","text":"${e2}$MIB MiB per console, TUI-style dump","mark":"current"},
  {"ref":"panel","text":"${e3}regime $BATCH — this is what is being measured","mark":"info"},
  {$note_key"ref":"note","text":"${e4}the layout is redone on every good re-read","mark":"info"},
  "",
  {"text":"six lines and a warning: the size of a real working panel","mark":"done"$panel_link}$filler
]}
JSON
}

# **How many filler lines, and the floor they have to reach.** The floor is 7/8 of the contract's
# `MAX_BYTES` (262 144), written here as a literal because this is shell, and held against the
# constant by `crates/app/tests/batches.rs` so a ceiling that moves cannot silently move «near» with
# it. The count is sized so the compact document lands between the floor and the ceiling with room
# for the patch's write-out, which is the pane's own `serialize` and not these bytes.
LARGE_LINES=1590
LARGE_FLOOR=229376
large_filler() {  # every line keyed, so the index a patch builds is at its widest
    fill_n=1
    while [ "$fill_n" -le "$LARGE_LINES" ]; do
        printf ',\n  {"key":"fill-%d","ref":"%d","text":"%s","mark":"done"}' "$fill_n" "$fill_n" \
            "a step of a long plan, of the length a working panel writes one: what it did, where, and what is next"
        fill_n=$((fill_n + 1))
    done
}

publish() {  # publish <directory> <n> <which of the consoles>
    # **Where the temporary lands does NOT matter, and batch D exists for having checked it.** The
    # suspicion was that a temporary inside the watched directory would wake the window on its own
    # when closed, doubling the wake-ups per publication. It is false by construction:
    # `watcher::raise_matching` only raises the flag when the notice carries the exact path
    # `<dir>/panel.json`, so any other file in the directory is invisible. D confirms it by
    # measuring, and it is kept so the hypothesis is not raised again.
    if [ "$BATCH" = D ]; then
        tmp="/run/user/$(id -u)/.measure-$(basename "$1").json"
    else
        tmp="$1/.measure.json"
    fi
    document_body "$3" "$2" > "$tmp"
    mv "$tmp" "$1/panel.json"   # in one go, as the contract requires
}

# **What the channel batches publish is that same body**, with the turn left as the literal `{n}` for
# the publisher to substitute per publication — **and with an address on one of its lines**, which is
# the one difference from what the file-seeded batches write.
#
# **The key goes on all three channel batches and not only on the one that patches.** `U` needs a line
# it can name; `S` is its control and `V`'s, and a control that publishes fewer bytes than the batch
# it is compared against is not a control. The file batches are left byte for byte as they were, so
# nothing outside this trio moves.
#
# **What that costs, said here and in the batch table rather than left to be discovered.** `S`'s own
# control is `C`, which is file-seeded, so `S` now publishes thirteen bytes `C` does not — and `S`
# publishes thirteen bytes more than the `S` runs already measured for the reading milestone. A key
# is **never painted and never read** by the application (nothing in `crates/app/src/panel.rs`
# touches it), so what it costs is one more string kept by a parse that already runs once a second;
# the alternative — putting it on every batch — would move all eighteen measured runs instead of
# three, which is worse for the same reason.
document_of() {  # document_of <which of the consoles>
    document_body "$1" '{n}' note
}

# **What batch U sends after each publication**, declared here beside the document for the same
# reason the document is declared here at all: a load written inside the publisher is a load this
# table does not say out loud.
#
# **Two ops, one of each kind the frame pays differently for**: one addressed by key, which is the
# only work a patch adds that the document's size drives, and one scalar, which is the cheapest op
# there is. **Both write back exactly what is already there** — the replacement is the line it
# replaces word for word, and the scalar is the expiry the document already declares — so what is
# painted after the patch is what would have been painted without it. The batch differs from its
# control in the work, not in the picture.
#
# **The scalar is the expiry and not the phase, and that is not interchangeable.** `phase` is
# painted, on the identity row: setting it would have made this batch draw «· patched ·» where its
# control draws nothing, ten cells laid out every frame and a row that wraps differently on a narrow
# panel — and the difference published as what a patch costs. The expiry is read and never drawn,
# and 600 is what the document above already says, so setting it moves no pixel. **D13**, caught in
# review.
OPS='[{"op":"replace","key":"note","line":{"key":"note","ref":"note","text":"the layout is redone on every good re-read","mark":"info"}},
      {"op":"set","field":"stale_after_minutes","value":600}]'

# **One publisher per console, each holding its connection open for the whole run.** A connection per
# publication would claim and release the pane every turn, which measures the holder's churn instead
# of the path an agent really takes. The rate is the same one the file seeding uses — once a second —
# so the two batches differ in the transport and in nothing else.
# **The identifier is cut with parameter expansion and not with `sed`**, and that is not taste: a
# guard in this repository extracts every pattern the script runs over the report and checks it still
# matches something, and a `sed` here — over a directory name, not over the report — is picked up by
# that extractor and fails it. Keeping the shell's own expansion leaves the guard reading only what
# it is about.
publish_over_the_channel() {  # publish_over_the_channel <directories>
    which=0
    publishers=""
    # **The verb the load uses, and the three batches differ in this and in nothing else.** `S`
    # publishes a whole document every second; `V` publishes one and then asks for it back; `U`
    # publishes one and then changes a line of it by name. Same consoles, same dumping, same
    # interval, same bytes published — which is what makes `S` an honest control for either of them
    # rather than a run that happens to be nearby.
    verb=publish
    [ "${READS:-}" = 1 ] && verb=read
    [ "${PATCHES:-}" = 1 ] && verb=patch
    # **Every document is weighed before any is published**, so a failure leaves no publisher holding
    # a pane for the hour it was launched for. The floor is checked on a lower bound of the compact
    # form the producer actually sends: the bytes minus three per newline, because each newline of
    # this body is followed by at most two spaces of indentation and the wire carries neither.
    # Stripping every space instead would also strip the ones inside the texts — about an eighth of
    # the document, enough to fall under the floor. X feeds no written-out size of its own — only
    # reads and patches do — so this is the only proof that the control is as large as Y. A failure
    # returns before the witness is written, which is what invalidates the run.
    if [ "${LARGE:-}" = 1 ]; then
        for d in $1; do
            which=$((which + 1))
            document=$(document_of "$which")
            bytes=$(printf '%s' "$document" | wc -c)
            lines=$(printf '%s' "$document" | wc -l)
            compact=$((bytes - 3 * lines))
            if [ "$compact" -lt "$LARGE_FLOOR" ]; then
                echo "the large document weighs at least $compact bytes on the wire, under the floor \
of $LARGE_FLOOR: this batch would publish a medium panel under the ceiling's name" >&2
                return 1
            fi
        done
        which=0
    fi
    for d in $1; do
        which=$((which + 1))
        # **On standard input and not as an argument**: Linux refuses a single argument longer than
        # `MAX_ARG_STRLEN` (32 pages, 128 KiB with 4 KiB pages), and X's document is about twice that.
        # A here-document reaches `cat` on its standard input, and a pipe carries it on, so the
        # document is never an argument of anything and that limit never applies.
        document_of "$which" | "$PUBLISHER" \
            "$d" "${d##*agent-terminal-}" "bench" \
            3600 1000 "$verb" "$OPS" >&2 &
        publishers="$publishers $!"
    done

    # **The witness is written once every publisher is confirmed to have survived its first second**,
    # and not when they were backgrounded. Backgrounding a command that cannot run succeeds: the job
    # exits immediately and the loop above never notices, so a run where nobody published anything
    # would be published as one where somebody did — which is exactly the deceit this witness exists
    # to prevent, and it is not hypothetical here: the preflight used not to build this example, so
    # the ordinary first run of the batch went down this path.
    sleep 1
    for pid in $publishers; do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "a publisher died before it published anything over the channel" >&2
            return 1
        fi
    done
    : > "$WITNESS"
}

# The directories are created by the application at startup, so they have to be waited for. They are
# not read from PANEL_DIR: that variable is seen by the programs inside, not by a process outside.
seed() {
    mode=$1
    waited=0
    # **ALL the directories are waited for, and only then is it checked that they are exactly
    # those.** It used to wait for there to be two and list whatever was there: with two tabs there
    # are four, so the wait was satisfied by the first two and the listing picked up two, three or
    # four depending on how the race fell — a batch with some consoles having a panel and others
    # not, that is, with the regime mixed, and published as good because the witness was written all
    # the same.
    while [ "$(ls -d /run/user/"$(id -u)"/agent-terminal-* 2>/dev/null | wc -l)" -lt "$DIRECTORIES" ]; do
        waited=$((waited + 1))
        [ "$waited" -gt 100 ] && { echo "the $DIRECTORIES panel directories did not show up" >&2; return 1; }
        sleep 0.1
    done
    dirs=$(ls -d /run/user/"$(id -u)"/agent-terminal-*)
    count=$(echo "$dirs" | wc -l)
    if [ "$count" -ne "$DIRECTORIES" ]; then
        echo "$count panel directories showed up and $DIRECTORIES were expected: nothing is seeded" >&2
        return 1
    fi

    # **Batch S never writes a file**, which is the whole of what it measures: the channel's path
    # does not touch the disk, and a batch that seeded both would publish the sum of the two.
    if [ "${CONTROL:-}" = 1 ]; then
        # **The socket, not the directory.** `Pane::with_panel` creates the panel directory, execs
        # the program and only then binds the endpoint, so a directory observed in between makes
        # every publisher fail with ENOENT — and round 2's own liveness check turns that race into an
        # invalidated run, with a surviving publisher left holding its pane for an hour. Waiting for
        # `control.sock` is waiting for the thing the publisher actually needs.
        waited=0
        for d in $dirs; do
            while [ ! -e "$d/control.sock" ]; do
                waited=$((waited + 1))
                [ "$waited" -gt 100 ] && { echo "the control endpoint in $d did not show up" >&2; return 1; }
                sleep 0.1
            done
        done
        publish_over_the_channel "$dirs"
        return $?
    fi

    if [ "$mode" = quiet ]; then
        n=0
        for d in $dirs; do n=$((n + 1)); publish "$d" "$n" "$n"; done
        : > "$WITNESS"
        return 0
    fi

    # Talkative: it republishes for as long as the window lives. Once a second — the contract
    # measures one publication in 1 frame, so this is the recommended case repeated, not the
    # pathological one. And it overlaps with the dumping on purpose: a publication frame with no
    # console output would not enter the report's series.
    n=0
    while pgrep -x app > /dev/null 2>&1; do
        n=$((n + 1))
        which=0
        for d in $dirs; do which=$((which + 1)); publish "$d" "$n" "$which"; done
        : > "$WITNESS"
        sleep 1
    done
}

case "$BATCH" in
    A) : ;;                       # no panel: nobody publishes anything
    B) seed quiet & ;;
    C|D|E|F|G|H|I|J|K|L|M|N|O|P|Q|R|S|T|U|V|W|X|Y|Z) seed talkative & ;;
esac

echo "batch $BATCH · $TABS tab(s) of $PANES · $DUMPING of $DIRECTORIES consoles dumping $MIB MiB · window $WINDOW_SIZE · do not touch the machine until it closes" >&2

# An absolute path on purpose: the console does not inherit the directory the batch was launched
# from, and a relative `$0` would leave the application looking for a shell that does not exist.
REPORT="$OUTPUT/batch-$BATCH-$(date +%H%M%S).txt"
# **The exit status is captured, and that became necessary once the error output goes to a file.**
# With `set -e` a non-zero status cut the script off **at the launch line**: neither the warning
# below nor the invalidation got to run, and the panic — which used to be read in the terminal —
# stayed in a file nobody named.
STATUS=0
export MEASURE_AS_CONSOLE=1 MEASURE_MIB="$MIB" MEASURE_BATCH="$BATCH" \
    MEASURE_RUNS="$RUNS" MEASURE_TRIES="$DUMPING" \
    MEASURE_SIZE="$WINDOW_SIZE" MEASURE_DRAGS="$DRAGS" MEASURE_GESTURE="$GESTURE" MEASURE_CJK="${CJK:-}" \
    MEASURE_EMOJI="${EMOJI:-}" MEASURE_ACTIONS="${ACTIONS:-}" \
    MEASURE_LINKS="${LINKS:-}" \
    XDG_CONFIG_HOME="$CONFIG" \
    SHELL="$ROOT/scripts/measure-frame.sh"
# **The two that ask for a layout are not exported when the batch restores, and that is what batch L
# consists of.** The application opens with the factory layout **as soon as either of the two is
# there** — on purpose, so a batch does not depend on whatever had been saved — so seeding the file
# **and** asking for four panes would give the same report with the file not even read: four
# consoles, the same grid, and no guard firing. The report's `restored:` line is what tells the two
# things apart, and that is why it is checked below.
# **The strip's knob is exported by PRESENCE and so it cannot travel with an empty value**, unlike
# its five neighbours above. The application asks whether the variable is there, not what it says —
# that is what keeps the eighteen published batches comparable, because a knob read as `= 1` would
# turn every batch that forgot it into a batch with strips — so exporting `MEASURE_HEADER=""` on
# every run would paint one everywhere. It is unset for the same reason `MEASURE_TABS` is below: one
# the user had exported by hand would otherwise reach the application and put a row under every
# console of a batch that is compared against runs with none.
if [ "${HEADER:-}" = 1 ]; then
    export MEASURE_HEADER=1
else
    unset MEASURE_HEADER
fi
if [ "$RESTORES" = 0 ]; then
    export MEASURE_TABS="$TABS" MEASURE_PANES="$PANES"
else
    # **And they are removed from the environment; not setting them is not enough.** They used to go
    # in front of the command, so they fixed their value for the child **whatever came** from
    # outside; with a conditional `export`, one the user had exported beforehand — which is how the
    # documentation says to open a particular layout by hand — would reach the application and make
    # it open with the factory layout, with the seeded file not even read.
    unset MEASURE_TABS MEASURE_PANES
fi
"$APP" > "$REPORT" 2> "$REPORT.err" || STATUS=$?

wait 2>/dev/null || true

# **The error output goes to a file and not to the terminal.** Writing there happens **inside the
# frame's pass**, and it costs whatever the reader on the other end of the pipe takes: a terminal,
# far more than a file. Mind the reason, which is weaker than it looks and had better not be
# overstated: of the warnings the application sends that way, **one declares its cost** — the one
# about the size the desktop does not respect, which is timed on purpose — and **the others do not**.
# So this does not «plug a leak»: it makes both classes cheaper. In practice they are **one line per
# run** — measured: four runs, one line each — so the difference is negligible; but **a batch taken
# after this line is not strictly comparable** with the earlier annexes, which were taken with the
# error output on a terminal.
#
# It does not invalidate the run: a warning can be legitimate — the first run has no file to read —
# and deciding which ones matter belongs to the milestone that introduces them. Here it is only said
# that they are there, so nobody discovers them the other way round, by suspecting a number.
if [ -s "$REPORT.err" ]; then
    # **A single line is dumped whole, and not counted.** Measured: the four trial runs wrote exactly
    # one, always the same. A warning that comes out in all of them warns of nothing, and swapping a
    # message that was read for a number is losing information in exchange for nothing.
    LINES=$(grep -c "" "$REPORT.err")
    if [ "$LINES" = 1 ]; then
        echo "the run said on its error output: $(cat "$REPORT.err")" >&2
    else
        # **Without naming the file**: if any guard below invalidates the run, the `.err` is renamed
        # along with the report, and the name that had been printed here would no longer exist.
        echo "the run wrote $LINES lines on its error output, in the .err file next to the report" >&2
    fi
else
    rm -f "$REPORT.err"
fi

invalidate() {
    mv "$REPORT" "$REPORT.INVALID"
    # The error output goes with the report: if it keeps the good name while the report becomes
    # invalid, the pair stops reading as a pair and the loose file looks like it belongs to another
    # run. It does not exist in the normal case: it is deleted a few lines above when it comes out
    # empty.
    if [ -f "$REPORT.err" ]; then
        mv "$REPORT.err" "$REPORT.INVALID.err"
    fi
    echo "INVALID BATCH: $1" >&2
    echo "Report marked as $REPORT.INVALID" >&2
    exit 1
}

# The application publishes the report on the way out, so if it finished badly the report is half
# done — and at a glance a truncated report looks quite like a good one.
#
# **It is the first guard, and that is not style.** Every guard reads a line of the report, and a
# report truncated by a panic is missing lines: a guard placed above this one invalidates the run
# for the line it could not read — true, and it hides the crash that took the line away.
if [ "$STATUS" != 0 ]; then
    invalidate "the application finished with status $STATUS; what it said is in the .err next to it"
fi

# **The font-discovery guard, and it reads both ways like the ones above.** Batch P is the only one
# that dumps in Japanese, and therefore the only one that walks the path of searching the machine
# for fonts. Without this number, a run in which the load never got out — or in which the machine
# had no CJK fonts — would publish the discovery section **empty** and it would read as «searching
# for fonts costs nothing», which is the same deceit the witness prevents with the panels.
#
# And the other way round: if a batch that is NOT P searches for fonts, something other than the
# load is triggering them, and its spans mix in work that does not belong to what it claims to
# measure.
SEARCHED=$(grep -oE "[0-9]+ characters searched" "$REPORT" | grep -oE "^[0-9]+" | head -1)
SEARCHED=${SEARCHED:-0}
if [ "${CJK:-}" = 1 ] && [ "$SEARCHED" = 0 ]; then
    invalidate "this batch dumps in Japanese to measure what searching the machine for fonts costs \
and it searched for none: either the load never got out, or the machine has nothing to offer, and in \
both cases the discovery section would be published empty and would read as costing nothing"
fi
if [ "${CJK:-}" != 1 ] && [ "$SEARCHED" != 0 ]; then
    invalidate "this batch does not dump in Japanese and yet it searched the machine for $SEARCHED fonts: \
something other than the load is triggering the discovery, so its spans mix in work that does not belong \
to what it claims to measure"
fi


# **The colour-glyph guard, and it reads both ways like all the ones above.** Batch Q is the only
# one whose consoles dump emoji, and Z the only one whose panels carry them: those two, and no other,
# walk the rasterising path. Without this number, a run in which the load never got out — or in which
# the machine had no colour emoji font — would publish the colour section **empty** and it would
# read as «painting a colour emoji costs nothing», which is exactly the deceit the witness prevents
# with the panels.
#
# And the other way round: if any other batch rasterises colour glyphs, something other than the
# load is triggering it, and its spans mix in work that does not belong to what it claims to
# measure. The usual load is pure ASCII: there is not one emoji in it.

RASTERIZED=$(grep -oE "[0-9]+ glyphs rasterized" "$REPORT" | grep -oE "^[0-9]+" | head -1)
RASTERIZED=${RASTERIZED:-0}
COLOR_GLYPHS=
if [ "${EMOJI:-}" = 1 ] || [ "${PANEL_EMOJI:-}" = 1 ]; then
    COLOR_GLYPHS=1
fi
if [ "$COLOR_GLYPHS" = 1 ] && [ "$RASTERIZED" = 0 ]; then
    invalidate "this batch paints emoji to measure what a colour glyph costs and it rasterised none: \
either the load never got out, or the machine has no colour emoji font, and in both cases the colour \
section would be published empty and would read as costing nothing"
fi
if [ "$COLOR_GLYPHS" != 1 ] && [ "$RASTERIZED" != 0 ]; then
    invalidate "this batch paints no emoji and yet it rasterised $RASTERIZED colour glyphs: \
something other than the load is triggering it, so its spans mix in work that does not belong to what \
it claims to measure"
fi

# **The panel's buttons, read in both directions like the emoji above and for the same reason.**
# Batch R is the only one that declares any, so it is the only one that lays out a button's rows,
# walks them to work out what a press may reach, and composes what one sends.
#
# **Two numbers and not one.** Painted with none pressed says the injector never hit a rectangle — a
# narrow window, a seeding that did not arrive, a derivation that produced nothing — and the batch
# would publish «pressing a button costs nothing», which is the deceit the witness already prevents
# with the panels. Pressed with none painted cannot happen and says the walk stopped matching.
#
# And the other way round: a batch that declares no buttons and paints one is being seeded by
# something other than its own load, so its spans carry work that does not belong to what it claims
# to measure.
# **The line is taken once and its two numbers are read off it**, rather than with two greps over the
# report. A pattern that reads the report has to carry an anchor, or the guard that ties every one of
# them to a test cannot see it — and two unanchored `[0-9]+` over the whole file would in any case be
# free to pick up somebody else's number.
BUTTONS_LINE=$(grep -E "^buttons:" "$REPORT" | head -1)
DRAWN=$(printf '%s' "$BUTTONS_LINE" | sed -n 's/^.*[^0-9]\([0-9][0-9]*\) drawn.*$/\1/p')
PRESSED=$(printf '%s' "$BUTTONS_LINE" | sed -n 's/^.*[^0-9]\([0-9][0-9]*\) pressed.*$/\1/p')
# **An empty capture invalidates instead of becoming a zero**, for the reason the links below give:
# a reworded line would otherwise read as «no buttons» and silence both directions at once.
if [ -z "$DRAWN" ] || [ -z "$PRESSED" ]; then
    invalidate "the report's buttons line could not be read: «$BUTTONS_LINE»"
fi
if [ "${ACTIONS:-}" = 1 ] && [ "$DRAWN" = 0 ]; then
    invalidate "this batch declares buttons to measure what painting and pressing one costs and it \
drew none: the seeding never reached the panel, or the strip was too short for a whole button, and \
either way the buttons section would be published empty and would read as costing nothing"
fi
if [ "${ACTIONS:-}" = 1 ] && [ "$PRESSED" = 0 ]; then
    invalidate "this batch drew $DRAWN button(s) and pressed none: the injector never landed on one, \
so what is published under «pressing a button» is the cost of a press that reached nothing"
fi
if [ "${ACTIONS:-}" != 1 ] && [ "$DRAWN" != 0 ]; then
    invalidate "this batch declares no buttons and yet it drew $DRAWN: something other than its own \
load is seeding them, so its spans mix in work that does not belong to what it claims to measure"
fi

# **The links, read in both directions like the buttons.** Batch W must find and activate every
# source — a run where one source never reached the grid would publish two thirds of the work under
# the name of all of it — and every other batch must activate none. Sources are only looked for
# while W asks for them, so the other direction reads the activations alone. That every gesture
# opened something is checked with the drags below, where the injected count is read.
#
# **An empty capture invalidates instead of becoming a zero**: a reworded report line would
# otherwise read as «no activations» and silence both directions at once.
LINKS_LINE=$(grep -E "^links:" "$REPORT" | head -1)
LINK_FOUND=$(printf '%s' "$LINKS_LINE" | sed -n 's/^links: *\([0-9]*\) sources found .*$/\1/p')
LINK_PLAIN=$(printf '%s' "$LINKS_LINE" | sed -n 's/^links:.* · \([0-9]*\) plain .*$/\1/p')
LINK_OSC=$(printf '%s' "$LINKS_LINE" | sed -n 's/^links:.* · \([0-9]*\) OSC 8 .*$/\1/p')
LINK_PANEL=$(printf '%s' "$LINKS_LINE" | sed -n 's/^links:.* · \([0-9]*\) panel activations$/\1/p')
if [ -z "$LINK_FOUND" ] || [ -z "$LINK_PLAIN" ] || [ -z "$LINK_OSC" ] || [ -z "$LINK_PANEL" ]; then
    invalidate "the report's links line could not be read: «$LINKS_LINE»"
fi
if [ "${LINKS:-}" = 1 ]; then
    if [ "$LINK_FOUND" = 0 ] || [ "$LINK_PLAIN" = 0 ] || [ "$LINK_OSC" = 0 ] || [ "$LINK_PANEL" = 0 ]; then
        invalidate "batch W did not find and activate all three link sources: $LINKS_LINE"
    fi
fi
if [ "${LINKS:-}" != 1 ] && { [ "$LINK_PLAIN" != 0 ] || [ "$LINK_OSC" != 0 ] || [ "$LINK_PANEL" != 0 ]; }; then
    invalidate "this batch asks for no link activations and yet reports some: $LINKS_LINE"
fi

# **The control channel, read in both directions, exactly like the buttons above.** A batch that
# declares a publisher and applied nothing never reached a pane — nothing bound, or the client could
# not connect — and its control series would be published as «the channel costs nothing». And a batch
# that declares none and applied something is being published to by somebody other than its own load,
# so its spans carry work it does not claim.
#
# **Only the first number is read in both directions**, and the other two are published rather than
# checked: a refusal needs a second source and a release needs a connection to end, and neither is
# something the ordinary batch can promise. A guard that demanded them would fire on a correct run.
CONTROL_LINE=$(grep -E "^control:" "$REPORT" | head -1)
APPLIED=$(printf '%s' "$CONTROL_LINE" | sed -n 's/^.*[^0-9]\([0-9][0-9]*\) applied.*$/\1/p')
if [ -z "$APPLIED" ]; then
    invalidate "the report's control line could not be read: «$CONTROL_LINE»"
fi
if [ "${CONTROL:-}" = 1 ] && [ "$APPLIED" = 0 ]; then
    invalidate "this batch publishes over the control channel and the window applied nothing: \
nothing bound its endpoint, or the publisher never reached it, and what is published under «the \
control channel» is the cost of a drain that never had anything in it"
fi
if [ "${CONTROL:-}" != 1 ] && [ "$APPLIED" != 0 ]; then
    invalidate "this batch declares no publisher over the control channel and yet the window applied \
$APPLIED document(s): something other than its own load is publishing, so its spans mix in work that \
does not belong to what it claims to measure"
fi

# **The document read back, in both directions, and it is the execution guard `V` exists for.** The
# document is serialised and answered on the connection thread, outside every frame span, so the
# difference between `V` and `S` is NOT read latency and must not be published as such. The count and
# largest size prove that the off-frame path really ran under the ordinary dumping load. Any other
# batch answering a read has workload its own load never asked for, so the guard remains symmetric.
#
# **The numbers come out with `sed` and the block sits below the status check**, and neither is style.
# Under `set -e` an assignment whose pipeline **ends** in a `grep` with no match inherits its status 1
# and **kills the script right there** — the report is never renamed, nothing is printed, and every
# guard below, the status check included, never runs. That is why the line itself is safe with a
# `grep … | head -1`, whose status is `head`'s, and why the first version of this block — which ended
# in `grep -oE` — was fatal on any report missing the line. The trap is documented twice further down,
# in the two guards it had already bitten, and this one walked into it anyway. `sed -n …p` exits 0
# with no output, so the empty case reaches the `if` instead of ending the run.
DOCUMENTS_LINE=$(grep -E "^documents:" "$REPORT" | head -1)
if [ -z "$DOCUMENTS_LINE" ]; then
    invalidate "the report does not declare what the control channel wrote out. Without that datum a \
run that exercised the off-frame read path and one that answered none are indistinguishable, and \
batch V has neither its execution proof nor the size of the document it serialised"
else
    READ_BACK=$(printf '%s' "$DOCUMENTS_LINE" | sed -n 's/^documents: *\([0-9]*\) read back.*/\1/p')
    READ_REFUSED=$(printf '%s' "$DOCUMENTS_LINE" | sed -n 's/.*, \([0-9]*\) refused.*/\1/p')
    LARGEST_SERIALIZED=$(printf '%s' "$DOCUMENTS_LINE" | sed -n 's/.*, \([0-9]*\) bytes the largest written out$/\1/p')
    READ_BACK=${READ_BACK:-0}
    READ_REFUSED=${READ_REFUSED:-0}
    LARGEST_CAPTURED=$LARGEST_SERIALIZED
    LARGEST_SERIALIZED=${LARGEST_SERIALIZED:-0}
    if [ "${READS:-}" = 1 ] && [ "$READ_BACK" = 0 ]; then
        invalidate "this batch reads the document back over the control channel and the window \
answered none of the reads ($READ_REFUSED refused): the publisher never reached the pane, or nothing \
was ever published on it, so the off-frame path this batch exists to exercise never ran"
    fi
    if [ "${READS:-}" = 1 ] && [ "$LARGEST_SERIALIZED" = 0 ]; then
        invalidate "this batch read a document back but declared no serialized size: its count and \
size disagree, so batch V cannot prove which off-frame work ran"
    fi
    if [ "${READS:-}" != 1 ] && [ "$READ_BACK" != 0 ]; then
        invalidate "this batch asks for no read of the document and yet the window answered \
$READ_BACK: something other than its own load is asking, so its report contains off-frame work that \
does not belong to this batch"
    fi

    # **The patching pair, read in both directions like every other, and then a third guard nothing
    # else here has.** A patch is the one verb of this channel a batch can promise will be accepted:
    # its publisher holds the pane it patches and every op names a key it published itself. So a
    # refusal is not a state of the world to be reported — it is this batch measuring the parse and
    # the refusal instead of the walk, the index and the install, and publishing the difference as
    # what patching costs. The panel it leaves behind is the published one either way, so nothing on
    # screen gives it away.
    PATCHED=$(printf '%s' "$DOCUMENTS_LINE" | sed -n 's/.*, \([0-9]*\) patched,.*/\1/p')
    PATCHES_REFUSED=$(printf '%s' "$DOCUMENTS_LINE" | sed -n 's/.*, \([0-9]*\) patches refused,.*/\1/p')
    # **An extraction that came back empty is invalidated, not defaulted to zero**, and that is the
    # difference between a guard and a decoration. The line is there — the block above proved it —
    # so an empty capture means the wording moved and this `sed` no longer matches it. Defaulting to
    # zero would then read as «no patches», which silences the third guard below on the batch that
    # patches and the reverse guard on every batch that does not: the whole pair would go quiet on a
    # rewording, and the reports would all agree with each other.
    if [ -z "$PATCHED" ] || [ -z "$PATCHES_REFUSED" ]; then
        invalidate "the «documents:» line does not carry the patch counts where this script reads \
them: «$DOCUMENTS_LINE». The wording moved and every guard below reads zero, so a run that patched \
nothing and one that patched everything would both pass"
        PATCHED=0
        PATCHES_REFUSED=0
    fi
    if [ "${PATCHES:-}" = 1 ] && [ "$PATCHED" = 0 ]; then
        invalidate "this batch changes the document over the control channel and the window applied \
none of the patches ($PATCHES_REFUSED refused): the publisher never reached the pane, or it never \
took the hold a patch needs, and what is published under «the control channel» is the cost of a \
drain that never had a patch in it"
    fi
    if [ "${PATCHES:-}" != 1 ] && [ "$PATCHED" != 0 ]; then
        invalidate "this batch changes no document and yet the window applied $PATCHED patches: \
something other than its own load is patching, so its spans mix in work that does not belong to \
what it claims to measure"
    fi
    if [ "${PATCHES:-}" = 1 ] && [ "$PATCHES_REFUSED" != 0 ]; then
        invalidate "this batch had $PATCHES_REFUSED of its patches refused: a refusal skips the \
write-out and the install, so what the run would publish as the cost of patching is the cost of \
stopping half way through one"
    fi

    # **The large document, read in both directions on the size the pane itself wrote out.** Only a
    # read or a patch feeds that size, so the batch that proves it is Y — the patch that applied was
    # at that size, as the pane serialized it — and the batches that can betray a leak are U and V.
    # S, X and the file batches feed nothing, which is why the batch table is held by the suite
    # instead. **An empty capture is invalidated here rather than read as the zero it defaults to**:
    # zero would pass the mirror and fail Y, so a rewording would look like Y being small.
    if [ "${LARGE:-}" = 1 ] || [ "${PATCHES:-}" = 1 ] || [ "${READS:-}" = 1 ]; then
        if [ -z "$LARGEST_CAPTURED" ]; then
            invalidate "the «documents:» line does not carry the written-out size where this script \
reads it: «$DOCUMENTS_LINE». Without it the large batch cannot prove its size and a leak into U or V \
cannot be seen"
        fi
    fi
    if [ "${LARGE:-}" = 1 ] && [ "${PATCHES:-}" = 1 ] && [ "$LARGEST_SERIALIZED" -lt "$LARGE_FLOOR" ]; then
        invalidate "this batch patches a document near the ceiling and the largest it wrote out was \
$LARGEST_SERIALIZED bytes, under the floor of $LARGE_FLOOR: what it would publish as the cost of a \
patch at the ceiling is the cost of one on a smaller panel"
    fi
    if [ "${LARGE:-}" != 1 ] && [ "$LARGEST_SERIALIZED" -ge "$LARGE_FLOOR" ]; then
        invalidate "this batch publishes no large document and yet wrote one out of \
$LARGEST_SERIALIZED bytes: the large load leaked into it, so its spans are not the ones it claims"
    fi
fi


# **The strips, read in both directions, and this is the only guard that protects the OTHER runs.**
# Every pair above catches a batch measuring something its own load did not ask for; this one also
# catches a batch measuring the right thing with the wrong window. A strip takes a row off **every**
# console, so a run it leaked into publishes a grid one row per pane shorter than every batch it is
# compared against — and nothing else gives that away: the report is complete, the guards are quiet
# and the numbers look a little better.
#
# **The count is what the painting produced and `$HEADER` is what the batch asked for**, and the
# report keeps them apart on purpose: deriving either from the other would make this pair unable to
# report the one case it exists for.
#
# **It goes BELOW the status check, like the settings guard above and for its reason**: a report
# truncated by a panic has no `header:` line either, and diagnosing that as «it asked for a strip and
# painted none» would cover up the real failure.
#
# **And the number is pulled with `sed` and not with a `grep` in an assignment**, which is the trap
# this script already documents one guard up: under `set -e` an assignment whose last command is a
# `grep` with no match inherits its status 1 and **kills the script right there** — the report is not
# renamed, nothing is printed, and every guard below never runs. The first version of this block was
# written that way and its own missing-line case was the one that triggered it. `sed -n …p` exits 0
# with no output, so the empty case reaches the `if` instead of ending the run.
HEADER_LINE=$(grep -E "^header:" "$REPORT" | head -1)
if [ -z "$HEADER_LINE" ]; then
    invalidate "the report does not declare whether a strip was painted over the consoles. Without \
that datum a run with the strip on and one with it off are indistinguishable, and they do not share \
a grid"
else
    WITH_STRIP=$(printf '%s' "$HEADER_LINE" | sed -n 's/^header: *\([0-9]*\) frame.*/\1/p')
    WITH_STRIP=${WITH_STRIP:-0}
    if [ "${HEADER:-}" = 1 ] && [ "$WITH_STRIP" = 0 ]; then
        invalidate "this batch asks for a strip over every console to measure what painting it costs \
and not one frame painted one: the gate did not let it through. A window too short to hold the band is \
not the explanation — the cut is clamped and a degenerate strip still counts its frame"
    fi
    if [ "${HEADER:-}" != 1 ] && [ "$WITH_STRIP" != 0 ]; then
        invalidate "this batch asks for no strip and yet $WITH_STRIP frame(s) painted one: every \
console of this run is a row per pane shorter than the batches it is compared against, so its grid \
describes a window none of them was measured with"
    fi
fi

# **The settings guard, and this one invalidates on two counts.** Ever since the terminal reads a
# settings file, a run that read the measurer's own would have **another grid** — the font size
# decides it — and its spans do not compare with those of any published annex. The application has
# reading vetoed under these variables, so the report **must always** say «factory»:
#
#   - if it does not say so **in any form**, the report comes from a binary that does not declare
#     this and the run cannot be judged — the same deceit the witness prevents with the panels;
#   - and if it says anything else — «the user's», or «factory» because it looked and found nothing
#     — **the veto has been broken** and that run has to be thrown away. The three states are told
#     apart by their first noun on purpose: the script empties the settings directory, so a broken
#     veto on the bench produces «it looked and there was nothing», and if that started the same way
#     as the vetoed one it would pass for good.
#
# **The `grep` goes INSIDE the `if` and not in an assignment, and that difference is the only thing
# that makes this guard exist.** With `set -e`, an assignment taking its value from a `grep -c` with
# no matches inherits its status 1 and **kills the script right there**: the report is not renamed,
# nothing is printed, and the script exits in silence. The first version of this guard was written
# that way and **was dead in both cases it exists to catch** — checked by running the script, not
# the bare `grep`, which is how it was verified wrongly the first time. And being above the status
# check, it took every other guard down with it.
#
# **And it goes BELOW the status check on purpose**: a report truncated by a panic has no settings
# line either, and diagnosing that as «it does not declare the settings» would cover up the real
# failure.
if ! grep -q "^settings: *vetoed" "$REPORT"; then
    if grep -q "^settings:" "$REPORT"; then
        invalidate "this run LOOKED AT the user's settings instead of having the file vetoed: under \
the measurement gate it may not even look, because the font size decides the grid and every published \
figure depends on it. The veto has been broken"
    else
        invalidate "the report does not declare whether the user's settings were read. Without that \
datum, a run that read them and one that ran with the factory ones publish indistinguishable reports \
with grids that may have nothing to do with each other"
    fi
fi

# **The blink guard, and it is one-directional by construction rather than by preference.** Unlike
# the ones for emoji, fonts and mouse reports, there is no batch in which this should fire: the
# cursor blinks by default, so the measurement gate switches it off for every measured run — the same
# gate that vetoes the settings file — and a repaint on the blink's account is therefore always a
# broken gate. Written all the same, because D13's rule is about the paths that COULD go undeclared:
# a repaint nobody names is published as the interface framework's cost, which is the figure D3
# judges egui by, and a zero somebody can read is what makes the claim checkable.
#
# The `grep` goes inside the `if` for the `set -e` reason spelled out just above.
if ! grep -q "^blink: *0 frame" "$REPORT"; then
    if grep -q "^blink:" "$REPORT"; then
        WOKEN=$(sed -n 's/^blink: *\([0-9]*\) frame.*/\1/p' "$REPORT")
        invalidate "a blinking cursor was painted in $WOKEN frame(s) of this run, so it has work nothing \
in the load asked for. The blink is switched off for every measured run by the same gate that vetoes \
the settings file, so that gate has been broken"
    else
        invalidate "the report does not declare what a blinking cursor cost. Without that datum a run \
whose window repainted 1.7 times a second on its own publishes the same report as one that came to \
rest, and the difference lands in the framework's residue"
    fi
fi

# **The chosen-font guard, and it is the twin of the one above for a path the one above cannot see.**
# Asking for a family by name is the ONLY thing that makes the terminal enumerate the machine's fonts
# at start-up — between 17.3 and 23.2 ms — and it does **not** go through the discovery counters, so
# the guard that invalidates a batch which searched for fonts without dumping Japanese would not
# notice it. Reading the settings is vetoed under these variables, so no batch can ask for a family
# and this line must **always** say «none»:
#
#   - anything else means the veto broke and that run mixes an enumeration into a start-up it
#     declares as clean;
#   - and no line at all means the binary does not declare this path, so the run cannot be judged.
#
# The `grep` goes inside the `if` for the same `set -e` reason spelled out just above.
if ! grep -q "^chosen font: *none" "$REPORT"; then
    if grep -q "^chosen font:" "$REPORT"; then
        invalidate "this run asked the machine for a font family by name, which enumerates every font \
installed —between 17.3 and 23.2 ms— inside a start-up this report declares as clean. Reading the \
settings is vetoed under the measurement gate, so the veto has been broken"
    else
        invalidate "the report does not declare whether a font family was asked for by name. Without \
that datum, a run that enumerated the machine's fonts at start-up and one that did not publish \
indistinguishable reports"
    fi
fi

if [ "$BATCH" != A ] && [ ! -f "$WITNESS" ]; then
    invalidate "nobody got to publish a panel, so this is a batch A under another name"
fi

# **That the window opened the tabs it was asked for.** If the variable did not arrive — a change in
# the name, an old binary — the application opens a single one and the report comes out shaped like
# a one-tab batch under a two-tab label: exactly the same deceit the witness prevents with the
# panels.
DECLARED=$(sed -n 's/^tabs: *\([0-9]*\)$/\1/p' "$REPORT" | head -1)
if [ "$DECLARED" != "$TABS" ]; then
    invalidate "the report says «$DECLARED» tabs and $TABS were asked for"
fi

# **That the drags asked for actually happened.** It is the same deceit the witness prevents with
# the panels, and here it bites harder: with no gesture during the batch the report declares its
# cost as **zero** and that batch reads as «the gesture costs nothing» instead of as «there was no
# gesture». It happens if the variable does not arrive — a name change, an old binary — if the
# layout has a single console, or if the window closes before completing the first one.
if [ "$DRAGS" != 0 ]; then
    INJECTED=$(sed -n 's/^drags: *\([0-9]*\) injected · .*$/\1/p' "$REPORT" | head -1)
    WITH_EFFECT=$(sed -n 's/^drags: *[0-9]* injected · \([0-9]*\) effective · .*$/\1/p' "$REPORT" | head -1)
    MARKED=$(sed -n 's/^drags: *[0-9]* injected · [0-9]* effective · \([0-9]*\) frames with marked text .*$/\1/p' "$REPORT" | head -1)
    REPORTED=$(sed -n 's/^drags: .* marked text · \([0-9]*\) reports to the program .*$/\1/p' "$REPORT" | head -1)
    WHICH_GESTURE=$(sed -n 's/^drags: .* reports to the program (\(.*\))$/\1/p' "$REPORT" | head -1)
    if [ -z "$INJECTED" ]; then
        invalidate "drags were asked for and the report declares no drag line at all"
    fi
    if [ "$INJECTED" = 0 ]; then
        invalidate "drags were asked for and the report says not one whole drag was injected"
    fi
    # **And that at least one HAD AN EFFECT, which is what the batch really measures.** Injecting the
    # gesture does not mean anything changes: the geometric floor rejects dropping a console when the
    # destination cannot give both of them their minimum, and the panel's ceiling leaves the width
    # untouched if it was already at its limit. With a narrow window that happens to **all of them**:
    # the run injects a thousand gestures, changes nothing, and without this check it would be
    # published as «the gesture costs nothing».
    # **The search's effect is not a layout change**, so its batch is held by its own line above —
    # presses that found something and paintings that marked it — and not by this count.
    if [ "$WITH_EFFECT" = 0 ] && [ "$GESTURE" != search ]; then
        invalidate "$INJECTED gestures ($WHICH_GESTURE) were injected and none of them changed \
anything: the window is too narrow for the operation to be accepted, so this batch does not measure \
the gesture"
    fi
    # **And the marked-text guard, which reads BOTH ways.** Since **D20** the primary button on its
    # own inside a console marks text, so any gesture that loses its handle — the panel's, the
    # divider's, the one that grabs the console — turns into a selection **with nothing else giving
    # it away**: it is injected whole, it declares effects, and it publishes the cost of marking
    # under another thing's label. That is why the batch that measures marking demands there be
    # some, and the others demand there be none.
    if [ -z "$MARKED" ]; then
        invalidate "drags were asked for and the report does not declare how many left text marked"
    fi
    # **And MORE THAN ONE PER GESTURE is demanded, not simply more than zero.** Every gesture extends
    # the selection once per drag frame, so a healthy gesture leaves of the order of two: asking for
    # «at least one in the whole run» would let through a batch in which only the first one marked
    # and the rest fell down another path. It is the same arithmetic as the reflow guard of the
    # divider batch.
    if [ "$GESTURE" = mark ] && [ "$MARKED" -le "$INJECTED" ]; then
        invalidate "$INJECTED marking gestures were injected and only $MARKED frames left text \
marked: that is not even one per gesture, so most of them came in somewhere else and the run publishes \
the cost of something else under its label"
    fi
    # **Batch W demands an opened link for every gesture it completed**, not one per source: a
    # gesture counts once its release is sent whether or not its press landed on a link, so a run
    # whose points went stale would publish presses that fell into selection or focus under the
    # name of opening links.
    if [ "$GESTURE" = link ] && [ "$((LINK_PLAIN + LINK_OSC + LINK_PANEL))" -lt "$INJECTED" ]; then
        invalidate "$INJECTED link gestures were completed and only $((LINK_PLAIN + LINK_OSC + LINK_PANEL)) \
opened a link: the rest landed somewhere else, so the run measures something other than opening links"
    fi
    if [ "$GESTURE" != mark ] && [ "$MARKED" != 0 ]; then
        invalidate "this batch measures «$WHICH_GESTURE» and yet it left text marked in $MARKED frames: \
its gesture is falling down the selection path, so it measures something else"
    fi
    # **And the guard on the reports to the program, which reads the same two ways.** Batch O does
    # THE SAME walk as N and they differ only in whether the left console's program asked for the
    # mouse: without this number, a run in which that program never got to ask — the sequence that
    # does not come out, an old binary, the load starting first — would publish the spans of a
    # **selection** under the mouse's label, and the two batches would be indistinguishable from
    # reading the report. And the other way round: a report escaping in any other batch would slip a
    # trip to the pseudoterminal into its mean with nothing giving it away.
    if [ -z "$REPORTED" ]; then
        invalidate "drags were asked for and the report does not declare how many mouse reports came out"
    fi
    if [ "$GESTURE" = mouse ] && [ "$REPORTED" -le "$INJECTED" ]; then
        invalidate "$INJECTED mouse gestures were injected and only $REPORTED reports came out: that \
is not even one per gesture, so the program never got to ask for the mouse and the run publishes the \
cost of a selection under the mouse's label"
    fi
    if [ "$GESTURE" = mouse ] && [ "$MARKED" != 0 ]; then
        invalidate "this batch sends the mouse to the program and on top of that left text marked in \
$MARKED frames: both paths are being walked at once, so its spans mix the two things"
    fi
    if [ "$GESTURE" != mouse ] && [ "$REPORTED" != 0 ]; then
        invalidate "this batch measures «$WHICH_GESTURE» and yet it sent $REPORTED mouse reports to \
the program: its gesture is falling down the mouse path, so it measures something else"
    fi
    echo "gestures during the batch: $INJECTED injected, $WITH_EFFECT effective, $MARKED frames with \
marked text, $REPORTED reports to the program ($WHICH_GESTURE)" >&2
fi

# **And that the text was reflowed, in the batch that exists to measure that.** Moving the tree's
# divider changes the width of **two** consoles, and reflowing each one's text is the span the
# scrollback made more expensive, from 7 µs to almost a millisecond. A run where the divider moved
# without ever crossing a cell boundary — too short a walk, a coarser grid — would publish the span
# as zero and it would read as «reflowing costs nothing» instead of as «nothing was reflowed»: the
# same disguise the witness prevents with the panels and the effect count with the gestures.
#
# **Only where the gesture changes a console's width.** The one that moves consoles reorders the
# tree without touching the share-out, and demanding the line of it would invalidate correct runs.
if [ "$GESTURE" = divider ]; then
    REFLOWS=$(sed -n 's/^reflow: *\([0-9]*\) times .*$/\1/p' "$REPORT" | head -1)
    if [ -z "$REFLOWS" ]; then
        invalidate "this batch moves the tree's divider and the report declares no reflow at all: \
the work the scrollback made more expensive would be published as framework cost"
    fi
    # **And that there be MORE of them than gestures, not simply more than zero.** The line existing
    # is not enough: startup already reflows two or three times on its own — the compositor hands
    # over the window size when the consoles are already alive — so a run whose divider never crossed
    # a single cell boundary would publish «reflow: 2 times» and **would pass**, which is exactly the
    # case this guard claims to catch. Every whole gesture moves the divider several cells, so it has
    # to leave at least one reflow of its own: the two published runs give 320 for 159 gestures and
    # 1,397 for 697.
    if [ "$REFLOWS" -le "$INJECTED" ]; then
        invalidate "$INJECTED gestures were injected and there were only $REFLOWS reflows: that is \
not even one per gesture, so the divider moved without crossing cell boundaries and this batch does \
not measure the worst case it promises"
    fi
    echo "reflows during the batch: $REFLOWS for $INJECTED gestures" >&2
fi

# **That saving the layout was at least ATTEMPTED, in the batches that exercise it.** It is the same
# deceit the witness prevents with the panels and with the gestures: if nobody tried to save, the
# report declares that path's cost as **zero** and the batch reads as «saving costs nothing» instead
# of as «nothing was saved». It happens if the deadline does not expire before the window closes, if
# the layout returns to where it started — the gesture's cycle does that with two consoles — or if
# the tree never gets moved.
#
# **The ATTEMPTS are counted and not the writes, and that is not lowering the bar: it is that a batch
# never writes.** The application vetoes saving as soon as it sees a measurement-gate variable — so
# as not to clobber the layout of whoever is measuring — so a run's write count is **zero by
# construction** and demanding it invalidated every batch with gestures. The work that has to be
# declared does happen and is timed: copying the layout, validating it and reaching the gate. What
# the guard watches is that that path gets walked. **Once per run, not once per deadline**: a refusal
# the session cannot lift ends the attempts after the first (D16, item 4), so a good batch declares
# exactly one attempt, and the guard invalidates both the zero and anything above one.
#
# **It is only demanded where the batch moves the layout.** In a still batch there is nothing to
# save, and neither in the one that marks text, the one that sends the mouse to the program, the one
# that presses a button on the panel, **nor the one that opens links, nor the search**: none of them touches the tree
# or the share-out, so
# their runs do not attempt to save
# even once and demanding it would invalidate them while being right for the wrong reason. That the
# list of exempt gestures grows is not an oversight: it is that «injects gestures» and «moves the
# layout» stopped being the same thing the moment there was a gesture that does not move it, and
# since then every new gesture that does not move it comes in here.
if [ "$DRAGS" != 0 ] && [ "$GESTURE" != mark ] && [ "$GESTURE" != mouse ] && [ "$GESTURE" != button ] \
    && [ "$GESTURE" != link ] && [ "$GESTURE" != search ]; then
    ATTEMPTS=$(sed -n 's/^writing the layout to disk *([0-9]* writes of \([0-9]*\) attempts)$/\1/p' "$REPORT" | head -1)
    if [ -z "$ATTEMPTS" ]; then
        invalidate "drags were asked for and the report declares no attempt at saving the layout"
    fi
    if [ "$ATTEMPTS" = 0 ]; then
        invalidate "the layout was moved and saving it was not attempted even once: that path's cost \
would be published as zero, which reads as «saving costs nothing»"
    fi
    if [ -n "$ATTEMPTS" ] && [ "$ATTEMPTS" -gt 1 ]; then
        invalidate "saving was attempted $ATTEMPTS times in a session that cannot write: every attempt \
after the first pays for a write that cannot happen (D16, item 4), and the run measures that"
    fi
    echo "attempts at saving the layout during the batch: $ATTEMPTS (none of them writes: a batch has \
saving vetoed)" >&2
fi

# **The scrollback's search, read in both directions (D61).** Batch E must have pressed, found and
# marked: a run whose query never matched would publish a walk that found nothing under the name of
# searching, and one that never marked would publish the presses alone. Every other batch must have
# done none of it — a search nobody asked for is work in its spans it does not claim. **And E must
# have sent the program nothing**: the search keeps its keys, so a keystroke in the report means the
# query or a press leaked to a shell, and the run measured typing as well.
#
# **An empty capture invalidates instead of becoming a zero**, for the links' reason above.
SEARCH_LINE=$(grep -E "^search:" "$REPORT" | head -1)
SEARCH_PRESSES=$(printf '%s' "$SEARCH_LINE" | sed -n 's/^search: *\([0-9]*\) presses, .*$/\1/p')
SEARCH_FOUND=$(printf '%s' "$SEARCH_LINE" | sed -n 's/^search: *[0-9]* presses, \([0-9]*\) found .*$/\1/p')
SEARCH_OPEN=$(printf '%s' "$SEARCH_LINE" | sed -n 's/^search:.* · \([0-9]*\) paintings with a search open, .*$/\1/p')
SEARCH_MARKED=$(printf '%s' "$SEARCH_LINE" | sed -n 's/^search:.* paintings with a search open, \([0-9]*\) with matches marked .*$/\1/p')
if [ -z "$SEARCH_PRESSES" ] || [ -z "$SEARCH_FOUND" ] || [ -z "$SEARCH_OPEN" ] || [ -z "$SEARCH_MARKED" ]; then
    invalidate "the report's search line could not be read: «$SEARCH_LINE»"
fi
if [ "${SEARCH:-}" = 1 ]; then
    if [ "$SEARCH_PRESSES" = 0 ] || [ "$SEARCH_FOUND" = 0 ] || [ "$SEARCH_MARKED" = 0 ]; then
        invalidate "batch E did not press, find and mark: $SEARCH_LINE"
    fi
    if grep -q "^key in the frame → handed over to be written" "$REPORT"; then
        invalidate "batch E sent keystrokes to a program: the search let its query or a press through, \
so the run measured typing into a shell as well as searching"
    fi
fi
# **The other direction reads the paintings with a search OPEN, not the ones that marked**: a search
# opened on a query that matches nothing on screen marks no cell and still paints its row inside the
# console's span — which is work the batch does not claim.
if [ "${SEARCH:-}" != 1 ] && { [ "$SEARCH_PRESSES" != 0 ] || [ "$SEARCH_OPEN" != 0 ]; }; then
    invalidate "this batch asks for no search and yet reports one: $SEARCH_LINE"
fi

# **That the layout came out of the FILE, in the batch that claims to measure that.** It is the guard
# that makes batch L distinguishable from H, and without it L measures nothing of what it promises:
# if the seeding fails — the directory cannot be written, the format changes and the document stops
# being understood, the settings variable does not arrive — the application opens with the factory
# layout, **one tab with two consoles**, and although the console guard below would catch that by
# pure arithmetic, the day a restoring batch asks for exactly two nobody would catch it any more.
# What is checked is what the report declares, not what was deduced from a count.
if [ "$RESTORES" = 1 ]; then
    RESTORED=$(sed -n 's/^restored: *\([0-9]*\) consoles .*$/\1/p' "$REPORT" | head -1)
    if [ -z "$RESTORED" ]; then
        invalidate "this batch seeds a layout and the report declares no restored console: the window \
opened with the factory layout, so this is a batch H under another name"
    fi
    if [ "$RESTORED" != "$DIRECTORIES" ]; then
        invalidate "the report says it restored $RESTORED consoles and $DIRECTORIES were seeded"
    fi
    echo "consoles restored from the file: $RESTORED" >&2
fi

# **That the bar was painted and declared it, in the batches that have one.** With two tabs or more
# there is a bar, and ever since its tab says what the panes of each tab are asking for, painting it
# **walks**. If that span is not declared, the subtraction takes it and it gets published as
# framework cost — the figure D3 judges egui with. Without this guard, a two-tab batch whose bar
# stopped declaring itself would pass for good and the annex would publish the bar's work under
# egui's label.
#
# **Only where there is a bar**: with a single tab it is not painted, and demanding the line would
# invalidate correct runs. It is the same rule as the drag guard.
if [ "$TABS" -gt 1 ]; then
    FRAMES_WITH_BAR=$(sed -n 's/^tab bar: *\([0-9]*\) frames .*$/\1/p' "$REPORT" | head -1)
    if [ -z "$FRAMES_WITH_BAR" ]; then
        invalidate "this batch has $TABS tabs and the report does not declare what the bar cost: its \
work would be published as framework cost"
    fi
    # **And that it got to mark some tab.** The walk that accumulates the attention happens **whether
    # there is attention or not**, so a run in which the seeded `attention` never reached the tab —
    # the field misspelt, the seeding out of time — would measure the bar **without the half this
    # milestone added to it** and would read exactly like a good one. It is the same deceit the
    # witness prevents with the panels and the effect count with the gestures.
    MARKED_TABS=$(sed -n 's/^tab bar: .* · \([0-9]*\) tabs marked at once at most .*$/\1/p' "$REPORT" | head -1)
    if [ "${MARKED_TABS:-0}" = 0 ]; then
        invalidate "the bar was painted $FRAMES_WITH_BAR times and did not mark a single tab: this \
batch measures the walk without what gets walked"
    fi
    echo "frames with the bar painted: $FRAMES_WITH_BAR · tabs marked at once: $MARKED_TABS" >&2
fi

# **That the consoles were the ones asked for.** With the layout variable not arriving, the
# application opens two columns and the report comes out shaped like a two-pane batch under a
# four-pane label. The pane lines are counted, which is what the report really publishes.
CONSOLES=$(grep -c '^  pane ' "$REPORT" || true)
if [ "$CONSOLES" != "$DIRECTORIES" ]; then
    invalidate "the report publishes $CONSOLES consoles and $DIRECTORIES were asked for ($TABS × $PANES)"
fi

# **That the window opened with the size it was asked for.** It is the check that was missing and the
# reason three runs of the same batch could give three grids: if the desktop imposes its own, the
# application says so on its error output — which, now that it goes to a file, is read in the `.err`
# next to it and no longer in the terminal — **and** publishes the one it had, so here it can be seen
# without guessing.
#
# **With a margin and not an exact comparison**, and the margin is `alike`'s, which is the
# application's: the compositor hands the size over with a fraction and not always the same one, so
# an exact comparison would throw away good runs.
HAD=$(size_of "$REPORT")
if ! alike "$WINDOW_SIZE" "$HAD"; then
    invalidate "a window of ${WINDOW_SIZE%x*}×${WINDOW_SIZE#*x} points was asked for and the report says it had «$HAD»"
fi

# **WHAT IS STILL NOT CHECKED, AND IS WORTH KNOWING: that the window did not change size HALF WAY.**
# The header publishes the **final** grid, so a window that changed and went back to its place gives
# a report identical to a clean run's, with the other grid's spans inside the series. It was
# attempted in F2.2 and **reverted with two measurements in hand**: (1) the compositor sends its
# configuration while the consoles **are already dumping**, so every clean run has a frame with
# another size and its samples inside; and (2) telling that apart from a real anomaly requires
# **counting** contaminated frames and setting a threshold, which is a number nobody can justify
# today because the scenario — something resizes the window half way — has never been observed. It
# is left as an issue of its own.

echo "done: $(basename "$REPORT")" >&2
# **The first pane, by its name and not by its position.** The report names them with a number that
# is not reused when one closes, so «pane 0» is always the first console the window opened — which
# is the one these batches compare across runs.
echo "grid: $(grep -m1 '  pane 0:' "$REPORT" | tr -s ' ')" >&2
