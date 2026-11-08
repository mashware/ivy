//! The other half of the inverted ratchet: **it fails for prose that is still in Spanish**.
//!
//! Its sibling [`glossary`](../glossary.rs) guards *identifiers* and says so in its own header —
//! «comments, rustdoc and string literals, for the same reason: prose has its own delivery». That
//! delivery has now landed for every Rust file under `src/` and `examples/`, and nothing was left
//! watching it.
//! AC3 of the milestone («comments and rustdoc in English») is marked `test` and had no test: a
//! later delivery could put a Spanish comment back into any of these files and the suite would
//! stay green, which is the exact shape of hole this milestone keeps paying for.
//!
//! # The oracle, and what it cannot do
//!
//! A line is called Spanish when it carries **three distinct words** from a closed list. The list
//! is the criterion the two sibling guards already use — short words, closed set, compared as
//! whole words — extended from function words to the grammar and the domain nouns that make up
//! Spanish prose. The threshold is three and not one because this repository **quotes Spanish on
//! purpose**: comment lines of these source and example files carry a Spanish literal inside an English
//! sentence, and at one marker every one of them would break the build.
//!
//! **What it does not see, with the sentences that prove it.** A Spanish sentence carrying no word
//! from the list walks straight through: «formato desconocido» and «ruta inalcanzable» pass, and
//! they are asserted below as passing so this paragraph cannot quietly stop being true. No closed
//! list catches them — one wide enough would stop being closed and would start colliding with
//! English. The same evasion is on record for the sibling oracle in `settings.rs`, which was
//! attacked deliberately and reported those two sentences.
//!
//! **The non-ASCII half of the sibling oracle is deliberately absent, and it was measured before
//! being dropped.** `settings.rs` judges thirteen short on-screen messages and can afford
//! `is_ascii()`; here **2 745 of the 18 141 comment lines are not pure ASCII** — angle quotes, em
//! dashes, `µs`, Japanese, emoji — so that test reports a material part of the corpus. The narrower
//! variant — a Spanish accent or `ñ` anywhere in the line — leaves **five** lines, and it would
//! need a line-by-line exemption for every one of them: two are deliberate Spanish citations
//! («pestañas» and «válido», both in `src/settings.rs`) and **the other three are English sentences
//! about the letter itself**, `src/lib.rs` explaining why a measurement variable is spelled without
//! an eñe. It was eight until this delivery translated the panel's three claim phrases. A line-level exemption is a hole: it would also excuse a fresh Spanish sentence written
//! on that same line. One oracle, with its limits stated, was chosen over two with an escape hatch.
//!
//! **It does not judge a unit, and no narrow guard could.** «a line of {} characters» beside a byte
//! length is English, so it walks through — and that exact sentence shipped past this file in the
//! delivery whose subject was a mislabelled constant. Whether «characters» is right depends on the
//! type of a number measured elsewhere, a dozen lines or a function away, which a line walk cannot
//! see. Measured over `src/` and `examples/` of every crate in September 2026, before deciding:
//! «character» within three lines of a byte length (`len()`, `as_bytes`, `u8`, `bytes()`) matched 28
//! lines, none of them wrong, and would have caught neither of the two that shipped — that sentence,
//! and a comment calling the kernel's 4 095 bytes «characters»; «a number followed by characters»
//! caught both and matched 6 lines, every one of them correct, so it would have been an allowlist
//! rather than a guard. That sentence is asserted below as passing. **Where a unit is bound is the
//! test that feeds it non-ASCII input**, the one input where bytes and characters differ — for the
//! held paste's question, the `ñ` held-paste test in `src/keyboard.rs`.
//!
//! **What a green here means, exactly**: no comment line in the declared source and example files carries three
//! different common Spanish words. It does **not** mean every comment is good English.
//!
//! **The prototypes are the one exception to both paragraphs above**: every line of the two files
//! that are not markdown is judged, and with the accent half — their Spanish is labels, not
//! sentences. [`prototype_spanish`] says why and what it still misses.

mod common;

use common::{fold, list_against_disk, markdown_below, rust_sources};
use std::collections::BTreeSet;
use std::path::Path;

/// The files whose prose is English, with their text pulled in **at compile time**.
///
/// `include_str!` and not a directory walk, and the sibling records why: reading paths at run time
/// turns a renamed or deleted file into a target that is silently skipped, and a guard with no file
/// to look at reads exactly like a clean one. Measured there — one hyphen turned into an underscore
/// left four tests green. Here the same mistake does not compile.
///
/// **`src/lib.rs` is here, and adding it was the one line an earlier delivery promised it would
/// be.** It is the largest source in the corpus — 6 216 of the 18 141 comment lines —
/// and it is why the list is written as data and every floor below is a lower bound rather than an
/// exact count: the file it watches will keep growing. It carried 7 319 until the module ceiling
/// moved the measurement bench's injected gestures out into `src/injected.rs`, which is the next
/// entry down.
const SOURCES: &[(&str, &str, usize)] = &[
    (
        "src/viewer/origin.rs",
        include_str!("../src/viewer/origin.rs"),
        0,
    ),
    (
        "src/viewer/delivery.rs",
        include_str!("../src/viewer/delivery.rs"),
        0,
    ),
    (
        "src/viewer/diff.rs",
        include_str!("../src/viewer/diff.rs"),
        0,
    ),
    (
        "src/viewer/review.rs",
        include_str!("../src/viewer/review.rs"),
        0,
    ),
    (
        "src/viewer/review/message.rs",
        include_str!("../src/viewer/review/message.rs"),
        0,
    ),
    (
        "src/links/local.rs",
        include_str!("../src/links/local.rs"),
        0,
    ),
    ("src/viewer.rs", include_str!("../src/viewer.rs"), 0),
    (
        "src/viewer/load.rs",
        include_str!("../src/viewer/load.rs"),
        0,
    ),
    (
        "src/viewer/markdown.rs",
        include_str!("../src/viewer/markdown.rs"),
        0,
    ),
    (
        "src/viewer/tests.rs",
        include_str!("../src/viewer/tests.rs"),
        0,
    ),
    (
        "src/viewer/view.rs",
        include_str!("../src/viewer/view.rs"),
        0,
    ),
    (
        "src/viewer/interaction_tests.rs",
        include_str!("../src/viewer/interaction_tests.rs"),
        0,
    ),
    (
        "src/painter/pane.rs",
        include_str!("../src/painter/pane.rs"),
        129,
    ),
    ("src/ligatures.rs", include_str!("../src/ligatures.rs"), 6),
    (
        "src/ligature_tests.rs",
        include_str!("../src/ligature_tests.rs"),
        0,
    ),
    (
        "src/fonts/ligatures.rs",
        include_str!("../src/fonts/ligatures.rs"),
        4,
    ),
    (
        "src/painter/ligatures.rs",
        include_str!("../src/painter/ligatures.rs"),
        5,
    ),
    (
        "src/painter/ligatures/tests.rs",
        include_str!("../src/painter/ligatures/tests.rs"),
        2,
    ),
    (
        "src/console_input.rs",
        include_str!("../src/console_input.rs"),
        1,
    ),
    ("src/summary.rs", include_str!("../src/summary.rs"), 1),
    (
        "src/summary/input.rs",
        include_str!("../src/summary/input.rs"),
        1,
    ),
    (
        "src/summary/model.rs",
        include_str!("../src/summary/model.rs"),
        1,
    ),
    (
        "src/summary/view.rs",
        include_str!("../src/summary/view.rs"),
        1,
    ),
    (
        "src/summary/probe.rs",
        include_str!("../src/summary/probe.rs"),
        1,
    ),
    (
        "src/panel/summary.rs",
        include_str!("../src/panel/summary.rs"),
        1,
    ),
    (
        "src/measure/overview.rs",
        include_str!("../src/measure/overview.rs"),
        1,
    ),
    (
        "src/tests/overview.rs",
        include_str!("../src/tests/overview.rs"),
        1,
    ),
    (
        "src/settings/editor/view/footer.rs",
        include_str!("../src/settings/editor/view/footer.rs"),
        1,
    ),
    (
        "src/settings/editor/view/typography.rs",
        include_str!("../src/settings/editor/view/typography.rs"),
        1,
    ),
    (
        "src/settings/editor/view/tests/usability.rs",
        include_str!("../src/settings/editor/view/tests/usability.rs"),
        1,
    ),
    (
        "src/settings/editor/view/tests/layout.rs",
        include_str!("../src/settings/editor/view/tests/layout.rs"),
        1,
    ),
    (
        "src/tab_bar/interaction.rs",
        include_str!("../src/tab_bar/interaction.rs"),
        1,
    ),
    (
        "src/tests/integrated_chrome.rs",
        include_str!("../src/tests/integrated_chrome.rs"),
        1,
    ),
    (
        "src/tests/pane_rename.rs",
        include_str!("../src/tests/pane_rename.rs"),
        1,
    ),
    (
        "src/chrome/branding.rs",
        include_str!("../src/chrome/branding.rs"),
        1,
    ),
    ("src/chrome.rs", include_str!("../src/chrome.rs"), 1),
    (
        "src/header/integrated.rs",
        include_str!("../src/header/integrated.rs"),
        1,
    ),
    ("src/pane_view.rs", include_str!("../src/pane_view.rs"), 1),
    ("src/visibility.rs", include_str!("../src/visibility.rs"), 1),
    (
        "src/settings/editor/probe.rs",
        include_str!("../src/settings/editor/probe.rs"),
        1,
    ),
    (
        "src/settings/editor/view/tests.rs",
        include_str!("../src/settings/editor/view/tests.rs"),
        1,
    ),
    (
        "src/settings/editor/view/colors.rs",
        include_str!("../src/settings/editor/view/colors.rs"),
        1,
    ),
    (
        "src/settings/editor/view/appearance.rs",
        include_str!("../src/settings/editor/view/appearance.rs"),
        1,
    ),
    (
        "src/tests/settings_chrome.rs",
        include_str!("../src/tests/settings_chrome.rs"),
        1,
    ),
    (
        "src/tab_bar/chrome.rs",
        include_str!("../src/tab_bar/chrome.rs"),
        3,
    ),
    (
        "src/panel/cache.rs",
        include_str!("../src/panel/cache.rs"),
        5,
    ),
    (
        "src/settings/editor/runtime.rs",
        include_str!("../src/settings/editor/runtime.rs"),
        7,
    ),
    (
        "src/fonts/selection.rs",
        include_str!("../src/fonts/selection.rs"),
        42,
    ),
    (
        "src/settings/editor/script.rs",
        include_str!("../src/settings/editor/script.rs"),
        2,
    ),
    (
        "src/tests/settings_editor.rs",
        include_str!("../src/tests/settings_editor.rs"),
        1,
    ),
    (
        "src/fonts/startup.rs",
        include_str!("../src/fonts/startup.rs"),
        15,
    ),
    (
        "src/settings/editor.rs",
        include_str!("../src/settings/editor.rs"),
        3,
    ),
    (
        "src/settings/editor/view.rs",
        include_str!("../src/settings/editor/view.rs"),
        1,
    ),
    (
        "src/settings/editor/timing.rs",
        include_str!("../src/settings/editor/timing.rs"),
        1,
    ),
    (
        "src/settings/document.rs",
        include_str!("../src/settings/document.rs"),
        3,
    ),
    (
        "src/settings/store.rs",
        include_str!("../src/settings/store.rs"),
        2,
    ),
    ("src/lib.rs", include_str!("../src/lib.rs"), 6_207),
    ("src/alarms.rs", include_str!("../src/alarms.rs"), 133),
    ("src/closing.rs", include_str!("../src/closing.rs"), 115),
    ("src/control.rs", include_str!("../src/control.rs"), 362),
    (
        "src/control/retiring.rs",
        include_str!("../src/control/retiring.rs"),
        1,
    ),
    (
        "src/panel/driving.rs",
        include_str!("../src/panel/driving.rs"),
        114,
    ),
    (
        "src/panel/pane.rs",
        include_str!("../src/panel/pane.rs"),
        333,
    ),
    ("src/injected.rs", include_str!("../src/injected.rs"), 330),
    (
        "src/injected/links.rs",
        include_str!("../src/injected/links.rs"),
        11,
    ),
    (
        "src/injected/search.rs",
        include_str!("../src/injected/search.rs"),
        13,
    ),
    ("src/links.rs", include_str!("../src/links.rs"), 46),
    (
        "src/links/console.rs",
        include_str!("../src/links/console.rs"),
        29,
    ),
    ("src/panel.rs", include_str!("../src/panel.rs"), 1_067),
    (
        "src/panel/document.rs",
        include_str!("../src/panel/document.rs"),
        9,
    ),
    (
        "src/panel/buttons.rs",
        include_str!("../src/panel/buttons.rs"),
        24,
    ),
    (
        "src/panel/links.rs",
        include_str!("../src/panel/links.rs"),
        40,
    ),
    (
        "src/panel/color.rs",
        include_str!("../src/panel/color.rs"),
        50,
    ),
    ("src/measure.rs", include_str!("../src/measure.rs"), 780),
    (
        "src/measure/header.rs",
        include_str!("../src/measure/header.rs"),
        579,
    ),
    ("src/session.rs", include_str!("../src/session.rs"), 737),
    ("src/fonts.rs", include_str!("../src/fonts.rs"), 834),
    (
        "src/fonts/atlas.rs",
        include_str!("../src/fonts/atlas.rs"),
        135,
    ),
    ("src/header.rs", include_str!("../src/header.rs"), 224),
    (
        "src/pointer/header.rs",
        include_str!("../src/pointer/header.rs"),
        1,
    ),
    (
        "src/header/actions.rs",
        include_str!("../src/header/actions.rs"),
        1,
    ),
    (
        "src/header/controls.rs",
        include_str!("../src/header/controls.rs"),
        1,
    ),
    (
        "src/header/menu.rs",
        include_str!("../src/header/menu.rs"),
        1,
    ),
    (
        "src/header/splitting.rs",
        include_str!("../src/header/splitting.rs"),
        1,
    ),
    (
        "src/tests/header_controls.rs",
        include_str!("../src/tests/header_controls.rs"),
        1,
    ),
    ("src/settings.rs", include_str!("../src/settings.rs"), 599),
    ("src/keyboard.rs", include_str!("../src/keyboard.rs"), 985),
    ("src/painter.rs", include_str!("../src/painter.rs"), 746),
    (
        "src/painter/notices.rs",
        include_str!("../src/painter/notices.rs"),
        222,
    ),
    (
        "src/painter/blocks.rs",
        include_str!("../src/painter/blocks.rs"),
        87,
    ),
    (
        "src/painter/emoji.rs",
        include_str!("../src/painter/emoji.rs"),
        30,
    ),
    (
        "src/pty_output.rs",
        include_str!("../src/pty_output.rs"),
        456,
    ),
    ("src/watcher.rs", include_str!("../src/watcher.rs"), 203),
    ("src/theme.rs", include_str!("../src/theme.rs"), 202),
    (
        "src/theme/catalog.rs",
        include_str!("../src/theme/catalog.rs"),
        1,
    ),
    ("src/mouse.rs", include_str!("../src/mouse.rs"), 201),
    ("src/opening.rs", include_str!("../src/opening.rs"), 148),
    // Pointer documentation moved to pointer/header.rs alongside its behavior.
    ("src/pointer.rs", include_str!("../src/pointer.rs"), 623),
    (
        "src/pointer/links.rs",
        include_str!("../src/pointer/links.rs"),
        28,
    ),
    ("src/report.rs", include_str!("../src/report.rs"), 238),
    ("src/search.rs", include_str!("../src/search.rs"), 48),
    // Three comment lines moved with the tab interaction to its own module.
    ("src/tab_bar.rs", include_str!("../src/tab_bar.rs"), 168),
    (
        "src/tests/markdown.rs",
        include_str!("../src/tests/markdown.rs"),
        26,
    ),
    ("src/tiling.rs", include_str!("../src/tiling.rs"), 276),
    ("src/tree.rs", include_str!("../src/tree.rs"), 512),
    ("src/main.rs", include_str!("../src/main.rs"), 1),
    (
        "examples/fonts.rs",
        include_str!("../examples/fonts.rs"),
        168,
    ),
    (
        "examples/sweep.rs",
        include_str!("../examples/sweep.rs"),
        134,
    ),
    ("examples/leak.rs", include_str!("../examples/leak.rs"), 59),
    (
        "examples/screen_demo.rs",
        include_str!("../examples/screen_demo.rs"),
        34,
    ),
];

