//! The other half of the inverted ratchet: **it fails for prose that is still in Spanish**.
//!
//! Its sibling [`glossary`](../glossary.rs) guards *identifiers* and says so in its own header —
//! «comments, rustdoc and string literals, for the same reason: prose has its own delivery». That
//! delivery has now landed for all seventeen files of this crate, and nothing was left watching it.
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
//! purpose**: comment lines of these seventeen files carry a Spanish literal inside an English
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
//! `is_ascii()`; here **1 850 of the 12 719 comment lines are not pure ASCII** — angle quotes, em
//! dashes, `µs`, Japanese, emoji — so that test reports a seventh of the corpus. The narrower
//! variant — a Spanish accent or `ñ` anywhere in the line — leaves **eight** lines, and it would
//! need a line-by-line exemption for every one of them: five are deliberate Spanish citations
//! («pestañas», «válido» and three of a claim phrase) and **the other three are English sentences
//! about the letter itself**, `src/lib.rs` explaining why a measurement variable is spelled without
//! an eñe. A line-level exemption is a hole: it would also excuse a fresh Spanish sentence written
//! on that same line. One oracle, with its limits stated, was chosen over two with an escape hatch.
//!
//! **What a green here means, exactly**: no comment line in these seventeen files carries three
//! different common Spanish words. It does **not** mean every comment is good English.

use std::collections::BTreeSet;

/// The files whose prose is English, with their text pulled in **at compile time**.
///
/// `include_str!` and not a directory walk, and the sibling records why: reading paths at run time
/// turns a renamed or deleted file into a target that is silently skipped, and a guard with no file
/// to look at reads exactly like a clean one. Measured there — one hyphen turned into an underscore
/// left four tests green. Here the same mistake does not compile.
///
/// **`src/lib.rs` is here, and adding it was the one line the previous delivery promised it would
/// be.** It is more than half the crate's prose on its own — 7 319 of the 12 719 comment lines —
/// and it is why the list is written as data and every floor below is a lower bound rather than an
/// exact count: the file it watches will keep growing.
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
/// nothing says otherwise. Measured as this delivery leaves the tree: **12 719** comment lines, the
/// thinnest file being `src/main.rs` with **one** — it is four lines of `fn main`, and a floor that
/// demanded more of it would have to be a table instead of a number.
///
/// **Two floors and not one over the sum**, the lesson the sibling paid for twice, and adding
/// `src/lib.rs` is what makes it matter: `src/main.rs`, `examples/leak.rs` and
/// `examples/screen_demo.rs` together carry 94 lines, so losing all three leaves 12 625 — and now
/// that one file carries 7 319 of the total, **losing every other file at once still leaves 7 319**
/// and any total worth setting waves it through. The per-file floor is what notices a file whose
/// comments stopped being found.
///
/// **The total floor is 12 500 and not 12 000, and the 500 was measured rather than chosen.** With
/// the corpus at 12 719 a floor of 12 000 leaves 719 lines of slack, which is more than any file of
/// this crate carries bar three — so `src/settings.rs` could be cut from 467 comment lines to one
/// and both floors would still pass. Checked by doing exactly that. At 12 500 the slack is 219 and
/// every file above that size is defended; the four below it — `src/main.rs`, `examples/leak.rs`,
/// `examples/screen_demo.rs` and `src/mouse.rs` — are not, and that is what the per-file floor is
/// for and why it is stated here rather than left to be discovered.
const COMMENT_LINES_AT_LEAST: usize = 12_500;
const COMMENT_LINES_PER_FILE_AT_LEAST: usize = 1;

