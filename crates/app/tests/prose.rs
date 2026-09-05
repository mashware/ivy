//! The other half of the inverted ratchet: **it fails for prose that is still in Spanish**.
//!
//! Its sibling [`glossary`](../glossary.rs) guards *identifiers* and says so in its own header —
//! «comments, rustdoc and string literals, for the same reason: prose has its own delivery». That
//! delivery has now landed for all eighteen files of this crate, and nothing was left watching it.
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
//! purpose**: comment lines of these eighteen files carry a Spanish literal inside an English
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
//! `is_ascii()`; here **1 851 of the 12 734 comment lines are not pure ASCII** — angle quotes, em
//! dashes, `µs`, Japanese, emoji — so that test reports a seventh of the corpus. The narrower
//! variant — a Spanish accent or `ñ` anywhere in the line — leaves **five** lines, and it would
//! need a line-by-line exemption for every one of them: two are deliberate Spanish citations
//! («pestañas» and «válido», both in `src/settings.rs`) and **the other three are English sentences
//! about the letter itself**, `src/lib.rs` explaining why a measurement variable is spelled without
//! an eñe. It was eight until this delivery translated the panel's three claim phrases. A line-level exemption is a hole: it would also excuse a fresh Spanish sentence written
//! on that same line. One oracle, with its limits stated, was chosen over two with an escape hatch.
//!
//! **What a green here means, exactly**: no comment line in these eighteen files carries three
//! different common Spanish words. It does **not** mean every comment is good English.

use std::collections::BTreeSet;

/// The files whose prose is English, with their text pulled in **at compile time**.
///
/// `include_str!` and not a directory walk, and the sibling records why: reading paths at run time
/// turns a renamed or deleted file into a target that is silently skipped, and a guard with no file
/// to look at reads exactly like a clean one. Measured there — one hyphen turned into an underscore
/// left four tests green. Here the same mistake does not compile.
///
/// **`src/lib.rs` is here, and adding it was the one line an earlier delivery promised it would
/// be.** It is more than half the crate's prose on its own — 7 043 of the 12 734 comment lines —
/// and it is why the list is written as data and every floor below is a lower bound rather than an
/// exact count: the file it watches will keep growing. It carried 7 319 until the module ceiling
/// moved the measurement bench's injected gestures out into `src/injected.rs`, which is the next
/// entry down.
const SOURCES: &[(&str, &str)] = &[
    ("src/lib.rs", include_str!("../src/lib.rs")),
    ("src/injected.rs", include_str!("../src/injected.rs")),
    ("src/panel.rs", include_str!("../src/panel.rs")),
    ("src/measure.rs", include_str!("../src/measure.rs")),
    ("src/session.rs", include_str!("../src/session.rs")),
    ("src/fonts.rs", include_str!("../src/fonts.rs")),
    ("src/settings.rs", include_str!("../src/settings.rs")),
    ("src/keyboard.rs", include_str!("../src/keyboard.rs")),
    ("src/painter.rs", include_str!("../src/painter.rs")),
    ("src/pty_output.rs", include_str!("../src/pty_output.rs")),
    ("src/watcher.rs", include_str!("../src/watcher.rs")),
    ("src/theme.rs", include_str!("../src/theme.rs")),
    ("src/mouse.rs", include_str!("../src/mouse.rs")),
    ("src/main.rs", include_str!("../src/main.rs")),
    ("examples/fonts.rs", include_str!("../examples/fonts.rs")),
    ("examples/sweep.rs", include_str!("../examples/sweep.rs")),
    ("examples/leak.rs", include_str!("../examples/leak.rs")),
    (
        "examples/screen_demo.rs",
        include_str!("../examples/screen_demo.rs"),
    ),
];