/// How many comment lines the walk has to still find, in total and in each file.
///
/// **Without a floor this test passes by being empty**, which is how the guards of this suite die:
/// somebody reshapes the comment detection, the walk finds nothing, every file looks clean and
/// nothing says otherwise. The table above is that per-file floor: each row records what the same
/// [`comment_lines`] walk measured when the row was last paid. It is a lower bound rather than an
/// equality, so adding an explanation stays green and removing one below the recorded count asks
/// the delivery to lower that file's claim explicitly.
///
/// **The total stays beside the table, but it is no longer the first guard to see an ordinary
/// loss.** A current per-file floor sees that immediately. The total is the backstop across valid
/// edits that deliberately lower several rows: their combined cost cannot silently spend the
/// repository-wide floor as well.
///
/// **The floor is a number over the corpus, so a split has to raise it.** The tiling and the tree
/// left `src/lib.rs` carrying 723 comment lines with them, and until they were listed here the
/// corpus read 12 921 against a floor of 12 500 — still green, with 723 lines of argument outside
/// the guard's sight. **A list of files is the one thing a module split silently narrows**, and a
/// floor left where it was is what turns that into silence.
///
/// **And it went unpaid for a second time, which is why the number moved so far in one delivery.**
/// The floor sat at 13 300 against a corpus that had reached 15 692 — two thousand of slack, in a
/// guard whose whole argument is that the slack is what it defends. The delivery that took the
/// keys' walk and the wheel out of `src/lib.rs` is the one that measured it again, and the lesson is
/// the file's own: a floor is re-measured by the delivery that touches the list, not by the one that
/// happens to read the comment.
///
/// **The files are proved separately from their contents.** `include_str!` still makes a renamed or
/// deleted declared file a compile error. [`rust_sources`] walks only to find the opposite mistake:
/// a real file nobody declared. It never supplies the text being judged, so a path cannot disappear
/// from the guard merely because the directory walk did not return it.
// Re-measured at 19,488 by the existing walker after replacing obsolete wall-clock ratio
// explanations with the scoped-clock rationale; the feature's original 19,500 included those.
const COMMENT_LINES_AT_LEAST: usize = 19_488;

/// How many **different** listed words a line needs before it is called Spanish.
///
/// One is unusable and two is not enough margin: `src/settings.rs:1791` cites «pestañas» and
/// «válido» inside an English sentence and scores exactly two. Three is the smallest threshold that
/// leaves that line alone without an exemption of its own — and a real Spanish sentence of comment
/// length clears it easily: the shortest positive case asserted below scores five.
const MARKERS_TO_CALL_IT_SPANISH: usize = 3;

/// The Spanish this milestone keeps **on purpose**, each with the reason it is still Spanish.
///
/// Every one is removed from a line before the line is judged. **Removed, not excused**: exempting
/// the whole line would also excuse a fresh Spanish sentence written beside the citation, and these
/// citations sit in the middle of the densest prose in the crate.
///
/// **One of them earns its place on its own today** — `src/settings.rs:1485` scores 7 markers with
/// the list empty, and [`the_islands_are_what_keeps_the_english_prose_green`] asserts that. It was
/// two until this delivery translated the claim phrases: `src/panel.rs:512` quoted one of them and
/// now scores nothing. The rest are cheap insurance with a real scenario: `partir-al-lado`,
/// `iconos-del-sistema` and `glifos-de-color` carry «al», «lado», «del», «de» and «glifos» between
/// them, so a comment citing three of these names scores five without a Spanish word of its own.
///
/// **`// celda` and `/* celda */` are not here, and that is the answer rather than an oversight.**
/// They were the Spanish comment fixtures of the settings parser; the prose delivery translated them
/// to `# a comment` and `# only comments`, so there is nothing left to exempt.
/// [`no_island_is_stale`] is what makes that a fact instead of a claim: an entry naming text the
/// declared source and example files no longer contain fails, so this list cannot keep exempting
/// what left the tree.
///
/// The whole payload of `examples/screen_demo.rs` — «pantalla de prueba del pintor», «Colores con
/// nombre», the rows of `rojo verde amarillo` — is Spanish and needs no entry: it is **exempt by
/// construction**, because it lives in string literals and this guard reads comment lines. Its
/// literals are kept Spanish because what the demo tests is **their length** in cells against the
/// wide characters beside them, so retyping them in English changes what the screen proves.
const ISLANDS: &[(&str, &str)] = &[
    (
        "partir-al-lado",
        "a retired Spanish action name, kept as a NEGATIVE fixture: the point of the assertion is \
         that the terminal refuses to bind it",
    ),
    ("cerrar-division", "another retired action name"),
    ("pestaña-nueva", "and the third"),
    (
        "iconos-del-sistema",
        "a font setting VALUE somebody types into their settings file; the keys and values are a \
         delivery of their own",
    ),
    ("sistema:", "the prefix of that same setting's value"),
    ("glifos-de-color", "and the other font setting value"),
    (
        "no lleva ninguna pestana",
        "one of three ASCII-folded Spanish sentences that src/settings.rs quotes as the evidence \
         that its own oracle is evadable",
    ),
    ("no tiene texto UTF-8 valido", "the second of those three"),
    ("no cabe", "the third"),
];