/// How many **different** listed words a line needs before it is called Spanish.
///
/// One is unusable and two is not enough margin: `src/settings.rs:1663` cites «pestañas» and
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
/// Two of them earn their place on their own today — `src/panel.rs:512` and `src/settings.rs:1358`
/// score 3 and 7 markers with the list empty, and [`the_islands_are_what_keeps_the_english_prose_green`]
/// asserts that. The rest are cheap insurance with a real scenario: `partir-al-lado`,
/// `iconos-del-sistema` and `glifos-de-color` carry «al», «lado», «del», «de» and «glifos» between
/// them, so a comment citing three of these names scores five without a Spanish word of its own.
///
/// **`// celda` and `/* celda */` are not here, and that is the answer rather than an oversight.**
/// They were the Spanish comment fixtures of the settings parser; the prose delivery translated them
/// to `# a comment` and `# only comments`, so there is nothing left to exempt.
/// [`no_island_is_stale`] is what makes that a fact instead of a claim: an entry naming text the
/// seventeen files no longer contain fails, so this list cannot keep exempting what left the tree.
///
/// The whole payload of `examples/screen_demo.rs` — «pantalla de prueba del pintor», «Colores con
/// nombre», the rows of `rojo verde amarillo` — is Spanish and needs no entry: it is **exempt by
/// construction**, because it lives in string literals and this guard reads comment lines. Its
/// literals are kept Spanish because what the demo tests is **their length** in cells against the
/// wide characters beside them, so retyping them in English changes what the screen proves.
const ISLANDS: &[(&str, &str)] = &[
    (
        "te espera a ti",
        "a claim phrase of the contract, compared by equality against docs/04-contrato-panel.md by \
         a guard whose own translation ships later",
    ),
    (
        "atascada por algo que no eres tú",
        "the second claim phrase, same reason",
    ),
    (
        "por algo que **no** eres tú",
        "how src/panel.rs quotes that phrase shortened, bold markers and all: the doc explaining \
         why the three phrases may not be shortened has to be able to show one",
    ),
    (
        "terminó y nadie lo ha mirado",
        "the third claim phrase, same reason",
    ),
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
/// a verdict. Measured over all 12 719 comment lines of the seventeen files: **zero** English lines
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
/// a string literal reads as a comment: there is exactly one in the seventeen files,
/// `src/settings.rs:1773` asserting `code.starts_with("//")`, and it is English. A `/*` inside a
/// literal would be worse — it would open a comment that never closes and hand the rest of the file
/// over as prose — so the leftover depth comes back with the lines and every file is asserted to
/// end at zero. The sibling `glossary.rs` does own a lexer, and it is 180 lines of one; the whole
/// difference here is that a false positive on an English literal costs nothing while a missed
/// block-comment line costs the guard.
///
/// **Not one of the seventeen files uses `/* */` today**, and that has a consequence worth writing
/// down: replacing the block-comment arm with `if false` leaves the corpus at 12 719 lines and every
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
/// The shape exists nowhere in the seventeen files, so the walk's block-comment arm has no subject in
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
        "the walk found {total} comment lines and there are 12 719: the detection stopped matching \
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
/// has to walk past. **One of the six changed with this delivery**: the entry copied from the
/// warning the panel paints when nobody publishes quoted «sin noticias desde hace 5 min», and that
/// sentence is English now, so the line stopped being an example of anything.
///
/// **Its first replacement was worthless and the floor below did not notice**, which is the reason
/// that floor is no longer a count. The line first put here scores **zero** markers with the island
/// list empty, so it could never have failed whatever happened to the oracle, the islands or the
/// word list — a fixture that cannot fail is not a fixture. It was swapped for a real line that
/// scores two.
const ENGLISH_QUOTING_SPANISH: &[&str] = &[
    "/// is exactly what separates it from the second, «por algo que **no** eres tú». Shortening them",
    "/// evadable.** «no lleva ninguna pestana», «no tiene texto UTF-8 valido» and «no cabe» carry not one",
    "// the silence does not belie its «terminó y nadie lo ha mirado»: it is its normal state.",
    "/// definition, so the silence does not belie its «terminó y nadie lo ha mirado»: it is its",
    "/// carries accents —«pestañas», «válido»— and the non-ASCII half catches those on its own; folding",
    "/// The three phrases are **the contract's, word for word**, including the «a ti» of the first — which",
];

#[test]
fn the_oracle_answers_what_it_promises() {
    // **A floor on the subjects, because without it these loops pass by looking at nothing.** It is
    // the way the guards of this suite die, and the sibling has it on record for its own lists.
    assert!(
        REAL_SPANISH.len() >= 5 && ENGLISH_QUOTING_SPANISH.len() >= 6,
        "the two fixture lists have {} and {} entries: somebody trimmed them and these loops \
         stopped checking the oracle",
        REAL_SPANISH.len(),
        ENGLISH_QUOTING_SPANISH.len()
    );

    // **And a floor on what those entries are worth, which a count cannot express.** An entry that
    // scores nothing with the island list empty passes the loop below no matter what the oracle
    // does, so a list of six such entries is a list of none. This delivery put one there and the
    // count said nothing. Five of the six clear two markers today.
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
    assert!(
        discriminating >= 4,
        "only {discriminating} of the English fixtures carry two Spanish words with the islands \
         empty: the rest cannot fail whatever happens to the oracle, so this list has stopped \
         defending the exemptions"
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
/// text the seventeen files no longer contain excuses nothing and is read by the next reader as
/// «still Spanish on purpose» — which is how `// celda` and `/* celda */` were nearly kept here
/// after the prose delivery had already translated those fixtures to `# a comment`.
///
/// **The floor went from fourteen to thirteen with this delivery, and that is the mechanism
/// working rather than being relaxed.** The panel's silence banner was on the list only because two
/// integration tests in `src/lib.rs` expected it word for word; that file is translated now, the
/// banner says «no news», and the entry had to go or this very test would fail on it.
#[test]
fn no_island_is_stale() {
    assert!(
        ISLANDS.len() >= 13,
        "the island list has {} entries: it shrinks only when a delivery translates one of them, \
         and an empty one turns the guard below into a test of nothing",
        ISLANDS.len()
    );
    for (island, why) in ISLANDS {
        let folded = fold(island);
        assert!(
            SOURCES.iter().any(|(_, src)| fold(src).contains(&folded)),
            "«{island}» is exempted here as «{why}» and no longer appears in any of the seventeen \
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
/// afternoon: a mechanism nobody can see working is one this project deletes. Measured —
/// `src/panel.rs:512` scores 3 and `src/settings.rs:1358` scores 7 with the list empty.
#[test]
fn the_islands_are_what_keeps_the_english_prose_green() {
    let raw = |line: &str| -> usize {
        fold(line)
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|word| SPANISH_WORDS.contains(word))
            .collect::<BTreeSet<_>>()
            .len()
    };
    let would_fail = ENGLISH_QUOTING_SPANISH
        .iter()
        .filter(|line| raw(line) >= MARKERS_TO_CALL_IT_SPANISH)
        .count();
    assert!(
        would_fail >= 2,
        "with the islands removed only {would_fail} of the English lines this crate writes would be \
         reported: the exemptions are no longer doing anything, so either they or the threshold \
         should go"
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
        "this guard judged {examined} comment lines and there are 12 719: whatever it reports, it is \
         not reporting on the prose of these seventeen files"
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