/// How many comment lines the walk has to still find, in total and in the thinnest file.
///
/// **Without a floor this test passes by being empty**, which is how the guards of this suite die:
/// somebody reshapes the comment detection, the walk finds nothing, every file looks clean and
/// nothing says otherwise. Measured as this delivery leaves the tree: **12 734** comment lines, the
/// thinnest file being `src/main.rs` with **one** — it is four lines of `fn main`, and a floor that
/// demanded more of it would have to be a table instead of a number.
///
/// **Two floors and not one over the sum**, the lesson the sibling paid for twice, and adding
/// `src/lib.rs` is what makes it matter: `src/main.rs`, `examples/leak.rs` and
/// `examples/screen_demo.rs` together carry 94 lines, so losing all three leaves 12 640 — above the
/// total floor, and reported by nothing but the per-file one. That is the case the second floor is
/// for. **What the total floor does still catch is the big one**: `src/lib.rs` carries 7 043 of the
/// 12 734, so losing it drops the corpus to 5 691 and the total fires loudly. The two floors answer
/// different halves, which is why there are two.
///
/// **The total floor is 12 500 and not 12 000, and the 500 was measured rather than chosen.** With
/// the corpus at 12 734 a floor of 12 000 leaves 734 lines of slack, which is more than any file of
/// this crate carries bar four — so `src/settings.rs` could be cut from 473 comment lines to one and
/// both floors would still pass. Checked by doing exactly that. At 12 500 the slack is 234 and every
/// file above that size is defended; the **eight** below it are not — `src/main.rs` (1),
/// `examples/screen_demo.rs` (34), `examples/leak.rs` (59), `src/mouse.rs` (94),
/// `examples/sweep.rs` (134), `examples/fonts.rs` (168), `src/theme.rs` (202) and
/// `src/watcher.rs` (203) — and that is what the per-file floor is for and why it is stated here
/// rather than left to be discovered.
const COMMENT_LINES_AT_LEAST: usize = 12_500;
const COMMENT_LINES_PER_FILE_AT_LEAST: usize = 1;

/// How many **different** listed words a line needs before it is called Spanish.
///
/// One is unusable and two is not enough margin: `src/settings.rs:1664` cites «pestañas» and
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
/// **One of them earns its place on its own today** — `src/settings.rs:1358` scores 7 markers with
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
/// eighteen files no longer contain fails, so this list cannot keep exempting what left the tree.
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
/// a verdict. Measured over all 12 734 comment lines of the eighteen files: **zero** English lines
/// reach three markers.
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

/// Lowercase, and Spanish accents folded onto ASCII.
///
/// Both sides get folded — the line and the island — so a citation written «terminó» is found by an
/// entry written the same way and by one written without the accent. The word list is ASCII on
/// purpose for the same reason: real Spanish carries accents and the folding is what makes a single
/// entry cover both spellings.
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// The comment lines of a file, plus the block-comment depth left over at the end.
///
/// Three shapes: a `//` comment — `///` and `//!` included, they are the same two slashes — the
/// line that opens a `/* */`, and **every continuation line of a block comment**, which is the one
/// a naive walk drops and which is where the longest prose in this crate lives.
///
/// **It is a line walk and not a lexer, and the price is stated rather than hidden.** A `//` inside
/// a string literal reads as a comment: there is exactly one in the eighteen files,
/// `src/settings.rs:1778` asserting `code.starts_with("//")`, and it is English. A `/*` inside a
/// literal would be worse — it would open a comment that never closes and hand the rest of the file
/// over as prose — so the leftover depth comes back with the lines and every file is asserted to
/// end at zero. The sibling `glossary.rs` does own a lexer, and it is 180 lines of one; the whole
/// difference here is that a false positive on an English literal costs nothing while a missed
/// block-comment line costs the guard.
///
/// **Not one of the eighteen files uses `/* */` today**, and that has a consequence worth writing
/// down: replacing the block-comment arm with `if false` leaves the corpus at 12 734 lines and every
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
/// The shape exists nowhere in the eighteen files, so the walk's block-comment arm has no subject in
/// the tree and no floor can defend it. This is its subject.
const A_FILE_WITH_A_BLOCK_COMMENT: &str = "fn one() {}\n\
                                           /*\n\
                                           \x20* Aqui se decide el ancho de cada celda.\n\
                                           \x20*/\n\
                                           fn two() {}\n";

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

    let (_, depth) = comment_lines(A_FILE_WITH_AN_UNCLOSED_BLOCK);
    assert_eq!(
        depth, 1,
        "a `/*` inside a string literal has to leave the walk owing a close, or the guard that \
         reports it in a real file can never fire"
    );
}