/// Common Spanish words, to judge a line of prose.
///
/// **It is the criterion the two sibling guards already use**, taken from `SPANISH_FUNCTION_WORDS`
/// in `src/settings.rs` and widened: that list judges thirteen short on-screen messages, and a
/// corpus of five thousand lines of free prose needs the grammar and the domain nouns as well —
/// «pero», «desde», «aunque», «ventana», «celda», «raton». Words only, compared whole and folded
/// onto ASCII, so `none` is not «no» and `serde` is not «de».
///
/// **Two entries of the sibling list are missing on purpose: «son» and «hay».** They are the two
/// that are plainly English nouns too, and the criterion this project set for such a list is that
/// no entry is also an English word. The sibling could afford them — thirteen short messages, no
/// collision possible — and this one judges English prose, where they are the two that would fire.
/// «min» was tried and dropped for the same reason: it is how English abbreviates a minute.
///
/// «no» and «sin» are absent for the reason the two siblings give: they are English words, and a
/// guard that breaks the build has to be wrong less often than the code it judges.
///
/// The list is closed, and the threshold above is what covers its remaining softness: a handful of
/// entries — «con», «la», «es» — do exist in some corner of English, and one of them alone is never
/// a verdict. Measured over all 18 141 comment lines of the declared source and example files:
/// **zero** English lines reach three markers.
const SPANISH_WORDS: &[&str] = &[
    // ── The function words, from the sibling list in src/settings.rs ──
    "de",
    "del",
    "la",
    "el",
    "los",
    "las",
    "lo",
    "que",
    "con",
    "por",
    "para",
    "una",
    "uno",
    "al",
    "mas",
    "ya",
    "se",
    "es",
    "ninguna",
    "ninguno",
    "lleva",
    "tiene",
    "cabe",
    "texto",
    "textos",
    "valido",
    "invalido",
    "pestana",
    "pestanas",
    "fichero",
    "ficheros",
    "disposicion",
    "disposiciones",
    "guardar",
    "pudo",
    "puede",
    "esta",
    "estan",
    "sido",
    // ── Grammar, which is what carries a sentence and what the sibling list has no room for ──
    "pero",
    "sino",
    "tambien",
    "tampoco",
    "porque",
    "cuando",
    "donde",
    "cual",
    "cuales",
    "quien",
    "quienes",
    "cuanto",
    "cuantos",
    "cuantas",
    "desde",
    "hasta",
    "entre",
    "sobre",
    "segun",
    "aunque",
    "mientras",
    "siempre",
    "nunca",
    "entonces",
    "ahora",
    "antes",
    "despues",
    "aqui",
    "alli",
    "ahi",
    "eso",
    "esto",
    "esa",
    "ese",
    "esas",
    "esos",
    "aquel",
    "aquella",
    "algo",
    "alguien",
    "alguna",
    "algunas",
    "alguno",
    "algunos",
    "otro",
    "otra",
    "otros",
    "otras",
    "mismo",
    "misma",
    "mismos",
    "mismas",
    "todo",
    "toda",
    "todos",
    "todas",
    "cada",
    "muchos",
    "muchas",
    "poco",
    "pocos",
    "pocas",
    // ── Verbs, which no glossary of types was ever going to name ──
    "hace",
    "hacer",
    "hecho",
    "hizo",
    "decir",
    "dicho",
    "dijo",
    "ser",
    "estar",
    "tener",
    "tenia",
    "tenian",
    "habia",
    "habian",
    "eran",
    "fue",
    "fueron",
    "sera",
    "seran",
    "deja",
    "dejar",
    "dejo",
    "pone",
    "poner",
    "puesto",
    "pide",
    "pedir",
    "viene",
    "venir",
    "llega",
    "llegar",
    "llego",
    "queda",
    "quedar",
    "quedo",
    "pasa",
    "pasar",
    "paso",
    "mira",
    "mirar",
    "mirado",
    "escribe",
    "escribir",
    "escrito",
    "sabe",
    "saber",
    "sabido",
    "quiere",
    "querer",
    "debe",
    "suele",
    "ocurre",
    "cuenta",
    "cuentas",
    // ── The nouns this crate's prose is made of ──
    "consola",
    "consolas",
    "divisiones",
    "ventana",
    "ventanas",
    "pantalla",
    "linea",
    "lineas",
    "celda",
    "celdas",
    "fila",
    "filas",
    "columna",
    "columnas",
    "ancho",
    "anchura",
    "altura",
    "raton",
    "teclado",
    "tecla",
    "teclas",
    "cadena",
    "numero",
    "numeros",
    "coste",
    "costes",
    "medida",
    "medidas",
    "informe",
    "informes",
    "prueba",
    "pruebas",
    "guion",
    "hito",
    "hitos",
    "rama",
    "ramas",
    "arbol",
    "arboles",
    "hoja",
    "hojas",
    "nodo",
    "nodos",
    "hueco",
    "huecos",
    "salida",
    "salidas",
    "entrada",
    "entradas",
    "ordenes",
    "marca",
    "marcas",
    "nombre",
    "nombres",
    "campo",
    "campos",
    "clave",
    "claves",
    "valores",
    "lado",
    "lados",
    "borde",
    "bordes",
    "frontera",
    "fronteras",
    "mitad",
    "mitades",
    "gesto",
    "gestos",
    "programa",
    "programas",
    "trabajo",
    "trabajos",
    "usuario",
    "usuarios",
    "cambio",
    "cambios",
    "fuente",
    "fuentes",
    "glifo",
    "glifos",
    "noticias",
];

/// The comment lines of a file, plus the block-comment depth left over at the end.
///
/// Three shapes: a `//` comment — `///` and `//!` included, they are the same two slashes — the
/// line that opens a `/* */`, and **every continuation line of a block comment**, which is the one
/// a naive walk drops and which is where the longest prose in this crate lives.
///
/// **It is a line walk and not a lexer, and the price is stated rather than hidden.** A `//` inside
/// a string literal reads as a comment: there is exactly one in the declared files,
/// `src/settings.rs:1911` asserting `code.starts_with("//")`, and it is English. A `/*` inside a
/// literal would be worse — it would open a comment that never closes and hand the rest of the file
/// over as prose — so the leftover depth comes back with the lines and every file is asserted to
/// end at zero. The sibling `glossary.rs` does own a lexer, and it is 180 lines of one; the whole
/// difference here is that a false positive on an English literal costs nothing while a missed
/// block-comment line costs the guard.
///
/// **Not one of the declared files uses `/* */` today**, and that has a consequence worth writing
/// down: replacing the block-comment arm with `if false` left the corpus at its full count and every
/// floor green — measured, and it was the one mutation of eleven that stayed green.
/// [`the_walk_finds_a_block_comment_and_notices_one_that_never_closes`] is what covers that arm, on
/// a fixture, because the tree cannot.
fn comment_lines(src: &str) -> (Vec<(usize, String)>, usize) {
    let mut found = Vec::new();
    let mut depth = 0usize;
    for (nth, line) in src.lines().enumerate() {
        // Rust block comments nest, so the depth moves by the balance of the line and not by a flag.
        let opens = line.matches("/*").count();
        let closes = line.matches("*/").count();
        // Inside a block comment, or the line that opens one: the **whole** line is prose either
        // way, because a block comment's continuation lines carry no marker of their own.
        if depth > 0 || opens > 0 {
            found.push((nth + 1, line.to_owned()));
        } else if let Some(at) = line.find("//") {
            found.push((nth + 1, line[at..].to_owned()));
        }
        depth = depth + opens - closes.min(depth + opens);
    }
    (found, depth)
}

/// The different listed words a line carries, with the declared Spanish removed first.
fn spanish_markers(line: &str) -> BTreeSet<String> {
    let mut text = fold(line);
    for (island, _) in ISLANDS {
        let island = fold(island);
        while let Some(at) = text.find(&island) {
            // A space and not nothing: gluing the halves together would invent a word that neither
            // side had, and the words are compared whole.
            text.replace_range(at..at + island.len(), " ");
        }
    }
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| SPANISH_WORDS.contains(word))
        .map(str::to_owned)
        .collect()
}

/// A file whose Spanish lives on a **continuation** line, carrying no `//` of its own.
///
/// The shape exists nowhere in the declared files, so the walk's block-comment arm has no subject in
/// the tree and no floor can defend it. This is its subject.
const A_FILE_WITH_A_BLOCK_COMMENT: &str = "fn one() {}\n\
                                           /*\n\
                                           \x20* Aqui se decide el ancho de cada celda.\n\
                                           \x20*/\n\
                                           fn two() {}\n";

/// A file whose Spanish sits **after code on the same line**, the shape `src/theme.rs` writes
/// sixteen of.
///
/// [`REAL_SPANISH`] carries the same shape but hands it straight to the oracle, so the walk never
/// sees it: narrowing the `//` arm to full-line comments would drop every one of these from the
/// corpus, and only the per-file floors of the files that happen to carry them would notice.
const A_FILE_WITH_A_TRAILING_COMMENT: &str =
    "let width = 8; // aqui se decide el ancho de cada celda\nfn two() {}\n";

/// A file where a string literal holds an opening `/*` that nothing closes.
///
/// This is the failure the depth check exists for: from that line on, a walk with a flag hands the
/// rest of the file to the oracle as if it were prose.
const A_FILE_WITH_AN_UNCLOSED_BLOCK: &str = "let opener = \"/*\";\nfn two() {}\n";

#[test]
fn the_walk_finds_a_block_comment_and_notices_one_that_never_closes() {
    let (lines, depth) = comment_lines(A_FILE_WITH_A_BLOCK_COMMENT);
    assert_eq!(depth, 0, "the block comment closes on its own line");
    let numbers: Vec<usize> = lines.iter().map(|(at, _)| *at).collect();
    assert_eq!(
        numbers,
        vec![2, 3, 4],
        "the walk has to take the three lines of the block comment and neither `fn`: it took \
         {numbers:?}"
    );
    // **The middle line is the whole point**: it has no `//` on it, so only the block-comment arm
    // can reach it, and it is the one carrying the Spanish.
    let markers = spanish_markers(&lines[1].1);
    assert!(
        markers.len() >= MARKERS_TO_CALL_IT_SPANISH,
        "the continuation line «{}» is Spanish and scored {markers:?}",
        lines[1].1.trim()
    );

    let (lines, _) = comment_lines(A_FILE_WITH_A_TRAILING_COMMENT);
    assert_eq!(
        lines,
        vec![(1, "// aqui se decide el ancho de cada celda".to_owned())],
        "the walk has to take the comment after the code, from its `//` on, and not the code"
    );
    let markers = spanish_markers(&lines[0].1);
    assert!(
        markers.len() >= MARKERS_TO_CALL_IT_SPANISH,
        "the trailing comment «{}» is Spanish and scored {markers:?}",
        lines[0].1
    );

    let (_, depth) = comment_lines(A_FILE_WITH_AN_UNCLOSED_BLOCK);
    assert_eq!(
        depth, 1,
        "a `/*` inside a string literal has to leave the walk owing a close, or the guard that \
         reports it in a real file can never fire"
    );
}

/// The check every file list of this suite leans on, over lists whose answer is known.
///
/// **It needs its own subject because the tree cannot give it one**: every list matches the tree
/// on a green day, so a check that answered «they match» whatever it was handed left the three
/// lists that use it green — measured. It lives here and not in `common` because a test there would run once
/// per binary that includes it.
#[test]
fn a_list_is_told_apart_from_the_disk_in_both_directions() {
    let on_disk: BTreeSet<String> = ["src/a.rs", "src/b.rs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        list_against_disk("L", &["src/a.rs", "src/b.rs"], &on_disk),
        Ok(())
    );
    for (declared, says) in [
        (&["src/a.rs"][..], "src/b.rs"),
        (&["src/a.rs", "src/b.rs", "src/c.rs"][..], "src/c.rs"),
        (&["src/a.rs", "src/a.rs"][..], "duplicate"),
    ] {
        let failure = list_against_disk("L", declared, &on_disk)
            .expect_err("a list that is not the disk was taken for it");
        assert!(failure.contains(says), "{declared:?}: {failure}");
    }
}

/// The words [`SPANISH_WORDS`] takes from the sibling list are still the sibling list.
///
/// They were copied by hand and nothing tied the copy to its source, so a word the sibling gained
/// was a word this guard would never know about. The sibling's list is private to its test module,
/// so it is read out of the text this file already includes rather than made visible for a test.
#[test]
fn the_sibling_function_words_are_all_here_except_the_two_english_nouns() {
    const LEFT_OUT: &[&str] = &["hay", "son"];
    let settings = SOURCES
        .iter()
        .find(|(name, _, _)| *name == "src/settings.rs")
        .expect("src/settings.rs is one of the declared sources")
        .1;
    let list = settings
        .split_once("const SPANISH_FUNCTION_WORDS: &[&str] = &[")
        .expect(
            "src/settings.rs no longer declares SPANISH_FUNCTION_WORDS where this test reads it",
        )
        .1
        .split_once("];")
        .expect("the sibling list never closes")
        .0;
    let sibling: BTreeSet<&str> = list.split('"').skip(1).step_by(2).collect();
    assert!(
        sibling.len() >= 41,
        "the sibling list yielded {} words and it carries forty-one: this read stopped matching",
        sibling.len()
    );
    for word in LEFT_OUT {
        assert!(
            sibling.contains(word),
            "«{word}» is left out as a sibling word that is also English, and the sibling no longer \
             has it: the exemption is stale"
        );
        assert!(
            !SPANISH_WORDS.contains(word),
            "«{word}» is an English noun too, and this guard judges English prose"
        );
    }
    let missing: Vec<_> = sibling
        .iter()
        .filter(|word| !LEFT_OUT.contains(word) && !SPANISH_WORDS.contains(word))
        .collect();
    assert!(
        missing.is_empty(),
        "the sibling list in src/settings.rs has words SPANISH_WORDS does not: {missing:?}"
    );
}

/// The walk is still finding the prose — **per file and in total**.
#[test]
fn the_walk_still_reads_the_comments_of_every_file() {
    let declared: Vec<&str> = SOURCES.iter().map(|(name, _, _)| *name).collect();
    let on_disk = rust_sources(Path::new(env!("CARGO_MANIFEST_DIR")), &["src", "examples"]);
    if let Err(failure) = list_against_disk("SOURCES", &declared, &on_disk) {
        panic!("{failure}. Without a row a file has no prose floor");
    }

    let mut total = 0usize;
    for (name, src, floor) in SOURCES {
        let (lines, depth) = comment_lines(src);
        assert_eq!(
            depth, 0,
            "{name} ends inside a block comment: a `/*` in a string literal opened one that never \
             closes, and from there this walk is handing code to the oracle as if it were prose"
        );
        assert!(
            lines.len() >= *floor,
            "{name} yielded {} comment lines below its recorded floor of {floor}: either its prose \
             shrank without the row being lowered deliberately, or detection stopped matching for \
             this file alone — which the corpus total can miss",
            lines.len(),
        );
        total += lines.len();
    }
    assert!(
        total >= COMMENT_LINES_AT_LEAST,
        "the walk found {total} comment lines and there are 18 141: the detection stopped matching \
         and this guard is judging almost nothing"
    );
}

