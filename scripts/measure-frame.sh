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
APP="$ROOT/target/release/app"
LOAD="$ROOT/target/release/host-load"

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
BATCH=${1:?the batch is missing: A, B, C, D, F, G, H, I, J, K, L, M, N, O, P, Q or R}
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
case "$BATCH" in
    A|B|C|D) TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console ;;
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
    Q)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=0; GESTURE=console; EMOJI=1 ;;     # two, and both dumping EMOJI: the only one that rasterises colour glyphs
    R)       TABS=1; PANES=2x1; DUMPING=2; DRAGS=100000; GESTURE=button; ACTIONS=1 ;;   # two, and pressing a button on a panel: its control is C
    *) echo "unknown batch «$BATCH»: use A, B, C, D, F, G, H, I, J, K, L, M, N, O, P, Q or R" >&2; exit 2 ;;
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


# **Both binaries, before anything else.** Without `host-load` the two consoles die on the spot and
# the window closes: out comes a report with a header and no spans, which at a glance looks like a
# valid run. And if the application cannot launch this script as a shell, it falls back to the
# system's and the two consoles sit at a prompt: the window never closes and publishes nothing.
for binary in "$APP" "$LOAD"; do
    [ -x "$binary" ] || { echo "$binary is missing — build with: cargo build --release -p app -p host-load" >&2; exit 1; }
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
    # **The attention goes into what is seeded now that the tab shows it.** Without it, the bar
    # would be painted with nobody asking for anything: it would walk the same — the walk does not
    # depend on there being attention — but it would paint not a single symbol, and the batch would
    # publish what the bar costs **without the half this milestone added**. It is shared out per
    # console so that different classes live together in one tab, which is what forces the tab to
    # compare: the one on an odd turn waits, the even one is taken as finished.
    if [ $(( $3 % 2 )) = 0 ]; then attention=done; else attention=wait; fi
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
    cat > "$tmp" <<JSON
{"v":1,"stale_after_minutes":600,"attention":"$attention",$actions"lines":[
  {"text":"Batch $BATCH · publication $2","style":"title"},
  "",
  {"ref":"load","text":"$MIB MiB per console, TUI-style dump","mark":"current"},
  {"ref":"panel","text":"regime $BATCH — this is what is being measured","mark":"info"},
  {"ref":"note","text":"the layout is redone on every good re-read","mark":"info"},
  "",
  {"text":"six lines and a warning: the size of a real working panel","mark":"done"}
]}
JSON
    mv "$tmp" "$1/panel.json"   # in one go, as the contract requires
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
    C|D|F|G|H|I|J|K|L|M|N|O|P|Q|R) seed talkative & ;;
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
    XDG_CONFIG_HOME="$CONFIG" \
    SHELL="$ROOT/scripts/measure-frame.sh"
# **The two that ask for a layout are not exported when the batch restores, and that is what batch L
# consists of.** The application opens with the factory layout **as soon as either of the two is
# there** — on purpose, so a batch does not depend on whatever had been saved — so seeding the file
# **and** asking for four panes would give the same report with the file not even read: four
# consoles, the same grid, and no guard firing. The report's `restored:` line is what tells the two
# things apart, and that is why it is checked below.
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
# one that dumps emoji, and therefore the only one that walks the rasterising path. Without this
# number, a run in which the load never got out — or in which the machine had no colour emoji font —
# would publish the colour section **empty** and it would read as «painting a colour emoji costs
# nothing», which is exactly the deceit the witness prevents with the panels.
#
# And the other way round: if a batch that is NOT Q rasterises colour glyphs, something other than
# the load is triggering it, and its spans mix in work that does not belong to what it claims to
# measure. The usual load is pure ASCII: there is not one emoji in it.
RASTERIZED=$(grep -oE "[0-9]+ glyphs rasterized" "$REPORT" | grep -oE "^[0-9]+" | head -1)
RASTERIZED=${RASTERIZED:-0}
if [ "${EMOJI:-}" = 1 ] && [ "$RASTERIZED" = 0 ]; then
    invalidate "this batch dumps emoji to measure what rasterising a colour glyph costs and it \
rasterised none: either the load never got out, or the machine has no colour emoji font, and in both \
cases the colour section would be published empty and would read as costing nothing"
fi
if [ "${EMOJI:-}" != 1 ] && [ "$RASTERIZED" != 0 ]; then
    invalidate "this batch does not dump emoji and yet it rasterised $RASTERIZED colour glyphs: \
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
DRAWN=$(printf '%s' "$BUTTONS_LINE" | grep -oE "[0-9]+ drawn" | grep -oE "^[0-9]+")
PRESSED=$(printf '%s' "$BUTTONS_LINE" | grep -oE "[0-9]+ pressed" | grep -oE "^[0-9]+")
DRAWN=${DRAWN:-0}
PRESSED=${PRESSED:-0}
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

# The application publishes the report on the way out, so if it finished badly the report is half
# done — and at a glance a truncated report looks quite like a good one.
if [ "$STATUS" != 0 ]; then
    invalidate "the application finished with status $STATUS; what it said is in the .err next to it"
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
    if [ "$WITH_EFFECT" = 0 ]; then
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
# the guard watches is that that path gets walked.
#
# **It is only demanded where the batch moves the layout.** In a still batch there is nothing to
# save, and neither in the one that marks text, the one that sends the mouse to the program, **nor
# the one that presses a button on the panel**: none of them touches the tree or the share-out, so
# their runs do not attempt to save
# even once and demanding it would invalidate them while being right for the wrong reason. That the
# list of exempt gestures grows is not an oversight: it is that «injects gestures» and «moves the
# layout» stopped being the same thing the moment there was a gesture that does not move it, and
# since then every new gesture that does not move it comes in here.
if [ "$DRAGS" != 0 ] && [ "$GESTURE" != mark ] && [ "$GESTURE" != mouse ] && [ "$GESTURE" != button ]; then
    ATTEMPTS=$(sed -n 's/^writing the layout to disk *([0-9]* writes of \([0-9]*\) attempts)$/\1/p' "$REPORT" | head -1)
    if [ -z "$ATTEMPTS" ]; then
        invalidate "drags were asked for and the report declares no attempt at saving the layout"
    fi
    if [ "$ATTEMPTS" = 0 ]; then
        invalidate "the layout was moved and saving it was not attempted even once: that path's cost \
would be published as zero, which reads as «saving costs nothing»"
    fi
    echo "attempts at saving the layout during the batch: $ATTEMPTS (none of them writes: a batch has \
saving vetoed)" >&2
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