/// The walk is still finding the prose — **per file and in total**.
#[test]
fn the_walk_still_reads_the_comments_of_every_file() {
    assert!(
        SOURCES.len() >= 18,
        "the list of translated files has {} entries and this delivery left eighteen, which is \
         every file of the crate: it only ever grows, so this is somebody emptying it and every \
         check downstream going quiet",
        SOURCES.len()
    );
    let mut total = 0usize;
    for (name, src) in SOURCES {
        let (lines, depth) = comment_lines(src);
        assert_eq!(
            depth, 0,
            "{name} ends inside a block comment: a `/*` in a string literal opened one that never \
             closes, and from there this walk is handing code to the oracle as if it were prose"
        );
        assert!(
            lines.len() >= COMMENT_LINES_PER_FILE_AT_LEAST,
            "{name} yielded {} comment lines: either its prose is gone, or the detection stopped \
             matching for this file alone — which no total would notice",
            lines.len()
        );
        total += lines.len();
    }
    assert!(
        total >= COMMENT_LINES_AT_LEAST,
        "the walk found {total} comment lines and there are 12 734: the detection stopped matching \
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
/// stopped being examples of anything. Sweeping the eighteen files for a replacement found none:
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
            SOURCES.iter().any(|(_, src)| src.contains(line)),
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
}

/// Every declared island is still in the tree, and none of them is Spanish enough to fail on its own.
///
/// **The first half is what keeps this list from rotting into a blanket exemption.** An entry naming
/// text the eighteen files no longer contain excuses nothing and is read by the next reader as
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
            SOURCES.iter().any(|(_, src)| fold(src).contains(&folded)),
            "«{island}» is exempted here as «{why}» and no longer appears in any of the eighteen \
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
/// property of the eighteen files never opened one, and emptying [`ISLANDS`] did not fail it. The
/// number it published was arrived at by hand and then written into the prose as if measured. Now
/// the walk the file already owns does the counting, which is also what makes the figure below true
/// rather than asserted.
///
/// **One line is what the sweep returns, and it fell from two with this delivery** —
/// `src/settings.rs:1358`, scoring 7. It used to be that line and `src/panel.rs:512` scoring 3; the
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
    for (name, src) in SOURCES {
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

    for (name, src) in SOURCES {
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
        "this guard judged {examined} comment lines and there are 12 734: whatever it reports, it is \
         not reporting on the prose of these eighteen files"
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

/// Every document of the repository, pulled in **at compile time** for the reason [`SOURCES`] gives.
///
/// **AC5 of the milestone («`docs/` translated, with no file mixing the two languages inside
/// itself») was marked `test` and had no test.** Measured before this list was written: a Spanish
/// paragraph inserted into `docs/02-decisiones.md`, `docs/04-contrato-panel.md`,
/// `docs/05-roadmap.md` and `docs/08-configuracion.md` **at the same time** left the whole suite
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
        "docs/02-decisiones.md",
        include_str!("../../../docs/02-decisiones.md"),
    ),
    (
        "docs/03-arquitectura.md",
        include_str!("../../../docs/03-arquitectura.md"),
    ),
    (
        "docs/04-contrato-panel.md",
        include_str!("../../../docs/04-contrato-panel.md"),
    ),
    (
        "docs/05-roadmap.md",
        include_str!("../../../docs/05-roadmap.md"),
    ),
    (
        "docs/06-referencias.md",
        include_str!("../../../docs/06-referencias.md"),
    ),
    (
        "docs/07-panorama.md",
        include_str!("../../../docs/07-panorama.md"),
    ),
    (
        "docs/08-configuracion.md",
        include_str!("../../../docs/08-configuracion.md"),
    ),
    (
        "docs/09-glosario-de-prosa.md",
        include_str!("../../../docs/09-glosario-de-prosa.md"),
    ),
    ("README.md", include_str!("../../../README.md")),
    (
        "prototipos/README.md",
        include_str!("../../../prototipos/README.md"),
    ),
];

/// How many prose lines the walk has to still find, in total and in the thinnest document.
///
/// **Two floors and not one**, the lesson [`COMMENT_LINES_AT_LEAST`] already paid for. Measured as
/// this guard lands: **5 005** prose lines, `docs/02-decisiones.md` carrying 3 546 of them and
/// `prototipos/README.md` being the thinnest at **13**.
///
/// The total floor leaves 205 lines of slack, which is more than five of the eleven documents carry
/// — so `docs/01-vision.md` (129) could be emptied and the total would wave it through. The per-file
/// floor is what notices that. The other way round, `docs/07-panorama.md` (599) going silent drops
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
/// `docs/09-glosario-de-prosa.md` — is **exempt by construction**, because it lives inside
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
///   and its output are not prose in any language, and `docs/08-configuracion.md` publishes on-screen
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
/// wrong half. Measured — opening a stray fence at the top of `docs/03-arquitectura.md` left every
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