/// Sentences that are Spanish beyond argument, and the shape each one is here to defend.
///
/// They are hard-coded rather than read from a file of the crate, and the delivery that translated
/// `src/lib.rs` is the proof it was the right call: that file was the richest Spanish in the tree
/// and a fixture taken from it would have turned the day it was translated into a red suite. There
/// is no Spanish left to read them from now, so hard-coded is the only place they can live.
const REAL_SPANISH: &[&str] = &[
    // Accented, which the folding has to reach.
    "// se pinta a la derecha, según el borde al que te acerques",
    // **Pure ASCII, and this is the case that matters most.** Spanish written without its accents is
    // what a hurried revert produces, and no cheap non-ASCII test sees it at all.
    "/// Lo que cuesta el gesto esta medido, y no es del framework.",
    "//! La ventana abre como la dejaste, con sus pestanas y sus consolas.",
    // A trailing comment, which is the shape `src/theme.rs` writes sixteen of.
    "Color32::from_rgb(0x1E, 0x1E, 0x1E), // el mismo que el fondo de la ventana",
    // A block-comment continuation line, with no `//` anywhere on it.
    "   cada frontera mueve solo a sus dos vecinas, y el arbol no se entera",
];

/// English lines of these very files that quote Spanish, which must stay green.
///
/// Copied from the tree because an invented example proves nothing about it: every one of these is
/// a line somebody actually wrote in these files, and each carries a Spanish citation the oracle
/// has to walk past.
///
/// **The list went from six entries to two, and shrinking is what it is supposed to do.** Four of
/// the six quoted the panel's three claim phrases; this delivery translated those phrases — the
/// contract's own table publishes them in English now — so the lines stopped quoting Spanish and
/// stopped being examples of anything. Sweeping the declared files for a replacement found none:
/// **two** lines are all the tree still has that are English and cite Spanish, and both are in
/// `src/settings.rs`, where the citations are the evidence for its own oracle being evadable. An
/// invented third would be the failure this list was written against.
///
/// **An earlier delivery put a worthless entry here and the floor below did not notice**, which is
/// why that floor is not a count. The line first put here scored **zero** markers with the island
/// list empty, so it could never have failed whatever happened to the oracle, the islands or the
/// word list — a fixture that cannot fail is not a fixture. Both survivors score two or more.
///
/// **And the «copied from the tree» above is asserted now, not promised.** The sibling list has had
/// [`no_island_is_stale`] since it existed; this one had nothing, so a line reworded in
/// `src/settings.rs` — or an invented one typed straight in — left every floor here green while the
/// fixtures described text nobody had written. With two entries left and both in the same file that
/// stopped being a remote possibility: [`the_oracle_answers_what_it_promises`] now looks each one up
/// in [`SOURCES`] before it judges anything.
const ENGLISH_QUOTING_SPANISH: &[&str] = &[
    "/// evadable.** «no lleva ninguna pestana», «no tiene texto UTF-8 valido» and «no cabe» carry not one",
    "/// carries accents —«pestañas», «válido»— and the non-ASCII half catches those on its own; folding",
];

#[test]
fn the_oracle_answers_what_it_promises() {
    // **A floor on the subjects, because without it these loops pass by looking at nothing.** It is
    // the way the guards of this suite die, and the sibling has it on record for its own lists.
    assert!(
        REAL_SPANISH.len() >= 5 && ENGLISH_QUOTING_SPANISH.len() >= 2,
        "the two fixture lists have {} and {} entries: somebody trimmed them and these loops \
         stopped checking the oracle",
        REAL_SPANISH.len(),
        ENGLISH_QUOTING_SPANISH.len()
    );

    // **Every English fixture is a line of these files, and that is checked and not trusted.** The
    // list says it was copied from the tree; without this, rewording the line it was copied from
    // leaves a fixture describing text nobody wrote, and every floor around it green. It is what
    // [`no_island_is_stale`] does for the sibling list.
    for line in ENGLISH_QUOTING_SPANISH {
        assert!(
            SOURCES.iter().any(|(_, src, _)| src.contains(line)),
            "«{line}» is here as a line of these files and no file contains it: it was reworded, or \
             invented, and either way it stopped being evidence about the oracle"
        );
    }

    // **And a floor on what those entries are worth, which a count cannot express.** An entry that
    // scores nothing with the island list empty passes the loop below no matter what the oracle
    // does, so a list of six such entries is a list of none. An earlier delivery put one there and
    // the count said nothing.
    //
    // **It is an equality and not a floor, which is the second lesson of that delivery.** With
    // `>= 2` over a list of two, a third worthless entry is waved through by a count that the two
    // real ones already satisfy — the same hole one size up. Every entry earns its place or this
    // fails. Both of the two clear two markers today, and one of them clears seven.
    let discriminating = ENGLISH_QUOTING_SPANISH
        .iter()
        .filter(|line| {
            fold(line)
                .split(|c: char| !c.is_ascii_alphabetic())
                .filter(|word| SPANISH_WORDS.contains(word))
                .collect::<BTreeSet<_>>()
                .len()
                >= 2
        })
        .count();
    assert_eq!(
        discriminating,
        ENGLISH_QUOTING_SPANISH.len(),
        "{discriminating} of the {} English fixtures carry two Spanish words with the islands \
         empty: the rest cannot fail whatever happens to the oracle, so they have stopped \
         defending the exemptions",
        ENGLISH_QUOTING_SPANISH.len()
    );

    for line in REAL_SPANISH {
        let markers = spanish_markers(line);
        assert!(
            markers.len() >= MARKERS_TO_CALL_IT_SPANISH,
            "«{line}» is Spanish and the oracle scores it {} marker(s) — {markers:?}",
            markers.len()
        );
    }

    for line in ENGLISH_QUOTING_SPANISH {
        let markers = spanish_markers(line);
        assert!(
            markers.len() < MARKERS_TO_CALL_IT_SPANISH,
            "«{line}» is English prose quoting Spanish on purpose and the oracle calls it Spanish \
             with {markers:?}: either an island stopped matching the text as it is written, or the \
             word list grew past what the threshold can carry"
        );
    }

    // **The evasions the header declares, asserted as evasions.** A limit written in prose and
    // nowhere else is a limit that quietly stops being true; if the list ever grows enough to catch
    // one of these, this case is what makes somebody come back and rewrite the paragraph.
    for evasion in ["formato desconocido", "ruta inalcanzable"] {
        let markers = spanish_markers(&format!("// {evasion}"));
        assert!(
            markers.is_empty(),
            "the header declares «{evasion}» as a sentence this oracle cannot see and it now scores \
             {markers:?}: the claim is out of date, not wrong in the good direction — rewrite it"
        );
    }

    // **And the unit the header declares it cannot judge**, in the words it shipped in.
    let mislabelled = "\"this paste has a line of {} characters, and a program that reads lines\"";
    let markers = spanish_markers(mislabelled);
    assert!(
        markers.is_empty(),
        "the header declares a byte count called «characters» as something this oracle cannot see, \
         and «{mislabelled}» now scores {markers:?}: rewrite the paragraph"
    );
}

/// Every declared island is still in the tree, and none of them is Spanish enough to fail on its own.
///
/// **The first half is what keeps this list from rotting into a blanket exemption.** An entry naming
/// text the declared files no longer contain excuses nothing and is read by the next reader as
/// «still Spanish on purpose» — which is how `// celda` and `/* celda */` were nearly kept here
/// after the prose delivery had already translated those fixtures to `# a comment`.
///
/// **The floor went from fourteen to nine over two deliveries, and that is the mechanism working
/// rather than being relaxed.** The panel's silence banner left first: it was on the list only
/// because two integration tests in `src/lib.rs` expected it word for word, and once that file was
/// translated the banner said «no news». Then went the four entries of the claim — the three phrases
/// the panel paints and the shortened form one comment quotes — because `src/panel.rs` now writes
/// them in English and the contract's own table publishes them that way. Every one of the five had
/// to go or this very test would fail on it.
#[test]
fn no_island_is_stale() {
    assert!(
        ISLANDS.len() >= 9,
        "the island list has {} entries: it shrinks only when a delivery translates one of them, \
         and an empty one turns the guard below into a test of nothing",
        ISLANDS.len()
    );
    for (island, why) in ISLANDS {
        let folded = fold(island);
        assert!(
            SOURCES
                .iter()
                .any(|(_, src, _)| fold(src).contains(&folded)),
            "«{island}» is exempted here as «{why}» and no longer appears in any declared source \
             files: it was translated, or reworded, and what is left is a standing permission for \
             Spanish that nothing needs"
        );
        // And the exemption really does neutralise it: a line that is nothing but the island has to
        // come out clean, or the guard would fail on the very citation it is meant to allow.
        let markers = spanish_markers(&format!("// {island}"));
        assert!(
            markers.len() < MARKERS_TO_CALL_IT_SPANISH,
            "a comment citing only «{island}» still scores {markers:?}: the entry does not match \
             the text the way the sources write it"
        );
    }
}

/// The islands are load-bearing **today**, not insurance for later.
///
/// Without this, emptying [`ISLANDS`] and watching the suite stay green would be a possible
/// afternoon: a mechanism nobody can see working is one this project deletes.
///
/// **It sweeps the corpus, and it used to sweep a list of fixtures.** Until this delivery the count
/// came from [`ENGLISH_QUOTING_SPANISH`] — two hand-written strings — so the test named after a
/// property of the declared files never opened one, and emptying [`ISLANDS`] did not fail it. The
/// number it published was arrived at by hand and then written into the prose as if measured. Now
/// the walk the file already owns does the counting, which is also what makes the figure below true
/// rather than asserted.
///
/// **One line is what the sweep returns, and it fell from two with this delivery** —
/// `src/settings.rs:1485`, scoring 7. It used to be that line and `src/panel.rs:512` scoring 3; the
/// second quoted a claim phrase, this delivery translated the three phrases, and it stopped citing
/// anything. The day this reaches zero the message below is the instruction: the islands, or the
/// threshold, should go.
#[test]
fn the_islands_are_what_keeps_the_english_prose_green() {
    // **The oracle with the exemptions switched off**, which is the whole question: how many lines
    // of these files would be called Spanish if nobody had excused their citations.
    let without_islands = |line: &str| -> usize {
        fold(line)
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|word| SPANISH_WORDS.contains(word))
            .collect::<BTreeSet<_>>()
            .len()
    };

    let mut saved = Vec::new();
    for (name, src, _) in SOURCES {
        for (line, text) in comment_lines(src).0 {
            if without_islands(&text) >= MARKERS_TO_CALL_IT_SPANISH
                && spanish_markers(&text).len() < MARKERS_TO_CALL_IT_SPANISH
            {
                saved.push(format!("{name}:{line}"));
            }
        }
    }

    assert!(
        !saved.is_empty(),
        "with the islands removed not one comment line of these files would be reported: the \
         exemptions are no longer doing anything, so either they or the threshold should go"
    );
}

#[test]
fn no_comment_in_the_translated_files_reads_as_spanish() {
    let mut reported = Vec::new();
    let mut examined = 0usize;

    for (name, src, _) in SOURCES {
        let (lines, _) = comment_lines(src);
        examined += lines.len();
        for (line, text) in lines {
            let markers = spanish_markers(&text);
            if markers.len() >= MARKERS_TO_CALL_IT_SPANISH {
                reported.push(format!(
                    "  {name}:{line}: {} — {}",
                    markers.into_iter().collect::<Vec<_>>().join(", "),
                    text.trim()
                ));
            }
        }
    }

    // The same floor as [`the_walk_still_reads_the_comments_of_every_file`], repeated here on
    // purpose: this is the assertion somebody reads as «the prose is English», and it may not be
    // able to say that while having looked at nothing. Its sibling guard could be deleted and this
    // one would go on passing.
    assert!(
        examined >= COMMENT_LINES_AT_LEAST,
        "this guard judged {examined} comment lines and there are 18 141: whatever it reports, it is \
         not reporting on the prose of the declared source and example files"
    );

    assert!(
        reported.is_empty(),
        "{} comment line(s) of files this milestone has already translated read as Spanish. Either \
         the prose was reverted, or a new comment was written in Spanish — the words the oracle \
         found come first on each line:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

// ── The documents, which are the other half of the same milestone ────────────────────────────────

/// Every document of `docs/` and `prototypes/` plus the root README — the set
/// [`every_document_has_a_row_and_an_english_name`] holds against the disk — pulled in **at compile
/// time** for the reason [`SOURCES`] gives.
///
/// **AC5 of the milestone («`docs/` translated, with no file mixing the two languages inside
/// itself») was marked `test` and had no test.** Measured before this list was written: a Spanish
/// paragraph inserted into `docs/02-decisions.md`, `docs/04-panel-contract.md`,
/// `docs/05-roadmap.md` and `docs/08-configuration.md` **at the same time** left the whole suite
/// green, 893 passed. Three of those four are already read by a guard — the glossary annex, the
/// three claim rows, the key table — but every one of those reads an **anchor**, and an anchor says
/// nothing about the language of the file around it.
///
/// The two READMEs are here for the same reason and by the same milestone: they were translated in
/// the same delivery and nothing was left watching them either.
const DOCUMENTS: &[(&str, &str)] = &[
    (
        "docs/01-vision.md",
        include_str!("../../../docs/01-vision.md"),
    ),
    (
        "docs/02-decisions.md",
        include_str!("../../../docs/02-decisions.md"),
    ),
    (
        "docs/03-architecture.md",
        include_str!("../../../docs/03-architecture.md"),
    ),
    (
        "docs/04-panel-contract.md",
        include_str!("../../../docs/04-panel-contract.md"),
    ),
    (
        "docs/05-roadmap.md",
        include_str!("../../../docs/05-roadmap.md"),
    ),
    (
        "docs/06-references.md",
        include_str!("../../../docs/06-references.md"),
    ),
    (
        "docs/07-landscape.md",
        include_str!("../../../docs/07-landscape.md"),
    ),
    (
        "docs/08-configuration.md",
        include_str!("../../../docs/08-configuration.md"),
    ),
    (
        "docs/09-prose-glossary.md",
        include_str!("../../../docs/09-prose-glossary.md"),
    ),
    ("README.md", include_str!("../../../README.md")),
    (
        "prototypes/README.md",
        include_str!("../../../prototypes/README.md"),
    ),
];

/// How many prose lines the walk has to still find, in total and in the thinnest document.
///
/// **Two floors and not one**, the lesson [`COMMENT_LINES_AT_LEAST`] already paid for. Measured as
/// this guard lands: **5 005** prose lines, `docs/02-decisions.md` carrying 3 546 of them and
/// `prototypes/README.md` being the thinnest at **13**.
///
/// The total floor leaves 205 lines of slack, which is more than five of the eleven documents carry
/// — so `docs/01-vision.md` (129) could be emptied and the total would wave it through. The per-file
/// floor is what notices that. The other way round, `docs/07-landscape.md` (599) going silent drops
/// the corpus to 4 406 and the total fires loudly while the per-file one, looking at eleven
/// documents that all still answer, would not. The two floors answer different halves.
const DOCUMENT_LINES_AT_LEAST: usize = 4_800;
const DOCUMENT_LINES_PER_FILE_AT_LEAST: usize = 10;

/// Spanish a document quotes **on purpose**, and why it may.
///
/// The same mechanism as [`ISLANDS`] and with the same ratchet behind it: an entry naming text no
/// document contains fails [`no_document_island_is_stale`], so this list cannot keep excusing what
/// left the tree.
///
/// **It has one entry, and that is the measurement rather than a target.** The bulk of the Spanish
/// these documents quote — the glossary annex's arrow lists, the false-friend table of
/// `docs/09-prose-glossary.md` — is **exempt by construction**, because it lives inside
/// backticks and [`document_prose`] reads what is outside them. Eleven lines were reported before
/// that rule and one after it, and the one left is a bug report quoted word for word in guillemets,
/// where no backtick belongs.
const DOCUMENT_ISLANDS: &[(&str, &str)] = &[(
    "el teclado no llegaba a ninguna consola",
    "a bug quoted word for word in D19, in the words it was reported in: rewriting the quotation in \
     English would be reporting a bug nobody reported",
)];

/// The prose lines of a Markdown document: no fenced block, and no inline code span.
///
/// **The two exclusions are what make one list of islands enough**, and they are structural rather
/// than a list of exceptions — the lesson `examples/screen_demo.rs` teaches on the sibling corpus,
/// where a whole file's Spanish needs no entry because it lives in string literals.
///
/// - A **fenced block** is a configuration file, a `panel.json` or a terminal transcript. Its keys
///   and its output are not prose in any language, and `docs/08-configuration.md` publishes on-screen
///   messages there that `src/settings.rs` already confronts with the code word for word.
/// - An **inline code span** is a name being cited. The glossary annex is four thousand lines of
///   exactly that — a Spanish name, an arrow, the English one that replaced it — and it is a
///   document **about** those names, so its citations are its subject.
///
/// A line with no letter left outside the spans carries no prose and is not counted: it would
/// inflate the floors with blank lines and table rules.
///
/// **The count of fences comes back with the lines, and it is not decoration.** An **unbalanced**
/// fence does not hide a document, it **inverts** it: every block becomes prose and every paragraph
/// becomes a block, so the walk goes on returning a plausible number of lines while judging the
/// wrong half. Measured — opening a stray fence at the top of `docs/03-architecture.md` left every
/// floor green. [`the_walk_still_reads_the_prose_of_every_document`] is what asserts it is even.
fn document_prose(md: &str) -> (Vec<(usize, String)>, usize) {
    let mut found = Vec::new();
    let mut fences = 0usize;
    for (nth, line) in md.lines().enumerate() {
        if line.starts_with("```") {
            fences += 1;
            continue;
        }
        if fences % 2 == 1 {
            continue;
        }
        // The even segments are what lies outside the backticks. A line with an odd backtick — a
        // span this walk cannot see the end of — degrades to dropping its tail, which costs a
        // citation and never a sentence.
        let outside = line.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        if outside.chars().any(char::is_alphabetic) {
            found.push((nth + 1, outside));
        }
    }
    (found, fences)
}

/// The listed words a document line carries, with the declared Spanish removed first.
fn document_markers(line: &str) -> BTreeSet<String> {
    let mut text = fold(line);
    for (island, _) in DOCUMENT_ISLANDS {
        let island = fold(island);
        while let Some(at) = text.find(&island) {
            text.replace_range(at..at + island.len(), " ");
        }
    }
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| SPANISH_WORDS.contains(word))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_walk_still_reads_the_prose_of_every_document() {
    let mut total = 0usize;
    for (name, md) in DOCUMENTS {
        let (lines, fences) = document_prose(md);
        assert_eq!(
            fences % 2,
            0,
            "«{name}» opens {fences} fences and closes one fewer: the walk reads the complement of \
             its prose from there on — every block judged as a sentence and every sentence \
             skipped — and the floors go on passing"
        );
        assert!(
            lines.len() >= DOCUMENT_LINES_PER_FILE_AT_LEAST,
            "«{name}» gave {} prose lines: either it emptied out, or this walk stopped finding its \
             prose and the guard below is judging it clean without having read it",
            lines.len()
        );
        total += lines.len();
    }
    assert!(
        total >= DOCUMENT_LINES_AT_LEAST,
        "the walk found {total} prose lines across the eleven documents and there are 5 005: the \
         detection stopped matching and this guard is judging almost nothing"
    );
}

/// The words the documents' names carried until #150 renamed them, so that none of them comes back.
///
/// **A name is judged by a list and not by the prose oracle**, because a name is one to three
/// words and the oracle needs three markers before it calls anything Spanish. The list is these
/// whole words plus [`SPANISH_WORDS`], compared word by word — «contrato» does not catch
/// «contratos». A document named with a Spanish word on neither list walks through by its name; its
/// body is still judged by [`no_document_mixes_the_two_languages`], because the census below gives
/// it a row, and an English body under a Spanish name passes both. «panorama» is not here: it is an
/// English word too.
const RETIRED_NAME_WORDS: &[&str] = &[
    "decisiones",
    "arquitectura",
    "contrato",
    "referencias",
    "configuracion",
    "glosario",
    "prosa",
    "prototipos",
];

/// The listed Spanish words a file or directory name carries: folded, then split on anything that
/// is not an ASCII letter. One is a verdict here, unlike in prose — the store's names are judged
/// the same way.
fn spanish_words_in_name(name: &str) -> BTreeSet<String> {
    fold(name)
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| RETIRED_NAME_WORDS.contains(word) || SPANISH_WORDS.contains(word))
        .map(str::to_owned)
        .collect()
}

/// The name check is exercised on names chosen for it, not only on a tree that happens to be clean.
#[test]
fn a_name_is_judged_by_its_words() {
    for name in [
        "docs/10-decisiones.md",
        "docs/10-ficheros.md",
        "docs/10-Configuración.md",
        "prototipos",
    ] {
        assert!(
            !spanish_words_in_name(name).is_empty(),
            "«{name}» is named in Spanish and the name check let it through"
        );
    }
    for (name, _) in DOCUMENTS {
        assert!(
            spanish_words_in_name(name).is_empty(),
            "«{name}» is one of the English documents and the name check reports it: {:?}",
            spanish_words_in_name(name)
        );
    }
}

/// The documents under `root` that [`DOCUMENTS`] has to declare: every Markdown file of `docs/` and
/// `prototypes/`, and the README.
fn documents_on_disk(root: &Path) -> BTreeSet<String> {
    let mut found = markdown_below(root, &["docs", "prototypes"]);
    found.insert("README.md".to_owned());
    found
}

/// What under `root` is named in Spanish: any document of [`documents_on_disk`], by both lists,
/// and any directory at the root, by [`RETIRED_NAME_WORDS`] alone.
///
/// **The root's files are not judged, and its directories only against the retired words.** A
/// personal `CLAUDE.md` or `FLOW.md` sits at the root of the main checkout untracked, and so can a
/// personal directory; a guard that fails on somebody's own setup is one that gets switched off.
/// The retired words are the names this repository used and dropped, so a directory carrying one
/// is the old tree coming back — a `prototipos/` beside `prototypes/` — and not somebody's own.
fn names_in_spanish(root: &Path) -> Vec<String> {
    let mut found: Vec<String> = documents_on_disk(root)
        .into_iter()
        .filter(|name| !spanish_words_in_name(name).is_empty())
        .collect();
    for entry in std::fs::read_dir(root)
        .unwrap_or_else(|failure| panic!("cannot read {}: {failure}", root.display()))
    {
        let entry = entry.unwrap_or_else(|failure| panic!("cannot read an entry: {failure}"));
        // `Path::is_dir` follows a symbolic link, so `prototipos -> prototypes` is judged too.
        if !entry.path().is_dir() {
            continue;
        }
        let name = format!("{}/", entry.file_name().to_string_lossy());
        if fold(&name)
            .split(|c: char| !c.is_ascii_alphabetic())
            .any(|word| RETIRED_NAME_WORDS.contains(&word))
        {
            found.push(name);
        }
    }
    found.sort();
    found
}

/// [`names_in_spanish`] over a tree built for it: what it must report, and what it must leave.
#[test]
fn the_names_are_judged_where_the_guard_says() {
    let root = std::env::temp_dir().join(format!("agent-terminal-names-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["docs", "prototypes", "prototipos", "pruebas"] {
        std::fs::create_dir_all(root.join(dir)).expect("create a fixture directory");
    }
    for file in [
        "README.md",
        "docs/02-decisions.md",
        "docs/10-ficheros.MD",
        "prototypes/README.md",
        "prototipos.txt",
    ] {
        std::fs::write(root.join(file), "").expect("write a fixture file");
    }

    let reported = names_in_spanish(&root);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(
        reported,
        ["docs/10-ficheros.MD", "prototipos/"],
        "a document named with a listed word (whatever the case of its extension) and a root \
         directory named with a retired word are reported; a root file, and a root directory \
         named with a word only the general list has, are not"
    );
}

/// **Every document has a row in [`DOCUMENTS`], and nothing [`names_in_spanish`] judges is named
/// in Spanish.**
///
/// Without the census a document with no row was judged by nothing: `docs/DEBT.md` sat outside
/// the language guard until #150 deleted it, with nothing saying so. And
/// `docs/09-prose-glossary.md` was created under a Spanish name *during* the milestone whose point
/// was retiring them, which is the convention still producing instances rather than a leftover.
#[test]
fn every_document_has_a_row_and_an_english_name() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let declared: Vec<&str> = DOCUMENTS.iter().map(|(name, _)| *name).collect();
    if let Err(failure) = list_against_disk("DOCUMENTS", &declared, &documents_on_disk(&root)) {
        panic!(
            "{failure}. Without a row a document's language is judged by nothing — a scratch file \
             counts too, because this walks the disk and not the index"
        );
    }

    let spanish = names_in_spanish(&root);
    assert!(
        spanish.is_empty(),
        "these are named in Spanish, and D27 moved the whole repository to English, names \
         included: {spanish:?}"
    );
}

/// An island naming text no document carries any more is a standing permission nothing needs.
#[test]
fn no_document_island_is_stale() {
    assert!(
        !DOCUMENT_ISLANDS.is_empty(),
        "the document island list is empty: either say so in the prose above and delete the \
         mechanism, or it has been trimmed and the guard below stopped being able to explain itself"
    );
    for (island, why) in DOCUMENT_ISLANDS {
        assert!(
            DOCUMENTS.iter().any(|(_, md)| md.contains(island)),
            "«{island}» is exempted here as «{why}» and no document contains it: it was translated, \
             or reworded, and what is left is a standing permission for Spanish that nothing needs"
        );
        // And the exemption really neutralises it, or the guard would fail on the very citation it
        // is meant to allow.
        let markers = document_markers(island);
        assert!(
            markers.len() < MARKERS_TO_CALL_IT_SPANISH,
            "a line carrying only «{island}» still scores {markers:?}: the entry does not match the \
             text the way the documents write it"
        );
    }
}

/// The islands are load-bearing **today** over this corpus too, not insurance for later.
#[test]
fn the_document_islands_are_what_keeps_the_english_documents_green() {
    let without_islands = |line: &str| -> usize {
        fold(line)
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|word| SPANISH_WORDS.contains(word))
            .collect::<BTreeSet<_>>()
            .len()
    };

    let saved: Vec<String> = DOCUMENTS
        .iter()
        .flat_map(|(name, md)| {
            document_prose(md)
                .0
                .into_iter()
                .filter_map(move |(line, text)| {
                    (without_islands(&text) >= MARKERS_TO_CALL_IT_SPANISH
                        && document_markers(&text).len() < MARKERS_TO_CALL_IT_SPANISH)
                        .then(|| format!("{name}:{line}"))
                })
        })
        .collect();

    assert!(
        !saved.is_empty(),
        "with the islands removed not one line of these documents would be reported: the \
         exemptions are no longer doing anything, so either they or the threshold should go"
    );
}

/// **AC5, as an assertion instead of a promise**: no document mixes the two languages.
///
/// The same oracle, the same word list and the same threshold as
/// [`no_comment_in_the_translated_files_reads_as_spanish`], over the documents instead of the
/// comments — and with the same limits, which are written out in this file's header: a Spanish
/// sentence carrying fewer than three listed words walks through, and a green here means no line
/// carries three, never that the English is good.
#[test]
fn no_document_mixes_the_two_languages() {
    let mut reported = Vec::new();
    let mut examined = 0usize;

    for (name, md) in DOCUMENTS {
        for (line, text) in document_prose(md).0 {
            examined += 1;
            let markers = document_markers(&text);
            if markers.len() >= MARKERS_TO_CALL_IT_SPANISH {
                reported.push(format!(
                    "  {name}:{line}: {} — {}",
                    markers.into_iter().collect::<Vec<_>>().join(", "),
                    text.trim()
                ));
            }
        }
    }

    // The same floor as [`the_walk_still_reads_the_prose_of_every_document`], repeated here for the
    // reason its sibling gives: this is the assertion somebody reads as «the documents are
    // English», and it may not be able to say that while having looked at nothing.
    assert!(
        examined >= DOCUMENT_LINES_AT_LEAST,
        "this guard judged {examined} prose lines and there are 5 005: whatever it reports, it is \
         not reporting on the language of these eleven documents"
    );

    assert!(
        reported.is_empty(),
        "{} line(s) of documents this milestone has already translated read as Spanish. Either the \
         prose was reverted, or new Spanish was written — the words the oracle found come first on \
         each line:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

// ── The prototypes, which no delivery of the milestone named ─────────────────────────────────────

/// The files of `prototypes/` that are not markdown, pulled in at compile time like [`SOURCES`].
///
/// **They were translated after the milestone closed, because nothing here read them.** `SOURCES`
/// walks Rust comments and `DOCUMENTS` walks markdown, so a Python script and an HTML page carried
/// their Spanish past every guard until the tree was swept by hand.
// Per-file floors preserve coverage without imposing the older prototypes' size on the 88-line chrome demo.
const PROTOTYPES: &[(&str, &str, usize)] = &[
    (
        "prototypes/settings-appearance.html",
        include_str!("../../../prototypes/settings-appearance.html"),
        80,
    ),
    (
        "prototypes/chrome-demo/index.html",
        include_str!("../../../prototypes/chrome-demo/index.html"),
        88,
    ),
    (
        "prototypes/flow-panel",
        include_str!("../../../prototypes/flow-panel"),
        500,
    ),
    (
        "prototypes/mockup-terminal.html",
        include_str!("../../../prototypes/mockup-terminal.html"),
        500,
    ),
];

/// The letters Spanish writes and English does not — the accented ones [`fold`] folds, plus the two
/// inverted marks.
const SPANISH_LETTERS: &[char] = &[
    'á', 'é', 'í', 'ó', 'ú', 'ü', 'ñ', '¿', '¡', 'Á', 'É', 'Í', 'Ó', 'Ú', 'Ü', 'Ñ',
];

/// Why a prototype line reads as Spanish, or `None`: the listed words it carries, then its accented
/// letters.
///
/// **Every line is judged, not only the comments, and it takes a second check the siblings refuse.**
/// A prototype's prose is mostly what it paints — labels, sample panels, «2 h ago» — and none of
/// that is a comment, so [`comment_lines`] would read past it. And a label is one or two words: it
/// scores one marker, so the oracle alone never calls it Spanish. The accented letters are the only
/// thing that sees «líneas» or «Esperándote». The module header chose one oracle over two for
/// comments, which are sentences; these lines are not.
///
/// **No islands**: [`ISLANDS`] are justified by lines of `src/`, and would excuse the same text here.
///
/// **What it still cannot see**: a short Spanish label with no accent («Ver salida», «hace 2 h»),
/// asserted below as passing so this line cannot quietly stop being true.
fn prototype_spanish(line: &str) -> Option<String> {
    let words: BTreeSet<String> = fold(line)
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| SPANISH_WORDS.contains(word))
        .map(str::to_owned)
        .collect();
    let letters: String = line
        .chars()
        .filter(|c| SPANISH_LETTERS.contains(c))
        .collect();
    (words.len() >= MARKERS_TO_CALL_IT_SPANISH || !letters.is_empty()).then(|| {
        format!(
            "[{}] [{letters}]",
            words.into_iter().collect::<Vec<_>>().join(" ")
        )
    })
}

#[test]
fn the_prototype_oracle_answers_what_it_promises() {
    for spanish in [
        "# el panel que se abre cuando no hay trabajo",
        r#"out.append(f"{real}/{est} líneas")"#,
        r#"title="Esperándote""#,
        "¿cuota por canal o global?",
        "Pingüino",
    ] {
        assert!(
            prototype_spanish(spanish).is_some(),
            "«{spanish}» is Spanish and the prototype oracle let it through"
        );
    }
    for passes in [
        r#"acts: [["Ver salida", ""]],"#,
        r#"return f"hace {int(secs // 60)} min""#,
        r#"font-family: "JetBrains Mono", Consolas, monospace;"#,
        "# the ▶ marks the running one — ⚠ ● ↗ ─",
    ] {
        assert!(
            prototype_spanish(passes).is_none(),
            "«{passes}» is asserted as passing — English, or the stated limit — and the oracle \
             reported it: either the limit moved, and the doc of `prototype_spanish` has to say so, \
             or it is flagging English"
        );
    }
}

/// Every file below `dir` that [`markdown_below`] does not take, named from the repo root — at any
/// depth, and with the extension compared the way that walk compares it, so a file is judged by one
/// list and never by both or neither.
fn prototypes_on_disk(root: &Path, dir: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for entry in std::fs::read_dir(dir).expect("prototypes/ is readable") {
        let path = entry.expect("a readable entry").path();
        if path.is_dir() {
            found.extend(prototypes_on_disk(root, &path));
        } else if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            let relative = path.strip_prefix(root).expect("below prototypes/");
            found.insert(format!("prototypes/{}", relative.to_string_lossy()));
        }
    }
    found
}

#[test]
fn no_line_of_the_prototypes_reads_as_spanish() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../prototypes");
    let on_disk = prototypes_on_disk(&root, &root);
    let declared: BTreeSet<String> = PROTOTYPES
        .iter()
        .map(|(name, _, _)| (*name).to_owned())
        .collect();
    assert_eq!(
        declared, on_disk,
        "PROTOTYPES and the non-markdown files of prototypes/ disagree: a file nobody declared is \
         read by no language guard"
    );

    let mut reported = Vec::new();
    for (name, src, floor) in PROTOTYPES {
        assert!(
            src.lines().count() >= *floor,
            "«{name}» gave {} lines: either it emptied out, or the walk below judges it clean \
             without having read it",
            src.lines().count()
        );
        for (n, line) in src.lines().enumerate() {
            if let Some(why) = prototype_spanish(line) {
                reported.push(format!("{name}:{}  {why}  {}", n + 1, line.trim()));
            }
        }
    }
    assert!(
        reported.is_empty(),
        "{} line(s) of the prototypes read as Spanish — the listed words, then the accented \
         letters, come first on each line:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

// ── The stored knowledge, which is the last thing this milestone translated ──────────────────────

/// Where the domain-memory store lives, from this test's own file.
const STORE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.domain-memory");

/// How many records the walk has to still find, how much prose in total, and how little in the
/// thinnest of them.
///
/// **Measured as this delivery leaves the tree:** 74 records carrying 2 127 lines of prose, the
/// thinnest being an aspect with 7 and the fattest the store's own manual with 116.
///
/// **The record floor is the exact count and not a margin below it**, which is a stronger claim
/// than [`SOURCES`] can make and it is the store's own rule that allows it: `instructions.md` says a
/// feature removed from the product is **archived, not deleted** — `status: archived`, because
/// «we used to do X and removed it because Y» is worth keeping. So the file count only ever grows,
/// and any drop is somebody deleting knowledge.
///
/// **And there are three floors, not two.** The sum hides what a single record does: with the total
/// at 2 127 against a floor of 2 000, the fattest record in the store could be emptied and the sum
/// would wave it through. That is the same lesson [`COMMENT_LINES_AT_LEAST`] already paid for, one
/// corpus over.
const RECORDS_AT_LEAST: usize = 74;
const STORE_PROSE_AT_LEAST: usize = 2_000;
const RECORD_PROSE_AT_LEAST: usize = 5;

/// How many paths the records cite between them, counting every occurrence and not the distinct set.
///
/// **The first version of this floor was 30 against a real 174**, because it was written from the
/// count of *distinct* paths — 33 — which is a different quantity and a fifth of the right one. It
/// would have let 82 % of the citations stop being checked with nothing failing, which is precisely
/// the shape of guard this file exists to refuse.
const CITED_PATHS_AT_LEAST: usize = 165;

/// How many records still hand over a headline for [`no_records_headline_reads_as_spanish`] to read.
///
/// **Seventy-two and not seventy-four**: `instructions.md` and `primer.md` carry no frontmatter, so
/// they carry no `name:` either, and the two of them are the only records that legitimately yield
/// nothing here.
///
/// **The exact count and not a margin**, for the reason [`RECORDS_AT_LEAST`] gives: the store's own
/// rule archives instead of deleting, so this number only grows. A drop of one is either a record
/// gone or a `name:` that stopped being found — and a headline that stops being found is judged
/// English without having been read, which is the failure this floor exists to refuse.
const HEADLINES_AT_LEAST: usize = 72;

/// The one cited path that is not in the tree, and why that is correct.
///
/// `CLAUDE.md` is in `.gitignore`: it is the operating manual of whoever works here, it lives in
/// their checkout and it reaches no worktree. A record citing it is citing something real that this
/// guard cannot see, so it is named here rather than silently skipped by an extension filter —
/// which is what the first version did, and that filter also skipped the other 32 `.md` citations
/// along with it.
const CITED_BUT_NOT_IN_THE_TREE: &[&str] = &["CLAUDE.md"];

/// Spanish a record quotes **on purpose**, and why it may.
///
/// Same mechanism as [`ISLANDS`] and [`DOCUMENT_ISLANDS`], and the same ratchet: an entry naming
/// text no record contains fails [`no_store_island_is_stale`].
///
/// **Every one of them is in the migration's own records**, and that is not a coincidence: those
/// five files are where this repository writes down what it used to call things. A citation of a
/// retired name is a historical fact, and translating it would be reporting that the thing was
/// called something it was never called.
const STORE_ISLANDS: &[(&str, &str)] = &[(
    "escribir la disposición",
    "one of twelve report literals the bench census table quotes as they were in August 2026, \
     before the measurement script was translated: the report says «writing the layout» today, and \
     the record's whole point is what the old literal did to the guards",
)];

/// The `.md` files of the store, read from disk with their paths.
///
/// **A directory walk here and `include_str!` for the crate's own sources, and the difference is
/// deliberate.** [`SOURCES`] names its files one by one because a guard that resolves paths at run
/// time turns a renamed source into a target it silently skips. That argument does not carry over:
/// the store is data, its files are renamed by the tool that owns them, and a walk **finds** a
/// renamed record instead of missing it. What the walk cannot notice is a record that disappears —
/// so [`RECORDS_AT_LEAST`] is the floor that answers exactly that, and it is why the count is
/// asserted before anything is judged.
fn store_records() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("the store is not readable at {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry of the store").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                let name = path
                    .strip_prefix(std::path::Path::new(STORE))
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                out.push((
                    name,
                    std::fs::read_to_string(&path).expect("a record of the store"),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(std::path::Path::new(STORE), &mut out);
    out.sort();
    out
}

/// A record split into its frontmatter and its body, or all body when it has none.
///
/// `instructions.md` and `primer.md` carry no frontmatter — they are the store's manual and its
/// overview, not records — and treating them as headerless is what keeps them inside the corpus
/// instead of quietly outside it.
fn record_halves(md: &str) -> (&str, &str, usize) {
    match md
        .strip_prefix("---")
        .and_then(|rest| rest.split_once("\n---"))
    {
        // **The two delimiters are already counted, once each.** `front` opens with the newline
        // that follows the first `---`, so its first line is empty and stands for that delimiter;
        // `body` opens with what is left of the closing `---` line, so **its** first line is empty
        // and stands for that one. Adding two on top put every reported line **two** past the real
        // one — checked by seeding a Spanish line at 49 and reading 51 back. Off-by-one here fails
        // nothing, which is exactly why it survived: it only sends whoever reads a report to the
        // wrong line, and that is worse than saying nothing.
        Some((front, body)) => (front, body, front.lines().count()),
        None => ("", md, 0),
    }
}

/// The prose lines of a record, and how many fences it opened.
///
/// The three exclusions are the ones [`document_prose`] makes and for the same reasons, plus one of
/// its own: **the frontmatter is not prose**. It is addresses, checksums and symbol names —
/// `content_hashes` alone is a wall of hexadecimal — and judging it as language would report a sha
/// as Spanish about as often as chance allows.
///
/// **The fence count comes back with the lines for the reason its sibling gives**: an unbalanced
/// fence does not hide a record, it **inverts** it — every block read as prose and every paragraph
/// skipped — and the totals stay plausible throughout.
fn record_prose(md: &str) -> (Vec<(usize, String)>, usize) {
    let (_, body, offset) = record_halves(md);
    let mut found = Vec::new();
    let mut fences = 0usize;
    for (nth, line) in body.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fences += 1;
            continue;
        }
        if fences % 2 == 1 {
            continue;
        }
        let outside = line.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        if outside.chars().any(char::is_alphabetic) {
            found.push((offset + nth + 1, outside));
        }
    }
    (found, fences)
}

/// Every path a record cites under `file_paths:`, keyed off that block and not off an extension.
///
/// **The key and not the suffix**, because `symbols:` is a list of the same shape two lines down: a
/// symbol that happens to look like a module file name would be read as a path and reported red,
/// and a citation of a `.md` document would be skipped for having the wrong suffix. The first
/// version filtered by extension and did both.
fn cited_paths(front: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut inside = false;
    for line in front.lines() {
        // **The dash is found after trimming, not at a fixed indent.** A record re-indented by hand
        // — or by a tool that reflows YAML — writes `- path` at column zero, and matching the exact
        // two spaces would walk past every citation it carries with the floor still satisfied by
        // the rest.
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if inside {
                found.push(rest.trim());
            }
        } else if !line.starts_with(char::is_whitespace) {
            inside = line.starts_with("file_paths:");
        }
    }
    found
}

/// The record's headline: the whole value of `name:`, folded blocks joined back into one sentence.
///
/// **One sentence and not one line per line.** Sixty-eight of the seventy-two are written `name: >-`
/// with the text wrapped across two or three indented lines, and judging those lines apart splits a
/// Spanish sentence into halves that each score below the threshold. Joining is what makes the
/// oracle see what a reader sees.
///
/// The value stops at the next key, found as a line that does not start with whitespace — the same
/// rule [`cited_paths`] uses to leave a block, and for the same reason: it survives a re-indent.
fn record_headline(front: &str) -> String {
    let mut lines = front.lines().skip_while(|line| !line.starts_with("name:"));
    let Some(first) = lines.next().and_then(|line| line.strip_prefix("name:")) else {
        return String::new();
    };
    // `>-`, `>`, `|` and `|-` are the block styles the store writes; what follows on that same line
    // is the whole value in the four records that do not use one.
    let mut headline = first
        .trim()
        .trim_matches(['>', '|', '-', '+'])
        .trim()
        .to_string();
    for line in lines {
        if !line.starts_with(char::is_whitespace) {
            break;
        }
        headline.push(' ');
        headline.push_str(line.trim());
    }
    headline.trim().to_string()
}

/// The headline of a record is prose, and this milestone claims it is English prose.
///
/// **This is the half of the frontmatter [`record_prose`] is right to skip and wrong to lose.** That
/// exclusion is argued on `content_hashes` — a wall of hexadecimal — and the argument holds for the
/// hashes, the ids and the dates. It does not hold for `name:`, which is a full sentence written for
/// a person: it is what the store hands back first when an agent searches it, so a record whose
/// headline is Spanish reads as Spanish at the one moment anybody looks.
///
/// **The hex wall never reached the threshold anyway**, which is why narrowing the exclusion instead
/// of widening it is safe: a sha is split on non-letters, so the only listed words it can spell are
/// those written entirely over `a`–`f` — «de» and «cabe» — and two is below
/// [`MARKERS_TO_CALL_IT_SPANISH`]. Judging `name:` alone keeps even that out of reach.
///
/// **Discarded: judging `tags:` with the same oracle.** A tag is one word on a line of its own, so
/// three different listed words on one line is unreachable and the guard would be a test of nothing;
/// catching a Spanish tag needs a one-marker rule, which is a different claim with a different false
/// positive rate and belongs to whoever writes it.
#[test]
fn no_records_headline_reads_as_spanish() {
    let mut reported = Vec::new();
    let mut read = 0usize;
    for (name, md) in store_records() {
        let headline = record_headline(record_halves(&md).0);
        if headline.is_empty() {
            continue;
        }
        read += 1;
        let markers = record_markers(&headline);
        if markers.len() >= MARKERS_TO_CALL_IT_SPANISH {
            reported.push(format!("  {name}: {headline}"));
        }
    }
    assert!(
        read >= HEADLINES_AT_LEAST,
        "this guard read {read} headlines and the store has {HEADLINES_AT_LEAST}: a record lost its \
         `name:`, or the block style changed and the value is no longer being found — either way \
         what is not read is being called English"
    );
    assert!(
        reported.is_empty(),
        "{} record headline(s) read as Spanish. The headline is what a search of the store returns \
         before anything else, so this is the one line of a record that is read whether or not the \
         record is:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

#[test]
fn the_walk_still_reads_the_prose_of_every_record() {
    let records = store_records();
    assert!(
        records.len() >= RECORDS_AT_LEAST,
        "the store gave {} records and this delivery left {RECORDS_AT_LEAST}: the store's own rule \
         is that a retired entry is archived and not deleted, so a drop is somebody deleting \
         knowledge — or this walk no longer finding it",
        records.len()
    );
    let mut total = 0usize;
    for (name, md) in &records {
        let (lines, fences) = record_prose(md);
        assert_eq!(
            fences % 2,
            0,
            "«{name}» opens {fences} fences and closes one fewer: from there on the walk reads the \
             complement of its prose — every block judged as a sentence and every sentence skipped \
             — and the totals stay plausible"
        );
        assert!(
            lines.len() >= RECORD_PROSE_AT_LEAST,
            "«{name}» gave {} lines of prose: either it emptied out, or the walk stopped finding \
             its body and it is being judged clean without having been read",
            lines.len()
        );
        total += lines.len();
    }
    assert!(
        total >= STORE_PROSE_AT_LEAST,
        "the walk found {total} lines of prose across the store and there are 2 127: the \
         frontmatter split, the fence count or the backtick split stopped matching"
    );
}

#[test]
fn no_record_of_the_store_mixes_the_two_languages() {
    let mut reported = Vec::new();
    let mut examined = 0usize;
    for (name, md) in store_records() {
        let (lines, _) = record_prose(&md);
        examined += lines.len();
        for (line, text) in lines {
            let markers = record_markers(&text);
            if markers.len() >= MARKERS_TO_CALL_IT_SPANISH {
                reported.push(format!("  {name}:{line}: {}", text.trim()));
            }
        }
    }
    // The same floor as its sibling and repeated here for the same reason: this is the assertion
    // somebody reads as «the store is English», and it may not say that while having looked at
    // nothing.
    assert!(
        examined >= STORE_PROSE_AT_LEAST,
        "this guard judged {examined} lines of the store and there are 2 127: whatever it reports, \
         it is not reporting on the stored knowledge"
    );
    assert!(
        reported.is_empty(),
        "{} line(s) of the stored knowledge read as Spanish. The store is the first thing an agent \
         is told to consult, so Spanish coming back here hands the retired vocabulary to whoever \
         comes next as if it were current:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

/// Every fenced block of a record, joined into one string, with the line its fence opened on.
///
/// **Joined and not line by line**, for the reason [`record_headline`] gives: the store's fences are
/// `mermaid` diagrams, and a diagram's line is a three-word label — `B -->|nothing| C[...]` — which
/// no per-line threshold can judge. The block is the sentence here.
///
/// A block left open at the end of the file is still handed over. That is not a second parity check
/// — [`the_walk_still_reads_the_prose_of_every_record`] owns that — it is so an unbalanced fence
/// costs a red assertion rather than a block that quietly stops being read.
fn record_fences(md: &str) -> Vec<(usize, Vec<&str>)> {
    let (_, body, offset) = record_halves(md);
    let mut blocks = Vec::new();
    let mut open: Option<(usize, Vec<&str>)> = None;
    for (nth, line) in body.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            match open.take() {
                Some(block) => blocks.push(block),
                None => open = Some((offset + nth + 1, Vec::new())),
            }
            continue;
        }
        if let Some((_, held)) = open.as_mut() {
            held.push(line);
        }
    }
    blocks.extend(open);
    blocks
}

/// How many fenced blocks the store carries, and how many lines they hold between them.
///
/// **Both, because either alone passes by looking at nothing.** Three blocks of one line each would
/// satisfy a block count; one fat block would satisfy a line count. As this delivery leaves the tree
/// there are three blocks holding twenty-two lines: two `mermaid` diagrams in the records and the
/// record template inside `instructions.md`.
const FENCED_BLOCKS_AT_LEAST: usize = 3;
const FENCED_LINES_AT_LEAST: usize = 22;

/// What a record fences off is prose too when the fence holds a diagram.
///
/// [`record_prose`] skips fenced blocks, and for a corpus of code samples that is right. **The
/// store's fences are not code**: all three hold English sentences a reader reads — «a thread gains a
/// wait», «the thread stops collecting» — and this delivery translated every one of them. Rewriting
/// a diagram back into Spanish passed the six guards without a word.
///
/// **Measured:** the three blocks score **zero** listed words between them as they stand; the same
/// two diagrams written back in Spanish score sixteen. So the threshold that judges prose serves
/// here unchanged — what had to change is the unit it judges, which is the block and not the line.
#[test]
fn no_fenced_block_of_a_record_reads_as_spanish() {
    let mut reported = Vec::new();
    let mut blocks = 0usize;
    let mut lines = 0usize;
    for (name, md) in store_records() {
        for (at, held) in record_fences(&md) {
            blocks += 1;
            lines += held.len();
            let markers = record_markers(&held.join(" "));
            if markers.len() >= MARKERS_TO_CALL_IT_SPANISH {
                reported.push(format!("  {name}:{at}: {markers:?}"));
            }
        }
    }
    assert!(
        blocks >= FENCED_BLOCKS_AT_LEAST && lines >= FENCED_LINES_AT_LEAST,
        "this guard found {blocks} fenced blocks holding {lines} lines, and the store has \
         {FENCED_BLOCKS_AT_LEAST} holding {FENCED_LINES_AT_LEAST}: the fence walk stopped matching, \
         so whatever it reports it is not reporting on the diagrams"
    );
    assert!(
        reported.is_empty(),
        "{} fenced block(s) of the stored knowledge read as Spanish. A diagram is read by whoever \
         opens the record and by nothing else in this file, so Spanish coming back inside a fence \
         is Spanish nobody is told about:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

/// A record's **file name** is prose this milestone translated, and nothing above reads one.
///
/// This delivery renamed sixty-nine files. [`store_records`] walks the directory precisely so a
/// renamed record is found instead of missed — which is right, and which also means a record renamed
/// **back** into Spanish is found, counted towards every floor and judged clean. So is a brand new
/// record that arrives with a Spanish name and English prose: the walk adds it, the count grows and
/// every assertion above is satisfied. Both were tried against the guards as this delivery left
/// them, and both passed.
///
/// **One marker and not [`MARKERS_TO_CALL_IT_SPANISH`], and this is where the number is argued.** A
/// slug is four or five words, so three different listed words in one is rare even in Spanish:
/// `migracion-a-ingles` scores **zero** and `la-cache-se-invalida-por-celdas` scores four, and a
/// threshold of three would wave the first one through. The reason one is affordable here and not in
/// prose is that a file name is **chosen**, not written: it is a closed string of hyphen-separated
/// segments, each compared whole, so there is no sentence for a word like «la» to hide in.
///
/// **Measured, both ways.** Over the seventy-four paths the store carries today — directories
/// included, since the tool derives them from the feature slug and this delivery renamed one of
/// those too — **zero** score a single marker. Over the sixty-nine paths the delivery retired,
/// **sixty-eight** score at least one; the one that escapes is `migracion-a-ingles`, whose three
/// segments are all absent from the list. That is the honest limit of this guard: it catches a
/// Spanish name made of Spanish grammar, not one made only of nouns the list never needed.
///
/// **No exemption list, and that is deliberate.** The islands elsewhere in this file exist because
/// prose has to be able to **quote** a retired name. A file name does not quote anything — whoever
/// names a record picks the words — so the day a record genuinely has to carry one, the exemption
/// gets written then, with the reason, rather than standing open now for nothing.
#[test]
fn no_record_of_the_store_is_named_in_spanish() {
    let records = store_records();
    assert!(
        records.len() >= RECORDS_AT_LEAST,
        "this guard judged {} record names and this delivery left {RECORDS_AT_LEAST}: whatever it \
         reports, it is not reporting on the store's file names",
        records.len()
    );
    let mut reported = Vec::new();
    for (name, _) in &records {
        let listed = spanish_words_in_name(name);
        if !listed.is_empty() {
            reported.push(format!("  {name} → {listed:?}"));
        }
    }
    assert!(
        reported.is_empty(),
        "{} record(s) of the store are filed under a Spanish name. The name is the record's address \
         — it is what `search_knowledge` returns, what a citation points at and what somebody types \
         to open it — so a Spanish one hands the retired vocabulary on regardless of what the prose \
         inside says:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

#[test]
fn no_record_points_at_a_file_that_is_not_there() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut missing = Vec::new();
    let mut checked = 0usize;
    let mut unfronted = Vec::new();
    for (name, md) in store_records() {
        let (front, _, offset) = record_halves(&md);
        // **A record whose frontmatter never closes yields none**, and a guard that shrugs at that
        // walks past every path it cites. The two files that legitimately carry no frontmatter are
        // the store's manual and its primer, and they cite nothing — so anything else without one
        // is a record that lost its head.
        if offset == 0 && !matches!(name.as_str(), "instructions.md" | "primer.md") {
            unfronted.push(name.clone());
        }
        for cited in cited_paths(front) {
            checked += 1;
            if CITED_BUT_NOT_IN_THE_TREE.contains(&cited) {
                continue;
            }
            if !root.join(cited).exists() {
                missing.push(format!("  {name} cites {cited}"));
            }
        }
    }
    assert!(
        unfronted.is_empty(),
        "{} record(s) yielded no frontmatter, so every path they cite went unchecked: their \
         delimiters are unbalanced, or the shape changed — {unfronted:?}",
        unfronted.len()
    );
    assert!(
        checked >= CITED_PATHS_AT_LEAST,
        "this guard checked {checked} cited paths and the store cites 174: the frontmatter changed \
         shape and most of the citations are no longer being verified"
    );
    assert!(
        missing.is_empty(),
        "{} record(s) of the stored knowledge point at a file the tree does not have. The store is \
         consulted before the code is read, so a dead path sends whoever comes next looking for \
         something that was renamed:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

/// No record lists the same thing twice, in any of its frontmatter blocks.
///
/// **This one is not about language, and it is here because a duplicate broke the store outright.**
/// The index the MCP server answers from has a unique key over `(entry, symbol)`, so a record whose
/// `symbols:` names the same thing twice makes `domain-memory reindex` abort with
/// `UNIQUE constraint failed` — and `--fresh` deletes the index **before** it rebuilds, so the store
/// is left with **no index at all** and every lookup answers nothing.
///
/// It happened on the delivery that translated the store: two different Spanish names — «repartir»
/// and «colocar», in guillemets because the tree no longer has either and a backtick here would be
/// a citation of nothing — were repointed at what the code calls them today and both landed on
/// `place`. The
/// suite was green, the guards were green, the PR merged, and the failure only appeared when
/// somebody ran the rebuild.
///
/// **All three list blocks are checked and not just `symbols:`**, because the reason is the shape
/// and not the key: a repeated `file_paths:` entry makes the same guard downstream count a citation
/// twice, and a repeated tag is a lie about what the record is about. Only `symbols:` can abort the
/// rebuild today; the other two are free to check and there is no argument for waiting until one of
/// them acquires a constraint of its own.
#[test]
fn no_record_lists_the_same_thing_twice() {
    let mut reported = Vec::new();
    let mut checked = 0usize;
    for (name, md) in store_records() {
        let (front, _, _) = record_halves(&md);
        let mut block = "";
        let mut seen: Vec<(&str, String)> = Vec::new();
        for line in front.lines() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("- ") {
                if !block.is_empty() {
                    let value = rest.trim().trim_matches('\'').to_owned();
                    checked += 1;
                    if seen.contains(&(block, value.clone())) {
                        reported.push(format!("  {name}: {block} lists «{value}» twice"));
                    } else {
                        seen.push((block, value));
                    }
                }
            } else if !line.starts_with(char::is_whitespace) {
                block = line
                    .split(':')
                    .next()
                    .filter(|key| matches!(*key, "symbols" | "file_paths" | "tags"))
                    .unwrap_or_default();
            }
        }
    }
    // **A floor, because this loop passes by finding nothing to compare.** The three blocks carry
    // 174 paths, 234 symbols and their tags between them; if the walk stops finding the dash, it
    // reports every record unique without having read one.
    assert!(
        checked >= 500,
        "this guard compared {checked} frontmatter entries and the store lists over 500: the walk \
         stopped finding them and no record is being checked"
    );
    assert!(
        reported.is_empty(),
        "{} record(s) list the same thing twice. A repeated `symbols:` entry aborts \
         `domain-memory reindex` on a unique-key violation, and `--fresh` clears the index before \
         it rebuilds — so the store is left with no index and every lookup answers \
         nothing:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

/// Every exemption of [`CITED_BUT_NOT_IN_THE_TREE`] is still cited by some record.
///
/// Without it the list is a standing permission: the day nothing cites `CLAUDE.md` any more, the
/// entry excuses a path that is not there for anybody.
#[test]
fn no_exempted_path_is_stale() {
    let records = store_records();
    for exempt in CITED_BUT_NOT_IN_THE_TREE {
        assert!(
            records
                .iter()
                .any(|(_, md)| cited_paths(record_halves(md).0).contains(exempt)),
            "«{exempt}» is exempted from the path check and no record cites it any more: the \
             exemption excuses nothing and what is left is a hole"
        );
    }
}

/// Every declared island of the store is still quoted by some record, and the list is not empty.
#[test]
fn no_store_island_is_stale() {
    assert!(
        !STORE_ISLANDS.is_empty(),
        "the store's island list is empty: either the last deliberate Spanish citation was \
         translated — in which case say so and delete the mechanism — or somebody emptied the list \
         and turned the guard below into a test of nothing"
    );
    let records = store_records();
    for (island, why) in STORE_ISLANDS {
        let folded = fold(island);
        assert!(
            records.iter().any(|(_, md)| fold(md).contains(&folded)),
            "«{island}» is exempted here as «{why}» and no record quotes it any more: it was \
             translated, and what is left is a standing permission for Spanish that nothing needs"
        );
        let markers = record_markers(island);
        assert!(
            markers.len() < MARKERS_TO_CALL_IT_SPANISH,
            "a line citing only «{island}» still scores {markers:?}: the entry does not match the \
             text the way the records write it"
        );
    }
}

/// The store's islands are load-bearing **today**, the same claim [`ISLANDS`] has to make.
#[test]
fn the_store_islands_are_what_keeps_its_english_green() {
    let without_islands = |line: &str| -> usize {
        fold(line)
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|word| SPANISH_WORDS.contains(word))
            .collect::<BTreeSet<_>>()
            .len()
    };
    let mut saved = Vec::new();
    for (name, md) in store_records() {
        for (line, text) in record_prose(&md).0 {
            if without_islands(&text) >= MARKERS_TO_CALL_IT_SPANISH
                && record_markers(&text).len() < MARKERS_TO_CALL_IT_SPANISH
            {
                saved.push(format!("{name}:{line}"));
            }
        }
    }
    assert!(
        !saved.is_empty(),
        "with the store's islands removed not one of its lines would be reported: the exemption is \
         no longer doing anything, so either it or the threshold should go"
    );
}

/// The listed words a record's line carries, with the declared Spanish removed first.
fn record_markers(line: &str) -> BTreeSet<String> {
    let mut text = fold(line);
    for (island, _) in STORE_ISLANDS {
        let island = fold(island);
        while let Some(at) = text.find(&island) {
            text.replace_range(at..at + island.len(), " ");
        }
    }
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| SPANISH_WORDS.contains(word))
        .map(str::to_owned)
        .collect()
}
